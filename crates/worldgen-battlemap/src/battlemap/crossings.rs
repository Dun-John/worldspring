//! Bridges, fords, ferries and jetties: crossing shapes, decks and their levels.

use super::*;
pub use crate::model::consts::JETTY_FT;

pub const JETTY_WIDTH_FT: f64 = 9.0;

/// A street square steps up once shaping leaves it climbing at least `g_on` along its street (and
/// `HAND_MARGIN` more than it did); a flight's square goes back to paving once its slope is below
/// `g_off` (and that much gentler than it was, or on a plateau). Steeper than `HAND_CLIFF` it
/// stays a rock face.
pub(super) const HAND_MARGIN: f64 = 0.03;
pub(super) const HAND_CLIFF: f64 = 1.0;

/// The most flights of steps' axes the ground draws treads for in one chunk (the renderer's
/// `STAIR_AXES`).
pub const MAX_STAIR_AXES: usize = 64;

/// Stepping stones along a ford, this far apart (ft).
pub(super) const FORD_STEP_FT: f64 = 3.5;

pub(super) fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]).max(1e-9);
    [v[0] / l, v[1] / l]
}

pub(super) fn crossing_shape(x: &crate::lod::roads::RiverCrossing, tile: &TerrainOut) -> CrossingShape {
    let (u, deck) = (x.u, x.class.width_ft() + 4.0);
    let clear = |p: [f64; 2]| {
        tile.pieces.iter().all(|r| {
            r.pts.windows(2).all(|q| {
                let (a, b) = (q[0].p, q[1].p);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
                let (ex, ey) = (p[0] - a[0] - dx * t, p[1] - a[1] - dy * t);
                crate::core::sqrt(ex * ex + ey * ey) >= 0.5 * (q[0].w + (q[1].w - q[0].w) * t) + 4.0
            })
        })
    };
    let reach = |sign: f64| {
        let mut t = x.half_span();
        let corner = |t: f64, side: f64| [x.at[0] + u[0] * sign * t - u[1] * side * 0.5 * deck, x.at[1] + u[1] * sign * t + u[0] * side * 0.5 * deck];
        while t < 600.0 && !(clear(corner(t, 1.0)) && clear(corner(t, -1.0))) {
            t += 2.0;
        }
        t
    };
    let (fwd, back) = (reach(1.0), reach(-1.0));
    let (kind, size, extra) = match x.kind {
        CrossingKind::Bridge => (ShapeKind::RoadBridge, deck, 8.0),
        CrossingKind::Ford => (ShapeKind::Ford, x.class.width_ft(), 0.0),
        CrossingKind::Ferry => (ShapeKind::Ferry, x.class.width_ft().max(12.0), -(JETTY_FT - 10.0) - 4.0),
    };
    let (a, b) = ([x.at[0] - u[0] * (back + extra), x.at[1] - u[1] * (back + extra)], [x.at[0] + u[0] * (fwd + extra), x.at[1] + u[1] * (fwd + extra)]);
    let raft = (kind == ShapeKind::Ferry).then(|| raft_on(a, b, size));
    CrossingShape { kind, a, b, size, raft, dropped: false, stone: false }
}

/// A ferry's raft (centre, length, width ft) on its rope from a to b: a quadratic whose control
/// point is the middle moved 4% of the length to the right of the line (its point at t is
/// 2t(1 - t) of that out), 30% of the way over.
fn raft_on(a: [f64; 2], b: [f64; 2], size: f64) -> ([f64; 2], f64, f64) {
    let u = unit2([b[0] - a[0], b[1] - a[1]]);
    let (t, len) = (0.3, crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])));
    let off = 2.0 * t * (1.0 - t) * 0.04 * len;
    let at = [a[0] + (b[0] - a[0]) * t - u[1] * off, a[1] + (b[1] - a[1]) * t + u[0] * off];
    let w = (1.2 * size).max(14.0);
    (at, 1.6 * w, w)
}

/// The sea ferries' landing stages a chunk sees (`t0::ferries`), as ferries are drawn: from
/// where the water begins out from each landing (`SeaFerry::shores`: the same in every chunk), a
/// stage of two jetties end to end (the line between them a mooring), the ferry boat moored
/// alongside its outer end. `h0`: the chunk's halo corner (world ft).
pub(super) fn sea_ferry_shapes(world: &World, t0: &T0, h0: [f64; 2]) -> Vec<CrossingShape> {
    let size = crate::t0::roads::RoadClass::Track.width_ft().max(12.0);
    let span = HS as f64 * SQUARE_FT;
    let mut out = Vec::new();
    for f in &t0.ferries {
        let near = |p: [f64; 2], r: f64| p[0] + r >= h0[0] && p[0] - r <= h0[0] + span && p[1] + r >= h0[1] && p[1] - r <= h0[1] + span;
        if !near(f.a, crate::t0::ferries::SHORE_REACH_FT + 80.0) && !near(f.b, crate::t0::ferries::SHORE_REACH_FT + 80.0) {
            continue;
        }
        let shores = f.shores(world, t0);
        for (p, q) in [(shores[0], f.b), (shores[1], f.a)] {
            if !near(p, 2.0 * JETTY_FT + 60.0) {
                continue;
            }
            let u = unit2([q[0] - p[0], q[1] - p[1]]);
            let along = |t: f64, side: f64| [p[0] + u[0] * t - u[1] * side, p[1] + u[1] * t + u[0] * side];
            let (a, b) = (along(JETTY_FT - 4.0, 0.0), along(JETTY_FT - 3.5, 0.0));
            let (_, l, w) = raft_on(a, b, size);
            let raft = (along(2.0 * JETTY_FT - 3.5 - 0.5 * l, 0.5 * (JETTY_WIDTH_FT + w) + 1.0), l, w);
            out.push(CrossingShape { kind: ShapeKind::Ferry, a, b, size, raft: Some(raft), dropped: false, stone: false });
        }
    }
    out
}

/// The sea ferries' tracks over a shelf: from where each land leg ends (`INLAND_FT` back from
/// its landing on the water's contour) on to the stage where the water begins
/// (`SeaFerry::shores`), as track squares over the dry ground of the chunk's halo (corner `h0`,
/// world ft; `hh`, `hw` its ground and water) that have no road yet.
pub(super) fn sea_ferry_tracks(world: &World, t0: &T0, h0: [f64; 2], hh: &[f32], hw: &[f32], road_h: &mut [u8]) {
    use crate::t0::roads::RoadClass;
    let span = HS as f64 * SQUARE_FT;
    let half = 0.5 * RoadClass::Track.width_ft();
    for f in &t0.ferries {
        for (k, (p, q)) in [(f.a, f.b), (f.b, f.a)].into_iter().enumerate() {
            let r = crate::t0::ferries::SHORE_REACH_FT + 40.0;
            if p[0] + r < h0[0] || p[0] - r > h0[0] + span || p[1] + r < h0[1] || p[1] - r > h0[1] + span {
                continue;
            }
            let u = unit2([q[0] - p[0], q[1] - p[1]]);
            let w = f.shores(world, t0)[k];
            let a = [p[0] - u[0] * crate::t0::ferries::INLAND_FT, p[1] - u[1] * crate::t0::ferries::INLAND_FT];
            let len = (w[0] - p[0]) * u[0] + (w[1] - p[1]) * u[1] + crate::t0::ferries::INLAND_FT - 3.0;
            if len <= crate::t0::ferries::INLAND_FT {
                continue;
            }
            for j in 0..HS {
                for i in 0..HS {
                    let h = j * HS + i;
                    let c = [h0[0] + (i as f64 + 0.5) * SQUARE_FT, h0[1] + (j as f64 + 0.5) * SQUARE_FT];
                    let (dx, dy) = (c[0] - a[0], c[1] - a[1]);
                    let (along, across) = (dx * u[0] + dy * u[1], -dx * u[1] + dy * u[0]);
                    if (0.0..=len).contains(&along) && across.abs() <= half && road_h[h] == 0 && hw[h] + 0.05 < hh[h] {
                        road_h[h] = RoadClass::Track as u8 + 1;
                    }
                }
            }
        }
    }
}

/// A crossing put down by hand as decks (world ft polygons) for the map's site tiles: a
/// bridge's; a ferry's jetties, raft and rope (a ford has none).
/// A settlement's bridge's deck style (`ShapeKind::Deck` size; `seed` the "battlemap" stream): 1 a
/// village or town's timber bridge, 2 a city's stone bridge, 3 an old stone bridge (some of either,
/// by where the deck stands).
pub fn bridge_style(seed: u64, l: &crate::town::Layout, d: &[[f64; 2]]) -> f32 {
    let city = l.tier >= crate::t0::settle::Tier::City;
    let c = crate::town::geom::centroid(d);
    let old = unit(hash3(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 0x01d5)) < if city { 0.35 } else { 0.30 };
    if old { 3.0 } else if city { 2.0 } else { 1.0 }
}

pub fn hand_decks(c: &crate::world::Crossing) -> Vec<Vec<[f64; 2]>> {
    use crate::world::CrossingKind as K;
    let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
    let mid = |p: [f64; 2], q: [f64; 2]| [0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1])];
    match c.kind {
        K::Bridge => vec![crate::town::rect(mid(c.a, c.b), u, c.length(), c.width)],
        K::Ford => vec![],
        K::Ferry => {
            let (a, b) = ([c.a[0] + u[0] * JETTY_FT, c.a[1] + u[1] * JETTY_FT], [c.b[0] - u[0] * JETTY_FT, c.b[1] - u[1] * JETTY_FT]);
            let (at, l, w) = raft_on(a, b, c.width);
            vec![
                crate::town::rect(mid(c.a, a), u, JETTY_FT, JETTY_WIDTH_FT),
                crate::town::rect(mid(b, c.b), u, JETTY_FT, JETTY_WIDTH_FT),
                crate::town::rect(mid(a, b), u, c.length() - 2.0 * JETTY_FT, 1.5),
                crate::town::rect(at, u, l, w),
            ]
        }
    }
}

/// A crossing put down by hand as drawn (`CrossingShape`), and a bridge's deck level (ft): clear
/// of the highest water under it (rivers, lakes, the sea), else level with the lower end. A
/// ferry's line runs between its jetties' ends, `JETTY_FT` out from the points given. Pure
/// functions of the world, so every chunk finds the same.
pub(super) fn hand_crossing(c: &crate::world::Crossing, world: &World, t0: &T0) -> (CrossingShape, Deck) {
    use crate::world::CrossingKind as K;
    let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
    let (kind, a, b) = match c.kind {
        K::Bridge => (ShapeKind::RoadBridge, c.a, c.b),
        K::Ford => (ShapeKind::Ford, c.a, c.b),
        K::Ferry => (ShapeKind::Ferry, [c.a[0] + u[0] * JETTY_FT, c.a[1] + u[1] * JETTY_FT], [c.b[0] - u[0] * JETTY_FT, c.b[1] - u[1] * JETTY_FT]),
    };
    let raft = (kind == ShapeKind::Ferry).then(|| raft_on(a, b, c.width));
    let top = if kind == ShapeKind::RoadBridge { hand_deck_ends(c, world, t0) } else { Deck { ends: (f32::MIN, f32::MIN), span: (0.0, 1.0) } };
    (CrossingShape { kind, a, b, size: c.width, raft, dropped: false, stone: c.material == crate::world::CrossingMaterial::Stone }, top)
}

/// Bridge decks that overlap (a road crossing a meander twice, two roads at a confluence)
/// become one: along the same line, one bridge from end to end; else the greater road's
/// (then the longer) stays and the other is dropped. In a stable order (by crossing point),
/// so every chunk decides alike.
pub(super) fn merge_bridges(crossings: &[crate::lod::roads::RiverCrossing], shapes: &mut [CrossingShape]) {
    let mut order: Vec<usize> = (0..shapes.len()).filter(|&i| shapes[i].kind == ShapeKind::RoadBridge).collect();
    order.sort_by(|&p, &q| crossings[p].at[0].total_cmp(&crossings[q].at[0]).then(crossings[p].at[1].total_cmp(&crossings[q].at[1])));
    let seg_dist = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
        let (ex, ey) = (a[0] + dx * t - p[0], a[1] + dy * t - p[1]);
        crate::core::sqrt(ex * ex + ey * ey)
    };
    let len = |c: &CrossingShape| crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
    for (n, &i) in order.iter().enumerate() {
        for &j in &order[n + 1..] {
            if shapes[i].dropped || shapes[j].dropped {
                continue;
            }
            let (a, b) = (&shapes[i], &shapes[j]);
            let near = 0.5 * (a.size + b.size);
            let d = seg_dist(a.a, b.a, b.b).min(seg_dist(a.b, b.a, b.b)).min(seg_dist(b.a, a.a, a.b)).min(seg_dist(b.b, a.a, a.b));
            let crosses = {
                let (r, q) = ([a.b[0] - a.a[0], a.b[1] - a.a[1]], [b.b[0] - b.a[0], b.b[1] - b.a[1]]);
                let den = r[0] * q[1] - r[1] * q[0];
                den.abs() > 1e-9 && {
                    let t = ((b.a[0] - a.a[0]) * q[1] - (b.a[1] - a.a[1]) * q[0]) / den;
                    let v = ((b.a[0] - a.a[0]) * r[1] - (b.a[1] - a.a[1]) * r[0]) / den;
                    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&v)
                }
            };
            if d >= near && !crosses {
                continue;
            }
            let u = unit2([a.b[0] - a.a[0], a.b[1] - a.a[1]]);
            let ub = unit2([b.b[0] - b.a[0], b.b[1] - b.a[1]]);
            if (u[0] * ub[0] + u[1] * ub[1]).abs() > 0.95 && d < 0.5 * near {
                // One bridge over both: from the earliest end to the latest along the first.
                let ts = [a.a, a.b, b.a, b.b].map(|p| (p[0] - a.a[0]) * u[0] + (p[1] - a.a[1]) * u[1]);
                let (lo, hi) = (ts.iter().cloned().fold(f64::MAX, f64::min), ts.iter().cloned().fold(f64::MIN, f64::max));
                let o = a.a;
                let size = a.size.max(b.size);
                let s = &mut shapes[i];
                (s.a, s.b, s.size) = ([o[0] + u[0] * lo, o[1] + u[1] * lo], [o[0] + u[0] * hi, o[1] + u[1] * hi], size);
                shapes[j].dropped = true;
            } else {
                let keep_i = (crossings[i].class, -len(&shapes[i])) <= (crossings[j].class, -len(&shapes[j]));
                shapes[if keep_i { j } else { i }].dropped = true;
            }
        }
    }
}

/// The road surface (ft) at a point on a road: the nearest road piece within its width.
pub(super) fn road_level(roads: &[crate::lod::roads::RoadPiece], p: [f64; 2]) -> Option<f32> {
    let mut best: Option<(f64, f64)> = None;
    for piece in roads {
        let reach = 0.5 * piece.class.width_ft() + SQUARE_FT;
        for seg in piece.pts.windows(2) {
            let (a, b) = (seg[0].p, seg[1].p);
            if p[0] < a[0].min(b[0]) - reach || p[0] > a[0].max(b[0]) + reach || p[1] < a[1].min(b[1]) - reach || p[1] > a[1].max(b[1]) + reach {
                continue;
            }
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
            let (ex, ey) = (a[0] + dx * t - p[0], a[1] + dy * t - p[1]);
            let d = crate::core::sqrt(ex * ex + ey * ey);
            if d < reach && best.is_none_or(|b| d < b.0) {
                best = Some((d, seg[0].z + (seg[1].z - seg[0].z) * t));
            }
        }
    }
    best.map(|b| b.1 as f32)
}
