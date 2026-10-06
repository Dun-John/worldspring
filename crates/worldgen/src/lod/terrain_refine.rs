//! Terrain height tiles for every quadtree level.
//!
//! Levels coarser than `first_refine_level` sample the T0 grid. Finer levels are
//! Catmull-Rom interpolation of the parent tile plus band-limited detail noise whose
//! wavelength is below what the parent can represent, so the parent's shape is kept.
//!
//! Invariant (seams): every sample, halo included, is a pure function of its global
//! lattice index at that level. Positions are always `global_index * spacing` (exact in
//! f64), and every stencil reads only parent samples that exist in the parent's halo.

use super::rivers::{self, Piece};
use super::roads::{self, RoadPiece};
use crate::World;
use crate::core::{
    noise::{gradient2, smoothstep},
    rng,
    tile::{HALO, PADDED, TILE_N, TileKey},
};
use crate::t0::{T0, hydro::DRY};

/// Detail amplitude scale: amplitude = C * wavelength^H (ft).
const DETAIL_C: f64 = 0.05;
const DETAIL_H: f64 = 0.8;
/// Detail wavelength in samples of the level being generated.
const DETAIL_WAVELENGTH: f64 = 5.0;

/// Padded heights: `PADDED`² f32, row-major; sample (i, j) of the tile, with i and j in
/// `-HALO ..= TILE_N + HALO`, lives at `(j + HALO) * PADDED + (i + HALO)`.
pub type Padded = Vec<f32>;

#[inline]
pub fn padded_index(i: i64, j: i64) -> usize {
    ((j + HALO as i64) as usize) * PADDED + (i + HALO as i64) as usize
}

/// Gully spacing and blending-cell size, in samples of the level being generated.
const GULLY_SPACING: f64 = 3.0;
const GULLY_CELL: f64 = 8.0;
/// Rivers re-carve a band this many samples wide at each level (coarser levels carved the
/// broad valley already; finer levels sharpen the channel).
const CARVE_REACH_SAMPLES: f64 = 8.0;
/// Levels with coarser sample spacing than this skip carving.
const CARVE_MAX_SPACING_FT: f64 = 2_500.0;

pub struct TerrainOut {
    /// Final (carved) heights, `PADDED`².
    pub padded: Padded,
    /// River water surface per padded sample (`DRY` outside channels).
    pub river_water: Vec<f32>,
    /// River polylines near the tile, sampled for this level.
    pub pieces: Vec<Piece>,
    /// Road polylines near the tile, sampled for this level.
    pub roads: Vec<RoadPiece>,
    /// Road class + 1 per padded sample (0 = none); empty where roads are not carved.
    pub road_mask: Vec<u8>,
}

pub fn terrain_tile(world: &World, t0: &T0, key: &TileKey, parent: Option<&[f32]>) -> TerrainOut {
    let g = &world.geom;
    let s = g.spacing_ft(key.level);
    let (ox, oy) = g.tile_origin_ft(key);
    let size = g.tile_size_ft(key.level);
    let refined = key.level >= g.first_refine_level;
    let road_pieces = roads::pieces(&t0.roads, [ox, oy, ox + size, oy + size], (HALO as f64 + CORRIDOR_OUT_SAMPLES) * s + 200.0, s.max(20.0));
    // Road corridors: at levels too coarse to carve a road bed, refinement detail fades out
    // near roads, so the ground stays close to the road's profile and the fine levels only
    // cut and fill a few feet (instead of trenching through coarse-level relief).
    let corridor = (refined && s > roads::CARVE_MAX_SPACING_FT).then(|| road_corridor(&road_pieces, key, s));
    let mut padded = if refined {
        refine(world, t0, parent.expect("refined levels need the parent tile"), key, corridor.as_deref())
    } else {
        sample_t0(world, t0, key)
    };
    // Shore banks: a pure function of position, applied at every level (idempotent), so
    // lake and sea shores follow the smooth water contour at any zoom. (Refined levels apply
    // them inside `refine`, sharing the water lookup.)
    if !refined {
        for j in -(HALO as i64)..=(TILE_N + HALO) as i64 {
            for i in -(HALO as i64)..=(TILE_N + HALO) as i64 {
                let (level, mask) = t0.sample_lake((key.x as i64 * TILE_N as i64 + i) as f64 * s, (key.y as i64 * TILE_N as i64 + j) as f64 * s);
                let k = padded_index(i, j);
                padded[k] = T0::shore_bank(padded[k] as f64, level, mask) as f32;
            }
        }
    }
    let mut river_water = vec![DRY; PADDED * PADDED];
    // Settlement pads (weights per padded sample) where rivers are carved: they get the town
    // valley (`rivers::carve`).
    let carving = refined && s <= CARVE_MAX_SPACING_FT;
    let town = if carving { town_weights(t0, key, s) } else { None };
    // Each river is sampled as far out as it carves (plus the halo and a sample of slack).
    let reach_of = |w: f64| {
        let r = rivers::carve_reach(w, s, CARVE_REACH_SAMPLES);
        if town.is_some() { r.max(rivers::TOWN_VALLEY_FT) } else { r }
    };
    let pieces = rivers::pieces(&t0.rivers, [ox, oy, ox + size, oy + size], &|w| reach_of(w) + (HALO as f64 + 1.0) * s, s, t0.cell_ft);
    let mut road_mask = Vec::new();
    // Road beds first so rivers cut through them (bridges span the channel).
    if refined && s <= roads::CARVE_MAX_SPACING_FT {
        let origin = [
            (key.x as i64 * TILE_N as i64 - HALO as i64) as f64 * s,
            (key.y as i64 * TILE_N as i64 - HALO as i64) as f64 * s,
        ];
        road_mask = roads::carve(&road_pieces, &mut padded, PADDED, origin, s);
    }
    // Coarse levels already have the T0 valleys and cannot resolve a channel; carving
    // starts where samples get fine enough to matter (children carve for themselves).
    if refined && s <= CARVE_MAX_SPACING_FT {
        // Padded sample (0, 0) is lattice index -HALO: position = global index * spacing.
        let origin = [
            (key.x as i64 * TILE_N as i64 - HALO as i64) as f64 * s,
            (key.y as i64 * TILE_N as i64 - HALO as i64) as f64 * s,
        ];
        rivers::carve(&pieces, &mut padded, &mut river_water, PADDED, origin, s, CARVE_REACH_SAMPLES, &|x, y| t0.sample_water(x, y), town.as_deref());
    }
    TerrainOut { padded, river_water, pieces, roads: road_pieces, road_mask }
}

/// Each padded sample's settlement-pad weight (as `refine` blends the ground to what the
/// layouts were planned on: 1 within 0.9 of a settlement's reach, easing to 0 at 1.2), or
/// None where no settlement reaches the tile.
fn town_weights(t0: &T0, key: &TileKey, s: f64) -> Option<Vec<f32>> {
    let (n, h) = (TILE_N as i64, HALO as i64);
    let (gx0, gy0) = (key.x as i64 * n, key.y as i64 * n);
    let (tx0, ty0, tx1, ty1) = ((gx0 - h) as f64 * s, (gy0 - h) as f64 * s, (gx0 + n + h) as f64 * s, (gy0 + n + h) as f64 * s);
    let pads: Vec<(f64, f64, f64)> = t0
        .settlements
        .iter()
        .map(|st| (st.x, st.y, crate::town::reach(st)))
        .filter(|&(x, y, r)| x + 1.2 * r >= tx0 && x - 1.2 * r <= tx1 && y + 1.2 * r >= ty0 && y - 1.2 * r <= ty1)
        .collect();
    if pads.is_empty() {
        return None;
    }
    let mut out = vec![0f32; PADDED * PADDED];
    for cj in -h..=n + h {
        let y = (gy0 + cj) as f64 * s;
        for ci in -h..=n + h {
            let x = (gx0 + ci) as f64 * s;
            out[padded_index(ci, cj)] = pads.iter().fold(0.0f64, |m, &(px, py, r)| {
                if (x - px).abs() >= 1.2 * r || (y - py).abs() >= 1.2 * r {
                    return m;
                }
                let d = crate::core::sqrt((x - px) * (x - px) + (y - py) * (y - py));
                m.max(1.0 - smoothstep(0.9 * r, 1.2 * r, d))
            }) as f32;
        }
    }
    out.iter().any(|w| *w > 0.0).then_some(out)
}

fn sample_t0(world: &World, t0: &T0, key: &TileKey) -> Padded {
    let s = world.geom.spacing_ft(key.level);
    let (gx0, gy0) = (key.x as i64 * TILE_N as i64, key.y as i64 * TILE_N as i64);
    let h = HALO as i64;
    let mut out = vec![0f32; PADDED * PADDED];
    for j in -h..=TILE_N as i64 + h {
        let y = (gy0 + j) as f64 * s;
        for i in -h..=TILE_N as i64 + h {
            let x = (gx0 + i) as f64 * s;
            out[padded_index(i, j)] = t0.sample(x, y, s) as f32;
        }
    }
    out
}

/// Catmull-Rom weights for the four nodes around fraction `t` (as in `T0::ground_with`).
fn catmull_rom_weights(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [0.5 * (-t3 + 2.0 * t2 - t), 0.5 * (3.0 * t3 - 5.0 * t2 + 2.0), 0.5 * (-3.0 * t3 + 4.0 * t2 + t), 0.5 * (t3 - t2)]
}

/// Catmull-Rom at t = 0.5.
const HALF: [f32; 4] = [-0.0625, 0.5625, 0.5625, -0.0625];

/// Road corridors, in samples of the level: detail is gone within `IN` and back by `OUT`.
const CORRIDOR_IN_SAMPLES: f64 = 1.0;
const CORRIDOR_OUT_SAMPLES: f64 = 3.5;

/// Detail weight per padded sample (1 = full detail, 0 on the road).
fn road_corridor(pieces: &[RoadPiece], key: &TileKey, s: f64) -> Vec<f32> {
    let mut w = vec![1f32; PADDED * PADDED];
    let origin = [(key.x as i64 * TILE_N as i64 - HALO as i64) as f64 * s, (key.y as i64 * TILE_N as i64 - HALO as i64) as f64 * s];
    // The mask is smooth over several samples: half-sample vertices are plenty (the road
    // pieces are sampled far more finely for carving). A vertex is kept where the line enters
    // a new world-grid cell, so neighbouring tiles thin a road identically.
    let cell = 0.5 * s;
    let key = |p: [f64; 2]| (crate::core::floor(p[0] / cell) as i64, crate::core::floor(p[1] / cell) as i64);
    for piece in pieces {
        let half = 0.5 * piece.class.width_ft();
        let (w_in, w_out) = (half + CORRIDOR_IN_SAMPLES * s, half + CORRIDOR_OUT_SAMPLES * s);
        let last = piece.pts.len().saturating_sub(1);
        let pts: Vec<[f64; 2]> = piece.pts.iter().enumerate().filter(|&(i, c)| i == 0 || i == last || key(c.p) != key(piece.pts[i - 1].p)).map(|(_, c)| c.p).collect();
        for seg in pts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let (sx0, sx1) = ((a[0].min(b[0]) - w_out - origin[0]) / s, (a[0].max(b[0]) + w_out - origin[0]) / s);
            let (sy0, sy1) = ((a[1].min(b[1]) - w_out - origin[1]) / s, (a[1].max(b[1]) + w_out - origin[1]) / s);
            if sx1 < 0.0 || sy1 < 0.0 || sx0 > (PADDED - 1) as f64 || sy0 > (PADDED - 1) as f64 {
                continue;
            }
            let (ix0, ix1) = (sx0.ceil().max(0.0) as usize, (sx1.floor() as usize).min(PADDED - 1));
            let (iy0, iy1) = (sy0.ceil().max(0.0) as usize, (sy1.floor() as usize).min(PADDED - 1));
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            for iy in iy0..=iy1 {
                let py = origin[1] + iy as f64 * s;
                for ix in ix0..=ix1 {
                    let px = origin[0] + ix as f64 * s;
                    let t = (((px - a[0]) * dx + (py - a[1]) * dy) / len2).clamp(0.0, 1.0);
                    let (cx, cy) = (a[0] + dx * t - px, a[1] + dy * t - py);
                    let d = crate::core::sqrt(cx * cx + cy * cy);
                    let k = iy * PADDED + ix;
                    w[k] = w[k].min(smoothstep(w_in, w_out, d) as f32);
                }
            }
        }
    }
    w
}

fn refine(world: &World, t0: &T0, parent: &[f32], key: &TileKey, corridor: Option<&[f32]>) -> Padded {
    assert_eq!(parent.len(), PADDED * PADDED);
    let p = world.params();
    let s = world.geom.spacing_ft(key.level);
    let h = HALO as i64;
    let n = TILE_N as i64;
    // Child sample c maps to parent coordinate off + c/2, where off is 0 or 128.
    let off_x = (key.x & 1) as i64 * (n / 2);
    let off_y = (key.y & 1) as i64 * (n / 2);
    let par = |pi: i64, pj: i64| parent[padded_index(pi, pj)];

    // Horizontal pass over the parent rows the vertical pass will need.
    let pj_min = off_y - h / 2 - 1;
    let pj_max = off_y + n / 2 + h / 2 + 1;
    let rows = (pj_max - pj_min + 1) as usize;
    let mut tmp = vec![0f32; rows * PADDED];
    for r in 0..rows {
        let pj = pj_min + r as i64;
        for c in -h..=n + h {
            let t = 2 * off_x + c;
            let pu = t.div_euclid(2);
            let v = if t & 1 == 0 {
                par(pu, pj)
            } else {
                HALF[0] * par(pu - 1, pj) + HALF[1] * par(pu, pj) + HALF[2] * par(pu + 1, pj) + HALF[3] * par(pu + 2, pj)
            };
            tmp[r * PADDED + (c + h) as usize] = v;
        }
    }
    let row = |pj: i64, c: i64| tmp[(pj - pj_min) as usize * PADDED + (c + h) as usize];

    let seed = rng::hash2(world.stream("lod.detail"), key.level as i64, 0);
    let gseed = rng::hash2(world.stream("lod.gully"), key.level as i64, 0);
    let wavelength = DETAIL_WAVELENGTH * s;
    let amp = DETAIL_C * libm::pow(wavelength, DETAIL_H) * p.ruggedness;
    let (gx0, gy0) = (key.x as i64 * n, key.y as i64 * n);
    let inv_2s = 1.0 / (2.0 * s);
    // Settlement pads: towns and their fields stand on the smooth ground their layout was
    // planned on (the T0 surface), so fine detail fades out over each settlement's reach.
    let (tx0, ty0, tx1, ty1) = ((gx0 - h) as f64 * s, (gy0 - h) as f64 * s, (gx0 + n + h) as f64 * s, (gy0 + n + h) as f64 * s);
    let pads: Vec<(f64, f64, f64)> = t0
        .settlements
        .iter()
        .map(|st| (st.x, st.y, crate::town::reach(st)))
        .filter(|&(x, y, r)| x + 1.1 * r >= tx0 && x - 1.1 * r <= tx1 && y + 1.1 * r >= ty0 && y - 1.1 * r <= ty1)
        .collect();
    // Inside a settlement the ground is exactly T0::ground_at (what its layout was planned
    // on: shorelines, piers and pads agree with the drawn terrain). Lattice nodes cached.
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
    // The lattice nodes this tile's samples can touch (Catmull-Rom reaches one node out).
    let (nx0, ny0) = (crate::core::floor(tx0 / lattice) as i64 - 1, crate::core::floor(ty0 / lattice) as i64 - 1);
    let (nx1, ny1) = (crate::core::floor(tx1 / lattice) as i64 + 2, crate::core::floor(ty1 / lattice) as i64 + 2);
    let nw = (nx1 - nx0 + 1) as usize;
    let nodes: Vec<f64> = if pads.is_empty() {
        Vec::new()
    } else {
        (ny0..=ny1).flat_map(|j| (nx0..=nx1).map(move |i| (i, j))).map(|(i, j)| t0.ground_node(i, j, lattice)).collect()
    };

    let mut out = vec![0f32; PADDED * PADDED];
    for cj in -h..=n + h {
        let t = 2 * off_y + cj;
        let pv = t.div_euclid(2);
        let odd = t & 1 == 1;
        let ny = (gy0 + cj) as f64 / DETAIL_WAVELENGTH;
        // Pads this row can touch.
        let y = (gy0 + cj) as f64 * s;
        let row_pads: Vec<(f64, f64, f64)> = pads.iter().copied().filter(|&(_, py, r)| (y - py).abs() < 1.2 * r).collect();
        // This row's ground: the lattice nodes interpolated down each column once
        // (Catmull-Rom in y), leaving a 4-tap interpolation in x per sample.
        let col: Vec<f64> = if row_pads.is_empty() {
            Vec::new()
        } else {
            let v = y / lattice;
            let j0 = crate::core::floor(v);
            let wv = catmull_rom_weights(v - j0);
            (0..nw)
                .map(|c| (0..4).map(|b| wv[b] * nodes[((j0 as i64 + b as i64 - 1 - ny0) as usize) * nw + c]).sum())
                .collect()
        };
        for ci in -h..=n + h {
            let base = if odd {
                HALF[0] * row(pv - 1, ci) + HALF[1] * row(pv, ci) + HALF[2] * row(pv + 1, ci) + HALF[3] * row(pv + 2, ci)
            } else {
                row(pv, ci)
            } as f64;

            // Parent-cell slope drives roughness (steep ground gets rougher detail).
            let pu = (2 * off_x + ci).div_euclid(2);
            let (a, b, c, d) = (par(pu, pv), par(pu + 1, pv), par(pu, pv + 1), par(pu + 1, pv + 1));
            let sx = ((b - a) + (d - c)) as f64 * 0.5 * inv_2s;
            let sy = ((c - a) + (d - b)) as f64 * 0.5 * inv_2s;
            let slope = crate::core::sqrt(sx * sx + sy * sy);

            let above = base - p.sea_level_ft;
            let mut rough = 0.22 + 2.4 * smoothstep(0.03, 0.6, slope) + 0.8 * smoothstep(1_500.0, 6_000.0, above);
            if above < 0.0 {
                rough *= 0.25;
            }
            let nx = (gx0 + ci) as f64 / DETAIL_WAVELENGTH;
            let mut detail = amp * rough * gradient2(seed, nx, ny);

            // Gullies: ridges and furrows running down the fall line on steep ground, blended
            // over local cells so the direction stays stable at any world coordinate.
            let steep = smoothstep(0.05, 0.4, slope);
            if steep > 0.0 && above > 0.0 {
                let (px, py) = (-sy / slope, sx / slope);
                let (gx, gy) = ((gx0 + ci) as f64 / GULLY_CELL, (gy0 + cj) as f64 / GULLY_CELL);
                let (cx, cy) = (crate::core::floor(gx), crate::core::floor(gy));
                let (fx, fy) = (gx - cx, gy - cy);
                let (wx, wy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
                let mut v = 0.0;
                for (ox, oy, wt) in [(0.0, 0.0, (1.0 - wx) * (1.0 - wy)), (1.0, 0.0, wx * (1.0 - wy)), (0.0, 1.0, (1.0 - wx) * wy), (1.0, 1.0, wx * wy)] {
                    let (rx, ry) = ((fx - ox) * GULLY_CELL, (fy - oy) * GULLY_CELL);
                    let phase = rng::unit(rng::hash2(gseed, (cx + ox) as i64, (cy + oy) as i64));
                    let u = (rx * px + ry * py) / GULLY_SPACING + phase;
                    v += wt * (1.0 - 2.0 * crate::core::fabs(u - crate::core::floor(u) - 0.5));
                }
                let along = 0.6 + 0.4 * gradient2(gseed ^ 0x77, nx * 0.4, ny * 0.4);
                detail += amp * 1.6 * steep * along * (v - 0.5);
            }
            // Shorelines stay where the parent put them: near a lake or sea surface the
            // detail fades out, otherwise a few feet of noise on flat ground would move the
            // shore by thousands of feet from one level to the next.
            if let Some(c) = corridor {
                detail *= c[padded_index(ci, cj)] as f64;
            }
            let mut pad = 0.0;
            if !row_pads.is_empty() {
                let x = (gx0 + ci) as f64 * s;
                pad = row_pads.iter().fold(0.0f64, |m, &(px, py, r)| {
                    if (x - px).abs() >= 1.2 * r {
                        return m;
                    }
                    let d = crate::core::sqrt((x - px) * (x - px) + (y - py) * (y - py));
                    m.max(1.0 - smoothstep(0.9 * r, 1.2 * r, d))
                });
                detail *= 1.0 - pad;
            }
            let (w, mask) = t0.sample_lake((gx0 + ci) as f64 * s, (gy0 + cj) as f64 * s);
            let mut h = base + detail;
            if w > DRY {
                let band = 3.0 + 2.5 * amp * rough;
                h = base + detail * smoothstep(0.0, band, crate::core::fabs(base - w as f64));
                h = T0::shore_bank(h, w, mask);
            }
            if pad > 0.0 {
                let x = (gx0 + ci) as f64 * s;
                let u = x / lattice;
                let i0 = crate::core::floor(u);
                let wu = catmull_rom_weights(u - i0);
                let g: f64 = (0..4).map(|a| wu[a] * col[(i0 as i64 + a as i64 - 1 - nx0) as usize]).sum();
                // The shore bank at this point, from the lookup above (same as ground_at's).
                let g = if w > DRY { T0::shore_bank(g, w, mask) } else { g };
                h += (g - h) * pad;
            }
            out[padded_index(ci, cj)] = h as f32;
        }
    }
    out
}
