//! Dev tool: print underground sites as text plans (one letter per room, `.` rock; `E` the way
//! in, `v`/`^` ways down/up, `*` items, `~` lava, `!` hazards), with timings.
//! `cargo run --release -p worldgen --example under -- [seed] [kind|theme...]` (kinds: dungeon, crypt,
//! cave, mine, lava_tube; themes: `under::THEMES` keys, e.g. temple, sunken_temple; default all, the first site of
//! each). TEXT=1 prints the design's text form instead (`under::design::to_text`, what agents read and write with
//! get/set_site_design). MAKE=theme[:size[:levels]][,...] adds created sites of those themes on dry land (as the vital
//! test does) and prints only them (e.g. MAKE=temple:large:3,sunken_temple).
//! ID=u:… prints only that site. FLOOR=1 prints floor heights in the plan. LEVEL=n prints only level n (1 the top); ROOMS=1 leaves the furniture out; BBOX=1 prints, instead of the plans, the share
//! of the grid each level's rooms span (their bounding box). `keep` / `undercroft` print a keep's deep dungeons (`k:`) / a sewer
//! section with a grate and its undercroft below (`w:`).
//! Terrain tools on the printed site, after generation (`under::water`/`chasm`/`rubble_slope`; generation untouched):
//! WATER=wet|flooded, CHASM=level|room (the biggest room's), SPAN=bridge|stones (default bridge), RUBBLE=1 (a slope in
//! the biggest room). Letters: `-` shallow water, `=` deep water, `#` chasm, `+` rope bridge, `:` stepping stones,
//! `%` rubble slope; a line per level counts the squares still unreached (0 expected).
//! LEDGES=1: over every site (not the first of each kind), raised chambers (over 3 ft) and their open sheer edges
//! (squares of the ledge over a floor more than 3 ft lower): a total line, no plans.
//! BOSS=1 (BOSS=list: a line per site too; BOSS=plan, with ID=: the chamber, items by letter, heights in 2.5 ft, doors
//! '?' secret '+' plain, ',' floor outside): over every site (keeps' deep dungeons too), the boss chamber's layout
//! (`under::boss`, told by its items) and centrepiece by theme, a line per theme and the spread; caves', lava tubes'
//! and mines' layouts; hazards (share, mean, kinds) and second ways in (share, secret); CHECK lines where a throne
//! stands outside keep dungeons, dungeons and dwarven halls, a ritual circle outside temples and cults, or a chamber
//! has more than one second way in.
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
    let ledges_only = std::env::var("LEDGES").is_ok();
    let mut ledges = Ledges::default();
    let boss_only = std::env::var("BOSS").is_ok();
    // BIOME=1: caves of this seed by look (green/dry/cold/rock, or ice whatever the biome), as one line.
    let biome_only = std::env::var("BIOME").is_ok();
    let mut looks: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut boss = Boss::default();
    for li in first..town::layout_count(&t0) {
        if !made.is_empty() && t0.created_site(li - first).is_none() {
            continue;
        }
        let l = town::layout(&world, &t0, li);
        // (The ways down, then `keep`: keeps' deep dungeons, `undercroft`: sewer sections with grates.)
        let mut sites: Vec<(String, [f64; 2], String)> = l.entrances.iter().enumerate().map(|(k, e)| (format!("u:{li}:{k}"), e.at, e.kind.name().replace(' ', "_"))).collect();
        if wanted.iter().any(|w| w == "keep") || boss_only {
            sites.extend(l.buildings.iter().filter(|b| interior::has_deep_dungeon(b)).map(|b| (format!("k:{li}:{}", b.id), town::geom::centroid(&b.poly), "keep".to_string())));
        }
        if wanted.iter().any(|w| w == "undercroft") {
            sites.extend(l.entrances.iter().filter(|e| e.kind == worldgen::under::UnderKind::Sewer).map(|e| (worldgen::under::sewer_id(li, e.at), e.at, "undercroft".to_string())));
        }
        for (id, at, kind) in sites {
            if !by_theme && !wanted.is_empty() && !wanted.contains(&kind) {
                continue;
            }
            // ID=u:…: only that site.
            if std::env::var("ID").is_ok_and(|w| w != id) {
                continue;
            }
            let t = std::time::Instant::now();
            let Some(mut it) = interior::generate_id(&world, &t0, &id) else { continue };
            let ms = t.elapsed().as_secs_f64() * 1e3;
            if biome_only {
                if kind == "cave" {
                    *looks.entry(if it.theme == Some("ice") { "ice" } else { ["rock", "green", "dry", "cold"][it.look as usize] }).or_default() += 1;
                }
                continue;
            }
            // (By theme: each theme asked for; MAKE: every site made.)
            let key = if by_theme && !matches!(kind.as_str(), "keep" | "undercroft") { it.theme.unwrap_or("-").to_string() } else { kind.clone() };
            if by_theme && !wanted.contains(&key) {
                continue;
            }
            if ledges_only {
                ledges.add(&it, &key, ms);
                continue;
            }
            if boss_only {
                boss.add(&it, it.theme.unwrap_or(if kind == "keep" { "keep" } else { "-" }), ms);
                continue;
            }
            // SKIP=n: the n+1-th site of each kind instead of the first.
            let skip: usize = std::env::var("SKIP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            let count = seen_n.entry(key.clone()).or_insert(0usize);
            *count += 1;
            if made.is_empty() && std::env::var("ID").is_err() && (*count != skip + 1 || !seen.insert(key.clone())) {
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
            // BBOX=1: per level, the share of the grid its rooms' bounding box covers (top first), no plan.
            if std::env::var("BBOX").is_ok() {
                let shares: Vec<String> = it
                    .levels
                    .iter()
                    .rev()
                    .map(|lv| {
                        let (mut x0, mut y0, mut x1, mut y1) = (it.nx, it.ny, 0, 0);
                        for (k, _) in lv.cells.iter().enumerate().filter(|(_, c)| **c >= 0) {
                            let (i, j) = (k % it.nx, k / it.nx);
                            (x0, y0, x1, y1) = (x0.min(i), y0.min(j), x1.max(i), y1.max(j));
                        }
                        let (w, h) = ((x1 + 1).saturating_sub(x0), (y1 + 1).saturating_sub(y0));
                        format!("{:.2} ({w}x{h}, {} rooms)", (w * h) as f64 / ((it.nx - 2) * (it.ny - 2)) as f64, lv.rooms.len())
                    })
                    .collect();
                println!("   bbox {}", shares.join(" | "));
                continue;
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
                for f in lv.furniture.iter().filter(|f| !bare || matches!(f.kind, "exit" | "down" | "up" | "water_shallow" | "water_deep" | "chasm" | "rope_bridge" | "stepping_stone" | "stone_bridge")) {
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
                        "stone_bridge" => '"',
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
                // FLOOR=1: the floor heights instead (`Level.floor`: '0' level, ':' 5 ft, 'D' 10 ft; '.' rock).
                if std::env::var("FLOOR").is_ok() && !lv.floor.is_empty() {
                    let f = lv.floor.as_bytes();
                    for (j, row) in grid.iter_mut().enumerate() {
                        for (i, c) in row.iter_mut().enumerate() {
                            *c = if lv.cells[j * it.nx + i] < 0 { '.' } else { f[j * it.nx + i] as char };
                        }
                    }
                }
                for row in grid {
                    println!("  {}", row.into_iter().collect::<String>());
                }
            }
        }
    }
    if biome_only {
        println!("seed {seed}: caves {} {looks:?}", looks.values().sum::<usize>());
    }
    if ledges_only {
        ledges.print();
    }
    if boss_only {
        boss.print();
    }
}

/// BOSS=1's counts: per theme, the boss chambers' layouts and centrepieces.
#[derive(Default)]
struct Boss {
    by_theme: std::collections::BTreeMap<String, (std::collections::BTreeMap<&'static str, usize>, std::collections::BTreeMap<&'static str, usize>)>,
    layouts: std::collections::BTreeMap<&'static str, usize>,
    /// Per site size (keeps' as "keep"): each chamber's bounding box (short side, long side) and its layout.
    sizes: std::collections::BTreeMap<String, Vec<(usize, usize, &'static str)>>,
    n: usize,
    worst_ms: f64,
    /// Caves, lava tubes and mines: per kind ("cave", "lava tube", "mine") the layouts.
    wild: std::collections::BTreeMap<&'static str, std::collections::BTreeMap<&'static str, usize>>,
    /// Every boss chamber (built and natural): how many, with hazards, hazards in all, by kind; with a second way in,
    /// of them secret; more than one (a CHECK).
    all: usize,
    perilous: usize,
    perils: std::collections::BTreeMap<&'static str, usize>,
    sides: usize,
    secret: usize,
    /// Chambers with cover (an item giving cover, not terrain) in each half of their box, north, south, west and east.
    halves: usize,
}

impl Boss {
    fn add(&mut self, it: &interior::Interior, theme: &str, ms: f64) {
        if theme == "-" {
            return;
        }
        self.worst_ms = self.worst_ms.max(ms);
        let nx = it.nx;
        let lv = &it.levels[0];
        let Some(ri) = lv.rooms.iter().position(|r| r.kind == worldgen::under::BOSS && r.squares > 0) else { return };
        let room = lv.cells.iter().filter(|&&c| c == ri as i16).count();
        let inside: Vec<&interior::Item> = lv.furniture.iter().filter(|f| lv.cells[f.y as usize * nx + f.x as usize] == ri as i16).collect();
        let sq = |k: &str| inside.iter().filter(|f| f.kind == k).map(|f| (f.w * f.h) as usize).sum::<usize>();
        let has = |k: &str| inside.iter().any(|f| f.kind == k);
        // (Hazards and the second way in, every chamber.)
        self.all += 1;
        {
            let sq: Vec<usize> = (0..lv.cells.len()).filter(|&k| lv.cells[k] == ri as i16).collect();
            let (x0, x1) = (sq.iter().map(|k| k % nx).min().unwrap_or(0), sq.iter().map(|k| k % nx).max().unwrap_or(0));
            let (y0, y1) = (sq.iter().map(|k| k / nx).min().unwrap_or(0), sq.iter().map(|k| k / nx).max().unwrap_or(0));
            let cover: Vec<&&interior::Item> = inside.iter().filter(|f| f.cover >= 1 && !worldgen::under::TERRAIN.contains(&f.kind) && !matches!(f.kind, "stone_bridge" | "rope_bridge" | "stepping_stone")).collect();
            let (mx, my) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
            let c = |f: &&&interior::Item| (f.x as f64 + f.w as f64 / 2.0 - 0.5, f.y as f64 + f.h as f64 / 2.0 - 0.5);
            let all = [cover.iter().any(|f| c(f).0 < mx), cover.iter().any(|f| c(f).0 > mx), cover.iter().any(|f| c(f).1 < my), cover.iter().any(|f| c(f).1 > my)];
            self.halves += all.iter().all(|&b| b) as usize;
            if std::env::var("BOSS").is_ok_and(|v| v == "list") && !all.iter().all(|&b| b) {
                println!("{} {theme}: no cover in a half (w e n s {all:?}), {}x{}", it.id, x1 - x0 + 1, y1 - y0 + 1);
            }
        }
        let perils: Vec<&'static str> = inside.iter().map(|f| f.name).filter(|n| worldgen::under::PERILS.contains(n)).collect();
        self.perilous += !perils.is_empty() as usize;
        for p in &perils {
            *self.perils.entry(p).or_default() += 1;
        }
        let doors: Vec<&interior::Door> = lv.doors.iter().filter(|d| d.rooms.contains(&(ri as i16))).collect();
        let side = doors.len().saturating_sub(!lv.natural as usize);
        if side > 1 {
            println!("CHECK {}: {side} second ways into the boss chamber", it.id);
        }
        if side > 0 {
            self.sides += 1;
            self.secret += doors.iter().any(|d| d.kind == "secret") as usize;
        }
        if lv.natural {
            let kind: &'static str = match theme {
                "mine" => "mine",
                "lava_tube" => "lava tube",
                _ => "cave",
            };
            let isle = (0..lv.cells.len()).any(|k| {
                lv.cells[k] == ri as i16 && [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx].iter().any(|&m| m < lv.cells.len() && lv.cells[m] >= 0 && lv.cells[m] != ri as i16 && lv.furniture.iter().any(|f| f.kind == "water_deep" && f.x as usize == m % nx && f.y as usize == m / nx))
            });
            let count = |k: &str| inside.iter().filter(|f| f.kind == k).count();
            let layout = if kind == "mine" {
                // (Walkways: the scaffolding's pieces, joined side by side; across the face if its long side runs
                // across the line from the way in.)
                let on: Vec<usize> = inside.iter().filter(|f| f.kind == "scaffold").flat_map(|f| (f.y..f.y + f.h).flat_map(move |j| (f.x..f.x + f.w).map(move |i| j as usize * nx + i as usize))).collect();
                let n = lv.cells.len();
                let step = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < n).then_some(k + nx)].into_iter().flatten();
                let walk = |from: &[usize], inside_only: bool| {
                    let mut d = vec![usize::MAX; n];
                    let mut q: std::collections::VecDeque<usize> = from.iter().copied().collect();
                    for &k in from {
                        d[k] = 0;
                    }
                    while let Some(k) = q.pop_front() {
                        for m in step(k) {
                            if d[m] == usize::MAX && lv.cells[m] >= 0 && (!inside_only || lv.cells[m] == ri as i16) {
                                d[m] = d[k] + 1;
                                q.push_back(m);
                            }
                        }
                    }
                    d
                };
                let start: Vec<usize> = lv.furniture.iter().filter(|f| matches!(f.kind, "up" | "exit")).map(|f| f.y as usize * nx + f.x as usize).collect();
                let from = walk(&start, false);
                let entry = (0..n).filter(|&k| lv.cells[k] == ri as i16).min_by_key(|&k| (from[k], k)).unwrap_or(0);
                let mut seen = std::collections::BTreeSet::new();
                let (mut walks, mut across) = (0, false);
                for &s in &on {
                    if !seen.insert(s) {
                        continue;
                    }
                    walks += 1;
                    let mut todo = vec![s];
                    let mut comp = Vec::new();
                    while let Some(k) = todo.pop() {
                        comp.push(k);
                        for m in [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx] {
                            if on.contains(&m) && seen.insert(m) {
                                todo.push(m);
                            }
                        }
                    }
                    // (Its long side across the line from the way in to it.)
                    let (xs, ys) = (comp.iter().map(|k| k % nx), comp.iter().map(|k| k / nx));
                    let (w, h) = (xs.clone().max().unwrap_or(0) - xs.clone().min().unwrap_or(0), ys.clone().max().unwrap_or(0) - ys.clone().min().unwrap_or(0));
                    let (cx, cy) = (xs.sum::<usize>() as f64 / comp.len() as f64, ys.sum::<usize>() as f64 / comp.len() as f64);
                    let (dx, dy) = ((cx - (entry % nx) as f64).abs(), (cy - (entry / nx) as f64).abs());
                    across |= if w > h { dy > dx } else { dx > dy };
                }
                if walks == 1 && across { "face gallery" } else if walks >= 2 { "twin walkways" } else if walks == 1 { "side walkway" } else { "(none)" }
            } else if has("rope_bridge") || has("chasm") {
                "rope-bridge chasm"
            } else if has("lava") {
                "lava channel"
            } else if has("water_deep") && has("stepping_stone") {
                "pool island"
            } else if isle {
                "lake island"
            } else if has("platform") || has("balcony") {
                "ledges"
            } else if has("cave_in") || sq("rubble") >= 5 {
                "rubble field"
            } else if count("rock_column") + count("basalt") >= 3 {
                "rock columns"
            } else {
                "(none)"
            };
            *self.wild.entry(kind).or_default().entry(layout).or_default() += 1;
            if std::env::var("BOSS").is_ok_and(|v| v == "list") {
                println!("{} {theme}: {layout}, perils {perils:?}, side {side}", it.id);
            }
            if std::env::var("BOSS").is_ok_and(|v| v == "plan") {
                println!("{} {theme}: {layout}", it.id);
                plan(lv, ri, nx, &inside);
            }
            return;
        }
        let layout = if has("chasm") {
            "chasm and bridge"
        } else if (has("water_deep") || has("lava")) && (has("stone_bridge") || has("stepping_stone")) {
            "moat"
        } else if (sq("platform") + sq("balcony")) * 5 > room * 2 {
            "sunken arena"
        } else if has("balcony") {
            "balconies"
        } else if has("platform") {
            "raised platform"
        } else if has("wall_stub") {
            "broken walls"
        } else if inside.iter().filter(|f| f.kind == "pit").count() >= 2 {
            "pits"
        } else if sq("statue") >= 4 {
            "statue gallery"
        } else if sq("rubble") >= 6 {
            "rubble ring"
        } else if sq("pillar") >= 4 {
            "pillar grid"
        } else {
            "(none)"
        };
        let centre = if has("throne") && has("forge") {
            "forge-throne"
        } else if has("throne") {
            "throne"
        } else if has("altar") && has("glyph") {
            "altar+circle"
        } else if has("arcane_workbench") {
            "workshop"
        } else if has("cage") {
            "cell block"
        } else if has("sarcophagus") {
            if has("dais") || has("platform") { "sarcophagus on bier" } else { "sarcophagus" }
        } else if has("chair_of_office") {
            "chief's seat"
        } else if has("totem") {
            "totem+mounds"
        } else if has("nest") {
            "lair"
        } else if has("long_table") {
            "war table"
        } else if has("hoard") {
            "hoard"
        } else {
            "(none)"
        };
        if has("throne") && !matches!(theme, "keep" | "dungeon" | "dwarven_hall") {
            println!("CHECK {}: a throne in a {theme}'s boss chamber", it.id);
        }
        if has("glyph") && !matches!(theme, "temple" | "sunken_temple" | "cult_sanctum") {
            println!("CHECK {}: a ritual circle in a {theme}'s boss chamber", it.id);
        }
        // (BOSS=plan: the chamber, its items by letter beside its floor heights in 5 ft.)
        if std::env::var("BOSS").is_ok_and(|v| v == "plan") {
            println!("{} {theme}: {layout}, {centre}", it.id);
            plan(lv, ri, nx, &inside);
        }
        // (BOSS=list: a line per chamber.)
        if std::env::var("BOSS").is_ok_and(|v| v == "list") {
            println!("{} {theme}: {layout}, {centre}", it.id);
        }
        let e = self.by_theme.entry(theme.to_string()).or_default();
        *e.0.entry(layout).or_default() += 1;
        *e.1.entry(centre).or_default() += 1;
        *self.layouts.entry(layout).or_default() += 1;
        let (bw, bh) = {
            let sq: Vec<usize> = (0..lv.cells.len()).filter(|&k| lv.cells[k] == ri as i16).collect();
            let w = sq.iter().map(|k| k % nx).max().unwrap_or(0) - sq.iter().map(|k| k % nx).min().unwrap_or(0) + 1;
            let h = sq.iter().map(|k| k / nx).max().unwrap_or(0) - sq.iter().map(|k| k / nx).min().unwrap_or(0) + 1;
            (w.min(h), w.max(h))
        };
        let size = worldgen::under::site_size(it).map_or("keep", |s| s.name()).to_string();
        if std::env::var("BOSS").is_ok_and(|v| v == "list") {
            println!("  {size} {bw}x{bh}");
        }
        self.sizes.entry(size).or_default().push((bw, bh, layout));
        self.n += 1;
    }

    fn print(&self) {
        let fmt = |m: &std::collections::BTreeMap<&str, usize>| m.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
        for (t, (l, c)) in &self.by_theme {
            println!("  {t}: layouts {} | centrepieces {}", fmt(l), fmt(c));
        }
        let spread: Vec<String> = self.layouts.iter().map(|(k, v)| format!("{k} {v} ({:.0}%)", 100.0 * *v as f64 / self.n.max(1) as f64)).collect();
        for (size, v) in &self.sizes {
            let mid = |mut x: Vec<usize>| {
                x.sort();
                format!("{}/{}/{}", x[0], x[x.len() / 2], x[x.len() - 1])
            };
            let none = v.iter().filter(|q| q.2 == "(none)").count();
            println!("  size {size}: {} chambers, short side {} long side {} (min/median/max squares), no layout {none}", v.len(), mid(v.iter().map(|q| q.0).collect()), mid(v.iter().map(|q| q.1).collect()));
        }
        println!("boss: {} built boss chambers (worst site {:.0} ms); layouts {}", self.n, self.worst_ms, spread.join(", "));
        for (kind, m) in &self.wild {
            let n: usize = m.values().sum();
            let spread: Vec<String> = m.iter().map(|(k, v)| format!("{k} {v} ({:.0}%)", 100.0 * *v as f64 / n.max(1) as f64)).collect();
            println!("boss {kind}: {n} chambers; layouts {}", spread.join(", "));
        }
        let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
        let total: usize = self.perils.values().sum();
        println!(
            "boss hazards: {} of {} chambers ({:.0}%), {:.2} each where any; kinds {}",
            self.perilous,
            self.all,
            pct(self.perilous, self.all),
            total as f64 / self.perilous.max(1) as f64,
            self.perils.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")
        );
        println!("boss ways round: {} of {} ({:.0}%), secret {} ({:.0}%), door/arch {}", self.sides, self.all, pct(self.sides, self.all), self.secret, pct(self.secret, self.sides), self.sides - self.secret);
        println!("boss cover: in every half (north, south, west, east) of {} of {} chambers ({:.0}%)", self.halves, self.all, pct(self.halves, self.all));
    }
}

/// BOSS=plan's picture of a boss chamber: its items by letter beside its floor heights (2.5 ft a digit, '/' a ramp),
/// '@' where its name goes.
fn plan(lv: &interior::Level, ri: usize, nx: usize, inside: &[&interior::Item]) {
    let f = worldgen::interior::height::of_level(lv, nx);
    // (The chamber and a square round it, for the second way in.)
    let sq: Vec<usize> = (0..lv.cells.len()).filter(|&k| lv.cells[k] == ri as i16).collect();
    let (x0, x1) = (sq.iter().map(|k| k % nx).min().unwrap_or(0), sq.iter().map(|k| k % nx).max().unwrap_or(0));
    let (y0, y1) = (sq.iter().map(|k| k / nx).min().unwrap_or(0), sq.iter().map(|k| k / nx).max().unwrap_or(0));
    let door = |k: usize| lv.doors.iter().filter(|d| d.rooms.contains(&(ri as i16))).find(|d| {
        // (The squares either side of its edge.)
        let (x, y) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize);
        let (a, b) = if d.a[1] == d.b[1] { (y.saturating_sub(1) * nx + x, y * nx + x) } else { (y * nx + x.saturating_sub(1), y * nx + x) };
        a == k || b == k
    });
    for y in y0..=y1 {
        let mut a = String::new();
        let mut b = String::new();
        for x in x0..=x1 {
            let k = y * nx + x;
            if lv.cells[k] != ri as i16 {
                a.push(if lv.cells[k] >= 0 { ',' } else { ' ' });
                b.push(' ');
                continue;
            }
            let top = inside.iter().rev().find(|i| x as u16 >= i.x && (x as u16) < i.x + i.w && y as u16 >= i.y && (y as u16) < i.y + i.h && !matches!(i.kind, "platform" | "balcony" | "scaffold"));
            a.push(match (door(k), top) {
                (Some(d), _) => if d.kind == "secret" { '?' } else { '+' },
                (_, Some(i)) => match i.name {
                    "steam vent" => 'v', "cracked floor" => 'x', "flooded pit" => 'u', "pressure plate" => '^',
                    _ => match i.kind {
                        "pillar" => 'P', "statue" => 'S', "wall_stub" => 'W', "collapsed_wall" => 'w', "fallen_column" => 'c', "rubble" => 'r', "boulder" => 'B',
                        "pit" => 'O', "chasm" => '#', "stone_bridge" | "rope_bridge" => '=', "water_deep" | "lava" => '~', "water_shallow" => '-', "stepping_stone" => ':', "steps" => '/', "climb_ladder" => 'H',
                        "dais" => 'd', "throne" => 'T', "altar" => 'A', "glyph" => 'g', "sarcophagus" => 'X', "hoard" => '$', "chest" => 'C', "brazier" => 'b',
                        "banner" => 'f', "candles" => 'i', "forge" => 'F', "anvil" => 'n', "cage" => 'G', "desk" => 'D', "chains" => 'h', "arcane_workbench" => 'K',
                        "arcane_focus" => 'o', "bookshelf" => 'L', "totem" => 't', "bone_mound" => 'M', "nest" => 'N', "bones" => 'z', "chair_of_office" => 'Y',
                        "rock_column" | "basalt" => 'R', "rubble_slope" => '/', "cave_in" => 'k', "ore_vein" => 'V', "tools" => 'l', "rail" => '|', "ore_cart" => 'Q',
                        "crystal" | "ice" => '*', "fungus" | "mushroom" => 'm', "stalagmite" => 's', _ => '&',
                    },
                },
                // (The room's name: `Room::center`.)
                _ if lv.rooms[ri].center == [x as f32 + 0.5, y as f32 + 0.5] => '@',
                _ => '.',
            });
            let ft = f.ft[k];
            b.push(if f.ramp[k] { '/' } else if ft == 0.0 { '.' } else { (b'0' + (ft / 2.5) as u8) as char });
        }
        println!("  {a}   {b}");
    }
}

/// LEDGES=1's counts: per raised chamber its open edge (squares), the ones with none and whether a lower floor lies
/// within 3 squares of them (else none near), by theme.
#[derive(Default)]
struct Ledges {
    edges: Vec<usize>,
    near: usize,
    none_near: usize,
    by_theme: std::collections::BTreeMap<String, (usize, usize)>,
    worst_ms: f64,
}

impl Ledges {
    fn add(&mut self, it: &interior::Interior, theme: &str, ms: f64) {
        let nx = it.nx;
        self.worst_ms = self.worst_ms.max(ms);
        for lv in &it.levels {
            if lv.floor.is_empty() {
                continue;
            }
            let f = worldgen::interior::height::of_level(lv, nx);
            let n = lv.cells.len();
            let side = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then(|| k + 1), (k >= nx).then(|| k - nx), (k + nx < n).then(|| k + nx)].into_iter().flatten();
            // (Squares reached from the way in only by a climb, as the vital `r5_no_climb` walks: natural levels only.)
            if lv.natural {
                let blocks = worldgen::under::blocked_squares(&lv.furniture, nx, n / nx);
                let start = lv.furniture.iter().find(|f| matches!(f.kind, "exit" | "up")).map(|f| f.y as usize * nx + f.x as usize);
                if let Some(start) = start {
                    let mut seen = vec![false; n];
                    let mut stack = vec![start];
                    seen[start] = true;
                    while let Some(k) = stack.pop() {
                        for q in side(k) {
                            if !seen[q] && lv.cells[q] >= 0 && !blocks[q] && f.sheer(k, q) <= 0.0 {
                                seen[q] = true;
                                stack.push(q);
                            }
                        }
                    }
                    let lost: Vec<usize> = (0..n).filter(|&q| lv.cells[q] >= 0 && !blocks[q] && !seen[q]).collect();
                    if !lost.is_empty() {
                        println!("CLIMB {} {}: {} squares only by a climb, first at ({}, {})", it.id, lv.name, lost.len(), lost[0] % nx, lost[0] / nx);
                    }
                }
            }
            for (r, room) in lv.rooms.iter().enumerate() {
                if room.raise_ft <= 3.0 {
                    continue;
                }
                let sq: Vec<usize> = (0..n).filter(|&k| lv.cells[k] == r as i16).collect();
                if sq.is_empty() {
                    continue;
                }
                let edge = sq.iter().filter(|&&k| side(k).any(|m| lv.cells[m] >= 0 && f.sheer(k, m) < 0.0)).count();
                let e = self.by_theme.entry(theme.to_string()).or_default();
                e.0 += 1;
                if edge > 0 {
                    e.1 += 1;
                    self.edges.push(edge);
                } else {
                    let low = |m: usize| lv.cells[m] >= 0 && lv.cells[m] != r as i16 && f.ft[m] + 3.0 < room.raise_ft;
                    let near = sq.iter().any(|&k| {
                        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                        (-3..=3isize).any(|dj| (-3..=3isize).any(|di| {
                            let (x, y) = (i + di, j + dj);
                            x >= 0 && y >= 0 && (x as usize) < nx && (y as usize) < n / nx && low(y as usize * nx + x as usize)
                        }))
                    });
                    if near { self.near += 1 } else { self.none_near += 1 }
                }
            }
        }
    }

    fn print(&mut self) {
        self.edges.sort_unstable();
        let total = self.edges.len() + self.near + self.none_near;
        let at = |q: f64| self.edges.get(((self.edges.len().max(1) - 1) as f64 * q) as usize).copied().unwrap_or(0);
        let themes: Vec<String> = self.by_theme.iter().map(|(t, (a, b))| format!("{t} {b}/{a}")).collect();
        println!(
            "ledges: {total} raised (worst site {:.0} ms), {} with an open edge ({:.0}%); edge squares min {} median {} max {}; none open: {} with a lower floor within 3 squares, {} none near; by theme {}",
            self.worst_ms,
            self.edges.len(),
            100.0 * self.edges.len() as f64 / total.max(1) as f64,
            at(0.0),
            at(0.5),
            at(1.0),
            self.near,
            self.none_near,
            themes.join(", ")
        );
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
