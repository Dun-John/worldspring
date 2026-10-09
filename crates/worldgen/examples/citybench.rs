//! Dev tool: time the heaviest content natively: T0, the metropolis layout, and the battlemap
//! chunks (terrain refine + chunk + pack) around its centre.
//! `cargo run --release -p worldgen --example citybench -- [seed] [radius] [--edited]`
//! (`--edited`: also the ward editor on it: a change tried, the town built with it, and cold).
use std::time::Instant;

use worldgen::core::tile::TileKey;
use worldgen::lod::terrain_refine::terrain_tile;
use worldgen::t0::T0;
use worldgen::t0::settle::Tier;
use worldgen::{World, WorldFile};

fn main() {
    let edited = std::env::args().any(|a| a == "--edited");
    let a: Vec<String> = std::env::args().filter(|a| !a.starts_with("--")).collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let r: i64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let t = Instant::now();
    let t0 = T0::generate(&world);
    println!("T0 {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let g = &world.geom;
    let si = (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier == Tier::Metropolis).max_by_key(|&i| t0.settlements[i].population).unwrap_or(0);
    let s = &t0.settlements[si];
    let t = Instant::now();
    let l = worldgen::town::layout(&world, &t0, si);
    println!("layout #{si}: {:.0} ms, {} buildings", t.elapsed().as_secs_f64() * 1e3, l.buildings.len());
    // Output fingerprint (to confirm a speed-up changes nothing).
    let mut lh = String::new();
    for b in &l.buildings {
        lh.push_str(&format!("{:?}{}{:?}{}", b.poly, b.floors, b.func, b.pad_ft));
    }
    for p in l.streets.iter().chain(&l.walls).chain(&l.districts) {
        lh.push_str(&format!("{p:?}"));
    }
    println!("layout hash {:016x}", worldgen::core::hash::fnv64(lh.as_bytes()));
    let mut chunk_bytes: Vec<u8> = Vec::new();
    let lv = g.max_level;
    let size = g.tile_size_ft(lv);
    let (cx, cy) = ((s.x / size) as i64, (s.y / size) as i64);
    let (mut refine, mut chunk_ms, mut pack_ms, mut worst, mut n, mut objs, mut kb) = (0.0, 0.0, 0.0, 0.0f64, 0, 0, 0);
    for dy in -r..=r {
        for dx in -r..=r {
            let key = TileKey::surface(lv, (cx + dx) as u32, (cy + dy) as u32);
            // The parent chain, as the app would have it.
            let mut parent: Option<Vec<f32>> = None;
            let mut tile = None;
            for l in 0..=lv {
                let pk = TileKey::surface(l, key.x >> (lv - l), key.y >> (lv - l));
                let t = Instant::now();
                let tt = terrain_tile(&world, &t0, &pk, parent.as_deref());
                if l == lv {
                    refine += t.elapsed().as_secs_f64() * 1e3;
                }
                parent = Some(tt.padded.clone());
                tile = Some(tt);
            }
            let tile = tile.unwrap();
            let t = Instant::now();
            let c = worldgen::battlemap::generate(&world, &t0, &key, &tile);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let bytes = worldgen::battlemap::pack(&world, &c);
            chunk_bytes.extend_from_slice(&bytes);
            pack_ms += t.elapsed().as_secs_f64() * 1e3;
            chunk_ms += ms;
            worst = worst.max(ms);
            n += 1;
            objs += c.objects.len();
            kb += bytes.len() / 1024;
        }
    }
    println!("chunks hash {:016x}", worldgen::core::hash::fnv64(&chunk_bytes));
    let n = n as f64;
    println!(
        "{n} chunks: refine {:.1} ms, battlemap {:.1} ms (worst {worst:.1}), pack {:.2} ms, {:.0} objects, {:.0} KB avg",
        refine / n,
        chunk_ms / n,
        pack_ms / n,
        objs as f64 / n,
        kb as f64 / n
    );
    if edited {
        ward_editor(world, &t0, si);
    }
}

/// The ward editor on the metropolis: a corner moved, a ward made a castle, another rolled again.
fn ward_editor(mut world: World, t0: &T0, si: usize) {
    use worldgen::town::wards;
    let t = Instant::now();
    let plan = wards::plan_json(&world, t0, si).unwrap();
    println!("plan (built with the layout's ground) {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let corner = plan["corners"].as_array().unwrap().iter().find(|c| c["pinned"].is_null() && c["gate"].is_null() && c["wall"].is_null()).unwrap()["corner"].clone();
    let commons: Vec<u64> = plan["patches"].as_array().unwrap().iter().filter(|p| p["in_town"] == true && p["ward"] == "common").filter_map(|p| p["patch"].as_u64()).take(2).collect();
    let req = format!(r#"{{"moves":[{{"corner":{corner},"by":[24,16]}}],"patches":[{{"patch":{},"ward":"castle"}},{{"patch":{},"reroll":true}}]}}"#, commons[0], commons[1]);
    let t = Instant::now();
    let (edit, report) = wards::change(&world, t0, si, &serde_json::from_str(&req).unwrap()).unwrap();
    println!("change tried {:.0} ms: {}", t.elapsed().as_secs_f64() * 1e3, report["buildings"]);
    world.file.edits.towns.insert(wards::key(si), edit.unwrap());
    let t = Instant::now();
    let l = worldgen::town::layout(&world, t0, si);
    println!("edited layout (as tried, kept) {:.2} ms, {} buildings", t.elapsed().as_secs_f64() * 1e3, l.buildings.len());
    worldgen::town::forget_from(0);
    let t = Instant::now();
    let l = worldgen::town::layout(&world, t0, si);
    println!("edited layout cold (generated + plan + build) {:.0} ms, {} buildings", t.elapsed().as_secs_f64() * 1e3, l.buildings.len());
    let t = Instant::now();
    let (_, report) = wards::change(&world, t0, si, &serde_json::from_str(&format!(r#"{{"patches":[{{"patch":{},"reroll":true}}]}}"#, commons[1])).unwrap()).unwrap();
    println!("changed again (plan kept: built and diffed) {:.0} ms: {}", t.elapsed().as_secs_f64() * 1e3, report["buildings"]);
}
