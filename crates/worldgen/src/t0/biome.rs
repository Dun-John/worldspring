//! Biomes from climate (a weighted Whittaker diagram) plus terrain overrides: alpine and ice
//! on high ground, swamps on flat wet floodplains and lake margins, volcanic waste around
//! active volcanoes, salt flats in dry basins (blighted woods and ashlands are only painted).
//! Each cell keeps its two best climate biomes and a blend factor so the renderer can draw
//! ecotones instead of hard borders.

use super::climate::Climate;
use super::hydro::{Hydro, LakeKind, NO_LAKE};
use crate::World;
use crate::core::noise::smoothstep;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Biome {
    Ocean = 0,
    Lake,
    Ice,
    Tundra,
    Alpine,
    Taiga,
    TemperateForest,
    TemperateRainforest,
    Grassland,
    Steppe,
    ColdDesert,
    HotDesert,
    Savanna,
    Jungle,
    Swamp,
    Volcanic,
    SaltFlat,
    /// Painted only: woods sickened by a curse or rot (dead and dying trees, fungus).
    Blight,
    /// Painted only: land buried under old ash falls (grey, sparse, burnt stumps).
    Ashland,
}

pub const ALL: [Biome; 19] = [
    Biome::Ocean,
    Biome::Lake,
    Biome::Ice,
    Biome::Tundra,
    Biome::Alpine,
    Biome::Taiga,
    Biome::TemperateForest,
    Biome::TemperateRainforest,
    Biome::Grassland,
    Biome::Steppe,
    Biome::ColdDesert,
    Biome::HotDesert,
    Biome::Savanna,
    Biome::Jungle,
    Biome::Swamp,
    Biome::Volcanic,
    Biome::SaltFlat,
    Biome::Blight,
    Biome::Ashland,
];

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "ocean",
            Biome::Lake => "lake",
            Biome::Ice => "ice",
            Biome::Tundra => "tundra",
            Biome::Alpine => "alpine",
            Biome::Taiga => "taiga",
            Biome::TemperateForest => "temperate_forest",
            Biome::TemperateRainforest => "temperate_rainforest",
            Biome::Grassland => "grassland",
            Biome::Steppe => "steppe",
            Biome::ColdDesert => "cold_desert",
            Biome::HotDesert => "hot_desert",
            Biome::Savanna => "savanna",
            Biome::Jungle => "jungle",
            Biome::Swamp => "swamp",
            Biome::Volcanic => "volcanic",
            Biome::SaltFlat => "salt_flat",
            Biome::Blight => "blighted_woods",
            Biome::Ashland => "ashlands",
        }
    }

    pub fn from_u8(v: u8) -> Biome {
        ALL.get(v as usize).copied().unwrap_or(Biome::Grassland)
    }
}

/// Whittaker climate niches: (biome, mean °C, annual mm, spread °C, spread mm).
const NICHES: [(Biome, f64, f64, f64, f64); 11] = [
    (Biome::Tundra, -7.0, 300.0, 5.0, 400.0),
    (Biome::Taiga, 0.5, 600.0, 4.5, 450.0),
    (Biome::TemperateForest, 10.0, 1100.0, 5.0, 450.0),
    (Biome::TemperateRainforest, 10.0, 2400.0, 5.0, 700.0),
    (Biome::Grassland, 13.0, 580.0, 7.0, 220.0),
    (Biome::Steppe, 8.0, 330.0, 7.0, 140.0),
    (Biome::ColdDesert, 6.0, 130.0, 7.0, 120.0),
    (Biome::HotDesert, 22.0, 120.0, 6.0, 140.0),
    (Biome::Savanna, 24.0, 900.0, 4.0, 400.0),
    (Biome::Jungle, 26.0, 2300.0, 4.0, 800.0),
    (Biome::Grassland, 20.0, 650.0, 4.0, 200.0),
];

pub struct Volcanic {
    /// (cell x, cell y, radius in cells) of active volcanoes.
    pub vents: Vec<(f64, f64, f64)>,
}

/// Packed per cell: primary | secondary << 8 | blend(0..255) << 16.
pub fn classify(world: &World, w: usize, h: usize, cell_ft: f64, height: &[f64], land: &[bool], clim: &Climate, hydro: &Hydro, volcanic: &Volcanic) -> Vec<u32> {
    let p = world.params();
    let sea = p.sea_level_ft;
    let weights: Vec<f64> = NICHES.iter().map(|n| p.biome_weight(n.0.name())).collect();
    let swamp_w = p.biome_weight("swamp");
    let noise = world.stream("t0.biome.noise");
    let n = w * h;
    let mut out = vec![0u32; n];

    // Near-river / lake wetness for swamps, spread a few miles by a cheap dilation (and the
    // niches' noise, in miles too).
    let k_cell = crate::core::tile::cell_scale(cell_ft);
    let (spread, fade) = ((crate::core::round(3.0 / k_cell) as usize).max(1), libm::pow(0.7, k_cell.min(1.0)) as f32);
    let mut wet = vec![0.0f32; n];
    for i in 0..n {
        if hydro.lake_of[i] != NO_LAKE {
            wet[i] = 1.0;
        } else if land[i] {
            wet[i] = (libm::log10(hydro.discharge[i].max(1.0) as f64) as f32 - 4.5).clamp(0.0, 1.0);
        }
    }
    for _ in 0..spread {
        let prev = wet.clone();
        for j in 1..h - 1 {
            for i in 1..w - 1 {
                let k = j * w + i;
                let m = prev[k - 1].max(prev[k + 1]).max(prev[k - w]).max(prev[k + w]);
                wet[k] = prev[k].max(m * fade);
            }
        }
    }

    let mut niche_noise = crate::core::noise::Fbm::new(noise, 3, 2.0, 0.5);
    let mut lobes = crate::core::noise::Fbm::new(noise ^ 0x10be, 3, 2.0, 0.5);
    let mut ragged = crate::core::noise::Fbm::new(noise ^ 0x5a6e, 3, 2.0, 0.5);
    for k in 0..n {
        let (i, j) = (k % w, k / w);
        if !land[k] {
            out[k] = Biome::Ocean as u32;
            continue;
        }
        if hydro.lake_of[k] != NO_LAKE {
            let lake = &hydro.lakes[hydro.lake_of[k] as usize];
            out[k] = if lake.kind == LakeKind::SaltFlat { Biome::SaltFlat } else { Biome::Lake } as u32;
            continue;
        }
        let t = clim.temp[k] as f64;
        let pr = clim.precip[k] as f64;
        let nz = niche_noise.at(i as f64 * k_cell / 7.0, j as f64 * k_cell / 7.0);
        // Snowlines and the like stay ragged on fine cells, where the niches' noise is smooth.
        let edge = if k_cell < 1.0 { nz + (1.0 - k_cell) * 0.8 * ragged.at(i as f64 / 4.0, j as f64 / 4.0) } else { nz };

        // Climate niches: best two by weighted score.
        let mut score = [0.0f64; ALL.len()];
        for (idx, &(b, tc, pc, st, sp)) in NICHES.iter().enumerate() {
            let dt = (t - tc) / st;
            let dp = (pr - pc) / sp;
            let s = weights[idx] * libm::exp(-(dt * dt + dp * dp));
            score[b as usize] = score[b as usize].max(s);
        }
        let (mut first, mut second) = ((Biome::Grassland as usize, 0.0), (Biome::Grassland as usize, 0.0));
        for (b, &s) in score.iter().enumerate() {
            if s > first.1 {
                second = first;
                first = (b, s);
            } else if s > second.1 {
                second = (b, s);
            }
        }
        let mut b1 = Biome::from_u8(first.0 as u8);
        let mut b2 = Biome::from_u8(second.0 as u8);
        if first.1 <= 0.0 {
            // Every plausible biome is disabled by weights: fall back by temperature.
            b1 = if t < 0.0 { Biome::Tundra } else { Biome::Grassland };
            b2 = b1;
        }
        let mut blend = if first.1 > 0.0 { second.1 / (first.1 + second.1) } else { 0.0 };

        // Terrain overrides.
        let above = height[k] - sea;
        let slope = local_slope(w, h, height, i, j, cell_ft);
        let override_to = if t < -10.0 + 2.0 * edge && p.biome_weight("ice") > 0.0 {
            Some(Biome::Ice)
        } else if t < 3.0 + 2.0 * edge && above > 5_500.0 && p.biome_weight("alpine") > 0.0 {
            Some(Biome::Alpine)
        } else if volcanic.vents.iter().any(|&(vx, vy, r)| {
            // Lobed as flows run out from it, at any size of cone or cell.
            let (dx, dy) = (i as f64 - vx, j as f64 - vy);
            let d = crate::core::sqrt(dx * dx + dy * dy);
            if d >= 1.3 * r {
                return false;
            }
            let (ca, sa) = if d > 0.0 { (dx / d, dy / d) } else { (1.0, 0.0) };
            let lobe = lobes.at(vx + 2.2 * ca, vy + 2.2 * sa) + 0.25 * nz;
            d < r * (0.8 + 0.4 * lobe)
        }) && p.biome_weight("volcanic") > 0.0
        {
            Some(Biome::Volcanic)
        } else {
            let flat = 1.0 - smoothstep(0.002, 0.01, slope);
            let score = wet[k] as f64 * flat * smoothstep(700.0, 1300.0, pr) * smoothstep(0.0, 4.0, t) * swamp_w;
            (score + 0.25 * nz > 0.55).then_some(Biome::Swamp)
        };
        if let Some(o) = override_to {
            b2 = b1;
            b1 = o;
            blend = 0.0;
        }
        let blend_byte = ((blend * 2.0).min(1.0) * 255.0) as u32;
        out[k] = b1 as u32 | (b2 as u32) << 8 | blend_byte << 16;
    }
    out
}

pub fn local_slope(w: usize, h: usize, height: &[f64], i: usize, j: usize, cell_ft: f64) -> f64 {
    let at = |x: usize, y: usize| height[y.min(h - 1) * w + x.min(w - 1)];
    let gx = (at(i + 1, j) - at(i.saturating_sub(1), j)) / (2.0 * cell_ft);
    let gy = (at(i, j + 1) - at(i, j.saturating_sub(1))) / (2.0 * cell_ft);
    crate::core::sqrt(gx * gx + gy * gy)
}
