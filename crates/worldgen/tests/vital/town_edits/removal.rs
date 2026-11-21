//! Roads and fences removed by hand.

use super::*;

/// (Roadmap 6 R5) Roads removed by hand, one of each kind (a stretch of the network where it
/// crosses a river, a town's street over a bridge, a village's lane, a common's path): drawn
/// nowhere, laid out as before. The terrain and the layouts are as generated (gates and all); no
/// battlemap square on a removed line is road, street or gravel; the stretch's bridge or ford and
/// the street's deck go; no packed tile line at any level runs on the stretch; the same whatever
/// is asked first and after the layouts are forgotten; brought back, all is as generated.
#[test]
fn road_removal() {
    use worldgen::battlemap::{self as bm, Chunk, HS, SQ, SQUARE_FT, ShapeKind, URBAN_DECK, URBAN_PATH, URBAN_STREET};
    use worldgen::lod::{rivers, roads};
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, ParkKind, cut, geom, places::Spot, wards};
    use worldgen::world::TownEdit;
    type P = [f64; 2];
    let file: WorldFile = serde_json::from_str(&world_json(1)).unwrap();
    let mut ex = Executor::new(World::new(file).unwrap());
    let g = ex.world.geom.clone();
    let base_edits = ex.world.file.edits.clone();
    let seed = ex.world.stream("battlemap");
    let line_d = |p: P, pts: &[P]| pts.windows(2).map(|w| geom::seg_dist(p, w[0], w[1])).fold(f64::MAX, f64::min);
    let fine = g.max_level;
    let fsize = g.tile_size_ft(fine);
    let key_at = |level: u8, p: P| TileKey::surface(level, (p[0] / g.tile_size_ft(level)) as u32, (p[1] / g.tile_size_ft(level)) as u32);
    // (Far enough inside its battlemap that what is drawn of it is drawn there.)
    let inner = |p: P| (0..2).all(|a| (p[a] - (p[a] / fsize).floor() * fsize).min(((p[a] / fsize).floor() + 1.0) * fsize - p[a]) > 120.0);
    let mid = |pts: &[P]| {
        let half = 0.5 * pts.windows(2).map(|w| geom_dist(w[0], w[1])).sum::<f64>();
        let mut run = 0.0;
        for w in pts.windows(2) {
            let d = geom_dist(w[0], w[1]);
            if run + d >= half {
                return geom::lerp(w[0], w[1], (half - run) / d.max(1e-9));
            }
            run += d;
        }
        pts[0]
    };
    let (ri, at, rd, ti, rk, dk, vi, lk, pi, pk, pj, lid, sid, spot) = {
        let (world, t0) = (&ex.world, &ex.t0);
        let n = t0.settlements.len();
        // A stretch over a river (a bridge or a ford) far from towns and other stretches.
        let near_town = |p: P| t0.settlements.iter().any(|s| geom_dist([s.x, s.y], p) < town::reach(s) + 2_000.0);
        let tile = 40_000.0;
        let mut found = None;
        'scan: for ty in 0..(g.map_h_ft / tile).ceil() as i64 {
            for tx in 0..(g.map_w_ft / tile).ceil() as i64 {
                let rect = [tx as f64 * tile, ty as f64 * tile, (tx + 1) as f64 * tile, (ty + 1) as f64 * tile];
                if t0.roads.segments_near(rect, 0.0).is_empty() {
                    continue;
                }
                let vp = rivers::pieces(&t0.rivers, rect, &|_| 0.0, 40.0, t0.cell_ft);
                for p in roads::pieces(&t0.roads, rect, 0.0, 40.0) {
                    for c in roads::river_crossings(std::slice::from_ref(&p), &vp) {
                        let clear = t0.roads.segments_near([c.at[0] - 600.0, c.at[1] - 600.0, c.at[0] + 600.0, c.at[1] + 600.0], 0.0).iter().all(|s| s.0 == p.road);
                        if c.kind != worldgen::t0::roads::CrossingKind::Ferry && clear && !near_town(c.at) && inner(c.at) {
                            found = Some((p.road as usize, c.at));
                            break 'scan;
                        }
                    }
                }
            }
        }
        let (ri, at) = found.expect("a stretch over a river away from towns");
        let rd = worldgen::roadids::road_ids(t0)[ri].clone();
        let mut towns: Vec<usize> = (0..n).filter(|&i| t0.settlements[i].tier >= Tier::Town).collect();
        towns.sort_by_key(|&i| (std::cmp::Reverse(t0.settlements[i].population), i));
        // A town's street over a bridge, the only one over it.
        let (ti, rk, dk) = towns
            .iter()
            .find_map(|&i| {
                let l = town::layout(world, t0, i);
                l.bridges.iter().enumerate().filter(|(_, d)| l.lined.is_none() && inner(geom::centroid(d))).find_map(|(dk, d)| {
                    let c = geom::centroid(d);
                    let on: Vec<usize> = l.roads.iter().enumerate().filter(|(_, r)| line_d(c, &r.0) <= 0.5 * r.2 + 2.0).map(|(k, _)| k).collect();
                    (on.len() == 1).then(|| (i, on[0], dk))
                })
            })
            .expect("a town's street over a bridge");
        // A village's lane.
        let (vi, lk) = (0..n)
            .filter(|&i| t0.settlements[i].tier == Tier::Village)
            .find_map(|i| {
                let l = town::layout(world, t0, i);
                l.streets.iter().enumerate().find(|(_, s)| s.windows(2).map(|w| geom_dist(w[0], w[1])).sum::<f64>() >= 150.0 && inner(mid(s))).map(|(k, _)| (i, k))
            })
            .expect("a village's lane");
        // A common's longest path.
        let (pi, pk, pj) = towns
            .iter()
            .find_map(|&i| {
                let l = town::layout(world, t0, i);
                l.parks.iter().enumerate().filter(|(_, p)| p.kind == ParkKind::Common && p.spot != Spot::None).find_map(|(k, pk)| {
                    let lay = bm::park_layout(pk, bm::land_biome(t0, geom::centroid(&pk.pts)), l.tier >= Tier::City, seed);
                    let len = |pts: &[P]| pts.windows(2).map(|w| geom_dist(w[0], w[1])).sum::<f64>();
                    let j = (0..lay.paths.len()).max_by(|&a, &b| len(&lay.paths[a].0).total_cmp(&len(&lay.paths[b].0)))?;
                    (len(&lay.paths[j].0) >= 40.0 && inner(mid(&lay.paths[j].0))).then_some((i, k, j))
                })
            })
            .expect("a common's path");
        let (tl, vl, pl) = (town::layout(world, t0, ti), town::layout(world, t0, vi), town::layout(world, t0, pi));
        let sid = cut::street_ids(world, t0, &tl).into_iter().find(|x| x.0 == cut::ROADS && x.1 == rk).unwrap().2;
        let lid = cut::street_ids(world, t0, &vl).into_iter().find(|x| x.0 == cut::STREETS && x.1 == lk).unwrap().2;
        assert_eq!(cut::parse(&cut::street_id(ti, sid)), Some((ti, format!("{sid:08x}"))), "a street's id does not read back");
        (ri, at, rd, ti, rk, dk, vi, lk, pi, pk, pj, lid, sid, cut::park_spot(&pl, pk))
    };
    let layouts = |ex: &Executor| [ti, vi, pi].map(|i| format!("{:?}", *town::layout(&ex.world, &ex.t0, i)));
    let (tl, vl, pl) = (town::layout(&ex.world, &ex.t0, ti), town::layout(&ex.world, &ex.t0, vi), town::layout(&ex.world, &ex.t0, pi));
    let street = tl.roads[rk].clone();
    let deck = tl.bridges[dk].clone();
    let lane = vl.streets[lk].clone();
    let park = pl.parks[pk].clone();
    let lay = bm::park_layout(&park, bm::land_biome(&ex.t0, geom::centroid(&park.pts)), pl.tier >= Tier::City, seed);
    let path = lay.paths[pj].clone();
    let keys = [key_at(fine, at), key_at(fine, geom::centroid(&deck)), key_at(fine, mid(&lane)), key_at(fine, mid(&path.0))];
    let digest = |c: &Chunk| format!("{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}", c.height, c.surface, c.urban, c.road_halo, c.structures, c.objects, c.building_polys);
    let levels: Vec<TileKey> = (0..=fine).map(|lv| key_at(lv, at)).collect();
    let base_layouts = layouts(&ex);
    let base_chunks: Vec<Chunk> = keys.iter().map(|&k| ex.battlemap(k)).collect();
    let base_packed: Vec<Vec<u8>> = levels.iter().map(|&k| ex.terrain_packed(k)).collect();

    // Everything removed at once.
    let mut removed = base_edits.clone();
    removed.removed_roads.insert(rd.clone());
    for (li, k) in [(ti, format!("{sid:08x}")), (vi, format!("{lid:08x}")), (pi, format!("{spot}:{pj}"))] {
        let s = &ex.t0.settlements[li];
        removed.towns.entry(wards::key(li)).or_insert_with(|| TownEdit { at: [s.x, s.y], ..Default::default() }).removed.insert(k);
    }
    ex.world.file.edits = removed.clone();
    town::forget_from(0);
    assert_eq!(layouts(&ex), base_layouts, "a removal changed a layout");
    // (The terrain is laid out as before: a tile made again under the removal is the one cached.)
    let fk = keys[0];
    let parent = ex.terrain(fk.parent().unwrap());
    let again = worldgen::lod::terrain_refine::terrain_tile(&ex.world, &ex.t0, &fk, Some(&parent.padded));
    assert!(again.padded == ex.terrain(fk).padded, "a removal changed the terrain");
    let chunks: Vec<Chunk> = keys.iter().map(|&k| ex.battlemap(k)).collect();
    let centre = |c: &Chunk, i: i64, j: i64| {
        let (ox, oy) = g.tile_origin_ft(&c.key);
        [ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT]
    };
    let world_pt = |c: &Chunk, p: [f32; 2]| {
        let (ox, oy) = g.tile_origin_ft(&c.key);
        [ox + p[0] as f64 * SQUARE_FT, oy + p[1] as f64 * SQUARE_FT]
    };
    // The stretch: no road square on it (clear of every other road), its crossing gone.
    {
        let tile = ex.terrain(keys[0]);
        let (gone, other): (Vec<&roads::RoadPiece>, Vec<&roads::RoadPiece>) = tile.roads.iter().partition(|r| r.road as usize == ri);
        let pts = |r: &roads::RoadPiece| r.pts.iter().map(|c| c.p).collect::<Vec<P>>();
        let (mut on, mut was) = (0, 0);
        for j in -1..=SQ as i64 {
            for i in -1..=SQ as i64 {
                let p = centre(&chunks[0], i, j);
                let h = (j + 1) as usize * HS + (i + 1) as usize;
                if gone.iter().any(|r| line_d(p, &pts(r)) <= 0.5 * r.class.width_ft() - 1.0) && other.iter().all(|r| line_d(p, &pts(r)) > 0.5 * r.class.width_ft() + 10.0) {
                    on += 1;
                    was += (base_chunks[0].road_halo[h] != 0) as usize;
                    assert_eq!(chunks[0].road_halo[h], 0, "{rd}: still a road at {p:?}");
                }
            }
        }
        assert!(on > 0 && was > 0, "{rd}: no road square to remove ({on}, {was})");
        let crossing = |c: &Chunk| c.structures.iter().any(|s| matches!(s.kind, ShapeKind::RoadBridge | ShapeKind::Ford) && s.pts.len() >= 2 && geom_dist(geom::lerp(world_pt(c, s.pts[0]), world_pt(c, s.pts[1]), 0.5), at) < 40.0);
        assert!(crossing(&base_chunks[0]), "{rd}: no bridge or ford at {at:?}");
        assert!(!crossing(&chunks[0]), "{rd}: its bridge or ford stays");
    }
    // The town's street and its deck.
    {
        let c = &chunks[1];
        let dc = geom::centroid(&deck);
        let others: Vec<&(Vec<P>, u8, f64)> = tl.roads.iter().enumerate().filter(|(k, _)| *k != rk).map(|(_, r)| r).collect();
        let (mut on, mut was, mut under) = (0, 0, 0);
        for j in 0..SQ as i64 {
            for i in 0..SQ as i64 {
                let (p, k) = (centre(c, i, j), j as usize * SQ + i as usize);
                let in_other_deck = tl.bridges.iter().chain(&tl.piers).enumerate().any(|(b, d)| b != dk && geom::contains(d, p));
                if geom::contains(&deck, p) && !in_other_deck {
                    under += 1;
                    assert_ne!(c.urban[k], URBAN_DECK, "the deck under the removed street stays at {p:?}");
                }
                if line_d(p, &street.0) <= 0.5 * street.2 - 2.5 && others.iter().all(|r| line_d(p, &r.0) > 0.5 * r.2 + 5.0) && !in_other_deck {
                    on += 1;
                    was += matches!(base_chunks[1].urban[k], URBAN_STREET | URBAN_DECK) as usize;
                    assert!(!matches!(c.urban[k], URBAN_STREET | URBAN_DECK), "the removed street is still drawn at {p:?}");
                }
            }
        }
        assert!(on > 0 && was > 0 && under > 0, "no street or deck square to remove ({on}, {was}, {under})");
        let decked = |c: &Chunk| c.structures.iter().any(|s| s.kind == ShapeKind::Deck && geom_dist(geom::centroid(&s.pts.iter().map(|&q| world_pt(c, q)).collect::<Vec<P>>()), dc) < 2.0);
        assert!(decked(&base_chunks[1]) && !decked(c), "the removed street's bridge deck stays");
    }
    // The village's lane.
    {
        let c = &chunks[2];
        let (mut on, mut was) = (0, 0);
        for j in 0..SQ as i64 {
            for i in 0..SQ as i64 {
                let (p, k) = (centre(c, i, j), j as usize * SQ + i as usize);
                let clear = vl.roads.iter().all(|r| line_d(p, &r.0) > 0.5 * r.2 + 5.0) && vl.streets.iter().enumerate().all(|(o, s)| o == lk || line_d(p, s) > 10.0);
                if line_d(p, &lane) < 4.0 && clear {
                    on += 1;
                    was += (base_chunks[2].urban[k] == URBAN_STREET) as usize;
                    assert_ne!(c.urban[k], URBAN_STREET, "the removed lane is still drawn at {p:?}");
                }
            }
        }
        assert!(on > 0 && was > 0, "no lane square to remove ({on}, {was})");
    }
    // The common's path: lawn over it.
    {
        let c = &chunks[3];
        let (mut on, mut was) = (0, 0);
        for j in 0..SQ as i64 {
            for i in 0..SQ as i64 {
                let (p, k) = (centre(c, i, j), j as usize * SQ + i as usize);
                let clear = lay.paths.iter().enumerate().all(|(o, q)| o == pj || line_d(p, &q.0) > 0.5 * q.1 + 3.5) && lay.rounds.iter().all(|&(rc, r)| geom_dist(p, rc) > r + 3.5);
                if geom::contains(&park.pts, p) && line_d(p, &path.0) < 0.5 * path.1 && clear {
                    on += 1;
                    was += (base_chunks[3].urban[k] == URBAN_PATH) as usize;
                    assert_ne!(c.urban[k], URBAN_PATH, "the removed path is still drawn at {p:?}");
                }
            }
        }
        assert!(on > 0 && was > 0, "no path square to remove ({on}, {was})");
    }
    // No packed line at any level on the stretch (away from every other road).
    let packed: Vec<Vec<u8>> = levels.iter().map(|&k| ex.terrain_packed(k)).collect();
    let lines_of = |b: &[u8], key: &TileKey| -> Vec<Vec<P>> {
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
        let mut o = 48 + u(32) * 4 + u(36) * 2 + u(40) + u(44);
        let nl = u(o);
        o += 8;
        let starts: Vec<usize> = (0..=nl).map(|k| u(o + 4 * k)).collect();
        o += 4 * (nl + 1);
        let f = |v: usize, a: usize| f32::from_le_bytes(b[o + 16 * v + 4 * a..o + 16 * v + 4 * a + 4].try_into().unwrap()) as f64;
        let ((ox, oy), size) = (g.tile_origin_ft(key), g.tile_size_ft(key.level));
        (0..nl).map(|k| (starts[k]..starts[k + 1]).map(|v| [ox + f(v, 0) * size, oy + f(v, 1) * size]).collect()).collect()
    };
    let mut checked = 0;
    for (lv, key) in levels.iter().enumerate() {
        let tile = ex.terrain(*key);
        let ((ox, oy), size) = (g.tile_origin_ft(key), g.tile_size_ft(key.level));
        let others: Vec<Vec<P>> = tile.roads.iter().filter(|r| r.road as usize != ri).map(|r| r.pts.iter().map(|c| c.p).collect()).collect();
        let on: Vec<P> = tile.roads.iter().filter(|r| r.road as usize == ri).flat_map(|r| r.pts.iter().map(|c| c.p)).filter(|p| p[0] > ox && p[0] < ox + size && p[1] > oy && p[1] < oy + size && others.iter().all(|o| line_d(*p, o) > 300.0)).collect();
        if on.is_empty() {
            continue;
        }
        let near = |b: &[u8], d: f64| lines_of(b, key).iter().any(|l| on.iter().any(|&p| line_d(p, l) <= d));
        assert!(near(&base_packed[lv], 1.0), "level {lv}: the stretch is not among the tile's lines as generated");
        assert!(!near(&packed[lv], 5.0), "level {lv}: a line still runs on the removed stretch {rd}");
        checked += 1;
    }
    assert!(checked >= 3, "the stretch was checked at {checked} levels only");
    // The same asked the other way round, after the layouts are forgotten.
    town::forget_from(0);
    let back: Vec<String> = keys.iter().rev().map(|&k| digest(&ex.battlemap(k))).collect();
    assert!(back.into_iter().rev().eq(chunks.iter().map(digest)), "the removals depend on what was asked first");
    // Brought back, all is as generated.
    ex.world.file.edits = base_edits;
    town::forget_from(0);
    assert!(keys.iter().map(|&k| digest(&ex.battlemap(k))).eq(base_chunks.iter().map(digest)), "brought back, a battlemap is not as generated");
    assert!(levels.iter().map(|&k| ex.terrain_packed(k)).eq(base_packed), "brought back, a tile is not as generated");
    assert_eq!(layouts(&ex), base_layouts, "brought back, a layout is not as generated");
}

#[test]
fn fence_edits() {
    use worldgen::battlemap::{self as bm, Chunk, Kind, SQ, SQUARE_FT, ShapeKind};
    use worldgen::model::fences::{Fence, FenceChange, FenceGap, FenceKind};
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, ParkKind, fence, geom, places::Spot, wards};
    use worldgen::world::{BuildingEdit, TownEdit};
    type P = [f64; 2];
    let file: WorldFile = serde_json::from_str(&world_json(1)).unwrap();
    let mut ex = Executor::new(World::new(file).unwrap());
    let g = ex.world.geom.clone();
    let base_edits = ex.world.file.edits.clone();
    let seed = ex.world.stream("battlemap");
    let fine = g.max_level;
    let fsize = g.tile_size_ft(fine);
    let key_at = |p: P| TileKey::surface(fine, (p[0] / fsize) as u32, (p[1] / fsize) as u32);
    let inner = |p: P, m: f64| (0..2).all(|a| (p[a] - (p[a] / fsize).floor() * fsize).min(((p[a] / fsize).floor() + 1.0) * fsize - p[a]) > m);
    let to_world = |c: &Chunk, q: [f32; 2]| {
        let (ox, oy) = g.tile_origin_ft(&c.key);
        [ox + q[0] as f64 * SQUARE_FT, oy + q[1] as f64 * SQUARE_FT]
    };
    // (A run's shape within `d` ft of `p`.)
    let shape_near = |c: &Chunk, p: P, d: f64| {
        c.structures.iter().any(|s| matches!(s.kind, ShapeKind::Hedge | ShapeKind::DryWall | ShapeKind::Fence) && s.pts.len() >= 2 && geom::seg_dist(p, to_world(c, s.pts[0]), to_world(c, s.pts[1])) < d)
    };
    let objects_near = |c: &Chunk, kind: Kind, p: P, d: f64| c.objects.iter().filter(|o| o.kind == kind && geom_dist(to_world(c, [o.x, o.y]), p) < d).count();
    let digest = |c: &Chunk| format!("{:?}|{:?}|{:?}|{:?}|{:?}", c.height, c.urban, c.structures, c.objects, c.edges);
    let n = ex.t0.settlements.len();
    // A village's croft hedges (one to go, one to become a picket fence with a gate) and a field's
    // side (a gap cut over it), each drawn on its battlemap as generated.
    let villages: Vec<usize> = (0..n).filter(|&i| ex.t0.settlements[i].tier == Tier::Village).collect();
    let mut found = None;
    'v: for &vi in &villages {
        let l = town::layout(&ex.world, &ex.t0, vi);
        let srcs = fence::sources(&l, &ex.t0, fence::EVERYWHERE);
        let long = |s: &(u8, P, P, FenceKind), tag: u8, d: f64| s.0 == tag && geom_dist(s.1, s.2) >= d && inner(geom::lerp(s.1, s.2, 0.5), 60.0);
        // (Away from rivers: a bank's cliff squares are hard going with or without the hedge.)
        let dry = |s: &(u8, P, P, FenceKind)| {
            [0.0, 0.5, 1.0].iter().all(|&t| {
                let p = geom::lerp(s.1, s.2, t);
                worldgen::lod::rivers::surface_near(&ex.t0.rivers, p[0], p[1], 200.0, ex.t0.cell_ft).is_none()
            })
        };
        let hedges: Vec<(u8, P, P, FenceKind)> = srcs.iter().copied().filter(|s| long(s, fence::CROFT, 30.0) && s.3 == FenceKind::Hedgerow && dry(s)).collect();
        let sides: Vec<(u8, P, P, FenceKind)> = srcs.iter().copied().filter(|s| long(s, fence::FIELD, 40.0)).collect();
        let drawn = |ex: &mut Executor, s: &(u8, P, P, FenceKind)| {
            let m = geom::lerp(s.1, s.2, 0.5);
            let c = ex.battlemap(key_at(m));
            shape_near(&c, m, 1.0) && shape_near(&c, geom::lerp(s.1, s.2, 0.3), 1.0) && shape_near(&c, geom::lerp(s.1, s.2, 0.7), 1.0)
        };
        let mut picked: Vec<(u8, P, P, FenceKind)> = Vec::new();
        for h in &hedges {
            let far = picked.iter().all(|p| geom_dist(geom::lerp(p.1, p.2, 0.5), geom::lerp(h.1, h.2, 0.5)) > 60.0 && fence::run_key(p.0, p.1, p.2) != fence::run_key(h.0, h.1, h.2));
            if far && drawn(&mut ex, h) {
                picked.push(*h);
                if picked.len() == 2 {
                    break;
                }
            }
        }
        if picked.len() < 2 {
            continue;
        }
        for s in &sides {
            if drawn(&mut ex, s) {
                found = Some((vi, picked[0], picked[1], *s, srcs.clone()));
                break 'v;
            }
        }
    }
    let (vi, gone, picket, side, srcs) = found.expect("a village with croft hedges and a field's side drawn");
    let gone_key = fence::run_key(gone.0, gone.1, gone.2);
    let picket_key = fence::run_key(picket.0, picket.1, picket.2);
    assert_eq!(fence::parse(&fence::fr_id(vi, gone_key)), Some((vi, format!("{gone_key:08x}"))), "a run's id does not read back");
    // A park's clipped hedge, its pieces on the battlemap.
    let (pi, spot, pj, pieces) = {
        let (world, t0) = (&ex.world, &ex.t0);
        (0..n)
            .filter(|&i| t0.settlements[i].tier >= Tier::Town)
            .find_map(|i| {
                let l = town::layout(world, t0, i);
                l.parks.iter().enumerate().filter(|(_, p)| p.kind == ParkKind::Formal && p.spot != Spot::None).find_map(|(k, pk)| {
                    let lay = bm::park_layout(pk, bm::land_biome(t0, geom::centroid(&pk.pts)), l.tier >= Tier::City, seed);
                    let runs = worldgen::park::hedge_runs(&lay);
                    runs.iter().enumerate().find_map(|(j, r)| {
                        let pts: Vec<P> = r.iter().map(|&q| lay.props[q].0).collect();
                        let one = pts.iter().all(|&p| inner(p, 10.0) && key_at(p) == key_at(pts[0]));
                        (lay.props[r[0]].2 == Kind::Hedge && r.len() >= 4 && one).then(|| (i, town::cut::park_spot(&l, k), j, pts))
                    })
                })
            })
            .expect("a formal garden's hedge")
    };
    // A dry spot for the fences drawn by hand: one of each kind laid in sections, a gate in each.
    let vc = { let s = &ex.t0.settlements[vi]; [s.x, s.y] };
    let spot_key = key_at([vc[0] + fsize, vc[1]]);
    let drawn_at = {
        let c = ex.battlemap(spot_key);
        let (ox, oy) = g.tile_origin_ft(&spot_key);
        let dry = |i: usize, j: usize| (j..j + 12).all(|y| (i..i + 12).all(|x| c.water_level[y * SQ + x] <= c.height[y * SQ + x]));
        let (i, j) = (4..SQ - 16).flat_map(|j| (4..SQ - 16).map(move |i| (i, j))).find(|&(i, j)| dry(i, j)).expect("a dry spot");
        [ox + i as f64 * SQUARE_FT + 5.0, oy + j as f64 * SQUARE_FT + 5.0]
    };
    // (`None`: a low stone wall drawn as a dry-stone wall, a high wall raised as a garden wall.)
    let kinds = [(FenceKind::Picket, Some(Kind::Picket)), (FenceKind::IronRailing, Some(Kind::IronRailing)), (FenceKind::Stockade, Some(Kind::Stockade)), (FenceKind::LowWall, None), (FenceKind::HighWall, None)];
    let line = |k: usize| (geom::add(drawn_at, [0.0, 10.0 * k as f64]), geom::add(drawn_at, [40.0, 10.0 * k as f64]));
    let keys: Vec<TileKey> = {
        let mut k = vec![key_at(geom::lerp(gone.1, gone.2, 0.5)), key_at(geom::lerp(picket.1, picket.2, 0.5)), key_at(geom::lerp(side.1, side.2, 0.5)), key_at(pieces[0]), spot_key];
        k.dedup();
        k
    };
    let layouts = |ex: &Executor| [vi, pi].map(|i| format!("{:?}", *town::layout(&ex.world, &ex.t0, i)));
    let base_layouts = layouts(&ex);
    let base_chunks: Vec<Chunk> = keys.iter().map(|&k| ex.battlemap(k)).collect();
    let base_padded = ex.terrain(keys[0]).padded.clone();
    let idx = |p: P| keys.iter().position(|&k| k == key_at(p)).unwrap();

    // Every edit at once.
    let mut e = base_edits.clone();
    let (sm, sn) = (geom::lerp(side.1, side.2, 0.5), { let d = geom::sub(side.2, side.1); let l = geom_dist(side.1, side.2); [-d[1] / l, d[0] / l] });
    e.fence_gaps.insert("fg:0".into(), FenceGap { pts: vec![geom::sub(sm, geom::mul(sn, 8.0)), geom::add(sm, geom::mul(sn, 8.0))], r: 4.0 });
    let pl = geom_dist(picket.1, picket.2);
    for (li, k, ch) in [
        (vi, format!("{gone_key:08x}"), FenceChange { gone: true, ..Default::default() }),
        (vi, format!("{picket_key:08x}"), FenceChange { kind: Some(FenceKind::Picket), gates: vec![(0.5 * pl) as f32], ..Default::default() }),
        (pi, format!("{spot}:{pj}"), FenceChange { gone: true, ..Default::default() }),
    ] {
        let s = &ex.t0.settlements[li];
        e.towns.entry(wards::key(li)).or_insert_with(|| TownEdit { at: [s.x, s.y], ..Default::default() }).fences.insert(k, ch);
    }
    for (k, &(fk, _)) in kinds.iter().enumerate() {
        let (a, b) = line(k);
        e.fences.insert(format!("f:{k}"), Fence { kind: fk, pts: vec![a, b], closed: false, gates: vec![20.0] });
    }
    ex.world.file.edits = e.clone();
    town::forget_from(0);
    assert_eq!(layouts(&ex), base_layouts, "a fence edit changed a layout");
    town::forget_from(0);
    let fk = keys[0];
    let parent = ex.terrain(fk.parent().unwrap());
    let again = worldgen::lod::terrain_refine::terrain_tile(&ex.world, &ex.t0, &fk, Some(&parent.padded));
    assert!(again.padded == base_padded, "a fence edit changed the terrain");
    let chunks: Vec<Chunk> = keys.iter().map(|&k| ex.battlemap(k)).collect();
    // The run gone: no shape and no hard going along it (away from its ends and every other run).
    let others: Vec<(P, P)> = srcs.iter().filter(|s| fence::run_key(s.0, s.1, s.2) != gone_key).map(|s| (s.1, s.2)).collect();
    let check_gone = |chunks: &[Chunk], what: &str| {
        let c = &chunks[idx(geom::lerp(gone.1, gone.2, 0.5))];
        let (ox, oy) = g.tile_origin_ft(&c.key);
        let gl = geom_dist(gone.1, gone.2);
        let mut on = 0;
        for t in [0.2, 0.35, 0.5, 0.65, 0.8] {
            assert!(!shape_near(&c, geom::lerp(gone.1, gone.2, t), 1.0) || others.iter().any(|&(a, b)| geom::seg_dist(geom::lerp(gone.1, gone.2, t), a, b) < 5.0), "{what}: the gone run is still drawn");
        }
        for j in 0..SQ {
            for i in 0..SQ {
                let p = [ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT];
                let s = geom::dot(geom::sub(p, gone.1), geom::sub(gone.2, gone.1)) / gl;
                if geom::seg_dist(p, gone.1, gone.2) < 1.0 && s > 5.0 && s < gl - 5.0 && others.iter().all(|&(a, b)| geom::seg_dist(p, a, b) > 5.0) {
                    on += 1;
                    assert_eq!(c.edges[j * SQ + i] & 16, 0, "{what}: the gone run's square at {p:?} is still hard going");
                }
            }
        }
        assert!(on > 0, "{what}: no square of the gone run to check");
    };
    check_gone(&chunks, "removed");
    // The run made a picket fence, a gate in its middle.
    {
        let c = &chunks[idx(geom::lerp(picket.1, picket.2, 0.5))];
        let along = c.objects.iter().filter(|o| o.kind == Kind::Picket && geom::seg_dist(to_world(&c, [o.x, o.y]), picket.1, picket.2) < 0.5).count();
        assert!(along >= 3, "the run changed to a picket fence has {along} sections");
        let (s, e2) = fence::canonical(picket.1, picket.2);
        let gp = geom::lerp(s, e2, 0.5);
        // (A gate's middle moves to its square's, 3.5 ft at most.)
        assert_eq!(objects_near(&c, Kind::Gate, gp, 3.6), 1, "no gate in the changed run");
        let leaf = c.objects.iter().find(|o| o.kind == Kind::Gate).map(|o| to_world(&c, [o.x, o.y])).unwrap();
        assert_eq!(objects_near(&c, Kind::Picket, leaf, 2.4), 0, "a picket section in the gate");
    }
    // The gap over the field's side.
    {
        let (b, c) = (&base_chunks[idx(sm)], &chunks[idx(sm)]);
        assert!(shape_near(&b, sm, 1.0) && !shape_near(&c, sm, 2.0), "the gap did not cut the field's side");
        assert!(shape_near(&c, geom::lerp(side.1, side.2, 0.2), 1.0) || !shape_near(&b, geom::lerp(side.1, side.2, 0.2), 1.0), "the gap cut more than it covers");
    }
    // The park's hedge gone, piece by piece.
    {
        let (b, c) = (&base_chunks[idx(pieces[0])], &chunks[idx(pieces[0])]);
        let was: usize = pieces.iter().map(|&p| objects_near(&b, Kind::Hedge, p, 0.3)).sum();
        let now: usize = pieces.iter().map(|&p| objects_near(&c, Kind::Hedge, p, 0.3)).sum();
        assert!(was >= 2 && now == 0, "the park's hedge: {was} pieces before, {now} after");
    }
    // The fences drawn by hand: sections along each (a low wall's hard going, a high wall's raised
    // squares), a stockade's blocking the way and sight, each gate's square free.
    {
        let c = &chunks[idx(drawn_at)];
        let (ox, oy) = g.tile_origin_ft(&c.key);
        let sq = |p: P| (((p[1] - oy) / SQUARE_FT).floor() as usize) * SQ + ((p[0] - ox) / SQUARE_FT).floor() as usize;
        for (k, &(fk, kind)) in kinds.iter().enumerate() {
            let (a, b) = line(k);
            let gp = geom::add(a, [20.0, 0.0]);
            match (fk, kind) {
                (_, Some(kind)) => {
                    let along = c.objects.iter().filter(|o| o.kind == kind && geom::seg_dist(to_world(&c, [o.x, o.y]), a, b) < 0.3).count();
                    assert!(along >= 6, "{kind:?}: {along} sections drawn");
                }
                (FenceKind::LowWall, None) => {
                    let p = geom::add(a, [7.5, 0.0]);
                    assert!(shape_near(&c, p, 1.0) && c.edges[sq(p)] & 16 != 0, "the low wall is not drawn or not hard going");
                    let q = c.objects.iter().filter(|o| o.kind == Kind::Gate).map(|o| to_world(&c, [o.x, o.y])).find(|&q| geom_dist(q, gp) < 3.6).unwrap();
                    assert!(!shape_near(&c, q, 2.0), "the low wall runs through its gate");
                }
                _ => {
                    let p = geom::add(a, [7.5, 0.0]);
                    // (One of the squares either side of the gate's middle is its way through.)
                    let free = [-2.4, 2.4].iter().any(|&dx| c.urban[sq(geom::add(gp, [dx, 0.0]))] != bm::URBAN_WALL) || c.urban[sq(gp)] != bm::URBAN_WALL;
                    assert!(c.urban[sq(p)] == bm::URBAN_WALL && free, "the high wall is not raised, or raised in its gate");
                    assert!(c.structures.iter().any(|s| s.kind == ShapeKind::GardenWall && s.pts.len() >= 2 && geom::seg_dist(p, to_world(c, s.pts[0]), to_world(c, s.pts[1])) < 1.0), "the high wall is not drawn");
                }
            }
            assert_eq!(objects_near(&c, Kind::Gate, gp, 3.6), 1, "{fk:?}: no gate");
            // A gate is open passage for every kind: the square it stands in reads free (no object's
            // footprint, as the app's flags lay them (`LONG_FOOTPRINT`), no hard going, no raised wall).
            let leaf = c.objects.iter().filter(|o| o.kind == Kind::Gate).map(|o| to_world(&c, [o.x, o.y])).find(|&q| geom_dist(q, gp) < 3.6).unwrap();
            let k = sq(leaf);
            let (gi, gj) = ((k % SQ) as i64, (k / SQ) as i64);
            for o in c.objects.iter().filter(|o| o.kind != Kind::Gate) {
                let inf = bm::info(o.kind as u16);
                let hw = match o.kind {
                    Kind::Picket | Kind::IronRailing => 0.1,
                    Kind::Stockade => 0.22,
                    Kind::WattleFence => 0.12,
                    Kind::Hedge => 0.29,
                    _ => continue,
                };
                if !(inf.blocks_move || inf.difficult) {
                    continue;
                }
                let (cs, sn) = ((o.rot as f64).cos(), (o.rot as f64).sin());
                let hl = 0.49 * o.scale as f64;
                let mut u = -hl;
                while u <= hl + 1e-6 {
                    let mut w = -hw;
                    while w <= hw + 1e-6 {
                        let (x, y) = (o.x as f64 + cs * u - sn * w, o.y as f64 + sn * u + cs * w);
                        assert!(!(x.floor() as i64 == gi && y.floor() as i64 == gj), "{fk:?}: the gate's square is blocked by a {:?}", o.kind);
                        w += 0.1_f64.min(hw);
                    }
                    u += 0.1_f64.min(hl);
                }
            }
            // (Unless a tree or a bush the countryside put by it is what is hard going.)
            let own = !c.objects.iter().any(|o| !fence::is_fence_object(o.kind) && (o.x as f64 - gi as f64 - 0.5).abs() <= 2.0 && (o.y as f64 - gj as f64 - 0.5).abs() <= 2.0);
            assert!(!own || c.edges[k] & 16 == 0, "{fk:?}: the gate's square is hard going");
            assert_ne!(c.urban[k], bm::URBAN_WALL, "{fk:?}: the gate's square is raised");
            if kind == Some(Kind::Stockade) {
                let p = geom::add(a, [7.5, 0.0]);
                assert!(c.objects.iter().any(|o| { let i = bm::info(o.kind as u16); i.blocks_move && i.blocks_sight && geom_dist(to_world(&c, [o.x, o.y]), p) < 2.6 }), "the stockade does not block");
            }
        }
    }
    // The same asked the other way round, after the layouts are forgotten.
    town::forget_from(0);
    let back: Vec<String> = keys.iter().rev().map(|&k| digest(&ex.battlemap(k))).collect();
    assert!(back.into_iter().rev().eq(chunks.iter().map(digest)), "the fence edits depend on what was asked first");
    // An unrelated edit (a building of the village's taken away) keeps every run's id; the removal
    // still applies.
    {
        let l = town::layout(&ex.world, &ex.t0, vi);
        let b0 = &l.buildings[0];
        let mut e2 = e.clone();
        e2.buildings.insert(format!("b:{vi}:{}", b0.id), BuildingEdit { at: geom::centroid(&b0.poly), removed: true, ..Default::default() });
        ex.world.file.edits = e2;
        town::forget_from(0);
        let l2 = town::layout(&ex.world, &ex.t0, vi);
        assert_ne!(l2.buildings.len(), l.buildings.len(), "the building was not taken away");
        let ids = |l: &town::Layout| fence::sources(l, &ex.t0, fence::EVERYWHERE).iter().map(|s| fence::run_key(s.0, s.1, s.2)).collect::<Vec<u32>>();
        assert_eq!(ids(&l2), ids(&l), "an unrelated edit changed the runs' ids");
        let chunks2: Vec<Chunk> = keys.iter().map(|&k| ex.battlemap(k)).collect();
        check_gone(&chunks2, "after an unrelated edit");
    }
    // Brought back, all is as generated.
    ex.world.file.edits = base_edits;
    town::forget_from(0);
    assert!(keys.iter().map(|&k| digest(&ex.battlemap(k))).eq(base_chunks.iter().map(digest)), "brought back, a battlemap is not as generated");
    assert_eq!(layouts(&ex), base_layouts, "brought back, a layout is not as generated");
}
