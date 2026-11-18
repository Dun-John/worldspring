use super::*;

// ---------------------------------------------------------------------------------------------
// Convex pieces.

/// Where a ray from a corner first meets the outline: a vertex (by its place in the ring) or a new
/// point on edge j (from point j to j + 1).
#[derive(Clone, Copy)]
enum Hit {
    Vertex(usize),
    Edge(usize, I),
}

/// Whether corner c turns the wrong way by more than `TOL` (c = orient(p, v, q) < 0).
fn wrong_way(c: i128, p: I, q: I) -> bool {
    let e = isub(q, p);
    let c = c as f64;
    c * c > TOL * TOL * idot(e, e) as f64
}

fn is_reflex(r: &[I], k: usize) -> bool {
    let n = r.len();
    let (p, v, q) = (r[pv(k, n)], r[k], r[nx(k, n)]);
    let c = orient(p, v, q);
    c < 0 && wrong_way(c, p, q)
}

/// Whether direction `x` from corner `w` (between `prev` and `next`, region on the left) points into
/// the region.
fn in_wedge(prev: I, w: I, next: I, x: I) -> bool {
    let (a, b) = (isub(next, w), isub(prev, w));
    if icross(a, b) > 0 { icross(a, x) > 0 && icross(x, b) > 0 } else { !(icross(b, x) >= 0 && icross(x, a) >= 0) }
}

/// The first point of `rings` that a ray from corner k of ring `ro` along `d` meets, with its
/// distance (grid steps) and ring. Vertices within `TOL` of the ray count as met. `boxes` are the
/// rings' `run_boxes`: a run lying wholly behind the corner, wholly to one side of the ray beyond
/// the tolerance or wholly farther than the point met so far is passed over (it could not change
/// what is met, so the result is the same as looking at every edge).
fn cast(rings: &[&[I]], boxes: &[Vec<[i64; 4]>], ro: usize, k: usize, d: I) -> Option<(f64, usize, Hit)> {
    let v = rings[ro][k];
    let dd = idot(d, d) as f64;
    let tol2 = TOL * TOL * dd;
    let mut best: Option<(f64, usize, Hit)> = None;
    for (ri, r) in rings.iter().enumerate() {
        let n = r.len();
        if n == 0 {
            continue;
        }
        // Each point's side of the ray (left > 0) and distance along it, carried along a run
        // (in i64 while the run and the corner are within 2^28 steps, about a million feet: the
        // products fit).
        let lim = |x: i64| x.abs() < 1 << 28;
        let near = lim(v[0]) && lim(v[1]);
        for (c, b) in boxes[ri].iter().enumerate() {
            let small = near && b.iter().all(|&x| lim(x));
            let at = |p: I| {
                let w = isub(p, v);
                if small {
                    ((d[0] * w[1] - d[1] * w[0]) as i128, (w[0] * d[0] + w[1] * d[1]) as i128)
                } else {
                    (icross(d, w), idot(w, d))
                }
            };
            // The side and distance are linear, so their least and greatest over the box are at
            // the corners picked by the signs of d.
            let corner = |hx: bool, hy: bool| [if hx { b[2] } else { b[0] }, if hy { b[3] } else { b[1] }];
            let (px, py) = (d[0] >= 0, d[1] >= 0);
            let (smin, smax) = (at(corner(py, !px)).0, at(corner(!py, px)).0);
            let (lmin, lmax) = (at(corner(!px, !py)).1, at(corner(px, py)).1);
            let beyond = |s: i128| (s as f64) * (s as f64) > tol2;
            if lmax <= 0 || (smin > 0 && beyond(smin)) || (smax < 0 && beyond(smax)) || best.is_some_and(|bb| lmin > 0 && lmin as f64 / dd > bb.0 * (1.0 + 1e-9)) {
                continue;
            }
            let (mut sa, mut la) = at(r[c * RUN]);
            for j in c * RUN..((c + 1) * RUN).min(n) {
                let (a, b) = (r[j], r[nx(j, n)]);
                let (sb, lb) = at(b);
                if a != v && la > 0 {
                    let t = la as f64 / dd;
                    let s = sa as f64;
                    if s * s <= tol2 && best.is_none_or(|bb| t < bb.0) && in_wedge(r[pv(j, n)], a, b, isub(v, a)) {
                        best = Some((t, ri, Hit::Vertex(j)));
                    }
                }
                // An edge facing the corner crosses the ray from its right to its left.
                if sa < 0 && sb > 0 && a != v && b != v && (la > 0 || lb > 0) {
                    let (fa, fb) = (sa as f64, sb as f64);
                    let t = (fa * lb as f64 - fb * la as f64) / ((fa - fb) * dd);
                    if t > 0.0 && best.is_none_or(|bb| t < bb.0) {
                        let e = isub(b, a);
                        let den = icross(d, e);
                        let tn = icross(isub(a, v), e);
                        let (tn, den) = if den < 0 { (-tn, -den) } else { (tn, den) };
                        let q = [rdiv(v[0] as i128 * den + d[0] as i128 * tn, den), rdiv(v[1] as i128 * den + d[1] as i128 * tn, den)];
                        // A point a step from an end (diagonally too): take the end, if the ray may enter there.
                        // Otherwise the point itself, unless it rounded onto an end.
                        let near = |x: I| idot(isub(q, x), isub(q, x)) <= 2;
                        if near(a) && in_wedge(r[pv(j, n)], a, b, isub(v, a)) {
                            best = Some((t, ri, Hit::Vertex(j)));
                        } else if near(b) && in_wedge(a, b, r[nx(nx(j, n), n)], isub(v, b)) {
                            best = Some((t, ri, Hit::Vertex(nx(j, n))));
                        } else if q != a && q != b {
                            best = Some((t, ri, Hit::Edge(j, q)));
                        }
                    }
                }
                sa = sb;
                la = lb;
            }
        }
    }
    best.map(|(t, ri, h)| (t * sqrt(dd), ri, h))
}

/// Whether a cut from `v` to `q` stays clear of every edge of `rings` but the one it ends on and
/// those through its ends (the ray behind it is exact up to the tolerance; this check is exact).
fn clear(rings: &[&[I]], v: I, q: I, hit: Hit, hit_ring: usize) -> bool {
    let (x0, x1, y0, y1) = (v[0].min(q[0]), v[0].max(q[0]), v[1].min(q[1]), v[1].max(q[1]));
    for (ri, r) in rings.iter().enumerate() {
        let n = r.len();
        for j in 0..n {
            if ri == hit_ring && matches!(hit, Hit::Edge(e, _) if e == j) {
                continue;
            }
            let (a, b) = (r[j], r[nx(j, n)]);
            if a[0].max(b[0]) < x0 || a[0].min(b[0]) > x1 || a[1].max(b[1]) < y0 || a[1].min(b[1]) > y1 {
                continue;
            }
            // Edges through either end (the edge the cut ends on, or one the corner lies on) meet it
            // there and nowhere else.
            if a == v || b == v || a == q || b == q || orient(a, b, q) == 0 || orient(a, b, v) == 0 {
                continue;
            }
            if seg_meet(a, b, v, q) {
                return false;
            }
        }
    }
    true
}

/// Ring `g` with hole `h` spliced in at `hit`, joined from the hole's corner k.
fn splice(g: &[I], hit: Hit, h: &[I], k: usize) -> Vec<I> {
    let m = h.len();
    let (j, q, new) = match hit {
        Hit::Vertex(j) => (j, g[j], false),
        Hit::Edge(j, q) => (j, q, true),
    };
    let mut out = Vec::with_capacity(g.len() + m + 3);
    out.extend_from_slice(&g[..=j]);
    if new {
        out.push(q);
    }
    for i in 0..=m {
        out.push(h[(k + i) % m]);
    }
    out.push(q);
    out.extend_from_slice(&g[j + 1..]);
    out
}

/// Edges per run in `run_boxes`.
const RUN: usize = 16;

/// Bounding boxes (x0, y0, x1, y1) of a ring's runs of `RUN` edges (run c: edges c * RUN onwards,
/// with both ends of each), to pass over far edges at a glance.
fn run_boxes(t: &[I]) -> Vec<[i64; 4]> {
    let n = t.len();
    (0..n.div_ceil(RUN))
        .map(|c| {
            let mut b = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
            for j in c * RUN..=((c + 1) * RUN).min(n) {
                grow(&mut b, t[j % n]);
            }
            b
        })
        .collect()
}

fn grow(b: &mut [i64; 4], p: I) {
    *b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
}

fn meets(a: &[i64; 4], b: &[i64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// A hole's corner lying on another ring (one of its corners, or inside one of its edges), where the
/// hole can join that ring: (hole, its corner, the ring, where on it). Empty rings (holes already
/// joined) are passed over. Only the runs of edges whose boxes meet the hole's are looked at.
fn touch(rings: &[Vec<I>], boxes: &[Vec<[i64; 4]>]) -> Option<(usize, usize, usize, Hit)> {
    let mut near: Vec<(usize, usize)> = Vec::new();
    for h in 1..rings.len() {
        let hr = &rings[h];
        let m = hr.len();
        if m == 0 {
            continue;
        }
        let mut hb = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
        for &p in hr {
            grow(&mut hb, p);
        }
        near.clear();
        for (g, bs) in boxes.iter().enumerate() {
            if g != h {
                near.extend(bs.iter().enumerate().filter(|(_, b)| meets(b, &hb)).map(|(c, _)| (g, c)));
            }
        }
        for k in 0..m {
            let v = hr[k];
            let into = isub(hr[(k + 1) % m], v);
            for &(g, c) in &near {
                let t = &rings[g];
                let n = t.len();
                for j in c * RUN..((c + 1) * RUN).min(n) {
                    let (a, b) = (t[j], t[nx(j, n)]);
                    if v[0] < a[0].min(b[0]) || v[0] > a[0].max(b[0]) || v[1] < a[1].min(b[1]) || v[1] > a[1].max(b[1]) {
                        continue;
                    }
                    if a == v {
                        if in_wedge(t[pv(j, n)], a, b, into) {
                            return Some((h, k, g, Hit::Vertex(j)));
                        }
                    } else if b != v && orient(a, b, v) == 0 {
                        return Some((h, k, g, Hit::Edge(j, v)));
                    }
                }
            }
        }
    }
    None
}

/// A cast waiting in `bridge`: from corner k of a hole along one of its edges' extensions, what it
/// met (length, ring, where), and the corner and the point met: a change to the rings farther than
/// `CAST_PAD` from the segment between them cannot alter it.
#[derive(Clone, Copy)]
struct Cast {
    k: usize,
    met: Option<(f64, usize, Hit)>,
    seg: (I, I),
    stale: bool,
}

/// How far from a cast, in grid steps, a change to the rings may alter it: more than the vertex
/// tolerance, the snap of a point met on an edge and of an end taken for it.
const CAST_PAD: i64 = 4;

/// Whether segment ab meets box b (x0, y0, x1, y1): their boxes meet and the box's corners are not
/// all on one side of the segment's line.
fn seg_meets_box(a: I, b: I, bx: &[i64; 4]) -> bool {
    if !meets(&[a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])], bx) {
        return false;
    }
    let s = [[bx[0], bx[1]], [bx[2], bx[1]], [bx[2], bx[3]], [bx[0], bx[3]]].map(|c| orient(a, b, c).signum());
    !(s.iter().all(|&x| x > 0) || s.iter().all(|&x| x < 0))
}

fn joins_of(rings: &[Vec<I>], h: usize) -> Vec<Cast> {
    let r = &rings[h];
    let n = r.len();
    let mut out = Vec::new();
    for k in 0..n {
        if is_reflex(r, k) {
            out.push(Cast { k, met: None, seg: ([0; 2], [0; 2]), stale: true });
            out.push(Cast { k, met: None, seg: ([0; 2], [0; 2]), stale: true });
        }
    }
    out
}

/// Casts a stale cast again (entries come in pairs per corner: the extension of the edge before
/// it, then of the edge after).
fn recast(rings: &[Vec<I>], refs: &[&[I]], boxes: &[Vec<[i64; 4]>], h: usize, i: usize, e: &mut Cast) {
    let r = &rings[h];
    let n = r.len();
    let k = e.k;
    let (p, v, q) = (r[pv(k, n)], r[k], r[nx(k, n)]);
    let d = if i.is_multiple_of(2) { isub(v, p) } else { isub(v, q) };
    e.met = cast(refs, boxes, h, k, d);
    e.seg = (v, e.met.map_or(v, |(_, ri, hit)| hit_point(&rings[ri], hit)));
    e.stale = false;
}

fn hit_point(r: &[I], hit: Hit) -> I {
    match hit {
        Hit::Vertex(j) => r[j],
        Hit::Edge(_, q) => q,
    }
}

/// The holes joined to the outer ring one by one, each along the shortest edge extension from one
/// of its corners to another ring, giving one ring that runs along each join both ways.
///
/// Rings keep their places (a joined hole's is left empty), and each hole's casts are kept between
/// joins: a join changes the rings only within the box of its bridge, the hole it joined and (when
/// it met an edge) that edge, so only the casts passing within `CAST_PAD` of that box, those of the
/// ring it joined and those that met nothing are cast again; casts that met the joined ring elsewhere have
/// their places in it shifted. The joins chosen are those of casting everything afresh each time.
fn bridge(outer: Vec<I>, holes: Vec<Vec<I>>) -> Option<Vec<I>> {
    let mut rings = vec![outer];
    rings.extend(holes);
    let mut left = rings.len();
    let mut joins: Vec<Option<Vec<Cast>>> = vec![None; rings.len()];
    let mut boxes: Vec<Vec<[i64; 4]>> = rings.iter().map(|t| run_boxes(t)).collect();
    while left > 1 {
        // A hole touching another ring at a point joins it there, by no bridge at all.
        if let Some((h, k, g, at)) = touch(&rings, &boxes) {
            let hole = std::mem::take(&mut rings[h]);
            let m = hole.len();
            let t = &rings[g];
            let mut out = Vec::with_capacity(t.len() + m + 1);
            let j = match at {
                Hit::Vertex(j) => {
                    out.extend_from_slice(&t[..=j]);
                    j
                }
                Hit::Edge(j, q) => {
                    out.extend_from_slice(&t[..=j]);
                    out.push(q);
                    j
                }
            };
            for i in 1..=m {
                out.push(hole[(k + i) % m]);
            }
            out.extend_from_slice(&t[j + 1..]);
            rings[g] = out;
            boxes[g] = run_boxes(&rings[g]);
            boxes[h] = Vec::new();
            left -= 1;
            joins.iter_mut().for_each(|j| *j = None);
            continue;
        }
        let refs: Vec<&[I]> = rings.iter().map(|r| r.as_slice()).collect();
        for h in 1..rings.len() {
            if rings[h].is_empty() {
                continue;
            }
            let js = joins[h].get_or_insert_with(|| joins_of(&rings, h));
            for (i, e) in js.iter_mut().enumerate() {
                if e.stale {
                    recast(&rings, &refs, &boxes, h, i, e);
                }
            }
        }
        let mut best: Option<(f64, usize, usize, usize, Hit)> = None;
        for h in 1..rings.len() {
            for e in joins[h].iter().flatten() {
                if let Some((len, ri, hit)) = e.met
                    && ri != h
                    && best.is_none_or(|b| len < b.0)
                    && clear(&refs, rings[h][e.k], hit_point(&rings[ri], hit), hit, ri)
                {
                    best = Some((len, h, e.k, ri, hit));
                }
            }
        }
        let (_, h, k, ri, hit) = best?;
        // What the join changes: the bridge, the hole and the edge split.
        let mut changed = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
        for &p in &rings[h] {
            grow(&mut changed, p);
        }
        grow(&mut changed, hit_point(&rings[ri], hit));
        let (j, ins) = match hit {
            Hit::Vertex(j) => (j, rings[h].len() + 2),
            Hit::Edge(j, _) => {
                let g = &rings[ri];
                grow(&mut changed, g[j]);
                grow(&mut changed, g[nx(j, g.len())]);
                (j, rings[h].len() + 3)
            }
        };
        let changed = [changed[0] - CAST_PAD, changed[1] - CAST_PAD, changed[2] + CAST_PAD, changed[3] + CAST_PAD];
        let at_vertex = matches!(hit, Hit::Vertex(_));
        let hole = std::mem::take(&mut rings[h]);
        let old = std::mem::take(&mut rings[ri]);
        rings[ri] = splice(&old, hit, &hole, k);
        boxes[ri] = run_boxes(&rings[ri]);
        boxes[h] = Vec::new();
        left -= 1;
        let hole_casts = joins[h].take();
        if ri > 0 {
            // A hole joined to a hole: the casts of the corners that keep their neighbours carry over.
            let was = joins[ri].take();
            let from = |p: usize| -> Option<(&[I], &[Cast], usize)> {
                let base = j + 1 + usize::from(!at_vertex);
                if p <= j {
                    Some((&old, was.as_deref()?, p))
                } else if p > j + ins {
                    Some((&old, was.as_deref()?, p - ins))
                } else if p >= base && p < base + hole.len() {
                    Some((&hole, hole_casts.as_deref()?, (k + p - base) % hole.len()))
                } else {
                    None
                }
            };
            let r = &rings[ri];
            let n = r.len();
            let mut js = Vec::new();
            for p in 0..n {
                if !is_reflex(r, p) {
                    continue;
                }
                let kept = from(p).and_then(|(sr, sc, sk)| {
                    let sn = sr.len();
                    let at = sc.partition_point(|e| e.k < sk);
                    let same = r[pv(p, n)] == sr[pv(sk, sn)] && r[p] == sr[sk] && r[nx(p, n)] == sr[nx(sk, sn)];
                    (same && at + 1 < sc.len() && sc[at].k == sk).then(|| [sc[at], sc[at + 1]])
                });
                match kept {
                    Some(pair) => js.extend(pair.map(|e| Cast { k: p, ..e })),
                    None => js.extend([Cast { k: p, met: None, seg: (r[p], r[p]), stale: true }; 2]),
                }
            }
            joins[ri] = Some(js);
        }
        let shift = |i: usize| if i > j { i + ins } else { i };
        for js in joins.iter_mut().flatten() {
            for e in js.iter_mut() {
                match e.met {
                    Some((_, r, _)) if r == h => e.stale = true,
                    None => e.stale = true,
                    _ if seg_meets_box(e.seg.0, e.seg.1, &changed) => e.stale = true,
                    Some((len, r, met)) if r == ri => {
                        let moved = match met {
                            Hit::Vertex(i) => Hit::Vertex(shift(i)),
                            // (Edge j itself is split, so it lies in the changed box; past a vertex
                            // joined at, its edge is the one after the second copy.)
                            Hit::Edge(i, x) => Hit::Edge(if i == j && at_vertex { i + ins } else { shift(i) }, x),
                        };
                        e.met = Some((len, r, moved));
                    }
                    _ => {}
                }
            }
        }
    }
    Some(std::mem::take(&mut rings[0]))
}

/// A ring without its corners that are straight or turn the wrong way within the tolerance,
/// dropped until none are left (spike tips go too).
pub(super) fn simplify(r: &[I]) -> Vec<I> {
    let mut r = r.to_vec();
    r.dedup();
    while r.len() > 1 && r[0] == r[r.len() - 1] {
        r.pop();
    }
    // One at a time, the straightest first (so the result does not depend on where the ring
    // starts): two corners a step apart can each look straight against the other.
    loop {
        let n = r.len();
        if n < 3 {
            return r;
        }
        let mut best: Option<(f64, I, usize)> = None;
        for k in 0..n {
            let (p, v, q) = (r[pv(k, n)], r[k], r[nx(k, n)]);
            let c = orient(p, v, q);
            if c > 0 || (c < 0 && wrong_way(c, p, q)) {
                continue;
            }
            let e = isub(q, p);
            let off = fabs(c as f64) / sqrt((idot(e, e) as f64).max(1.0));
            if best.is_none_or(|(b, bv, _)| off < b || (off == b && v < bv)) {
                best = Some((off, v, k));
            }
        }
        match best {
            Some((_, _, k)) => {
                r.remove(k);
            }
            None => return r,
        }
    }
}

fn walk_ring(r: &[I], from: usize, to: usize) -> Vec<I> {
    let n = r.len();
    let mut out = Vec::new();
    let mut i = from;
    loop {
        out.push(r[i]);
        if i == to || out.len() > n {
            break;
        }
        i = nx(i, n);
    }
    out
}

pub(super) fn pieces_of(s: &Shape, out: &mut Vec<Vec<I>>, depth: u32) {
    // Corners straight within the tolerance go first, so none hides a reflex corner between them.
    let outer: Vec<I> = simplify(&s.outer.iter().filter_map(|p| grid(*p)).collect::<Vec<I>>());
    let holes: Vec<Vec<I>> = s.holes.iter().map(|h| simplify(&h.iter().filter_map(|p| grid(*p)).collect::<Vec<I>>())).filter(|h| h.len() >= 3).collect();
    if outer.len() < 3 {
        return;
    }
    if holes.is_empty() {
        cut(outer, out, depth);
        return;
    }
    match bridge(outer, holes) {
        Some(r) => cut(r, out, depth),
        None => {
            // No join found: cut the shape along the line of a hole's first edge instead.
            let h = &s.holes[0];
            line_cut(s.clone(), h[0], geom::sub(h[1], h[0]), out, depth);
        }
    }
}

/// The fallback: a shape cut in two along a whole line, each side cut into pieces again.
fn line_cut(s: Shape, p: P, dir: P, out: &mut Vec<Vec<I>>, depth: u32) {
    if depth >= 8 {
        out.push(s.outer.iter().filter_map(|p| grid(*p)).collect());
        return;
    }
    let (l, r) = split(&[s], p, dir);
    for part in l.iter().chain(r.iter()) {
        pieces_of(part, out, depth + 1);
    }
}

/// A cut waiting in a piece: from corner k along an extension of one of its edges to `hit` (point
/// `q`), `len` grid steps long.
#[derive(Clone, Copy)]
struct Cand {
    len: f64,
    k: usize,
    hit: Hit,
    q: I,
}

fn cands_at(r: &[I], boxes: &[Vec<[i64; 4]>], k: usize, out: &mut Vec<Cand>) {
    let n = r.len();
    let (p, v, nx) = (r[(k + n - 1) % n], r[k], r[(k + 1) % n]);
    for d in [isub(v, p), isub(v, nx)] {
        if let Some((len, _, hit)) = cast(&[r], boxes, 0, k, d) {
            let q = match hit {
                Hit::Vertex(j) => r[j],
                Hit::Edge(_, q) => q,
            };
            out.push(Cand { len, k, hit, q });
        }
    }
}

/// Whether closed segments ab and cd meet.
fn seg_meet(a: I, b: I, c: I, d: I) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c).signum(), orient(a, b, d).signum(), orient(c, d, a).signum(), orient(c, d, b).signum());
    if o1 * o2 < 0 && o3 * o4 < 0 {
        return true;
    }
    let on = |p: I, q: I, x: I| x[0] >= p[0].min(q[0]) && x[0] <= p[0].max(q[0]) && x[1] >= p[1].min(q[1]) && x[1] <= p[1].max(q[1]);
    (o1 == 0 && on(a, b, c)) || (o2 == 0 && on(a, b, d)) || (o3 == 0 && on(c, d, a)) || (o4 == 0 && on(c, d, b))
}

/// A ring (region on its left, possibly running both ways along joins) cut at its reflex corners
/// until every piece is convex, the shortest cut in a piece first. A piece keeps its waiting cuts
/// when it is cut again, unless the new cut crosses them or ends at their corner.
fn cut(ring: Vec<I>, out: &mut Vec<Vec<I>>, depth: u32) {
    let n0 = ring.len();
    let mut budget = 64 + 4 * n0;
    if n0 < 3 || area2(&ring) <= 0 {
        return;
    }
    // Pieces waiting: (ring, waiting cuts, corners to cast cuts from, reflex corners).
    let reflex: Vec<usize> = (0..n0).filter(|&k| is_reflex(&ring, k)).collect();
    let mut stack: Vec<(Vec<I>, Vec<Cand>, Vec<usize>, Vec<usize>)> = vec![(ring, Vec::new(), reflex.clone(), reflex)];
    while let Some((r, mut cands, mut todo, reflex)) = stack.pop() {
        let n = r.len();
        if n < 3 {
            continue;
        }
        todo.sort_unstable();
        todo.dedup();
        if !todo.is_empty() {
            let boxes = [run_boxes(&r)];
            for &k in &todo {
                cands_at(&r, &boxes, k, &mut cands);
            }
        }
        if reflex.is_empty() {
            // Corners straight within the tolerance go; if a run of them added up to a reflex
            // corner, the simpler ring is cut again.
            let s = simplify(&r);
            if s.len() >= 3 && area2(&s) > 0 {
                let rs: Vec<usize> = (0..s.len()).filter(|&k| is_reflex(&s, k)).collect();
                if !rs.is_empty() && budget > 0 {
                    budget -= 1;
                    stack.push((s, Vec::new(), rs.clone(), rs));
                } else {
                    out.push(s);
                }
            }
            continue;
        }
        let mut best: Option<usize> = None;
        if budget > 0 {
            budget -= 1;
            loop {
                best = None;
                for (i, c) in cands.iter().enumerate() {
                    if best.is_none_or(|b| c.len < cands[b].len) {
                        best = Some(i);
                    }
                }
                match best {
                    Some(b) if !clear(&[&r], r[cands[b].k], cands[b].q, cands[b].hit, 0) => {
                        cands.swap_remove(b);
                    }
                    _ => break,
                }
            }
        }
        let Some(bi) = best else {
            // No cut found: cut along the whole line of a reflex corner's edge instead.
            let k = reflex[0];
            let (p, v) = (feet(r[(k + n - 1) % n]), feet(r[k]));
            let ring: Vec<P> = r.iter().map(|q| feet(*q)).collect();
            for s in region(&ring) {
                line_cut(s, v, geom::sub(v, p), out, depth);
            }
            continue;
        };
        let c = cands[bi];
        let (k, v, q) = (c.k, r[c.k], c.q);
        // The two sides, as runs of the ring: (first corner, corners in the run, place of the first
        // in the side), and the corner the cut ends at, if any.
        let (a, b, runs, jv) = match c.hit {
            Hit::Vertex(j) => (walk_ring(&r, k, j), walk_ring(&r, j, k), [(k, (j + n - k) % n + 1, 0), (j, (k + n - j) % n + 1, 0)], Some(j)),
            Hit::Edge(j, x) => {
                let mut a = walk_ring(&r, k, j);
                a.push(x);
                let mut b = vec![x];
                b.extend(walk_ring(&r, (j + 1) % n, k));
                (a, b, [(k, (j + n - k) % n + 1, 0), ((j + 1) % n, (k + n - j - 1) % n + 1, 1)], None)
            }
        };
        let map = |side: usize, i: usize| {
            let (from, len, at) = runs[side];
            let o = if i >= from { i - from } else { i + n - from };
            (o < len).then_some(o + at)
        };
        let lens = [a.len(), b.len()];
        let map_hit = |side: usize, h: Hit| match h {
            Hit::Vertex(jj) => map(side, jj).map(Hit::Vertex),
            Hit::Edge(jj, x) => match (map(side, jj), map(side, nx(jj, n))) {
                (Some(c0), Some(c1)) if c1 == nx(c0, lens[side]) => Some(Hit::Edge(c0, x)),
                _ => None,
            },
        };
        let mut stale: Vec<usize> = vec![k];
        stale.extend(jv);
        let (x0, x1, y0, y1) = (v[0].min(q[0]), v[0].max(q[0]), v[1].min(q[1]), v[1].max(q[1]));
        for cd in &cands {
            let (a, b) = (r[cd.k], cd.q);
            let apart = a[0].max(b[0]) < x0 || a[0].min(b[0]) > x1 || a[1].max(b[1]) < y0 || a[1].min(b[1]) > y1;
            if (!apart && seg_meet(a, b, v, q)) || (0..2).all(|s| map(s, cd.k).is_none_or(|_| map_hit(s, cd.hit).is_none())) {
                stale.push(cd.k);
            }
        }
        stale.sort_unstable();
        stale.dedup();
        let rings = [&a, &b];
        let mut kids: [(Vec<Cand>, Vec<usize>, Vec<usize>); 2] = [(Vec::new(), Vec::new(), Vec::new()), (Vec::new(), Vec::new(), Vec::new())];
        // Reflex corners carry over, but for the two the cut touches, whose corners it split.
        for &rk in &reflex {
            if rk == k || Some(rk) == jv {
                continue;
            }
            let st = stale.binary_search(&rk).is_ok();
            for (side, kid) in kids.iter_mut().enumerate() {
                if let Some(kk) = map(side, rk) {
                    kid.2.push(kk);
                    if st {
                        kid.1.push(kk);
                    }
                }
            }
        }
        for sk in std::iter::once(k).chain(jv) {
            for (side, kid) in kids.iter_mut().enumerate() {
                if let Some(kk) = map(side, sk) {
                    if is_reflex(rings[side], kk) {
                        kid.2.push(kk);
                        kid.1.push(kk);
                    }
                }
            }
        }
        for cd in &cands {
            if stale.binary_search(&cd.k).is_ok() {
                continue;
            }
            for (side, kid) in kids.iter_mut().enumerate() {
                if let (Some(kk), Some(h)) = (map(side, cd.k), map_hit(side, cd.hit)) {
                    kid.0.push(Cand { len: cd.len, k: kk, hit: h, q: cd.q });
                }
            }
        }
        let [(ca, ta, ra), (cb, tb, rb)] = kids;
        stack.push((a, ca, ta, ra));
        stack.push((b, cb, tb, rb));
    }
}
