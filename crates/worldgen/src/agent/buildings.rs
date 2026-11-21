//! Buildings and works: streets in a box, building edits, wall works spots, overlap tests.

use super::*;
use crate::world::BuildingEditChecks;
use crate::town::geom::{lerp, seg_dist};
use crate::town::handwall::cut;
pub use crate::town::pieces::BuildingChange;

/// The streets, walls, squares and world roads in an area (`list_streets`): each layout (settlement or site) in `rect` gives its streets
/// (`{what: street, settlement, width_ft, kind, pts}`, a generated settlement's with its `tr:` id, one added by hand with its `ts:` id), town and castle walls (`wall`; a hand-drawn one with its `c:` id and name) and
/// squares (`square`, whole: `kind`, `pts`), then the world's roads (`road`: `id` rd:, `class`, `pts`); lines are cut to the area
/// (`inside` tells which points are in it), points in whole ft.
pub fn streets_in(world: &World, t0: &T0, rect: [f64; 4], inside: &dyn Fn(P) -> bool) -> Vec<Value> {
    let mut out = Vec::new();
    let generated = t0.settlements.len() + t0.base_pois;
    let pts_json = |r: &Vec<P>| json!(r.iter().map(|p| json!([p[0], p[1]])).collect::<Vec<_>>());
    for l in town::layouts_near(world, t0, rect) {
        let li = l.index as usize;
        let (s, site) = whereabouts(world, t0, li);
        let mut head = json!({ "settlement": s });
        if li >= t0.settlements.len()
            && li < generated + world.file.edits.created.len()
            && site.as_ref().is_some_and(|v| !v.is_null())
            && place_of(world, t0, li).1 != s["name"].as_str().map(str::to_string)
        {
            head["site"] = site.unwrap_or(Value::Null);
        }
        let with = |what: &str, extra: Value| {
            let mut v = head.clone();
            v["what"] = json!(what);
            for (k, x) in extra.as_object().cloned().unwrap_or_default() {
                v[k] = x;
            }
            v
        };
        let added = town::added::ids(world, t0, &l);
        // (R5) A generated settlement's own streets with their `tr:` ids (`removed`: taken off the map by hand).
        let own = town::added::home(world, t0, li).is_some();
        let trs = if own { town::cut::street_ids(world, t0, &l) } else { Vec::new() };
        let cut = if own { town::cut::Cut::of(world, t0, &l) } else { None };
        for (k, (pts, class, w)) in l.roads.iter().enumerate() {
            let id = added.iter().find(|x| x.0 == k).map(|x| format!("ts:{li}:{}", x.1)).or_else(|| trs.iter().find(|x| x.0 == town::cut::ROADS && x.1 == k).map(|x| town::cut::street_id(li, x.2)));
            let removed = cut.as_ref().is_some_and(|c| c.road(k));
            for r in clip_runs(pts, inside) {
                let mut x = json!({ "width_ft": crate::core::round(*w), "kind": street_kind(*class), "pts": pts_json(&r) });
                if let Some(id) = &id {
                    x["id"] = json!(id);
                }
                if removed {
                    x["removed"] = json!(true);
                }
                out.push(with("street", x));
            }
        }
        let created = if li >= generated { world.file.edits.created.get(li - generated) } else { None };
        for w in &l.walls {
            for r in clip_runs(w, inside) {
                let mut x = json!({ "width_ft": 9, "pts": pts_json(&r) });
                if let Some(c) = created {
                    x["id"] = json!(c.id);
                    x["name"] = json!(c.name);
                }
                out.push(with("wall", x));
            }
        }
        for p in l.plazas.iter().filter(|p| p.kind != town::PlazaKind::Bailey) {
            // (Touches the area: a corner or its middle in it, an edge running through it (sampled every 2 ft, so a small box on a long edge counts), or the area's middle on the square.)
            let outline: Vec<P> = p.pts.iter().chain(p.pts.first()).copied().collect();
            let mid = [0.5 * (rect[0] + rect[2]), 0.5 * (rect[1] + rect[3])];
            if p.pts.iter().any(|q| inside(*q)) || inside(geom::centroid(&p.pts)) || outline.windows(2).any(|e| (0..=(geom::dist(e[0], e[1]) / 2.0).ceil() as usize).any(|k| inside(geom::lerp(e[0], e[1], k as f64 / (geom::dist(e[0], e[1]) / 2.0).ceil().max(1.0))))) || (inside(mid) && geom::contains(&p.pts, mid)) {
                let pts: Vec<P> = p.pts.iter().map(|q| [crate::core::round(q[0]), crate::core::round(q[1])]).collect();
                out.push(with("square", json!({ "kind": plaza_kind(p.kind), "pts": pts_json(&pts) })));
            }
        }
    }
    // The roads as the blocker check sees them (`rc.eval` along each stretch, not the coarse `roadids::line`), the stretches near the area
    // joined where they follow each other.
    let ids = crate::roadids::road_ids(t0);
    let mut near: Vec<(u32, u32)> = t0.roads.segments_near(rect, 0.0);
    near.sort_unstable();
    near.dedup();
    let mut at = 0;
    while at < near.len() {
        let ri = near[at].0;
        let rc = &t0.roads.roads[ri as usize];
        let mut line: Vec<P> = Vec::new();
        let mut last = u32::MAX;
        while at < near.len() && near[at].0 == ri {
            let k = near[at].1;
            if line.is_empty() || k != last.wrapping_add(1) {
                for r in clip_runs(&std::mem::take(&mut line), inside) {
                    out.push(road_json(&ids, ri, rc, &r));
                }
            }
            let from = if line.is_empty() { 0 } else { 1 };
            line.extend((from..=16).map(|j| rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p));
            last = k;
            at += 1;
        }
        for r in clip_runs(&line, inside) {
            out.push(road_json(&ids, ri, rc, &r));
        }
    }
    out
}

pub(super) fn road_json(ids: &[String], ri: u32, rc: &crate::lod::roads::RoadCurve, r: &[P]) -> Value {
    let pts = r.iter().map(|p| json!([p[0], p[1]])).collect::<Vec<_>>();
    json!({ "what": "road", "id": ids[ri as usize], "class": crate::roadids::class_name(rc.class), "width_ft": rc.class.width_ft(), "pts": pts })
}

/// Generated buildings (not drawn by hand) whose middle lies inside `poly` (world ft), as they
/// stand: (building id, its middle as generated: an edit's `at`).
pub fn generated_buildings_in(world: &World, t0: &T0, poly: &[P]) -> Vec<(String, P)> {
    if poly.len() < 3 {
        return Vec::new();
    }
    let (x0, y0) = poly.iter().fold((f64::MAX, f64::MAX), |a, p| (a.0.min(p[0]), a.1.min(p[1])));
    let (x1, y1) = poly.iter().fold((f64::MIN, f64::MIN), |a, p| (a.0.max(p[0]), a.1.max(p[1])));
    let mut out = Vec::new();
    for l in town::layouts_near(world, t0, [x0, y0, x1, y1]) {
        let li = l.index as usize;
        if li >= t0.settlements.len() + t0.base_pois && !town::is_settlement(t0, li) {
            continue;
        }
        let base = town::base_layout(world, t0, li);
        for b in &l.buildings {
            if geom::contains(poly, geom::centroid(&b.poly))
                && let Some(g) = base.building(b.id as usize)
            {
                out.push((format!("b:{li}:{}", b.id), geom::centroid(&g.poly)));
            }
        }
    }
    out
}

/// Generated building `id`'s edit with `change` made: checked (a new footprint as a drawn
/// building's, skipping itself). `None` when it is then as generated.
pub fn building_edit(world: &World, t0: &T0, id: &str, change: &BuildingChange) -> Result<Option<crate::world::BuildingEdit>, String> {
    let (li, bi) = generated_building(t0, id).ok_or_else(|| town::pieces::not_generated(world, t0, id))?;
    let base = town::base_layout(world, t0, li);
    let g = base.building(bi as usize).ok_or_else(|| format!("no such building: {id}"))?;
    let at = geom::centroid(&g.poly);
    let mut e = world.file.edits.buildings.get(id).filter(|e| town::edit_applies(g, town::generated_layout(world, t0, li).building(bi as usize), e)).cloned().unwrap_or_default();
    if e.removed {
        return Err(format!("{id} was removed: restore it first (restore_building)"));
    }
    // (An outline drawn before a square or close cut the building back stays set aside.)
    if !town::edit_stands(g, &e) {
        e.poly.clear();
    }
    e.at = at;
    let keep = |v: &Option<String>| v.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "auto");
    if change.func.is_some() {
        e.func = keep(&change.func);
    }
    if change.roof.is_some() {
        e.roof = keep(&change.roof);
    }
    if change.tint.is_some() {
        e.tint = keep(&change.tint);
    }
    for (v, list, what) in [(&change.material, &town::roof::MATERIALS[..], "material"), (&change.shape, &town::roof::SHAPES[..], "shape"), (&change.tint, &crate::world::TINTS[..], "tint"), (&change.roof, &crate::world::ROOFS[..], "roof")] {
        if let Some(v) = keep(v)
            && !list.contains(&v.as_str())
        {
            return Err(format!("{what} must be one of {}, or auto", list.join(", ")));
        }
    }
    if change.material.is_some() {
        e.material = keep(&change.material);
    }
    if change.shape.is_some() {
        e.shape = keep(&change.shape);
    }
    if let Some(s) = change.shade {
        if s.is_some_and(|s| s > 4) {
            return Err("shade must be 0 to 4 (2 the usual), or auto".into());
        }
        e.shade = s;
    }
    if change.structure.is_some() {
        let generated = match g.structure {
            town::Structure::Ruin => "ruin",
            _ => "roofed",
        };
        e.structure = keep(&change.structure).filter(|s| s != generated);
    }
    if let Some(f) = change.floors {
        e.floors = Some(f).filter(|&f| f != g.floors);
    }
    if let Some(p) = &change.poly {
        e.poly = p.clone();
        if !p.is_empty() {
            building_spot(world, t0, p, e.func.as_deref(), id, Some((li, Some(bi))))?;
        }
    }
    if g.structure == town::Structure::Open && (e.structure.is_some() || !e.poly.is_empty()) {
        return Err(format!("{id} is a graveyard: it can be removed, not rebuilt"));
    }
    e.check()?;
    Ok((!e.is_noop()).then_some(e))
}

/// `building_edit` (or with `{"remove": true}`, `building_removal`) as JSON: `{edit}` (null:
/// as generated) or `{error}`.
pub fn building_edit_json(world: &World, t0: &T0, id: &str, change_json: &str) -> String {
    let v: Value = serde_json::from_str(change_json).unwrap_or_default();
    // (R5 towns) A piece of a castle made by hand: its edit is kept on the castle, by `key`
    // (`{}` as the change just names the piece).
    if generated_building(t0, id).is_none()
        && let Some(p) = town::pieces::piece_of(world, t0, id)
    {
        let r = if v["remove"].as_bool() == Some(true) {
            town::pieces::piece_removal(&p).map(Some)
        } else {
            let name = v["name"].as_str().map(str::to_string);
            serde_json::from_value::<BuildingChange>(v).map_err(|e| e.to_string()).and_then(|c| town::pieces::piece_edit(world, &p, &c, name.as_deref()))
        };
        return match r {
            Ok(e) => json!({ "piece": { "castle": p.castle, "key": p.key, "keep": p.keep(), "edit": e } }),
            Err(e) => json!({ "error": e }),
        }
        .to_string();
    }
    let r = if v["remove"].as_bool() == Some(true) {
        building_removal(world, t0, id).map(Some)
    } else {
        serde_json::from_value::<BuildingChange>(v).map_err(|e| e.to_string()).and_then(|c| building_edit(world, t0, id, &c))
    };
    match r {
        Ok(e) => json!({ "edit": e }),
        Err(e) => json!({ "error": e }),
    }
    .to_string()
}

/// Where a castle or a wall drawn by hand (`c`: its `poly` or `pts`, `closed`) may stand.
pub struct WorksSpot {
    /// Its point: a castle's middle, the middle of a wall's extent.
    pub at: P,
    pub name: String,
    /// The world's own buildings in its way, to be taken away with it: (building id, its middle
    /// as generated).
    pub in_way: Vec<(String, P)>,
    /// Those of them with a name, by id (for the refusal: "b:5:8 (the Smithy)").
    pub in_way_names: crate::core::hash::FastMap<String, String>,
    /// (Roadmap 6 P7) A wall's line with its open ends put onto the walls they meet (`WALL_JOIN`):
    /// None where neither end meets one.
    pub pts: Option<Vec<P>>,
    /// (Roadmap 6 P7) A castle's gates asked for that cannot be built: (side, why).
    pub gates_not_built: Vec<(u32, String)>,
}

/// An open wall's end this near another wall's line (or a castle's curtain), short of it or past
/// it, joins it: the end is put on that line (ft).
pub const WALL_JOIN: f64 = 15.0;

/// The nearest point on the lines `walls` to `p`, within `reach`.
fn nearest_on_walls(walls: &[&Vec<P>], p: P, reach: f64) -> Option<P> {
    let mut best: Option<(f64, P)> = None;
    for w in walls {
        for s in w.windows(2) {
            let d = geom::sub(s[1], s[0]);
            let f = (geom::dot(geom::sub(p, s[0]), d) / geom::dot(d, d).max(1e-9)).clamp(0.0, 1.0);
            let q = geom::add(s[0], geom::mul(d, f));
            let dq = geom::dist(p, q);
            if dq <= reach && best.is_none_or(|b| dq < b.0) {
                best = Some((dq, q));
            }
        }
    }
    best.map(|b| b.1)
}

/// Where a castle (its outline) or a wall (its line) may stand: on the map, on dry land out of
/// river channels, not across other walls; a castle off squares, roads and main streets (a
/// wall gets gates where roads and streets cross it). Buildings of the world's own in its way
/// are listed to be taken away with it; a building drawn by hand refuses it. `skip`: its own
/// layout, when it is being redrawn. Named (the nearest settlement's culture) by its id.
pub fn works_spot(world: &World, t0: &T0, c: &crate::world::Created, id: &str, skip: Option<usize>) -> Result<WorksSpot, String> {
    let castle = c.kind == "castle";
    let pts: &[P] = if castle { &c.poly } else { &c.pts };
    if pts.len() < 2 {
        return Err(format!("a {} needs at least {} corners", c.kind, if castle { 3 } else { 2 }));
    }
    // (An open wall's ends near another wall's line, or a castle's curtain, put on it: a T-join.)
    let mut joined: Option<Vec<P>> = None;
    let mut ends: Vec<P> = Vec::new();
    if !castle && !c.closed {
        let b = bbox(pts);
        let r = WALL_JOIN + 1.0;
        let near = town::layouts_near(world, t0, [b[0] - r, b[1] - r, b[2] + r, b[3] + r]);
        let walls: Vec<&Vec<P>> = near.iter().filter(|l| Some(l.index as usize) != skip).flat_map(|l| l.walls.iter()).collect();
        let mut line = pts.to_vec();
        let last = line.len() - 1;
        for k in [0, last] {
            if let Some(q) = nearest_on_walls(&walls, line[k], WALL_JOIN)
                && geom::dist(q, line[if k == 0 { 1 } else { last - 1 }]) >= 10.0
            {
                line[k] = [crate::core::round(q[0] * 64.0) / 64.0, crate::core::round(q[1] * 64.0) / 64.0];
                ends.push(line[k]);
            }
        }
        if !ends.is_empty() {
            joined = Some(line);
        }
    }
    let pts: &[P] = joined.as_deref().unwrap_or(pts);
    if pts.iter().any(|p| p[0] < 0.0 || p[1] < 0.0 || p[0] > world.geom.map_w_ft || p[1] > world.geom.map_h_ft) {
        return Err("that place is off the map".to_string());
    }
    let n = pts.len();
    let ring = castle || c.closed;
    let segs: Vec<(P, P)> = (0..if ring { n } else { n - 1 }).map(|i| (pts[i], pts[(i + 1) % n])).collect();
    let (x0, y0) = pts.iter().fold((f64::MAX, f64::MAX), |a, p| (a.0.min(p[0]), a.1.min(p[1])));
    let (x1, y1) = pts.iter().fold((f64::MIN, f64::MIN), |a, p| (a.0.max(p[0]), a.1.max(p[1])));
    let at = if castle { geom::centroid(pts) } else { [0.5 * (x0 + x1), 0.5 * (y0 + y1)] };
    // The line (or outline), every 10 ft, and a castle's middle must be dry.
    let mut probes: Vec<P> = if castle { vec![at] } else { Vec::new() };
    for &(a, b) in &segs {
        let k = (geom::dist(a, b) / 10.0).ceil().max(1.0) as usize;
        probes.extend((0..=k).map(|s| geom::lerp(a, b, s as f64 / k as f64)));
    }
    let shore = town::Shore::new(world, t0, [x0, y0, x1, y1]);
    // (R5 towns: a closed wall is broken where it meets the water, as a town's: `sites::wall_site`.)
    for p in probes.iter().filter(|_| !(c.closed && c.kind == "wall")) {
        if shore.wet(*p) {
            return Err(format!("that is in the water: {}", if castle { "draw it on dry land" } else { "end the wall at the shore" }));
        }
        let q = crate::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 2.0, t0.cell_ft);
        if (q.0 - p[0]).abs() + (q.1 - p[1]).abs() > 0.01 && !shore.on_islet(*p) && !shore.raised(*p) {
            return Err(format!("that is in a river: {}", if castle { "draw it on the bank" } else { "end the wall at the bank" }));
        }
    }
    // Within this of the line, a building is in the wall's way (the wall's half width and a foot).
    const WALL_CLEAR: f64 = 5.5;
    // (A joined end drawn back clear of what stands against the wall it meets: a castle's ranges.)
    let back = |p: P, q: P| if ends.contains(&p) { geom::lerp(p, q, (8.0 / geom::dist(p, q).max(1e-9)).min(0.5)) } else { p };
    let body: Vec<(P, P)> = segs.iter().map(|&(a, b)| (back(a, b), back(b, a))).collect();
    let in_line = |poly: &[P]| body.iter().any(|&(a, b)| poly_seg_dist(poly, a, b) < WALL_CLEAR);
    // (Where a joined end meets the other's line, or another wall's end meets this one's, is no
    // crossing: within the wall's half width of that end.)
    // (A castle's curtain stands inside its outline, so a wall T-joined onto it ends within `WALL_JOIN` of it.)
    let tol = if castle { WALL_JOIN } else { 1.0 };
    let joint = |p: P, w: &[P]| ends.iter().chain([w[0], w[w.len() - 1]].iter().filter(|e| segs.iter().any(|&(a, b)| geom::seg_dist(**e, a, b) < tol))).any(|e| geom::dist(*e, p) < WALL_CLEAR.max(tol));
    let crosses_line = |w: &[P]| {
        w.windows(2).any(|s| {
            segs.iter().any(|&(a, b)| {
                (geom::segs_cross_mul(a, b, s[0], s[1]) && !joint(seg_meet(a, b, s[0], s[1]), w)) || (geom::seg_dist(s[0], a, b) < 1.0 && !joint(s[0], w)) || (geom::seg_dist(s[1], a, b) < 1.0 && !joint(s[1], w))
            })
        })
    };
    let generated = t0.settlements.len() + t0.base_pois;
    let mut in_way: Vec<(String, P)> = Vec::new();
    let mut in_way_names: crate::core::hash::FastMap<String, String> = Default::default();
    for l in town::layouts_near(world, t0, [x0, y0, x1, y1]) {
        let li = l.index as usize;
        if Some(li) == skip {
            continue;
        }
        // (Roadmap 6 P7: a building drawn by hand may stand in a castle's yard, not on its walls.)
        let drawn = castle && li >= generated && li.checked_sub(generated).and_then(|k| world.file.edits.created.get(k)).is_some_and(|c| c.kind == "building");
        for b in &l.buildings {
            let hit = if castle && !drawn { polys_overlap(pts, &b.poly) } else { in_line(&b.poly) };
            if !hit {
                continue;
            }
            if li >= generated && !town::is_settlement(t0, li) {
                let c = geom::centroid(&b.poly);
                return Err(format!("that crosses b:{li}:{} ({}) at ({:.0}, {:.0})", b.id, building_name(world, &l, b), c[0], c[1]));
            }
            let base = town::base_layout(world, t0, li);
            if let Some(g) = base.building(b.id as usize) {
                let id = format!("b:{li}:{}", b.id);
                if let Some(n) = world.file.edits.renames.get(&id).cloned().or_else(|| b.name.clone()).filter(|n| !n.is_empty()) {
                    in_way_names.insert(id.clone(), n);
                }
                in_way.push((id, geom::centroid(&g.poly)));
            }
        }
        if l.walls.iter().any(|w| crosses_line(w) || (castle && w.iter().enumerate().any(|(k, p)| !(k == 0 || k + 1 == w.len()) | ((0..pts.len()).all(|i| geom::seg_dist(*p, pts[i], pts[(i + 1) % pts.len()]) > WALL_JOIN)) && geom::contains(pts, *p)))) {
            return Err(wall_crossed(world, t0, &l, &segs));
        }
        if castle {
            if l.plazas.iter().any(|p| polys_overlap(pts, &p.pts)) {
                return Err(format!("that is on {}", if l.site { "a yard" } else { "a square (a market, quay or green)" }));
            }
            // Paved streets and alleys (class 3) may run under a castle's yard; roads, approaches
            // and main streets may not.
            for (r, _, w) in l.roads.iter().filter(|r| r.1 != 3) {
                if r.windows(2).any(|s| poly_seg_dist(pts, s[0], s[1]) < 0.5 * w) {
                    return Err("that is on a road or a main street: castles stand beside them".to_string());
                }
            }
        }
    }
    if castle {
        for (ri, k) in t0.roads.segments_near([x0, y0, x1, y1], 40.0) {
            let rc = &t0.roads.roads[ri as usize];
            let half = 0.5 * rc.class.width_ft();
            let rp: Vec<P> = (0..=16).map(|j| rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p).collect();
            if rp.windows(2).any(|s| poly_seg_dist(pts, s[0], s[1]) < half) {
                return Err("that is on a road: castles stand beside them".to_string());
            }
        }
    }
    in_way.sort_by(|a, b| a.0.cmp(&b.0));
    in_way.dedup_by(|a, b| a.0 == b.0);
    // (A castle as it would be built: a drawn keep on a tower, a gate or a building drawn in its
    // yard is refused; the gates asked for that cannot be built, told.)
    let mut gates_not_built = Vec::new();
    if castle {
        let r = town::sites::castle_report(world, t0, c, at);
        if let Some(why) = r.keep {
            return Err(format!("the drawn keep stands {why}: draw it clear of the curtain's towers, its gates and the buildings in the yard"));
        }
        let sides = c.castle_gates();
        gates_not_built = r.gates.into_iter().filter_map(|(i, why)| sides.get(i).map(|&g| (g, why))).collect();
    }
    Ok(WorksSpot { at, name: works_name(world, t0, castle, at, id), in_way, in_way_names, pts: joined, gates_not_built })
}

/// The name a castle or wall `id` standing at `at` is given when none is asked for (the nearest
/// settlement's culture).
pub fn works_name(world: &World, t0: &T0, castle: bool, at: P, id: &str) -> String {
    use crate::t0::names::{NameKind, Namer};
    let culture = t0.settlements.iter().min_by(|a, b| (a.x - at[0]).hypot(a.y - at[1]).total_cmp(&(b.x - at[0]).hypot(b.y - at[1]))).map_or(0, |s| s.culture as usize);
    let mut namer = Namer::new(crate::core::hash::fnv64(id.as_bytes()) ^ world.seed);
    let mut word = namer.name(NameKind::Settlement, culture);
    for _ in 0..8 {
        if !named(t0, &word) {
            break;
        }
        word = namer.name(NameKind::Settlement, culture);
    }
    format!("{word} {}", if castle { "Castle" } else { "Wall" })
}

/// (Roadmap 6 P7) The walls someone named, drawn heavier at city scale and lettered along their
/// lines: walls drawn by hand given a name of their own (not the one `works_name` gives them),
/// and hand towns' wall lines with a name. (id `c:<n>` or `<layout>/m<id>`, name, line in world
/// ft, closed lines ending where they began.) With `layout`, only that layout's.
pub fn named_walls(world: &World, t0: &T0, layout: Option<usize>) -> Vec<(String, String, Vec<P>)> {
    let generated = t0.settlements.len() + t0.base_pois;
    let mut out = Vec::new();
    let e = &world.file.edits;
    let ring = |pts: &[P], closed: bool| pts.iter().copied().chain(pts.first().copied().filter(|_| closed && pts.len() > 2)).collect::<Vec<P>>();
    let created: Box<dyn Iterator<Item = (usize, &crate::world::Created)>> = match layout {
        Some(li) => Box::new(li.checked_sub(generated).and_then(|k| e.created.get(k).map(|c| (k, c))).into_iter()),
        None => Box::new(e.created.iter().enumerate()),
    };
    for (_, c) in created {
        if c.kind != "wall" || c.removed || c.pts.len() < 2 || c.name.trim().is_empty() || c.name == works_name(world, t0, false, [c.x, c.y], &c.id) {
            continue;
        }
        out.push((c.id.clone(), c.name.trim().to_string(), ring(&c.pts, c.closed)));
    }
    let (from, to) = match layout {
        Some(li) if li >= generated => return out,
        Some(li) => (format!("{li}/m"), format!("{li}/n")),
        None => (String::new(), String::from("\u{10ffff}")),
    };
    for (key, v) in e.streets.range(from..to) {
        let Some((li, rest)) = key.split_once('/') else { continue };
        if !rest.starts_with('m') || rest[1..].parse::<u32>().is_err() || li.parse::<usize>().is_err() {
            continue;
        }
        let Some(name) = v.get("name").and_then(|n| n.as_str()).map(str::trim).filter(|n| !n.is_empty()) else { continue };
        let pts: Vec<P> = v.get("pts").and_then(|p| serde_json::from_value(p.clone()).ok()).unwrap_or_default();
        if pts.len() >= 2 {
            out.push((key.clone(), name.to_string(), ring(&pts, v.get("closed").and_then(|c| c.as_bool()).unwrap_or(false))));
        }
    }
    out
}

/// `works_spot` as JSON: `{x, y, name, in_way: [{id, at}]}` or `{error}`. The castle or wall is
/// given as a created site (`kind`, `poly` or `pts`, `closed`), `id` its id (`c:<n>`); one
/// being redrawn is not in its own way.
pub fn works_spot_json(world: &World, t0: &T0, created: &str) -> String {
    let c: crate::world::Created = match serde_json::from_str(created) {
        Ok(c) => c,
        Err(e) => return json!({ "error": e.to_string() }).to_string(),
    };
    let skip = layout_of(world, t0, &c.id);
    match works_spot(world, t0, &c, &c.id, skip) {
        Ok(s) => {
            let mut v = json!({ "x": s.at[0], "y": s.at[1], "name": s.name, "in_way": s.in_way.iter().map(|(id, at)| json!({ "id": id, "at": at })).collect::<Vec<_>>() });
            if let Some(p) = s.pts {
                v["pts"] = json!(p);
            }
            if !s.gates_not_built.is_empty() {
                v["gates_not_built"] = json!(s.gates_not_built.iter().map(|(g, why)| json!({ "side": g, "why": why })).collect::<Vec<_>>());
            }
            v
        }
        Err(e) => json!({ "error": e }),
    }
    .to_string()
}

/// `generated_buildings_in` as JSON: `[{id, at}]`.
pub fn generated_buildings_in_json(world: &World, t0: &T0, poly: &[P]) -> String {
    Value::Array(generated_buildings_in(world, t0, poly).into_iter().map(|(id, at)| json!({ "id": id, "at": at })).collect()).to_string()
}

/// The edit that takes generated building `id` away.
pub fn building_removal(world: &World, t0: &T0, id: &str) -> Result<crate::world::BuildingEdit, String> {
    if let Some((k, ..)) = drawn_pair(world, t0, id) {
        return Err(if world.file.edits.created[k].removed { format!("{id} is already deleted (restore_feature brings it back)") } else { format!("{id} is drawn by hand: delete_feature deletes it") });
    }
    let (li, bi) = generated_building(t0, id).ok_or_else(|| town::pieces::not_generated(world, t0, id))?;
    let base = town::base_layout(world, t0, li);
    let g = base.building(bi as usize).ok_or_else(|| format!("no such building: {id}"))?;
    Ok(crate::world::BuildingEdit { at: geom::centroid(&g.poly), removed: true, ..Default::default() })
}

/// What `building_spot` leaves out for the building `id`: a generated building (`b:<layout>:<id>`
/// in a settlement or generated site) itself, a drawn building's (`c:<n>`, or its `b:<layout>:0`)
/// whole layout.
pub fn spot_skip(world: &World, t0: &T0, id: &str) -> Option<(usize, Option<u32>)> {
    if let Some((li, bi)) = generated_building(t0, id) {
        return Some((li, Some(bi)));
    }
    layout_of(world, t0, id).map(|li| (li, None))
}

/// A generated building's id (`b:<layout>:<id>` in a settlement or a generated site, not a
/// building drawn by hand): (layout, id).
pub fn generated_building(t0: &T0, id: &str) -> Option<(usize, u32)> {
    let mut parts = id.split(':');
    if parts.next()? != "b" {
        return None;
    }
    let li: usize = parts.next()?.parse().ok()?;
    let bi: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || (li >= t0.settlements.len() + t0.base_pois && !town::is_settlement(t0, li)) {
        return None;
    }
    Some((li, bi))
}

/// What a building drawn by hand can be: businesses (catalog key, name, category) and homes.
pub fn building_funcs_json() -> String {
    let businesses: Vec<Value> = town::catalog::CATALOG.iter().map(|f| json!({ "key": f.key, "name": f.name, "category": f.category })).collect();
    let homes: Vec<Value> = crate::world::BUILDING_HOMES.iter().zip(town::catalog::RESIDENTIAL).map(|(k, n)| json!({ "key": k, "name": n })).collect();
    json!({ "businesses": businesses, "homes": homes }).to_string()
}

/// `building_spot` as JSON: `{x, y, name}` or `{error, short}` (`short`: what to tell a person, with no ids or world-ft points).
pub fn building_spot_json(world: &World, t0: &T0, poly: &[P], func: Option<&str>, id: &str) -> String {
    let skip = spot_skip(world, t0, id);
    match building_spot(world, t0, poly, func, id, skip) {
        Ok((q, name)) => {
            let mut v = json!({ "x": q[0], "y": q[1], "name": name });
            // (R5 M20: on a hand bridge's deck, the bridge it stands on: `Created::on`.)
            if let Some((on, None)) = crate::wharf::deck_under(world, poly) {
                v["on"] = json!(on);
            }
            v
        }
        Err(e) => {
            let short = spot_blockers(world, t0, poly, skip, false).ok().and_then(|(v, _)| v.into_iter().next()).map(|b| b.short()).unwrap_or_else(|| e.clone());
            json!({ "error": e, "short": short })
        }
    }
    .to_string()
}

/// Whether the world's own buildings `ids` (taken away or changed by edits) may come back: none may lie under a building drawn by hand.
/// `Err` names the first that would, and the drawn building to delete first.
pub fn restore_clash(world: &World, t0: &T0, ids: &[String]) -> Result<(), String> {
    let generated = t0.settlements.len() + t0.base_pois;
    for id in ids {
        let Some((li, bi)) = generated_building(t0, id) else { continue };
        // (Taken away or changed by `update_building`: the generated footprint is what comes back.)
        if !world.file.edits.buildings.contains_key(id) {
            continue;
        }
        let Some(poly) = town::base_layout(world, t0, li).building(bi as usize).map(|g| g.poly.clone()) else { continue };
        let inner = inner_of(&poly);
        for l in town::layouts_near(world, t0, bbox(&poly)) {
            let drawn = (l.index as usize >= generated).then(|| world.file.edits.created.get(l.index as usize - generated)).flatten().filter(|c| c.kind == "building");
            if let Some(c) = drawn
                && l.buildings.iter().any(|b| polys_overlap(&inner, &b.poly))
            {
                return Err(format!("{id} would come back under {} (drawn by hand): remove {} first", c.id, c.id));
            }
        }
    }
    Ok(())
}

/// Whether two polygons share any area (or one holds the other).
pub fn polys_overlap_pub(a: &[P], b: &[P]) -> bool {
    polys_overlap(a, b)
}

/// Whether two polygons share any area (or one holds the other).
pub(super) fn polys_overlap(a: &[P], b: &[P]) -> bool {
    a.iter().any(|p| geom::contains(b, *p)) || b.iter().any(|p| geom::contains(a, *p)) || (0..a.len()).any(|i| (0..b.len()).any(|j| geom::segs_cross_mul(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()])))
}

/// Where lines a-b and c-d meet (a-b's middle if they run alike).
fn seg_meet(a: P, b: P, c: P, d: P) -> P {
    let (r, q) = (geom::sub(b, a), geom::sub(d, c));
    let den = r[0] * q[1] - r[1] * q[0];
    if den.abs() < 1e-12 {
        return geom::lerp(a, b, 0.5);
    }
    let t = ((c[0] - a[0]) * q[1] - (c[1] - a[1]) * q[0]) / den;
    geom::add(a, geom::mul(r, t))
}

/// Distance from a segment to a polygon (0 when it crosses or lies inside).
pub(super) fn poly_seg_dist(poly: &[P], a: P, b: P) -> f64 {
    let n = poly.len();
    if geom::contains(poly, a) || geom::contains(poly, b) || (0..n).any(|i| geom::segs_cross_mul(poly[i], poly[(i + 1) % n], a, b)) {
        return 0.0;
    }
    let to_edges = (0..n).flat_map(|i| [geom::seg_dist(a, poly[i], poly[(i + 1) % n]), geom::seg_dist(b, poly[i], poly[(i + 1) % n])]);
    poly.iter().map(|p| geom::seg_dist(*p, a, b)).chain(to_edges).fold(f64::MAX, f64::min)
}

/// `creation_spot` as JSON: `{x, y, name}` or `{error}`. The kind `settlement` (the app's land check for a settlement put down by hand) has no overlap check.
pub fn creation_spot_json(world: &World, t0: &T0, kind: &str, under: Option<&str>, id: &str, p: [f64; 2]) -> String {
    let overlap = if kind == "settlement" { Overlap::Skip } else { Overlap::RefuseShort };
    match creation_spot_with(world, t0, kind, under, id, p, overlap) {
        Ok((q, name)) => json!({ "x": q[0], "y": q[1], "name": name }),
        Err(e) => json!({ "error": e }),
    }
    .to_string()
}

/// Why a castle's or wall's line is refused where it meets the walls of layout `l`: the wall's id
/// (a wall or castle drawn by hand, else the town's own walls) and where (world ft).
pub fn wall_crossed(world: &World, t0: &T0, l: &crate::town::Layout, segs: &[(P, P)]) -> String {
    let id = crate::agent::feature_of_layout(world, t0, l.index as usize).unwrap_or_else(|| format!("layout {}", l.index));
    let what = if l.site { format!("wall {id}") } else { format!("the town wall of {id}") };
    let mut at: Option<P> = None;
    'find: for w in &l.walls {
        for s in w.windows(2) {
            for &(a, b) in segs {
                if let Some(t) = cut(a, b, s[0], s[1]) {
                    at = Some(lerp(a, b, t));
                    break 'find;
                }
                if seg_dist(s[0], a, b) < 1.0 {
                    at = Some(s[0]);
                    break 'find;
                }
            }
        }
    }
    let at = at.or_else(|| l.walls.iter().flatten().next().copied()).unwrap_or([0.0, 0.0]);
    format!("that crosses {what} at ({:.0}, {:.0})", at[0], at[1])
}
