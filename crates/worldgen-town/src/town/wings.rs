//! (Roadmap 7, 163) L, U and courtyard footprints: on a corner lot an inn, a townhouse, a guildhall, a
//! warehouse or a manor (and the like, `kind_of`) may wrap the corner in an L, a range along each street;
//! on a large deep lot it may stand round a yard, a U open to the back (or to the street, as its kind has
//! it) or a closed court with a gateway through its front range. About one in four of those that can
//! (`SHARE`, rolled from the town's roof seed and the building's id). Its ranges are whole squares wide on
//! its interior's grid (`interior::Shell`: centred on the footprint); the lot it leaves is its own ground
//! (`Layout::grounds`). The footprint is rectilinear: its ranges (`arms`) are what the battlemap roofs
//! (`app/src/gen/roofMesh.ts` `rangesRoof`), each hipped or gabled, valleys where they meet.

use super::geom::{P, add, area, clean, contains, cross, dot, len, mul, obb, seg_dist, sub};
use super::{Building, Layout, Structure, Ward, catalog, roof};
use crate::core::rng::{hash3, unit};

/// The share of the buildings that could take wings that do.
pub const SHARE: f64 = 0.25;
/// A side whose middle lies this near a street's edge (ft) faces it.
const STREET_FT: f64 = 14.0;
const SQ: f64 = 5.0;
/// A range's least and most width (squares): 25 ft at least, a room off a passage deep, so that its
/// end is a gable or a hip as big as the main range's (a share of the lot's least side between).
const ARM_SQ: i32 = 5;
const ARM_MAX_SQ: i32 = 7;
/// The yard's least side behind an L (squares), and a U's or a court's.
const L_YARD_SQ: i32 = 3;
const YARD_SQ: i32 = 4;
/// No two shaped buildings' lots nearer than this (ft): not side by side on a street, nor one lot apart.
const NEAR_FT: f64 = 30.0;
/// At most this many in one block, and of them one U or court.
const PER_BLOCK: usize = 2;
/// A court's gateway (squares wide).
const GATE_SQ: i32 = 2;

/// A footprint's form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    L,
    /// A U, its yard open to the back, or to the street (`true`).
    U(bool),
    Court,
}

/// What forms a building's kind may take: (its U open to the street, it may stand round a court).
fn kind_of(b: &Building) -> Option<(bool, bool)> {
    let Some(f) = b.func else {
        return match catalog::RESIDENTIAL[b.residential as usize] {
            "townhouse" => Some((false, false)),
            "noble estate" => Some((true, true)),
            _ => None,
        };
    };
    Some(match catalog::CATALOG[f as usize].key {
        "inn" | "warehouse" | "caravanserai" | "customs_house" | "brewery" | "stables" => (false, true),
        "merchant_guild" | "craft_guild" => (true, true),
        "adventurers_guild" | "mages_guild" | "bank" | "courthouse" | "library" | "bards_college" | "auction_house" | "orphanage" => (true, false),
        "bathhouse" | "distillery" | "winery" => (false, false),
        _ => return None,
    })
}

/// Shapes every building that takes wings (after the temples: `temple::shape_all`), in id order, none
/// near another (`NEAR_FT`, `PER_BLOCK`), so that they stand out. In a town laid out anew (`edit`: the
/// world and the layout as generated), one on the lot it was shaped on as generated keeps its shape; no
/// building with edits of its own (`b:<layout>:<id>`) is shaped anew.
pub(super) fn shape_all(l: &mut Layout, edit: Option<(&crate::World, &Layout)>) {
    let edited: Vec<u32> = edit.map(|(w, _)| super::building_edits(w, l.index).map(|(id, _)| id).collect()).unwrap_or_default();
    let boxes: Vec<[f64; 4]> = l.blocks.iter().map(|q| super::geom::bbox(q)).collect();
    let block_of = |p: &[P]| {
        let c = super::geom::centroid(p);
        (0..l.blocks.len()).find(|&k| boxes[k][0] <= c[0] && c[0] <= boxes[k][2] && boxes[k][1] <= c[1] && c[1] <= boxes[k][3] && contains(&l.blocks[k], c))
    };
    // (Each taken: its index, footprint, lot, block, a U or a court.)
    let mut done: Vec<(usize, Vec<P>, Vec<P>, Option<usize>, bool)> = Vec::new();
    if let Some((_, g)) = edit {
        for (k, b) in l.buildings.iter().enumerate() {
            let Some(was) = g.building(b.id as usize).filter(|w| g.wings.binary_search(&w.id).is_ok()) else { continue };
            let same = g.grounds.iter().find(|q| q.0 == b.id).is_some_and(|q| q.1.len() == b.poly.len() && q.1.iter().zip(&b.poly).all(|(a, c)| len(sub(*a, *c)) < 0.01));
            if same && b.parts.is_empty() && b.structure == Structure::Roofed {
                done.push((k, was.poly.clone(), b.poly.clone(), block_of(&b.poly), was.poly.len() > 6));
            }
        }
    }
    for (k, b) in l.buildings.iter().enumerate() {
        if edited.contains(&b.id) || done.iter().any(|d| d.0 == k) {
            continue;
        }
        let Some((p, form)) = shaped(l, b) else { continue };
        let big = form != Form::L;
        let block = block_of(&b.poly);
        let mine = done.iter().filter(|d| block.is_some() && d.3 == block);
        if mine.clone().count() >= PER_BLOCK || (big && mine.clone().any(|d| d.4)) || done.iter().any(|d| poly_gap(&d.2, &b.poly) < NEAR_FT) {
            continue;
        }
        done.push((k, p, b.poly.clone(), block, big));
    }
    for (k, p, _, _, _) in done {
        let b = &mut l.buildings[k];
        let lot = std::mem::replace(&mut b.poly, p);
        let id = b.id;
        l.grounds.push((id, lot));
        l.wings.push(id);
    }
    l.wings.sort_unstable();
}

/// The least distance between two footprints (ft; 0 where they touch or overlap).
fn poly_gap(a: &[P], b: &[P]) -> f64 {
    if a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p)) {
        return 0.0;
    }
    let d = |a: &[P], b: &[P]| a.iter().flat_map(|p| (0..b.len()).map(move |k| seg_dist(*p, b[k], b[(k + 1) % b.len()]))).fold(f64::MAX, f64::min);
    d(a, b).min(d(b, a))
}

/// The footprint building `b` takes and its form, if it takes wings (`shape_all`).
pub fn shaped(l: &Layout, b: &Building) -> Option<(Vec<P>, Form)> {
    if b.structure != Structure::Roofed || !b.parts.is_empty() || b.keep.is_some() || b.deck.is_some() || b.ward == Ward::Castle || b.roof.is_some() || b.shape.is_some() {
        return None;
    }
    let (front_open, court) = kind_of(b)?;
    if unit(hash3(l.roof_seed, b.id as i64, 0x5749_4e47, 0)) >= SHARE {
        return None;
    }
    let poly = clean(&b.poly);
    if poly.len() != 4 {
        return None;
    }
    let u = obb(&poly).axis;
    let v = [-u[1], u[0]];
    // (Square corners, within 2 degrees.)
    for k in 0..4 {
        let e = sub(poly[(k + 1) % 4], poly[k]);
        let l = len(e);
        if l < 1e-6 || (dot(e, u).abs() / l).min(dot(e, v).abs() / l) > 0.035 {
            return None;
        }
    }
    // The rectangle inside it square to its frame.
    let mut s: Vec<f64> = poly.iter().map(|p| dot(*p, u)).collect();
    let mut t: Vec<f64> = poly.iter().map(|p| dot(*p, v)).collect();
    s.sort_by(f64::total_cmp);
    t.sort_by(f64::total_cmp);
    let (s0, s1, t0, t1) = (s[1], s[2], t[1], t[2]);
    let at = |a: f64, c: f64| add(mul(u, a), mul(v, c));
    let (mid_s, mid_t) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
    let inward = |a: f64, c: f64| at(a + 0.02 * (mid_s - a).signum(), c + 0.02 * (mid_t - c).signum());
    if [(s0, t0), (s1, t0), (s1, t1), (s0, t1)].iter().any(|&(a, c)| !contains(&poly, inward(a, c))) || (s1 - s0) * (t1 - t0) < 0.95 * area(&poly).abs() {
        return None;
    }
    // Its sides: each a frame (origin, along, into the lot, length, depth) with the front at y = 0.
    let frames = [
        ([s0, t0], [1.0, 0.0], [0.0, 1.0], s1 - s0, t1 - t0),
        ([s1, t0], [0.0, 1.0], [-1.0, 0.0], t1 - t0, s1 - s0),
        ([s1, t1], [-1.0, 0.0], [0.0, -1.0], s1 - s0, t1 - t0),
        ([s0, t1], [0.0, -1.0], [1.0, 0.0], t1 - t0, s1 - s0),
    ];
    let world = |f: usize, x: f64, y: f64| {
        let (o, ex, ey, _, _) = frames[f];
        at(o[0] + ex[0] * x + ey[0] * y, o[1] + ex[1] * x + ey[1] * y)
    };
    let street = |p: P| l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| seg_dist(p, q[0], q[1]) - 0.5 * w)).fold(f64::MAX, f64::min);
    let d: Vec<f64> = (0..4).map(|f| street(world(f, 0.5 * frames[f].3, 0.0))).collect();
    let front = (0..4).min_by(|&a, &c| d[a].total_cmp(&d[c]))?;
    if d[front] > STREET_FT {
        return None;
    }
    // (Its roof one the battlemap draws over ranges, `roofMesh.ts`: pitched, or flat behind a parapet;
    // `roof::look` takes a shaped building's lot's area, so it stays as it is here.)
    if super::palace::merges(l, b) || !matches!(roof::look(l, b).shape, roof::Shape::Hip | roof::Shape::Gable | roof::Shape::Flat) {
        return None;
    }
    // (The sides at x = 0 and x = X: the next side round and the one before.)
    let (left, right) = (d[(front + 3) % 4] <= STREET_FT, d[(front + 1) % 4] <= STREET_FT);
    let (xl, yl) = (frames[front].3, frames[front].4);
    let (nx, ny) = (crate::core::round(xl / SQ) as i32, crate::core::round(yl / SQ) as i32);
    // (The interior's grid lines: centred, whole squares in from either side.)
    let (ox, oy) = (0.5 * (xl - nx as f64 * SQ), 0.5 * (yl - ny as f64 * SQ));
    let gx = |k: i32| ox + k as f64 * SQ;
    let gy = |k: i32| oy + k as f64 * SQ;
    let w = (crate::core::round(0.3 * xl.min(yl) / SQ) as i32).clamp(ARM_SQ, ARM_MAX_SQ);
    let fits_l = (left || right) && nx - w >= L_YARD_SQ && ny - w >= L_YARD_SQ && nx * ny >= 80;
    let fits_u = nx - 2 * w >= YARD_SQ && ny - w >= YARD_SQ && nx * ny >= 140;
    let fits_c = court && nx - 2 * w >= YARD_SQ.max(GATE_SQ + 2) && ny - 2 * w >= YARD_SQ;
    if !(fits_l || fits_u || fits_c) {
        return None;
    }
    let pick = unit(hash3(l.roof_seed, b.id as i64, 0x5749_4e47, 1));
    let form = match (fits_l, fits_u, fits_c) {
        (_, _, true) if pick < 0.4 || !fits_u && !fits_l => Form::Court,
        (true, true, _) => if pick < 0.7 { Form::L } else { Form::U(front_open) },
        (true, false, _) => Form::L,
        _ => Form::U(front_open),
    };
    let (xr, yb) = (xl - gx(w), yl - gy(w));
    let (a, c) = (gx(w), gy(w));
    let pts: Vec<(f64, f64)> = match form {
        // (Its side range along the side street, the left one if both are.)
        Form::L if left => vec![(0.0, 0.0), (xl, 0.0), (xl, c), (a, c), (a, yl), (0.0, yl)],
        Form::L => vec![(0.0, 0.0), (xl, 0.0), (xl, yl), (xr, yl), (xr, c), (0.0, c)],
        Form::U(false) => vec![(0.0, 0.0), (xl, 0.0), (xl, yl), (xr, yl), (xr, c), (a, c), (a, yl), (0.0, yl)],
        Form::U(true) => vec![(0.0, 0.0), (a, 0.0), (a, yb), (xr, yb), (xr, 0.0), (xl, 0.0), (xl, yl), (0.0, yl)],
        Form::Court => {
            // (A wall either side of the gate, a square of the yard behind it at least.)
            let g = w + (nx - 2 * w - GATE_SQ) / 2;
            let (g0, g1) = (gx(g), gx(g + GATE_SQ));
            vec![(0.0, 0.0), (g0, 0.0), (g0, c), (a, c), (a, yb), (xr, yb), (xr, c), (g1, c), (g1, 0.0), (xl, 0.0), (xl, yl), (0.0, yl)]
        }
    };
    let mut out: Vec<P> = pts.iter().map(|&(x, y)| world(front, x, y)).collect();
    if (area(&out) < 0.0) != (area(&b.poly) < 0.0) {
        out.reverse();
    }
    Some((out, form))
}

/// A building's ranges (`arms`) if it is an L, a U or a court `shape_all` made (`Layout::wings`): not one
/// drawn by hand, nor a keep's or a temple's pieces, nor a castle's.
pub fn ranges(l: &Layout, b: &Building) -> Option<Vec<[P; 4]>> {
    if !is_shaped(l, b) || !b.parts.is_empty() || b.keep.is_some() || b.ward == Ward::Castle {
        return None;
    }
    let mut r = arms(&b.poly)?;
    // (A court's front range one roof over its gateway, a carriage arch: not two stubs either side of it.)
    if b.poly.len() == 12 {
        let u = obb(&b.poly).axis;
        let v = [-u[1], u[0]];
        let span = |q: &[P; 4], d: P| q.iter().map(|p| dot(*p, d)).fold((f64::MAX, f64::MIN), |(a, c), x| (a.min(x), c.max(x)));
        let ccw = area(&b.poly) > 0.0;
        'pairs: for i in 0..r.len() {
            for j in i + 1..r.len() {
                for (d, e) in [(u, v), (v, u)] {
                    let ((a0, a1), (c0, c1)) = (span(&r[i], d), span(&r[j], d));
                    let ((t0, t1), (e0, e1)) = (span(&r[i], e), span(&r[j], e));
                    let gap = a0.max(c0) - a1.min(c1);
                    if (t0 - e0).abs() < 0.01 && (t1 - e1).abs() < 0.01 && gap > 0.0 && gap < GATE_SQ as f64 * SQ + 1.5 {
                        let (s0, s1) = (a0.min(c0), a1.max(c1));
                        let at = |x: f64, y: f64| add(mul(d, x), mul(e, y));
                        let q = [at(s0, t0), at(s1, t0), at(s1, t1), at(s0, t1)];
                        r[i] = if (area(&q) > 0.0) == ccw { q } else { [q[3], q[2], q[1], q[0]] };
                        r.remove(j);
                        break 'pairs;
                    }
                }
            }
        }
    }
    Some(r)
}

/// Whether `shape_all` shaped building `b` (its lot then in `Layout::grounds`).
#[inline]
pub fn is_shaped(l: &Layout, b: &Building) -> bool {
    b.poly.len() > 4 && l.wings.binary_search(&b.id).is_ok()
}

/// A rectilinear footprint's ranges (world ft, each four corners wound as the footprint is): the
/// largest rectangles it holds on the grid of its corners' lines, each overlapping the next where they
/// meet (an L's two, a U's three, a court's five); None for a footprint of one (or not square-cornered).
pub fn arms(poly: &[P]) -> Option<Vec<[P; 4]>> {
    let m = poly.len();
    if !(6..=16).contains(&m) {
        return None;
    }
    let u = obb(poly).axis;
    let v = [-u[1], u[0]];
    for k in 0..m {
        let e = sub(poly[(k + 1) % m], poly[k]);
        let l = len(e);
        if l < 0.5 || (cross(e, u).abs() / l).min(cross(e, v).abs() / l) > 1e-6 {
            return None;
        }
    }
    let line = |vals: Vec<f64>| {
        let mut out: Vec<f64> = Vec::new();
        let mut vals = vals;
        vals.sort_by(f64::total_cmp);
        for x in vals {
            if out.last().is_none_or(|&l| x - l > 0.01) {
                out.push(x);
            }
        }
        out
    };
    let xs = line(poly.iter().map(|p| dot(*p, u)).collect());
    let ys = line(poly.iter().map(|p| dot(*p, v)).collect());
    let (cx, cy) = (xs.len() - 1, ys.len() - 1);
    let at = |a: f64, c: f64| add(mul(u, a), mul(v, c));
    let cell: Vec<bool> = (0..cx * cy).map(|k| contains(poly, at(0.5 * (xs[k % cx] + xs[k % cx + 1]), 0.5 * (ys[k / cx] + ys[k / cx + 1])))).collect();
    let full = |i0: usize, i1: usize, j0: usize, j1: usize| (j0..=j1).all(|j| (i0..=i1).all(|i| cell[j * cx + i]));
    let mut rects: Vec<[usize; 4]> = Vec::new();
    for j0 in 0..cy {
        for i0 in 0..cx {
            for j1 in j0..cy {
                for i1 in i0..cx {
                    if !full(i0, i1, j0, j1) {
                        continue;
                    }
                    // (Maximal: no row or column more on any side.)
                    let grows = (i0 > 0 && full(i0 - 1, i1, j0, j1)) || (i1 + 1 < cx && full(i0, i1 + 1, j0, j1)) || (j0 > 0 && full(i0, i1, j0 - 1, j1)) || (j1 + 1 < cy && full(i0, i1, j0, j1 + 1));
                    if !grows {
                        rects.push([i0, i1, j0, j1]);
                    }
                }
            }
        }
    }
    if rects.len() < 2 {
        return None;
    }
    let ccw = area(poly) > 0.0;
    Some(
        rects
            .iter()
            .map(|&[i0, i1, j0, j1]| {
                let (a0, a1, b0, b1) = (xs[i0], xs[i1 + 1], ys[j0], ys[j1 + 1]);
                let q = [at(a0, b0), at(a1, b0), at(a1, b1), at(a0, b1)];
                if (area(&q) > 0.0) == ccw { q } else { [q[3], q[2], q[1], q[0]] }
            })
            .collect(),
    )
}
