//! QA: landforms per ruggedness. For ruggedness 0.5 / 1 / 2 on seeds 1-8 (or the seeds given),
//! counts on the T0 grid: plateaus (broad flat upland tops ringed by lower ground), mesas (small
//! ones), escarpments (long runs of steep cells below the high ranges), canyons (runs of river
//! cells with both sides much higher); slope stats (max, 99th percentile on land); river cells
//! that climb (must be 0); and tile seams (halo mismatches at a few levels over a canyon or the
//! steepest lowland cell, must be 0).
//! `cargo run --release -p worldgen --example landformcheck -- [seeds]`   (`RUG=0.5,1,2`)

use worldgen::core::tile::{HALO, TILE_N, TileKey};
use worldgen::lod::terrain_refine::padded_index;
use worldgen::pipeline::Executor;
use worldgen::t0::{T0, hydro};
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let seeds: Vec<u32> = if a.is_empty() { (1..=8).collect() } else { a.iter().filter_map(|s| s.parse().ok()).collect() };
    let rugs: Vec<f64> = std::env::var("RUG").ok().map(|v| v.split(',').filter_map(|s| s.parse().ok()).collect()).unwrap_or(vec![0.5, 1.0, 2.0]);
    for &rug in &rugs {
        let mut tot = [0usize; 6];
        for &seed in &seeds {
            let mut file = WorldFile { seed, ..Default::default() };
            file.params.ruggedness = rug;
            let world = World::new(file).unwrap();
            let (world_w, world_h) = (world.geom.map_w_ft, world.geom.map_h_ft);
            let t0 = T0::generate(&world);
            let s = stats(&world, &t0);
            let (seams, fine, town_slope) = seams(world, s.probe, &s.edge_towns);
            println!(
                "rug {rug:.1} seed {seed}: plateaus {} mesas {} escarpments {} canyons {} terraced {:.1}% | slope max {:.2} p99 {:.3} | climbs {} | towns by a riser {}/{} (steepest within their pads {town_slope:.2}) | seam mismatches {seams} | fine slope at a riser {fine:.2} (at {:.3},{:.3} of the map)",
                s.plateaus, s.mesas, s.scarps, s.canyons, 100.0 * s.terraced, s.max_slope, s.p99, s.climbs, s.edge_towns.len(), s.towns, s.probe.0 / world_w, s.probe.1 / world_h
            );
            for (t, v) in tot.iter_mut().zip([s.plateaus, s.mesas, s.scarps, s.canyons, s.climbs, seams]) {
                *t += v;
            }
        }
        println!("== rug {rug:.1}: plateaus {} mesas {} escarpments {} canyons {} climbs {} seams {}", tot[0], tot[1], tot[2], tot[3], tot[4], tot[5]);
    }
}

struct Stats {
    plateaus: usize,
    mesas: usize,
    scarps: usize,
    canyons: usize,
    max_slope: f64,
    p99: f64,
    climbs: usize,
    probe: (f64, f64),
    terraced: f64,
    edge_towns: Vec<(f64, f64, f64)>,
    towns: usize,
}

fn stats(world: &World, t0: &T0) -> Stats {
    let p = world.params();
    let sea = p.sea_level_ft;
    let (w, h) = (t0.height.w, t0.height.h);
    let ht = |i: i64, j: i64| t0.height.data[(j.clamp(0, h as i64 - 1) as usize) * w + i.clamp(0, w as i64 - 1) as usize] as f64;
    let hy = &t0.extra.as_ref().unwrap().hydro;
    let land: Vec<bool> = (0..w * h).map(|k| t0.height.data[k] as f64 > sea && hy.lake_of[k] == hydro::NO_LAKE).collect();
    let mut slope = vec![0.0f64; w * h];
    for j in 1..h - 1 {
        for i in 1..w - 1 {
            let (i, j) = (i as i64, j as i64);
            let (sx, sy) = ((ht(i + 1, j) - ht(i - 1, j)) / (2.0 * t0.cell_ft), (ht(i, j + 1) - ht(i, j - 1)) / (2.0 * t0.cell_ft));
            slope[j as usize * w + i as usize] = (sx * sx + sy * sy).sqrt();
        }
    }
    let mut ls: Vec<f64> = (0..w * h).filter(|&k| land[k]).map(|k| slope[k]).collect();
    ls.sort_by(f64::total_cmp);
    let max_slope = ls.last().copied().unwrap_or(0.0);
    let p99 = ls.get(ls.len() * 99 / 100).copied().unwrap_or(0.0);
    let comps = |pred: &dyn Fn(usize) -> bool| -> Vec<Vec<usize>> {
        let mut seen = vec![false; w * h];
        let mut out = Vec::new();
        for k0 in 0..w * h {
            if seen[k0] || !pred(k0) {
                continue;
            }
            let (mut comp, mut stack) = (Vec::new(), vec![k0]);
            seen[k0] = true;
            while let Some(k) = stack.pop() {
                comp.push(k);
                let (i, j) = ((k % w) as i64, (k / w) as i64);
                for (di, dj) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                    let (ni, nj) = (i + di, j + dj);
                    if ni < 0 || nj < 0 || ni >= w as i64 || nj >= h as i64 {
                        continue;
                    }
                    let nk = nj as usize * w + ni as usize;
                    if !seen[nk] && pred(nk) {
                        seen[nk] = true;
                        stack.push(nk);
                    }
                }
            }
            out.push(comp);
        }
        out
    };
    // Flat upland tops (600 ft up or more): tablelands (60 cells or more, 40% of the ring 200 ft
    // lower) and mesas (smaller, 70% of the ring lower). The ring just outside is 200 ft lower or more.
    let (mut plateaus, mut mesas) = (0, 0);
    for c in comps(&|k| land[k] && slope[k] < 0.015 && t0.height.data[k] as f64 - sea > 600.0) {
        if c.len() < 6 {
            continue;
        }
        let top = c.iter().map(|&k| t0.height.data[k] as f64).sum::<f64>() / c.len() as f64;
        let inside: std::collections::HashSet<usize> = c.iter().copied().collect();
        let (mut ring, mut lower) = (0, 0);
        for &k in &c {
            let (i, j) = ((k % w) as i64, (k / w) as i64);
            for (di, dj) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                let nk = ((j + dj).clamp(0, h as i64 - 1) as usize) * w + (i + di).clamp(0, w as i64 - 1) as usize;
                if !inside.contains(&nk) {
                    ring += 1;
                    lower += (ht(i + di, j + dj) < top - 200.0) as usize;
                }
            }
        }
        if c.len() >= 60 && lower * 10 >= ring * 4 {
            plateaus += 1;
        } else if c.len() < 60 && lower * 10 >= ring * 7 {
            mesas += 1;
        }
    }
    // Escarpments: long runs of steps (ground steeper than 1 in 20 with gentle ground 3 cells above
    // and below it, 250 ft apart).
    let sl = |i: i64, j: i64| slope[(j.clamp(0, h as i64 - 1) as usize) * w + i.clamp(0, w as i64 - 1) as usize];
    let step = |k: usize| {
        if !land[k] || slope[k] < 0.05 {
            return false;
        }
        let (i, j) = ((k % w) as i64, (k / w) as i64);
        let (gx, gy) = (ht(i + 1, j) - ht(i - 1, j), ht(i, j + 1) - ht(i, j - 1));
        let (di, dj) = if gx.abs() > gy.abs() { (gx.signum() as i64, 0) } else { (0, gy.signum() as i64) };
        let (up, down) = ((i + 3 * di, j + 3 * dj), (i - 3 * di, j - 3 * dj));
        sl(up.0, up.1) < 0.03 && sl(down.0, down.1) < 0.03 && ht(up.0, up.1) - ht(down.0, down.1) > 250.0
    };
    let mut scarps = 0;
    let mut probe = None;
    for c in comps(&step) {
        let (mut i0, mut i1, mut j0, mut j1) = (usize::MAX, 0, usize::MAX, 0);
        for &k in &c {
            (i0, i1, j0, j1) = (i0.min(k % w), i1.max(k % w), j0.min(k / w), j1.max(k / w));
        }
        if (i1 - i0).max(j1 - j0) >= 8 {
            scarps += 1;
            probe.get_or_insert(c[c.len() / 2]);
        }
    }
    // Canyons: river cells with both sides (2 cells across the flow) 400 ft above and gentle
    // ground on the rims (3 cells across), in runs of 6.
    let (mut canyons, mut climbs) = (0, 0);
    for r in &hy.rivers {
        let mut run = 0;
        for pair in r.cells.windows(2) {
            let (a, b) = (pair[0] as usize, pair[1] as usize);
            let (ha, hb) = (t0.height.data[a], t0.height.data[b]);
            let into_water = hb as f64 <= sea || hy.lake_of[b] != hydro::NO_LAKE;
            climbs += (!into_water && hb >= ha) as usize;
            let (ai, aj, bi, bj) = ((a % w) as i64, (a / w) as i64, (b % w) as i64, (b / w) as i64);
            let (pi, pj) = (-(bj - aj), bi - ai);
            let side = ht(ai + 2 * pi, aj + 2 * pj).min(ht(ai - 2 * pi, aj - 2 * pj));
            let rims = sl(ai + 3 * pi, aj + 3 * pj).max(sl(ai - 3 * pi, aj - 3 * pj));
            if land[a] && side - ha as f64 > 400.0 && rims < 0.06 {
                run += 1;
                if run == 6 {
                    canyons += 1;
                    probe = Some(a);
                }
            } else {
                run = 0;
            }
        }
    }
    // The steepest terraced cell (a riser), where refined levels step the ground into cliffs.
    let riser = (0..w * h).filter(|&k| land[k] && !t0.landform.data.is_empty()).max_by(|&a, &b| (t0.landform.data[a] as f64 * slope[a]).total_cmp(&(t0.landform.data[b] as f64 * slope[b])));
    let k = riser.or(probe).unwrap_or(w * h / 2);
    // Settlements whose cell or a neighbour is a stepped riser (weight > 0.3, slope > 1 in 25).
    let riser_at = |k: usize| !t0.landform.data.is_empty() && t0.landform.data[k] > 0.3 && slope[k] > 0.04;
    let edge_towns: Vec<(f64, f64, f64)> = t0
        .settlements
        .iter()
        .filter(|st| {
            let (i, j) = ((st.x / t0.cell_ft).round() as i64, (st.y / t0.cell_ft).round() as i64);
            (-1..=1).any(|dj| (-1..=1).any(|di| riser_at(((j + dj).clamp(0, h as i64 - 1) as usize) * w + (i + di).clamp(0, w as i64 - 1) as usize)))
        })
        .map(|st| (st.x, st.y, worldgen::t0::radii::pad_extent(worldgen::t0::radii::reach(st))))
        .collect();
    let terraced = if t0.landform.data.is_empty() { 0.0 } else { (0..w * h).filter(|&k| land[k] && t0.landform.data[k] > 0.5).count() as f64 / land.iter().filter(|&&l| l).count() as f64 };
    Stats { edge_towns, towns: t0.settlements.len(), terraced, plateaus, mesas, scarps, canyons, max_slope, p99, climbs, probe: ((k % w) as f64 * t0.cell_ft, (k / w) as f64 * t0.cell_ft) }
}

/// Halo mismatches between a tile and its right / lower neighbours at a few levels round `at`,
/// and the steepest ground in the tile there at the finest level stepped (~20 ft samples).
fn seams(world: World, at: (f64, f64), towns: &[(f64, f64, f64)]) -> (usize, f64, f64) {
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let (h, n) = (HALO as i64, TILE_N as i64);
    let mut bad = 0;
    let fine = (0..=g.max_level).rev().find(|&l| g.spacing_ft(l) >= worldgen::t0::landform::SHARPEN_MIN_FT).unwrap();
    let mut levels = vec![g.first_refine_level.saturating_sub(1), g.first_refine_level, g.first_refine_level + 3, fine];
    levels.dedup();
    for level in levels {
        let size = g.tile_size_ft(level);
        let (x, y) = ((at.0 / size) as u32, (at.1 / size) as u32);
        let a = ex.terrain(TileKey::surface(level, x, y)).padded.clone();
        let right = ex.terrain(TileKey::surface(level, x + 1, y)).padded.clone();
        let below = ex.terrain(TileKey::surface(level, x, y + 1)).padded.clone();
        for j in -h..=n + h {
            for d in -h..=h {
                bad += (a[padded_index(n + d, j)].to_bits() != right[padded_index(d, j)].to_bits()) as usize;
                bad += (a[padded_index(j, n + d)].to_bits() != below[padded_index(j, d)].to_bits()) as usize;
            }
        }
    }
    let s = g.spacing_ft(fine);
    let size = g.tile_size_ft(fine);
    let t = ex.terrain(TileKey::surface(fine, (at.0 / size) as u32, (at.1 / size) as u32)).padded.clone();
    let mut steep = 0f64;
    for j in 0..n {
        for i in 0..n {
            let (sx, sy) = ((t[padded_index(i + 1, j)] - t[padded_index(i, j)]) as f64 / s, (t[padded_index(i, j + 1)] - t[padded_index(i, j)]) as f64 / s);
            steep = steep.max((sx * sx + sy * sy).sqrt());
        }
    }
    // Round up to six towns by a riser: the steepest ground within their pads at that level.
    let mut town_slope = 0f64;
    for &(x, y, e) in towns.iter().take(6) {
        let (tx, ty) = ((x / size) as u32, (y / size) as u32);
        let t = ex.terrain(TileKey::surface(fine, tx, ty)).padded.clone();
        let (ox, oy) = ((tx as i64 * n) as f64 * s, (ty as i64 * n) as f64 * s);
        for j in 0..n {
            for i in 0..n {
                let (px, py) = (ox + i as f64 * s, oy + j as f64 * s);
                if (px - x).hypot(py - y) < e {
                    let (sx, sy) = ((t[padded_index(i + 1, j)] - t[padded_index(i, j)]) as f64 / s, (t[padded_index(i, j + 1)] - t[padded_index(i, j)]) as f64 / s);
                    town_slope = town_slope.max((sx * sx + sy * sy).sqrt());
                }
            }
        }
    }
    (bad, steep, town_slope)
}
