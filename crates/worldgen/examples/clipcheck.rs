//! QA for `town::clip` (integer polygon booleans, offsets, splits, convex pieces): seeded random
//! cases in families (convex, star-shaped, rectilinear combs rotated, grids of blocks sharing edges
//! exactly, near-degenerate pairs with shared vertices, points on edges, spikes and repeated points,
//! checkerboards touching at corners, shapes with holes), each checked for:
//! - valid output: grid points, rings of 3+ points with no repeated or collinear ones, outers
//!   anticlockwise and holes clockwise, each ring from its least vertex, shapes and holes in order,
//!   no two edges crossing or overlapping, holes inside their outer, shapes not inside each other;
//! - area identities: |A∪B| + |A∩B| = |A| + |B| and |A−B| = |A| − |A∩B|, within the perimeters
//!   times 2/256 ft;
//! - permutation: rings shuffled, reversed, started elsewhere, shapes repeated: the same bits;
//! - convex pieces: convex, not overlapping, inside the shape, adding up to its area;
//! - buffers and offsets: points inside the band or offset at just under the distance, outside at
//!   just over (round joins), bevel ⊆ round ⊆ mitre;
//! - per-edge inset: convex rings against `geom::inset` (vertices within 1/256 ft, more at sharp
//!   corners by 1/sin(half the angle); not compared near a collapse), concave ones inside the ring and,
//!   before the skeleton's first event, against the ring of the setback lines' meeting points
//!   (vertices within 4/256 ft, scaled the same way);
//! - split: the sides on their sides, not overlapping, adding up.
//! Then timings. Prints failures by check with the worst cases; exits 1 on any.
//! `cargo run --release -p worldgen --example clipcheck [cases per family]` (default 2000; `SEED=n`;
//! `FULL=1` prints failing shapes whole, at full precision).

use std::collections::BTreeMap;
use std::time::Instant;
use worldgen::core::rng::Pcg32;
use worldgen::town::clip::{self, Ends, Join, Shape};
use worldgen::town::geom::{self, P};

const G: f64 = 1.0 / 256.0;

/// The seeded generator behind a shared reference, so draws can sit in a generator's arguments.
struct R(std::cell::RefCell<Pcg32>);

impl R {
    fn new(seed: u64, seq: u64) -> Self {
        R(std::cell::RefCell::new(Pcg32::new(seed, seq)))
    }
    fn range(&self, lo: f64, hi: f64) -> f64 {
        self.0.borrow_mut().range(lo, hi)
    }
    fn below(&self, n: u32) -> u32 {
        self.0.borrow_mut().below(n)
    }
}

#[derive(Default)]
struct Check {
    cases: u64,
    fails: u64,
    worst: f64,
    worst_note: String,
    notes: Vec<String>,
}

#[derive(Default)]
struct Report(BTreeMap<&'static str, Check>);

impl Report {
    /// Record a case: `bad` is how far past its tolerance it went (<= 0 passes).
    fn add(&mut self, name: &'static str, bad: f64, note: impl FnOnce() -> String) {
        let c = self.0.entry(name).or_default();
        c.cases += 1;
        if bad > 0.0 {
            c.fails += 1;
            let n = note();
            if c.notes.len() < 4 {
                c.notes.push(n.clone());
            }
            if bad > c.worst {
                c.worst = bad;
                c.worst_note = n;
            }
        }
    }
    fn ok(&mut self, name: &'static str, ok: bool, note: impl FnOnce() -> String) {
        self.add(name, if ok { 0.0 } else { 1.0 }, note);
    }
    /// Record a measured value against a tolerance (kept as the worst ratio either way).
    fn within(&mut self, name: &'static str, err: f64, tol: f64, note: impl FnOnce() -> String) {
        let c = self.0.entry(name).or_default();
        let ratio = err / tol.max(1e-12);
        if ratio > 1.0 || !err.is_finite() {
            c.cases += 1;
            c.fails += 1;
            let n = format!("err {err:.6} tol {tol:.6}: {}", note());
            if c.notes.len() < 4 {
                c.notes.push(n.clone());
            }
            if !(ratio <= c.worst) {
                c.worst = ratio;
                c.worst_note = n;
            }
        } else {
            c.cases += 1;
            if ratio > c.worst && c.fails == 0 {
                c.worst = ratio;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Generators (grid-exact unless rotated).

fn snapg(p: P) -> P {
    [(p[0] * 256.0).round() / 256.0, (p[1] * 256.0).round() / 256.0]
}

trait Gen {
    fn convex(&self, c: P, r: f64, n: usize) -> Vec<P>;
    fn star(&self, c: P, r: f64, n: usize, inner: f64) -> Vec<P>;
    fn comb(&self, c: P, w: f64, teeth: usize, rot: f64) -> Vec<P>;
}

impl Gen for R {
fn convex(&self, c: P, r: f64, n: usize) -> Vec<P> {
    let rng = self;
    let mut a: Vec<f64> = (0..n).map(|_| rng.range(0.0, std::f64::consts::TAU)).collect();
    a.sort_by(|x, y| x.total_cmp(y));
    let (rx, ry, rot) = (r, r * rng.range(0.3, 1.0), rng.range(0.0, 3.2));
    let (cs, sn) = (rot.cos(), rot.sin());
    let pts: Vec<P> = a
        .iter()
        .map(|t| {
            let (x, y) = (rx * t.cos(), ry * t.sin());
            snapg([c[0] + x * cs - y * sn, c[1] + x * sn + y * cs])
        })
        .collect();
    // Rounding can make a nearly straight corner turn the wrong way: the hull is convex for sure.
    geom::hull(pts)
}

fn star(&self, c: P, r: f64, n: usize, inner: f64) -> Vec<P> {
    let rng = self;
    let mut a: Vec<f64> = (0..n).map(|_| rng.range(0.0, std::f64::consts::TAU)).collect();
    a.sort_by(|x, y| x.total_cmp(y));
    a.dedup_by(|x, y| (*x - *y).abs() < 1e-3);
    a.iter().map(|t| { let rr = r * rng.range(inner, 1.0); snapg([c[0] + rr * t.cos(), c[1] + rr * t.sin()]) }).collect()
}

fn comb(&self, c: P, w: f64, teeth: usize, rot: f64) -> Vec<P> {
    let rng = self;
    let h = w * rng.range(0.2, 0.6);
    let th = w * rng.range(0.1, 0.5);
    let tw = w / (2 * teeth + 1) as f64;
    let mut pts = vec![[0.0, 0.0], [w, 0.0], [w, h]];
    for k in (0..teeth).rev() {
        let x1 = tw * (2 * k + 2) as f64;
        let x0 = tw * (2 * k + 1) as f64;
        pts.extend([[x1, h], [x1, h + th], [x0, h + th], [x0, h]]);
    }
    pts.push([0.0, h]);
    let (cs, sn) = (rot.cos(), rot.sin());
    pts.iter().map(|p| [c[0] + p[0] * cs - p[1] * sn, c[1] + p[0] * sn + p[1] * cs]).collect()
}
}

fn ring(r: Vec<P>) -> Vec<Shape> {
    vec![Shape::ring(r)]
}

fn perim_ring(r: &[P]) -> f64 {
    (0..r.len()).map(|i| geom::dist(r[i], r[(i + 1) % r.len()])).sum()
}

fn perim(set: &[Shape]) -> f64 {
    set.iter().map(|s| perim_ring(&s.outer) + s.holes.iter().map(|h| perim_ring(h)).sum::<f64>()).sum()
}

fn shuffle<T>(rng: &R, v: &mut [T]) {
    for i in (1..v.len()).rev() {
        let j = rng.below(i as u32 + 1) as usize;
        v.swap(i, j);
    }
}

fn rot_rev(rng: &R, r: &[P]) -> Vec<P> {
    let mut v = r.to_vec();
    if !v.is_empty() {
        let k = rng.below(v.len() as u32) as usize;
        v.rotate_left(k);
    }
    if rng.below(2) == 1 {
        v.reverse();
    }
    v
}

fn permute(rng: &R, set: &[Shape]) -> Vec<Shape> {
    let mut v: Vec<Shape> = set.to_vec();
    if !v.is_empty() && rng.below(4) == 0 {
        let k = rng.below(v.len() as u32) as usize;
        v.push(v[k].clone());
    }
    shuffle(rng, &mut v);
    for s in v.iter_mut() {
        s.outer = rot_rev(rng, &s.outer);
        shuffle(rng, &mut s.holes);
        for h in s.holes.iter_mut() {
            *h = rot_rev(rng, h);
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Validity.

type I = [i64; 2];

fn gi(p: P) -> Option<I> {
    let (x, y) = (p[0] * 256.0, p[1] * 256.0);
    if x.round() != x || y.round() != y {
        return None;
    }
    Some([x as i64, y as i64])
}

fn orient(a: I, b: I, c: I) -> i128 {
    (b[0] - a[0]) as i128 * (c[1] - a[1]) as i128 - (b[1] - a[1]) as i128 * (c[0] - a[0]) as i128
}

/// Whether two segments cross or overlap (more than touching at a point).
fn bad_pair(a: I, b: I, c: I, d: I) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1.signum() * o2.signum() < 0 && o3.signum() * o4.signum() < 0 {
        return true;
    }
    if o1 == 0 && o2 == 0 {
        // Collinear: overlapping along a stretch?
        let ax = if a[0] != b[0] { 0 } else { 1 };
        let (p0, p1) = (a[ax].min(b[ax]), a[ax].max(b[ax]));
        let (q0, q1) = (c[ax].min(d[ax]), c[ax].max(d[ax]));
        return p0.max(q0) < p1.min(q1);
    }
    false
}

fn valid(set: &[Shape]) -> Result<(), String> {
    let mut edges: Vec<(I, I)> = Vec::new();
    let mut prev_outer: Option<Vec<I>> = None;
    for (si, s) in set.iter().enumerate() {
        let mut rings: Vec<Vec<I>> = Vec::new();
        for (h, r) in std::iter::once(&s.outer).chain(s.holes.iter()).enumerate() {
            let Some(ri) = r.iter().map(|p| gi(*p)).collect::<Option<Vec<I>>>() else { return Err(format!("shape {si} ring {h}: off the grid")) };
            let n = ri.len();
            if n < 3 {
                return Err(format!("shape {si} ring {h}: {n} points"));
            }
            let a = geom::area(r);
            if (h == 0) != (a > 0.0) {
                return Err(format!("shape {si} ring {h}: area {a}"));
            }
            for k in 0..n {
                let (p, v, q) = (ri[(k + n - 1) % n], ri[k], ri[(k + 1) % n]);
                if v == q {
                    return Err(format!("shape {si} ring {h}: repeated point"));
                }
                if orient(p, v, q) == 0 {
                    return Err(format!("shape {si} ring {h}: collinear point {:?}", r[k]));
                }
            }
            if (0..n).any(|k| ri[k] < ri[0]) {
                return Err(format!("shape {si} ring {h}: not from its least vertex"));
            }
            for k in 0..n {
                edges.push((ri[k], ri[(k + 1) % n]));
            }
            rings.push(ri);
        }
        for k in 2..rings.len() {
            if rings[k] < rings[k - 1] {
                return Err(format!("shape {si}: holes out of order"));
            }
        }
        if prev_outer.as_ref().is_some_and(|p| rings[0] < *p) {
            return Err(format!("shape {si}: shapes out of order"));
        }
        prev_outer = Some(rings[0].clone());
        // Holes inside the outer, not inside each other.
        for (h, hole) in s.holes.iter().enumerate() {
            let m = geom::lerp(hole[0], hole[1], 0.5);
            if !geom::contains(&s.outer, m) {
                return Err(format!("shape {si}: hole {h} outside its outer"));
            }
            for (g, other) in s.holes.iter().enumerate() {
                if g != h && geom::contains(other, m) {
                    return Err(format!("shape {si}: hole {h} inside hole {g}"));
                }
            }
        }
    }
    // Shapes not inside one another.
    for (i, s) in set.iter().enumerate() {
        let m = geom::lerp(s.outer[0], s.outer[1], 0.5);
        for (j, t) in set.iter().enumerate() {
            if i != j && clip::contains(std::slice::from_ref(t), m) {
                return Err(format!("shape {i} inside shape {j}"));
            }
        }
    }
    edges.sort_by_key(|e| e.0[0].min(e.1[0]));
    for i in 0..edges.len() {
        let (a, b) = edges[i];
        let x1 = a[0].max(b[0]);
        for &(c, d) in &edges[i + 1..] {
            if c[0].min(d[0]) > x1 {
                break;
            }
            if bad_pair(a, b, c, d) {
                return Err(format!("edges cross or overlap: {:?}-{:?} and {:?}-{:?}", a, b, c, d));
            }
        }
    }
    Ok(())
}

fn desc(set: &[Shape]) -> String {
    if std::env::var("FULL").is_ok() {
        return format!("{set:?}");
    }
    let mut s = String::new();
    for sh in set {
        s.push_str(&format!("[{}]", sh.outer.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect::<Vec<_>>().join(" ")));
        for h in &sh.holes {
            s.push_str(&format!(" hole[{}]", h.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect::<Vec<_>>().join(" ")));
        }
        s.push_str("; ");
    }
    if s.len() > 900 {
        s.truncate(900);
        s.push_str("...");
    }
    s
}

// ---------------------------------------------------------------------------------------------
// Checks.

fn check_pair(rep: &mut Report, rng: &R, fam: &str, a: &[Shape], b: &[Shape], pieces: bool) {
    let na = clip::normalize(a);
    let nb = clip::normalize(b);
    let u = clip::union(a, b);
    let x = clip::intersection(a, b);
    let d = clip::difference(a, b);
    for (name, r) in [("valid: normalize", &na), ("valid: union", &u), ("valid: intersection", &x), ("valid: difference", &d)] {
        let v = valid(r);
        rep.ok(name, v.is_ok(), || format!("{fam}: {} | A {} | B {}", v.clone().err().unwrap_or_default(), desc(a), desc(b)));
    }
    let tol = 2.0 * G * (perim(a) + perim(b)) + 1e-6;
    let (aa, ab, au, ax, ad) = (clip::area(&na), clip::area(&nb), clip::area(&u), clip::area(&x), clip::area(&d));
    rep.within("area: |A∪B|+|A∩B| = |A|+|B|", (au + ax - aa - ab).abs(), tol, || format!("{fam}: A {} | B {}", desc(a), desc(b)));
    rep.within("area: |A−B| = |A|−|A∩B|", (ad - aa + ax).abs(), tol, || format!("{fam}: A {} | B {}", desc(a), desc(b)));
    let (pa, pb) = (permute(rng, a), permute(rng, b));
    let same = clip::union(&pa, &pb) == u && clip::intersection(&pa, &pb) == x && clip::difference(&pa, &pb) == d && clip::normalize(&pa) == na;
    rep.ok("permutation: same bits", same, || format!("{fam}: A {} | B {}", desc(a), desc(b)));
    if pieces {
        for set in [&d, &u] {
            check_pieces(rep, fam, set);
        }
    }
}

fn check_pieces(rep: &mut Report, fam: &str, set: &[Shape]) {
    if set.is_empty() {
        return;
    }
    let pcs = clip::convex_pieces(set);
    let total: f64 = pcs.iter().map(|p| geom::area(p)).sum();
    let pp: f64 = pcs.iter().map(|p| perim_ring(p)).sum();
    let tol = G * (perim(set) + pp) + 1e-6;
    let bad: Vec<usize> = (0..pcs.len()).filter(|&i| !clip::is_convex(&pcs[i]) || geom::area(&pcs[i]) <= 0.0).collect();
    rep.ok("pieces: convex, anticlockwise", bad.is_empty(), || format!("{fam}: piece {:?} of {} | shape {}", bad.first().map(|&i| &pcs[i]), pcs.len(), desc(set)));
    rep.within("pieces: add up to the area", (total - clip::area(set)).abs(), tol, || format!("{fam}: {} pieces | {}", pcs.len(), desc(set)));
    let as_shapes: Vec<Shape> = pcs.iter().map(|p| Shape::ring(p.clone())).collect();
    let out = clip::area(&clip::difference(&as_shapes, set));
    rep.within("pieces: inside the shape", out, tol, || format!("{fam}: {}", desc(set)));
    let mut worst = (0.0, 0, 0);
    let bb: Vec<(f64, f64, f64, f64)> = pcs
        .iter()
        .map(|p| p.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, q| (b.0.min(q[0]), b.1.min(q[1]), b.2.max(q[0]), b.3.max(q[1]))))
        .collect();
    for i in 0..pcs.len() {
        for j in i + 1..pcs.len() {
            let (p, q) = (bb[i], bb[j]);
            if p.0 >= q.2 || q.0 >= p.2 || p.1 >= q.3 || q.1 >= p.3 {
                continue;
            }
            let ov = convex_overlap(&pcs[i], &pcs[j]);
            let t = G * (perim_ring(&pcs[i]) + perim_ring(&pcs[j]));
            if ov / t > worst.0 {
                worst = (ov / t, i, j);
            }
        }
    }
    rep.within("pieces: no overlaps", worst.0, 1.0, || format!("{fam}: pieces {:?} and {:?}", pcs[worst.1], pcs[worst.2]));
}

/// Area shared by two convex anticlockwise polygons (Sutherland-Hodgman with `geom::clip_plain`).
fn convex_overlap(a: &[P], b: &[P]) -> f64 {
    let mut cur = a.to_vec();
    for i in 0..b.len() {
        let (p, q) = (b[i], b[(i + 1) % b.len()]);
        let e = geom::sub(q, p);
        let n = [e[1], -e[0]];
        cur = geom::clip_plain(&cur, n, geom::dot(p, n));
        if cur.len() < 3 {
            return 0.0;
        }
    }
    geom::area(&cur).max(0.0)
}

fn seg_dist_all(p: P, line: &[P]) -> f64 {
    (0..line.len() - 1).map(|i| geom::seg_dist(p, line[i], line[i + 1])).fold(f64::MAX, f64::min)
}

fn check_buffer(rep: &mut Report, rng: &R) {
    let n = 2 + rng.below(5) as usize;
    let mut line = vec![snapg([rng.range(-50.0, 50.0), rng.range(-50.0, 50.0)])];
    for _ in 1..n {
        let l = line[line.len() - 1];
        line.push(snapg([l[0] + rng.range(-60.0, 60.0), l[1] + rng.range(-60.0, 60.0)]));
    }
    let (l, r) = (if rng.below(6) == 0 { 0.0 } else { rng.range(0.5, 15.0) }, rng.range(0.5, 15.0));
    let join = [Join::Round, Join::Bevel, Join::Miter(2.0)][rng.below(3) as usize];
    let ends = if rng.below(2) == 0 { Ends::Flat } else { Ends::Square };
    let b = clip::buffer(&line, l, r, join, ends);
    let v = valid(&b);
    rep.ok("valid: buffer", v.is_ok(), || format!("{} | line {:?} l {l} r {r} {join:?} {ends:?}", v.clone().err().unwrap_or_default(), line));
    let mut ext = line.clone();
    if ends == Ends::Square {
        let m = ext.len();
        let w = l.max(r);
        let u0 = geom::mul(geom::sub(ext[1], ext[0]), 1.0 / geom::dist(ext[1], ext[0]));
        let u1 = geom::mul(geom::sub(ext[m - 1], ext[m - 2]), 1.0 / geom::dist(ext[m - 1], ext[m - 2]));
        ext[0] = geom::sub(ext[0], geom::mul(u0, w));
        ext[m - 1] = geom::add(ext[m - 1], geom::mul(u1, w));
    }
    let wmax = l.max(r);
    let mut bad = String::new();
    for i in 0..line.len() - 1 {
        let (a, c) = (line[i], line[i + 1]);
        let len = geom::dist(a, c);
        let u = geom::mul(geom::sub(c, a), 1.0 / len);
        let nl = [-u[1], u[0]];
        for t in [0.1, 0.3, 0.5, 0.7, 0.9] {
            let p = geom::lerp(a, c, t);
            for (side, w) in [(1.0, l), (-1.0, r)] {
                if w > 0.05 {
                    let q = geom::add(p, geom::mul(nl, side * (w - 0.02)));
                    if !clip::contains(&b, q) {
                        bad = format!("inside point {q:?} missing");
                    }
                }
                let q = geom::add(p, geom::mul(nl, side * (w + 0.05)));
                let far = seg_dist_all(q, &ext) > wmax + 0.03;
                let clear_of_mitres = match join {
                    Join::Miter(k) => (1..line.len() - 1).all(|j| geom::dist(q, line[j]) > k * wmax + 0.05),
                    _ => true,
                };
                if far && clear_of_mitres && clip::contains(&b, q) {
                    bad = format!("outside point {q:?} inside");
                }
            }
        }
    }
    rep.ok("buffer: points at the right distance", bad.is_empty(), || format!("{bad} | line {:?} l {l} r {r} {join:?} {ends:?}", line));
}

fn ring_dist(p: P, set: &[Shape]) -> f64 {
    let mut best = f64::MAX;
    for s in set {
        for r in std::iter::once(&s.outer).chain(s.holes.iter()) {
            for i in 0..r.len() {
                best = best.min(geom::seg_dist(p, r[i], r[(i + 1) % r.len()]));
            }
        }
    }
    best
}

fn check_offset(rep: &mut Report, rng: &R, base: &[Shape]) {
    // The region the rings stand for (a star ring can cross itself).
    let base = &clip::normalize(base)[..];
    if base.is_empty() {
        return;
    }
    let d = if rng.below(2) == 0 { rng.range(0.3, 8.0) } else { -rng.range(0.3, 8.0) };
    let round = clip::offset(base, d, Join::Round);
    let bevel = clip::offset(base, d, Join::Bevel);
    // Unlimited, so it holds the round offset (a limited mitre bevels its sharpest corners).
    let miter = clip::offset(base, d, Join::Miter(1e6));
    let limited = clip::offset(base, d, Join::Miter(2.0));
    for (name, r) in [("valid: offset round", &round), ("valid: offset bevel", &bevel), ("valid: offset mitre", &miter), ("valid: offset mitre", &limited)] {
        let v = valid(r);
        rep.ok(name, v.is_ok(), || format!("{} | d {d} | {}", v.clone().err().unwrap_or_default(), desc(base)));
    }
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for s in base {
        for p in &s.outer {
            for c in 0..2 {
                lo[c] = lo[c].min(p[c]);
                hi[c] = hi[c].max(p[c]);
            }
        }
    }
    let mut bad = String::new();
    for _ in 0..60 {
        let p = [rng.range(lo[0] - d.abs() - 2.0, hi[0] + d.abs() + 2.0), rng.range(lo[1] - d.abs() - 2.0, hi[1] + d.abs() + 2.0)];
        let dist = ring_dist(p, base);
        if (dist - d.abs()).abs() < 0.06 + 2.0 * G {
            continue;
        }
        let inb = clip::contains(base, p);
        let want = if d > 0.0 { inb || dist < d } else { inb && dist > -d };
        if clip::contains(&round, p) != want {
            bad = format!("point {p:?} (dist {dist:.4}, in base {inb}) should be {}", if want { "in" } else { "out" });
        }
    }
    rep.ok("offset: round at the right distance", bad.is_empty(), || format!("{bad} | d {d} | {}", desc(base)));
    let tol = 2.0 * G * (perim(&round) + perim(base)) + 1e-6;
    let (small, mid, big) = if d > 0.0 { (&bevel, &round, &miter) } else { (&miter, &round, &bevel) };
    let e1 = clip::area(&clip::difference(small, mid));
    let e2 = clip::area(&clip::difference(mid, big));
    rep.within("offset: bevel ⊆ round ⊆ mitre", e1.max(e2), tol, || format!("d {d} | {}", desc(base)));
}

fn check_inset_convex(rep: &mut Report, rng: &R) {
    let n = 3 + rng.below(10) as usize;
    let r = rng.range(5.0, 120.0);
    let poly = rng.convex([rng.range(-1000.0, 1000.0), rng.range(-1000.0, 1000.0)], r, n);
    if poly.len() < 3 {
        return;
    }
    let poly = if rng.below(2) == 0 { poly } else { poly.iter().rev().cloned().collect() };
    let m = poly.len();
    let d: Vec<f64> = (0..m).map(|_| if rng.below(4) == 0 { 0.0 } else { rng.range(0.0, r * 0.5) }).collect();
    let ours = clip::inset_edges(&poly, &d);
    let theirs = geom::inset(&poly, &d);
    let v = valid(&ours);
    rep.ok("valid: inset", v.is_ok(), || format!("{} | {:?} d {:?}", v.clone().err().unwrap_or_default(), poly, d));
    if theirs.len() < 3 || geom::area(&theirs).abs() < 1e-6 {
        rep.within("inset: convex = geom::inset (area)", clip::area(&ours), 2.0 * G * perim_ring(&poly), || format!("collapsed | {:?} d {:?}", poly, d));
        return;
    }
    if ours.is_empty() {
        rep.within("inset: convex = geom::inset (area)", geom::area(&theirs).abs(), 2.0 * G * perim_ring(&poly), || format!("ours empty | {:?} d {:?}", poly, d));
        return;
    }
    // Near a collapse `geom::inset` leaves corners a hair apart; its vertices mean little there.
    let tiny = (0..theirs.len()).any(|i| geom::dist(theirs[i], theirs[(i + 1) % theirs.len()]) < 0.05);
    let t = clip::region(&theirs);
    let sym = clip::area(&clip::difference(&ours, &t)) + clip::area(&clip::difference(&t, &ours));
    rep.within("inset: convex = geom::inset (area)", sym, G * (perim_ring(&theirs) + 1.0), || format!("{:?} d {:?}", poly, d));
    if tiny {
        rep.ok("inset: convex, near collapse (vertices not compared)", true, String::new);
        return;
    }
    // Vertex distances, both ways (theirs taken as exact), in grid steps scaled by how sharp the
    // corner is: two setback lines each off by a rounding meet 1/sin(half the angle) as far off.
    let tr = vec![Shape::ring(theirs.clone())];
    let k = theirs.len();
    let sharp: Vec<f64> = (0..k)
        .map(|i| {
            let (a, b) = (geom::sub(theirs[(i + k - 1) % k], theirs[i]), geom::sub(theirs[(i + 1) % k], theirs[i]));
            let (la, lb) = (geom::len(a), geom::len(b));
            if la < 1e-9 || lb < 1e-9 {
                return 1.0;
            }
            let ang = geom::cross(a, b).abs().atan2(geom::dot(a, b));
            (1.0 / (ang * 0.5).sin()).max(1.0)
        })
        .collect();
    let scale = |p: P| (0..k).filter(|&i| geom::dist(p, theirs[i]) < 0.25).map(|i| sharp[i]).fold(1.0, f64::max);
    let mut worst: f64 = 0.0;
    for s in &ours {
        for p in &s.outer {
            worst = worst.max(ring_dist(*p, &tr) / scale(*p));
        }
    }
    for (i, p) in theirs.iter().enumerate() {
        worst = worst.max(ring_dist(*p, &ours) / sharp[i]);
    }
    rep.within("inset: convex = geom::inset (vertices, ft)", worst, G, || format!("{:?} d {:?}", poly, d));
}

fn check_inset_concave(rep: &mut Report, rng: &R) {
    let n = 5 + rng.below(14) as usize;
    let r = rng.range(20.0, 150.0);
    let poly = if rng.below(2) == 0 {
        rng.star([0.0, 0.0], r, n, 0.45)
    } else {
        rng.comb([0.0, 0.0], r, 1 + rng.below(4) as usize, 0.0).into_iter().map(snapg).collect()
    };
    let m = poly.len();
    let d: Vec<f64> = (0..m).map(|_| if rng.below(4) == 0 { 0.0 } else { rng.range(0.0, r * 0.08) }).collect();
    let ours = clip::inset_edges(&poly, &d);
    let v = valid(&ours);
    rep.ok("valid: inset", v.is_ok(), || format!("{} | {:?} d {:?}", v.clone().err().unwrap_or_default(), poly, d));
    let base = clip::region(&poly);
    let out = clip::area(&clip::difference(&ours, &base));
    rep.within("inset: concave inside its ring", out, G * perim_ring(&poly), || format!("{:?} d {:?}", poly, d));
    // Before the first skeleton event the result is exactly the ring of the setback lines' meeting
    // points: compare against it there.
    let Some(want) = skeleton_offset(&poly, &d) else {
        rep.ok("inset: concave, cases with an event (not compared)", true, String::new);
        return;
    };
    let w = clip::region(&want);
    let sym = clip::area(&clip::difference(&ours, &w)) + clip::area(&clip::difference(&w, &ours));
    rep.within("inset: concave = skeleton offset (area)", sym, 2.0 * G * perim_ring(&poly), || format!("{:?} d {:?}", poly, d));
    let wr = vec![Shape::ring(want.clone())];
    let k = want.len();
    let sharp: Vec<f64> = (0..k)
        .map(|i| {
            let (a, b) = (geom::sub(want[(i + k - 1) % k], want[i]), geom::sub(want[(i + 1) % k], want[i]));
            let ang = geom::cross(a, b).abs().atan2(geom::dot(a, b));
            (1.0 / (ang * 0.5).sin()).max(1.0)
        })
        .collect();
    let scale = |p: P| (0..k).filter(|&i| geom::dist(p, want[i]) < 0.25).map(|i| sharp[i]).fold(1.0, f64::max);
    let mut worst: f64 = 0.0;
    for s in &ours {
        for p in s.outer.iter().chain(s.holes.iter().flatten()) {
            worst = worst.max(ring_dist(*p, &wr) / scale(*p));
        }
    }
    for (i, p) in want.iter().enumerate() {
        worst = worst.max(ring_dist(*p, &ours) / sharp[i]);
    }
    rep.within("inset: concave = skeleton offset (vertices, ft)", worst, 4.0 * G, || format!("{:?} d {:?}", poly, d));
}

/// The ring of consecutive setback lines' meeting points of an anticlockwise ring, if no edge has
/// turned round or shrunk away and it does not cross itself (no straight-skeleton event yet).
fn skeleton_offset(poly: &[P], d: &[f64]) -> Option<Vec<P>> {
    let m = poly.len();
    let u: Vec<P> = (0..m).map(|i| { let e = geom::sub(poly[(i + 1) % m], poly[i]); geom::mul(e, 1.0 / geom::len(e)) }).collect();
    let n: Vec<P> = u.iter().map(|e| [-e[1], e[0]]).collect();
    let mut q = Vec::with_capacity(m);
    for i in 0..m {
        let h = (i + m - 1) % m;
        let det = geom::cross(n[h], n[i]);
        if det.abs() < 1e-3 {
            return None;
        }
        let (ch, ci) = (geom::dot(poly[h], n[h]) + d[h], geom::dot(poly[i], n[i]) + d[i]);
        q.push([(ch * n[i][1] - ci * n[h][1]) / det, (n[h][0] * ci - n[i][0] * ch) / det]);
    }
    for i in 0..m {
        if geom::dot(geom::sub(q[(i + 1) % m], q[i]), u[i]) <= 0.05 {
            return None;
        }
    }
    let o = |a: P, b: P, c: P| geom::cross(geom::sub(b, a), geom::sub(c, a));
    for i in 0..m {
        for j in i + 1..m {
            if j == i + 1 || (i == 0 && j == m - 1) {
                continue;
            }
            let (a, b, c, e) = (q[i], q[(i + 1) % m], q[j], q[(j + 1) % m]);
            if o(a, b, c) * o(a, b, e) <= 0.0 && o(c, e, a) * o(c, e, b) <= 0.0 {
                return None;
            }
            if geom::seg_dist(c, a, b) < 0.05 || geom::seg_dist(a, c, e) < 0.05 {
                return None;
            }
        }
    }
    (geom::area(&q) > 0.0).then_some(q)
}

fn check_split(rep: &mut Report, rng: &R, set: &[Shape]) {
    if set.is_empty() {
        return;
    }
    let s0 = &set[0].outer;
    let p = geom::lerp(s0[0], s0[s0.len() / 2], rng.range(0.2, 0.8));
    let a = rng.range(0.0, std::f64::consts::TAU);
    let dir = [a.cos(), a.sin()];
    let (l, r) = clip::split(set, p, dir);
    for (name, x) in [("valid: split left", &l), ("valid: split right", &r)] {
        let v = valid(x);
        rep.ok(name, v.is_ok(), || format!("{} | p {p:?} dir {dir:?} | {}", v.clone().err().unwrap_or_default(), desc(set)));
    }
    let tol = 2.0 * G * (perim(set) + perim(&l) + perim(&r)) + 1e-6;
    rep.within("split: sides add up", (clip::area(&l) + clip::area(&r) - clip::area(set)).abs(), tol, || format!("p {p:?} dir {dir:?} | {}", desc(set)));
    let mut worst: f64 = 0.0;
    for (side, x) in [(1.0, &l), (-1.0, &r)] {
        for s in x.iter() {
            for q in s.outer.iter().chain(s.holes.iter().flatten()) {
                worst = worst.max(-side * geom::cross(dir, geom::sub(*q, p)));
            }
        }
    }
    rep.within("split: sides on their side (ft)", worst, 2.0 * G, || format!("p {p:?} dir {dir:?} | {}", desc(set)));
    rep.within("split: sides apart", clip::area(&clip::intersection(&l, &r)), tol, || format!("p {p:?} dir {dir:?}"));
}

// ---------------------------------------------------------------------------------------------
// Families.

fn grid_blocks(rng: &R, nx: usize, ny: usize) -> Vec<Vec<P>> {
    let s = rng.range(20.0, 80.0);
    let rot = rng.range(0.0, 3.2);
    let (cs, sn) = (rot.cos(), rot.sin());
    let mut pts = vec![vec![[0.0; 2]; ny + 1]; nx + 1];
    for (i, col) in pts.iter_mut().enumerate() {
        for (j, p) in col.iter_mut().enumerate() {
            let (x, y) = (i as f64 * s + rng.range(-0.3, 0.3) * s, j as f64 * s + rng.range(-0.3, 0.3) * s);
            *p = [x * cs - y * sn, x * sn + y * cs];
        }
    }
    let mut out = Vec::new();
    for i in 0..nx {
        for j in 0..ny {
            out.push(vec![pts[i][j], pts[i + 1][j], pts[i + 1][j + 1], pts[i][j + 1]]);
        }
    }
    out
}

fn degenerate_pair(rng: &R) -> (Vec<Shape>, Vec<Shape>) {
    let a = rng.star([0.0, 0.0], rng.range(20.0, 100.0), 6 + rng.below(10) as usize, 0.5);
    let n = a.len();
    let c = geom::mean(&a);
    // B: some of A's vertices, points on A's edges, points inside and out, by angle round A's middle.
    let mut b: Vec<(f64, P)> = Vec::new();
    for i in 0..n {
        let roll = rng.below(4);
        let p = match roll {
            0 => a[i],
            1 => geom::lerp(a[i], a[(i + 1) % n], rng.range(0.1, 0.9)),
            2 => geom::lerp(c, a[i], rng.range(0.3, 1.4)),
            _ => continue,
        };
        let p = if rng.below(2) == 0 { snapg(p) } else { p };
        b.push(((p[1] - c[1]).atan2(p[0] - c[0]), p));
    }
    b.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut bb: Vec<P> = b.into_iter().map(|x| x.1).collect();
    let mut aa = a.clone();
    // Spikes and repeated points.
    for v in [&mut aa, &mut bb] {
        if v.len() > 2 && rng.below(2) == 0 {
            let k = rng.below(v.len() as u32) as usize;
            let p = v[k];
            v.insert(k, p);
        }
        if v.len() > 2 && rng.below(2) == 0 {
            let k = rng.below(v.len() as u32) as usize;
            let p = v[k];
            let s = [p[0] + rng.range(-5.0, 5.0), p[1] + rng.range(-5.0, 5.0)];
            v.splice(k + 1..k + 1, [s, p]);
        }
    }
    (ring(aa), ring(bb))
}

fn checker(rng: &R) -> (Vec<Shape>, Vec<Shape>) {
    let s = rng.range(5.0, 30.0).round();
    let (n, m) = (2 + rng.below(4) as i32, 2 + rng.below(4) as i32);
    let sq = |i: i32, j: i32| vec![[i as f64 * s, j as f64 * s], [(i + 1) as f64 * s, j as f64 * s], [(i + 1) as f64 * s, (j + 1) as f64 * s], [i as f64 * s, (j + 1) as f64 * s]];
    let mut a = Vec::new();
    let mut b = Vec::new();
    for i in 0..n {
        for j in 0..m {
            if (i + j) % 2 == 0 {
                a.push(Shape::ring(sq(i, j)));
            } else if rng.below(2) == 0 {
                b.push(Shape::ring(sq(i, j)));
            }
        }
    }
    // A triangle with a corner on a square's edge or corner.
    let t = [s * 0.5, 0.0];
    b.push(Shape::ring(vec![t, [s * 0.8, s * 0.6], [s * 0.2, s * 0.6]]));
    (a, b)
}

fn with_holes(rng: &R) -> Vec<Shape> {
    let r = rng.range(60.0, 200.0);
    let base = if rng.below(2) == 0 { rng.star([0.0, 0.0], r, 10 + rng.below(40) as usize, 0.7) } else { rng.convex([0.0, 0.0], r, 6 + rng.below(30) as usize) };
    let mut cut = Vec::new();
    for _ in 0..1 + rng.below(5) {
        let c = [rng.range(-r * 0.6, r * 0.6), rng.range(-r * 0.6, r * 0.6)];
        let rr = rng.range(3.0, r * 0.25);
        cut.push(Shape::ring(if rng.below(2) == 0 { rng.convex(c, rr, 4 + rng.below(8) as usize) } else { rng.star(c, rr, 5 + rng.below(8) as usize, 0.5) }));
    }
    clip::difference(&ring(base), &cut)
}

fn main() {
    let cases: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut rep = Report::default();
    let t0 = Instant::now();
    for fam in 0..8u64 {
        let rng = R::new(seed, 100 + fam);
        for i in 0..cases {
            let pieces = i % 4 == 0;
            match fam {
                0 => {
                    let a = rng.convex([0.0, 0.0], rng.range(5.0, 100.0), 3 + rng.below(20) as usize);
                    let b = rng.convex([rng.range(-60.0, 60.0), rng.range(-60.0, 60.0)], rng.range(5.0, 100.0), 3 + rng.below(20) as usize);
                    check_pair(&mut rep, &rng, "convex", &ring(a), &ring(b), pieces);
                }
                1 => {
                    let a = rng.star([0.0, 0.0], rng.range(5.0, 100.0), 5 + rng.below(30) as usize, 0.3);
                    let b = rng.star([rng.range(-50.0, 50.0), rng.range(-50.0, 50.0)], rng.range(5.0, 100.0), 5 + rng.below(30) as usize, 0.3);
                    let a = ring(a);
                    check_pair(&mut rep, &rng, "star", &a, &ring(b), pieces);
                    if i % 4 == 1 {
                        check_offset(&mut rep, &rng, &a);
                    }
                }
                2 => {
                    let w = rng.range(20.0, 120.0);
                    let a = rng.comb([0.0, 0.0], w, 1 + rng.below(6) as usize, rng.range(0.0, 3.2));
                    let b = rng.comb([rng.range(-w, w) * 0.5, rng.range(-w, w) * 0.5], w, 1 + rng.below(6) as usize, rng.range(0.0, 3.2));
                    check_pair(&mut rep, &rng, "comb", &ring(a), &ring(b), pieces);
                }
                3 => {
                    let blocks = grid_blocks(&rng, 2 + rng.below(4) as usize, 2 + rng.below(4) as usize);
                    let (mut a, mut b) = (Vec::new(), Vec::new());
                    for q in blocks {
                        match rng.below(3) {
                            0 => a.push(Shape::ring(q)),
                            1 => b.push(Shape::ring(q)),
                            _ => {
                                a.push(Shape::ring(q.clone()));
                                b.push(Shape::ring(q));
                            }
                        }
                    }
                    check_pair(&mut rep, &rng, "grid", &a, &b, pieces);
                }
                4 => {
                    let (a, b) = degenerate_pair(&rng);
                    check_pair(&mut rep, &rng, "degenerate", &a, &b, pieces);
                }
                5 => {
                    let (a, b) = checker(&rng);
                    check_pair(&mut rep, &rng, "checker", &a, &b, pieces);
                }
                6 => {
                    let a = with_holes(&rng);
                    let r = rng.range(20.0, 120.0);
                    let b = rng.star([rng.range(-80.0, 80.0), rng.range(-80.0, 80.0)], r, 5 + rng.below(12) as usize, 0.4);
                    check_pair(&mut rep, &rng, "holes", &a, &ring(b), pieces);
                    if i % 2 == 0 {
                        check_pieces(&mut rep, "holes", &a);
                        check_split(&mut rep, &rng, &a);
                    } else {
                        check_offset(&mut rep, &rng, &a);
                    }
                }
                _ => {
                    check_buffer(&mut rep, &rng);
                    check_inset_convex(&mut rep, &rng);
                    check_inset_concave(&mut rep, &rng);
                    // Bad input never panics and gives nothing.
                    let bad = ring(vec![[0.0, 0.0], [f64::NAN, 1.0], [1.0, 1.0]]);
                    let inf = ring(vec![[0.0, 0.0], [f64::INFINITY, 1.0], [1.0, 1.0]]);
                    let ok = clip::union(&bad, &ring(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])).is_empty() && clip::normalize(&inf).is_empty() && clip::convex_pieces(&bad).is_empty();
                    rep.ok("bad input: empty", ok, || "NaN or infinite input gave shapes".into());
                }
            }
        }
    }
    let checks = t0.elapsed().as_secs_f64();

    // Timings.
    let rng = R::new(seed, 7);
    let block = ring(rng.star([0.0, 0.0], 200.0, 40, 0.75));
    let mut cutters = Vec::new();
    for k in 0..20 {
        let c = [rng.range(-180.0, 180.0), rng.range(-180.0, 180.0)];
        let n = 4 + (k % 9);
        cutters.push(Shape::ring(if k % 2 == 0 { rng.convex(c, rng.range(5.0, 25.0), n) } else { rng.star(c, rng.range(5.0, 25.0), n, 0.5) }));
    }
    let time = |f: &mut dyn FnMut() -> usize, reps: usize| -> (f64, usize) {
        let mut out = f();
        let t = Instant::now();
        for _ in 0..reps {
            out = f();
        }
        (t.elapsed().as_secs_f64() * 1e3 / reps as f64, out)
    };
    let (t_diff, n_diff) = time(&mut || clip::difference(&block, &cutters).len(), 2000);
    let mut holed: Vec<Shape> = Vec::new();
    for _ in 0..100 {
        let outer = rng.star([0.0, 0.0], 200.0, 60, 0.7);
        let hs: Vec<Shape> = [[-80.0, 0.0], [60.0, 60.0], [40.0, -70.0]].iter().map(|c| Shape::ring(rng.convex(*c, rng.range(10.0, 30.0), 4 + rng.below(5) as usize))).collect();
        holed = clip::difference(&ring(outer), &hs);
        if holed.len() == 1 && holed[0].holes.len() == 3 {
            break;
        }
    }
    let nv = holed.iter().map(|s| s.outer.len()).sum::<usize>();
    let (t_pcs, n_pcs) = time(&mut || clip::convex_pieces(&holed).len(), 2000);
    let (t_norm, _) = time(&mut || clip::normalize(&holed).len(), 2000);
    let grid: Vec<Shape> = grid_blocks(&rng, 10, 10).into_iter().map(Shape::ring).collect();
    let (t_grid, _) = time(&mut || clip::union_all(&grid).len(), 300);
    let (t_off, _) = time(&mut || clip::offset(&block, -6.0, Join::Round).len(), 500);
    let line: Vec<P> = (0..12).map(|i| [i as f64 * 40.0, if i % 2 == 0 { 0.0 } else { 25.0 }]).collect();
    let (t_buf, _) = time(&mut || clip::buffer(&line, 10.0, 10.0, Join::Round, Ends::Square).len(), 1000);

    let mut failed = 0;
    println!("clipcheck: {cases} cases per family, seed {seed}, checks took {checks:.1} s");
    for (name, c) in &rep.0 {
        let worst = if c.fails > 0 { format!("worst {:.3}", c.worst) } else { format!("worst {:.3} of tolerance", c.worst) };
        println!("  {:<46} {:>7} cases {:>5} failed  {}", name, c.cases, c.fails, worst);
        if c.fails > 0 {
            failed += c.fails;
            println!("      worst: {}", c.worst_note);
            for n in c.notes.iter().take(2) {
                println!("      e.g. {n}");
            }
        }
    }
    println!("timings (ms per call):");
    println!("  difference: 40-vertex block minus 20 shapes of 4-12 vertices  {t_diff:.4}  ({n_diff} shapes)");
    println!("  convex_pieces: {nv}-vertex shape with {} holes          {t_pcs:.4}  ({n_pcs} pieces)", holed.first().map_or(0, |s| s.holes.len()));
    println!("    of which normalize (its input made canonical)          {t_norm:.4}");
    println!("  union_all: 100 grid blocks sharing edges                   {t_grid:.4}");
    println!("  offset: 40-vertex block shrunk 6 ft, round                  {t_off:.4}");
    println!("  buffer: 12-point polyline, 10 ft each side, round, square   {t_buf:.4}");
    if failed > 0 {
        println!("FAILED: {failed}");
        std::process::exit(1);
    }
    println!("all passed");
}
