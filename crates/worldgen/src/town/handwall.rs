//! (R5 towns, M18) Walls drawn by hand round a town's patches, and where streets cross a wall.
//!
//! `around_patches` lays a ring on the outer edge of a set of patches on the town's plan (the
//! patch edges are the middle lines of the streets, so the wall stands down the middle of the
//! street round them: a lane inside it, a lane outside, the blocks untouched). The line is
//! frozen in the world file when the wall is made (`Created.pts`): a pure function of the plan
//! as it stood then; `update_fortification` with `around_patches` lays it again on the plan as
//! it stands. Its gates: where a main street crosses (always), where a side street crosses (with
//! `street_gates`), and, when no road, approach or main street would give the ring a gate, at
//! the side street crossing nearest the market square (the busiest), so every ring has a way in.

use super::geom::*;
use super::wards;
use crate::World;
use crate::core::hash::{FastMap, FastSet};
use crate::t0::T0;
use crate::t0::settle::{Settlement, Tier};
use crate::world::Created;

/// Most corners a ring round patches may have (a city's old town is 100-200 patch corners).
pub const RING_MAX_CORNERS: usize = 256;
/// A gate this near a corner is put on the corner (as `sites::wall_site`).
const CORNER_SNAP_FT: f64 = 20.0;

/// A street where it crosses a wall: its corner (an index into the wall's line), where it is
/// (world ft), a main street or a side street, and whether it has a gate.
#[derive(Clone, Debug)]
pub struct Crossing {
    pub corner: Option<usize>,
    pub at: P,
    pub main: bool,
    pub gate: bool,
}

/// A ring laid round patches: its line (world ft), gate corners, the streets crossing it, the
/// patches it encloses that were not asked for (holes in the set), and its plan corners.
#[derive(Clone, Debug, Default)]
pub struct Ring {
    pub pts: Vec<P>,
    pub gates: Vec<u32>,
    pub crossings: Vec<Crossing>,
    pub enclosed: Vec<usize>,
    pub plan_corners: Vec<usize>,
    /// Sides (from corner k to the next) with no street on them: patches of one district group,
    /// their lots running across the edge (the buildings there stand in the wall's way).
    pub bare: Vec<usize>,
}

/// The ring round `patches` of town `index` on its plan as laid out (`Edits.towns`): refused
/// for patches not on the plan, a set in more than one piece, patches meeting only at a corner,
/// a side on the town's own standing wall, or more than `RING_MAX_CORNERS` corners.
pub fn around_patches(world: &World, t0: &T0, index: usize, patches: &[usize], street_gates: bool) -> Result<Ring, String> {
    let s = super::settlement(t0, index).ok_or("not a settlement")?;
    if s.tier < Tier::Town {
        return Err("a village has no patches: give the wall's line (pts)".into());
    }
    let plan = super::town_plan(world, t0, index).ok_or("not a town")?;
    let center = [s.x, s.y];
    let (ch, _) = wards::edit_of(world, t0, index).map(|e| wards::changes(&plan, e, center)).unwrap_or_default();
    let mesh = wards::moved_mesh(&plan, &ch.moves);
    let (ward, group) = ch.wards(&plan);
    let nf = plan.mesh.faces.len();
    let listed = |p: usize| p < nf && plan.mesh.faces[p].len() >= 3 && plan.face_ok[p] && !plan.wet[p] && (p < plan.n_inner || plan.ward[p].is_some() || ward[p].is_some());
    if patches.is_empty() {
        return Err("around_patches: at least one patch (get_town_plan lists them)".into());
    }
    if let Some(p) = patches.iter().find(|&&p| !listed(p)) {
        return Err(format!("patch {p} is not one of the town's patches (get_town_plan lists them)"));
    }
    let set: FastSet<usize> = patches.iter().copied().collect();
    let in_set = |f: i32| f >= 0 && set.contains(&(f as usize));
    // One piece: patches joined by a side.
    let mut sorted: Vec<usize> = set.iter().copied().collect();
    sorted.sort_unstable();
    let mut seen: FastSet<usize> = FastSet::default();
    let mut pieces: Vec<Vec<usize>> = Vec::new();
    for &p in &sorted {
        if seen.contains(&p) {
            continue;
        }
        let (mut piece, mut todo) = (Vec::new(), vec![p]);
        seen.insert(p);
        while let Some(q) = todo.pop() {
            piece.push(q);
            for &nb in &mesh.labels[q] {
                if in_set(nb) && seen.insert(nb as usize) {
                    todo.push(nb as usize);
                }
            }
        }
        piece.sort_unstable();
        pieces.push(piece);
    }
    if pieces.len() > 1 {
        // (A patch outside the town on its own: it isn't one of the town's.)
        if let Some(p) = pieces.iter().find(|q| q.iter().all(|&f| f >= plan.n_inner)).map(|q| q[0]) {
            return Err(format!("patch {p} is not one of the town's patches: it lies outside the town (get_town_plan marks the town's in_town)"));
        }
        let list: Vec<String> = pieces.iter().map(|p| format!("[{}]", p.iter().map(|q| q.to_string()).collect::<Vec<_>>().join(", "))).collect();
        return Err(format!("those patches are not one piece: {} (a ring goes round one piece: add the patches between them, or make a wall for each)", list.join(" and ")));
    }
    // The sides with a patch of the set on one side only, chained into loops.
    let mut adj: FastMap<usize, Vec<usize>> = FastMap::default();
    let mut faces_of: FastMap<(usize, usize), (i32, i32)> = FastMap::default();
    for f in 0..nf {
        let m = mesh.faces[f].len();
        for e in 0..m {
            let (a, b) = (mesh.faces[f][e], mesh.faces[f][(e + 1) % m]);
            let k = (a.min(b), a.max(b));
            let slot = faces_of.entry(k).or_insert((-1, -1));
            if slot.0 < 0 { slot.0 = f as i32 } else { slot.1 = f as i32 }
        }
    }
    for &p in &sorted {
        let m = mesh.faces[p].len();
        for e in 0..m {
            if !in_set(mesh.labels[p][e]) {
                let (a, b) = (mesh.faces[p][e], mesh.faces[p][(e + 1) % m]);
                adj.entry(a).or_default().push(b);
                adj.entry(b).or_default().push(a);
            }
        }
    }
    let world_pt = |v: usize| add(mesh.pos[v], center);
    let mut starts: Vec<usize> = adj.keys().copied().collect();
    starts.sort_unstable();
    if let Some(&v) = starts.iter().find(|v| adj[*v].len() > 2) {
        let q = world_pt(v);
        return Err(format!("those patches meet only at corner {v} ({:.0}, {:.0}): a ring can't pass a corner twice; add a patch there or leave one out", q[0], q[1]));
    }
    let mut used: FastSet<usize> = FastSet::default();
    let mut loops: Vec<Vec<usize>> = Vec::new();
    for &v0 in &starts {
        if used.contains(&v0) {
            continue;
        }
        let (mut lp, mut prev, mut v) = (vec![v0], usize::MAX, v0);
        used.insert(v0);
        loop {
            let Some(&next) = adj[&v].iter().find(|&&w| w != prev && (!used.contains(&w) || (w == v0 && lp.len() > 2))) else { break };
            if next == v0 {
                break;
            }
            used.insert(next);
            lp.push(next);
            (prev, v) = (v, next);
        }
        loops.push(lp);
    }
    let ring_area = |lp: &[usize]| area(&lp.iter().map(|&v| mesh.pos[v]).collect::<Vec<_>>()).abs();
    let outer = loops.iter().max_by(|a, b| ring_area(a).total_cmp(&ring_area(b))).cloned().ok_or("those patches have no outline")?;
    let local: Vec<P> = outer.iter().map(|&v| mesh.pos[v]).collect();
    // (Patches inside the ring that were not asked for: the holes of the set.)
    let enclosed: Vec<usize> = (0..nf).filter(|&f| !set.contains(&f) && listed(f) && contains(&local, centroid(&mesh.face_pts(f)))).collect();
    // Corners less than 10 ft from the one before go (a wall's sides are 10 ft at least).
    let mut vs: Vec<usize> = Vec::new();
    for &v in &outer {
        if vs.last().is_none_or(|&u| dist(mesh.pos[u], mesh.pos[v]) >= 10.0) {
            vs.push(v);
        }
    }
    while vs.len() > 3 && dist(mesh.pos[vs[0]], mesh.pos[*vs.last().unwrap()]) < 10.0 {
        vs.pop();
    }
    if vs.len() > RING_MAX_CORNERS {
        return Err(format!("a ring round those patches has {} corners (at most {RING_MAX_CORNERS}): wall fewer patches, or give its line (pts)", vs.len()));
    }
    // Not on the town's own wall where it stands.
    if ch.walls.unwrap_or(plan.walled) {
        let mut on_wall: FastSet<(usize, usize)> = FastSet::default();
        for (chain, closed, _) in &plan.wall_runs {
            let k = chain.len();
            for i in 0..if *closed { k } else { k.saturating_sub(1) } {
                let (a, b) = (chain[i], chain[(i + 1) % k]);
                on_wall.insert((a.min(b), a.max(b)));
            }
        }
        for i in 0..vs.len() {
            let (a, b) = (vs[i], vs[(i + 1) % vs.len()]);
            if on_wall.contains(&(a.min(b), a.max(b))) {
                let q = world_pt(a);
                return Err(format!("the ring's side from corner {a} ({:.0}, {:.0}) to corner {b} runs on the town's own wall: leave out the patches on the town's edge there, or take its walls down (edit_town walls: false)", q[0], q[1]));
            }
        }
    }
    let ring_of: FastMap<usize, usize> = vs.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    let side_of = |a: usize, b: usize| -> i8 {
        let (f, g) = faces_of.get(&(a.min(b), a.max(b))).copied().unwrap_or((-1, -1));
        match (in_set(f), in_set(g)) {
            (true, true) => 1,
            (false, false) => -1,
            _ => 0,
        }
    };
    // A street runs on a side: inside, between two patches not merged; outside, beside a patch of
    // the town or its outskirts (not the water).
    let street = |a: usize, b: usize| -> bool {
        let (f, g) = faces_of.get(&(a.min(b), a.max(b))).copied().unwrap_or((-1, -1));
        if f < 0 || g < 0 {
            return false;
        }
        let (f, g) = (f as usize, g as usize);
        if plan.wet[f] || plan.wet[g] || (group[f] != usize::MAX && group[f] == group[g]) {
            return false;
        }
        listed(f) || listed(g)
    };
    let bare: Vec<usize> = (0..vs.len())
        .filter(|&i| {
            let (a, b) = (vs[i], vs[(i + 1) % vs.len()]);
            let (f, g) = faces_of.get(&(a.min(b), a.max(b))).copied().unwrap_or((-1, -1));
            f >= 0 && g >= 0 && group[f as usize] != usize::MAX && group[f as usize] == group[g as usize]
        })
        .collect();
    let near_ring = |v: usize| -> Option<usize> { ring_of.get(&v).copied().or_else(|| (0..vs.len()).min_by(|&i, &j| dist(mesh.pos[vs[i]], mesh.pos[v]).total_cmp(&dist(mesh.pos[vs[j]], mesh.pos[v])))) };
    // Main streets: each path, where it goes from inside to outside (or back), through the ring
    // corner where it first met the wall.
    let mut main: Vec<usize> = Vec::new();
    for path in &plan.paths {
        let (mut last, mut met): (i8, Option<usize>) = (0, None);
        for k in 1..path.len() {
            let c = side_of(path[k - 1], path[k]);
            if c == 0 {
                met = met.or(Some(path[k - 1]));
                continue;
            }
            if last != 0 && c != last {
                if let Some(i) = near_ring(met.unwrap_or(path[k - 1])) {
                    main.push(i);
                }
            }
            (last, met) = (c, None);
        }
    }
    main.sort_unstable();
    main.dedup();
    // Side streets: a corner of the ring with a street going out (its corners are where three
    // patches meet, so a street there ran on into the streets inside and round it; one going in
    // meets the wall at a T inside).
    let mut side: Vec<usize> = Vec::new();
    let mut way_in: Vec<usize> = Vec::new();
    for (i, &v) in vs.iter().enumerate() {
        let mut ends: Vec<usize> = Vec::new();
        for &f in &plan.vf[v] {
            let m = mesh.faces[f].len();
            if let Some(e) = mesh.faces[f].iter().position(|&u| u == v) {
                ends.push(mesh.faces[f][(e + 1) % m]);
                ends.push(mesh.faces[f][(e + m - 1) % m]);
            }
        }
        ends.sort_unstable();
        ends.dedup();
        let goes = |s: i8| ends.iter().any(|&w| side_of(v, w) == s && street(v, w));
        if goes(1) {
            way_in.push(i);
        }
        if goes(-1) && !main.contains(&i) {
            side.push(i);
        }
    }
    let mut gates: Vec<u32> = main.iter().map(|&i| i as u32).collect();
    if street_gates {
        gates.extend(side.iter().map(|&i| i as u32));
    }
    let pts: Vec<P> = vs.iter().map(|&v| world_pt(v)).collect();
    // A way in: no gate would come of the ring (none set, no road, approach or main street
    // crossing it on dry ground): the side street nearest the market square gets one (else the
    // corner nearest it where a street comes from inside, else the corner nearest it).
    let plaza = plan.plaza.map_or(center, |p| add(centroid(&mesh.face_pts(p)), center));
    let by_plaza = |c: &[usize]| {
        let mut c = c.to_vec();
        c.sort_by(|&a, &b| dist(pts[a], plaza).total_cmp(&dist(pts[b], plaza)));
        c
    };
    let mut order = by_plaza(&side);
    order.extend(by_plaza(&way_in));
    order.extend(by_plaza(&(0..pts.len()).collect::<Vec<_>>()));
    way_in_gate(world, t0, &pts, &mut gates, &order);
    gates.sort_unstable();
    gates.dedup();
    let mut crossings: Vec<Crossing> = main.iter().map(|&i| (i, true)).chain(side.iter().map(|&i| (i, false))).map(|(i, m)| Crossing { corner: Some(i), at: pts[i], main: m, gate: gates.contains(&(i as u32)) }).collect();
    crossings.sort_by_key(|c| c.corner);
    Ok(Ring { pts, gates, crossings, enclosed, plan_corners: vs, bare })
}

/// A way into a ring: when it would be built with no gate (none set, none where a road, approach
/// or main street crosses, or those in the water), a gate on the first corner of `order` that
/// gives it one (tried in order, up to 12), else on `order`'s first.
pub fn way_in_gate(world: &World, t0: &T0, pts: &[P], gates: &mut Vec<u32>, order: &[usize]) {
    let built = |g: &[u32]| builds_gate(world, t0, pts, g);
    if order.is_empty() || built(gates) {
        return;
    }
    let mut tried: Vec<usize> = Vec::new();
    for &k in order {
        if tried.len() >= 12 {
            break;
        }
        if tried.contains(&k) {
            continue;
        }
        tried.push(k);
        let mut g = gates.clone();
        g.push(k as u32);
        if built(&g) {
            *gates = g;
            gates.sort_unstable();
            return;
        }
    }
    gates.push(order[0] as u32);
    gates.sort_unstable();
}

/// Where the roads, approaches and main streets crossing a wall's line would give it gates (as
/// `sites::wall_site` finds them, before gates set by hand and before close ones merge): world ft.
pub fn road_crossings(world: &World, t0: &T0, pts: &[P], closed: bool) -> Vec<P> {
    let c = Created { kind: "wall".into(), pts: pts.to_vec(), closed, ..Default::default() };
    let lanes = super::sites::wall_lanes(world, t0, &c).lanes;
    let n = pts.len();
    let cuts: Vec<Vec<f64>> = if closed { ring_cuts(pts, &lanes) } else { (0..n.saturating_sub(1)).map(|i| lanes.iter().filter_map(|&(p, q)| cut(pts[i], pts[i + 1], p, q)).collect()).collect() };
    cuts.iter().enumerate().flat_map(|(i, ts)| ts.iter().map(move |&t| lerp(pts[i], pts[(i + 1) % n], t))).collect()
}

/// The side streets (paved strips, class 3) of the towns a wall reaches that cross its line,
/// one point per street (its two strips are one crossing): world ft.
pub fn side_crossings(world: &World, t0: &T0, pts: &[P], closed: bool) -> Vec<P> {
    let c = Created { kind: "wall".into(), pts: pts.to_vec(), closed, ..Default::default() };
    let reached = super::sites::wall_lanes(world, t0, &c).reached;
    let mut lanes: Vec<(P, P)> = Vec::new();
    for i in reached {
        let town = super::base_layout(world, t0, i as usize);
        for (r, _, _) in town.roads.iter().filter(|r| r.1 == 3) {
            lanes.extend(r.windows(2).map(|q| (q[0], q[1])));
        }
    }
    let n = pts.len();
    let cuts: Vec<Vec<f64>> = if closed { ring_cuts(pts, &lanes) } else { (0..n.saturating_sub(1)).map(|i| lanes.iter().filter_map(|&(p, q)| cut(pts[i], pts[i + 1], p, q)).collect()).collect() };
    let mut out: Vec<P> = Vec::new();
    for (i, ts) in cuts.iter().enumerate() {
        let mut ts = ts.clone();
        ts.sort_by(f64::total_cmp);
        for t in ts {
            let at = lerp(pts[i], pts[(i + 1) % n], t);
            if !out.iter().any(|q| dist(*q, at) < CORNER_SNAP_FT) {
                out.push(at);
            }
        }
    }
    out
}

/// A gate at `at` on a wall's line: on the corner within `CORNER_SNAP_FT`, else a new corner on
/// the nearest side (the gate indices after it move up). Returns its corner.
pub fn add_gate(pts: &mut Vec<P>, gates: &mut Vec<u32>, closed: bool, at: P) -> usize {
    let n = pts.len();
    if let Some(k) = (0..n).filter(|&k| dist(pts[k], at) < CORNER_SNAP_FT).min_by(|&a, &b| dist(pts[a], at).total_cmp(&dist(pts[b], at))) {
        if !gates.contains(&(k as u32)) {
            gates.push(k as u32);
            gates.sort_unstable();
        }
        return k;
    }
    let sides = if closed { n } else { n - 1 };
    let i = (0..sides).min_by(|&a, &b| seg_dist(at, pts[a], pts[(a + 1) % n]).total_cmp(&seg_dist(at, pts[b], pts[(b + 1) % n]))).unwrap_or(0);
    let (a, b) = (pts[i], pts[(i + 1) % n]);
    let ab = sub(b, a);
    let t = (dot(sub(at, a), ab) / dot(ab, ab).max(1e-9)).clamp(0.0, 1.0);
    pts.insert(i + 1, lerp(a, b, t));
    for g in gates.iter_mut() {
        if *g as usize > i {
            *g += 1;
        }
    }
    gates.push(i as u32 + 1);
    gates.sort_unstable();
    i + 1
}

/// Where lanes cross a closed ring's sides (fractions along each side). A lane crossing a side
/// counts; one that only comes to the ring (ends on it) counts only where another leaves it on
/// the other side at the same point (a street through a corner), and one running along a side
/// does not (a street round patches walled on their edge).
pub fn ring_cuts(ring: &[P], lanes: &[(P, P)]) -> Vec<Vec<f64>> {
    const ON_FT: f64 = 0.5;
    let n = ring.len();
    let mut cuts: Vec<Vec<f64>> = vec![Vec::new(); n];
    if n < 3 {
        return cuts;
    }
    let near = |p: P| (0..n).map(|i| (seg_dist(p, ring[i], ring[(i + 1) % n]), i)).min_by(|a, b| a.0.total_cmp(&b.0)).unwrap_or((f64::MAX, 0));
    let side = |p: P| if near(p).0 < ON_FT { 0i8 } else if contains(ring, p) { 1 } else { -1 };
    let mut touch: Vec<(P, i8)> = Vec::new();
    for &(p, q) in lanes {
        let (sp, sq) = (side(p), side(q));
        if sp != 0 && sq != 0 {
            for i in 0..n {
                if let Some(t) = cut(ring[i], ring[(i + 1) % n], p, q) {
                    cuts[i].push(t);
                }
            }
        } else if sp == 0 && sq != 0 {
            touch.push((p, sq));
        } else if sq == 0 && sp != 0 {
            touch.push((q, sp));
        }
    }
    for (k, &(p, s)) in touch.iter().enumerate() {
        let twin = |t: &(P, i8)| dist(t.0, p) < 1.0;
        if s == 1 && touch.iter().any(|t| twin(t) && t.1 == -1) && !touch[..k].iter().any(|t| twin(t) && t.1 == 1) {
            let (_, i) = near(p);
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let ab = sub(b, a);
            cuts[i].push((dot(sub(p, a), ab) / dot(ab, ab).max(1e-9)).clamp(0.0, 1.0));
        }
    }
    cuts
}

/// Where the segment a→b crosses p→q, as a fraction along a→b.
fn cut(a: P, b: P, p: P, q: P) -> Option<f64> {
    let (r, s) = (sub(b, a), sub(q, p));
    let d = cross(r, s);
    if d.abs() < 1e-9 {
        return None;
    }
    let ap = sub(p, a);
    let (t, u) = (cross(ap, s) / d, cross(ap, r) / d);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// The settlement a wall drawn by hand at `center` stands in or round (within its reach): its
/// walls are built in that town's style.
pub fn town_of_wall<'a>(t0: &'a T0, center: P) -> Option<&'a Settlement> {
    t0.settlements.iter().chain(t0.made.iter().map(|m| &m.s)).filter(|s| dist([s.x, s.y], center) <= super::reach(s)).min_by(|a, b| dist([a.x, a.y], center).total_cmp(&dist([b.x, b.y], center)))
}

/// Why a castle's or wall's line is refused where it meets the walls of layout `l`: the wall's id
/// (a wall or castle drawn by hand, else the town's own walls) and where (world ft).
pub fn wall_crossed(world: &World, t0: &T0, l: &super::Layout, segs: &[(P, P)]) -> String {
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

/// The gates of a wall's layout (world ft): each pair of gate towers (52 ft apart, `wall_pieces`)
/// is one, at their middle; a lone tower is a gate at the end of a line.
pub fn gates_of(l: &super::Layout) -> Vec<P> {
    let t = &l.gate_towers;
    let mut out = Vec::new();
    let mut k = 0;
    while k < t.len() {
        if k + 1 < t.len() && dist(t[k], t[k + 1]) < 60.0 {
            out.push(lerp(t[k], t[k + 1], 0.5));
            k += 2;
        } else {
            out.push(t[k]);
            k += 1;
        }
    }
    out
}

/// Whether a ring with this line and gate corners is built with a gate (`sites::wall_site`: those
/// set, and where roads, approaches and main streets cross, less those in the water).
pub fn builds_gate(world: &World, t0: &T0, pts: &[P], gates: &[u32]) -> bool {
    !super::sites::wall_gates(world, t0, &Created { kind: "wall".into(), pts: pts.to_vec(), gates: gates.to_vec(), closed: true, ..Default::default() }).is_empty()
}
