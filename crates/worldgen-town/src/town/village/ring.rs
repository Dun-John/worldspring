//! Ring villages: the ring road round a green, spurs and roads onto the ring.

use super::*;

pub(super) const SLIVER_FT: f64 = 30.0;

// The lane round the green (R5 towns, 103): the lanes reaching the green meet it in Ts, the roads
// across it cross it, the cottages along its outer side face the green across it.

/// The green's radius (ft), from the draw of 35-55 it was.
pub(super) const GREEN_FT: (f64, f64) = (45.0, 70.0);
/// How far outside the ring's centre line the lanes reaching the green are cut (ft): none runs along
/// the ring nearer, and each runs on to it from there (`to_ring`).
pub(super) const RING_CLEAR_FT: f64 = 20.0;
/// The shallowest a lane or the ring meets a way at (degrees).
const RING_T_DEG: f64 = 55.0;
/// The shallowest a lane meets the ring at where it cannot at `RING_T_DEG` (degrees).
const RING_T_LATE_DEG: f64 = 40.0;
/// How near a road's edge the ring's edge comes before it gives way to the road (ft).
const RING_ROAD_FT: f64 = 8.0;
/// The ring's points (15° apart: a house along it square to its pieces by half that).
const RING_N: usize = 24;
/// The farthest the ring runs on to a road, or a lane on to the ring (ft).
const RING_REACH_FT: f64 = 45.0;
/// How far on round the circle a ring run's end left short of a road looks for one (`ring_on`, degrees).
const RING_ON_DEG: f64 = 60.0;
/// The shortest run of the ring kept that reaches no road at either end (ft).
const RING_LONE_FT: f64 = 120.0;
/// How far outside the ring a lane by the green that cannot meet it is cut back to (ft): no house
/// laid by its end on the green's rim.
const RING_OFF_FT: f64 = 32.0;

/// Angle `a` round the green turned to the ring's nearest point (`ring`): a yard's front on the ring
/// along its tangent there, square to the pieces either side by half their turn. As it was with no
/// green.
pub(super) fn ring_at(g: &Ground, a: f64) -> f64 {
    let seg = std::f64::consts::TAU / RING_N as f64;
    if g.green.1 > 0.0 { crate::core::round(a / seg) * seg } else { a }
}

/// A road ending at the village's middle and no other way through it there (a spur to the village):
/// its pieces within `SPUR_FT` of that end, and the way it arrives along (a unit vector).
pub(super) fn spur_end(lanes: &[(Vec<P>, f64)], n_roads: usize) -> Option<(Vec<(P, P)>, P)> {
    let at_mid = |p: P| len(p) < 1.0;
    let ends: Vec<usize> = (0..n_roads).filter(|&j| at_mid(lanes[j].0[0]) || at_mid(lanes[j].0[lanes[j].0.len() - 1])).collect();
    if ends.len() != 1 || (0..n_roads).any(|j| j != ends[0] && lanes[j].0.windows(2).any(|s| seg_dist([0.0, 0.0], s[0], s[1]) < 1.0)) {
        return None;
    }
    // (Back from the end along the road, on through the pieces of the road that join it end to end.)
    let mut j = ends[0];
    let mut q: Vec<P> = lanes[j].0.clone();
    if !at_mid(q[q.len() - 1]) {
        q.reverse();
    }
    let t = sub(q[q.len() - 1], q[q.len() - 2]);
    if len(t) < 1e-6 {
        return None;
    }
    let mut pieces = Vec::new();
    let mut walked = 0.0;
    loop {
        for w in q.windows(2).rev() {
            if walked >= SPUR_FT {
                break;
            }
            pieces.push((w[0], w[1]));
            pieces.push((w[1], w[0]));
            walked += dist(w[0], w[1]);
        }
        let start = q[0];
        let Some(k) = (0..n_roads).find(|&k| k != j && (dist(lanes[k].0[0], start) < 1e-3 || dist(lanes[k].0[lanes[k].0.len() - 1], start) < 1e-3)) else { break };
        if walked >= SPUR_FT {
            break;
        }
        j = k;
        q = lanes[k].0.clone();
        if dist(q[q.len() - 1], start) >= 1e-3 {
            q.reverse();
        }
    }
    Some((pieces, mul(t, 1.0 / len(t))))
}

/// How much of a spur's end the ring does not give way to (ft).
const SPUR_FT: f64 = 60.0;

/// The well for a green beyond the end of a spur arriving along `t` at the middle (`spur_end`): the
/// end on the ring's circle at one of its points (`ring`), the road meeting it within 37.5° of
/// square; the well dry and `ok`, no more than two of eight points round the green wet.
pub(super) fn spur_well(site: &Site, t: P, green_r: f64, ok: &dyn Fn(P) -> bool) -> Option<P> {
    let rr = green_r + 0.5 * LANE_W;
    let seg = std::f64::consts::TAU / RING_N as f64;
    let a = libm::atan2(-t[1], -t[0]);
    let a = crate::core::round(a / seg) * seg;
    [0.0, 1.0, -1.0, 2.0, -2.0].into_iter().map(|k| sub([0.0, 0.0], polar_pt(rr, a + k * seg))).find(|&w| {
        let wet = (0..8).filter(|&k| site.wet(add(w, polar_pt(green_r, std::f64::consts::TAU * k as f64 / 8.0)))).count();
        !site.wet(w) && wet <= 2 && ok(w)
    })
}

/// Whether lane `pts` is a run of the ring round `well` (`ring`): its points but its ends on the
/// circle of radius `rr` (or a node on one of its chords).
pub(super) fn on_ring(pts: &[P], well: P, rr: f64) -> bool {
    let n = pts.len();
    n >= 4 && pts[1..n - 1].iter().all(|&p| (dist(p, well) - rr + 0.75).abs() < 1.25)
}

/// The lane round the green: its centre line `rr` ft round `well`, `RING_N` points anticlockwise. Where
/// a road crosses it or runs by (their edges within `RING_ROAD_FT`), or the water, it gives way; each
/// run left runs on round the circle to where a road crosses it within 50°, else straight on along
/// its tangent to a road (its last piece meeting the road at `RING_T_DEG` or squarer; from up to two
/// points back), a node of the road's (two such on a road within 12 ft: one crossroads).
/// A run shorter than 40 ft is dropped. Pushed onto `lanes`; the points the runs reach out to.
pub(super) fn ring(site: &Site, well: P, rr: f64, lanes: &mut Vec<(Vec<P>, f64)>, n_roads: usize, spur: &[(P, P)]) -> Vec<P> {
    let n = RING_N;
    let at = |k: usize| {
        let a = std::f64::consts::TAU * (k % n) as f64 / n as f64;
        (add(well, polar_pt(rr, a)), [-libm::sin(a), libm::cos(a)])
    };
    let ways = &lanes[..];
    // (The last of a spur ending on the ring (`spur_well`) is not given way to: it ends at a node of it.)
    let by_spur = |s: &[P]| spur.iter().any(|&(a, b)| dist(a, s[0]) < 1e-6 && dist(b, s[1]) < 1e-6);
    let road_d = |p: P| (0..n_roads).flat_map(|j| ways[j].0.windows(2).filter(|s| !by_spur(s)).map(move |s| seg_dist(p, s[0], s[1]) - 0.5 * ways[j].1)).fold(f64::MAX, f64::min);
    let crosses_road = |a: P, b: P| (0..n_roads).any(|j| ways[j].0.windows(2).any(|s| !by_spur(s) && seg_cross(a, b, s[0], s[1]).is_some()));
    // (A point given way at, or the piece on from it.)
    let blocked: Vec<bool> = (0..n).map(|k| {
        let (p, q) = (at(k).0, at(k + 1).0);
        road_d(p) < 0.5 * LANE_W + RING_ROAD_FT || site.wet(p) || site.near_river(p, 8.0) || crosses_road(p, q)
    }).collect();
    let Some(b0) = blocked.iter().position(|&b| b) else {
        lanes.push(((0..=n).map(|k| at(k).0).collect(), LANE_W));
        return Vec::new();
    };
    let mut runs: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    for i in 1..=n {
        if blocked[(b0 + i) % n] {
            if cur.len() >= 2 {
                runs.push(std::mem::take(&mut cur));
            }
            cur.clear();
        } else {
            cur.push(b0 + i);
        }
    }
    // (A run's end on from `p` along `path` to road `j` at its last point (the road along `tg`
    // there): its last piece at a T's angle, over dry ground, across no lane nor another road.)
    let t_min = libm::sin(RING_T_DEG.to_radians());
    let to = |p: P, path: Vec<P>, j: usize, tg: P| -> Option<(usize, Vec<P>)> {
        let mut from = p;
        let last = path.len() - 1;
        for (i, &x) in path.iter().enumerate() {
            let v = sub(x, from);
            let vl = len(v);
            if vl < 1e-6 || vl > RING_REACH_FT {
                return None;
            }
            if i == last && cross(mul(v, 1.0 / vl), mul(tg, 1.0 / len(tg).max(1e-9))).abs() < t_min {
                return None;
            }
            let m = (vl / 4.0).ceil().max(1.0) as usize;
            let dry = (1..=m).all(|k| {
                let s = lerp(from, x, k as f64 / m as f64);
                !site.wet(s) && !site.near_river(s, 6.0)
            });
            let clear = ways.iter().enumerate().all(|(w, (q, _))| q.windows(2).all(|s| seg_cross(from, x, s[0], s[1]).is_none_or(|(t, _)| (w == j && i == last) || t * vl > vl - 1.0)));
            if !dry || !clear {
                return None;
            }
            from = x;
        }
        Some((j, path))
    };
    // (Where the roads cross the ring's circle: the angle there, the road, the point, its way.)
    let mut rims: Vec<(f64, usize, P, P)> = Vec::new();
    for j in 0..n_roads {
        for s in ways[j].0.windows(2) {
            let (w, e) = (sub(s[0], well), sub(s[1], s[0]));
            let (qa, qb, qc) = (dot(e, e), dot(w, e), dot(w, w) - rr * rr);
            let disc = qb * qb - qa * qc;
            if qa < 1e-9 || disc < 0.0 {
                continue;
            }
            for t in [(-qb - crate::core::sqrt(disc)) / qa, (-qb + crate::core::sqrt(disc)) / qa] {
                if (0.0..=1.0).contains(&t) {
                    let x = add(s[0], mul(e, t));
                    rims.push((libm::atan2(x[1] - well[1], x[0] - well[0]), j, x, e));
                }
            }
        }
    }
    // (From the ring's point `k` on (`dir` 1: anticlockwise) to a road: round the circle to where one
    // crosses it within 50° on, else straight on along its tangent to the first road it meets. The
    // points on from `k`.)
    let step = std::f64::consts::TAU / n as f64;
    let reach = |k: usize, dir: f64| -> Option<(usize, Vec<P>)> {
        let (p, t) = at(k);
        let a = step * (k % n) as f64;
        let on = |b: f64| (dir * (b - a)).rem_euclid(std::f64::consts::TAU);
        let rim = rims.iter().filter(|r| on(r.0) > 1e-6 && on(r.0) <= 50f64.to_radians()).min_by(|x, y| on(x.0).total_cmp(&on(y.0)));
        if let Some(&(b, j, x, e)) = rim {
            // (The ring's points short of it, then it.)
            let mut path: Vec<P> = (1..4i64).take_while(|&i| i as f64 * step < on(b) - 0.02).map(|i| at((k as i64 + dir as i64 * i).rem_euclid(n as i64) as usize).0).collect();
            path.push(x);
            if let Some(hit) = to(p, path, j, e) {
                return Some(hit);
            }
        }
        let d = mul(t, dir);
        let far = add(p, mul(d, RING_REACH_FT));
        let mut best: Option<(f64, usize, P, P)> = None;
        for j in 0..n_roads {
            for s in ways[j].0.windows(2) {
                if let Some((t, _)) = seg_cross(p, far, s[0], s[1])
                    && best.is_none_or(|b| t < b.0)
                {
                    best = Some((t, j, lerp(p, far, t), sub(s[1], s[0])));
                }
            }
        }
        best.and_then(|(_, j, x, tg)| to(p, vec![x], j, tg))
    };
    let mut out: Vec<Vec<P>> = Vec::new();
    // (Where each run reaches a road: its place in `out`, its far end or not, the road.)
    let mut ends: Vec<(usize, bool, usize)> = Vec::new();
    for run in runs {
        let m = run.len();
        let (mut lo, mut hi) = (0, m);
        let (mut head, mut tail) = (None, None);
        for k in 0..3.min(m - 1) {
            if let Some(h) = reach(run[k], -1.0) {
                (head, lo) = (Some(h), k);
                break;
            }
        }
        for k in 0..3 {
            if m - k < lo + 2 {
                break;
            }
            if let Some(h) = reach(run[m - 1 - k], 1.0) {
                (tail, hi) = (Some(h), m - k);
                break;
            }
        }
        let pts: Vec<P> = head.iter().flat_map(|h| h.1.iter().rev().copied()).chain(run[lo..hi].iter().map(|&k| at(k).0)).chain(tail.iter().flat_map(|h| h.1.iter().copied())).collect();
        // (A run reaching no road at either end, short: none; it would be left joined to nothing.)
        if pts.len() < 2 || length(&pts) < 40.0 || (head.is_none() && tail.is_none() && length(&pts) < RING_LONE_FT) {
            continue;
        }
        if let Some((j, _)) = head {
            ends.push((out.len(), false, j));
        }
        if let Some((j, _)) = tail {
            ends.push((out.len(), true, j));
        }
        out.push(pts);
    }
    // (Two runs reaching a road within 12 ft of each other: one crossroads half way.)
    let end_of = |out: &[Vec<P>], i: usize, tail: bool| if tail { out[i].len() - 1 } else { 0 };
    for a in 0..ends.len() {
        for b in a + 1..ends.len() {
            let ((ia, ta, ja), (ib, tb, jb)) = (ends[a], ends[b]);
            let (xa, xb) = (out[ia][end_of(&out, ia, ta)], out[ib][end_of(&out, ib, tb)]);
            if ja == jb && ia != ib && dist(xa, xb) < 12.0 && dist(xa, xb) > 1e-6 {
                let mid = lerp(xa, xb, 0.5);
                let x = ways[ja].0.windows(2).map(|s| nearest_on(mid, s[0], s[1])).min_by(|p, q| dist(*p, mid).total_cmp(&dist(*q, mid))).unwrap_or(mid);
                let (ka, kb) = (end_of(&out, ia, ta), end_of(&out, ib, tb));
                out[ia][ka] = x;
                out[ib][kb] = x;
            }
        }
    }
    let first = lanes.len();
    let mut reached = Vec::new();
    lanes.extend(out.into_iter().map(|pts| (pts, LANE_W)));
    for (i, tail, j) in ends {
        let li = first + i;
        let k = if tail { lanes[li].0.len() - 1 } else { 0 };
        let x = lanes[li].0[k];
        let x = node(lanes, j, x);
        lanes[li].0[k] = x;
        reached.push(x);
    }
    reached
}

/// Where `join_to` finds no way straight on: a lane ending at the last of `own` (lane `me`) run on to
/// a way within `JOIN_LATE_FT` in two pieces, a bend: the first turned from it by up to
/// `JOIN_TURN_DEG`, 8-25 ft, the second turned from that by as much, meeting the way at `T_DEG` or
/// squarer; each over dry ground, across no way, `clear` (the second short of the way). Not back onto
/// the way it leaves near its start. The way, the bend's point and the point on the way.
pub(super) fn bend_to(site: &Site, own: &[P], me: usize, lanes: &[(Vec<P>, f64)], clear: &dyn Fn(P, P, f64) -> bool) -> Option<(usize, P, P)> {
    let n = own.len();
    if n < 2 {
        return None;
    }
    let (p, p0) = (own[n - 1], own[0]);
    let d = sub(p, own[n - 2]);
    let d = mul(d, 1.0 / len(d).max(1e-9));
    let (turn, t_min) = (libm::cos(JOIN_TURN_DEG.to_radians()), libm::sin(T_DEG.to_radians()));
    let dry = |a: P, b: P| {
        let m = (dist(a, b) / 4.0).ceil().max(1.0) as usize;
        (1..=m).all(|k| {
            let s = lerp(a, b, k as f64 / m as f64);
            !site.wet(s) && !site.near_river(s, 6.0)
        })
    };
    let crosses = |a: P, b: P| {
        let vl = dist(a, b);
        lanes.iter().enumerate().any(|(i, (q, _))| i != me && q.windows(2).any(|s| seg_cross(a, b, s[0], s[1]).is_some_and(|(t, _)| t * vl > 0.5 && t * vl < vl - 1.0)))
            || own[..n - 1].windows(2).any(|s| seg_cross(a, b, s[0], s[1]).is_some_and(|(t, _)| t * vl > 0.5))
    };
    let mut cands: Vec<(f64, usize, P, P, f64)> = Vec::new();
    for (j, (q, w)) in lanes.iter().enumerate() {
        if j == me {
            continue;
        }
        let parent = q.windows(2).any(|s| seg_dist(p0, s[0], s[1]) <= 1.0);
        for s in q.windows(2) {
            let (a, b) = (s[0], s[1]);
            let el = dist(a, b);
            if el < 1e-6 || seg_dist(p, a, b) > JOIN_LATE_FT {
                continue;
            }
            let tu = mul(sub(b, a), 1.0 / el);
            let k = (el / 5.0).ceil() as usize;
            for x in std::iter::once(nearest_on(p, a, b)).chain((0..=k).map(|i| lerp(a, b, i as f64 / k as f64))) {
                if dist(p, x) > JOIN_LATE_FT + 10.0 || (parent && dist(x, p0) < 45.0) {
                    continue;
                }
                for deg in [15.0, 30.0, 45.0, -15.0, -30.0, -45.0] {
                    let u = rotate(d, f64::to_radians(deg));
                    for step in [8.0, 12.0, 18.0, 25.0] {
                        let m = add(p, mul(u, step));
                        let v = sub(x, m);
                        let vl = len(v);
                        if vl < 4.0 {
                            continue;
                        }
                        let v = mul(v, 1.0 / vl);
                        if dot(u, v) < turn || cross(v, tu).abs() < t_min {
                            continue;
                        }
                        cands.push((step + vl, j, m, x, 0.5 * w));
                    }
                }
            }
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ok = |m: P, x: P, hw: f64| dry(p, m) && dry(m, x) && !crosses(p, m) && !crosses(m, x) && clear(p, m, 0.0) && clear(m, x, hw);
    cands.into_iter().take(60).find(|&(_, _, m, x, hw)| ok(m, x, hw)).map(|(_, j, m, x, _)| (j, m, x))
}

/// A run of the ring (lane `li`, its points `pts` toward the end) left short of a road once the village
/// is laid out (`ring` gave way to a lane since cut back, or the run was cut back to its last house):
/// on round the circle to where a road crosses it within `RING_ON_DEG`, else along its last piece to
/// a road within `RING_REACH_FT` (at `RING_T_DEG` or squarer), as `ring`'s runs. Each piece over dry
/// ground, across no way but its road at its end, its band clear of the buildings and plots; the last
/// at `RING_T_LATE_DEG` or squarer to the road. The road and the points on, the last on it.
pub(super) fn ring_on(g: &Ground, l: &Layout, lanes: &[(Vec<P>, f64)], li: usize, n_roads: usize, pts: &[P]) -> Option<(usize, Vec<P>)> {
    let (well, green) = g.green;
    let n = pts.len();
    if green <= 0.0 || n < 2 || !on_ring(pts, well, green + 0.5 * LANE_W) {
        return None;
    }
    let rr = green + 0.5 * LANE_W;
    let (e, prev) = (pts[n - 1], pts[n - 2]);
    let r_e = sub(e, well);
    let dir = if cross(r_e, sub(e, prev)) >= 0.0 { 1.0 } else { -1.0 };
    let a0 = libm::atan2(r_e[1], r_e[0]);
    let on = |b: f64| (dir * (b - a0)).rem_euclid(std::f64::consts::TAU);
    // (A piece from `a` to `b`, the last onto road `j` (along `tg`) or none.)
    let piece_ok = |a: P, b: P, last: Option<(usize, P)>| {
        let v = sub(b, a);
        let vl = len(v);
        if vl < 1e-6 {
            return false;
        }
        let u = mul(v, 1.0 / vl);
        if let Some((_, tg)) = last
            && cross(u, mul(tg, 1.0 / len(tg).max(1e-9))).abs() < libm::sin(RING_T_LATE_DEG.to_radians())
        {
            return false;
        }
        let m = (vl / 4.0).ceil().max(1.0) as usize;
        // (Onto a road: up to its bridge's foot, the bank by the road.)
        let bank = if last.is_some() { 1.0 } else { 6.0 };
        let dry = (1..=m).all(|k| {
            let s = lerp(a, b, k as f64 / m as f64);
            !g.site.wet(s) && !g.site.near_river(s, bank)
        });
        let onto = |w: usize| last.is_some_and(|l| l.0 == w);
        let past = |w: usize, s: &[P]| seg_cross(a, b, s[0], s[1]).is_none_or(|(t, _)| onto(w) || t * vl < 0.5 || t * vl > vl - 1.0);
        let clear = lanes.iter().enumerate().all(|(w, (q, _))| w == li || q.windows(2).all(|s| past(w, s)));
        let short = last.map_or(0.0, |(j, _)| 0.5 * lanes[j].1 + 2.0);
        let band = if vl - short > 0.5 { quad(a, u, LANE_W, vl - short) } else { Vec::new() };
        dry && clear && g.lane_off_green(a, b) && (band.is_empty() || (!building_hit(l, &band, None) && !g.plot_hit(&band)))
    };
    // (Where the roads cross the circle ahead within `RING_ON_DEG`, the nearest first.)
    let mut rims: Vec<(f64, usize, P, P)> = Vec::new();
    for j in 0..n_roads {
        for s in lanes[j].0.windows(2) {
            let (w, ev) = (sub(s[0], well), sub(s[1], s[0]));
            let (qa, qb, qc) = (dot(ev, ev), dot(w, ev), dot(w, w) - rr * rr);
            let disc = qb * qb - qa * qc;
            if qa < 1e-9 || disc < 0.0 {
                continue;
            }
            for t in [(-qb - crate::core::sqrt(disc)) / qa, (-qb + crate::core::sqrt(disc)) / qa] {
                if (0.0..=1.0).contains(&t) {
                    let x = add(s[0], mul(ev, t));
                    let b = libm::atan2(x[1] - well[1], x[0] - well[0]);
                    if on(b) > 1e-3 && on(b) <= RING_ON_DEG.to_radians() {
                        rims.push((on(b), j, x, ev));
                    }
                }
            }
        }
    }
    rims.sort_by(|a, b| a.0.total_cmp(&b.0));
    let step = std::f64::consts::TAU / RING_N as f64;
    // (The ring's own points on from `e` short of `upto` (radians on), on its lattice.)
    let lattice = |upto: f64| -> Vec<P> {
        let k0 = if dir > 0.0 { crate::core::floor(a0 / step) + 1.0 } else { crate::core::ceil(a0 / step) - 1.0 };
        (0..RING_N as i64).map(|i| (k0 + dir * i as f64) * step).take_while(|&a| on(a) < upto - 0.02).filter(|&a| on(a) > 0.02).map(|a| add(well, polar_pt(rr, a))).collect()
    };
    // (A path on: every piece fine, the last onto road `j` along `tg`.)
    let path_ok = |path: &[P], j: usize, tg: P| {
        let mut from = e;
        path.iter().enumerate().all(|(i, &q)| {
            let ok = piece_ok(from, q, (i == path.len() - 1).then_some((j, tg)));
            from = q;
            ok
        })
    };
    if let Some(&(b, j, x, tg)) = rims.first() {
        let mut path = lattice(b);
        path.push(x);
        if path_ok(&path, j, tg) {
            return Some((j, path));
        }
        // (Else off the ring short of it to the road's nearest point, turned by up to `JOIN_TURN_DEG`:
        // a bridge's foot where the road crosses the river there.)
        let ring_pts = &path[..path.len() - 1];
        for k in (0..=ring_pts.len()).rev() {
            let (q, back) = if k == 0 { (e, prev) } else { (ring_pts[k - 1], if k == 1 { e } else { ring_pts[k - 2] }) };
            let Some((f, ftg)) = lanes[j].0.windows(2).map(|s| (nearest_on(q, s[0], s[1]), sub(s[1], s[0]))).min_by(|x, y| dist(q, x.0).total_cmp(&dist(q, y.0))) else { continue };
            let (u, v) = (sub(q, back), sub(f, q));
            if len(v) > RING_REACH_FT || len(v) < 1.0 || dot(u, v) < libm::cos(JOIN_TURN_DEG.to_radians()) * len(u) * len(v) {
                continue;
            }
            let mut path: Vec<P> = ring_pts[..k].to_vec();
            path.push(f);
            if path_ok(&path, j, ftg) {
                return Some((j, path));
            }
        }
    }
    // (Straight on along its last piece to the first road it meets.)
    let d = mul(sub(e, prev), 1.0 / dist(e, prev).max(1e-9));
    let far = add(e, mul(d, RING_REACH_FT));
    let mut best: Option<(f64, usize, P, P)> = None;
    for j in 0..n_roads {
        for s in lanes[j].0.windows(2) {
            if let Some((t, _)) = seg_cross(e, far, s[0], s[1])
                && t * RING_REACH_FT > 0.5
                && best.is_none_or(|b| t < b.0)
            {
                best = Some((t, j, lerp(e, far, t), sub(s[1], s[0])));
            }
        }
    }
    let (_, j, x, tg) = best?;
    (cross(d, mul(tg, 1.0 / len(tg).max(1e-9))).abs() >= libm::sin(RING_T_DEG.to_radians()) && piece_ok(e, x, Some((j, tg)))).then(|| (j, vec![x]))
}

/// The lanes ending by the green (cut `RING_CLEAR_FT` outside the ring by `outside`, or left there)
/// run on to it (`ring`, lanes `ring`): straight on, else turned toward the green's middle by up to
/// 45°, to meet it at `RING_T_DEG` or squarer within `RING_REACH_FT`, over dry ground and across no
/// way; else the same from up to 40 ft back along it (a lane running by the green, not at it); else
/// all that at `RING_T_LATE_DEG`. A node
/// of the ring's there. Else it is cut back to `RING_OFF_FT` outside the ring (`off_rim`: no house is
/// laid by its end on the green's rim), or dropped where too little is left and no lane meets it;
/// else left to `settle_ends`.
pub(super) fn to_ring(site: &Site, well: P, rr: f64, lanes: &mut [(Vec<P>, f64)], n_roads: usize, ring: std::ops::Range<usize>) {
    let rc = rr + RING_CLEAR_FT;
    for li in n_roads..ring.start {
        for tail in [false, true] {
            if lanes[li].0.len() < 2 {
                continue;
            }
            let mut pts = lanes[li].0.clone();
            if !tail {
                pts.reverse();
            }
            let e = pts[pts.len() - 1];
            if dist(e, well) > rc + 10.0 || on_way(lanes, li, e) {
                continue;
            }
            let total = length(&pts);
            // (Where other lanes meet it: it is not cut back past them.)
            let junction = lanes.iter().enumerate().filter(|&(j, (q, _))| j != li && q.len() >= 2).flat_map(|(_, (q, _))| [q[0], q[q.len() - 1]]).filter(|&a| pts.windows(2).any(|s| seg_dist(a, s[0], s[1]) <= 1.0)).map(|a| arc_at(&pts, a)).fold(0.0, f64::max);
            let mut found = None;
            'trim: for (t_deg, back) in [RING_T_DEG, RING_T_LATE_DEG].into_iter().flat_map(|t| [0.0, 10.0, 20.0, 30.0, 40.0].map(|b| (t, b))) {
                let t_min = libm::sin(t_deg.to_radians());
                let keep = total - back;
                if back > 0.0 && (keep < 25.0 || keep < junction + 1.0) {
                    continue;
                }
                let cut = if back > 0.0 { cut_at(&pts, keep) } else { pts.clone() };
                let n = cut.len();
                let (e, prev) = (cut[n - 1], cut[n - 2]);
                if dist(e, prev) < 1e-6 {
                    continue;
                }
                let d = mul(sub(e, prev), 1.0 / dist(e, prev));
                let inward = mul(sub(well, e), 1.0 / dist(well, e));
                let side = if cross(d, inward) >= 0.0 { 1.0 } else { -1.0 };
                let gap = libm::acos(dot(d, inward).clamp(-1.0, 1.0));
                for turn in [0.0, 10.0, 20.0, 30.0, 40.0, 45.0] {
                    let dir = rotate(d, side * f64::to_radians(turn).min(gap));
                    // (Where it meets the ring: |e + t dir - well| = rr, the nearer.)
                    let w = sub(e, well);
                    let (b, c) = (dot(w, dir), dot(w, w) - rr * rr);
                    let disc = b * b - c;
                    if disc < 0.0 {
                        continue;
                    }
                    let t = -b - crate::core::sqrt(disc);
                    if t <= 0.0 || t > RING_REACH_FT {
                        continue;
                    }
                    let y = add(e, mul(dir, t));
                    let tg = [-(y[1] - well[1]) / rr, (y[0] - well[0]) / rr];
                    if cross(dir, tg).abs() < t_min {
                        continue;
                    }
                    let Some((rj, y)) = ring.clone().flat_map(|j| lanes[j].0.windows(2).map(move |s| (j, nearest_on(y, s[0], s[1])))).filter(|q| dist(q.1, y) < 1.5).min_by(|p, q| dist(p.1, y).total_cmp(&dist(q.1, y))) else { continue };
                    let vl = dist(e, y);
                    let m = (vl / 4.0).ceil().max(1.0) as usize;
                    let dry = (1..=m).all(|i| {
                        let s = lerp(e, y, i as f64 / m as f64);
                        !site.wet(s) && !site.near_river(s, 6.0)
                    });
                    let clear = lanes.iter().enumerate().all(|(j, (q, _))| j == li || q.windows(2).all(|s| seg_cross(e, y, s[0], s[1]).is_none_or(|(t, _)| t * vl > vl - 1.0)));
                    if dry && clear {
                        found = Some((cut, rj, y));
                        break 'trim;
                    }
                }
            }
            if let Some((mut cut, rj, y)) = found {
                let x = node(lanes, rj, y);
                cut.push(x);
                if !tail {
                    cut.reverse();
                }
                lanes[li].0 = cut;
            } else if let Some(mut cut) = off_rim(&pts, well, rr, junction) {
                // (It cannot meet the ring: back out of the green's reach, no house laid by its rim.)
                if !tail {
                    cut.reverse();
                }
                lanes[li].0 = cut;
            } else if junction <= 0.0 && dist(e, well) <= rr + RING_OFF_FT {
                // (Too little of it off the green's rim, nothing on it: dropped.)
                lanes[li].0.clear();
            }
        }
    }
}

/// A lane by the green (`pts` toward its end) that cannot meet the ring (centre line `rr` round `well`)
/// cut back to where it is `RING_OFF_FT` outside it, but not short of its last junction (`junction` ft
/// along) nor to under 25 ft: the lane then, if cut.
fn off_rim(pts: &[P], well: P, rr: f64, junction: f64) -> Option<Vec<P>> {
    let total = length(pts);
    let mut at = total;
    while at > 0.0 && dist(point_along(pts, at).0, well) <= rr + RING_OFF_FT {
        at -= 2.0;
    }
    (at < total && at >= 25.0 && at >= junction + 1.0).then(|| cut_at(pts, at))
}
