//! Growing a village’s lanes: branch points, joins, lane ends and farmsteads.

use super::*;

/// A branch lane's step as it grows (ft), and how finely each step is looked along (ft).
const STEP_FT: f64 = 25.0;
const PROBE_FT: f64 = 5.0;
/// How near a way's edge a growing lane comes before it turns to join it (ft); how far a lane's end
/// looks for a way to run on to as it grows (ft), and once the village is laid out.
const JOIN_NEAR_FT: f64 = 15.0;
pub(super) const JOIN_REACH_FT: f64 = 60.0;
pub(super) const JOIN_LATE_FT: f64 = 40.0;
/// The longest stub past a crossroads cut there though a house stands by it, the house by a way still (ft).
const CROSS_STUB_FT: f64 = 35.0;
/// The hardest turn a lane takes to join a way (degrees), and the shallowest its T meets it at.
pub(super) const JOIN_TURN_DEG: f64 = 45.0;
pub(super) const T_DEG: f64 = 35.0;
/// How far a lane may run on past its last house or yard and be cut back to it (ft); farther, over
/// open ground, it ends at a farmstead.
const FARM_RUN_FT: f64 = 150.0;

/// Where a branch lane leaves: a point on its parent's centre line, the way it heads, its depth, and
/// its parent (its place in the lanes; `usize::MAX`: none, it leaves the green).
pub(super) type Start = (P, P, u32, usize);

/// Branch starts every `every` ft along lane `li` (`pts`), on its centre line, alternating sides at
/// random.
pub(super) fn branch_points(pts: &[P], li: usize, every: f64, rng: &mut Pcg32, out: &mut Vec<Start>, depth: u32) {
    let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let mut a = every * rng.range(0.5, 1.0);
    while a < total - 0.5 * every {
        let (p, u) = point_along(pts, a);
        let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
        let d = rotate([-u[1] * side, u[0] * side], rng.range(-0.35, 0.35));
        out.push((p, d, depth, li));
        a += every * rng.range(0.8, 1.3);
    }
}

/// Where segments `a`-`b` and `c`-`d` cross: the share of the way along each.
pub(super) fn seg_cross(a: P, b: P, c: P, d: P) -> Option<(f64, f64)> {
    let (e, f) = (sub(b, a), sub(d, c));
    let den = cross(e, f);
    if den.abs() < 1e-9 {
        return None;
    }
    let (t, u) = (cross(sub(c, a), f) / den, cross(sub(c, a), e) / den);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// How many times a lane (from `from` in `lanes`) crosses another lane or a road but within a foot
/// of either's ends.
pub(super) fn crossings(lanes: &[(Vec<P>, f64)], from: usize) -> usize {
    let mut n = 0;
    for li in from..lanes.len() {
        let a = &lanes[li].0;
        for (j, (b, _)) in lanes.iter().enumerate() {
            if j == li || (j >= from && j < li) || a.len() < 2 || b.len() < 2 {
                continue;
            }
            let ends = [a[0], a[a.len() - 1], b[0], b[b.len() - 1]];
            for s in a.windows(2) {
                for t in b.windows(2) {
                    if let Some((u, _)) = seg_cross(s[0], s[1], t[0], t[1])
                        && ends.iter().all(|e| dist(*e, lerp(s[0], s[1], u)) > 1.0)
                    {
                        n += 1;
                    }
                }
            }
        }
    }
    n
}

/// The point of segment `a`-`b` nearest `p`.
pub(super) fn nearest_on(p: P, a: P, b: P) -> P {
    let e = sub(b, a);
    let l2 = dot(e, e);
    if l2 < 1e-12 {
        return a;
    }
    lerp(a, b, (dot(sub(p, a), e) / l2).clamp(0.0, 1.0))
}

pub(super) fn length(pts: &[P]) -> f64 {
    pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// How far along `pts` its point nearest `p` lies (ft).
pub(super) fn arc_at(pts: &[P], p: P) -> f64 {
    let (mut best, mut at, mut acc) = (f64::MAX, 0.0, 0.0);
    for w in pts.windows(2) {
        let q = nearest_on(p, w[0], w[1]);
        let d = dist(p, q);
        if d < best {
            (best, at) = (d, acc + dist(w[0], q));
        }
        acc += dist(w[0], w[1]);
    }
    at
}

/// `pts` cut `keep` ft along.
pub(super) fn cut_at(pts: &[P], keep: f64) -> Vec<P> {
    let mut out = vec![pts[0]];
    let mut acc = 0.0;
    for s in pts.windows(2) {
        let d = dist(s[0], s[1]);
        if acc + d >= keep {
            if keep - acc > 0.05 {
                out.push(lerp(s[0], s[1], (keep - acc) / d.max(1e-9)));
            }
            return out;
        }
        out.push(s[1]);
        acc += d;
    }
    out
}

/// Whether `p` lies on a way but lane `li` (within a foot of its centre line).
pub(super) fn on_way(lanes: &[(Vec<P>, f64)], li: usize, p: P) -> bool {
    lanes.iter().enumerate().any(|(j, (q, _))| j != li && q.windows(2).any(|s| seg_dist(p, s[0], s[1]) <= 1.0))
}

/// `x` on lane `j`'s centre line made one of its points, or the point of it within 2 ft (returned):
/// the node a lane joining there shares.
pub(super) fn node(lanes: &mut [(Vec<P>, f64)], j: usize, x: P) -> P {
    node_within(lanes, j, x, 2.0)
}

/// `node`, the point of the lane within `snap` ft of `x` taken for it.
pub(super) fn node_within(lanes: &mut [(Vec<P>, f64)], j: usize, x: P, snap: f64) -> P {
    let pts = &mut lanes[j].0;
    let Some(k) = (0..pts.len().saturating_sub(1)).min_by(|&a, &b| seg_dist(x, pts[a], pts[a + 1]).total_cmp(&seg_dist(x, pts[b], pts[b + 1]))) else { return x };
    for v in [pts[k], pts[k + 1]] {
        if dist(v, x) < snap {
            return v;
        }
    }
    pts.insert(k + 1, x);
    x
}

/// Where a lane ending at the last of `own` (`me`: its place in `lanes`, if there) can run on to join
/// a way in a T: the nearest point of a way within `reach` ft it turns to by `JOIN_TURN_DEG` at most
/// and meets at `T_DEG` or more, the ground on the way there dry, across no other way nor itself, and
/// `clear` (from the end to short of the way); but where `skip` (the way, the point) rules it out.
pub(super) fn join_to(site: &Site, own: &[P], me: usize, lanes: &[(Vec<P>, f64)], reach: f64, skip: &dyn Fn(usize, P) -> bool, clear: &dyn Fn(P, P, f64) -> bool) -> Option<(usize, P)> {
    let n = own.len();
    if n < 2 {
        return None;
    }
    let p = own[n - 1];
    let d = sub(p, own[n - 2]);
    let d = mul(d, 1.0 / len(d).max(1e-9));
    let (turn, t_min) = (libm::cos(JOIN_TURN_DEG.to_radians()), libm::sin(T_DEG.to_radians()));
    let mut cands: Vec<(f64, usize, P, f64)> = Vec::new();
    for (j, (q, w)) in lanes.iter().enumerate() {
        if j == me {
            continue;
        }
        for s in q.windows(2) {
            let (a, b) = (s[0], s[1]);
            let el = dist(a, b);
            if el < 1e-6 || seg_dist(p, a, b) > reach {
                continue;
            }
            let tu = mul(sub(b, a), 1.0 / el);
            let ray = add(p, mul(d, reach));
            let m = (el / 6.0).ceil() as usize;
            let xs = std::iter::once(nearest_on(p, a, b)).chain(seg_cross(p, ray, a, b).map(|(t, _)| lerp(p, ray, t))).chain((0..=m).map(|k| lerp(a, b, k as f64 / m as f64)));
            for x in xs {
                let v = sub(x, p);
                let vl = len(v);
                if vl < 0.5 || vl > reach || skip(j, x) {
                    continue;
                }
                let v = mul(v, 1.0 / vl);
                let c = dot(v, d);
                if c < turn || cross(v, tu).abs() < t_min {
                    continue;
                }
                cands.push((vl + 30.0 * (1.0 - c), j, x, 0.5 * w));
            }
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    'c: for (_, j, x, hw) in cands.into_iter().take(40) {
        let vl = dist(p, x);
        let m = (vl / 4.0).ceil() as usize;
        if (1..=m).any(|k| {
            let s = lerp(p, x, k as f64 / m as f64);
            site.wet(s) || site.near_river(s, 6.0)
        }) {
            continue;
        }
        for (i, (q, _)) in lanes.iter().enumerate() {
            if i == me {
                continue;
            }
            for s in q.windows(2) {
                if seg_cross(p, x, s[0], s[1]).is_some_and(|(t, _)| t * vl > 0.5 && t * vl < vl - 1.0) {
                    continue 'c;
                }
            }
        }
        if own[..n - 1].windows(2).any(|s| seg_cross(p, x, s[0], s[1]).is_some_and(|(t, _)| t * vl > 0.5)) || !clear(p, x, hw) {
            continue;
        }
        return Some((j, x));
    }
    None
}

/// Grow a lane from `p0` on its parent's centre line (`parent`: its place in `lanes`) heading `d0`
/// (after watabou's road trackers): 25-ft steps, each turning (by up to 20°) toward the most level
/// ground, looked along every 5 ft. It stops at water, the river's banks or `max_r`; where it crosses
/// a way, or comes within `JOIN_NEAR_FT` of one's edge, it runs on to it (`join_to`) and ends on its
/// centre line in a T, as its far end does where a way is near. Its points, and the way it joins at
/// its end and where.
pub(super) fn grow(site: &Site, p0: P, d0: P, max_r: f64, lanes: &[(Vec<P>, f64)], parent: usize, rng: &mut Pcg32) -> (Vec<P>, Option<(usize, P)>) {
    let mut pts = vec![p0];
    let (mut p, mut d) = (p0, d0);
    let wander = rng.range(-0.08, 0.08);
    // (Its parent by its start is no way to join, nor what meets its parent there (the next piece of a
    // road, a lane leaving it); nor, leaving the green, what crosses its middle.)
    let skip = |j: usize, x: P| (j == parent && dist(x, p0) < 60.0) || dist(x, p0) < 45.0;
    let t_min = libm::sin(T_DEG.to_radians());
    let me = usize::MAX;
    let no_band = |_: P, _: P, _: f64| true;
    for _ in 0..(max_r / STEP_FT) as usize + 4 {
        let h = site.height(p);
        let mut best: Option<(f64, P)> = None;
        for turn in [-0.35, 0.0, 0.35] {
            let nd = rotate(d, turn + wander);
            let q = add(p, mul(nd, STEP_FT));
            let cost = (site.height(q) - h).abs() + if turn == 0.0 { 0.0 } else { 0.6 };
            if best.is_none_or(|b| cost < b.0) {
                best = Some((cost, nd));
            }
        }
        d = best.unwrap().1;
        let q = add(p, mul(d, STEP_FT));
        let probes: Vec<P> = (1..=5).map(|k| add(p, mul(d, PROBE_FT * k as f64))).collect();
        if len(q) > max_r || probes.iter().any(|&s| dist(s, p0) > 12.0 && (site.wet(s) || site.near_river(s, 10.0))) {
            break;
        }
        // (Across a way: it ends on it there, if at a T's angle; else it turns onto one, or stops.)
        let mut hit: Option<(f64, usize, P, P)> = None;
        for (j, (lp, _)) in lanes.iter().enumerate() {
            for s in lp.windows(2) {
                let Some((t, _)) = seg_cross(p, q, s[0], s[1]) else { continue };
                let x = lerp(p, q, t);
                if dist(x, p0) > 2.0 && (parent != usize::MAX || dist(x, p0) > 45.0) && hit.is_none_or(|h| t < h.0) {
                    hit = Some((t, j, x, sub(s[1], s[0])));
                }
            }
        }
        let near = || lanes.iter().enumerate().any(|(j, (lp, w))| lp.windows(2).any(|s| probes.iter().any(|&pr| seg_dist(pr, s[0], s[1]) < 0.5 * w + JOIN_NEAR_FT && !skip(j, nearest_on(pr, s[0], s[1])))));
        if hit.is_some() || near() {
            // (From the step's end if it crosses nothing, else from its start.)
            let mut joined = None;
            if hit.is_none() {
                pts.push(q);
                joined = join_to(site, &pts, me, lanes, JOIN_REACH_FT, &skip, &no_band);
                if joined.is_none() {
                    pts.pop();
                }
            }
            if let Some((j, x)) = joined.or_else(|| join_to(site, &pts, me, lanes, JOIN_REACH_FT, &skip, &no_band)) {
                pts.push(x);
                return (pts, Some((j, x)));
            }
            if let Some((_, j, x, tg)) = hit {
                if cross(d, mul(tg, 1.0 / len(tg).max(1e-9))).abs() >= t_min {
                    pts.push(x);
                    return (pts, Some((j, x)));
                }
                return (pts, None);
            }
        }
        pts.push(q);
        p = q;
    }
    match join_to(site, &pts, me, lanes, JOIN_REACH_FT, &skip, &no_band) {
        Some((j, x)) => {
            pts.push(x);
            (pts, Some((j, x)))
        }
        None => (pts, None),
    }
}

/// Distance from `p` to `poly`'s outline (0 inside it).
fn outline_d(poly: &[P], p: P) -> f64 {
    if contains(poly, p) {
        return 0.0;
    }
    let m = poly.len();
    (0..m).map(|k| seg_dist(p, poly[k], poly[(k + 1) % m])).fold(f64::MAX, f64::min)
}

/// A lane may end at `p`: a building within 10 ft, a plot within 7.5 with a building within 14 (a
/// croft cut back off the sand after, `fields::crofts_off_sand`, leaves its cottage by the lane).
fn at_home(g: &Ground, l: &Layout, p: P) -> bool {
    near_home(g, l, p, 10.0)
}

/// `at_home`, a building within `m` ft.
fn near_home(g: &Ground, l: &Layout, p: P, m: f64) -> bool {
    let by = |bb: [f64; 4], m: f64| p[0] > bb[0] - m && p[0] < bb[2] + m && p[1] > bb[1] - m && p[1] < bb[3] + m;
    let house = |m: f64| l.buildings.iter().any(|b| by(bbox(&b.poly), m) && outline_d(&b.poly, p) <= m);
    house(m) || (g.boxes.iter().zip(&g.plots).any(|(bb, pl)| by(*bb, 7.5) && outline_d(&pl.pts, p) <= 7.5) && house(14.0))
}

/// By the water: on it, within 30 ft of the river or with water 30 ft off.
fn by_water(site: &Site, p: P) -> bool {
    site.wet(p) || site.near_river(p, 30.0) || (0..8).any(|k| site.wet(add(p, polar_pt(30.0, std::f64::consts::TAU * k as f64 / 8.0))))
}

/// Every lane's two ends joined or there on purpose, once the village is laid out: an end on a way
/// (or a ring closed on itself) stays; else it runs on to a way within `JOIN_LATE_FT` where the
/// ground between is clear of the plots and buildings; else, by the water, it is cut back to its
/// last house or yard (about 6 ft past it); else, run on more than `FARM_RUN_FT` past it, it ends at a farmstead
/// (`farmstead`), or is cut back to it. A lane is never cut short of a junction on it (where a lane
/// leaves it or ends on it: it may end at that T); one with nothing along it nor on it is dropped.
/// Children first (a lane's after its parent's), again until nothing changes.
pub(super) fn settle_ends(g: &mut Ground, l: &mut Layout, lanes: &mut Vec<(Vec<P>, f64)>, n_roads: usize, hedge_share: f64, rng: &mut Pcg32) {
    g.lanes = LaneGrid::new(lanes);
    for _ in 0..4 {
        let mut changed = false;
        for li in (n_roads..lanes.len()).rev() {
            for end in [1, 0] {
                if lanes[li].0.len() < 2 {
                    continue;
                }
                if end == 0 {
                    lanes[li].0.reverse();
                }
                let done = settle_end(g, l, lanes, li, n_roads, hedge_share, rng);
                if end == 0 {
                    lanes[li].0.reverse();
                }
                if done {
                    changed = true;
                    g.lanes = LaneGrid::new(lanes);
                }
            }
        }
        if !changed {
            break;
        }
    }
    lanes.retain(|(pts, _)| pts.len() >= 2);
    g.lanes = LaneGrid::new(lanes);
}

/// `settle_ends` for lane `li`'s last point: whether it changed.
fn settle_end(g: &mut Ground, l: &mut Layout, lanes: &mut [(Vec<P>, f64)], li: usize, n_roads: usize, hedge_share: f64, rng: &mut Pcg32) -> bool {
    let pts = lanes[li].0.clone();
    let e = pts[pts.len() - 1];
    if on_way(lanes, li, e) || at_gate(g, e) || e == pts[0] {
        return false;
    }
    // (Run on to a way near it: its band clear of the buildings, the plots and the green, short of the way's edge.)
    let clear = |a: P, b: P, hw: f64| {
        let v = sub(b, a);
        let vl = len(v) - hw - 2.0;
        if vl < 0.5 {
            return true;
        }
        let u = mul(v, 1.0 / len(v));
        let band = quad(a, u, LANE_W, vl);
        // (A house it runs by square to it, as `square_to`.)
        let (cos, sin) = (libm::cos(SQUARE_DEG.to_radians()), libm::sin(SQUARE_DEG.to_radians()));
        let square = l.buildings.iter().filter(|h| h.structure == Structure::Roofed && h.poly.len() == 4 && poly_gap(&h.poly, &[a, b]) < 0.5 * LANE_W + SQUARE_FT).all(|h| {
            let e = sub(h.poly[1], h.poly[0]);
            let k = (dot(e, u) / len(e).max(1e-9)).abs();
            k > cos || k < sin
        });
        square && !building_hit(l, &band, None) && !g.plot_hit(&band) && g.lane_off_green(a, b)
    };
    // (The farthest junction on it: it keeps to there at least.)
    let anchor = lanes.iter().enumerate().filter(|&(j, (q, _))| j != li && j >= n_roads && q.len() >= 2).flat_map(|(_, (q, _))| [q[0], q[q.len() - 1]]).filter(|&a| pts.windows(2).any(|s| seg_dist(a, s[0], s[1]) <= 1.0)).map(|a| (arc_at(&pts, a), a)).max_by(|x, y| x.0.total_cmp(&y.0));
    let floor = anchor.map_or(0.0, |a| a.0);
    let total = length(&pts);
    // (Its last house or yard, a house's front within 10 ft of its edge.)
    let reach = 0.5 * lanes[li].1 + 10.0;
    let mut last = None;
    let mut at = total - 2.0;
    while at > floor {
        if near_home(g, l, point_along(&pts, at).0, reach) {
            last = Some(at);
            break;
        }
        at -= 2.0;
    }
    // (Across a way just before it, past its last house, a crossroads: it ends there.)
    let mut acc = 0.0;
    let mut cross_at = None;
    for k in 1..pts.len() - 1 {
        acc += dist(pts[k - 1], pts[k]);
        // (Or its last house just past it, the stub on past the crossroads short: the house by it.)
        let stub = total - acc <= CROSS_STUB_FT && {
            // (Every house by the stub still by a way: the crossroads' or the lane short of it.)
            let (kept, cut) = (&pts[..=k], &pts[k..]);
            let by = |q: &[P], h: &[P], m: f64| q.windows(2).any(|s| poly_gap(h, s) < m);
            let other = |h: &[P]| lanes.iter().enumerate().any(|(j, (q, w))| j != li && by(q, h, 0.5 * w + 10.0));
            l.buildings.iter().filter(|b| by(cut, &b.poly, 0.5 * LANE_W + 12.0)).all(|b| by(kept, &b.poly, 0.5 * LANE_W + 10.0) || other(&b.poly))
        };
        if total - acc <= JOIN_LATE_FT && acc >= floor - 0.5 && (stub || last.is_none_or(|s| s <= acc + 2.0)) && on_way(lanes, li, pts[k]) {
            cross_at = Some(k);
            break;
        }
    }
    if let Some(k) = cross_at {
        lanes[li].0.truncate(k + 1);
        return true;
    }
    // (A few feet short of a way: on it.)
    let near = lanes.iter().enumerate().filter(|&(j, _)| j != li).flat_map(|(j, (q, _))| q.windows(2).map(move |s| (j, nearest_on(e, s[0], s[1])))).min_by(|a, b| dist(e, a.1).total_cmp(&dist(e, b.1)));
    // (No harder a turn for it than a join's.)
    let turn_ok = |x: P| {
        let n = pts.len();
        n < 3 || {
            let (u, v) = (sub(pts[n - 2], pts[n - 3]), sub(x, pts[n - 2]));
            dot(u, v) >= libm::cos(JOIN_TURN_DEG.to_radians()) * len(u) * len(v)
        }
    };
    if let Some((j, x)) = near
        && dist(e, x) <= 6.0
        && dist(pts[pts.len() - 2], e) > 10.0
        && turn_ok(x)
    {
        let x = node(lanes, j, x);
        let n = lanes[li].0.len();
        lanes[li].0[n - 1] = x;
        return true;
    }
    if let Some((j, x)) = join_to(g.site, &pts, li, lanes, JOIN_LATE_FT, &|_, _| false, &clear) {
        let x = node(lanes, j, x);
        lanes[li].0.push(x);
        return true;
    }
    if let Some((j, m, x)) = bend_to(g.site, &pts, li, lanes, &clear) {
        let x = node(lanes, j, x);
        lanes[li].0.push(m);
        lanes[li].0.push(x);
        return true;
    }
    if let Some((j, on)) = ring_on(g, l, lanes, li, n_roads, &pts) {
        let x = node(lanes, j, on[on.len() - 1]);
        lanes[li].0.extend_from_slice(&on[..on.len() - 1]);
        lanes[li].0.push(x);
        return true;
    }
    let water = by_water(g.site, e);
    if at_home(g, l, e) {
        return false;
    }
    let from = last.unwrap_or(floor);
    if !water && total - from >= FARM_RUN_FT {
        if let Some(lane) = farmstead(g, l, lanes, li, from + 40.0, hedge_share, rng) {
            lanes[li].0 = lane;
            return true;
        }
    }
    match (last, anchor) {
        (Some(at), _) => lanes[li].0 = cut_at(&pts, at),
        (None, Some((at, a))) => {
            let mut cut = cut_at(&pts, at);
            let n = cut.len();
            if n >= 2 && dist(cut[n - 1], a) < 1.5 {
                cut[n - 1] = a;
            } else {
                cut.push(a);
            }
            lanes[li].0 = cut;
        }
        (None, None) => lanes[li].0.clear(),
    }
    if lanes[li].0.len() < 2 || length(&lanes[li].0) < 1.0 {
        lanes[li].0.clear();
    }
    true
}

/// A farmstead where lane `li` runs on over open ground: a farmhouse (`Ward::Farm`) across the back of
/// a yard (`PlotKind::Farmyard`, hedged or fenced, as the manor's), a barn (the catalog's stables, its
/// stalls inside) along one side, the lane run to its gate. The yard's front 7 ft past the lane's end,
/// on ground open (`yard_ok`) and level enough to work; else with the lane cut every 20 ft back, to
/// `from` ft along it. The lane as it is then, if it stands: run on to `FARM_GATE_FT` short of the
/// gate, its round end in the gate's gap.
pub(super) fn farmstead(g: &mut Ground, l: &mut Layout, lanes: &[(Vec<P>, f64)], li: usize, from: f64, hedge_share: f64, rng: &mut Pcg32) -> Option<Vec<P>> {
    let barn_f = catalog::index_of("stables")?;
    let pts = &lanes[li].0;
    let (hw_, hd) = (rng.range(34.0, 44.0), rng.range(20.0, 26.0));
    let (bw, bd) = (rng.range(38.0, 50.0), rng.range(22.0, 28.0));
    let yw = rng.range(76.0, 96.0).max(2.0 * bd + 30.0).max(hw_ + 14.0);
    let yd = rng.range(72.0, 92.0).max(bw + hd + 24.0);
    let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
    let bound = if rng.next_f64() < hedge_share { Bound::Hedge } else { Bound::Fence };
    let total = length(pts);
    let mut at = total;
    while at >= from {
        let (e, u) = point_along(pts, at);
        let t = [u[1], -u[0]];
        let m = add(e, mul(u, 7.0));
        let yard = quad(m, u, yw, yd);
        let barn = rect(add(m, add(mul(t, side * (0.5 * yw - 6.0 - 0.5 * bd)), mul(u, 8.0 + 0.5 * bw))), u, bw, bd);
        let house = rect(add(m, add(mul(t, -side * (0.5 * yw - 6.0 - 0.5 * hw_)), mul(u, yd - 6.0 - 0.5 * hd))), t, hw_, hd);
        let (lo, hi) = g.site.relief(&yard);
        if hi - lo < 8.0 && g.site.buildable(&house) && g.site.buildable(&barn) {
            // (Off the lanes as they would be: this one cut there.)
            let mut trial = lanes.to_vec();
            trial[li].0 = cut_at(pts, at);
            let held = std::mem::replace(&mut g.lanes, LaneGrid::new(&trial));
            if yard_ok(g, l, &yard) && yard_clear(l, &yard) && square_to(&trial, &house, t, None, SQUARE_DEG) && square_to(&trial, &barn, u, None, SQUARE_DEG) {
                let mut home = new_building(g.site, house, Ward::Farm, None, Structure::Roofed);
                home.residential = catalog::residential_for(Ward::Farm, false) as u8;
                l.buildings.push(home);
                l.buildings.push(new_building(g.site, barn, Ward::Rural, Some(barn_f), Structure::Roofed));
                g.add(Plot { pts: yard, kind: PlotKind::Farmyard, bound });
                let mut lane = cut_at(pts, at);
                lane.push(add(e, mul(u, 7.0 - FARM_GATE_FT)));
                return Some(lane);
            }
            g.lanes = held;
        }
        at -= 20.0;
    }
    None
}

/// A branch joining lane `end` that runs within `SLIVER_FT` of it all the way: a sliver between the
/// two (they leave their way side by side), not a loop.
pub(super) fn sliver(lane: &[P], end: Option<(usize, P)>, lanes: &[(Vec<P>, f64)]) -> bool {
    let Some((j, _)) = end else { return false };
    let (q, total) = (&lanes[j].0, length(lane));
    let n = (total / 5.0).ceil().max(1.0) as usize;
    (0..=n).all(|k| {
        let p = point_along(lane, total * k as f64 / n as f64).0;
        q.windows(2).any(|s| seg_dist(p, s[0], s[1]) < SLIVER_FT)
    })
}
