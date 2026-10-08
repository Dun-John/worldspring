//! Settlements and points of interest at continent scale.
//!
//! Sites are scored for habitability (fertile biome, flat ground, fresh water, sheltered
//! harbours, river confluences; penalized at altitude) and placed tier by tier (metropolis →
//! city → town → village) with spacing, like central places. Each gets a character (port,
//! river town, mining town, fortress, market, farming/fishing/lumber/herding village, oasis)
//! from its surroundings. Ruins and wizard towers are scattered in the wilds.

use serde::Serialize;

use super::biome::Biome;
use super::flood::neighbors;
use super::hydro::{Hydro, Mouth, NO_LAKE};
use crate::World;
use crate::core::noise::{fbm, smoothstep};
use crate::core::rng::Pcg32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Village = 0,
    Town,
    City,
    Metropolis,
}

impl Tier {
    pub fn from_u8(v: u8) -> Tier {
        [Tier::Village, Tier::Town, Tier::City, Tier::Metropolis][(v as usize).min(3)]
    }
    pub fn name(self) -> &'static str {
        ["village", "town", "city", "metropolis"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettleKind {
    Port = 0,
    River,
    Mining,
    Fortress,
    Market,
    Farming,
    Fishing,
    Lumber,
    Herding,
    Oasis,
}

impl SettleKind {
    pub fn from_u8(v: u8) -> SettleKind {
        use SettleKind::*;
        [Port, River, Mining, Fortress, Market, Farming, Fishing, Lumber, Herding, Oasis][(v as usize).min(9)]
    }
    pub fn name(self) -> &'static str {
        ["port", "river town", "mining town", "fortress", "market town", "farming", "fishing", "lumber", "herding", "oasis"][self as usize]
    }
}

#[derive(Clone, Debug)]
pub struct Settlement {
    pub tier: Tier,
    pub kind: SettleKind,
    /// T0 cell index and world position (ft).
    pub cell: usize,
    pub x: f64,
    pub y: f64,
    pub population: u32,
    pub coastal: bool,
    pub river: bool,
    pub capital: bool,
    /// Per-settlement seed for its layout.
    pub seed: u64,
    /// Naming culture (set with the gazetteer).
    pub culture: u8,
    /// The sketch stroke that pinned it, if any.
    pub pin: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PoiKind {
    Ruin = 0,
    Tower,
    Camp,
    Waystation,
    /// Underground entrances (M6): a cave mouth on rugged ground or a sea cliff, a mine adit
    /// in the hills near a settlement, a lava-tube skylight on a volcano's flank.
    Cave,
    Mine,
    LavaTube,
    /// Created only: a bare way underground (a stairwell, cave mouth or grate).
    Entrance,
    /// Created only: a building drawn by hand (`world::Created::poly`).
    Building,
}

impl PoiKind {
    pub fn from_u8(v: u8) -> PoiKind {
        const ALL: [PoiKind; 9] = [PoiKind::Ruin, PoiKind::Tower, PoiKind::Camp, PoiKind::Waystation, PoiKind::Cave, PoiKind::Mine, PoiKind::LavaTube, PoiKind::Entrance, PoiKind::Building];
        ALL[(v as usize).min(ALL.len() - 1)]
    }
}

#[derive(Clone, Debug)]
pub struct Poi {
    pub kind: PoiKind,
    pub x: f64,
    pub y: f64,
    pub seed: u64,
}

pub struct Inputs<'a> {
    pub world: &'a World,
    pub w: usize,
    pub h: usize,
    pub cell_ft: f64,
    pub height: &'a [f64],
    pub land: &'a [bool],
    pub biome: &'a [u32],
    pub hydro: &'a Hydro,
    /// Settlements the sketch places (before any of their tier are chosen).
    pub pins: &'a [super::sketch::Pin],
}

fn fertility(b: Biome) -> f64 {
    match b {
        Biome::Grassland => 1.0,
        Biome::TemperateForest => 0.8,
        Biome::Savanna => 0.7,
        Biome::Steppe => 0.6,
        Biome::TemperateRainforest => 0.6,
        Biome::Jungle => 0.45,
        Biome::Taiga => 0.45,
        Biome::Swamp => 0.2,
        Biome::ColdDesert => 0.2,
        Biome::HotDesert => 0.12,
        Biome::Tundra => 0.15,
        Biome::Alpine => 0.1,
        _ => 0.0,
    }
}

/// Place settlements of the given tiers, keeping `existing` ones. `roads` (from
/// `roads::preview`) favours sites on roads and most of all at junctions, so later
/// tiers grow at the crossroads the earlier tiers' roads made.
pub fn place(inp: &Inputs, existing: Vec<Settlement>, tiers: &[Tier], roads: Option<&super::roads::Preview>) -> Vec<Settlement> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let world = inp.world;
    let p = world.params();
    let sea = p.sea_level_ft;
    let n = w * h;
    let big_river = 900_000.0;
    let river_q = 90_000.0 / p.river_density;
    let ore_seed = world.stream("t0.settle.ore");

    // Per-cell context.
    let biome_of = |k: usize| Biome::from_u8((inp.biome[k] & 0xff) as u8);
    let slope_of = |k: usize| super::biome::local_slope(w, h, inp.height, k % w, k / w, cell);
    let mut score = vec![0.0f64; n];
    let mut coastal = vec![false; n];
    let mut on_river = vec![false; n];
    // Where rivers meet the sea or a lake, and where they join: the best town sites.
    let mut mouth = vec![0.0f64; n];
    // Only sea and lake mouths make a harbour (a confluence is a river town).
    let mut harbour = vec![false; n];
    for r in &inp.hydro.rivers {
        let (Some(&last), Some(&q)) = (r.cells.last(), r.q.last()) else { continue };
        let strength = match r.mouth {
            Mouth::Ocean | Mouth::Lake => 1.0,
            Mouth::Confluence => 0.6,
            _ => 0.0,
        } * smoothstep(river_q, big_river * 2.0, q as f64);
        if strength <= 0.0 {
            continue;
        }
        let (mx, my) = ((last as usize % w) as i64, (last as usize / w) as i64);
        for dy in -2i64..=2 {
            for dx in -2i64..=2 {
                let (x, y) = (mx + dx, my + dy);
                if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                    let k = y as usize * w + x as usize;
                    let fall = 1.0 - (crate::core::sqrt((dx * dx + dy * dy) as f64) / 3.0).min(1.0);
                    mouth[k] = mouth[k].max(strength * fall);
                    if matches!(r.mouth, Mouth::Ocean | Mouth::Lake) && strength * fall > 0.5 {
                        harbour[k] = true;
                    }
                }
            }
        }
    }
    for k in 0..n {
        if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE {
            continue;
        }
        let (mut ocean_n, mut lake_n) = (0, 0);
        for (nb, _) in neighbors(w, h, k) {
            if !inp.land[nb] {
                ocean_n += 1;
            } else if inp.hydro.lake_of[nb] != NO_LAKE {
                lake_n += 1;
            }
        }
        // A coast is a coast on barren ground too (a pinned port there is still a port); the
        // ground only decides how good a site it is.
        coastal[k] = ocean_n > 0;
        let b = biome_of(k);
        let fert = fertility(b);
        if fert <= 0.0 {
            continue;
        }
        let above = inp.height[k] - sea;
        let flat = 1.0 - smoothstep(0.02, 0.15, slope_of(k));
        let mut water = 0.0f64;
        let q = inp.hydro.discharge[k] as f64;
        if ocean_n > 0 {
            // Sheltered coasts (a few sea neighbours, not a headland) make better harbours.
            water += if (2..=4).contains(&ocean_n) { 1.0 } else { 0.6 };
        }
        if q >= river_q {
            on_river[k] = true;
            water += 0.5 + 0.5 * smoothstep(river_q, big_river * 4.0, q);
        }
        if lake_n > 0 {
            water += 0.6;
        }
        water += 1.4 * mouth[k];
        let dry_fix = if matches!(b, Biome::HotDesert | Biome::ColdDesert) && water < 0.5 { 0.2 } else { 1.0 };
        let altitude = 1.0 - 0.8 * smoothstep(3_000.0, 8_000.0, above);
        score[k] = fert * (0.35 + 0.65 * flat) * (0.35 + water) * altitude * dry_fix;
    }

    // How many of each tier, from land area (square miles) and density. At density 0 none are
    // chosen, but the sketch's pins still stand: what is placed by hand wins over the dial.
    let cell_mi = cell / 5280.0;
    let land_area = (0..n).filter(|&k| score[k] > 0.0).count() as f64 * cell_mi * cell_mi;
    let none = p.settlement_density <= 0.0;
    let villages = if none { 0 } else { (land_area / 3_500.0 * p.settlement_density).round().max(3.0) as usize };
    let quota = [
        (Tier::Metropolis, if none { 0 } else { (villages / 90).max(1) }, 180.0),
        (Tier::City, if none { 0 } else { (villages / 18).max(1) }, 90.0),
        (Tier::Town, if none { 0 } else { (villages / 5).max(2) }, 40.0),
        (Tier::Village, villages, 14.0),
    ];

    // Roads: a site on a road is better, a junction much better (within a cell of it).
    let road_bonus = |k: usize, tier: Tier| -> f64 {
        let Some(rp) = roads else { return 1.0 };
        let (x, y) = ((k % w) as i64, (k / w) as i64);
        let (mut on, mut at) = (false, false);
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                on |= rp.usage[j] > 0;
                at |= rp.junction[j];
            }
        }
        let (on_b, at_b) = if tier >= Tier::Town { (3.0, 5.0) } else { (2.0, 3.5) };
        if at { at_b } else if on { on_b } else { 1.0 }
    };

    let mut out = existing;
    for (tier, count, spacing_mi) in quota {
        if !tiers.contains(&tier) {
            continue;
        }
        let mut rng = Pcg32::new(world.stream("t0.settle"), 21 + tier as u64);
        let mut cands: Vec<(f64, usize)> = (0..n).filter(|&k| score[k] > 0.05).map(|k| (score[k] * road_bonus(k, tier) * (0.55 + 0.45 * rng.next_f64()), k)).collect();
        cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let spacing = spacing_mi * 5280.0 / cell;
        // A settlement of this tier at cell `k`, standing at (x, y) ft.
        let make = |k: usize, rng: &mut Pcg32, x: f64, y: f64, capital: bool, pin: Option<u32>| {
            let (cx, cy) = ((k % w) as f64, (k / w) as f64);
            let (lo, hi) = match tier {
                Tier::Metropolis => (25_000.0, 80_000.0),
                Tier::City => (5_000.0, 25_000.0),
                Tier::Town => (500.0, 5_000.0),
                Tier::Village => (50.0, 500.0),
            };
            let population = (lo * libm::pow(hi / lo, rng.next_f64())) as u32;
            let b = biome_of(k);
            let above = inp.height[k] - sea;
            let ore = fbm(ore_seed, cx / 12.0, cy / 12.0, 3, 2.0, 0.5);
            // A pin stands where it was drawn, not at its cell's centre: one drawn on the coast is
            // coastal when the sea is within reach of that point (the shore it then moves to).
            let pinned_coast = pin.is_some() && {
                let r = 1.6f64;
                let (pi, pj) = (crate::core::round(x / cell) as i64, crate::core::round(y / cell) as i64);
                (-2..=2i64).any(|dj| {
                    (-2..=2i64).any(|di| {
                        let (i, j) = (pi + di, pj + dj);
                        let (dx, dy) = (i as f64 - x / cell, j as f64 - y / cell);
                        i >= 0 && j >= 0 && i < w as i64 && j < h as i64 && dx * dx + dy * dy <= r * r && !inp.land[j as usize * w + i as usize]
                    })
                })
            };
            let is_coast = coastal[k] || harbour[k] || pinned_coast;
            let kind = if is_coast {
                if tier >= Tier::Town { SettleKind::Port } else { SettleKind::Fishing }
            } else if above > 2_500.0 && ore > 0.1 {
                SettleKind::Mining
            } else if above > 2_500.0 && tier >= Tier::Town {
                SettleKind::Fortress
            } else if on_river[k] && tier >= Tier::Town {
                SettleKind::River
            } else if tier >= Tier::City {
                SettleKind::Market
            } else if matches!(b, Biome::HotDesert | Biome::ColdDesert) {
                SettleKind::Oasis
            } else if matches!(b, Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle) {
                SettleKind::Lumber
            } else if matches!(b, Biome::Steppe | Biome::Savanna | Biome::Tundra) {
                SettleKind::Herding
            } else if on_river[k] {
                SettleKind::Fishing
            } else {
                SettleKind::Farming
            };
            Settlement {
                tier,
                kind,
                cell: k,
                x,
                y,
                population,
                coastal: is_coast,
                river: on_river[k],
                capital,
                seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32,
                culture: 0,
                pin,
            }
        };
        // Pinned ones first (they count toward the tier's quota), with their own seeds.
        let mut placed = 0;
        for pin in inp.pins.iter().filter(|p| p.tier == tier) {
            let mut prng = Pcg32::new(crate::core::rng::hash2(world.stream("t0.pin"), pin.stroke as i64, 0), 7);
            let capital = tier == Tier::Metropolis && !out.iter().any(|s| s.capital);
            out.push(make(pin.cell, &mut prng, pin.x, pin.y, capital, Some(pin.stroke as u32)));
            placed += 1;
        }
        if count == 0 {
            continue;
        }
        for &(_, k) in &cands {
            if placed >= count {
                break;
            }
            let (cx, cy) = ((k % w) as f64, (k / w) as f64);
            let ok = out.iter().all(|s| {
                let (sx, sy) = ((s.cell % w) as f64, (s.cell / w) as f64);
                let d = crate::core::sqrt((sx - cx) * (sx - cx) + (sy - cy) * (sy - cy));
                let need = if s.tier >= tier { spacing } else { 10.0 * 5280.0 / cell };
                d >= need.max(10.0 * 5280.0 / cell)
            });
            if !ok {
                continue;
            }
            let capital = tier == Tier::Metropolis && !out.iter().any(|s| s.capital);
            out.push(make(k, &mut rng, cx * cell, cy * cell, capital, None));
            placed += 1;
        }
    }
    out
}

/// Move each waterside settlement off its cell centre onto its water: ports and fishing
/// villages stand just inland of the real shoreline (by a third of their built-up radius, so
/// the town reaches the water), river towns just beside the river. `wet(x, y)` is standing
/// water (sea or lake); `river(x, y)` is the nearest river centreline point within reach:
/// (point, half width).
pub fn snap_to_water(settlements: &mut [Settlement], cell: f64, wet: &dyn Fn(f64, f64) -> bool, river: &dyn Fn(f64, f64) -> Option<([f64; 2], f64)>) {
    for s in settlements.iter_mut() {
        let r = crate::town::urban_radius(s.tier, s.population);
        let inland = 0.33 * r;
        // Nearest shore point by casting rays outward.
        let mut shore: Option<(f64, [f64; 2], [f64; 2])> = None;
        for a in 0..48 {
            let ang = std::f64::consts::TAU * a as f64 / 48.0;
            let u = [libm::cos(ang), libm::sin(ang)];
            let mut t = 25.0;
            while t < 2.2 * cell {
                let q = [s.x + u[0] * t, s.y + u[1] * t];
                if wet(q[0], q[1]) {
                    if shore.is_none_or(|b| t < b.0) {
                        shore = Some((t, q, u));
                    }
                    break;
                }
                t += 50.0;
            }
        }
        if s.coastal
            && let Some((_, q, u)) = shore
        {
            // Back inland from the shore until on dry ground.
            let mut back = inland;
            let mut c = [q[0] - u[0] * back, q[1] - u[1] * back];
            while wet(c[0], c[1]) && back < 3.0 * inland + 400.0 {
                back += 50.0;
                c = [q[0] - u[0] * back, q[1] - u[1] * back];
            }
            (s.x, s.y) = (c[0], c[1]);
            continue;
        }
        if s.river
            && let Some((p, hw)) = river(s.x, s.y)
        {
            let d = [s.x - p[0], s.y - p[1]];
            let dl = crate::core::sqrt(d[0] * d[0] + d[1] * d[1]).max(1e-6);
            let off = hw + 0.25 * r;
            (s.x, s.y) = (p[0] + d[0] / dl * off, p[1] + d[1] / dl * off);
        }
    }
}

/// Ruins and wizard towers, away from settlements; then cave mouths, mines and lava tubes
/// (`volcanoes`: T0 cell centre and cone radius ft).
pub fn place_pois(inp: &Inputs, settlements: &[Settlement], volcanoes: &[(f64, f64, f64)]) -> Vec<Poi> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let p = inp.world.params();
    let mut rng = Pcg32::new(inp.world.stream("t0.poi"), 22);
    let n_ruins = ((settlements.len() as f64) * 0.35 * p.poi_density).round() as usize;
    let n_towers = ((settlements.len() as f64) * 0.06 * p.poi_density).round().max(if p.poi_density > 0.0 { 1.0 } else { 0.0 }) as usize;
    let mut out: Vec<Poi> = Vec::new();
    let away = |x: f64, y: f64, out: &[Poi], min_s: f64, min_p: f64| {
        settlements.iter().all(|s| dist(s.x, s.y, x, y) >= min_s) && out.iter().all(|q| dist(q.x, q.y, x, y) >= min_p)
    };
    for (kind, count, min_s_mi) in [(PoiKind::Ruin, n_ruins, 6.0), (PoiKind::Tower, n_towers, 25.0)] {
        let mut tries = 0;
        let mut placed = 0;
        while placed < count && tries < count * 60 + 100 {
            tries += 1;
            let k = rng.below((w * h) as u32) as usize;
            if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE {
                continue;
            }
            let b = Biome::from_u8((inp.biome[k] & 0xff) as u8);
            if matches!(b, Biome::Ice | Biome::SaltFlat) {
                continue;
            }
            // Ruins favour old, overgrown or buried places.
            if kind == PoiKind::Ruin && !matches!(b, Biome::Jungle | Biome::HotDesert | Biome::TemperateForest | Biome::Swamp) && rng.next_f64() < 0.5 {
                continue;
            }
            let (x, y) = ((k % w) as f64 * cell, (k / w) as f64 * cell);
            if !away(x, y, &out, min_s_mi * 5280.0, 8.0 * 5280.0) {
                continue;
            }
            out.push(Poi { kind, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32 });
            placed += 1;
        }
    }
    // Underground entrances, from their own stream (the surface sites above stay put).
    let mut rng = Pcg32::new(inp.world.stream("t0.poi.under"), 23);
    let sea = p.sea_level_ft;
    // Steepest drop to a neighbour (ft per cell): caves open in rugged ground and sea cliffs.
    let relief = |k: usize| {
        let (x, y) = (k % w, k / w);
        let mut r: f64 = 0.0;
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                r = r.max((inp.height[k] - inp.height[ny as usize * w + nx as usize].max(sea)).abs());
            }
        }
        r
    };
    let n_caves = ((settlements.len() as f64) * 0.2 * p.poi_density).round() as usize;
    let n_mines = ((settlements.len() as f64) * 0.1 * p.poi_density).round() as usize;
    for (kind, count) in [(PoiKind::Cave, n_caves), (PoiKind::Mine, n_mines)] {
        let (mut tries, mut placed) = (0, 0);
        while placed < count && tries < count * 200 + 200 {
            tries += 1;
            let k = rng.below((w * h) as u32) as usize;
            if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE || inp.height[k] < sea + 20.0 {
                continue;
            }
            let b = Biome::from_u8((inp.biome[k] & 0xff) as u8);
            if matches!(b, Biome::Ice | Biome::SaltFlat | Biome::Swamp) {
                continue;
            }
            let (x, y) = ((k % w) as f64 * cell, (k / w) as f64 * cell);
            let ok = match kind {
                PoiKind::Cave => relief(k) > 250.0 + 400.0 * rng.next_f64(),
                // Mines: hills, worked from a settlement a few miles off.
                _ => {
                    relief(k) > 150.0 && inp.height[k] > sea + 600.0 && settlements.iter().any(|s| (2.0 * 5280.0..15.0 * 5280.0).contains(&dist(s.x, s.y, x, y)))
                }
            };
            if !ok || !away(x, y, &out, 2.0 * 5280.0, 5.0 * 5280.0) {
                continue;
            }
            out.push(Poi { kind, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32 });
            placed += 1;
        }
    }
    // A lava tube on each volcano's flank, where it opens on dry land.
    for &(vx, vy, r_ft) in volcanoes {
        for _ in 0..12 {
            let a = rng.range(0.0, std::f64::consts::TAU);
            let d = r_ft * rng.range(0.7, 1.4);
            let (x, y) = (vx * cell + d * libm::cos(a), vy * cell + d * libm::sin(a));
            let (i, j) = ((x / cell).round(), (y / cell).round());
            if i < 0.0 || j < 0.0 || i >= w as f64 || j >= h as f64 {
                continue;
            }
            let k = j as usize * w + i as usize;
            if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE || inp.height[k] < sea + 20.0 || !away(x, y, &out, 1.0 * 5280.0, 2.0 * 5280.0) {
                continue;
            }
            out.push(Poi { kind: PoiKind::LavaTube, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32 });
            break;
        }
    }
    out
}

#[inline]
fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    crate::core::sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by))
}
