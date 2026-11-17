//! Town edits called by the edits guarantee: wards, castles by the wall, ground
//! edits and bridge shops.

use super::*;

/// The ward editor (`Edits.towns`) on the largest walled city: one corner moved and one patch
/// made a castle. Patches away from them keep their buildings (ids, footprints, functions and
/// names), the districts their names, the wall its towers; the new buildings pass the interior
/// rules; the castle stands with its keep and towers after every other; the result is
/// repeatable, round-trips as an edit op and leaves the world hash alone. Walls taken down keep
/// the gates and every building; walls put up round an unwalled town stand; a stale edit is set
/// aside; a corner on the water is refused.
pub(super) fn ward_edits(base: &World, t0: &worldgen::t0::T0) {
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
    // (Where the street moved most: near the end of a segment the edit changed, on land: a wall
    // across a bridge stands in the river, broken there.)
    let off_bridges = |p: [f64; 2]| l.bridges.iter().all(|q| !geom::contains(q, p) && (0..q.len()).all(|i| geom::seg_dist(p, q[i], q[(i + 1) % q.len()]) > 30.0));
    let (a, b) = after
        .iter()
        .copied()
        .find(|&(a, b)| geom::dist(a, b) > 40.0 && !before.contains(&(a, b)) && before.iter().all(|&(p, q)| geom::seg_dist(geom::lerp(a, b, 0.85), p, q) > 5.0) && off_bridges(geom::lerp(a, b, 0.85)))
        .expect("a main street the edit moved");
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
pub(super) fn ward_castles_by_the_wall(base: &World, t0: &worldgen::t0::T0) {
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
pub(super) fn ground_edits(base: &World) {
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
    let k2 = others[2.min(others.len() - 1)];
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
    let open = |k2: usize| (k2 - 6..=k2 + 6).all(|q| !wet(&plain_c.1, q) && plain_c.1.building[q] == 0);
    let mut west: Vec<usize> = (14..i0 - 10).collect();
    west.sort_by_key(|&o| (o as i64 - 40).abs());
    let dry = west.into_iter().map(|o| j * SQ + i0 - o).find(|&k2| open(k2)).expect("open dry ground west of the shore");
    let (dp, top) = (at(dry), plain_c.1.height[dry]);
    let pond = |dx: f64| Water::outline(vec![[dp[0] + dx - 20.0, dp[1] - 20.0], [dp[0] + dx + 20.0, dp[1] - 20.0], [dp[0] + dx + 20.0, dp[1] + 20.0], [dp[0] + dx - 20.0, dp[1] + 20.0]], WaterKind::Pond, top + 2.0);
    ex.world.file.edits.waters.insert("e:a".into(), pond(-15.0));
    ex.world.file.edits.waters.insert("e:b".into(), pond(15.0));
    let c = pack(&mut ex, key).1;
    for i in dry % SQ - 5..=dry % SQ + 5 {
        let q = j * SQ + i;
        assert!(wet(&c, q) && (c.water_level[q] - top - 2.0).abs() < 0.01, "overlapping ponds: square {q} at {} under {}", c.height[q], c.water_level[q]);
    }
    let s = worldgen::town::Shore::new(&ex.world, &ex.t0, [dp[0] - 100.0, dp[1] - 100.0, dp[0] + 100.0, dp[1] + 100.0]);
    assert!(s.wet(dp) && !s.wet([dp[0], dp[1] + 40.0]), "the ponds' shore");
    ex.world.file.edits = Edits::default();
    assert!(pack(&mut ex, key).0 == plain_c.0 && inlet_pack != plain_c.0, "the shore's chunk with and without the water outlines");
}

/// R5 M20: shops on a bridge put down by hand over a river, clear of every layout. Its lots
/// (`wharf::bridge_lots`) are refused before the bridge and accepted on it (`on` the bridge),
/// its lane refused; a shop drawn there stands at the deck's level with no cellar, its front door
/// on the lane, on dry squares of the battlemap with the lane open round it at the deck's level;
/// it follows its bridge (laid out again when it is widened, removed with it).
pub(super) fn bridge_shops(base: &World) {
    use worldgen::battlemap::{HS, SQ, SQUARE_FT};
    use worldgen::world::{Created, Crossing, CrossingKind};
    use worldgen::{agent, interior, town, wharf};
    let mut ex = Executor::new(World::new(base.file.clone()).unwrap());
    let (g, first) = (ex.world.geom.clone(), ex.t0.settlements.len() + ex.t0.base_pois);
    // A river 50-150 ft wide, its banks found from its middle across its way; a 50-ft bridge
    // from 10 ft up one bank to 10 ft up the other.
    let cell = ex.t0.cell_ft;
    let bridge = ex
        .t0
        .rivers
        .rivers
        .iter()
        .flat_map(|r| (0..r.segments()).map(move |k| (r.eval(k, 0.5, 5.0, cell).p, (r.eval(k, 0.45, 5.0, cell), r.eval(k, 0.55, 5.0, cell)))))
        .filter(|(_, (a, _))| (50.0..150.0).contains(&a.w))
        .filter(|(p, _)| p[0] > 2000.0 && p[1] > 2000.0 && p[0] < g.map_w_ft - 2000.0 && p[1] < g.map_h_ft - 2000.0)
        .filter(|(p, _)| town::layouts_near(&ex.world, &ex.t0, [p[0] - 600.0, p[1] - 600.0, p[0] + 600.0, p[1] + 600.0]).is_empty())
        .find_map(|(p, (a, b))| {
            let d = [b.p[0] - a.p[0], b.p[1] - a.p[1]];
            let dl = (d[0] * d[0] + d[1] * d[1]).sqrt();
            let n = [-d[1] / dl, d[0] / dl];
            let s = town::Shore::new(&ex.world, &ex.t0, [p[0] - 200.0, p[1] - 200.0, p[0] + 200.0, p[1] + 200.0]);
            let bank = |sign: f64| (0..60).map(|k| k as f64 * 5.0).find(|t| { let q = [p[0] + n[0] * sign * t, p[1] + n[1] * sign * t]; !s.wet(q) && !s.in_river(q) }).map(|t| [p[0] + n[0] * sign * (t + 10.0), p[1] + n[1] * sign * (t + 10.0)]);
            let c = Crossing { kind: CrossingKind::Bridge, a: bank(-1.0)?, b: bank(1.0)?, width: 50.0, material: Default::default() };
            (c.length() >= 90.0 && c.length() <= 300.0).then_some(c)
        })
        .expect("a river to bridge");
    assert!(bridge.problem(g.map_w_ft, g.map_h_ft).is_none(), "a 50-ft bridge: {:?}", bridge.problem(g.map_w_ft, g.map_h_ft));
    assert!(Crossing { kind: CrossingKind::Ford, ..bridge.clone() }.problem(g.map_w_ft, g.map_h_ft).is_some(), "a 50-ft ford was let through");
    assert!(wharf::bridge_lots(&Crossing { kind: CrossingKind::Ford, ..bridge.clone() }, 14.0, 18.0, 20.0).is_empty(), "shops on a ford");
    let lots = wharf::bridge_lots(&bridge, wharf::SHOP_LANE_FT, wharf::SHOP_DEPTH_FT, wharf::SHOP_FRONT_FT);
    assert!(lots.len() >= 4 && lots.len() % 2 == 0, "{} lots on a {:.0}-ft bridge", lots.len(), bridge.length());
    // In the middle of the river: the lot nearest it.
    let mid = [0.5 * (bridge.a[0] + bridge.b[0]), 0.5 * (bridge.a[1] + bridge.b[1])];
    let lot = lots.iter().min_by(|p, q| geom_dist(town::geom::centroid(p), mid).total_cmp(&geom_dist(town::geom::centroid(q), mid))).unwrap().clone();
    let u = [(bridge.b[0] - bridge.a[0]) / bridge.length(), (bridge.b[1] - bridge.a[1]) / bridge.length()];
    let lane_foot: Vec<[f64; 2]> = [[-5.0, -5.0], [5.0, -5.0], [5.0, 5.0], [-5.0, 5.0]].iter().map(|o| [mid[0] + u[0] * o[0] - u[1] * o[1], mid[1] + u[1] * o[0] + u[0] * o[1]]).collect();
    let set = |ex: &mut Executor, e: worldgen::world::Edits| {
        let old = std::mem::replace(&mut ex.world.file.edits, e).created;
        ex.t0.apply_edits(&ex.world);
        town::forget_created(&old, &ex.world.file.edits.created, first);
    };
    assert!(agent::building_spot(&ex.world, &ex.t0, &lot, None, "c:0", None).is_err(), "a shop in the river before the bridge");
    let mut e = ex.world.file.edits.clone();
    e.crossings.insert("v:shops".into(), bridge.clone());
    set(&mut ex, e.clone());
    let (at, _) = agent::building_spot(&ex.world, &ex.t0, &lot, Some("tailor"), "c:0", None).unwrap_or_else(|m| panic!("a shop on the bridge refused: {m}"));
    assert_eq!(wharf::deck_under(&ex.world, &lot), Some(("v:shops".to_string(), None)), "the shop is not on the bridge");
    match agent::building_spot(&ex.world, &ex.t0, &lane_foot, None, "c:0", None) {
        Ok(_) => panic!("a building on the bridge's lane"),
        Err(m) => assert!(m.contains("lane of bridge v:shops"), "the lane's refusal: {m}"),
    }
    e.created.push(Created { id: "c:0".into(), kind: "building".into(), x: at[0], y: at[1], name: String::new(), poly: lot.clone(), func: Some("tailor".into()), floors: Some(2), on: Some("v:shops".into()), ..Default::default() });
    set(&mut ex, e.clone());
    let top = worldgen::battlemap::hand_deck_top(&bridge, &ex.world, &ex.t0);
    let li = agent::layout_of(&ex.world, &ex.t0, "c:0").expect("the shop's layout");
    let l = town::layout(&ex.world, &ex.t0, li);
    let b = l.buildings.first().expect("the shop");
    assert!((b.pad_ft - top).abs() < 0.01 && b.deck.is_some() && !interior::has_cellar(b), "the shop: pad {} on a deck at {top}, cellar {}", b.pad_ft, interior::has_cellar(b));
    let inside = interior::generate(&ex.world, &ex.t0, li, 0).expect("the shop's interior");
    assert!(inside.levels.iter().all(|v| v.z >= 0) && inside.levels[inside.entry_level].z == 0, "the shop has a level below its deck");
    let ground = &inside.levels[inside.entry_level];
    let door = ground.doors.iter().find(|d| d.kind == "front").expect("the shop's front door");
    let dm = [0.5 * (door.a[0] + door.b[0]) as f64, 0.5 * (door.a[1] + door.b[1]) as f64];
    let dw = [inside.origin[0] + (inside.axis[0] * dm[0] + inside.across[0] * dm[1]) * SQUARE_FT, inside.origin[1] + (inside.axis[1] * dm[0] + inside.across[1] * dm[1]) * SQUARE_FT];
    let line = |p: [f64; 2]| town::geom::seg_dist(p, bridge.a, bridge.b);
    assert!(line(dw) < 0.5 * wharf::SHOP_LANE_FT + 3.0, "the shop's door is {:.1} ft off the lane's middle, not on the lane", line(dw));
    // On the battlemap: built on, dry, the lane beside it open at the deck's level.
    let size = g.tile_size_ft(g.max_level);
    let m = town::geom::centroid(&lot);
    let key = TileKey::surface(g.max_level, (m[0] / size) as u32, (m[1] / size) as u32);
    let c = ex.battlemap(key);
    let k2 = |p: [f64; 2]| ((p[1] - key.y as f64 * size) / SQUARE_FT) as usize * SQ + ((p[0] - key.x as f64 * size) / SQUARE_FT) as usize;
    assert!(c.building[k2(m)] != 0, "no building at the shop's middle");
    let mut lane_sq = 0;
    for q in 0..SQ * SQ {
        let p = [key.x as f64 * size + ((q % SQ) as f64 + 0.5) * SQUARE_FT, key.y as f64 * size + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
        if line(p) < 0.5 * wharf::SHOP_LANE_FT - 2.0 && town::geom::dist(p, m) < 20.0 {
            let h = (q / SQ + 1) * HS + q % SQ + 1;
            assert!(c.water_level[q] < c.height[q] && c.building[q] == 0 && (c.halo_height[h] - top).abs() <= 0.5, "lane square {p:?}: {} on a deck at {top}", c.halo_height[h]);
            lane_sq += 1;
        }
    }
    assert!(lane_sq >= 4, "too few lane squares by the shop ({lane_sq})");
    // Widened: laid out again on it; taken away: gone with it.
    let wide = Crossing { width: 70.0, ..bridge.clone() };
    e.crossings.insert("v:shops".into(), wide.clone());
    assert_eq!(e.follow_crossing("v:shops", Some(&bridge)), vec!["c:0".to_string()], "the shop didn't follow its bridge");
    set(&mut ex, e.clone());
    assert!(e.created[0].poly != lot && !e.created[0].removed && wharf::deck_under(&ex.world, &e.created[0].poly) == Some(("v:shops".to_string(), None)), "the shop laid out again off its widened bridge");
    let l = town::layout(&ex.world, &ex.t0, li);
    assert!(l.buildings.first().is_some_and(|b| b.poly == e.created[0].poly), "the shop's layout kept its old lot");
    e.crossings.remove("v:shops");
    e.follow_crossing("v:shops", Some(&wide));
    assert!(e.created[0].removed, "the shop stayed when its bridge went");
}

/// Roadmap 6 R4: a street added to a generated town (a row's road) is one of its streets, its
/// ends joined to the nearest of the others; the buildings under its band are gone and every
/// other keeps its id; the layout is the same whatever was asked first; without it the town is
/// as built; a ward edit keeps it. A drawn house with `faces` fronts that chord.
pub(super) fn row_street(ex: &mut Executor, house: &worldgen::world::Created) {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, added, geom, wards};
    use worldgen::world::{AddedStreet, TownEdit};
    use worldgen::model::paths::PathKind;
    let t0 = &ex.t0;
    let ti = (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier == Tier::Town).max_by_key(|&i| t0.settlements[i].population).expect("a town");
    let s = &t0.settlements[ti];
    let g = town::layout(&ex.world, t0, ti);
    let line_d = |p: [f64; 2], pts: &[[f64; 2]]| pts.windows(2).map(|w| worldgen::town::geom::seg_dist(p, w[0], w[1])).fold(f64::MAX, f64::min);
    // A straight street 120 ft long through a building near the middle, along it, joined at both ends.
    let mut near: Vec<&worldgen::town::Building> = g.buildings.iter().collect();
    near.sort_by(|a, b| geom_dist(geom::centroid(&a.poly), g.center).total_cmp(&geom_dist(geom::centroid(&b.poly), g.center)).then(a.id.cmp(&b.id)));
    let streets = near
        .iter()
        .map(|b| {
            let (c, d) = (geom::centroid(&b.poly), geom::main_axis(&b.poly));
            let pts = vec![[c[0] - 60.0 * d[0], c[1] - 60.0 * d[1]], [c[0] + 60.0 * d[0], c[1] + 60.0 * d[1]]].into_iter().map(|p| p.map(|v| (v * 2.0).round() * 0.5)).collect();
            std::collections::BTreeMap::from([(1u32, AddedStreet { pts, kind: PathKind::Paved, width: None })])
        })
        .find(|st| added::lines(&g, st)[0].1.len() == 4)
        .expect("a street that joins at both ends");
    let laid = added::lines(&g, &streets).remove(0).1;
    let mut file = ex.world.file.clone();
    file.edits.towns.insert(wards::key(ti), TownEdit { at: [s.x, s.y], streets: streets.clone(), ..Default::default() });
    let world = World::new(file).unwrap();
    assert_eq!(world.hash, ex.world.hash, "an added street changed the world hash");
    let l = town::layout(&world, t0, ti);
    let st = l.roads.iter().find(|r| r.0 == laid).expect("the added street is not one of the town's roads");
    assert!(st.1 == added::CLASS && st.2 == 18.0, "the added street's class {} or width {}", st.1, st.2);
    let others: Vec<&Vec<[f64; 2]>> = g.roads.iter().map(|r| &r.0).chain(&g.streets).collect();
    for end in [laid[0], laid[laid.len() - 1]] {
        assert!(others.iter().any(|o| line_d(end, o) <= 1.0), "an end of the added street at {end:?} meets no street");
    }
    let seg_seg = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let o = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        if o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0 {
            return 0.0;
        }
        geom::seg_dist(a, c, d).min(geom::seg_dist(b, c, d)).min(geom::seg_dist(c, a, b)).min(geom::seg_dist(d, a, b))
    };
    let under = |poly: &[[f64; 2]]| geom::contains(poly, laid[0]) || laid.windows(2).any(|w| (0..poly.len()).any(|i| seg_seg(w[0], w[1], poly[i], poly[(i + 1) % poly.len()]) <= 9.0));
    assert!(l.buildings.iter().all(|b| !under(&b.poly)), "a building stands on the added street");
    let gone = g.buildings.iter().filter(|b| under(&b.poly)).count();
    assert!(gone > 0, "no building was under the added street");
    for b in g.buildings.iter().filter(|b| !under(&b.poly)) {
        assert!(l.building(b.id as usize).is_some_and(|o| o.poly == b.poly), "b:{ti}:{} moved or went", b.id);
    }
    assert_eq!(l.buildings.len() + gone, g.buildings.len(), "buildings not under the street changed");
    // Pure: the same asked after other layouts round it; without the street, the town as built.
    let print = format!("{:?}", *l);
    town::forget_from(0);
    let c = geom::centroid(&laid);
    let round = town::layouts_near(&world, t0, [c[0] - 2_000.0, c[1] - 2_000.0, c[0] + 2_000.0, c[1] + 2_000.0]);
    assert!(round.iter().any(|r| r.index as usize == ti), "layouts_near misses the town");
    assert_eq!(format!("{:?}", *town::layout(&world, t0, ti)), print, "the town with its street depends on what was asked first");
    let mut bare = world.file.clone();
    bare.edits.towns.get_mut(&wards::key(ti)).unwrap().streets.clear();
    assert_eq!(format!("{:?}", *town::layout(&World::new(bare).unwrap(), t0, ti)), format!("{:?}", *g), "without its street the town is not as built");
    // A ward edit keeps the street.
    let (edit, _) = wards::change(&world, t0, ti, &Default::default()).expect("a ward edit");
    assert_eq!(edit.map(|e| e.streets), Some(streets), "a ward edit dropped the added street");

    // A drawn house (50 by 30 ft where `house` stands) fronts its `faces` chord: its front door
    // on the side nearest it.
    let li = worldgen::agent::layout_of(&ex.world, &ex.t0, &house.id).expect("the house's layout");
    let first = ex.t0.settlements.len() + ex.t0.base_pois;
    let b0 = house.poly.iter().fold([f64::MAX, f64::MAX], |b, p| [b[0].min(p[0]), b[1].min(p[1])]);
    let b = [b0[0], b0[1], b0[0] + 50.0, b0[1] + 30.0];
    let was = ex.world.file.edits.clone();
    let set = |ex: &mut Executor, e: worldgen::world::Edits| {
        let old = std::mem::replace(&mut ex.world.file.edits, e).created;
        ex.t0.apply_edits(&ex.world);
        town::forget_created(&old, &ex.world.file.edits.created, first);
    };
    for (chord, side) in [([[b[0], b[1] - 10.0], [b[2], b[1] - 10.0]], 0usize), ([[b[2] + 10.0, b[1]], [b[2] + 10.0, b[3]]], 1)] {
        let mut e = was.clone();
        let c = e.created.iter_mut().find(|c| c.id == house.id).unwrap();
        (c.poly, c.faces) = (vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]], Some(chord));
        set(ex, e);
        let it = worldgen::interior::generate_id(&ex.world, &ex.t0, &format!("b:{li}:0")).expect("the house's interior");
        let lv = &it.levels[it.entry_level];
        let d = lv.doors.iter().find(|d| d.kind == "front").expect("a front door");
        let m = worldgen::interior::to_world(&it, 0.5 * (d.a[0] + d.b[0]) as f64, 0.5 * (d.a[1] + d.b[1]) as f64);
        let on = if side == 0 { m[1] - b[1] < 6.0 } else { b[2] - m[0] < 6.0 };
        assert!(on, "the house's front door at {m:?} is not on the side facing its chord {chord:?}");
    }
    set(ex, was);
}

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
        let hedges: Vec<(u8, P, P, FenceKind)> = srcs.iter().copied().filter(|s| long(s, fence::CROFT, 30.0) && s.3 == FenceKind::Hedgerow).collect();
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
