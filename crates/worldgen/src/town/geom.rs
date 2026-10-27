//! Small 2D polygon toolkit for settlement layouts (all f64 ft, counter-clockwise or not —
//! functions that care say so). Convex polygons are the common case: Voronoi patches and the
//! lots cut from them stay convex.

pub type P = [f64; 2];

#[inline]
pub fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
#[inline]
pub fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
#[inline]
pub fn mul(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s]
}
#[inline]
pub fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
#[inline]
pub fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
#[inline]
pub fn len(a: P) -> f64 {
    crate::core::sqrt(dot(a, a))
}
#[inline]
pub fn dist(a: P, b: P) -> f64 {
    len(sub(a, b))
}
#[inline]
pub fn lerp(a: P, b: P, t: f64) -> P {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Signed area (positive when counter-clockwise in a y-up frame).
pub fn area(poly: &[P]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| cross(poly[i], poly[(i + 1) % n])).sum::<f64>() * 0.5
}

pub fn centroid(poly: &[P]) -> P {
    let n = poly.len();
    let a = area(poly);
    if a.abs() < 1e-9 {
        let s = poly.iter().fold([0.0, 0.0], |acc, p| add(acc, *p));
        return mul(s, 1.0 / n.max(1) as f64);
    }
    let mut c = [0.0, 0.0];
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let f = cross(p, q);
        c[0] += (p[0] + q[0]) * f;
        c[1] += (p[1] + q[1]) * f;
    }
    mul(c, 1.0 / (6.0 * a))
}

/// The mean of a polygon's corners: a box's middle, exact far from the origin (where `centroid`'s
/// products lose feet on a small one).
pub fn mean(poly: &[P]) -> P {
    let s = poly.iter().fold([0.0, 0.0], |acc, p| add(acc, *p));
    mul(s, 1.0 / poly.len().max(1) as f64)
}

pub fn contains(poly: &[P], p: P) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Distance from `p` to segment `ab`.
pub fn seg_dist(p: P, a: P, b: P) -> f64 {
    let ab = sub(b, a);
    let t = (dot(sub(p, a), ab) / dot(ab, ab).max(1e-12)).clamp(0.0, 1.0);
    dist(p, add(a, mul(ab, t)))
}

/// The least distance between the outlines of two polygons (0 where they cross).
pub fn poly_gap(a: &[P], b: &[P]) -> f64 {
    let o = |p: P, q: P, r: P| cross(sub(q, p), sub(r, p));
    let mut best = f64::MAX;
    for i in 0..a.len() {
        let (p, q) = (a[i], a[(i + 1) % a.len()]);
        for j in 0..b.len() {
            let (r, t) = (b[j], b[(j + 1) % b.len()]);
            if o(p, q, r) * o(p, q, t) < 0.0 && o(r, t, p) * o(r, t, q) < 0.0 {
                return 0.0;
            }
            best = best.min(seg_dist(p, r, t)).min(seg_dist(q, r, t)).min(seg_dist(r, p, q)).min(seg_dist(t, p, q));
        }
    }
    best
}

/// A polygon whose edges carry a label (edge i runs from vertex i to i + 1).
#[derive(Clone, Debug)]
pub struct Labeled {
    pub pts: Vec<P>,
    pub labels: Vec<i32>,
}

/// Keep the part of a convex polygon where `dot(p, n) <= c`; the new edge along the line gets
/// `label`. Works for any simple polygon clipped by a half-plane, keeping one piece.
pub fn clip(poly: &Labeled, n: P, c: f64, label: i32) -> Labeled {
    let m = poly.pts.len();
    let mut out = Labeled { pts: Vec::with_capacity(m + 1), labels: Vec::with_capacity(m + 1) };
    for i in 0..m {
        let (a, b) = (poly.pts[i], poly.pts[(i + 1) % m]);
        let (da, db) = (dot(a, n) - c, dot(b, n) - c);
        if da <= 0.0 {
            out.pts.push(a);
            out.labels.push(poly.labels[i]);
        }
        if (da <= 0.0) != (db <= 0.0) {
            let t = da / (da - db);
            out.pts.push(lerp(a, b, t));
            // Leaving the kept side starts the cut edge (to the entry point); entering
            // continues the rest of the original edge.
            out.labels.push(if da <= 0.0 { label } else { poly.labels[i] });
        }
    }
    out
}

/// Keep the part of a plain polygon where `dot(p, n) <= c`.
pub fn clip_plain(poly: &[P], n: P, c: f64) -> Vec<P> {
    let lp = Labeled { pts: poly.to_vec(), labels: vec![-1; poly.len()] };
    clip(&lp, n, c, -1).pts
}

/// Shrink a convex polygon by moving every edge inward by its own distance (`d[i]` for edge
/// i). Returns an empty polygon when it collapses.
pub fn inset(poly: &[P], d: &[f64]) -> Vec<P> {
    let m = poly.len();
    if m < 3 {
        return Vec::new();
    }
    let ccw = area(poly) > 0.0;
    let mut cur = poly.to_vec();
    for i in 0..m {
        let (a, b) = (poly[i], poly[(i + 1) % m]);
        let e = sub(b, a);
        let l = len(e);
        if l < 1e-9 {
            continue;
        }
        // Outward normal.
        let nrm = if ccw { [e[1] / l, -e[0] / l] } else { [-e[1] / l, e[0] / l] };
        cur = clip_plain(&cur, nrm, dot(a, nrm) - d[i]);
        if cur.len() < 3 {
            return Vec::new();
        }
    }
    cur
}

/// Split a convex polygon by the line through `p` with direction `dir`.
pub fn split(poly: &[P], p: P, dir: P) -> (Vec<P>, Vec<P>) {
    let n = [-dir[1], dir[0]];
    let c = dot(p, n);
    (clip_plain(poly, n, c), clip_plain(poly, mul(n, -1.0), -c))
}

/// Longest edge direction (unit) of a polygon — lots are cut across it.
pub fn main_axis(poly: &[P]) -> P {
    let m = poly.len();
    let mut best = ([1.0, 0.0], 0.0);
    for i in 0..m {
        let e = sub(poly[(i + 1) % m], poly[i]);
        let l = len(e);
        if l > best.1 {
            best = (mul(e, 1.0 / l), l);
        }
    }
    best.0
}

/// Extent of a polygon along a unit direction.
pub fn extent(poly: &[P], dir: P) -> (f64, f64) {
    poly.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(dot(*p, dir)), hi.max(dot(*p, dir))))
}

/// Minimum-area oriented bounding box of a convex polygon (edge-aligned, rotating calipers).
pub struct Obb {
    /// Unit direction of the long side.
    pub axis: P,
    pub long: f64,
    pub short: f64,
}

pub fn obb(poly: &[P]) -> Obb {
    let m = poly.len();
    let mut best = Obb { axis: [1.0, 0.0], long: 0.0, short: 0.0 };
    let mut best_area = f64::MAX;
    for i in 0..m {
        let e = sub(poly[(i + 1) % m], poly[i]);
        let l = len(e);
        if l < 1e-9 {
            continue;
        }
        let u = mul(e, 1.0 / l);
        let v = [-u[1], u[0]];
        let (a0, a1) = extent(poly, u);
        let (b0, b1) = extent(poly, v);
        let (w, h) = (a1 - a0, b1 - b0);
        if w * h < best_area {
            best_area = w * h;
            best = if w >= h { Obb { axis: u, long: w, short: h } } else { Obb { axis: v, long: h, short: w } };
        }
    }
    best
}

/// Drop repeated and collinear vertices (clipping leaves them where a vertex lies on the
/// cut line), so vertex counts mean real corners.
pub fn clean(poly: &[P]) -> Vec<P> {
    let mut pts: Vec<P> = Vec::with_capacity(poly.len());
    for p in poly {
        if pts.last().is_none_or(|q| dist(*q, *p) > 0.05) {
            pts.push(*p);
        }
    }
    while pts.len() > 1 && dist(pts[0], *pts.last().unwrap()) <= 0.05 {
        pts.pop();
    }
    let mut changed = true;
    while changed && pts.len() > 3 {
        changed = false;
        let m = pts.len();
        for i in 0..m {
            let (a, b, c) = (pts[(i + m - 1) % m], pts[i], pts[(i + 1) % m]);
            let (ab, bc) = (sub(b, a), sub(c, b));
            if cross(ab, bc).abs() <= 1e-3 * len(ab) * len(bc) {
                pts.remove(i);
                changed = true;
                break;
            }
        }
    }
    pts
}

/// The convex hull of points, anticlockwise (in axes with y up), without collinear points.
pub fn hull(mut pts: Vec<P>) -> Vec<P> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let mut out: Vec<P> = Vec::with_capacity(pts.len() + 1);
    for pass in 0..2 {
        let start = out.len();
        let it: Box<dyn Iterator<Item = &P>> = if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
        for &p in it {
            while out.len() >= start + 2 && cross(sub(out[out.len() - 1], out[out.len() - 2]), sub(p, out[out.len() - 1])) <= 0.0 {
                out.pop();
            }
            out.push(p);
        }
        out.pop();
    }
    out
}

/// Regular polygon approximating a circle.
pub fn circle(c: P, r: f64, n: usize) -> Vec<P> {
    (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)]
        })
        .collect()
}

/// The outline of the union of overlapping convex polygons (each wound either way), anticlockwise
/// (in axes with y up): the pieces of their edges outside every other one (an edge another runs
/// along the same way kept once, two running opposite ways along each other dropped), joined end
/// to end. None unless they make one loop (apart, or round a hole).
pub fn union_outline(parts: &[Vec<P>]) -> Option<Vec<P>> {
    const EPS: f64 = 1e-6;
    let parts: Vec<Vec<P>> = parts.iter().filter(|p| p.len() >= 3).map(|p| if area(p) < 0.0 { p.iter().rev().copied().collect() } else { p.clone() }).collect();
    let edges = |q: &Vec<P>| (0..q.len()).map(|k| (q[k], q[(k + 1) % q.len()])).collect::<Vec<_>>();
    // How far `p` lies inside convex `q` (its least distance in from an edge; negative outside).
    let depth = |q: &Vec<P>, p: P| edges(q).iter().map(|&(a, b)| cross(sub(b, a), sub(p, a)) / len(sub(b, a)).max(1e-12)).fold(f64::MAX, f64::min);
    let mut segs: Vec<(P, P)> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        for (a, b) in edges(p) {
            let ab = sub(b, a);
            let l2 = dot(ab, ab);
            if l2 < EPS {
                continue;
            }
            // Where it meets the others' edges, and where their corners lie on it.
            let mut ts = vec![0.0, 1.0];
            for (j, q) in parts.iter().enumerate() {
                if j == i {
                    continue;
                }
                for (c, d) in edges(q) {
                    let cd = sub(d, c);
                    let den = cross(ab, cd);
                    if den.abs() > EPS {
                        let t = cross(sub(c, a), cd) / den;
                        let u = cross(sub(c, a), ab) / den;
                        if t > 0.0 && t < 1.0 && (-EPS..=1.0 + EPS).contains(&u) {
                            ts.push(t);
                        }
                    }
                    for v in [c, d] {
                        let t = dot(sub(v, a), ab) / l2;
                        if t > 0.0 && t < 1.0 && cross(ab, sub(v, a)).abs() / crate::core::sqrt(l2) < 1e-4 {
                            ts.push(t);
                        }
                    }
                }
            }
            ts.sort_by(f64::total_cmp);
            for w in ts.windows(2) {
                if w[1] - w[0] < 1e-9 {
                    continue;
                }
                let mid = lerp(a, b, 0.5 * (w[0] + w[1]));
                let keep = parts.iter().enumerate().all(|(j, q)| {
                    if j == i {
                        return true;
                    }
                    let d = depth(q, mid);
                    if d > 1e-4 {
                        return false;
                    }
                    if d < -1e-4 {
                        return true;
                    }
                    // On one of its edges: kept by the first of two running the same way.
                    !edges(q).iter().any(|&(c, e)| {
                        let ce = sub(e, c);
                        seg_dist(mid, c, e) < 1e-4 && (dot(ce, ab) < 0.0 || j < i)
                    })
                });
                if keep {
                    segs.push((lerp(a, b, w[0]), lerp(a, b, w[1])));
                }
            }
        }
    }
    if segs.is_empty() {
        return None;
    }
    // Joined end to end: each piece's end is the next one's start.
    let n = segs.len();
    let mut used = vec![false; n];
    let mut out = vec![segs[0].0];
    let mut cur = 0;
    used[0] = true;
    for _ in 1..n {
        let end = segs[cur].1;
        let next = (0..n).filter(|&k| !used[k]).min_by(|&x, &y| dist(segs[x].0, end).total_cmp(&dist(segs[y].0, end)))?;
        if dist(segs[next].0, end) > 1e-3 {
            return None;
        }
        out.push(segs[next].0);
        used[next] = true;
        cur = next;
    }
    if dist(segs[cur].1, segs[0].0) > 1e-3 {
        return None;
    }
    let out = clean(&out);
    (out.len() >= 3 && area(&out) > 0.0).then_some(out)
}
