//! Names and notes: positions, ids, NPC and plot rows, where things are, place names.

use super::*;
pub use crate::model::ids::{split_room_id, split_point_id};

/// The created site a layout id was deleted with: `b:<layout>:..`, `d:`, `t:`, `u:`, `k:`, `w:`, `l:`
/// and `r:` ids of a site whose `c:` entry is removed (layout numbers stay, so the ids are retired).
pub fn deleted_with(world: &World, t0: &T0, id: &str) -> Option<String> {
    let k = crate::world::layout_in_id(id)?.checked_sub(t0.settlements.len() + t0.base_pois)?;
    world.file.edits.created.get(k).filter(|c| c.removed).map(|c| c.id.clone())
}

/// What to say of an id that was deleted: a created site (`c:<n>`) or what lay in its layout.
pub fn deleted_note(world: &World, t0: &T0, id: &str) -> Option<String> {
    if let Some(cid) = deleted_with(world, t0, id) {
        return Some(format!("{id} was deleted with {cid} (restore_feature brings it back)"));
    }
    world.file.edits.created.iter().any(|c| c.id == id && c.removed).then(|| format!("{id} was deleted (restore_feature brings it back)"))
}

/// The layout index of a settlement or site feature (by overlay order; created sites follow
/// the generated ones).
pub fn layout_of(world: &World, t0: &T0, id: &str) -> Option<usize> {
    if let Some(k) = world.file.edits.created.iter().position(|c| c.id == id) {
        return Some(t0.settlements.len() + t0.base_pois + k);
    }
    let fs = place_ids(t0);
    let f = fs.iter().find(|f| f.0 == id)?;
    if SETTLEMENT_KINDS.contains(&f.1) {
        fs.iter().filter(|g| SETTLEMENT_KINDS.contains(&g.1)).position(|g| g.0 == id)
    } else if SITE_KINDS.contains(&f.1) {
        fs.iter().filter(|g| SITE_KINDS.contains(&g.1)).position(|g| g.0 == id).map(|k| t0.settlements.len() + k)
    } else {
        None
    }
}

/// One of the world's own settlements as its sketch pin would take it (M10, `update_settlement`
/// and the app's card): its layout, the pin that made it (a stroke index), its tier now and as
/// placed, what it lives by (a `PIN_KINDS` key), its name now, its cell centre (the pin's spot),
/// capital, and whether a port or a fishing village may stand there (sea or lake within reach,
/// as a pin needs; fishing also on a river).
pub fn settlement_facts(world: &World, t0: &T0, id: &str) -> Result<Value, String> {
    use crate::t0::settle::SettleKind;
    let li = layout_of(world, t0, id).filter(|&li| li < t0.settlements.len()).ok_or_else(|| format!("id: a settlement (a metropolis, city, town or village; search_features finds them), got {id:?}"))?;
    let s = &t0.settlements[li];
    let name = world.file.edits.renames.get(id).cloned().or_else(|| t0.extra.as_ref()?.overlay.features.iter().find(|f| f.id == id).map(|f| f.name.clone())).unwrap_or_default();
    let gw = t0.height.w;
    let spot = [(s.cell % gw) as f64 * t0.cell_ft, (s.cell / gw) as f64 * t0.cell_ft];
    let wet = (1..=4).any(|r| {
        (0..24).any(|q| {
            let ang = core::f64::consts::TAU * q as f64 / 24.0;
            let d = 0.4 * r as f64 * t0.cell_ft;
            t0.sample_lake(spot[0] + d * libm::cos(ang), spot[1] + d * libm::sin(ang)).1 >= 0.5
        })
    });
    let shore = wet || s.coastal || matches!(s.kind, SettleKind::Port | SettleKind::Fishing);
    Ok(json!({
        "layout": li, "pin": s.pin, "tier": s.tier.name(), "planned": s.planned.name(), "kind": crate::world::PIN_KINDS[s.kind as usize],
        "name": name, "spot": spot, "capital": s.capital, "port_ok": shore, "fishing_ok": shore || s.river,
    }))
}

/// The feature id of layout `li` (a settlement or site).
pub fn feature_of_layout(world: &World, t0: &T0, li: usize) -> Option<String> {
    let n = t0.settlements.len();
    if li >= n + t0.base_pois {
        return world.file.edits.created.get(li - n - t0.base_pois).map(|c| c.id.clone());
    }
    let fs = place_ids(t0);
    if li < n {
        fs.into_iter().filter(|g| SETTLEMENT_KINDS.contains(&g.1)).nth(li).map(|f| f.0)
    } else {
        fs.into_iter().filter(|g| SITE_KINDS.contains(&g.1)).nth(li - n).map(|f| f.0)
    }
}

pub fn name_of(world: &World, t0: &T0, id: &str) -> Option<String> {
    if let Some(n) = world.file.edits.renames.get(id) {
        return Some(n.clone());
    }
    features(world, t0).into_iter().find(|f| f.id == id).map(|f| f.name).or_else(|| place_name(t0, id))
}

/// A settlement's or site's generated name from `set_place_names`, when T0 has no overlay.
fn place_name(t0: &T0, id: &str) -> Option<String> {
    if t0.extra.is_some() {
        return None;
    }
    PLACES.with(|p| p.borrow().iter().find(|f| f.0 == id).map(|f| f.2.clone()))
}

/// A created entrance's building as `{id, name}`, and whether it still has the way down (a building gone, ruined or
/// without a cellar leaves a plain surface opening).
pub(super) fn in_building_json(world: &World, t0: &T0, c: &crate::world::Created) -> Value {
    let id = c.in_building.clone().unwrap_or_default();
    let name = building_label(world, t0, &id);
    let mut v = json!({ "id": id, "name": name });
    if crate::sitelinks::host_of(world, t0, c).is_none() {
        v["standing"] = json!(false);
        v["note"] = json!("that building is gone, or has no cellar now: the entrance shows as an opening on the surface");
    }
    v
}

/// Building `id`'s (`b:<layout>:<n>`) name as `get_feature` gives it: renamed, its own, else its trade. A building taken away keeps the name it
/// had as generated.
pub fn building_label(world: &World, t0: &T0, id: &str) -> Option<String> {
    let (li, bi) = crate::sitelinks::parse_building(id).filter(|(li, _)| *li < town::layout_count(t0))?;
    let l = town::layout(world, t0, li);
    if let Some(b) = l.building(bi) {
        return Some(building_name(world, &l, b));
    }
    let l = town::base_layout(world, t0, li);
    l.building(bi).map(|b| building_name(world, &l, b))
}

/// The id of the level of building `host` whose way down leads to the site of layout `el` (`u:<el>:0`), if it has one.
pub fn way_down_level(world: &World, t0: &T0, host: &str, el: usize) -> Option<String> {
    let it = crate::interior::generate_id(world, t0, host)?;
    let to = format!("u:{el}:0");
    it.levels.iter().position(|lv| lv.links.iter().any(|k| k.to == to)).map(|li| format!("l:{}:{li}", it.id))
}

/// A building's display name (renamed, its own, or its trade).
pub(super) fn building_name(world: &World, l: &town::Layout, b: &town::Building) -> String {
    let id = format!("b:{}:{}", l.index, b.id);
    world.file.edits.renames.get(&id).cloned().or_else(|| b.name.clone()).unwrap_or_else(|| b.label().to_string())
}

/// Where an id is (ft), for features, buildings, districts, towers, underground sites.
pub fn position(world: &World, t0: &T0, id: &str) -> Option<P> {
    if deleted_with(world, t0, id).is_some() {
        return None;
    }
    let edits = &world.file.edits;
    // An NPC is where they were placed; a plot point at its first place.
    if let Some(n) = edits.npcs.get(id) {
        let l = n.location.as_ref()?;
        return match (l.x, l.y) {
            (Some(x), Some(y)) => Some([x, y]),
            _ => position(world, t0, &l.id).filter(|_| !is_authored(&l.id)),
        };
    }
    if let Some(p) = edits.plots.get(id) {
        return p.anchors.iter().filter(|a| !is_authored(a)).find_map(|a| position(world, t0, a));
    }
    // A road stretch halfway along; a crossing put down by hand at its middle.
    if id.starts_with("rd:") {
        return crate::roadids::road_index(t0, id).map(|i| crate::roadids::midpoint(&t0.roads.roads[i]));
    }
    if let Some(c) = edits.crossings.get(id) {
        return Some(crate::roadids::crossing_mid(c));
    }
    if let Some(w) = edits.wharves.get(id) {
        return Some(crate::wharf::mid(w));
    }
    // A link put down by hand at its first end.
    if let Some(l) = edits.links.get(id) {
        let (site, li) = crate::links::site_of(&l.a.level)?;
        let it = crate::interior::generate_id(world, t0, site)?;
        it.levels.get(li)?;
        return Some(crate::interior::to_world(&it, l.a.x as f64 + 0.5, l.a.y as f64 + 0.5));
    }
    // A point note at its spot (on its level, which must be there).
    if let Some((site, li, _)) = split_point_id(id) {
        let at = edits.notes.get(id)?.at?;
        let it = crate::interior::generate_id(world, t0, site)?;
        it.levels.get(li)?;
        return Some(crate::interior::to_world(&it, at[0], at[1]));
    }
    // A level is where its site is; a room at its middle.
    if let Some((site, li, ri)) = split_room_id(id) {
        let it = crate::interior::generate_id(world, t0, site)?;
        let lv = it.levels.get(li)?;
        return match ri {
            Some(ri) => lv.rooms.get(ri).map(|r| crate::interior::to_world(&it, r.center[0] as f64, r.center[1] as f64)),
            None => position(world, t0, site),
        };
    }
    let mut parts = id.split(':');
    let head = parts.next()?;
    let nums: Vec<usize> = parts.filter_map(|p| p.parse().ok()).collect();
    match (head, nums.as_slice()) {
        ("b", [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).building(*bi).map(|b| geom::centroid(&b.poly)),
        // (A district the ward plan left with no patch has no label: the town's middle.)
        ("d", [li, qi]) if *li < town::layout_count(t0) => {
            let l = town::layout(world, t0, *li);
            l.quarters.get(*qi).map(|q| q.label.get(q.label.len() / 2).copied().or_else(|| q.patches.first().map(|p| geom::centroid(p))).unwrap_or(l.center))
        }
        ("h", _) if edits.plateaus.contains_key(id) => {
            let pts = &edits.plateaus[id].pts;
            Some(geom::centroid(pts))
        }
        ("e", _) if edits.waters.contains_key(id) => Some(geom::centroid(&edits.waters[id].shore())),
        ("w", [li]) if town::is_settlement(t0, *li) => Some(town::layout(world, t0, *li).center),
        ("t", [li, k]) if *li < town::layout_count(t0) => {
            let l = town::layout(world, t0, *li);
            l.tower_index(*k).and_then(|i| crate::interior::towers(&l).get(i).map(|t| t.0))
        }
        ("u", [li, k]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).entrance(*k).map(|e| e.at),
        ("k", [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).building(*bi).map(|b| geom::centroid(&b.poly)),
        ("w", [li, sx, sy]) if town::is_settlement(t0, *li) => Some([(*sx as f64 + 0.5) * crate::under::SEWER_SECTION_FT, (*sy as f64 + 0.5) * crate::under::SEWER_SECTION_FT]),
        ("sq" | "gt", _) => town::places::find(world, t0, id).map(|x| x.1.at),
        _ => features(world, t0).into_iter().find(|f| f.id == id).map(|f| [f.x, f.y]),
    }
}

/// The site and level of a level, room or point note id (`l:`, `r:`, `m:`).
pub fn site_level(id: &str) -> Option<(&str, usize)> {
    split_room_id(id).map(|(s, l, _)| (s, l)).or_else(|| split_point_id(id).map(|(s, l, _)| (s, l)))
}

/// The room of level `li` a grid point lies in.
pub fn room_at(it: &crate::interior::Interior, li: usize, at: [f64; 2]) -> Option<usize> {
    let lv = it.levels.get(li)?;
    let (x, y) = (crate::core::floor(at[0]), crate::core::floor(at[1]));
    if x < 0.0 || y < 0.0 || x >= it.nx as f64 || y >= it.ny as f64 {
        return None;
    }
    usize::try_from(*lv.cells.get(y as usize * it.nx + x as usize)?).ok()
}

/// A note as `list_notes` shows it: id, kind (place, room or point), its place path (site, level
/// and room names joined by " › "), the room id a point lies in, text, tags and a point's spot.
pub fn note_row(world: &World, t0: &T0, id: &str, n: &crate::world::Note) -> Option<Value> {
    note_row_in(world, t0, id, n, &mut FastMap::default())
}

/// `note_row` with the interiors already generated kept in `cache` (by site), for a whole list. A point note whose
/// site no longer resolves is a row with `place: null`.
pub fn note_row_in(world: &World, t0: &T0, id: &str, n: &crate::world::Note, cache: &mut FastMap<String, Option<crate::interior::Interior>>) -> Option<Value> {
    let name = |i: &str| name_any(world, t0, i);
    let level_path = |site: &str, li: usize| vec![name(site).unwrap_or_else(|| site.to_string()), name(&format!("l:{site}:{li}")).unwrap_or_else(|| format!("level {li}"))];
    let (kind, place, room) = if let Some((site, li, _)) = split_point_id(id) {
        let it = cache.entry(site.to_string()).or_insert_with(|| crate::interior::generate_id(world, t0, site));
        match it {
            Some(it) => {
                let ri = n.at.and_then(|at| room_at(it, li, at)).map(|ri| format!("r:{site}:{li}:{ri}"));
                let mut path = level_path(site, li);
                path.extend(ri.as_ref().and_then(|r| name(r)));
                ("point", path.join(" › "), ri)
            }
            None => {
                let mut v = json!({ "id": id, "kind": "point", "place": null, "missing": format!("{site} no longer has an inside"), "text": n.text, "tags": n.tags, "room": null });
                if let Some(at) = n.at {
                    v["at"] = json!(at);
                }
                return Some(v);
            }
        }
    } else if let Some((site, li, ri)) = split_room_id(id) {
        let mut path = level_path(site, li);
        if ri.is_some() {
            path.extend(name(id));
        }
        (if ri.is_some() { "room" } else { "place" }, path.join(" › "), None)
    } else {
        ("place", name(id).unwrap_or_else(|| id.to_string()), None)
    };
    let mut v = json!({ "id": id, "kind": kind, "place": place, "text": n.text, "tags": n.tags, "room": room });
    if let Some(at) = n.at {
        v["at"] = json!(at);
    }
    Some(v)
}

/// NPC and plot ids (which are not places themselves).
pub fn is_authored(id: &str) -> bool {
    id.starts_with("n:") || id.starts_with("p:")
}

/// An NPC as agents see it (`brief`: name, stance and where only).
pub fn npc_json(world: &World, t0: &T0, id: &str, brief: bool) -> Option<Value> {
    let n = world.file.edits.npcs.get(id)?;
    let at = n.location.as_ref().map(|l| {
        let mut v = json!({ "id": l.id, "name": name_any(world, t0, &l.id) });
        if let Some(lv) = l.level {
            v["level"] = json!(lv);
        }
        if let (Some(x), Some(y)) = (l.x, l.y) {
            v["x_ft"] = json!(x.round());
            v["y_ft"] = json!(y.round());
        }
        v
    });
    if brief {
        return Some(json!({ "id": id, "name": n.name, "stance": n.attitude.stance, "status": n.status, "location": at, "tags": n.tags }));
    }
    let plots: Vec<Value> = world.file.edits.plots.iter().filter(|(_, p)| p.npcs.iter().any(|k| k == id)).map(|(k, p)| json!({ "id": k, "title": p.title, "status": p.status })).collect();
    Some(json!({
        "id": id,
        "name": n.name,
        "appearance": n.appearance,
        "mannerisms": n.mannerisms,
        "attitude": n.attitude,
        "goals": n.goals,
        "notes": n.notes,
        "tags": n.tags,
        "portrait": n.portrait,
        "status": n.status,
        "location": at,
        "plots": plots,
    }))
}

/// A plot point as agents see it, its places and NPCs named.
pub fn plot_json(world: &World, t0: &T0, id: &str, brief: bool) -> Option<Value> {
    let p = world.file.edits.plots.get(id)?;
    let anchors: Vec<Value> = p.anchors.iter().map(|a| json!({ "id": a, "name": name_any(world, t0, a) })).collect();
    let npcs: Vec<Value> = p.npcs.iter().map(|k| json!({ "id": k, "name": world.file.edits.npcs.get(k).map(|n| n.name.clone()) })).collect();
    let mut v = json!({ "id": id, "title": p.title, "status": p.status, "anchors": anchors, "npcs": npcs, "tags": p.tags });
    if !brief {
        v["text"] = json!(p.text);
    }
    Some(v)
}

/// A place by id, for the app's notebook: its name (as renamed) and where it is.
pub fn place(world: &World, t0: &T0, id: &str) -> Option<Value> {
    let p = position(world, t0, id)?;
    Some(json!({ "id": id, "name": name_any(world, t0, id), "generated": generated_name(world, t0, id), "x": p[0], "y": p[1] }))
}

/// Any id's display name: a feature, building, district, NPC or plot (else null).
pub fn name_any(world: &World, t0: &T0, id: &str) -> Option<String> {
    if deleted_with(world, t0, id).is_some() {
        return None;
    }
    let edits = &world.file.edits;
    if let Some(n) = edits.npcs.get(id) {
        return Some(n.name.clone());
    }
    if let Some(p) = edits.plots.get(id) {
        return Some(p.title.clone());
    }
    if let Some(n) = edits.renames.get(id) {
        return Some(n.clone());
    }
    generated_name(world, t0, id)
}

/// A place's name before renames (a building's own name or its trade, a level's or room's
/// generated name, a tower's or way underground's label, a feature's overlay name).
fn generated_name(world: &World, t0: &T0, id: &str) -> Option<String> {
    if deleted_with(world, t0, id).is_some() {
        return None;
    }
    // A point note: "Note in" the room it lies in (as renamed), else on its level.
    if let Some((site, li, _)) = split_point_id(id) {
        let at = world.file.edits.notes.get(id)?.at?;
        let it = crate::interior::generate_id(world, t0, site)?;
        let lv = it.levels.get(li)?;
        let room = room_at(&it, li, at).map(|ri| name_any(world, t0, &format!("r:{site}:{li}:{ri}")).unwrap_or_else(|| lv.rooms[ri].kind.to_string()));
        return Some(format!("Note in {}", room.unwrap_or_else(|| world.file.edits.renames.get(&format!("l:{site}:{li}")).cloned().unwrap_or_else(|| lv.name.clone()))));
    }
    if let Some((site, li, ri)) = split_room_id(id) {
        let it = crate::interior::generate_id(world, t0, site)?;
        let lv = it.levels.get(li)?;
        return match ri {
            Some(ri) => lv.rooms.get(ri).map(|r| r.kind.to_string()),
            None => Some(lv.name.clone()),
        };
    }
    if let Some(c) = world.file.edits.crossings.get(id) {
        return Some(crate::roadids::crossing_generated(c));
    }
    if let Some(w) = world.file.edits.wharves.get(id) {
        return Some(crate::wharf::generated_name(w));
    }
    if let Some(w) = world.file.edits.waters.get(id) {
        return Some(w.look(world.params().sea_level_ft).kind.name().to_string());
    }
    if id.starts_with("rd:") {
        let i = crate::roadids::road_index(t0, id)?;
        let r = &t0.roads.roads[i];
        let fs = features(world, t0);
        return r.stroke.and_then(|s| crate::roadids::drawn_road(world, t0, &fs, s)).map(|f| fs[f].name.clone());
    }
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    match (id.split(':').next(), nums.as_slice()) {
        (Some("b"), [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).building(*bi).map(|b| b.name.clone().unwrap_or_else(|| b.label().to_string())),
        (Some("d"), [li, qi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).quarters.get(*qi).map(|q| q.name.clone()),
        // Towers and ways underground as `layout_names` lists them; a sewer section as its site.
        (Some("t"), [li, k]) if *li < town::layout_count(t0) => {
            let l = town::layout(world, t0, *li);
            l.tower_index(*k).and_then(|i| crate::interior::towers(&l).get(i).map(|t| format!("{} {}", if t.2 { "Gate tower" } else { "Wall tower" }, i + 1)))
        }
        (Some("u"), [li, k]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).entrance(*k)
            .map(|e| crate::sitelinks::dungeon_name(world, t0, *li).unwrap_or_else(|| cap(e.kind.name()))),
        (Some("k"), [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).building(*bi).filter(|b| crate::interior::has_deep_dungeon(b)).map(|_| "Deep dungeons".into()),
        (Some("w"), [li, ..]) if *li < town::layout_count(t0) => crate::interior::generate_id(world, t0, id).map(|it| it.name.unwrap_or_else(|| it.function.to_string())),
        (Some("sq" | "gt"), _) => town::places::find(world, t0, id).map(|x| x.1.name),
        _ => match world.file.edits.created.iter().find(|c| c.id == id) {
            Some(c) => Some(c.name.clone()),
            None => match &t0.extra {
                Some(e) => e.overlay.features.iter().find(|f| f.id == id).map(|f| f.name.clone()),
                None => place_name(t0, id),
            },
        },
    }
}

/// The NPCs and plot points at the places `ids` (each with the place it is at), as `at_place`
/// counts them.
pub(super) fn authored_at(world: &World, t0: &T0, ids: &[String]) -> (Vec<Value>, Vec<Value>) {
    let edits = &world.file.edits;
    if edits.npcs.is_empty() && edits.plots.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let at: Vec<_> = ids.iter().map(|id| at_place(world, t0, id)).collect();
    let here = |a: &str| at.iter().any(|f| f(a));
    let npcs = edits.npcs.iter().filter(|(_, n)| n.location.as_ref().is_some_and(|l| here(&l.id))).filter_map(|(k, _)| npc_json(world, t0, k, true)).collect();
    let plots = edits.plots.iter().filter(|(_, p)| p.anchors.iter().any(|a| here(a))).filter_map(|(k, _)| plot_json(world, t0, k, true)).collect();
    (npcs, plots)
}

/// Whether an NPC's location or a plot's anchor (`a`) is at place `id`: the place itself, or a
/// level or room of it (a room of a level too); for a settlement's or site's id, anything in its
/// layout (`b d t u k w:<layout>:` ids, and their levels and rooms); for a settlement, the same
/// for each site in it (`sites_of`: by membership for drawn things), the sites' own ids included.
pub fn at_place(world: &World, t0: &T0, id: &str) -> impl Fn(&str) -> bool + use<> {
    let mut ids = vec![id.to_string()];
    let mut layouts: Vec<usize> = Vec::new();
    // A building drawn by hand is one place under both its ids (its layout is the building).
    if let Some((k, c, b)) = drawn_building_ids(world, t0, id) {
        ids.extend([c, b].into_iter().filter(|x| x != id));
        layouts.push(t0.settlements.len() + t0.base_pois + k);
    } else if let Some(li) = layout_of(world, t0, id) {
        layouts.push(li);
        if is_town(world, t0, li) {
            for c in sites_of(world, t0, li) {
                layouts.push(c);
                ids.extend(feature_of_layout(world, t0, c));
            }
        }
    }
    move |a: &str| {
        let room = split_room_id(a).or_else(|| split_point_id(a).map(|(s, l, _)| (s, l, Some(0))));
        let at = room.map_or(a, |r| r.0);
        if ids.iter().any(|i| i == a || i == at) || room.is_some_and(|(s, lv, r)| r.is_some() && ids.contains(&format!("l:{s}:{lv}"))) {
            return true;
        }
        let mut parts = at.split(':');
        matches!(parts.next(), Some("b" | "d" | "t" | "u" | "k" | "w" | "sq" | "gt")) && parts.next().and_then(|s| s.parse::<usize>().ok()).is_some_and(|li| layouts.contains(&li))
    }
}

/// The settlements' sites, for the app: `{n, base, pairs, owners, pushed, sizes}`, with the counts
/// of settlements and generated sites the layout indices assume. `pairs`: `[[site layout,
/// settlement]…]`, the world's own sites standing in one (`town::parents`) and the drawn things
/// part of one; `owners`: `[[layout, town layout, "gap"|"hand", ft|null]…]`, every drawn thing
/// part of a town or a place (`town::members`); `pushed`: `[[place layout, town layout, [building
/// ids]]…]`; `sizes`: `[[feature id, size class, label extent ft, people]…]` (`town_sizes`).
pub fn parents_json(world: &World, t0: &T0) -> String {
    let owners = town::members::owners(world, t0);
    let first = t0.settlements.len() + t0.base_pois;
    let mut pairs: Vec<(usize, usize)> = town::parents(t0).into_iter().filter(|p| p.0 < first).collect();
    pairs.extend(owners.iter().filter(|o| town::is_settlement(t0, o.1.town)).map(|o| (o.0, o.1.town)));
    pairs.sort_unstable();
    let why = |o: &town::Owner| match o.why {
        town::Why::Gap(g) => json!(["gap", (g * 10.0).round() / 10.0]),
        town::Why::Hand => json!(["hand", null]),
        town::Why::Inside => json!(["inside", null]),
    };
    let owners: Vec<Value> = owners.iter().map(|(li, o)| {
        let w = why(o);
        json!([li, o.town, w[0], w[1]])
    }).collect();
    let pushed: Vec<Value> = town::members::all_pushed(world, t0).into_iter().map(|(p, t, ids)| json!([p, t, ids])).collect();
    let sizes: Vec<Value> = town_sizes(world, t0).into_iter().map(|(id, k, e, n)| json!([id, k, e, n])).collect();
    json!({ "n": t0.settlements.len(), "base": t0.base_pois, "pairs": pairs, "owners": owners, "pushed": pushed, "sizes": sizes }).to_string()
}

/// Whether layout `li` is a town things can be part of: a settlement, or a place (`town::members`).
pub(crate) fn is_town(world: &World, t0: &T0, li: usize) -> bool {
    town::is_settlement(t0, li) || li.checked_sub(t0.settlements.len() + t0.base_pois).and_then(|k| world.file.edits.created.get(k)).is_some_and(|c| c.kind == "place" && !c.removed)
}

/// The sites in town `si`, by layout: the world's own standing in it (`town::sites_in`) and the
/// drawn things part of it (`town::members`, which a drawn thing's being in its radius is not).
pub(crate) fn sites_of(world: &World, t0: &T0, si: usize) -> Vec<usize> {
    let first = t0.settlements.len() + t0.base_pois;
    let mut v: Vec<usize> = town::sites_in(t0, si).into_iter().filter(|&li| li < first).collect();
    v.extend(town::members::members_of(world, t0, si).into_iter().map(|m| m.0));
    v.sort_unstable();
    v
}

/// The town site layout `li` belongs to: a drawn thing's by membership (`town::members::owner`:
/// a settlement or a place), a site of the world's own the settlement it stands in.
pub(crate) fn town_of(world: &World, t0: &T0, li: usize) -> Option<usize> {
    if li >= t0.settlements.len() + t0.base_pois { town::members::owner(world, t0, li).map(|o| o.town) } else { town::parent_of(t0, li) }
}

/// `{id, name}` of layout `li`'s feature (a settlement or site), or null.
fn layout_ref(world: &World, t0: &T0, li: usize) -> Value {
    feature_of_layout(world, t0, li).map_or(Value::Null, |f| json!({ "name": name_of(world, t0, &f), "id": f }))
}

/// Where something in layout `li` is: the settlement (the layout's own, or the one its site
/// stands in; `{id, name}` or null), and for a site's layout the site (`{id, name}`).
pub(crate) fn whereabouts(world: &World, t0: &T0, li: usize) -> (Value, Option<Value>) {
    if town::is_settlement(t0, li) {
        return (layout_ref(world, t0, li), None);
    }
    (town_of(world, t0, li).map_or(Value::Null, |si| layout_ref(world, t0, si)), Some(layout_ref(world, t0, li)))
}

/// Sets `settlement` (and `site`, in a site's layout) on `v` for something in layout `li`.
pub(super) fn set_whereabouts(world: &World, t0: &T0, v: &mut Value, li: usize) {
    let (s, site) = whereabouts(world, t0, li);
    v["settlement"] = s;
    if let Some(site) = site {
        v["site"] = site;
    }
}

/// A building drawn by hand is one place with two ids: for either (its created id `c:<n>` or its
/// building id `b:<layout>:0`), its place in `edits.created`, its created id (its name, note and
/// hidden mark are kept under it) and its building id; removed ones too.
pub fn drawn_pair(world: &World, t0: &T0, id: &str) -> Option<(usize, String, String)> {
    let base = t0.settlements.len() + t0.base_pois;
    let k = match id.strip_prefix("b:").and_then(|r| r.strip_suffix(":0")).and_then(|s| s.parse::<usize>().ok()) {
        Some(li) => li.checked_sub(base)?,
        None => world.file.edits.created.iter().position(|c| c.id == id)?,
    };
    let c = world.file.edits.created.get(k).filter(|c| c.kind == "building")?;
    Some((k, c.id.clone(), format!("b:{}:0", base + k)))
}

/// `drawn_pair` for a building drawn by hand still standing (renames, `at_place`).
pub fn drawn_building_ids(world: &World, t0: &T0, id: &str) -> Option<(usize, String, String)> {
    drawn_pair(world, t0, id).filter(|p| !world.file.edits.created[p.0].removed)
}

/// The id a place's note and hidden mark are kept under: a building drawn by hand's created id.
pub fn note_key(world: &World, t0: &T0, id: &str) -> String {
    drawn_pair(world, t0, id).map_or_else(|| id.to_string(), |p| p.1)
}

/// Whether a place is hidden (a building drawn by hand under either of its ids).
pub(super) fn is_hidden(world: &World, t0: &T0, id: &str) -> bool {
    let h = &world.file.edits.hidden;
    h.contains(id) || drawn_pair(world, t0, id).is_some_and(|(_, c, b)| h.contains(&c) || h.contains(&b))
}

/// Whether site layout `li` is a building drawn by hand (its building is `b:<li>:0`).
fn drawn_building(t0: &T0, li: usize) -> bool {
    li.checked_sub(t0.settlements.len()).and_then(|k| t0.pois.get(k)).is_some_and(|p| p.kind == crate::t0::settle::PoiKind::Building)
}

/// The sites in settlement `si`: id, kind and name, a drawn building's `building` id, and with
/// `underground` their ways underground.
pub(super) fn sites_json(world: &World, t0: &T0, si: usize, underground: bool) -> Vec<Value> {
    let fs = features(world, t0);
    sites_of(world, t0, si)
        .into_iter()
        .filter_map(|li| {
            let id = feature_of_layout(world, t0, li)?;
            let f = fs.iter().find(|f| f.id == id)?;
            let mut v = json!({ "id": id, "kind": f.kind, "name": f.name });
            if drawn_building(t0, li) {
                v["building"] = json!(format!("b:{li}:0"));
            }
            if underground {
                let l = town::layout(world, t0, li);
                v["underground"] = json!(l.entrances.iter().filter(|e| e.kind != crate::under::UnderKind::Sewer).map(|e| format!("u:{li}:{}", e.id)).collect::<Vec<_>>());
            }
            Some(v)
        })
        .collect()
}

/// What belongs to settlement `si` by membership (`town::members`): its drawn things
/// (`{id, kind, name, why}`: "gap" within 300 ft of its edge or a member, "hand" joined by hand) and
/// the places its outskirts were pushed out into (`{place, name, buildings}`).
pub(super) fn members_json(world: &World, t0: &T0, si: usize) -> (Vec<Value>, Vec<Value>) {
    let fs = features(world, t0);
    let members = town::members::members_of(world, t0, si)
        .into_iter()
        .filter_map(|(li, o)| {
            let id = feature_of_layout(world, t0, li)?;
            let f = fs.iter().find(|f| f.id == id)?;
            let why = match o.why {
                town::Why::Gap(_) => "gap",
                town::Why::Hand => "hand",
                town::Why::Inside => "inside",
            };
            Some(json!({ "id": id, "kind": f.kind, "name": f.name, "why": why }))
        })
        .collect();
    let pushed = town::members::pushed(world, t0, si)
        .into_iter()
        .filter_map(|(pl, ids)| {
            let id = feature_of_layout(world, t0, pl)?;
            let name = fs.iter().find(|f| f.id == id).map(|f| f.name.clone()).unwrap_or_default();
            Some(json!({ "place": id, "name": name, "buildings": ids.iter().map(|b| format!("b:{si}:{b}")).collect::<Vec<_>>() }))
        })
        .collect();
    (members, pushed)
}

/// The buildings drawn by hand in settlement `si` that have a trade: (`b:<site>:0`, its layout).
pub(super) fn drawn_businesses(world: &World, t0: &T0, si: usize) -> Vec<(String, std::rc::Rc<town::Layout>)> {
    sites_of(world, t0, si)
        .into_iter()
        .filter(|&li| drawn_building(t0, li))
        .map(|li| (format!("b:{li}:0"), town::layout(world, t0, li)))
        .filter(|(_, l)| l.building(0).is_some_and(|b| b.func.is_some()))
        .collect()
}

/// The generated buildings with a trade pushed out into place `place` (`town::members`): (`b:` id,
/// its town's layout, its id); none for any other layout.
pub(super) fn pushed_businesses(world: &World, t0: &T0, place: usize) -> Vec<(String, std::rc::Rc<town::Layout>, u32)> {
    let mut out = Vec::new();
    for (_, t, ids) in town::members::all_pushed(world, t0).into_iter().filter(|p| p.0 == place) {
        let l = town::layout(world, t0, t);
        out.extend(ids.into_iter().filter(|&id| l.building(id as usize).is_some_and(|b| b.func.is_some())).map(|id| (format!("b:{t}:{id}"), l.clone(), id)));
    }
    out
}

/// The buildings drawn by hand in settlement `si` that are homes (no trade), as `drawn_businesses`.
fn drawn_homes(world: &World, t0: &T0, si: usize) -> Vec<(String, std::rc::Rc<town::Layout>)> {
    sites_of(world, t0, si)
        .into_iter()
        .filter(|&li| drawn_building(t0, li))
        .map(|li| (format!("b:{li}:0"), town::layout(world, t0, li)))
        .filter(|(_, l)| l.building(0).is_some_and(|b| b.func.is_none()))
        .collect()
}

/// A place's note (a building drawn by hand: the notes under both its ids, as one).
pub(super) fn note_of(world: &World, t0: &T0, id: &str) -> Value {
    let notes = &world.file.edits.notes;
    let n = match drawn_pair(world, t0, id) {
        Some((_, c, b)) => crate::world::Note::join(notes.get(&c), notes.get(&b)),
        None => notes.get(id).cloned(),
    };
    n.map(|n| json!({ "text": n.text, "tags": n.tags })).unwrap_or(Value::Null)
}

pub(super) fn compass(from: P, to: P) -> &'static str {
    let a = libm::atan2(to[1] - from[1], to[0] - from[0]).to_degrees();
    // Screen y points south.
    ["E", "SE", "S", "SW", "W", "NW", "N", "NE"][((((a + 22.5) % 360.0) + 360.0) % 360.0 / 45.0) as usize % 8]
}

/// The areas a point lies in (land or sea, ranges, forests, lakes...), largest first.
pub(super) fn regions_at(fs: &[Feature], shapes: &[Cow<Shape>], p: P) -> Vec<Value> {
    let mut out: Vec<(f64, &Feature)> =
        fs.iter().zip(shapes).filter(|(_, s)| matches!(***s, Shape::Area { .. }) && s.distance(p).0 == 0.0).map(|(f, s)| (s.area_sq_ft(), f)).collect();
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().map(|(_, f)| json!({ "id": f.id, "kind": f.kind, "name": f.name })).collect()
}

pub(super) fn nearest_settlements(fs: &[Feature], p: P, skip: &str, n: usize) -> Vec<Value> {
    let mut v: Vec<&Feature> = fs.iter().filter(|f| SETTLEMENT_KINDS.contains(&f.kind) && f.id != skip).collect();
    v.sort_by(|a, b| geom::dist([a.x, a.y], p).total_cmp(&geom::dist([b.x, b.y], p)));
    v.into_iter()
        .take(n)
        .map(|f| json!({ "id": f.id, "name": f.name, "kind": f.kind, "distance_mi": mi(geom::dist([f.x, f.y], p)), "direction": compass(p, [f.x, f.y]) }))
        .collect()
}

/// The names in a settlement's or site's layout: its districts, businesses, wall and gate
/// towers, ways underground (sewer grates aside) and keeps' deep dungeons.
pub fn layout_names(world: &World, t0: &T0, li: usize) -> Vec<Value> {
    if li >= town::layout_count(t0) || li.checked_sub(t0.settlements.len() + t0.base_pois).and_then(|k| world.file.edits.created.get(k)).is_some_and(|c| c.removed) {
        return Vec::new();
    }
    let l = town::layout(world, t0, li);
    let mut out = Vec::new();
    for (qi, q) in l.quarters.iter().enumerate() {
        out.push(name_entry(world, format!("d:{li}:{qi}"), "district", q.name.clone(), q.label.get(q.label.len() / 2).copied(), false));
    }
    // (Those pushed out into a place listed with it: `pushed_businesses`.)
    for (id, cl, bi) in l.buildings.iter().filter(|b| b.func.is_some() && town::members::building_owner(world, t0, li, b.id).is_none()).map(|b| (format!("b:{li}:{}", b.id), l.clone(), b.id)).chain(pushed_businesses(world, t0, li)) {
        let Some(b) = cl.building(bi as usize) else { continue };
        let generated = b.name.clone().unwrap_or_else(|| b.label().to_string());
        let mut v = name_entry(world, id, "building", generated, Some(geom::centroid(&b.poly)), true);
        v["function"] = json!(b.label());
        out.push(v);
    }
    for (k, (at, _, gate)) in crate::interior::towers(&l).into_iter().enumerate() {
        out.push(name_entry(world, format!("t:{li}:{}", l.tower_id(k)), "tower", format!("{} {}", if gate { "Gate tower" } else { "Wall tower" }, k + 1), Some(at), true));
    }
    for p in town::places::of(world, t0, &l) {
        out.push(name_entry(world, p.id, p.kind, p.name, Some(p.at), false));
    }
    for e in l.entrances.iter().filter(|e| e.kind != crate::under::UnderKind::Sewer) {
        let name = crate::sitelinks::dungeon_name(world, t0, li).unwrap_or_else(|| cap(e.kind.name()));
        out.push(name_entry(world, format!("u:{li}:{}", e.id), "underground", name, Some(e.at), true));
    }
    for b in l.buildings.iter().filter(|b| crate::interior::has_deep_dungeon(b)) {
        out.push(name_entry(world, format!("k:{li}:{}", b.id), "underground", "Deep dungeons".into(), Some(geom::centroid(&b.poly)), true));
    }
    // A settlement's buildings drawn by hand that have a trade, as its businesses.
    if is_town(world, t0, li) {
        for (id, cl) in drawn_businesses(world, t0, li).into_iter().chain(drawn_homes(world, t0, li)) {
            let Some(b) = cl.building(0) else { continue };
            let hidden = is_hidden(world, t0, &id);
            let mut v = name_entry(world, id, "building", b.name.clone().unwrap_or_else(|| b.label().to_string()), Some(geom::centroid(&b.poly)), true);
            v["function"] = json!(b.label());
            if hidden {
                v["hidden"] = json!(true);
            }
            out.push(v);
        }
    }
    out
}

/// The names in a building or site: each level, then its rooms (ids `l:` and `r:`).
pub fn site_names(world: &World, t0: &T0, site: &str) -> Option<Vec<Value>> {
    if deleted_with(world, t0, site).is_some() {
        return None;
    }
    let it = crate::interior::generate_id(world, t0, site)?;
    let mut out = Vec::new();
    for (li, lv) in it.levels.iter().enumerate() {
        out.push(name_entry(world, format!("l:{site}:{li}"), "level", lv.name.clone(), None, false));
        for (ri, r) in lv.rooms.iter().enumerate().filter(|(_, r)| r.squares > 0 && r.kind != "floor") {
            let p = crate::interior::to_world(&it, r.center[0] as f64, r.center[1] as f64);
            out.push(name_entry(world, format!("r:{site}:{li}:{ri}"), "room", r.kind.to_string(), Some(p), false));
        }
    }
    Some(out)
}

/// The names in a layout (`scope` its index) or a building or site (`scope` its id), JSON.
pub fn names_json(world: &World, t0: &T0, scope: &str) -> String {
    let v = match scope.parse::<usize>() {
        Ok(li) => Some(layout_names(world, t0, li)),
        Err(_) => site_names(world, t0, scope),
    };
    serde_json::to_string(&v.unwrap_or_default()).expect("serializable")
}

/// Names to rename (`list_names`): with `within` (a building or site) its levels and rooms;
/// with `settlement` (a settlement's or site's id) what is in its layout; else every named
/// feature (natural ones, settlements, sites, created sites). `kind` keeps only that kind.
pub fn names(world: &World, t0: &T0, kind: Option<&str>, within: Option<&str>, settlement: Option<&str>) -> Result<Value, String> {
    let list = if let Some(site) = within {
        site_names(world, t0, site).ok_or_else(|| deleted_note(world, t0, site).unwrap_or_else(|| format!("not a building or site with levels: {site}")))?
    } else if let Some(s) = settlement {
        if let Some(n) = deleted_note(world, t0, s) {
            return Err(n);
        }
        let li = layout_of(world, t0, s).ok_or_else(|| format!("not a settlement or site: {s}"))?;
        let mut v = layout_names(world, t0, li);
        // A settlement's sites (by their own ids; a drawn business is also among the buildings).
        if is_town(world, t0, li) {
            for c in sites_of(world, t0, li) {
                let (Some(id), Some(p)) = (feature_of_layout(world, t0, c), t0.pois.get(c - t0.settlements.len())) else { continue };
                let generated = generated_name(world, t0, &id).unwrap_or_default();
                v.push(name_entry(world, id, "site", generated, Some([p.x, p.y]), false));
            }
        }
        v
    } else {
        let edits = &world.file.edits;
        let fs = t0.extra.as_ref().map(|e| e.overlay.features.as_slice()).unwrap_or_default();
        let fs_all = features(world, t0);
        let mut v: Vec<Value> = fs.iter().map(|f| name_entry(world, f.id.clone(), f.kind, f.name.clone(), Some([f.x, f.y]), false)).collect();
        v.extend(edits.created.iter().filter(|c| !c.removed).map(|c| name_entry(world, c.id.clone(), &c.kind, c.name.clone(), Some([c.x, c.y]), false)));
        // Road stretches someone renamed (not every stretch: list_roads has their ids), and the
        // crossings put down by hand.
        if edits.renames.keys().any(|k| k.starts_with("rd:")) {
            for (i, id) in crate::roadids::road_ids(t0).into_iter().enumerate().filter(|(_, id)| edits.renames.contains_key(id)) {
                let r = &t0.roads.roads[i];
                let generated = r.stroke.and_then(|s| crate::roadids::drawn_road(world, t0, &fs_all, s)).map(|f| fs_all[f].name.clone()).unwrap_or_default();
                let mut ent = name_entry(world, id, "road", generated.clone(), Some(crate::roadids::midpoint(r)), false);
                if generated.is_empty() {
                    ent["generated"] = Value::Null;
                }
                v.push(ent);
            }
        }
        v.extend(edits.crossings.iter().map(|(id, c)| name_entry(world, id.clone(), "crossing", crate::roadids::crossing_generated(c), Some(crate::roadids::crossing_mid(c)), false)));
        v.extend(edits.wharves.iter().map(|(id, w)| name_entry(world, id.clone(), "wharf", crate::wharf::generated_name(w), Some(crate::wharf::mid(w)), false)));
        // (R5 towns: squares, parks and gates are in their towns' layouts.)
        if matches!(kind, Some("square" | "park" | "gate")) {
            v.extend((0..t0.settlements.len()).chain(t0.made.iter().map(|m| t0.settlements.len() + m.poi)).filter(|&i| town::settlement(t0, i).is_some_and(|s| s.tier >= crate::t0::settle::Tier::Town)).flat_map(|li| layout_names(world, t0, li)));
        }
        v
    };
    Ok(json!({ "names": list.into_iter().filter(|v| kind.is_none_or(|k| v["kind"] == k)).collect::<Vec<_>>() }))
}

// ---------------------------------------------------------------------------------------
// Routes over the road network.
