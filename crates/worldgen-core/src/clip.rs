//! Integer polygon booleans for hand-laid town geometry: union, difference and intersection of
//! regions with holes, offsets (a region grown or shrunk, a polyline buffered to a band, a ring set
//! back by its own distance per edge), a region split by a line, and convex pieces for cutting lots.
//!
//! Method (written here, not ported): points are rounded to a grid of 1/256 ft held in `i64`, and
//! every predicate on grid points is exact (`i128` cross products), so no tolerance ever decides
//! the topology. The edges of all inputs are split where they cross by snap rounding: each crossing
//! is rounded to its grid point (a hot pixel), every edge passing through a hot pixel's square is
//! bent through its centre, and bent pieces are split again at any hot centre lying on them. The
//! pieces then meet only at their ends or lie exactly on each other, and pieces lying on each other
//! are merged, adding up their windings. An exact ray count gives each piece's winding numbers on
//! both sides; the pieces where the operation's predicate differs between the sides are kept, linked
//! into rings turning by exact angle order (sharpest left turn first), split where a ring touches
//! itself, and each hole goes into the smallest outer ring holding it.
//!
//! Offsets, the per-edge setback and splits by a line build pieces (edge strips, corner joins,
//! half-planes) and run them through the same booleans; convex pieces are cut on the grid with the
//! same exact predicates.
//!
//! Guarantees: output vertices are grid points and every output edge lies within 0.71/256 ft (half
//! a grid square's diagonal) of the exact result's boundary; shared edges (a block and the street band beside it), collinear overlaps, a
//! vertex on another's edge, rings touching at a point, zero-area spikes and repeated points need no
//! care from callers. NaN, infinite or out-of-range input (beyond 30,000,000 ft) gives an empty
//! result, and nothing panics. Inputs are read by the positive winding rule (each ring is first
//! turned so outers run anticlockwise and holes clockwise): shapes of one set overlapping are their
//! union, a hole only takes area from its own shape. Output is canonical: outers anticlockwise
//! (positive `geom::area`), holes clockwise, no repeated or collinear vertices, each ring starting at
//! its least vertex (x, then y), holes and shapes in the order of their rings. It depends on the
//! regions only, so the same input with rings permuted, reversed or starting elsewhere gives the
//! same bits.

// Nothing in generation calls this yet: it is the geometry for P3's hand-laid towns (town/hand.rs).
#![allow(dead_code)]

use super::geom::{self, P};
use crate::hash::FastMap;
use crate::{ceil, fabs, round, sqrt};

/// Grid steps per foot.
const SCALE: f64 = 256.0;
/// Coordinates beyond this many feet make an input invalid (keeps every product inside `i128`).
const LIMIT_FT: f64 = 3.0e7;
/// Corners within this many grid steps of straight count as straight when cutting convex pieces:
/// a cut's far end is rounded to the grid, so the corner it leaves is only straight to that.
const TOL: f64 = 1.0;
/// How far a setback strip overshoots its end into its neighbour's, ft (two grid steps: more than
/// the rounding of either strip's corners).
const SEAM: f64 = 2.0 / SCALE;
/// Greatest gap between a round join's chords and its arc, ft.
const ROUND_TOL: f64 = 0.05;

type I = [i64; 2];

/// A region: an outer ring and the holes in it. Shapes coming out of this module are canonical (see
/// the module comment); shapes going in may have either orientation and repeated points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    pub outer: Vec<P>,
    pub holes: Vec<Vec<P>>,
}

impl Shape {
    /// A shape with no holes.
    pub fn ring(outer: Vec<P>) -> Self {
        Shape { outer, holes: Vec::new() }
    }

    /// Area: the outer's less the holes' (for canonical shapes).
    pub fn area(&self) -> f64 {
        geom::area(&self.outer) + self.holes.iter().map(|h| geom::area(h)).sum::<f64>()
    }
}

/// How an offset fills the gap at a corner it opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Join {
    /// Sharp, while the point is within this many offsets of the corner (Clipper's limit; 2 is
    /// usual); beyond it, bevelled.
    Miter(f64),
    Bevel,
    /// An arc, in chords no more than 0.05 ft inside it.
    Round,
}

/// The ends of a buffered polyline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ends {
    /// Cut square at the end points.
    Flat,
    /// Cut square past each end point by the larger half-width.
    Square,
}

// ---------------------------------------------------------------------------------------------
// Public operations.

/// A ring (either orientation) as a canonical region; a ring crossing itself keeps the parts it
/// winds round anticlockwise once it is turned to run anticlockwise overall.
pub fn region(ring: &[P]) -> Vec<Shape> {
    normalize(&[Shape::ring(ring.to_vec())])
}

/// The canonical form of a set of shapes: their union.
pub fn normalize(set: &[Shape]) -> Vec<Shape> {
    run(set, &[], Op::Union)
}

/// The union of many shapes (the same as `normalize`).
pub fn union_all(set: &[Shape]) -> Vec<Shape> {
    run(set, &[], Op::Union)
}

pub fn union(a: &[Shape], b: &[Shape]) -> Vec<Shape> {
    run(a, b, Op::Union)
}

pub fn intersection(a: &[Shape], b: &[Shape]) -> Vec<Shape> {
    run(a, b, Op::Inter)
}

/// What of `a` is not in `b`.
pub fn difference(a: &[Shape], b: &[Shape]) -> Vec<Shape> {
    run(a, b, Op::Diff)
}

/// Total area of canonical shapes.
pub fn area(set: &[Shape]) -> f64 {
    set.iter().map(Shape::area).sum()
}

/// Whether a point is inside canonical shapes (in an outer and in none of its holes).
pub fn contains(set: &[Shape], p: P) -> bool {
    set.iter().any(|s| geom::contains(&s.outer, p) && !s.holes.iter().any(|h| geom::contains(h, p)))
}

/// Shapes grown by `d` ft (shrunk when `d` is negative), corners filled by `join`. Grown: the shapes
/// and every point within `d` of their outlines on the outside, with round joins (sharp corners
/// with the others). Shrunk: what is left after taking away everything within `-d` of the outlines.
pub fn offset(set: &[Shape], d: f64, join: Join) -> Vec<Shape> {
    if !d.is_finite() {
        return Vec::new();
    }
    let base = normalize(set);
    if fabs(d) * SCALE < 0.5 || base.is_empty() {
        return base;
    }
    let grow = d > 0.0;
    let (r, s) = (fabs(d), if grow { 1.0 } else { -1.0 });
    let mut pieces: Vec<Shape> = Vec::new();
    for sh in &base {
        for ring in std::iter::once(&sh.outer).chain(sh.holes.iter()) {
            let n = ring.len();
            // Canonical rings have the region on their left: grown pieces go right, shrunk ones left.
            let norms: Vec<P> = (0..n)
                .map(|i| {
                    let e = geom::sub(ring[(i + 1) % n], ring[i]);
                    let l = geom::len(e);
                    [s * e[1] / l, -s * e[0] / l]
                })
                .collect();
            for i in 0..n {
                let (a, b, c) = (ring[i], ring[(i + 1) % n], ring[(i + 2) % n]);
                let (n1, n2) = (norms[i], norms[(i + 1) % n]);
                pieces.push(Shape::ring(vec![a, b, off(b, n1, r), off(a, n1, r)]));
                // A corner turning away from the offset side opens a gap there.
                if s * geom::cross(geom::sub(b, a), geom::sub(c, b)) > 0.0 {
                    pieces.push(Shape::ring(join_piece(b, n1, n2, off(b, n1, r), off(b, n2, r), r, grow, join)));
                }
            }
        }
    }
    if grow {
        union(&base, &pieces)
    } else {
        difference(&base, &pieces)
    }
}

/// A polyline buffered to a band: `left` ft on its left (going from the first point), `right` ft on
/// its right, bends filled by `join`, ends cut by `ends`.
pub fn buffer(line: &[P], left: f64, right: f64, join: Join, ends: Ends) -> Vec<Shape> {
    if !(left.is_finite() && right.is_finite()) {
        return Vec::new();
    }
    let (l, r) = (left.max(0.0), right.max(0.0));
    if (l + r) * SCALE < 0.5 {
        return Vec::new();
    }
    let mut pts: Vec<P> = Vec::with_capacity(line.len());
    for p in line {
        let Some(q) = grid(*p) else { return Vec::new() };
        let q = feet(q);
        if pts.last() != Some(&q) {
            pts.push(q);
        }
    }
    let ext = l.max(r);
    if pts.len() == 1 {
        if ends == Ends::Flat {
            return Vec::new();
        }
        let p = pts[0];
        return region(&[[p[0] - ext, p[1] - ext], [p[0] + ext, p[1] - ext], [p[0] + ext, p[1] + ext], [p[0] - ext, p[1] + ext]]);
    }
    let m = pts.len();
    if ends == Ends::Square {
        let u0 = unit(geom::sub(pts[1], pts[0]));
        let u1 = unit(geom::sub(pts[m - 1], pts[m - 2]));
        pts[0] = geom::sub(pts[0], geom::mul(u0, ext));
        pts[m - 1] = geom::add(pts[m - 1], geom::mul(u1, ext));
    }
    let nl: Vec<P> = (0..m - 1)
        .map(|i| {
            let u = unit(geom::sub(pts[i + 1], pts[i]));
            [-u[1], u[0]]
        })
        .collect();
    let mut pieces: Vec<Shape> = Vec::new();
    for i in 0..m - 1 {
        let (a, b, n) = (pts[i], pts[i + 1], nl[i]);
        pieces.push(Shape::ring(vec![off(a, n, l), off(a, n, -r), off(b, n, -r), off(b, n, l)]));
    }
    for i in 1..m - 1 {
        let v = pts[i];
        let (n1, n2) = (nl[i - 1], nl[i]);
        let t = geom::cross(geom::sub(v, pts[i - 1]), geom::sub(pts[i + 1], v));
        if t > 0.0 && r > 0.0 {
            // Turning left opens the right side.
            let (m1, m2) = ([-n1[0], -n1[1]], [-n2[0], -n2[1]]);
            pieces.push(Shape::ring(join_piece(v, m1, m2, off(v, n1, -r), off(v, n2, -r), r, true, join)));
        } else if t < 0.0 && l > 0.0 {
            pieces.push(Shape::ring(join_piece(v, n1, n2, off(v, n1, l), off(v, n2, l), l, false, join)));
        }
    }
    normalize(&pieces)
}

/// A simple ring shrunk by its own distance per edge (`d[i]` for the edge from point i to i + 1,
/// 0 allowed), either orientation: the weighted straight-skeleton offset, mitred at every corner.
/// Each edge takes away its face of the skeleton (from the edge to the two mitre points) while it
/// and its neighbours keep their setback lines the right way round; next to an edge that has
/// shrunk away it takes its whole strip up to its neighbours' lines instead. A convex ring so gives
/// `geom::inset`'s polygon (to the grid); a concave one is exact until the skeleton's first event
/// and close after it (a strip is never carried past a neighbouring edge ending in a reflex corner).
/// May come apart into several shapes.
pub fn inset_edges(ring: &[P], d: &[f64]) -> Vec<Shape> {
    let n = ring.len();
    if n < 3 || d.len() < n {
        return Vec::new();
    }
    let mut pts: Vec<P> = Vec::with_capacity(n);
    let mut ds: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let (Some(a), Some(b)) = (grid(ring[i]), grid(ring[(i + 1) % n])) else { return Vec::new() };
        if !d[i].is_finite() {
            return Vec::new();
        }
        if a != b {
            pts.push(feet(a));
            ds.push(d[i].max(0.0));
        }
    }
    let m = pts.len();
    if m < 3 {
        return Vec::new();
    }
    let a2 = geom::area(&pts);
    if a2 == 0.0 {
        return Vec::new();
    }
    if a2 < 0.0 {
        // Reversed, edge j runs from old point m-1-j to m-2-j: old edge m-2-j backwards.
        let old = ds.clone();
        pts.reverse();
        for j in 0..m {
            ds[j] = old[(2 * m - 2 - j) % m];
        }
    }
    let pi: Vec<I> = pts.iter().map(|p| grid(*p).unwrap_or([0, 0])).collect();
    // The turn at point i (between edge i-1 and edge i), exact: >0 convex, <0 reflex.
    let turn: Vec<i128> = (0..m).map(|i| orient(pi[(i + m - 1) % m], pi[i], pi[(i + 1) % m]).signum()).collect();
    if turn.iter().all(|&t| t >= 0) {
        // Convex: the half-planes themselves, rounded once.
        return region(&geom::inset(&pts, &ds));
    }
    let base = region(&pts);
    let u: Vec<P> = (0..m).map(|i| unit(geom::sub(pts[(i + 1) % m], pts[i]))).collect();
    let inner: Vec<P> = u.iter().map(|e| [-e[1], e[0]]).collect();
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in &pts {
        for c in 0..2 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    let dmax = ds.iter().cloned().fold(0.0, f64::max);
    let reach = geom::dist(lo, hi) + 4.0 * dmax + 1.0;
    let pad = dmax + 1.0;
    // A point in edge e's own part of the setback (off the edge line even when its distance is 0).
    let zone = |e: usize| geom::add(geom::lerp(pts[e], pts[(e + 1) % m], 0.5), geom::mul(inner[e], ds[e] * 0.5 + 0.01));
    // Where the setback lines of the two edges at each corner meet (none at a straight corner),
    // computed once so the two faces sharing it meet exactly.
    let mitre: Vec<Option<P>> = (0..m)
        .map(|v| {
            let h = (v + m - 1) % m;
            let det = geom::cross(inner[h], inner[v]);
            if turn[v] == 0 || fabs(det) < 1e-12 {
                return None;
            }
            let (ch, cv) = (geom::dot(pts[h], inner[h]) + ds[h], geom::dot(pts[v], inner[v]) + ds[v]);
            Some([(ch * inner[v][1] - cv * inner[h][1]) / det, (inner[h][0] * cv - inner[v][0] * ch) / det])
        })
        .collect();
    // The ends of edge e's setback line: the mitre points, or square off at a straight corner.
    let ends = |e: usize| {
        let f = (e + 1) % m;
        (mitre[e].unwrap_or(off(pts[e], inner[e], ds[e])), mitre[f].unwrap_or(off(pts[f], inner[e], ds[e])))
    };
    // An edge whose setback line has turned round or shrunk away: the skeleton has had an event.
    let flipped: Vec<bool> = (0..m)
        .map(|e| {
            let (p, q) = ends(e);
            geom::dot(geom::sub(q, p), u[e]) <= 1.0 / SCALE
        })
        .collect();
    let mut pieces: Vec<Shape> = Vec::new();
    for i in 0..m {
        // Even a setback under a grid step takes its face: the faces share their mitre points, so
        // leaving one out would leave its sliver.
        let di = ds[i];
        let (h, j) = ((i + m - 1) % m, (i + 1) % m);
        let quad = !flipped[h] && !flipped[i] && !flipped[j];
        if di <= 0.0 {
            // No setback: the face is flat, but once the neighbours' mitre points are rounded off
            // the edge's line a hair-thin sliver back to the corner would stay. Its hull on the grid
            // covers that (taking at most a grid step off the result along the edge).
            if quad {
                let (ea, eb) = ends(i);
                let ps: Vec<P> = [pts[i], pts[j], eb, ea].iter().filter_map(|p| grid(*p)).map(feet).collect();
                let hl = geom::hull(ps);
                if hl.len() >= 3 {
                    pieces.push(Shape::ring(hl));
                }
            }
            continue;
        }
        if quad {
            // No event here: the edge's face of the straight skeleton, exactly.
            let (ea, eb) = ends(i);
            pieces.push(Shape::ring(vec![pts[i], pts[j], eb, ea]));
            continue;
        }
        // Beside an event: the edge's whole strip, ended as below (exact for a convex ring).
        let (a, b) = (pts[i], pts[j]);
        let (ui, ni) = (u[i], inner[i]);
        // The edge's strip, from a little outside it to its setback, long enough to meet any end.
        let back = geom::mul(ni, -0.05);
        let fwd = geom::mul(ni, di);
        let a0 = geom::sub(a, geom::mul(ui, reach));
        let b0 = geom::add(b, geom::mul(ui, reach));
        let mut z = vec![geom::add(a0, back), geom::add(b0, back), geom::add(b0, fwd), geom::add(a0, fwd)];
        // Its ends: past a neighbour's line at a convex corner, at the mitre at a reflex one, square
        // at a straight one. Each overshoots by `SEAM` so neighbouring strips overlap and leave no
        // sliver between their rounded edges (the overshoot lies outside the ring or in the
        // neighbour's own setback).
        let mut cap_b = None;
        z = match turn[j] {
            t if t > 0 => {
                let k = (j + 1) % m;
                if turn[k] < 0 {
                    cap_b = mitre_side(pts[k], inner[j], ds[j], inner[k], ds[k], zone(j), 0.0);
                }
                geom::clip_plain(&z, geom::mul(inner[j], -1.0), SEAM - geom::dot(b, inner[j]))
            }
            t if t < 0 => end_at_mitre(&z, b, ni, di, inner[j], ds[j], a, ui),
            _ => geom::clip_plain(&z, ui, geom::dot(b, ui) + SEAM),
        };
        let mut cap_a = None;
        z = match turn[i] {
            t if t > 0 => {
                let g = (h + m - 1) % m;
                if turn[h] < 0 {
                    cap_a = mitre_side(pts[h], inner[h], ds[h], inner[g], ds[g], zone(h), 0.0);
                }
                geom::clip_plain(&z, geom::mul(inner[h], -1.0), SEAM - geom::dot(a, inner[h]))
            }
            t if t < 0 => end_at_mitre(&z, a, ni, di, inner[h], ds[h], b, geom::mul(ui, -1.0)),
            _ => geom::clip_plain(&z, geom::mul(ui, -1.0), SEAM - geom::dot(a, ui)),
        };
        // A convex end whose neighbouring edge ends in a reflex corner: the part of the strip past
        // the end stops at that corner's mitre line (the edge's own stretch is not cut).
        let mut parts = Vec::new();
        if cap_a.is_none() && cap_b.is_none() {
            parts.push(z);
        } else {
            let (sa, sb) = (geom::dot(a, ui), geom::dot(b, ui));
            let mut body = geom::clip_plain(&z, ui, sb);
            body = geom::clip_plain(&body, geom::mul(ui, -1.0), -sa);
            parts.push(body);
            let mut eb = geom::clip_plain(&z, geom::mul(ui, -1.0), -sb);
            if let Some((nrm, c)) = cap_b {
                eb = geom::clip_plain(&eb, nrm, c);
            }
            parts.push(eb);
            let mut ea = geom::clip_plain(&z, ui, sa);
            if let Some((nrm, c)) = cap_a {
                ea = geom::clip_plain(&ea, nrm, c);
            }
            parts.push(ea);
        }
        for mut z in parts {
            for (c, nrm) in [(hi[0] + pad, [1.0, 0.0]), (hi[1] + pad, [0.0, 1.0]), (-(lo[0] - pad), [-1.0, 0.0]), (-(lo[1] - pad), [0.0, -1.0])] {
                z = geom::clip_plain(&z, nrm, c);
            }
            if z.len() >= 3 {
                pieces.push(Shape::ring(z));
            }
        }
    }
    difference(&base, &pieces)
}

/// A region cut by the line through `p` along `dir`: (the part on its left, the part on its right).
pub fn split(set: &[Shape], p: P, dir: P) -> (Vec<Shape>, Vec<Shape>) {
    let l = geom::len(dir);
    if !(l > 0.0 && l.is_finite()) || grid(p).is_none() {
        return (Vec::new(), Vec::new());
    }
    let base = normalize(set);
    if base.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let (mut lo, mut hi) = (p, p);
    for s in &base {
        for q in &s.outer {
            for c in 0..2 {
                lo[c] = lo[c].min(q[c]);
                hi[c] = hi[c].max(q[c]);
            }
        }
    }
    let reach = 2.0 * geom::dist(lo, hi) + 10.0;
    let u = geom::mul(dir, 1.0 / l);
    let nl = [-u[1], u[0]];
    // Both sides share the same two points on the line, so their cut edges are the same.
    let a = feet(grid(geom::sub(p, geom::mul(u, reach))).unwrap_or([0, 0]));
    let b = feet(grid(geom::add(p, geom::mul(u, reach))).unwrap_or([0, 0]));
    let left = Shape::ring(vec![a, b, off(b, nl, reach), off(a, nl, reach)]);
    let right = Shape::ring(vec![b, a, off(a, nl, -reach), off(b, nl, -reach)]);
    (intersection(&base, &[left]), intersection(&base, &[right]))
}

/// Shapes (holes allowed) cut into convex polygons for lot cutting, anticlockwise, in a fixed order
/// (by their rings). A hole touching the outline (or another hole) joins it there; others are
/// joined along the shortest extension of one of their edges to another ring. Then, while a piece
/// has a reflex corner, it is cut along the extension of one of that corner's edges to the outline,
/// the shortest such cut first (a surveyor's cut: it leaves the corner straight on one side); each
/// cut is checked exactly against every edge before it is made. Where no cut can be found (never in
/// clipcheck) a piece is split along the whole line of an edge instead. Pieces are convex within
/// 1/256 ft (`is_convex`) and together the region, to a grid step along the cuts.
pub fn convex_pieces(set: &[Shape]) -> Vec<Vec<P>> {
    let mut out: Vec<Vec<I>> = Vec::new();
    for s in normalize(set) {
        pieces_of(&s, &mut out, 0);
    }
    for r in out.iter_mut() {
        rotate_least(r);
    }
    out.sort();
    out.into_iter().map(|r| r.into_iter().map(feet).collect()).collect()
}

/// Whether a ring (either orientation) is convex: it winds round once and no corner turns the
/// wrong way by more than 1/256 ft (its distance from the line through its neighbours).
pub fn is_convex(ring: &[P]) -> bool {
    let Some(mut r) = grid_ring(ring) else { return false };
    if area2(&r) < 0 {
        r.reverse();
    }
    // Corners straight or turning the wrong way within the tolerance are dropped first.
    let r = simplify(&r);
    let n = r.len();
    if n < 3 || area2(&r) <= 0 || (0..n).any(|k| orient(r[(k + n - 1) % n], r[k], r[(k + 1) % n]) < 0) {
        return false;
    }
    // Winding round once: the edges' x and y directions each change sign at most twice.
    let (mut flips, mut last) = ([0; 2], [0i64; 2]);
    for k in 0..2 * n {
        let e = isub(r[(k + 1) % n], r[k % n]);
        for c in 0..2 {
            let sg = e[c].signum();
            if sg != 0 {
                if last[c] != 0 && sg != last[c] && k >= n {
                    flips[c] += 1;
                }
                last[c] = sg;
            }
        }
    }
    flips[0] <= 2 && flips[1] <= 2
}

// ---------------------------------------------------------------------------------------------
// Grid arithmetic.

fn grid(p: P) -> Option<I> {
    // NaN fails both comparisons.
    if !(fabs(p[0]) <= LIMIT_FT && fabs(p[1]) <= LIMIT_FT) {
        return None;
    }
    Some([round(p[0] * SCALE) as i64, round(p[1] * SCALE) as i64])
}

fn feet(p: I) -> P {
    [p[0] as f64 / SCALE, p[1] as f64 / SCALE]
}

#[inline]
fn isub(a: I, b: I) -> I {
    [a[0] - b[0], a[1] - b[1]]
}

#[inline]
fn icross(a: I, b: I) -> i128 {
    a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128
}

#[inline]
fn idot(a: I, b: I) -> i128 {
    a[0] as i128 * b[0] as i128 + a[1] as i128 * b[1] as i128
}

/// Twice the signed area of triangle abc (positive when c is left of a->b).
#[inline]
fn orient(a: I, b: I, c: I) -> i128 {
    icross(isub(b, a), isub(c, a))
}

/// The next and previous place round a ring of `n` (no division in the hot loops).
#[inline]
fn nx(i: usize, n: usize) -> usize {
    if i + 1 == n { 0 } else { i + 1 }
}

#[inline]
fn pv(i: usize, n: usize) -> usize {
    if i == 0 { n - 1 } else { i - 1 }
}

fn area2(r: &[I]) -> i128 {
    let n = r.len();
    (0..n).map(|i| icross(r[i], r[nx(i, n)])).sum()
}

/// `x / den` rounded to the nearest integer (halves up), `den > 0`.
fn rdiv(x: i128, den: i128) -> i64 {
    (2 * x + den).div_euclid(2 * den) as i64
}

/// A ring on the grid with repeated points dropped; `None` when a point is invalid.
fn grid_ring(r: &[P]) -> Option<Vec<I>> {
    let mut out: Vec<I> = Vec::with_capacity(r.len());
    for p in r {
        let q = grid(*p)?;
        if out.last() != Some(&q) {
            out.push(q);
        }
    }
    while out.len() > 1 && out[0] == out[out.len() - 1] {
        out.pop();
    }
    Some(out)
}

fn rotate_least(r: &mut [I]) {
    if let Some(k) = (0..r.len()).min_by_key(|&i| r[i]) {
        r.rotate_left(k);
    }
}

fn off(p: P, n: P, d: f64) -> P {
    [p[0] + n[0] * d, p[1] + n[1] * d]
}

fn unit(v: P) -> P {
    let l = geom::len(v);
    if l > 0.0 { [v[0] / l, v[1] / l] } else { [1.0, 0.0] }
}

/// The piece filling the gap an offset opens at corner `v`, between `p1 = v + r n1` and
/// `p2 = v + r n2` (unit normals; the gap turns anticlockwise from n1 to n2 when `ccw`).
fn join_piece(v: P, n1: P, n2: P, p1: P, p2: P, r: f64, ccw: bool, join: Join) -> Vec<P> {
    let c = geom::dot(n1, n2).clamp(-1.0, 1.0);
    match join {
        Join::Bevel => vec![v, p1, p2],
        Join::Miter(limit) => {
            let k = 1.0 + c;
            if k > 1e-12 {
                let m = off(v, geom::add(n1, n2), r / k);
                if geom::dist(m, v) <= limit.max(1.0) * r {
                    return vec![v, p1, m, p2];
                }
            }
            vec![v, p1, p2]
        }
        Join::Round => {
            let phi = libm::acos(c);
            let step = if r > 2.0 * ROUND_TOL { 2.0 * libm::acos(1.0 - ROUND_TOL / r) } else { core::f64::consts::FRAC_PI_4 };
            let steps = (ceil(phi / step.max(1e-3)) as usize).clamp(1, 256);
            let a0 = libm::atan2(n1[1], n1[0]);
            let sg = if ccw { 1.0 } else { -1.0 };
            let mut out = vec![v, p1];
            for k in 1..steps {
                let a = a0 + sg * phi * k as f64 / steps as f64;
                out.push([v[0] + r * libm::cos(a), v[1] + r * libm::sin(a)]);
            }
            out.push(p2);
            out
        }
    }
}

/// The half-plane (normal, offset for `clip_plain`) on `keep`'s side of the mitre line at corner
/// `v`, from `v` to where the setback lines of its two edges meet (inner normals `ni`, `nj`,
/// distances `di`, `dj`), moved `over` ft past the line.
fn mitre_side(v: P, ni: P, di: f64, nj: P, dj: f64, keep: P, over: f64) -> Option<(P, f64)> {
    let det = geom::cross(ni, nj);
    if fabs(det) < 1e-12 {
        return None;
    }
    let q = [(nj[1] * di - ni[1] * dj) / det, (-nj[0] * di + ni[0] * dj) / det];
    let l = geom::len(q);
    if l < 1e-12 {
        return None;
    }
    let mut nrm = [q[1] / l, -q[0] / l];
    if geom::dot(geom::sub(keep, v), nrm) > 0.0 {
        nrm = [-nrm[0], -nrm[1]];
    }
    Some((nrm, geom::dot(v, nrm) + over))
}

/// A setback strip `z` ended at reflex corner `v` along its mitre line, keeping the side of `far`
/// (the edge's other end); `toward` is the edge's unit direction from `far` to `v`. The strip
/// overshoots the line by `SEAM` at `v`, tapering to nothing at the mitre point (rounded to the
/// grid, so both strips meeting there pivot on the same point): the strips then overlap along the
/// mitre line without notching the corner of the result. It also stops `SEAM` past the farther of
/// `v` and the mitre point along the edge (a mitre line nearly along the strip cuts it nowhere near).
fn end_at_mitre(z: &[P], v: P, ni: P, di: f64, nj: P, dj: f64, far: P, toward: P) -> Vec<P> {
    let det = geom::cross(ni, nj);
    if fabs(det) < 1e-12 {
        return z.to_vec();
    }
    let q = [(nj[1] * di - ni[1] * dj) / det, (-nj[0] * di + ni[0] * dj) / det];
    let Some(mg) = grid(geom::add(v, q)) else { return z.to_vec() };
    let mg = feet(mg);
    let e = geom::sub(mg, v);
    let l = geom::len(e);
    if l < 1e-9 {
        return z.to_vec();
    }
    // Off the line, away from `far`.
    let mut away = [e[1] / l, -e[0] / l];
    if geom::dot(geom::sub(far, v), away) > 0.0 {
        away = [-away[0], -away[1]];
    }
    let a = off(v, away, SEAM);
    let f = geom::sub(mg, a);
    let fl = geom::len(f);
    let mut nrm = [f[1] / fl, -f[0] / fl];
    if geom::dot(geom::sub(far, a), nrm) > 0.0 {
        nrm = [-nrm[0], -nrm[1]];
    }
    let z = geom::clip_plain(z, nrm, geom::dot(a, nrm));
    let reach = geom::dot(v, toward).max(geom::dot(mg, toward)) + SEAM;
    geom::clip_plain(&z, toward, reach)
}

// ---------------------------------------------------------------------------------------------
// The boolean engine.

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
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

fn run(a: &[Shape], b: &[Shape], op: Op) -> Vec<Shape> {
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
fn simplify(r: &[I]) -> Vec<I> {
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

fn pieces_of(s: &Shape, out: &mut Vec<Vec<I>>, depth: u32) {
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
