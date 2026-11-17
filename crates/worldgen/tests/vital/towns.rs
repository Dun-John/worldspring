//! Towns against roads and water, the settlement catalog, hand towns and the
//! acceptance city.

use super::*;

/// Towns meet their roads and water: every road that ends at a town or city continues from
/// its end to a gate; every pier starts on land (dry ground within 20 ft of one end) and
/// stands over water.
#[test]
fn towns_meet_roads_and_water() {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, geom};
    let mut ex = Executor::new(World::from_json(&world_json(1)).unwrap());
    let (world, t0) = (&ex.world, &ex.t0);
    // Isolated realms (world 1 has some): each walled, with one track and one gate, no other road
    // within 20 miles of it or of its track but where that joins them, no town or king's road
    // within 100 miles, each village's track a spur of its own to the realm's; and the
    // hierarchy's own tier counts as without them.
    {
        let sts = &t0.settlements;
        let realms: Vec<usize> = (0..sts.len()).filter(|&i| sts[i].isolated && sts[i].tier >= Tier::Town).collect();
        assert!(!realms.is_empty(), "world 1 has no isolated realm");
        for &i in &realms {
            let s = &sts[i];
            let c = [s.x, s.y];
            let trim = town::road_trim_radius(s.tier, s.population) + 1_500.0;
            let own: Vec<[f64; 2]> = std::iter::once(c).chain(sts.iter().filter(|v| v.isolated && v.tier == Tier::Village && geom::dist([v.x, v.y], c) < 12.0 * 5280.0).map(|v| [v.x, v.y])).collect();
            let rs = &t0.roads.roads;
            let ends = |r: &worldgen::lod::roads::RoadCurve| [r.pts[0], r.pts[r.pts.len() - 1]];
            let tracks = rs.iter().filter(|r| ends(r).iter().any(|&p| geom::dist(p, c) < trim)).count();
            assert_eq!(tracks, 1, "realm {i} at {c:?}: {tracks} roads reach it");
            // (Its own: those ending at it or a village of its, or mostly along its routes as found.)
            let mine: Vec<[f64; 2]> = t0.extra.as_ref().unwrap().realm_paths.iter().filter(|p| p.0 == i).flat_map(|p| p.1.iter().copied()).collect();
            let along = |r: &worldgen::lod::roads::RoadCurve| 2 * r.pts.iter().filter(|&&p| mine.iter().any(|&q| geom::dist(p, q) < 1.5 * t0.cell_ft)).count() >= r.pts.len();
            let paths: Vec<&Vec<[f64; 2]>> = t0.extra.as_ref().unwrap().realm_paths.iter().filter(|p| p.0 == i).map(|p| &p.1).collect();
            let track = paths.iter().find(|p| geom::dist(p[0], c) < t0.cell_ft).expect("a realm's track");
            let outer = track[track.len() - 1];
            let far: Vec<[f64; 2]> = track.iter().copied().filter(|&q| geom::dist(q, outer) > 20.0 * 5280.0).collect();
            for r in rs.iter().filter(|r| !along(r) && !ends(r).iter().any(|&p| own.iter().any(|&o| geom::dist(p, o) < trim))) {
                let near = r.pts.iter().map(|&p| geom::dist(p, c)).fold(f64::INFINITY, f64::min);
                assert!(near > 20.0 * 5280.0, "realm {i} at {c:?}: a {:?} passes {near:.0} ft from it", r.class);
                let by = r.pts.iter().flat_map(|&p| far.iter().map(move |&q| geom::dist(p, q))).fold(f64::INFINITY, f64::min);
                assert!(by > 18.0 * 5280.0, "realm {i} at {c:?}: a {:?} passes {by:.0} ft from its track", r.class);
                if r.class == worldgen::t0::roads::RoadClass::KingsRoad {
                    let gap = r.pts.iter().flat_map(|&p| own.iter().map(move |&o| geom::dist(p, o))).fold(f64::INFINITY, f64::min);
                    assert!(gap >= 100.0 * 5280.0, "realm {i} at {c:?}: a king's road {gap:.0} ft from it");
                }
            }
            for o in sts.iter().filter(|o| !o.isolated && o.tier >= Tier::Town) {
                let gap = own.iter().map(|&p| geom::dist(p, [o.x, o.y])).fold(f64::INFINITY, f64::min);
                assert!(gap >= 100.0 * 5280.0, "realm {i} at {c:?}: a {:?} {gap:.0} ft from it", o.tier);
            }
            for sp in paths.iter().filter(|p| geom::dist(p[0], c) >= t0.cell_ft) {
                let end = sp[sp.len() - 1];
                assert!(track.iter().any(|&q| geom::dist(q, end) < 1.5 * t0.cell_ft), "realm {i} at {c:?}: a village's track from {:?} ends off the realm's", sp[0]);
            }
            let l = town::layout(world, t0, i);
            assert!(!l.walls.is_empty() && l.gates.len() == 1, "realm {i} at {c:?}: walls {}, gates {}", l.walls.len(), l.gates.len());
        }
        let count = |t: Tier| sts.iter().filter(|s| s.tier == t && !s.isolated).count();
        let v = count(Tier::Village);
        assert_eq!([count(Tier::Town), count(Tier::City), count(Tier::Metropolis)], [(v / 5).max(2), (v / 18).max(1), (v / 90).max(1)], "the tiers' counts moved ({v} villages)");
    }
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
    fn channel(t0: &worldgen::t0::T0, bb: [f64; 4]) -> Channel {
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
    }
    // More than `inset` ft inside a channel.
    fn in_channel(ch: &Channel, p: [f64; 2], inset: f64) -> bool {
        ch.1.get(&((p[0] / 100.0).floor() as i64, (p[1] / 100.0).floor() as i64)).into_iter().flatten().any(|&i| {
            let (a, b, hw) = ch.0[i];
            geom::seg_dist(p, a, b) < hw - inset
        })
    }
    // Steps only in cities and up; each flight long and high enough, on dry land outside plazas,
    // and clear of the streets crossing it (alleys may open onto it from the side). Collects the
    // flights (settlement, centreline, rise) and counts the pieces.
    fn check_steps(t0: &worldgen::t0::T0, i: usize, l: &town::Layout, flights: &mut Vec<(usize, Vec<[f64; 2]>, f32)>, pieces: &mut usize) {
        let prm = town::stairs::StairParams::DEFAULT;
        let s = &t0.settlements[i];
        assert!(s.tier >= town::stairs::STAIR_TIER || l.stairs.is_empty(), "settlement {i} ({:?}) has steps", s.tier);
        let ch = if l.stairs.is_empty() { Default::default() } else { channel(t0, l.bbox) };
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
            *pieces += 1;
        }
    }
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
    // Flights of steps (settlement, centreline, rise), for the drawn ground below.
    let prm = town::stairs::StairParams::DEFAULT;
    let mut flights: Vec<(usize, Vec<[f64; 2]>, f32)> = Vec::new();
    let mut stair_pieces = 0;
    for (i, s) in t0.settlements.iter().enumerate() {
        let l = town::layout(world, t0, i);
        all_piers.extend(l.piers.iter().map(|p| (i, ends(p).0, ends(p).1)));
        check_steps(t0, i, &l, &mut flights, &mut stair_pieces);
        if s.tier < Tier::Town {
            continue;
        }
        // Walls end at the banks (water guards the gap), towers stand on land, and every street
        // over a river runs on a bridge.
        let ch = channel(t0, l.bbox);
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
    // (Since item 101 about half the villages stand inland, so fewer have piers.)
    assert!(all_piers.len() > 120, "too few piers: {}", all_piers.len());
    for (i, e1, e2) in all_piers {
        assert!(over_water(e1) || over_water(e2), "settlement {i}: the pier at {:?} ends on drawn land", geom::lerp(e1, e2, 0.5));
    }
    // Seed 1's cities stand on flat river and harbour sites: steep streets are found in a world
    // whose cities have them (seed 13), its cities' flights checked as above.
    let mut sx = Executor::new(World::from_json(&world_json(13)).unwrap());
    let mut steep: Vec<(usize, Vec<[f64; 2]>, f32)> = Vec::new();
    for i in (0..sx.t0.settlements.len()).filter(|&i| sx.t0.settlements[i].tier >= town::stairs::STAIR_TIER) {
        let l = town::layout(&sx.world, &sx.t0, i);
        check_steps(&sx.t0, i, &l, &mut steep, &mut stair_pieces);
    }
    // Flights stand on the ground the battlemap draws: dry, and climbing it as steeply as they
    // were judged to (less a little for the sampling).
    assert!(steep.len() >= 3 && stair_pieces >= flights.len() + steep.len(), "too few flights of steps: {} ({stair_pieces} pieces)", steep.len());
    for (x, fl) in [(&mut ex, &flights), (&mut sx, &steep)] {
        let g = x.world.geom.clone();
        let (s, size) = (g.spacing_ft(g.max_level), g.tile_size_ft(g.max_level));
        for (i, pts, rise) in fl {
            let (mut climb, mut length) = (0.0, 0.0);
            let mut last: Option<f64> = None;
            for seg in pts.windows(2) {
                let n = (geom::dist(seg[0], seg[1]) / 5.0).ceil().max(1.0) as usize;
                for j in 0..=n {
                    let p = geom::lerp(seg[0], seg[1], j as f64 / n as f64);
                    let key = TileKey::surface(g.max_level, (p[0] / size) as u32, (p[1] / size) as u32);
                    let (ox, oy) = g.tile_origin_ft(&key);
                    let k = padded_index(((p[0] - ox) / s).round() as i64, ((p[1] - oy) / s).round() as i64);
                    let tile = x.terrain(key);
                    assert!(x.t0.sample_water(p[0], p[1]).max(tile.river_water[k]) <= tile.padded[k], "settlement {i}: steps under water at {p:?}");
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
    }
    // The steepest flights are seed 13's (its world's geometry the same as seed 1's).
    assert_eq!(format!("{:?}", sx.world.geom), format!("{g:?}"));
    let flights = &steep;
    // The battlemap draws the highest flight as steps: most of its centreline (2.5 ft in from its
    // ends) in the chunk holding its middle; steps only on dry street squares, off network roads,
    // and under nothing that blocks the way.
    use worldgen::battlemap::{self as bm, SQ, SQUARE_FT, Surface};
    let (i, pts, _) = flights.iter().max_by(|a, b| a.2.total_cmp(&b.2)).unwrap();
    let mid = geom::lerp(pts[0], pts[pts.len() - 1], 0.5);
    let key = TileKey::surface(g.max_level, (mid[0] / size) as u32, (mid[1] / size) as u32);
    let (ox, oy) = g.tile_origin_ft(&key);
    let c = sx.battlemap(key);
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
    sx.world.file.edits.plateaus.insert("h:level".into(), ground::plateau_for(&outline, at_square(&c, mid) as f64, 0.0).unwrap());
    let levelled = sx.battlemap(key);
    let left = line.iter().filter(|&&k| levelled.surface[k] == Surface::Stairs).count();
    assert!(left == 0 && line.iter().all(|&k| levelled.surface[k] == Surface::Cobble), "a plateau over the flight at {mid:?} leaves {left} of its {} squares as steps", line.len());
    sx.world.file.edits = Default::default();
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
    // The economy's re-tiering keeps each tier's count, moves a place a tier at most, keeps the
    // capital a metropolis, and sizes each place within its tier.
    {
        use worldgen::t0::settle::{Tier, pop_range};
        let places = &t0.extra.as_ref().unwrap().economy.places;
        assert!(places.len() > 10, "too few towns ranked ({})", places.len());
        for tier in [Tier::Town, Tier::City, Tier::Metropolis] {
            let (was, now) = (places.iter().filter(|p| p.was == tier).count(), places.iter().filter(|p| t0.settlements[p.index].tier == tier).count());
            assert_eq!(was, now, "{tier:?}: {was} placed, {now} after re-tiering");
        }
        for p in places {
            let s = &t0.settlements[p.index];
            assert!((s.tier as i32 - p.was as i32).abs() <= 1, "settlement {} went from {:?} to {:?}", p.index, p.was, s.tier);
            assert!(!s.capital || s.tier == Tier::Metropolis, "the capital is a {:?}", s.tier);
        }
        for (i, s) in t0.settlements.iter().enumerate() {
            let (lo, hi) = pop_range(s.tier);
            assert!(s.population as f64 >= lo - 1.0 && s.population as f64 <= hi, "settlement {i} ({:?}) has {} people", s.tier, s.population);
        }
    }
    for i in 0..t0.settlements.len() {
        let l = town::generate(&world, &t0, i);
        let mut have = vec![0usize; CATALOG.len()];
        for b in &l.buildings {
            if let Some(f) = b.func {
                have[f as usize] += 1;
            }
        }
        assert!(have[inn] >= 1, "settlement {i} ({:?}) has no inn", l.tier);
        // (A village's church stands for its shrine: `town::village::reserve`.)
        let temple = CATALOG.iter().position(|f| f.key == "temple").unwrap();
        for (fi, f) in CATALOG.iter().enumerate() {
            let need = required_count(f, l.tier, l.on_water);
            let has = if f.key == "shrine" && l.tier == worldgen::t0::settle::Tier::Village { have[fi] + have[temple] } else { have[fi] };
            // (A village's mill stands only by the water: `town::village::mill`.)
            let need = if f.key == "mill" && l.tier == worldgen::t0::settle::Tier::Village { 0 } else { need };
            assert!(has >= need, "settlement {i} ({:?}) has {} of {} (needs {need})", l.tier, has, f.name);
        }
    }
}

/// Towns laid out by hand (Roadmap 6, P3): converting a town gives the same records twice, and its
/// layout equals the generated one (buildings, ids, walls and towers within 1/64 ft); two unrelated
/// edits give the same records and layout in either order; a world remade round it keeps it as it
/// was; its battlemap keeps the guarantee (buildings, cover, seams); a blank town with streets drawn
/// round its centre builds blocks of homes, and its population is counted from them.
#[test]
fn hand_town_guarantee() {
    use worldgen::t0::{T0, remap, settle::Tier};
    use worldgen::town::{self, Layout, hand, handedit};
    let base = World::from_json(&world_json(3)).unwrap();
    let t0 = T0::generate(&base);
    let ask = |w: &World, t0: &T0, li: usize, req: &str| -> World {
        let asked: handedit::HandRequest = serde_json::from_str(req).unwrap();
        let (ch, _) = handedit::change(w, t0, li, &asked).unwrap_or_else(|e| panic!("L{li} {req}: {e}"));
        let mut f = w.file.clone();
        ch.apply(&mut f.edits);
        World::new(f).unwrap()
    };
    let near = |a: f64, b: f64| (a - b).abs() <= 1.0 / 64.0 + 1e-9;
    let same = |a: &Layout, b: &Layout, what: &str| {
        let (mut x, mut y) = (a.buildings.iter().collect::<Vec<_>>(), b.buildings.iter().collect::<Vec<_>>());
        x.sort_by_key(|b| b.id);
        y.sort_by_key(|b| b.id);
        assert_eq!(x.len(), y.len(), "{what}: buildings");
        for (p, q) in x.iter().zip(&y) {
            assert!(p.id == q.id && p.func == q.func && p.poly.len() == q.poly.len(), "{what}: building {} / {}", p.id, q.id);
            assert!(p.poly.iter().zip(&q.poly).all(|(u, v)| near(u[0], v[0]) && near(u[1], v[1])), "{what}: building {} moved", p.id);
        }
        assert_eq!(a.walls.len(), b.walls.len(), "{what}: walls");
        for (p, q) in a.walls.iter().zip(&b.walls) {
            assert!(p.len() == q.len() && p.iter().zip(q).all(|(u, v)| near(u[0], v[0]) && near(u[1], v[1])), "{what}: a wall moved");
        }
        assert!(a.towers.len() == b.towers.len() && a.towers.iter().zip(&b.towers).all(|(u, v)| near(u[0], v[0]) && near(u[1], v[1])), "{what}: towers");
    };
    // The biggest town (a city's conversion takes long): converted twice the same, as generated.
    let li = (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier == Tier::Town).max_by_key(|&i| (t0.settlements[i].population, std::cmp::Reverse(i))).expect("a town");
    assert_eq!(hand::convert(&base, &t0, li).unwrap(), hand::convert(&base, &t0, li).unwrap(), "L{li}: converted otherwise the second time");
    let w1 = ask(&base, &t0, li, r#"{"op": "lay_out_by_hand"}"#);
    let gen_l = town::generated_layout(&base, &t0, li);
    let hand_l = town::base_layout(&w1, &t0, li);
    same(&hand_l, &gen_l, &format!("L{li} converted"));
    let (people, tier) = hand::size(&w1, li).expect("counted");
    assert!(people > 0 && tier >= Tier::Village, "L{li}: {people} people");

    // Two unrelated edits, in either order: a street across one block, another block refilled.
    let plan = handedit::plan_json(&w1, &t0, li).unwrap();
    let recs = hand::HandRecords::of(&w1.file.edits.streets, li);
    let ids = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect::<Vec<u32>>()).unwrap_or_default();
    let lots: Vec<&serde_json::Value> = plan["blocks"].as_array().unwrap().iter().filter(|b| b["fill"]["kind"] == "generated" && ids(&b["streets"]).len() >= 4).collect();
    let mid = |id: u32| {
        let st = &recs.streets[&id];
        let line: Vec<[f64; 2]> = std::iter::once(recs.junctions[&st.a].p).chain(st.bends.iter().copied()).chain([recs.junctions[&st.b].p]).collect();
        worldgen::town::geom::lerp(line[0], line[1], 0.5)
    };
    let a = lots[0];
    let round = ids(&a["streets"]);
    let far = round.iter().copied().skip(1).max_by(|x, y| geom_dist(mid(*x), mid(round[0])).total_cmp(&geom_dist(mid(*y), mid(round[0]))).then(y.cmp(x))).unwrap();
    let (m0, m1) = (mid(round[0]), mid(far));
    let split = format!(r#"{{"op": "add_street", "points": [[{}, {}], [{}, {}]]}}"#, m0[0], m0[1], m1[0], m1[1]);
    let b = lots.iter().skip(1).find(|b| ids(&b["streets"]).iter().all(|e| !round.contains(e))).expect("a block apart");
    let refill = format!(r#"{{"op": "set_block", "block": {}, "fill": {{"kind": "generated", "ward": "craft"}}}}"#, b["id"]);
    let ab = ask(&ask(&w1, &t0, li, &split), &t0, li, &refill);
    let ba = ask(&ask(&w1, &t0, li, &refill), &t0, li, &split);
    assert_eq!(ab.file.edits.streets, ba.file.edits.streets, "L{li}: the order of two unrelated edits changed the records");
    let edited = town::base_layout(&ab, &t0, li);
    same(&edited, &town::base_layout(&ba, &t0, li), &format!("L{li} edits in either order"));
    // Back to generated: the generated town again.
    same(&town::base_layout(&ask(&ab, &t0, li, r#"{"op": "back_to_generated"}"#), &t0, li), &gen_l, &format!("L{li} back to generated"));

    // A world remade round it (a pin far away): the town as it was laid by hand.
    let far_s = t0.settlements.iter().max_by(|p, q| geom_dist([p.x, p.y], [t0.settlements[li].x, t0.settlements[li].y]).total_cmp(&geom_dist([q.x, q.y], [t0.settlements[li].x, t0.settlements[li].y]))).unwrap();
    let mut nf = ab.file.clone();
    nf.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "pin", "tier": "village", "pts": [[far_s.x + 15_840.0, far_s.y]] })).unwrap());
    let new = World::new(nf.clone()).unwrap();
    let (t1, followed) = remap::remake(&ab, &t0, &new, &[]);
    let edits = followed.map_or_else(|| ab.file.edits.clone(), |f| f.edits);
    let nl = edits.streets.keys().find_map(|k| k.parse::<usize>().ok()).expect("the hand town's header");
    assert!(nl < worldgen::world::PARKED_LAYOUT, "L{li}: set aside by the remake");
    let wharves = |e: &worldgen::world::Edits, n: usize| e.wharves.keys().filter(|k| k.starts_with(&format!("q:h{n}-"))).count();
    assert_eq!(wharves(&edits, nl), wharves(&ab.file.edits, li), "L{li}: its wharves not moved with it (now L{nl})");
    nf.edits = edits;
    let remade = World::new(nf).unwrap();
    same(&town::base_layout(&remade, &t1, nl), &edited, &format!("L{li} remade (now L{nl})"));

    // Its battlemap: the chunk at its centre and those east and south of it.
    {
        use worldgen::battlemap::{self, HS, SQ};
        let s = &t0.settlements[li];
        let mut ex = Executor::new(World::new(ab.file.clone()).unwrap());
        let g = ex.world.geom.clone();
        let size = g.tile_size_ft(g.max_level);
        let key = TileKey::surface(g.max_level, (s.x / size) as u32, (s.y / size) as u32);
        let c = ex.battlemap(key);
        assert!(c.building.iter().any(|&b| b != 0), "L{li}: its battlemap at its centre has no buildings");
        let dry: Vec<bool> = (0..SQ * SQ).map(|k| c.water_level[k] <= c.height[k] && c.surface[k] != battlemap::Surface::Planks).collect();
        let dist = battlemap::cover_distance(&c);
        for k in 0..SQ * SQ {
            assert!(!dry[k] || dist[k] <= battlemap::cover_gap(&c, k), "L{li} {key:?}: square {k} is {} squares from cover", dist[k]);
        }
        let east = ex.battlemap(TileKey::surface(key.level, key.x + 1, key.y));
        let south = ex.battlemap(TileKey::surface(key.level, key.x, key.y + 1));
        for j in 0..SQ {
            assert_eq!(c.halo_height[(j + 1) * HS + SQ + 1].to_bits(), east.height[j * SQ].to_bits(), "L{li} {key:?}: battlemap seam at row {j}");
            assert_eq!(c.halo_height[(SQ + 1) * HS + j + 1].to_bits(), south.height[j].to_bits(), "L{li} {key:?}: battlemap seam at column {j}");
        }
    }

    // A blank town (another town's place): its roads in to its centre, then four streets round
    // the centre make blocks of the header's fill, built on, its people counted from its homes.
    let lb = (0..t0.settlements.len()).filter(|&i| i != li && t0.settlements[i].tier == Tier::Town).max_by_key(|&i| (t0.settlements[i].population, std::cmp::Reverse(i))).expect("another town");
    let mut f = base.file.clone();
    let recs = hand::blank(&base, &t0, lb).unwrap();
    assert!(recs.iter().all(|(k, _)| !k.contains("/k")), "a blank town has no blocks");
    let roads_in = recs.iter().any(|(k, _)| k.contains("/e"));
    f.edits.streets.extend(recs);
    let mut wb = World::new(f).unwrap();
    let l0 = town::base_layout(&wb, &t0, lb);
    assert!(l0.buildings.is_empty(), "L{lb}: a blank town has buildings");
    assert!(!roads_in || l0.roads.iter().any(|r| r.1 == 4), "L{lb}: a blank town's streets are not drawn");
    let c = [t0.settlements[lb].x, t0.settlements[lb].y];
    let r = 260.0;
    let sq = [[-r, -r], [r, -r], [r, r], [-r, r]].map(|d| [c[0] + d[0], c[1] + d[1]]);
    for k in 0..4 {
        let (p, q) = (sq[k], sq[(k + 1) % 4]);
        wb = ask(&wb, &t0, lb, &format!(r#"{{"op": "add_street", "points": [[{}, {}], [{}, {}]]}}"#, p[0], p[1], q[0], q[1]));
    }
    let lbl = town::base_layout(&wb, &t0, lb);
    assert!(lbl.buildings.len() >= 10, "L{lb}: a blank town with streets drawn built {} buildings", lbl.buildings.len());
    // (A layout's buildings stand in world ft.)
    let out = lbl.buildings.iter().flat_map(|b| b.poly.iter()).map(|p| (p[0] - c[0]).abs().max((p[1] - c[1]).abs())).fold(0.0, f64::max);
    assert!(out <= r, "L{lb}: a building {out:.0} ft out from its centre, outside its streets");
    let (n, _) = hand::size(&wb, lb).expect("counted");
    assert_eq!(n, hand::people_of(&lbl), "L{lb}: its people not counted from its homes");
    assert!(n > 0, "L{lb}: no people");
    // (The same again from nothing: determinism.)
    same(&lbl, &town::base_layout(&World::new(wb.file.clone()).unwrap(), &T0::generate(&base), lb), &format!("L{lb} blank, built again"));
}

/// The acceptance city (Roadmap 6 P11; `tests/fixtures/acceptcity.json`, written by `examples/acceptcity.rs`
/// through the change functions): seed 3's metropolis laid out by hand and grown to ~3.5 mi across, and a
/// blank town laid out from a street plan. The grown city laid out twice the same; battlemaps across it
/// built, none empty, each square within reach of cover (a cemetery's middle too), no building over a
/// street (a row's neither) or in the water; the blank town built (its wall gated where its streets run
/// out), its people counted. Native times are printed.
#[test]
fn acceptance_city() {
    use worldgen::battlemap::{self, SQ};
    use worldgen::town::{self, hand};
    let mut file: WorldFile = serde_json::from_str(include_str!("../fixtures/acceptcity.json")).unwrap();
    file.gen_version = worldgen::world::GEN_VERSION;
    let start = std::time::Instant::now();
    let mut ex = Executor::new(World::new(file).unwrap());
    let t_t0 = start.elapsed().as_secs_f64();
    let head = |from: &str| ex.world.file.edits.streets.iter().find(|(k, v)| k.parse::<usize>().is_ok() && v["from"] == from).map(|(k, _)| k.parse::<usize>().unwrap()).unwrap_or_else(|| panic!("no {from} hand town"));
    let (city, blank) = (head("generated"), head("blank"));
    let print = |l: &town::Layout| format!("{:?}", (l.buildings.iter().map(|b| (b.id, b.func, b.poly.iter().map(|p| [p[0].to_bits(), p[1].to_bits()]).collect::<Vec<_>>())).collect::<Vec<_>>(), &l.walls, &l.towers, &l.gates));
    let t = std::time::Instant::now();
    let a = hand::build_hand(&ex.world, &ex.t0, city).expect("the city laid out").layout;
    let t_city = t.elapsed().as_secs_f64();
    let b = hand::build_hand(&ex.world, &ex.t0, city).expect("the city laid out").layout;
    assert!(print(&a) == print(&b), "L{city}: laid out otherwise the second time");
    assert!(a.buildings.len() > 20_000 && !a.walls.is_empty() && !a.gates.is_empty(), "L{city}: {} buildings, {} walls, {} gates", a.buildings.len(), a.walls.len(), a.gates.len());
    let bb = a.buildings.iter().flat_map(|b| b.poly.iter()).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])]);
    let across = (bb[2] - bb[0]).max(bb[3] - bb[1]) / 5280.0;
    assert!(across >= 3.0, "L{city}: {across:.2} mi across");

    // The blank town: its blocks built, its wall gated, its people its homes'.
    let lb = town::base_layout(&ex.world, &ex.t0, blank);
    assert!(lb.buildings.len() >= 100 && !lb.walls.is_empty() && lb.gates.len() >= 4, "L{blank}: {} buildings, {} walls, {} gates", lb.buildings.len(), lb.walls.len(), lb.gates.len());
    assert_eq!(hand::size(&ex.world, blank).map(|s| s.0), Some(hand::people_of(&lb)), "L{blank}: its people not counted from its homes");

    // The concave castle beside it: three gates (a pair of gate towers each), its round keep.
    let ci = ex.world.file.edits.created.iter().position(|c| c.kind == "castle").expect("no castle");
    let cl = town::layout(&ex.world, &ex.t0, ex.t0.settlements.len() + ex.t0.base_pois + ci);
    let fi = town::catalog::index_of("castle").unwrap() as u16;
    assert!(cl.gate_towers.len() >= 6 && cl.buildings.iter().any(|b| b.func == Some(fi)), "the castle: {} gate towers, keep {}", cl.gate_towers.len(), cl.buildings.iter().any(|b| b.func == Some(fi)));

    // Battlemaps: a 6 x 6 sample over the city (in each cell, at the building nearest its middle: the
    // city's bounds take in sea and fields) and the blank town's centre.
    let g = ex.world.geom.clone();
    let size = g.tile_size_ft(g.max_level);
    let mids: Vec<[f64; 2]> = a.buildings.iter().map(|b| town::geom::centroid(&b.poly)).collect();
    let mut keys: Vec<TileKey> = (0..36)
        .filter_map(|k| {
            let (w, h) = ((bb[2] - bb[0]) / 6.0, (bb[3] - bb[1]) / 6.0);
            let (x0, y0) = (bb[0] + w * (k % 6) as f64, bb[1] + h * (k / 6) as f64);
            let mid = [x0 + 0.5 * w, y0 + 0.5 * h];
            mids.iter().filter(|p| p[0] >= x0 && p[0] < x0 + w && p[1] >= y0 && p[1] < y0 + h).min_by(|p, q| geom_dist(**p, mid).total_cmp(&geom_dist(**q, mid))).copied()
        })
        .map(|p| TileKey::surface(g.max_level, (p[0] / size) as u32, (p[1] / size) as u32))
        .collect();
    let grid = keys.clone();
    let gridded = grid.len();
    keys.push(TileKey::surface(g.max_level, (lb.center[0] / size) as u32, (lb.center[1] / size) as u32));
    // (And one in each open block: squares, parks, gardens, a cemetery and fields far from any building.)
    let recs = hand::HandRecords::of(&ex.world.file.edits.streets, city);
    let open: Vec<&hand::Block> = recs.blocks.values().filter(|b| matches!(b.fill, hand::BlockFill::Open { .. })).collect();
    keys.extend(open.iter().step_by(open.len().div_ceil(24).max(1)).map(|b| TileKey::surface(g.max_level, (b.at[0] / size) as u32, (b.at[1] / size) as u32)));
    keys.sort_by_key(|k| (k.y, k.x));
    keys.dedup();
    let chunks = keys.len();
    let t = std::time::Instant::now();
    let (mut squares, mut with_buildings, mut grid_bare) = (0, 0, 0);
    for key in keys {
        let c = ex.battlemap(key);
        with_buildings += !c.buildings.is_empty() as usize;
        // (Each grid chunk was picked at a building's middle: it has buildings.)
        grid_bare += (c.buildings.is_empty() && grid.contains(&key)) as usize;
        // (Not empty: buildings, or a town's open ground, a square, a park, a cemetery, fields.)
        assert!(!c.buildings.is_empty() || c.urban.iter().any(|&u| u != 0), "{key:?}: an empty battlemap");
        let dist = battlemap::cover_distance(&c);
        for k in 0..SQ * SQ {
            let dry = c.water_level[k] <= c.height[k] && c.surface[k] != battlemap::Surface::Planks;
            assert!(!dry || dist[k] <= battlemap::cover_gap(&c, k), "{key:?}: square {k} is {} squares from cover", dist[k]);
            if c.building[k] != 0 {
                squares += 1;
                assert!(c.water_level[k] <= c.height[k], "{key:?}: a building in the water at square {k}");
            }
        }
    }
    let t_maps = t.elapsed().as_secs_f64();
    assert!(gridded >= 24 && grid_bare == 0, "{with_buildings} of {chunks} battlemaps with buildings ({gridded} on the grid, {grid_bare} of them bare)");
    // No building laid by hand (a block's or a row's) over a street's paved way (as towncheck's
    // refill check: up to 10 ft each side of its line, its last 12 ft to each junction left out).
    for (li, l) in [(city, &a), (blank, &lb)] {
        use worldgen::town::clip::{self, Ends, Join};
        let recs = hand::HandRecords::of(&ex.world.file.edits.streets, li);
        let per = worldgen::town::wards::PER_PATCH * worldgen::town::wards::ROLLS;
        let laid: Vec<&town::Building> = l.buildings.iter().filter(|b| b.id >= hand::HAND_ID).collect();
        let rows = laid.iter().filter(|b| (b.id - hand::HAND_ID) / per >= hand::ROW_BLOCK).count();
        let cell = |p: [f64; 2]| ((p[0] / 64.0).floor() as i64, (p[1] / 64.0).floor() as i64);
        let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
        for (k, b) in laid.iter().enumerate() {
            let bb = b.poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])]);
            let (c0, c1) = (cell([bb[0], bb[1]]), cell([bb[2], bb[3]]));
            for x in c0.0..=c1.0 {
                for y in c0.1..=c1.1 {
                    grid.entry((x, y)).or_default().push(k);
                }
            }
        }
        let (mut over, mut checked) = (Vec::new(), 0);
        for (id, st) in &recs.streets {
            let (Some(j0), Some(j1)) = (recs.junctions.get(&st.a), recs.junctions.get(&st.b)) else { continue };
            let mut line: Vec<[f64; 2]> = std::iter::once(j0.p).chain(st.bends.iter().copied()).chain([j1.p]).collect();
            let trim = |line: &mut Vec<[f64; 2]>| {
                let (p, q) = (line[0], line[1]);
                let d = geom_dist(p, q);
                line[0] = [p[0] + (q[0] - p[0]) * 12.0 / d, p[1] + (q[1] - p[1]) * 12.0 / d];
                d > 24.0
            };
            if st.set_l + st.set_r <= 0.0 || !trim(&mut line) || !{
                line.reverse();
                trim(&mut line)
            } {
                continue;
            }
            checked += 1;
            let buf = clip::buffer(&line, st.set_r.min(10.0), st.set_l.min(10.0), Join::Miter(2.0), Ends::Flat);
            let bb = line.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])]);
            let (c0, c1) = (cell([bb[0] - 20.0, bb[1] - 20.0]), cell([bb[2] + 20.0, bb[3] + 20.0]));
            let mut near: Vec<usize> = (c0.0..=c1.0).flat_map(|x| (c0.1..=c1.1).map(move |y| (x, y))).filter_map(|c| grid.get(&c)).flatten().copied().collect();
            near.sort_unstable();
            near.dedup();
            for k in near {
                let o = clip::area(&clip::intersection(&buf, &clip::region(&laid[k].poly)));
                if o > 1.0 {
                    over.push(format!("b{} over e{id}: {o:.1} sq ft", laid[k].id));
                }
            }
        }
        assert!(over.is_empty(), "L{li}: {} buildings over a street: {:?}", over.len(), &over[..over.len().min(8)]);
        // (The check not gone vacuous: streets and row buildings looked at.)
        let (want_streets, want_rows) = if li == city { (500, 20) } else { (10, 0) };
        assert!(checked >= want_streets && rows >= want_rows, "L{li}: {checked} streets, {rows} row buildings checked");
    }
    println!(
        "acceptance city: T0 {t_t0:.1} s, L{city} laid out in {t_city:.1} s ({} buildings, {across:.2} mi across), L{blank} {} buildings, {chunks} battlemaps in {t_maps:.1} s ({squares} building squares); {:.1} s in all",
        a.buildings.len(),
        lb.buildings.len(),
        start.elapsed().as_secs_f64()
    );
}
