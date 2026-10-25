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
    // Flights of steps (settlement, centreline, width, rise), for the drawn ground below.
    let prm = town::stairs::StairParams::DEFAULT;
    let mut flights: Vec<(usize, Vec<[f64; 2]>, f32)> = Vec::new();
    let mut stair_pieces = 0;
    for (i, s) in t0.settlements.iter().enumerate() {
        let l = town::layout(world, t0, i);
        all_piers.extend(l.piers.iter().map(|p| (i, ends(p).0, ends(p).1)));
        // Steps only in cities and up; each flight long and high enough, on dry land outside
        // plazas, and clear of the streets crossing it (alleys may open onto it from the side).
        assert!(s.tier >= town::stairs::STAIR_TIER || l.stairs.is_empty(), "settlement {i} ({:?}) has steps", s.tier);
        let ch = if l.stairs.is_empty() { Default::default() } else { channel(l.bbox) };
        for (k, st) in l.stairs.iter().enumerate() {
            let length: f64 = st.pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
            assert!(length >= prm.min_run - 1e-6 && st.rise as f64 >= prm.min_rise - 1e-3, "settlement {i}: a flight at {:?} only {length:.1} ft long, rising {:.1} ft", st.pts[0], st.rise);
            for seg in st.pts.windows(2) {
                let (u, d) = (geom::sub(seg[1], seg[0]), geom::dist(seg[0], seg[1]).max(1e-9));
                let n = (d / 2.5).ceil() as usize;
                for j in 0..=n {
                    let p = geom::lerp(seg[0], seg[1], j as f64 / n as f64);
                    assert!(!in_channel(&ch, p, 0.0), "settlement {i}: steps in the river at {p:?}");
                    assert!(!l.plazas.iter().any(|q| geom::contains(&q.pts, p)), "settlement {i}: steps in a plaza at {p:?}");
                }
                for (pts, class, w) in l.roads.iter().filter(|r| r.1 >= 3 && r.2 != 17.0) {
                    for r in pts.windows(2) {
                        let v = geom::sub(r[1], r[0]);
                        let cos = geom::dot(u, v).abs() / (d * geom::len(v).max(1e-9));
                        if cos > 0.866 {
                            continue;
                        }
                        let near = (0..=n).map(|j| geom::seg_dist(geom::lerp(seg[0], seg[1], j as f64 / n as f64), r[0], r[1])).fold(f64::MAX, f64::min);
                        assert!(near >= 0.5 * w - 0.5, "settlement {i}: flight {} runs {near:.1} ft from a crossing class {class} street ({w} ft wide) at {:?}", st.flight, r[0]);
                    }
                }
            }
            if k == 0 || l.stairs[k - 1].flight != st.flight {
                flights.push((i, st.pts.clone(), st.rise));
            }
            stair_pieces += 1;
        }
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
    // Flights stand on the ground the battlemap draws: dry, and climbing it as steeply as they
    // were judged to (less a little for the sampling).
    assert!(flights.len() >= 3 && stair_pieces >= flights.len(), "too few flights of steps: {} ({stair_pieces} pieces)", flights.len());
    for (i, pts, rise) in &flights {
        let (mut climb, mut length) = (0.0, 0.0);
        let mut last: Option<f64> = None;
        for seg in pts.windows(2) {
            let n = (geom::dist(seg[0], seg[1]) / 5.0).ceil().max(1.0) as usize;
            for j in 0..=n {
                let p = geom::lerp(seg[0], seg[1], j as f64 / n as f64);
                let key = TileKey::surface(g.max_level, (p[0] / size) as u32, (p[1] / size) as u32);
                let (ox, oy) = g.tile_origin_ft(&key);
                let k = padded_index(((p[0] - ox) / s).round() as i64, ((p[1] - oy) / s).round() as i64);
                let tile = ex.terrain(key);
                assert!(ex.t0.sample_water(p[0], p[1]).max(tile.river_water[k]) <= tile.padded[k], "settlement {i}: steps under water at {p:?}");
                let h = tile.padded[k] as f64;
                if let Some(q) = last {
                    climb += (h - q).abs();
                }
                last = Some(h);
            }
            length += geom::dist(seg[0], seg[1]);
        }
        assert!(climb / length >= prm.g_off - 0.03, "settlement {i}: a flight at {:?} climbs only {:.3} on the drawn ground (rise {rise:.1} ft over {length:.0} ft)", pts[0], climb / length);
    }
    // The battlemap draws the highest flight as steps: most of its centreline (2.5 ft in from its
    // ends) in the chunk holding its middle; steps only on dry street squares, off network roads,
    // and under nothing that blocks the way.
    use worldgen::battlemap::{self as bm, SQ, SQUARE_FT, Surface};
    let (i, pts, _) = flights.iter().max_by(|a, b| a.2.total_cmp(&b.2)).unwrap();
    let mid = geom::lerp(pts[0], pts[pts.len() - 1], 0.5);
    let key = TileKey::surface(g.max_level, (mid[0] / size) as u32, (mid[1] / size) as u32);
    let (ox, oy) = g.tile_origin_ft(&key);
    let c = ex.battlemap(key);
    let square = |p: [f64; 2]| {
        let (x, y) = (((p[0] - ox) / SQUARE_FT).floor(), ((p[1] - oy) / SQUARE_FT).floor());
        (x >= 0.0 && y >= 0.0 && x < SQ as f64 && y < SQ as f64).then(|| y as usize * SQ + x as usize)
    };
    let total: f64 = pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
    let mut line: Vec<usize> = Vec::new();
    let mut s0 = 0.0;
    for seg in pts.windows(2) {
        let d = geom::dist(seg[0], seg[1]);
        let n = (d / 1.0).ceil().max(1.0) as usize;
        for j in 0..=n {
            let s = s0 + d * j as f64 / n as f64;
            if s >= 2.5 && s <= total - 2.5 && let Some(k) = square(geom::lerp(seg[0], seg[1], j as f64 / n as f64)) && !line.contains(&k) {
                line.push(k);
            }
        }
        s0 += d;
    }
    let on = line.iter().filter(|&&k| c.surface[k] == Surface::Stairs).count();
    assert!(line.len() >= 2 && on as f64 >= 0.6 * line.len() as f64, "settlement {i}: the flight at {mid:?} is drawn as steps on {on} of its {} squares", line.len());
    // Its axis comes with the chunk (the ground draws the treads square across it).
    let local = [((mid[0] - ox) / SQUARE_FT) as f32, ((mid[1] - oy) / SQUARE_FT) as f32];
    assert!(
        c.structures.iter().any(|s| s.kind == bm::ShapeKind::Stairs && geom::seg_dist([local[0] as f64, local[1] as f64], [s.pts[0][0] as f64, s.pts[0][1] as f64], [s.pts[1][0] as f64, s.pts[1][1] as f64]) <= 0.5 * s.size as f64 + 0.1),
        "the battlemap at {mid:?} has no axis for the flight's treads"
    );
    for k in (0..SQ * SQ).filter(|&k| c.surface[k] == Surface::Stairs) {
        assert!(c.surface[k].is_road() && c.urban[k] == bm::URBAN_STREET && c.building[k] == 0 && c.water_level[k] <= c.height[k], "steps on square {k} of the battlemap at {mid:?}: not a dry street square");
        assert_eq!(c.road_halo[(k / SQ + 1) * (SQ + 2) + k % SQ + 1], 0, "steps on a network road (square {k} of the battlemap at {mid:?})");
    }
    for o in c.objects.iter().filter(|o| c.info(o).blocks_move) {
        let k = (o.y.floor() as usize).min(SQ - 1) * SQ + (o.x.floor() as usize).min(SQ - 1);
        assert!(c.surface[k] != Surface::Stairs, "a {:?} stands on the steps at {mid:?}", o.kind);
    }
    // On ground shaped by hand: a plateau over the flight levels it, and its steps go back to
    // paving; a plateau's sloping edge across a flat street of the largest settlement steps the
    // street where it climbs (never on top), each new step under the axis of its treads, from the
    // bottom up. Nothing else made with them.
    use worldgen::ground;
    let at_square = |c: &bm::Chunk, p: [f64; 2]| c.height[(((p[1] - oy) / SQUARE_FT).floor().clamp(0.0, SQ as f64 - 1.0) as usize) * SQ + ((p[0] - ox) / SQUARE_FT).floor().clamp(0.0, SQ as f64 - 1.0) as usize];
    let b = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let outline = [[b[0] - 10.0, b[1] - 10.0], [b[2] + 10.0, b[1] - 10.0], [b[2] + 10.0, b[3] + 10.0], [b[0] - 10.0, b[3] + 10.0]];
    ex.world.file.edits.plateaus.insert("h:level".into(), ground::plateau_for(&outline, at_square(&c, mid) as f64, 0.0).unwrap());
    let levelled = ex.battlemap(key);
    let left = line.iter().filter(|&&k| levelled.surface[k] == Surface::Stairs).count();
    assert!(left == 0 && line.iter().all(|&k| levelled.surface[k] == Surface::Cobble), "a plateau over the flight at {mid:?} leaves {left} of its {} squares as steps", line.len());
    ex.world.file.edits = Default::default();
    let big = ex.t0.settlements.iter().max_by_key(|s| s.population).unwrap().clone();
    assert!(big.tier >= town::stairs::STAIR_TIER, "the largest settlement is not a city");
    let key = TileKey::surface(g.max_level, (big.x / size) as u32, (big.y / size) as u32);
    let (ox, oy) = g.tile_origin_ft(&key);
    let plain = ex.battlemap(key);
    let streets: Vec<usize> = (0..SQ * SQ)
        .filter(|&k| (16..SQ - 16).contains(&(k % SQ)) && (16..SQ - 16).contains(&(k / SQ)))
        .filter(|&k| plain.surface[k] == Surface::Cobble && plain.urban[k] == bm::URBAN_STREET && plain.road_halo[(k / SQ + 1) * (SQ + 2) + k % SQ + 1] == 0)
        .collect();
    let mut stepped = 0;
    for &k in streets.iter().step_by((streets.len() / 12).max(1)) {
        let p = [ox + ((k % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((k / SQ) as f64 + 0.5) * SQUARE_FT];
        let (half, edge) = (20.0, 30.0);
        let outline = [[-half, -half], [half, -half], [half, half], [-half, half]].map(|d| [p[0] + d[0], p[1] + d[1]]);
        ex.world.file.edits.plateaus.insert("h:ramp".into(), ground::plateau_for(&outline, plain.height[k] as f64 + 6.0, edge).unwrap());
        let c = ex.battlemap(key);
        let new: Vec<usize> = (0..SQ * SQ).filter(|&q| c.surface[q] == Surface::Stairs && plain.surface[q] != Surface::Stairs).collect();
        let axes: Vec<&bm::VectorShape> = c.structures.iter().filter(|s| s.kind == bm::ShapeKind::Stairs).collect();
        assert!(axes.len() <= bm::MAX_STAIR_AXES, "{} treads' axes in the battlemap round {p:?}: the ground draws {}", axes.len(), bm::MAX_STAIR_AXES);
        for &q in &new {
            let centre = [ox + ((q % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
            let d = (0..4).map(|e| geom::seg_dist(centre, outline[e], outline[(e + 1) % 4])).fold(f64::MAX, f64::min);
            assert!(!geom::contains(&outline, centre) && d < edge + SQUARE_FT, "a step at {centre:?} off the plateau's edge round {p:?}");
            assert!(c.urban[q] == bm::URBAN_STREET && c.building[q] == 0 && c.road_halo[(q / SQ + 1) * (SQ + 2) + q % SQ + 1] == 0, "a step at {centre:?} off the street");
            // As the ground's shader finds the axis: within a little of its ends and width.
            let local = [(q % SQ) as f64 + 0.5, (q / SQ) as f64 + 0.5];
            let covered = axes.iter().any(|s| {
                let (a, b) = ([s.pts[0][0] as f64, s.pts[0][1] as f64], [s.pts[1][0] as f64, s.pts[1][1] as f64]);
                let len = geom::dist(a, b).max(1e-4);
                let u = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
                let pa = [local[0] - a[0], local[1] - a[1]];
                let t = geom::dot(pa, u);
                (-t).max(t - len).max((pa[0] * u[1] - pa[1] * u[0]).abs() - 0.5 * s.size as f64) <= 0.6
            });
            assert!(covered, "a step at {centre:?} with no treads' axis");
        }
        if new.len() >= 4 {
            // The new axes run up the slope (drawn ground a fifth and four fifths of the way along).
            let mut climb = 0.0;
            for s in axes.iter().filter(|s| !plain.structures.iter().any(|o| o.kind == s.kind && o.pts == s.pts)) {
                let h = |t: f32| {
                    let v = [s.pts[0][0] + (s.pts[1][0] - s.pts[0][0]) * t, s.pts[0][1] + (s.pts[1][1] - s.pts[0][1]) * t];
                    let q = (v[1].floor().clamp(0.0, SQ as f32 - 1.0) as usize + 1) * (SQ + 2) + v[0].floor().clamp(0.0, SQ as f32 - 1.0) as usize + 1;
                    c.halo_height[q] - c.roof_lift[q]
                };
                assert!(h(0.8) - h(0.2) >= -0.05, "treads' axis {:?} runs down the slope round {p:?}", s.pts);
                climb += h(0.8) - h(0.2);
            }
            assert!(climb > 1.0, "the new treads' axes round {p:?} climb {climb} ft in all");
            stepped = new.len();
            break;
        }
    }
    assert!(stepped >= 4, "no street of the largest settlement steps up a plateau's sloping edge (tried {})", streets.len().min(12));
    ex.world.file.edits = Default::default();
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
            for kind in [worldgen::town::ParkKind::Formal, worldgen::town::ParkKind::Common] {
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

/// Interiors (M5): every enterable building and wall/gate tower in a sample of settlements (the capital, a city,
/// a town, a village, a wizard's tower) opens with at least two levels, stairs on the same
/// squares on every level, one front door on the ground floor, no furniture on a doorway (but a
/// low dais, walked over) or the stairs, and every door and room reachable from the front door across open floor
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
    let (mut checked, mut worst_ms, mut drinking, mut towers, mut cells, mut thrones_looked) = (0, 0.0f64, 0, 0, 0, 0);
    for si in picks {
        let l = town::layout(&world, &t0, si);
        // Buildings, then wall and gate towers.
        // (By id: the lots a palace took in leave theirs unused.)
        let ids: Vec<String> = l
            .buildings
            .iter()
            .filter(|b| b.structure == Structure::Roofed)
            .map(|b| format!("b:{si}:{}", b.id))
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
            // Thrones only in a castle's buildings and a palace.
            if let Some(b) = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok()).and_then(|k| l.building(k)) {
                let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
                let throned = b.keep.is_some() || b.ward == town::catalog::Ward::Castle || key == Some("castle") || key == Some("palace");
                let it = interior::generate_id(&world, &t0, &id).unwrap();
                for lv in &it.levels {
                    assert!(throned || !lv.furniture.iter().any(|f| matches!(f.kind, "throne" | "throne_dais")), "{id} ({}) {}: a throne outside a castle or a palace", it.function, lv.name);
                }
                thrones_looked += 1;
            }
        }
    }
    assert!(thrones_looked > 500, "too few buildings looked at for thrones ({thrones_looked})");
    // Every wizard's tower is round inside: its outer wall drawn round, its rooms round.
    let mut round = 0;
    for p in t0.pois.iter().enumerate().filter(|(_, p)| p.kind == worldgen::t0::settle::PoiKind::Tower).map(|(p, _)| t0.settlements.len() + p) {
        let l = town::layout(&world, &t0, p);
        for b in l.buildings.iter().filter(|b| b.structure == Structure::Roofed && b.func.is_none() && b.residential as usize == town::catalog::WIZARD_TOWER) {
            let it = interior::generate(&world, &t0, p, b.id as usize).unwrap();
            assert!(it.outlines.len() == 1 && it.outlines[0].len() >= 8, "{}: a wizard's tower not drawn round", it.id);
            for lv in &it.levels {
                assert!(lv.rooms.iter().all(|r| r.squares == 0 || r.shape == interior::shape::ROUND), "{} {}: a wizard's tower's room not round", it.id, lv.name);
            }
            round += 1;
        }
    }
    assert!(round >= 3, "too few wizard's towers checked ({round})");
    designed_building(&world, &t0, by_tier(Tier::Town).expect("a town"));
    assert!(checked > 500, "too few interiors ({checked})");
    assert!(drinking > 10, "too few inns and taverns checked ({drinking})");
    assert!(towers > 20, "too few wall towers checked ({towers})");
    assert!(cells > 50, "too few prison cells checked ({cells})");
    // The M5 budget (an interior opens within 300 ms); typical is well under 1 ms.
    assert!(worst_ms < 300.0, "slowest interior took {worst_ms:.1} ms");
}

//// Every city's castle in seeds 1 to 8 holds together (`castle_works`: its keep laid out by zones,
/// about half its halls open two storeys), an inner curtain in about half of them, each at least
/// `GATES_APART` (90 ft) from its outer gate.
#[test]
fn castles_guarantee() {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, geom};
    let (mut castles, mut inner, mut open_halls, mut closed_halls) = (0, 0, 0, 0);
    for seed in 1..=8 {
        let w = World::from_json(&world_json(seed)).unwrap();
        let t = worldgen::t0::T0::generate(&w);
        for si in (0..t.settlements.len()).filter(|&i| t.settlements[i].tier >= Tier::City) {
            let l = town::layout(&w, &t, si);
            if l.castles.is_empty() {
                continue;
            }
            castle_works(&w, &t, &l, si, &mut open_halls, &mut closed_halls);
            castle_ground(&w, &t, &l, si, &l.plazas.iter().find(|p| p.kind == town::PlazaKind::Bailey).expect("a bailey").pts);
            castles += 1;
            if l.castles.len() > 1 {
                inner += 1;
                // The gates: pairs of gate towers 52 ft apart in the castle; the inner one the nearest
                // the inner bailey's middle.
                let g: Vec<[f64; 2]> = l.gate_towers.iter().copied().filter(|p| l.castles.iter().any(|(c, r)| geom_dist(*p, *c) <= r + 1.0)).collect();
                let mut mids = Vec::new();
                for a in 0..g.len() {
                    for b in a + 1..g.len() {
                        if (geom_dist(g[a], g[b]) - 52.0).abs() < 0.5 {
                            mids.push(geom::lerp(g[a], g[b], 0.5));
                        }
                    }
                }
                let ci = l.castles[1].0;
                let (k, m) = mids.iter().enumerate().min_by(|a, b| geom_dist(*a.1, ci).total_cmp(&geom_dist(*b.1, ci))).expect("an inner gate");
                let gap = mids.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, o)| geom_dist(*o, *m)).fold(f64::MAX, f64::min);
                assert!(gap >= 89.0, "#{si} (seed {seed}): the inner gate {gap:.0} ft from the outer one");
                // The inner curtain stands the outer bailey's width (40 ft at least) inside the outer
                // one all along, not only at its corners.
                let bailey = &l.plazas.iter().find(|p| p.kind == town::PlazaKind::Bailey).expect("a bailey").pts;
                let n = bailey.len();
                let depth = |p: [f64; 2]| (0..n).map(|k| geom::seg_dist(p, bailey[k], bailey[(k + 1) % n])).fold(f64::MAX, f64::min);
                for w in l.walls.iter().filter(|w| w.iter().all(|p| geom::contains(bailey, *p) && depth(*p) > 20.0)) {
                    for s in w.windows(2) {
                        let steps = (geom_dist(s[0], s[1]) / 5.0).ceil().max(1.0) as usize;
                        for j in 0..=steps {
                            let p = geom::lerp(s[0], s[1], j as f64 / steps as f64);
                            assert!(depth(p) >= 40.0, "#{si} (seed {seed}): the inner curtain {:.0} ft from the outer at {p:?}", depth(p));
                        }
                    }
                }
            }
        }
    }
    assert!(castles >= 40, "too few castles checked ({castles})");
    // About half the keeps with a floor over the hall have it open two storeys.
    assert!(open_halls * 4 >= open_halls + closed_halls && closed_halls * 4 >= open_halls + closed_halls, "{open_halls} halls open two storeys, {closed_halls} not");
    assert!(inner * 10 >= castles * 3 && inner * 10 <= castles * 7, "{inner} of {castles} castles with an inner curtain");
}

/// A town's castle keeps to its ground: no town gate opens straight into its bailey and no street or
/// road runs through it but the castle's own way to its gate; its bailey is closed all round, by its
/// curtain, the town wall or a tower, but where the water or a river's channel is; and no curtain
/// runs alongside the town wall within 30 ft of it for more than 30 ft (where the town wall is the
/// castle's, it has none of its own). `ring`: its bailey.
fn castle_ground(world: &World, t0: &worldgen::t0::T0, l: &worldgen::town::Layout, si: usize, ring: &[[f64; 2]]) {
    use worldgen::town::geom;
    let n = ring.len();
    let depth = |p: [f64; 2]| {
        let d = (0..n).map(|k| geom::seg_dist(p, ring[k], ring[(k + 1) % n])).fold(f64::MAX, f64::min);
        if geom::contains(ring, p) { d } else { -d }
    };
    // Points every 5 ft along a polyline, and the way it runs there.
    let along = |pts: &[[f64; 2]]| -> Vec<([f64; 2], [f64; 2])> {
        let mut v = Vec::new();
        for s in pts.windows(2) {
            let d = geom_dist(s[0], s[1]);
            if d < 1e-6 {
                continue;
            }
            let u = geom::mul(geom::sub(s[1], s[0]), 1.0 / d);
            let m = (d / 5.0).ceil() as usize;
            v.extend((0..=m).map(|k| (geom::lerp(s[0], s[1], k as f64 / m as f64), u)));
        }
        v
    };
    let crosses = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (o1, o2) = (geom::cross(geom::sub(b, a), geom::sub(c, a)), geom::cross(geom::sub(b, a), geom::sub(d, a)));
        let (o3, o4) = (geom::cross(geom::sub(d, c), geom::sub(a, c)), geom::cross(geom::sub(d, c), geom::sub(b, c)));
        o1 * o2 < 0.0 && o3 * o4 < 0.0
    };
    let towers = worldgen::interior::towers(l);
    let segs: Vec<([f64; 2], [f64; 2])> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    // The castle's gates: pairs of gate towers a passage apart in it (one set in the town by hand has
    // its towers among `extra_towers`); its own way, from its outer gate.
    let g: Vec<[f64; 2]> = l.gate_towers.iter().copied().chain(l.extra_towers.iter().filter(|t| t.1).map(|t| t.0)).filter(|p| l.castles.iter().any(|(c, r)| geom_dist(*p, *c) <= r + 1.0)).collect();
    let mut gates = Vec::new();
    for a in 0..g.len() {
        for b in a + 1..g.len() {
            if (geom_dist(g[a], g[b]) - 52.0).abs() < 0.5 {
                gates.push(geom::lerp(g[a], g[b], 0.5));
            }
        }
    }
    let outer = gates.iter().copied().min_by(|a, b| depth(*a).abs().total_cmp(&depth(*b).abs())).expect("a castle gate");
    let own = |r: &(Vec<[f64; 2]>, u8, f64)| r.1 == 3 && (r.2 - 10.0).abs() < 1e-9 && geom_dist(r.0[0], outer) < 12.0;
    // No town gate opens into the bailey: from its middle no straight way into it crosses no wall or tower.
    for &tg in l.gates.iter().filter(|q| depth(**q) > -60.0) {
        for k in 0..36 {
            let a = k as f64 * std::f64::consts::TAU / 36.0;
            for r in [15.0, 25.0, 35.0, 45.0] {
                let p = [tg[0] + r * a.cos(), tg[1] + r * a.sin()];
                let open = depth(p) > 4.0 && !segs.iter().any(|s| crosses(tg, p, s.0, s.1) || geom::seg_dist(p, s.0, s.1) < 4.5) && !towers.iter().any(|t| geom::seg_dist(t.0, tg, p) < t.1);
                assert!(!open, "#{si}: the town gate at {tg:?} opens into the bailey (to {p:?})");
            }
        }
    }
    // No street or road through the bailey but its own way.
    for st in l.streets.iter().chain(l.roads.iter().filter(|r| !own(r)).map(|r| &r.0)) {
        let inside = along(st).iter().filter(|x| depth(x.0) > 3.0).count() as f64 * 5.0;
        assert!(inside <= 15.0, "#{si}: a street runs {inside:.0} ft inside the bailey");
    }
    // Closed all round, but by the water.
    let by_river = |p: [f64; 2]| {
        let c = worldgen::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 6.0, t0.cell_ft);
        (c.0 - p[0]).abs() + (c.1 - p[1]).abs() > 0.01
    };
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
    let mut closed = ring.to_vec();
    closed.push(ring[0]);
    let mut open = 0.0;
    for (p, _) in along(&closed) {
        let shut = segs.iter().any(|s| geom::seg_dist(p, s.0, s.1) <= 10.0) || towers.iter().any(|t| geom_dist(t.0, p) < t.1 + 6.0) || gates.iter().any(|m| geom_dist(*m, p) < 30.0);
        let wet = t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice) || by_river(p);
        open = if shut || wet { 0.0 } else { open + 5.0 };
        assert!(open <= 15.0, "#{si}: the bailey is open on dry ground at {p:?}");
    }
    // No curtain just inside the town wall: the castle's walls are those on its rings (the outer
    // bailey's, or wholly inside it, an inner curtain's), the town's the rest.
    let on_ring = |p: [f64; 2]| depth(p).abs() < 1.5;
    let (mut ours, mut town) = (Vec::new(), Vec::new());
    for w in &l.walls {
        let pts = along(w);
        let on = pts.iter().filter(|x| on_ring(x.0)).count();
        if w.iter().all(|p| depth(*p) > 20.0) || on * 10 >= pts.len() * 8 {
            ours.push(w);
        } else {
            town.extend(w.windows(2).map(|s| (s[0], s[1])));
        }
    }
    for w in ours {
        let mut run = 0.0;
        for (p, u) in along(w) {
            let beside = town.iter().any(|&(a, b)| {
                let d = geom_dist(a, b);
                d > 1.0 && geom::cross(u, geom::mul(geom::sub(b, a), 1.0 / d)).abs() < 0.26 && (3.0..30.0).contains(&geom::seg_dist(p, a, b))
            });
            run = if beside { run + 5.0 } else { 0.0 };
            assert!(run <= 35.0, "#{si}: a curtain runs within 30 ft of the town wall for {run:.0} ft at {p:?}");
        }
    }
}

/// A castle holds together: its own towers and gatehouses (within its curtains) stand clear of each
/// other, nothing built stands on its walls or towers, and play mode can walk in through its gates
/// to every building in it and all but a few squares of its baileys (`castle_reach`); its keep (the
/// castle) is made of convex pieces whose union is its footprint, stands clear of the curtain, and
/// inside has a room in each of its towers on every floor (round in a round tower) with a spiral
/// stair in its middle, reached through one way in at most (the rest of the tower its wall), keeps
/// every rule interiors keep (`check_interior`), and has its great hall furnished as a whole: the
/// throne dais at its far end, nothing shutting a square of it off.
fn castle_works(world: &World, t0: &worldgen::t0::T0, l: &worldgen::town::Layout, si: usize, open_halls: &mut usize, closed_halls: &mut usize) {
    use worldgen::interior;
    use worldgen::town::{self, PartKind, Structure, geom};
    let towers = interior::towers(l);
    let ours = |p: [f64; 2]| l.castles.iter().any(|(c, r)| geom_dist(p, *c) <= r + 1.0);
    // (A square tower reaches further across its corners.)
    let reach = |t: &([f64; 2], f64)| if l.square_tower(t.0).is_some() { t.1 * std::f64::consts::SQRT_2 } else { t.1 };
    for (a, ta) in towers.iter().enumerate().filter(|(_, t)| ours(t.0)) {
        for tb in towers[a + 1..].iter().filter(|t| ours(t.0)) {
            assert!(geom_dist(ta.0, tb.0) >= reach(&(ta.0, ta.1)) + reach(&(tb.0, tb.1)) - 3.0, "#{si}: towers at {:?} and {:?} stand on each other", ta.0, tb.0);
        }
    }
    let near_wall = |p: &[[f64; 2]]| l.walls.iter().flat_map(|w| w.windows(2)).any(|s| p.iter().any(|q| geom::seg_dist(*q, s[0], s[1]) < 4.0) || geom::contains(p, s[0]));
    let fi = town::catalog::index_of("castle").unwrap() as u16;
    let mut keeps = 0;
    // (A temple's pieces are no keep's: `town::temple`.)
    for b in l.buildings.iter().filter(|b| b.func == Some(fi) || (!b.parts.is_empty() && !town::temple::shaped(b) && town::belfry::of(b).is_none())) {
        let tag = format!("#{si} b:{si}:{}", b.id);
        assert!(!near_wall(&b.poly), "{tag}: the keep stands on a wall");
        assert!(towers.iter().all(|t| !geom::contains(&b.poly, t.0)), "{tag}: a tower stands in the keep");
        if b.parts.is_empty() {
            continue;
        }
        keeps += 1;
        assert!(b.parts[0].kind == PartKind::Body, "{tag}: its first piece is not its body");
        for p in &b.parts {
            let n = p.poly.len();
            let turns: Vec<f64> = (0..n).map(|k| geom::cross(geom::sub(p.poly[(k + 1) % n], p.poly[k]), geom::sub(p.poly[(k + 2) % n], p.poly[(k + 1) % n]))).collect();
            assert!(turns.iter().all(|&t| t >= -1e-6) || turns.iter().all(|&t| t <= 1e-6), "{tag}: a piece is not convex");
            // (Its corners' mean: `centroid` on a small piece far out in world ft is not exact.)
            let c = geom::mul(p.poly.iter().fold([0.0, 0.0], |a, q| geom::add(a, *q)), 1.0 / n as f64);
            assert!(geom::contains(&b.poly, c), "{tag}: a piece lies outside its footprint");
        }
        let whole: f64 = geom::area(&b.poly).abs();
        let biggest = b.parts.iter().map(|p| geom::area(&p.poly).abs()).fold(0.0, f64::max);
        assert!(whole >= biggest - 1.0, "{tag}: its footprint is smaller than its body");
        if b.structure != Structure::Roofed {
            continue;
        }
        let it = interior::generate(world, t0, si, b.id as usize).expect("a keep can be entered");
        // Laid out by zones on every floor, none fallen back on the shared layout.
        let fell = interior::hub_fallbacks(world, t0, si, b.id as usize);
        assert!(fell.is_empty(), "{tag}: floors {fell:?} not laid out by zones");
        let sh = interior::Shell::of(l, b);
        let (nx, ny) = (it.nx, it.ny);
        assert_eq!(sh.towers.len(), b.parts.iter().filter(|p| matches!(p.kind, PartKind::Tower { .. })).count(), "{tag}: a tower has no room");
        // Each tower (`Shell::towers`): a room filling it on the ground floor and every floor up to
        // its top (two at least), round in a round tower, its spiral stair at its middle, a way in.
        for (ti, (sq, round, mid)) in sh.towers.iter().enumerate() {
            let on: Vec<usize> = (0..it.levels.len()).filter(|&li| it.levels[li].z >= 0 && it.levels[li].cells[*mid] >= 0).collect();
            assert!(on.len() >= 2 && on[0] == it.entry_level && on.windows(2).all(|w| w[1] == w[0] + 1), "{tag}: tower {ti} stands on levels {on:?}");
            for &li in &on {
                let lv = &it.levels[li];
                let room = lv.cells[*mid];
                assert!(sq.iter().all(|&k| lv.cells[k] == room), "{tag} {}: tower {ti}'s room doesn't fill it", lv.name);
                assert!((lv.rooms[room as usize].shape == interior::shape::ROUND) == *round, "{tag} {}: tower {ti}'s room is drawn {} round", lv.name, if *round { "not" } else { "" });
                assert!(lv.furniture.iter().any(|f| f.kind == "spiral_stair" && f.y as usize * nx + f.x as usize == *mid), "{tag} {}: tower {ti} without its spiral stair", lv.name);
                assert!(lv.doors.iter().any(|d| d.rooms.contains(&room)), "{tag} {}: tower {ti}'s room has no way in", lv.name);
            }
        }
        // Laid out by zones (`interior::hub`): its great hall on the ground floor (a great tower's
        // on the first, over its stores); a hall, L or T keep entered through its screens passage
        // (or a forebuilding's entrance hall opening onto it); the main stair at the dais end,
        // behind the hall's high end (in the landing off it, past the hall's middle from the
        // screens). A hall open two storeys: its height left open above it (no way into it), a
        // minstrels' gallery over it reached.
        let kind = interior::hub::keep_kind(b).expect("a keep of pieces");
        use interior::hub::Kind;
        let hall_z = if matches!(kind, Kind::Great | Kind::Plain) && b.floors >= 2 { 1 } else { 0 };
        let li = it.levels.iter().position(|lv| lv.z == hall_z).expect("the hall's floor");
        let g = &it.levels[li];
        let hall = g.rooms.iter().position(|r| r.kind == "great hall").unwrap_or_else(|| panic!("{tag} ({}): no great hall on {}", kind.name(), g.name)) as i16;
        if kind != Kind::ShellKeep {
            let at = |r: i16| {
                let sq: Vec<usize> = (0..nx * ny).filter(|&k| g.cells[k] == r).collect();
                [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64]
            };
            let st = it.stairs;
            let landing = g.cells[st[1] * nx + st[0]];
            assert!(g.rooms[landing as usize].kind == "landing" && g.doors.iter().any(|d| d.rooms == [hall.min(landing), hall.max(landing)]), "{tag} ({}): the main stair isn't off the hall's high end", kind.name());
            if let Some(sc) = g.rooms.iter().position(|r| r.kind == "screens passage") {
                let (h, s) = (at(hall), at(sc as i16));
                let p = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
                assert!(geom::dot(geom::sub(p, h), geom::sub(h, s)) > 0.0, "{tag} ({}): the main stair is at the screens end", kind.name());
            }
            if matches!(kind, Kind::Hall | Kind::L | Kind::T) {
                let front = g.doors.iter().find(|d| d.kind == "front").expect("a front door");
                let r = front.rooms[0];
                let screens = |o: i16| g.rooms[o as usize].kind == "screens passage";
                let into = screens(r) || (g.rooms[r as usize].kind == "entrance hall" && g.doors.iter().any(|d| d.rooms.contains(&r) && d.rooms.iter().any(|&o| o >= 0 && screens(o))));
                assert!(into, "{tag} ({}): the front door opens into the {}", kind.name(), g.rooms[r as usize].kind);
            }
        }
        if let Some(up) = it.levels.get(li + 1).filter(|lv| lv.rooms.iter().any(|r| r.kind == interior::OPEN_BELOW && r.squares > 0)) {
            if kind != Kind::ShellKeep {
                let gallery = up.rooms.iter().position(|r| r.kind == "minstrels' gallery").unwrap_or_else(|| panic!("{tag} {}: an open hall without its gallery", up.name));
                assert!(up.doors.iter().any(|d| d.rooms.contains(&(gallery as i16))), "{tag} {}: the minstrels' gallery has no way in", up.name);
                // (Reached from the main stair through passages only, never through a room.)
                let st = it.stairs;
                let mut seen = vec![false; up.rooms.len()];
                let mut stack = vec![up.cells[st[1] * nx + st[0]]];
                while let Some(a) = stack.pop() {
                    if a < 0 || seen[a as usize] || !interior::is_circulation(up.rooms[a as usize].kind) {
                        continue;
                    }
                    seen[a as usize] = true;
                    stack.extend(up.doors.iter().filter(|d| d.rooms.contains(&a)).map(|d| if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] }));
                }
                assert!(seen[gallery], "{tag} {}: the minstrels' gallery is reached only through a room", up.name);
                // (Its open height over the hall below.)
                for k in (0..nx * ny).filter(|&k| up.cells[k] >= 0 && up.rooms[up.cells[k] as usize].kind == interior::OPEN_BELOW) {
                    assert!(g.cells[k] == hall, "{tag} {}: open to below over the {}", up.name, g.rooms[g.cells[k].max(0) as usize].kind);
                }
                *open_halls += 1;
            }
        } else if kind != Kind::ShellKeep && it.levels.iter().any(|lv| lv.z == hall_z + 1 && lv.z < b.floors as i8) {
            *closed_halls += 1;
        }
        assert!(it.outlines.len() >= b.parts.len(), "{tag}: {} outline parts for {} pieces", it.outlines.len(), b.parts.len());
        check_interior(world, t0, l, si, &format!("b:{si}:{}", b.id), &building_buckets(l));
        // Its great hall furnished as a whole (`interior::hall`): its throne dais at the far end of
        // the hall (from the screens to the dais end), and no square of its floor shut off by what
        // stands on it.
        let plans = interior::hall_plans(world, t0, si, b.id as usize);
        let plan = plans.iter().find(|p| it.levels[p.level].rooms[p.room as usize].kind == "great hall").unwrap_or_else(|| panic!("{tag} ({}): its great hall not furnished as one", kind.name()));
        let lv = &it.levels[plan.level];
        let f = plan.frame;
        let dais = lv.furniture.iter().find(|d| d.kind == "throne_dais" && lv.cells[d.y as usize * nx + d.x as usize] == plan.room).unwrap_or_else(|| panic!("{tag} ({}): no throne dais in its hall", kind.name()));
        let far = [(dais.x, dais.y), (dais.x + dais.w - 1, dais.y + dais.h - 1)].iter().map(|&(x, y)| f.local(x as isize, y as isize).0).max().unwrap();
        assert!(far >= f.len as isize - 3, "{tag} ({}): the throne dais {} rows from the far end of its hall", kind.name(), f.len as isize - 1 - far);
        let mut blocked = vec![false; nx * ny];
        for it in lv.furniture.iter().filter(|it| it.blocks_move) {
            for j in it.y as usize..(it.y + it.h) as usize {
                for i in it.x as usize..(it.x + it.w) as usize {
                    blocked[j * nx + i] = true;
                }
            }
        }
        let door_sq = |d: &interior::Door| {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
        };
        let ways: Vec<usize> = lv.doors.iter().flat_map(door_sq).filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] == plan.room).collect();
        let mut seen = vec![false; nx * ny];
        let mut stack = ways[..1.min(ways.len())].to_vec();
        for &k in &stack {
            seen[k] = true;
        }
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (a, c) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < nx && c < ny && !seen[c * nx + a] && lv.cells[c * nx + a] == plan.room && !blocked[c * nx + a] {
                    seen[c * nx + a] = true;
                    stack.push(c * nx + a);
                }
            }
        }
        assert!(ways.iter().all(|&k| seen[k]) && (0..nx * ny).all(|k| lv.cells[k] != plan.room || blocked[k] || seen[k]), "{tag} ({}): its hall's furniture shuts part of it off", kind.name());
    }
    // (A castle laid out by a town or drawn by hand keeps its keep.)
    assert!(keeps >= 1 || !l.buildings.iter().any(|b| b.func == Some(fi)), "#{si}: its keep is not made of pieces");
    for b in l.buildings.iter().filter(|b| b.parts.is_empty() && b.ward == town::catalog::Ward::Castle && geom_dist(geom::centroid(&b.poly), l.castles[0].0) < l.castles[0].1) {
        assert!(!near_wall(&b.poly), "#{si} b:{si}:{}: a building stands on the curtain", b.id);
    }
    castle_reach(l, si);
}

/// Play mode walks into a castle: on its 5-ft squares, blocked as play mode blocks them (a building
/// on a square's middle, a wall within 5.5 ft, a tower within its reach and 1.5 ft), from the
/// streets and the ground round it every building in its bailey has a square beside it reached (its
/// middle within a square and a half of a wall), and
/// all but 3% of its bailey's open squares are (slivers between a range and the curtain may not
/// be).
fn castle_reach(l: &worldgen::town::Layout, si: usize) {
    use worldgen::town::{PlazaKind, geom};
    let Some(bailey) = l.plazas.iter().find(|p| p.kind == PlazaKind::Bailey).map(|p| &p.pts) else { panic!("#{si}: a castle without its bailey") };
    let (cc, cr) = l.castles[0];
    let s = 5.0;
    let (x0, y0) = (((cc[0] - cr - 60.0) / s).floor() * s, ((cc[1] - cr - 60.0) / s).floor() * s);
    let n = ((2.0 * cr + 120.0) / s).ceil() as usize;
    let mid = |k: usize| [x0 + ((k % n) as f64 + 0.5) * s, y0 + ((k / n) as f64 + 0.5) * s];
    let near = |q: &[[f64; 2]], p: [f64; 2], m: f64| q.iter().any(|v| (v[0] - p[0]).abs() < m + 200.0 && (v[1] - p[1]).abs() < m + 200.0);
    let bs: Vec<&worldgen::town::Building> = l.buildings.iter().filter(|b| near(&b.poly, cc, cr)).collect();
    let blocked: Vec<bool> = (0..n * n)
        .map(|k| {
            let p = mid(k);
            bs.iter().any(|b| geom::contains(&b.poly, p))
                || l.walls.iter().any(|w| w.windows(2).any(|q| geom::seg_dist(p, q[0], q[1]) <= 5.5))
                || l.towers.iter().any(|t| match l.square_tower(*t) {
                    Some(u) => {
                        let d = geom::sub(p, *t);
                        geom::dot(d, u).abs() <= 12.5 && geom::cross(u, d).abs() <= 12.5
                    }
                    None => geom_dist(p, *t) <= 12.5,
                })
                || l.gate_towers.iter().any(|t| geom_dist(p, *t) <= 16.5)
                || l.extra_towers.iter().any(|t| geom_dist(p, t.0) <= if t.1 { 16.5 } else { 12.5 })
        })
        .collect();
    let mut seen = vec![false; n * n];
    let mut q = std::collections::VecDeque::new();
    for k in 0..n * n {
        let (i, j) = (k % n, k / n);
        let p = mid(k);
        let street = !geom::contains(bailey, p) && l.roads.iter().any(|(pts, _, w)| pts.windows(2).any(|q| geom::seg_dist(p, q[0], q[1]) <= 0.5 * w));
        if !blocked[k] && (i == 0 || j == 0 || i == n - 1 || j == n - 1 || street) {
            seen[k] = true;
            q.push_back(k);
        }
    }
    while let Some(k) = q.pop_front() {
        let (i, j) = (k % n, k / n);
        for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
            if a < n && b < n && !blocked[b * n + a] && !seen[b * n + a] {
                seen[b * n + a] = true;
                q.push_back(b * n + a);
            }
        }
    }
    let open: Vec<usize> = (0..n * n).filter(|&k| !blocked[k] && geom::contains(bailey, mid(k))).collect();
    let sealed = open.iter().filter(|&&k| !seen[k]).count();
    assert!(sealed * 100 <= 3 * open.len(), "#{si}: {sealed} of the bailey's {} open squares can't be reached", open.len());
    for b in bs.iter().filter(|b| b.ward == worldgen::town::catalog::Ward::Castle && geom::contains(bailey, geom::centroid(&b.poly))) {
        let m = b.poly.len();
        let by = (0..n * n).any(|k| seen[k] && (0..m).any(|e| geom::seg_dist(mid(k), b.poly[e], b.poly[(e + 1) % m]) < 7.5));
        assert!(by, "#{si} b:{si}:{}: a building in the castle can't be reached", b.id);
    }
}

/// A building's interior designed by hand (the designer's changes: a room split in two by a wall,
/// the stairs moved, a back door to the outside): it keeps every rule interiors keep, the world
/// builds it (its hash unchanged; the design travels as an edit op and back); once the footprint
/// changes the design is set aside (the generated interior stands); with another storey it is
/// fitted to the building, its own floors kept; indoor props and an uploaded picture stand on its
/// floor with their rules; a storey added and taken away in the designer comes back as it was;
/// with no cellar or two, the world builds it (the text plan reading back).
fn designed_building(world: &World, t0: &worldgen::t0::T0, si: usize) {
    use worldgen::interior::{self, design as bd};
    use worldgen::town::{self, Structure, geom};
    use worldgen::under::design::{self, DesignRoom};
    let l = town::layout(world, t0, si);
    let buckets = building_buckets(&l);
    let ok = |d: &design::SiteDesign, id: &str| d.problems(world, t0, id).unwrap().1.iter().all(|p| !p.blocking);
    let mut done = None;
    for b in l.buildings.iter().filter(|b| b.structure == Structure::Roofed && b.floors >= 2 && b.poly.len() == 4) {
        let id = format!("b:{si}:{}", b.id);
        let (mut d, aside) = bd::design_of(world, t0, &id, false).unwrap();
        assert!(!aside, "{id}: set aside with no design saved");
        let (nx, ny) = (d.nx as usize, d.ny as usize);
        let g = d.levels.iter().position(|lv| lv.z == 0).unwrap();
        // The ground floor's biggest room, split across its depth: the far part a study.
        let mut cells = design::decode(&d.levels[g].cells, nx * ny).unwrap();
        let big = (0..d.levels[g].rooms.len()).max_by_key(|&r| cells.iter().filter(|&&c| c == r as i16).count()).unwrap() as i16;
        let ys: Vec<usize> = (0..nx * ny).filter(|&k| cells[k] == big).map(|k| k / nx).collect();
        let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        if y1 - y0 < 3 {
            continue;
        }
        let cut = (y0 + y1 + 1) / 2;
        let study = d.levels[g].rooms.len() as i16;
        for k in 0..nx * ny {
            if cells[k] == big && k / nx >= cut {
                cells[k] = study;
            }
        }
        d.levels[g].rooms.push(DesignRoom { kind: "study".into(), ..Default::default() });
        d.levels[g].cells = design::encode(&cells);
        d.add_doors(Some(g));
        if !ok(&d, &id) {
            continue;
        }
        // A back door: an outside wall of the study onto open ground, its doorway clear.
        let mut back = false;
        'door: for k in (0..nx * ny).filter(|&k| cells[k] == study) {
            for side in 0..4u16 {
                let (x, y) = ((k % nx) as u16, (k / nx) as u16);
                let Some((_, other, _, _)) = bd::door_at(x, y, side, nx, ny) else { continue };
                if other.is_some_and(|o| cells[o] >= 0) || d.levels[g].doors.iter().any(|dr| (dr[0], dr[1], dr[2]) == (x, y, side)) {
                    continue;
                }
                let mut t = d.clone();
                t.levels[g].doors.push([x, y, side, bd::BACK]);
                if ok(&t, &id) {
                    d = t;
                    back = true;
                    break 'door;
                }
            }
        }
        // The stairs moved: the same size, elsewhere on the floor of every level they reach.
        let [sx, sy, sw, sh_] = bd::stairs_of(&d);
        let mut moved = false;
        'stairs: for y in 0..=ny - sh_ {
            for x in 0..=nx - sw {
                if (x, y) == (sx, sy) {
                    continue;
                }
                let mut t = d.clone();
                t.stairs = Some([x as u16, y as u16, sw as u16, sh_ as u16]);
                if ok(&t, &id) {
                    d = t;
                    moved = true;
                    break 'stairs;
                }
            }
        }
        // The study drawn round (else octagonal, else chamfered): the squares outside the shape go
        // to the rooms beside it.
        let mut shaped = false;
        for want in [interior::shape::ROUND, interior::shape::OCTAGON, interior::shape::CHAMFERED] {
            let mut t = d.clone();
            if t.set_shape_checked(world, t0, &id, g, study as usize, want).is_err() {
                continue;
            }
            // Taken: drawn so, every rule kept.
            let (it, problems) = t.problems(world, t0, &id).unwrap();
            assert!(problems.iter().all(|p| !p.blocking), "{id}: the study made {want} breaks a rule: {problems:?}");
            assert_eq!(it.levels[g].rooms[study as usize].shape, want, "{id}: the study made {want} is drawn square");
            d = t;
            shaped = true;
            break;
        }
        if back && moved && shaped {
            done = Some((id, d, b.floors));
            break;
        }
    }
    let (id, mut d, floors) = done.expect("a building to design");
    // An uploaded picture's rules (passable, difficult ground, 2 ft, half cover), and grain
    // sacks (in their fourth look) and the picture (its back to the east) put down in the study.
    let mut file = world.file.clone();
    let kettle = worldgen::world::SpriteMeta { name: "Kettle".into(), size: 1.0, cover: 1, blocks_move: false, blocks_sight: false, difficult: true, height_ft: 2.0 };
    file.edits.sprites.insert("k3ttle".into(), kettle);
    let ws = World::new(file.clone()).unwrap();
    let g = d.levels.iter().position(|lv| lv.z == 0).unwrap();
    let cells = design::decode(&d.levels[g].cells, d.nx as usize * d.ny as usize).unwrap();
    let study = d.levels[g].rooms.iter().position(|r| r.kind == "study").unwrap() as i16;
    for kind in ["sacks", "s:k3ttle"] {
        let nx = d.nx as usize;
        let put = (0..cells.len()).filter(|&k| cells[k] == study).find_map(|k| {
            let mut t = d.clone();
            let (back, variant) = if kind == "sacks" { (None, Some(3)) } else { (Some(1), None) };
            t.levels[g].items.push(design::DesignItem { kind: kind.into(), x: (k % nx) as u16, y: (k / nx) as u16, w: 1, h: 1, back, variant });
            t.problems(&ws, t0, &id).unwrap().1.iter().all(|p| !p.blocking).then_some(t)
        });
        d = put.unwrap_or_else(|| panic!("{id}: no room in the study for {kind}"));
    }
    let (built, problems) = d.problems(&ws, t0, &id).unwrap();
    assert!(problems.iter().all(|p| !p.blocking), "{id}: {problems:?}");
    let floor = &built.levels[built.entry_level].furniture;
    let sacks = floor.iter().find(|f| f.kind == "sacks").expect("the sacks");
    assert!((sacks.name, sacks.cover, sacks.blocks_move) == ("grain sacks", 1, true), "{id}: the sacks' rules: {sacks:?}");
    assert!((sacks.back, sacks.variant) == (None, Some(3)), "{id}: the sacks' look: {sacks:?}");
    let pic = floor.iter().find(|f| f.kind == interior::SPRITE).expect("the picture");
    assert!(pic.sprite == 1 && built.sprites == [interior::InteriorSprite { asset: "k3ttle".into(), name: "Kettle".into() }], "{id}: the picture's table: {:?}", built.sprites);
    assert!((pic.cover, pic.blocks_move, pic.height_ft) == (1, false, 2.0), "{id}: the picture's rules: {pic:?}");
    assert!((pic.back, pic.variant) == (Some(1), None), "{id}: the picture's side: {pic:?}");
    // The world builds it, as every interior is built: the vital rules hold.
    let v = serde_json::to_value(&d).unwrap();
    let undo = file.edits.apply(&worldgen::world::EditOp::Set { field: "designs".into(), key: id.clone(), value: v.clone() }).unwrap();
    assert_eq!(serde_json::to_value(&file.edits.designs[&id]).unwrap(), v, "{id}: the design changed on the round trip");
    assert!(matches!(undo, worldgen::world::EditOp::Unset { .. }), "{id}: undoing a new design unsets it");
    let w2 = World::from_json(&serde_json::to_string(&file).unwrap()).unwrap();
    assert!(bd::design_of(&w2, t0, &id, false).unwrap().0 == d, "{id}: the design read back from the world differs");
    assert_eq!(w2.hash, world.hash, "designs are edits: the world hash stays");
    let it = interior::generate_id(&w2, t0, &id).unwrap();
    assert_eq!(serde_json::to_string(&it).unwrap(), serde_json::to_string(&built).unwrap(), "{id}: the world doesn't build the design");
    let g = &it.levels[it.entry_level];
    assert!(g.rooms.iter().any(|r| r.kind == "study" && r.shape != 0) && g.doors.iter().any(|d| d.kind == "back"), "{id}: the study, drawn shaped, and its back door");
    check_interior(&w2, t0, &town::layout(&w2, t0, si), si, &id, &buckets);
    // A new footprint: the design no longer fits, the generated interior stands.
    let b = l.building(id.rsplit(':').next().unwrap().parse().unwrap()).unwrap();
    let c = geom::centroid(&b.poly);
    let smaller: Vec<[f64; 2]> = b.poly.iter().map(|p| [c[0] + (p[0] - c[0]) * 0.8, c[1] + (p[1] - c[1]) * 0.8]).collect();
    let edit = |change: worldgen::agent::BuildingChange| {
        let mut f = file.clone();
        f.edits.buildings.insert(id.clone(), worldgen::agent::building_edit(&w2, t0, &id, &change).unwrap().expect("a change"));
        World::new(f).unwrap()
    };
    let w3 = edit(worldgen::agent::BuildingChange { poly: Some(smaller), ..Default::default() });
    let it3 = interior::generate_id(&w3, t0, &id).unwrap();
    assert!(!it3.levels.iter().any(|lv| lv.doors.iter().any(|d| d.kind == "back")), "{id}: a design built on a footprint it wasn't made for");
    assert!(bd::design_of(&w3, t0, &id, false).unwrap().1, "{id}: the design not set aside");
    // Another storey: the design follows, its ground floor as designed.
    let w4 = edit(worldgen::agent::BuildingChange { floors: Some(floors + 1), ..Default::default() });
    let fit = bd::refit(&w4, t0, &id, &d, None).unwrap().expect("refitted");
    assert!(bd::refit(world, t0, &id, &d, Some(floors + 1)).unwrap().as_ref() == Some(&fit), "{id}: fitted ahead of the change, it differs");
    assert!(bd::problems_as(world, t0, &id, &fit, Some(floors + 1)).unwrap().1.iter().all(|p| !p.blocking), "{id}: fitted ahead of the change, it breaks a rule");
    assert_eq!(fit.levels.len(), d.levels.len() + 1, "{id}: no storey added");
    assert!(fit.levels.iter().find(|lv| lv.z == 0) == d.levels.iter().find(|lv| lv.z == 0), "{id}: the ground floor's design lost");
    let (_, problems) = fit.problems(&w4, t0, &id).unwrap();
    assert!(problems.iter().all(|p| !p.blocking), "{id}: refitted: {problems:?}");
    // The text plan reads back as the design.
    let text = design::to_text(&d, &|_, _| None).unwrap();
    let (back, _) = design::from_text(&text, &bd::design_of(world, t0, &id, false).unwrap().0).unwrap();
    assert!(back == d, "{id}: the text plan doesn't read back:\n{text}");
    // The designer's storeys (its draft through the editor's call): one added, then taken away
    // again, is the design it was; with the building changed to match, the world builds it.
    let draft = |d: &design::SiteDesign, n: u8| -> design::SiteDesign {
        let v: serde_json::Value = serde_json::from_str(&design::design_json(&w2, t0, &id, Some(&serde_json::to_string(d).unwrap()), Some(&format!(r#"{{"refit":{n}}}"#)))).unwrap();
        assert!(v["problems"].as_array().unwrap().iter().all(|p| p["blocking"] == false), "{id}: {n} storeys in the designer: {}", v["problems"]);
        serde_json::from_value(v["design"].clone()).unwrap()
    };
    let up = draft(&d, floors + 1);
    assert_eq!(up.levels.iter().filter(|lv| lv.z >= 0 && !lv.roof).count(), floors as usize + 1, "{id}: no storey added in the designer");
    assert!(draft(&up, floors) == d, "{id}: a storey added and taken away: not the design it was");
    let mut f5 = file.clone();
    f5.edits.buildings.insert(id.clone(), worldgen::agent::building_edit(&w2, t0, &id, &worldgen::agent::BuildingChange { floors: Some(floors + 1), ..Default::default() }).unwrap().expect("a change"));
    f5.edits.designs.insert(id.clone(), up.clone());
    let w5 = World::new(f5).unwrap();
    let it5 = interior::generate_id(&w5, t0, &id).unwrap();
    assert!(it5.sprites.len() == 1 && it5.levels.iter().filter(|lv| lv.z >= 0 && !lv.roof).count() == floors as usize + 1, "{id}: the taller building's design not built");
    check_interior(&w5, t0, &town::layout(&w5, t0, si), si, &id, &buckets);
    // Its cellars designed: none, or two (the stairs going on down to a storeroom); through the
    // editor's call, as the world builds it, as a text plan.
    for n in [0, 2] {
        let v: serde_json::Value = serde_json::from_str(&design::design_json(&w2, t0, &id, Some(&serde_json::to_string(&d).unwrap()), Some(&format!(r#"{{"cellars":{n}}}"#)))).unwrap();
        let c: design::SiteDesign = serde_json::from_value(v["design"].clone()).unwrap();
        assert_eq!(c.levels.iter().filter(|lv| lv.z < 0).count(), n, "{id}: {n} cellars asked");
        let needs = interior::has_deep_dungeon(b);
        let refused = v["problems"].as_array().unwrap().iter().any(|p| p["blocking"] == true);
        assert_eq!(refused, n == 0 && needs, "{id}: {n} cellars: {}", v["problems"]);
        if refused {
            continue;
        }
        let mut f = file.clone();
        f.edits.designs.insert(id.clone(), c.clone());
        let w = World::new(f).unwrap();
        let it = interior::generate_id(&w, t0, &id).unwrap();
        assert!(it.levels.iter().filter(|lv| lv.z < 0).count() == n && it.levels[it.entry_level].z == 0, "{id}: {n} cellars not built");
        check_interior(&w, t0, &town::layout(&w, t0, si), si, &id, &buckets);
        let text = design::to_text(&c, &|_, _| None).unwrap();
        let (back, _) = design::from_text(&text, &bd::design_of(world, t0, &id, false).unwrap().0).unwrap();
        assert!(back == c, "{id}: {n} cellars: the text plan doesn't read back:
{text}");
    }
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
    // Shapes are only drawn: a room keeps one while its squares are the shape's. The outer wall
    // drawn round a footprint off the grid (`outlines`: on the footprint, pushed out where squares
    // reach past it) leaves every square's middle a tenth of a square inside its inner face, keeps
    // within a square of them, and never cuts a generated stair block.
    for lv in &it.levels {
        for (ri, r) in lv.rooms.iter().enumerate() {
            assert!(r.shape == 0 || interior::shape::fits(&lv.cells, nx, ri as i16, r.shape), "{tag} {}: the {} is drawn shaped ({}) off its squares", lv.name, r.kind, r.shape);
        }
    }
    if !it.outlines.is_empty() {
        // How far a point lies inside the outline (its parts' best), negative outside.
        let inset = |x: f32, y: f32| {
            it.outlines
                .iter()
                .map(|o| {
                    (0..o.len())
                        .map(|k| {
                            let (a, b) = (o[k], o[(k + 1) % o.len()]);
                            ((b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])) / (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9)
                        })
                        .fold(f32::MAX, f32::min)
                })
                .fold(f32::MIN, f32::max)
        };
        for lv in &it.levels {
            for k in (0..nx * ny).filter(|&k| lv.cells[k] >= 0) {
                let d = inset((k % nx) as f32 + 0.5, (k / nx) as f32 + 0.5);
                assert!(d >= interior::WALL_HALF as f32 + 0.1 - 1e-3, "{tag} {}: square {},{} only {d:.2} inside its outer wall", lv.name, k % nx, k / nx);
            }
            // (Unless no square of the building is clear of the wall.)
            let st = it.stairs;
            let room = (0..nx * ny).any(|k| {
                let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                lv.cells[k] >= 0 && [(i, j), (i + 1.0, j), (i, j + 1.0), (i + 1.0, j + 1.0)].iter().all(|c| inset(c.0, c.1) >= interior::WALL_HALF as f32 - 1e-3)
            });
            if lv.has_stairs && room && !world.file.edits.designs.contains_key(id) {
                for (x, y) in [(st[0], st[1]), (st[0] + st[2], st[1]), (st[0], st[1] + st[3]), (st[0] + st[2], st[1] + st[3])] {
                    let d = inset(x as f32, y as f32);
                    assert!(d >= interior::WALL_HALF as f32 - 1e-3, "{tag} {}: the outer wall cuts the stairs {st:?} ({d:.2} in)", lv.name);
                }
            }
        }
        let g = &it.levels[it.entry_level];
        // (A keep's towers in grid units: a square of wall round each one's room.)
        let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok()).and_then(|k| l.building(k));
        let towers: Vec<Vec<[f64; 2]>> = own
            .map(|b| {
                b.parts
                    .iter()
                    .filter(|p| matches!(p.kind, worldgen::town::PartKind::Tower { .. }))
                    .map(|p| p.poly.iter().map(|q| {
                        let d = geom::sub(*q, it.origin);
                        [geom::dot(d, it.axis) / 5.0, geom::dot(d, it.across) / 5.0]
                    }).collect())
                    .collect()
            })
            .unwrap_or_default();
        let by_tower = |v: &[f32; 2]| {
            let p = [v[0] as f64, v[1] as f64];
            towers.iter().any(|t| geom::contains(t, p) || (0..t.len()).any(|k| geom::seg_dist(p, t[k], t[(k + 1) % t.len()]) <= 1.0))
        };
        for v in it.outlines.iter().flatten() {
            let near = (0..nx * ny).filter(|&k| g.cells[k] >= 0).map(|k| {
                let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                let (dx, dy) = ((i - v[0]).max(v[0] - i - 1.0).max(0.0), (j - v[1]).max(v[1] - j - 1.0).max(0.0));
                (dx * dx + dy * dy).sqrt()
            });
            let d = near.fold(f32::MAX, f32::min);
            // (A keep's tower: a square of wall round its room.)
            let most = if by_tower(v) { 1.5 } else { 1.0 };
            assert!(d <= most, "{tag}: its outer wall stands {d:.2} squares off its floor at {v:?}");
        }
    }
    // (Generated ones; a design may have no cellar.)
    assert!(it.levels.len() >= 2 || world.file.edits.designs.contains_key(id), "{tag}: only {} level", it.levels.len());
    // A building's interior as a design builds it again exactly, and keeps the designer's rules.
    if id.starts_with("b:") {
        let (d, _) = interior::design::design_of(world, t0, id, false).unwrap();
        let (built, problems) = interior::design::problems(world, t0, id, &d).unwrap();
        assert!(problems.is_empty(), "{tag}: as a design: {problems:?}");
        assert_eq!(serde_json::to_string(&built).unwrap(), serde_json::to_string(&it).unwrap(), "{tag}: built again from its design, it differs");
    }
    // One chapel per building; each spiral stair on the same square on a run of two levels or more
    // (a keep's tower rises as high as the piece it stands on, and a storey more).
    let chapels = it.levels.iter().flat_map(|lv| lv.rooms.iter()).filter(|r| r.kind == "chapel").count();
    assert!(chapels <= 1, "{tag}: {chapels} chapels");
    let spirals: Vec<Vec<(u16, u16)>> = it.levels.iter().map(|lv| lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| (f.x, f.y)).collect()).collect();
    let mut at: Vec<(u16, u16)> = spirals.iter().flatten().copied().collect();
    at.sort();
    at.dedup();
    for s in at {
        let on: Vec<usize> = (0..spirals.len()).filter(|&li| spirals[li].contains(&s)).collect();
        assert!(on.len() >= 2, "{tag}: a spiral stair at {s:?} on one level only");
        assert!(on.windows(2).all(|w| w[1] == w[0] + 1), "{tag}: the spiral stair at {s:?} skips a level");
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
            let hit = buckets.get(&((p[0] / 100.0).floor() as i64, (p[1] / 100.0).floor() as i64)).into_iter().flatten().find(|&&k| Some(l.buildings[k].id as usize) != own && geom::contains(&l.buildings[k].poly, p));
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
                    // (A platform, walked over, may lie in a doorway.)
                    assert!(interior::hall::is_platform(f.kind) || !lv.doors.iter().any(|d| door_sq(d).contains(&(i as isize, j as isize))), "{tag} {}: {} in a doorway", lv.name, f.name);
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
        // From the stairs (or, above the main stairs, every spiral stair; a ground floor with
        // neither, a designed one-storey building without a cellar, from its doors).
        let mut starts: Vec<usize> = if lv.has_stairs {
            vec![st[1] * nx + st[0]]
        } else {
            lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize).collect()
        };
        if starts.is_empty() && lv.z == 0 {
            starts = lv.doors.iter().filter(|d| d.rooms[1] < 0).flat_map(door_sq).filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] >= 0).collect();
        }
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
        // Every spiral stair reached from the stairs, and from its room's doors with nothing but a
        // rug in the way (not a bed one could step over): the way on up to the rooms above it.
        let mut occupied = vec![false; nx * ny];
        for f in lv.furniture.iter().filter(|f| f.kind != "rug" && f.kind != "spiral_stair") {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    occupied[j * nx + i] = true;
                }
            }
        }
        for f in lv.furniture.iter().filter(|f| f.kind == "spiral_stair") {
            let k0 = f.y as usize * nx + f.x as usize;
            assert!(seen[k0], "{tag} {}: the spiral stair at ({}, {}) is cut off", lv.name, f.x, f.y);
            let r = lv.cells[k0];
            let mut near: Vec<usize> = lv.doors.iter().flat_map(door_sq).filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] == r && seen[k]).collect();
            if near.is_empty() {
                continue;
            }
            let mut got = vec![false; nx * ny];
            for &k in &near {
                got[k] = true;
            }
            while let Some(k) = near.pop() {
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    let q = b.wrapping_mul(nx).wrapping_add(a);
                    if a < nx && b < ny && !got[q] && lv.cells[q] == r && !occupied[q] {
                        got[q] = true;
                        near.push(q);
                    }
                }
            }
            assert!(got[k0], "{tag} {}: furniture shuts the spiral stair at ({}, {}) off from its room's doors", lv.name, f.x, f.y);
        }
        for (ri, room) in lv.rooms.iter().enumerate() {
            let open = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && !blocked[k]);
            let reached = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && seen[k]);
            if !interior::walkable(room.kind) {
                // A hall's height left open, a roof's slopes: no door, no floor, nothing on them.
                assert!(!reached && !lv.doors.iter().any(|d| d.rooms.contains(&(ri as i16))), "{tag} {}: the {} has a way in", lv.name, room.kind);
                assert!(!lv.furniture.iter().any(|f| (f.y..f.y + f.h).any(|j| (f.x..f.x + f.w).any(|i| lv.cells[j as usize * nx + i as usize] == ri as i16))), "{tag} {}: something stands on the {}", lv.name, room.kind);
                continue;
            }
            assert!(!open || reached, "{tag} {}: the {} is unreachable", lv.name, room.kind);
        }
        // Floors with corridors: no room is reached through more than one other room (a
        // guardroom is a way through, as for cells; so is the public room the front door opens
        // into, as `connect` lays it out: a taproom, a reading room, a great hall, counted from
        // as well on a floor with a corridor).
        let entry = lv.doors.iter().find(|d| d.kind == "front").and_then(|d| d.rooms.iter().copied().find(|&r| r >= 0));
        let circ = |r: usize| interior::is_way_through(lv.rooms[r].kind);
        if lv.rooms.len() >= 8 && (0..lv.rooms.len()).any(circ) {
            let mut hops = vec![usize::MAX; lv.rooms.len()];
            let mut q = std::collections::VecDeque::new();
            for r in (0..lv.rooms.len()).filter(|&r| circ(r) || entry == Some(r as i16)) {
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
                if room.squares > 0 && hops[ri] != usize::MAX && interior::walkable(room.kind) {
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
    let (mut designed, mut shaped_rooms) = (0, 0);
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
        // Locked doors open in order: each key reached without its lock, every square in the end.
        let order = worldgen::under::design::key_order(it);
        assert!(order.is_empty(), "{tag}: keys out of order: {order:?}");
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
            // (One in fifty, or one in a section with fewer: a corner square cut off by water.)
            assert!(missed * 50 <= points.max(50), "{tag}: {missed} of {points} street points have no sewer beneath");
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
                let again = d.build(&it.id, it.settlement, it.building, &Default::default());
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
                            *room = design::DesignRoom { kind: design::CHAMBER.into(), ..Default::default() };
                        }
                    }
                }
                assert!(names.is_empty() && back == same, "{tag}: its text reads back as something else");
                designed += 1;
                // Shaped rooms come back as well: every third room given a shape in turn (its squares
                // fitted to it; one whose squares can't be is built square), as built and as text.
                use worldgen::interior::shape;
                let mut shaped = d.clone();
                let mut turn = 0;
                for li2 in 0..shaped.levels.len() {
                    for ri in (0..shaped.levels[li2].rooms.len()).step_by(3) {
                        let code = [shape::ROUND, shape::OCTAGON, shape::APSE_N, shape::APSE_E, shape::APSE_S, shape::APSE_W, shape::CHAMFERED, shape::CAVERN][turn % 8];
                        turn += 1;
                        // (Refused where another room or a rule is in the way: nothing changes; taken,
                        // the site keeps every rule.)
                        // (Checked the cheap way round: the change alone, then held to the rules.)
                        let mut next = shaped.clone();
                        if next.set_shape(li2, ri, code).is_err() {
                            assert!(next == shaped, "{tag}: a shape refused, yet the design changed");
                            continue;
                        }
                        if next.problems(&world, &t0, &it.id).unwrap().1.iter().all(|p| !p.blocking) {
                            shaped = next;
                        } else {
                            let before = shaped.clone();
                            assert!(shaped.set_shape_checked(&world, &t0, &it.id, li2, ri, code).is_err() && shaped == before, "{tag}: a shape that breaks a rule is taken");
                        }
                    }
                }
                let built = shaped.build(&it.id, it.settlement, it.building, &Default::default());
                let mut kept = shaped.clone();
                for (lv, b) in kept.levels.iter_mut().zip(&built.levels) {
                    for (ri, (r, br)) in lv.rooms.iter_mut().zip(&b.rooms).enumerate() {
                        if br.shape != r.shape {
                            assert!(br.squares == 0 || !shape::fits(&b.cells, it.nx, ri as i16, r.shape), "{tag}: a room fitting its shape built square");
                            r.shape = 0;
                        } else if r.shape != 0 {
                            shaped_rooms += 1;
                        }
                    }
                }
                // (Doors in any order: a doorway moved round a shape's corner comes last in the design.)
                let doors_sorted = |mut d: design::SiteDesign| {
                    d.levels.iter_mut().for_each(|lv| lv.doors.sort());
                    d
                };
                let (fa, fb) = (doors_sorted(design::SiteDesign::from_interior(&built)), doors_sorted(kept.clone()));
                assert!(fa == fb, "{tag}: its shaped rooms don't come back from what it builds");
                let text = design::to_text(&kept, &|_, _| None).unwrap_or_else(|e| panic!("{tag}: {e}"));
                assert!(design::from_text(&text, &kept).unwrap_or_else(|e| panic!("{tag}: {e}\n{text}")).0 == kept, "{tag}: its shaped rooms' text reads back as something else");
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
        for (k, b) in l.buildings.iter().enumerate() {
            let bi = b.id;
            if interior::has_deep_dungeon(&l.buildings[k]) {
                let id = format!("k:{li}:{bi}");
                let t = std::time::Instant::now();
                let it = interior::generate_id(&world, &t0, &id).unwrap_or_else(|| panic!("{id}: no deep dungeons"));
                worst_ms = worst_ms.max(t.elapsed().as_secs_f64() * 1e3);
                *kinds.entry("deep dungeons").or_default() += 1;
                check(&format!("{id} (deep dungeons)"), &it, &l, None);
            }
            if worldgen::under::sewer_link_of(&t0, &l, &l.buildings[k]).is_some() {
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
    assert!(shaped_rooms > 100, "too few shaped rooms built ({shaped_rooms})");
    designed_site(&world, &t0);
    designed_cellars(&world, &t0);
}

/// Cellars designed where they lead on: a large keep's and a cellar on the sewers dug two deep
/// keep their ways on (the stairs down to the deep dungeons, the trapdoor to the sewers) on the
/// deepest, each with its way back, the deep dungeons below it; without a cellar, refused.
fn designed_cellars(world: &World, t0: &worldgen::t0::T0) {
    use worldgen::interior::{self, design as bd};
    use worldgen::town;
    let (li, keep, sewer) = (0..t0.settlements.len())
        .find_map(|li| {
            let l = town::layout(world, t0, li);
            let keep = l.buildings.iter().find(|b| interior::has_deep_dungeon(b))?.id;
            let sewer = l.buildings.iter().find(|b| worldgen::under::sewer_link_of(t0, &l, b).is_some() && !interior::has_deep_dungeon(b))?.id;
            Some((li, keep, sewer))
        })
        .expect("a city with a large keep and a cellar on the sewers");
    let mut file = world.file.clone();
    for bi in [keep, sewer] {
        let id = format!("b:{li}:{bi}");
        let (d, _) = bd::design_of(world, t0, &id, false).unwrap();
        let mut none = d.clone();
        assert!(bd::set_cellars(&mut none, 0));
        let (_, problems) = bd::problems(world, t0, &id, &none).unwrap();
        assert!(problems.iter().any(|p| p.blocking && p.text.contains("needs a cellar")), "{id}: no cellar, its way on lost: {problems:?}");
        let mut two = d.clone();
        assert!(bd::set_cellars(&mut two, 2));
        let (_, problems) = bd::problems(world, t0, &id, &two).unwrap();
        assert!(problems.iter().all(|p| !p.blocking), "{id}: two cellars: {problems:?}");
        file.edits.designs.insert(id, two);
    }
    let w = World::new(file).unwrap();
    let l = town::layout(&w, t0, li);
    for bi in [keep, sewer] {
        let id = format!("b:{li}:{bi}");
        let it = interior::generate_id(&w, t0, &id).unwrap();
        assert!(it.levels[0].z == -2 && !it.levels[0].links.is_empty() && it.levels[1].links.is_empty(), "{id}: its ways on not on the deepest cellar");
        for k in &it.levels[0].links {
            let there = interior::generate_id(&w, t0, &k.to).unwrap_or_else(|| panic!("{id}: a way to {} that doesn't exist", k.to));
            assert!(there.levels.iter().any(|lv| lv.links.iter().any(|b| b.to == id)), "{id}: {} has no way back", k.to);
            if k.to.starts_with("k:") {
                assert!(there.levels.iter().all(|lv| lv.elevation_ft < it.levels[0].elevation_ft), "{id}: the deep dungeons are not below its deepest cellar");
            }
        }
        check_interior(&w, t0, &l, li, &id, &building_buckets(&l));
    }
}

/// A site designed by hand (as an agent writes one, in text): a level dug below the deepest,
/// joined by a way down, doors added where needed, a chest turned and in another look (read back
/// from the text plan), a room furnished; the world then builds the
/// design instead of the generated site. Breaking a rule (a room shut off, the way in moved, a
/// way up not under the way down) is caught.
/// A JSON value with every number that isn't a whole one rounded to 9 significant digits.
fn rounded(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Number(n) if !(n.is_i64() || n.is_u64()) => {
            let x = n.as_f64().unwrap();
            Value::String(format!("{x:.8e}"))
        }
        Value::Array(a) => Value::Array(a.into_iter().map(rounded).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, rounded(v))).collect()),
        v => v,
    }
}

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
            "site dungeon · {} levels\nlevel {n}\nitems: {}; down {x},{y}\nlevel {}: Level {} · the vault\nrooms: a=landing (cavern); b=secret vault (round) \"The Hoard\"\ngrid:\n{grid}doors: {doors}\nitems: chest {},{} variant 2 back e; chasm {},{} 1x5; rope_bridge {},{}; water_shallow {},{}; water_deep {},{}\n",
            n + 1,
            items.join("; "),
            n + 1,
            n + 1,
            x + 5,
            y,
            x + 4,
            y - 2,
            x + 4,
            y + 1,
            x + 3,
            y + 2,
            x + 3,
            y - 2
        )
    };
    let (d, names) = design::from_text(&level("auto"), &base).unwrap_or_else(|e| panic!("{id}: {e}"));
    assert_eq!(d.levels.len(), n + 1, "{id}: a level added");
    assert_eq!(names, vec![(0, 1, "The Hoard".to_string())], "{id}: an unknown kind of room is a chamber by that name");
    assert_eq!(d.levels[0].items.iter().filter(|f| f.kind == "up").map(|f| (f.x as usize, f.y as usize)).collect::<Vec<_>>(), vec![(x, y)], "{id}: the way up is put under the way down");
    let (it, problems) = d.problems(world, t0, &id).unwrap();
    assert!(problems.iter().all(|p| !p.blocking), "{id}: {problems:?}");
    assert_eq!(it.levels[0].doors.len(), 1, "{id}: one door from the landing into the vault");
    let chest = it.levels[0].furniture.iter().find(|f| f.kind == "chest").expect("the chest");
    assert!((chest.back, chest.variant) == (Some(1), Some(2)), "{id}: the chest's side and look: {chest:?}");
    let text = design::to_text(&d, &|_, _| None).unwrap();
    assert!(text.contains(&format!("chest {},{} back e variant 2", x + 5, y)), "{id}: the chest in the text plan:\n{text}");
    assert!(design::from_text(&text, &base).unwrap().0 == d, "{id}: the text plan doesn't read back:\n{text}");
    // Terrain: a chasm across the vault (the chest beyond it) crossed by a rope bridge; without the bridge, cut off.
    assert!(text.contains(&format!("chasm {},{} 1x5; rope_bridge {},{}", x + 4, y - 2, x + 4, y + 1)), "{id}: the chasm and bridge in the text plan:\n{text}");
    let mut fallen = d.clone();
    fallen.levels[0].items.retain(|f| f.kind != "rope_bridge");
    assert!(fallen.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.text.contains("can't be reached")), "{id}: the chasm cut nothing off");
    // Shapes: the landing a cavern breaking in; the vault round in the plan but drawn square (its
    // squares fill their box: noted), until the designer fits it to an octagon (its corners rock).
    use worldgen::interior::shape;
    assert_eq!((it.levels[0].rooms[0].shape, it.levels[0].rooms[1].shape), (shape::CAVERN, 0), "{id}: the landing's and vault's shapes as built");
    assert!(problems.iter().any(|p| !p.blocking && p.text.contains("drawn square")), "{id}: the vault drawn square, not noted: {problems:?}");
    assert!(text.contains("a=landing (cavern); b=secret vault (round)"), "{id}: the shapes in the text plan:\n{text}");
    let v: serde_json::Value = serde_json::from_str(&design::design_json(world, t0, &id, Some(&serde_json::to_string(&d).unwrap()), Some(r#"{"shape":{"level":0,"room":1,"shape":"octagon"}}"#))).unwrap();
    let d: design::SiteDesign = serde_json::from_value(v["design"].clone()).unwrap();
    assert!(v["problems"].as_array().unwrap().iter().all(|p| p["blocking"] == false), "{id}: the octagonal vault: {}", v["problems"]);
    assert_eq!(v["interior"]["levels"][0]["rooms"][1]["shape"], shape::OCTAGON, "{id}: the vault not drawn octagonal");
    let vault = design::decode(&d.levels[0].cells, nx * ny).unwrap().iter().filter(|&&c| c == 1).count();
    assert_eq!(vault, 21, "{id}: the octagonal vault's squares");
    let text = design::to_text(&d, &|_, _| None).unwrap();
    assert!(text.contains("b=secret vault (octagon)") && design::from_text(&text, &base).unwrap().0 == d, "{id}: the octagonal vault's text plan doesn't read back:\n{text}");
    let built = d.problems(world, t0, &id).unwrap().0;
    assert!(design::SiteDesign::from_interior(&built).levels[0].rooms == d.levels[0].rooms, "{id}: the shapes don't come back from what the design builds");
    // Broken: the vault shut off; the way in moved; the way up off the way down.
    let (shut, _) = design::from_text(&level(""), &base).unwrap();
    assert!(shut.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.level == 0 && p.text.contains("can't be reached")), "{id}: a room shut off");
    let mut moved = d.clone();
    let top = moved.levels.last_mut().unwrap();
    let e = top.items.iter_mut().find(|f| f.kind == "exit").unwrap();
    e.x += 1;
    assert!(moved.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.text.contains("under the entrance")), "{id}: the way in moved");
    let mut off = d.clone();
    off.levels[0].items.iter_mut().find(|f| f.kind == "up").unwrap().y += 1;
    assert!(off.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.level == 0), "{id}: the way up off the way down");
    // Furnished: props go where they leave everything reachable.
    let mut f = d.clone();
    let had = f.levels[0].items.len();
    f.furnish(world, t0, &id, 0, 0, 7);
    f.levels[0].rooms[0].kind = "armory".into();
    f.furnish(world, t0, &id, 0, 0, 7);
    assert!(f.levels[0].items.len() > had, "{id}: the armory got nothing");
    assert!(f.problems(world, t0, &id).unwrap().1.iter().all(|p| !p.blocking), "{id}: furnishing broke it");
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
    assert!(by_field.set_fields(&fields.to_string()).unwrap().is_none(), "{id}: created sites untouched");
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
    assert_eq!(it2.levels[0].rooms[1].shape, shape::OCTAGON, "{id}: the world doesn't build the vault octagonal");
    let w3 = World::from_json(&serde_json::to_string(&w2.file).unwrap()).unwrap();
    // (serde_json reads a long float back to within a unit in its last place, the design's origin
    // among them: the same but for that.)
    let it3 = serde_json::to_value(worldgen::interior::generate_id(&w3, t0, &id).unwrap()).unwrap();
    assert_eq!(rounded(it3), rounded(serde_json::to_value(&it2).unwrap()), "{id}: read back");
    assert!(design::design_of(&w2, t0, &id, true).unwrap() == base, "{id}: the generated site is still there underneath");
    // The design under another site's key (edits kept for another sketch or generator): set
    // aside, that site built as generated; its own site keeps it.
    let other = (0..worldgen::town::layout_count(t0)).filter(|&l| l != li).map(|l| format!("u:{l}:0")).find(|o| design::editable(world, t0, o).is_ok()).expect("another site");
    let mut f4 = w2.file.clone();
    let d = f4.edits.designs.remove(&id).unwrap();
    f4.edits.designs.insert(other.clone(), d);
    let w4 = World::new(f4).unwrap();
    assert!(!design::misplaced(&w2, t0, &id), "{id}: its own design is built");
    assert!(design::misplaced(&w4, t0, &other), "{other}: a design made for {id} is not set aside");
    let (ol, _) = design::editable(world, t0, &other).unwrap();
    let generated = serde_json::to_value(worldgen::under::generate(world, t0, ol, 0).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(worldgen::interior::generate_id(&w4, t0, &other).unwrap()).unwrap(), generated, "{other}: not built as generated");
    assert!(design::design_of(&w4, t0, &other, false).unwrap() == design::design_of(&w4, t0, &other, true).unwrap(), "{other}: the misplaced design is read back");
    assert!(worldgen::interior::design::set_aside(&w4, t0).iter().any(|(i, _)| *i == other), "{other}: not listed as set aside");
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
    ward_edits(&base, &t0);
    ward_castles_by_the_wall(&base, &t0);
    ground_edits(&base);
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
    // Out of town, a site is in no settlement; a removed one is nowhere.
    assert!(town::parent_of(t0, li).is_none() && agent::get(world, t0, "c:0").unwrap().get("settlement") == Some(&serde_json::Value::Null), "a ruin two miles out stands in a town");
    assert!(town::parents(t0).iter().all(|&(s, _)| s != li + 1), "a removed site stands in a town");
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
    // uploaded sprite's, turned by hand), the payload names the sprite, and the app's entries
    // survive mapd.
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
        ("objects", "o:idol", serde_json::json!({ "kind": format!("s:{asset}"), "x": free[0] + 20.0, "y": free[1], "rot": 1.5, "scale": 1.0, "variant": 0, "turned": true })),
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
    assert!(rock.kind as u16 == 12 && rock.variant == 2 && !rock.turned && edited.info(rock).blocks_move, "the placed boulder is wrong");
    let idol = edited.objects.iter().find(|o| o.sprite != 0).expect("the placed sprite");
    let ii = edited.info(idol);
    assert!(edited.name(idol) == "Idol" && ii.cover == 3 && ii.blocks_sight && (ii.radius - 1.0).abs() < 1e-6 && (idol.rot - 1.5).abs() < 1e-6 && idol.turned, "the sprite's rules are wrong");
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
    let l = town::layout(world, t0, ci);
    let plaza = &l.plazas.iter().find(|p| p.kind.is_market() || p.kind == town::PlazaKind::Civic).expect("a city with a square").pts;
    let on_plaza = geom::circle(geom::centroid(plaza), 8.0, 12);
    assert!(agent::building_spot(world, t0, &on_plaza, None, "c:99", None).is_err_and(|e| e.contains("square")), "a building on a market square is allowed");

    // The world's own buildings edited: one taken away (gone from the battlemap's buildings and
    // squares and from the gazetteer, its ground free to build on), one made a smithy of three
    // storeys (its interior is one); every other id and footprint stays, the hash too, the
    // chunk is repeatable; an edit whose building no longer stands where it was is set aside.
    use worldgen::world::BuildingEdit;
    let base_l = town::layout(world, t0, ci);
    let homes: Vec<&town::Building> = base_l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.func.is_none() && geom::area(&b.poly).abs() > 400.0).collect();
    // (The one taken away stands clear of the streets, so its ground is free once it is gone.)
    let k = homes.iter().position(|b| agent::building_spot(world, t0, &b.poly, None, "c:99", Some((ci, Some(b.id)))).is_ok()).expect("a home clear of the streets");
    let others: Vec<_> = homes.iter().enumerate().filter(|&(i, _)| i != k).map(|(_, b)| *b).collect();
    let (gone, smithy, stale) = (homes[k], others[0], others[1]);
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
    let free = agent::building_spot(world, t0, &gone.poly, None, "c:99", None);
    assert!(free.is_ok(), "a removed building's ground is not free: {free:?}");
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
    // And an inn drawn where the city's home was taken away, with an NPC in it and a plot in one
    // of its rooms: it stands in the city (see below).
    let s = &t0.settlements[ci];
    assert!(geom_dist(gc, [s.x, s.y]) < town::holds_radius(s), "the home taken away is out of town");
    let inn_id = format!("c:{}", n0 + 2);
    let (at, name) = agent::building_spot(world, t0, &gone.poly, Some("inn"), &inn_id, None).unwrap();
    let inn = Created { id: inn_id.clone(), kind: "building".into(), x: at[0], y: at[1], name, poly: gone.poly.clone(), floors: Some(2), func: Some("inn".into()), ..Default::default() };
    // A camp in the middle of the city, removed: in no town.
    let gone_camp = Created { id: format!("c:{}", n0 + 3), kind: "camp".into(), x: s.x, y: s.y, name: "Gone Camp".into(), removed: true, ..Default::default() };
    let mut e = f.edits.clone();
    for c in [&keep, &wall, &inn, &gone_camp] {
        let v = serde_json::to_value(c).unwrap();
        e.apply(&worldgen::world::EditOp::Set { field: "created".into(), key: c.id.clone(), value: v.clone() }).unwrap();
        assert_eq!(serde_json::to_value(e.created.last().unwrap()).unwrap(), v, "{} changed on the round trip", c.kind);
    }
    let ib = format!("b:{}:0", t0.settlements.len() + t0.base_pois + n0 + 2);
    e.npcs.insert("n:host".into(), worldgen::world::Npc { name: "Host".into(), location: Some(worldgen::world::NpcPlace { id: ib.clone(), ..Default::default() }), ..Default::default() });
    e.plots.insert("p:upstairs".into(), worldgen::world::Plot { title: "Upstairs".into(), anchors: vec![format!("r:{ib}:0:0")], ..Default::default() });
    // A building drawn by hand is one place: an NPC at its created id, a note under its building id.
    e.npcs.insert("n:cook".into(), worldgen::world::Npc { name: "Cook".into(), location: Some(worldgen::world::NpcPlace { id: inn_id.clone(), ..Default::default() }), ..Default::default() });
    e.notes.insert(ib.clone(), worldgen::world::Note { text: "Rooms upstairs".into(), tags: vec!["inn".into()] });
    let ex = Executor::new(World::new(WorldFile { edits: e, ..f.clone() }).unwrap());
    let (world, t0) = (&ex.world, &ex.t0);
    assert_eq!(world.hash, base.hash, "castles and walls changed the world hash");
    // The inn is the city's: its settlement (and its building's), among the city's sites and
    // businesses, its NPC and plot the city's; `parents` agrees.
    let ii = agent::layout_of(world, t0, &inn_id).expect("the inn's layout");
    assert_eq!(ib, format!("b:{ii}:0"));
    assert!(town::parent_of(t0, ii) == Some(ci) && town::parents(t0).contains(&(ii, ci)), "the inn drawn in the city is not the city's");
    assert_eq!(agent::get(world, t0, &inn_id).unwrap()["settlement"]["id"], city, "the inn's settlement");
    let got = agent::get(world, t0, &ib).unwrap();
    assert!(got["settlement"]["id"] == city && got["site"]["id"] == inn_id, "the inn's building: {}", got["settlement"]);
    assert!(got["npcs"].as_array().unwrap().iter().any(|n| n["id"] == "n:cook"), "the inn's building misses the NPC at its created id");
    let note = agent::get(world, t0, &inn_id).unwrap()["notes"].clone();
    assert!(note["text"] == "Rooms upstairs" && note == got["notes"], "the inn's note differs by its id: {note} vs {}", got["notes"]);
    let (one, two) = (worldgen::world::Note { text: "A".into(), tags: vec!["x".into()] }, worldgen::world::Note { text: "B".into(), tags: vec!["x".into(), "y".into()] });
    assert_eq!(worldgen::world::Note::join(Some(&one), Some(&two)), Some(worldgen::world::Note { text: "A\n\nB".into(), tags: vec!["x".into(), "y".into()] }), "two notes on one place lose text when joined");
    let kids = agent::children(world, t0, &city).unwrap();
    assert!(kids["sites"].as_array().unwrap().iter().any(|s| s["id"] == inn_id && s["building"] == ib), "the inn is not among the city's sites: {}", kids["sites"]);
    assert!(kids["businesses"].as_array().unwrap().iter().any(|b| b["id"] == ib), "the inn is not among the city's businesses");
    let gi = ii + 1;
    assert!(town::parent_of(t0, gi).is_none() && !town::parents(t0).iter().any(|p| p.0 == gi) && !town::sites_in(t0, ci).contains(&gi), "a removed site stands in the city");
    assert!(!kids["sites"].as_array().unwrap().iter().any(|s| s["id"] == gone_camp.id), "a removed site is among the city's sites");
    let got = agent::get(world, t0, &city).unwrap();
    assert!(got["npcs"].as_array().unwrap().iter().any(|n| n["id"] == "n:host") && got["plots"].as_array().unwrap().iter().any(|p| p["id"] == "p:upstairs"), "the city does not show the inn's NPC and plot");
    // Found on the map, it is in the city's district there.
    let quarter = town::layout(world, t0, ci).quarters.iter().find(|q| q.patches.iter().any(|p| geom::contains(p, gc))).map(|q| q.name.clone());
    let pins = serde_json::to_value(worldgen::gazetteer::in_view(world, t0, [gc[0] - 50.0, gc[1] - 50.0, gc[0] + 50.0, gc[1] + 50.0], 100)).unwrap();
    let pin = pins.as_array().unwrap().iter().find(|h| h["id"] == ib).expect("the inn is not found on the map");
    assert!(quarter.is_some() && pin["district"] == serde_json::json!(quarter), "the inn's district: {} (the city's there: {quarter:?})", pin["district"]);
    assert!(pin["district_id"].as_str().is_some_and(|d| d.starts_with(&format!("d:{ci}:"))), "the inn's district id: {}", pin["district_id"]);
    let li = agent::layout_of(world, t0, &keep.id).expect("the castle's layout");
    let l = town::layout(world, t0, li);
    let fi = town::catalog::index_of("castle").unwrap() as u16;
    let k = l.buildings.iter().find(|b| b.func == Some(fi)).expect("the castle has no keep");
    assert_eq!(k.name.as_deref(), Some(keep.name.as_str()), "the keep is not named as the castle");
    // (A big one has an inner curtain too: a gatehouse and a `castles` entry each.)
    assert!(
        !l.walls.is_empty() && l.towers.len() >= 3 && l.gate_towers.len() == 2 * l.castles.len() && (1..=2).contains(&l.castles.len()),
        "the castle's curtain: {} walls, {} towers, {} gate towers, {} curtains",
        l.walls.len(),
        l.towers.len(),
        l.gate_towers.len(),
        l.castles.len()
    );
    castle_works(world, t0, &l, li, &mut 0, &mut 0);
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

/// Places created by hand stay put when the world is remade: keyed by their ids, not by where
/// they fall in the list of layouts, a created ruin's layout and dungeon and a drawn building's
/// layout and interior come out the same after a site drawn in the sketch far away moves them
/// along. Changing one created entry forgets only its own cached layout: the others are kept
/// (the same `Rc`), and every one is what a cold rebuild makes.
#[test]
fn created_sites_survive_a_remade_world() {
    use worldgen::world::Created;
    use worldgen::{agent, interior, town};
    let file: WorldFile = serde_json::from_str(&world_json(1)).unwrap();
    let base = World::new(file.clone()).unwrap();
    let t0 = worldgen::t0::T0::generate(&base);
    let sea = base.params().sea_level_ft;
    // Dry open land two miles from a town, clear of every layout; the far settlement from it.
    let spot = (0..t0.settlements.len() * 8)
        .map(|k| {
            let s = &t0.settlements[k / 8];
            let a = (k % 8) as f64 * std::f64::consts::TAU / 8.0;
            [s.x + 10_560.0 * libm::cos(a), s.y + 10_560.0 * libm::sin(a)]
        })
        .find(|p| {
            let g = t0.sample(p[0], p[1], 20.0);
            g > sea + 20.0 && (t0.sample_water(p[0], p[1]) as f64) < g && town::layouts_near(&base, &t0, [p[0] - 1_200.0, p[1] - 1_200.0, p[0] + 1_200.0, p[1] + 1_200.0]).is_empty()
        })
        .expect("a dry open spot");
    let far = t0.settlements.iter().max_by(|a, b| geom_dist([a.x, a.y], spot).total_cmp(&geom_dist([b.x, b.y], spot))).map(|s| [s.x + 3_000.0, s.y]).unwrap();
    let ell = |c: [f64; 2]| [[0.0, 0.0], [50.0, 0.0], [50.0, 20.0], [20.0, 20.0], [20.0, 45.0], [0.0, 45.0]].map(|p| [c[0] + p[0], c[1] + p[1]]).to_vec();
    let inn = (0..64)
        .map(|k| ell([spot[0] + 400.0 + 60.0 * (k % 8) as f64, spot[1] - 200.0 + 60.0 * (k / 8) as f64].map(|v| (v / 5.0).round() * 5.0)))
        .find_map(|poly| Some((agent::building_spot(&base, &t0, &poly, Some("inn"), "c:1", None).ok()?, poly)))
        .expect("open ground for a building");
    let ((at, name), poly) = inn;
    let mut a = file.clone();
    a.edits.created = vec![
        Created { id: "c:0".into(), kind: "ruin".into(), x: spot[0], y: spot[1], name: "The Kept Ruin".into(), under: Some("dungeon".into()), ..Default::default() },
        Created { id: "c:1".into(), kind: "building".into(), x: at[0], y: at[1], name, poly, floors: Some(2), func: Some("inn".into()), roof: Some("battlements".into()), ..Default::default() },
        Created { id: "c:2".into(), kind: "camp".into(), x: spot[0] - 600.0, y: spot[1] + 600.0, name: "Camp".into(), ..Default::default() },
    ];
    // The same world remade with a site drawn far away: one more of the world's own sites.
    let mut b = a.clone();
    b.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "site", "kind": "ruin", "pts": [far] })).unwrap());
    let (ea, eb) = (Executor::new(World::new(a).unwrap()), Executor::new(World::new(b).unwrap()));
    let first = |ex: &Executor| ex.t0.settlements.len() + ex.t0.base_pois;
    assert_eq!(first(&eb), first(&ea) + 1, "the sketch's site did not move the created places along");
    // A layout, an interior or a site underground as JSON, its layout index left out (ids).
    let plain = |s: String, li: usize| ["b", "t", "u", "k", "w", "d"].iter().fold(s, |s, p| s.replace(&format!("\"{p}:{li}:"), &format!("\"{p}:#:"))).replace(&format!("\"settlement\":{li},"), "\"settlement\":#,");
    let laid = |ex: &Executor, li: usize| {
        let mut l = (*town::layout(&ex.world, &ex.t0, li)).clone();
        assert_eq!(l.index as usize, li);
        l.index = 0;
        format!("{l:?}")
    };
    for k in 0..2 {
        let (la, lb) = (first(&ea) + k, first(&eb) + k);
        assert_eq!(laid(&ea, la), laid(&eb, lb), "c:{k}: laid out anew when the world was remade");
    }
    let (ra, rb) = (first(&ea), first(&eb));
    let l = town::layout(&ea.world, &ea.t0, ra);
    let e = l.entrances.iter().find(|e| e.kind == worldgen::under::UnderKind::Dungeon).expect("no dungeon under the ruin");
    let site = |ex: &Executor, li: usize| plain(serde_json::to_string(&interior::generate_id(&ex.world, &ex.t0, &format!("u:{li}:{}", e.id)).expect("the dungeon")).unwrap(), li);
    assert_eq!(site(&ea, ra), site(&eb, rb), "the ruin's dungeon was rolled again when the world was remade");
    let inside = |ex: &Executor, li: usize| plain(serde_json::to_string(&interior::generate_id(&ex.world, &ex.t0, &format!("b:{li}:0")).expect("the inn's interior")).unwrap(), li);
    assert_eq!(inside(&ea, ra + 1), inside(&eb, rb + 1), "the drawn inn's interior was rolled again when the world was remade");

    // With the Settlements dial at None only pins make towns: a world of three pins remade with a
    // fourth (on the dry land farthest from them) has one more settlement, so the world's own sites
    // move along a number. Each drawn from the same seed (`layout_identity`) is laid out the same,
    // its site underground too.
    let pins: Vec<serde_json::Value> = t0.settlements[..3].iter().map(|s| serde_json::json!({ "tool": "pin", "tier": "town", "pts": [[s.x, s.y]] })).collect();
    let pinned = |pins: &[serde_json::Value]| {
        let mut v = serde_json::to_value(&file).unwrap();
        v["params"]["settlement_density"] = serde_json::json!(0.0);
        v["sketch"] = serde_json::json!({ "strokes": pins });
        Executor::new(World::new(serde_json::from_value(v).unwrap()).unwrap())
    };
    let ep = pinned(&pins);
    let g = &ep.world.geom;
    let lonely = |p: &[f64; 2]| ep.t0.settlements.iter().map(|s| geom_dist([s.x, s.y], *p)).fold(f64::MAX, f64::min);
    let pin = (1..40 * 40)
        .map(|k| [g.map_w_ft * (k % 40) as f64 / 40.0, g.map_h_ft * (k / 40) as f64 / 40.0])
        .filter(|p| {
            let h = ep.t0.sample(p[0], p[1], 20.0);
            h > sea + 50.0 && (ep.t0.sample_water(p[0], p[1]) as f64) < h
        })
        .max_by(|p, q| lonely(p).total_cmp(&lonely(q)))
        .expect("dry land");
    let more = [pins.clone(), vec![serde_json::json!({ "tool": "pin", "tier": "village", "name": "Lonely", "pts": [pin] })]].concat();
    let ec = pinned(&more);
    let n = ep.t0.settlements.len();
    assert_eq!(ec.t0.settlements.len(), n + 1, "the pinned village");
    let (mut alike, mut along, mut dungeons) = (0, 0, 0);
    for pa in 0..ep.t0.base_pois {
        let p = &ep.t0.pois[pa];
        let Some(pc) = (0..ec.t0.base_pois).find(|&k| (ec.t0.pois[k].x - p.x).abs() < 1.0 && (ec.t0.pois[k].y - p.y).abs() < 1.0) else { continue };
        let (la, lc) = (n + pa, n + 1 + pc);
        if town::layout_identity(&ep.t0, la) != town::layout_identity(&ec.t0, lc) {
            continue;
        }
        alike += 1;
        along += (la != lc) as usize;
        assert_eq!(plain(laid(&ep, la), la), plain(laid(&ec, lc), lc), "site {la} (now {lc}): laid out anew though drawn alike");
        let l = town::layout(&ep.world, &ep.t0, la);
        if dungeons < 12
            && let Some(e) = l.entrances.iter().find(|e| e.kind != worldgen::under::UnderKind::Sewer)
        {
            dungeons += 1;
            let under = |ex: &Executor, li: usize| plain(serde_json::to_string(&interior::generate_id(&ex.world, &ex.t0, &format!("u:{li}:{}", e.id)).expect("its site underground")).unwrap(), li);
            assert_eq!(under(&ep, la), under(&ec, lc), "u:{la}:{} (now {lc}): rolled again though drawn alike", e.id);
        }
    }
    assert!(along > 0 && dungeons > 0, "no site moved along and stayed alike ({alike} alike, {along} moved, {dungeons} underground)");

    // The world as generated, remade with a town added (as Edit › Places adds one: a pin beside
    // the world's own) on the dry land farthest from every settlement: every village and site
    // after it moves along a number, and none is pushed out. Settlements and sites are drawn from
    // their cells and places, not their numbers: nearly all stand where they stood, drawn alike
    // (`layout_identity`), laid out the same (but those whose roads changed), the buildings'
    // interiors inside them the same, and named the same.
    let lonely = |p: &[f64; 2]| t0.settlements.iter().map(|s| geom_dist([s.x, s.y], *p)).fold(f64::MAX, f64::min);
    let g = &base.geom;
    let pin = (1..40 * 40)
        .map(|k| [g.map_w_ft * (k % 40) as f64 / 40.0, g.map_h_ft * (k / 40) as f64 / 40.0])
        .filter(|p| {
            let h = t0.sample(p[0], p[1], 20.0);
            h > sea + 50.0 && (t0.sample_water(p[0], p[1]) as f64) < h
        })
        .max_by(|p, q| lonely(p).total_cmp(&lonely(q)))
        .expect("dry land");
    let mut d = file.clone();
    d.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "pin", "tier": "town", "added": true, "pts": [pin] })).unwrap());
    let (e0, ed) = (Executor::new(World::new(file.clone()).unwrap()), Executor::new(World::new(d).unwrap()));
    assert!(ed.t0.settlements.iter().any(|s| s.pin.is_some() && geom_dist([s.x, s.y], pin) < 5_280.0), "the added town");
    let (s0, sd) = (e0.t0.settlements.len(), ed.t0.settlements.len());
    let (mut alike, mut along, mut laid_same, mut insides) = (0, 0, 0, 0);
    for (i, s) in e0.t0.settlements.iter().enumerate() {
        let Some(j) = (0..sd).find(|&j| ed.t0.settlements[j].tier == s.tier && geom_dist([ed.t0.settlements[j].x, ed.t0.settlements[j].y], [s.x, s.y]) < 1.0) else { continue };
        if town::layout_identity(&e0.t0, i) != town::layout_identity(&ed.t0, j) {
            continue;
        }
        alike += 1;
        along += (i != j) as usize;
        if plain(laid(&e0, i), i) != plain(laid(&ed, j), j) {
            continue;
        }
        laid_same += 1;
        // A shifted town's tavern or other house with a trade: its interior as before.
        let l = town::layout(&e0.world, &e0.t0, i);
        if i != j
            && insides < 6
            && let Some(b) = l.buildings.iter().find(|b| b.func.is_some() && b.structure == town::Structure::Roofed)
        {
            insides += 1;
            let inside = |ex: &Executor, li: usize| plain(serde_json::to_string(&interior::generate_id(&ex.world, &ex.t0, &format!("b:{li}:{}", b.id)).expect("an interior")).unwrap(), li);
            assert_eq!(inside(&e0, i), inside(&ed, j), "b:{i}:{} (now {j}): its interior rolled again", b.id);
        }
    }
    let n = e0.t0.settlements.len();
    assert_eq!(sd, n + 1, "a town added beside the world's own pushed one out");
    assert!(alike * 100 >= n * 97 && laid_same * 100 >= alike * 95 && along >= 20 && insides > 0, "an added town re-laid the others: of {n}, {alike} drawn alike ({along} moved along a number), {laid_same} laid out the same, {insides} interiors checked");
    let sites = (0..e0.t0.base_pois)
        .filter(|&pa| {
            let p = &e0.t0.pois[pa];
            (0..ed.t0.base_pois).any(|pc| (ed.t0.pois[pc].x - p.x).abs() < 1.0 && (ed.t0.pois[pc].y - p.y).abs() < 1.0 && town::layout_identity(&e0.t0, s0 + pa) == town::layout_identity(&ed.t0, sd + pc))
        })
        .count();
    assert!(sites * 100 >= e0.t0.base_pois * 95, "an added town moved the world's own sites: {sites} of {} where they were, drawn alike", e0.t0.base_pois);
    let names = |ex: &Executor| ex.t0.extra.as_ref().unwrap().overlay.features.iter().map(|f| (f.id.clone(), f.name.clone())).collect::<std::collections::HashMap<_, _>>();
    let (n0, nd) = (names(&e0), names(&ed));
    let renamed = n0.iter().filter(|(id, name)| nd.get(*id).is_some_and(|m| m != *name)).count();
    assert!(renamed * 100 <= n0.len(), "an added town renamed {renamed} of {} places", n0.len());

    // An added town 5 miles from one of the world's own villages (and no other settlement within
    // 10): that village moves away (the least room kept round a settlement), another is placed in
    // its stead, and every other settlement is drawn as before, each from its own cell, not from
    // how many were placed before it. One drawn on a town is refused.
    use worldgen::t0::settle::Tier;
    let near_village = t0.settlements.iter().enumerate().filter(|(_, s)| s.tier == Tier::Village).find_map(|(i, s)| {
        (0..8).map(|k| (k as f64) * std::f64::consts::TAU / 8.0).map(|a| [s.x + 26_400.0 * libm::cos(a), s.y + 26_400.0 * libm::sin(a)]).find(|p| {
            let h = t0.sample(p[0], p[1], 20.0);
            h > sea + 50.0 && (t0.sample_water(p[0], p[1]) as f64) < h && t0.settlements.iter().enumerate().all(|(j, o)| j == i || geom_dist([o.x, o.y], *p) > 12.0 * 5280.0)
        }).map(|p| (i, p))
    });
    let (vi, at) = near_village.expect("dry land 5 miles from a lone village");
    let added_at = |p: [f64; 2]| {
        let mut f = file.clone();
        f.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "pin", "tier": "town", "added": true, "pts": [p] })).unwrap());
        Executor::new(World::new(f).unwrap())
    };
    let ev = added_at(at);
    let v = &t0.settlements[vi];
    let sv = &ev.t0.settlements;
    assert_eq!(sv.len(), n + 1, "the village pushed away was not placed elsewhere");
    assert!(!sv.iter().any(|s| geom_dist([s.x, s.y], [v.x, v.y]) < 1.0), "the village 5 miles from the added town stayed");
    let kept = (0..n).filter(|&i| i != vi).filter(|&i| {
        let s = &t0.settlements[i];
        (0..sv.len()).any(|j| sv[j].tier == s.tier && geom_dist([sv[j].x, sv[j].y], [s.x, s.y]) < 1.0 && town::layout_identity(&e0.t0, i) == town::layout_identity(&ev.t0, j))
    });
    let kept = kept.count();
    assert!(kept * 100 >= (n - 1) * 97, "a village pushed away by an added town redrew the others: {kept} of {} drawn alike", n - 1);
    let town_at = t0.settlements.iter().find(|s| s.tier == Tier::Town).map(|s| [s.x + 2_000.0, s.y]).unwrap();
    let eo = added_at(town_at);
    let k = eo.world.file.sketch.strokes.len() - 1;
    let refused = eo.t0.extra.as_ref().unwrap().overlay.conflicts.iter().any(|c| c.stroke == k && c.message.contains("within 3 miles of a town"));
    assert!(refused && eo.t0.settlements.len() == n, "a town added on a town was placed ({} settlements)", eo.t0.settlements.len());
    // Twelve miles from a lone village (past the least room, within a village's own spacing of
    // 14): the village stays.
    let past = t0.settlements.iter().filter(|s| s.tier == Tier::Village).find_map(|s| {
        (0..8).map(|k| (k as f64) * std::f64::consts::TAU / 8.0).map(|a| [s.x + 63_360.0 * libm::cos(a), s.y + 63_360.0 * libm::sin(a)]).find(|p| {
            let h = t0.sample(p[0], p[1], 20.0);
            h > sea + 50.0 && (t0.sample_water(p[0], p[1]) as f64) < h && t0.settlements.iter().filter(|o| geom_dist([o.x, o.y], *p) < 16.0 * 5280.0).count() == 1
        }).map(|p| ([s.x, s.y], p))
    });
    let (v12, at12) = past.expect("dry land 12 miles from a lone village");
    let e12 = added_at(at12);
    assert!(e12.t0.settlements.iter().any(|s| s.tier == Tier::Village && geom_dist([s.x, s.y], v12) < 1.0), "a village 12 miles from an added town moved away");

    // One created entry changed (the camp moved): only its layout is forgotten.
    let mut ex = ea;
    let f = first(&ex);
    let kept: Vec<_> = (0..3).map(|k| town::layout(&ex.world, &ex.t0, f + k)).collect();
    let old = ex.world.file.edits.created.clone();
    ex.world.file.edits.created[2].x += 200.0;
    ex.t0.apply_edits(&ex.world);
    town::forget_created(&old, &ex.world.file.edits.created, f);
    let now: Vec<_> = (0..3).map(|k| town::layout(&ex.world, &ex.t0, f + k)).collect();
    assert!(std::rc::Rc::ptr_eq(&kept[0], &now[0]) && std::rc::Rc::ptr_eq(&kept[1], &now[1]), "an unchanged created place was laid out again");
    assert!(!std::rc::Rc::ptr_eq(&kept[2], &now[2]) && geom_dist(now[2].center, [old[2].x + 200.0, old[2].y]) < 1.0, "the moved camp kept its old layout");
    town::forget_from(0);
    for k in 0..3 {
        let cold = town::layout(&ex.world, &ex.t0, f + k);
        assert_eq!(format!("{:?}", now[k]), format!("{cold:?}"), "c:{k}: the cached layout is not what a cold rebuild makes");
    }
}

/// The ward editor (`Edits.towns`) on the largest walled city: one corner moved and one patch
/// made a castle. Patches away from them keep their buildings (ids, footprints, functions and
/// names), the districts their names, the wall its towers; the new buildings pass the interior
/// rules; the castle stands with its keep and towers after every other; the result is
/// repeatable, round-trips as an edit op and leaves the world hash alone. Walls taken down keep
/// the gates and every building; walls put up round an unwalled town stand; a stale edit is set
/// aside; a corner on the water is refused.
fn ward_edits(base: &World, t0: &worldgen::t0::T0) {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, geom, wards};
    use std::rc::Rc;
    use worldgen::world::{Edits, TownEdit};
    let n = t0.settlements.len();
    let plan_of = |li: usize| wards::plan_json(base, t0, li).unwrap();
    let ci = (0..n).filter(|&i| t0.settlements[i].tier >= Tier::City && plan_of(i)["walls"]["generated"] == true).max_by_key(|&i| t0.settlements[i].population).expect("a walled city");
    let g = town::layout(base, t0, ci);
    let plan = plan_of(ci);
    let spacing = plan["max_move_ft"].as_f64().unwrap();
    let patches = plan["patches"].as_array().unwrap();
    let corners = plan["corners"].as_array().unwrap();
    let at = |v: &serde_json::Value| [v["at"][0].as_f64().unwrap(), v["at"][1].as_f64().unwrap()];
    let moves = |c: &serde_json::Value| format!(r#"{{"moves":[{{"corner":{},"by":[30,20]}}]}}"#, c["corner"]);
    let goes = |c: &serde_json::Value| wards::change(base, t0, ci, &serde_json::from_str(&moves(c)).unwrap()).is_ok_and(|(_, r)| r["corners"]["moved"] == 1);
    // (One a main street runs through, so moving it moves the street: see the wall below.)
    let on_street = |c: &serde_json::Value| g.roads.iter().filter(|r| r.1 != 3).any(|r| r.0.iter().any(|&q| geom_dist(q, at(c)) < 1.0));
    let corner = corners.iter().filter(|c| c["pinned"].is_null() && c["gate"].is_null() && c["wall"].is_null() && patches.iter().filter(|p| p["in_town"] == true && p["corners"].as_array().unwrap().contains(&c["corner"])).count() >= 3 && on_street(c)).find(|c| goes(c)).expect("an inner corner that moves");
    let cp = at(corner);
    let patch = patches.iter().filter(|p| p["in_town"] == true && matches!(p["ward"].as_str(), Some("common" | "craft" | "merchant")) && geom_dist(at(p), cp) > 3.0 * spacing).min_by(|a, b| geom_dist(at(a), g.center).total_cmp(&geom_dist(at(b), g.center))).expect("a ward to make a castle");
    let pc = at(patch);
    let req = format!(r#"{{"moves":[{{"corner":{},"by":[30,20]}}],"patches":[{{"patch":{},"ward":"castle"}}]}}"#, corner["corner"], patch["patch"]);
    let req: wards::TownRequest = serde_json::from_str(&req).unwrap();
    let (edit, report) = wards::change(base, t0, ci, &req).expect("the change");
    let edit = edit.expect("an edit");
    assert_eq!(report["corners"]["moved"], 1, "the corner did not move: {report}");
    assert!(report["rects"].as_array().is_some_and(|r| !r.is_empty()), "nothing to draw again: {report}");
    let mut e = Edits::default();
    let v = serde_json::to_value(&edit).unwrap();
    e.apply(&worldgen::world::EditOp::Set { field: "towns".into(), key: wards::key(ci), value: v.clone() }).unwrap();
    assert_eq!(serde_json::to_value(&e.towns[&wards::key(ci)]).unwrap(), v, "the town edit changed on the round trip");
    let world = World::new(WorldFile { edits: e.clone(), ..base.file.clone() }).unwrap();
    assert_eq!(world.hash, base.hash, "a town edit changed the world hash");
    let l = town::layout(&world, t0, ci);
    // Away from the corner and the castle (and the castle's neighbours, whose blocks it borders),
    // every building is as generated.
    let far = |p: [f64; 2]| geom_dist(p, cp) > 2.0 * spacing && geom_dist(p, pc) > 2.5 * spacing;
    let mut kept = 0;
    for b in g.buildings.iter().filter(|b| far(geom::centroid(&b.poly))) {
        let o = l.building(b.id as usize).unwrap_or_else(|| panic!("b:{ci}:{} is gone", b.id));
        assert!(o.poly == b.poly && o.func == b.func && o.name == b.name && o.floors == b.floors, "b:{ci}:{} changed: poly {} {} func {} name {} floors {}", b.id, o.poly.len(), b.poly.len(), o.func == b.func, o.name == b.name, o.floors == b.floors);
        kept += 1;
    }
    assert!(kept > g.buildings.len() / 2, "too few buildings far from the change ({kept} of {})", g.buildings.len());
    assert!(l.buildings.windows(2).all(|w| w[0].id < w[1].id), "building ids out of order");
    assert_eq!(l.quarters.iter().map(|q| &q.name).collect::<Vec<_>>(), g.quarters.iter().map(|q| &q.name).collect::<Vec<_>>(), "district names changed");
    assert!(l.towers == g.towers && l.gate_towers == g.gate_towers, "the wall's towers changed");
    assert!(!l.extra_towers.is_empty() && l.castles.len() > g.castles.len(), "no castle on the patch: {} extra towers, {} castles", l.extra_towers.len(), l.castles.len());
    assert_eq!(worldgen::interior::towers(&l).len(), g.towers.len() + g.gate_towers.len() + l.extra_towers.len());
    let fi = town::catalog::index_of("castle").unwrap() as u16;
    let keep = l.buildings.iter().find(|b| b.id >= wards::NEW_ID && b.func == Some(fi) && geom_dist(geom::centroid(&b.poly), pc) < spacing).expect("the new castle has no keep");
    assert!(keep.floors == 4 && keep.name.is_some(), "the keep: {} storeys, name {:?}", keep.floors, keep.name);
    let fresh: Vec<&town::Building> = l.buildings.iter().filter(|b| b.id >= wards::NEW_ID && b.structure == town::Structure::Roofed).collect();
    assert!(fresh.len() >= 5, "too few new buildings ({})", fresh.len());
    let buckets = building_buckets(&l);
    for b in &fresh {
        check_interior(&world, t0, &l, ci, &format!("b:{ci}:{}", b.id), &buckets);
    }
    for k in g.towers.len() + g.gate_towers.len()..worldgen::interior::towers(&l).len() {
        check_interior(&world, t0, &l, ci, &format!("t:{ci}:{k}"), &buckets);
    }
    // Repeatable: built again with nothing kept.
    let again = format!("{:?}", *l);
    town::forget_from(0);
    assert_eq!(format!("{:?}", *town::layout(&world, t0, ci)), again, "a town laid out anew is not repeatable");

    // A wall drawn by hand across a main street the edit moved takes its gate from the street as
    // laid out, not where the generated one ran (item 48): the same cold or kept, rebuilt after the
    // town changes under it, and left alone by a town edit that moves nothing it reads.
    let lanes = |l: &town::Layout| -> Vec<([f64; 2], [f64; 2])> { l.roads.iter().filter(|r| r.1 != 3).flat_map(|r| r.0.windows(2).map(|q| (q[0], q[1]))).collect() };
    let (before, after) = (lanes(&g), lanes(&l));
    // (Where the street moved most: near the end of a segment the edit changed.)
    let (a, b) = after.iter().copied().find(|&(a, b)| geom::dist(a, b) > 40.0 && !before.contains(&(a, b)) && before.iter().all(|&(p, q)| geom::seg_dist(geom::lerp(a, b, 0.85), p, q) > 5.0)).expect("a main street the edit moved");
    let m = geom::lerp(a, b, 0.85);
    let d = geom::sub(b, a);
    let nrm = geom::mul([-d[1], d[0]], 1.0 / geom::len(d).max(1e-9));
    let wall = worldgen::world::Created { id: "c:0".into(), kind: "wall".into(), x: m[0], y: m[1], name: "Test Wall".into(), pts: vec![geom::add(m, geom::mul(nrm, -60.0)), geom::add(m, geom::mul(nrm, 60.0))], ..Default::default() };
    let with_wall = |mut e: Edits| {
        e.created.push(wall.clone());
        let w = World::new(WorldFile { edits: e, ..base.file.clone() }).unwrap();
        let mut t = worldgen::t0::T0::from_bytes(&t0.to_bytes()).unwrap();
        t.apply_edits(&w);
        (w, t)
    };
    let (wb, tb) = with_wall(Edits::default());
    let (we, te) = with_wall(e.clone());
    let li = n + tb.base_pois;
    let at_m = |g: &[[f64; 2]]| g.chunks(2).any(|p| p.len() == 2 && geom_dist(geom::lerp(p[0], p[1], 0.5), m) < 20.0);
    town::forget_from(0);
    let generated = town::layout(&wb, &tb, li).gate_towers.clone();
    let laid_out = town::layout(&we, &te, li);
    assert!(at_m(&laid_out.gate_towers), "no gate where the wall crosses the street as laid out: {:?} at {m:?}", laid_out.gate_towers);
    assert_ne!(generated, laid_out.gate_towers, "the wall kept the gates of the generated streets");
    let kept = format!("{:?}", *laid_out);
    town::forget_from(0);
    assert_eq!(format!("{:?}", *town::layout(&we, &te, li)), kept, "a wall built after the town was laid out anew differs from one built before");
    let patch = patches.iter().filter(|p| p["in_town"] == true && matches!(p["ward"].as_str(), Some("common" | "craft" | "merchant")) && p["patch"] != patch["patch"] && geom_dist(at(p), m) > 5.0 * spacing && geom_dist(at(p), pc) > 3.0 * spacing).min_by(|a, b| geom_dist(at(b), m).total_cmp(&geom_dist(at(a), m))).expect("a ward far from the wall");
    let req = format!(r#"{{"patches":[{{"patch":{},"lots":"huge"}}]}}"#, patch["patch"]);
    let (edit5, _) = wards::change(&world, t0, ci, &serde_json::from_str(&req).unwrap()).unwrap();
    let mut e5 = e.clone();
    e5.towns.insert(wards::key(ci), edit5.expect("lots changed"));
    let (w5, t5) = with_wall(e5);
    let l5 = town::layout(&w5, t0, ci);
    assert!(lanes(&l5) == after && !Rc::ptr_eq(&l5, &l), "the far edit moved a main street or changed nothing");
    let held = town::layout(&we, &te, li);
    assert!(Rc::ptr_eq(&town::layout(&w5, &t5, li), &held), "a wall was laid out again by a town edit that moved nothing it reads");

    // Walls taken down: the gates and every building stay (but a castle's, whose ranges may stand
    // otherwise where the town wall was its wall), ids and all, no id twice; only castle curtains
    // stand.
    let (edit2, _) = wards::change(&world, t0, ci, &serde_json::from_str(r#"{"walls":false}"#).unwrap()).unwrap();
    let mut e2 = e.clone();
    e2.towns.insert(wards::key(ci), edit2.unwrap());
    let w2 = World::new(WorldFile { edits: e2, ..base.file.clone() }).unwrap();
    let l2 = town::layout(&w2, t0, ci);
    assert_eq!(l2.gates, l.gates, "the gates moved with the walls down");
    let rest = |l: &town::Layout| l.buildings.iter().filter(|b| b.ward != town::catalog::Ward::Castle).map(|b| (b.id, b.poly.clone())).collect::<Vec<_>>();
    assert!(rest(&l2) == rest(&l), "buildings changed with the walls down");
    let mut ids: Vec<u32> = l2.buildings.iter().map(|b| b.id).collect();
    ids.sort_unstable();
    assert!(ids.windows(2).all(|w| w[0] != w[1]), "a building id twice with the walls down");
    assert!(l2.walls.iter().flatten().all(|p| l2.castles.iter().any(|(c, r)| geom_dist(*p, *c) <= r + 30.0)), "a town wall still stands");
    assert!(l2.towers.len() < l.towers.len() && l2.gate_towers.len() < l.gate_towers.len(), "the wall towers still stand");

    // Walls put up round an unwalled town.
    if let Some(ti) = (0..n).filter(|&i| t0.settlements[i].tier == Tier::Town && plan_of(i)["walls"]["generated"] == false).max_by_key(|&i| t0.settlements[i].population) {
        let (edit3, _) = wards::change(base, t0, ti, &serde_json::from_str(r#"{"walls":true}"#).unwrap()).unwrap();
        let mut e3 = Edits::default();
        e3.towns.insert(wards::key(ti), edit3.expect("walls put up"));
        let w3 = World::new(WorldFile { edits: e3, ..base.file.clone() }).unwrap();
        let (open, walled) = (town::layout(base, t0, ti), town::layout(&w3, t0, ti));
        assert!(open.walls.is_empty() && !walled.walls.is_empty() && walled.towers.len() >= 3 && walled.gates == open.gates, "no walls put up round town {ti}");
    }

    // Set aside when the town is elsewhere; a corner on the water refused.
    let mut e4 = e.clone();
    e4.towns.insert(wards::key(ci), TownEdit { at: [edit.at[0] + 5.0, edit.at[1]], ..edit.clone() });
    let w4 = World::new(WorldFile { edits: e4, ..base.file.clone() }).unwrap();
    assert!(wards::edit_of(&w4, t0, ci).is_none() && wards::set_aside(&w4, t0).iter().any(|(k, _)| *k == wards::key(ci)), "a stale town edit was applied");
    assert_eq!(format!("{:?}", *town::layout(&w4, t0, ci)), format!("{:?}", *g), "a stale town edit changed the town");
    if let Some(pinned) = corners.iter().find(|c| c["pinned"] == true) {
        let req = format!(r#"{{"moves":[{{"corner":{},"by":[10,0]}}]}}"#, pinned["corner"]);
        assert!(wards::change(base, t0, ci, &serde_json::from_str(&req).unwrap()).is_err(), "a corner on the water moved");
    }
    let village = (0..n).find(|&i| t0.settlements[i].tier == Tier::Village).unwrap();
    assert!(wards::plan_json(base, t0, village).is_err(), "a village has wards");
}

/// Castles set in the ward editor by the town wall (the patches nearest it in the largest walled town
/// with no castle of its own, two of them, walls up and taken down): each keeps to its ground
/// (`castle_ground`), and taking the wall down moves no other building, its id and outline the same.
fn ward_castles_by_the_wall(base: &World, t0: &worldgen::t0::T0) {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, geom, wards};
    use worldgen::world::Edits;
    let n = t0.settlements.len();
    let mut towns: Vec<usize> = (0..n).filter(|&i| t0.settlements[i].tier == Tier::Town).collect();
    towns.sort_by_key(|&i| (std::cmp::Reverse(t0.settlements[i].population), i));
    let (ti, plan) = towns
        .into_iter()
        .map(|i| (i, wards::plan_json(base, t0, i).unwrap()))
        .find(|(_, p)| p["walls"]["generated"] == true && p["patches"].as_array().unwrap().iter().all(|q| q["ward"] != "castle"))
        .expect("a walled town with no castle");
    let g = town::layout(base, t0, ti);
    let at = |v: &serde_json::Value| [v["at"][0].as_f64().unwrap(), v["at"][1].as_f64().unwrap()];
    let to_wall = |p: [f64; 2]| g.walls.iter().flat_map(|w| w.windows(2)).map(|s| geom::seg_dist(p, s[0], s[1])).fold(f64::MAX, f64::min);
    let mut near: Vec<&serde_json::Value> = plan["patches"].as_array().unwrap().iter().filter(|p| p["in_town"] == true && matches!(p["ward"].as_str(), Some("common" | "craft" | "merchant" | "patriciate" | "slum"))).collect();
    near.sort_by(|a, b| to_wall(at(a)).total_cmp(&to_wall(at(b))));
    let mut picked: Vec<&serde_json::Value> = Vec::new();
    for p in near {
        if picked.len() < 2 && picked.iter().all(|q| geom_dist(at(q), at(p)) > 250.0) {
            picked.push(p);
        }
    }
    assert!(picked.len() == 2, "too few patches by the wall of #{ti}");
    for patch in picked {
        let laid = |walls: &str| {
            let req = format!(r#"{{"patches":[{{"patch":{},"ward":"castle"}}]{walls}}}"#, patch["patch"]);
            let (edit, _) = wards::change(base, t0, ti, &serde_json::from_str(&req).unwrap()).expect("the change");
            let mut e = Edits::default();
            e.towns.insert(wards::key(ti), edit.expect("an edit"));
            let w = World::new(WorldFile { edits: e, ..base.file.clone() }).unwrap();
            (town::layout(&w, t0, ti), w)
        };
        let ((up, wu), (down, wd)) = (laid(""), laid(r#","walls":false"#));
        for (l, w) in [(&up, &wu), (&down, &wd)] {
            let ring = &l.plazas.iter().filter(|p| p.kind == town::PlazaKind::Bailey).min_by(|a, b| geom_dist(geom::centroid(&a.pts), at(patch)).total_cmp(&geom_dist(geom::centroid(&b.pts), at(patch)))).expect("a bailey").pts;
            castle_ground(w, t0, l, ti, ring);
        }
        let rest = |l: &town::Layout| l.buildings.iter().filter(|b| b.ward != town::catalog::Ward::Castle).map(|b| (b.id, b.poly.clone())).collect::<Vec<_>>();
        assert!(rest(&down) == rest(&up), "#{ti} patch {}: buildings changed with the walls down round a castle set by hand", patch["patch"]);
    }
}

/// Ground shaped by hand (`Edits.ground`, `Edits.plateaus`) at the largest settlement: a sheer
/// plateau across a chunk corner and a mound brushed across a chunk edge. The four chunks agree at
/// their seams bit for bit; the plateau's top is flat at its level on the drawn ground (leaving
/// out roads, buildings, plaza features, walls and towers); lowering a dry shore (by brush and by
/// a pit) leaves every dry square dry; terrain samples never see the edits; the edits survive a
/// round trip as JSON and as edit ops; clearing them gives the chunks back byte for byte.
fn ground_edits(base: &World) {
    use worldgen::battlemap::{self, Chunk, HS, SQ, SQUARE_FT};
    use worldgen::ground::{self, Brush, Stroke};
    use worldgen::world::{EditOp, Edits};
    let mut ex = Executor::new(World::new(base.file.clone()).unwrap());
    let g = ex.world.geom.clone();
    let lvl = g.max_level;
    let size = g.tile_size_ft(lvl);
    let s = ex.t0.settlements.iter().max_by_key(|s| s.population).unwrap().clone();
    let corner = [(s.x / size).round() * size, (s.y / size).round() * size];
    let (kx, ky) = ((corner[0] / size) as u32, (corner[1] / size) as u32);
    let keys = [(kx - 1, ky - 1), (kx, ky - 1), (kx - 1, ky), (kx, ky)].map(|(x, y)| TileKey::surface(lvl, x, y));
    let pack = |ex: &mut Executor, k: TileKey| {
        let c = ex.battlemap(k);
        (battlemap::pack(&ex.world, &c), c)
    };
    let plain: Vec<(Vec<u8>, Chunk)> = keys.iter().map(|&k| pack(&mut ex, k)).collect();
    let padded: Vec<Vec<f32>> = keys.iter().map(|&k| ex.terrain(k).padded.clone()).collect();
    // The tiles round the corner at the four finest levels, as generated.
    let tile_keys: Vec<TileKey> = (lvl - 3..=lvl)
        .flat_map(|l| {
            let ts = g.tile_size_ft(l);
            let mut ks: Vec<TileKey> = [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]].iter().map(|d| TileKey::surface(l, ((corner[0] + d[0]) / ts) as u32, ((corner[1] + d[1]) / ts) as u32)).collect();
            ks.sort_by_key(|k| (k.y, k.x));
            ks.dedup();
            ks
        })
        .collect();
    let plain_tiles: Vec<Vec<u8>> = tile_keys.iter().map(|&k| ex.terrain_packed(k)).collect();
    // A square's centre (world ft) and its height in the chunk holding it.
    let centre = |k: &TileKey, i: usize, j: usize| [k.x as f64 * size + (i as f64 + 0.5) * SQUARE_FT, k.y as f64 * size + (j as f64 + 0.5) * SQUARE_FT];
    let half = 60.0;
    let inside = |p: [f64; 2], m: f64| (p[0] - corner[0]).abs() <= half - m && (p[1] - corner[1]).abs() <= half - m;
    let mut top = f32::MIN;
    for (q, k) in keys.iter().enumerate() {
        for k2 in 0..SQ * SQ {
            if inside(centre(k, k2 % SQ, k2 / SQ), 0.0) {
                top = top.max(plain[q].1.height[k2]);
            }
        }
    }
    let level = (top + 10.0).round();
    let outline = [[-half, -half], [half, -half], [half, half], [-half, half]].map(|d| [corner[0] + d[0], corner[1] + d[1]]);
    let plateau = ground::plateau_for(&outline, level as f64, 0.0).expect("the plateau");
    let mut e = Edits::default();
    e.plateaus.insert("h:test".into(), plateau);
    // The mound: on the edge between the southern two, where both sides have open dry ground.
    let open = |c: &Chunk, k: &TileKey, at: [f64; 2]| -> Vec<usize> {
        (0..SQ * SQ).filter(|&k2| geom_dist(centre(k, k2 % SQ, k2 / SQ), at) < 20.0 && c.building[k2] == 0 && c.water_level[k2] <= c.height[k2]).collect()
    };
    // (Centred in a row of blocks, 80 ft: the brush's 40 ft reach keeps to that row.)
    let dab = (6..28)
        .filter(|t| t % 4 == 2)
        .map(|t| [corner[0], corner[1] + 20.0 * t as f64])
        .find(|&p| [2, 3].iter().all(|&q| open(&plain[q].1, &keys[q], p).len() >= 4))
        .map(|p| [p])
        .expect("open ground on the chunks' edge");
    let changed = ground::stamp_brush(&mut e.ground, &Stroke { mode: Brush::Raise, path: &dab, radius_ft: 40.0, height_ft: 12.0, hardness: 0.5, strength: 1.0 }, &|_, _| 0.0).unwrap();
    assert!(changed.len() == 2 && changed.iter().all(|k| e.ground.contains_key(k)), "the mound's blocks: {changed:?}");
    // As JSON and as ops, unchanged.
    let json = serde_json::to_string(&e).unwrap();
    assert_eq!(serde_json::from_str::<Edits>(&json).unwrap(), e, "shaped ground changed on a JSON round trip");
    let mut by_ops = Edits::default();
    for op in Edits::default().diff(&e) {
        let undo = by_ops.apply(&op).unwrap();
        assert!(matches!(undo, EditOp::Unset { .. }), "a new entry's undo is not an unset");
    }
    assert_eq!(by_ops, e, "shaped ground changed through edit ops");
    let ops = e.diff(&Edits::default());
    let mut back = e.clone();
    let undo: Vec<EditOp> = ops.iter().map(|op| back.apply(op).unwrap()).collect();
    assert!(back.is_empty(), "shaped ground left after its ops were undone");
    for op in undo.iter().rev() {
        back.apply(op).unwrap();
    }
    assert_eq!(back, e, "shaped ground not restored by the inverse ops");

    ex.world.file.edits = e.clone();
    let shaped: Vec<(Vec<u8>, Chunk)> = keys.iter().map(|&k| pack(&mut ex, k)).collect();
    assert!(shaped.iter().zip(&plain).all(|(a, b)| a.0 != b.0), "a chunk the shaping reaches is unchanged");
    for (q, k) in keys.iter().enumerate() {
        assert_eq!(ex.terrain(*k).padded, padded[q], "terrain samples changed with shaped ground");
    }
    // Seams: each chunk's halo ring is its neighbour's edge, bit for bit (heights and water).
    let ring = |a: &Chunk, b: &Chunk, east: bool| {
        for t in 0..SQ {
            let (ah, bk) = if east { ((t + 1) * HS + SQ + 1, t * SQ) } else { ((SQ + 1) * HS + t + 1, t) };
            let (bh, ak) = if east { ((t + 1) * HS, t * SQ + SQ - 1) } else { (t + 1, (SQ - 1) * SQ + t) };
            assert_eq!(a.halo_height[ah].to_bits(), b.height[bk].to_bits(), "shaped ground: seam at {t} ({})", if east { "east" } else { "south" });
            assert_eq!(b.halo_height[bh].to_bits(), a.height[ak].to_bits(), "shaped ground: seam at {t} ({})", if east { "west" } else { "north" });
            assert_eq!(a.halo_water[ah].to_bits(), b.water_level[bk].to_bits(), "shaped ground: water seam at {t}");
        }
    };
    ring(&shaped[0].1, &shaped[1].1, true);
    ring(&shaped[2].1, &shaped[3].1, true);
    ring(&shaped[0].1, &shaped[2].1, false);
    ring(&shaped[1].1, &shaped[3].1, false);
    // The mound stands, on both sides of the edge.
    for q in [2, 3] {
        let (a, b) = (&plain[q].1, &shaped[q].1);
        let open = open(a, &keys[q], dab[0]);
        let rise = open.iter().map(|&k2| b.height[k2] - a.height[k2]).sum::<f32>() / open.len().max(1) as f32;
        assert!(!open.is_empty() && rise >= 8.0, "no mound in chunk {:?} ({} open squares, {rise} ft up)", keys[q], open.len());
    }
    // Flat on top: squares two in from the outline, as drawn.
    let mut flat = 0;
    for (q, k) in keys.iter().enumerate() {
        let c = &shaped[q].1;
        for k2 in 0..SQ * SQ {
            let (i, j) = (k2 % SQ, k2 / SQ);
            let h = (j + 1) * HS + i + 1;
            if !inside(centre(k, i, j), 2.0 * SQUARE_FT) || c.building[k2] != 0 || c.road_halo[h] != 0 || matches!(c.urban[k2], battlemap::URBAN_PLAZA | battlemap::URBAN_WALL | battlemap::URBAN_TOWER) {
                continue;
            }
            let drawn = c.halo_height[h] - c.roof_lift[h];
            assert!((drawn - level).abs() <= 0.1, "shaped ground: square ({i}, {j}) of {k:?} at {drawn} on a plateau at {level}");
            flat += 1;
        }
    }
    // (How many depends on the buildings standing there: at least enough to mean something.)
    assert!(flat >= 50, "too few open squares on the plateau ({flat})");
    // Tiles: the three finest levels draw the shaping (texels of the shaped heights, the plateau
    // at its level at the corner) and send their samples on as generated; coarser ones unchanged.
    use worldgen::core::tile::{PADDED, TILE_SAMPLES};
    let mut drawn: Vec<(TileKey, Vec<f32>)> = Vec::new();
    for (q, &k) in tile_keys.iter().enumerate() {
        let bytes = ex.terrain_packed(k);
        if k.level + 2 < lvl {
            assert!(bytes == plain_tiles[q], "tile {k:?} above the three finest levels changed with shaped ground");
            continue;
        }
        let samples = 48..48 + PADDED * PADDED * 4;
        assert!(bytes != plain_tiles[q], "tile {k:?} doesn't show the shaped ground");
        assert!(bytes[samples.clone()] == plain_tiles[q][samples.clone()], "tile {k:?}'s samples for its children changed with shaped ground");
        let tile = ex.terrain(k);
        let h = worldgen::payload::shaped_heights(&ex.world, &ex.t0, &k, &tile).expect("shaped heights");
        let base = f32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let tex = samples.end;
        for j in 0..TILE_SAMPLES {
            for i in 0..TILE_SAMPLES {
                let t = tex + (j * TILE_SAMPLES + i) * 8;
                let v = u16::from_le_bytes([bytes[t], bytes[t + 1]]);
                assert_eq!(v, worldgen::payload::f16_bits(h[(j + HALO) * PADDED + i + HALO] - base), "tile {k:?}'s texel ({i}, {j}) isn't its shaped height");
            }
        }
        let ts = g.tile_size_ft(k.level);
        let (ci, cj) = (((corner[0] - k.x as f64 * ts) / ts * TILE_N as f64).round() as usize, ((corner[1] - k.y as f64 * ts) / ts * TILE_N as f64).round() as usize);
        if ci <= TILE_N && cj <= TILE_N {
            let at = h[(cj + HALO) * PADDED + ci + HALO];
            assert!((at - level).abs() < 0.01, "tile {k:?} draws the plateau's corner at {at}, not {level}");
        }
        drawn.push((k, h));
    }
    // Seamless: the samples two tiles share are shaped alike, bit for bit.
    let mut seams = 0;
    for (a, ha) in &drawn {
        for (b, hb) in &drawn {
            let edge = |t: usize, east: bool, h: &[f32], far: bool| {
                let e = if far { TILE_N } else { 0 };
                let (i, j) = if east { (e, t) } else { (t, e) };
                h[(j + HALO) * PADDED + i + HALO].to_bits()
            };
            for east in [true, false] {
                let next = if east { b.level == a.level && b.x == a.x + 1 && b.y == a.y } else { b.level == a.level && b.y == a.y + 1 && b.x == a.x };
                if next {
                    seams += 1;
                    for t in 0..TILE_SAMPLES {
                        assert_eq!(edge(t, east, ha, true), edge(t, east, hb, false), "shaped ground: tile seam {a:?} | {b:?} at {t}");
                    }
                }
            }
        }
    }
    assert!(seams >= 4, "too few tile seams checked ({seams})");
    // Cleared: as generated, byte for byte.
    ex.world.file.edits = Edits::default();
    for (q, k) in keys.iter().enumerate() {
        assert!(pack(&mut ex, *k).0 == plain[q].0, "chunk {k:?} differs once the shaping is cleared");
    }
    for (q, &k) in tile_keys.iter().enumerate() {
        assert!(ex.terrain_packed(k) == plain_tiles[q], "tile {k:?} differs once the shaping is cleared");
    }

    // A building across a chunk edge, raised on one side only (a sheer plateau 10 ft past the
    // edge, under its far corners): both chunks put its roof at one height, raised as much as its
    // ground floor inside.
    let si = (0..ex.t0.settlements.len()).max_by_key(|&i| ex.t0.settlements[i].population).unwrap();
    let l = worldgen::town::layout(&ex.world, &ex.t0, si);
    let (b, seam) = l
        .buildings
        .iter()
        .filter(|b| b.structure == worldgen::town::Structure::Roofed)
        .find_map(|b| {
            let (x0, x1) = b.poly.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[0]), a.1.max(p[0])));
            let (y0, y1) = b.poly.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
            let seam = (x1 / size).floor() * size;
            (seam > x0 + 15.0 && x1 > seam + 15.0 && (y0 / size).floor() == (y1 / size).floor() && y0 - (y0 / size).floor() * size > 15.0 && ((y1 / size).floor() + 1.0) * size - y1 > 15.0)
                .then_some((b.clone(), seam))
        })
        .expect("a building across a chunk edge");
    let (x1, (y0, y1)) = (
        b.poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max),
        b.poly.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1]))),
    );
    let ky = (y0 / size).floor() as u32;
    let sides = [TileKey::surface(lvl, (seam / size) as u32 - 1, ky), TileKey::surface(lvl, (seam / size) as u32, ky)];
    let id = format!("b:{si}:{}", b.id);
    let roof = |ex: &mut Executor, k: TileKey| {
        let c = ex.battlemap(k);
        let mine: Vec<f32> = (0..SQ * SQ).filter(|&k2| c.building[k2] != 0 && c.buildings[c.building[k2] as usize - 1] == (l.index, b.id)).map(|k2| c.height[k2]).collect();
        assert!(!mine.is_empty() && mine.iter().all(|h| h.to_bits() == mine[0].to_bits()), "building {id}'s roof is not level in chunk {k:?}");
        mine[0]
    };
    let ground_floor = |ex: &Executor| worldgen::interior::generate_id(&ex.world, &ex.t0, &id).expect("the building's interior").levels.iter().find(|v| v.z == 0).unwrap().elevation_ft;
    let before = (sides.map(|k| roof(&mut ex, k)), ground_floor(&ex));
    let outline = [[seam + 10.0, y0 - 20.0], [x1 + 20.0, y0 - 20.0], [x1 + 20.0, y1 + 20.0], [seam + 10.0, y1 + 20.0]];
    let mut e = Edits::default();
    e.plateaus.insert("h:side".into(), ground::plateau_for(&outline, b.pad_ft as f64 + 12.0, 0.0).unwrap());
    ex.world.file.edits = e;
    let after = (sides.map(|k| roof(&mut ex, k)), ground_floor(&ex));
    let rise = after.0[0] - before.0[0];
    assert_eq!(after.0[0].to_bits(), after.0[1].to_bits(), "building {id} across a chunk edge stands at {} and {} on ground shaped on one side", after.0[0], after.0[1]);
    assert!(rise >= 2.0, "building {id} rose only {rise} ft with a plateau 12 ft up under its far corners");
    assert!((after.1 - before.1 - rise).abs() < 1e-3, "building {id}'s ground floor rose {} ft, its roof {rise} ft", after.1 - before.1);
    ex.world.file.edits = Edits::default();

    // Lowering never floods: a dry square on a shore, dug by brush and by a pit.
    let settlements = ex.t0.settlements.clone();
    let (key, k2) = settlements
        .iter()
        .map(|s| TileKey::surface(lvl, (s.x / size) as u32, (s.y / size) as u32))
        .find_map(|k| {
            let c = ex.battlemap(k);
            let wet = |k2: usize| c.water_level[k2] > c.height[k2];
            (20..SQ - 20)
                .flat_map(|j| (20..SQ - 20).map(move |i| j * SQ + i))
                .find(|&k2| !wet(k2) && c.building[k2] == 0 && wet(k2 + 1) && wet(k2 + 2))
                .map(|k2| (k, k2))
        })
        .expect("a shore");
    let c = ex.battlemap(key);
    let p = centre(&key, k2 % SQ, k2 / SQ);
    let mut e = Edits::default();
    ground::stamp_brush(&mut e.ground, &Stroke { mode: Brush::Lower, path: &[p], radius_ft: 30.0, height_ft: 20.0, hardness: 0.8, strength: 1.0 }, &|_, _| 0.0).unwrap();
    let pit = [[-25.0, -25.0], [25.0, -25.0], [25.0, 25.0], [-25.0, 25.0]].map(|d| [p[0] - 30.0 + d[0], p[1] + d[1]]);
    e.plateaus.insert("h:pit".into(), ground::plateau_for(&pit, c.height[k2] as f64 - 30.0, 10.0).unwrap());
    ex.world.file.edits = e;
    let dug = ex.battlemap(key);
    let lowered = (0..SQ * SQ).filter(|&k| dug.height[k] < c.height[k] - 1.0).count();
    assert!(lowered >= 20, "the shore was hardly lowered ({lowered} squares)");
    for k in 0..SQ * SQ {
        assert!(c.water_level[k] > c.height[k] || dug.water_level[k] <= dug.height[k], "lowering flooded square ({}, {}) of {key:?}", k % SQ, k / SQ);
    }
    ex.world.file.edits = Edits::default();
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
