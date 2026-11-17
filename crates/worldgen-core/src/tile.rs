//! Quadtree tile geometry. World units are feet; the origin is the map's top-left corner.
//!
//! Every tile holds `TILE_SAMPLES`² corner-aligned height samples (neighbors share edge
//! samples exactly) plus a `HALO` ring, so children and gradients never need neighbor tiles.

use serde::{Deserialize, Serialize};

pub const TILE_N: usize = 256;
pub const TILE_SAMPLES: usize = TILE_N + 1;
pub const HALO: usize = 4;
pub const PADDED: usize = TILE_SAMPLES + 2 * HALO;
/// Sample spacing at the finest level: two samples per 5-ft battle grid square.
pub const FINEST_SPACING_FT: f64 = 2.5;
pub const FT_PER_MILE: f64 = 5280.0;
pub const T0_MAX_DIM: usize = 1024;
/// The default map's width: the continent-scale stages are tuned on its T0 cell, and measure
/// distances and areas against it on any other (`cell_scale`).
pub const REF_MAP_W_FT: f64 = 1200.0 * FT_PER_MILE;
pub const REF_CELL_FT: f64 = REF_MAP_W_FT / (T0_MAX_DIM - 1) as f64;

/// A grid cell of `cell_ft` against the default map's T0 cell: 1 on a 1200-mile map, 1/8 on a
/// 150-mile one. Lengths counted in cells divide by it, areas by its square.
#[inline]
pub fn cell_scale(cell_ft: f64) -> f64 {
    cell_ft / REF_CELL_FT
}

pub const LAYER_SURFACE: u8 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileKey {
    pub layer: u8,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl TileKey {
    pub fn surface(level: u8, x: u32, y: u32) -> Self {
        Self { layer: LAYER_SURFACE, level, x, y }
    }

    pub fn parent(&self) -> Option<TileKey> {
        (self.level > 0).then(|| TileKey { level: self.level - 1, x: self.x / 2, y: self.y / 2, ..*self })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WorldGeom {
    pub map_w_ft: f64,
    pub map_h_ft: f64,
    pub max_level: u8,
    /// Side of the square quadtree domain; the map occupies its top-left `map_w × map_h`.
    pub domain_ft: f64,
    pub t0_w: usize,
    pub t0_h: usize,
    pub t0_cell_ft: f64,
    /// Levels below this sample the T0 grid directly; this level and finer refine their parent.
    pub first_refine_level: u8,
    pub tile_n: usize,
    pub halo: usize,
}

impl WorldGeom {
    pub fn new(map_w_ft: f64, map_h_ft: f64) -> Self {
        let finest_tile = FINEST_SPACING_FT * TILE_N as f64;
        let longest = map_w_ft.max(map_h_ft);
        let mut max_level = 0u8;
        while finest_tile * ((1u64 << max_level) as f64) < longest {
            max_level += 1;
        }
        let domain_ft = finest_tile * (1u64 << max_level) as f64;
        let (t0_w, t0_h) = if map_w_ft >= map_h_ft {
            (T0_MAX_DIM, ((T0_MAX_DIM as f64 * map_h_ft / map_w_ft) as usize).max(2))
        } else {
            (((T0_MAX_DIM as f64 * map_w_ft / map_h_ft) as usize).max(2), T0_MAX_DIM)
        };
        let t0_cell_ft = map_w_ft / (t0_w - 1) as f64;
        let mut g = Self {
            map_w_ft,
            map_h_ft,
            max_level,
            domain_ft,
            t0_w,
            t0_h,
            t0_cell_ft,
            first_refine_level: 0,
            tile_n: TILE_N,
            halo: HALO,
        };
        g.first_refine_level = (0..=max_level).find(|&l| g.spacing_ft(l) < t0_cell_ft).unwrap_or(max_level);
        g
    }

    pub fn tile_size_ft(&self, level: u8) -> f64 {
        self.domain_ft / (1u64 << level) as f64
    }

    pub fn spacing_ft(&self, level: u8) -> f64 {
        self.tile_size_ft(level) / TILE_N as f64
    }

    pub fn tile_origin_ft(&self, key: &TileKey) -> (f64, f64) {
        let s = self.tile_size_ft(key.level);
        (key.x as f64 * s, key.y as f64 * s)
    }

    pub fn tiles_across(&self, level: u8) -> (u32, u32) {
        let s = self.tile_size_ft(level);
        ((self.map_w_ft / s).ceil().max(1.0) as u32, (self.map_h_ft / s).ceil().max(1.0) as u32)
    }

    pub fn tile_in_map(&self, key: &TileKey) -> bool {
        let (nx, ny) = self.tiles_across(key.level);
        key.level <= self.max_level && key.x < nx && key.y < ny
    }
}
