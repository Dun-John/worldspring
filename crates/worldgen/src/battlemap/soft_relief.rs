//! Small closed relief drawn soft (v63 underground): the knolls, hollows and ditches of
//! `relief_in_cell` are marked with edge bit 128 where they stand on gentle ground, and
//! `ground.frag` draws them with soft shading and a faint contour instead of a rock face and a
//! bold rim. Real slopes, banks and cliffs (steep ground under the feature) and the face of a
//! dwarven hall's crag keep their ink; the crag's back and horns are soft. Rendering only: tiers
//! and cliff bits are unchanged.

use super::{HS, Relief, SQ};

// v63 underground: `Chunk::edges` bit.
/// Small closed relief, drawn soft.
pub const SOFT: u8 = 128;

/// Squares (beyond a feature's rim) still marked, so the face never meets an unmarked square.
const MARGIN_SQ: f64 = 2.0;
/// Steepest grade (ft per ft) of the ground under a feature that still counts as gentle.
const GENTLE: f64 = 0.35;

/// Distance (squares) from a feature's rim, negative inside (as `relief_value`'s outline).
fn past_rim(f: &Relief, x: f64, y: f64) -> f64 {
    let (dx, dy) = (x - f.cx, y - f.cy);
    let along = (dx * f.dir.0 + dy * f.dir.1) / f.elong;
    let across = -dx * f.dir.1 + dy * f.dir.0;
    let th = libm::atan2(across, along);
    let edge = 1.0 + f.wobble.iter().enumerate().map(|(k, (a, ph))| a * libm::cos((k + 1) as f64 * th + ph)).sum::<f64>();
    crate::core::sqrt(along * along + across * across) - f.r * edge
}

/// The `HS`² mask (halo grid whose (0, 0) is global square (gsx, gsy)) of squares in or by a
/// feature whose ground before the relief (`base`) is gentle.
pub(super) fn mask(features: &[Relief], base: &[f32], gsx: i64, gsy: i64) -> Vec<bool> {
    let mut out = vec![false; if features.is_empty() { 0 } else { HS * HS }];
    for f in features {
        let reach = super::RELIEF_REACH + MARGIN_SQ + 1.0;
        let (x0, x1) = (((f.cx - reach) as i64 - gsx).max(0), ((f.cx + reach) as i64 - gsx + 1).min(HS as i64));
        let (y0, y1) = (((f.cy - reach) as i64 - gsy).max(0), ((f.cy + reach) as i64 - gsy + 1).min(HS as i64));
        for y in y0..y1 {
            for x in x0..x1 {
                let k = y as usize * HS + x as usize;
                if out[k] || past_rim(f, (gsx + x) as f64 + 0.5, (gsy + y) as f64 + 0.5) > MARGIN_SQ {
                    continue;
                }
                out[k] = grade(base, x as usize, y as usize) < GENTLE;
            }
        }
    }
    out
}

/// Grade (ft per ft) of the `HS`² grid at (x, y), from its neighbours (one-sided at the rim).
fn grade(h: &[f32], x: usize, y: usize) -> f64 {
    let at = |x: usize, y: usize| h[y * HS + x] as f64;
    let (xa, xb) = (x.saturating_sub(1), (x + 1).min(HS - 1));
    let (ya, yb) = (y.saturating_sub(1), (y + 1).min(HS - 1));
    let gx = (at(xb, y) - at(xa, y)) / ((xb - xa).max(1) as f64 * super::SQUARE_FT);
    let gy = (at(x, yb) - at(x, ya)) / ((yb - ya).max(1) as f64 * super::SQUARE_FT);
    crate::core::sqrt(gx * gx + gy * gy)
}

/// Sets `SOFT` on the chunk's marked squares and on a crag's back and horns (`crag`: its lift on
/// the `HS`² grid, `crag_m`: bit 2 by its face; empty when there is none), but not by the face,
/// which keeps its rock and ink.
pub(super) fn flag(edges: &mut [u8], mask: &[bool], crag: &[f32], crag_m: &[u8]) {
    if mask.is_empty() && crag.is_empty() {
        return;
    }
    // Within `r` squares of the halo square (x, y) (clipped to the halo grid).
    let near = |x: usize, y: usize, r: usize, hit: &dyn Fn(usize) -> bool| {
        (y.saturating_sub(r)..(y + r + 1).min(HS)).any(|yy| (x.saturating_sub(r)..(x + r + 1).min(HS)).any(|xx| hit(yy * HS + xx)))
    };
    for k in 0..SQ * SQ {
        let (x, y) = (k % SQ + 1, k / SQ + 1);
        let h = y * HS + x;
        let (on_crag, by_face) = if crag.is_empty() { (false, false) } else { (near(x, y, 2, &|q| crag[q] > 0.0), crag_m[h] & 2 != 0) };
        if ((!mask.is_empty() && mask[h]) || on_crag) && !by_face {
            edges[k] |= SOFT;
        }
    }
}
