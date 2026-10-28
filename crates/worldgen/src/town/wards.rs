//! The ward editor (`Edits.towns`): a town laid out anew by hand. Its patches' corners are moved
//! (as watabou's Warp tools move them: one at a time, or by brushes), patches are given another
//! ward, lot size, a merge with a neighbour or a fresh roll, and its walls are put up or taken
//! down. The changes go in between planning the town and building it (`plan_town`,
//! `build_town`), so what was decided on the plan (gates, main streets, district groups and
//! names, the towers along each wall) stays, and only the patches touched come out differently.
//!
//! Buildings that come out as they were generated keep their ids and everything decided about
//! them (function, name, storeys); new ones get ids of their own patch (`NEW_ID`), so editing
//! one ward never renumbers another's. A function the town had that went with the buildings
//! taken away goes to a new building where one fits; otherwise the change wins, and the reply
//! says what was lost.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde::Deserialize;
use serde_json::{Value, json};

use super::catalog::{self, CATALOG, Ward};
use super::geom::*;
use super::mesh::Mesh;
use super::{Building, Layout, ParkKind, PlazaKind, Structure, TownPlan};
use crate::World;
use crate::core::hash::{FastMap, FastSet, Fnv64};
use crate::core::rng::{Pcg32, hash3, unit};
use crate::t0::T0;
use crate::t0::settle::Tier;
use crate::world::{CornerMove, LOT_SIZES, PARK_KINDS, PatchEdit, SQUARE_KINDS, SquareEdit, TOWN_WARDS, TownEdit};

/// Ids of the buildings a town edit adds: `NEW_ID + patch × PER_PATCH + k` (k in the order its
/// patch builds them); in a patch rolled again (`reroll` r ≥ 1), `ROLLED_ID + (patch × ROLLS +
/// (r − 1) mod ROLLS) × PER_PATCH + k`, so a roll's buildings never take the last roll's ids
/// (and what was said about them is set aside), except a business, which keeps its id and name
/// wherever it goes (`functions`).
pub const NEW_ID: u32 = 1_000_000;
pub const PER_PATCH: u32 = 10_000;
pub const ROLLED_ID: u32 = 100_000_000;
pub const ROLLS: u32 = 64;

/// Each lot size's target lot area, times the ward's usual (`LOT_SIZES` order).
const LOT_FACTOR: [f64; 4] = [0.6, 1.2, 2.4, 4.0];

/// A town edit as the builder takes it (local ft, the plan's corners and patches).
#[derive(Default)]
pub(super) struct Changes {
    /// Corners moved (ascending corner).
    pub moves: Vec<(usize, P)>,
    /// Patches given another ward (`None`: open ground).
    pub ward: BTreeMap<usize, Option<Ward>>,
    /// Target lot area as a multiple of the ward's usual.
    pub lots: BTreeMap<usize, f64>,
    pub reroll: BTreeMap<usize, u32>,
    /// (patch, the neighbour whose district it joins).
    pub merge: Vec<(usize, usize)>,
    pub walls: Option<bool>,
    /// Squares where streets meet set by hand (corner → kind; `None`: no square there).
    pub squares: BTreeMap<usize, Option<PlazaKind>>,
    /// Temple patches kept as a close round the temple, or not.
    pub close: BTreeMap<usize, bool>,
    /// Parks laid out as a formal garden or a common, set by hand.
    pub park: BTreeMap<usize, ParkKind>,
}

impl Changes {
    /// Wards and district groups with the patches set by hand: a patch given another ward leaves
    /// its group; one merged joins its neighbour's group, ward and all.
    pub fn wards(&self, plan: &TownPlan) -> (Vec<Option<Ward>>, Vec<usize>) {
        let (mut ward, mut group) = (plan.ward.clone(), plan.group.clone());
        for (&p, &w) in &self.ward {
            if w != plan.ward[p] {
                ward[p] = w;
                group[p] = usize::MAX;
            }
        }
        for &(p, q) in &self.merge {
            if group[q] == usize::MAX {
                group[q] = q;
            }
            ward[p] = ward[q];
            group[p] = group[q];
        }
        (ward, group)
    }
}

/// Wards that build on lots and so can share a district (`plan_town`'s groups).
fn groupable(w: Option<Ward>) -> bool {
    matches!(w, Some(Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Docks | Ward::Military | Ward::Temple))
}

/// A ward's name (`TOWN_WARDS`; `None`: "empty").
pub fn ward_name(w: Option<Ward>) -> &'static str {
    w.map_or("empty", Ward::name)
}

fn parse_ward(name: &str) -> Result<Option<Ward>, String> {
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
fn parse_square(name: &str) -> Result<Option<PlazaKind>, String> {
    if name == "none" {
        return Ok(None);
    }
    super::squares::corner_kind(name).map(Some).ok_or_else(|| format!("a square is one of {} or none", SQUARE_KINDS.join(", ")))
}

/// A park's kind by name (`PARK_KINDS`).
fn parse_park(name: &str) -> Result<ParkKind, String> {
    ParkKind::parse(name).ok_or_else(|| format!("a park is one of {} (or auto)", PARK_KINDS.join(", ")))
}

fn lot_factor(name: &str) -> Result<f64, String> {
    LOT_SIZES.iter().position(|l| *l == name).map(|k| LOT_FACTOR[k]).ok_or_else(|| format!("lots must be one of {}", LOT_SIZES.join(", ")))
}

/// Whether corner `v` (local ft) may be moved from `from` to `to`.
fn corner_ok(plan: &TownPlan, v: usize, from: P, to: P) -> Result<(), String> {
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
fn square_here(plan: &TownPlan, v: usize, at: P) -> Result<(), String> {
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
    let s = &t0.settlements[index];
    let face: Vec<P> = plan.mesh.face_pts(p).iter().map(|q| add(*q, [s.x, s.y])).collect();
    super::generated_layout(world, t0, index).buildings.iter().any(|b| b.func == Some(fi as u16) && contains(&face, centroid(&b.poly)))
}

/// Whether patch `p` is one to set (`at`: its middle as planned, local ft).
fn patch_here(plan: &TownPlan, p: usize, at: P) -> Result<(), String> {
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
fn merge_ok(plan: &TownPlan, ward: &[Option<Ward>], p: usize, q: usize) -> Result<(), String> {
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

/// The changes a town edit makes to its plan (`center`: the town's, world ft), and the parts of
/// it set aside (why).
pub(super) fn changes(plan: &TownPlan, e: &TownEdit, center: P) -> (Changes, Vec<String>) {
    let mut ch = Changes { walls: e.walls, ..Default::default() };
    let mut aside = Vec::new();
    let local = |p: [f64; 2]| sub(p, center);
    for c in &e.corners {
        match corner_ok(plan, c.v as usize, local(c.from), local(c.to)) {
            Ok(()) => ch.moves.push((c.v as usize, local(c.to))),
            Err(why) => aside.push(format!("corner {}: {why}", c.v)),
        }
    }
    ch.moves.sort_by_key(|m| m.0);
    ch.moves.dedup_by_key(|m| m.0);
    let mut merges = Vec::new();
    for pe in &e.patches {
        let p = pe.p as usize;
        if let Err(why) = patch_here(plan, p, local(pe.at)) {
            aside.push(format!("patch {p}: {why}"));
            continue;
        }
        match pe.ward.as_deref().map(parse_ward) {
            Some(Ok(w)) => {
                ch.ward.insert(p, w);
            }
            Some(Err(why)) => aside.push(format!("patch {p}: {why}")),
            None => {}
        }
        match pe.lots.as_deref().map(lot_factor) {
            Some(Ok(f)) => {
                ch.lots.insert(p, f);
            }
            Some(Err(why)) => aside.push(format!("patch {p}: {why}")),
            None => {}
        }
        if pe.reroll > 0 {
            ch.reroll.insert(p, pe.reroll);
        }
        if let Some(q) = pe.merge_with {
            merges.push((p, q as usize));
        }
        if let Some(on) = pe.close {
            ch.close.insert(p, on);
        }
        match pe.park.as_deref().map(parse_park) {
            Some(Ok(k)) => {
                ch.park.insert(p, k);
            }
            Some(Err(why)) => aside.push(format!("patch {p}: {why}")),
            None => {}
        }
    }
    for se in &e.squares {
        let v = se.v as usize;
        match square_here(plan, v, local(se.at)).and_then(|()| parse_square(&se.kind)) {
            Ok(k) => {
                ch.squares.insert(v, k);
            }
            Err(why) => aside.push(format!("square at corner {v}: {why}")),
        }
    }
    // Merges once every ward is set (a patch joins a neighbour as that one now is).
    let (ward, _) = ch.wards(plan);
    for (p, q) in merges {
        match (q < plan.mesh.faces.len()).then(|| merge_ok(plan, &ward, p, q)).unwrap_or_else(|| Err("no such patch".into())) {
            Ok(()) => ch.merge.push((p, q)),
            Err(why) => aside.push(format!("patch {p}: {why}")),
        }
    }
    (ch, aside)
}

/// The plan's mesh with corners moved (ascending corner; pinned ones stay): each goes as far as
/// keeps every patch round it convex (`Mesh::try_move`: all the way, half, a quarter or not at
/// all), over a few rounds so corners moved together (a brush stroke) make room for each other.
pub(super) fn moved_mesh(plan: &TownPlan, moves: &[(usize, P)]) -> Mesh {
    let mut mesh = plan.mesh.clone();
    for _ in 0..4 {
        let mut any = false;
        for &(v, to) in moves {
            if v < mesh.pos.len() && !plan.pinned[v] && dist(mesh.pos[v], to) > 0.01 {
                any |= mesh.try_move(&plan.vf, v, to);
            }
        }
        if !any {
            break;
        }
    }
    mesh
}

// ---------------------------------------------------------------------------------------
// Memo: a town's plan (per world), and its layout with each edit (per edit), so a change to
// it builds the town again without planning it again.

thread_local! {
    static PLANS: RefCell<(u64, FastMap<u32, Rc<TownPlan>>)> = RefCell::new((0, FastMap::default()));
    static TOWNS: RefCell<FastMap<u32, (u64, Rc<Layout>)>> = RefCell::new(FastMap::default());
}

/// A town's plan, made once per world (`make`) and kept.
pub(super) fn plan(world: &World, index: usize, make: impl FnOnce() -> TownPlan) -> Rc<TownPlan> {
    let key = index as u32;
    if let Some(p) = PLANS.with(|c| {
        let c = c.borrow();
        if c.0 == world.hash { c.1.get(&key).cloned() } else { None }
    }) {
        return p;
    }
    let p = Rc::new(make());
    PLANS.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != world.hash {
            c.0 = world.hash;
            c.1.clear();
        }
        c.1.insert(key, p.clone());
    });
    p
}

pub(super) fn forget_from(index: usize) {
    PLANS.with(|c| c.borrow_mut().1.retain(|&k, _| (k as usize) < index));
    TOWNS.with(|c| c.borrow_mut().retain(|&k, _| (k as usize) < index));
}

pub(super) fn forget(gone: &FastSet<u32>) {
    PLANS.with(|c| c.borrow_mut().1.retain(|k, _| !gone.contains(k)));
    TOWNS.with(|c| c.borrow_mut().retain(|k, _| !gone.contains(k)));
}

/// The key of a town's entry in `Edits.towns`.
pub fn key(index: usize) -> String {
    index.to_string()
}

/// The town edit that applies to layout `index`: a town's (not a village's), with the town still
/// where it was edited, changing something.
pub fn edit_of<'a>(world: &'a World, t0: &T0, index: usize) -> Option<&'a TownEdit> {
    if world.file.edits.towns.is_empty() {
        return None;
    }
    let s = t0.settlements.get(index).filter(|s| s.tier >= Tier::Town)?;
    let e = world.file.edits.towns.get(&key(index))?;
    (!e.is_noop() && dist(e.at, [s.x, s.y]) <= TownEdit::SAME_AT_FT).then_some(e)
}

/// A hash of the world and a town edit.
fn hash_edit(world: &World, e: &TownEdit) -> u64 {
    let mut h = Fnv64::default();
    h.write(&world.hash.to_le_bytes());
    for c in &e.corners {
        h.write(&c.v.to_le_bytes());
        for x in c.from.iter().chain(&c.to) {
            h.write(&x.to_le_bytes());
        }
    }
    h.write(&[0xfd]);
    for p in &e.patches {
        h.write(&p.p.to_le_bytes());
        for x in &p.at {
            h.write(&x.to_le_bytes());
        }
        for t in [&p.ward, &p.lots] {
            h.write(t.as_deref().unwrap_or("-").as_bytes());
            h.write(&[0xff]);
        }
        h.write(&p.merge_with.map_or(u32::MAX, |q| q).to_le_bytes());
        h.write(&p.reroll.to_le_bytes());
        h.write(&[p.close.map_or(2, |c| c as u8)]);
        h.write(p.park.as_deref().unwrap_or("-").as_bytes());
        h.write(&[0xff]);
    }
    h.write(&[e.walls.map_or(2, |w| w as u8), 0xfe]);
    for q in &e.squares {
        h.write(&q.v.to_le_bytes());
        for x in &q.at {
            h.write(&x.to_le_bytes());
        }
        h.write(q.kind.as_bytes());
        h.write(&[0xff]);
    }
    h.finish()
}

/// A hash of the world and the town edit that applies to layout `index`, if any.
pub fn fingerprint(world: &World, t0: &T0, index: usize) -> Option<u64> {
    edit_of(world, t0, index).map(|e| hash_edit(world, e))
}

/// Layout `index` as its town edit lays it out, if one applies.
pub(super) fn edited(world: &World, t0: &T0, index: usize) -> Option<Rc<Layout>> {
    edit_of(world, t0, index).map(|e| with_edit(world, t0, index, e))
}

/// Town `index` laid out with edit `e` (whatever `Edits.towns` holds: a change tried first).
pub fn with_edit(world: &World, t0: &T0, index: usize, e: &TownEdit) -> Rc<Layout> {
    let key = index as u32;
    let fp = hash_edit(world, e);
    if let Some(l) = TOWNS.with(|c| c.borrow().get(&key).filter(|x| x.0 == fp).map(|x| x.1.clone())) {
        return l;
    }
    let l = Rc::new(super::generate_with(world, t0, index, Some(e), None));
    TOWNS.with(|c| c.borrow_mut().insert(key, (fp, l.clone())));
    l
}

// ---------------------------------------------------------------------------------------
// Ids: what came out as generated keeps its id.

/// Give the buildings of a town laid out anew (`id`: the patch each stands on) their ids: one
/// with a footprint the generated layout `generated` has, or lying mostly on one (a lot trimmed
/// back by a square), takes that building's id and what was decided about it; the rest are new
/// (`NEW_ID`), with functions the town lost (none further than `spacing` and a half from where
/// they stood: about a patch away), homes and storeys of their own. Leaves the buildings in id
/// order.
pub(super) fn inherit(world: &World, t0: &T0, edit: &TownEdit, generated: &Layout, spacing: f64, l: &mut Layout) {
    let rolls: FastMap<u32, u32> = edit.patches.iter().filter(|p| p.reroll > 0).map(|p| (p.p, p.reroll)).collect();
    const CELL: f64 = 2.0;
    let cell = |p: P| (crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64);
    let mut grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
    // (A town hall's lot as generated: under its belfry, its body (`belfry`).)
    let lot = |b: &'_ Building| -> Vec<P> { super::belfry::of(b).map_or_else(|| b.poly.clone(), |_| b.parts[0].poly.clone()) };
    for (k, b) in generated.buildings.iter().enumerate() {
        grid.entry(cell(centroid(&lot(b)))).or_default().push(k as u32);
    }
    let same = |a: &[P], b: &[P]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| dist(*p, *q) < 0.01);
    let mut used = vec![false; generated.buildings.len()];
    let mut next: BTreeMap<u32, u32> = BTreeMap::new();
    let mut fresh: Vec<usize> = Vec::new();
    // (Each building's patch, while its id still is that.)
    let patch: Vec<u32> = l.buildings.iter().map(|b| b.id).collect();
    for (k, b) in l.buildings.iter_mut().enumerate() {
        let (cx, cy) = cell(centroid(&b.poly));
        let hit = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (cx + dx, cy + dy))).filter_map(|c| grid.get(&c)).flatten().map(|&g| g as usize).find(|&g| !used[g] && same(&lot(&generated.buildings[g]), &b.poly));
        match hit {
            Some(g) => {
                used[g] = true;
                let o = &generated.buildings[g];
                (b.id, b.func, b.residential, b.floors, b.structure, b.roof, b.tint) = (o.id, o.func, o.residential, o.floors, o.structure, o.roof, o.tint);
                b.name = o.name.clone();
            }
            None => {
                let n = next.entry(b.id).or_insert(0);
                b.id = match rolls.get(&b.id) {
                    Some(r) => ROLLED_ID + (b.id * ROLLS + (r - 1) % ROLLS) * PER_PATCH + *n,
                    None => NEW_ID + b.id * PER_PATCH + *n,
                };
                *n += 1;
                fresh.push(k);
            }
        }
    }
    // A temple's lot giving the temple as generated on it (`temple::gives`) is that temple.
    for k in fresh.clone() {
        let at = centroid(&l.buildings[k].poly);
        let hit = (0..generated.buildings.len()).find(|&g| !used[g] && super::temple::shaped(&generated.buildings[g]) && dist(centroid(&generated.buildings[g].poly), at) < 80.0 && super::temple::gives(&l.buildings[k].poly, &generated.buildings[g]));
        if let Some(g) = hit {
            used[g] = true;
            let (o, b) = (&generated.buildings[g], &mut l.buildings[k]);
            (b.id, b.func, b.residential, b.floors, b.structure, b.roof, b.tint) = (o.id, o.func, o.residential, o.floors, o.structure, o.roof, o.tint);
            b.name = o.name.clone();
            fresh.retain(|&j| j != k);
        }
    }
    // A new building lying mostly on one generated that none took, of the same structure (half
    // of each on the other: a lot trimmed back by a square or a close) is that building, cut down;
    // not in a patch whose lots were rolled, sized anew or given another ward (a roll never takes
    // another's ids).
    let reach = 1.5 * spacing;
    let relotted: Vec<u32> = edit.patches.iter().filter(|p| p.reroll > 0 || p.lots.is_some() || p.ward.is_some()).map(|p| p.p).collect();
    for k in fresh.clone() {
        let b = &l.buildings[k];
        if b.func.is_some() || relotted.contains(&patch[k]) {
            continue;
        }
        let (at, whole) = (centroid(&b.poly), area(&b.poly).abs());
        let hit = (0..generated.buildings.len())
            .filter(|&g| !used[g] && generated.buildings[g].structure == b.structure && dist(centroid(&generated.buildings[g].poly), at) < 80.0)
            .map(|g| (super::overlap(&b.poly, &generated.buildings[g].poly), g))
            .filter(|&(a, g)| a >= 0.5 * whole && a >= 0.5 * area(&generated.buildings[g].poly).abs())
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
        if let Some((_, g)) = hit {
            used[g] = true;
            let (o, b) = (&generated.buildings[g], &mut l.buildings[k]);
            (b.id, b.func, b.residential, b.floors, b.structure, b.roof, b.tint) = (o.id, o.func, o.residential, o.floors, o.structure, o.roof, o.tint);
            b.name = o.name.clone();
            fresh.retain(|&j| j != k);
        }
    }
    // A building laid out for a purpose (a close's temple) takes the place, id and name of one the
    // town lost with that purpose (the nearest, about a patch away at most; notes and people there
    // follow it); else of one kept as generated on its patch, which is a home again with the new
    // one's id.
    let mut lost: Vec<(usize, P, u32, Option<String>, bool)> = generated
        .buildings
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .filter_map(|(b, _)| b.func.map(|f| (f as usize, centroid(&b.poly), b.id, b.name.clone(), b.ward == Ward::Castle)))
        .collect();
    // (Nearest pairs first, so each takes its own patch's before a neighbour's.)
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for &k in &fresh {
        let Some(f) = l.buildings[k].func else { continue };
        let at = centroid(&l.buildings[k].poly);
        pairs.extend((0..lost.len()).filter(|&i| lost[i].0 == f as usize && dist(lost[i].1, at) <= reach).map(|i| (dist(lost[i].1, at), k, i)));
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (mut placed, mut gone) = (Vec::new(), vec![false; lost.len()]);
    for (_, k, i) in pairs {
        if placed.contains(&k) || gone[i] {
            continue;
        }
        (l.buildings[k].id, l.buildings[k].name) = (lost[i].2, lost[i].3.clone());
        placed.push(k);
        gone[i] = true;
    }
    let mut i = 0;
    lost.retain(|_| {
        i += 1;
        !gone[i - 1]
    });
    fresh.retain(|k| !placed.contains(k));
    for k in fresh.clone() {
        let Some(f) = l.buildings[k].func else { continue };
        if let Some(j) = (0..l.buildings.len()).find(|&j| !fresh.contains(&j) && patch[j] == patch[k] && l.buildings[j].func == Some(f) && l.buildings[j].structure == Structure::Roofed) {
            let new_id = l.buildings[k].id;
            (l.buildings[k].id, l.buildings[k].name) = (l.buildings[j].id, l.buildings[j].name.take());
            let b = &mut l.buildings[j];
            b.id = new_id;
            b.func = None;
            b.residential = catalog::residential_for(b.ward, area(&b.poly).abs() > 2500.0) as u8;
            fresh.retain(|&x| x != k);
            fresh.push(j);
        }
    }
    functions(world, t0, l, &fresh, lost, reach);
    l.buildings.sort_by_key(|b| b.id);
}

/// What the new buildings of a town laid out anew are: a castle's keep is the castle; functions
/// the town lost (each with where it stood, its id and name) go to the new buildings within
/// `reach` of where it stood that suit them best (as `assign_functions` picks, with noise per
/// building), else to the nearest home within `reach`, else nowhere (`functions_lost` lists it);
/// the building that takes one takes the lost one's id and name too (its notes and people follow
/// it; a home's own id goes, as a building taken away does). The rest are homes. Names and storeys
/// as the town's own. While a castle stands in the town laid out anew, what a castle lost stays in
/// a castle's buildings and the town's in the town's: how many ranges a castle has (more where the
/// town wall is its wall) moves no other building's id.
fn functions(world: &World, t0: &T0, l: &mut Layout, fresh: &[usize], mut lost: Vec<(usize, P, u32, Option<String>, bool)>, reach: f64) {
    let stream = world.stream("town.edit");
    let index = super::rng_key(t0, l.index as usize);
    let noise = |id: u32, salt: i64| unit(hash3(stream, index, id as i64, salt));
    let castle = catalog::index_of("castle");
    for &k in fresh {
        let b = &mut l.buildings[k];
        if let Some(c) = castle
            && b.keep.is_some()
            && b.func.is_none()
        {
            b.func = Some(c as u16);
            if let Some(i) = lost.iter().position(|x| x.0 == c) {
                lost.remove(i);
            }
        }
    }
    lost.sort_by(|a, b| (!CATALOG[a.0].big, a.0).cmp(&(!CATALOG[b.0].big, b.0)).then(a.1[0].total_cmp(&b.1[0])).then(a.1[1].total_cmp(&b.1[1])));
    let mut adopted: Vec<usize> = Vec::new();
    let areas: Vec<f64> = fresh.iter().map(|&k| area(&l.buildings[k].poly).abs()).collect();
    let max_area = areas.iter().cloned().fold(1.0, f64::max);
    let mut taken: Vec<bool> = fresh.iter().map(|&k| l.buildings[k].func.is_some()).collect();
    let mut named: Vec<usize> = Vec::new();
    // (Kept as generated or new: a castle with no new building still keeps its own.)
    let castle_here = l.buildings.iter().any(|b| b.ward == Ward::Castle);
    for (fi, was, was_id, was_name, was_castle) in lost {
        let f = &CATALOG[fi];
        // (A castle's to a castle's building while one stands; the town's never to one.)
        let in_castle = was_castle && castle_here;
        let mut best: Option<(f64, usize)> = None;
        for (j, &k) in fresh.iter().enumerate() {
            let b = &l.buildings[k];
            if taken[j] || b.structure != Structure::Roofed || dist(centroid(&b.poly), was) > reach || (b.ward == Ward::Castle && !was_castle) || (in_castle && b.ward != Ward::Castle) {
                continue;
            }
            let ward_score = f.wards.iter().position(|w| *w == b.ward).map_or(0.0, |p| 10.0 - 2.0 * p as f64);
            let size = areas[j] / max_area;
            let size_score = if f.big { 6.0 * size } else { -2.0 * size };
            let cramped = if areas[j] < 500.0 { 100.0 } else { 0.0 };
            let score = ward_score + size_score - cramped + 1.5 * noise(b.id, fi as i64);
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, j));
            }
        }
        let Some((_, j)) = best else {
            // None new will do (a close taken away, its temple with it): the nearest home of the
            // function's own wards about a patch from where it stood takes it on, else the nearest
            // home there, else it stays lost (a graveyard too: a home is no open ground).
            if f.key == "graveyard" || in_castle {
                continue;
            }
            let homes = |own: bool| {
                (0..l.buildings.len())
                    .filter(|k| !fresh.contains(k) && !adopted.contains(k))
                    .filter(|&k| {
                        let b = &l.buildings[k];
                        b.func.is_none() && b.structure == Structure::Roofed && (!own || f.wards.contains(&b.ward)) && area(&b.poly).abs() >= 500.0 && dist(centroid(&b.poly), was) <= reach
                    })
                    .min_by(|&a, &b| dist(centroid(&l.buildings[a].poly), was).total_cmp(&dist(centroid(&l.buildings[b].poly), was)).then(a.cmp(&b)))
            };
            let home = homes(true).or_else(|| homes(false));
            if let Some(k) = home {
                let b = &mut l.buildings[k];
                (b.func, b.id) = (Some(fi as u16), was_id);
                if was_name.is_some() {
                    b.name = was_name;
                    named.push(k);
                }
                adopted.push(k);
            }
            continue;
        };
        taken[j] = true;
        let b = &mut l.buildings[fresh[j]];
        b.func = Some(fi as u16);
        b.id = was_id;
        if was_name.is_some() {
            b.name = was_name;
            named.push(fresh[j]);
        }
        if f.key == "graveyard" {
            b.structure = Structure::Open;
        }
    }
    let fresh: Vec<usize> = fresh.iter().copied().chain(adopted).collect();
    let fresh = &fresh[..];
    let tier = l.tier;
    let center = l.center;
    // (A new business is named unlike any other in the town: a new keep isn't the citadel's twin.)
    let mut names: std::collections::BTreeSet<String> = l.buildings.iter().filter(|b| b.func.is_some()).filter_map(|b| b.name.clone()).collect();
    for &k in fresh {
        let b = &mut l.buildings[k];
        let (id, big) = (b.id, area(&b.poly).abs() > 2500.0);
        let coin = |p: f64, salt: i64| noise(id, salt) < p;
        match b.func {
            Some(fi) => {
                let f = &CATALOG[fi as usize];
                // (One that took a lost building's place keeps its name.)
                let keep = named.contains(&k);
                if let Some(n) = &b.name
                    && !keep
                {
                    names.remove(n);
                }
                for salt in (0..8).filter(|_| !keep) {
                    let mut rng = Pcg32::new(hash3(stream, index, id as i64, 0x4e + salt), 43);
                    b.name = super::sites::trade_name(Some(fi), &mut rng, t0, center);
                    if b.name.as_ref().is_none_or(|n| !names.contains(n)) {
                        break;
                    }
                }
                names.extend(b.name.clone());
                b.floors = match f.key {
                    "castle" | "palace" => 4,
                    "inn" | "tavern" | "town_hall" | "library" | "arcane_academy" | "guildhall" => 2 + coin(0.5, 1) as u8,
                    "temple" | "warehouse" | "arena" | "graveyard" | "shrine" => 1,
                    _ => 1 + (tier >= Tier::Town && coin(0.6, 1)) as u8,
                };
            }
            None => {
                b.residential = catalog::residential_for(b.ward, big) as u8;
                b.floors = match b.ward {
                    Ward::Noble | Ward::Merchant | Ward::Castle => 2 + coin(0.4, 2) as u8,
                    Ward::Slum | Ward::Common | Ward::Craft | Ward::Docks => 1 + (tier >= Tier::City && coin(0.5, 2)) as u8,
                    _ => 1,
                };
            }
        }
    }
}

/// Ways underground of a town laid out anew: one where the generated layout has one of its kind
/// (within a foot) keeps that one's id; the rest are new (`NEW_ID` on).
pub(super) fn inherit_entrances(generated: &Layout, l: &mut Layout) {
    let mut used = vec![false; generated.entrances.len()];
    let mut n = 0;
    for e in &mut l.entrances {
        match generated.entrances.iter().enumerate().position(|(g, o)| !used[g] && o.kind == e.kind && dist(o.at, e.at) < 1.0) {
            Some(g) => {
                used[g] = true;
                e.id = generated.entrances[g].id;
            }
            None => {
                e.id = NEW_ID + n;
                n += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// What changed.

/// Where two layouts of a town differ (world ft rectangles, merged where they meet): buildings
/// by id, and every wall, street, road, square, field, block, bridge, pier, district, tower,
/// gate and way underground that one has and the other hasn't. Tiles and battlemaps there are
/// made again.
pub fn changed_rects(a: &Layout, b: &Layout) -> Vec<[f64; 4]> {
    let mut rects: Vec<[f64; 4]> = Vec::new();
    let bbox = |pts: &[P]| pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])]);
    fn ids(l: &Layout) -> FastMap<u32, &Building> {
        l.buildings.iter().map(|b| (b.id, b)).collect()
    }
    let (ia, ib) = (ids(a), ids(b));
    let same = |x: &Building, y: &Building| x.poly == y.poly && x.func == y.func && x.residential == y.residential && x.floors == y.floors && x.structure == y.structure && x.name == y.name;
    for (id, x) in &ia {
        match ib.get(id) {
            Some(y) if same(x, y) => {}
            other => {
                rects.push(bbox(&x.poly));
                if let Some(y) = other {
                    rects.push(bbox(&y.poly));
                }
            }
        }
    }
    rects.extend(ib.iter().filter(|(id, _)| !ia.contains_key(id)).map(|(_, y)| bbox(&y.poly)));
    // Everything else, as polylines (point things as one point, ± 20 ft) told apart by hash.
    let things = |l: &Layout| {
        let mut out: Vec<(u64, Vec<P>)> = Vec::new();
        let mut add = |tag: u8, extra: f64, pts: &[P]| {
            let mut h = Fnv64::default();
            h.write(&[tag]);
            h.write(&extra.to_le_bytes());
            for p in pts {
                h.write(&(crate::core::round(p[0] * 8.0) as i64).to_le_bytes());
                h.write(&(crate::core::round(p[1] * 8.0) as i64).to_le_bytes());
            }
            out.push((h.finish(), pts.to_vec()));
        };
        for (pts, class, w) in &l.roads {
            add(1, *class as f64 * 1000.0 + w, pts);
        }
        for p in &l.plazas {
            add(4, p.kind as u8 as f64, &p.pts);
        }
        for p in &l.parks {
            // (Its kind, key and sides on a street, hashed to a value a float holds exactly.)
            let h = p.fronts.iter().fold(hash3(p.key, p.kind as u8 as i64, 0, 0x16), |h, &f| hash3(h, f as i64, 0, 0x16));
            add(16, (h >> 12) as f64, &p.pts);
        }
        for (tag, list) in [(2, &l.walls), (3, &l.streets), (5, &l.fields), (6, &l.districts), (7, &l.blocks), (8, &l.bridges), (9, &l.piers), (10, &l.monuments)] {
            for pts in list {
                add(tag, 0.0, pts);
            }
        }
        for q in &l.quarters {
            add(11, 0.0, &q.label);
        }
        let pad = |p: P| [[p[0] - 20.0, p[1] - 20.0], [p[0] + 20.0, p[1] + 20.0]];
        for (t, r, gate) in crate::interior::towers(l) {
            add(12, r + gate as u8 as f64, &pad(t));
        }
        for g in &l.gates {
            add(13, 0.0, &pad(*g));
        }
        for e in &l.entrances {
            add(14, e.kind as u8 as f64, &pad(e.at));
        }
        for s in &l.stairs {
            add(15, s.width, &s.pts);
        }
        out
    };
    let (ta, tb) = (things(a), things(b));
    let mut count: FastMap<u64, i32> = FastMap::default();
    for (h, _) in &ta {
        *count.entry(*h).or_default() += 1;
    }
    for (h, _) in &tb {
        *count.entry(*h).or_default() -= 1;
    }
    for (h, pts) in ta.iter().chain(&tb) {
        if count.get(h).is_some_and(|c| *c != 0) {
            rects.push(bbox(pts));
        }
    }
    merge_rects(rects, 10.0)
}

/// Rectangles grown by `pad` and merged until none overlap.
fn merge_rects(rects: Vec<[f64; 4]>, pad: f64) -> Vec<[f64; 4]> {
    let mut out: Vec<[f64; 4]> = Vec::new();
    for r in rects.into_iter().filter(|r| r[0] <= r[2]) {
        let mut r = [r[0] - pad, r[1] - pad, r[2] + pad, r[3] + pad];
        loop {
            let Some(k) = out.iter().position(|o| o[0] <= r[2] && r[0] <= o[2] && o[1] <= r[3] && r[1] <= o[3]) else { break };
            let o = out.swap_remove(k);
            r = [r[0].min(o[0]), r[1].min(o[1]), r[2].max(o[2]), r[3].max(o[3])];
        }
        out.push(r);
    }
    out.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    out
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
    let s = t0.settlements.get(index).ok_or("not a settlement")?;
    if s.tier < Tier::Town {
        return Err("a village has no wards: it grows along its roads".into());
    }
    let plan = super::town_plan(world, t0, index).ok_or("not a town")?;
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
    let now = super::base_layout(world, t0, index);
    let built = built_closes(&now, &mesh, center);
    let generated = built_closes(&super::generated_layout(world, t0, index), &plan.mesh, center);
    let parks = built_parks(&now, &mesh, center);
    let (squares, closes) = super::squares::spots(&plan, &ch, &ward, &group, &|p| built.contains(&p));
    let world_pt = |p: P| round1(add(p, center));
    let listed = |p: usize| plan.mesh.faces[p].len() >= 3 && plan.face_ok[p] && !plan.wet[p] && (p < plan.n_inner || plan.ward[p].is_some() || ward[p].is_some());
    let mut district: FastMap<usize, usize> = FastMap::default();
    for (qi, (_, members)) in plan.quarters.iter().enumerate() {
        for &m in members {
            district.insert(m, qi);
        }
    }
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
            v["district"] = json!({ "name": world.file.edits.renames.get(&id).cloned().unwrap_or_else(|| plan.quarters[qi].0.name.clone()), "id": id });
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
        if let Some(k) = parks.get(&p) {
            v["park"] = json!(k.name());
        }
        if ward[p] == Some(Ward::Park) && ch.park.contains_key(&p) {
            v["park_by_hand"] = json!(true);
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
            v["park_refused"] = json!(super::PARK_SLIVER);
        }
        patches.push(v);
    }
    let gate = |v: usize| plan.gates.contains(&v);
    let wall: Vec<bool> = {
        let mut w = vec![false; mesh.pos.len()];
        for (chain, _, _) in &plan.wall_runs {
            for &v in chain {
                w[v] = true;
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
            if wall[v] {
                c["wall"] = json!(true);
            }
            if super::squares::junction(&plan, &ward, &group, v) {
                if super::squares::at_close(&plan, v, &closes) {
                    c["square_refused"] = json!(AT_CLOSE);
                } else {
                    c["junction"] = json!(true);
                }
            }
            let now = squares.iter().find(|s| s.0 == v).map(|s| s.1);
            if let Some(k) = now {
                c["square"] = json!(k.name());
                c["square_ft"] = json!(crate::core::round(super::squares::cut_reach(&plan, &mesh, v, k)));
            }
            let planned = plan.squares.iter().find(|s| s.0 == v).map(|s| s.1);
            if now != planned {
                c["square_generated"] = json!(planned.map_or("none", |k| k.name()));
            }
            c
        })
        .collect();
    Ok(json!({
        "layout": index,
        "center": round1(center),
        "max_move_ft": crate::core::round(plan.spacing),
        "walls": { "built": ch.walls.unwrap_or(plan.walled), "generated": plan.walled },
        "edited": e.is_some(),
        "patches": patches,
        "corners": corners,
        "set_aside": aside,
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
    let mut e = edit_of(world, t0, index).cloned().unwrap_or_default();
    // Keep only what still applies (a stale part would be set aside anyway).
    let (ch, _) = changes(&plan, &e, center);
    let mut moves: BTreeMap<usize, P> = ch.moves.iter().copied().collect();
    e.patches.retain(|pe| patch_here(&plan, pe.p as usize, local(pe.at)).is_ok());
    e.squares.retain(|se| square_here(&plan, se.v as usize, local(se.at)).is_ok() && parse_square(&se.kind).is_ok());
    match req.reset.as_deref() {
        None => {}
        Some("all") => {
            moves.clear();
            e.patches.clear();
            e.walls = None;
            e.squares.clear();
        }
        Some("corners") => moves.clear(),
        Some("patches") => e.patches.clear(),
        Some("squares") => e.squares.clear(),
        Some(r) => return Err(format!("reset: all, corners, patches or squares, not {r}")),
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
                    let nb = super::adj_vertices(&mesh, v);
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
        let sq = super::squares::spots(&plan, &ch, &ward, &group, &|_| false).0;
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
            if !super::squares::junction(&plan, &ward, &group, v) {
                return Err(format!("corner {v}: {NO_SQUARE}"));
            }
            // The closes as built now, and those asked for with this change.
            let now = built_closes(&super::base_layout(world, t0, index), &moved_mesh(&plan, &ch.moves), center);
            let asked = |p: usize| now.contains(&p) || req.patches.iter().any(|pr| pr.patch as usize == p && matches!(pr.close, Some(Value::Bool(true))));
            if super::squares::at_close(&plan, v, &super::squares::spots(&plan, &ch, &ward, &group, &asked).1) {
                return Err(format!("corner {v}: {AT_CLOSE}"));
            }
        }
        e.squares.push(SquareEdit { v: sr.corner, at: q64(world_pt(plan.mesh.pos[v])), kind: sr.kind.clone() });
    }
    e.squares.sort_by_key(|se| se.v);
    match &req.walls {
        None => {}
        Some(Value::Bool(b)) => e.walls = (*b != plan.walled).then_some(*b),
        Some(Value::String(a)) if a == "auto" => e.walls = None,
        Some(_) => return Err("walls: true, false or \"auto\"".into()),
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
        e.squares.retain(|se| se.kind == "none" || super::squares::junction(&plan, &ward, &group, se.v as usize));
    }
    e.patches.retain(|pe| !pe.is_noop());
    e.patches.sort_by_key(|pe| pe.p);
    let (ch, aside) = changes(&plan, &e, center);
    if let Some(why) = aside.first() {
        return Err(why.clone());
    }
    // What it does.
    let before = super::base_layout(world, t0, index);
    let after = if e.is_noop() { super::generated_layout(world, t0, index) } else { with_edit(world, t0, index, &e) };
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
    let generated = super::generated_layout(world, t0, index);
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
    Ok(((!e.is_noop()).then_some(e), report))
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
        let Some(index) = k.parse::<usize>().ok().filter(|&i| i < t0.settlements.len()) else {
            let parked = k.parse::<usize>().is_ok_and(|i| i >= crate::world::PARKED_LAYOUT);
            out.push((k.clone(), if parked { "set aside when the world was remade or upgraded" } else { "not a town" }.into()));
            continue;
        };
        let s = &t0.settlements[index];
        if s.tier < Tier::Town {
            out.push((k.clone(), "a village has no wards".into()));
        } else if dist(e.at, [s.x, s.y]) > TownEdit::SAME_AT_FT {
            out.push((k.clone(), "the town was laid out anew (it moved)".into()));
        } else if let Some(plan) = super::town_plan(world, t0, index) {
            for why in changes(&plan, e, [s.x, s.y]).1 {
                out.push((k.clone(), why));
            }
        }
    }
    out
}
