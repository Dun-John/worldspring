//! Interiors: the interiors guarantee and designed buildings.

use super::*;

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
        // (A study big enough for the pieces put down in it below, drawn round.)
        if cells.iter().filter(|&&c| c == study).count() < 9 {
            continue;
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
    // (The study keeps the pieces the room had; a study they fill is cleared first, as a designer would.)
    let nx = d.nx as usize;
    let in_study = |f: &design::DesignItem| (f.y..f.y + f.h).any(|y| (f.x..f.x + f.w).any(|x| cells[y as usize * nx + x as usize] == study));
    for kind in ["sacks", "s:k3ttle"] {
        let (back, variant) = if kind == "sacks" { (None, Some(3)) } else { (Some(1), None) };
        let put = |d: &design::SiteDesign| {
            (0..cells.len()).filter(|&k| cells[k] == study).find_map(|k| {
                let mut t = d.clone();
                t.levels[g].items.push(design::DesignItem { kind: kind.into(), x: (k % nx) as u16, y: (k / nx) as u16, w: 1, h: 1, back, variant });
                t.problems(&ws, t0, &id).unwrap().1.iter().all(|p| !p.blocking).then_some(t)
            })
        };
        let put = put(&d).or_else(|| {
            let mut t = d.clone();
            t.levels[g].items.retain(|f| !in_study(f));
            put(&t)
        });
        d = put.unwrap_or_else(|| panic!("{id}: no room in the study for {kind}"));
    }
    // (A ballista on any free square of the ground floor.)
    for (kind, variant) in [("ballista", None)] {
        let put = (0..cells.len()).filter(|&k| cells[k] >= 0).find_map(|k| {
            let mut t = d.clone();
            t.levels[g].items.push(design::DesignItem { kind: kind.into(), x: (k % nx) as u16, y: (k / nx) as u16, w: 1, h: 1, back: None, variant });
            t.problems(&ws, t0, &id).unwrap().1.iter().all(|p| !p.blocking).then_some(t)
        });
        d = put.unwrap_or_else(|| panic!("{id}: no square for a {kind}"));
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
