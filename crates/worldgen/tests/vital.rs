//! The only test suite: vital requirements, not per-module coverage.
//! Run with `cargo test --release -p worldgen`.

use worldgen::core::tile::{HALO, TILE_N, TileKey};
use worldgen::lod::terrain_refine::padded_index;
use worldgen::pipeline::{Executor, det_report};
use worldgen::{World, WorldFile};

fn world_json(seed: u32) -> String {
    serde_json::to_string(&WorldFile { seed, ..Default::default() }).unwrap()
}

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
/// sea, a lake, a confluence, or dries out (never climbs through terrain).
#[test]
fn rivers_flow_downhill() {
    for seed in [3u32, 99] {
        let world = World::from_json(&world_json(seed)).unwrap();
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
            if d > 200.0 && grade > steepest.0 {
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

/// Towns meet their roads and water: every road that ends at a town or city continues from
/// its end to a gate; every pier starts on land (dry ground within 20 ft of one end) and
/// stands over water.
#[test]
fn towns_meet_roads_and_water() {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, geom};
    let mut ex = Executor::new(World::from_json(&world_json(1)).unwrap());
    let (world, t0) = (&ex.world, &ex.t0);
    let wet = |p: [f64; 2]| {
        if t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], world.geom.spacing_ft(world.geom.first_refine_level - 1)) {
            return true;
        }
        // River channel: sample the curve every ~20 ft (segments can be over a mile long).
        t0.rivers.segments_near(p[0] - 400.0, p[1] - 400.0, p[0] + 400.0, p[1] + 400.0, 0.0).iter().any(|&(ri, k)| {
            let r = &t0.rivers.rivers[ri as usize];
            let n = ((r.s[k as usize + 1] - r.s[k as usize]) / 20.0).ceil().max(8.0) as usize;
            (0..=n).any(|j| {
                let c = r.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft);
                geom::dist(c.p, p) < 0.5 * c.w
            })
        })
    };
    // The river channels round a box (the fine curve every ~20 ft), by 100-ft grid cell.
    type Channel = (Vec<([f64; 2], [f64; 2], f64)>, std::collections::HashMap<(i64, i64), Vec<usize>>);
    let channel = |bb: [f64; 4]| -> Channel {
        let mut segs = Vec::new();
        for (ri, k) in t0.rivers.segments_near(bb[0] - 400.0, bb[1] - 400.0, bb[2] + 400.0, bb[3] + 400.0, 0.0) {
            let r = &t0.rivers.rivers[ri as usize];
            let n = ((r.s[k as usize + 1] - r.s[k as usize]) / 20.0).ceil().max(8.0) as usize;
            let pts: Vec<_> = (0..=n).map(|j| r.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft)).collect();
            segs.extend(pts.windows(2).map(|w| (w[0].p, w[1].p, 0.5 * w[0].w.min(w[1].w))));
        }
        let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
        for (i, &(a, b, hw)) in segs.iter().enumerate() {
            for cy in ((a[1].min(b[1]) - hw) / 100.0).floor() as i64..=((a[1].max(b[1]) + hw) / 100.0).floor() as i64 {
                for cx in ((a[0].min(b[0]) - hw) / 100.0).floor() as i64..=((a[0].max(b[0]) + hw) / 100.0).floor() as i64 {
                    grid.entry((cx, cy)).or_default().push(i);
                }
            }
        }
        (segs, grid)
    };
    // More than `inset` ft inside a channel.
    let in_channel = |ch: &Channel, p: [f64; 2], inset: f64| {
        ch.1.get(&((p[0] / 100.0).floor() as i64, (p[1] / 100.0).floor() as i64)).into_iter().flatten().any(|&i| {
            let (a, b, hw) = ch.0[i];
            geom::seg_dist(p, a, b) < hw - inset
        })
    };
    let (mut roads_checked, mut piers_checked, mut walls_checked) = (0, 0, 0);
    // A pier is a rectangle; its ends are the midpoints of its two short sides.
    let ends = |pier: &[[f64; 2]]| {
        let m = pier.len();
        let mut sides: Vec<(f64, [f64; 2])> = (0..m).map(|k| (geom::dist(pier[k], pier[(k + 1) % m]), geom::lerp(pier[k], pier[(k + 1) % m], 0.5))).collect();
        sides.sort_by(|a, b| a.0.total_cmp(&b.0));
        (sides[0].1, sides[1].1)
    };
    // Every pier's ends (villages' too), for the drawn shore below.
    let mut all_piers: Vec<(usize, [f64; 2], [f64; 2])> = Vec::new();
    for (i, s) in t0.settlements.iter().enumerate() {
        let l = town::layout(world, t0, i);
        all_piers.extend(l.piers.iter().map(|p| (i, ends(p).0, ends(p).1)));
        if s.tier < Tier::Town {
            continue;
        }
        // Walls end at the banks (water guards the gap), towers stand on land, and every street
        // over a river runs on a bridge.
        let ch = channel(l.bbox);
        if !ch.0.is_empty() {
            for w in &l.walls {
                for seg in w.windows(2) {
                    let n = (geom::dist(seg[0], seg[1]) / 10.0).ceil().max(1.0) as usize;
                    for j in 0..=n {
                        let p = geom::lerp(seg[0], seg[1], j as f64 / n as f64);
                        assert!(!in_channel(&ch, p, 2.0), "settlement {i}: wall in the river at {p:?}");
                    }
                }
                walls_checked += 1;
            }
            for t in l.towers.iter().chain(&l.gate_towers) {
                assert!(!in_channel(&ch, *t, 2.0), "settlement {i}: tower in the river at {t:?}");
            }
            for (pts, class, _) in &l.roads {
                for seg in pts.windows(2) {
                    let n = (geom::dist(seg[0], seg[1]) / 10.0).ceil().max(1.0) as usize;
                    for j in 0..=n {
                        let p = geom::lerp(seg[0], seg[1], j as f64 / n as f64);
                        assert!(!in_channel(&ch, p, 2.0) || l.bridges.iter().any(|b| geom::contains(b, p)), "settlement {i}: a class {class} street over the river with no bridge at {p:?}");
                    }
                }
            }
        }
        for (ri, rc) in t0.roads.roads.iter().enumerate() {
            let n = rc.pts.len();
            for (k, t) in [(0usize, 0.0), (n - 2, 1.0)] {
                let e = rc.eval(k, t, 5.0, t0.cell_ft).p;
                if geom::dist(e, [s.x, s.y]) > 1.05 * worldgen::town::road_trim_radius(s.tier, s.population) + 50.0 {
                    continue;
                }
                // A street from the road's end to a gate (in two pieces where it turns into a
                // town street on the way).
                let to_gate = |p: [f64; 2]| l.gates.iter().any(|g| geom::dist(*g, p) < 1.0);
                let joined = l.roads.iter().any(|(pts, _, _)| {
                    let end = *pts.last().unwrap();
                    geom::dist(pts[0], e) < 1.0 && (to_gate(end) || l.roads.iter().any(|(q, _, _)| geom::dist(q[0], end) < 1e-6 && to_gate(*q.last().unwrap())))
                }) || to_gate(e);
                assert!(joined, "settlement {i}: road {ri} ends {:.0} ft from a gate with no street to it", l.gates.iter().map(|g| geom::dist(*g, e)).fold(f64::MAX, f64::min));
                roads_checked += 1;
            }
        }
        for pier in &l.piers {
            let (e1, e2) = ends(pier);
            let near_land = |e: [f64; 2]| (0..16).any(|a| {
                let ang = std::f64::consts::TAU * a as f64 / 16.0;
                (0..=4).any(|d| !wet([e[0] + ang.cos() * 5.0 * d as f64, e[1] + ang.sin() * 5.0 * d as f64]))
            });
            assert!(near_land(e1) || near_land(e2), "settlement {i}: a pier at {:?} does not touch land", geom::centroid(pier));
            assert!(wet(geom::centroid(pier)), "settlement {i}: a pier at {:?} is not over water", geom::centroid(pier));
            piers_checked += 1;
        }
    }
    assert!(roads_checked > 20 && piers_checked > 10 && walls_checked > 10, "too little checked: {roads_checked} road ends, {piers_checked} piers, {walls_checked} walls by rivers");
    // Piers reach the water the battlemap draws, not just the water their layout was planned
    // by: under a settlement the terrain is the ground it was planned on.
    let g = ex.world.geom.clone();
    let (s, size) = (g.spacing_ft(g.max_level), g.tile_size_ft(g.max_level));
    let mut over_water = |p: [f64; 2]| {
        let key = TileKey::surface(g.max_level, (p[0] / size) as u32, (p[1] / size) as u32);
        let (ox, oy) = g.tile_origin_ft(&key);
        let k = padded_index(((p[0] - ox) / s).round() as i64, ((p[1] - oy) / s).round() as i64);
        let tile = ex.terrain(key);
        ex.t0.sample_water(p[0], p[1]).max(tile.river_water[k]) > tile.padded[k]
    };
    assert!(all_piers.len() > 200, "too few piers: {}", all_piers.len());
    for (i, e1, e2) in all_piers {
        assert!(over_water(e1) || over_water(e2), "settlement {i}: the pier at {:?} ends on drawn land", geom::lerp(e1, e2, 0.5));
    }
}

/// Settlements: every one gets its guaranteed buildings — an inn in every village, the full
/// catalog in a metropolis (water trades when on water) — checked on every settlement.
#[test]
fn settlement_catalog_guarantees() {
    use worldgen::town::{self, catalog::{CATALOG, required_count}};
    let world = World::from_json(&world_json(1)).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let inn = CATALOG.iter().position(|f| f.key == "inn").unwrap();
    assert!(t0.settlements.len() > 50, "too few settlements ({})", t0.settlements.len());
    for i in 0..t0.settlements.len() {
        let l = town::generate(&world, &t0, i);
        let mut have = vec![0usize; CATALOG.len()];
        for b in &l.buildings {
            if let Some(f) = b.func {
                have[f as usize] += 1;
            }
        }
        assert!(have[inn] >= 1, "settlement {i} ({:?}) has no inn", l.tier);
        for (fi, f) in CATALOG.iter().enumerate() {
            let need = required_count(f, l.tier, l.on_water);
            assert!(have[fi] >= need, "settlement {i} ({:?}) has {} of {} (needs {need})", l.tier, have[fi], f.name);
        }
    }
}

/// Battlemap guarantee: in sampled land chunks across seeds and biomes (and in town), every
/// 32×32-square window has ≥ 3 cover objects or buildings, ≥ 2 elevation tiers, and a hazard/prop or atmosphere; and no
/// dry square is more than MAX_COVER_GAP squares from cover. Mostly-water windows (any depth) and bridge
/// decks are exempt.
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
        {
            use worldgen::t0::settle::Tier;
            let st = &ex.t0.settlements;
            if let Some(s) = st.iter().max_by_key(|s| s.population) {
                sites.push((s.x, s.y));
            }
            if let Some(s) = st.iter().find(|s| s.tier == Tier::Village) {
                sites.push((s.x, s.y));
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
            let key = TileKey::surface(g.max_level, (x / size) as u32, (y / size) as u32);
            let c = ex.battlemap(key);
            checked += 1;
            if attempt < sites.len() {
                assert!(c.building.iter().any(|&b| b != 0), "seed {seed}: settlement chunk {key:?} has no buildings");
            }
            if checked == 1 {
                // Seamless chunks: this chunk's east halo column is the east neighbour's first
                // column, bit for bit (heights and water).
                use worldgen::battlemap::HS;
                let east = ex.battlemap(TileKey::surface(key.level, key.x + 1, key.y));
                for j in 0..SQ {
                    assert_eq!(c.halo_height[(j + 1) * HS + SQ + 1].to_bits(), east.height[j * SQ].to_bits(), "battlemap seam at row {j}");
                }
            }
            biomes_seen.insert(ex.t0.biome.get(i, j) & 0xff);

            let dry: Vec<bool> = (0..SQ * SQ).map(|k| c.water_level[k] <= c.height[k] && c.surface[k] != battlemap::Surface::Planks).collect();
            // Water of any depth is exempt: the guarantee is for ground you fight on.
            let wet: Vec<bool> = (0..SQ * SQ).map(|k| c.water_level[k] > c.height[k]).collect();
            let dist = battlemap::cover_distance(&c);
            for k in 0..SQ * SQ {
                assert!(!dry[k] || dist[k] <= battlemap::MAX_COVER_GAP, "seed {seed} {key:?}: square {k} is {} squares from cover", dist[k]);
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
                    assert!(cover >= 3, "seed {seed} {key:?} window ({wx},{wy}): only {cover} cover objects");
                    assert!(tmax > tmin, "seed {seed} {key:?} window ({wx},{wy}): single elevation tier");
                    assert!(feature, "seed {seed} {key:?} window ({wx},{wy}): no hazard, prop or atmosphere");
                }
            }
        }
        assert!(checked >= 20, "seed {seed}: only {checked} land chunks found");
    }
    assert!(biomes_seen.len() >= 4, "too few biomes sampled: {biomes_seen:?}");
}

fn world_sea(ex: &Executor) -> f64 {
    ex.world.params().sea_level_ft
}

/// Interiors (M5): every enterable building and wall/gate tower in a sample of settlements (the capital, a city,
/// a town, a village, a wizard's tower) opens with at least two levels, stairs on the same
/// squares on every level, one front door on the ground floor, no furniture on a doorway or
/// the stairs, and every door and room reachable from the front door across open floor
/// (stairs join the levels); inns, taverns, breweries and the like have a bar and at least two
/// tables on the ground floor. Each interior generates within the 300 ms budget.
#[test]
fn interiors_guarantee() {
    use worldgen::interior;
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, Structure};
    let world = World::from_json(&world_json(1)).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let by_tier = |t: Tier| (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier == t).max_by_key(|&i| t0.settlements[i].population);
    let mut picks: Vec<usize> = [Tier::Metropolis, Tier::City, Tier::Town, Tier::Village].iter().filter_map(|&t| by_tier(t)).collect();
    if let Some(p) = t0.pois.iter().position(|p| p.kind == worldgen::t0::settle::PoiKind::Tower) {
        picks.push(t0.settlements.len() + p);
    }
    let (mut checked, mut worst_ms, mut drinking, mut towers, mut cells) = (0, 0.0f64, 0, 0, 0);
    for si in picks {
        let l = town::layout(&world, &t0, si);
        // Buildings, then wall and gate towers.
        let ids: Vec<String> = (0..l.buildings.len())
            .filter(|&bi| l.buildings[bi].structure == Structure::Roofed)
            .map(|bi| format!("b:{si}:{bi}"))
            .chain((0..interior::towers(&l).len()).map(|k| format!("t:{si}:{k}")))
            .collect();
        towers += interior::towers(&l).len();
        let buckets = building_buckets(&l);
        for id in ids {
            let (ms, drinks, n) = check_interior(&world, &t0, &l, si, &id, &buckets);
            worst_ms = worst_ms.max(ms);
            drinking += drinks as usize;
            cells += n;
            checked += 1;
        }
    }
    assert!(checked > 500, "too few interiors ({checked})");
    assert!(drinking > 10, "too few inns and taverns checked ({drinking})");
    assert!(towers > 20, "too few wall towers checked ({towers})");
    assert!(cells > 50, "too few prison cells checked ({cells})");
    // The M5 budget (an interior opens within 300 ms); typical is well under 1 ms.
    assert!(worst_ms < 300.0, "slowest interior took {worst_ms:.1} ms");
}

/// A layout's roofed buildings by 100-ft bucket, for "does this door open into a neighbour's wall".
fn building_buckets(l: &worldgen::town::Layout) -> std::collections::HashMap<(i64, i64), Vec<usize>> {
    use worldgen::town::Structure;
    let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (k, b) in l.buildings.iter().enumerate() {
        if b.structure != Structure::Roofed {
            continue;
        }
        let (x0, y0, x1, y1) = b.poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        for by in (y0 / 100.0).floor() as i64..=(y1 / 100.0).floor() as i64 {
            for bx in (x0 / 100.0).floor() as i64..=(x1 / 100.0).floor() as i64 {
                buckets.entry((bx, by)).or_default().push(k);
            }
        }
    }
    buckets
}

/// The rules every interior keeps (`interiors_guarantee`): building or tower `id` of layout `si`
/// (`l`). Returns how long it took (ms), whether it is a drinking place, and its prison cells.
fn check_interior(world: &World, t0: &worldgen::t0::T0, l: &worldgen::town::Layout, si: usize, id: &str, buckets: &std::collections::HashMap<(i64, i64), Vec<usize>>) -> (f64, bool, usize) {
    use worldgen::interior;
    use worldgen::town::geom;
    let (mut drinking, mut cells) = (false, 0);
    let bi = &id;
    let t = std::time::Instant::now();
    let it = interior::generate_id(&world, &t0, &id).expect("enterable");
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let (nx, ny) = (it.nx, it.ny);
    let tag = format!("{bi} ({})", it.function);
    assert!(it.levels.len() >= 2, "{tag}: only {} level", it.levels.len());
    // One chapel per building; spiral stairs on the same square on every level with them.
    let chapels = it.levels.iter().flat_map(|lv| lv.rooms.iter()).filter(|r| r.kind == "chapel").count();
    assert!(chapels <= 1, "{tag}: {chapels} chapels");
    let spirals: Vec<Vec<(u16, u16)>> = it.levels.iter().map(|lv| lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| (f.x, f.y)).collect()).collect();
    if let Some(first) = spirals.iter().find(|v| !v.is_empty()) {
        for v in spirals.iter().filter(|v| !v.is_empty()) {
            assert_eq!(v, first, "{tag}: spiral stairs move between levels");
        }
        assert!(spirals.iter().filter(|v| !v.is_empty()).count() >= 2, "{tag}: spiral stairs on one level only");
    }
    // Cells: under 15 ft a side, each with a door onto a corridor or guardroom (never
    // through another cell).
    for lv in &it.levels {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, r)| r.kind == "cell") {
            let sq: Vec<usize> = (0..it.nx * it.ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % it.nx), a.1.max(k % it.nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / it.nx), a.1.max(k / it.nx)));
            assert!(x1 - x0 < 3 && y1 - y0 < 3, "{tag} {}: a {}x{} ft cell", lv.name, 5 * (x1 - x0 + 1), 5 * (y1 - y0 + 1));
            let doors: Vec<i16> = lv.doors.iter().filter(|d| d.rooms.contains(&(ri as i16))).map(|d| if d.rooms[0] == ri as i16 { d.rooms[1] } else { d.rooms[0] }).collect();
            assert!(!doors.is_empty(), "{tag} {}: a cell with no door", lv.name);
            assert!(doors.iter().all(|&o| o < 0 || lv.rooms[o as usize].kind != "cell"), "{tag} {}: a cell entered through another cell", lv.name);
            cells += 1;
        }
    }
    // Drinking places serve from a bar to tables on the ground floor.
    const DRINKING: [&str; 10] = ["Inn", "Tavern", "Alehouse", "Fine dining", "Brewery", "Winery", "Distillery", "Gambling den", "Smugglers' den", "Caravanserai"];
    if DRINKING.contains(&it.function) {
        let g = &it.levels[it.entry_level];
        let bars = g.furniture.iter().filter(|f| f.kind == "bar").count();
        let tables = g.furniture.iter().filter(|f| f.kind == "table" || f.kind == "booth_table").count();
        assert!(bars >= 1 && tables >= 2, "{tag}: {bars} bars and {tables} tables on the ground floor");
        // Guests come in through the taproom and go upstairs from it.
        let public = |r: i16| r >= 0 && matches!(g.rooms[r as usize].kind, "common room" | "taproom");
        let front = g.doors.iter().find(|d| d.kind == "front").expect("front door");
        assert!(public(front.rooms[0]), "{tag}: the front door opens into the {}", g.rooms[front.rooms[0] as usize].kind);
        let stair_room = g.cells[it.stairs[1] * it.nx + it.stairs[0]];
        assert!(public(stair_room), "{tag}: the stairs are in the {}", g.rooms[stair_room.max(0) as usize].kind);
        drinking = true;
    }
    let st = it.stairs;
    let on_stairs = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
    for lv in &it.levels {
        if lv.has_stairs {
            for j in st[1]..st[1] + st[3] {
                for i in st[0]..st[0] + st[2] {
                    assert!(lv.cells[j * nx + i] >= 0, "{tag} {}: stairs outside the building", lv.name);
                }
            }
        }
        let fronts = lv.doors.iter().filter(|d| d.kind == "front").count();
        assert_eq!(fronts, if lv.z == 0 { 1 } else { 0 }, "{tag} {}: {fronts} front doors", lv.name);
        // The front door opens onto open ground, not into a neighbouring building.
        if let Some(d) = lv.doors.iter().find(|d| d.kind == "front") {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
            let outside = sq.into_iter().find(|&(i, j)| i < 0 || j < 0 || i >= nx as isize || j >= ny as isize || lv.cells[j as usize * nx + i as usize] < 0).expect("door on an outside wall");
            let p = interior::to_world(&it, outside.0 as f64 + 0.5, outside.1 as f64 + 0.5);
            let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok());
            let hit = buckets.get(&((p[0] / 100.0).floor() as i64, (p[1] / 100.0).floor() as i64)).into_iter().flatten().find(|&&k| Some(k) != own && geom::contains(&l.buildings[k].poly, p));
            assert!(hit.is_none(), "{tag}: the front door opens into building {}", hit.unwrap());
        }
        // Squares either side of each door.
        let door_sq = |d: &interior::Door| -> Vec<(isize, isize)> {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            if x0 == x1 { vec![(x0 - 1, y0), (x0, y0)] } else { vec![(x0, y0 - 1), (x0, y0)] }
        };
        let inb = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny;
        let mut blocked = vec![false; nx * ny];
        for f in &lv.furniture {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    assert!(!lv.has_stairs || !on_stairs(i, j), "{tag} {}: {} on the stairs", lv.name, f.name);
                    assert!(!lv.doors.iter().any(|d| door_sq(d).contains(&(i as isize, j as isize))), "{tag} {}: {} in a doorway", lv.name, f.name);
                    if f.blocks_move {
                        blocked[j * nx + i] = true;
                    }
                }
            }
        }
        // Flood over open floor from the stairs, crossing between rooms only at doors.
        let door_between = |a: (usize, usize), b: (usize, usize)| {
            lv.doors.iter().any(|d| {
                let s = door_sq(d);
                s.contains(&(a.0 as isize, a.1 as isize)) && s.contains(&(b.0 as isize, b.1 as isize))
            })
        };
        // From the stairs (or, above the main stairs, every spiral stair).
        let starts: Vec<usize> = if lv.has_stairs {
            vec![st[1] * nx + st[0]]
        } else {
            lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize).collect()
        };
        let mut seen = vec![false; nx * ny];
        let mut stack = starts.clone();
        for &k in &starts {
            seen[k] = true;
        }
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i as isize + di, j as isize + dj);
                if !inb((a, b)) {
                    continue;
                }
                let q = b as usize * nx + a as usize;
                if seen[q] || lv.cells[q] < 0 || blocked[q] {
                    continue;
                }
                if lv.cells[q] != lv.cells[k] && !door_between((i, j), (a as usize, b as usize)) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        for d in &lv.doors {
            let reached = door_sq(d).into_iter().filter(|&p| inb(p)).any(|(i, j)| seen[j as usize * nx + i as usize]);
            assert!(reached, "{tag} {}: a {} door is cut off", lv.name, d.kind);
        }
        for (ri, room) in lv.rooms.iter().enumerate() {
            let open = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && !blocked[k]);
            let reached = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && seen[k]);
            assert!(!open || reached, "{tag} {}: the {} is unreachable", lv.name, room.kind);
        }
        // Floors with corridors: no room is reached through more than one other room (a
        // guardroom is a way through, as for cells).
        let circ = |r: usize| matches!(lv.rooms[r].kind, "hall" | "corridor" | "landing" | "great hall" | "mess hall" | "guardroom");
        if lv.rooms.len() >= 8 && (0..lv.rooms.len()).any(circ) {
            let mut hops = vec![usize::MAX; lv.rooms.len()];
            let mut q = std::collections::VecDeque::new();
            for r in (0..lv.rooms.len()).filter(|&r| circ(r)) {
                hops[r] = 0;
                q.push_back(r);
            }
            while let Some(r) = q.pop_front() {
                for d in &lv.doors {
                    let [a, b] = d.rooms;
                    let other = if a == r as i16 { b } else if b == r as i16 { a } else { continue };
                    if other >= 0 && hops[other as usize] == usize::MAX {
                        hops[other as usize] = hops[r] + 1;
                        q.push_back(other as usize);
                    }
                }
            }
            for (ri, room) in lv.rooms.iter().enumerate() {
                if room.squares > 0 && hops[ri] != usize::MAX {
                    assert!(hops[ri] <= 2, "{tag} {}: the {} is {} rooms from a corridor", lv.name, room.kind, hops[ri] - 1);
                }
            }
        }
    }
    (ms, drinking, cells)
}

/// M6: every underground site opens where its entrance is on the surface, has at least two
/// levels joined by a way down and a way up on the same square, can be walked end to end
/// (around blocking furniture), and ends in one boss chamber on its deepest level that is a
/// dead end (nothing else on the level is reached through it). Sewers match the streets above:
/// their tunnels lie under paved streets, and the streets over dry ground have sewer beneath.
/// Each site generates within 300 ms.
#[test]
fn underground_guarantee() {
    use worldgen::interior::{self, Level};
    use worldgen::town::{self, geom};
    use worldgen::under::{SiteSize, THEMES};
    let base = World::from_json(&world_json(1)).unwrap();
    let mut t0 = worldgen::t0::T0::generate(&base);
    // Every theme at every size, as created sites (bare entrances on dry land clear of other
    // layouts); the huge ones as deep as a site goes, the rest 1 to 6 levels.
    let sea = base.params().sea_level_ft;
    let g = &base.geom;
    let spots: Vec<[f64; 2]> = (0..90 * 60)
        .map(|k| [(k % 90) as f64 / 90.0 * g.map_w_ft + 1_000.0, (k / 90) as f64 / 60.0 * g.map_h_ft + 1_000.0])
        .filter(|p| {
            let h = t0.sample(p[0], p[1], 20.0);
            h > sea + 20.0 && (t0.sample_water(p[0], p[1]) as f64) < h && town::layouts_near(&base, &t0, [p[0] - 300.0, p[1] - 300.0, p[0] + 300.0, p[1] + 300.0]).is_empty()
        })
        .collect();
    let mut file = base.file.clone();
    for (i, (t, size)) in THEMES.iter().flat_map(|t| SiteSize::ALL.map(|s| (t, s))).enumerate() {
        let p = spots[i * spots.len() / (THEMES.len() * 4)];
        let levels = if size == SiteSize::Huge { worldgen::under::MAX_LEVELS } else { 1 + (i % 6) as u8 };
        file.edits.created.push(worldgen::world::Created {
            id: format!("c:{i}"),
            kind: "entrance".into(),
            x: p[0],
            y: p[1],
            name: format!("{} {}", t.key, size.name()),
            under: Some(t.kind.key().into()),
            size: Some(size.name().into()),
            levels: Some(levels),
            theme: Some(t.key.into()),
            ..Default::default()
        });
    }
    let world = World::new(file).unwrap();
    t0.apply_edits(&world);
    let t0 = t0;
    let (mut worst_ms, mut kinds) = (0.0f64, std::collections::BTreeMap::<&str, usize>::new());
    let mut designed = 0;
    // Squares reached from `start` over floor, stepping between rooms only through doors (any
    // step on a natural level), never onto `blocked`.
    let flood = |lv: &Level, nx: usize, ny: usize, start: usize, blocked: &dyn Fn(usize) -> bool| {
        let door = |a: usize, b: usize| {
            let (ia, ja, ib, jb) = ((a % nx) as f32, (a / nx) as f32, (b % nx) as f32, (b / nx) as f32);
            // The shared edge of two neighbouring squares.
            let (p, q) = if ja == jb { ([ia.max(ib), ja], [ia.max(ib), ja + 1.0]) } else { ([ia, ja.max(jb)], [ia + 1.0, ja.max(jb)]) };
            lv.doors.iter().any(|d| (d.a == p && d.b == q) || (d.a == q && d.b == p))
        };
        let mut seen = vec![false; nx * ny];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for q in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten() {
                if seen[q] || lv.cells[q] < 0 || blocked(q) {
                    continue;
                }
                if !lv.natural && lv.cells[q] != lv.cells[k] && !door(k, q) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        seen
    };
    // Every way to another site leads to one that has a way straight back.
    let links_back = |tag: &str, it: &interior::Interior| {
        for lv in &it.levels {
            for k in &lv.links {
                let there = interior::generate_id(&world, &t0, &k.to).unwrap_or_else(|| panic!("{tag}: a way to {} that doesn't exist", k.to));
                assert!(there.levels.iter().any(|l2| l2.links.iter().any(|b| b.to == it.id)), "{tag}: {} has no way back", k.to);
            }
        }
    };
    let check = |tag: &str, it: &interior::Interior, l: &town::Layout, entrance: Option<[f64; 2]>| {
        links_back(tag, it);
        let (nx, ny) = (it.nx, it.ny);
        let n = it.levels.len();
        assert!(n >= 1, "{tag}: no levels");
        assert_eq!(it.entry_level, n - 1, "{tag}: the way in is not on the top level");
        let at = |lv: &Level, kind: &str| lv.furniture.iter().filter(|f| f.kind == kind).map(|f| f.y as usize * nx + f.x as usize).collect::<Vec<_>>();
        // The way in sits on the surface entrance (a sewer section without a grate is entered
        // from its neighbours: walk it from any square).
        let exit = at(&it.levels[n - 1], "exit");
        let mut start = match entrance {
            Some(e_at) => {
                // (A sewer section has a way up at each of its grates.)
                assert!(exit.len() == 1 || (it.function == "sewer" && !exit.is_empty()), "{tag}: {} ways in", exit.len());
                let near = |k: usize| geom::dist(interior::to_world(it, (k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5), e_at);
                let k = *exit.iter().min_by(|&&a, &&b| near(a).total_cmp(&near(b))).unwrap();
                assert!(near(k) < 8.0, "{tag}: the way in is {:.0} ft from the entrance", near(k));
                k
            }
            None => {
                assert!(exit.is_empty(), "{tag}: a way in with no entrance above");
                // From the way up (deep dungeons), else any square no furniture stands on.
                let top = &it.levels[n - 1];
                let held = |q: usize| top.furniture.iter().any(|f| f.blocks_move && (f.x as usize..(f.x + f.w) as usize).contains(&(q % nx)) && (f.y as usize..(f.y + f.h) as usize).contains(&(q / nx)));
                at(top, "up").first().copied().or_else(|| (0..nx * ny).find(|&q| top.cells[q] >= 0 && !held(q))).unwrap_or_else(|| panic!("{tag}: no floor"))
            }
        };
        // Levels bottom to top: each but the deepest has a way down onto the way up below.
        for li2 in (0..n).rev() {
            let lv = &it.levels[li2];
            let down = at(lv, "down");
            if li2 == 0 {
                assert!(down.is_empty(), "{tag}: a way down from the deepest level");
            } else {
                assert_eq!(down.len(), 1, "{tag} {}: {} ways down", lv.name, down.len());
                assert_eq!(at(&it.levels[li2 - 1], "up"), down, "{tag} {}: the way down and the way up below differ", lv.name);
            }
            // Everything is reached, around blocking furniture too.
            let blocks: Vec<bool> = {
                let mut b = vec![false; nx * ny];
                for f in lv.furniture.iter().filter(|f| f.blocks_move) {
                    for j in f.y..f.y + f.h {
                        for i in f.x..f.x + f.w {
                            b[j as usize * nx + i as usize] = true;
                        }
                    }
                }
                b
            };
            assert!(!blocks[start] && lv.cells[start] >= 0, "{tag} {}: the way in is not open floor", lv.name);
            for f in &lv.furniture {
                for j in f.y..f.y + f.h {
                    for i in f.x..f.x + f.w {
                        assert!(lv.cells[j as usize * nx + i as usize] >= 0, "{tag} {}: {} in the rock", lv.name, f.name);
                    }
                }
            }
            let seen = flood(lv, nx, ny, start, &|q| blocks[q]);
            let lost = (0..nx * ny).filter(|&q| lv.cells[q] >= 0 && !blocks[q] && !seen[q]).count();
            assert_eq!(lost, 0, "{tag} {}: {lost} squares unreachable", lv.name);
            if li2 > 0 {
                start = at(lv, "down")[0];
            }
        }
        // One boss chamber, on the deepest level, a dead end.
        let boss: Vec<(usize, usize)> = it.levels.iter().enumerate().flat_map(|(li2, lv)| lv.rooms.iter().enumerate().filter(|(_, r)| r.kind == worldgen::under::BOSS && r.squares > 0).map(move |(ri, _)| (li2, ri))).collect();
        assert_eq!(boss.len(), 1, "{tag}: {} boss chambers", boss.len());
        let (bl, br) = boss[0];
        assert_eq!(bl, 0, "{tag}: the boss chamber is not on the deepest level");
        let deep = &it.levels[0];
        // (From the way up; a site of one level from its way in.)
        let up = at(deep, "up").first().or(at(deep, "exit").first()).copied().unwrap_or_else(|| panic!("{tag}: no way onto the deepest level"));
        let seen = flood(deep, nx, ny, up, &|q| deep.cells[q] == br as i16);
        let cut = (0..nx * ny).filter(|&q| deep.cells[q] >= 0 && deep.cells[q] != br as i16 && !seen[q]).count();
        assert_eq!(cut, 0, "{tag}: {cut} squares are reached only through the boss chamber");
        if it.function == "sewer" {
            let top = &it.levels[n - 1];
            let streets = worldgen::under::street_segments(l);
            let reach = worldgen::under::SEWER_REACH_FT + 0.01;
            for q in 0..nx * ny {
                let r = top.cells[q];
                if r >= 0 && top.rooms[r as usize].kind == "sewer tunnel" {
                    let c = interior::to_world(it, (q % nx) as f64 + 0.5, (q / nx) as f64 + 0.5);
                    assert!(streets.iter().any(|&(a, b, _)| geom::seg_dist(c, a, b) <= reach), "{tag}: a sewer tunnel under no street at {c:?}");
                }
            }
            let wet = |p: [f64; 2]| t0.sample_water(p[0], p[1]) as f64 > t0.sample(p[0], p[1], 5.0);
            let (mut points, mut missed) = (0, 0);
            for &(a, b, _) in &streets {
                let steps = (geom::dist(a, b) / 2.5).ceil().max(1.0) as usize;
                for s in 0..=steps {
                    let p = geom::lerp(a, b, s as f64 / steps as f64);
                    let g = interior::to_grid(it, p);
                    // The whole section, edges included: neighbouring sections line up only
                    // if neither drops its edge runs.
                    if g[0] < 0.0 || g[1] < 0.0 || g[0] >= nx as f64 || g[1] >= ny as f64 || wet(p) {
                        continue;
                    }
                    points += 1;
                    if top.cells[g[1] as usize * nx + g[0] as usize] < 0 {
                        missed += 1;
                    }
                }
            }
            assert!(missed * 50 <= points, "{tag}: {missed} of {points} street points have no sewer beneath");
        }
    };
    for li in 0..town::layout_count(&t0) {
        let l = town::layout(&world, &t0, li);
        for (k, e) in l.entrances.iter().enumerate() {
            let tag = format!("u:{li}:{k} ({})", e.kind.name());
            let t = std::time::Instant::now();
            let it = interior::generate_id(&world, &t0, &format!("u:{li}:{k}")).unwrap_or_else(|| panic!("{tag}: no site"));
            worst_ms = worst_ms.max(t.elapsed().as_secs_f64() * 1e3);
            *kinds.entry(e.kind.name()).or_default() += 1;
            check(&tag, &it, &l, Some(e.at));
            // The designer holds sites to the same rules, and copies one exactly (as a design,
            // and through the text form agents write).
            if e.kind != worldgen::under::UnderKind::Sewer {
                use worldgen::under::design;
                let d = design::SiteDesign::from_interior(&it);
                let problems = design::check(&it, Some(d.entry));
                assert!(problems.is_empty(), "{tag}: the designer finds {problems:?}");
                let again = d.build(&it.id, it.settlement, it.building);
                assert_eq!(serde_json::to_string(&again).unwrap(), serde_json::to_string(&it).unwrap(), "{tag}: its design builds something else");
                let text = design::to_text(&d, &|_, _| None).unwrap_or_else(|e| panic!("{tag}: {e}"));
                // (Over the same site emptied: the text says it all.)
                let mut blank = d.clone();
                for lv in &mut blank.levels {
                    *lv = design::DesignLevel { elevation_ft: lv.elevation_ft, natural: lv.natural, cells: vec![-1, (it.nx * it.ny) as i32], ..Default::default() };
                }
                let (back, names) = design::from_text(&text, &blank).unwrap_or_else(|e| panic!("{tag}: {e}\n{text}"));
                // (Rooms left with no squares are not in the text: chambers, after the last one used.)
                let mut same = d.clone();
                for lv in &mut same.levels {
                    let cells = design::decode(&lv.cells, it.nx * it.ny).unwrap();
                    let used = |r: usize| cells.contains(&(r as i16));
                    while lv.rooms.len() > 0 && !used(lv.rooms.len() - 1) {
                        lv.rooms.pop();
                    }
                    for (r, room) in lv.rooms.iter_mut().enumerate() {
                        if !used(r) {
                            *room = design::DesignRoom { kind: design::CHAMBER.into(), raise_ft: 0 };
                        }
                    }
                }
                assert!(names.is_empty() && back == same, "{tag}: its text reads back as something else");
                designed += 1;
            }
            // A created site is what it was asked to be.
            if let Some(c) = li.checked_sub(t0.settlements.len() + t0.base_pois).and_then(|c| world.file.edits.created.get(c)) {
                assert_eq!(it.levels.len(), c.levels.unwrap() as usize, "{tag} ({}): levels", c.name);
                assert_eq!(it.theme, c.theme.as_deref(), "{tag} ({}): theme", c.name);
            }
        }
        // Every section of a city's sewers, grate or not.
        let streets = worldgen::under::street_segments(&l);
        if !l.entrances.iter().any(|e| e.kind == worldgen::under::UnderKind::Sewer) {
            continue;
        }
        let s = worldgen::under::SEWER_SECTION_FT;
        let sec = |v: f64| (v / s).floor() as i64;
        let (x0, y0) = streets.iter().fold((i64::MAX, i64::MAX), |m, (a, b, _)| (m.0.min(sec(a[0].min(b[0]))), m.1.min(sec(a[1].min(b[1])))));
        let (x1, y1) = streets.iter().fold((i64::MIN, i64::MIN), |m, (a, b, _)| (m.0.max(sec(a[0].max(b[0]))), m.1.max(sec(a[1].max(b[1])))));
        let mut n_sec = 0;
        for sy in y0..=y1 {
            for sx in x0..=x1 {
                let id = format!("w:{li}:{sx}:{sy}");
                let t = std::time::Instant::now();
                let Some(it) = interior::generate_id(&world, &t0, &id) else { continue };
                worst_ms = worst_ms.max(t.elapsed().as_secs_f64() * 1e3);
                let grate = l.entrances.iter().find(|e| e.kind == worldgen::under::UnderKind::Sewer && worldgen::under::sewer_section(e.at) == [sx as f64 * s, sy as f64 * s]).map(|e| e.at);
                check(&format!("{id} (sewer section)"), &it, &l, grate);
                n_sec += 1;
            }
        }
        assert!(n_sec > 0, "layout {li}: no sewer sections");
        // Keeps' deep dungeons, and the cellars that open into the sewers.
        for bi in 0..l.buildings.len() {
            if interior::has_deep_dungeon(&l.buildings[bi]) {
                let id = format!("k:{li}:{bi}");
                let t = std::time::Instant::now();
                let it = interior::generate_id(&world, &t0, &id).unwrap_or_else(|| panic!("{id}: no deep dungeons"));
                worst_ms = worst_ms.max(t.elapsed().as_secs_f64() * 1e3);
                *kinds.entry("deep dungeons").or_default() += 1;
                check(&format!("{id} (deep dungeons)"), &it, &l, None);
            }
            if worldgen::under::sewer_link_of(&t0, &l, &l.buildings[bi]).is_some() {
                let id = format!("b:{li}:{bi}");
                let it = interior::generate_id(&world, &t0, &id).unwrap();
                assert!(it.levels[0].links.iter().any(|k| k.to.starts_with("w:")), "{id}: no way into the sewers");
                links_back(&id, &it);
                *kinds.entry("sewer cellars").or_default() += 1;
            }
        }
    }
    for kind in ["dungeon", "crypt", "cave", "mine", "lava tube", "sewer", "catacombs", "deep dungeons", "sewer cellars"] {
        assert!(kinds.get(kind).copied().unwrap_or(0) > 0, "no {kind} in the world: {kinds:?}");
    }
    assert!(worst_ms < 300.0, "slowest underground site took {worst_ms:.1} ms");
    assert!(designed > 0, "no site to design");
    designed_site(&world, &t0);
}

/// A site designed by hand (as an agent writes one, in text): a level dug below the deepest,
/// joined by a way down, doors added where needed, a room furnished; the world then builds the
/// design instead of the generated site. Breaking a rule (a room shut off, the way in moved, a
/// way up not under the way down) is caught.
fn designed_site(world: &World, t0: &worldgen::t0::T0) {
    use worldgen::under::design;
    let (li, k) = (0..worldgen::town::layout_count(t0))
        .find_map(|li| {
            let l = worldgen::town::layout(world, t0, li);
            l.entrances.iter().position(|e| e.kind == worldgen::under::UnderKind::Dungeon).map(|k| (li, k))
        })
        .expect("a dungeon");
    let id = format!("u:{li}:{k}");
    let base = design::design_of(world, t0, &id, false).unwrap();
    let (nx, ny, n) = (base.nx as usize, base.ny as usize, base.levels.len());
    // A free square of the deepest level's floor, clear of the edge, for the way down.
    let deep = &base.levels[0];
    let cells = design::decode(&deep.cells, nx * ny).unwrap();
    let held = |i: usize, j: usize| deep.items.iter().any(|f| (f.x as usize..(f.x + f.w) as usize).contains(&i) && (f.y as usize..(f.y + f.h) as usize).contains(&j));
    let (x, y) = (3..ny - 6)
        .flat_map(|j| (3..nx - 8).map(move |i| (i, j)))
        .find(|&(i, j)| cells[j * nx + i] >= 0 && !held(i, j) && !deep.doors.iter().any(|d| (d[0] as usize, d[1] as usize) == (i, j)))
        .expect("free floor");
    let items: Vec<String> = deep.items.iter().map(|f| format!("{} {},{} {}x{}", f.kind, f.x, f.y, f.w, f.h)).collect();
    // The new level: a landing round the way up, a vault east of it.
    let grid: String = (0..ny)
        .map(|j| (0..nx).map(|i| if i + 1 >= x && i <= x + 1 && j + 1 >= y && j <= y + 1 { 'a' } else if i >= x + 2 && i <= x + 6 && j + 2 >= y && j <= y + 2 { 'b' } else { '.' }).collect::<String>() + "\n")
        .collect();
    let level = |doors: &str| {
        format!(
            "site dungeon · {} levels\nlevel {n}\nitems: {}; down {x},{y}\nlevel {}: Level {} · the vault\nrooms: a=landing; b=secret vault \"The Hoard\"\ngrid:\n{grid}doors: {doors}\nitems: chest {},{}\n",
            n + 1,
            items.join("; "),
            n + 1,
            n + 1,
            x + 5,
            y
        )
    };
    let (d, names) = design::from_text(&level("auto"), &base).unwrap_or_else(|e| panic!("{id}: {e}"));
    assert_eq!(d.levels.len(), n + 1, "{id}: a level added");
    assert_eq!(names, vec![(0, 1, "The Hoard".to_string())], "{id}: an unknown kind of room is a chamber by that name");
    assert_eq!(d.levels[0].items.iter().filter(|f| f.kind == "up").map(|f| (f.x as usize, f.y as usize)).collect::<Vec<_>>(), vec![(x, y)], "{id}: the way up is put under the way down");
    let (it, problems) = d.problems(&id);
    assert!(problems.iter().all(|p| !p.blocking), "{id}: {problems:?}");
    assert_eq!(it.levels[0].doors.len(), 1, "{id}: one door from the landing into the vault");
    // Broken: the vault shut off; the way in moved; the way up off the way down.
    let (shut, _) = design::from_text(&level(""), &base).unwrap();
    assert!(shut.problems(&id).1.iter().any(|p| p.blocking && p.level == 0 && p.text.contains("can't be reached")), "{id}: a room shut off");
    let mut moved = d.clone();
    let top = moved.levels.last_mut().unwrap();
    let e = top.items.iter_mut().find(|f| f.kind == "exit").unwrap();
    e.x += 1;
    assert!(moved.problems(&id).1.iter().any(|p| p.blocking && p.text.contains("under the entrance")), "{id}: the way in moved");
    let mut off = d.clone();
    off.levels[0].items.iter_mut().find(|f| f.kind == "up").unwrap().y += 1;
    assert!(off.problems(&id).1.iter().any(|p| p.blocking && p.level == 0), "{id}: the way up off the way down");
    // Furnished: props go where they leave everything reachable.
    let mut f = d.clone();
    let had = f.levels[0].items.len();
    f.furnish(0, 0, 7);
    f.levels[0].rooms[0].kind = "armory".into();
    f.furnish(0, 0, 7);
    assert!(f.levels[0].items.len() > had, "{id}: the armory got nothing");
    assert!(f.problems(&id).1.iter().all(|p| !p.blocking), "{id}: furnishing broke it");
    // A design travels as an edit op (the app's, an agent's) and comes back as it was.
    let mut file = world.file.clone();
    let v = serde_json::to_value(&f).unwrap();
    let undo = file.edits.apply(&worldgen::world::EditOp::Set { field: "designs".into(), key: id.clone(), value: v.clone() }).unwrap();
    assert_eq!(serde_json::to_value(&file.edits.designs[&id]).unwrap(), v, "{id}: the design changed on the round trip");
    assert!(matches!(undo, worldgen::world::EditOp::Unset { .. }), "{id}: undoing a new design unsets it");
    // Live edits reach the generators field by field, or as patches of keyed fields: the same
    // edits as sent whole.
    let mut changed = file.edits.clone();
    changed.renames.insert(format!("r:{id}:0:1"), "The Hoard".into());
    changed.designs.remove(&id);
    let mut by_field = file.edits.clone();
    let fields = serde_json::json!({ "renames": changed.renames, "designs": changed.designs });
    assert!(!by_field.set_fields(&fields.to_string()).unwrap(), "{id}: created sites untouched");
    assert!(by_field == changed, "{id}: edits set field by field differ");
    let mut patched = file.edits.clone();
    let patch = serde_json::json!({ "renames": { "set": { format!("r:{id}:0:1"): "The Hoard" } }, "designs": { "unset": [id] } });
    patched.patch_fields(&patch.to_string()).unwrap();
    assert!(patched == changed, "{id}: patched edits differ");
    // The world builds the design (its hash unchanged), the same in a world read back from JSON.
    let w2 = World::new(file).unwrap();
    assert_eq!(w2.hash, world.hash, "designs are edits: the world hash stays");
    let it2 = worldgen::interior::generate_id(&w2, t0, &id).unwrap();
    assert_eq!(it2.levels.len(), n + 1, "{id}: the world builds the design");
    let w3 = World::from_json(&serde_json::to_string(&w2.file).unwrap()).unwrap();
    assert_eq!(serde_json::to_string(&worldgen::interior::generate_id(&w3, t0, &id).unwrap()).unwrap(), serde_json::to_string(&it2).unwrap(), "{id}: read back");
    assert!(design::design_of(&w2, t0, &id, true).unwrap() == base, "{id}: the generated site is still there underneath");
}

/// Agent edits: a created ruin is a real site (laid out where it was put, its crypt a full
/// underground site ending in a boss chamber, on the battlemap there); a removed one is gone;
/// edits never change the world hash (nothing generated moves); renames, search and routes
/// answer through the agent queries.
#[test]
fn agent_edits_guarantee() {
    use worldgen::world::Created;
    use worldgen::{agent, interior, town, town::geom};
    let mut file: WorldFile = serde_json::from_str(&world_json(1)).unwrap();
    let base = World::new(file.clone()).unwrap();
    let t0 = worldgen::t0::T0::generate(&base);
    let sea = base.params().sea_level_ft;
    // Dry open land two miles from a town, clear of every layout.
    let spot = (0..t0.settlements.len() * 8)
        .map(|k| {
            let s = &t0.settlements[k / 8];
            let a = (k % 8) as f64 * std::f64::consts::TAU / 8.0;
            [s.x + 10_560.0 * libm::cos(a), s.y + 10_560.0 * libm::sin(a)]
        })
        .find(|p| {
            let g = t0.sample(p[0], p[1], 20.0);
            g > sea + 20.0 && (t0.sample_water(p[0], p[1]) as f64) < g && town::layouts_near(&base, &t0, [p[0] - 400.0, p[1] - 400.0, p[0] + 400.0, p[1] + 400.0]).is_empty()
        })
        .expect("a dry open spot");
    let city = agent::features(&base, &t0).into_iter().find(|f| f.kind == "city").expect("a city").id;
    file.edits.created.push(Created { id: "c:0".into(), kind: "ruin".into(), x: spot[0], y: spot[1], name: "The Agent's Ruin".into(), under: Some("crypt".into()), ..Default::default() });
    file.edits.created.push(Created { id: "c:1".into(), kind: "cave".into(), x: spot[0] + 3_000.0, y: spot[1], name: "Gone Cave".into(), removed: true, ..Default::default() });
    file.edits.renames.insert(city.clone(), "Agentholm".into());
    let world = World::new(file).unwrap();
    assert_eq!(world.hash, base.hash, "edits changed the world hash");
    let ex = Executor::new(world);
    let (world, t0) = (&ex.world, &ex.t0);

    let li = agent::layout_of(world, t0, "c:0").expect("the created ruin's layout");
    assert_eq!(li, t0.settlements.len() + t0.base_pois);
    let l = town::layout(world, t0, li);
    assert!(geom_dist(l.center, spot) < 1.0, "the ruin is not where it was put");
    assert!(l.buildings.iter().any(|b| b.structure == town::Structure::Ruin), "no ruined walls");
    let k = l.entrances.iter().position(|e| e.kind == worldgen::under::UnderKind::Crypt).expect("no crypt under the ruin");
    let crypt = interior::generate_id(world, t0, &format!("u:{li}:{k}")).expect("the crypt");
    assert!(crypt.levels[0].rooms.iter().any(|r| r.kind == worldgen::under::BOSS && r.squares > 0), "the crypt has no boss chamber");
    // On the battlemap there.
    let g = &world.geom;
    let size = g.tile_size_ft(g.max_level);
    let key = TileKey::surface(g.max_level, (spot[0] / size) as u32, (spot[1] / size) as u32);
    let chunk = Executor::new(World::from_json(&serde_json::to_string(&world.file).unwrap()).unwrap()).battlemap(key);
    assert!(chunk.urban.iter().any(|&u| u == worldgen::battlemap::URBAN_RUIN), "the ruin is not on the battlemap");
    // The removed cave is laid out nowhere.
    let near = town::layouts_near(world, t0, [spot[0] + 2_000.0, spot[1] - 1_000.0, spot[0] + 4_000.0, spot[1] + 1_000.0]);
    assert!(near.iter().all(|l| l.index as usize != li + 1), "a removed site is still laid out");
    // Agent queries.
    assert_eq!(agent::get(world, t0, &city).unwrap()["name"], "Agentholm");
    assert!(agent::search(world, t0, "agent's ruin", None, 5, false)["results"].as_array().unwrap().iter().any(|r| r["id"] == "c:0"));
    let a = agent::position(world, t0, &city).unwrap();
    let r = agent::route(world, t0, a, spot);
    assert!(r["travel"]["normal_days"].as_f64().unwrap() > 0.0, "no route: {r}");

    // Edits made at the same time (the user's in the app, an agent's in mapd) both stay, in
    // either order, and undoing one leaves the other.
    let e0 = world.file.edits.clone();
    let mut user = e0.clone();
    user.renames.insert(city.clone(), "Userholm".into());
    user.created[1].removed = false;
    let mut agent_e = e0.clone();
    agent_e.notes.insert(city.clone(), worldgen::world::Note { text: "agent lore".into(), tags: vec![] });
    agent_e.created.push(Created { id: "c:2".into(), kind: "camp".into(), x: spot[0], y: spot[1] + 3_000.0, name: "Camp".into(), ..Default::default() });
    let (ou, oa) = (e0.diff(&user), e0.diff(&agent_e));
    let merged = |first: &[worldgen::world::EditOp], then: &[worldgen::world::EditOp]| {
        let mut e = e0.clone();
        let undo: Vec<_> = first.iter().map(|op| e.apply(op).unwrap()).collect();
        then.iter().for_each(|op| drop(e.apply(op).unwrap()));
        (e, undo)
    };
    let ((a, undo_user), (b, _)) = (merged(&ou, &oa), merged(&oa, &ou));
    assert_eq!(a, b, "concurrent edits merge differently by order");
    assert!(a.renames[&city] == "Userholm" && a.notes.contains_key(&city) && a.created.len() == 3 && !a.created[1].removed, "a concurrent edit was lost");
    let mut c = a.clone();
    undo_user.iter().rev().for_each(|op| drop(c.apply(op).unwrap()));
    assert!(c.renames[&city] == "Agentholm" && c.created[1].removed && c.notes.contains_key(&city) && c.created.len() == 3, "undo touched another's edit");

    // NPCs and plot points: the app's entries come back from mapd exactly as sent (serde would
    // drop fields it does not know), both sides' new ones merge, and a place shows its own.
    let npc = serde_json::json!({ "name": "Mira", "appearance": "tall", "mannerisms": "hums", "attitude": { "stance": "friendly", "text": "owes them" }, "goals": "gold", "notes": "a spy", "tags": ["guild"], "portrait": "00112233445566778899aabbccddeeff", "location": { "id": format!("b:{}:0", agent::layout_of(world, t0, &city).unwrap()), "level": 0, "x": 1.5, "y": 2.5 }, "status": "alive" });
    let plot = serde_json::json!({ "title": "The heist", "text": "...", "status": "active", "anchors": [city.clone()], "npcs": ["n:app"], "tags": [] });
    let ops = [
        worldgen::world::EditOp::Set { field: "npcs".into(), key: "n:app".into(), value: npc.clone() },
        worldgen::world::EditOp::Set { field: "plots".into(), key: "p:agent".into(), value: plot.clone() },
    ];
    let mut d = a.clone();
    ops.iter().for_each(|op| drop(d.apply(op).unwrap()));
    let back = serde_json::to_value(&d).unwrap();
    assert!(back["npcs"]["n:app"] == npc && back["plots"]["p:agent"] == plot, "an NPC or plot changed on the round trip: {}", back["npcs"]);
    let mut f = World::new(WorldFile { edits: d, ..world.file.clone() }).unwrap().file;
    f.edits.npcs.insert("n:other".into(), worldgen::world::Npc { name: "Other".into(), ..Default::default() });
    let w2 = World::new(f).unwrap();
    let got = agent::get(&w2, t0, &city).unwrap();
    assert!(got["npcs"].as_array().unwrap().iter().any(|n| n["id"] == "n:app") && got["plots"][0]["id"] == "p:agent", "the city does not show its NPC and plot: {got}");
    assert_eq!(w2.hash, base.hash, "NPCs changed the world hash");

    // A site's levels and rooms take names (`l:`/`r:` ids) that agents see, and are places.
    let site = format!("u:{li}:{k}");
    let ri = crypt.levels[0].rooms.iter().position(|r| r.kind == worldgen::under::BOSS).unwrap();
    let (lid, rid) = (format!("l:{site}:0"), format!("r:{site}:0:{ri}"));
    let mut f = w2.file.clone();
    f.edits.renames.insert(lid.clone(), "The Pit".into());
    f.edits.renames.insert(rid.clone(), "Vesk's Tomb".into());
    let w3 = World::new(f).unwrap();
    let got = agent::get(&w3, t0, &site).unwrap();
    assert!(got["interior"]["levels"][0]["name"] == "The Pit" && got["interior"]["levels"][0]["named_rooms"][0]["name"] == "Vesk's Tomb", "renamed levels and rooms do not reach agents: {got}");
    let names = agent::names(&w3, t0, Some("room"), Some(&site), None).unwrap();
    assert!(names["names"].as_array().unwrap().iter().any(|n| n["id"] == rid && n["name"] == "Vesk's Tomb"), "list_names misses a renamed room");
    assert!(agent::position(&w3, t0, &rid).is_some() && agent::position(&w3, t0, &format!("l:{site}:{}", crypt.levels.len())).is_none(), "level and room ids resolve wrongly");

    // Objects put down and taken away by hand: generated ones go (one by kind and place, all in
    // a circle), placed ones come in the chunk holding them with their rules (built-in, or an
    // uploaded sprite's), the payload names the sprite, and the app's entries survive mapd.
    let gen_chunk = |f: &WorldFile| {
        let mut ex = Executor::new(World::new(f.clone()).unwrap());
        let c = ex.battlemap(key);
        let packed = worldgen::battlemap::pack(&ex.world, &c);
        (c, packed)
    };
    let (plain, _) = gen_chunk(&w3.file);
    let origin = [key.x as f64 * size, key.y as f64 * size];
    let at = |o: &worldgen::battlemap::Object| [origin[0] + o.x as f64 * 5.0, origin[1] + o.y as f64 * 5.0];
    let one = plain.objects.iter().find(|o| o.sprite == 0).expect("a generated object");
    let (one_at, one_kind) = (at(one), one.kind as u16);
    let area = plain.objects.iter().map(|o| at(o)).find(|p| geom_dist(*p, one_at) > 60.0).expect("objects elsewhere");
    let asset = "0123456789abcdef0123456789abcdef";
    let free = [origin[0] + 320.0, origin[1] + 322.5];
    let edit_ops = [
        ("cleared", "x:one", serde_json::json!({ "x": one_at[0] + 0.5, "y": one_at[1], "kind": one_kind })),
        ("cleared", "x:area", serde_json::json!({ "x": area[0], "y": area[1], "r": 15.0 })),
        ("objects", "o:rock", serde_json::json!({ "kind": 12, "x": free[0], "y": free[1], "rot": 0.0, "scale": 1.0, "variant": 2 })),
        ("objects", "o:idol", serde_json::json!({ "kind": format!("s:{asset}"), "x": free[0] + 20.0, "y": free[1], "rot": 1.5, "scale": 1.0, "variant": 0 })),
        ("objects", "o:away", serde_json::json!({ "kind": 12, "x": origin[0] - 10.0, "y": free[1], "rot": 0.0, "scale": 1.0, "variant": 0 })),
        ("sprites", asset, serde_json::json!({ "name": "Idol", "size": 2.0, "cover": 3, "blocks_move": true, "blocks_sight": true, "difficult": false, "height_ft": 9.0 })),
    ];
    let mut e = w3.file.edits.clone();
    for (field, k, v) in &edit_ops {
        e.apply(&worldgen::world::EditOp::Set { field: (*field).into(), key: (*k).into(), value: v.clone() }).unwrap();
    }
    let back = serde_json::to_value(&e).unwrap();
    assert!(edit_ops.iter().all(|(f, k, v)| back[f][k] == *v), "an object edit changed on the round trip: {}", back["objects"]);
    let f = WorldFile { edits: e, ..w3.file.clone() };
    assert_eq!(World::new(f.clone()).unwrap().hash, base.hash, "object edits changed the world hash");
    let (edited, packed) = gen_chunk(&f);
    assert_eq!(packed, gen_chunk(&f).1, "an edited chunk is not repeatable");
    let gone = |c: &worldgen::battlemap::Chunk, p: [f64; 2], r: f64, kind: Option<u16>| {
        !c.objects.iter().any(|o| o.sprite == 0 && geom_dist(at(o), p) <= r && kind.is_none_or(|k| k == o.kind as u16) && !(geom_dist(at(o), free) < 0.1))
    };
    assert!(gone(&edited, one_at, 1.0, Some(one_kind)) && gone(&edited, area, 15.0, None), "cleared objects are still there");
    let removed = plain.objects.len() - plain.objects.iter().filter(|o| geom_dist(at(o), area) > 15.0 && !(o.kind as u16 == one_kind && geom_dist(at(o), one_at) <= 1.25)).count();
    assert_eq!(edited.objects.len(), plain.objects.len() - removed + 2, "objects other than the cleared ones changed, or a placed one is missing or misplaced");
    let rock = edited.objects.iter().find(|o| o.sprite == 0 && geom_dist(at(o), free) < 0.01).expect("the placed boulder");
    assert!(rock.kind as u16 == 12 && rock.variant == 2 && edited.info(rock).blocks_move, "the placed boulder is wrong");
    let idol = edited.objects.iter().find(|o| o.sprite != 0).expect("the placed sprite");
    let ii = edited.info(idol);
    assert!(edited.name(idol) == "Idol" && ii.cover == 3 && ii.blocks_sight && (ii.radius - 1.0).abs() < 1e-6 && (idol.rot - 1.5).abs() < 1e-6, "the sprite's rules are wrong");
    assert!(packed.ends_with(asset.as_bytes()) && edited.sprites.len() == 1, "the payload does not name the sprite");
    let dist = worldgen::battlemap::cover_distance(&edited);
    let (ix, iy) = (idol.x as usize, idol.y as usize);
    assert_eq!(dist[iy * worldgen::battlemap::SQ + ix], 0, "the sprite gives no cover");

    // Buildings drawn by hand: an L-shaped inn and a round tower on open ground get their own
    // layouts, interiors (every room on a floor reached through doors), roofs as chosen on the
    // battlemap; the world hash stays; one drawn over another, or on a road, is refused.
    let free_at = |poly: &dyn Fn([f64; 2]) -> Vec<[f64; 2]>| {
        (0..64)
            .map(|k| [spot[0] + 400.0 + 60.0 * (k % 8) as f64, spot[1] - 200.0 + 60.0 * (k / 8) as f64].map(|v| (v / 5.0).round() * 5.0))
            .find(|c| agent::building_spot(&w3, t0, &poly(*c), Some("inn"), "c:9", None).is_ok())
            .expect("open ground for a building")
    };
    let ell = |c: [f64; 2]| [[0.0, 0.0], [50.0, 0.0], [50.0, 20.0], [20.0, 20.0], [20.0, 45.0], [0.0, 45.0]].map(|p| [c[0] + p[0], c[1] + p[1]]).to_vec();
    let at_inn = free_at(&ell);
    let round = |c: [f64; 2]| geom::circle([c[0] + 25.0, c[1] + 25.0], 15.0, 16);
    let at_tower = free_at(&|c| if geom_dist(c, at_inn) < 120.0 { vec![] } else { round(c) });
    let mk = |id: &str, poly: Vec<[f64; 2]>, func: &str, floors: u8, roof: &str, tint: Option<&str>| {
        let (p, name) = agent::building_spot(&w3, t0, &poly, Some(func), id, None).unwrap();
        Created { id: id.into(), kind: "building".into(), x: p[0], y: p[1], name, poly, floors: Some(floors), func: Some(func.into()), roof: Some(roof.into()), tint: tint.map(str::to_string), ..Default::default() }
    };
    let mut e = w3.file.edits.clone();
    let n0 = e.created.len();
    let inn = mk(&format!("c:{n0}"), ell(at_inn), "inn", 2, "battlements", Some("slate"));
    let tower = mk(&format!("c:{}", n0 + 1), round(at_tower), "wizard_tower", 4, "cone", None);
    assert!(inn.check().is_ok() && tower.check().is_ok() && !inn.name.is_empty(), "a drawn building is refused");
    for c in [&inn, &tower] {
        let v = serde_json::to_value(c).unwrap();
        e.apply(&worldgen::world::EditOp::Set { field: "created".into(), key: c.id.clone(), value: v.clone() }).unwrap();
        assert_eq!(serde_json::to_value(e.created.last().unwrap()).unwrap(), v, "a building changed on the round trip");
    }
    let f = WorldFile { edits: e, ..w3.file.clone() };
    let mut ex = Executor::new(World::new(f).unwrap());
    assert_eq!(ex.world.hash, base.hash, "buildings changed the world hash");
    for (c, levels, roof) in [(&inn, 4, 1 | 2 << 2), (&tower, 5, 2)] {
        let li = agent::layout_of(&ex.world, &ex.t0, &c.id).expect("the building's layout");
        let l = town::layout(&ex.world, &ex.t0, li);
        assert!(l.buildings.len() == 1 && l.buildings[0].poly == c.poly, "{}: not laid out as drawn", c.id);
        let it = interior::generate_id(&ex.world, &ex.t0, &format!("b:{li}:0")).expect("an interior");
        assert_eq!(it.levels.len(), levels, "{}: levels {:?}", c.id, it.levels.iter().map(|l| &l.name).collect::<Vec<_>>());
        assert_eq!(it.levels.iter().any(|l| l.roof), roof & 3 == 1, "{}: a walkable roof or not as chosen", c.id);
        for lv in it.levels.iter().filter(|l| l.z >= 0 && !l.roof) {
            let mut seen = vec![false; lv.rooms.len()];
            let st = lv.cells[it.stairs[1] * it.nx + it.stairs[0]];
            let first = lv.doors.iter().find(|d| d.kind == "front").map_or(st, |d| d.rooms[0].max(d.rooms[1]));
            let mut todo = vec![first];
            while let Some(r) = todo.pop() {
                if r < 0 || std::mem::replace(&mut seen[r as usize], true) {
                    continue;
                }
                todo.extend(lv.doors.iter().filter(|d| d.rooms.contains(&r)).map(|d| if d.rooms[0] == r { d.rooms[1] } else { d.rooms[0] }));
            }
            let used = |ri: usize| lv.cells.iter().any(|&c| c == ri as i16);
            assert!((0..lv.rooms.len()).all(|ri| seen[ri] || !used(ri)), "{} {}: a room no door reaches", c.id, lv.name);
        }
        let ground = it.levels.iter().find(|l| l.z == 0).unwrap().cells.iter().filter(|&&c| c >= 0).count() as f64 * 25.0;
        let area = geom::area(&c.poly).abs();
        assert!((ground - area).abs() < 0.25 * area, "{}: {ground} sq ft inside a {area} sq ft footprint", c.id);
        let key = TileKey::surface(g.max_level, (c.x / size) as u32, (c.y / size) as u32);
        let chunk = ex.battlemap(key);
        let k = chunk.buildings.iter().position(|b| b.0 as usize == li).expect("the building is not on the battlemap");
        assert_eq!(chunk.building_polys[k].2, roof, "{}: roof not as chosen", c.id);
    }
    let (world, t0) = (&ex.world, &ex.t0);
    let over = ell([at_inn[0] + 10.0, at_inn[1] + 10.0]);
    assert!(agent::building_spot(world, t0, &over, None, "c:99", None).is_err_and(|e| e.contains("overlaps")), "a building drawn over another is allowed");
    let skip = agent::spot_skip(world, t0, &inn.id);
    assert!(agent::building_spot(world, t0, &over, None, &inn.id, skip).is_ok(), "reshaping a building is refused by itself");
    let road = &t0.roads.roads[0];
    let rp = road.eval(road.pts.len() / 2, 0.5, 5.0, t0.cell_ft).p;
    let on_road = geom::circle(rp, 12.0, 12);
    assert!(agent::building_spot(world, t0, &on_road, None, "c:99", None).is_err(), "a building on a road is allowed");
    let ci = agent::layout_of(world, t0, &city).unwrap();
    let plaza = &town::layout(world, t0, ci).plazas[0];
    let on_plaza = geom::circle(geom::centroid(plaza), 8.0, 12);
    assert!(agent::building_spot(world, t0, &on_plaza, None, "c:99", None).is_err_and(|e| e.contains("square")), "a building on a market square is allowed");

    // The world's own buildings edited: one taken away (gone from the battlemap's buildings and
    // squares and from the gazetteer, its ground free to build on), one made a smithy of three
    // storeys (its interior is one); every other id and footprint stays, the hash too, the
    // chunk is repeatable; an edit whose building no longer stands where it was is set aside.
    use worldgen::world::BuildingEdit;
    let base_l = town::layout(world, t0, ci);
    let homes: Vec<&town::Building> = base_l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.func.is_none() && geom::area(&b.poly).abs() > 400.0).collect();
    let (gone, smithy, stale) = (homes[0], homes[1], homes[2]);
    let bid = |b: &town::Building| format!("b:{ci}:{}", b.id);
    let mut e = world.file.edits.clone();
    e.buildings.insert(bid(gone), agent::building_removal(world, t0, &bid(gone)).unwrap());
    let change = agent::BuildingChange { func: Some("blacksmith".into()), floors: Some(3), ..Default::default() };
    e.buildings.insert(bid(smithy), agent::building_edit(world, t0, &bid(smithy), &change).unwrap().expect("a change"));
    e.buildings.insert(bid(stale), BuildingEdit { at: [0.0, 0.0], removed: true, ..Default::default() });
    let back = serde_json::to_value(&e).unwrap();
    assert_eq!(serde_json::from_value::<worldgen::world::Edits>(back).unwrap(), e, "building edits changed on the round trip");
    let f = WorldFile { edits: e, ..world.file.clone() };
    let mut ex = Executor::new(World::new(f.clone()).unwrap());
    assert_eq!(ex.world.hash, base.hash, "building edits changed the world hash");
    let l = town::layout(&ex.world, &ex.t0, ci);
    assert!(l.building(gone.id as usize).is_none() && l.buildings.len() == base_l.buildings.len() - 1, "a removed building is still laid out");
    assert!(base_l.buildings.iter().filter(|b| b.id != gone.id).all(|b| l.building(b.id as usize).is_some_and(|n| n.poly == b.poly)), "another building moved or lost its id");
    assert!(l.building(stale.id as usize).is_some() && town::set_aside(&ex.world, &ex.t0).iter().any(|(k, _)| *k == bid(stale)), "an edit for a building no longer there was applied");
    let fi = town::catalog::index_of("blacksmith").unwrap();
    let s = l.building(smithy.id as usize).unwrap();
    assert!(s.func == Some(fi as u16) && s.floors == 3, "the changed building is not a three-storey smithy");
    let it = interior::generate_id(&ex.world, &ex.t0, &bid(smithy)).expect("the smithy's interior");
    assert!(it.function == town::catalog::CATALOG[fi].name && it.levels.iter().filter(|l| l.z >= 0 && !l.roof).count() == 3, "the interior is not the smithy's: {} {}", it.function, it.levels.len());
    let gc = geom::centroid(&gone.poly);
    let key = TileKey::surface(g.max_level, (gc[0] / size) as u32, (gc[1] / size) as u32);
    let chunk = ex.battlemap(key);
    assert!(!chunk.buildings.contains(&(ci as u32, gone.id)), "the removed building is on the battlemap");
    let (sx, sy) = (((gc[0] - key.x as f64 * size) / 5.0) as usize, ((gc[1] - key.y as f64 * size) / 5.0) as usize);
    assert_eq!(chunk.building[sy * worldgen::battlemap::SQ + sx], 0, "the removed building's squares are still built on");
    let packed = worldgen::battlemap::pack(&ex.world, &chunk);
    let again = ex.battlemap(key);
    assert_eq!(packed, worldgen::battlemap::pack(&ex.world, &again), "an edited chunk is not repeatable");
    let (world, t0) = (&ex.world, &ex.t0);
    assert!(!matches!(worldgen::gazetteer::query(world, t0, gc[0], gc[1]), Some(worldgen::gazetteer::Hit::Building { ref id, .. }) if *id == bid(gone)), "the gazetteer still finds the removed building");
    assert!(agent::building_spot(world, t0, &gone.poly, None, "c:99", None).is_ok(), "a removed building's ground is not free");
    assert!(agent::building_spot(world, t0, &smithy.poly, None, "c:99", None).is_err(), "a standing building's ground is free");

    // Castles and walls drawn by hand: a castle has a curtain with towers, a gatehouse and a
    // keep that is the castle (named as the site), and its towers and buildings keep the
    // interior rules; a wall gets a gate where a road crosses it, and a building on it is
    // refused; neither changes the world hash.
    let n0 = f.edits.created.len();
    let works = |kind: &str, id: String, poly: Vec<[f64; 2]>, pts: Vec<[f64; 2]>| Created { id, kind: kind.into(), poly, pts, ..Default::default() };
    let rect_at = |c: [f64; 2]| vec![[c[0] - 150.0, c[1] - 100.0], [c[0] + 150.0, c[1] - 100.0], [c[0] + 150.0, c[1] + 100.0], [c[0] - 150.0, c[1] + 100.0]];
    let castle_spot = (0..t0.settlements.len() * 8)
        .map(|k| {
            let s = &t0.settlements[k / 8];
            let a = (k % 8) as f64 * std::f64::consts::TAU / 8.0 + 0.3;
            [s.x + 15_840.0 * libm::cos(a), s.y + 15_840.0 * libm::sin(a)].map(|v| (v / 5.0).round() * 5.0)
        })
        .find(|p| t0.sample(p[0], p[1], 20.0) > sea + 20.0 && agent::works_spot(world, t0, &works("castle", "c:98".into(), rect_at(*p), vec![]), "c:98", None).is_ok_and(|s| s.in_way.is_empty()))
        .expect("open ground for a castle");
    let mut keep = works("castle", format!("c:{n0}"), rect_at(castle_spot), vec![]);
    let s = agent::works_spot(world, t0, &keep, &keep.id, None).unwrap();
    (keep.x, keep.y, keep.name) = (s.at[0], s.at[1], s.name);
    assert!(keep.check().is_ok() && keep.name.ends_with("Castle"), "a castle drawn by hand is refused: {:?}", keep.check());
    // A wall across a road, 150 ft each side of it, square to it.
    let (wall_pts, crossing) = t0
        .roads
        .roads
        .iter()
        .filter_map(|rc| {
            let k = rc.pts.len() / 2;
            let (a, b) = (rc.eval(k, 0.45, 5.0, t0.cell_ft).p, rc.eval(k, 0.55, 5.0, t0.cell_ft).p);
            let d = geom::sub(b, a);
            let nrm = geom::mul([-d[1], d[0]], 1.0 / geom::len(d).max(1e-9));
            let m = geom::lerp(a, b, 0.5);
            let pts = vec![geom::add(m, geom::mul(nrm, -150.0)), geom::add(m, geom::mul(nrm, 150.0))];
            let ok = agent::works_spot(world, t0, &works("wall", "c:98".into(), vec![], pts.clone()), "c:98", None).is_ok_and(|s| s.in_way.is_empty());
            (ok && geom_dist(m, castle_spot) > 2_000.0).then_some((pts, m))
        })
        .next()
        .expect("a road to wall across");
    let mut wall = works("wall", format!("c:{}", n0 + 1), vec![], wall_pts.clone());
    let s = agent::works_spot(world, t0, &wall, &wall.id, None).unwrap();
    (wall.x, wall.y, wall.name) = (s.at[0], s.at[1], s.name);
    assert!(wall.check().is_ok(), "a wall drawn by hand is refused: {:?}", wall.check());
    let mut e = f.edits.clone();
    for c in [&keep, &wall] {
        let v = serde_json::to_value(c).unwrap();
        e.apply(&worldgen::world::EditOp::Set { field: "created".into(), key: c.id.clone(), value: v.clone() }).unwrap();
        assert_eq!(serde_json::to_value(e.created.last().unwrap()).unwrap(), v, "{} changed on the round trip", c.kind);
    }
    let ex = Executor::new(World::new(WorldFile { edits: e, ..f.clone() }).unwrap());
    let (world, t0) = (&ex.world, &ex.t0);
    assert_eq!(world.hash, base.hash, "castles and walls changed the world hash");
    let li = agent::layout_of(world, t0, &keep.id).expect("the castle's layout");
    let l = town::layout(world, t0, li);
    let fi = town::catalog::index_of("castle").unwrap() as u16;
    let k = l.buildings.iter().find(|b| b.func == Some(fi)).expect("the castle has no keep");
    assert_eq!(k.name.as_deref(), Some(keep.name.as_str()), "the keep is not named as the castle");
    assert!(!l.walls.is_empty() && l.towers.len() >= 3 && l.gate_towers.len() == 2 && l.castles.len() == 1, "the castle's curtain: {} walls, {} towers, {} gate towers", l.walls.len(), l.towers.len(), l.gate_towers.len());
    assert!(l.walls.iter().flatten().all(|p| geom::contains(&keep.poly, *p)), "the curtain stands outside the outline drawn");
    let buckets = building_buckets(&l);
    let ids: Vec<String> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed).map(|b| format!("b:{li}:{}", b.id)).chain((0..interior::towers(&l).len()).map(|k| format!("t:{li}:{k}"))).collect();
    assert!(ids.len() >= 5, "too few castle interiors ({ids:?})");
    for id in &ids {
        check_interior(world, t0, &l, li, id, &buckets);
    }
    let wi = agent::layout_of(world, t0, &wall.id).expect("the wall's layout");
    let wl = town::layout(world, t0, wi);
    let gate = wl.gate_towers.chunks(2).find(|g| g.len() == 2 && geom_dist(geom::lerp(g[0], g[1], 0.5), crossing) < 40.0);
    assert!(gate.is_some(), "no gate where the road crosses the wall: gate towers {:?}, road at {crossing:?}", wl.gate_towers);
    assert!(wl.towers.len() >= 2, "a wall without towers at its ends");
    let on_wall = geom::lerp(wall_pts[0], crossing, 0.4);
    let hut = geom::circle(on_wall, 8.0, 12);
    assert!(agent::building_spot(world, t0, &hut, None, "c:99", None).is_err_and(|e| e.contains("wall")), "a building on a wall is allowed");
    assert!(agent::works_spot(world, t0, &works("castle", "c:99".into(), rect_at(castle_spot), vec![]), "c:99", None).is_err(), "a castle over a castle is allowed");
}

fn geom_dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Sketch guarantee (M8): a drawn coastline, ranges, a river, a painted biome and settlement
/// pins make a coherent world that follows them. Land well inside the outline and sea well
/// outside it; high ground along the ranges; a mapped river along the drawn one, flowing
/// downhill; the desert painted; pinned settlements of the right tier and name, near their
/// pins and (towns and up) on the road network; a pin far out at sea reported, not placed.
#[test]
fn sketch_guarantee() {
    const MI: f64 = 5280.0;
    let mi = |x: f64, y: f64| serde_json::json!([(x * MI).round(), (y * MI).round()]);
    let c = (600.0, 450.0);
    let outline: Vec<(f64, f64)> = (0..48)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / 48.0;
            let r = 1.0 + 0.12 * (3.0 * a).sin() + 0.08 * (5.0 * a + 1.0).cos();
            (c.0 + 430.0 * r * a.cos(), c.1 + 300.0 * r * a.sin())
        })
        .collect();
    let range = [(330.0, 260.0), (450.0, 280.0), (560.0, 330.0), (700.0, 300.0)];
    let river = [(250.0, 330.0), (300.0, 420.0), (360.0, 520.0), (380.0, 640.0), (380.0, 800.0)];
    let desert = [(930.0, 330.0), (1000.0, 380.0), (990.0, 520.0), (900.0, 500.0), (880.0, 400.0)];
    let pins = [("metropolis", Some("Kingsreach"), (585.0, 690.0)), ("city", Some("Highmoor"), (450.0, 340.0)), ("town", None, (780.0, 420.0)), ("village", Some("Little Ford"), (300.0, 520.0))];
    let mut strokes = vec![
        serde_json::json!({ "tool": "land", "closed": true, "pts": outline.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
        serde_json::json!({ "tool": "range", "radius_ft": 18.0 * MI, "strength": 0.9, "pts": range.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
        serde_json::json!({ "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.8, "pts": river.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
        serde_json::json!({ "tool": "biome", "biome": "hot_desert", "closed": true, "pts": desert.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
    ];
    for (tier, name, p) in pins {
        strokes.push(serde_json::json!({ "tool": "pin", "tier": tier, "name": name, "pts": [mi(p.0, p.1)] }));
    }
    strokes.push(serde_json::json!({ "tool": "pin", "tier": "town", "name": "Lost", "pts": [mi(60.0, 60.0)] }));
    let file: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 5, "sketch": { "strokes": strokes } })).unwrap();
    let world = World::new(file).unwrap();
    let start = std::time::Instant::now();
    let t0 = worldgen::t0::T0::generate(&world);
    println!("sketched T0 in {:.1} s", start.elapsed().as_secs_f64());
    let extra = t0.extra.as_ref().unwrap();
    let sea = world.params().sea_level_ft;
    let cell = t0.cell_ft;
    let idx = |x: f64, y: f64| ((y / cell).round() as usize).min(t0.height.h - 1) * t0.height.w + ((x / cell).round() as usize).min(t0.height.w - 1);
    let dry = |x: f64, y: f64| t0.height.data[idx(x * MI, y * MI)] as f64 > sea && t0.water.data[idx(x * MI, y * MI)] <= worldgen::t0::hydro::DRY;

    // Coastline: 60+ mi inside the outline is land (lakes aside), 60+ mi outside is sea.
    let (mut inside_ok, mut outside_ok) = (0, 0);
    for p in &outline {
        let (dx, dy) = (p.0 - c.0, p.1 - c.1);
        let l = (dx * dx + dy * dy).sqrt();
        let (ix, iy) = (p.0 - dx / l * 60.0, p.1 - dy / l * 60.0);
        inside_ok += usize::from(t0.height.data[idx(ix * MI, iy * MI)] as f64 > sea);
        outside_ok += usize::from(!dry(p.0 + dx / l * 60.0, p.1 + dy / l * 60.0));
    }
    assert!(inside_ok >= 46 && outside_ok >= 46, "coastline not followed: {inside_ok}/48 inside land, {outside_ok}/48 outside sea");

    // Range: high ground all along it.
    let mut high = 0;
    let mut samples = 0;
    for w in range.windows(2) {
        for t in 0..10 {
            let f = t as f64 / 10.0;
            let (x, y) = (w[0].0 + (w[1].0 - w[0].0) * f, w[0].1 + (w[1].1 - w[0].1) * f);
            let mut top = f64::MIN;
            for dy in -6..=6 {
                for dx in -6..=6 {
                    top = top.max(t0.height.data[idx((x + dx as f64) * MI, (y + dy as f64) * MI)] as f64 - sea);
                }
            }
            high += usize::from(top > 4_000.0);
            samples += 1;
        }
    }
    assert!(high * 10 >= samples * 8, "range too low along its line: {high}/{samples} samples above 4,000 ft");

    // River: a mapped river passes every drawn waypoint on land, and all rivers flow downhill.
    let hydro = &extra.hydro;
    let near_river = |x: f64, y: f64| {
        let k = idx(x * MI, y * MI);
        let (i, j) = ((k % t0.height.w) as i64, (k / t0.height.w) as i64);
        hydro.rivers.iter().any(|r| r.cells.iter().any(|&c| ((c as usize % t0.height.w) as i64 - i).abs() <= 2 && ((c as usize / t0.height.w) as i64 - j).abs() <= 2))
    };
    for p in river.iter().filter(|p| dry(p.0, p.1)) {
        assert!(near_river(p.0, p.1), "no mapped river near the drawn river at ({}, {}) mi", p.0, p.1);
    }
    for r in &hydro.rivers {
        for pair in r.cells.windows(2) {
            let (a, b) = (pair[0] as usize, pair[1] as usize);
            let into_water = t0.height.data[b] as f64 <= sea || hydro.lake_of[b] != worldgen::t0::hydro::NO_LAKE;
            assert!(into_water || t0.height.data[b] < t0.height.data[a], "a river climbs in the sketched world");
        }
    }

    // Biome paint: the desert's interior is desert.
    let mut desert_cells = 0;
    let mut tried = 0;
    for dy in -40..=40 {
        for dx in -30..=30 {
            let (x, y) = (940.0 + dx as f64, 430.0 + dy as f64);
            if dry(x, y) {
                tried += 1;
                desert_cells += usize::from(t0.biome.data[idx(x * MI, y * MI)] & 0xff == worldgen::t0::biome::Biome::HotDesert as u32);
            }
        }
    }
    assert!(tried > 100 && desert_cells * 10 >= tried * 9, "painted desert not desert: {desert_cells}/{tried}");

    // Pins: placed, named, near their pins, on the roads; the one at sea is reported.
    let towns: Vec<_> = extra.overlay.features.iter().filter(|f| ["metropolis", "city", "town", "village"].contains(&f.kind)).collect();
    assert_eq!(towns.len(), t0.settlements.len());
    for (k, (tier, name, p)) in pins.iter().enumerate() {
        let stroke = 4 + k as u32;
        let i = t0.settlements.iter().position(|s| s.pin == Some(stroke)).unwrap_or_else(|| panic!("pin {stroke} placed no settlement"));
        let (s, f) = (&t0.settlements[i], towns[i]);
        assert_eq!(f.kind, *tier, "pin {stroke} has the wrong tier");
        if let Some(n) = name {
            assert_eq!(f.name, *n, "pin {stroke} lost its name");
        }
        let d = ((s.x / MI - p.0).powi(2) + (s.y / MI - p.1).powi(2)).sqrt();
        assert!(d < 15.0, "pin {stroke} settled {d:.1} mi from where it was drawn");
        let road = t0.roads.roads.iter().any(|r| r.pts.iter().any(|q| ((q[0] - s.x).powi(2) + (q[1] - s.y).powi(2)).sqrt() < 3.0 * MI));
        // (Villages may be off the network, as generated ones are; towns and up never.)
        assert!(road || f.kind == "village", "pinned {} {} has no road", f.kind, f.name);
    }
    assert!(!t0.settlements.iter().any(|s| s.pin == Some(8)), "a settlement was placed far out at sea");
    assert!(extra.overlay.conflicts.iter().any(|c| c.stroke == 8), "the pin at sea was not reported");

    sketch_rivers_and_ports();
    sketch_relief_and_lakes();
    sketch_names_and_sites();
}

/// Sketch guarantee (R2): drawn rivers stay rivers and coasts keep their ports. On a low plain
/// with a hard coast, a long river drawn to stop a few miles short of the sea and a tributary
/// drawn into it (drawn first: the order strokes are drawn in doesn't matter) both flow on to
/// the sea, are mapped as rivers over at least 90% of their drawn courses with hardly any lake
/// along them, no dry land lies below sea level, and a town pinned on the coast over barren
/// ground is a port.
fn sketch_rivers_and_ports() {
    const MI: f64 = 5280.0;
    let mi = |p: &(f64, f64)| serde_json::json!([(p.0 * MI).round(), (p.1 * MI).round()]);
    let coast = [(30.0, 30.0), (390.0, 30.0), (390.0, 270.0), (30.0, 270.0)];
    let main = [(60.0, 140.0), (140.0, 160.0), (220.0, 140.0), (300.0, 160.0), (384.0, 150.0)];
    let tributary = [(150.0, 60.0), (190.0, 100.0), (220.0, 140.0)];
    let port = (389.6, 220.0);
    let barren = [(370.0, 205.0), (400.0, 205.0), (400.0, 235.0), (370.0, 235.0)];
    let strokes = serde_json::json!([
        { "tool": "land", "closed": true, "hard": true, "pts": coast.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "river", "radius_ft": 2.0 * MI, "strength": 0.5, "pts": tributary.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.7, "pts": main.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "biome", "biome": "volcanic", "closed": true, "hard": true, "pts": barren.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "pin", "tier": "town", "name": "Saltmouth", "pts": [mi(&port)] },
    ]);
    let params = serde_json::json!({ "width_mi": 420.0, "height_mi": 300.0, "max_elev_ft": 1000.0, "ruggedness": 0.1, "settlement_density": 0.0 });
    let file: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 7, "params": params, "sketch": { "strokes": strokes } })).unwrap();
    let world = World::new(file).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let extra = t0.extra.as_ref().unwrap();
    let hydro = &extra.hydro;
    let sea = world.params().sea_level_ft;
    let (w, h, cell) = (t0.height.w, t0.height.h, t0.cell_ft);
    let no_lake = worldgen::t0::hydro::NO_LAKE;

    // Each drawn river: its cells on land, a mapped river within a cell of each.
    let mut river_cell = vec![false; w * h];
    for r in &hydro.rivers {
        for &c in &r.cells {
            river_cell[c as usize] = true;
        }
    }
    for (name, pts) in [("long river", &main[..]), ("tributary", &tributary[..])] {
        let (mut on, mut all, mut lake) = (0, 0, 0);
        let mut last = usize::MAX;
        for seg in pts.windows(2) {
            for t in 0..=200 {
                let f = t as f64 / 200.0;
                let (x, y) = ((seg[0].0 + (seg[1].0 - seg[0].0) * f) * MI, (seg[0].1 + (seg[1].1 - seg[0].1) * f) * MI);
                let (i, j) = (((x / cell).round() as usize).min(w - 1), ((y / cell).round() as usize).min(h - 1));
                let k = j * w + i;
                if k == last || (t0.height.data[k] as f64 <= sea && hydro.lake_of[k] == no_lake && t0.water.data[k] > worldgen::t0::hydro::DRY) {
                    continue;
                }
                last = k;
                all += 1;
                lake += usize::from(hydro.lake_of[k] != no_lake);
                on += usize::from((j.saturating_sub(1)..=(j + 1).min(h - 1)).any(|y| (i.saturating_sub(1)..=(i + 1).min(w - 1)).any(|x| river_cell[y * w + x])));
            }
        }
        assert!(on * 10 >= all * 9, "the drawn {name} is mapped as a river over only {on}/{all} cells ({lake} in lakes)");
        assert!(lake * 20 <= all, "the drawn {name} runs through lakes over {lake}/{all} cells");
    }
    // (Both reach the sea: neither is left ending inland.)
    let stuck: Vec<&str> = extra.overlay.conflicts.iter().filter(|c| c.stroke <= 2 && c.message.contains("inland")).map(|c| c.message.as_str()).collect();
    assert!(stuck.is_empty(), "a drawn river did not flow on to the sea: {stuck:?}");

    // No dry land below sea level (a river or plain dug under the sea).
    let low = (0..w * h).filter(|&k| (t0.height.data[k] as f64) < sea && t0.water.data[k] <= worldgen::t0::hydro::DRY && hydro.lake_of[k] == no_lake).count();
    assert_eq!(low, 0, "{low} cells of dry land below sea level");

    // The pin on the barren coast is a port.
    let s = t0.settlements.iter().find(|s| s.pin == Some(4)).expect("the coastal pin placed no settlement");
    assert!(s.coastal && s.kind == worldgen::t0::settle::SettleKind::Port, "the town pinned on the coast is not a port ({:?}, coastal {})", s.kind, s.coastal);
}

/// Sketch guarantee (R2.4): relief and water drawn as areas and points. With the plates'
/// mountains off, the land is high only where drawn: a massif is mountains across its outline,
/// a plateau stands its height above the land round it without damming the streams that cross
/// it, and the plains stay low. A lake drawn on the plain fills its outline (and little more),
/// keeps its name and, with a river drawn through it, is fresh and the river runs on out of it.
/// Drawn volcanoes stand where drawn, as drawn, and no others are placed.
fn sketch_relief_and_lakes() {
    const MI: f64 = 5280.0;
    let p = |x: f64, y: f64| serde_json::json!([(x * MI).round(), (y * MI).round()]);
    let ring = |cx: f64, cy: f64, rx: f64, ry: f64, n: usize, wob: f64| -> Vec<(f64, f64)> {
        (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                let r = 1.0 + wob * (3.0 * a).sin() + wob * 0.6 * (5.0 * a + 1.0).cos();
                (cx + rx * r * a.cos(), cy + ry * r * a.sin())
            })
            .collect()
    };
    let pts = |v: &[(f64, f64)]| v.iter().map(|q| p(q.0, q.1)).collect::<Vec<_>>();
    let massif = ring(170.0, 130.0, 90.0, 45.0, 20, 0.08);
    let plateau = ring(390.0, 285.0, 80.0, 40.0, 20, 0.08);
    let lake = ring(350.0, 150.0, 18.0, 10.0, 16, 0.08);
    let river = [(210.0, 160.0), (280.0, 165.0), (330.0, 152.0), (370.0, 148.0), (450.0, 130.0), (520.0, 110.0), (575.0, 100.0)];
    let strokes = serde_json::json!([
        { "tool": "land", "closed": true, "pts": pts(&ring(300.0, 200.0, 260.0, 165.0, 48, 0.05)) },
        { "tool": "massif", "closed": true, "radius_ft": 12.0 * MI, "strength": 0.8, "pts": pts(&massif) },
        { "tool": "elevation", "closed": true, "radius_ft": 10.0 * MI, "delta_ft": 1200.0, "pts": pts(&plateau) },
        { "tool": "lake", "closed": true, "name": "Mirrormere", "pts": pts(&lake) },
        { "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.6, "pts": pts(&river) },
        { "tool": "volcano", "kind": "caldera", "activity": "dormant", "name": "Mount Ash", "strength": 0.7, "pts": [p(450.0, 230.0)] },
        { "tool": "volcano", "kind": "strato", "activity": "active", "name": "Emberhorn", "strength": 0.8, "pts": [p(120.0, 280.0)] },
    ]);
    let params = serde_json::json!({ "width_mi": 600.0, "height_mi": 400.0, "procedural_mountains": 0.0, "volcanoes": 0 });
    let file: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 5, "params": params, "sketch": { "strokes": strokes } })).unwrap();
    let world = World::new(file).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let extra = t0.extra.as_ref().unwrap();
    let hydro = &extra.hydro;
    let sea = world.params().sea_level_ft;
    let (w, h, cell) = (t0.height.w, t0.height.h, t0.cell_ft);
    let no_lake = worldgen::t0::hydro::NO_LAKE;
    let inside = |poly: &[(f64, f64)], x: f64, y: f64| {
        let mut ins = false;
        for e in 0..poly.len() {
            let (a, b) = (poly[e], poly[(e + 1) % poly.len()]);
            if (a.1 <= y) != (b.1 <= y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                ins = !ins;
            }
        }
        ins
    };
    // Distance (mi) from a point to an outline's edge.
    let edge = |poly: &[(f64, f64)], x: f64, y: f64| {
        (0..poly.len())
            .map(|e| {
                let (a, b) = (poly[e], poly[(e + 1) % poly.len()]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                ((a.0 + t * dx - x).powi(2) + (a.1 + t * dy - y).powi(2)).sqrt()
            })
            .fold(f64::MAX, f64::min)
    };
    let at = |k: usize| ((k % w) as f64 * cell / MI, (k / w) as f64 * cell / MI);
    let above = |k: usize| t0.height.data[k] as f64 - sea;
    let land = |k: usize| t0.height.data[k] as f64 > sea || hydro.lake_of[k] != no_lake;

    // The massif: mountains across it (10+ mi inside, the 80th percentile over 4,000 ft).
    let mut core: Vec<f64> = (0..w * h).filter(|&k| { let (x, y) = at(k); inside(&massif, x, y) && edge(&massif, x, y) > 10.0 }).map(above).collect();
    core.sort_by(|a, b| a.total_cmp(b));
    let m80 = core[core.len() * 8 / 10];
    assert!(m80 > 4_000.0, "the massif is not mountains across its outline: 80th percentile {m80:.0} ft");

    // The plains (land 40+ mi from anything drawn): low, with the plates' mountains off.
    let mut plains: Vec<f64> = (0..w * h)
        .filter(|&k| {
            let (x, y) = at(k);
            land(k) && edge(&massif, x, y) > 40.0 && !inside(&massif, x, y) && edge(&plateau, x, y) > 40.0 && !inside(&plateau, x, y) && ((x - 450.0).powi(2) + (y - 230.0).powi(2)).sqrt() > 40.0 && ((x - 120.0).powi(2) + (y - 280.0).powi(2)).sqrt() > 40.0
        })
        .map(above)
        .collect();
    plains.sort_by(|a, b| a.total_cmp(b));
    let p95 = plains[plains.len() * 95 / 100];
    assert!(p95 < 2_500.0, "the plains rise to {p95:.0} ft (95th percentile) with the plates' mountains off");

    // The plateau: 1,200 ft drawn, at least 900 ft over the land round it (median inside 10+ mi
    // from its edge vs a ring 15–35 mi out), and no undrawn lakes dammed round it.
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    let top = median((0..w * h).filter(|&k| { let (x, y) = at(k); land(k) && inside(&plateau, x, y) && edge(&plateau, x, y) > 10.0 }).map(above).collect());
    let round = median((0..w * h).filter(|&k| { let (x, y) = at(k); land(k) && !inside(&plateau, x, y) && (15.0..35.0).contains(&edge(&plateau, x, y)) }).map(above).collect());
    assert!(top - round > 900.0, "the plateau stands only {:.0} ft over the land round it", top - round);
    let near: Vec<usize> = (0..w * h).filter(|&k| { let (x, y) = at(k); land(k) && (inside(&plateau, x, y) || edge(&plateau, x, y) < 30.0) }).collect();
    let dammed = near.iter().filter(|&&k| hydro.lake_of[k] != no_lake && hydro.lakes[hydro.lake_of[k] as usize].stroke.is_none()).count();
    assert!(dammed * 50 <= near.len(), "lakes dammed round the plateau: {dammed}/{} cells", near.len());

    // The lake: its outline's cells are its water, hardly any beyond; named, fresh.
    let li = hydro.lakes.iter().position(|l| l.stroke == Some(3)).expect("the drawn lake is not a lake");
    let lk = &hydro.lakes[li];
    let ins = (0..w * h).filter(|&k| { let (x, y) = at(k); inside(&lake, x, y) }).count();
    let wet = lk.cells.iter().filter(|&&c| { let (x, y) = at(c as usize); inside(&lake, x, y) }).count();
    assert!(wet * 10 >= ins * 9 && (lk.cells.len() - wet) * 10 <= ins, "the drawn lake: {wet}/{ins} cells of its outline wet, {} outside", lk.cells.len() - wet);
    assert_eq!(lk.kind, worldgen::t0::hydro::LakeKind::Fresh, "a lake with a river drawn out of it is not fresh");
    let feature = extra.overlay.features.iter().find(|f| f.name == "Mirrormere").expect("the drawn lake lost its name");
    assert!(feature.kind == "lake" && edge(&lake, feature.x / MI, feature.y / MI) < 20.0, "Mirrormere is a {} at ({:.0}, {:.0}) mi", feature.kind, feature.x / MI, feature.y / MI);

    // The river through it: mapped as a river (lake and sea aside) over 90%+ of its course.
    let mut river_cell = vec![false; w * h];
    for r in &hydro.rivers {
        for &c in &r.cells {
            river_cell[c as usize] = true;
        }
    }
    let (mut on, mut all, mut last) = (0, 0, usize::MAX);
    for seg in river.windows(2) {
        for t in 0..=200 {
            let f = t as f64 / 200.0;
            let (x, y) = (seg[0].0 + (seg[1].0 - seg[0].0) * f, seg[0].1 + (seg[1].1 - seg[0].1) * f);
            let (i, j) = (((x * MI / cell).round() as usize).min(w - 1), ((y * MI / cell).round() as usize).min(h - 1));
            let k = j * w + i;
            if k == last || !land(k) || hydro.lake_of[k] != no_lake || inside(&lake, x, y) {
                continue;
            }
            last = k;
            all += 1;
            on += usize::from((j.saturating_sub(1)..=(j + 1).min(h - 1)).any(|y| (i.saturating_sub(1)..=(i + 1).min(w - 1)).any(|x| river_cell[y * w + x])));
        }
    }
    assert!(on * 10 >= all * 9, "the river drawn through the lake is mapped over only {on}/{all} cells");

    // Volcanoes: the two drawn, where drawn, as drawn; none placed.
    let volcanoes: Vec<_> = extra.overlay.features.iter().filter(|f| f.kind == "volcano").collect();
    assert_eq!(volcanoes.len(), 2, "volcanoes: {:?}", volcanoes.iter().map(|f| &f.name).collect::<Vec<_>>());
    for (name, x, y, what) in [("Mount Ash", 450.0, 230.0, "dormant caldera"), ("Emberhorn", 120.0, 280.0, "active stratovolcano")] {
        let f = volcanoes.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("the volcano {name} lost its name"));
        let d = ((f.x / MI - x).powi(2) + (f.y / MI - y).powi(2)).sqrt();
        assert!(d < 2.0 && f.detail.as_deref().is_some_and(|s| s.starts_with(what)), "{name}: {:?}, {d:.1} mi from where drawn", f.detail);
    }
    println!("relief and lakes: massif 80th pct {m80:.0} ft, plains 95th pct {p95:.0} ft, plateau +{:.0} ft ({dammed} dammed cells), lake {wet}/{ins} (+{}), river {on}/{all}", top - round, lk.cells.len() - wet);
}

/// Sketch guarantee (R3): what the sketch names keeps its name, and what it places stays put.
/// Two named mountain strokes drawn into one range name its two parts; a named river, a region
/// outline, two region names in one plain (each naming its own part), a named blighted wood and
/// named ashlands (both making battlemaps) name what they were drawn on; a pinned capital is the capital, a pinned fortress a fortress,
/// a pin's wards name its districts (the central one first); a site drawn as a ruin with a crypt
/// is one, named, and stays where it was, with its crypt, when the sketch changes elsewhere.
fn sketch_names_and_sites() {
    const MI: f64 = 5280.0;
    let p = |x: f64, y: f64| serde_json::json!([(x * MI).round(), (y * MI).round()]);
    let ring = |cx: f64, cy: f64, rx: f64, ry: f64, n: usize| -> Vec<serde_json::Value> {
        (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                p(cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect()
    };
    let wards = ["Old Market", "Crown Hill", "Tanner's Row"];
    let mut strokes = vec![
        serde_json::json!({ "tool": "land", "closed": true, "pts": ring(300.0, 200.0, 270.0, 175.0, 48) }),
        serde_json::json!({ "tool": "massif", "closed": true, "radius_ft": 10.0 * MI, "strength": 0.9, "name": "Greyspine Massif", "pts": ring(150.0, 110.0, 70.0, 40.0, 20) }),
        serde_json::json!({ "tool": "range", "radius_ft": 15.0 * MI, "strength": 0.9, "name": "Silberquel Ridge", "pts": [p(215.0, 110.0), p(280.0, 100.0), p(340.0, 90.0)] }),
        serde_json::json!({ "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.8, "name": "Glory Run", "pts": [p(330.0, 140.0), p(380.0, 200.0), p(450.0, 260.0), p(520.0, 330.0), p(560.0, 380.0)] }),
        serde_json::json!({ "tool": "biome", "biome": "grassland", "closed": true, "hard": true, "pts": [p(60.0, 190.0), p(300.0, 190.0), p(300.0, 330.0), p(60.0, 330.0)] }),
        serde_json::json!({ "tool": "region", "name": "Zemni Fields", "pts": [p(110.0, 250.0)] }),
        serde_json::json!({ "tool": "region", "name": "Marrow Valley", "pts": [p(250.0, 260.0)] }),
        serde_json::json!({ "tool": "region", "name": "Truscan Vale", "closed": true, "pts": [p(420.0, 160.0), p(470.0, 160.0), p(470.0, 200.0), p(420.0, 200.0)] }),
        serde_json::json!({ "tool": "biome", "biome": "blighted_woods", "closed": true, "name": "The Pallid Grove", "pts": ring(470.0, 110.0, 30.0, 22.0, 16) }),
        serde_json::json!({ "tool": "biome", "biome": "ashlands", "closed": true, "name": "The Cinderwaste", "pts": ring(150.0, 290.0, 30.0, 20.0, 16) }),
        serde_json::json!({ "tool": "site", "kind": "ruin", "under": "crypt", "name": "Barrow of Kings", "pts": [p(350.0, 300.0)] }),
        serde_json::json!({ "tool": "pin", "tier": "town", "capital": true, "name": "Rexxentrum", "pts": [p(200.0, 230.0)] }),
        serde_json::json!({ "tool": "pin", "tier": "city", "kind": "fortress", "name": "Bladegarden", "wards": wards, "pts": [p(380.0, 280.0)] }),
        // A king's road bending south between the two pins, and a line no planned road crosses.
        serde_json::json!({ "tool": "road", "kind": "kings_road", "name": "The Gilded Roadway", "pts": [p(201.0, 232.0), p(225.0, 275.0), p(270.0, 305.0), p(330.0, 305.0), p(378.0, 282.0)] }),
        serde_json::json!({ "tool": "road", "kind": "none", "pts": [p(150.0, 150.0), p(150.0, 215.0), p(140.0, 260.0)] }),
    ];
    let params = serde_json::json!({ "width_mi": 600.0, "height_mi": 400.0, "volcanoes": 0 });
    let file = |strokes: &[serde_json::Value]| -> World {
        let f: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 9, "params": params, "sketch": { "strokes": strokes } })).unwrap();
        World::new(f).unwrap()
    };
    let mut ex = Executor::new(file(&strokes));
    // The painted blighted woods and ashlands make battlemaps, with their objects.
    for (x, y) in [(470.0, 110.0), (150.0, 290.0)] {
        let size = ex.world.geom.tile_size_ft(ex.world.geom.max_level);
        let key = TileKey::surface(ex.world.geom.max_level, (x * MI / size) as u32, (y * MI / size) as u32);
        assert!(!ex.battlemap(key).objects.is_empty(), "no battlemap objects in the painted biome at ({x}, {y}) mi");
    }
    let (world, t0) = (&ex.world, &ex.t0);
    let extra = t0.extra.as_ref().unwrap();
    let feats = &extra.overlay.features;
    let named = |name: &str| feats.iter().filter(|f| f.name == name).collect::<Vec<_>>();
    let conflicts: Vec<&str> = extra.overlay.conflicts.iter().map(|c| c.message.as_str()).collect();

    for (name, kind) in [("Greyspine Massif", "range"), ("Silberquel Ridge", "range"), ("Glory Run", "river"), ("Zemni Fields", "plains"), ("Marrow Valley", "plains"), ("The Pallid Grove", "blight"), ("The Cinderwaste", "ashlands")] {
        let fs = named(name);
        assert!(fs.iter().any(|f| f.kind == kind), "no {kind} named {name} (named so: {:?}; conflicts {conflicts:?})", fs.iter().map(|f| f.kind).collect::<Vec<_>>());
    }
    let vale = named("Truscan Vale");
    assert_eq!(vale.len(), 1, "the region outline named {} features", vale.len());
    // The two plains names name two parts of the plain, each holding its own name's point.
    let part = |name: &str| extra.overlay.shapes[feats.iter().position(|f| f.name == name && f.kind == "plains").unwrap()].clone();
    assert!(part("Zemni Fields").distance([110.0 * MI, 250.0 * MI]).0 == 0.0 && part("Marrow Valley").distance([250.0 * MI, 260.0 * MI]).0 == 0.0, "the plain is not split between its names");

    // Pins: the capital, the fortress and its wards.
    let at = |name: &str| t0.settlements.iter().position(|s| feats.iter().any(|f| f.name == name && (f.x - s.x).abs() < 1.0 && (f.y - s.y).abs() < 1.0)).unwrap_or_else(|| panic!("no settlement {name}"));
    let (capital, fort) = (at("Rexxentrum"), at("Bladegarden"));
    assert!(t0.settlements[capital].capital && t0.settlements.iter().filter(|s| s.capital).count() == 1, "the pinned capital is not the only capital");
    assert_eq!(t0.settlements[fort].kind, worldgen::t0::settle::SettleKind::Fortress, "the pinned fortress is not a fortress");
    let layout = worldgen::town::generate(&world, &t0, fort);
    let centre = layout.quarters.iter().find(|q| q.kind == worldgen::town::QuarterKind::Center).expect("a centre");
    assert_eq!(centre.name, wards[0], "the first ward doesn't name the central district");
    assert!(wards[1..].iter().all(|w| layout.quarters.iter().any(|q| q.name == *w)), "the wards don't name districts");

    // The drawn road: followed (every half mile of its line within a mile of the road made from
    // it), named, joining the pins at its ends; no planned road crosses the `none` line.
    let seg = |q: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let t = (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
        ((a[0] + t * dx - q[0]).powi(2) + (a[1] + t * dy - q[1]).powi(2)).sqrt()
    };
    let strokes_of = |tool_kind: Option<&str>| world.file.sketch.strokes.iter().position(|s| s.tool == worldgen::world::SketchTool::Road && s.kind.as_deref() == tool_kind).unwrap();
    let (gilded, none) = (strokes_of(Some("kings_road")), strokes_of(Some("none")));
    let made: Vec<&worldgen::lod::roads::RoadCurve> = t0.roads.roads.iter().filter(|r| r.stroke == Some(gilded as u32)).collect();
    let line = &world.file.sketch.strokes[gilded].pts;
    let (mut n, mut far) = (0, Vec::new());
    for w in line.windows(2) {
        let k = (((w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])) / (0.5 * MI)).ceil() as usize;
        for j in 0..k {
            let q = [w[0][0] + (w[1][0] - w[0][0]) * j as f64 / k as f64, w[0][1] + (w[1][1] - w[0][1]) * j as f64 / k as f64];
            let d = made.iter().flat_map(|r| r.pts.windows(2).map(|v| seg(q, v[0], v[1]))).fold(f64::INFINITY, f64::min);
            n += 1;
            if d > MI {
                far.push((q[0] / MI, q[1] / MI, d / MI));
            }
        }
    }
    assert!(far.len() * 20 <= n, "the drawn road strays over a mile from its line at {} of {n} points: {:?}", far.len(), &far[..far.len().min(6)]);
    assert!(made.iter().all(|r| r.class == worldgen::t0::roads::RoadClass::KingsRoad), "the drawn king's road isn't one");
    assert!(feats.iter().any(|f| f.kind == "road" && f.name == "The Gilded Roadway"), "the named road is not a feature");
    let near_pin = |name: &str| {
        let s = &t0.settlements[at(name)];
        let r = worldgen::town::road_trim_radius(s.tier, s.population) + 3_000.0;
        made.iter().any(|c| [c.pts[0], *c.pts.last().unwrap()].iter().any(|e| (e[0] - s.x).hypot(e[1] - s.y) < r))
    };
    assert!(near_pin("Rexxentrum") && near_pin("Bladegarden"), "the drawn road doesn't reach the pins at its ends");
    let cut = &world.file.sketch.strokes[none].pts;
    let crossing = t0.roads.roads.iter().filter(|r| r.stroke.is_none()).find_map(|r| {
        r.pts.windows(2).find_map(|v| {
            let side = |p: [f64; 2], q: [f64; 2], o: [f64; 2]| (q[0] - p[0]) * (o[1] - p[1]) - (q[1] - p[1]) * (o[0] - p[0]);
            cut.windows(2).find(|c| side(v[0], v[1], c[0]) * side(v[0], v[1], c[1]) < 0.0 && side(c[0], c[1], v[0]) * side(c[0], c[1], v[1]) < 0.0).map(|_| (v[0][0] / MI, v[0][1] / MI))
        })
    });
    assert!(crossing.is_none(), "a planned road crosses the no-road line at {crossing:?} mi");

    // The site: a named ruin over a crypt, where it was drawn; still there when the sketch changes
    // elsewhere.
    let site = |t0: &worldgen::t0::T0| {
        let i = (0..t0.base_pois).find(|&i| t0.pois[i].stroke.is_some()).expect("the site was not placed");
        (i, t0.pois[i].kind, t0.pois[i].x, t0.pois[i].y, t0.created_site(i).and_then(|c| c.under), t0.pois[i].seed)
    };
    let (i, kind, x, y, under, seed) = site(&t0);
    assert!(kind == worldgen::t0::settle::PoiKind::Ruin && under == Some(worldgen::under::UnderKind::Crypt), "the site is not a ruin over a crypt");
    assert!(((x / MI - 350.0).powi(2) + (y / MI - 300.0).powi(2)).sqrt() < 2.0, "the site moved from where it was drawn");
    let id = feats.iter().find(|f| f.name == "Barrow of Kings").expect("the site lost its name").id.clone();
    strokes.push(serde_json::json!({ "tool": "pin", "tier": "village", "name": "Elsewhere", "pts": [p(520.0, 120.0)] }));
    let world2 = file(&strokes);
    let mut t2 = worldgen::t0::T0::generate(&world2);
    t2.apply_edits(&world2);
    let (_, kind2, x2, y2, under2, seed2) = site(&t2);
    assert!(kind2 == kind && under2 == under && (x2 - x).abs() < 1.0 && (y2 - y).abs() < 1.0 && seed2 == seed, "the site changed with the sketch elsewhere");
    let f2 = t2.extra.as_ref().unwrap().overlay.features.iter().find(|f| f.name == "Barrow of Kings").expect("the site lost its name");
    assert_eq!(f2.id, id, "the site's id changed with the sketch elsewhere");
    let _ = i;

    // Generated roads off: only the drawn road and short spurs from it to the settlements beside
    // it, no roadside inns; the settlements stand where they would without the road.
    let off = |strokes: &[serde_json::Value]| {
        let mut f = file(strokes).file;
        f.params.generated_roads = false;
        let w = World::new(f).unwrap();
        let t = worldgen::t0::T0::generate(&w);
        (w, t)
    };
    let roads = &strokes[strokes.len() - 3..strokes.len() - 1];
    assert!(roads.iter().all(|s| s["tool"] == "road"), "the road strokes are not where this test expects them");
    let (w_road, t_road) = off(&strokes[..strokes.len() - 1]);
    let (_, t_none) = off(&strokes[..strokes.len() - 3]);
    let length = |r: &worldgen::lod::roads::RoadCurve| r.pts.windows(2).map(|v| (v[1][0] - v[0][0]).hypot(v[1][1] - v[0][1])).sum::<f64>();
    let gilded = w_road.file.sketch.strokes.iter().position(|s| s.kind.as_deref() == Some("kings_road")).unwrap() as u32;
    assert!(t_road.roads.roads.iter().any(|r| r.stroke == Some(gilded)), "the drawn road is not made with generated roads off");
    let long: Vec<f64> = t_road.roads.roads.iter().filter(|r| r.stroke.is_none()).map(length).filter(|&l| l > 10.0 * MI).map(|l| l / MI).collect();
    assert!(long.is_empty(), "with generated roads off, roads not drawn run {long:?} mi");
    assert!(t_none.roads.roads.is_empty(), "with generated roads off and none drawn, there are roads");
    assert!(!t_road.pois.iter().any(|p| p.kind == worldgen::t0::settle::PoiKind::Waystation), "roadside inns made with generated roads off");
    let places = |t: &worldgen::t0::T0| t.settlements.iter().map(|s| (s.x.round() as i64, s.y.round() as i64, s.tier as u8)).collect::<Vec<_>>();
    assert_eq!(places(&t_road), places(&t_none), "drawing a road moved settlements with generated roads off");
}
