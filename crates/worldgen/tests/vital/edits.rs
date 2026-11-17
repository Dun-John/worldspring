//! Agent edits: the edits guarantee, hand castle options, created sites in a remade
//! world, islets and decks.

use super::*;

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
    islet_and_deck_edits(&base);
    bridge_shops(&base);
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
    agent_e.notes.insert(city.clone(), worldgen::world::Note { text: "agent lore".into(), tags: vec![], at: None });
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
    // A note at a point in a room (`m:<site>:<level>:<id>`, `at` in grid units) is a place in it.
    let (mid, c) = (format!("m:{site}:0:x1"), crypt.levels[0].rooms[ri].center);
    let mut f = w3.file.clone();
    f.edits.notes.insert(mid.clone(), worldgen::world::Note { text: "A chest".into(), tags: vec!["treasure".into()], at: Some([c[0] as f64, c[1] as f64]) });
    let f: WorldFile = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
    let w4 = World::new(f).unwrap();
    assert!(agent::position(&w4, t0, &mid) == agent::position(&w4, t0, &rid) && agent::name_any(&w4, t0, &mid).as_deref() == Some("Note in Vesk's Tomb"), "a point note does not resolve to its room");

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
    e.notes.insert(ib.clone(), worldgen::world::Note { text: "Rooms upstairs".into(), tags: vec!["inn".into()], at: None });
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
    let (one, two) = (worldgen::world::Note { text: "A".into(), tags: vec!["x".into()], at: None }, worldgen::world::Note { text: "B".into(), tags: vec!["x".into(), "y".into()], at: None });
    assert_eq!(worldgen::world::Note::join(Some(&one), Some(&two)), Some(worldgen::world::Note { text: "A\n\nB".into(), tags: vec!["x".into(), "y".into()], at: None }), "two notes on one place lose text when joined");
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

/// (Roadmap 6 P7) A castle drawn by hand on a concave outline: its curtain keeps inside the outline
/// with a tower at each corner turning inward, a gate in the middle of each side asked for (one
/// drawbridge each over its moat), its keep round or drawn as asked and its moat as asked; a moat
/// left to its roll is the same under another id (keyed to the outline); a building drawn in its
/// yard is let in, the castle's own buildings standing clear of it. The castle's guarantees hold.
#[test]
fn hand_castle_options() {
    use worldgen::pipeline::Executor;
    use worldgen::town::{self, geom};
    use worldgen::world::Created;
    use worldgen::agent;
    let mut ex = Executor::new(World::from_json(&world_json(1)).unwrap());
    let sea = ex.world.params().sea_level_ft;
    let first = ex.t0.settlements.len() + ex.t0.base_pois;
    let n0 = ex.world.file.edits.created.len();
    // A U open to the south (y runs south): 700 × 500 ft, its notch 240 ft wide and 230 deep.
    let u = |c: [f64; 2]| [[-350.0, -250.0], [350.0, -250.0], [350.0, 250.0], [120.0, 250.0], [120.0, 20.0], [-120.0, 20.0], [-120.0, 250.0], [-350.0, 250.0]].map(|q| [c[0] + q[0], c[1] + q[1]]).to_vec();
    let castle = |id: usize, at: [f64; 2]| Created { id: format!("c:{id}"), kind: "castle".into(), x: at[0], y: at[1], poly: u(at), name: format!("Castle {id}"), ..Default::default() };
    let (world, t0) = (&ex.world, &ex.t0);
    let at = (0..t0.settlements.len() * 8)
        .map(|k| {
            let s = &t0.settlements[k / 8];
            let a = (k % 8) as f64 * std::f64::consts::TAU / 8.0 + 0.3;
            [s.x + 15_840.0 * libm::cos(a), s.y + 15_840.0 * libm::sin(a)].map(|v| (v / 5.0).round() * 5.0)
        })
        .find(|p| t0.sample(p[0], p[1], 20.0) > sea + 20.0 && agent::works_spot(world, t0, &castle(n0, *p), "c:98", None).is_ok_and(|s| s.in_way.is_empty()))
        .expect("open ground for a castle");
    let gated = Created { gates: vec![0, 1, 7], keep_shape: Some("round".into()), moat: Some("dry".into()), ..castle(n0, at) };
    assert!(gated.check().is_ok(), "a concave castle with three gates is refused: {:?}", gated.check());
    assert!(Created { keep_shape: Some("drawn".into()), ..gated.clone() }.check().is_err(), "a drawn keep without its outline is let in");
    // The same outline under other ids: its moat by roll, none, and wet with a drawn keep.
    let far = [at[0] + 3_000.0, at[1]];
    let keep_poly = vec![[at[0] - 80.0, at[1] - 200.0], [at[0] + 40.0, at[1] - 200.0], [at[0] + 40.0, at[1] - 120.0], [at[0] - 80.0, at[1] - 120.0], [at[0] - 110.0, at[1] - 160.0]];
    let others = [
        castle(n0 + 1, at),
        Created { moat: Some("none".into()), ..castle(n0 + 2, far) },
        Created { moat: Some("wet".into()), keep_poly: keep_poly.iter().map(|q| [q[0] + 3_000.0, q[1]]).collect(), ..castle(n0 + 3, far) },
    ];
    let mut e = ex.world.file.edits.clone();
    e.created.push(gated.clone());
    e.created.extend(others.iter().cloned());
    let set = |ex: &mut Executor, e: worldgen::world::Edits| {
        let old = std::mem::replace(&mut ex.world.file.edits, e).created;
        ex.t0.apply_edits(&ex.world);
        town::forget_created(&old, &ex.world.file.edits.created, first);
    };
    set(&mut ex, e.clone());
    let (world, t0) = (&ex.world, &ex.t0);
    let li = first + n0;
    let l = town::layout(world, t0, li);
    let tag = "the U castle";
    castle_works(world, t0, &l, li, &mut 0, &mut 0);
    assert!(l.walls.iter().flatten().all(|p| geom::contains(&gated.poly, *p)), "{tag}: its curtain stands outside the outline drawn");
    for c in [[at[0] + 120.0, at[1] + 20.0], [at[0] - 120.0, at[1] + 20.0]] {
        assert!(l.towers.iter().any(|t| geom_dist(*t, c) < 15.0), "{tag}: no tower at the corner turning inward at {c:?}");
    }
    let g = &l.gate_towers;
    let pairs: Vec<[f64; 2]> = (0..g.len()).flat_map(|a| (a + 1..g.len()).filter(move |&b| (geom_dist(g[a], g[b]) - 52.0).abs() < 2.0).map(move |b| geom::lerp(g[a], g[b], 0.5))).collect();
    for side in [0usize, 1, 7] {
        let (a, b) = (gated.poly[side], gated.poly[(side + 1) % 8]);
        let mid = geom::lerp(a, b, 0.5);
        assert!(pairs.iter().any(|g| geom_dist(*g, mid) < 12.0), "{tag}: no gate in the middle of side {side} ({mid:?}); gates {pairs:?}");
    }
    let fi = town::catalog::index_of("castle").unwrap() as u16;
    let k = l.buildings.iter().find(|b| b.func == Some(fi)).expect("the U castle has no keep");
    // (A round great tower: one piece, its round body.)
    let round = k.parts.len() == 1 && matches!(k.parts[0].kind, town::PartKind::Body);
    assert!(round && k.poly.len() == 24, "{tag}: its keep is not round ({} corners, {} pieces)", k.poly.len(), k.parts.len());
    assert!(l.moats.len() == 1 && !l.moats[0].wet && l.moats[0].bridges.len() == 3, "{tag}: {} moats, want one dry with 3 drawbridges", l.moats.len());
    let rolled = town::layout(world, t0, li + 1);
    let none = town::layout(world, t0, li + 2);
    let wet = town::layout(world, t0, li + 3);
    assert!(none.moats.is_empty(), "a castle with moat none has one");
    assert!(wet.moats.len() == 1 && wet.moats[0].wet, "a castle with a wet moat asked has {}", if wet.moats.is_empty() { "none" } else { "a dry one" });
    let wk = wet.buildings.iter().find(|b| b.func == Some(fi)).expect("the drawn keep");
    assert!(wk.poly.len() == 5 && wk.poly.iter().all(|p| others[2].keep_poly.iter().any(|q| geom_dist(*p, *q) < 0.01)), "the drawn keep is not as drawn: {:?}", wk.poly);
    // (Keyed to the outline: a castle with its moat rolled drawn on the outline of one under another id.)
    let mut moved = e.clone();
    moved.created[n0 + 2] = castle(n0 + 2, at);
    set(&mut ex, moved);
    let (world, t0) = (&ex.world, &ex.t0);
    let again = town::layout(world, t0, li + 2);
    let roll = |l: &town::Layout| l.moats.first().map(|m| (m.wet, (m.width * 8.0).round() as i64));
    assert_eq!(roll(&again), roll(&rolled), "the same outline rolled another moat under another id");
    // A building drawn in the yard, on its ranges along the south curtain: let in; the castle's own
    // buildings stand clear of it.
    let yard_l = town::layout(world, t0, li);
    let range = yard_l.buildings.iter().filter(|b| b.keep.is_none()).find(|b| b.poly.iter().all(|p| p[1] < at[1] - 150.0)).expect("a range along the south curtain");
    let c = geom::centroid(&range.poly);
    let hut = vec![[c[0] - 15.0, c[1] - 10.0], [c[0] + 15.0, c[1] - 10.0], [c[0] + 15.0, c[1] + 10.0], [c[0] - 15.0, c[1] + 10.0]];
    let spot = agent::building_spot(world, t0, &hut, None, "c:99", None);
    assert!(spot.is_ok(), "a building in a hand castle's yard is refused: {spot:?}");
    let mut with = ex.world.file.edits.clone();
    with.created.push(Created { id: format!("c:{}", n0 + 4), kind: "building".into(), x: c[0], y: c[1], name: "Hut".into(), poly: hut.clone(), ..Default::default() });
    set(&mut ex, with);
    let l = town::layout(&ex.world, &ex.t0, li);
    assert!(l.buildings.iter().all(|b| b.poly.iter().all(|p| !geom::contains(&hut, *p)) && hut.iter().all(|p| !geom::contains(&b.poly, *p))), "the castle's buildings stand on the one drawn in its yard");
    assert!(l.moats.len() == 1 && l.buildings.iter().any(|b| b.func == Some(fi)), "the castle lost its keep or moat to the building drawn in its yard");
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
    // how many were placed before it. One drawn on a town is refused. (Not an isolated realm's
    // village: a pin beside a realm moves the realm.)
    use worldgen::t0::settle::Tier;
    let near_village = t0.settlements.iter().enumerate().filter(|(_, s)| s.tier == Tier::Village && !s.isolated).find_map(|(i, s)| {
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
    let town_at = t0.settlements.iter().find(|s| s.tier == Tier::Town && s.planned == Tier::Town).map(|s| [s.x + 2_000.0, s.y]).unwrap();
    let eo = added_at(town_at);
    let k = eo.world.file.sketch.strokes.len() - 1;
    let refused = eo.t0.extra.as_ref().unwrap().overlay.conflicts.iter().any(|c| c.stroke == k && c.message.contains("within 3 miles of a town"));
    assert!(refused && eo.t0.settlements.len() == n, "a town added on a town was placed ({} settlements)", eo.t0.settlements.len());
    // Twelve miles from a lone village (past the least room, within a village's own spacing of
    // 14): the village stays.
    let past = t0.settlements.iter().filter(|s| s.tier == Tier::Village && !s.isolated).find_map(|s| {
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

/// P6 6b: a generated islet taken away (`Edits.hidden`) is no longer one (it leaves the shore and the ground, its id is
/// stable and brings it back), and a hand bridge's deck climbs from end to end, at most `DECK_GRADE_MAX`.
fn islet_and_deck_edits(base: &World) {
    use worldgen::world::{Crossing, CrossingKind};
    let mut ex = Executor::new(World::new(base.file.clone()).unwrap());
    let isl = worldgen::t0::islets::islets(&ex.t0).iter().find(|i| i.river).expect("a river islet").clone();
    assert!(isl.id.starts_with("i:") && isl.id.len() == 10, "an islet's id: {}", isl.id);
    let r = [isl.at[0] - 5.0, isl.at[1] - 5.0, isl.at[0] + 5.0, isl.at[1] + 5.0];
    assert!(worldgen::town::Shore::new(&ex.world, &ex.t0, r).on_islet(isl.at), "the generated islet isn't one");
    ex.world.file.edits.hidden.insert(isl.id.clone());
    assert!(!worldgen::town::Shore::new(&ex.world, &ex.t0, r).on_islet(isl.at), "the islet taken away is still one");
    assert!(worldgen::t0::islets::near_in(&ex.t0, &ex.world.file.edits, r).next().is_none(), "near_in lists it");
    ex.world.file.edits.hidden.remove(&isl.id);
    assert!(worldgen::town::Shore::new(&ex.world, &ex.t0, r).on_islet(isl.at), "the islet brought back isn't one");
    // A bridge over dry ground from level ground to a plateau 60 ft up, 200 ft away: the deck climbs, at most 1 in 8.
    let s = ex.t0.settlements.iter().max_by_key(|s| s.population).unwrap();
    let a = [s.x + 3000.0, s.y];
    let b = [a[0] + 200.0, a[1]];
    let gb = ex.t0.ground_at(b[0], b[1], 5.0) as f32;
    let outline = [[b[0] - 30.0, b[1] - 30.0], [b[0] + 30.0, b[1] - 30.0], [b[0] + 30.0, b[1] + 30.0], [b[0] - 30.0, b[1] + 30.0]];
    ex.world.file.edits.plateaus.insert("h:up".into(), worldgen::ground::plateau_for(&outline, gb as f64 + 60.0, 0.0).unwrap());
    let c = Crossing { kind: CrossingKind::Bridge, a, b, width: 20.0, material: Default::default() };
    let deck = worldgen::battlemap::hand_deck_ends(&c, &ex.world, &ex.t0);
    let (da, db) = deck.ends;
    assert!(db > da + 10.0 && (db - da) as f64 <= worldgen::battlemap::DECK_GRADE_MAX * 200.0 + 0.01, "the deck's ends {da} and {db}");
    assert!(deck.level(0.5) > da && deck.level(0.5) < db, "the deck doesn't climb");
    // (Its far end is the plateau's middle: the deck is at its end's level from the plateau's edge, 170 ft along, on.)
    assert!((deck.level(0.85) - db).abs() < 0.01 && deck.level(0.8) < db, "the deck doesn't meet the plateau's edge: {deck:?}");
}

// R5 121
