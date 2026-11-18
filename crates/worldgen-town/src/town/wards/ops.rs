//! The ward editor's validators and its requests side: what an edit may do (`parse_*`, `corner_ok`,
//! `merge_ok`…), and reading and changing a town's plan as JSON for agents and the app.

use super::*;

/// Wards that build on lots and so can share a district (`plan_town`'s groups).
pub(super) fn groupable(w: Option<Ward>) -> bool {
    matches!(w, Some(Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Docks | Ward::Military | Ward::Temple))
}

/// A ward's name (`TOWN_WARDS`; `None`: "empty").
pub fn ward_name(w: Option<Ward>) -> &'static str {
    w.map_or("empty", Ward::name)
}

pub(super) fn parse_ward(name: &str) -> Result<Option<Ward>, String> {
    Ok(Some(match name {
        "plaza" => Ward::Plaza,
        "castle" => Ward::Castle,
        "temple" => Ward::Temple,
        "merchant" => Ward::Merchant,
        "craft" => Ward::Craft,
        "noble" => Ward::Noble,
        "common" => Ward::Common,
        "slum" => Ward::Slum,
        "docks" => Ward::Docks,
        "military" => Ward::Military,
        "farm" => Ward::Farm,
        "park" => Ward::Park,
        "empty" => return Ok(None),
        _ => return Err(format!("ward must be one of {}", TOWN_WARDS.join(", "))),
    }))
}

/// A square's kind by name (`SQUARE_KINDS`; "none": no square).
pub(super) fn parse_square(name: &str) -> Result<Option<PlazaKind>, String> {
    if name == "none" {
        return Ok(None);
    }
    super::super::squares::corner_kind(name).map(Some).ok_or_else(|| format!("a square is one of {} or none", SQUARE_KINDS.join(", ")))
}

/// A park's kind by name (`PARK_KINDS`).
pub(super) fn parse_park(name: &str) -> Result<ParkKind, String> {
    ParkKind::parse(name).ok_or_else(|| format!("a park is one of {} (or auto)", PARK_KINDS.join(", ")))
}

pub(super) fn lot_factor(name: &str) -> Result<f64, String> {
    LOT_SIZES.iter().position(|l| *l == name).map(|k| LOT_FACTOR[k]).ok_or_else(|| format!("lots must be one of {}", LOT_SIZES.join(", ")))
}

/// Whether corner `v` (local ft) may be moved from `from` to `to`.
pub(super) fn corner_ok(plan: &TownPlan, v: usize, from: P, to: P) -> Result<(), String> {
    let Some(&at) = plan.mesh.pos.get(v).filter(|_| !plan.vf[v].is_empty()) else { return Err("no such corner".into()) };
    if plan.pinned[v] {
        return Err("it is on the water or a river: it stays put".into());
    }
    if dist(at, from) > TownEdit::SAME_AT_FT {
        return Err("the town was laid out anew: that corner is somewhere else now".into());
    }
    let d = dist(at, to);
    if !d.is_finite() || d > plan.spacing + 0.5 {
        return Err(format!("that is {:.0} ft from where it was laid out: at most {:.0}", d, plan.spacing));
    }
    Ok(())
}

/// Whether corner `v` is one to set a square at (`at`: where it was planned, local ft).
pub(super) fn square_here(plan: &TownPlan, v: usize, at: P) -> Result<(), String> {
    if v >= plan.mesh.pos.len() || plan.vf[v].is_empty() {
        return Err("no such corner".into());
    }
    if dist(plan.mesh.pos[v], at) > TownEdit::SAME_AT_FT {
        return Err("the town was laid out anew: that corner is somewhere else now".into());
    }
    Ok(())
}

/// Why no square opens at corner `v` with these wards.
const NO_SQUARE: &str = "no square opens there: it needs three streets or more meeting between patches built on lots, inside the walls and off the water";

/// Why a close can't be set on a temple's patch.
const NO_CLOSE: &str = "no temple stands on that patch to have a close round it";

/// Why a close can't be set on a temple's patch with a square at a corner.
const SQUARE_THERE: &str = "a square opens at its corner";

/// Why no square opens at a corner of a close.
const AT_CLOSE: &str = "a temple's close is at that corner: its houses face the close, not the streets";

/// Why a garden can't be set on a patch.
const NOT_PARK: &str = "only a park's patch is laid out as a garden: give it the park ward first";
/// Why a patch outside the town's districts (a farm's, open country) takes none.
pub(super) const OUTSIDE_DISTRICT: &str = "a district holds the town's patches only: this one is open country, in no district";

/// Whether patch `p` is in one of the town's districts as generated (in the town or its sprawl),
/// or joins one with the ward `w` it is given by hand (R6 P8: built on, not a farm's,
/// `districts::of`).
pub(super) fn in_a_district(plan: &TownPlan, p: usize, w: Option<Option<Ward>>) -> bool {
    plan.quarters.iter().any(|(_, m)| m.contains(&p)) || w.is_some_and(super::super::districts::built)
}

/// Why a close set on a temple's patch isn't laid out.
const NO_ROOM: &str = "no room for a close there: the temple and churchyard must fit, with houses along at least 65% of the block's streets";

/// The patches of `mesh` (corners as they stand) whose close layout `l` opens (by where its
/// temples' closes stand; `center`: the town's middle, world ft).
fn built_closes(l: &Layout, mesh: &Mesh, center: P) -> Vec<usize> {
    let mut out: Vec<usize> = l
        .plazas
        .iter()
        .filter(|pz| pz.kind == PlazaKind::TempleClose)
        .filter_map(|pz| {
            let c = sub(centroid(&pz.pts), center);
            (0..mesh.faces.len()).find(|&f| mesh.faces[f].len() >= 3 && contains(&mesh.face_pts(f), c))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The patches of `mesh` (corners as they stand) that layout `l` lays out as parks, and how (by
/// where each park's middle stands; `center`: the town's middle, world ft).
fn built_parks(l: &Layout, mesh: &Mesh, center: P) -> BTreeMap<usize, ParkKind> {
    l.parks
        .iter()
        .filter_map(|pk| {
            let c = sub(centroid(&pk.pts), center);
            (0..mesh.faces.len()).find(|&f| mesh.faces[f].len() >= 3 && contains(&mesh.face_pts(f), c)).map(|f| (f, pk.kind))
        })
        .collect()
}

/// Whether the town as generated has a temple on patch `p` (its middle on the patch as planned).
fn has_temple(world: &World, t0: &T0, plan: &TownPlan, index: usize, p: usize) -> bool {
    let Some(fi) = catalog::index_of("temple") else { return false };
    let Some(s) = super::super::settlement(t0, index) else { return false };
    let face: Vec<P> = plan.mesh.face_pts(p).iter().map(|q| add(*q, [s.x, s.y])).collect();
    super::super::generated_layout(world, t0, index).buildings.iter().any(|b| b.func == Some(fi as u16) && contains(&face, centroid(&b.poly)))
}

/// Whether patch `p` is one to set (`at`: its middle as planned, local ft).
pub(super) fn patch_here(plan: &TownPlan, p: usize, at: P) -> Result<(), String> {
    if p >= plan.mesh.faces.len() || plan.mesh.faces[p].len() < 3 || !plan.face_ok[p] {
        return Err("no such patch".into());
    }
    if plan.wet[p] {
        return Err("that patch is on the water".into());
    }
    if dist(centroid(&plan.mesh.face_pts(p)), at) > TownEdit::SAME_AT_FT {
        return Err("the town was laid out anew: that patch is somewhere else now".into());
    }
    Ok(())
}

/// Whether patch `p` can join neighbour `q`'s district (both building on lots, as they will be).
pub(super) fn merge_ok(plan: &TownPlan, ward: &[Option<Ward>], p: usize, q: usize) -> Result<(), String> {
    if p == q {
        return Err("a patch can't merge with itself".into());
    }
    let face = &plan.mesh.faces[p];
    let m = face.len();
    let Some(e) = (0..m).find(|&e| plan.mesh.labels[p][e] == q as i32) else { return Err(format!("patch {q} is not next to patch {p}")) };
    if !groupable(ward[p]) || !groupable(ward[q]) {
        return Err("only wards built on lots merge (merchant, craft, common, noble, slum, docks, military, temple)".into());
    }
    if plan.main_edge(face[e], face[(e + 1) % m]) {
        return Err("a main street runs between them".into());
    }
    if (p < plan.n_inner) != (q < plan.n_inner) {
        return Err("one is inside the town and the other outside it".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Reading and changing a town's plan (agents, the app).

/// A point on a 1/64-ft grid: written to JSON and read back it is the same number (so an edit
/// round-tripped through the app keys the same cached layout).
fn q64(p: P) -> [f64; 2] {
    [crate::core::round(p[0] * 64.0) / 64.0, crate::core::round(p[1] * 64.0) / 64.0]
}

fn round1(p: P) -> [f64; 2] {
    [crate::core::round(p[0] * 10.0) / 10.0, crate::core::round(p[1] * 10.0) / 10.0]
}

/// The town of layout `index` and its plan, or why it has none.
fn town_of<'a>(t0: &'a T0, world: &World, index: usize) -> Result<(&'a crate::t0::settle::Settlement, Rc<TownPlan>), String> {
    let s = super::super::settlement(t0, index).ok_or("not a settlement")?;
    if s.tier < Tier::Town {
        return Err("a village has no wards: it grows along its roads".into());
    }
    let plan = super::super::town_plan(world, t0, index).ok_or("not a town")?;
    Ok((s, plan))
}

/// A town's plan as the ward editor sees it: its patches (corners, ward as planned and as set,
/// lot size, district, neighbours, a temple's close) and corners (where each stands now, where it
/// was planned, pinned, gate, wall, whether a square can open there and the one that does), how far
/// a corner may move, the walls, and what of its edit is set aside.
pub fn plan_json(world: &World, t0: &T0, index: usize) -> Result<Value, String> {
    let (s, plan) = town_of(t0, world, index)?;
    let center = [s.x, s.y];
    let raw = world.file.edits.towns.get(&key(index));
    let mut aside: Vec<String> = Vec::new();
    if let Some(e) = raw
        && dist(e.at, center) > TownEdit::SAME_AT_FT
    {
        aside.push("the town was laid out anew (it moved): the whole edit".into());
    }
    let e = edit_of(world, t0, index);
    let (ch, more) = e.map(|e| changes(&plan, e, center)).unwrap_or_default();
    aside.extend(more);
    let mesh = moved_mesh(&plan, &ch.moves);
    let (ward, group) = ch.wards(&plan);
    // The closes as built (where their temple and churchyard fit), now and as generated.
    let now = super::super::base_layout(world, t0, index);
    let built = built_closes(&now, &mesh, center);
    let generated = built_closes(&super::super::generated_layout(world, t0, index), &plan.mesh, center);
    let parks = built_parks(&now, &mesh, center);
    let (squares, closes) = super::super::squares::spots(&plan, &ch, &ward, &group, &|p| built.contains(&p));
    let world_pt = |p: P| round1(add(p, center));
    let listed = |p: usize| plan.mesh.faces[p].len() >= 3 && plan.face_ok[p] && !plan.wet[p] && (p < plan.n_inner || plan.ward[p].is_some() || ward[p].is_some());
    // (R5 towns: the districts as drawn by hand.)
    let quarters = super::super::districts::of(&plan, &ch);
    let district = super::super::districts::membership(&quarters);
    let edit_for = |p: usize| e.and_then(|e| e.patches.iter().find(|x| x.p as usize == p));
    let mut patches = Vec::new();
    let mut corner_used = vec![false; mesh.pos.len()];
    for p in (0..plan.mesh.faces.len()).filter(|&p| listed(p)) {
        for &v in &mesh.faces[p] {
            corner_used[v] = true;
        }
        let mut v = json!({
            "patch": p,
            "at": world_pt(centroid(&plan.mesh.face_pts(p))),
            "corners": mesh.faces[p],
            "ward": ward_name(ward[p]),
            "in_town": p < plan.n_inner,
            "neighbours": plan.mesh.labels[p].iter().filter(|&&q| q >= 0 && listed(q as usize)).collect::<Vec<_>>(),
        });
        if ward[p] != plan.ward[p] {
            v["ward_generated"] = json!(ward_name(plan.ward[p]));
        }
        if let Some(&qi) = district.get(&p) {
            let id = format!("d:{index}:{qi}");
            v["district"] = json!({ "name": super::super::districts::name_now(world, index, qi, &quarters[qi].0.name), "id": id });
        }
        if group[p] != usize::MAX && group[p] != p {
            v["lots_like"] = json!(group[p]);
        }
        if let Some(pe) = edit_for(p) {
            if let Some(l) = &pe.lots {
                v["lots"] = json!(l);
            }
            if pe.reroll > 0 {
                v["reroll"] = json!(pe.reroll);
            }
            if let Some(q) = pe.merge_with {
                v["merged_with"] = json!(q);
            }
        }
        let (close, planned) = (closes.contains(&p), generated.contains(&p));
        if close {
            v["close"] = json!(true);
        } else if ward[p] == Some(Ward::Temple) {
            // Whether one can be set, and why not.
            if !plan.closes.contains(&p) && !has_temple(world, t0, &plan, index, p) {
                v["close_refused"] = json!(NO_CLOSE);
            } else if mesh.faces[p].iter().any(|u| squares.iter().any(|s| s.0 == *u)) {
                v["close_refused"] = json!(SQUARE_THERE);
            } else if !now.close_room.contains(&(p as u32)) {
                v["close_refused"] = json!(NO_ROOM);
            }
        }
        if close != planned {
            v["close_generated"] = json!(planned);
        }
        // A park: how it is laid out, and whether that was set by hand.
        // (A palace's garden on a patch of another ward is the palace's: `palace::gardens`.)
        if let Some(k) = parks.get(&p).filter(|_| ward[p] == Some(Ward::Park)) {
            v["park"] = json!(k.name());
        }
        if ward[p] == Some(Ward::Park) && ch.park.contains_key(&p) {
            v["park_by_hand"] = json!(true);
        }
        // (R5 towns: a cemetery the town lays beside a road out on an outskirts' patch.)
        if ward[p] != Some(Ward::Park) && parks.get(&p) == Some(&ParkKind::Cemetery) {
            v["cemetery_by_road"] = json!(true);
        }
        if ward[p] != Some(Ward::Park)
            && plan.ward[p] != Some(Ward::Park)
            && let Some(why) = plan.park_refused[p]
        {
            v["park_refused"] = json!(why);
        }
        // (A park's patch whose block came out too small once its streets were cut: built on as a
        // common ward's, and said so.)
        if ward[p] == Some(Ward::Park) && !parks.contains_key(&p) && p < plan.n_inner {
            v["ward"] = json!(ward_name(Some(Ward::Common)));
            v["ward_planned"] = json!(ward_name(Some(Ward::Park)));
            v["park_refused"] = json!(super::super::PARK_SLIVER);
        }
        patches.push(v);
    }
    // (R5 towns, M18b: gates put in or taken out by hand.)
    let gate = |v: usize| {
        let (put, out) = super::super::wallgates::corner_marks(e, v);
        (plan.gates.contains(&v) && !out) || put
    };
    // (R5 towns, M18c: each wall corner's neighbours along the wall, null at an open run's end.)
    let wall: Vec<Option<[Option<usize>; 2]>> = {
        let mut w = vec![None; mesh.pos.len()];
        for (chain, closed, _) in &plan.wall_runs {
            let n = chain.len();
            for (i, &v) in chain.iter().enumerate() {
                let prev = if i > 0 { Some(chain[i - 1]) } else if *closed && n > 2 { Some(chain[n - 1]) } else { None };
                let next = if i + 1 < n { Some(chain[i + 1]) } else if *closed && n > 2 { Some(chain[0]) } else { None };
                w[v] = Some([prev, next]);
            }
        }
        w
    };
    let corners: Vec<Value> = (0..mesh.pos.len())
        .filter(|&v| corner_used[v])
        .map(|v| {
            let mut c = json!({ "corner": v, "at": world_pt(mesh.pos[v]) });
            if dist(mesh.pos[v], plan.mesh.pos[v]) > 0.05 {
                c["planned"] = json!(world_pt(plan.mesh.pos[v]));
            }
            if plan.pinned[v] {
                c["pinned"] = json!(true);
            }
            if gate(v) {
                c["gate"] = json!(true);
            }
            match super::super::wallgates::corner_marks(e, v) {
                (true, _) => c["gate_by_hand"] = json!(true),
                (_, true) => c["gate_removed"] = json!(true),
                _ => {}
            }
            if let Some(nb) = wall[v] {
                c["wall"] = json!(true);
                c["wall_next"] = json!(nb);
                if super::super::wallgates::tower_down(world, t0, index, e, v, world_pt(mesh.pos[v])) {
                    c["tower_removed"] = json!(true);
                }
            }
            if super::super::squares::junction(&plan, &ward, &group, v) {
                if super::super::squares::at_close(&plan, v, &closes) {
                    c["square_refused"] = json!(AT_CLOSE);
                } else {
                    c["junction"] = json!(true);
                }
            }
            let now = squares.iter().find(|s| s.0 == v).map(|s| s.1);
            if let Some(k) = now {
                c["square"] = json!(k.name());
                c["square_ft"] = json!(crate::core::round(super::super::squares::cut_reach(&plan, &mesh, v, k)));
            }
            let planned = plan.squares.iter().find(|s| s.0 == v).map(|s| s.1);
            if now != planned {
                c["square_generated"] = json!(planned.map_or("none", |k| k.name()));
            }
            c
        })
        .collect();
    // (The towers as built along the wall now, gate towers apart: `interior::towers`.)
    let towers = crate::interior::towers(&now);
    let gate_towers = towers.iter().filter(|t| t.2).count();
    let mut walls_json = json!({ "built": ch.walls.unwrap_or(plan.walled), "generated": plan.walled, "towers": towers.len() - gate_towers, "gate_towers": gate_towers, "gates": now.gates.len() });
    if let Some(h) = super::super::wallgates::plan_json(world, t0, index, e) {
        walls_json["by_hand"] = h;
    }
    Ok(json!({
        "layout": index,
        "center": round1(center),
        "max_move_ft": crate::core::round(plan.spacing),
        "walls": walls_json,
        "edited": e.is_some(),
        "patches": patches,
        "corners": corners,
        "districts": quarters.iter().enumerate().map(|(qi, (q, m, touched))| {
            let mut d = json!({ "id": format!("d:{index}:{qi}"), "name": super::super::districts::name_now(world, index, qi, &q.name), "kind": q.kind.name(), "patches": m });
            if qi >= plan.quarters.len() {
                d["by_hand"] = json!(true);
            } else if *touched {
                d["changed"] = json!(true);
            }
            d
        }).collect::<Vec<_>>(),
        "set_aside": aside,
        "places": super::super::places::plan_json(world, t0, index),
    }))
}

/// A change asked of a town (`edit_town`): corners moved one by one (`to`, or `by` from where it
/// stands now), brushes (`equalize` patches toward regular polygons, `relax` corners toward their
/// neighbours round a point), patches set, squares set where streets meet, walls, and parts put
/// back as generated.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TownRequest {
    #[serde(default)]
    pub moves: Vec<MoveRequest>,
    #[serde(default)]
    pub equalize: Vec<u32>,
    #[serde(default)]
    pub relax: Option<RelaxRequest>,
    #[serde(default)]
    pub patches: Vec<PatchRequest>,
    #[serde(default)]
    pub squares: Vec<SquareRequest>,
    /// true, false or "auto" (as generated).
    #[serde(default)]
    pub walls: Option<Value>,
    /// "all", "corners", "patches" or "squares": back to as generated.
    #[serde(default)]
    pub reset: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveRequest {
    pub corner: u32,
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    #[serde(default)]
    pub by: Option<[f64; 2]>,
    /// Back where it was planned.
    #[serde(default)]
    pub as_generated: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelaxRequest {
    pub at: [f64; 2],
    pub radius_ft: f64,
    /// 0–1: how far toward the middle of its neighbours a corner at the centre goes.
    #[serde(default = "half")]
    pub amount: f64,
}

fn half() -> f64 {
    0.5
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchRequest {
    pub patch: u32,
    /// A ward (`TOWN_WARDS`) or "auto".
    #[serde(default)]
    pub ward: Option<String>,
    /// A lot size (`LOT_SIZES`) or "auto".
    #[serde(default)]
    pub lots: Option<String>,
    /// A neighbouring patch, or "none".
    #[serde(default)]
    pub merge_with: Option<Value>,
    #[serde(default)]
    pub reroll: bool,
    /// A temple's patch kept as a close round the temple: true, false or "auto".
    #[serde(default)]
    pub close: Option<Value>,
    /// A park's patch laid out as a garden (`PARK_KINDS`) or "auto".
    #[serde(default)]
    pub park: Option<String>,
    /// (R5 towns) The district it goes in: a district's id (`d:<layout>:<n>`), a name (a new
    /// district, or the one of that name) or "auto".
    #[serde(default)]
    pub district: Option<String>,
    #[serde(default)]
    pub as_generated: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SquareRequest {
    pub corner: u32,
    /// A square (`SQUARE_KINDS`), "none" or "auto".
    pub kind: String,
}

/// Town `index` changed as asked: the town edit it then has (`None`: as generated) and what the
/// change does (corners that went all the way, part way or not at all; buildings added and taken
/// away; functions the town lost; the rectangles to draw again).
pub fn change(world: &World, t0: &T0, index: usize, req: &TownRequest) -> Result<(Option<TownEdit>, Value), String> {
    let (s, plan) = town_of(t0, world, index)?;
    let center = [s.x, s.y];
    let local = |p: [f64; 2]| sub(p, center);
    let world_pt = |p: P| add(p, center);
    // (R5 towns: an edit kept for its emptied districts alone, changing nothing, still holds their ids.)
    // (Roadmap 6 R4: or for its added streets, which the plan never sees; R5: or its removals; R6:
    // or its fences' changes.)
    let kept = || world.file.edits.towns.get(&key(index)).filter(|e| (!e.districts.is_empty() || !e.streets.is_empty() || !e.removed.is_empty() || !e.fences.is_empty()) && dist(e.at, center) <= TownEdit::SAME_AT_FT);
    let mut e = edit_of(world, t0, index).or_else(kept).cloned().unwrap_or_default();
    let e0 = e.clone();
    // Keep only what still applies (a stale part would be set aside anyway).
    let (ch, _) = changes(&plan, &e, center);
    let mut moves: BTreeMap<usize, P> = ch.moves.iter().copied().collect();
    // (R5 towns: the districts and patches as they stand, for what the change moves.)
    let (districts_before, moves_before) = (super::super::districts::of(&plan, &ch), ch.moves.clone());
    e.patches.retain(|pe| patch_here(&plan, pe.p as usize, local(pe.at)).is_ok());
    e.squares.retain(|se| square_here(&plan, se.v as usize, local(se.at)).is_ok() && parse_square(&se.kind).is_ok());
    super::super::wallgates::keep_live(&plan, center, &mut e.wall);
    match req.reset.as_deref() {
        None => {}
        Some("all") => {
            moves.clear();
            e.patches.clear();
            e.walls = None;
            e.squares.clear();
            e.wall = Default::default();
        }
        Some("corners") => moves.clear(),
        Some("patches") => e.patches.clear(),
        Some("squares") => e.squares.clear(),
        Some("walls") => {
            e.walls = None;
            e.wall = Default::default();
        }
        Some(r) => return Err(format!("reset: all, corners, patches, squares or walls, not {r}")),
    }
    let mut asked: Vec<usize> = Vec::new();
    // Corners one by one.
    let now = moved_mesh(&plan, &moves.iter().map(|(&v, &p)| (v, p)).collect::<Vec<_>>());
    for m in &req.moves {
        let v = m.corner as usize;
        if v >= plan.mesh.pos.len() || plan.vf[v].is_empty() {
            return Err(format!("corner {v}: no such corner"));
        }
        if m.as_generated {
            moves.remove(&v);
            continue;
        }
        let to = match (m.to, m.by) {
            (Some(t), None) => local(t),
            (None, Some(d)) => add(now.pos[v], d),
            _ => return Err(format!("corner {v}: give to [x_ft, y_ft] or by [dx_ft, dy_ft]")),
        };
        corner_ok(&plan, v, plan.mesh.pos[v], to).map_err(|why| format!("corner {v}: {why}"))?;
        moves.insert(v, to);
        asked.push(v);
    }
    // Brushes, on the corners as they stand after the moves above.
    let clamp = |v: usize, to: P| {
        let at = plan.mesh.pos[v];
        let d = dist(at, to);
        if d > plan.spacing { add(at, mul(sub(to, at), plan.spacing / d)) } else { to }
    };
    if !req.equalize.is_empty() || req.relax.is_some() {
        let mut mesh = moved_mesh(&plan, &moves.iter().map(|(&v, &p)| (v, p)).collect::<Vec<_>>());
        for &p in &req.equalize {
            let p = p as usize;
            if p >= plan.mesh.faces.len() {
                return Err(format!("equalize patch {p}: no such patch"));
            }
            patch_here(&plan, p, centroid(&plan.mesh.face_pts(p))).map_err(|why| format!("equalize patch {p}: {why}"))?;
            let before: Vec<P> = mesh.face_pts(p);
            mesh.equalize(&plan.vf, p, &|v| plan.pinned[v], 0.9);
            for (k, &v) in plan.mesh.faces[p].clone().iter().enumerate() {
                if dist(mesh.pos[v], before[k]) > 0.05 {
                    let to = clamp(v, mesh.pos[v]);
                    moves.insert(v, to);
                    asked.push(v);
                }
            }
        }
        if let Some(rx) = &req.relax {
            if !(rx.radius_ft > 0.0 && rx.radius_ft <= 5_000.0) || !(0.0..=1.0).contains(&rx.amount) {
                return Err("relax: radius_ft 1-5000 and amount 0-1".into());
            }
            let at = local(rx.at);
            let targets: Vec<(usize, P)> = (0..mesh.pos.len())
                .filter(|&v| !plan.pinned[v] && !plan.vf[v].is_empty() && dist(mesh.pos[v], at) < rx.radius_ft)
                .filter_map(|v| {
                    let nb = super::super::adj_vertices(&mesh, v);
                    if nb.is_empty() {
                        return None;
                    }
                    let mean = mul(nb.iter().fold([0.0, 0.0], |a, &u| add(a, mesh.pos[u])), 1.0 / nb.len() as f64);
                    let w = rx.amount * (1.0 - dist(mesh.pos[v], at) / rx.radius_ft);
                    Some((v, lerp(mesh.pos[v], mean, w)))
                })
                .collect();
            for (v, to) in targets {
                if mesh.try_move(&plan.vf, v, to) {
                    moves.insert(v, clamp(v, mesh.pos[v]));
                    asked.push(v);
                }
            }
        }
    }
    // Patches.
    for pr in &req.patches {
        let p = pr.patch as usize;
        if p >= plan.mesh.faces.len() {
            return Err(format!("patch {p}: no such patch"));
        }
        let at = centroid(&plan.mesh.face_pts(p));
        patch_here(&plan, p, at).map_err(|why| format!("patch {p}: {why}"))?;
        if pr.as_generated {
            e.patches.retain(|x| x.p as usize != p);
            continue;
        }
        let k = match e.patches.iter().position(|x| x.p as usize == p) {
            Some(k) => k,
            None => {
                e.patches.push(PatchEdit { p: p as u32, at: q64(world_pt(at)), ..Default::default() });
                e.patches.len() - 1
            }
        };
        let pe = &mut e.patches[k];
        match pr.ward.as_deref() {
            Some("auto") => pe.ward = None,
            Some(w) => {
                let parsed = parse_ward(w).map_err(|why| format!("patch {p}: {why}"))?;
                // (A park only where one can be laid out: mostly dry, with a way in.)
                if parsed == Some(Ward::Park)
                    && plan.ward[p] != Some(Ward::Park)
                    && let Some(why) = plan.park_refused[p]
                {
                    return Err(format!("patch {p}: {why}"));
                }
                pe.ward = (parsed != plan.ward[p]).then(|| w.to_string());
            }
            None => {}
        }
        match pr.lots.as_deref() {
            Some("auto") => pe.lots = None,
            Some(l) => {
                lot_factor(l).map_err(|why| format!("patch {p}: {why}"))?;
                pe.lots = Some(l.to_string());
            }
            None => {}
        }
        match &pr.merge_with {
            None => {}
            Some(Value::String(n)) if n == "none" || n == "auto" => pe.merge_with = None,
            Some(v) => pe.merge_with = Some(v.as_u64().filter(|q| (*q as usize) < plan.mesh.faces.len()).ok_or_else(|| format!("patch {p}: merge_with a neighbouring patch, or \"none\""))? as u32),
        }
        if pr.reroll {
            pe.reroll += 1;
        }
        match &pr.close {
            None => {}
            Some(Value::String(a)) if a == "auto" => pe.close = None,
            Some(Value::Bool(on)) => {
                let w = match pe.ward.as_deref() {
                    Some(n) => parse_ward(n).map_err(|why| format!("patch {p}: {why}"))?,
                    None => plan.ward[p],
                };
                if *on && w != Some(Ward::Temple) {
                    return Err(format!("patch {p}: only a temple's patch has a close"));
                }
                if *on && !plan.closes.contains(&p) && !has_temple(world, t0, &plan, index, p) {
                    return Err(format!("patch {p}: {NO_CLOSE}"));
                }
                pe.close = (*on != plan.closes.contains(&p)).then_some(*on);
            }
            Some(_) => return Err(format!("patch {p}: close: true, false or \"auto\"")),
        }
        if let Some(d) = &pr.district {
            let w = match e.patches[k].ward.as_deref() {
                Some(n) => Some(parse_ward(n).map_err(|why| format!("patch {p}: {why}"))?),
                None => None,
            };
            if !in_a_district(&plan, p, w) && d.trim() != "auto" {
                return Err(format!("patch {p}: {OUTSIDE_DISTRICT}"));
            }
            let mut d = super::super::districts::resolve(world, &plan, index, &mut e, p, d).map_err(|why| format!("patch {p}: {why}"))?;
            std::mem::swap(&mut e.patches[k].district, &mut d);
        }
        let pe = &mut e.patches[k];
        match pr.park.as_deref() {
            None => {}
            Some("auto") => pe.park = None,
            Some(k) => {
                parse_park(k).map_err(|why| format!("patch {p}: {why}"))?;
                let w = match pe.ward.as_deref() {
                    Some(n) => parse_ward(n).map_err(|why| format!("patch {p}: {why}"))?,
                    None => plan.ward[p],
                };
                if w != Some(Ward::Park) {
                    return Err(format!("patch {p}: {NOT_PARK}"));
                }
                pe.park = Some(k.to_string());
            }
        }
    }
    // A park's garden goes with the park (a patch given another ward).
    for pe in &mut e.patches {
        let w = pe.ward.as_deref().map_or(Ok(plan.ward[pe.p as usize]), parse_ward);
        if pe.park.is_some() && w != Ok(Some(Ward::Park)) {
            pe.park = None;
        }
    }
    // A close asked for where a square opens at its corner is refused.
    for pr in req.patches.iter().filter(|pr| matches!(pr.close, Some(Value::Bool(true)))) {
        let (ch, _) = changes(&plan, &e, center);
        let (ward, group) = ch.wards(&plan);
        let sq = super::super::squares::spots(&plan, &ch, &ward, &group, &|_| false).0;
        if plan.mesh.faces[pr.patch as usize].iter().any(|u| sq.iter().any(|s| s.0 == *u)) {
            return Err(format!("patch {}: {SQUARE_THERE}", pr.patch));
        }
    }
    // Squares, once every ward is set (one opens only where three streets meet between lots).
    for sr in &req.squares {
        let v = sr.corner as usize;
        if v >= plan.mesh.pos.len() || plan.vf[v].is_empty() {
            return Err(format!("corner {v}: no such corner"));
        }
        e.squares.retain(|se| se.v != sr.corner);
        if sr.kind == "auto" {
            continue;
        }
        let k = parse_square(&sr.kind).map_err(|why| format!("corner {v}: {why}"))?;
        if k == plan.squares.iter().find(|s| s.0 == v).map(|s| s.1) {
            continue;
        }
        if k.is_some() {
            let (ch, _) = changes(&plan, &e, center);
            let (ward, group) = ch.wards(&plan);
            if !super::super::squares::junction(&plan, &ward, &group, v) {
                return Err(format!("corner {v}: {NO_SQUARE}"));
            }
            // The closes as built now, and those asked for with this change.
            let now = built_closes(&super::super::base_layout(world, t0, index), &moved_mesh(&plan, &ch.moves), center);
            let asked = |p: usize| now.contains(&p) || req.patches.iter().any(|pr| pr.patch as usize == p && matches!(pr.close, Some(Value::Bool(true))));
            if super::super::squares::at_close(&plan, v, &super::super::squares::spots(&plan, &ch, &ward, &group, &asked).1) {
                return Err(format!("corner {v}: {AT_CLOSE}"));
            }
        }
        e.squares.push(SquareEdit { v: sr.corner, at: q64(world_pt(plan.mesh.pos[v])), kind: sr.kind.clone() });
    }
    e.squares.sort_by_key(|se| se.v);
    let mut wall_asked = None;
    match &req.walls {
        None => {}
        Some(Value::Bool(b)) => e.walls = (*b != plan.walled).then_some(*b),
        Some(Value::String(a)) if a == "auto" => e.walls = None,
        // (R5 towns, M18b: gates and towers by hand.)
        Some(v @ Value::Object(_)) => {
            e.corners = moves.iter().filter(|(v, to)| dist(plan.mesh.pos[**v], **to) > 0.05).map(|(&v, &to)| CornerMove { v: v as u32, from: q64(world_pt(plan.mesh.pos[v])), to: q64(world_pt(to)) }).collect();
            wall_asked = Some(super::super::wallgates::request(world, t0, index, &plan, center, &mut e, v)?);
        }
        Some(_) => return Err("walls: true, false, \"auto\" or {built, gates_add, gates_remove, towers_add, towers_remove}".into()),
    }
    // The edit as it now stands; merges checked once every ward is set.
    e.at = q64(center);
    e.corners = moves.iter().filter(|(v, to)| dist(plan.mesh.pos[**v], **to) > 0.05).map(|(&v, &to)| CornerMove { v: v as u32, from: q64(world_pt(plan.mesh.pos[v])), to: q64(world_pt(to)) }).collect();
    // A merge the wards no longer allow (its patch or the one it joined given a ward that isn't
    // built on lots) goes, unless it was asked for now.
    let mut unmerged = Vec::new();
    {
        let (ch, _) = changes(&plan, &TownEdit { patches: e.patches.iter().map(|pe| PatchEdit { merge_with: None, ..pe.clone() }).collect(), ..e.clone() }, center);
        let (ward, _) = ch.wards(&plan);
        let asked: Vec<u32> = req.patches.iter().filter(|pr| pr.merge_with.is_some()).map(|pr| pr.patch).collect();
        for pe in &mut e.patches {
            if let Some(q) = pe.merge_with
                && !asked.contains(&pe.p)
                && (q as usize) < plan.mesh.faces.len()
                && merge_ok(&plan, &ward, pe.p as usize, q as usize).is_err()
            {
                pe.merge_with = None;
                unmerged.push(pe.p);
            }
        }
    }
    // A square set by hand where the wards no longer leave three streets goes too.
    {
        let (ch, _) = changes(&plan, &e, center);
        let (ward, group) = ch.wards(&plan);
        e.squares.retain(|se| se.kind == "none" || super::super::squares::junction(&plan, &ward, &group, se.v as usize));
    }
    e.patches.retain(|pe| !pe.is_noop());
    e.patches.sort_by_key(|pe| pe.p);
    // (R5 towns: a district made by hand that no patch is in any more stays in the list, empty, so
    // its id is never another's (its rename stays its own); the edit is kept for it alone.)
    let (ch, aside) = changes(&plan, &e, center);
    if let Some(why) = aside.first() {
        return Err(why.clone());
    }
    // What it does.
    let before = super::super::base_layout(world, t0, index);
    let after = if e.is_noop() { super::super::generated_layout(world, t0, index) } else { with_edit(world, t0, index, &e) };
    let mesh = moved_mesh(&plan, &ch.moves);
    // A close asked for that isn't laid out (no room: `NO_ROOM`) is refused.
    let laid = built_closes(&after, &mesh, center);
    if let Some(pr) = req.patches.iter().find(|pr| matches!(pr.close, Some(Value::Bool(true))) && !laid.contains(&(pr.patch as usize))) {
        return Err(format!("patch {}: {NO_ROOM}", pr.patch));
    }
    asked.sort();
    asked.dedup();
    let (mut held, mut part, mut stayed) = (0, 0, Vec::new());
    for &v in &asked {
        let Some(&to) = moves.get(&v) else { continue };
        let (got, from) = (mesh.pos[v], plan.mesh.pos[v]);
        if dist(got, to) <= 0.5 {
            held += 1;
        } else if dist(got, from) > 0.5 {
            part += 1;
            stayed.push(json!({ "corner": v, "asked": round1(world_pt(to)), "got": round1(world_pt(got)) }));
        } else {
            stayed.push(json!({ "corner": v, "asked": round1(world_pt(to)), "got": round1(world_pt(got)) }));
        }
    }
    // (By id and footprint: a patch laid out again reuses its new buildings' ids.)
    let ids = |l: &Layout| l.buildings.iter().map(|b| (b.id, b.poly.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]).collect::<Vec<u64>>())).collect::<std::collections::BTreeSet<_>>();
    let (ib, ia) = (ids(&before), ids(&after));
    let generated = super::super::generated_layout(world, t0, index);
    let funcs = |l: &Layout| {
        let mut m: BTreeMap<&'static str, i32> = BTreeMap::new();
        for b in &l.buildings {
            if let Some(f) = b.func {
                *m.entry(CATALOG[f as usize].key).or_default() += 1;
            }
        }
        m
    };
    let (fg, fa) = (funcs(&generated), funcs(&after));
    let lost: Vec<&str> = fg.iter().filter(|(k, n)| fa.get(*k).copied().unwrap_or(0) < **n).map(|(k, _)| *k).collect();
    let mut report = json!({
        "corners": { "asked": asked.len(), "moved": held, "part_way": part, "stayed": asked.len() - held - part },
        "buildings": { "before": before.buildings.len(), "after": after.buildings.len(), "added": ia.difference(&ib).count(), "taken_away": ib.difference(&ia).count() },
        "walls": { "built": ch.walls.unwrap_or(plan.walled), "towers": crate::interior::towers(&after).len(), "gates": after.gates.len() },
        "squares": after.plazas.iter().filter(|p| !matches!(p.kind, PlazaKind::Quay | PlazaKind::Yard | PlazaKind::Bailey)).map(|p| p.kind.name()).collect::<Vec<_>>(),
        "parks": after.parks.iter().map(|p| p.kind.name()).collect::<Vec<_>>(),
        "rects": changed_rects(&before, &after).iter().map(|r| r.map(crate::core::round)).collect::<Vec<_>>(),
    });
    // Which patches changed, and what about them (settings and corners; districts are in districts_changed).
    {
        let was = moved_mesh(&plan, &moves_before);
        let mut what: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
        let default = PatchEdit::default();
        let mut ps: Vec<u32> = e0.patches.iter().chain(&e.patches).map(|pe| pe.p).collect();
        ps.sort();
        ps.dedup();
        for p in ps {
            let (a, b) = (e0.patches.iter().find(|pe| pe.p == p).unwrap_or(&default), e.patches.iter().find(|pe| pe.p == p).unwrap_or(&default));
            let list = what.entry(p as usize).or_default();
            for (name, differs) in [("ward", a.ward != b.ward), ("lots", a.lots != b.lots), ("merge", a.merge_with != b.merge_with), ("reroll", a.reroll != b.reroll), ("close", a.close != b.close), ("park", a.park != b.park)] {
                if differs {
                    list.push(name);
                }
            }
        }
        for (f, vs) in mesh.faces.iter().enumerate() {
            if vs.iter().any(|&v| v < was.pos.len() && dist(was.pos[v], mesh.pos[v]) > 0.5) {
                what.entry(f).or_default().push("corners");
            }
        }
        what.retain(|_, w| !w.is_empty());
        if !what.is_empty() {
            let n = what.len();
            let mut list: Vec<Value> = what.iter().take(30).map(|(p, w)| json!({ "patch": p, "what": w })).collect();
            if n > 30 {
                list.push(json!(format!("and {} more", n - 30)));
            }
            report["patches_changed"] = json!(list);
        }
    }
    if !stayed.is_empty() {
        report["corners"]["short"] = json!(stayed.into_iter().take(20).collect::<Vec<_>>());
        report["corners"]["why"] = json!("a corner goes only as far as keeps every patch round it convex");
    }
    if !lost.is_empty() {
        report["functions_lost"] = json!(lost);
    }
    if !unmerged.is_empty() {
        report["unmerged"] = json!(unmerged);
    }
    if let Some(a) = wall_asked {
        report["wall"] = super::super::wallgates::report(world, t0, index, &before, &after, &mut e, a);
    }
    // (R5 towns) Patches put in another district, and renames and notes carried or lost.
    {
        let now = super::super::districts::of(&plan, &ch);
        let (was, is) = (super::super::districts::membership(&districts_before), super::super::districts::membership(&now));
        let mut ps: Vec<usize> = was.keys().chain(is.keys()).copied().collect();
        ps.sort();
        ps.dedup();
        let id = |q: Option<&usize>| q.map(|q| format!("d:{index}:{q}"));
        let moved: Vec<Value> = ps.iter().filter(|p| was.get(p) != is.get(p)).map(|&p| json!({ "patch": p, "from": id(was.get(&p)), "to": id(is.get(&p)) })).collect();
        if !moved.is_empty() {
            report["districts_changed"] = json!(moved);
        }
        let (carried, lost) = super::super::carry::renames(world, index, center, &before, &after, &moved_mesh(&plan, &moves_before), &mesh, 1.5 * plan.spacing);
        if !carried.is_empty() {
            report["renames_carried"] = json!(carried.iter().map(|(a, b, n)| json!({ "from": format!("b:{index}:{a}"), "to": format!("b:{index}:{b}"), "name": n })).collect::<Vec<_>>());
        }
        if !lost.is_empty() {
            report["renames_lost"] = json!(lost.iter().map(|(a, n)| json!({ "id": format!("b:{index}:{a}"), "name": n })).collect::<Vec<_>>());
        }
    }
    Ok(((!e.is_noop() || !e.districts.is_empty() || !e.streets.is_empty() || !e.removed.is_empty() || !e.fences.is_empty()).then_some(e), report))
}

/// `change` as JSON: `{edit, report}` or `{error}` (for the app's worker).
pub fn change_json(world: &World, t0: &T0, index: usize, req: &str) -> String {
    let r = serde_json::from_str::<TownRequest>(req).map_err(|e| e.to_string()).and_then(|req| change(world, t0, index, &req));
    match r {
        Ok((edit, report)) => json!({ "edit": edit, "report": report }),
        Err(e) => json!({ "error": e }),
    }
    .to_string()
}

/// Town edits set aside, whole or in part (key, why).
pub fn set_aside(world: &World, t0: &T0) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (k, e) in &world.file.edits.towns {
        let Some(index) = k.parse::<usize>().ok().filter(|&i| super::super::is_settlement(t0, i)) else {
            let parked = k.parse::<usize>().is_ok_and(|i| i >= crate::world::PARKED_LAYOUT);
            out.push((k.clone(), if parked { "set aside when the world was remade or upgraded" } else { "not a town" }.into()));
            continue;
        };
        let Some(s) = super::super::settlement(t0, index) else { continue };
        // (A village's entry holding only added streets is no ward edit: `town::added`; nor one
        // holding only removals, `town::cut`, or fences' changes, `town::fence`.)
        let streets_only = e.is_noop() && e.districts.is_empty() && (!e.streets.is_empty() || !e.removed.is_empty() || !e.fences.is_empty());
        if s.tier < Tier::Town && streets_only && dist(e.at, [s.x, s.y]) <= TownEdit::SAME_AT_FT {
            continue;
        } else if s.tier < Tier::Town {
            out.push((k.clone(), if streets_only { "the village was laid out anew (it moved)" } else { "a village has no wards" }.into()));
        } else if dist(e.at, [s.x, s.y]) > TownEdit::SAME_AT_FT {
            out.push((k.clone(), "the town was laid out anew (it moved)".into()));
        } else if let Some(plan) = super::super::town_plan(world, t0, index) {
            for why in changes(&plan, e, [s.x, s.y]).1 {
                out.push((k.clone(), why));
            }
        }
    }
    out
}
