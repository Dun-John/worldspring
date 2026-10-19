//! Dev tool: print underground sites as text plans (one letter per room, `.` rock; `E` the way
//! in, `v`/`^` ways down/up, `*` items, `~` lava, `!` hazards), with timings.
//! `cargo run --release -p worldgen --example under -- [seed] [kind...]` (kinds: dungeon, crypt,
//! cave, mine, lava_tube; default all, the first site of each). TEXT=1 prints the design's text
//! form instead (`under::design::to_text`, what agents read and write with get/set_site_design).
use worldgen::{World, WorldFile, interior, town};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let wanted: Vec<String> = a.iter().skip(2).cloned().collect();
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let mut seen = std::collections::BTreeSet::new();
    let mut seen_n = std::collections::BTreeMap::new();
    for li in 0..town::layout_count(&t0) {
        let l = town::layout(&world, &t0, li);
        for (k, e) in l.entrances.iter().enumerate() {
            let kind = e.kind.name().replace(' ', "_");
            // SKIP=n: the n+1-th site of each kind instead of the first.
            let skip: usize = std::env::var("SKIP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            let count = seen_n.entry(kind.clone()).or_insert(0usize);
            *count += 1;
            if !wanted.is_empty() && !wanted.contains(&kind) || *count != skip + 1 || !seen.insert(kind.clone()) {
                continue;
            }
            let t = std::time::Instant::now();
            let it = interior::generate_id(&world, &t0, &format!("u:{li}:{k}")).unwrap();
            let ms = t.elapsed().as_secs_f64() * 1e3;
            println!(
                "== {} u:{li}:{k} ({}x{}, {} levels, {ms:.1} ms) at fx {:.6} fy {:.6}",
                it.function,
                it.nx,
                it.ny,
                it.levels.len(),
                e.at[0] / world.geom.map_w_ft,
                e.at[1] / world.geom.map_h_ft
            );
            if std::env::var("TEXT").is_ok() {
                let d = worldgen::under::design::SiteDesign::from_interior(&it);
                println!("{}", worldgen::under::design::to_text(&d, &|_, _| None).unwrap_or_else(|e| e));
                continue;
            }
            for lv in it.levels.iter().rev() {
                let mut kinds: Vec<String> = lv
                    .rooms
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.squares > 0)
                    .map(|(i, r)| {
                        let shape = worldgen::interior::shape::name(r.shape).map(|s| format!(" ({s})")).unwrap_or_default();
                        format!("{}={}{}{shape}", (b'a' + (i % 26) as u8) as char, r.kind, if r.raise_ft > 0.0 { format!("+{}", r.raise_ft) } else { String::new() })
                    })
                    .collect();
                kinds.dedup();
                println!("-- {} (z {}, {} items): {}", lv.name, lv.z, lv.furniture.len(), kinds.join(", "));
                let mut grid: Vec<Vec<char>> = (0..it.ny)
                    .map(|j| {
                        (0..it.nx)
                            .map(|i| {
                                let r = lv.cells[j * it.nx + i];
                                if r < 0 { '.' } else { (b'a' + (r as usize % 26) as u8) as char }
                            })
                            .collect()
                    })
                    .collect();
                for f in &lv.furniture {
                    let c = match f.kind {
                        "exit" => 'E',
                        "down" => 'v',
                        "up" => '^',
                        "lava" => '~',
                        _ if f.hazard.is_some() => '!',
                        _ => '*',
                    };
                    for j in f.y..f.y + f.h {
                        for i in f.x..f.x + f.w {
                            grid[j as usize][i as usize] = c;
                        }
                    }
                }
                for row in grid {
                    println!("  {}", row.into_iter().collect::<String>());
                }
            }
        }
    }
}
