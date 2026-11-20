//! A wharf's ground half (`wharf`): its water, level, made ground and decks, below the
//! settlements so that `ground` shapes quays and breakwaters without them.

use crate::t0::T0;
use crate::core::geom;
use crate::world::{Wharf, WharfKind};

type P = [f64; 2];

/// The ground ft above the water a quay's or breakwater's top stands at least, and at most.
pub const MADE_ABOVE_FT: (f32, f32) = (3.0, 10.0);

/// The highest water (ft) under a wharf's line: standing water every 10 ft along it, and any river
/// it crosses or comes within `margin` ft of. `hydro::DRY` where none.
pub fn water(w: &Wharf, t0: &T0, margin: f64) -> f32 {
    let cache = crate::lod::rivers::CrossingCache::default();
    let mut top = crate::t0::hydro::DRY;
    // (A shore quay's line every ~40 ft: its points are ~5 ft apart.)
    let pts: Vec<P> = if w.shore { w.pts.iter().copied().step_by(8).chain(w.pts.last().copied()).collect() } else { w.pts.clone() };
    for s in pts.windows(2) {
        let n = (geom::dist(s[0], s[1]) / 10.0).ceil().max(1.0) as usize;
        for k in 0..=n {
            let p = geom::lerp(s[0], s[1], k as f64 / n as f64);
            top = top.max(t0.sample_water(p[0], p[1]));
        }
        if let Some((z, _)) = t0.rivers.crossing_level(s[0], s[1], margin, t0.cell_ft, &cache) {
            top = top.max(z as f32);
        }
    }
    top
}

/// The level (ft) of a wharf's top: a quay's or breakwater's the bank's ground at `pts[0]`, kept
/// `MADE_ABOVE_FT` over the water; a pier's deck clear of the water as a hand bridge's
/// (`battlemap::DECK_CLEARANCE_FT`). On dry ground, the ground at `pts[0]`.
pub fn level(w: &Wharf, t0: &T0) -> f32 {
    let bank = t0.ground_at(w.pts[0][0], w.pts[0][1], 5.0) as f32;
    let water = water(w, t0, margin(w));
    if water <= crate::t0::hydro::DRY {
        return bank;
    }
    match w.kind {
        WharfKind::Pier => water + crate::model::consts::DECK_CLEARANCE_FT,
        _ => bank.clamp(water + MADE_ABOVE_FT.0, water + MADE_ABOVE_FT.1),
    }
}

pub use geom::unit;

/// How far from its line (ft) a wharf's water is looked for.
fn margin(w: &Wharf) -> f64 {
    match w.kind {
        WharfKind::Quay => 10.0,
        _ => 0.5 * w.width,
    }
}

/// How wet T0 is at `p`: in a river's channel, very; else the standing water's depth there
/// (negative on dry land).
pub fn wetness(t0: &T0, p: P) -> f64 {
    let q = crate::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 0.0, t0.cell_ft);
    if (q.0 - p[0]).abs() + (q.1 - p[1]).abs() > 0.01 {
        return 1000.0;
    }
    let water = t0.sample_water(p[0], p[1]);
    if water <= crate::t0::hydro::DRY { -1000.0 } else { water as f64 - t0.ground_at(p[0], p[1], 5.0) }
}

/// The unit normal toward the dry side of the side a–b (a quay's), from T0 at its middle: the
/// left one (`[-u.y, u.x]`) unless the right is drier.
pub fn dry_side(t0: &T0, a: P, b: P, width: f64) -> P {
    let u = unit(geom::sub(b, a));
    let n = [-u[1], u[0]];
    let (m, d) = (geom::lerp(a, b, 0.5), (0.5 * width).max(10.0));
    let (left, right) = (wetness(t0, geom::add(m, geom::mul(n, d))), wetness(t0, geom::add(m, geom::mul(n, -d))));
    if right < left { [-n[0], -n[1]] } else { n }
}

/// The made ground of a quay or breakwater (world ft polygons; a pier has none): a quay's strip
/// from its line to the dry side, a breakwater's along its line, each side one quadrilateral and
/// each joint closed (a quay's by the wedge between its sides' strips, a breakwater's by a round).
pub fn strips(w: &Wharf, t0: &T0) -> Vec<Vec<P>> {
    let mut out = Vec::new();
    match w.kind {
        WharfKind::Pier => {}
        WharfKind::Quay if w.shore => out.extend(shore_pieces(w).into_iter().map(|p| p.0)),
        WharfKind::Quay => {
            let sides: Vec<P> = w.pts.windows(2).map(|s| dry_side(t0, s[0], s[1], w.width)).collect();
            for (s, n) in w.pts.windows(2).zip(&sides) {
                let o = geom::mul(*n, w.width);
                out.push(vec![s[0], s[1], geom::add(s[1], o), geom::add(s[0], o)]);
            }
            for k in 1..sides.len() {
                let p = w.pts[k];
                out.push(vec![p, geom::add(p, geom::mul(sides[k - 1], w.width)), geom::add(p, geom::mul(sides[k], w.width))]);
            }
        }
        WharfKind::Breakwater => {
            out.extend(decks(w));
        }
    }
    out.retain(|p| geom::area(p).abs() > 1.0);
    out
}

/// A pier's (or breakwater's) deck along its line, `width` ft across: one rectangle per side and
/// a round at each joint (world ft polygons).
pub fn decks(w: &Wharf) -> Vec<Vec<P>> {
    let mut out: Vec<Vec<P>> = w.pts.windows(2).map(|s| geom::rect(geom::lerp(s[0], s[1], 0.5), geom::sub(s[1], s[0]), geom::dist(s[0], s[1]), w.width)).collect();
    for &p in &w.pts[1..w.pts.len().saturating_sub(1)] {
        out.push(geom::circle(p, 0.5 * w.width, 12));
    }
    out
}

/// The bounds (world ft: x0, y0, x1, y1) a wharf's ground and decks keep within (a shore quay's
/// piers too).
pub fn bounds(w: &Wharf) -> [f64; 4] {
    let r = if w.shore && w.piers > 0.0 { w.width.max(Wharf::SHORE_PIER_FT) } else { w.width } + 1.0;
    w.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - r), b[1].min(p[1] - r), b[2].max(p[0] + r), b[3].max(p[1] + r)])
}

// R7 173: quays brushed along the shore (`Wharf::shore`).

/// A shore quay's normals at its points, toward the dry side: the left of its line's tangent
/// there (across the points either side).
pub fn shore_normals(pts: &[P]) -> Vec<P> {
    let n = pts.len();
    (0..n)
        .map(|k| {
            let u = unit(geom::sub(pts[(k + 1).min(n - 1)], pts[k.saturating_sub(1)]));
            [-u[1], u[0]]
        })
        .collect()
}

/// A shore quay's back line: `width` ft in from each point of its face along its normal, closing
/// onto the face at each end in a quarter ellipse over about its width (a round end on the beach,
/// never a straight cut).
pub fn shore_back(w: &Wharf) -> Vec<P> {
    let (pts, normal) = (&w.pts, shore_normals(&w.pts));
    let n = pts.len();
    let mut along = vec![0.0; n];
    for k in 1..n {
        along[k] = along[k - 1] + geom::dist(pts[k - 1], pts[k]);
    }
    let run = along[n - 1];
    let reach = w.width.max(10.0).min(0.3 * run).max(1e-6);
    (0..n)
        .map(|k| {
            let d = along[k].min(run - along[k]);
            let u = (1.0 - d / reach).max(0.0);
            let e = crate::core::sqrt(1.0 - u * u).max(0.04);
            geom::add(pts[k], geom::mul(normal[k], w.width * e))
        })
        .collect()
}

/// A shore quay's length (ft) in each of its pieces (`shore_pieces`).
pub const SHORE_PIECE_FT: f64 = 40.0;

/// A shore quay's made ground in pieces of about `SHORE_PIECE_FT` along it: each its face's
/// points `k0..=k1` then its back's `k1..=k0` (a polygon), with how many of them are the face's.
pub fn shore_pieces(w: &Wharf) -> Vec<(Vec<P>, usize)> {
    let n = w.pts.len();
    if n < 2 {
        return Vec::new();
    }
    let back = shore_back(w);
    let np = (crate::core::round(w.length() / SHORE_PIECE_FT) as usize).clamp(1, n - 1);
    (0..np)
        .map(|j| {
            let (a, b) = (j * (n - 1) / np, (j + 1) * (n - 1) / np);
            let mut poly: Vec<P> = w.pts[a..=b].to_vec();
            poly.extend(back[a..=b].iter().rev());
            (poly, b - a + 1)
        })
        .collect()
}

/// A shore quay's piers (`Wharf::piers` ft apart, none within half that of its ends): each
/// `SHORE_PIER_FT` straight out from its face into the water, from 2 ft back on the quay, of its
/// material; only where T0 has water under its middle and tip.
pub fn shore_piers(w: &Wharf, t0: &T0) -> Vec<Wharf> {
    if !w.shore || w.kind != WharfKind::Quay || w.piers <= 0.0 || w.pts.len() < 2 {
        return Vec::new();
    }
    let normal = shore_normals(&w.pts);
    let len = w.length();
    let mut out = Vec::new();
    let (mut at, mut run, mut k) = (0.5 * w.piers, 0.0, 0);
    while at <= len - 0.5 * w.piers + 1e-6 {
        while k + 2 < w.pts.len() && run + geom::dist(w.pts[k], w.pts[k + 1]) < at {
            run += geom::dist(w.pts[k], w.pts[k + 1]);
            k += 1;
        }
        let d = geom::dist(w.pts[k], w.pts[k + 1]).max(1e-9);
        let t = ((at - run) / d).clamp(0.0, 1.0);
        let (p, nn) = (geom::lerp(w.pts[k], w.pts[k + 1], t), unit(geom::lerp(normal[k], normal[k + 1], t)));
        let (root, tip) = (geom::add(p, geom::mul(nn, 2.0)), geom::sub(p, geom::mul(nn, Wharf::SHORE_PIER_FT)));
        if wetness(t0, tip) > 0.0 && wetness(t0, geom::lerp(p, tip, 0.5)) > 0.0 {
            out.push(Wharf::new(WharfKind::Pier, vec![root, tip], Wharf::SHORE_PIER_WIDTH_FT, w.material));
        }
        at += w.piers;
    }
    out
}
