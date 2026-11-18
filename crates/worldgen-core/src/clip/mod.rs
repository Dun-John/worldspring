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

use super::geom::{self, P};
use crate::hash::FastMap;
use crate::{ceil, fabs, round, sqrt};

mod boolean;
mod convex;
mod grid;

use boolean::*;
use convex::*;
use grid::*;

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
    #[inline]
    pub fn ring(outer: Vec<P>) -> Self {
        Shape { outer, holes: Vec::new() }
    }

    /// Area: the outer's less the holes' (for canonical shapes).
    #[inline]
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
#[inline]
pub fn region(ring: &[P]) -> Vec<Shape> {
    normalize(&[Shape::ring(ring.to_vec())])
}

/// The canonical form of a set of shapes: their union.
#[inline]
pub fn normalize(set: &[Shape]) -> Vec<Shape> {
    run(set, &[], Op::Union)
}

/// The union of many shapes (the same as `normalize`).
#[inline]
pub fn union_all(set: &[Shape]) -> Vec<Shape> {
    run(set, &[], Op::Union)
}

#[inline]
pub fn union(a: &[Shape], b: &[Shape]) -> Vec<Shape> {
    run(a, b, Op::Union)
}

#[inline]
pub fn intersection(a: &[Shape], b: &[Shape]) -> Vec<Shape> {
    run(a, b, Op::Inter)
}

/// What of `a` is not in `b`.
#[inline]
pub fn difference(a: &[Shape], b: &[Shape]) -> Vec<Shape> {
    run(a, b, Op::Diff)
}

/// Total area of canonical shapes.
#[inline]
pub fn area(set: &[Shape]) -> f64 {
    set.iter().map(Shape::area).sum()
}

/// Whether a point is inside canonical shapes (in an outer and in none of its holes).
#[inline]
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
#[inline]
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
