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

mod ops;
pub use ops::*;

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
pub(super) const LOT_FACTOR: [f64; 4] = [0.6, 1.2, 2.4, 4.0];

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
    /// (R5 towns) Patches put in another district, and the districts made by hand (by name, in
    /// the order made: `districts`).
    pub district: BTreeMap<usize, super::districts::Target>,
    pub districts: Vec<String>,
    /// (R5 towns, M18b) Gates and towers of the town's wall set by hand (`wallgates`).
    pub wall: super::wallgates::WallChanges,
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

/// The changes a town edit makes to its plan (`center`: the town's, world ft), and the parts of
/// it set aside (why).
pub(super) fn changes(plan: &TownPlan, e: &TownEdit, center: P) -> (Changes, Vec<String>) {
    let mut ch = Changes { walls: e.walls, districts: e.districts.clone(), ..Default::default() };
    let mut aside = Vec::new();
    ch.wall = super::wallgates::resolve(plan, &e.wall, center, &mut aside);
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
        match pe.district.as_deref().map(|d| if !in_a_district(plan, p, ch.ward.get(&p).copied()) { Err(OUTSIDE_DISTRICT.to_string()) } else { super::districts::parse(plan, d) }) {
            Some(Ok(t)) => {
                if let super::districts::Target::Named(n) = &t
                    && !ch.districts.contains(n)
                {
                    ch.districts.push(n.clone());
                }
                ch.district.insert(p, t);
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
    static PLANS: RefCell<(u64, FastMap<u32, (u64, Rc<TownPlan>)>)> = RefCell::new((0, FastMap::default()));
    static TOWNS: RefCell<FastMap<u32, (u64, Rc<Layout>)>> = RefCell::new(FastMap::default());
}

/// A town's plan, made once per world and culture (`make`; `culture`: the one it is laid out
/// in, R5 121) and kept.
pub(super) fn plan(world: &World, t0: &T0, index: usize, culture: u8, make: impl FnOnce() -> TownPlan) -> Rc<TownPlan> {
    let key = index as u32;
    // (183: and the roads its ward edit keeps, `plan::frozen_fp`.)
    // (161: and the planning set for it.)
    let culture = (culture as u64) ^ super::plan::frozen_fp(world, t0, index) ^ (super::planning::mark(world, t0, index) << 8);
    if let Some(p) = PLANS.with(|c| {
        let c = c.borrow();
        if c.0 == world.hash { c.1.get(&key).filter(|p| p.0 == culture).map(|p| p.1.clone()) } else { None }
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
        c.1.insert(key, (culture, p.clone()));
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
    let s = super::settlement(t0, index).filter(|s| s.tier >= Tier::Town)?;
    // (A town laid out by hand has its records instead: its edit is kept only to go back to.)
    if super::hand::is_hand(world, index) {
        return None;
    }
    let e = world.file.edits.towns.get(&key(index))?;
    (!e.is_noop() && dist(e.at, [s.x, s.y]) <= TownEdit::SAME_AT_FT).then_some(e)
}

/// A hash of the world and a town edit.
pub(super) fn hash_edit(world: &World, e: &TownEdit) -> u64 {
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
        // (R5 towns: only when set, so an edit without districts hashes as it did.)
        if let Some(d) = &p.district {
            h.write(&[0xfa]);
            h.write(d.as_bytes());
            h.write(&[0xff]);
        }
    }
    for d in &e.districts {
        h.write(&[0xf9]);
        h.write(d.as_bytes());
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
    // (R5 towns, M18b: only when set.)
    super::wallgates::hash(&mut h, &e.wall);
    // (183: the roads it keeps, when it does.)
    if let Some(roads) = &e.roads {
        h.write(&[0xfa]);
        for x in roads.iter().flatten() {
            h.write(&x.to_le_bytes());
        }
    }
    h.finish()
}

/// `hash_edit` for town `index`, with the culture it is laid out in when changed (R5 121) and the
/// buildings drawn by hand over it (R6 P8, `drawn_over`).
pub(super) fn hash_in(world: &World, t0: &T0, index: usize, e: &TownEdit) -> u64 {
    let h = match super::culture::mark(world, t0, index) {
        0 => hash_edit(world, e),
        c => crate::core::rng::hash2(hash_edit(world, e), c, 0xc017),
    };
    let h = match super::planning::mark(world, t0, index) {
        0 => h,
        p => crate::core::rng::hash2(h, p as i64, 0x91a7),
    };
    let drawn = drawn_over(world, t0, index);
    if drawn.is_empty() {
        return h;
    }
    let mut f = Fnv64::default();
    f.write(&h.to_le_bytes());
    for poly in drawn {
        for p in poly {
            f.write(&p[0].to_le_bytes());
            f.write(&p[1].to_le_bytes());
        }
        f.write(&[0xfe]);
    }
    f.finish()
}

/// (R6 P8) The outlines of the buildings drawn by hand within town `index`'s reach (world ft;
/// those taken away left out), in the order drawn.
fn drawn_over<'a>(world: &'a World, t0: &T0, index: usize) -> Vec<&'a [P]> {
    let Some(s) = super::settlement(t0, index) else { return Vec::new() };
    let (c, r) = ([s.x, s.y], super::reach(s));
    world.file.edits.created.iter().filter(|c| c.kind == "building" && !c.removed && c.poly.len() >= 3).map(|d| d.poly.as_slice()).filter(|poly| poly.iter().any(|p| dist(*p, c) < r)).collect()
}

/// A hash of the world and the town edit that applies to layout `index`, if any.
pub fn fingerprint(world: &World, t0: &T0, index: usize) -> Option<u64> {
    edit_of(world, t0, index).map(|e| hash_in(world, t0, index, e))
}

/// Town `index` laid out with edit `e` (whatever `Edits.towns` holds: a change tried first).
pub fn with_edit(world: &World, t0: &T0, index: usize, e: &TownEdit) -> Rc<Layout> {
    with_edit_fp(world, t0, index, e, hash_in(world, t0, index, e))
}

/// `with_edit`, its hash (`hash_in`) at hand.
pub(super) fn with_edit_fp(world: &World, t0: &T0, index: usize, e: &TownEdit, fp: u64) -> Rc<Layout> {
    let key = index as u32;
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
    // (Roadmap 7, 163: an L's, a U's or a court's, the lot it was shaped on, kept as its ground, `wings`;
    // an inn's that took others in, its own lot, the last kept.)
    let lot = |b: &'_ Building| -> Vec<P> {
        let ground = || super::wings::ranges(generated, b).and_then(|_| generated.grounds.iter().rfind(|g| g.0 == b.id).map(|g| g.1.clone()));
        super::belfry::of(b).map_or_else(|| ground().unwrap_or_else(|| b.poly.clone()), |_| b.parts[0].poly.clone())
    };
    for (k, b) in generated.buildings.iter().enumerate() {
        grid.entry(cell(centroid(&lot(b)))).or_default().push(k as u32);
    }
    let same = |a: &[P], b: &[P]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| dist(*p, *q) < 0.01);
    // (R6 P8) A building drawn by hand pushes out those laid out under it (as a hand town's
    // does; not a castle's, nor one as generated that has edits of its own, which stay to be
    // restored or changed): one generated there is lost, its function moved as any other's.
    let drawn = drawn_over(world, t0, l.index as usize);
    if !drawn.is_empty() {
        let edited: Vec<u32> = super::building_edits(world, l.index).map(|(id, _)| id).collect();
        let bb = |poly: &[P]| bbox(poly);
        let boxes: Vec<[f64; 4]> = drawn.iter().map(|d| bb(d)).collect();
        l.buildings.retain(|b| {
            let o = bb(&b.poly);
            let (cx, cy) = cell(centroid(&b.poly));
            let has_edits = || (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (cx + dx, cy + dy))).filter_map(|c| grid.get(&c)).flatten().any(|&g| edited.contains(&generated.buildings[g as usize].id) && same(&lot(&generated.buildings[g as usize]), &b.poly));
            b.ward == Ward::Castle || !drawn.iter().zip(&boxes).any(|(d, q)| o[0] < q[2] && q[0] < o[2] && o[1] < q[3] && q[1] < o[3] && super::on_lot(d, &b.poly)) || has_edits()
        });
    }
    let mut used = vec![false; generated.buildings.len()];
    let mut next: BTreeMap<u32, u32> = BTreeMap::new();
    let mut fresh: Vec<usize> = Vec::new();
    // (Each building's patch, while its id still is that.)
    let patch: Vec<u32> = l.buildings.iter().map(|b| b.id).collect();
    for (k, b) in l.buildings.iter_mut().enumerate() {
        let (cx, cy) = cell(centroid(&b.poly));
        let near = |(cx, cy): (i64, i64)| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dx| (cx + dx, cy + dy))).filter_map(|c| grid.get(&c)).flatten().map(|&g| g as usize);
        let hit = near((cx, cy)).find(|&g| !used[g] && same(&lot(&generated.buildings[g]), &b.poly));
        // (A lot that stood as generated under a round roof: the ring it was given, `round_off`.)
        let hit = hit.or_else(|| {
            let ring = super::roof::round_footprint(&b.poly)?;
            // (Its door turns the generated ring, `round_off`: the same corners from another one.)
            near(cell(centroid(&ring))).find(|&g| {
                let o = lot(&generated.buildings[g]);
                !used[g] && o.len() == ring.len() && (0..ring.len()).any(|r| (0..ring.len()).all(|k| dist(o[k], ring[(k + r) % ring.len()]) < 0.01))
            })
        });
        // (R5 towns, 115: a lot that stood as generated under a yurt, sized and set by its id and
        // household: `yurts::ring`, its lot kept in `Layout::yurts`.)
        let hit = hit.or_else(|| {
            let y = generated.yurts.iter().find(|y| y.lot.len() == b.poly.len() && same(&y.lot, &b.poly))?;
            generated.buildings.iter().position(|o| o.id == y.id).filter(|&g| !used[g])
        });
        // (R5 towns: a lot that stood as generated under a felt hall, its ring about the lot's widest
        // middle, `roof::hall_ring`.)
        let hit = hit.or_else(|| {
            let c = super::roof::hall_centre(&b.poly);
            near(cell(c)).find(|&g| {
                let o = &generated.buildings[g].poly;
                let n = o.len() as f64;
                !used[g] && o.len() == super::roof::HALL_SIDES && dist(o.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]), c) < 0.01
            })
        });
        match hit {
            Some(g) => {
                used[g] = true;
                let o = &generated.buildings[g];
                (b.id, b.func, b.residential, b.floors, b.structure, b.roof, b.tint, b.material, b.shape, b.shade) = (o.id, o.func, o.residential, o.floors, o.structure, o.roof, o.tint, o.material, o.shape, o.shade);
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
            (b.id, b.func, b.residential, b.floors, b.structure, b.roof, b.tint, b.material, b.shape, b.shade) = (o.id, o.func, o.residential, o.floors, o.structure, o.roof, o.tint, o.material, o.shape, o.shade);
            b.name = o.name.clone();
            fresh.retain(|&j| j != k);
        }
    }
    // A new building lying mostly on one generated that none took, of the same structure (half
    // of each on the other: a lot trimmed back by a square or a close) is that building, cut down;
    // not in a patch whose lots were rolled, sized anew or given another ward (a roll never takes
    // another's ids), and a castle's only a castle's (a range never takes a house's id: how many
    // ranges it has then moves no other building's).
    let reach = 1.5 * spacing;
    let relotted: Vec<u32> = edit.patches.iter().filter(|p| p.reroll > 0 || p.lots.is_some() || p.ward.is_some()).map(|p| p.p).collect();
    for k in fresh.clone() {
        let b = &l.buildings[k];
        if b.func.is_some() || relotted.contains(&patch[k]) {
            continue;
        }
        let (at, whole) = (centroid(&b.poly), area(&b.poly).abs());
        let hit = (0..generated.buildings.len())
            .filter(|&g| !used[g] && generated.buildings[g].structure == b.structure && (generated.buildings[g].ward == Ward::Castle) == (b.ward == Ward::Castle) && dist(centroid(&generated.buildings[g].poly), at) < 80.0)
            .map(|g| (super::overlap(&b.poly, &generated.buildings[g].poly), g))
            .filter(|&(a, g)| a >= 0.5 * whole && a >= 0.5 * area(&generated.buildings[g].poly).abs())
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
        if let Some((_, g)) = hit {
            used[g] = true;
            let (o, b) = (&generated.buildings[g], &mut l.buildings[k]);
            (b.id, b.func, b.residential, b.floors, b.structure, b.roof, b.tint, b.material, b.shape, b.shade) = (o.id, o.func, o.residential, o.floors, o.structure, o.roof, o.tint, o.material, o.shape, o.shade);
            b.name = o.name.clone();
            fresh.retain(|&j| j != k);
        }
    }
    // A building laid out for a purpose (a close's temple) takes the place, id and name of one the
    // town lost with that purpose (the nearest, about a patch away at most; notes and people there
    // follow it); else of one kept as generated on its patch, which is a home again with the new
    // one's id. (Not its windmills: out in the fields, kept as generated after, `super::layout`.)
    let mut lost: Vec<(usize, P, u32, Option<String>, bool)> = generated
        .buildings
        .iter()
        .zip(&used)
        .filter(|(b, u)| !**u && !super::rural::windmill(b) && !super::cemetery::is_tomb(b))
        .filter_map(|(b, _)| b.func.map(|f| (f as usize, centroid(&b.poly), b.id, b.name.clone(), b.ward == Ward::Castle)))
        .collect();
    // (Nearest pairs first, so each takes its own patch's before a neighbour's.)
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for &k in &fresh {
        let Some(f) = l.buildings[k].func else { continue };
        let at = centroid(&l.buildings[k].poly);
        let castle = l.buildings[k].ward == Ward::Castle;
        pairs.extend((0..lost.len()).filter(|&i| lost[i].0 == f as usize && lost[i].4 == castle && dist(lost[i].1, at) <= reach).map(|i| (dist(lost[i].1, at), k, i)));
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
        if let Some(j) = (0..l.buildings.len()).find(|&j| patch[j] == patch[k] && l.buildings[j].func == Some(f) && l.buildings[j].structure == Structure::Roofed && !fresh.contains(&j)) {
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
                    b.name = super::sites::trade_name(Some(fi), &mut rng, t0, center, names.iter().map(String::as_str));
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
            // (An emptied district has no label: nothing to draw.)
            if pts.is_empty() {
                return;
            }
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
