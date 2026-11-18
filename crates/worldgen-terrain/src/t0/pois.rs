//! Where ruins, wizard towers, caves, mines and lava tubes go: the points of interest T0 scatters.

use super::biome::Biome;
use super::hydro::NO_LAKE;
use super::settle::{Inputs, Poi, PoiKind, Settlement};
use crate::core::rng::{Pcg32, hash3};

/// Miles from each cell to the nearest of `own`'s settlements or a cell of the world's own roads
/// (`roads`: route usage per cell, empty with no roads): a two-pass 8-neighbour chamfer, so it
/// goes by the cells alone.
fn people_field(inp: &Inputs, own: &[&Settlement], roads: &[u8]) -> Vec<f32> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let mut d = vec![f32::MAX; w * h];
    for s in own {
        let (i, j) = (((s.x / cell).round().max(0.0) as usize).min(w - 1), ((s.y / cell).round().max(0.0) as usize).min(h - 1));
        d[j * w + i] = 0.0;
    }
    for (k, &u) in roads.iter().enumerate().take(w * h) {
        if u > 0 {
            d[k] = 0.0;
        }
    }
    let (a, b) = ((cell / 5280.0) as f32, (cell * std::f64::consts::SQRT_2 / 5280.0) as f32);
    let relax = |d: &mut [f32], x: usize, y: usize, nx: i64, ny: i64, c: f32| {
        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
            let v = d[ny as usize * w + nx as usize] + c;
            if v < d[y * w + x] {
                d[y * w + x] = v;
            }
        }
    };
    for y in 0..h {
        for x in 0..w {
            let (xi, yi) = (x as i64, y as i64);
            relax(&mut d, x, y, xi - 1, yi, a);
            relax(&mut d, x, y, xi, yi - 1, a);
            relax(&mut d, x, y, xi - 1, yi - 1, b);
            relax(&mut d, x, y, xi + 1, yi - 1, b);
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let (xi, yi) = (x as i64, y as i64);
            relax(&mut d, x, y, xi + 1, yi, a);
            relax(&mut d, x, y, xi, yi + 1, a);
            relax(&mut d, x, y, xi + 1, yi + 1, b);
            relax(&mut d, x, y, xi - 1, yi + 1, b);
        }
    }
    d
}

/// How willing a cave is to open this far (miles) from people: 0.2 at the 2-mi clear zone's edge
/// rising to 1.0 at 5 mi, 1.0 to 25 mi, falling fast to 0.05 at 35 mi and flat to 50, then 0.6
/// beyond (the wilds are their own draw, about a fifth of the caves).
fn cave_pull(mi: f64) -> f64 {
    const WILD: f64 = 0.6;
    if mi < 5.0 {
        0.2 + 0.8 * ((mi - 2.0) / 3.0).clamp(0.0, 1.0)
    } else if mi <= 25.0 {
        1.0
    } else if mi < 50.0 {
        if mi < 35.0 { 1.0 - 0.95 * (mi - 25.0) / 10.0 } else { 0.05 }
    } else {
        WILD
    }
}

/// Ruins and wizard towers, away from settlements; then cave mouths, mines and lava tubes
/// (`volcanoes`: T0 cell centre and cone radius ft).
pub fn place_pois(inp: &Inputs, settlements: &[Settlement], volcanoes: &[(f64, f64, f64)], roads: &[u8]) -> Vec<Poi> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let p = inp.world.params();
    // Each try draws from its own stream (where it looks, whether it takes the place, the seed),
    // so a try a settlement now blocks (or one more settlement's extra ruin) moves no other site.
    let stream = inp.world.stream("t0.poi");
    // How many there are, and which settlements mines are worked from, go by the world's own
    // settlements: one added beside them (`Pin::added`) only keeps sites off its ground.
    let own: Vec<&Settlement> = settlements.iter().filter(|s| !s.pin.is_some_and(|st| inp.pins.iter().any(|p| p.stroke == st as usize && p.added))).collect();
    let n_ruins = ((own.len() as f64) * 0.35 * p.poi_density).round() as usize;
    let n_towers = ((own.len() as f64) * 0.06 * p.poi_density).round().max(if p.poi_density > 0.0 { 1.0 } else { 0.0 }) as usize;
    let mut out: Vec<Poi> = Vec::new();
    let away = |x: f64, y: f64, out: &[Poi], min_s: f64, min_p: f64| {
        settlements.iter().all(|s| dist(s.x, s.y, x, y) >= min_s) && out.iter().all(|q| dist(q.x, q.y, x, y) >= min_p)
    };
    for (kind, count, min_s_mi) in [(PoiKind::Ruin, n_ruins, 6.0), (PoiKind::Tower, n_towers, 25.0)] {
        let mut tries = 0;
        let mut placed = 0;
        while placed < count && tries < count * 60 + 100 {
            tries += 1;
            let mut rng = Pcg32::new(hash3(stream, kind as i64, tries as i64, 0), 22);
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
            let seed = rng.next_u32() as u64 | (rng.next_u32() as u64) << 32;
            if kind == PoiKind::Ruin && !dwarven_ground(inp, k, seed) {
                continue;
            }
            out.push(Poi { kind, x, y, seed, stroke: None });
            placed += 1;
        }
    }
    // Underground entrances, from their own stream (the surface sites above stay put).
    let stream = inp.world.stream("t0.poi.under");
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
    let people = people_field(inp, &own, roads);
    let n_caves = ((own.len() as f64) * 0.2 * p.poi_density).round() as usize;
    let n_mines = ((own.len() as f64) * 0.1 * p.poi_density).round() as usize;
    for (kind, count) in [(PoiKind::Cave, n_caves), (PoiKind::Mine, n_mines)] {
        let (mut tries, mut placed) = (0, 0);
        while placed < count && tries < count * 600 + 600 {
            tries += 1;
            let mut rng = Pcg32::new(hash3(stream, kind as i64, tries as i64, 0), 23);
            let k = rng.below((w * h) as u32) as usize;
            if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE || inp.height[k] < sea + 20.0 {
                continue;
            }
            let b = Biome::from_u8((inp.biome[k] & 0xff) as u8);
            if matches!(b, Biome::SaltFlat | Biome::Swamp) || (b == Biome::Ice && kind == PoiKind::Mine) {
                continue;
            }
            let (x, y) = ((k % w) as f64 * cell, (k / w) as f64 * cell);
            let ok = match kind {
                PoiKind::Cave => relief(k) > 250.0 + 400.0 * rng.next_f64(),
                // Mines: hills, worked from a settlement a few miles off.
                _ => {
                    relief(k) > 150.0 && inp.height[k] > sea + 600.0 && own.iter().any(|s| (2.0 * 5280.0..15.0 * 5280.0).contains(&dist(s.x, s.y, x, y)))
                }
            };
            if !ok || !away(x, y, &out, 2.0 * 5280.0, 5.0 * 5280.0) {
                continue;
            }
            // Caves lean towards people (the try's own draw, so no other try moves).
            if kind == PoiKind::Cave && rng.next_f64() >= cave_pull(people[k] as f64) {
                continue;
            }
            out.push(Poi { kind, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32, stroke: None });
            placed += 1;
        }
    }
    // A lava tube on each volcano's flank, where it opens on dry land.
    for (v, &(vx, vy, r_ft)) in volcanoes.iter().enumerate() {
        let mut rng = Pcg32::new(hash3(stream, PoiKind::LavaTube as i64, v as i64, 0), 23);
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
            out.push(Poi { kind: PoiKind::LavaTube, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32, stroke: None });
            break;
        }
    }
    out
}

#[inline]
fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    crate::core::sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by))
}

/// v63: a ruin over a dwarven hall stands in hills or mountains; on flat ground 6 in 7 such tries
/// are passed over (the count of ruins stays: another try takes the place).
fn dwarven_ground(inp: &Inputs, k: usize, seed: u64) -> bool {
    let world = inp.world;
    // (Never a cave: the cold only matters to ice.)
    if crate::model::under::wild_theme(world, seed, crate::model::under::ruin_under(world, seed), true, crate::model::under::Cold::No) != "dwarven_hall" {
        return true;
    }
    let (w, h, sea) = (inp.w, inp.h, world.params().sea_level_ft);
    let (x, y) = (k % w, k / w);
    let mut relief: f64 = 0.0;
    for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
            relief = relief.max((inp.height[k] - inp.height[ny as usize * w + nx as usize].max(sea)).abs());
        }
    }
    relief > 150.0 || inp.height[k] > sea + 1500.0 || crate::core::rng::unit(crate::core::rng::hash2(seed, 0x6477, 0)) < 1.0 / 7.0
}
