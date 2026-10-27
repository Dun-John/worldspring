//! Castles: a curtain wall along the brow of the ground round its highest point (where the ground
//! falls away from the top, inside the castle's block), towers at its turns, round or (in some
//! castles) square, a gatehouse on the side facing the way in; in a big one an inner curtain round
//! the keep, an inner bailey inside an outer. The keep stands on the high point, its footprint
//! made of pieces (`Building::parts`): a great tower with corner towers, a hall with a
//! forebuilding, an L or a T with towers at its outer corners, or a round shell keep on a motte.
//! Ranges of buildings stand against the inside of the outer curtain (or of the town wall where that
//! is the castle's).
//!
//! The keep's pieces are laid on the 5-ft grid of its body (whole squares from the body's corner),
//! so its interior's squares meet its walls and its towers' rooms come out round about their middle
//! (`interior::Shell::of`).

use super::catalog::Ward;
use super::geom::*;
use super::{Building, District, Ground, Layout, Motte, Part, PartKind, Plaza, PlazaKind, Structure, wall_pieces};
use crate::core::rng::Pcg32;

/// The curtain stands this far in from the block's edge (ft).
const CURTAIN_IN: f64 = 6.0;
/// Ground with less relief than this over the castle is flat: the curtain keeps to the block.
const FLAT_FT: f64 = 5.0;
/// The drops below the top (ft) a curtain may follow, nearest the top first.
const DROPS: [f64; 6] = [6.0, 9.0, 12.0, 16.0, 22.0, 30.0];
/// A curtain along the brow stands back this far from a river's channel (ft), as a block's quay
/// does: clear of the river where walls are cut.
const WATER_BACK: f64 = 16.0;
/// The least distance from the high point to the curtain (ft): room for a keep and a yard.
const R_MIN: f64 = 48.0;
/// A curtain enclosing this much (sq ft) and this wide (ft) may get an inner curtain round the
/// keep: a capital's; else a town's as its patch was sized for one (`Setting::inner`), or enclosing
/// `TOWN_INNER` where it has the whole patch, one drawn by hand `HAND_INNER` and 200 ft across.
const BIG_AREA: f64 = 30_000.0;
const BIG_WIDTH: f64 = 160.0;
const TOWN_INNER: f64 = 170_000.0;
const HAND_INNER: f64 = 80_000.0;
/// The outer bailey's least width between the two curtains (ft), and its width on the gate's
/// side, where it is broad (a yard to use).
const BAILEY_GAP: f64 = 48.0;
const BROAD: f64 = 80.0;
/// The inner gate stands at least this far from the outer one (ft): the way between them runs
/// along the outer bailey, not straight through both.
const GATES_APART: f64 = 90.0;
/// The share of castles whose towers are square.
const SQUARE_SHARE: f64 = 0.35;
/// The gate passage's half width between its towers, as `wall_pieces` lays it (ft).
const GATE_HALF: f64 = 26.0;
/// A side of the castle this near a town wall (ft), and running along it, is that wall.
const WALL_SIDE: f64 = 9.0;
/// A gate tower stands this far from any other tower (ft, centre to centre), where a gate fits so.
const GATE_CLEAR: f64 = 34.0;
/// A castle's gate stands this far from a town gate (ft), where it fits so.
const GATE_AWAY: f64 = 90.0;
/// Towers of a castle and of the town stand this far clear of each other (ft, wall to wall).
const TOWERS_CLEAR: f64 = 12.0;
/// A corner of a castle's ring sharper than this (degrees) is cut back where it is this wide (ft):
/// no wedge of yard between two walls closing in on each other.
const SPIKE_DEG: f64 = 40.0;
const SPIKE_WIDE: f64 = 60.0;

/// How a castle is laid out.
#[derive(Clone, Copy, Debug)]
pub(super) struct Setting {
    /// The curtain follows the brow of the ground (a town's castle); else it keeps to the outline
    /// (one drawn by hand: only its keep and an inner curtain go to the high ground).
    pub follow: bool,
    /// A capital's castle: an inner curtain wherever one fits.
    pub capital: bool,
    /// The town wall stands along the sides given as on it (else they get a curtain of their own).
    pub wall_up: bool,
    /// A castle the town planned (not one set by hand in its place): a town tower crowding its
    /// gatehouse goes (a hand castle's leave every `t:` id as it was).
    pub planned: bool,
    /// An inner curtain where it fits (a town's, its patch cut to a size for one: `cut_bound`);
    /// None: by its size (a town's on its whole patch, one drawn by hand).
    pub inner: Option<bool>,
    /// A great castle's (a capital's, a metropolis's): a hall range in its inner bailey (else along
    /// the curtain) before the other ranges.
    pub hall: bool,
}

/// A castle on `block` (convex, local ft): its curtain (see the module), gate facing `gate_toward`
/// (in a town, on the side nearest a street, facing the centre among those); where a side runs
/// along a town wall already standing, that wall is the castle's on that side. `laid`: the town
/// wall's stretches and towers as laid out, standing or not (its gate and way keep clear of them
/// either way: the same castle with the wall taken down). With `keep`, the
/// keep (4 storeys, pushed last), and with `yard`, ranges of buildings against the inside of the
/// curtain, clear of the keep and of the way from gate to gate. Its bailey is an open yard
/// (`PlazaKind::Bailey`). Its outer curtain's line.
#[allow(clippy::too_many_arguments)]
pub(super) fn castle<G: Ground>(
    site: &G,
    block_in: &[P],
    gate_toward: P,
    w: Ward,
    _dp: &District,
    set: Setting,
    town_wall: &[(P, P)],
    town_gates: &[P],
    laid: (&[(P, P)], &[(P, f64)]),
    prng: &mut Pcg32,
    l: &mut Layout,
    keep: bool,
    yard: bool,
) -> Option<Vec<P>> {
    // The town wall's line where it runs along the block (planned: the castle is the same with the
    // wall taken down, but for a curtain on that side and the ranges by it); a side along it is that
    // wall.
    let town_walls: &[(P, P)] = town_wall;
    // (A stream's water gap in it is the wall's, but where a street crosses into the castle's
    // ground there.)
    let crossing: Vec<(P, P)> = l
        .roads
        .iter()
        .filter(|r| r.1 != 3)
        .flat_map(|r| r.0.windows(2).map(|q| (q[0], q[1])).collect::<Vec<_>>())
        .collect();
    let along = |a: P, b: P| {
        let bridged = crossing.iter().any(|&(q, r)| crosses(a, b, q, r));
        // (A gap the wall as laid has there, by the water: a stretch of it ends near.)
        let gap = |p: P| town_walls.iter().any(|&(q, r)| dist(q, p).min(dist(r, p)) < 30.0);
        along_wall(town_walls, town_gates, &|p| !bridged && gap(p) && site.near_water(p, 6.0), a, b)
    };
    // The curtain's bound: 6 ft in from the block; a side on a town wall's line (`build_town` sets
    // it there) stays so where that wall runs its whole length, else stands well back from it.
    // (Corners a step apart, as an inset leaves them, made one.)
    let mut block: Vec<P> = Vec::new();
    for p in block_in {
        if block.last().is_none_or(|q| dist(*q, *p) > 0.5) {
            block.push(*p);
        }
    }
    while block.len() > 1 && dist(block[0], block[block.len() - 1]) <= 0.5 {
        block.pop();
    }
    if block.len() < 3 {
        return None;
    }
    if area(&block) < 0.0 {
        block.reverse();
    }
    let m = block.len();
    let d: Vec<f64> = (0..m)
        .map(|e| {
            let (a, b) = (block[e], block[(e + 1) % m]);
            let el = dist(a, b).max(1e-9);
            let u = mul(sub(b, a), 1.0 / el);
            let n = mul([-u[1], u[0]], CURTAIN_IN);
            let mid = lerp(a, b, 0.5);
            let on_line = town_walls
                .iter()
                .any(|&(q, r)| dist(q, r) >= 1.0 && cross(u, mul(sub(r, q), 1.0 / dist(q, r))).abs() < 0.12 && seg_dist(mid, q, r) <= 3.0);
            if on_line && !along(add(a, n), add(b, n)) { 30.0 } else { CURTAIN_IN }
        })
        .collect();
    let mut outer = inset(&block, &d);
    // A town gate at the end of a side on the town wall (no curtain of the castle's there), or by
    // any side of one set in the town by hand (on a patch the town didn't plan for it): the
    // castle's corner is cut back from it, square to the way in from the gate and clear of its
    // towers, so the gate opens onto the town, not into the bailey.
    for &g in town_gates {
        let n = outer.len();
        if n < 3 {
            break;
        }
        if !(0..n).any(|e| (!set.planned || along(outer[e], outer[(e + 1) % n])) && seg_dist(g, outer[e], outer[(e + 1) % n]) < 45.0) {
            continue;
        }
        let to = sub(centroid(&outer), g);
        let u = mul(to, 1.0 / len(to).max(1e-9));
        let reach = laid.1.iter().filter(|t| t.1 > 12.0 && dist(t.0, g) < 60.0).map(|t| dot(sub(t.0, g), u) + 25.0).fold(50.0, f64::max);
        outer = clip_plain(&outer, mul(u, -1.0), -(dot(g, u) + reach));
    }
    // Roads across the castle's ground (an approach road to a gate, a street bent to its bridge;
    // not the paved sides of the patches): it stands clear of them, by a little more than a
    // street's strip.
    // (Not a street laid on a town wall's own line, along a side where that wall is the castle's.)
    let on_wall_line = |a: P, b: P| {
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        town_walls
            .iter()
            .any(|&(q, r)| dist(q, r) >= 1.0 && cross(u, mul(sub(r, q), 1.0 / dist(q, r))).abs() < 0.12 && seg_dist(lerp(a, b, 0.5), q, r) <= 3.0)
    };
    for (pts, _, w) in l.roads.iter().filter(|r| r.1 != 3) {
        for s in pts.windows(2) {
            let (a, b) = (s[0], s[1]);
            let (n, el) = (outer.len(), dist(a, b));
            if n < 3 || el < 1e-6 || on_wall_line(a, b) {
                continue;
            }
            let clear = 0.5 * w + 8.0;
            if super::poly_seg_dist(&outer, a, b) >= clear {
                continue;
            }
            let u = mul(sub(b, a), 1.0 / el);
            let nrm = [-u[1], u[0]];
            let nrm = if dot(sub(centroid(&outer), a), nrm) >= 0.0 { nrm } else { mul(nrm, -1.0) };
            let cut = clip_plain(&outer, mul(nrm, -1.0), -(dot(a, nrm) + clear));
            if cut.len() >= 3 && area(&cut).abs() >= 0.75 * area(&outer).abs() {
                outer = cut;
            }
        }
    }
    if outer.len() < 3 {
        return None;
    }
    let mut others: Vec<(P, f64)> = l
        .towers
        .iter()
        .map(|t| (*t, 11.0))
        .chain(l.gate_towers.iter().map(|t| (*t, 15.0)))
        .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { 11.0 })))
        .chain(laid.1.iter().copied())
        .collect();
    // The ring, laid again where a road runs through it (below).
    let mut cuts = 0;
    let (survey, top, flat, ring) = loop {
        let survey = Survey::of(site, &outer);
        // (The top with room round it for a keep and a yard: a high point at the bound's edge has no
        // brow to follow.)
        let (hp, top) = survey
            .best(|p| inset_depth(&outer, p) >= R_MIN)
            .or_else(|| survey.best(|_| true))
            .unwrap_or((centroid(&outer), 0.0));
        let flat = survey.relief() < FLAT_FT;
        let a_out = area(&outer);
        let walled_out: Vec<bool> = (0..outer.len()).map(|e| along(outer[e], outer[(e + 1) % outer.len()])).collect();
        let ring = if flat || !set.follow {
            None
        } else {
            brow_ring(site, &outer, &walled_out, hp, top, a_out, if set.inner.is_some() { 0.8 } else { 0.5 })
        };
        // (On flat ground, or a top too small to hold it, or drawn by hand: the curtain keeps to the
        // block.)
        let ring = ring.unwrap_or_else(|| outer.clone());
        // Its sharp corners cut back (a town's castle: one drawn by hand keeps to its outline).
        // (Its corners slid along the town wall within the bound, and not up to a town gate.)
        let room = |p: P| inset_depth(&outer, p) >= -1.0 && town_gates.iter().all(|g| dist(*g, p) >= 60.0);
        let ring = if set.follow { meet_wall(&cut_spikes(&ring), &along, &room) } else { ring };
        // A road running through it (one the castle's ground lies across: an approach bent through
        // it, a main street) cuts the bound off it, its larger side kept, and the ring is laid
        // again.
        let through = if cuts < 4 { road_through(&l.roads, &ring) } else { None };
        let Some((a, b, w)) = through else { break (survey, top, flat, ring) };
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        let nrm = [-u[1], u[0]];
        let clear = 0.5 * w + 8.0;
        let sides = [clip_plain(&outer, mul(nrm, -1.0), -(dot(a, nrm) + clear)), clip_plain(&outer, nrm, dot(a, nrm) - clear)];
        let Some(cut) = sides.into_iter().filter(|s| s.len() >= 3).max_by(|x, y| area(x).abs().total_cmp(&area(y).abs())) else {
            break (survey, top, flat, ring);
        };
        if area(&cut).abs() < 0.25 * a_out.abs() {
            break (survey, top, flat, ring);
        }
        outer = cut;
        cuts += 1;
    };
    let walled: Vec<bool> = (0..ring.len()).map(|e| along(ring[e], ring[(e + 1) % ring.len()])).collect();
    let square = prng.next_f64() < SQUARE_SHARE;
    let short = obb(&hull(ring.clone())).short;
    let want = area(&ring) >= BIG_AREA
        && short >= BIG_WIDTH
        && (set.capital
            || set.inner.unwrap_or(if set.follow {
                area(&ring) >= TOWN_INNER
            } else {
                area(&ring) >= HAND_INNER && short >= 200.0
            }));
    // The outer curtain and its towers. (The streets its gate faces: none inside it.)
    let streets: Vec<(P, P)> = l
        .roads
        .iter()
        .flat_map(|(pts, _, _)| pts.windows(2).map(|s| (s[0], s[1])))
        .filter(|&(a, b)| inset_depth(&ring, lerp(a, b, 0.5)) <= 3.0)
        .collect();
    let street_dist = |p: P| streets.iter().map(|&(a, b)| seg_dist(p, a, b)).fold(f64::MAX, f64::min);
    // (Nor onto the water or the back of a building: ground open before it, a way's width 24 ft out.)
    let rn = ring.len();
    let reach = ring.iter().map(|p| dist(*p, centroid(&ring))).fold(0.0, f64::max) + 60.0;
    let before: Vec<Vec<P>> = l
        .buildings
        .iter()
        .filter(|b| b.poly.iter().any(|p| dist(*p, centroid(&ring)) < reach))
        .map(|b| b.poly.clone())
        .collect();
    let open_before = |p: P| {
        let Some(e) = (0..rn).min_by(|&a, &b| {
            seg_dist(p, ring[a], ring[(a + 1) % rn])
                .total_cmp(&seg_dist(p, ring[b], ring[(b + 1) % rn]))
                .then(a.cmp(&b))
        }) else {
            return true;
        };
        let u = mul(sub(ring[(e + 1) % rn], ring[e]), 1.0 / dist(ring[e], ring[(e + 1) % rn]).max(1e-9));
        let out = [u[1], -u[0]];
        [8.0, 16.0, 24.0].iter().all(|&d| {
            [-6.0, 0.0, 6.0].iter().all(|&s| {
                let q = add(add(p, mul(out, d)), mul(u, s));
                !site.wet(q) && before.iter().all(|b| super::poly_seg_dist(b, q, q) >= 3.0)
            })
        })
    };
    // (Not facing a town gate: the town's way in doesn't lead into the castle's.)
    let by_town_gate = |p: P| town_gates.iter().map(|g| GATE_AWAY - dist(p, *g)).fold(0.0, f64::max);
    let outer_score = |p: P| {
        let near = by_town_gate(p);
        let away = if near > 0.0 { 1_000.0 + 10.0 * near } else { 0.0 } + if open_before(p) { 0.0 } else { 2_000.0 };
        away + if streets.is_empty() {
            dist(p, gate_toward)
        } else {
            street_dist(p).max(15.0) + 0.2 * dist(p, gate_toward)
        }
    };
    let (t0, g0) = (l.towers.len(), l.gate_towers.len());
    // (A town's tower beside one of a castle it planned goes, below; else the castle's stands clear.)
    let gap = if set.planned { 4.0 } else { TOWERS_CLEAR };
    // A gate standing back from the streets (up the brow): a way from it to the nearest (`gate_way`);
    // where none can be laid, the gate elsewhere (three tries).
    let mut banned: Vec<P> = Vec::new();
    let (outer_gate, outer_in) = loop {
        let mark = (l.walls.len(), l.towers.len(), l.gate_towers.len(), l.square_towers.len(), others.len());
        let tried = |p: P| banned.iter().any(|b| dist(*b, p) < 60.0);
        let score = |p: P| outer_score(p) + if tried(p) { 5_000.0 } else { 0.0 };
        let (g, gin) = curtain(
            l,
            &ring,
            &walled,
            set.wall_up,
            &score,
            &|p| by_town_gate(p) <= 0.0 && open_before(p) && !tried(p),
            square,
            gap,
            &mut others,
            set.follow,
        );
        if streets.is_empty() || street_dist(g) <= 20.0 {
            break (g, gin);
        }
        match gate_way(site, l, &ring, &block, &streets, laid, g, gin) {
            Some(way) => {
                l.roads.push((way, 3, 10.0));
                break (g, gin);
            }
            None if banned.len() < 3 => {
                l.walls.truncate(mark.0);
                l.towers.truncate(mark.1);
                l.gate_towers.truncate(mark.2);
                l.square_towers.truncate(mark.3);
                others.truncate(mark.4);
                banned.push(g);
            }
            None => break (g, gin),
        }
    };
    let cc = centroid(&ring);
    l.castles.push((cc, ring.iter().map(|p| dist(*p, cc)).fold(0.0, f64::max)));
    // An inner curtain round the highest ground with room for it (on flat ground, the middle drawn
    // back from the gate), leaving a broad outer bailey before the gate.
    let inner = if want {
        // (On a hill, the highest spots with room, 40 ft apart at least; then, as on flat ground,
        // the middle drawn back from the gate, about halfway out.)
        let mut tries: Vec<(P, bool)> = Vec::new();
        if !flat {
            for (p, _) in survey.ranked(|p| inset_depth(&ring, p) >= R_MIN + BAILEY_GAP) {
                if tries.len() < 6 && tries.iter().all(|t| dist(t.0, p) >= 40.0) {
                    tries.push((p, false));
                }
            }
        }
        let away = sub(cc, outer_gate);
        let u = mul(away, 1.0 / len(away).max(1e-9));
        let reach = ray_exit(&ring, cc, u);
        // (A sized castle's (`inner` set) two more between.)
        let fs: &[f64] = if set.inner.is_some() { &[0.3, 0.2, 0.1, 0.0, 0.15, 0.05] } else { &[0.3, 0.2, 0.1, 0.0] };
        tries.extend(fs.iter().map(|f| (add(cc, mul(u, f * reach)), true)));
        // (A capital's where it fits at all: if no bailey broad across the gate's side leaves room,
        // one broad before the gate.)
        let cones: &[bool] = if set.capital { &[true, false] } else { &[true] };
        cones.iter().find_map(|&cone| {
            tries
                .iter()
                .find_map(|&(o, half)| inner_ring(site, &ring, o, if half { top } else { site.height(o) }, half, outer_gate, cone))
        })
    } else {
        None
    };
    let mut gates = vec![outer_gate];
    let mut lanes: Vec<(P, P)> = vec![(outer_gate, add(outer_gate, mul(outer_in, 30.0)))];
    if let Some(inner) = &inner {
        let walled_in = vec![false; inner.len()];
        let apart = |p: P| dist(p, outer_gate) >= GATES_APART;
        let (g, gin) = curtain(l, inner, &walled_in, true, &|p: P| dist(p, outer_gate), &apart, square, 4.0, &mut others, set.follow);
        gates.push(g);
        // The way from gate to gate along the outer bailey.
        let (a, b) = (add(outer_gate, mul(outer_in, 30.0)), sub(g, mul(gin, 30.0)));
        lanes.extend([(a, b), (b, g)]);
        let ci = centroid(inner);
        l.castles.push((ci, inner.iter().map(|p| dist(*p, ci)).fold(0.0, f64::max)));
    }
    // The inner curtain's towers (the ranges keep well clear of them: the outer bailey's way).
    let inner_towers: Vec<(P, f64)> = match &inner {
        Some(q) => l.towers[t0..]
            .iter()
            .map(|t| (*t, 11.0))
            .chain(l.gate_towers[g0..].iter().map(|t| (*t, 15.0)))
            .filter(|(t, _)| inset_depth(q, *t) > -2.0)
            .collect(),
        None => Vec::new(),
    };
    // The towers the keep keeps clear of: the castle's own, but those by a side along the town wall,
    // whose corners stand for them (a tower there whether the wall stands or not): the same keep
    // either way. (The ranges keep clear of every tower round the bailey: `by_ring`.)
    let rn = ring.len();
    let by_wall = |t: P| (0..rn).any(|e| walled[e] && seg_dist(t, ring[e], ring[(e + 1) % rn]) < 30.0);
    let towers: Vec<(P, f64)> = l.towers[t0..]
        .iter()
        .map(|t| (*t, 11.0))
        .chain(l.gate_towers[g0..].iter().map(|t| (*t, 15.0)))
        .filter(|(t, _)| !by_wall(*t))
        .chain((0..rn).filter(|&k| walled[k] != walled[(k + rn - 1) % rn]).map(|k| (ring[k], 11.0)))
        .collect();
    // The keep on the highest ground that holds it, inside the inner curtain if there is one (on flat
    // ground, about the middle).
    let court = inner.as_deref().unwrap_or(&ring);
    let axis = obb(&hull(court.to_vec())).axis;
    let c = centroid(court);
    let mut spots: Vec<(P, f64)> = survey.ranked(|p| inset_depth(court, p) >= KEEP_ROOM);
    if flat {
        spots.sort_by(|a, b| dist(a.0, c).total_cmp(&dist(b.0, c)));
        spots.insert(0, (c, 0.0));
    }
    spots.truncate(16);
    // (A small court: its middle.)
    spots.push((c, 0.0));
    let court_gate = *gates.last().unwrap_or(&outer_gate);
    let kept = if keep {
        place_keep(site, court, &spots, axis, square, &towers, court_gate, prng)
    } else {
        None
    };
    if yard {
        let clear: Vec<P> = kept.as_ref().map(|k| k.0.poly.clone()).unwrap_or_default();
        let motte = kept.as_ref().and_then(|k| k.1);
        // (Clear of every tower by the bailey: the castle's own, and the town's on the sides where
        // its wall is the castle's.)
        let by_ring: Vec<(P, f64)> = l.towers[..t0]
            .iter()
            .map(|t| (*t, 11.0))
            .chain(l.gate_towers[..g0].iter().map(|t| (*t, 15.0)))
            .chain(laid.1.iter().copied())
            .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { 11.0 })))
            .filter(|(t, _)| inset_depth(&ring, *t) > -40.0)
            .chain(l.towers[t0..].iter().map(|t| (*t, if l.square_tower(*t).is_some() { 15.6 } else { 11.0 })))
            .chain(l.gate_towers[g0..].iter().map(|t| (*t, 15.0)))
            .chain(towers.iter().copied())
            .collect();
        let from = l.buildings.len();
        // A great castle's hall range first: in the inner bailey along its curtain, else along the
        // outer.
        if set.hall {
            match &inner {
                Some(q) => {
                    let mut q = q.clone();
                    if area(&q) < 0.0 {
                        q.reverse();
                    }
                    let open = vec![false; q.len()];
                    ranges(site, &q, &open, None, &gates, &by_ring, &[], &lanes, &clear, motte, w, HALL, from, prng, l);
                }
                None => ranges(site, &ring, &walled, None, &gates, &by_ring, &[], &lanes, &clear, motte, w, HALL, from, prng, l),
            }
        }
        ranges(site, &ring, &walled, inner.as_deref(), &gates, &by_ring, &inner_towers, &lanes, &clear, motte, w, RANGE, from, prng, l);
        // (A town's castle sized to them, or one it planned on its whole patch: free-standing
        // buildings in the yard as well.)
        if set.follow && (set.inner.is_some() || set.planned) {
            l.castle_yard = yard_buildings(site, &ring, inner.as_deref(), &gates, &by_ring, &inner_towers, &lanes, &clear, motte, w, from, l);
        }
    }
    if let Some((mut b, motte)) = kept {
        b.ward = w;
        if let Some(m) = motte {
            l.mottes.push(m);
        }
        l.buildings.push(b);
    }
    // A town tower crowding a gatehouse or a tower of the castle's goes: not three towers in a row,
    // nor two side by side.
    if set.planned {
        let own: Vec<(P, f64)> = l.towers[t0..].iter().map(|t| (*t, if l.square_tower(*t).is_some() { 15.6 } else { 11.0 })).collect();
        let (towers, gates) = (&mut l.towers, &l.gate_towers[g0..]);
        let mut k = 0;
        towers.retain(|t| {
            k += 1;
            k > t0 || (gates.iter().all(|g| dist(*t, *g) >= 11.0 + 15.0 + TOWERS_CLEAR) && own.iter().all(|(o, r)| dist(*t, *o) >= 11.0 + r + TOWERS_CLEAR))
        });
    }
    // The bailey: an open yard of grass, built on only by hand.
    l.plazas.push(Plaza {
        pts: ring.clone(),
        kind: PlazaKind::Bailey,
    });
    Some(ring)
}

/// The side `a`-`b` of a castle runs along a town wall standing within `WALL_SIDE` of it, the whole
/// way (but through the passage of a gate at its end: `gates`, and the water gap where a stream
/// crosses it: `wet`).
fn along_wall(walls: &[(P, P)], gates: &[P], wet: &dyn Fn(P) -> bool, a: P, b: P) -> bool {
    let el = dist(a, b);
    if el < 1.0 || walls.is_empty() {
        return false;
    }
    let u = mul(sub(b, a), 1.0 / el);
    let ts = [0.05, 0.275, 0.5, 0.725, 0.95];
    let by_gate = |p: P| gates.iter().any(|g| dist(*g, p) < 45.0);
    let on = |p: P| {
        walls.iter().any(|&(q, r)| {
            let ql = dist(q, r);
            ql >= 1.0 && cross(u, mul(sub(r, q), 1.0 / ql)).abs() < 0.12 && seg_dist(p, q, r) <= WALL_SIDE
        })
    };
    !ts.iter().all(|&t| by_gate(lerp(a, b, t)))
        && ts.iter().any(|&t| on(lerp(a, b, t)))
        && ts.iter().all(|&t| {
            let p = lerp(a, b, t);
            // (A gap by the water with the wall on beyond it both ways.)
            by_gate(p) || on(p) || (wet(p) && on(sub(p, mul(u, 40.0))) && on(add(p, mul(u, 40.0))))
        })
}

/// The corners of `ring` (anticlockwise) sharper than `SPIKE_DEG` cut back where the ring is
/// `SPIKE_WIDE` across (to the next corner where that is near it), so long as what is left is a
/// simple ring holding most of it.
fn cut_spikes(ring: &[P]) -> Vec<P> {
    let mut pts = ring.to_vec();
    let mut tried: Vec<P> = Vec::new();
    for _ in 0..ring.len() {
        let m = pts.len();
        if m <= 3 {
            break;
        }
        let spike = (0..m).find(|&k| {
            let (a, b, c) = (pts[(k + m - 1) % m], pts[k], pts[(k + 1) % m]);
            let (u, v) = (sub(a, b), sub(c, b));
            let convex = cross(sub(b, a), sub(c, b)) > 0.0;
            convex && libm::atan2(cross(u, v).abs(), dot(u, v)) < SPIKE_DEG.to_radians() && !tried.iter().any(|t| dist(*t, b) < 1e-9)
        });
        let Some(k) = spike else { break };
        let (a, b, c) = (pts[(k + m - 1) % m], pts[k], pts[(k + 1) % m]);
        tried.push(b);
        let (u, v) = (sub(a, b), sub(c, b));
        let half = 0.5 * libm::atan2(cross(u, v).abs(), dot(u, v));
        let s = 0.5 * SPIKE_WIDE / libm::sin(half).max(1e-6);
        let (la, lc) = (len(u), len(v));
        // (Within 24 ft of the next corner: to that corner.)
        let at = |l: f64| if l - s < 24.0 { l } else { s };
        let (sa, sc) = (at(la), at(lc));
        let mut next: Vec<P> = Vec::new();
        for (j, p) in pts.iter().enumerate() {
            if j != k {
                next.push(*p);
                continue;
            }
            if sa < la {
                next.push(add(b, mul(u, sa / la)));
            }
            if sc < lc {
                next.push(add(b, mul(v, sc / lc)));
            }
        }
        if next.len() >= 3 && simple(&next) && area(&next) >= 0.6 * area(&pts) {
            pts = next;
        }
    }
    pts
}

/// A side of `ring` (anticlockwise) leaving a side along the town wall (`along`) at a slant, under
/// 35 degrees: their corner slid along the wall to where that side's far end stands square to it,
/// so the curtain meets the wall square, no wedge of ground left between it and the wall (where
/// the wall runs on so far, `room` holds there and the ring stays simple).
fn meet_wall(ring: &[P], along: &dyn Fn(P, P) -> bool, room: &dyn Fn(P) -> bool) -> Vec<P> {
    let mut pts = ring.to_vec();
    let m = pts.len();
    if m < 4 {
        return pts;
    }
    let unit = |a: P, b: P| mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
    for k in 0..m {
        let (a, v, c) = (pts[(k + m - 1) % m], pts[k], pts[(k + 1) % m]);
        let (wa, wc) = (along(a, v), along(v, c));
        if wa == wc {
            continue;
        }
        // (The wall's way along it, and the far end of the side leaving it.)
        let (u, far) = if wa { (unit(a, v), c) } else { (unit(c, v), a) };
        let leave = unit(v, far);
        if libm::atan2(cross(u, leave).abs(), dot(u, leave)) >= 35f64.to_radians() {
            continue;
        }
        let t = dot(sub(far, v), u);
        if t <= 1.0 {
            continue;
        }
        let to = add(v, mul(u, t));
        let mut next = pts.clone();
        next[k] = to;
        if room(to) && along(v, to) && simple(&next) && area(&next) >= area(&pts) {
            pts = next;
        }
    }
    pts
}

/// A road running through `ring` more than 15 ft (its middle more than 3 ft inside): the stretch of
/// it deepest inside and its width.
fn road_through(roads: &[(Vec<P>, u8, f64)], ring: &[P]) -> Option<(P, P, f64)> {
    let mut best: Option<(f64, P, P, f64)> = None;
    for (pts, _, w) in roads {
        let (mut inside, mut deep): (f64, Option<(f64, P, P)>) = (0.0, None);
        for s in pts.windows(2) {
            let n = crate::core::ceil(dist(s[0], s[1]) / 5.0).max(1.0) as usize;
            for j in 0..=n {
                let d = inset_depth(ring, lerp(s[0], s[1], j as f64 / n as f64));
                if d > 3.0 {
                    inside += 5.0;
                    if deep.is_none_or(|x| d > x.0) && dist(s[0], s[1]) > 1e-6 {
                        deep = Some((d, s[0], s[1]));
                    }
                }
            }
        }
        if let Some((d, a, b)) = deep.filter(|_| inside > 15.0)
            && best.is_none_or(|x| d > x.0)
        {
            best = Some((d, a, b, *w));
        }
    }
    best.map(|x| (x.1, x.2, x.3))
}

/// The segments `a`-`b` and `c`-`d` cross.
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (o3, o4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

/// Whether point `p` lies inside a plain polygon, and how far from its outline (ft; negative
/// outside).
fn inset_depth(poly: &[P], p: P) -> f64 {
    let m = poly.len();
    let d = (0..m).map(|k| seg_dist(p, poly[k], poly[(k + 1) % m])).fold(f64::MAX, f64::min);
    if contains(poly, p) { d } else { -d }
}

/// The castle's ground on a 10-ft lattice (local ft, whole multiples so a block moved a little
/// samples the same points): the dry points inside its outer bound and their heights.
struct Survey {
    i0: i64,
    j0: i64,
    nx: usize,
    ny: usize,
    h: Vec<Option<f64>>,
}

const SURVEY_FT: f64 = 10.0;

impl Survey {
    fn of<G: Ground>(site: &G, poly: &[P]) -> Survey {
        let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| {
            (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1]))
        });
        let f = |v: f64| crate::core::floor(v / SURVEY_FT) as i64;
        let (i0, j0, i1, j1) = (f(x0), f(y0), f(x1), f(y1));
        let (nx, ny) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
        let h = (0..nx * ny)
            .map(|k| {
                let p = [((i0 + (k % nx) as i64) as f64 + 0.5) * SURVEY_FT, ((j0 + (k / nx) as i64) as f64 + 0.5) * SURVEY_FT];
                (contains(poly, p) && !site.wet(p)).then(|| site.height(p))
            })
            .collect();
        Survey { i0, j0, nx, ny, h }
    }

    fn point(&self, k: usize) -> P {
        [
            ((self.i0 + (k % self.nx) as i64) as f64 + 0.5) * SURVEY_FT,
            ((self.j0 + (k / self.nx) as i64) as f64 + 0.5) * SURVEY_FT,
        ]
    }

    /// Highest and lowest dry ground minus each other.
    fn relief(&self) -> f64 {
        let (lo, hi) = self.h.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), &h| (a.min(h), b.max(h)));
        if lo <= hi { hi - lo } else { 0.0 }
    }

    /// A dry point's ground with its surroundings (20 ft round): their mean height.
    fn smoothed(&self, k: usize) -> Option<f64> {
        self.h[k]?;
        let (i, j) = ((k % self.nx) as i64, (k / self.nx) as i64);
        let (mut s, mut n) = (0.0, 0.0);
        for dj in -2..=2i64 {
            for di in -2..=2i64 {
                let (a, b) = (i + di, j + dj);
                if a >= 0 && b >= 0 && (a as usize) < self.nx && (b as usize) < self.ny && di * di + dj * dj <= 5 {
                    if let Some(h) = self.h[b as usize * self.nx + a as usize] {
                        s += h;
                        n += 1.0;
                    }
                }
            }
        }
        Some(s / n)
    }

    /// The dry points where `ok` holds, highest ground first (`smoothed`; equals row by row).
    fn ranked(&self, ok: impl Fn(P) -> bool) -> Vec<(P, f64)> {
        let mut v: Vec<(usize, P, f64)> = (0..self.nx * self.ny)
            .filter_map(|k| self.smoothed(k).map(|m| (k, self.point(k), m)))
            .filter(|x| ok(x.1))
            .collect();
        v.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        v.into_iter().map(|x| (x.1, x.2)).collect()
    }

    /// The top of the ground where `ok` holds: its point and height (`smoothed`).
    fn best(&self, ok: impl Fn(P) -> bool) -> Option<(P, f64)> {
        self.ranked(ok).into_iter().next()
    }
}

/// How far from `o` along unit `d` the ray first crosses the outline of `poly` (ft).
fn ray_exit(poly: &[P], o: P, d: P) -> f64 {
    ray_hit(poly, o, d).0
}

/// `ray_exit`, and the edge it crosses there.
fn ray_hit(poly: &[P], o: P, d: P) -> (f64, usize) {
    let m = poly.len();
    let (mut best, mut at) = (f64::MAX, 0);
    for k in 0..m {
        let (a, b) = (poly[k], poly[(k + 1) % m]);
        let e = sub(b, a);
        let den = cross(d, e);
        if den.abs() < 1e-12 {
            continue;
        }
        let ao = sub(a, o);
        let t = cross(ao, e) / den;
        let u = cross(ao, d) / den;
        if t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) && t < best {
            (best, at) = (t, k);
        }
    }
    if best == f64::MAX { (0.0, 0) } else { (best, at) }
}

/// Rays from `o`: evenly every 5 degrees, and at each corner of `poly` (so a ring keeping to it
/// keeps its corners), in turning order.
fn rays(poly: &[P], o: P) -> Vec<f64> {
    const N: usize = 72;
    let mut a: Vec<f64> = (0..N).map(|k| std::f64::consts::TAU * k as f64 / N as f64).collect();
    for p in poly {
        let d = sub(*p, o);
        if len(d) > 1e-6 {
            let t = libm::atan2(d[1], d[0]);
            a.push(if t < 0.0 { t + std::f64::consts::TAU } else { t });
        }
    }
    a.sort_by(f64::total_cmp);
    a.dedup_by(|x, y| (*x - *y).abs() < 1e-3);
    a
}

/// A ring round `o` with `r[i]` ft along each ray `a[i]`.
fn polar(o: P, a: &[f64], r: &[f64]) -> Vec<P> {
    a.iter().zip(r).map(|(&t, &r)| add(o, [r * libm::cos(t), r * libm::sin(t)])).collect()
}

/// `r` smoothed round the ring (a quarter each side, twice), each kept within `max`.
fn smooth(r: &[f64], max: &[f64]) -> Vec<f64> {
    let n = r.len();
    let mut cur = r.to_vec();
    for _ in 0..2 {
        cur = (0..n).map(|i| (0.25 * cur[(i + n - 1) % n] + 0.5 * cur[i] + 0.25 * cur[(i + 1) % n]).min(max[i])).collect();
    }
    cur
}

/// A curtain's ring made fit to build: corners off a straight run (within 4 ft) dropped, then
/// corners turning less than 12 degrees, and corners under 24 ft apart (32 where either turns
/// more than 60 degrees: a jog) merged; None if what is left isn't a simple polygon of three
/// corners or more.
fn tidy(ring: &[P], fixed: &[P]) -> Option<Vec<P>> {
    let mut pts = simplify_closed(ring, 4.0);
    // (A corner of a side along a town wall stays: the wall turns there.)
    let kept = |p: P| fixed.iter().any(|f| dist(*f, p) < 0.5);
    if !fixed.is_empty() {
        for f in fixed {
            if !pts.iter().any(|p| dist(*p, *f) < 0.5)
                && let Some(k) = ring.iter().position(|p| dist(*p, *f) < 0.5)
            {
                // (Put back where it stood in the ring.)
                let after = (0..k).rev().find_map(|j| pts.iter().position(|p| dist(*p, ring[j]) < 1e-9));
                pts.insert(after.map_or(0, |a| a + 1), ring[k]);
            }
        }
    }
    loop {
        let m = pts.len();
        if m <= 3 {
            break;
        }
        let turn = |k: usize| {
            let (a, b, c) = (pts[(k + m - 1) % m], pts[k], pts[(k + 1) % m]);
            let (u, v) = (sub(b, a), sub(c, b));
            libm::atan2(cross(u, v).abs(), dot(u, v))
        };
        // (A short jog: a run under 32 ft turning sharply at either end.)
        let short = (0..m).find(|&k| {
            let d = dist(pts[k], pts[(k + 1) % m]);
            (d < 24.0 || (d < 32.0 && turn(k).max(turn((k + 1) % m)) > 60f64.to_radians())) && !(kept(pts[k]) && kept(pts[(k + 1) % m]))
        });
        let flat = (0..m).find(|&k| turn(k) < 12f64.to_radians() && !kept(pts[k]));
        match (flat, short) {
            (Some(k), _) => {
                pts.remove(k);
            }
            (None, Some(k)) => {
                // (Of the two, the gentler turn goes; never a town wall's corner.)
                let j = (k + 1) % m;
                pts.remove(if kept(pts[j]) || (!kept(pts[k]) && turn(k) < turn(j)) { k } else { j });
            }
            (None, None) => break,
        }
    }
    (pts.len() >= 3 && simple(&pts) && area(&pts) > 0.0).then_some(pts)
}

/// Douglas-Peucker on a closed ring (within `tol` ft).
fn simplify_closed(ring: &[P], tol: f64) -> Vec<P> {
    let n = ring.len();
    if n <= 4 {
        return ring.to_vec();
    }
    let far = (1..n).max_by(|&a, &b| dist(ring[a], ring[0]).total_cmp(&dist(ring[b], ring[0]))).unwrap_or(n / 2);
    fn dp(pts: &[P], tol: f64, out: &mut Vec<P>) {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let (mut worst, mut at) = (0.0, 0);
        for (k, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
            let d = seg_dist(*p, a, b);
            if d > worst {
                worst = d;
                at = k;
            }
        }
        if worst > tol {
            dp(&pts[..=at], tol, out);
            dp(&pts[at..], tol, out);
        } else {
            out.push(a);
        }
    }
    let mut out = Vec::new();
    let first: Vec<P> = ring[..=far].to_vec();
    let mut second: Vec<P> = ring[far..].to_vec();
    second.push(ring[0]);
    dp(&first, tol, &mut out);
    dp(&second, tol, &mut out);
    out
}

/// No two edges of a ring cross.
fn simple(pts: &[P]) -> bool {
    let m = pts.len();
    for i in 0..m {
        let (a, b) = (pts[i], pts[(i + 1) % m]);
        for j in i + 2..m {
            if i == 0 && j == m - 1 {
                continue;
            }
            let (c, d) = (pts[j], pts[(j + 1) % m]);
            let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
            let (o3, o4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
            if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
                return false;
            }
        }
    }
    true
}

/// The curtain along the brow: round the high point `o` (its ground `top` ft), out along each ray
/// to where the ground first falls `drop` ft below the top (or the water), never past the block's
/// bound (`outer`) nor nearer the top than `R_MIN`; smoothed and tidied. The nearest the top that
/// holds at least `least` of the bound (half; a town's castle on a patch cut to its size, 80%), and
/// room for a keep and a yard, else None (the bound).
fn brow_ring<G: Ground>(site: &G, outer: &[P], walled: &[bool], o: P, top: f64, a_out: f64, least: f64) -> Option<Vec<P>> {
    let a = rays(outer, o);
    let hits: Vec<(f64, usize)> = a.iter().map(|&t| ray_hit(outer, o, [libm::cos(t), libm::sin(t)])).collect();
    let max: Vec<f64> = hits.iter().map(|h| h.0).collect();
    // (Out to a side along a town wall: that wall is the castle's there, its corners kept.)
    let n = outer.len();
    let fixed: Vec<P> = (0..n).filter(|&k| walled[k] || walled[(k + n - 1) % n]).map(|k| outer[k]).collect();
    let forced: Vec<bool> = hits
        .iter()
        .zip(&a)
        .map(|(h, &t)| walled.get(h.1).copied().unwrap_or(false) || fixed.iter().any(|f| dist(*f, add(o, mul([libm::cos(t), libm::sin(t)], h.0))) < 0.5))
        .collect();
    // Each ray's ground every 5 ft (none on the water).
    let profiles: Vec<Vec<Option<f64>>> = a
        .iter()
        .zip(&max)
        .map(|(&t, &m)| {
            let d = [libm::cos(t), libm::sin(t)];
            let n = crate::core::floor(m / 5.0) as usize;
            (1..=n)
                .map(|k| add(o, mul(d, k as f64 * 5.0)))
                .map(|p| (!site.near_water(p, WATER_BACK)).then(|| site.height(p)))
                .collect()
        })
        .collect();
    let want = (least * a_out).max((0.9 * a_out).min(14_000.0));
    for drop in DROPS {
        let r: Vec<f64> = profiles
            .iter()
            .zip(&max)
            .zip(&forced)
            .map(|((pr, &m), &forced)| {
                if forced {
                    return m;
                }
                let fall = pr.iter().position(|h| h.is_none_or(|h| h < top - drop));
                let r = fall.map_or(m, |k| k as f64 * 5.0 + 2.5);
                r.max(R_MIN.min(m)).min(m)
            })
            .collect();
        // (Along a town wall, on it: not smoothed.)
        let r: Vec<f64> = smooth(&r, &max).iter().zip(&max).zip(&forced).map(|((&s, &m), &f)| if f { m } else { s }).collect();
        let ring = polar(o, &a, &r);
        if let Some(ring) = tidy(&ring, &fixed)
            && area(&ring) >= want
            && contains(&ring, o)
        {
            return Some(ring);
        }
    }
    None
}

//// An inner curtain round `o` in a big castle: along the brow of a higher drop (on flat ground
/// about halfway out), at least `BAILEY_GAP` inside the outer `ring` all round and `BROAD` on the
/// side of its gate (`gate`), so the outer bailey is a yard to use there; None if that doesn't fit,
/// or what is left is too small for a keep and a yard or too big to leave an outer bailey.
fn inner_ring<G: Ground>(site: &G, ring: &[P], o: P, top: f64, flat: bool, gate: P, cone: bool) -> Option<Vec<P>> {
    let a = rays(ring, o);
    let to_gate = sub(gate, o);
    let g = mul(to_gate, 1.0 / len(to_gate).max(1e-9));
    let exit: Vec<f64> = a.iter().map(|&t| ray_exit(ring, o, [libm::cos(t), libm::sin(t)])).collect();
    // (A little more than each: the ring's sides between its rays come nearer.)
    const MORE: f64 = 6.0;
    let max: Vec<f64> = a
        .iter()
        .zip(&exit)
        .map(|(&t, &e)| {
            let d = [libm::cos(t), libm::sin(t)];
            // (With `cone`, the outer bailey broad all across the gate's side, not only before it.)
            let facing = cone && dot(d, g) > 0.64;
            let by_wall = e - if facing { BROAD } else { BAILEY_GAP } - MORE;
            // Out to where it comes `BROAD` from the gate.
            let (along, off) = (dot(d, to_gate), cross(d, to_gate).abs());
            let reach = BROAD + MORE;
            let by_gate = if along > 0.0 && off < reach {
                along - crate::core::sqrt(reach * reach - off * off)
            } else {
                f64::MAX
            };
            by_wall.min(by_gate)
        })
        .collect();
    if max.iter().any(|&m| m < R_MIN) {
        return None;
    }
    let r: Vec<f64> = a
        .iter()
        .zip(&max)
        .zip(&exit)
        .map(|((&t, &m), &e)| {
            let d = [libm::cos(t), libm::sin(t)];
            let half = 0.55 * e;
            if flat {
                return half.clamp(R_MIN, m);
            }
            let n = crate::core::floor(m / 5.0) as usize;
            let fall = (1..=n).find(|&k| {
                let p = add(o, mul(d, k as f64 * 5.0));
                site.wet(p) || site.height(p) < top - 5.0
            });
            fall.map_or(half, |k| (k as f64 * 5.0 + 2.5).min(half)).clamp(R_MIN, m)
        })
        .collect();
    let inner = tidy(&polar(o, &a, &smooth(&r, &max)), &[])?;
    let (ai, ao) = (area(&inner), area(ring));
    // Every stretch of it (not only its corners) the outer bailey's width from the outer curtain
    // and `BROAD` from the outer gate, and broad somewhere.
    let n = inner.len();
    let mut broad = 0.0;
    for k in 0..n {
        let (p, q) = (inner[k], inner[(k + 1) % n]);
        let steps = crate::core::ceil(dist(p, q) / 5.0).max(1.0) as usize;
        for j in 0..steps {
            let at = lerp(p, q, j as f64 / steps as f64);
            let d = inset_depth(ring, at);
            if d < BAILEY_GAP || dist(at, gate) < BROAD {
                return None;
            }
            if d >= BROAD - 10.0 {
                broad += dist(p, q) / steps as f64;
            }
        }
    }
    (ai >= 7_000.0 && ai <= 0.55 * ao && broad >= 60.0 && contains(&inner, o)).then_some(inner)
}

/// A curtain on `ring` (local ft) with its gatehouse at the place on a side with room for it (its
/// towers clear of the corners) where `ok` holds that `score` likes best (lowest), and towers on
/// every other corner and along long runs (square in a castle built so), each clear of the towers
/// already standing (`others`, which gets them: `TOWERS_CLEAR` from a gate tower, `gap` from any
/// other). The sides `walled` (along a town wall) get no
/// curtain while that wall stands (`up`): the rest runs from that wall round to it again; no gate
/// is on one either way (a stub left by a gap in it dropped where `stubs`). Returns the gate and the
/// way in there (unit).
#[allow(clippy::too_many_arguments)]
fn curtain(l: &mut Layout, ring: &[P], walled: &[bool], up: bool, score: &dyn Fn(P) -> f64, ok: &dyn Fn(P) -> bool, square: bool, gap: f64, others: &mut Vec<(P, f64)>, stubs: bool) -> (P, P) {
    let n = ring.len();
    let edge = |e: usize| (ring[e], ring[(e + 1) % n]);
    // (A town wall all round: none is.)
    let walled: Vec<bool> = if walled.iter().all(|&w| w) { vec![false; n] } else { walled.to_vec() };
    // (Its towers clear of the corners' towers: 30 ft from the passage to a corner.)
    let room = 2.0 * (GATE_HALF + 30.0);
    let mut spots: Vec<(usize, f64, P)> = Vec::new();
    for e in (0..n).filter(|&e| !walled[e]) {
        let (a, b) = edge(e);
        let el = dist(a, b);
        if el < room {
            continue;
        }
        let k = crate::core::floor((el - room) / 5.0) as usize;
        for j in 0..=k {
            let t = (GATE_HALF + 30.0 + 5.0 * j as f64) / el;
            spots.push((e, t, lerp(a, b, t)));
        }
    }
    // (Its towers clear of the towers already standing, a town wall's among them: not one beside
    // another.)
    let clear = |s: &(usize, f64, P)| {
        let (a, b) = edge(s.0);
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        [-GATE_HALF, GATE_HALF].iter().all(|&o| {
            others
                .iter()
                .all(|(q, r)| dist(add(s.2, mul(u, o)), *q) >= GATE_CLEAR.max(r + 15.0 + if *r >= 15.0 { TOWERS_CLEAR } else { gap }))
        })
    };
    let best = |spots: &[(usize, f64, P)], need: bool, apart: bool| {
        spots
            .iter()
            .filter(|s| (!need || ok(s.2)) && (!apart || clear(s)))
            .min_by(|x, y| score(x.2).total_cmp(&score(y.2)).then(x.0.cmp(&y.0)).then(x.1.total_cmp(&y.1)))
            .copied()
    };
    // (Where none of those will do, the middle of a side too short for corner towers clear of it, as
    // a small castle's: theirs go.)
    let short: Vec<(usize, f64, P)> = (0..n)
        .filter(|&e| !walled[e] && (2.0 * GATE_HALF + 20.0..room).contains(&dist(edge(e).0, edge(e).1)))
        .map(|e| (e, 0.5, lerp(edge(e).0, edge(e).1, 0.5)))
        .collect();
    let (gate_e, t) = match best(&spots, true, true)
        .or_else(|| best(&spots, true, false))
        .or_else(|| best(&short, true, true))
        .or_else(|| best(&short, true, false))
        .or_else(|| best(&spots, false, false))
    {
        Some(s) => (s.0, s.1),
        None => {
            let e = (0..n)
                .filter(|&e| !walled[e])
                .max_by(|&x, &y| dist(edge(x).0, edge(x).1).total_cmp(&dist(edge(y).0, edge(y).1)).then(y.cmp(&x)))
                .unwrap_or(0);
            (e, 0.5)
        }
    };
    let (a, b) = edge(gate_e);
    let gate = lerp(a, b, t);
    let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
    // (The ring runs anticlockwise: in is to the left.)
    let inward = [-u[1], u[0]];
    // (With the town wall taken down, its sides get a curtain too; the gate stays where it was.)
    let walled: Vec<bool> = if up { walled } else { vec![false; n] };
    // The lines the curtain runs along: all round through the gate, or from the town wall round to
    // it again along each run of sides off it.
    let mut lines: Vec<(Vec<(P, bool)>, bool)> = Vec::new();
    if walled.iter().all(|&w| !w) {
        let mut line: Vec<(P, bool)> = vec![(gate, true)];
        for k in 1..=n {
            line.push((ring[(gate_e + k) % n], false));
        }
        line.push((gate, true));
        lines.push((line, false));
    } else {
        let start = (0..n).find(|&e| walled[e] && !walled[(e + 1) % n]).unwrap_or(0);
        let mut e = (start + 1) % n;
        let mut line: Vec<(P, bool)> = Vec::new();
        for _ in 0..n {
            if walled[e] {
                if !line.is_empty() {
                    line.push((ring[e], false));
                    lines.push((std::mem::take(&mut line), true));
                }
            } else {
                line.push((ring[e], false));
                if e == gate_e {
                    line.push((gate, true));
                }
            }
            e = (e + 1) % n;
        }
        if !line.is_empty() {
            line.push((ring[e], false));
            lines.push((line, true));
        }
    }
    let mut pieces = Vec::new();
    let mut towers = Vec::new();
    let mut gate_towers = Vec::new();
    // (A town's castle (`stubs`): not a stub under 30 ft from the town wall ending in the open, as by a
    // water gap in it.)
    let standing: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let open_end = |p: P| !standing.iter().any(|&(a, b)| seg_dist(p, a, b) < 12.0);
    for (line, ends) in lines {
        let span: f64 = line.windows(2).map(|s| dist(s[0].0, s[1].0)).sum();
        if stubs && ends && span < 30.0 && !line.iter().any(|q| q.1) && line.len() >= 2 && (open_end(line[0].0) || open_end(line[line.len() - 1].0)) {
            continue;
        }
        let (mut p, t, g) = wall_pieces(&line, false);
        // (A line from the town wall: its ends carried on to that wall's line, its towers where
        // they stand on the ring, as with no town wall there.)
        if ends && line.len() >= 2 {
            let m = line.len();
            if let Some(first) = p.first_mut().filter(|w| w.len() >= 2 && dist(w[0], line[0].0) < 1e-9) {
                first[0] = to_wall(l, line[0].0, line[1].0);
            }
            if let Some(last) = p.last_mut().filter(|w| w.len() >= 2 && dist(w[w.len() - 1], line[m - 1].0) < 1e-9) {
                let k = last.len() - 1;
                last[k] = to_wall(l, line[m - 1].0, line[m - 2].0);
            }
        }
        pieces.extend(p);
        towers.extend(t);
        gate_towers.extend(g);
    }
    // Towers clear of the gatehouse, of each other and of every tower already standing (a square
    // one reaches further across its corners).
    let r = 11.0;
    let reach = if square { 15.6 } else { r };
    let mut kept: Vec<P> = Vec::new();
    // (In an order of their own, not the line's: the same towers whether a town wall's side has a
    // curtain or not.)
    towers.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    for t in towers {
        let clear = gate_towers.iter().all(|g| dist(t, *g) >= reach + 15.0 + 4.0)
            && kept.iter().all(|k| dist(t, *k) >= 2.0 * reach + 2.0)
            && others.iter().all(|(o, ro)| dist(t, *o) >= reach + ro + if *ro >= 15.0 { TOWERS_CLEAR } else { gap });
        if clear {
            kept.push(t);
        }
    }
    if square {
        for t in &kept {
            // Its sides along the longest stretch of wall it stands on.
            let along = pieces
                .iter()
                .flat_map(|w| w.windows(2))
                .filter(|s| seg_dist(*t, s[0], s[1]) < 1.0)
                .max_by(|x, y| dist(x[0], x[1]).total_cmp(&dist(y[0], y[1])))
                .map(|s| mul(sub(s[1], s[0]), 1.0 / dist(s[0], s[1]).max(1e-9)))
                .unwrap_or([1.0, 0.0]);
            l.square_towers.push((*t, along));
        }
    }
    others.extend(kept.iter().map(|t| (*t, reach)));
    others.extend(gate_towers.iter().map(|t| (*t, 15.0)));
    l.walls.extend(pieces);
    l.towers.extend(kept);
    l.gate_towers.extend(gate_towers);
    (gate, inward)
}

/// A way from a castle's gate (`gate`, `inward` the way in) to the nearest of `streets` (outside its
/// `ring`): straight where that is clear of the towers, walls (the town's as laid, `laid`, standing
/// or not) and buildings, else out from the gate first, else round what stands in the way
/// (`way_round`).
#[allow(clippy::too_many_arguments)]
fn gate_way<G: Ground>(site: &G, l: &Layout, ring: &[P], block: &[P], streets: &[(P, P)], laid: (&[(P, P)], &[(P, f64)]), gate: P, inward: P) -> Option<Vec<P>> {
    let from = sub(gate, mul(inward, 6.0));
    let nearest = |from: P| {
        let mut near: Vec<P> = streets
            .iter()
            .map(|&(a, b)| {
                let e = sub(b, a);
                let t = (dot(sub(from, a), e) / dot(e, e).max(1e-9)).clamp(0.0, 1.0);
                add(a, mul(e, t))
            })
            .collect();
        near.sort_by(|x, y| dist(*x, from).total_cmp(&dist(*y, from)).then(x[0].total_cmp(&y[0])).then(x[1].total_cmp(&y[1])));
        near.truncate(40);
        near
    };
    let towers: Vec<(P, f64)> = l
        .towers
        .iter()
        .map(|t| (*t, 11.0 * if l.square_tower(*t).is_some() { std::f64::consts::SQRT_2 } else { 1.0 }))
        .chain(l.gate_towers.iter().map(|t| (*t, 15.0)))
        .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { 11.0 })))
        .chain(laid.1.iter().copied())
        .collect();
    // (Clear of the buildings standing too.)
    let reach = ring.iter().map(|p| dist(*p, from)).fold(0.0, f64::max) + 260.0;
    let near: Vec<&Building> = l.buildings.iter().filter(|b| b.poly.iter().any(|p| dist(*p, from) < reach)).collect();
    let clear = |p: P, q: P| {
        towers.iter().all(|&(t, r)| seg_dist(t, p, q) >= r + 6.0)
            && l.walls
                .iter()
                .flat_map(|w| w.windows(2))
                .all(|s| !crosses(p, q, s[0], s[1]) && seg_dist(q, s[0], s[1]) >= 6.0 && seg_dist(p, s[0], s[1]) >= 5.0)
            && near.iter().all(|b| super::poly_seg_dist(&b.poly, p, q) >= 6.0)
            && laid.0.iter().all(|&(a, b)| !crosses(p, q, a, b) && seg_dist(q, a, b) >= 6.0 && seg_dist(p, a, b) >= 5.0)
    };
    let way = nearest(from)
        .into_iter()
        .find(|q| dist(*q, from) <= 160.0 && clear(from, *q))
        .map(|q| vec![from, q])
        .or_else(|| {
            [15.0, 25.0, 35.0, 50.0].iter().find_map(|&out| {
                let mid = sub(from, mul(inward, out));
                (inset_depth(ring, mid) < -5.0 && clear(from, mid))
                    .then(|| nearest(mid).into_iter().find(|q| dist(*q, mid) <= 120.0 && clear(mid, *q)).map(|q| vec![from, mid, q]))
                    .flatten()
            })
        })
        // (Else round what stands in the way: `way_round`.)
        .or_else(|| {
            let walls: Vec<Vec<P>> = l.walls.iter().cloned().chain(laid.0.iter().map(|&(a, b)| vec![a, b])).collect();
            way_round(site, ring, block, from, streets, &towers, &walls, &near, &clear)
        });
    way
}

/// A way from a gate (`from`, just outside it) to the nearest of `streets` round what stands in the
/// way: on a 5-ft grid (whole multiples of local ft), off the castle's ring, the water, and 6 ft
/// off walls, towers (`towers`: centre and reach) and buildings, keeping to the castle's own ground
/// round its ring (`block` and the streets round it) where it can; straightened where `clear`
/// allows. None if no street is in reach.
#[allow(clippy::too_many_arguments)]
fn way_round<G: Ground>(
    site: &G,
    ring: &[P],
    block: &[P],
    from: P,
    streets: &[(P, P)],
    towers: &[(P, f64)],
    walls: &[Vec<P>],
    near: &[&Building],
    clear: &dyn Fn(P, P) -> bool,
) -> Option<Vec<P>> {
    const S: f64 = 5.0;
    const REACH: f64 = 240.0;
    let (x0, y0, x1, y1) = ring.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| {
        (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1]))
    });
    let (i0, j0) = (crate::core::floor((x0 - REACH) / S) as i64, crate::core::floor((y0 - REACH) / S) as i64);
    let (nx, ny) = (
        (crate::core::ceil((x1 + REACH) / S) as i64 - i0 + 1) as usize,
        (crate::core::ceil((y1 + REACH) / S) as i64 - j0 + 1) as usize,
    );
    let centre = |k: usize| [((i0 + (k % nx) as i64) as f64 + 0.5) * S, ((j0 + (k / nx) as i64) as f64 + 0.5) * S];
    let cell = |p: P| {
        let (i, j) = (crate::core::floor(p[0] / S) as i64 - i0, crate::core::floor(p[1] / S) as i64 - j0);
        (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    // (Over the cells round a box: `f` for each.)
    let each = |lo: P, hi: P, f: &mut dyn FnMut(usize)| {
        let (a, b) = (crate::core::floor(lo[0] / S) as i64 - i0, crate::core::floor(lo[1] / S) as i64 - j0);
        let (c, d) = (crate::core::floor(hi[0] / S) as i64 - i0, crate::core::floor(hi[1] / S) as i64 - j0);
        for j in b.max(0)..=d.min(ny as i64 - 1) {
            for i in a.max(0)..=c.min(nx as i64 - 1) {
                f(j as usize * nx + i as usize);
            }
        }
    };
    // 1: too near something (6 ft); 2: in it; 4: a street; 8: too near a wall or a tower.
    let mut grid = vec![0u8; nx * ny];
    for (k, g) in grid.iter_mut().enumerate() {
        let p = centre(k);
        if inset_depth(ring, p) > -4.0 || site.wet(p) {
            *g = 3;
        }
    }
    for w in walls {
        for s in w.windows(2) {
            let (lo, hi) = (
                [s[0][0].min(s[1][0]) - 6.0, s[0][1].min(s[1][1]) - 6.0],
                [s[0][0].max(s[1][0]) + 6.0, s[0][1].max(s[1][1]) + 6.0],
            );
            each(lo, hi, &mut |k| {
                let d = seg_dist(centre(k), s[0], s[1]);
                grid[k] |= if d < 3.0 {
                    3
                } else if d < 6.0 {
                    9
                } else {
                    0
                };
            });
        }
    }
    for &(t, r) in towers {
        each([t[0] - r - 6.0, t[1] - r - 6.0], [t[0] + r + 6.0, t[1] + r + 6.0], &mut |k| {
            let d = dist(centre(k), t);
            grid[k] |= if d < r {
                3
            } else if d < r + 6.0 {
                9
            } else {
                0
            };
        });
    }
    for b in near {
        let (lo, hi) = b.poly.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |a, p| {
            ([a.0[0].min(p[0]), a.0[1].min(p[1])], [a.1[0].max(p[0]), a.1[1].max(p[1])])
        });
        each([lo[0] - 6.0, lo[1] - 6.0], [hi[0] + 6.0, hi[1] + 6.0], &mut |k| {
            let p = centre(k);
            let d = super::poly_seg_dist(&b.poly, p, p);
            grid[k] |= if d <= 0.0 {
                3
            } else if d < 6.0 {
                1
            } else {
                0
            };
        });
    }
    for &(a, b) in streets {
        let (lo, hi) = ([a[0].min(b[0]) - 4.0, a[1].min(b[1]) - 4.0], [a[0].max(b[0]) + 4.0, a[1].max(b[1]) + 4.0]);
        each(lo, hi, &mut |k| {
            if seg_dist(centre(k), a, b) <= 4.0 {
                grid[k] |= 4;
            }
        });
    }
    let start = cell(from)?;
    // Least cost out (10 a step, 14 across; three times that off the castle's own ground).
    let own: Vec<bool> = (0..nx * ny).map(|k| inset_depth(block, centre(k)) > -35.0).collect();
    let mut cost = vec![u32::MAX; nx * ny];
    let mut back = vec![usize::MAX; nx * ny];
    let mut heap = std::collections::BinaryHeap::new();
    cost[start] = 0;
    heap.push(std::cmp::Reverse((0u32, start)));
    let mut end = None;
    while let Some(std::cmp::Reverse((c, k))) = heap.pop() {
        if c > cost[k] {
            continue;
        }
        if k != start && grid[k] & 4 != 0 {
            end = Some(k);
            break;
        }
        let (i, j) = ((k % nx) as i64, (k / nx) as i64);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (a, b) = (i + di, j + dj);
            if a < 0 || b < 0 || a >= nx as i64 || b >= ny as i64 {
                continue;
            }
            let n = b as usize * nx + a as usize;
            // (Onto a street, anything not in a building nor near a wall or a tower.)
            let open = if grid[n] & 4 != 0 { grid[n] & 10 == 0 } else { grid[n] & 3 == 0 };
            if !open {
                continue;
            }
            let step = if di != 0 && dj != 0 { 14 } else { 10 } * if own[n] { 1 } else { 3 };
            let nc = c + step;
            if nc < cost[n] && nc <= 6_000 {
                cost[n] = nc;
                back[n] = k;
                heap.push(std::cmp::Reverse((nc, n)));
            }
        }
    }
    let end = end?;
    let mut path = vec![centre(end)];
    let mut k = end;
    while back[k] != start && back[k] != usize::MAX {
        k = back[k];
        path.push(centre(k));
    }
    path.push(from);
    path.reverse();
    // Straightened: from each point on to the farthest it reaches clear.
    let mut way = vec![from];
    let mut i = 0;
    while i + 1 < path.len() {
        let j = (i + 2..path.len()).rev().find(|&j| clear(path[i], path[j])).unwrap_or(i + 1);
        way.push(path[j]);
        i = j;
    }
    (way.len() >= 2).then_some(way)
}

/// Where a curtain from `p` (coming from `from`) meets the town wall nearest it, carried on along
/// its line (within 20 ft); else `p`.
fn to_wall(l: &Layout, p: P, from: P) -> P {
    let d = sub(p, from);
    let dl = len(d);
    if dl < 1e-9 {
        return p;
    }
    let d = mul(d, 1.0 / dl);
    let mut best: Option<(f64, P)> = None;
    for w in &l.walls {
        for s in w.windows(2) {
            let e = sub(s[1], s[0]);
            let den = cross(d, e);
            if den.abs() < 1e-9 {
                continue;
            }
            let ao = sub(s[0], p);
            let t = cross(ao, e) / den;
            let u = cross(ao, d) / den;
            if (0.0..=20.0).contains(&t) && (-0.05..=1.05).contains(&u) && best.is_none_or(|b| t < b.0) {
                best = Some((t, add(p, mul(d, t))));
            }
        }
    }
    best.map_or(p, |b| b.1)
}

/// The kinds of keep (`Building::keep`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeepKind {
    /// A great tower, nearly square, with a tower on each corner.
    Great,
    /// A long hall with a forebuilding before its door and towers at its far end.
    Hall,
    /// A block with a wing at one end, towers at its outer corners.
    L,
    /// A block with a wing from the middle of a long side.
    T,
    /// A round shell keep on a motte.
    Shell,
    /// A small tower keep with no towers of its own (a court too small for any other).
    Plain,
}

/// A keep's pieces in its own frame (ft, `s` along its axis, `t` across, its body about the
/// middle), each convex and wound anticlockwise; towers last.
struct Frame {
    parts: Vec<(Vec<P>, PartKind)>,
}

impl Frame {
    fn rect(&mut self, s0: f64, s1: f64, t0: f64, t1: f64, kind: PartKind) {
        self.parts.push((vec![[s0, t0], [s1, t0], [s1, t1], [s0, t1]], kind));
    }

    /// A tower `d` squares across on the corner (`s`, `t`) of a block, standing out from it along
    /// `ds`, `dt` (each 1 or -1): its middle on the corner (a whole number of squares across) or
    /// half a square out (an odd number), so its box lies on the block's squares.
    fn tower(&mut self, s: f64, t: f64, ds: f64, dt: f64, d: u32, round: bool) {
        let r = 2.5 * d as f64;
        let off = if d % 2 == 0 { 0.0 } else { 2.5 };
        let c = [s + ds * off, t + dt * off];
        let poly = if round {
            (0..16)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 16.0;
                    [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)]
                })
                .collect()
        } else {
            vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] - r], [c[0] + r, c[1] + r], [c[0] - r, c[1] + r]]
        };
        self.parts.push((poly, PartKind::Tower { round }));
    }
}

/// Feet rounded to whole squares (5 ft), at least `min`.
fn sq(v: f64, min: f64) -> f64 {
    (crate::core::round(v / 5.0) * 5.0).max(min)
}

/// The pieces of a keep of `kind` scaled by `f` (its rolls in `rolls`, 0..1 each): its frame.
fn keep_frame(kind: KeepKind, f: f64, rolls: &[f64; 6], round: bool) -> Frame {
    let mut fr = Frame { parts: Vec::new() };
    let sgn = |r: f64| if r < 0.5 { -1.0 } else { 1.0 };
    match kind {
        KeepKind::Great => {
            let l = sq(f * (50.0 + 25.0 * rolls[0]), 35.0);
            let w = sq(l * (0.8 + 0.2 * rolls[1]), 30.0);
            let d = if w < 45.0 {
                4
            } else if w < 60.0 {
                5
            } else {
                6
            };
            let (hl, hw) = (0.5 * l, 0.5 * w);
            fr.rect(-hl, hl, -hw, hw, PartKind::Body);
            for (ds, dt) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                fr.tower(ds * hl, dt * hw, ds, dt, d, round);
            }
        }
        KeepKind::Hall => {
            let l = sq(f * (75.0 + 35.0 * rolls[0]), 50.0);
            let w = sq(f * (35.0 + 12.0 * rolls[1]), 30.0);
            let (hl, hw) = (0.5 * l, 0.5 * w);
            fr.rect(-hl, hl, -hw, hw, PartKind::Body);
            // The forebuilding before one end of a long side, the towers at the other end.
            let (end, side) = (sgn(rolls[2]), sgn(rolls[3]));
            let fw = sq(f * (20.0 + 10.0 * rolls[4]), 15.0);
            let fd = sq(f * (15.0 + 10.0 * rolls[5]), 10.0);
            let s0 = if end > 0.0 { hl - 10.0 - fw } else { -hl + 10.0 };
            let (t0, t1) = if side > 0.0 { (hw - 5.0, hw + fd) } else { (-hw - fd, -hw + 5.0) };
            fr.rect(s0, s0 + fw, t0, t1, PartKind::Forebuilding);
            let d = if w < 40.0 { 4 } else { 5 };
            for dt in [-1.0, 1.0] {
                fr.tower(-end * hl, dt * hw, -end, dt, d, round);
            }
        }
        KeepKind::L | KeepKind::T => {
            let l = sq(f * (55.0 + 30.0 * rolls[0]), 40.0);
            let w = sq(f * (30.0 + 12.0 * rolls[1]), 30.0);
            let (hl, hw) = (0.5 * l, 0.5 * w);
            fr.rect(-hl, hl, -hw, hw, PartKind::Body);
            let side = sgn(rolls[3]);
            let wl = sq(f * (20.0 + 20.0 * rolls[4]), 15.0);
            let d = if w < 40.0 { 4 } else { 5 };
            let (t0, t1, tip) = if side > 0.0 { (hw - 5.0, hw + wl, hw + wl) } else { (-hw - wl, -hw + 5.0, -hw - wl) };
            if kind == KeepKind::L {
                // The wing at one end, its outer side on with the block's end.
                let end = sgn(rolls[2]);
                let ww = sq(w * (0.7 + 0.3 * rolls[5]), 20.0).min(l - 15.0);
                let (s0, s1) = if end > 0.0 { (hl - ww, hl) } else { (-hl, -hl + ww) };
                fr.rect(s0, s1, t0, t1, PartKind::Wing);
                fr.tower(end * hl, tip, end, side, d, round);
                fr.tower(-end * hl, -side * hw, -end, -side, d, round);
                fr.tower(end * hl, -side * hw, end, -side, d, round);
            } else {
                // The wing from the middle of a long side, on the block's squares.
                let ww = sq(w * (0.6 + 0.3 * rolls[5]), 15.0).min(l - 20.0);
                let s0 = -hl + crate::core::round((hl - 0.5 * ww) / 5.0) * 5.0;
                fr.rect(s0, s0 + ww, t0, t1, PartKind::Wing);
                fr.tower(s0, tip, -1.0, side, d, round);
                fr.tower(s0 + ww, tip, 1.0, side, d, round);
                fr.tower(-hl, -side * hw, -1.0, -side, d, round);
                fr.tower(hl, -side * hw, 1.0, -side, d, round);
            }
        }
        KeepKind::Plain => {
            let l = sq(f * (40.0 + 10.0 * rolls[0]), 20.0);
            let w = sq(l * (0.75 + 0.25 * rolls[1]), 20.0);
            fr.rect(-0.5 * l, 0.5 * l, -0.5 * w, 0.5 * w, PartKind::Body);
        }
        KeepKind::Shell => {
            let r = 2.5 * crate::core::round(f * (12.0 + 5.0 * rolls[0])).max(10.0);
            let ring: Vec<P> = (0..20)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 20.0;
                    [r * libm::cos(a), r * libm::sin(a)]
                })
                .collect();
            fr.parts.push((ring, PartKind::Body));
        }
    }
    fr
}

/// A keep's flat top this big (sq ft) or more is roofed.
const BIG_TOP: f64 = 1_100.0;

/// How each of a keep's pieces is topped, as the battlemap and tiles draw it (`seed`: the world's
/// stream for it): 0 open battlements (a wall walk of stone flags in its merlons, a broad one's
/// middle flat dark lead), 1 a hipped roof of lead, 2 of slate. A wing's top, and the donjon's at
/// one end of a long body (a squarer body is all hall roofs: 2); a big top (`BIG_TOP`) is roofed,
/// a smaller one open but in some keeps (rolled per keep), lead or slate rolled per keep too; a
/// long body's code has `DONJON_LOW` set when its donjon stands at the end nearer its first corner
/// (along its longer side from there, as the battlemap measures it), its hall range under hipped
/// roofs inside a wall walk either way. A forebuilding's top is a walled platform: 0. Towers: 2
/// (their cones and pyramids).
pub fn keep_tops(b: &Building, seed: u64) -> Vec<u8> {
    let c = centroid(&b.poly);
    let h = crate::core::rng::hash2(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64);
    let roofed = crate::core::rng::unit(h) < 0.35;
    let roof = if crate::core::rng::unit(crate::core::rng::hash2(h, 1, 2)) < 0.5 { 1 } else { 2 };
    b.parts
        .iter()
        .map(|p| {
            let top = match p.kind {
                PartKind::Tower { .. } => return 2,
                PartKind::Body if p.poly.len() == 4 => {
                    // (The donjon as drawn: a square at one end of a long body.)
                    let (a, w) = (dist(p.poly[0], p.poly[1]), dist(p.poly[1], p.poly[2]));
                    let (l, w) = (a.max(w), a.min(w));
                    if l <= 1.35 * w {
                        return 2;
                    }
                    let low = if crate::core::rng::unit(crate::core::rng::hash2(h, 3, 4)) < 0.5 { DONJON_LOW } else { 0 };
                    return (if w * w.min(0.42 * l) >= BIG_TOP || roofed { roof } else { 0 }) | low;
                }
                PartKind::Body => return 2,
                PartKind::Forebuilding => return 0,
                _ => area(&p.poly).abs(),
            };
            if top >= BIG_TOP || roofed { roof } else { 0 }
        })
        .collect()
}

/// Set in a long keep body's top code (`keep_tops`): its donjon stands at its first corner's end.
pub const DONJON_LOW: u8 = 4;

/// How a castle's keep with no pieces (reshaped or drawn by hand) is topped, as `keep_tops` tops a
/// body: its donjon (a square at one end of a long keep, as the battlemap draws it), else the whole
/// top if it has four corners; 0 for any other building.
pub fn plain_keep_top(b: &Building, seed: u64) -> u8 {
    let castle = b.ward == Ward::Castle || b.func.is_some_and(|f| super::catalog::CATALOG[f as usize].key == "castle");
    if !b.parts.is_empty() || b.poly.len() != 4 || !castle {
        return 0;
    }
    let c = centroid(&b.poly);
    let h = crate::core::rng::hash2(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64);
    let roofed = crate::core::rng::unit(h) < 0.35;
    let roof = if crate::core::rng::unit(crate::core::rng::hash2(h, 1, 2)) < 0.5 { 1 } else { 2 };
    let (a, w) = (dist(b.poly[0], b.poly[1]), dist(b.poly[1], b.poly[2]));
    let (l, w) = (a.max(w), a.min(w));
    let top = if l > 1.35 * w { w * w.min(0.42 * l) } else { l * w };
    if top >= BIG_TOP || roofed { roof } else { 0 }
}

/// Where a shell keep's motte reaches past its wall (ft), and how far its slope runs: a mound
/// eased up from the ground, nowhere steep enough for a cliff (its rise 10 to 12 ft).
const MOTTE_TOP: f64 = 8.0;
const MOTTE_SLOPE: f64 = 36.0;

/// Where a keep can stand: this far in from its curtain (ft).
const KEEP_ROOM: f64 = 36.0;

/// The keep: of a kind rolled (a shell keep in a small court now and then), as big as the court
/// holds clear of its curtain (16 ft) and its towers, on the first of `spots` it fits (the highest
/// ground first), on ground level enough to build on. Its building (4 storeys, the castle's
/// pieces) and a shell keep's motte, its way up and the shell's gate toward the court's gate
/// (`gate`).
#[allow(clippy::too_many_arguments)]
fn place_keep<G: Ground>(site: &G, court: &[P], spots: &[(P, f64)], axis: P, square: bool, towers: &[(P, f64)], gate: P, prng: &mut Pcg32) -> Option<(Building, Option<Motte>)> {
    const CLEAR: f64 = 16.0;
    let roll = prng.next_f64();
    let rolls: [f64; 6] = std::array::from_fn(|_| prng.next_f64());
    let small = obb(&hull(court.to_vec())).short < 150.0;
    let first = if roll < if small { 0.2 } else { 0.08 } {
        KeepKind::Shell
    } else if roll < 0.45 {
        KeepKind::Great
    } else if roll < 0.65 {
        KeepKind::Hall
    } else if roll < 0.85 {
        KeepKind::L
    } else {
        KeepKind::T
    };
    // Square towers on a square-towered castle's keep, else round.
    let round = !square;
    let v = [-axis[1], axis[0]];
    let world = |c: P, q: P| add(c, add(mul(axis, q[0]), mul(v, q[1])));
    // (Else a smaller great tower; in a court too small for one, a tower keep without corner towers.)
    for (kind, clear_by) in [(first, CLEAR), (KeepKind::Great, CLEAR), (KeepKind::Plain, 8.0)] {
        for f in [1.0, 0.86, 0.74, 0.64, 0.55] {
            let fr = keep_frame(kind, f, &rolls, round);
            for &(c, _) in spots {
                let mut parts: Vec<Part> = fr
                    .parts
                    .iter()
                    .map(|(p, k)| Part {
                        poly: p.iter().map(|q| world(c, *q)).collect(),
                        kind: *k,
                    })
                    .collect();
                // (A shell keep's ring from its gate round: its first corner toward the court's gate.)
                let way = sub(gate, c);
                let way = if len(way) > 1e-6 { mul(way, 1.0 / len(way)) } else { axis };
                if kind == KeepKind::Shell {
                    let r = dist(parts[0].poly[0], c);
                    let a0 = libm::atan2(way[1], way[0]);
                    parts[0].poly = (0..20)
                        .map(|k| {
                            let a = a0 + std::f64::consts::TAU * k as f64 / 20.0;
                            [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)]
                        })
                        .collect();
                }
                let Some(poly) = union_outline(&parts.iter().map(|p| p.poly.clone()).collect::<Vec<_>>()) else {
                    continue;
                };
                let reach = if kind == KeepKind::Shell { MOTTE_TOP + MOTTE_SLOPE } else { 0.0 };
                let fits = poly.iter().all(|p| inset_depth(court, *p) >= clear_by + reach) && !court.iter().any(|p| contains(&poly, *p));
                // (Off the curtain's towers.)
                let off_towers = towers.iter().all(|(t, r)| inset_depth(&poly, *t) < -(r + clear_by + reach));
                if !fits || !off_towers {
                    continue;
                }
                let (pad, motte) = if kind == KeepKind::Shell {
                    if poly.iter().any(|p| site.wet(*p)) || site.wet(c) {
                        continue;
                    }
                    let r = poly.iter().map(|p| dist(*p, c)).fold(0.0, f64::max);
                    let top = site.height(c) as f32 + 10.0 + 2.0 * rolls[1] as f32;
                    (
                        top,
                        Some(Motte {
                            at: c,
                            top_r: r + MOTTE_TOP,
                            foot_r: r + MOTTE_TOP + MOTTE_SLOPE,
                            top,
                            way,
                        }),
                    )
                } else {
                    if !site.buildable(&poly) {
                        continue;
                    }
                    (site.pad(&poly), None)
                };
                let b = Building {
                    poly,
                    ward: Ward::Castle,
                    func: None,
                    residential: 4,
                    name: None,
                    floors: 4,
                    pad_ft: pad,
                    structure: Structure::Roofed,
                    roof: None,
                    tint: None,
                    id: 0,
                    parts,
                    keep: Some(kind),
                };
                return Some((b, motte));
            }
        }
    }
    None
}

/// The ranges' sizes (ft): length least and most, depth least and most, and how many at most.
/// A castle's ranges, and a great castle's hall range (`Setting::hall`).
const RANGE: (f64, f64, f64, f64, usize) = (24.0, 44.0, 16.0, 24.0, usize::MAX);
const HALL: (f64, f64, f64, f64, usize) = (44.0, 64.0, 22.0, 30.0, 1);

/// Ranges against the inside of the curtain (local ft), or of the town wall where that is the
/// castle's (`walled`): blocks `size` deep and long along each stretch (a hall range along the
/// longest first), clear of the
/// walls, its towers and gate, of the keep (and its motte) by 12 ft, of an
/// inner curtain (20 ft, its towers 10 ft: the outer bailey stays a way round), of the way from
/// gate to gate (`lanes`, 20 ft) and of each other (and of the castle's buildings from `from`),
/// on ground fit to build on; stone ranges under slate, as the castle's own.
#[allow(clippy::too_many_arguments)]
fn ranges<G: Ground>(
    site: &G,
    ring: &[P],
    walled: &[bool],
    inner: Option<&[P]>,
    gates: &[P],
    towers: &[(P, f64)],
    inner_towers: &[(P, f64)],
    lanes: &[(P, P)],
    keep: &[P],
    motte: Option<Motte>,
    w: Ward,
    size: (f64, f64, f64, f64, usize),
    from: usize,
    prng: &mut Pcg32,
    l: &mut Layout,
) {
    let n = ring.len();
    let (lo, hi, dlo, dhi, most) = size;
    let first = l.buildings.len();
    // (The side each range stands along; none for the castle's buildings before these.)
    let mut sides: Vec<usize> = vec![usize::MAX; first - from];
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let mut order: Vec<usize> = (0..n).collect();
    if most < usize::MAX {
        order.sort_by(|&x, &y| dist(ring[y], ring[(y + 1) % n]).total_cmp(&dist(ring[x], ring[(x + 1) % n])).then(x.cmp(&y)));
    }
    for e in order {
        if l.buildings.len() - first >= most {
            break;
        }
        let (a, b) = (ring[e], ring[(e + 1) % n]);
        // (6 ft in from the curtain's line; the town wall's runs 6 ft out from the ring, but where a
        // street runs along its foot.)
        let backs: &[f64] = if walled[e] { &[0.0, 6.0] } else { &[6.0] };
        let el = dist(a, b);
        if el < lo + 16.0 {
            continue;
        }
        let u = mul(sub(b, a), 1.0 / el);
        // Inward (the ring runs anticlockwise).
        let nrm = [-u[1], u[0]];
        let mut t = 16.0;
        while t + lo <= el - 16.0 && l.buildings.len() - first < most {
            let length = prng.range(lo, hi).min(el - 16.0 - t);
            let depth = prng.range(dlo, dhi);
            let gap = prng.range(6.0, 12.0);
            let at = |s: f64, d: f64| add(add(a, mul(u, s)), mul(nrm, d));
            let mut placed = None;
            for &back in backs {
                let poly = vec![at(t, back), at(t + length, back), at(t + length, back + depth), at(t, back + depth)];
                let c = centroid(&poly);
                let inside = poly.iter().all(|p| inset_depth(ring, *p) >= back - 1.0)
                    && !ring.iter().any(|p| inset_depth(&poly, *p) > 1.0)
                    && walls.iter().all(|&(p, q)| super::poly_seg_dist(&poly, p, q) >= 5.5);
                let clear_inner = inner.is_none_or(|q| poly.iter().all(|p| inset_depth(q, *p) <= -20.0) && !q.iter().any(|p| inset_depth(&poly, *p) > -20.0))
                    && inner_towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 10.0));
                let clear_lanes = lanes.iter().all(|&(p, q)| super::poly_seg_dist(&poly, p, q) >= 20.0 && !contains(&poly, p));
                let clear_keep = keep.is_empty() || (poly.iter().all(|p| inset_depth(keep, *p) <= -12.0) && !keep.iter().any(|p| inset_depth(&poly, *p) >= -12.0));
                let clear_motte = motte.is_none_or(|m| poly.iter().all(|p| dist(*p, m.at) >= m.foot_r + 4.0) && dist(c, m.at) >= m.foot_r + 4.0);
                let clear_gate = gates
                    .iter()
                    .all(|g| poly.iter().all(|p| dist(*p, *g) >= GATE_HALF + 18.0) && inset_depth(&poly, *g) < -(GATE_HALF + 12.0));
                let clear_towers = towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 2.0));
                // (Two squares at least from a range along another side, as at a sharp corner: no yard
                // shut in behind them.)
                let clear_others = l.buildings[from..].iter().zip(&sides).all(|(o, &f)| {
                    let gap = if f == e { 4.0 } else { 10.0 };
                    !super::overlaps_or_crosses(&o.poly, &poly) && poly.iter().all(|p| inset_depth(&o.poly, *p) <= -gap) && o.poly.iter().all(|p| inset_depth(&poly, *p) <= -gap)
                });
                // (Off the streets and roads: a paved strip by the bailey.)
                let clear_roads = l
                    .roads
                    .iter()
                    .all(|(pts, _, rw)| pts.windows(2).all(|s| super::poly_seg_dist(&poly, s[0], s[1]) >= 0.5 * rw + 1.0));
                let rest = inside && clear_inner && clear_lanes && clear_keep && clear_motte && clear_gate && clear_towers && clear_others;
                // (Set back further only where it stands on a street.)
                if !rest || clear_roads {
                    placed = (rest && site.buildable(&poly)).then_some(poly);
                    break;
                }
            }
            if let Some(poly) = placed {
                let pad = site.pad(&poly);
                l.buildings.push(Building {
                    poly,
                    ward: w,
                    func: None,
                    residential: 0,
                    name: None,
                    floors: 1,
                    pad_ft: pad,
                    structure: Structure::Roofed,
                    roof: None,
                    // (Slate: `world::TINTS`.)
                    tint: Some(1),
                    id: 0,
                    parts: Vec::new(),
                    keep: None,
                });
                sides.push(e);
                t += length + gap;
            } else {
                t += 8.0;
            }
        }
    }
}

/// The free-standing buildings of a town castle's yard, largest first: a barracks, stables and a
/// chapel (a shrine), as many as the open yard holds (one in 12,000 sq ft of it, 3 at most).
const YARD: [(&str, f64, f64, f64, f64); 3] = [("barracks", 44.0, 56.0, 22.0, 26.0), ("stables", 34.0, 44.0, 20.0, 24.0), ("shrine", 26.0, 32.0, 16.0, 20.0)];

/// The yard's buildings (`YARD`) in the bailey (the outer one, where there is an inner), standing
/// free: 24 ft in from the curtain at least, 16 ft clear of the ranges and of each other and of the
/// keep (its motte 12), 20 of an inner curtain, of the way from gate to gate and of the gates, on
/// ground fit to build on; each along the curtain nearest it, as near it as it goes (the middle
/// of the yard left open). How many were placed.
#[allow(clippy::too_many_arguments)]
fn yard_buildings<G: Ground>(
    site: &G,
    ring: &[P],
    inner: Option<&[P]>,
    gates: &[P],
    towers: &[(P, f64)],
    inner_towers: &[(P, f64)],
    lanes: &[(P, P)],
    keep: &[P],
    motte: Option<Motte>,
    w: Ward,
    from: usize,
    l: &mut Layout,
) -> u8 {
    let n = ring.len();
    if n < 3 {
        return 0;
    }
    // (The yard left open: the bailey but the inner one, the keep and the ranges.)
    let built: f64 = l.buildings[from..].iter().map(|b| area(&b.poly).abs()).sum::<f64>() + area(keep).abs();
    let open = area(ring).abs() - inner.map_or(0.0, |q| area(q).abs()) - built;
    let want = ((open / 12_000.0).max(0.0) as usize).min(YARD.len());
    let (x0, y0, x1, y1) = ring.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let step = 6.0;
    let (nx, ny) = (crate::core::ceil((x1 - x0) / step) as usize, crate::core::ceil((y1 - y0) / step) as usize);
    // (The roads and the town's main streets, 18 ft wide.)
    let roads: Vec<(P, P, f64)> = l
        .roads
        .iter()
        .flat_map(|(pts, _, rw)| pts.windows(2).map(move |s| (s[0], s[1], *rw)))
        .chain(l.streets.iter().flat_map(|pts| pts.windows(2).map(|s| (s[0], s[1], 18.0))))
        .collect();
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let mut placed = 0;
    for &(key, l0, l1, d0, d1) in YARD.iter().take(want) {
        // (Sized by the yard: the more room, the longer.)
        let f = (open / 60_000.0).clamp(0.0, 1.0);
        let (long, deep) = (l0 + f * (l1 - l0), d0 + f * (d1 - d0));
        let mut best: Option<(f64, Vec<P>)> = None;
        for j in 0..=ny {
            for i in 0..=nx {
                let c = [x0 + i as f64 * step, y0 + j as f64 * step];
                let depth = inset_depth(ring, c);
                if depth < 24.0 + 0.5 * deep || best.as_ref().is_some_and(|b| b.0 <= depth) {
                    continue;
                }
                // (Along the curtain nearest it.)
                let Some(e) = (0..n).min_by(|&a, &b| seg_dist(c, ring[a], ring[(a + 1) % n]).total_cmp(&seg_dist(c, ring[b], ring[(b + 1) % n])).then(a.cmp(&b))) else {
                    continue;
                };
                let poly = super::rect(c, sub(ring[(e + 1) % n], ring[e]), long, deep);
                let fits = poly.iter().all(|p| inset_depth(ring, *p) >= 24.0)
                    && walls.iter().all(|&(p, q)| super::poly_seg_dist(&poly, p, q) >= 20.0)
                    && inner.is_none_or(|q| poly.iter().all(|p| inset_depth(q, *p) <= -20.0) && !q.iter().any(|p| inset_depth(&poly, *p) > -20.0))
                    && inner_towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 10.0))
                    && towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 12.0))
                    && lanes.iter().all(|&(p, q)| super::poly_seg_dist(&poly, p, q) >= 20.0 && !contains(&poly, p))
                    && gates.iter().all(|g| poly.iter().all(|p| dist(*p, *g) >= GATE_HALF + 24.0) && inset_depth(&poly, *g) < -(GATE_HALF + 18.0))
                    && (keep.is_empty() || (poly.iter().all(|p| inset_depth(keep, *p) <= -16.0) && !keep.iter().any(|p| inset_depth(&poly, *p) >= -16.0)))
                    && motte.is_none_or(|m| poly.iter().all(|p| dist(*p, m.at) >= m.foot_r + 12.0) && dist(c, m.at) >= m.foot_r + 12.0)
                    && l.buildings[from..].iter().all(|o| {
                        !super::overlaps_or_crosses(&o.poly, &poly) && poly.iter().all(|p| inset_depth(&o.poly, *p) <= -16.0) && o.poly.iter().all(|p| inset_depth(&poly, *p) <= -16.0)
                    })
                    && roads.iter().all(|&(p, q, rw)| super::poly_seg_dist(&poly, p, q) >= 0.5 * rw + 2.0)
                    && site.buildable(&poly);
                if fits {
                    best = Some((depth, poly));
                }
            }
        }
        let Some((_, poly)) = best else { continue };
        let pad = site.pad(&poly);
        l.buildings.push(Building {
            poly,
            ward: w,
            func: super::catalog::index_of(key).map(|f| f as u16),
            residential: 0,
            name: None,
            floors: 1,
            pad_ft: pad,
            structure: Structure::Roofed,
            roof: None,
            tint: Some(1),
            id: 0,
            parts: Vec::new(),
            keep: None,
        });
        placed += 1;
    }
    placed
}

/// Why a town castle's patch was not cut to its size (`cut_bound`; `Layout::castle_cut`): cut (1);
/// the patch too small to leave lots (2); a cut past the sides on the town wall leaves too little,
/// or keeps half as much again as wanted (3); what is left too small or narrow for lots (4); the
/// castle's part too narrow or sharp (5); its part held no keep, or its ring came out with a sharp
/// corner, so the castle has the whole patch after all (6).
pub(super) const CUT_DONE: u8 = 1;
pub(super) const CUT_SMALL: u8 = 2;
pub(super) const CUT_WALL: u8 = 3;
pub(super) const CUT_LEFT: u8 = 4;
pub(super) const CUT_NARROW: u8 = 5;
pub(super) const CUT_KEEP: u8 = 6;
/// The lane between the castle and the lots left (ft), and those lots' least area and width.
pub(super) const LANE: f64 = 14.0;
const LEFT_AREA: f64 = 9_000.0;
const LEFT_WIDE: f64 = 60.0;

/// A town castle's block cut with one chord, parallel to one of its sides: the castle's part
/// (`target` sq ft), a lane `LANE` wide, and the rest of the patch back to the town.
pub(super) struct Cut {
    pub kept: Vec<P>,
    pub left: Vec<P>,
    /// The lane's middle across the patch, and the street line of the lots' side on it (outward
    /// normal, offset: as `street_lines`).
    pub lane: (P, P),
    pub line: (P, f64),
}

/// The cut (`Cut`) of `block` (convex, its patch `patch`) leaving the castle `target` sq ft of the
/// patch: on the low side and towards `toward` (a unit direction: away from the town wall, or
/// towards the market), never leaving out a side on the town wall (`walls`, the patch's sides
/// there), with at least `LEFT_AREA` and `LEFT_WIDE` of the block left and the castle's part of the
/// patch `wide` across. Else why not (`CUT_*`).
pub(super) fn cut_bound(block: &[P], patch: &[P], walls: &[(P, P)], toward: P, height: &dyn Fn(P) -> f64, target: f64, wide: f64) -> Result<Cut, u8> {
    // (Measured on the patch, not the block: the same cut, and the same lots left, whether the town
    // wall stands or not.)
    let mut pp = patch.to_vec();
    if area(&pp) < 0.0 {
        pp.reverse();
    }
    let n = pp.len();
    if n < 3 || block.len() < 3 || area(&pp) < target + LEFT_AREA + 2_000.0 {
        return Err(CUT_SMALL);
    }
    // (A side on the wall and its block's corners, within 12 ft of its ends along the sides off it:
    // on the castle's side.)
    let fixed: Vec<P> = (0..n)
        .filter(|&k| walls.iter().any(|&(a, b)| dist(a, pp[k]) < 0.5 || dist(b, pp[k]) < 0.5))
        .flat_map(|k| {
            let (prev, next) = (pp[(k + n - 1) % n], pp[(k + 1) % n]);
            let step = |q: P| add(pp[k], mul(sub(q, pp[k]), 12.0_f64.min(dist(q, pp[k])) / dist(q, pp[k]).max(1e-9)));
            [pp[k], step(prev), step(next)]
        })
        .collect();
    let half = 0.5 * LANE;
    let mut why = CUT_SMALL;
    let mut best: Option<(f64, Cut)> = None;
    // (Across from a side: its outward normal, the lots on that side; along a side on the wall: away
    // from it; and `toward` itself.)
    let dirs: Vec<P> = (0..n)
        .filter(|&e| dist(pp[e], pp[(e + 1) % n]) >= LEFT_WIDE)
        .map(|e| {
            let (a, b) = (pp[e], pp[(e + 1) % n]);
            mul([b[1] - a[1], a[0] - b[0]], 1.0 / dist(a, b))
        })
        .chain(walls.iter().filter(|w| dist(w.0, w.1) > 1.0).map(|&(a, b)| {
            let v = mul(sub(b, a), 1.0 / dist(a, b));
            let nrm = [-v[1], v[0]];
            if dot(sub(centroid(&pp), a), nrm) >= 0.0 { nrm } else { mul(nrm, -1.0) }
        }))
        .chain((len(toward) > 0.5).then_some(toward))
        .collect();
    for u in dirs {
        let (lo, hi) = pp.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(dot(*p, u)), m.1.max(dot(*p, u))));
        let kept_at = |s: f64| clip_plain(&pp, u, s - half);
        let (mut s0, mut s1) = (lo + half, hi + half);
        for _ in 0..40 {
            let s = 0.5 * (s0 + s1);
            if area(&kept_at(s)).abs() < target {
                s0 = s;
            } else {
                s1 = s;
            }
        }
        // (Past the wall's side, where it reaches further: what is left is still the town's.)
        let mut s = fixed.iter().map(|p| dot(*p, u) + half + 1.0).fold(0.5 * (s0 + s1), f64::max);
        let pushed = s > 0.5 * (s0 + s1) + 0.5;
        // (And past where the castle's part is `wide` across.)
        while s < hi + half && obb(&kept_at(s)).short < wide {
            s += 5.0;
        }
        let left = clip_plain(block, mul(u, -1.0), -(s + half));
        if left.len() < 3 || area(&left).abs() < LEFT_AREA || obb(&left).short < LEFT_WIDE {
            why = why.max(if pushed { CUT_WALL } else { CUT_LEFT });
            continue;
        }
        let part = kept_at(s);
        // (Not where the wall's side keeps it twice as big: the castle has the whole patch.)
        if pushed && area(&part) > 2.0 * target {
            why = why.max(CUT_WALL);
            continue;
        }
        // (Nor a corner of it sharper than 50 degrees where the cut meets a side: a wedge of yard.)
        let k = part.len();
        let sharp = (0..k).any(|j| {
            let (a, b) = (sub(part[(j + k - 1) % k], part[j]), sub(part[(j + 1) % k], part[j]));
            let (la, lb) = (len(a), len(b));
            la > 1.0 && lb > 1.0 && dot(a, b) / (la * lb) > libm::cos(50.0_f64.to_radians())
        });
        if k < 3 || sharp || obb(&part).short < wide {
            why = why.max(CUT_NARROW);
            continue;
        }
        let kept = clip_plain(block, u, s - half);
        if kept.len() < 3 {
            continue;
        }
        // The lane's middle: the line across the patch.
        let mut ends: Vec<P> = Vec::new();
        let m = patch.len();
        for k in 0..m {
            let (p, q) = (patch[k], patch[(k + 1) % m]);
            let (dp, dq) = (dot(p, u) - s, dot(q, u) - s);
            if (dp < 0.0) != (dq < 0.0) {
                ends.push(lerp(p, q, dp / (dp - dq)));
            }
        }
        let along = [-u[1], u[0]];
        let (Some(ea), Some(eb)) = (
            ends.iter().copied().min_by(|x, y| dot(*x, along).total_cmp(&dot(*y, along))),
            ends.iter().copied().max_by(|x, y| dot(*x, along).total_cmp(&dot(*y, along))),
        ) else {
            continue;
        };
        // (Lower ground left to the town, the side `toward` preferred, and the castle's part the
        // nearest its size.)
        let score = height(centroid(&left)) - height(centroid(&part)) - 8.0 * dot(u, toward) + (area(&part) - target).max(0.0) / 2_000.0;
        if best.as_ref().is_none_or(|x| score < x.0) {
            best = Some((score, Cut { kept, left, lane: (ea, eb), line: (mul(u, -1.0), -(s + half)) }));
        }
    }
    best.map(|b| b.1).ok_or(why)
}
