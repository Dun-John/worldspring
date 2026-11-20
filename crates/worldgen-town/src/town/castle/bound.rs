//! The castle ground: wall runs and road cuts, rays and cut-outs, and tidying the
//! outline.

use super::*;

/// The side `a`-`b` of a castle runs along a town wall standing within `WALL_SIDE` of it, the whole
/// way (but through the passage of a gate at its end: `gates`, and the water gap where a stream
/// crosses it: `wet`).
pub(super) fn along_wall(walls: &[(P, P)], gates: &[P], wet: &dyn Fn(P) -> bool, a: P, b: P) -> bool {
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
pub(super) fn cut_spikes(ring: &[P], along: &dyn Fn(P, P) -> bool) -> Vec<P> {
    let mut pts = ring.to_vec();
    // (A spike tipped by a side under `SPIKE_TIP` ft, both its corners turning in: that side made
    // one corner first, so the spike is seen; the corner on a side along the town wall (`along`)
    // kept, else the side's middle; none where both sides beside it run along the wall.)
    const SPIKE_TIP: f64 = 15.0;
    for _ in 0..ring.len() {
        let m = pts.len();
        if m <= 3 {
            break;
        }
        let turns = |k: usize| cross(sub(pts[k], pts[(k + m - 1) % m]), sub(pts[(k + 1) % m], pts[k])) > 0.0;
        let Some(k) = (0..m).find(|&k| dist(pts[k], pts[(k + 1) % m]) < SPIKE_TIP && turns(k) && turns((k + 1) % m)) else { break };
        let (wa, wc) = (along(pts[(k + m - 1) % m], pts[k]), along(pts[(k + 1) % m], pts[(k + 2) % m]));
        if wa && wc {
            break;
        }
        let mut next = pts.clone();
        next[k] = if wa { pts[k] } else if wc { pts[(k + 1) % m] } else { lerp(pts[k], pts[(k + 1) % m], 0.5) };
        next.remove((k + 1) % m);
        if !simple(&next) {
            break;
        }
        pts = next;
    }
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
pub(super) fn meet_wall(ring: &[P], along: &dyn Fn(P, P) -> bool, room: &dyn Fn(P) -> bool) -> Vec<P> {
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
pub(super) fn road_through(roads: &[(Vec<P>, u8, f64)], ring: &[P]) -> Option<(P, P, f64)> {
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
pub(super) fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (o3, o4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

/// Whether point `p` lies inside a plain polygon, and how far from its outline (ft; negative
/// outside).
pub(super) fn inset_depth(poly: &[P], p: P) -> f64 {
    let m = poly.len();
    let d = (0..m).map(|k| seg_dist(p, poly[k], poly[(k + 1) % m])).fold(f64::MAX, f64::min);
    if contains(poly, p) { d } else { -d }
}

/// How far from `o` along unit `d` the ray first crosses the outline of `poly` (ft).
pub(super) fn ray_exit(poly: &[P], o: P, d: P) -> f64 {
    ray_hit(poly, o, d).0
}

/// `ray_exit`, and the edge it crosses there.
pub(super) fn ray_hit(poly: &[P], o: P, d: P) -> (f64, usize) {
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

/// The pieces of polyline `line` outside convex `poly` (2 points at least each).
pub(super) fn cut_out(line: &[P], poly: &[P]) -> Vec<Vec<P>> {
    let ccw = area(poly) > 0.0;
    let m = poly.len();
    // (The stretch of a segment inside the polygon: its parameters, if any.)
    let clip = |a: P, b: P| -> Option<(f64, f64)> {
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let d = sub(b, a);
        for k in 0..m {
            let (p, q) = (poly[k], poly[(k + 1) % m]);
            let e = sub(q, p);
            let s = if ccw { 1.0 } else { -1.0 };
            // (Inside: to the left of each edge, anticlockwise.)
            let num = s * cross(e, sub(a, p));
            let den = s * cross(e, d);
            if den.abs() < 1e-12 {
                if num < 0.0 {
                    return None;
                }
            } else {
                let t = -num / den;
                if den > 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
            }
        }
        (t1 - t0 > 1e-9).then_some((t0, t1))
    };
    let mut out: Vec<Vec<P>> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    for s in line.windows(2) {
        let (a, b) = (s[0], s[1]);
        if cur.is_empty() && clip(a, a).is_none() {
            cur.push(a);
        }
        match clip(a, b) {
            None => {
                if cur.is_empty() {
                    cur.push(a);
                }
                cur.push(b);
            }
            Some((t0, t1)) => {
                if t0 > 1e-9 {
                    if cur.is_empty() {
                        cur.push(a);
                    }
                    cur.push(lerp(a, b, t0));
                }
                if cur.len() >= 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                if t1 < 1.0 - 1e-9 {
                    cur = vec![lerp(a, b, t1), b];
                }
            }
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

/// Rays from `o`: evenly every 5 degrees, and at each corner of `poly` (so a ring keeping to it
/// keeps its corners), in turning order.
pub(super) fn rays(poly: &[P], o: P) -> Vec<f64> {
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
pub(super) fn polar(o: P, a: &[f64], r: &[f64]) -> Vec<P> {
    a.iter().zip(r).map(|(&t, &r)| add(o, [r * libm::cos(t), r * libm::sin(t)])).collect()
}

/// `r` smoothed round the ring (a quarter each side, twice), each kept within `max`.
pub(super) fn smooth(r: &[f64], max: &[f64]) -> Vec<f64> {
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
pub(super) fn tidy(ring: &[P], fixed: &[P]) -> Option<Vec<P>> {
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
pub(super) fn simplify_closed(ring: &[P], tol: f64) -> Vec<P> {
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
