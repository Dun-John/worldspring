//! Determinism, rivers, tile seams, zoom consistency, road grades, settlements on water
//! dwarven crag seams and waterfalls.

use super::*;

/// Same world file → same bytes, and a T0 loaded from bytes (as every worker but one does)
/// is the same T0. Cross-platform (WASM vs native) is `scripts/det-wasm.mjs`.
#[test]
fn determinism_repeatable() {
    let json = world_json(7);
    assert_eq!(det_report(&json).unwrap(), det_report(&json).unwrap());
    let world = World::from_json(&json).unwrap();
    let bytes = worldgen::t0::T0::generate(&world).to_bytes();
    assert!(worldgen::t0::T0::from_bytes(&bytes).unwrap().to_bytes() == bytes, "T0 bytes round trip differs");
}

/// Terrain logic: every river flows strictly downhill from source to mouth, and ends in the
/// sea, a lake, a confluence, or dries out (never climbs through terrain). On a small map too
/// (its cells an eighth the size, its volcanoes' gullies many cells long).
#[test]
fn rivers_flow_downhill() {
    let small = serde_json::to_string(&serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 99, "params": { "width_mi": 200.0, "height_mi": 150.0, "volcanoes": 3 } })).unwrap();
    for (seed, json) in [(3u32, world_json(3)), (99, world_json(99)), (99, small)] {
        let world = World::from_json(&json).unwrap();
        let t0 = worldgen::t0::T0::generate(&world);
        let extra = t0.extra.as_ref().unwrap();
        let hydro = &extra.hydro;
        let sea = world.params().sea_level_ft as f32;
        let mut checked = 0;
        for r in &hydro.rivers {
            for pair in r.cells.windows(2) {
                let (a, b) = (pair[0] as usize, pair[1] as usize);
                let (ha, hb) = (t0.height.data[a], t0.height.data[b]);
                let into_water = hb <= sea || hydro.lake_of[b] != worldgen::t0::hydro::NO_LAKE;
                assert!(into_water || hb < ha, "seed {seed}: river climbs from {ha} ft to {hb} ft");
                checked += 1;
            }
        }
        assert!(checked > 1000, "seed {seed}: too few river cells ({checked})");
        // The fine river curve is continuous: sampled densely, no step jumps sideways
        // (noise racing where the width changes once made 300 ft zigzags).
        for (ri, r) in t0.rivers.rivers.iter().enumerate() {
            for k in 0..r.segments() {
                let seg = r.s[k + 1] - r.s[k];
                let mut prev = r.eval(k, 0.0, 2.5, t0.cell_ft).p;
                for j in 1..=64 {
                    let p = r.eval(k, j as f64 / 64.0, 2.5, t0.cell_ft).p;
                    let step = ((p[0] - prev[0]).powi(2) + (p[1] - prev[1]).powi(2)).sqrt();
                    assert!(step < 3.0 * seg / 64.0 + 40.0, "seed {seed}: river {ri} segment {k} jumps {step:.0} ft");
                    prev = p;
                }
            }
        }
    }
}

/// A point on the biggest river (by discharge), mid-course: seams and consistency are
/// checked where rivers carve, not just on open ground.
fn big_river_point(ex: &Executor) -> (f64, f64) {
    let r = ex.t0.rivers.rivers.iter().max_by(|a, b| a.q.last().unwrap().total_cmp(b.q.last().unwrap())).unwrap();
    let k = r.segments() / 2;
    let p = r.eval(k, 0.5, 2.5, ex.t0.cell_ft);
    (p.p[0], p.p[1])
}

/// Neighboring tiles agree bit-for-bit on shared edges and halos at every kind of level
/// (T0-sampled, first refined, deep refined, finest), including where rivers are carved,
/// so seams can never appear.
#[test]
fn tile_seams_consistent() {
    let world = World::from_json(&world_json(11)).unwrap();
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let (rx, ry) = big_river_point(&ex);
    let mut levels = vec![g.first_refine_level.saturating_sub(1), g.first_refine_level, g.first_refine_level + 4, g.max_level - 2, g.max_level];
    levels.dedup();
    let (h, n) = (HALO as i64, TILE_N as i64);
    for level in levels {
        let size = g.tile_size_ft(level);
        let (x, y) = ((rx / size) as u32, (ry / size) as u32);
        let a = ex.terrain(TileKey::surface(level, x, y)).padded.clone();
        let right = ex.terrain(TileKey::surface(level, x + 1, y)).padded.clone();
        let below = ex.terrain(TileKey::surface(level, x, y + 1)).padded.clone();
        for j in -h..=n + h {
            for d in -h..=h {
                assert_eq!(
                    a[padded_index(n + d, j)].to_bits(),
                    right[padded_index(d, j)].to_bits(),
                    "x seam at level {level}, d {d}, j {j}"
                );
                assert_eq!(
                    a[padded_index(j, n + d)].to_bits(),
                    below[padded_index(j, d)].to_bits(),
                    "y seam at level {level}, d {d}, i {j}"
                );
            }
        }
    }
}

/// Zoom consistency: a child tile keeps its parent's shape (mean height over the same area),
/// and at every fine level a river's curve is visible water sitting in a carved channel.
#[test]
fn zoom_consistency() {
    let world = World::from_json(&world_json(5)).unwrap();
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let (rx, ry) = big_river_point(&ex);
    let n = TILE_N as i64;

    for (fx, fy) in [(0.4, 0.45), (0.6, 0.5)] {
        for level in g.first_refine_level..g.max_level {
            let size = g.tile_size_ft(level);
            let (x, y) = ((fx * g.map_w_ft / size) as u32, (fy * g.map_h_ft / size) as u32);
            let parent = ex.terrain(TileKey::surface(level, x, y)).padded.clone();
            let child = ex.terrain(TileKey::surface(level + 1, 2 * x, 2 * y)).padded.clone();
            let mean = |t: &[f32], lo: i64, hi: i64| {
                let (mut sum, mut cnt) = (0.0f64, 0.0);
                for j in lo..=hi {
                    for i in lo..=hi {
                        sum += t[padded_index(i, j)] as f64;
                        cnt += 1.0;
                    }
                }
                sum / cnt
            };
            let (pm, cm) = (mean(&parent, 0, n / 2), mean(&child, 0, n));
            let relief = parent.iter().fold(f32::MIN, |a, &b| a.max(b)) - parent.iter().fold(f32::MAX, |a, &b| a.min(b));
            let tol = 5.0 + 0.02 * relief as f64;
            assert!((pm - cm).abs() <= tol, "level {level}->{}: parent mean {pm:.1} vs child {cm:.1} (tol {tol:.1})", level + 1);
        }
    }

    for level in (g.max_level - 4)..=g.max_level {
        let s = g.spacing_ft(level);
        let size = g.tile_size_ft(level);
        let key = TileKey::surface(level, (rx / size) as u32, (ry / size) as u32);
        let (ox, oy) = g.tile_origin_ft(&key);
        let tile = ex.terrain(key);
        let (i, j) = (((rx - ox) / s).round() as i64, ((ry - oy) / s).round() as i64);
        let k = padded_index(i, j);
        let (h, water) = (tile.padded[k], tile.river_water[k]);
        assert!(water > -20_000.0, "level {level}: no river water on the river's curve");
        assert!(h < water, "level {level}: river bed {h} ft is not below its water {water} ft");
    }
}

/// Roads: every road's surface profile stays within its class's maximum grade, and at the
/// finest level the terrain under the road centreline is the road bed (cut and fill applied),
/// so the grade holds on the ground too. Checked on the steepest road stretch of the world.
#[test]
fn roads_within_grade() {
    let world = World::from_json(&world_json(1)).unwrap();
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let mut steepest = (0.0f64, 0usize, 0usize);
    let mut checked = 0;
    for (ri, r) in ex.t0.roads.roads.iter().enumerate() {
        let gmax = r.class.max_grade();
        for k in 1..r.pts.len() {
            let d = ((r.pts[k][0] - r.pts[k - 1][0]).powi(2) + (r.pts[k][1] - r.pts[k - 1][1]).powi(2)).sqrt();
            let grade = (r.z[k] - r.z[k - 1]).abs() as f64 / d.max(1e-6);
            assert!(grade <= gmax * 1.001, "road {ri} ({:?}) segment {k}: grade {grade:.3} > {gmax}", r.class);
            // (Not a road's end segments: there its bed blends into the junction's or gate's.)
            if d > 200.0 && grade > steepest.0 && k >= 2 && k + 1 < r.pts.len() {
                steepest = (grade, ri, k);
            }
            checked += 1;
        }
    }
    assert!(checked > 1000, "too few road segments ({checked})");

    let (_, ri, k) = steepest;
    let level = g.max_level;
    let s = g.spacing_ft(level);
    let size = g.tile_size_ft(level);
    for step in 0..=8 {
        let t = 0.25 + 0.5 * step as f64 / 8.0;
        let p = ex.t0.roads.roads[ri].eval(k - 1, t, s, ex.t0.cell_ft);
        let (x, y, z) = (p.p[0], p.p[1], p.z);
        let key = TileKey::surface(level, (x / size) as u32, (y / size) as u32);
        let (ox, oy) = g.tile_origin_ft(&key);
        let tile = ex.terrain(key);
        let (i, j) = (((x - ox) / s).round() as i64, ((y - oy) / s).round() as i64);
        let h = tile.padded[padded_index(i, j)] as f64;
        // Nearest sample is ≤ 1.8 ft off the centreline; the bed is flat across.
        assert!((h - z).abs() < 1.0, "fine terrain {h:.1} ft is off the road bed {z:.1} ft");
    }

    // The battlemap there shows the road, and nothing that blocks movement stands on it.
    use worldgen::battlemap::{SQ, info};
    let p = ex.t0.roads.roads[ri].eval(k - 1, 0.5, s, ex.t0.cell_ft);
    let key = TileKey::surface(level, (p.p[0] / size) as u32, (p.p[1] / size) as u32);
    let chunk = ex.battlemap(key);
    let on_road = |x: f32, y: f32| {
        let (i, j) = (x.floor() as usize, y.floor() as usize);
        i < SQ && j < SQ && chunk.surface[j * SQ + i].is_road()
    };
    let road_squares = chunk.surface.iter().filter(|s| s.is_road()).count();
    assert!(road_squares >= SQ / 2, "battlemap on a road has only {road_squares} road squares");
    let blocking = chunk.objects.iter().filter(|o| info(o.kind as u16).blocks_move && on_road(o.x, o.y)).count();
    assert_eq!(blocking, 0, "{blocking} movement-blocking objects stand on the road");
}

/// Ports and fishing settlements stand on their water: the sea, a lake or a river is within
/// half their built-up radius (plus 300 ft) of the centre, not a map cell (~1.2 mi) inland.
#[test]
fn settlements_on_their_water() {
    use worldgen::t0::settle::SettleKind;
    let world = World::from_json(&world_json(1)).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let mut checked = 0;
    for (i, s) in t0.settlements.iter().enumerate() {
        if !matches!(s.kind, SettleKind::Port | SettleKind::Fishing) {
            continue;
        }
        let limit = 0.5 * worldgen::town::urban_radius(s.tier, s.population) + 300.0;
        let mut near = false;
        'rays: for a in 0..64 {
            let ang = std::f64::consts::TAU * a as f64 / 64.0;
            let mut t = 0.0;
            while t <= limit {
                let (x, y) = (s.x + ang.cos() * t, s.y + ang.sin() * t);
                // Water only where its level is above the ground the terrain draws.
                if t0.sample_water(x, y) as f64 > t0.ground_at(x, y, world.geom.spacing_ft(world.geom.first_refine_level - 1)) {
                    near = true;
                    break 'rays;
                }
                t += 20.0;
            }
        }
        if !near {
            for (ri, k) in t0.rivers.segments_near(s.x - limit, s.y - limit, s.x + limit, s.y + limit, 0.0) {
                let r = &t0.rivers.rivers[ri as usize];
                for j in 0..=32 {
                    let c = r.eval(k as usize, j as f64 / 32.0, 20.0, t0.cell_ft);
                    if ((c.p[0] - s.x).powi(2) + (c.p[1] - s.y).powi(2)).sqrt() <= limit + 0.5 * c.w {
                        near = true;
                    }
                }
            }
        }
        assert!(near, "settlement {i} ({:?} {:?}) is not within {limit:.0} ft of water", s.tier, s.kind);
        checked += 1;
    }
    assert!(checked > 30, "too few waterside settlements ({checked})");
}

/// A dwarven hall's gate on flat ground stands in a crag the battlemap raises (`under::surface`):
/// the chunks round it still meet bit for bit, and the crag's face behind the gate is a cliff.
#[test]
fn dwarven_crag_seams() {
    use worldgen::battlemap::{HS, SQ, SQUARE_FT};
    use worldgen::under::surface::{Crag, STYLE_RUNE_GATE, entrance_style};
    let mut found = 0;
    for seed in [1u32, 3] {
        let world = World::from_json(&world_json(seed)).unwrap();
        let size = world.geom.tile_size_ft(world.geom.max_level);
        let level = world.geom.max_level;
        let mut ex = Executor::new(world);
        let n = ex.t0.settlements.len();
        let gates: Vec<_> = (n..n + ex.t0.pois.len())
            .flat_map(|i| {
                let l = worldgen::town::layout(&ex.world, &ex.t0, i);
                l.entrances.iter().filter(|e| entrance_style(&ex.world, &ex.t0, &l, e) == Some(STYLE_RUNE_GATE) && Crag::new(&ex.t0, e).is_some()).copied().collect::<Vec<_>>()
            })
            .collect();
        for e in gates {
            found += 1;
            let (kx, ky) = ((e.at[0] / size) as u32, (e.at[1] / size) as u32);
            let c = ex.battlemap(TileKey::surface(level, kx, ky));
            let seam = |a: &worldgen::battlemap::Chunk, east: &worldgen::battlemap::Chunk, south: &worldgen::battlemap::Chunk| {
                for j in 0..SQ {
                    assert_eq!(a.halo_height[(j + 1) * HS + SQ + 1].to_bits(), east.height[j * SQ].to_bits(), "seed {seed}: crag seam at row {j}");
                    assert_eq!(a.halo_height[(SQ + 1) * HS + j + 1].to_bits(), south.height[j].to_bits(), "seed {seed}: crag seam at column {j}");
                }
            };
            let (east, south) = (ex.battlemap(TileKey::surface(level, kx + 1, ky)), ex.battlemap(TileKey::surface(level, kx, ky + 1)));
            seam(&c, &east, &south);
            let (west, north) = (ex.battlemap(TileKey::surface(level, kx - 1, ky)), ex.battlemap(TileKey::surface(level, kx, ky - 1)));
            seam(&west, &c, &ex.battlemap(TileKey::surface(level, kx - 1, ky + 1)));
            seam(&north, &ex.battlemap(TileKey::surface(level, kx + 1, ky - 1)), &c);
            // Cliff ink between the opening and three squares behind it, across the gate.
            let mut cliff = false;
            for a in 0..=3 {
                for b in -1i32..=1 {
                    let p = [e.at[0] + e.dir[0] * a as f64 * SQUARE_FT - e.dir[1] * b as f64 * SQUARE_FT, e.at[1] + e.dir[1] * a as f64 * SQUARE_FT + e.dir[0] * b as f64 * SQUARE_FT];
                    let (cx, cy) = ((p[0] / size) as u32, (p[1] / size) as u32);
                    let k = ((p[1] - cy as f64 * size) / SQUARE_FT) as usize * SQ + ((p[0] - cx as f64 * size) / SQUARE_FT) as usize;
                    let ch = ex.battlemap(TileKey::surface(level, cx, cy));
                    cliff |= ch.edges[k] & 3 != 0 || (k % SQ > 0 && ch.edges[k - 1] & 1 != 0) || (k >= SQ && ch.edges[k - SQ] & 2 != 0);
                }
            }
            assert!(cliff, "seed {seed}: no cliff behind the dwarven gate at {:?}", e.at);
        }
    }
    assert!(found > 0, "no dwarven hall gate with a crag in seeds 1 and 3");
}

// R5 underground

/// Waterfalls: the same on a T0 loaded from bytes; at the finest level the river's water stands
/// at the fall's top above the lip and at its bottom below the face (the drop is on the ground),
/// every tile round the lip's carving and its shelf (and the lip's at the next coarser level) meets
/// its neighbors bit-for-bit, the cave behind a fall can be walked into from its chunk's edge (dry
/// squares, no step over 5 ft), and no feature players see tells of a cave.
#[test]
fn waterfalls_drop_and_cave_reachable() {
    // (Two worlds' first eight caves: each one reached by its shelf, `rivers::ledge`.)
    for seed in [1, 3] {
        let world = World::from_json(&world_json(seed)).unwrap();
        let g = world.geom.clone();
        let bytes = worldgen::t0::T0::generate(&world).to_bytes();
        let mut ex = Executor::new(world);
        let falls = ex.t0.rivers.falls.clone();
        assert!(!falls.is_empty(), "no waterfalls");
        let features = &ex.t0.extra.as_ref().unwrap().overlay.features;
        assert!(features.iter().filter(|f| !f.secret).all(|f| !f.detail.as_deref().unwrap_or("").contains("cave")), "a feature players see tells of a cave");
        assert_eq!(format!("{:?}", worldgen::t0::T0::from_bytes(&bytes).unwrap().rivers.falls), format!("{falls:?}"), "falls differ after load");
        let level = g.max_level;
        let (s, size, n, h) = (g.spacing_ft(level), g.tile_size_ft(level), TILE_N as i64, HALO as i64);
        let picks: Vec<_> = falls.iter().filter(|f| f.cave.is_some()).take(8).chain(falls.iter().filter(|f| f.cave.is_none()).take(1)).copied().collect();
        for f in picks {
            let at = |d: f64| [f.at[0] + f.dir[0] * d, f.at[1] + f.dir[1] * d];
            let mut water = |p: [f64; 2]| {
                let key = TileKey::surface(level, (p[0] / size) as u32, (p[1] / size) as u32);
                let (ox, oy) = g.tile_origin_ft(&key);
                let t = ex.terrain(key);
                let k = padded_index(((p[0] - ox) / s).round() as i64, ((p[1] - oy) / s).round() as i64);
                (t.padded[k], t.river_water[k])
            };
            let (bed, above) = water(at(-15.0));
            let (_, below) = water(at(f.face_ft as f64 + 30.0));
            assert!(bed < above && (above - f.top).abs() < 25.0, "fall {}: water above the lip {above} (bed {bed}), top {}", f.id, f.top);
            assert!(below - f.bottom < 25.0 && above - below > 0.6 * f.drop_ft(), "fall {}: water below {below}, above {above}, drop {}", f.id, f.drop_ft());
            // (The tiles the lip's kept bed, its pool and its shelf reach, each with the tile to its
            // right and below; then the lip's tile one level up.)
            let r = 120.0 + 3.0 * f.width as f64 + 2.0 * f.face_ft as f64 + 100.0;
            let span = |c: f64, sz: f64| ((c - r).max(0.0) / sz) as u32..=((c + r) / sz) as u32;
            let mut seams = Vec::new();
            for y in span(f.at[1], size) {
                for x in span(f.at[0], size) {
                    seams.push((level, x, y));
                }
            }
            let up = g.tile_size_ft(level - 1);
            seams.push((level - 1, (f.at[0] / up) as u32, (f.at[1] / up) as u32));
            for (lv, x, y) in seams {
                let a = ex.terrain(TileKey::surface(lv, x, y)).padded.clone();
                let right = ex.terrain(TileKey::surface(lv, x + 1, y)).padded.clone();
                let down = ex.terrain(TileKey::surface(lv, x, y + 1)).padded.clone();
                for j in -h..=n + h {
                    for d in -h..=h {
                        assert_eq!(a[padded_index(n + d, j)].to_bits(), right[padded_index(d, j)].to_bits(), "fall {}: x seam at {lv}/{x}/{y}", f.id);
                        assert_eq!(a[padded_index(j, n + d)].to_bits(), down[padded_index(j, d)].to_bits(), "fall {}: y seam at {lv}/{x}/{y}", f.id);
                    }
                }
            }
            let Some(c) = f.cave else { continue };
            let key = TileKey::surface(level, (c[0] / size) as u32, (c[1] / size) as u32);
            let (ox, oy) = g.tile_origin_ft(&key);
            let ch = ex.battlemap(key);
            let sq = worldgen::battlemap::SQ;
            let ft = size / sq as f64;
            let start = (((c[1] - oy) / ft) as usize).min(sq - 1) * sq + (((c[0] - ox) / ft) as usize).min(sq - 1);
            let dry = |k: usize| ch.water_level[k] <= ch.height[k];
            assert!(dry(start), "fall {}: cave mouth under water", f.id);
            let mut seen = vec![false; sq * sq];
            let (mut stack, mut edge) = (vec![start], false);
            seen[start] = true;
            while let Some(k) = stack.pop() {
                let (i, j) = (k % sq, k / sq);
                edge |= i == 0 || j == 0 || i + 1 == sq || j + 1 == sq;
                let next = [(i + 1 < sq).then(|| k + 1), (i > 0).then(|| k - 1), (j + 1 < sq).then(|| k + sq), (j > 0).then(|| k - sq)];
                for q in next.into_iter().flatten() {
                    if !seen[q] && dry(q) && (ch.height[q] - ch.height[k]).abs() <= 5.0 {
                        seen[q] = true;
                        stack.push(q);
                    }
                }
            }
            assert!(edge, "fall {}: cave at ({:.0}, {:.0}) can't be walked to from its chunk's edge", f.id, c[0], c[1]);
        }
    }
}
