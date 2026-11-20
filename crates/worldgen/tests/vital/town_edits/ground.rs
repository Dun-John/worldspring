//! Ground shaped by hand.

use super::*;

/// Ground shaped by hand (`Edits.ground`, `Edits.plateaus`) at the largest settlement: a sheer
/// plateau across a chunk corner and a mound brushed across a chunk edge. The four chunks agree at
/// their seams bit for bit; the plateau's top is flat at its level on the drawn ground (leaving
/// out roads, buildings, plaza features, walls and towers); lowering a dry shore (by brush and by
/// a pit) leaves every dry square dry; terrain samples never see the edits; the edits survive a
/// round trip as JSON and as edit ops; clearing them gives the chunks back byte for byte.
pub(in super::super) fn ground_edits(base: &World) {
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
    // P6: a pit 30 ft down on open dry ground has a floor of bare rock (`ground::CHASM_FT`);
    // `floor: ground` keeps the land's own there.
    let open_sq = |c: &Chunk, k: usize| c.building[k] == 0 && c.urban[k] == 0 && c.water_level[k] <= c.height[k] && c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] == 0;
    let (key, mid) = settlements
        .iter()
        .flat_map(|s| [(0.0, 0.0), (600.0, 0.0), (0.0, 600.0), (-600.0, 0.0), (0.0, -600.0)].map(|d| TileKey::surface(lvl, ((s.x + d.0) / size) as u32, ((s.y + d.1) / size) as u32)))
        .find_map(|k| {
            let c = ex.battlemap(k);
            (12..SQ - 12).flat_map(|j| (12..SQ - 12).map(move |i| (i, j))).find(|&(i, j)| (0..81).all(|d| open_sq(&c, (j + d / 9 - 4) * SQ + i + d % 9 - 4))).map(|(i, j)| (k, j * SQ + i))
        })
        .expect("open dry ground for a pit");
    let c = ex.battlemap(key);
    let m = centre(&key, mid % SQ, mid / SQ);
    let hole = [[-25.0, -25.0], [25.0, -25.0], [25.0, 25.0], [-25.0, 25.0]].map(|d| [m[0] + d[0], m[1] + d[1]]);
    let mut e = Edits::default();
    e.plateaus.insert("h:chasm".into(), ground::plateau_for(&hole, c.height[mid] as f64 - 30.0, 10.0).unwrap());
    ex.world.file.edits = e;
    let dug = ex.battlemap(key);
    let floor_sq: Vec<usize> = (0..SQ * SQ).filter(|&k| { let q = centre(&key, k % SQ, k / SQ); (q[0] - m[0]).abs() < 20.0 && (q[1] - m[1]).abs() < 20.0 } && open_sq(&c, k)).collect();
    let rock = |c: &Chunk| floor_sq.iter().filter(|&&k| c.surface[k] == battlemap::Surface::Rock).count();
    assert!(floor_sq.len() >= 40 && rock(&dug) == floor_sq.len(), "the pit's floor is rock on {} of {} open squares", rock(&dug), floor_sq.len());
    assert!(floor_sq.iter().all(|&k| dug.surface[k] != battlemap::Surface::Snow), "snow on the pit's rock floor");
    assert!(dug.objects.iter().all(|o| !(floor_sq.contains(&((o.y.max(0.0) as usize).min(SQ - 1) * SQ + (o.x.max(0.0) as usize).min(SQ - 1))) && matches!(o.kind, battlemap::Kind::TreeDeciduous | battlemap::Kind::Bush | battlemap::Kind::Flowers))), "a plant on the pit's rock floor");
    ex.world.file.edits.plateaus.get_mut("h:chasm").unwrap().floor = Some(ground::Floor::Ground);
    let lawn = ex.battlemap(key);
    assert!(rock(&lawn) <= rock(&c), "a pit with floor: ground draws rock on {} squares (the ground there {})", rock(&lawn), rock(&c));
    ex.world.file.edits = Edits::default();

    // P6: plateaus stack whatever their sizes and ids. A base, a terrace 15 ft above it in two
    // pieces of one level, and a lower piece inside the terrace (smallest, first by id): the
    // terrace is flat at its level across its joint and the lower piece, the base flat round it;
    // renamed, or read over a smaller rectangle, the same bit for bit.
    {
        let at = |x0: f64, y0: f64, x1: f64, y1: f64| [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|d| [corner[0] + d[0], corner[1] + d[1]]);
        let lv = level as f64;
        let stack = |ids: [&str; 4]| {
            let mut e = Edits::default();
            e.plateaus.insert(ids[0].into(), ground::plateau_for(&at(-60.0, -60.0, 60.0, 60.0), lv, 0.0).unwrap());
            e.plateaus.insert(ids[1].into(), ground::plateau_for(&at(-30.0, -30.0, 0.0, 30.0), lv + 15.0, 10.0).unwrap());
            e.plateaus.insert(ids[2].into(), ground::plateau_for(&at(0.0, -30.0, 30.0, 30.0), lv + 15.0, 10.0).unwrap());
            e.plateaus.insert(ids[3].into(), ground::plateau_for(&at(-10.0, -10.0, 10.0, 10.0), lv + 5.0, 0.0).unwrap());
            e
        };
        let (e, renamed) = (stack(["h:s0", "h:s1", "h:s2", "h:a"]), stack(["h:z0", "h:b1", "h:y2", "h:x"]));
        let sq0 = |v: f64| (v / SQUARE_FT).floor() as i64;
        let (gx0, gy0) = (sq0(corner[0] - 70.0), sq0(corner[1] - 70.0));
        let wide = [corner[0] - 80.0, corner[1] - 80.0, corner[0] + 80.0, corner[1] + 80.0];
        let (sh, sh2) = (ground::Shaper::build(&e, None, wide, SQUARE_FT).unwrap(), ground::Shaper::build(&renamed, None, wide, SQUARE_FT).unwrap());
        let (mut terrace, mut base) = (0, 0);
        for gy in gy0..gy0 + 28 {
            for gx in gx0..gx0 + 28 {
                let q = [(gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT];
                let (dx, dy) = ((q[0] - corner[0]).abs(), (q[1] - corner[1]).abs());
                let (gr, hw) = (ex.t0.sample(q[0], q[1], SQUARE_FT) as f32, ex.t0.sample_water(q[0], q[1]));
                let h = sh.at_square(gx, gy, gr, hw).0;
                assert_eq!(h.to_bits(), sh2.at_square(gx, gy, gr, hw).0.to_bits(), "stacked plateaus: square {q:?} changes with their ids");
                let small = ground::Shaper::build(&e, None, [q[0] - 1.0, q[1] - 1.0, q[0] + 1.0, q[1] + 1.0], SQUARE_FT);
                assert_eq!(h.to_bits(), small.map_or(gr, |s| s.at_square(gx, gy, gr, hw).0).to_bits(), "stacked plateaus: square {q:?} differs read over a smaller rectangle");
                if dx < 30.0 && dy < 30.0 {
                    assert!((h as f64 - lv - 15.0).abs() < 0.01, "stacked plateaus: terrace square ({dx}, {dy}) from the middle at {h}, not {}", lv + 15.0);
                    terrace += 1;
                } else if dx < 60.0 && dy < 60.0 && (dx > 45.0 || dy > 45.0) {
                    assert!((h as f64 - lv).abs() < 0.01, "stacked plateaus: base square ({dx}, {dy}) from the middle at {h}, not {lv}");
                    base += 1;
                } else if dx < 40.0 && dy < 40.0 {
                    assert!(h as f64 >= lv - 0.01 && h as f64 <= lv + 15.01, "stacked plateaus: the terrace's edge at ({dx}, {dy}) from the middle at {h}");
                }
            }
        }
        assert!(terrace >= 100 && base >= 100, "too few squares checked ({terrace} on the terrace, {base} on the base)");
    }

    // R5 M20: an islet, a breakwater and a quay built by hand on a shore. The islet's top is dry
    // rock at its level, and a building may stand on it; the breakwater's made ground is dry and
    // paved out over the water; the quay's strip is paved. Terrain samples never see them, seams
    // hold bit for bit, they survive JSON and edit ops, and cleared, the chunks are as generated.
    use worldgen::agent;
    use worldgen::world::{Wharf, WharfKind};
    let wet = |c: &Chunk, k2: usize| c.water_level[k2] > c.height[k2];
    assert!(!serde_json::to_string(&ground::plateau_for(&pit, 10.0, 0.0).unwrap()).unwrap().contains("kind"), "a plateau's file changed");
    // Shores: the first dry square of a row with 80 ft of open water east of it, rows 100 ft apart.
    let (key, shores) = settlements
        .iter()
        .map(|s| TileKey::surface(lvl, (s.x / size) as u32, (s.y / size) as u32))
        .find_map(|k| {
            let c = ex.battlemap(k);
            let mut rows: Vec<usize> = Vec::new();
            for j in 10..SQ - 10 {
                if rows.last().is_some_and(|&r| j < r / SQ + 20) {
                    continue;
                }
                if let Some(k2) = (10..SQ - 30).map(|i| j * SQ + i).find(|&k2| !wet(&c, k2) && c.building[k2] == 0 && (1..=16).all(|d| wet(&c, k2 + d))) {
                    rows.push(k2);
                }
            }
            (rows.len() >= 3).then_some((k, rows))
        })
        .expect("a shore with three rows of open water");
    let plain_c = pack(&mut ex, key);
    let plain_padded = ex.terrain(key).padded.clone();
    let near = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].map(|(dx, dy)| TileKey::surface(lvl, (key.x as i32 + dx) as u32, (key.y as i32 + dy) as u32));
    let plain_near: Vec<Vec<u8>> = near.iter().map(|&k| pack(&mut ex, k).0).collect();
    let fine = |p: [f64; 2]| TileKey::surface(lvl, (p[0] / size) as u32, (p[1] / size) as u32);
    let at = |k2: usize| centre(&key, k2 % SQ, k2 / SQ);
    // The islet: 45 ft out from the first shore that T0 has under water there.
    let (islet_k2, islet_at, islet) = shores
        .iter()
        .find_map(|&k2| {
            let p = [at(k2)[0] + 45.0, at(k2)[1]];
            ground::islet_for("h:isle", p, 18.0, 6.0, &ex.t0).ok().map(|i| (k2, p, i))
        })
        .expect("an islet off the shore");
    assert!(islet.kind == ground::PlateauKind::Islet && (12..=20).contains(&islet.pts.len()), "an islet's outline");
    let foot = |c: [f64; 2]| vec![[c[0] - 6.0, c[1] - 6.0], [c[0] + 6.0, c[1] - 6.0], [c[0] + 6.0, c[1] + 6.0], [c[0] - 6.0, c[1] + 6.0]];
    assert!(agent::building_spot(&ex.world, &ex.t0, &foot(islet_at), None, "c:99", None).is_err(), "a building in the water before the islet");
    let others: Vec<usize> = shores.iter().copied().filter(|&k2| k2 != islet_k2).collect();
    let (b_at, q_at) = (at(others[0]), at(others[1]));
    let breakwater = Wharf { kind: WharfKind::Breakwater, pts: vec![[b_at[0] - 10.0, b_at[1]], [b_at[0] + 60.0, b_at[1]]], width: 15.0, material: Default::default() };
    let quay = Wharf { kind: WharfKind::Quay, pts: vec![[q_at[0] + 2.5, q_at[1] - 30.0], [q_at[0] + 2.5, q_at[1] + 30.0]], width: 15.0, material: Default::default() };
    for w in [&breakwater, &quay] {
        assert!(w.problem(g.map_w_ft, g.map_h_ft).is_none(), "a {}: {:?}", w.kind.name(), w.problem(g.map_w_ft, g.map_h_ft));
    }
    let mut e = Edits::default();
    e.plateaus.insert("h:isle".into(), islet.clone());
    e.wharves.insert("q:bw".into(), breakwater.clone());
    e.wharves.insert("q:quay".into(), quay.clone());
    let json = serde_json::to_string(&e).unwrap();
    assert_eq!(serde_json::from_str::<Edits>(&json).unwrap(), e, "an islet or wharves changed on a JSON round trip");
    let mut by_ops = Edits::default();
    for op in Edits::default().diff(&e) {
        by_ops.apply(&op).unwrap();
    }
    assert_eq!(by_ops, e, "an islet or wharves changed through edit ops");
    ex.world.file.edits = e;
    let built = pack(&mut ex, key);
    assert!(built.0 != plain_c.0, "the shore's chunk is unchanged with an islet and wharves");
    assert_eq!(ex.terrain(key).padded, plain_padded, "terrain samples changed with an islet and wharves");
    let c = &built.1;
    // The islet: dry rock at its level over its middle.
    let mut top = 0;
    for k2 in 0..SQ * SQ {
        let (i, j) = (k2 % SQ, k2 / SQ);
        if geom_dist(centre(&key, i, j), islet_at) > 0.5 * 18.0 * 0.82 {
            continue;
        }
        let h = (j + 1) * HS + i + 1;
        assert!(!wet(c, k2), "islet square ({i}, {j}) of {key:?} is under water");
        assert!((c.halo_height[h] - c.roof_lift[h] - islet.level).abs() <= 0.1, "islet square ({i}, {j}) at {} on an islet at {}", c.halo_height[h] - c.roof_lift[h], islet.level);
        assert!(c.surface[k2] == battlemap::Surface::Rock, "islet square ({i}, {j}) is {:?}, not rock", c.surface[k2]);
        top += 1;
    }
    assert!(top >= 8, "too few squares on the islet's top ({top})");
    agent::building_spot(&ex.world, &ex.t0, &foot(islet_at), None, "c:99", None).expect("a building on the islet");
    // (S3: and a castle drawn on it.)
    let keep = worldgen::world::Created { id: "c:98".into(), kind: "castle".into(), poly: vec![[islet_at[0] - 8.0, islet_at[1] - 8.0], [islet_at[0] + 8.0, islet_at[1] - 8.0], [islet_at[0] + 8.0, islet_at[1] + 8.0], [islet_at[0] - 8.0, islet_at[1] + 8.0]], ..Default::default() };
    if let Err(e) = agent::works_spot(&ex.world, &ex.t0, &keep, "c:98", None) {
        panic!("a castle on the islet refused: {e}");
    }
    // The breakwater: made ground, dry and paved, out over the water.
    let mut made = 0;
    for k2 in 0..SQ * SQ {
        let p = centre(&key, k2 % SQ, k2 / SQ);
        if (p[1] - b_at[1]).abs() > 0.5 * breakwater.width - 3.0 || p[0] < b_at[0] + 10.0 || p[0] > b_at[0] + 55.0 {
            continue;
        }
        assert!(!wet(c, k2), "breakwater square {p:?} is under water");
        assert!(c.urban[k2] == battlemap::URBAN_PLAZA && c.surface[k2] == battlemap::Surface::Cobble, "breakwater square {p:?} is not paved ({:?})", c.surface[k2]);
        made += 1;
    }
    assert!(made >= 8, "too few squares on the breakwater ({made})");
    // The quay: its strip paved, on one side of its line.
    let paved = (0..SQ * SQ)
        .filter(|&k2| {
            let p = centre(&key, k2 % SQ, k2 / SQ);
            (p[1] - q_at[1]).abs() < 27.0 && (p[0] - q_at[0] - 2.5).abs() < quay.width && !wet(c, k2) && c.urban[k2] == battlemap::URBAN_PLAZA && c.surface[k2] == battlemap::Surface::Cobble
        })
        .count();
    assert!(paved >= 20, "too few paved squares on the quay ({paved})");
    // Seams with the four chunks round it.
    for (q, &k) in near.iter().enumerate() {
        let n = ex.battlemap(k);
        match q {
            0 => ring(c, &n, true),
            1 => ring(&n, c, true),
            2 => ring(c, &n, false),
            _ => ring(&n, c, false),
        }
    }
    // The finest tile under the islet draws it, its samples for its children as generated.
    let tk = fine(islet_at);
    let shaped_tile = ex.terrain_packed(tk);
    ex.world.file.edits = Edits::default();
    let plain_tile = ex.terrain_packed(tk);
    let samples = 48..48 + PADDED * PADDED * 4;
    assert!(shaped_tile != plain_tile, "tile {tk:?} doesn't show the islet");
    assert!(shaped_tile[samples.clone()] == plain_tile[samples], "tile {tk:?}'s samples changed with the islet");
    // Cleared: as generated, byte for byte.
    assert!(pack(&mut ex, key).0 == plain_c.0, "the shore's chunk differs once the islet and wharves are cleared");
    for (q, &k) in near.iter().enumerate() {
        assert!(pack(&mut ex, k).0 == plain_near[q], "chunk {k:?} differs once the islet and wharves are cleared");
    }
    assert!(agent::building_spot(&ex.world, &ex.t0, &foot(islet_at), None, "c:99", None).is_err(), "a building in the water once the islet is gone");

    // P6: a plateau 12 ft over the water where the islet stood holds a wall and a building.
    let water = ex.t0.sample_water(islet_at[0], islet_at[1]) as f64;
    let deck = [[-35.0, -20.0], [35.0, -20.0], [35.0, 20.0], [-35.0, 20.0]].map(|d| [islet_at[0] + d[0], islet_at[1] + d[1]]);
    let wall = worldgen::world::Created { id: "c:97".into(), kind: "wall".into(), pts: vec![[islet_at[0] - 15.0, islet_at[1]], [islet_at[0] + 15.0, islet_at[1]]], ..Default::default() };
    let refused = agent::works_spot(&ex.world, &ex.t0, &wall, "c:97", None).err().unwrap_or_default();
    assert!(refused.contains("in the water"), "a wall over the water: {refused:?}");
    ex.world.file.edits.plateaus.insert("h:deck".into(), ground::plateau_for(&deck, water + 12.0, 0.0).unwrap());
    if let Err(e) = agent::works_spot(&ex.world, &ex.t0, &wall, "c:97", None) {
        panic!("a wall on a plateau over the water refused: {e}");
    }
    agent::building_spot(&ex.world, &ex.t0, &foot(islet_at), None, "c:99", None).expect("a building on a plateau over the water");
    ex.world.file.edits = Edits::default();
    // No limit on the shift: a plateau 250 ft over the ground, a pit 300 ft under it and a plateau
    // over sea deeper than 150 ft each stand flat at their own level.
    {
        let (gx0, gy0) = ((corner[0] / SQUARE_FT).floor() as i64, (corner[1] / SQUARE_FT).floor() as i64);
        let area = [corner[0] - 80.0, corner[1] - 80.0, corner[0] + 80.0, corner[1] + 80.0];
        let box_at = [[-half, -half], [half, -half], [half, half], [-half, half]].map(|d| [corner[0] + d[0], corner[1] + d[1]]);
        for (name, gr, rise, pit) in [("plateau", 100.0f32, 250.0, false), ("pit", 1000.0, -300.0, true)] {
            let mut e = Edits::default();
            let pl = ground::plateau_for(&box_at, gr as f64 + rise, 0.0).unwrap();
            e.plateaus.insert("h:far".into(), pl);
            let sh = ground::Shaper::build(&e, None, area, SQUARE_FT).unwrap();
            let h = sh.at_square(gx0, gy0, gr, 0.0).0 as f64;
            assert!((h - (gr as f64 + rise)).abs() < 0.01, "a {name} {rise} ft from the ground is at {h}, not {}", gr as f64 + rise);
            assert!(pit == (h < gr as f64), "a {name}");
        }
        let (mut best, mut found) = (0.0, None);
        'scan: for r in 1..2000 {
            for d in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
                let p = [islet_at[0] + d[0] * r as f64 * 400.0, islet_at[1] + d[1] * r as f64 * 400.0];
                if p[0] < 200.0 || p[1] < 200.0 || p[0] > g.map_w_ft - 200.0 || p[1] > g.map_h_ft - 200.0 {
                    continue;
                }
                let depth = ex.t0.sample_water(p[0], p[1]) as f64 - ex.t0.sample(p[0], p[1], SQUARE_FT);
                if depth > best {
                    best = depth;
                }
                if depth > 200.0 {
                    found = Some(p);
                    break 'scan;
                }
            }
        }
        let p = found.unwrap_or_else(|| panic!("no sea deeper than 200 ft (deepest {best})"));
        let deck = [[-35.0, -20.0], [35.0, -20.0], [35.0, 20.0], [-35.0, 20.0]].map(|d| [p[0] + d[0], p[1] + d[1]]);
        assert!(agent::building_spot(&ex.world, &ex.t0, &foot(p), None, "c:99", None).is_err(), "a building in deep water");
        ex.world.file.edits.plateaus.insert("h:deep".into(), ground::plateau_for(&deck, ex.t0.sample_water(p[0], p[1]) as f64 + 12.0, 0.0).unwrap());
        agent::building_spot(&ex.world, &ex.t0, &foot(p), None, "c:99", None).expect("a building on a plateau over deep water");
        ex.world.file.edits = Edits::default();
    }

    // P6: water outlines. An inlet from 50 ft inland out into the sea at the sea's level: wet on
    // the battlemap all the way out to the sea's own water (one water at one level), the ground
    // round it as it was, refused to a building, the tile's samples as generated; two overlapping
    // ponds on dry land are one water with no bank between them; cleared, as generated.
    use worldgen::waters::{Water, WaterKind};
    // (No building, deck (a quay, a pier) or wall on the 50 ft inland the inlet cuts: it clears
    // none of them.)
    // (And for the ponds below: open dry ground west of a shore, on its row.)
    // (Flat: the ponds stand 2 ft over it.)
    let open = |k2: usize| (k2 - 6..=k2 + 6).all(|q| !wet(&plain_c.1, q) && plain_c.1.building[q] == 0 && (plain_c.1.height[q] - plain_c.1.height[k2]).abs() < 1.5);
    let west_of = |k2: usize| {
        let (j, i0) = (k2 / SQ, k2 % SQ);
        let mut west: Vec<usize> = (14..i0.saturating_sub(10)).collect();
        west.sort_by_key(|&o| (o as i64 - 40).abs());
        west.into_iter().map(|o| j * SQ + i0 - o).find(|&q| open(q))
    };
    let clear = |k2: usize| (k2 % SQ >= 11) && (k2 - 10..=k2).all(|q| plain_c.1.building[q] == 0 && !matches!(plain_c.1.urban[q], worldgen::battlemap::URBAN_DECK | worldgen::battlemap::URBAN_WALL | worldgen::battlemap::URBAN_TOWER));
    let k2 = others.iter().skip(2).chain(others.iter()).copied().find(|&k2| clear(k2)).expect("a shore clear of buildings for the inlet");
    let shore = at(k2);
    let sea = plain_c.1.water_level[k2 + 8];
    let inlet = Water::outline(vec![[shore[0] - 50.0, shore[1] - 15.0], [shore[0] + 30.0, shore[1] - 15.0], [shore[0] + 30.0, shore[1] + 15.0], [shore[0] - 50.0, shore[1] + 15.0]], WaterKind::Inlet, sea);
    assert!(inlet.problem().is_none(), "the inlet: {:?}", inlet.problem());
    ex.world.file.edits.waters.insert("e:inlet".into(), inlet.clone());
    let (inlet_pack, c) = pack(&mut ex, key);
    let (j, i0) = (k2 / SQ, k2 % SQ);
    for i in i0 - 8..=i0 + 16 {
        let q = j * SQ + i;
        assert!(wet(&c, q) && (c.water_level[q] - sea).abs() < 0.01, "the inlet: square {} from the shore is not the sea's water ({} over {})", i as i64 - i0 as i64, c.water_level[q], c.height[q]);
    }
    for q in [(j - 6) * SQ + i0 - 4, (j + 6) * SQ + i0 - 4, j * SQ + i0 - 14] {
        assert!(c.height[q] == plain_c.1.height[q] && c.water_level[q] == plain_c.1.water_level[q], "the inlet changed the ground outside it at square {q}");
    }
    assert!(agent::building_spot(&ex.world, &ex.t0, &foot([shore[0] - 25.0, shore[1]]), None, "c:99", None).is_err(), "a building in the inlet");
    let tk = fine(shore);
    let shaped_tile = ex.terrain_packed(tk);
    let json = serde_json::to_string(&ex.world.file.edits).unwrap();
    assert!(serde_json::from_str::<Edits>(&json).unwrap() == ex.world.file.edits, "water outlines through JSON");
    ex.world.file.edits = Edits::default();
    let plain_tile = ex.terrain_packed(tk);
    let samples = 48..48 + PADDED * PADDED * 4;
    assert!(shaped_tile != plain_tile && shaped_tile[samples.clone()] == plain_tile[samples], "tile {tk:?}: the inlet drawn, its samples as generated");
    // Two ponds overlapping on dry ground west of the shore, 2 ft above it (open ground along
    // the row looked at: no building there, as the town round the shore is laid out).
    let dry = others.iter().find_map(|&k2| west_of(k2)).expect("open dry ground west of a shore");
    let (dp, top) = (at(dry), plain_c.1.height[dry]);
    let pond = |dx: f64| Water::outline(vec![[dp[0] + dx - 20.0, dp[1] - 20.0], [dp[0] + dx + 20.0, dp[1] - 20.0], [dp[0] + dx + 20.0, dp[1] + 20.0], [dp[0] + dx - 20.0, dp[1] + 20.0]], WaterKind::Pond, top + 2.0);
    ex.world.file.edits.waters.insert("e:a".into(), pond(-15.0));
    ex.world.file.edits.waters.insert("e:b".into(), pond(15.0));
    let c = pack(&mut ex, key).1;
    for i in dry % SQ - 5..=dry % SQ + 5 {
        let q = dry / SQ * SQ + i;
        assert!(wet(&c, q) && (c.water_level[q] - top - 2.0).abs() < 0.01, "overlapping ponds: square {q} at {} under {}", c.height[q], c.water_level[q]);
    }
    let s = worldgen::town::Shore::new(&ex.world, &ex.t0, [dp[0] - 100.0, dp[1] - 100.0, dp[0] + 100.0, dp[1] + 100.0]);
    assert!(s.wet(dp) && !s.wet([dp[0], dp[1] + 40.0]), "the ponds' shore");
    ex.world.file.edits = Edits::default();
    assert!(pack(&mut ex, key).0 == plain_c.0 && inlet_pack != plain_c.0, "the shore's chunk with and without the water outlines");
}
