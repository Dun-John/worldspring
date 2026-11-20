//! QA: how network roads lie on battlemap ground where they climb. Picks the steepest stretches
//! of each world's roads (profile grade near the class limit, spread out), generates the
//! battlemaps there and walks each road's line over the squares: the grade along the road over
//! 20-ft windows (against the class limit), the bed's tilt across the road, and how far the
//! ground strays from the road's profile. Prints one line per seed and the worst places.
//! `HAND=1`: instead, roads drawn by hand straight down the steepest hillsides found (a track and a
//! footpath side by side, 600 ft): the bed's grade over 20-ft windows on dry road squares (the
//! track's against its kind's limit), the steepest step between neighbouring band squares, and
//! the footpath's squares of steps.
//! `cargo run --release -p worldgen --example gradecheck -- [seeds, default 1..8]`   (`SITES=n` per seed, default 10)
mod common;

use worldgen::battlemap::SQ;
use worldgen::core::tile::TileKey;
use worldgen::pipeline::Executor;

const WIN: f64 = 20.0;

fn main() {
    let a = common::args();
    let seeds: Vec<String> = if a.get(0).is_some() { (0..16).filter_map(|i| a.get(i).map(String::from)).collect() } else { (1..=8).map(|s| s.to_string()).collect() };
    let sites_n: usize = std::env::var("SITES").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    if std::env::var("HAND").is_ok_and(|v| v == "1") {
        return hand(&seeds, sites_n);
    }
    let mut all = (0usize, 0usize, 0usize, 0f64, 0f64);
    for seed in seeds {
        let mut ex = Executor::new(common::world(&seed));
        let g = ex.world.geom.clone();
        let size = g.tile_size_ft(g.max_level);
        // Steep stretches: chords at a grade near the class limit, spread over the map.
        let mut cand: Vec<(f64, [f64; 2])> = Vec::new();
        for r in &ex.t0.roads.roads {
            for i in 1..r.pts.len() {
                let ds = (r.s[i] - r.s[i - 1]).max(1.0);
                let ratio = (r.z[i] - r.z[i - 1]).abs() as f64 / ds / r.class.max_grade();
                if ratio >= 0.75 {
                    cand.push((ratio, [0.5 * (r.pts[i][0] + r.pts[i - 1][0]), 0.5 * (r.pts[i][1] + r.pts[i - 1][1])]));
                }
            }
        }
        cand.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let mut picked: Vec<[f64; 2]> = Vec::new();
        for (_, p) in cand {
            if picked.len() >= sites_n {
                break;
            }
            if picked.iter().all(|q| (q[0] - p[0]).hypot(q[1] - p[1]) > 6_000.0) {
                picked.push(p);
            }
        }
        let (mut wins, mut over, mut over50, mut worst, mut tilt) = (0usize, 0usize, 0usize, 0f64, 0f64);
        let mut worst_at = [0.0, 0.0];
        let mut stray = 0f64;
        let (mut prof, mut worst_prof) = (0f64, 0f64);
        for p in &picked {
            let key = TileKey::surface(g.max_level, (p[0] / size) as u32, (p[1] / size) as u32);
            let chunk = ex.battlemap(key);
            let tile = ex.terrain(key);
            let (ox, oy) = g.tile_origin_ft(&key);
            // Height at a point: bilinear over square centres (None off the chunk's inner squares
            // or off a road square).
            let at = |x: f64, y: f64| -> Option<f64> {
                let (fx, fy) = ((x - ox) / 5.0 - 0.5, (y - oy) / 5.0 - 0.5);
                if fx < 1.0 || fy < 1.0 || fx >= (SQ - 2) as f64 || fy >= (SQ - 2) as f64 {
                    return None;
                }
                let (i, j) = (fx.floor() as usize, fy.floor() as usize);
                let (tx, ty) = (fx - i as f64, fy - j as f64);
                let k = j * SQ + i;
                if !chunk.surface[((fy + 0.5) as usize) * SQ + (fx + 0.5) as usize].is_road() || chunk.building[k] != 0 {
                    return None;
                }
                let h = |k: usize| chunk.height[k] as f64;
                Some((h(k) * (1.0 - tx) + h(k + 1) * tx) * (1.0 - ty) + (h(k + SQ) * (1.0 - tx) + h(k + SQ + 1) * tx) * ty)
            };
            for piece in tile.roads.iter() {
                let gmax = piece.class.max_grade();
                let half = 0.5 * piece.class.width_ft();
                // Resample the line every 2.5 ft.
                let mut line: Vec<([f64; 2], f64, [f64; 2])> = Vec::new();
                for seg in piece.pts.windows(2) {
                    let (a, b) = (seg[0], seg[1]);
                    let len = (b.p[0] - a.p[0]).hypot(b.p[1] - a.p[1]);
                    if len < 1e-6 {
                        continue;
                    }
                    let d = [(b.p[0] - a.p[0]) / len, (b.p[1] - a.p[1]) / len];
                    let n = (len / 2.5).ceil() as usize;
                    for s in 0..n {
                        let t = s as f64 / n as f64;
                        line.push(([a.p[0] + (b.p[0] - a.p[0]) * t, a.p[1] + (b.p[1] - a.p[1]) * t], a.z + (b.z - a.z) * t, d));
                    }
                }
                let step = (WIN / 2.5) as usize;
                for i in 0..line.len().saturating_sub(step) {
                    let (p0, z0, d) = line[i];
                    let (p1, z1, _) = line[i + step];
                    let (Some(h0), Some(h1)) = (at(p0[0], p0[1]), at(p1[0], p1[1])) else { continue };
                    let run = (p1[0] - p0[0]).hypot(p1[1] - p0[1]).max(1.0);
                    let r = (h1 - h0).abs() / run / gmax;
                    wins += 1;
                    if r > 1.25 {
                        over += 1;
                    }
                    if r > 1.5 {
                        over50 += 1;
                    }
                    if r > worst {
                        worst = r;
                        worst_at = p0;
                        worst_prof = (z1 - z0).abs() / run / gmax;
                    }
                    prof = prof.max((z1 - z0).abs() / run / gmax);
                    stray = stray.max((h0 - z0).abs());
                    let w = (half - 2.5).max(1.5);
                    let nrm = [-d[1], d[0]];
                    if let (Some(l), Some(rr)) = (at(p0[0] + nrm[0] * w, p0[1] + nrm[1] * w), at(p0[0] - nrm[0] * w, p0[1] - nrm[1] * w)) {
                        tilt = tilt.max((l - rr).abs() / (2.0 * w));
                    }
                }
            }
        }
        println!(
            "seed {seed}: {} sites, {wins} windows, > 1.25x max {over} ({:.1}%), > 1.5x {over50}, worst {worst:.2}x at ({:.0}, {:.0}) (profile there {worst_prof:.2}x, steepest profile {prof:.2}x), max tilt across {tilt:.3}, max stray from profile {stray:.1} ft",
            picked.len(),
            100.0 * over as f64 / wins.max(1) as f64,
            worst_at[0],
            worst_at[1]
        );
        all.0 += wins;
        all.1 += over;
        all.2 += over50;
        all.3 = all.3.max(worst);
        all.4 = all.4.max(tilt);
    }
    println!("all: {} windows, > 1.25x max {} ({:.2}%), > 1.5x {}, worst {:.2}x, max tilt {:.3}", all.0, all.1, 100.0 * all.1 as f64 / all.0.max(1) as f64, all.2, all.3, all.4);
}

/// Hand roads down steep hillsides (see the header).
fn hand(seeds: &[String], sites_n: usize) {
    use worldgen::model::paths::{Path, PathKind};
    use worldgen::battlemap::Surface;
    let (mut tot_stairs, mut tot_sq, mut worst_track, mut worst_step) = (0usize, 0usize, 0f64, 0f64);
    for seed in seeds {
        let base = common::world(seed);
        let mut file = common::world_file(seed);
        let ex = Executor::new(base);
        let g = ex.world.geom.clone();
        let lattice = g.spacing_ft(g.first_refine_level.saturating_sub(1));
        let gr = |p: [f64; 2]| ex.t0.ground_at(p[0], p[1], lattice);
        let wet = |p: [f64; 2]| ex.t0.sample_water(p[0], p[1]) > gr(p) as f32 - 1.0;
        // Steep dry hillsides: a coarse scan, the steepest spread out.
        let mut cand: Vec<(f64, [f64; 2], [f64; 2])> = Vec::new();
        let step = 9_000.0;
        let (mut y, mut picked) = (step, Vec::<[f64; 2]>::new());
        while y < g.map_h_ft - step {
            let mut x = step;
            while x < g.map_w_ft - step {
                let (gx, gy) = ((gr([x + 50.0, y]) - gr([x - 50.0, y])) / 100.0, (gr([x, y + 50.0]) - gr([x, y - 50.0])) / 100.0);
                let s = gx.hypot(gy);
                if s > 0.2 && s < 0.6 && !wet([x, y]) && ex.t0.settlements.iter().all(|t| (t.x - x).hypot(t.y - y) > 6_000.0) {
                    cand.push((s, [x, y], [gx / s, gy / s]));
                }
                x += step;
            }
            y += step;
        }
        cand.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let mut sites = Vec::new();
        for &(s, p, d) in &cand {
            if sites.len() >= sites_n || picked.iter().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 20_000.0) {
                continue;
            }
            let n = [-d[1], d[0]];
            let line = |off: f64| -> Vec<[f64; 2]> { (0..4).map(|k| { let t = -300.0 + 200.0 * k as f64; [(p[0] + d[0] * t + n[0] * off).round(), (p[1] + d[1] * t + n[1] * off).round()] }).collect() };
            if line(0.0).iter().chain(line(60.0).iter()).any(|&q| wet(q)) {
                continue;
            }
            picked.push(p);
            let i = sites.len();
            file.edits.paths.insert(format!("gt{i}"), Path { pts: line(0.0), kind: PathKind::Track, width: None, name: None });
            file.edits.paths.insert(format!("gf{i}"), Path { pts: line(60.0), kind: PathKind::Footpath, width: None, name: None });
            sites.push((s, p, d, n));
        }
        let world = worldgen::World::new(file).unwrap();
        let mut ex = Executor::new(world);
        let size = g.tile_size_ft(g.max_level);
        let (mut stairs, mut sq, mut wt, mut ws) = (0usize, 0usize, 0f64, 0f64);
        let mut ground_slope = 0f64;
        let (mut bump_on, mut bump_off) = ((0f64, 0usize), (0f64, 0usize));
        for &(s, p, d, n) in &sites {
            ground_slope = ground_slope.max(s);
            // The chunks under the middle 400 ft of both lines.
            let mut keys = std::collections::BTreeSet::new();
            for k in 0..=40 {
                let t = -200.0 + 10.0 * k as f64;
                for off in [0.0, 60.0] {
                    let q = [p[0] + d[0] * t + n[0] * off, p[1] + d[1] * t + n[1] * off];
                    keys.insert(((q[0] / size) as u32, (q[1] / size) as u32));
                }
            }
            for (kx, ky) in keys {
                let key = TileKey::surface(g.max_level, kx, ky);
                let c = ex.battlemap(key);
                let (ox, oy) = g.tile_origin_ft(&key);
                let hat = |q: [f64; 2]| -> Option<(f64, Surface)> {
                    let (fx, fy) = ((q[0] - ox) / 5.0 - 0.5, (q[1] - oy) / 5.0 - 0.5);
                    if fx < 0.0 || fy < 0.0 || fx >= (SQ - 1) as f64 || fy >= (SQ - 1) as f64 {
                        return None;
                    }
                    let (i, j) = (fx.floor() as usize, fy.floor() as usize);
                    let (tx, ty) = (fx - i as f64, fy - j as f64);
                    let k = j * SQ + i;
                    let h = |k: usize| c.height[k] as f64;
                    let z = (h(k) * (1.0 - tx) + h(k + 1) * tx) * (1.0 - ty) + (h(k + SQ) * (1.0 - tx) + h(k + SQ + 1) * tx) * ty;
                    Some((z, c.surface[((fy + 0.5) as usize) * SQ + (fx + 0.5) as usize]))
                };
                for k in 0..SQ * SQ {
                    if c.surface[k] == Surface::Stairs {
                        stairs += 1;
                    }
                    sq += 1;
                }
                // The track's bed along its line (dry road squares).
                for k in 0..36 {
                    let t = -180.0 + 10.0 * k as f64;
                    let (a, b) = ([p[0] + d[0] * t, p[1] + d[1] * t], [p[0] + d[0] * (t + 20.0), p[1] + d[1] * (t + 20.0)]);
                    if let (Some((ha, sa)), Some((hb, sb))) = (hat(a), hat(b)) {
                        if sa.is_road() && sb.is_road() {
                            wt = wt.max((hb - ha).abs() / 20.0);
                        }
                    }
                    // Bumpiness: the mean second difference over 5 ft on the track's line, and on
                    // the open hillside 40 ft the other side of it.
                    for (off, acc) in [(0.0, &mut bump_on), (-40.0, &mut bump_off)] {
                        let q = |e: f64| [a[0] + n[0] * off + d[0] * e, a[1] + n[1] * off + d[1] * e];
                        if let (Some((h0, s0)), Some((h1, _)), Some((h2, _))) = (hat(q(-5.0)), hat(q(0.0)), hat(q(5.0))) {
                            if off != 0.0 || s0.is_road() {
                                acc.0 += (h0 - 2.0 * h1 + h2).abs();
                                acc.1 += 1;
                            }
                        }
                    }
                    for off in [0.0, 60.0] {
                        let (a, b) = ([a[0] + n[0] * off, a[1] + n[1] * off], [a[0] + n[0] * off + d[0] * 5.0, a[1] + n[1] * off + d[1] * 5.0]);
                        if let (Some((ha, _)), Some((hb, _))) = (hat(a), hat(b)) {
                            ws = ws.max((hb - ha).abs());
                        }
                    }
                }
            }
        }
        let bump = |b: (f64, usize)| b.0 / b.1.max(1) as f64;
        println!("seed {seed}: bumpiness (mean 5-ft second difference) on the track {:.3} ft, on the hillside beside it {:.3} ft", bump(bump_on), bump(bump_off));
        println!("seed {seed}: {} hillsides (steepest ground {ground_slope:.2}), track bed grade max {wt:.3} (limit 0.15), steepest 5-ft step on the lines {ws:.1} ft, stair squares {stairs} of {sq}", sites.len());
        tot_stairs += stairs;
        tot_sq += sq;
        worst_track = worst_track.max(wt);
        worst_step = worst_step.max(ws);
    }
    println!("all: stair squares {tot_stairs} of {tot_sq}, track bed grade max {worst_track:.3}, steepest step {worst_step:.1} ft");
}
