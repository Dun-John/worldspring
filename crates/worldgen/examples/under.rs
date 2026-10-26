//! Dev tool: print underground sites as text plans (one letter per room, `.` rock; `E` the way
//! in, `v`/`^` ways down/up, `*` items, `~` lava, `!` hazards), with timings.
//! `cargo run --release -p worldgen --example under -- [seed] [kind|theme...]` (kinds: dungeon, crypt,
//! cave, mine, lava_tube; themes: `under::THEMES` keys, e.g. temple, sunken_temple; default all, the first site of
//! each). TEXT=1 prints the design's text form instead (`under::design::to_text`, what agents read and write with
//! get/set_site_design). MAKE=theme[:size[:levels]][,...] adds created sites of those themes on dry land (as the vital
//! test does) and prints only them (e.g. MAKE=temple:large:3,sunken_temple).
//! LEVEL=n prints only level n (1 the top); ROOMS=1 leaves the furniture out. `keep` / `undercroft` print a keep's deep dungeons (`k:`) / a sewer
//! section with a grate and its undercroft below (`w:`).
//! Terrain tools on the printed site, after generation (`under::water`/`chasm`/`rubble_slope`; generation untouched):
//! WATER=wet|flooded, CHASM=level|room (the biggest room's), SPAN=bridge|stones (default bridge), RUBBLE=1 (a slope in
//! the biggest room). Letters: `-` shallow water, `=` deep water, `#` chasm, `+` rope bridge, `:` stepping stones,
//! `%` rubble slope; a line per level counts the squares still unreached (0 expected).
use worldgen::{World, WorldFile, interior, town};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let wanted: Vec<String> = a.iter().skip(2).cloned().collect();
    let mut world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let mut t0 = worldgen::t0::T0::generate(&world);
    let made = std::env::var("MAKE").unwrap_or_default();
    if !made.is_empty() {
        let sea = world.params().sea_level_ft;
        let g = &world.geom;
        let spots: Vec<[f64; 2]> = (0..90 * 60)
            .map(|k| [(k % 90) as f64 / 90.0 * g.map_w_ft + 1_000.0, (k / 90) as f64 / 60.0 * g.map_h_ft + 1_000.0])
            .filter(|p| {
                let h = t0.sample(p[0], p[1], 20.0);
                h > sea + 20.0 && (t0.sample_water(p[0], p[1]) as f64) < h && town::layouts_near(&world, &t0, [p[0] - 300.0, p[1] - 300.0, p[0] + 300.0, p[1] + 300.0]).is_empty()
            })
            .collect();
        let mut file = world.file.clone();
        let asks: Vec<&str> = made.split(',').collect();
        for (i, ask) in asks.iter().enumerate() {
            let mut f = ask.split(':');
            let theme = f.next().unwrap_or("temple");
            let t = worldgen::under::THEMES.iter().find(|t| t.key == theme).unwrap_or_else(|| panic!("no theme {theme}"));
            let p = spots[(i * 7919 + seed as usize * 131) % spots.len()];
            file.edits.created.push(worldgen::world::Created {
                id: format!("c:{i}"),
                kind: "entrance".into(),
                x: p[0],
                y: p[1],
                name: ask.to_string(),
                under: Some(t.kind.key().into()),
                size: f.next().map(|s| s.to_string()),
                levels: f.next().and_then(|s| s.parse().ok()),
                theme: Some(theme.into()),
                ..Default::default()
            });
        }
        world = World::new(file).unwrap();
        t0.apply_edits(&world);
    }
    let first = if made.is_empty() { 0 } else { t0.settlements.len() };
    let only_level: Option<usize> = std::env::var("LEVEL").ok().and_then(|v| v.parse().ok());
    let mut seen = std::collections::BTreeSet::new();
    let mut seen_n = std::collections::BTreeMap::new();
    let themes: Vec<&str> = worldgen::under::THEMES.iter().map(|t| t.key).collect();
    let by_theme = wanted.iter().any(|w| themes.contains(&w.as_str()));
    for li in first..town::layout_count(&t0) {
        if !made.is_empty() && t0.created_site(li - first).is_none() {
            continue;
        }
        let l = town::layout(&world, &t0, li);
        // (The ways down, then `keep`: keeps' deep dungeons, `undercroft`: sewer sections with grates.)
        let mut sites: Vec<(String, [f64; 2], String)> = l.entrances.iter().enumerate().map(|(k, e)| (format!("u:{li}:{k}"), e.at, e.kind.name().replace(' ', "_"))).collect();
        if wanted.iter().any(|w| w == "keep") {
            sites.extend(l.buildings.iter().filter(|b| interior::has_deep_dungeon(b)).map(|b| (format!("k:{li}:{}", b.id), town::geom::centroid(&b.poly), "keep".to_string())));
        }
        if wanted.iter().any(|w| w == "undercroft") {
            sites.extend(l.entrances.iter().filter(|e| e.kind == worldgen::under::UnderKind::Sewer).map(|e| (worldgen::under::sewer_id(li, e.at), e.at, "undercroft".to_string())));
        }
        for (id, at, kind) in sites {
            if !by_theme && !wanted.is_empty() && !wanted.contains(&kind) {
                continue;
            }
            let t = std::time::Instant::now();
            let Some(mut it) = interior::generate_id(&world, &t0, &id) else { continue };
            let ms = t.elapsed().as_secs_f64() * 1e3;
            // (By theme: each theme asked for; MAKE: every site made.)
            let key = if by_theme && !matches!(kind.as_str(), "keep" | "undercroft") { it.theme.unwrap_or("-").to_string() } else { kind.clone() };
            if by_theme && !wanted.contains(&key) {
                continue;
            }
            // SKIP=n: the n+1-th site of each kind instead of the first.
            let skip: usize = std::env::var("SKIP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            let count = seen_n.entry(key.clone()).or_insert(0usize);
            *count += 1;
            if made.is_empty() && (*count != skip + 1 || !seen.insert(key.clone())) {
                continue;
            }
            let unreached = terrain(&mut it, worldgen::core::rng::hash3(seed as u64, li as i64, worldgen::core::hash::fnv64(id.as_bytes()) as i64, 0x7e44));
            println!(
                "== {} {} {id} ({}x{}, {} levels, {ms:.1} ms) at fx {:.6} fy {:.6}",
                it.function,
                it.theme.unwrap_or("-"),
                it.nx,
                it.ny,
                it.levels.len(),
                at[0] / world.geom.map_w_ft,
                at[1] / world.geom.map_h_ft
            );
            // (A line of totals: locks, keys, portcullises, secret doors, water and spans in squares.)
            let doors = |k: &str| it.levels.iter().flat_map(|lv| &lv.doors).filter(|d| d.kind == k).count();
            let sq = |k: &str| it.levels.iter().flat_map(|lv| &lv.furniture).filter(|f| f.kind == k).map(|f| (f.w * f.h) as usize).sum::<usize>();
            println!(
                "   locks {}, keys {}, portcullises {}, secret doors {}; squares: shallow {}, deep {}, bridge {}, stones {}",
                it.levels.iter().flat_map(|lv| &lv.doors).filter(|d| d.lock != 0).count(),
                sq("key"),
                doors("portcullis"),
                doors("secret"),
                sq("water_shallow"),
                sq("water_deep"),
                sq("rope_bridge"),
                sq("stepping_stone")
            );
            if !unreached.is_empty() {
                println!("terrain: unreached squares per level (deepest first) {unreached:?}");
            }
            if std::env::var("TEXT").is_ok() {
                let d = worldgen::under::design::SiteDesign::from_interior(&it);
                println!("{}", worldgen::under::design::to_text(&d, &|_, _| None).unwrap_or_else(|e| e));
                continue;
            }
            let n = it.levels.len();
            for (li2, lv) in it.levels.iter().enumerate().rev() {
                if only_level.is_some_and(|w| w != n - li2) {
                    continue;
                }
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
                // (ROOMS=1: rooms and ways only, no furniture.)
                let bare = std::env::var("ROOMS").is_ok();
                for f in lv.furniture.iter().filter(|f| !bare || matches!(f.kind, "exit" | "down" | "up" | "water_shallow" | "water_deep" | "rope_bridge" | "stepping_stone")) {
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
