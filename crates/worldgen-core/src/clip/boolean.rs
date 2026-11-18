use super::*;

// ---------------------------------------------------------------------------------------------
// The boolean engine.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    Union,
    Inter,
    Diff,
}

/// An input edge of operand `k`, running with its ring.
struct Seg {
    a: I,
    b: I,
    k: usize,
}

/// A piece of the arrangement, `a < b`, with each operand's winding change crossing it from its
/// right to its left (going a->b).
#[derive(Clone, Copy)]
struct Frag {
    a: I,
    b: I,
    w: [i32; 2],
}

pub(super) fn run(a: &[Shape], b: &[Shape], op: Op) -> Vec<Shape> {
    let mut segs: Vec<Seg> = Vec::new();
    if !add_set(&mut segs, a, 0) || !add_set(&mut segs, b, 1) || segs.is_empty() {
        return Vec::new();
    }
    let fr = snap(&segs);
    let sd = sides(&fr);
    let inside = |w: [i32; 2]| match op {
        Op::Union => w[0] > 0 || w[1] > 0,
        Op::Inter => w[0] > 0 && w[1] > 0,
        Op::Diff => w[0] > 0 && w[1] <= 0,
    };
    let mut edges: Vec<(I, I)> = Vec::new();
    for (f, (l, r)) in fr.iter().zip(sd) {
        match (inside(l), inside(r)) {
            (true, false) => edges.push((f.a, f.b)),
            (false, true) => edges.push((f.b, f.a)),
            _ => {}
        }
    }
    assemble(link(edges))
}

/// The edges of a set as operand `k`, each ring turned so the positive winding rule reads it: outers
/// anticlockwise, holes clockwise. False when a point is invalid.
fn add_set(segs: &mut Vec<Seg>, set: &[Shape], k: usize) -> bool {
    for s in set {
        for (h, ring) in std::iter::once(&s.outer).chain(s.holes.iter()).enumerate() {
            let Some(mut r) = grid_ring(ring) else { return false };
            if r.len() < 2 {
                continue;
            }
            let a = area2(&r);
            if (h == 0 && a < 0) || (h > 0 && a > 0) {
                r.reverse();
            }
            let n = r.len();
            for i in 0..n {
                let (p, q) = (r[i], r[(i + 1) % n]);
                if p != q {
                    segs.push(Seg { a: p, b: q, k });
                }
            }
        }
    }
    true
}

/// Where two edges cross at a point inside both, rounded to the grid.
fn crossing(p: I, q: I, c: I, d: I) -> Option<I> {
    let (o1, o2) = (orient(p, q, c).signum(), orient(p, q, d).signum());
    let (o3, o4) = (orient(c, d, p).signum(), orient(c, d, q).signum());
    if o1 * o2 >= 0 || o3 * o4 >= 0 {
        return None;
    }
    let (r, s) = (isub(q, p), isub(d, c));
    let (mut den, mut num) = (icross(r, s), icross(isub(c, p), s));
    if den < 0 {
        den = -den;
        num = -num;
    }
    Some([rdiv(p[0] as i128 * den + r[0] as i128 * num, den), rdiv(p[1] as i128 * den + r[1] as i128 * num, den)])
}

/// Whether edge a->b meets the half-open grid square [h - 1/2, h + 1/2) (exact, in doubled units).
fn pixel_hit(a: I, b: I, h: I) -> bool {
    if a == h || b == h {
        return true;
    }
    let (px, py) = (2 * a[0] as i128, 2 * a[1] as i128);
    let (dx, dy) = (2 * (b[0] - a[0]) as i128, 2 * (b[1] - a[1]) as i128);
    let (l, r) = (2 * h[0] as i128 - 1, 2 * h[0] as i128 + 1);
    let (bo, t) = (2 * h[1] as i128 - 1, 2 * h[1] as i128 + 1);
    // Liang-Barsky on the closed square: the edge's parameter runs over [n0/d0, n1/d1].
    let (mut n0, mut d0, mut n1, mut d1) = (0i128, 1i128, 1i128, 1i128);
    for (pk, qk) in [(-dx, px - l), (dx, r - px), (-dy, py - bo), (dy, t - py)] {
        if pk == 0 {
            if qk < 0 {
                return false;
            }
        } else if pk < 0 {
            let (n, d) = (-qk, -pk);
            if n * d0 > n0 * d {
                n0 = n;
                d0 = d;
            }
        } else if qk * d1 < n1 * pk {
            n1 = qk;
            d1 = pk;
        }
    }
    if n0 * d1 > n1 * d0 {
        return false;
    }
    if n0 * d1 == n1 * d0 {
        // One point: inside unless on the square's right or top side.
        return px * d0 + n0 * dx < r * d0 && py * d0 + n0 * dy < t * d0;
    }
    // A stretch: outside only when it runs along the right or top side.
    !((dx == 0 && px == r) || (dy == 0 && py == t))
}

/// The arrangement of all edges: split at crossings by snap rounding, merged where they coincide.
fn snap(segs: &[Seg]) -> Vec<Frag> {
    let n = segs.len();
    let mut hot: Vec<I> = Vec::with_capacity(n + n / 2);
    for s in segs {
        hot.push(s.a);
    }
    // Crossings: each edge against those starting before it ends, in x.
    let mut ord: Vec<(i64, i64, i64, i64, usize)> = segs
        .iter()
        .enumerate()
        .map(|(i, s)| (s.a[0].min(s.b[0]), s.a[0].max(s.b[0]), s.a[1].min(s.b[1]), s.a[1].max(s.b[1]), i))
        .collect();
    ord.sort_unstable();
    for (oi, &(_, x1, y0, y1, i)) in ord.iter().enumerate() {
        let s = &segs[i];
        for &(tx0, _, ty0, ty1, j) in &ord[oi + 1..] {
            if tx0 > x1 {
                break;
            }
            if ty1 < y0 || ty0 > y1 {
                continue;
            }
            let t = &segs[j];
            if let Some(p) = crossing(s.a, s.b, t.a, t.b) {
                hot.push(p);
            }
        }
    }
    hot.sort_unstable();
    hot.dedup();
    let mut raw: Vec<Frag> = Vec::with_capacity(n * 2);
    let mut hits: Vec<(i128, I)> = Vec::new();
    let mut on: Vec<(i128, I)> = Vec::new();
    for s in segs {
        let w = if s.k == 0 { [1, 0] } else { [0, 1] };
        let d = isub(s.b, s.a);
        let dd = idot(d, d) as f64;
        let (x0, x1, y0, y1) = (s.a[0].min(s.b[0]), s.a[0].max(s.b[0]), s.a[1].min(s.b[1]), s.a[1].max(s.b[1]));
        hits.clear();
        let lo = hot.partition_point(|h| h[0] < x0);
        for &h in &hot[lo..] {
            if h[0] > x1 {
                break;
            }
            if h[1] < y0 || h[1] > y1 {
                continue;
            }
            // Squares whose centre is more than their half-diagonal from the line cannot be met.
            let c = orient(s.a, s.b, h) as f64;
            if c * c > 0.6 * dd {
                continue;
            }
            if pixel_hit(s.a, s.b, h) {
                hits.push((idot(isub(h, s.a), d), h));
            }
        }
        hits.sort_unstable();
        hits.dedup_by_key(|e| e.1);
        for k in 1..hits.len() {
            let (p, q) = (hits[k - 1].1, hits[k].1);
            if orient(s.a, s.b, p) == 0 && orient(s.a, s.b, q) == 0 {
                // Still on the edge: any hot centre on it was met above.
                push_frag(&mut raw, p, q, w);
                continue;
            }
            // A bent piece can pass over another hot centre: split there too.
            let e = isub(q, p);
            let len2 = idot(e, e);
            on.clear();
            let (bx0, bx1, by0, by1) = (p[0].min(q[0]), p[0].max(q[0]), p[1].min(q[1]), p[1].max(q[1]));
            let lo = hot.partition_point(|h| h[0] < bx0);
            for &h in &hot[lo..] {
                if h[0] > bx1 {
                    break;
                }
                if h[1] < by0 || h[1] > by1 || h == p || h == q || orient(p, q, h) != 0 {
                    continue;
                }
                let t = idot(isub(h, p), e);
                if t > 0 && t < len2 {
                    on.push((t, h));
                }
            }
            on.sort_unstable();
            let mut last = p;
            for &(_, h) in &on {
                push_frag(&mut raw, last, h, w);
                last = h;
            }
            push_frag(&mut raw, last, q, w);
        }
    }
    raw.sort_unstable_by(|x, y| (x.a, x.b).cmp(&(y.a, y.b)));
    let mut out: Vec<Frag> = Vec::with_capacity(raw.len());
    for f in raw {
        match out.last_mut() {
            Some(g) if g.a == f.a && g.b == f.b => {
                g.w[0] += f.w[0];
                g.w[1] += f.w[1];
            }
            _ => out.push(f),
        }
    }
    out.retain(|f| f.w != [0, 0]);
    out
}

fn push_frag(raw: &mut Vec<Frag>, p: I, q: I, w: [i32; 2]) {
    if p < q {
        raw.push(Frag { a: p, b: q, w });
    } else if q < p {
        raw.push(Frag { a: q, b: p, w: [-w[0], -w[1]] });
    }
}

/// Each piece's windings (left, right), by exact ray counts from its midpoint: down for pieces that
/// are not vertical, left for vertical ones. Half-open spans make rays through vertices count once.
fn sides(fr: &[Frag]) -> Vec<([i32; 2], [i32; 2])> {
    // `fr` is sorted by `a`, so the pieces that are not vertical are in order of their left ends.
    let nv: Vec<usize> = (0..fr.len()).filter(|&i| fr[i].a[0] != fr[i].b[0]).collect();
    let mut nh: Vec<(i64, usize)> = (0..fr.len()).filter(|&i| fr[i].a[1] != fr[i].b[1]).map(|i| (fr[i].a[1].min(fr[i].b[1]), i)).collect();
    nh.sort_unstable();
    let mut out = Vec::with_capacity(fr.len());
    for (i, f) in fr.iter().enumerate() {
        let m = [f.a[0] + f.b[0], f.a[1] + f.b[1]];
        let mut w = [0i32; 2];
        if f.a[0] != f.b[0] {
            let end = nv.partition_point(|&j| 2 * fr[j].a[0] <= m[0]);
            for &j in &nv[..end] {
                let g = &fr[j];
                if j == i || 2 * g.b[0] <= m[0] {
                    continue;
                }
                if orient([2 * g.a[0], 2 * g.a[1]], [2 * g.b[0], 2 * g.b[1]], m) > 0 {
                    w[0] += g.w[0];
                    w[1] += g.w[1];
                }
            }
            // Going +x, the left is above.
            out.push(([w[0] + f.w[0], w[1] + f.w[1]], w));
        } else {
            let end = nh.partition_point(|&(y, _)| 2 * y <= m[1]);
            for &(_, j) in &nh[..end] {
                let g = &fr[j];
                let (lo, hi, s) = if g.a[1] < g.b[1] { (g.a, g.b, -1) } else { (g.b, g.a, 1) };
                if j == i || 2 * hi[1] <= m[1] {
                    continue;
                }
                if orient([2 * lo[0], 2 * lo[1]], [2 * hi[0], 2 * hi[1]], m) < 0 {
                    w[0] += s * g.w[0];
                    w[1] += s * g.w[1];
                }
            }
            // Going +y, the left is where the ray runs.
            out.push((w, [w[0] - f.w[0], w[1] - f.w[1]]));
        }
    }
    out
}

/// Whether direction `a` comes before `b` turning anticlockwise from `r`.
fn ccw_before(r: I, a: I, b: I) -> bool {
    let half = |x: I| {
        let c = icross(r, x);
        if c > 0 || (c == 0 && idot(r, x) > 0) { 0 } else { 1 }
    };
    let (ha, hb) = (half(a), half(b));
    if ha != hb { ha < hb } else { icross(a, b) > 0 }
}

/// Kept directed edges linked into closed walks (sharpest left turn at each vertex), then split into
/// simple loops where a walk comes back to a vertex.
fn link(mut edges: Vec<(I, I)>) -> Vec<Vec<I>> {
    edges.sort_unstable();
    let n = edges.len();
    let mut used = vec![false; n];
    let mut loops: Vec<Vec<I>> = Vec::new();
    let mut walk: Vec<I> = Vec::new();
    for s in 0..n {
        if used[s] {
            continue;
        }
        walk.clear();
        let mut cur = s;
        for _ in 0..=n {
            used[cur] = true;
            let (u, v) = edges[cur];
            walk.push(u);
            let lo = edges.partition_point(|e| e.0 < v);
            let hi = edges.partition_point(|e| e.0 <= v);
            let back = isub(u, v);
            let mut best: Option<usize> = None;
            for j in lo..hi {
                if best.is_none_or(|b| ccw_before(back, isub(edges[b].1, v), isub(edges[j].1, v))) {
                    best = Some(j);
                }
            }
            match best {
                Some(j) if j == s => break,
                Some(j) if !used[j] => cur = j,
                // Not reached: every vertex has as many kept edges out as in, so a walk always comes
                // back to its first edge. Were it reached, the walk would go on by any unused edge and
                // might be emitted unclosed.
                _ => {
                    debug_assert!(false, "link: a walk did not close");
                    match (lo..hi).find(|&j| !used[j]) {
                        Some(j) => cur = j,
                        None => break,
                    }
                }
            }
        }
        split_loops(&walk, &mut loops);
    }
    loops
}

fn split_loops(walk: &[I], loops: &mut Vec<Vec<I>>) {
    let mut stack: Vec<I> = Vec::with_capacity(walk.len());
    let mut pos: FastMap<I, usize> = FastMap::default();
    for &v in walk {
        if let Some(&k) = pos.get(&v) {
            let lp = stack.split_off(k);
            for u in &lp {
                pos.remove(u);
            }
            loops.push(lp);
        }
        pos.insert(v, stack.len());
        stack.push(v);
    }
    if !stack.is_empty() {
        loops.push(stack);
    }
}

/// Drop vertices on the line through their neighbours (straight ones and spike tips).
fn drop_straight(r: &mut Vec<I>) {
    loop {
        let n = r.len();
        if n < 3 {
            return;
        }
        let keep: Vec<I> = (0..n).filter(|&i| orient(r[(i + n - 1) % n], r[i], r[(i + 1) % n]) != 0).map(|i| r[i]).collect();
        if keep.len() == n {
            return;
        }
        *r = keep;
    }
}

/// Whether doubled point `m` is inside ring `r` (it is never on it).
fn inside2(r: &[I], m: I) -> bool {
    let n = r.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = ([2 * r[i][0], 2 * r[i][1]], [2 * r[(i + 1) % n][0], 2 * r[(i + 1) % n][1]]);
        if (a[1] <= m[1]) != (b[1] <= m[1]) {
            let o = orient(a, b, m);
            if (b[1] > a[1]) == (o > 0) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Loops into canonical shapes: outers and holes by sign, holes into the smallest outer holding them.
fn assemble(loops: Vec<Vec<I>>) -> Vec<Shape> {
    let mut outers: Vec<(Vec<I>, i128, [i64; 4])> = Vec::new();
    let mut holes: Vec<Vec<I>> = Vec::new();
    for mut r in loops {
        drop_straight(&mut r);
        if r.len() < 3 {
            continue;
        }
        let a = area2(&r);
        if a == 0 {
            continue;
        }
        rotate_least(&mut r);
        if a > 0 {
            let mut bb = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
            for p in &r {
                bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
            }
            outers.push((r, a, bb));
        } else {
            holes.push(r);
        }
    }
    outers.sort();
    holes.sort();
    let mut kids: Vec<Vec<Vec<I>>> = vec![Vec::new(); outers.len()];
    for h in holes {
        let m = [h[0][0] + h[1][0], h[0][1] + h[1][1]];
        let mut best: Option<usize> = None;
        for (i, (o, a, bb)) in outers.iter().enumerate() {
            if m[0] < 2 * bb[0] || m[0] > 2 * bb[2] || m[1] < 2 * bb[1] || m[1] > 2 * bb[3] {
                continue;
            }
            if best.is_none_or(|b| *a < outers[b].1) && inside2(o, m) {
                best = Some(i);
            }
        }
        if let Some(i) = best {
            kids[i].push(h);
        }
    }
    outers
        .into_iter()
        .zip(kids)
        .map(|((o, _, _), hs)| Shape { outer: o.into_iter().map(feet).collect(), holes: hs.into_iter().map(|h| h.into_iter().map(feet).collect()).collect() })
        .collect()
}
