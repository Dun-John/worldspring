//! Dev tool: print underground sites as text plans (one letter per room, `.` rock; `E` the way
//! in, `v`/`^` ways down/up, `*` items, `~` lava, `!` hazards), with timings.
//! `cargo run --release -p worldgen --example under -- [seed] [kind...]` (kinds: dungeon, crypt,
//! cave, mine, lava_tube; default all, the first site of each). TEXT=1 prints the design's text
//! form instead (`under::design::to_text`, what agents read and write with get/set_site_design).
//! Terrain tools on the printed site, after generation (`under::water`/`chasm`/`rubble_slope`; generation untouched):
//! WATER=wet|flooded, CHASM=level|room (the biggest room's), SPAN=bridge|stones (default bridge), RUBBLE=1 (a slope in
//! the biggest room). Letters: `-` shallow water, `=` deep water, `#` chasm, `+` rope bridge, `:` stepping stones,
//! `%` rubble slope; a line per level counts the squares still unreached (0 expected).
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
            let mut it = interior::generate_id(&world, &t0, &format!("u:{li}:{k}")).unwrap();
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let unreached = terrain(&mut it, worldgen::core::rng::hash3(seed as u64, li as i64, k as i64, 0x7e44));
            println!(
                "== {} u:{li}:{k} ({}x{}, {} levels, {ms:.1} ms) at fx {:.6} fy {:.6}",
                it.function,
                it.nx,
                it.ny,
                it.levels.len(),
                e.at[0] / world.geom.map_w_ft,
                e.at[1] / world.geom.map_h_ft
            );
            if !unreached.is_empty() {
                println!("terrain: unreached squares per level (deepest first) {unreached:?}");
            }
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
                        "water_shallow" => '-',
                        "water_deep" => '=',
                        "chasm" => '#',
                        "rope_bridge" => '+',
                        "stepping_stone" => ':',
                        "rubble_slope" => '%',
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

/// The terrain tools the env asks for, on every level of `it`: the squares each level leaves unreached (empty if none
/// asked for).
fn terrain(it: &mut interior::Interior, seed: u64) -> Vec<usize> {
    use worldgen::under::{self, Ground, Span, Wetness};
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let wet = match env("WATER").as_str() {
        "wet" => Wetness::Wet,
        "flooded" => Wetness::Flooded,
        _ => Wetness::Dry,
    };
    let (chasm, rubble) = (env("CHASM"), env("RUBBLE") == "1");
    if wet == Wetness::Dry && chasm.is_empty() && !rubble {
        return Vec::new();
    }
    let span = if env("SPAN") == "stones" { Span::Stones } else { Span::Bridge };
    let (nx, ny) = (it.nx, it.ny);
    let mut out = Vec::new();
    for (li, lv) in it.levels.iter_mut().enumerate() {
        let s = worldgen::core::rng::hash2(seed, li as i64, 0);
        let lay = |lv: &mut interior::Level, f: &dyn Fn(&Ground) -> Vec<interior::Item>| {
            let items = f(&Ground::of_level(lv, nx, ny));
            lv.furniture.extend(items);
        };
        let biggest = (0..lv.rooms.len()).max_by_key(|&r| (lv.rooms[r].squares, std::cmp::Reverse(r))).map(|r| r as i16);
        match chasm.as_str() {
            "level" => lay(lv, &|g| under::chasm(g, None, span, s ^ 1)),
            "room" => lay(lv, &|g| under::chasm(g, biggest, span, s ^ 1)),
            _ => {}
        }
        lay(lv, &|g| under::water(g, wet, s));
        if rubble && let Some(r) = biggest {
            let sq: Vec<usize> = (0..nx * ny).filter(|&q| lv.cells[q] == r).collect();
            let (cx, cy) = (sq.iter().map(|q| q % nx).sum::<usize>() / sq.len().max(1), sq.iter().map(|q| q / nx).sum::<usize>() / sq.len().max(1));
            lay(lv, &|g| under::rubble_slope(g, cx.saturating_sub(2), cy.saturating_sub(1), 5, 3));
        }
        out.push(Ground::of_level(lv, nx, ny).unreached());
    }
    out
}
