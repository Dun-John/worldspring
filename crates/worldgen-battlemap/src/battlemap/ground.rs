//! Ground by biome: object profiles, atmosphere, surfaces, beaches and micro-relief.

use super::*;

/// Object mix per biome: (kind, count per 100 squares, allowed in shallow water).
pub(super) fn profile(b: Biome) -> &'static [(Kind, f64, bool)] {
    use Kind::*;
    match b {
        Biome::TemperateForest => &[(TreeDeciduous, 9.0, false), (Bush, 3.0, false), (FallenLog, 0.4, false), (Stump, 0.4, false), (Boulder, 0.3, false), (Flowers, 0.3, false), (Mushrooms, 0.3, false), (Brambles, 0.25, false)],
        Biome::TemperateRainforest => &[(TreeDeciduous, 7.0, false), (TreeConifer, 5.0, false), (Thicket, 1.5, false), (FallenLog, 0.8, false), (Mushrooms, 0.6, false), (Boulder, 0.4, false)],
        Biome::Taiga => &[(TreeConifer, 9.0, false), (Boulder, 0.6, false), (FallenLog, 0.5, false), (Bush, 1.0, false), (Snowdrift, 0.3, false)],
        Biome::Jungle => &[(TreeJungle, 8.0, false), (TreePalm, 2.0, false), (Thicket, 4.0, false), (Bush, 5.0, false), (Flowers, 0.6, false), (Quicksand, 0.08, false)],
        Biome::Grassland => &[(TallGrass, 3.0, false), (Bush, 0.6, false), (TreeDeciduous, 0.35, false), (Boulder, 0.3, false), (Flowers, 1.0, false), (Brambles, 0.12, false)],
        Biome::Steppe => &[(TallGrass, 2.0, false), (Boulder, 0.4, false), (Bush, 0.5, false), (RockSmall, 1.0, false), (Bones, 0.1, false)],
        Biome::Savanna => &[(TallGrass, 3.0, false), (TreeAcacia, 0.7, false), (Bush, 1.0, false), (Boulder, 0.3, false), (Bones, 0.1, false)],
        Biome::HotDesert => &[(Cactus, 0.35, false), (Boulder, 0.25, false), (RockSmall, 0.6, false), (Bones, 0.15, false), (Quicksand, 0.05, false)],
        Biome::ColdDesert => &[(Boulder, 0.8, false), (RockSmall, 2.0, false), (RockPile, 0.4, false), (Bush, 0.4, false)],
        Biome::Tundra => &[(Boulder, 0.6, false), (RockSmall, 1.0, false), (Snowdrift, 0.6, false), (Bush, 0.3, false), (ThinIce, 0.03, false)],
        Biome::Alpine => &[(Boulder, 1.6, false), (RockPile, 1.0, false), (RockSmall, 2.0, false), (TreeConifer, 0.4, false), (Scree, 0.3, false)],
        Biome::Ice => &[(Snowdrift, 2.0, false), (IceSpire, 0.3, false), (Boulder, 0.2, false), (ThinIce, 0.08, false)],
        Biome::Swamp => &[(TreeWillow, 2.5, false), (TreeDead, 1.5, false), (Reeds, 5.0, true), (Bog, 0.4, false), (Mushrooms, 0.4, false), (Bush, 1.0, false)],
        Biome::Volcanic => &[(BasaltPillar, 0.5, false), (Boulder, 1.0, false), (Obsidian, 0.4, false), (SteamVent, 0.15, false), (LavaPool, 0.06, false), (RockSmall, 1.5, false)],
        Biome::SaltFlat => &[(RockSmall, 0.3, false), (Bones, 0.08, false)],
        Biome::Blight => &[(TreeDead, 7.0, false), (FallenLog, 1.0, false), (Stump, 0.6, false), (Mushrooms, 0.9, false), (Brambles, 0.3, false), (Bog, 0.2, false), (Bones, 0.12, false)],
        Biome::Ashland => &[(TreeDead, 0.6, false), (Stump, 0.5, false), (Boulder, 0.6, false), (RockPile, 0.2, false), (Obsidian, 0.15, false), (Bones, 0.08, false)],
        Biome::Ocean | Biome::Lake => &[(Reeds, 3.0, true), (Boulder, 0.3, false)],
    }
}

/// Cover object used by the guarantee when a cell lacks cover.
pub(super) fn fallback_cover(b: Biome, rng: &mut Pcg32) -> Kind {
    use Kind::*;
    let opts: &[Kind] = match b {
        Biome::TemperateForest | Biome::TemperateRainforest => &[FallenLog, Boulder, Thicket, Bush],
        Biome::Taiga => &[Boulder, FallenLog, Bush],
        Biome::Jungle => &[Thicket, Bush, FallenLog],
        Biome::Grassland | Biome::Steppe | Biome::Savanna => &[Bush, Boulder, RockPile, Bush],
        Biome::HotDesert => &[Boulder, RockPile, Cactus],
        Biome::ColdDesert | Biome::Alpine => &[Boulder, RockPile],
        Biome::Tundra | Biome::Ice => &[Snowdrift, Boulder],
        Biome::Swamp => &[TreeDead, Bush, Thicket],
        Biome::Volcanic => &[Boulder, BasaltPillar, Obsidian],
        Biome::Blight => &[TreeDead, FallenLog, Boulder],
        Biome::Ashland => &[Boulder, RockPile, TreeDead],
        Biome::SaltFlat | Biome::Ocean | Biome::Lake => &[Boulder, RockPile],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

pub(super) fn fallback_feature(b: Biome, rng: &mut Pcg32) -> Kind {
    use Kind::*;
    let opts: &[Kind] = match b {
        Biome::TemperateForest | Biome::TemperateRainforest => &[Mushrooms, Flowers, Brambles],
        Biome::Taiga => &[Mushrooms, Bones],
        Biome::Jungle => &[Flowers, Quicksand, Mushrooms],
        Biome::Grassland | Biome::Savanna | Biome::Steppe => &[Flowers, Bones, Brambles, RockSmall],
        Biome::HotDesert | Biome::SaltFlat => &[Bones, RockSmall, Quicksand],
        Biome::ColdDesert | Biome::Alpine => &[Scree, RockSmall, Bones],
        Biome::Tundra | Biome::Ice => &[ThinIce, Bones],
        Biome::Swamp => &[Bog, Mushrooms],
        Biome::Volcanic => &[SteamVent, Obsidian],
        Biome::Blight => &[Mushrooms, Bog, Bones],
        Biome::Ashland => &[Bones, Obsidian],
        Biome::Ocean | Biome::Lake => &[Reeds, RockSmall],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

pub(super) fn atmosphere_for(b: Biome, h: u64) -> Atmosphere {
    let r = unit(h);
    match b {
        Biome::Swamp => if r < 0.5 { Atmosphere::Fog } else { Atmosphere::Fireflies },
        Biome::TemperateRainforest | Biome::Jungle => if r < 0.5 { Atmosphere::Mist } else { Atmosphere::Drizzle },
        Biome::TemperateForest => if r < 0.35 { Atmosphere::FallingLeaves } else if r < 0.5 { Atmosphere::Mist } else { Atmosphere::None },
        Biome::Taiga | Biome::Tundra | Biome::Ice => if r < 0.6 { Atmosphere::Snowfall } else { Atmosphere::None },
        Biome::Alpine => if r < 0.4 { Atmosphere::Snowfall } else if r < 0.6 { Atmosphere::Mist } else { Atmosphere::None },
        Biome::HotDesert => if r < 0.4 { Atmosphere::HeatHaze } else if r < 0.6 { Atmosphere::BlowingSand } else { Atmosphere::None },
        Biome::Volcanic => if r < 0.7 { Atmosphere::Embers } else { Atmosphere::HeatHaze },
        Biome::Blight => if r < 0.6 { Atmosphere::Fog } else { Atmosphere::Mist },
        Biome::Ashland => if r < 0.3 { Atmosphere::Embers } else { Atmosphere::None },
        _ => Atmosphere::None,
    }
}

/// Whether tundra square `k` of the chunk at (`ox`, `oy`) lies in a snow patch
/// (`t0::biome::tundra_snow` at the square's centre; only battlemaps show the snow).
pub(super) fn tundra_snowy(world: &World, ox: f64, oy: f64, k: usize) -> bool {
    let (x, y) = (ox + ((k % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((k / SQ) as f64 + 0.5) * SQUARE_FT);
    crate::t0::biome::tundra_snow(world.file.seed, x, y) >= crate::t0::biome::TUNDRA_SNOW_AT
}

pub(super) fn surface_for(b: Biome, slope: f64) -> Surface {
    if slope > 0.9 {
        return Surface::Rock;
    }
    match b {
        Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle => Surface::ForestFloor,
        Biome::Grassland => Surface::Grass,
        Biome::Steppe | Biome::Savanna => Surface::DryGrass,
        Biome::HotDesert => Surface::Sand,
        Biome::ColdDesert => Surface::Dirt,
        Biome::Tundra => if slope > 0.4 { Surface::Rock } else { Surface::Grass },
        Biome::Alpine => if slope > 0.5 { Surface::Rock } else { Surface::Snow },
        Biome::Ice => Surface::Snow,
        Biome::Swamp => Surface::Mud,
        Biome::Volcanic => Surface::Ash,
        Biome::SaltFlat => Surface::Salt,
        Biome::Blight => Surface::Mud,
        Biome::Ashland => Surface::Ash,
        Biome::Ocean | Biome::Lake => Surface::Sand,
    }
}

/// Whether a dry square's ground is sand: a beach (`beach`), or the biome's own (a hot desert's,
/// or the sea's or a lake's by their shores).
pub fn sand_ground(b: Biome, depth: f64, level: f64, sea: f64, slope: f64) -> bool {
    beach(depth, level, sea, slope) || surface_for(b, slope) == Surface::Sand
}

/// Largest reach of a feature from its centre (squares): features come from the chunk's cells
/// and their neighbours, so they must not reach past the next cell (seams).
pub(super) const RELIEF_REACH: f64 = 20.0;

pub(super) fn relief_in_cell(seed: u64, gx: i64, gy: i64, biome: Biome) -> Vec<Relief> {
    let hsh = hash2(seed, gx, gy);
    let mut rng = Pcg32::new(hsh, 7);
    let (p_exist, h_lo, h_hi, steep, elong): (f64, f64, f64, f64, f64) = match biome {
        Biome::HotDesert | Biome::SaltFlat => (0.8, 4.0, 12.0, 0.2, 2.6),
        Biome::Alpine | Biome::ColdDesert | Biome::Volcanic => (0.85, 5.0, 14.0, 0.9, 1.3),
        Biome::Swamp => (0.7, 2.0, 5.0, 0.3, 1.3),
        Biome::Ocean | Biome::Lake => (0.0, 0.0, 0.0, 0.0, 1.0),
        _ => (0.65, 3.0, 11.0, 0.5, 1.5),
    };
    let mut out = Vec::new();
    // A main feature most of the time, sometimes a smaller companion.
    for (k, p) in [(0, p_exist), (1, 0.35 * p_exist)] {
        if rng.next_f64() > p {
            continue;
        }
        let ang = rng.range(0.0, std::f64::consts::TAU);
        let ditch = rng.next_f64() < 0.25;
        let sign = if ditch { -1.0 } else { 1.0 };
        // Mostly small knolls and hollows, now and then a broad mound.
        let u = rng.next_f64();
        let scale = if k == 0 { 1.0 } else { 0.6 };
        let mut r = (2.0 + 6.0 * u * u) * scale;
        let mut el = elong * rng.range(0.7, 1.4) * if ditch { rng.range(1.2, 2.0) } else { 1.0 };
        let wobble = [(rng.range(0.0, 0.2), rng.range(0.0, std::f64::consts::TAU)), (rng.range(0.0, 0.14), rng.range(0.0, std::f64::consts::TAU)), (rng.range(0.0, 0.08), rng.range(0.0, std::f64::consts::TAU))];
        let swell = 1.0 / (1.0 - wobble.iter().map(|w| w.0).sum::<f64>());
        // Keep within the reach: long features get narrower rather than longer.
        let reach = r * el.max(1.0) * swell + 1.0;
        if reach > RELIEF_REACH {
            let f = RELIEF_REACH / reach;
            r *= f.sqrt();
            el = (el * f.sqrt()).max(1.0);
        }
        out.push(Relief {
            cx: gx as f64 * RELIEF_CELL as f64 + rng.range(3.0, RELIEF_CELL as f64 - 3.0),
            cy: gy as f64 * RELIEF_CELL as f64 + rng.range(3.0, RELIEF_CELL as f64 - 3.0),
            r,
            h: sign * rng.range(h_lo, h_hi) * (0.55 + 0.45 * (r / 6.0).min(1.5)),
            steep: (steep + rng.range(-0.25, 0.25)).clamp(0.0, 1.0),
            dir: (libm::cos(ang), libm::sin(ang)),
            elong: el,
            wobble,
        });
    }
    out
}

fn relief_value(f: &Relief, x: f64, y: f64) -> f64 {
    let (dx, dy) = (x - f.cx, y - f.cy);
    let along = (dx * f.dir.0 + dy * f.dir.1) / f.elong;
    let across = -dx * f.dir.1 + dy * f.dir.0;
    // Irregular outline: the radius swells and pinches with the angle.
    let th = libm::atan2(across, along);
    let edge = 1.0 + f.wobble.iter().enumerate().map(|(k, (a, ph))| a * libm::cos((k + 1) as f64 * th + ph)).sum::<f64>();
    let d = crate::core::sqrt(along * along + across * across) / (f.r * edge);
    if d >= 1.0 {
        return 0.0;
    }
    // Smooth dome blended toward a flat-topped, sharp-edged mesa by steepness.
    let dome = 1.0 - smoothstep(0.0, 1.0, d);
    let mesa = 1.0 - smoothstep(0.78, 0.92, d);
    f.h * (dome + (mesa - dome) * f.steep)
}

pub(super) fn dominant_biome(biome: &[Biome], water: &[f32], height: &[f32]) -> Biome {
    const N: usize = crate::t0::biome::ALL.len();
    let mut counts = [0usize; N];
    for k in 0..biome.len() {
        if water[k] <= height[k] {
            counts[biome[k] as usize] += 1;
        }
    }
    let best = (0..N).max_by_key(|&b| (counts[b], N - b)).unwrap_or(8);
    Biome::from_u8(best as u8)
}

/// Apply a relief feature to a `dim`² grid whose (0, 0) is global square (gx0, gy0).
pub(super) fn apply_relief(f: &Relief, height: &mut [f32], water: &[f32], gsx: i64, gsy: i64, dim: usize) {
    let reach = RELIEF_REACH + 1.0;
    let (x0, x1) = ((f.cx - reach) as i64 - gsx, (f.cx + reach) as i64 - gsx + 1);
    let (y0, y1) = ((f.cy - reach) as i64 - gsy, (f.cy + reach) as i64 - gsy + 1);
    for y in y0.max(0)..y1.min(dim as i64) {
        for x in x0.max(0)..x1.min(dim as i64) {
            let k = y as usize * dim + x as usize;
            if water[k] > height[k] - 1.0 {
                continue;
            }
            let v = relief_value(f, (gsx + x) as f64 + 0.5, (gsy + y) as f64 + 0.5);
            // Depressions never cut below the water table.
            height[k] = (height[k] as f64 + v).max(water[k] as f64 + 0.5) as f32;
        }
    }
}
