//! Scratch dev tool: shoreline consistency across levels. Samples a grid of fixed world
//! points around a map point and reports, per level, the wet fraction and how many points
//! flip wet/dry (and the mean height change) relative to the previous level.
//! `cargo run --release -p worldgen --example seacheck -- <seed|world.json> <fx> <fy> [half-width ft] [--json]`
//! `--json`: one JSON array on stdout, per level `{level, wet_pct, flips_pct, mean_dh_ft}` (`null` on the first).
mod common;

use serde_json::{json, Value};
use worldgen::core::tile::{HALO, PADDED, TileKey};
use worldgen::pipeline::Executor;

fn main() {
    let a = common::args();
    let (Some(arg), Some(fx), Some(fy)) = (a.get(0), a.get(1).and_then(|v| v.parse::<f64>().ok()), a.get(2).and_then(|v| v.parse::<f64>().ok())) else {
        eprintln!("usage: seacheck <seed|world.json> <fx> <fy> [half-width ft] [--json]");
        std::process::exit(2);
    };
    let half = a.num(3, 15_000.0);
    let world = common::world(arg);
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let (cx, cy) = (fx * g.map_w_ft, fy * g.map_h_ft);
    const N: usize = 60;
    let pts: Vec<(f64, f64)> = (0..N * N).map(|k| (cx - half + 2.0 * half * (k % N) as f64 / (N - 1) as f64, cy - half + 2.0 * half * (k / N) as f64 / (N - 1) as f64)).collect();
    let mut prev: Option<Vec<(f32, bool)>> = None;
    let mut levels = Vec::new();
    for level in 6..=14 {
        let s = g.spacing_ft(level);
        let size = g.tile_size_ft(level);
        let mut cur = Vec::with_capacity(pts.len());
        for &(x, y) in &pts {
            let key = TileKey::surface(level, (x / size) as u32, (y / size) as u32);
            let (ox, oy) = g.tile_origin_ft(&key);
            let (i, j) = (((x - ox) / s).round() as usize, ((y - oy) / s).round() as usize);
            let k = (j + HALO) * PADDED + i + HALO;
            let t = ex.terrain(key);
            let (h, rw) = (t.padded[k], t.river_water[k]);
            let w = ex.t0.sample_water(ox + i as f64 * s, oy + j as f64 * s).max(rw);
            cur.push((h, h < w));
        }
        let wet = 100.0 * cur.iter().filter(|c| c.1).count() as f64 / cur.len() as f64;
        let change = prev.as_ref().map(|p| {
            let flips = p.iter().zip(&cur).filter(|(a, b)| a.1 != b.1).count();
            let dh = p.iter().zip(&cur).map(|(a, b)| (a.0 - b.0).abs() as f64).sum::<f64>() / cur.len() as f64;
            (100.0 * flips as f64 / cur.len() as f64, dh)
        });
        let line = change.map(|(flips, dh)| format!("flips {flips:5.1}%  mean |dh| {dh:6.2} ft")).unwrap_or_default();
        if a.json {
            levels.push(json!({ "level": level, "wet_pct": wet, "flips_pct": change.map(|c| c.0), "mean_dh_ft": change.map(|c| c.1) }));
        } else {
            println!("L{level:<2} wet {wet:5.1}%  {line}");
        }
        prev = Some(cur);
    }
    if a.json {
        println!("{}", Value::Array(levels));
    }
}
