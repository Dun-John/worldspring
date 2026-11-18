//! Town edits called by the edits guarantee: wards, castles by the wall, ground
//! edits and bridge shops.

use super::*;

mod ground;
mod removal;

pub(super) use ground::ground_edits;

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

