//! The battlemap guarantee: parks reachable, the sea.

use super::*;

/// Battlemap guarantee: in sampled land chunks across seeds and biomes (and in town, on its squares of every
/// kind), every 32×32-square window has ≥ 3 cover objects or buildings, ≥ 2 elevation tiers, and a hazard/prop or
/// atmosphere; and no dry square is more than MAX_COVER_GAP squares from cover. Mostly-water windows (any depth) and
/// bridge decks are exempt. In a castle's yard its curtains, towers and buildings are its cover: a window of it holding
/// any of them needs no more, and its squares may be YARD_COVER_GAP from cover (`cover_gap`, `works_cover`).
#[test]
fn battlemap_guarantee() {
    use worldgen::battlemap::{self, SQ};
    const W: usize = 32;
    let mut biomes_seen = std::collections::BTreeSet::new();
    for seed in [3u32, 99] {
        let world = World::from_json(&world_json(seed)).unwrap();
        let g = world.geom.clone();
        let mut ex = Executor::new(world);
        let mut rng = worldgen::core::rng::Pcg32::new(seed as u64, 1);
        let mut checked = 0;
        // Built-up chunks first (the largest settlement and a village), then random land.
        let mut sites: Vec<(f64, f64)> = Vec::new();
        // (Sites whose chunk is checked against its neighbours too.)
        let mut seams: Vec<usize> = vec![0];
        let mut parks: Vec<usize> = Vec::new();
        // (Park sites whose chunk is the one holding the park's middle: a formal garden's basin.)
        let mut middles: Vec<usize> = Vec::new();
        {
            use worldgen::t0::settle::Tier;
            let st = &ex.t0.settlements;
            if let Some(s) = st.iter().max_by_key(|s| s.population) {
                sites.push((s.x, s.y));
            }
            if let Some(s) = st.iter().find(|s| s.tier == Tier::Village) {
                sites.push((s.x, s.y));
            }
            // A square of each kind besides the main one (`town::squares`), where a town has one.
            use worldgen::town::PlazaKind as K;
            for kind in [K::Crossroads, K::WellSquare, K::FountainSquare, K::Green, K::TempleClose] {
                let at = (0..st.len()).filter(|&i| st[i].tier >= Tier::Town).find_map(|i| worldgen::town::layout(&ex.world, &ex.t0, i).plazas.iter().find(|p| p.kind == kind).map(|p| worldgen::town::geom::centroid(&p.pts)));
                sites.extend(at.map(|c| (c[0], c[1])));
            }
            // The biggest park of each kind (`park_layout`): the chunk whose east and south edges pass
            // nearest its middle (the seams run through it), and the formal garden's chunk holding its
            // middle (its basin's gravel round), every square of them reachable.
            let all: Vec<(worldgen::town::ParkKind, f64, [f64; 2])> = (0..st.len())
                .filter(|&i| st[i].tier >= Tier::Town)
                .flat_map(|i| worldgen::town::layout(&ex.world, &ex.t0, i).parks.iter().map(|p| (p.kind, worldgen::town::geom::area(&p.pts).abs(), worldgen::town::geom::centroid(&p.pts))).collect::<Vec<_>>())
                .collect();
            // (R5 towns: a horse park's corrals too, 114.)
            for kind in [worldgen::town::ParkKind::Formal, worldgen::town::ParkKind::Common, worldgen::town::ParkKind::Horses] {
                let biggest = all.iter().filter(|p| p.0 == kind).max_by(|a, b| a.1.total_cmp(&b.1));
                if let Some(&(_, _, c)) = biggest {
                    seams.push(sites.len());
                    parks.push(sites.len());
                    sites.push((c[0], c[1]));
                    if kind == worldgen::town::ParkKind::Formal {
                        middles.push(sites.len());
                        sites.push((c[0], c[1]));
                    }
                }
            }
        }
        for attempt in 0..400 {
            if checked >= 30 {
                break;
            }
            let (x, y) = match sites.get(attempt) {
                Some(&p) => p,
                None => (rng.range(0.1, 0.9) * g.map_w_ft, rng.range(0.1, 0.9) * g.map_h_ft),
            };
            let cell = ex.t0.cell_ft;
            let (i, j) = ((x / cell) as usize, (y / cell) as usize);
            if (ex.t0.height.get(i, j) as f64) < g.map_h_ft * 0.0 + world_sea(&ex) {
                continue;
            }
            let size = g.tile_size_ft(g.max_level);
            let key = if parks.contains(&attempt) {
                TileKey::surface(g.max_level, ((x / size).round() as u32).saturating_sub(1), ((y / size).round() as u32).saturating_sub(1))
            } else {
                TileKey::surface(g.max_level, (x / size) as u32, (y / size) as u32)
            };
            let c = ex.battlemap(key);
            checked += 1;
            if attempt < sites.len() {
                assert!(c.building.iter().any(|&b| b != 0), "seed {seed}: settlement chunk {key:?} has no buildings");
            }
            if seams.contains(&attempt) {
                // Seamless chunks: this chunk's east and south halo rows are the neighbours' first,
                // bit for bit (heights and water).
                use worldgen::battlemap::HS;
                let east = ex.battlemap(TileKey::surface(key.level, key.x + 1, key.y));
                let south = ex.battlemap(TileKey::surface(key.level, key.x, key.y + 1));
                for j in 0..SQ {
                    assert_eq!(c.halo_height[(j + 1) * HS + SQ + 1].to_bits(), east.height[j * SQ].to_bits(), "seed {seed} {key:?}: battlemap seam at row {j}");
                    assert_eq!(c.halo_water[(j + 1) * HS + SQ + 1].to_bits(), east.water_level[j * SQ].to_bits(), "seed {seed} {key:?}: water seam at row {j}");
                    assert_eq!(c.halo_height[(SQ + 1) * HS + j + 1].to_bits(), south.height[j].to_bits(), "seed {seed} {key:?}: battlemap seam at column {j}");
                }
            }
            biomes_seen.insert(ex.t0.biome.get(i, j) & 0xff);

            let dry: Vec<bool> = (0..SQ * SQ).map(|k| c.water_level[k] <= c.height[k] && c.surface[k] != battlemap::Surface::Planks).collect();
            // Water of any depth is exempt: the guarantee is for ground you fight on.
            let wet: Vec<bool> = (0..SQ * SQ).map(|k| c.water_level[k] > c.height[k]).collect();
            let dist = battlemap::cover_distance(&c);
            for k in 0..SQ * SQ {
                assert!(!dry[k] || dist[k] <= battlemap::cover_gap(&c, k), "seed {seed} {key:?}: square {k} is {} squares from cover", dist[k]);
            }
            for wy in 0..=SQ - W {
                for wx in 0..=SQ - W {
                    let in_w = |ox: f32, oy: f32| ox >= wx as f32 && ox < (wx + W) as f32 && oy >= wy as f32 && oy < (wy + W) as f32;
                    let mut wet_n = 0;
                    let (mut tmin, mut tmax) = (i16::MAX, i16::MIN);
                    for yy in wy..wy + W {
                        for xx in wx..wx + W {
                            let k = yy * SQ + xx;
                            wet_n += wet[k] as usize;
                            if dry[k] {
                                tmin = tmin.min(c.tier[k]);
                                tmax = tmax.max(c.tier[k]);
                            }
                        }
                    }
                    if wet_n * 2 >= W * W {
                        continue;
                    }
                    // Cover objects, and buildings (their walls) in the window.
                    let mut blds = std::collections::BTreeSet::new();
                    for yy in wy..wy + W {
                        for xx in wx..wx + W {
                            if c.building[yy * SQ + xx] != 0 {
                                blds.insert(c.building[yy * SQ + xx]);
                            }
                        }
                    }
                    let cover = blds.len() + c.objects.iter().filter(|o| battlemap::info(o.kind as u16).cover >= 1 && in_w(o.x, o.y)).count();
                    let feature = c.atmosphere != battlemap::Atmosphere::None
                        || c.objects.iter().any(|o| battlemap::info(o.kind as u16).feature && in_w(o.x, o.y));
                    assert!(cover >= 3 || battlemap::works_cover(&c, wx, wy), "seed {seed} {key:?} window ({wx},{wy}): only {cover} cover objects");
                    assert!(tmax > tmin, "seed {seed} {key:?} window ({wx},{wy}): single elevation tier");
                    assert!(feature, "seed {seed} {key:?} window ({wx},{wy}): no hazard, prop or atmosphere");
                }
            }
            if parks.contains(&attempt) || middles.contains(&attempt) {
                park_reachable(&mut ex, key, seed);
            }
        }
        assert!(checked >= 20, "seed {seed}: only {checked} land chunks found");
    }
    assert!(biomes_seen.len() >= 4, "too few biomes sampled: {biomes_seen:?}");
}

/// Every open square of the parks in chunk `key` can be walked to from the ground round the parks or
/// the edge of the 3x3 chunks round it (square to square, not corner to corner, across chunk edges),
/// with what blocks movement as play mode has it: buildings, walls and towers, and objects (a hedge
/// every square its piece is drawn across, sampled along its turn; anything else the squares within
/// half its footprint), each only in its own chunk.
fn park_reachable(ex: &mut Executor, key: TileKey, seed: u32) {
    use worldgen::battlemap::{self, Kind, SQ, URBAN_TOWER, URBAN_WALL};
    const N: usize = 3 * SQ;
    let (ox, oy) = ex.world.geom.tile_origin_ft(&TileKey::surface(key.level, key.x - 1, key.y - 1));
    let size = N as f64 * battlemap::SQUARE_FT;
    let parks: Vec<Vec<[f64; 2]>> = worldgen::town::layouts_near(&ex.world, &ex.t0, [ox, oy, ox + size, oy + size]).iter().flat_map(|l| l.parks.iter().map(|p| p.pts.clone()).collect::<Vec<_>>()).collect();
    let centre = |k: usize| [ox + ((k % N) as f64 + 0.5) * battlemap::SQUARE_FT, oy + ((k / N) as f64 + 0.5) * battlemap::SQUARE_FT];
    let in_park: Vec<bool> = (0..N * N).map(|k| parks.iter().any(|p| worldgen::town::geom::contains(p, centre(k)))).collect();
    let mut block = vec![false; N * N];
    for (cj, ci) in (0..3).flat_map(|j| (0..3).map(move |i| (j, i))) {
        let c = ex.battlemap(TileKey::surface(key.level, key.x - 1 + ci as u32, key.y - 1 + cj as u32));
        let at = |i: usize, j: usize| (cj * SQ + j) * N + ci * SQ + i;
        for k in 0..SQ * SQ {
            if c.building[k] != 0 || matches!(c.urban[k], URBAN_WALL | URBAN_TOWER) {
                block[at(k % SQ, k / SQ)] = true;
            }
        }
        // (Each chunk's objects block its own squares only.)
        let mut set = |i: i64, j: i64| {
            if i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 {
                block[at(i as usize, j as usize)] = true;
            }
        };
        for o in &c.objects {
            let inf = c.info(o);
            if !inf.blocks_move {
                continue;
            }
            let (x, y) = (o.x as f64, o.y as f64);
            if o.sprite == 0 && o.kind == Kind::Hedge {
                let (cs, sn) = (libm::cos(o.rot as f64), libm::sin(o.rot as f64));
                let (hl, hw) = (0.49 * o.scale as f64, 0.29 * o.scale as f64);
                let mut u = -hl;
                while u <= hl + 1e-6 {
                    let mut w = -hw;
                    while w <= hw + 1e-6 {
                        set((x + cs * u - sn * w).floor() as i64, (y + sn * u + cs * w).floor() as i64);
                        w += 0.1f64.min(hw);
                    }
                    u += 0.1f64.min(hl);
                }
                continue;
            }
            let reach = (inf.radius as f64 * o.scale as f64 * 0.5).max(0.5);
            for j in (y - reach).floor() as i64..=(y + reach).floor() as i64 {
                for i in (x - reach).floor() as i64..=(x + reach).floor() as i64 {
                    if (i as f64 + 0.5 - x) * (i as f64 + 0.5 - x) + (j as f64 + 0.5 - y) * (j as f64 + 0.5 - y) <= reach * reach {
                        set(i, j);
                    }
                }
            }
        }
    }
    let mut seen = vec![false; N * N];
    let mut queue = std::collections::VecDeque::new();
    for k in 0..N * N {
        let (x, y) = (k % N, k / N);
        if !block[k] && (!in_park[k] || x == 0 || y == 0 || x == N - 1 || y == N - 1) {
            seen[k] = true;
            queue.push_back(k);
        }
    }
    while let Some(k) = queue.pop_front() {
        let (x, y) = ((k % N) as i64, (k / N) as i64);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && nx < N as i64 && ny < N as i64 {
                let n = ny as usize * N + nx as usize;
                if !seen[n] && !block[n] {
                    seen[n] = true;
                    queue.push_back(n);
                }
            }
        }
    }
    let sealed: Vec<usize> = (0..N * N).filter(|&k| in_park[k] && !block[k] && !seen[k]).collect();
    assert!(sealed.is_empty(), "seed {seed} {key:?}: {} park squares can't be reached, the first at {:?}", sealed.len(), sealed.first().map(|&k| centre(k)));
}

fn world_sea(ex: &Executor) -> f64 {
    ex.world.params().sea_level_ft
}
