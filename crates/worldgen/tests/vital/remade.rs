//! Remade worlds keep their changes: created settlements, pins, harbour ground,
//! culture changes.

use super::*;

/// A village and a town added by hand (`world::Created`, `t0::made`) move nothing else: the
/// world's T0, its network roads, its terrain tiles, every other layout and every battlemap
/// clear of the two and their tracks stay as they were; each joins the nearest road by a track
/// within a track's grade; removing them gives the world back; and remade with a site drawn far
/// away, each is laid out as before, with the culture and kind it was given where it was put.
#[test]
fn created_settlements_move_nothing_else() {
    use worldgen::t0::made;
    use worldgen::world::Created;
    use worldgen::town;
    let file: WorldFile = serde_json::from_str(&world_json(1)).unwrap();
    let mut ex0 = Executor::new(World::new(file.clone()).unwrap());
    let first = ex0.t0.settlements.len() + ex0.t0.base_pois;
    // Spots 3 to 6 miles round the world's settlements where each may be added.
    let spots = |ex: &Executor, kind: &str, id: &str| {
        (0..ex.t0.settlements.len() * 32)
            .map(|k| {
                let s = &ex.t0.settlements[k / 32];
                let (a, r) = ((k % 16) as f64 * std::f64::consts::TAU / 16.0, if k % 32 < 16 { 15_840.0 } else { 31_680.0 });
                [s.x + r * libm::cos(a), s.y + r * libm::sin(a)]
            })
            .find(|p| worldgen::t0::made::spot(&ex.world, &ex.t0, kind, id, *p).is_ok())
            .unwrap_or_else(|| panic!("no spot for a {kind}"))
    };
    let entry = |ex: &Executor, kind: &str, id: &str, p: [f64; 2]| {
        let tier = made::tier_of(kind);
        let lives = made::kind_at(&ex.world, &ex.t0, p[0], p[1], tier);
        let key = worldgen::world::PIN_KINDS[lives as usize].to_string();
        Created { id: id.into(), kind: kind.into(), x: p[0], y: p[1], name: format!("New {kind}"), culture: Some(made::culture_at(&ex.t0, p[0], p[1])), lives_by: Some(key), ..Default::default() }
    };
    let v = spots(&ex0, "village", "c:0");
    let mut a = file.clone();
    a.edits.created = vec![entry(&ex0, "village", "c:0", v)];
    let ev = Executor::new(World::new(a.clone()).unwrap());
    let t = spots(&ev, "town", "c:1");
    a.edits.created.push(entry(&ev, "town", "c:1", t));
    let mut ea = Executor::new(World::new(a.clone()).unwrap());
    assert_eq!(ea.t0.made.len(), 2, "both settlements made");
    assert!(town::is_settlement(&ea.t0, first) && town::is_settlement(&ea.t0, first + 1) && !town::is_settlement(&ea.t0, first + 2));

    // T0 and its network roads as the world made them.
    let bytes0 = ex0.t0.to_bytes();
    let mut back = Executor::new(World::new(a.clone()).unwrap());
    back.t0.apply_edits(&ex0.world);
    assert!(back.t0.to_bytes() == bytes0, "T0 changed under created settlements");
    assert_eq!(ea.t0.roads.roads.len(), ex0.t0.roads.roads.len());
    for (r, q) in ea.t0.roads.roads.iter().zip(&ex0.t0.roads.roads) {
        assert!(r.pts == q.pts && r.z == q.z, "a network road moved");
    }

    // Laid out by the village and town generators, each joined to the nearest road by a track.
    for (k, m) in ea.t0.made.iter().enumerate() {
        let l = town::layout(&ea.world, &ea.t0, first + k);
        assert_eq!(l.index as usize, first + k);
        assert!(l.buildings.len() >= 5, "c:{k}: {} buildings", l.buildings.len());
        assert_eq!(m.s.tier, made::tier_of(&a.edits.created[k].kind));
        assert!(m.track.len() >= 2, "c:{k}: no track");
        let end = *m.track.last().unwrap();
        let near = ea.t0.roads.segments_near([end[0] - 50.0, end[1] - 50.0, end[0] + 50.0, end[1] + 50.0], 0.0);
        assert!(!near.is_empty(), "c:{k}: the track does not reach a road");
        assert_eq!(m.class, made::link_class(m.s.tier));
        let gmax = m.class.max_grade();
        let mut run = 0.0;
        for i in (1..m.track.len()).rev() {
            let d = geom_dist(m.track[i], m.track[i - 1]);
            run += d;
            if run > 250.0 {
                assert!(((m.z[i] - m.z[i - 1]).abs() as f64) <= gmax * d + 0.01, "c:{k}: track too steep at {i}");
            }
        }
    }

    // Every other layout near them as it was; terrain tiles too.
    let reach = ea.t0.made.iter().map(|m| [m.s.x, m.s.y]).collect::<Vec<_>>();
    for p in &reach {
        let r = [p[0] - 30_000.0, p[1] - 30_000.0, p[0] + 30_000.0, p[1] + 30_000.0];
        let (la, l0) = (town::layouts_near(&ea.world, &ea.t0, r), town::layouts_near(&ex0.world, &ex0.t0, r));
        let others: Vec<String> = la.iter().filter(|l| (l.index as usize) < first).map(|l| format!("{l:?}")).collect();
        assert_eq!(others, l0.iter().map(|l| format!("{l:?}")).collect::<Vec<_>>(), "a layout near a created settlement changed");
    }
    let g = ea.world.geom.clone();
    let level = g.max_level;
    let size = g.tile_size_ft(level);
    let print = |c: &worldgen::battlemap::Chunk| format!("{:?}", (c.height.iter().map(|h| h.to_bits()).collect::<Vec<_>>(), &c.road_halo, &c.building, &c.buildings, &c.urban, c.objects.len()));
    let key_at = |p: [f64; 2]| TileKey::surface(level, (p[0] / size) as u32, (p[1] / size) as u32);
    // (Squares a created settlement's pad or track reaches.)
    let touched = |ex: &Executor, key: &TileKey| {
        let (ox, oy) = g.tile_origin_ft(key);
        made::Ground::new(&ex.world, &ex.t0, [ox - 10.0, oy - 10.0, ox + size + 10.0, oy + size + 10.0]).is_some()
    };
    let m0 = &ea.t0.made[0];
    let centre = key_at([m0.s.x, m0.s.y]);
    let mut differs = false;
    let mut same = 0;
    // A row of tiles through the village, out past its pad, and tiles along its track's road.
    let mut keys: Vec<TileKey> = (-12i64..=12).map(|d| TileKey::surface(level, (centre.x as i64 + d) as u32, centre.y)).collect();
    let end = *m0.track.last().unwrap();
    keys.extend((-6i64..=6).map(|d| TileKey::surface(level, (key_at(end).x as i64 + d) as u32, (key_at(end).y as i64 + d) as u32)));
    for key in keys {
        let (ca, c0) = (ea.battlemap(key), ex0.battlemap(key));
        if touched(&ea, &key) {
            differs |= print(&ca) != print(&c0);
        } else {
            assert_eq!(print(&ca), print(&c0), "battlemap {key:?} changed clear of the created settlements");
            same += 1;
        }
        assert!(ea.terrain(key.parent().unwrap()).padded == ex0.terrain(key.parent().unwrap()).padded, "terrain changed");
    }
    assert!(differs && same > 4, "the village's battlemap unchanged ({same} clear)");

    // Seamless round both: each chunk's east and south halo rows are its neighbours' first, bit for
    // bit (heights, water and roads), across the village and the town and along their links.
    use worldgen::battlemap::{HS, SQ};
    let mut seams = 0;
    for (k, m) in ea.t0.made.clone().iter().enumerate() {
        let c = key_at([m.s.x, m.s.y]);
        let mid = key_at(m.track[m.track.len() / 2]);
        let keys = (-2i64..=1).map(|d| TileKey::surface(level, (c.x as i64 + d) as u32, c.y)).chain([TileKey::surface(level, c.x, c.y - 1), TileKey::surface(level, c.x - 1, c.y - 1), mid]);
        for key in keys {
            let (a, east, south) = (ea.battlemap(key), ea.battlemap(TileKey::surface(level, key.x + 1, key.y)), ea.battlemap(TileKey::surface(level, key.x, key.y + 1)));
            for j in 0..SQ {
                assert_eq!(a.halo_height[(j + 1) * HS + SQ + 1].to_bits(), east.height[j * SQ].to_bits(), "c:{k} {key:?}: battlemap seam at row {j}");
                assert_eq!(a.halo_water[(j + 1) * HS + SQ + 1].to_bits(), east.water_level[j * SQ].to_bits(), "c:{k} {key:?}: water seam at row {j}");
                assert_eq!(a.road_halo[(j + 1) * HS + SQ + 1], east.road_halo[(j + 1) * HS + 1], "c:{k} {key:?}: road seam at row {j}");
                assert_eq!(a.halo_height[(SQ + 1) * HS + j + 1].to_bits(), south.height[j].to_bits(), "c:{k} {key:?}: battlemap seam at column {j}");
                assert_eq!(a.road_halo[(SQ + 1) * HS + j + 1], south.road_halo[HS + j + 1], "c:{k} {key:?}: road seam at column {j}");
            }
            seams += 2;
        }
    }
    assert!(seams >= 28, "{seams} seams checked round the created settlements");

    // Removed, the world is as it was (its place in the list kept).
    let mut r = a.clone();
    r.edits.created.iter_mut().for_each(|c| c.removed = true);
    let mut er = Executor::new(World::new(r).unwrap());
    assert!(er.t0.made.is_empty() && !town::is_settlement(&er.t0, first));
    assert_eq!(print(&er.battlemap(centre)), print(&ex0.battlemap(centre)), "removing the village left its ground");
    assert!(worldgen::t0::made::spot(&er.world, &er.t0, "village", "c:0", v).is_ok(), "its spot is free again");
    assert!(worldgen::t0::made::spot(&ea.world, &ea.t0, "village", "c:9", v).is_err(), "a village on a village");

    // Remade with a site drawn far away (one more of the world's own sites): each in its place,
    // with its culture and kind, laid out as before.
    let far = ex0.t0.settlements.iter().max_by(|p, q| geom_dist([p.x, p.y], v).total_cmp(&geom_dist([q.x, q.y], v))).map(|s| [s.x + 3_000.0, s.y]).unwrap();
    let mut b = a.clone();
    b.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "site", "kind": "ruin", "pts": [far] })).unwrap());
    let eb = Executor::new(World::new(b).unwrap());
    let fb = eb.t0.settlements.len() + eb.t0.base_pois;
    assert_eq!(fb, first + 1);
    for k in 0..2 {
        let (ma, mb) = (&ea.t0.made[k], &eb.t0.made[k]);
        assert_eq!((mb.s.x, mb.s.y, mb.s.culture, mb.s.kind, mb.s.tier, mb.s.population), (ma.s.x, ma.s.y, ma.s.culture, ma.s.kind, ma.s.tier, ma.s.population), "c:{k} changed when remade");
        assert_eq!(Some(ma.s.culture), a.edits.created[k].culture);
        let laid = |ex: &Executor, li: usize| {
            let mut l = (*town::layout(&ex.world, &ex.t0, li)).clone();
            l.index = 0;
            format!("{l:?}")
        };
        assert_eq!(laid(&ea, first + k), laid(&eb, fb + k), "c:{k}: laid out anew when the world was remade");
    }
}

/// A world remade from a changed sketch (a capital town pinned) keeps what was made in each of
/// its places only where that place came out the same (`t0::remap::remake`): a change keyed in a
/// layout lands in the new layout with the same fingerprint and identity, or is set aside at
/// `PARKED_LAYOUT` and up, each layout at a number of its own; none is lost or lands on another.
#[test]
fn a_remade_world_keeps_changes_on_their_places() {
    use worldgen::t0::{T0, remap};
    use worldgen::town::{layout_identity, layout_print};
    use worldgen::world::{Note, PARKED_LAYOUT, layout_in_id};
    let mut file: WorldFile = serde_json::from_str(&world_json(2)).unwrap();
    let old = World::new(file.clone()).unwrap();
    let t0 = T0::generate(&old);
    let first = t0.settlements.len() + t0.base_pois;
    let what = |w: &World, t0: &T0, li: usize| format!("{}:{}", layout_print(w, t0, li), layout_identity(t0, li));
    for li in 0..first {
        file.edits.notes.insert(format!("b:{li}:0"), Note { text: what(&old, &t0, li), ..Default::default() });
    }
    let town = t0.settlements.iter().find(|s| s.tier.name() == "town").expect("a town");
    let mut b = file.clone();
    b.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "pin", "tier": "town", "capital": true, "pts": [[town.x + 21_120.0, town.y + 15_840.0]] })).unwrap());
    let (old, new) = (World::new(file).unwrap(), World::new(b).unwrap());
    let (t1, followed) = remap::remake(&old, &t0, &new, &[]);
    let f = followed.expect("the remake moved changes");
    let (mut kept, mut parked) = (0, std::collections::BTreeSet::new());
    for (k, note) in &f.edits.notes {
        let li = layout_in_id(k).expect("a layout-local key");
        if li >= PARKED_LAYOUT {
            assert!(parked.insert(li), "{k}: two layouts set aside at one number");
        } else {
            assert_eq!(what(&new, &t1, li), note.text, "{k}: a change landed on a place that came out otherwise");
            kept += 1;
        }
    }
    assert_eq!(kept + parked.len(), first, "changes were lost");
    assert_eq!(f.parked, parked.len(), "the changes set aside were miscounted");
    assert!(kept > first / 2, "only {kept} of {first} places kept their changes");
}

/// (Roadmap 7, 183) A road drawn into a town changed in the ward editor leaves it as it was:
/// the remake keeps the edit on it (its patches, wards and gates as before, nothing set aside),
/// and the town is planned from the roads its edit kept.
#[test]
fn a_road_into_an_edited_town_keeps_its_edit() {
    use worldgen::t0::{T0, remap, settle::Tier};
    use worldgen::town::wards;
    let mut file: WorldFile = serde_json::from_str(&world_json(2)).unwrap();
    let w0 = World::new(file.clone()).unwrap();
    let t0 = T0::generate(&w0);
    let n = t0.settlements.len();
    let ci = (0..n).filter(|&i| t0.settlements[i].tier >= Tier::City).max_by_key(|&i| t0.settlements[i].population).expect("a city");
    let plan = wards::plan_json(&w0, &t0, ci).unwrap();
    let patch = plan["patches"].as_array().unwrap().iter().find(|p| p["in_town"] == true && p["ward"] == "common").expect("a common ward")["patch"].clone();
    let req = serde_json::from_str(&format!(r#"{{"patches":[{{"patch":{patch},"ward":"craft"}}]}}"#)).unwrap();
    let (edit, _) = wards::change(&w0, &t0, ci, &req).expect("the change");
    let edit = edit.expect("an edit");
    assert!(edit.roads.is_some(), "the edit kept no roads");
    file.edits.towns.insert(wards::key(ci), edit);
    let old = World::new(file.clone()).unwrap();
    let gates = |p: &serde_json::Value| p["corners"].as_array().unwrap().iter().filter(|c| !c["gate"].is_null()).map(|c| c["corner"].clone()).collect::<Vec<_>>();
    let before = wards::plan_json(&old, &t0, ci).unwrap();
    // A road from 5 mi off, from the side farthest from every gate.
    let s = &t0.settlements[ci];
    let ats: Vec<[f64; 2]> = before["corners"].as_array().unwrap().iter().filter(|c| !c["gate"].is_null()).map(|c| [c["at"][0].as_f64().unwrap() - s.x, c["at"][1].as_f64().unwrap() - s.y]).collect();
    let dir = (0..36)
        .map(|k| {
            let a = k as f64 * std::f64::consts::TAU / 36.0;
            [a.cos(), a.sin()]
        })
        .max_by(|a, b| {
            let near = |d: &[f64; 2]| ats.iter().map(|g| (g[0] * d[0] + g[1] * d[1]) / g[0].hypot(g[1])).fold(-1.0, f64::max);
            near(b).total_cmp(&near(a))
        })
        .unwrap();
    let far = 5.0 * 5280.0;
    let mut b = file.clone();
    b.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "road", "kind": "road", "pts": [[s.x + far * dir[0], s.y + far * dir[1]], [s.x + 0.5 * far * dir[0], s.y + 0.5 * far * dir[1]], [s.x + 300.0 * dir[0], s.y + 300.0 * dir[1]]] })).unwrap());
    let new = World::new(b).unwrap();
    let (t1, followed) = remap::remake(&old, &t0, &new, &[]);
    assert!(t1.roads.roads.len() != t0.roads.roads.len() || t1.roads.roads.iter().zip(&t0.roads.roads).any(|(a, b)| a.pts != b.pts), "the road changed no network road");
    let edits = followed.map_or_else(|| old.file.edits.clone(), |f| f.edits);
    let li = (0..t1.settlements.len()).find(|&i| (t1.settlements[i].x - s.x).abs() < 1.0 && (t1.settlements[i].y - s.y).abs() < 1.0).expect("the city stands");
    assert!(edits.towns.contains_key(&wards::key(li)), "the town's edit was set aside");
    let mut f2 = new.file.clone();
    f2.edits = edits;
    let w2 = World::new(f2).unwrap();
    let after = wards::plan_json(&w2, &t1, li).unwrap();
    assert_eq!(after["set_aside"], serde_json::json!([]), "parts of the edit were set aside");
    let ward = after["patches"].as_array().unwrap().iter().find(|p| p["patch"] == patch).unwrap()["ward"].clone();
    assert_eq!(ward, "craft", "the patch lost its ward");
    assert_eq!(gates(&after), gates(&before), "the town's gates moved");
    assert_eq!(after["districts"], before["districts"], "the districts changed");
}

/// A settlement a sketch pin makes takes the changes of a place only at its own spot (M11, M10):
/// a pin put on a town to change it (`Stroke::replaces`: a fishing village now) takes the town's
/// place, its layout number and its rename and notes, and moves no other settlement; a new pin far
/// from a town that vanished takes nothing of it, and the name a pin gives wins over a rename.
#[test]
fn a_pin_takes_only_the_changes_at_its_spot() {
    use worldgen::t0::remap::{self, Place, Places, Pinned};
    use worldgen::t0::{T0, settle::SettleKind};
    use worldgen::world::Note;
    // A pin on a town: the same place, a fishing village now.
    let mut file: WorldFile = serde_json::from_str(&world_json(2)).unwrap();
    let old = World::new(file.clone()).unwrap();
    let t0 = T0::generate(&old);
    let li = t0.settlements.iter().position(|s| s.tier.name() == "town" && s.planned.name() == "town" && s.coastal && s.pin.is_none()).expect("a town by the sea");
    let s = &t0.settlements[li];
    let id = worldgen::agent::feature_of_layout(&old, &t0, li).unwrap();
    file.edits.renames.insert(id.clone(), "Keepsake".into());
    file.edits.notes.insert(id.clone(), Note { text: "the harbour master owes".into(), ..Default::default() });
    let cell = [(s.cell % t0.height.w) as f64 * t0.cell_ft, (s.cell / t0.height.w) as f64 * t0.cell_ft];
    let mut b = file.clone();
    b.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "pin", "tier": "village", "kind": "fishing", "replaces": "town", "pts": [cell] })).unwrap());
    let (old, new) = (World::new(file).unwrap(), World::new(b).unwrap());
    let (t1, followed) = remap::remake(&old, &t0, &new, &[]);
    assert_eq!(t1.settlements.len(), t0.settlements.len(), "a pin in place of a town changed the number of settlements");
    let v = &t1.settlements[li];
    assert!(v.pin == Some(0) && v.tier.name() == "village" && v.kind == SettleKind::Fishing, "the pin's village did not take the town's place");
    let far = |a: &worldgen::t0::settle::Settlement, b: &worldgen::t0::settle::Settlement| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
    assert!(far(v, s) < 3.0 * 5280.0, "the pin's village stands {:.0} ft from the town", far(v, s));
    assert!(t1.settlements.iter().all(|o| o.pin.is_some() || far(o, s) > 3.0 * 5280.0), "a settlement still stands where the town was");
    let now = worldgen::agent::feature_of_layout(&new, &t1, li).unwrap();
    let e = followed.expect("the town's changes followed it").edits;
    assert_eq!(e.renames.get(&now).map(String::as_str), Some("Keepsake"), "the rename stayed behind ({id} -> {now})");
    assert!(e.notes.contains_key(&now), "the notes stayed behind");
    // A new pin far from a vanished town, and one on the spot of a vanished village.
    let place = |id: &str, kind: &str, x: f64, extent_mi: f64| Place { id: id.into(), kind: kind.into(), x, y: 0.0, extent_ft: extent_mi * 5280.0, name: String::new() };
    let before = Places { features: vec![place("town:a", "town", 0.0, 12.0), place("village:b", "village", 400_000.0, 5.0)], ..Default::default() };
    let pinned = vec![Pinned { id: "town:n".into(), name: Some("Newhold".into()) }, Pinned { id: "town:m".into(), name: Some("Highmoor".into()) }];
    let after = Places { features: vec![place("town:n", "town", 132_000.0, 12.0), place("town:m", "town", 401_000.0, 12.0)], pinned, ..Default::default() };
    let mut edits = worldgen::world::Edits::default();
    for (k, n) in [("town:a", "Keepsake"), ("village:b", "Lowmere")] {
        edits.renames.insert(k.into(), n.into());
        edits.notes.insert(k.into(), Note { text: n.into(), ..Default::default() });
    }
    let f = remap::follow(&edits, &before, &after, &|_, _| false, &[]).expect("the village's changes moved");
    assert!(!f.edits.renames.contains_key("town:n") && !f.edits.notes.contains_key("town:n"), "a pin 25 mi off took a vanished town's changes");
    let kept_as = |id: &str| f.vanished.iter().find(|v| v.id == id).map(|v| v.at.clone()).unwrap_or_default();
    assert_eq!(f.edits.renames.get(&kept_as("town:a")).map(String::as_str), Some("Keepsake"), "the vanished town's rename was lost");
    assert!(f.edits.notes.contains_key("town:m"), "the pin on the village's spot didn't take its notes");
    assert!(!f.edits.renames.contains_key("town:m") && f.edits.renames.get(&kept_as("village:b")).map(String::as_str) == Some("Lowmere"), "a rename won over the pin's name");
}

/// A remade world never drops a change without saying so (M11): a change on a feature the remade
/// world has no more, with nothing in its place, is set aside under a `gone:` id (listed with what
/// was made on it, as a settlement taken away is), never left on an id that names nothing nor
/// handed to a neighbour that stood there before; and
/// when the feature comes back (the stroke taken back), its changes come back to it.
#[test]
fn a_remade_world_never_drops_a_change() {
    use worldgen::t0::remap::{self, Place, Places};
    use worldgen::world::{Note, NpcPlace};
    let place = |id: &str, kind: &str, x: f64, name: &str| Place { id: id.into(), kind: kind.into(), x, y: 0.0, extent_ft: 6.0 * 5280.0, name: name.into() };
    let before = Places { features: vec![place("town:a", "town", 0.0, "Ashford"), place("village:b", "village", 100_000.0, "Brindle"), place("range:c", "range", 300_000.0, "Grey Peaks"), place("town:y", "town", 30_000.0, "Yarrow")], ..Default::default() };
    // (Everything vanished: the nearest new town of the remade world is 115 mi off; the town 6 mi
    // off stood there before, a neighbour.)
    let after = Places { features: vec![place("town:z", "town", 607_200.0, "Zell"), place("town:y", "town", 30_000.0, "Yarrow")], ..Default::default() };
    let mut edits = worldgen::world::Edits::default();
    edits.renames.insert("town:a".into(), "Keepsake".into());
    edits.notes.insert("village:b".into(), Note { text: "the miller lies".into(), ..Default::default() });
    edits.hidden.insert("range:c".into());
    let mut npc: worldgen::world::Npc = serde_json::from_value(serde_json::json!({ "name": "Mara" })).unwrap();
    npc.location = Some(NpcPlace { id: "village:b".into(), ..Default::default() });
    edits.npcs.insert("n1".into(), npc);
    let f = remap::follow(&edits, &before, &after, &|_, _| false, &[]).expect("changes on vanished places were left as they were");
    let named = |e: &worldgen::world::Edits| {
        let mut ids: Vec<String> = e.renames.keys().chain(e.notes.keys()).chain(e.hidden.iter()).cloned().collect();
        ids.extend(e.npcs.values().filter_map(|n| n.location.as_ref().map(|l| l.id.clone())));
        ids
    };
    for id in named(&f.edits) {
        assert!(id.starts_with("gone:") || after.features.iter().any(|p| p.id == id), "{id}: a change left on a place that is gone, not set aside");
    }
    assert!(!f.edits.renames.contains_key("town:y"), "a neighbour that stood there before took a vanished town's name");
    let what: Vec<(&str, &str, Vec<String>)> = f.vanished.iter().map(|v| (v.id.as_str(), v.name.as_str(), v.what.clone())).collect();
    assert_eq!(what, vec![("range:c", "Grey Peaks", vec!["hidden".to_string()]), ("town:a", "Keepsake", vec!["renamed".to_string()]), ("village:b", "Brindle", vec!["note".to_string(), "1 NPC".to_string()])], "the vanished places were not all listed");
    assert_eq!(f.aside, 4, "the changes set aside were miscounted");
    let b = &f.vanished.iter().find(|v| v.id == "village:b").unwrap().at;
    assert!(f.edits.notes.contains_key(b) && f.edits.renames.get(b).map(String::as_str) == Some("Brindle"), "the village's note lost what it was on");
    // Taken back: each comes back to its place, the name kept to say what it was on gone.
    let g = remap::follow(&f.edits, &after, &before, &|_, _| false, &[]).expect("nothing came back");
    assert_eq!(g.edits.renames, edits.renames, "the names did not come back as they were");
    assert_eq!((g.edits.notes.keys().collect::<Vec<_>>(), &g.edits.hidden), (edits.notes.keys().collect::<Vec<_>>(), &edits.hidden), "the notes or hidden marks did not come back");
    assert_eq!(g.edits.npcs["n1"].location.as_ref().map(|l| l.id.as_str()), Some("village:b"), "the NPC did not come back");
    assert!(g.vanished.is_empty() && g.kept == 4, "{} came back, {:?} set aside", g.kept, g.vanished);
}

// R5 M20

/// R5 M20: generated raised ground (a big port's breakwater, a river islet: the first generated
/// input to `ground::Shaper`) holds the seam invariant `ground_edits` checks for hand edits: the
/// breakwater's made ground is dry at its level on the battlemap and in the finest tile's samples
/// over it; the chunks round it and round the islet agree at their edges bit for bit, as do the
/// finest tiles under the breakwater; and every lighthouse's and islet tower's front door opens
/// onto dry ground it can be walked to (a lighthouse's from the bank over its breakwater).
#[test]
fn harbour_ground() {
    use worldgen::battlemap::{Chunk, HS, SQ, SQUARE_FT, URBAN_TOWER, URBAN_WALL};
    use worldgen::core::tile::{PADDED, TILE_SAMPLES};
    use worldgen::town::{self, geom, harbour};
    let mut ex = Executor::new(World::new(serde_json::from_str(&world_json(1)).unwrap()).unwrap());
    let g = ex.world.geom.clone();
    let lvl = g.max_level;
    let size = g.tile_size_ft(lvl);
    let key_at = |p: [f64; 2]| TileKey::surface(lvl, (p[0] / size) as u32, (p[1] / size) as u32);
    let ring = |a: &Chunk, b: &Chunk, east: bool, what: &str| {
        for t in 0..SQ {
            let (ah, bk) = if east { ((t + 1) * HS + SQ + 1, t * SQ) } else { ((SQ + 1) * HS + t + 1, t) };
            let (bh, ak) = if east { ((t + 1) * HS, t * SQ + SQ - 1) } else { (t + 1, (SQ - 1) * SQ + t) };
            assert_eq!(a.halo_height[ah].to_bits(), b.height[bk].to_bits(), "{what}: seam at {t} ({})", if east { "east" } else { "south" });
            assert_eq!(b.halo_height[bh].to_bits(), a.height[ak].to_bits(), "{what}: seam at {t} ({})", if east { "west" } else { "north" });
            assert_eq!(a.halo_water[ah].to_bits(), b.water_level[bk].to_bits(), "{what}: water seam at {t}");
        }
    };
    let seams = |ex: &mut Executor, k: TileKey, what: &str| {
        let c = ex.battlemap(k);
        for (q, (dx, dy)) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
            let n = ex.battlemap(TileKey::surface(lvl, (k.x as i32 + dx) as u32, (k.y as i32 + dy) as u32));
            match q {
                0 => ring(&c, &n, true, what),
                1 => ring(&n, &c, true, what),
                2 => ring(&c, &n, false, what),
                _ => ring(&n, &c, false, what),
            }
        }
    };
    // Walked from `from` (world ft) over dry open squares, steps of at most 2.5 ft, within `reach`
    // ft: whether a square `goal` holds is reached, and how many squares are.
    let walk = |ex: &mut Executor, from: [f64; 2], reach: f64, goal: &dyn Fn([f64; 2]) -> bool| -> (bool, usize) {
        let k0 = key_at([from[0] - reach, from[1] - reach]);
        let k1 = key_at([from[0] + reach, from[1] + reach]);
        let (cw, ch) = ((k1.x - k0.x + 1) as usize, (k1.y - k0.y + 1) as usize);
        let (nx, ny) = (cw * SQ, ch * SQ);
        let (ox, oy) = (k0.x as f64 * size, k0.y as f64 * size);
        let mut h = vec![f32::NAN; nx * ny];
        for cj in 0..ch {
            for ci in 0..cw {
                let c = ex.battlemap(TileKey::surface(lvl, k0.x + ci as u32, k0.y + cj as u32));
                for q in 0..SQ * SQ {
                    if c.height[q] > c.water_level[q] && c.building[q] == 0 && !matches!(c.urban[q], URBAN_WALL | URBAN_TOWER) {
                        h[(cj * SQ + q / SQ) * nx + ci * SQ + q % SQ] = c.height[q];
                    }
                }
            }
        }
        let at = |a: usize| [ox + ((a % nx) as f64 + 0.5) * SQUARE_FT, oy + ((a / nx) as f64 + 0.5) * SQUARE_FT];
        let start = (((from[1] - oy) / SQUARE_FT) as usize) * nx + ((from[0] - ox) / SQUARE_FT) as usize;
        if h[start].is_nan() {
            return (false, 0);
        }
        let mut seen = vec![false; nx * ny];
        seen[start] = true;
        let mut queue = std::collections::VecDeque::from([start]);
        let mut n = 0;
        while let Some(a) = queue.pop_front() {
            n += 1;
            if goal(at(a)) {
                return (true, n);
            }
            let (i, j) = ((a % nx) as i64, (a / nx) as i64);
            for (di, dj) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni >= nx as i64 || nj >= ny as i64 {
                    continue;
                }
                let b = nj as usize * nx + ni as usize;
                if !seen[b] && !h[b].is_nan() && (h[b] - h[a]).abs() <= 2.5 && geom_dist(at(b), from) < reach {
                    seen[b] = true;
                    queue.push_back(b);
                }
            }
        }
        (false, n)
    };
    // The square outside a building's front door (world ft).
    let door = |ex: &Executor, si: usize, id: u32| -> [f64; 2] {
        let it = worldgen::interior::generate(&ex.world, &ex.t0, si, id as usize).expect("an interior");
        let lv = &it.levels[it.entry_level];
        let d = lv.doors.iter().find(|d| d.kind == "front").expect("a front door");
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        let out = sq.into_iter().find(|&(i, j)| i < 0 || j < 0 || i >= it.nx as isize || j >= it.ny as isize || lv.cells[j as usize * it.nx + i as usize] < 0).expect("a door on an outside wall");
        worldgen::interior::to_world(&it, out.0 as f64 + 0.5, out.1 as f64 + 0.5)
    };
    // A big port's breakwater: its made ground dry at its level over its middle.
    let (si, l) = (0..ex.t0.settlements.len())
        .filter(|&i| ex.t0.settlements[i].tier >= worldgen::t0::settle::Tier::City)
        .map(|i| (i, town::layout(&ex.world, &ex.t0, i)))
        .find(|(_, l)| !l.moles.is_empty())
        .expect("a big port with a breakwater");
    let mole = l.moles[0].clone();
    let polys = mole.polys();
    let inner = |p: [f64; 2]| polys.iter().any(|pl| geom::contains(pl, p) && geom::poly_gap(pl, &[p]) >= 2.5);
    let n = mole.pts.len();
    let mid = geom::lerp(mole.pts[n / 2 - 1], mole.pts[n / 2], 0.5);
    let key = key_at(mid);
    let c = ex.battlemap(key);
    let mut on = 0;
    for k2 in 0..SQ * SQ {
        let p = [key.x as f64 * size + ((k2 % SQ) as f64 + 0.5) * SQUARE_FT, key.y as f64 * size + ((k2 / SQ) as f64 + 0.5) * SQUARE_FT];
        if !inner(p) || c.building[k2] != 0 {
            continue;
        }
        let h = (k2 / SQ + 1) * HS + k2 % SQ + 1;
        assert!(c.water_level[k2] < c.height[k2], "L{si}: breakwater square {p:?} is under water");
        assert!((c.halo_height[h] - c.roof_lift[h] - mole.level).abs() <= 0.1, "L{si}: breakwater square {p:?} at {}, its level {}", c.halo_height[h] - c.roof_lift[h], mole.level);
        on += 1;
    }
    assert!(on >= 20, "L{si}: too few breakwater squares in chunk {key:?} ({on})");
    seams(&mut ex, key, "a breakwater's chunk");
    // The finest tiles under it: its samples inside it at its level, and their shared edges alike.
    let bb = mole.bbox();
    let ts = size;
    let mut drawn: Vec<(TileKey, Vec<f32>)> = Vec::new();
    for ty in (bb[1] / ts) as u32..=(bb[3] / ts) as u32 {
        for tx in (bb[0] / ts) as u32..=(bb[2] / ts) as u32 {
            let k = TileKey::surface(lvl, tx, ty);
            let tile = ex.terrain(k);
            let h = worldgen::payload::shaped_heights(&ex.world, &ex.t0, &k, &tile).expect("the breakwater shapes the finest tile under it");
            let s = g.spacing_ft(lvl);
            for j in 0..TILE_SAMPLES {
                for i in 0..TILE_SAMPLES {
                    let p = [(tx as usize * TILE_N + i) as f64 * s, (ty as usize * TILE_N + j) as f64 * s];
                    if inner(p) && polys.iter().all(|pl| geom::poly_gap(pl, &[p]) >= 5.0 || !geom::contains(pl, p)) {
                        let v = h[(j + HALO) * PADDED + i + HALO];
                        assert!((v - mole.level).abs() < 0.01, "tile {k:?}: sample at {p:?} is {v}, the breakwater's level {}", mole.level);
                    }
                }
            }
            drawn.push((k, h));
        }
    }
    for (a, ha) in &drawn {
        for (b, hb) in &drawn {
            for east in [true, false] {
                if (east && b.x == a.x + 1 && b.y == a.y) || (!east && b.y == a.y + 1 && b.x == a.x) {
                    for t in 0..TILE_SAMPLES {
                        let (ia, ib) = if east { ((t + HALO) * PADDED + TILE_N + HALO, (t + HALO) * PADDED + HALO) } else { ((TILE_N + HALO) * PADDED + t + HALO, HALO * PADDED + t + HALO) };
                        assert_eq!(ha[ia].to_bits(), hb[ib].to_bits(), "a breakwater's tiles: seam {a:?} | {b:?} at {t}");
                    }
                }
            }
        }
    }
    // Its lighthouse: the door reached from the bank over the breakwater.
    let tip = *mole.pts.last().unwrap();
    let lh = l.buildings.iter().find(|b| harbour::is_lighthouse(b) && geom_dist(geom::mean(&b.poly), tip) < 2.0).expect("a lighthouse on the breakwater's head");
    let mut fill = polys.clone();
    fill.extend(mole.ramp().into_iter().map(|r| r.0));
    let from = door(&ex, si, lh.id);
    // (Within the breakwater's length of the door, and a little more.)
    let reach = 100.0 + mole.pts.windows(2).map(|w| geom_dist(w[0], w[1])).sum::<f64>().max(300.0);
    let (ok, n) = walk(&mut ex, from, reach, &|p| fill.iter().all(|pl| !geom::contains(pl, p) && geom::poly_gap(pl, &[p]) > 15.0));
    assert!(ok, "L{si}: the lighthouse b:{si}:{} isn't reached from the bank ({n} squares walked)", lh.id);
    // Islets: a river islet's chunk agrees with its neighbours; every tower's door opens onto its top.
    let islets: Vec<_> = worldgen::t0::islets::islets(&ex.t0).to_vec();
    let river = islets.iter().find(|i| i.river).expect("a river islet");
    seams(&mut ex, key_at(river.at), "a river islet's chunk");
    let mut towers = 0;
    for isl in islets.iter().filter(|i| i.tower) {
        let l = town::layout(&ex.world, &ex.t0, isl.settlement as usize);
        let Some(t) = l.buildings.iter().find(|b| town::is_watchtower(b) && geom::contains(&isl.pts, geom::mean(&b.poly))) else { continue };
        let from = door(&ex, isl.settlement as usize, t.id);
        let (_, n) = walk(&mut ex, from, 400.0, &|_| false);
        assert!(n >= 4, "the islet tower b:{}:{} opens onto {n} dry squares", isl.settlement, t.id);
        towers += 1;
    }
    assert!(towers >= 1, "no islet tower checked");
}

/// One of the world's own settlements laid out in another culture (`Edits.cultures`,
/// `town::culture`) moves nothing else: T0 and the layouts round it stay byte for byte; it is
/// laid out anew in that culture (a cached copy as a fresh one); the edits inside it are set
/// aside while its own name stays; asking again changes nothing; and changed back it is laid out
/// as before. Its fingerprint for a remake (`layout_print`) is its generated one.
#[test]
fn a_culture_change_moves_nothing_else() {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, culture};
    use worldgen::world::{BuildingEdit, PARKED_LAYOUT};
    let file: WorldFile = serde_json::from_str(&world_json(1)).unwrap();
    let mut ex = Executor::new(World::new(file.clone()).unwrap());
    let li = (0..ex.t0.settlements.len()).find(|&i| ex.t0.settlements[i].tier == Tier::Town && ex.t0.settlements[i].culture != 4).expect("a town not of the steppe");
    let was = ex.t0.settlements[li].culture;
    let laid = |ex: &Executor, i: usize| format!("{:?}", *town::layout(&ex.world, &ex.t0, i));
    let at = |ex: &Executor, i: usize| match town::settlement(&ex.t0, i) {
        Some(s) => [s.x, s.y],
        None => [ex.t0.pois[i - ex.t0.settlements.len()].x, ex.t0.pois[i - ex.t0.settlements.len()].y],
    };
    let here = at(&ex, li);
    let mut near: Vec<usize> = (0..town::layout_count(&ex.t0)).filter(|&i| i != li).collect();
    near.sort_by(|&a, &b| geom_dist(at(&ex, a), here).total_cmp(&geom_dist(at(&ex, b), here)).then(a.cmp(&b)));
    near.truncate(12);
    near.extend(town::sites_in(&ex.t0, li));
    let (bytes0, laid0) = (ex.t0.to_bytes(), laid(&ex, li));
    let others0: Vec<String> = near.iter().map(|&i| laid(&ex, i)).collect();
    let print0 = town::layout_print(&ex.world, &ex.t0, li);

    // Its own name and a building changed inside it, then the culture changed.
    let fid = worldgen::agent::feature_of_layout(&ex.world, &ex.t0, li).expect("its feature");
    let b0 = town::layout(&ex.world, &ex.t0, li).buildings[0].poly.clone();
    let mut e = ex.world.file.edits.clone();
    e.renames.insert(fid.clone(), "Keepsake".into());
    e.buildings.insert(format!("b:{li}:0"), BuildingEdit { at: b0.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / b0.len() as f64, a[1] + p[1] / b0.len() as f64]), removed: true, ..Default::default() });
    let c = culture::change(&mut e, &ex.t0, li, 4).unwrap();
    assert!(!c.unchanged && c.from == was && c.to == 4 && c.aside == vec![("buildings", 1)], "{c:?}");
    assert!(e.renames.get(&fid).is_some_and(|n| n == "Keepsake") && e.buildings.keys().all(|k| k.starts_with(&format!("b:{PARKED_LAYOUT}:"))), "its name moved or a building edit stayed");
    assert_eq!(e.cultures.get(&li.to_string()), Some(&4));
    ex.world.file.edits = e.clone();
    let l = town::layout(&ex.world, &ex.t0, li);
    assert!(l.culture == 4 && !l.yurts.is_empty() && laid(&ex, li) != laid0, "not laid out anew in the steppe's culture (yurts and all)");
    let fresh = Executor::new(World::new(WorldFile { edits: e.clone(), ..file.clone() }).unwrap());
    assert!(fresh.t0.to_bytes() == bytes0 && ex.t0.to_bytes() == bytes0, "T0 changed");
    assert_eq!(laid(&fresh, li), laid(&ex, li), "the cached layout is not the one laid out afresh");
    for (k, &i) in near.iter().enumerate() {
        assert!(laid(&ex, i) == others0[k] && laid(&fresh, i) == others0[k], "layout {i} changed with layout {li}'s culture");
    }
    assert_eq!(town::layout_print(&ex.world, &ex.t0, li), print0, "the remake's fingerprint follows the culture");

    // Again: nothing; back: as it was generated.
    assert!(culture::change(&mut e, &ex.t0, li, 4).unwrap().unchanged);
    let c = culture::change(&mut e, &ex.t0, li, was).unwrap();
    assert!(!c.unchanged && c.aside.is_empty() && e.cultures.is_empty(), "{c:?}");
    ex.world.file.edits = e;
    assert_eq!(laid(&ex, li), laid0, "changed back, it is not laid out as before");
}
