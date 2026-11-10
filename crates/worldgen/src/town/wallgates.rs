//! (R5 towns, M18b) Gates and towers set by hand on a town's own wall (`TownEdit::wall`, asked
//! through `edit_town`'s `walls`), by the plan's wall corners.
//!
//! A gate put in at a corner takes the town's gatehouse form (`gatehouse`: the gate towers are
//! laid like any other gate's), its `gt:` id by the corner and a name like any other (`places`,
//! after the generated gates so theirs stay), and a way out: a track from the gate along the
//! outskirts' patch edges to the nearest road (an approach or a road of the network) or out to the
//! open country past them. It is refused, with the reason, where there is none (water all round,
//! the wall's end, a castle's curtain, another gate under `GATE_APART_FT`, sides too short for a
//! gatehouse). A gate taken out closes the wall there: its road stops outside, its main street
//! inside. A tower goes up `f` of the way along a wall side (not within `TOWER_GAP_FT` of another
//! tower or of a gate, nor over a building or a street crossing the wall) and comes down by corner
//! or `t:` id; a corner's tower taken down goes back up by its corner.
//!
//! Ids: a town's towers keep the `t:` numbers they have without the wall edited (matched by place,
//! `Layout::tower_ids`); a tower put up by hand is `HAND_TOWER_ID + k`, a hand gate's two towers
//! `GATE_TOWER_ID + 2 × corner + side`, the tower on a corner whose gate was taken out
//! `CORNER_TOWER_ID + corner`.

use std::cell::RefCell;
use std::collections::BinaryHeap;
use std::rc::Rc;

use serde::Deserialize;
use serde_json::{Value, json};

use super::catalog::Ward;
use super::geom::*;
use super::mesh::Mesh;
use super::{Layout, Site, TownPlan};
use crate::World;
use crate::core::hash::FastMap;
use crate::t0::T0;
use crate::world::{HandTower, TownEdit, WallCorner, WallEdit};

pub const HAND_TOWER_ID: u32 = 1_000_000;
pub const GATE_TOWER_ID: u32 = 2_000_000;
pub const CORNER_TOWER_ID: u32 = 3_000_000;
/// A tower none of the others matched (should not happen: towncheck counts them).
pub const OTHER_TOWER_ID: u32 = 4_000_000;

/// Gate towers stand this far either side of the gate's corner (`wall_pieces_spaced`).
const GATE_HALF: f64 = 26.0;
const GATE_APART_FT: f64 = 80.0;
/// A wall side at a gate this long at least: its gate tower (r 15, 26 ft out) clear of the
/// corner tower at the far end (r 11).
const GATE_SIDE_FT: f64 = 56.0;
/// An along tower this near a gate's corner gives way to its gate tower.
const GATE_CLEAR_FT: f64 = 56.0;
pub const TOWER_GAP_FT: f64 = 20.0;
const WAY_MAX_FT: f64 = 2400.0;
pub const WAY_WIDTH_FT: f64 = 12.0;
/// A road whose gate was taken out stops this short of the wall; a main street this far inside.
const ROAD_BACK_FT: f64 = 30.0;
const STREET_BACK_FT: f64 = 14.0;
/// A tower this near where one stood without the wall edit keeps its number (towers stand 22 ft
/// apart and more).
const MATCH_FT: f64 = 60.0;

/// A wall edit as the builder takes it (the plan's corners).
#[derive(Clone, Debug, Default)]
pub(super) struct WallChanges {
    pub add: Vec<usize>,
    pub remove: Vec<usize>,
    /// (id, v, w, f)
    pub towers: Vec<(u32, usize, usize, f64)>,
    pub off: Vec<u32>,
}

impl WallChanges {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty() && self.towers.is_empty() && self.off.is_empty()
    }
}

/// What a hand-edited wall came to, as laid (local ft until `finish`, then world).
#[derive(Clone, Debug, Default)]
pub struct HandWall {
    /// Gates put in by hand and built: corner, where.
    pub gates: Vec<(u32, P)>,
    /// Each one's way out: corner, its length (ft).
    pub ways: Vec<(u32, f64)>,
    /// Gates refused: corner, why.
    pub refused: Vec<(u32, String)>,
    /// Towers put up by hand where they were set on the wall's line, and their ids.
    pub tags: Vec<(P, u32)>,
    /// Corners whose gate was taken out.
    pub closed: Vec<(u32, P)>,
}

/// Which wall run and place in it corner `v` is.
fn on_wall(plan: &TownPlan, v: usize) -> Option<(usize, usize)> {
    plan.wall_runs
        .iter()
        .enumerate()
        .find_map(|(r, (chain, _, _))| chain.iter().position(|&u| u == v).map(|k| (r, k)))
}

/// Corners `v` and `w` next to each other on the wall.
fn neighbours(plan: &TownPlan, v: usize, w: usize) -> bool {
    plan.wall_runs.iter().any(|(chain, closed, _)| {
        let n = chain.len();
        (0..if *closed { n } else { n.saturating_sub(1) }).any(|k| {
            let (a, b) = (chain[k], chain[(k + 1) % n]);
            (a == v && b == w) || (a == w && b == v)
        })
    })
}

/// The wall edit's parts that still apply to the plan (corners planned where they were).
pub(super) fn resolve(plan: &TownPlan, e: &WallEdit, center: P, aside: &mut Vec<String>) -> WallChanges {
    let mut out = WallChanges::default();
    let here = |v: u32, at: [f64; 2]| {
        (v as usize) < plan.mesh.pos.len() && on_wall(plan, v as usize).is_some() && dist(plan.mesh.pos[v as usize], sub(at, center)) <= TownEdit::SAME_AT_FT
    };
    for c in &e.gates_add {
        if here(c.v, c.at) && !plan.gates.contains(&(c.v as usize)) {
            out.add.push(c.v as usize);
        } else {
            aside.push(format!("wall gate at corner {}: the corner is not on the wall as planned", c.v));
        }
    }
    for c in &e.gates_remove {
        if here(c.v, c.at) && plan.gates.contains(&(c.v as usize)) {
            out.remove.push(c.v as usize);
        } else {
            aside.push(format!("gate taken out at corner {}: no gate there as planned", c.v));
        }
    }
    for t in &e.towers_add {
        if here(t.v, t.at) && neighbours(plan, t.v as usize, t.w as usize) && t.f > 0.0 && t.f < 1.0 {
            out.towers.push((t.id, t.v as usize, t.w as usize, t.f));
        } else {
            aside.push(format!("tower t:…:{}: its wall side is not as planned", t.id));
        }
    }
    out.off = e.towers_remove.clone();
    out
}

/// Drops the parts of a wall edit that no longer apply (before a change is made to it).
pub(super) fn keep_live(plan: &TownPlan, center: P, e: &mut WallEdit) {
    let here = |v: u32, at: [f64; 2]| {
        (v as usize) < plan.mesh.pos.len() && on_wall(plan, v as usize).is_some() && dist(plan.mesh.pos[v as usize], sub(at, center)) <= TownEdit::SAME_AT_FT
    };
    e.gates_add.retain(|c| here(c.v, c.at) && !plan.gates.contains(&(c.v as usize)));
    e.gates_remove.retain(|c| here(c.v, c.at) && plan.gates.contains(&(c.v as usize)));
    e.towers_add
        .retain(|t| here(t.v, t.at) && neighbours(plan, t.v as usize, t.w as usize) && t.f > 0.0 && t.f < 1.0);
}

/// A hash of the wall edit, for `wards::hash_edit` (only when there is one).
pub(super) fn hash(h: &mut crate::core::hash::Fnv64, e: &WallEdit) {
    if e.is_empty() {
        return;
    }
    h.write(&[0xf8]);
    for c in e.gates_add.iter().chain(&[WallCorner { v: u32::MAX, at: [0.0; 2] }]).chain(&e.gates_remove) {
        h.write(&c.v.to_le_bytes());
        for x in &c.at {
            h.write(&x.to_le_bytes());
        }
    }
    for t in &e.towers_add {
        for x in [t.id, t.v, t.w] {
            h.write(&x.to_le_bytes());
        }
        for x in [t.f, t.at[0], t.at[1]] {
            h.write(&x.to_le_bytes());
        }
    }
    h.write(&[0xf7]);
    for id in &e.towers_remove {
        h.write(&id.to_le_bytes());
    }
}

/// Why a gate can't go in at wall corner `v` (`gates`: those standing so far), if it can't.
fn gate_why(plan: &TownPlan, mesh: &Mesh, ward: &[Option<Ward>], gates: &[usize], v: usize) -> Option<String> {
    let Some((r, k)) = on_wall(plan, v) else {
        return Some("that corner is not on the wall".into());
    };
    let (chain, closed, _) = &plan.wall_runs[r];
    let n = chain.len();
    if !closed && (k == 0 || k + 1 == n) {
        return Some("the wall ends at that corner: a gate needs wall on both sides".into());
    }
    if plan.pinned[v] {
        return Some("that corner is on the water or a river".into());
    }
    if plan.vf[v].iter().any(|&f| ward.get(f).copied().flatten() == Some(Ward::Castle)) {
        return Some("the castle's curtain is the wall there".into());
    }
    let (prev, next) = (chain[(k + n - 1) % n], chain[(k + 1) % n]);
    let p = mesh.pos[v];
    if dist(p, mesh.pos[prev]).min(dist(p, mesh.pos[next])) < GATE_SIDE_FT {
        return Some(format!("the wall's sides at that corner are too short for a gatehouse (under {GATE_SIDE_FT} ft)"));
    }
    if let Some(&g) = gates.iter().find(|&&g| g != v && dist(mesh.pos[g], p) < GATE_APART_FT) {
        return Some(format!("the gate at corner {g} is under {GATE_APART_FT} ft away"));
    }
    None
}

/// Total order on ft for the way's search (lengths are never negative).
fn bits(d: f64) -> u64 {
    d.max(0.0).to_bits()
}

/// The way out through a gate at wall corner `v`: along the outskirts' patch edges (dry, off the
/// river, not along the wall) to the nearest corner by a road (an approach or the network's) or
/// on the plan's edge (the open country), joined onto the approach it reaches. World-free: local ft.
fn way_out(site: &Site, plan: &TownPlan, mesh: &Mesh, v: usize) -> Result<Vec<P>, String> {
    let nv = mesh.pos.len();
    let mut on_chain = vec![false; nv];
    for (chain, _, _) in &plan.wall_runs {
        for &u in chain {
            on_chain[u] = true;
        }
    }
    let approaches: Vec<Vec<P>> = plan
        .ends
        .iter()
        .enumerate()
        .filter_map(|(k, (e, _))| {
            plan.gate_of
                .get(k)
                .copied()
                .flatten()
                .filter(|g| *g != v)
                .map(|g| super::approach_curve(*e, mesh.pos[g], plan.heading_of[k]))
        })
        .collect();
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nv];
    let mut edge_v = vec![false; nv];
    for (f, face) in mesh.faces.iter().enumerate() {
        if f < plan.n_inner || plan.wet[f] {
            continue;
        }
        let m = face.len();
        for e in 0..m {
            // (A neighbour emptied when the plan was cut is none: the plan's edge.)
            let nb = mesh.labels[f][e];
            let nb = if nb >= 0 && mesh.faces.get(nb as usize).is_some_and(|g| g.len() >= 3) { nb } else { -1 };
            let (a, b) = (face[e], face[(e + 1) % m]);
            if nb < 0 {
                edge_v[a] = true;
                edge_v[b] = true;
            }
            if nb >= 0 && ((nb as usize) < plan.n_inner || plan.wet[nb as usize] || (nb as usize) < f) {
                continue;
            }
            if (on_chain[a] && a != v) || (on_chain[b] && b != v) {
                continue;
            }
            let (pa, pb) = (mesh.pos[a], mesh.pos[b]);
            let d = dist(pa, pb);
            let steps = crate::core::ceil(d / 8.0).max(1.0) as usize;
            if (0..=steps).any(|j| {
                let p = lerp(pa, pb, j as f64 / steps as f64);
                site.wet(p) || site.near_river(p, 4.0)
            }) {
                continue;
            }
            adj[a].push((b, d));
            adj[b].push((a, d));
        }
    }
    // (From the gate, straight across each patch outside it to its other corners: a corner of the
    // wall whose third street runs inward has none going out. The track keeps the outskirts' lots
    // and fields off its line, as a road does.)
    for (f, face) in mesh.faces.iter().enumerate() {
        if f < plan.n_inner || plan.wet[f] || !face.contains(&v) {
            continue;
        }
        for &x in face {
            if x == v || on_chain[x] || adj[v].iter().any(|e| e.0 == x) {
                continue;
            }
            let (pa, pb) = (mesh.pos[v], mesh.pos[x]);
            let d = dist(pa, pb);
            let steps = crate::core::ceil(d / 8.0).max(1.0) as usize;
            if (0..=steps).any(|j| {
                let p = lerp(pa, pb, j as f64 / steps as f64);
                site.wet(p) || site.near_river(p, 4.0)
            }) {
                continue;
            }
            adj[v].push((x, d));
        }
    }
    let join = |p: P| -> Option<P> {
        approaches
            .iter()
            .flat_map(|c| c.windows(2))
            .map(|s| {
                let d = sub(s[1], s[0]);
                let t = (dot(sub(p, s[0]), d) / dot(d, d).max(1e-9)).clamp(0.0, 1.0);
                add(s[0], mul(d, t))
            })
            .filter(|q| dist(*q, p) < 40.0)
            .min_by(|a, b| dist(*a, p).total_cmp(&dist(*b, p)))
    };
    let mut best = vec![f64::MAX; nv];
    let mut from = vec![usize::MAX; nv];
    let mut heap = BinaryHeap::new();
    best[v] = 0.0;
    heap.push(std::cmp::Reverse((bits(0.0), v)));
    while let Some(std::cmp::Reverse((d, u))) = heap.pop() {
        let d = f64::from_bits(d);
        if d > best[u] {
            continue;
        }
        if d > WAY_MAX_FT {
            break;
        }
        if u != v {
            let p = mesh.pos[u];
            let joined = join(p);
            if joined.is_some() || site.near_network_road(p, 20.0) || edge_v[u] {
                let mut path = vec![u];
                while let Some(&last) = path.last() {
                    if last == v {
                        break;
                    }
                    path.push(from[last]);
                }
                path.reverse();
                let mut pts: Vec<P> = path.iter().map(|&k| mesh.pos[k]).collect();
                if let Some(q) = joined.filter(|q| dist(*q, p) > 1.0) {
                    pts.push(q);
                }
                return Ok(pts);
            }
        }
        for &(w, l) in &adj[u] {
            let nd = d + l;
            if nd < best[w] {
                best[w] = nd;
                from[w] = u;
                heap.push(std::cmp::Reverse((bits(nd), w)));
            }
        }
    }
    Err("no way out of the wall there: water, the wall or nothing but patch edges further than 2,400 ft lie beyond that corner".into())
}

/// The gates the town's wall is laid with (the plan's, less those taken out, and those put in
/// that have a way out), and each hand gate's way out (corner, line: local ft); what came of the
/// hand edit goes in `hw`.
pub(super) fn settle(site: &Site, plan: &TownPlan, mesh: &Mesh, ward: &[Option<Ward>], wc: &WallChanges, hw: &mut HandWall) -> (Vec<usize>, Vec<(usize, Vec<P>)>) {
    if wc.is_empty() {
        return (plan.gates.clone(), Vec::new());
    }
    let mut gates: Vec<usize> = plan.gates.iter().copied().filter(|g| !wc.remove.contains(g)).collect();
    let mut ways = Vec::new();
    for &v in &wc.add {
        if gates.contains(&v) {
            continue;
        }
        let why = gate_why(plan, mesh, ward, &gates, v).map_or_else(|| way_out(site, plan, mesh, v), Err);
        match why {
            Ok(pts) => {
                gates.push(v);
                hw.gates.push((v as u32, mesh.pos[v]));
                hw.ways.push((v as u32, pts.windows(2).map(|s| dist(s[0], s[1])).sum()));
                ways.push((v, pts));
            }
            Err(why) => hw.refused.push((v as u32, why)),
        }
    }
    hw.closed = wc.remove.iter().map(|&v| (v as u32, mesh.pos[v])).collect();
    hw.tags = wc.towers.iter().map(|&(id, v, w, f)| (lerp(mesh.pos[v], mesh.pos[w], f), id)).collect();
    (gates, ways)
}

/// A line cut back by `by` ft at its end.
pub(super) fn trim_end(pts: &[P], by: f64) -> Vec<P> {
    let mut out = pts.to_vec();
    let mut left = by;
    while out.len() >= 2 {
        let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
        let d = dist(a, b);
        if d > left + 1.0 {
            let n = out.len();
            out[n - 1] = lerp(b, a, left / d);
            break;
        }
        left -= d;
        out.pop();
    }
    out
}

/// A road to a gate taken out stops short of the wall; a main street ending (or starting) at one
/// stops inside it.
pub(super) fn closed_road(wc: &WallChanges, g: usize, curve: Vec<P>) -> Vec<P> {
    if wc.remove.contains(&g) { trim_end(&curve, ROAD_BACK_FT) } else { curve }
}

pub(super) fn closed_street(wc: &WallChanges, path: &[usize], drawn: Vec<P>) -> Vec<P> {
    let mut d = drawn;
    if path.last().is_some_and(|v| wc.remove.contains(v)) {
        d = trim_end(&d, STREET_BACK_FT);
    }
    if path.first().is_some_and(|v| wc.remove.contains(v)) {
        d.reverse();
        d = trim_end(&d, STREET_BACK_FT);
        d.reverse();
    }
    d
}

/// The town wall's pieces, towers and gate towers along its runs (local ft) with `gates`: as
/// generated (`wall_pieces_spaced`) when nothing is set by hand; else the same towers where they
/// stand (each side's along the wall as planned, but those a new gate's towers take the place of),
/// the hand towers on their sides, and gate towers at each gate.
pub(super) fn pieces(plan: &TownPlan, mesh: &Mesh, gates: &[usize], wc: &WallChanges) -> (Vec<Vec<P>>, Vec<P>, Vec<P>) {
    let (mut pieces, mut towers, mut gate_towers) = (Vec::new(), Vec::new(), Vec::new());
    if wc.is_empty() && gates == plan.gates.as_slice() {
        for (chain, closed, along) in &plan.wall_runs {
            let corners: Vec<(P, bool)> = chain.iter().map(|&v| (mesh.pos[v], plan.gates.contains(&v))).collect();
            let (p, t, g, _) = super::wall_pieces_spaced(&corners, *closed, Some(along), super::SET);
            pieces.extend(p);
            towers.extend(t);
            gate_towers.extend(g);
        }
        return (pieces, towers, gate_towers);
    }
    let toward = |a: P, b: P, by: f64| add(a, mul(sub(b, a), by / dist(a, b).max(1e-9)));
    for (chain, closed, along) in &plan.wall_runs {
        let n = chain.len();
        if n == 0 {
            continue;
        }
        let m = if *closed { n + 1 } else { n };
        let pos = |kk: usize| mesh.pos[chain[kk % n]];
        let pg = |kk: usize| plan.gates.contains(&chain[kk % n]);
        let ng = |kk: usize| gates.contains(&chain[kk % n]);
        // (The towers along each side as planned: by the side ending at `kk`, as
        // `wall_pieces_spaced` counts them; `usize::MAX` where it fell back to the spacing.)
        let mut side_n: Vec<Option<usize>> = vec![None; m];
        {
            let (mut used, mut open) = (0usize, false);
            for kk in 0..m {
                if pg(kk) {
                    open = kk + 1 < m;
                    continue;
                }
                if open {
                    side_n[kk] = Some(along.get(used).copied().unwrap_or(usize::MAX));
                    used += 1;
                }
                open = true;
            }
        }
        let mut cur: Vec<P> = Vec::new();
        for kk in 0..m {
            let p = pos(kk);
            if kk > 0 {
                let q = pos(kk - 1);
                let start_gen = if pg(kk - 1) { toward(q, p, GATE_HALF) } else { q };
                let start_now = if ng(kk - 1) { toward(q, p, GATE_HALF) } else { q };
                let mut on_side: Vec<(f64, P)> = Vec::new();
                let (start, cnt) = match side_n[kk] {
                    Some(usize::MAX) => (start_gen, crate::core::floor(dist(start_gen, p) / super::SET) as usize),
                    Some(c) => (start_gen, c),
                    None => (start_now, 0),
                };
                for j in 1..=cnt {
                    let t = lerp(start, p, j as f64 / (cnt + 1) as f64);
                    let near_gate = (ng(kk) && dist(t, p) < GATE_CLEAR_FT) || (ng(kk - 1) && !pg(kk - 1) && dist(t, q) < GATE_CLEAR_FT);
                    if !near_gate {
                        on_side.push((dist(q, t), t));
                    }
                }
                let (a, b) = (chain[(kk - 1) % n], chain[kk % n]);
                for &(_, v, w, f) in &wc.towers {
                    let f = if v == a && w == b {
                        f
                    } else if v == b && w == a {
                        1.0 - f
                    } else {
                        continue;
                    };
                    let t = lerp(q, p, f);
                    on_side.push((dist(q, t), t));
                }
                on_side.sort_by(|x, y| x.0.total_cmp(&y.0));
                towers.extend(on_side.into_iter().map(|x| x.1));
            }
            if ng(kk) {
                if kk > 0 {
                    let e = toward(p, pos(kk - 1), GATE_HALF);
                    cur.push(e);
                    gate_towers.push(e);
                }
                if cur.len() >= 2 {
                    pieces.push(std::mem::take(&mut cur));
                }
                cur.clear();
                if kk + 1 < m {
                    let e = toward(p, pos(kk + 1), GATE_HALF);
                    cur.push(e);
                    gate_towers.push(e);
                }
                continue;
            }
            cur.push(p);
            if !(*closed && kk + 1 == m) {
                towers.push(p);
            }
        }
        if cur.len() >= 2 {
            pieces.push(cur);
        }
    }
    (pieces, towers, gate_towers)
}

thread_local! {
    /// Each town's towers laid out without its wall edit (the ids they keep): by town, with the
    /// hash of the edit they were laid out with.
    static BASE: RefCell<FastMap<u32, (u64, Rc<Vec<(P, bool)>>)>> = RefCell::new(FastMap::default());
}

/// The towers of town `index` as edit `e` lays it out without its wall part (world ft; their
/// places in the list are their `t:` numbers), and whether each flanks a gate.
fn base_towers(world: &World, t0: &T0, index: usize, e: &TownEdit) -> Rc<Vec<(P, bool)>> {
    let be = TownEdit {
        wall: WallEdit::default(),
        ..e.clone()
    };
    let fp = super::wards::hash_edit(world, &be);
    if let Some(t) = BASE.with(|c| c.borrow().get(&(index as u32)).filter(|x| x.0 == fp).map(|x| x.1.clone())) {
        return t;
    }
    let l = if be.is_noop() {
        super::generated_layout(world, t0, index)
    } else {
        super::wards::with_edit(world, t0, index, &be)
    };
    let t: Rc<Vec<(P, bool)>> = Rc::new(crate::interior::towers(&l).into_iter().map(|(p, _, g)| (p, g)).collect());
    BASE.with(|c| c.borrow_mut().insert(index as u32, (fp, t.clone())));
    t
}

/// Last of a town laid out with a wall edit: each tower's `t:` number (`Layout::tower_ids`), and
/// the towers taken down gone.
pub(super) fn finish(world: &World, t0: &T0, index: usize, e: &TownEdit, l: &mut Layout) {
    if e.wall.is_empty() {
        return;
    }
    let c = l.center;
    for p in l
        .hand_wall
        .gates
        .iter_mut()
        .map(|g| &mut g.1)
        .chain(l.hand_wall.tags.iter_mut().map(|t| &mut t.0))
        .chain(l.hand_wall.closed.iter_mut().map(|g| &mut g.1))
    {
        *p = add(*p, c);
    }
    let base = base_towers(world, t0, index, e);
    let all = crate::interior::towers(l);
    // (First a hand gate's towers and the towers put up by hand, by where they were set; then,
    // nearest first, a tower within `MATCH_FT` of where one stood without the edit is that one: a
    // curtain bowed anew where a gate went in or out carries its towers along.)
    let mut ids: Vec<Option<u32>> = vec![None; all.len()];
    let mut tag_used = vec![false; l.hand_wall.tags.len()];
    let mut sides: FastMap<u32, u32> = FastMap::default();
    for (k, &(p, _, gate)) in all.iter().enumerate() {
        if gate {
            if let Some(&(v, _)) = l
                .hand_wall
                .gates
                .iter()
                .filter(|g| dist(g.1, p) < GATE_HALF + 6.0)
                .min_by(|a, b| dist(a.1, p).total_cmp(&dist(b.1, p)))
            {
                let s = sides.entry(v).or_default();
                *s += 1;
                ids[k] = Some(GATE_TOWER_ID + 2 * v + (*s - 1).min(1));
            }
        } else if let Some(t) = (0..l.hand_wall.tags.len())
            .filter(|&t| !tag_used[t] && dist(l.hand_wall.tags[t].0, p) < 15.0)
            .min_by(|&a, &b| dist(l.hand_wall.tags[a].0, p).total_cmp(&dist(l.hand_wall.tags[b].0, p)))
        {
            tag_used[t] = true;
            ids[k] = Some(l.hand_wall.tags[t].1);
        }
    }
    let mut pairs: Vec<(u64, usize, usize)> = Vec::new();
    for (k, &(p, _, gate)) in all.iter().enumerate() {
        if ids[k].is_some() {
            continue;
        }
        for (j, &(q, g)) in base.iter().enumerate() {
            let d = dist(p, q);
            if g == gate && d < MATCH_FT {
                pairs.push((bits(d), k, j));
            }
        }
    }
    pairs.sort_unstable();
    let mut used = vec![false; base.len()];
    for (_, k, j) in pairs {
        if ids[k].is_none() && !used[j] {
            ids[k] = Some(j as u32);
            used[j] = true;
        }
    }
    let ids: Vec<u32> = (0..all.len())
        .map(|k| {
            if let Some(id) = ids[k] {
                return id;
            }
            let (p, _, gate) = all[k];
            if !gate && let Some(&(v, _)) = l.hand_wall.closed.iter().find(|g| dist(g.1, p) < 20.0) {
                return CORNER_TOWER_ID + v;
            }
            OTHER_TOWER_ID + k as u32
        })
        .collect();
    // Towers taken down (never a gate's).
    let off = &e.wall.towers_remove;
    let keep: Vec<bool> = (0..all.len()).map(|k| all[k].2 || !off.contains(&ids[k])).collect();
    let gone: Vec<P> = (0..all.len()).filter(|&k| !keep[k]).map(|k| all[k].0).collect();
    let (nt, ng) = (l.towers.len(), l.gate_towers.len());
    let mut k = 0;
    l.towers.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    let mut k = nt + ng;
    l.extra_towers.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    let away = |p: &P| !gone.iter().any(|g| dist(*g, *p) < 0.5);
    l.square_towers.retain(|t| away(&t.0));
    l.tower_forms.retain(|t| away(&t.0));
    l.tower_ids = (0..all.len()).filter(|&k| keep[k]).map(|k| ids[k]).collect();
}

impl Layout {
    /// The `t:` number of the `k`th of `interior::towers` (its place in the list, unless the
    /// town's wall was edited by hand: `tower_ids`).
    pub fn tower_id(&self, k: usize) -> usize {
        match self.tower_ids.get(k) {
            Some(&i) => i as usize,
            None if self.tower_ids.is_empty() => k,
            None => OTHER_TOWER_ID as usize + 500_000 + k,
        }
    }

    /// Which of `interior::towers` has `t:` number `id`.
    pub fn tower_index(&self, id: usize) -> Option<usize> {
        let n = crate::interior::towers(self).len();
        if self.tower_ids.is_empty() {
            return (id < n).then_some(id);
        }
        (0..n).find(|&k| self.tower_id(k) == id)
    }
}

// ---------------------------------------------------------------------------------------
// edit_town's `walls: {…}`.

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WallRequest {
    #[serde(default)]
    built: Option<Value>,
    #[serde(default)]
    gates_add: Vec<u32>,
    #[serde(default)]
    gates_remove: Vec<u32>,
    #[serde(default)]
    towers_add: Vec<Value>,
    #[serde(default)]
    towers_remove: Vec<Value>,
}

/// What was asked of the wall, for the reply.
#[derive(Default)]
pub(super) struct Asked {
    gates_add: Vec<u32>,
    gates_remove: Vec<u32>,
    towers_add: Vec<u32>,
    towers_back: Vec<u32>,
    towers_remove: Vec<u32>,
    refused: Vec<Value>,
}

/// `walls: {built, gates_add, gates_remove, towers_add, towers_remove}` made part of edit `e`
/// (`built`: true, false or "auto", as `walls` alone). A corner that isn't one or isn't on the
/// wall is an error; a gate or tower the rules don't allow is refused (said in the reply).
pub(super) fn request(world: &World, t0: &T0, index: usize, plan: &TownPlan, center: P, e: &mut TownEdit, v: &Value) -> Result<Asked, String> {
    let r: WallRequest = serde_json::from_value(v.clone()).map_err(|x| format!("walls: {x}"))?;
    match &r.built {
        None => {}
        Some(Value::Bool(b)) => e.walls = (*b != plan.walled).then_some(*b),
        Some(Value::String(a)) if a == "auto" => e.walls = None,
        Some(_) => return Err("walls.built: true, false or \"auto\"".into()),
    }
    let (ch, _) = super::wards::changes(plan, e, center);
    let mesh = super::wards::moved_mesh(plan, &ch.moves);
    let world_pt = |p: P| add(p, center);
    let corner = |v: u32| -> Result<usize, String> {
        let k = v as usize;
        if k >= plan.mesh.pos.len() || plan.vf[k].is_empty() {
            return Err(format!("corner {v}: no such corner"));
        }
        if on_wall(plan, k).is_none() {
            return Err(format!("corner {v} is not on the town's wall (get_town_plan marks the wall's corners wall: true)"));
        }
        Ok(k)
    };
    let planned = |k: usize| {
        [
            crate::core::round(world_pt(plan.mesh.pos[k])[0] * 64.0) / 64.0,
            crate::core::round(world_pt(plan.mesh.pos[k])[1] * 64.0) / 64.0,
        ]
    };
    let mut a = Asked::default();
    // (The towers as they stand without the wall edit: a corner's taken down goes back up.)
    let base = base_towers(world, t0, index, e);
    let w = &mut e.wall;
    for &g in &r.gates_remove {
        let k = corner(g)?;
        if let Some(i) = w.gates_add.iter().position(|c| c.v == g) {
            w.gates_add.remove(i);
            a.gates_remove.push(g);
        } else if plan.gates.contains(&k) && !w.gates_remove.iter().any(|c| c.v == g) {
            w.gates_remove.push(WallCorner { v: g, at: planned(k) });
            a.gates_remove.push(g);
        } else {
            a.refused.push(json!({ "gate_remove": g, "why": "no gate at that corner" }));
        }
    }
    for &g in &r.gates_add {
        let k = corner(g)?;
        if let Some(i) = w.gates_remove.iter().position(|c| c.v == g) {
            w.gates_remove.remove(i);
            a.gates_add.push(g);
        } else if plan.gates.contains(&k) || w.gates_add.iter().any(|c| c.v == g) {
            a.refused.push(json!({ "gate_add": g, "why": format!("there is a gate there already (gt:{index}:{g})") }));
        } else {
            w.gates_add.push(WallCorner { v: g, at: planned(k) });
            a.gates_add.push(g);
        }
    }
    // Towers: against the town as it stands.
    let before = super::base_layout(world, t0, index);
    let standing = crate::interior::towers(&before);
    let walled = !before.gate_spots.is_empty() || !standing.is_empty();
    let corner_tower = |k: usize| {
        let at = world_pt(mesh.pos[k]);
        (0..standing.len())
            .filter(|&i| !standing[i].2 && dist(standing[i].0, at) < 30.0)
            .min_by(|&x, &y| dist(standing[x].0, at).total_cmp(&dist(standing[y].0, at)))
    };
    for t in &r.towers_remove {
        let id = match t {
            Value::Number(n) => {
                let k = corner(n.as_u64().unwrap_or(u64::MAX).min(u32::MAX as u64) as u32)?;
                match corner_tower(k) {
                    Some(i) => before.tower_id(i) as u32,
                    None => {
                        a.refused.push(json!({ "tower_remove": t, "why": "no tower stands at that corner" }));
                        continue;
                    }
                }
            }
            Value::String(s) => {
                let parts: Vec<&str> = s.split(':').collect();
                match (
                    parts.as_slice(),
                    parts.get(1).and_then(|x| x.parse::<usize>().ok()),
                    parts.get(2).and_then(|x| x.parse::<u32>().ok()),
                ) {
                    (["t", _, _], Some(li), Some(id)) if li == index => id,
                    _ => return Err(format!("towers_remove: {s} is not one of this town's towers (t:{index}:<n>) nor a corner")),
                }
            }
            _ => return Err("towers_remove: wall corners or tower ids (t:<layout>:<n>)".into()),
        };
        if let Some(i) = w.towers_add.iter().position(|h| h.id == id) {
            w.towers_add.remove(i);
            if !w.towers_remove.contains(&id) {
                w.towers_remove.push(id);
            }
            a.towers_remove.push(id);
            continue;
        }
        match before.tower_index(id as usize).map(|i| standing[i].2) {
            None => a.refused.push(json!({ "tower_remove": t, "why": format!("no tower t:{index}:{id} stands on the wall") })),
            Some(true) => a
                .refused
                .push(json!({ "tower_remove": t, "why": "that is a gate's tower: take the gate out (gates_remove) instead" })),
            Some(false) => {
                w.towers_remove.push(id);
                a.towers_remove.push(id);
            }
        }
    }
    let gen_corner = |k: usize| -> Option<u32> {
        let at = world_pt(mesh.pos[k]);
        (0..base.len())
            .filter(|&i| !base[i].1 && dist(base[i].0, at) < 30.0)
            .min_by(|&x, &y| dist(base[x].0, at).total_cmp(&dist(base[y].0, at)))
            .map(|i| i as u32)
    };
    let mut next_id = w
        .towers_add
        .iter()
        .map(|h| h.id + 1)
        .chain(w.towers_remove.iter().filter(|&&i| (HAND_TOWER_ID..GATE_TOWER_ID).contains(&i)).map(|i| i + 1))
        .max()
        .unwrap_or(HAND_TOWER_ID)
        .max(HAND_TOWER_ID);
    for t in &r.towers_add {
        let (v, wv, f) = match t {
            Value::Number(n) => {
                let k = corner(n.as_u64().unwrap_or(u64::MAX).min(u32::MAX as u64) as u32)?;
                if let Some(i) = corner_tower(k) {
                    a.refused
                        .push(json!({ "tower_add": t, "why": format!("a tower stands at that corner already (t:{index}:{})", before.tower_id(i)) }));
                    continue;
                }
                // (A corner's tower taken down goes back up.)
                let back = gen_corner(k)
                    .filter(|id| w.towers_remove.contains(id))
                    .or_else(|| w.towers_remove.iter().copied().find(|&id| id == CORNER_TOWER_ID + k as u32));
                match back {
                    Some(id) => {
                        w.towers_remove.retain(|&x| x != id);
                        a.towers_back.push(id);
                    }
                    None => a
                        .refused
                        .push(json!({ "tower_add": t, "why": "a tower goes up on a corner only where one was taken down; give [corner, next corner, f] for one along a side" })),
                }
                continue;
            }
            Value::Array(xs) if xs.len() == 2 || xs.len() == 3 => {
                let v = corner(xs[0].as_u64().unwrap_or(u64::MAX).min(u32::MAX as u64) as u32)?;
                let wv = corner(xs[1].as_u64().unwrap_or(u64::MAX).min(u32::MAX as u64) as u32)?;
                let f = xs.get(2).map_or(Some(0.5), |x| x.as_f64()).ok_or("towers_add: [corner, next corner, f between 0 and 1]")?;
                if !neighbours(plan, v, wv) {
                    return Err(format!("towers_add: corners {v} and {wv} are not next to each other on the wall"));
                }
                if !(0.05..=0.95).contains(&f) {
                    return Err("towers_add: f is how far along the side, 0.05 to 0.95".into());
                }
                (v, wv, f)
            }
            _ => return Err("towers_add: a wall corner (to put back its tower) or [corner, next corner, f]".into()),
        };
        let at = world_pt(lerp(mesh.pos[v], mesh.pos[wv], f));
        if !walled {
            a.refused.push(json!({ "tower_add": t, "why": "the town has no wall standing" }));
            continue;
        }
        let why = tower_why(&before, index, at, unit(sub(mesh.pos[wv], mesh.pos[v])));
        if let Some(why) = why {
            a.refused.push(json!({ "tower_add": t, "why": why }));
            continue;
        }
        let id = next_id;
        next_id += 1;
        let at_v = planned(v);
        w.towers_add.push(HandTower {
            id,
            v: v as u32,
            w: wv as u32,
            f: crate::core::round(f * 1000.0) / 1000.0,
            at: at_v,
        });
        a.towers_add.push(id);
    }
    Ok(a)
}

fn unit(v: P) -> P {
    mul(v, 1.0 / len(v).max(1e-9))
}

/// Why a tower can't go up at `at` on the wall running `along` (world ft), in town `l` as it stands.
fn tower_why(l: &Layout, index: usize, at: P, along: P) -> Option<String> {
    const R: f64 = 11.0;
    for (k, (p, r, _)) in crate::interior::towers(l).into_iter().enumerate() {
        if dist(p, at) < r + R + TOWER_GAP_FT {
            return Some(format!("within {TOWER_GAP_FT} ft of tower t:{index}:{}", l.tower_id(k)));
        }
    }
    let gates = if l.gate_spots.len() == l.gates.len() { l.gate_spots.clone() } else { Vec::new() };
    for (g, p) in gates.iter().zip(&l.gates) {
        if dist(*p, at) < GATE_HALF + 15.0 + R + TOWER_GAP_FT {
            return Some(format!("within {TOWER_GAP_FT} ft of gate gt:{index}:{g}"));
        }
    }
    for b in &l.buildings {
        let n = b.poly.len();
        if contains(&b.poly, at) || (0..n).any(|i| seg_dist(at, b.poly[i], b.poly[(i + 1) % n]) < R + 2.0) {
            return Some(format!("over building b:{index}:{}", b.id));
        }
    }
    for (pts, class, w) in &l.roads {
        for s in pts.windows(2) {
            let d = sub(s[1], s[0]);
            let dl = len(d);
            if dl < 1e-6 {
                continue;
            }
            // (The paved strip along the wall inside it is the wall's street: towers stand on it.)
            if *class == 3 && (dot(d, along) / dl).abs() > 0.5 {
                continue;
            }
            if seg_dist(at, s[0], s[1]) < R + 0.5 * w {
                return Some("over a street or road that crosses the wall there".into());
            }
        }
    }
    None
}

/// The reply's `wall` part: gates and towers put in and taken out (ids and names) and what was
/// refused; refusals that only showed once laid out (a gate with no way out, a tower in the water)
/// are taken out of `e`.
pub(super) fn report(world: &World, t0: &T0, index: usize, before: &Layout, after: &Layout, e: &mut TownEdit, a: Asked) -> Value {
    let names = |l: &Layout| -> FastMap<String, String> { super::places::of(world, t0, l).into_iter().filter(|p| p.kind == "gate").map(|p| (p.id, p.name)).collect() };
    let (nb, na) = (names(before), names(after));
    let mut refused = a.refused;
    let mut gates_added = Vec::new();
    for g in &a.gates_add {
        let id = format!("gt:{index}:{g}");
        if let Some((_, why)) = after.hand_wall.refused.iter().find(|r| r.0 == *g) {
            refused.push(json!({ "gate_add": g, "why": why }));
            e.wall.gates_add.retain(|c| c.v != *g);
            continue;
        }
        let mut x = json!({ "corner": g, "id": id, "name": na.get(&id) });
        if let Some((_, l)) = after.hand_wall.ways.iter().find(|w| w.0 == *g) {
            x["way_out_ft"] = json!(crate::core::round(*l));
        }
        let towers: Vec<String> = (0..crate::interior::towers(after).len())
            .map(|k| after.tower_id(k) as u32)
            .filter(|&t| t == GATE_TOWER_ID + 2 * g || t == GATE_TOWER_ID + 2 * g + 1)
            .map(|t| format!("t:{index}:{t}"))
            .collect();
        if !towers.is_empty() {
            x["towers"] = json!(towers);
        }
        if after.gate_spots.is_empty() {
            x["built"] = json!(false);
        }
        gates_added.push(x);
    }
    let gates_removed: Vec<Value> = a
        .gates_remove
        .iter()
        .map(|g| json!({ "corner": g, "id": format!("gt:{index}:{g}"), "name": nb.get(&format!("gt:{index}:{g}")) }))
        .collect();
    let ids_after: Vec<usize> = (0..crate::interior::towers(after).len()).map(|k| after.tower_id(k)).collect();
    let all_after = crate::interior::towers(after);
    let mut towers_added = Vec::new();
    for id in a.towers_add.iter().chain(&a.towers_back) {
        match ids_after.iter().position(|&x| x == *id as usize) {
            Some(k) => towers_added.push(json!({ "id": format!("t:{index}:{id}"), "x_ft": crate::core::round(all_after[k].0[0]), "y_ft": crate::core::round(all_after[k].0[1]) })),
            None => {
                refused.push(json!({ "tower_add": format!("t:{index}:{id}"), "why": "it would stand in the water" }));
                e.wall.towers_add.retain(|h| h.id != *id);
            }
        }
    }
    let mut out = json!({});
    if !gates_added.is_empty() {
        out["gates_added"] = json!(gates_added);
    }
    if !gates_removed.is_empty() {
        out["gates_removed"] = json!(gates_removed);
    }
    if !towers_added.is_empty() {
        out["towers_added"] = json!(towers_added);
    }
    if !a.towers_remove.is_empty() {
        out["towers_removed"] = json!(a.towers_remove.iter().map(|id| format!("t:{index}:{id}")).collect::<Vec<_>>());
    }
    if !refused.is_empty() {
        out["refused"] = json!(refused);
    }
    out
}

/// `get_town_plan`'s gates and towers set by hand.
pub(super) fn plan_json(world: &World, t0: &T0, index: usize, e: Option<&TownEdit>) -> Option<Value> {
    let e = e.filter(|e| !e.wall.is_empty())?;
    let l = super::base_layout(world, t0, index);
    let names: FastMap<String, String> = super::places::of(world, t0, &l).into_iter().filter(|p| p.kind == "gate").map(|p| (p.id, p.name)).collect();
    let gates: Vec<Value> = e
        .wall
        .gates_add
        .iter()
        .map(|c| {
            let id = format!("gt:{index}:{}", c.v);
            let mut x = json!({ "corner": c.v, "id": id, "name": names.get(&id) });
            if let Some((_, why)) = l.hand_wall.refused.iter().find(|r| r.0 == c.v) {
                x["refused"] = json!(why);
            }
            x
        })
        .collect();
    Some(json!({
        "gates_added": gates,
        "gates_removed": e.wall.gates_remove.iter().map(|c| c.v).collect::<Vec<_>>(),
        "towers_added": e.wall.towers_add.iter().map(|h| json!({ "id": format!("t:{index}:{}", h.id), "corners": [h.v, h.w], "f": h.f })).collect::<Vec<_>>(),
        "towers_removed": e.wall.towers_remove.iter().map(|id| format!("t:{index}:{id}")).collect::<Vec<_>>(),
    }))
}

/// Whether a gate at corner `v` was put in by hand or taken out (`get_town_plan`'s corners).
pub(super) fn corner_marks(e: Option<&TownEdit>, v: usize) -> (bool, bool) {
    e.map_or((false, false), |e| {
        (e.wall.gates_add.iter().any(|c| c.v as usize == v), e.wall.gates_remove.iter().any(|c| c.v as usize == v))
    })
}

// ---------------------------------------------------------------------------------------
// M18a leftover: `street_gates` on a ring laid round patches, asked again without them.

/// A closed wall (world ft) whose every corner is a corner of one town's plan as laid out (a ring
/// laid round its patches: `handwall::around_patches`): that ring found again with a gate on every
/// street crossing it, its gates and crossings by the corners of `pts`. None for any other line.
pub fn ring_street_gates(world: &World, t0: &T0, pts: &[P]) -> Option<(Vec<u32>, Vec<super::handwall::Crossing>)> {
    if pts.len() < 3 {
        return None;
    }
    let s = super::handwall::town_of_wall(t0, centroid(pts))?;
    let index = t0.settlements.iter().position(|x| std::ptr::eq(x, s))?;
    if s.tier < crate::t0::settle::Tier::Town {
        return None;
    }
    let plan = super::town_plan(world, t0, index)?;
    let center = [s.x, s.y];
    let (ch, _) = super::wards::edit_of(world, t0, index).map(|e| super::wards::changes(&plan, e, center)).unwrap_or_default();
    let mesh = super::wards::moved_mesh(&plan, &ch.moves);
    let (ward, _) = ch.wards(&plan);
    let mut on: Vec<usize> = pts
        .iter()
        .map(|p| (0..mesh.pos.len()).find(|&v| !plan.vf[v].is_empty() && dist(add(mesh.pos[v], center), *p) < 1.0))
        .collect::<Option<_>>()?;
    let local: Vec<P> = pts.iter().map(|p| sub(*p, center)).collect();
    let nf = mesh.faces.len();
    let patches: Vec<usize> = (0..nf)
        .filter(|&f| mesh.faces[f].len() >= 3 && plan.face_ok[f] && !plan.wet[f] && (f < plan.n_inner || plan.ward[f].is_some() || ward[f].is_some()))
        .filter(|&f| contains(&local, centroid(&mesh.face_pts(f))))
        .collect();
    let ring = super::handwall::around_patches(world, t0, index, &patches, true).ok()?;
    let mut theirs = ring.plan_corners.clone();
    on.sort_unstable();
    theirs.sort_unstable();
    if on != theirs {
        return None;
    }
    let mine = |q: P| pts.iter().position(|p| dist(*p, q) < 1.0);
    let gates = ring.gates.iter().filter_map(|&g| mine(ring.pts[g as usize]).map(|k| k as u32)).collect();
    let crossings = ring.crossings.iter().map(|c| super::handwall::Crossing { corner: mine(c.at), ..c.clone() }).collect();
    Some((gates, crossings))
}
