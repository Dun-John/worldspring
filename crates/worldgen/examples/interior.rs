//! Dev tool: print building interiors as text (one letter per room, # stairs, * furniture).
//! `cargo run --release -p worldgen --example interior -- [seed] [settlement] [function...]`
use worldgen::interior;
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let si: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| (0..t0.settlements.len()).max_by_key(|&i| t0.settlements[i].population).unwrap());
    let wanted: Vec<String> = a.iter().skip(3).cloned().collect();
    let l = worldgen::town::layout(&world, &t0, si);
    for want in &wanted {
        let Some(bi) = want.parse::<usize>().ok().or_else(|| l.buildings.iter().position(|b| b.label().to_lowercase() == *want)) else {
            println!("no {want}");
            continue;
        };
        let t = std::time::Instant::now();
        let it = interior::generate(&world, &t0, si, bi).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let outline = if it.outlines.is_empty() { String::new() } else { format!(", drawn inside a {}-sided outline", it.outlines.iter().map(|o| o.len().to_string()).collect::<Vec<_>>().join("+")) };
        println!("== {} {:?} ({}x{} squares, {} levels{outline}, {ms:.2} ms)", it.function, it.name, it.nx, it.ny, it.levels.len());
        for lv in &it.levels {
            let room = |r: &interior::Room| format!("{}{}={}", r.kind, interior::shape::name(r.shape).map(|s| format!(" ({s})")).unwrap_or_default(), r.squares);
            println!("-- {} (z {}): {}", lv.name, lv.z, lv.rooms.iter().map(room).collect::<Vec<_>>().join(", "));
            let mut grid: Vec<Vec<char>> = (0..it.ny).map(|j| (0..it.nx).map(|i| {
                let c = lv.cells[j * it.nx + i];
                if c < 0 { ' ' } else { (b'a' + (c as u8 % 26)) as char }
            }).collect()).collect();
            if std::env::var("NOFURN").is_err() {
                for f in &lv.furniture {
                    for j in f.y..f.y + f.h {
                        for i in f.x..f.x + f.w {
                            grid[j as usize][i as usize] = '*';
                        }
                    }
                }
            }
            println!("  furniture: {}", lv.furniture.iter().map(|f| format!("{}@{},{} {}x{}", f.kind, f.x, f.y, f.w, f.h)).collect::<Vec<_>>().join(" "));
            for j in it.stairs[1]..it.stairs[1] + it.stairs[3] {
                for i in it.stairs[0]..it.stairs[0] + it.stairs[2] {
                    grid[j][i] = '#';
                }
            }
            for row in grid {
                println!("  {}", row.into_iter().collect::<String>());
            }
            println!("  doors: {}", lv.doors.iter().map(|d| format!("{}@({},{})", d.kind, d.a[0], d.a[1])).collect::<Vec<_>>().join(" "));
        }
    }
}
