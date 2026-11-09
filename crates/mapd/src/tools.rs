//! The MCP tools: read the world, look at it through the open app, and edit it. Every edit
//! is saved, logged and shown live in the app.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::core::tile::TileKey;
use worldgen::under::{MAX_LEVELS, SiteSize, THEMES, UnderKind};
use worldgen::world::{CREATABLE, Created, Edits, Note};

/// The themes by kind of site, for the tool's description.
fn theme_doc() -> String {
    let kinds = [UnderKind::Dungeon, UnderKind::Crypt, UnderKind::Catacombs, UnderKind::Cave, UnderKind::Mine, UnderKind::LavaTube];
    let by: Vec<String> = kinds.iter().map(|k| format!("{}: {}", k.key(), THEMES.iter().filter(|t| t.kind == *k).map(|t| t.key).collect::<Vec<_>>().join(", "))).collect();
    format!("The site underground; by its kind: {}", by.join("; "))
}

use crate::Shared;

pub const INSTRUCTIONS: &str = "A procedurally generated fantasy world (D&D 5e scale: 5-ft squares). Coordinates are feet from \
the map's top-left corner, x east and y south. Find places with search_features, features_near or world_overview, \
inspect them with get_feature and list_children, see them with render_view. Edits (rename, annotate, create, hide) are saved and appear live \
in the open map app. Created sites are generated like the world's own: a ruin's dungeon or crypt is fully playable. Battlemaps take objects put down or taken away by hand (place_objects, remove_objects), including uploaded sprites (upload_sprite); their ground can be shaped (shape_ground, set_plateau). Many edits at once go in one batch. Deletes can be undone (restore_feature); the creates (create_feature, create_building, create_npc, create_plot, place_objects) are safe to repeat: the same one again returns what is there. Every tool takes an optional world (the hash world_overview gives) and refuses to run if another world is open. The DM's notebook holds NPCs (list_npcs, create_npc, place_npc...) and plot points tied to places (list_plots, create_plot...); get_feature and describe_location show those at a place.";

pub(crate) fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn place_props() -> Value {
    crate::args::at_props()
}

fn merge(a: Value, b: Value) -> Value {
    let mut a = a;
    if let (Some(m), Value::Object(n)) = (a.as_object_mut(), b) {
        m.extend(n);
    }
    a
}

pub fn list() -> Value {
    let mut v = json!([
        { "name": "world_overview", "title": "World overview", "description": "The world's size, land masses, counts of named features and its largest settlements.", "inputSchema": schema(json!({}), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "search_features", "title": "Search", "description": "Named features matching a query, by their current names (towns, ranges, rivers, ruins...; from three letters also districts and businesses such as inns and temples). Empty query with a kind lists that kind. Returns {items, found} (truncated: true when limit cut the list). Each item has its extent: bounding box (ft) and area (sq mi), or a river's length (mi).", "inputSchema": schema(json!({ "query": { "type": "string", "default": "" }, "kind": { "type": "string", "description": "Only this kind (city, town, village, ruin, cave, range, forest, river, lake, building, district...)" }, "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 25 }, "include_hidden": { "type": "boolean", "default": false } }), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "get_feature", "title": "Feature details", "description": "A feature's details and relations: what and where it is, its extent (bounding box, area or length), its notes, the regions it lies in, nearest settlements, roads to other settlements, a settlement's districts, notable buildings, ways underground and the sites standing in it (sites), a building's or site's levels and rooms; for anything in a settlement's or site's layout, the settlement it is in (settlement: {id, name} or null) and the site (site). NPCs and plots at it, its sites, levels and rooms (a building drawn by hand: the same under either of its ids).", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_children", "title": "Children", "description": "What a feature contains: a settlement's districts, businesses (buildings drawn in it with a trade too) and the sites standing in it, a district's businesses, a building's or underground site's levels and rooms, a region's settlements and sites.", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_names", "title": "Names", "description": "Everything that can be renamed, each with its id, kind, current and generated name: every named feature (oceans, rivers, ranges, settlements, sites...); with settlement (a settlement's or site's id) its districts, businesses, towers and underground sites, and for a settlement the sites standing in it (kind site); with inside (a building or site id) its levels and rooms. Returns {items, found}.", "inputSchema": schema(json!({ "kind": { "type": "string", "description": "Only this kind (river, range, city, district, building, tower, underground, site, level, room...)" }, "settlement": { "type": "string" }, "inside": { "type": "string", "description": "A building or site id: its levels and rooms" } }), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "describe_location", "title": "What is here", "description": "What is at a place: elevation, biome, the areas it lies in (land, sea, range, forest, lake...), the building or district or site there, named features nearby (within a tenth of their size, 2 to 10 mi, measured to their nearest edge or course), nearest settlements.", "inputSchema": schema(place_props(), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "features_near", "title": "Near a place", "description": "Everything named within a radius of a place, nearest first (returns {items, found}; truncated: true when limit cut the list), measured to each feature's nearest edge or course (0 inside an area), with its distance, direction and extent. kinds limits it to some kinds (e.g. [\"river\", \"lake\", \"forest\"]); districts and businesses come when kinds asks for district or building, or with no kinds within a mile.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "minimum": 0.01, "maximum": 500, "default": 5 }, "kinds": { "type": "array", "items": { "type": "string" } }, "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 50 }, "include_hidden": { "type": "boolean", "default": false } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "route", "title": "Route", "description": "Distance between two places by road (where roads join them) or overland, with D&D 5e travel days (normal, fast, slow pace).", "inputSchema": schema(json!({ "from": crate::args::point_schema("Where from: an id, {x_ft, y_ft} or [x_ft, y_ft]"), "to": crate::args::point_schema("Where to: an id, {x_ft, y_ft} or [x_ft, y_ft]") }), &["from", "to"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_roads", "title": "Roads", "description": "The road network: named roads (drawn in the world's sketch) with the settlements along them, then the roads between settlements and junctions, longest first, each with its class (king's road, road, track), length and ends (a settlement's id and name, or the junction's point). Returns {items, found, named}. Filter by a place (at) and radius_mi, by settlement (roads ending there), by class, or drawn roads only.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "minimum": 0.01, "maximum": 3000, "default": 25 }, "settlement": { "type": "string", "description": "A settlement's id: only roads ending there" }, "class": { "type": "string", "enum": ["kings_road", "road", "track"] }, "drawn": { "type": "boolean", "default": false, "description": "Only roads drawn in the sketch" }, "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 50 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "get_battlemap", "title": "Battlemap", "description": "The battlemap (a 640-ft square of 5-ft squares) at a place: elevation range, surfaces, buildings, and every kind of object with its tactical rules (cover, movement, sight, hazards); objects near the place listed by square.", "inputSchema": schema(merge(place_props(), json!({ "radius_squares": { "type": "number", "minimum": 1, "maximum": 64, "default": 12 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "render_view", "title": "Look at the map", "description": "A screenshot of the open map app at a place (PNG, 1536x1024). size_ft is the ground across the image: up to about 450 shows the painted battlemap with its 5-ft grid, 3000 a village or a town's streets, 20000 a city and its fields, 200000 a region.", "inputSchema": schema(merge(place_props(), json!({ "size_ft": { "type": "number", "minimum": 50, "maximum": 5000000, "default": 3000 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "focus_view", "title": "Show the user", "description": "Move the open map app's view to a place (the user sees it).", "inputSchema": schema(merge(place_props(), json!({ "size_ft": { "type": "number", "minimum": 50, "maximum": 5000000, "default": 3000 } })), &[]) },
        { "name": "rename_feature", "title": "Rename", "description": "Rename anything named: a feature, building, district, tower, underground site, or a site's level (l:<site>:<level>) or room (r:<site>:<level>:<room>), ids as list_names gives them. An empty name restores the original.", "inputSchema": schema(json!({ "id": { "type": "string" }, "name": { "type": "string" } }), &["id", "name"]) },
        { "name": "annotate_feature", "title": "Add notes", "description": "Set (or append to) a feature's notes: lore, hooks, secrets for the DM, with tags. A building drawn by hand is one place: either of its ids.", "inputSchema": schema(json!({ "id": { "type": "string" }, "notes": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } }, "append": { "type": "boolean", "default": false } }), &["id", "notes"]) },
        { "name": "create_feature", "title": "Create a site", "description": "Add a site at a place on dry land, generated like the world's own (map, battlemap, interiors, underground): ruin (over a dungeon, crypt or catacombs), tower, camp, waystation (roadside inn), cave, mine, lava_tube, or entrance (a bare way underground: stairs, a cave mouth, a mine adit or a skylight, by 'under'). Sites with something underground take its size, levels and theme (anything left out is chosen as for the world's own). Or a castle (an outline: curtain wall, towers, gatehouse, keep, yard buildings) or a wall (a line of wall with towers; gates where asked and where roads and streets cross it), drawn by poly/rect or pts instead of a place; the world's own buildings in the way are named, or taken away with remove_in_way. Returns its id and the ids of its ways underground (a castle: its keep's building id, towers and gates).", "inputSchema": schema(merge(merge(place_props(), crate::works::props()), json!({
            "kind": { "type": "string", "enum": CREATABLE.iter().filter(|k| **k != "building").collect::<Vec<_>>() },
            "name": { "type": "string" },
            "under": { "type": "string", "enum": UnderKind::CREATABLE.map(|k| k.key()), "description": "What lies beneath a ruin (dungeon, crypt, catacombs) or an entrance (any)" },
            "size": { "type": "string", "enum": SiteSize::NAMES, "description": "The site underground" },
            "levels": { "type": "integer", "minimum": 1, "maximum": MAX_LEVELS },
            "theme": { "type": "string", "enum": THEMES.iter().map(|t| t.key).collect::<Vec<_>>(), "description": theme_doc() },
        })), &["kind"]) },
        { "name": "update_feature", "title": "Update", "description": "Change a feature's name and/or notes in one go. A building drawn by hand is one place: either of its ids.", "inputSchema": schema(json!({ "id": { "type": "string" }, "name": { "type": "string" }, "notes": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } } }), &["id"]) },
        { "name": "hide_feature", "title": "Hide", "description": "Hide a feature from the map's labels and search (or show it again). It is still there. A building drawn by hand is one place: either of its ids (its business pin is hidden too).", "inputSchema": schema(json!({ "id": { "type": "string" }, "hidden": { "type": "boolean", "default": true } }), &["id"]) },
        { "name": "delete_feature", "title": "Delete a created site", "description": "Delete sites that were created, buildings drawn by hand too (a building by either of its ids): id or ids, all or none in one change. Its ids are retired (get_feature and searches no longer find them); NPCs, plots, notes and a hand design that pointed at it are kept and reported in left_pointing. restore_feature brings it back. The world's own buildings go with remove_buildings; other generated features can only be hidden.", "inputSchema": schema(crate::args::ids_props("created site (c:<n>, or a drawn building's b:<layout>:0)"), &[]) },
    ]);
    if let Some(a) = v.as_array_mut() {
        a.extend(crate::notebook::list());
        a.extend(crate::sites::list());
        a.extend(crate::scatter::list());
        a.extend(crate::build::list());
        a.extend(crate::works::list());
        a.extend(crate::crossings::list());
        a.extend(crate::ground::list());
        a.extend(crate::design::list());
        a.extend(crate::towns::list());
        a.extend(crate::batch::list());
        // Any tool can say which world it means, and has its hints (a tool's own win).
        for t in a.iter_mut() {
            let hints = annotations(t["name"].as_str().unwrap_or(""));
            if !t["annotations"].is_object() {
                t["annotations"] = json!({});
            }
            for (k, v) in hints.as_object().into_iter().flatten() {
                if t["annotations"].get(k).is_none() {
                    t["annotations"][k] = v.clone();
                }
            }
            t["inputSchema"]["properties"]["world"] = json!({ "type": "string", "description": "The world meant (its hash, from world_overview): the call is refused if mapd has another world open" });
        }
    }
    v
}

/// Tools that only read (or look): `readOnlyHint`.
const READS: &[&str] = &[
    "world_overview", "search_features", "get_feature", "list_children", "list_names", "describe_location", "features_near", "route", "list_roads", "get_battlemap", "render_view",
    "list_crossings", "get_site_design", "ground_heights", "list_npcs", "get_npc", "list_plots", "get_plot", "list_sprites", "check_building_spot", "get_town_plan",
];
/// Writes that delete, replace or reset work (`destructiveHint`); every other write only adds or adjusts.
const DESTRUCTIVE: &[&str] = &[
    "delete_feature", "delete_npc", "delete_plot", "delete_plateau", "remove_objects", "remove_buildings", "remove_crossings", "reset_site_design", "set_site_design", "edit_town", "batch",
];
/// Writes that leave the world the same when repeated with the same arguments (`idempotentHint`): setters, restorers,
/// `focus_view` and the creates that return what is already there.
const IDEMPOTENT: &[&str] = &[
    "focus_view", "rename_feature", "update_feature", "hide_feature", "create_feature", "restore_feature", "create_building", "update_building", "restore_building", "create_npc",
    "update_npc", "place_npc", "create_plot", "update_plot", "place_objects", "restore_objects", "set_site_design", "reset_site_design", "set_plateau", "update_fortification",
];

/// A tool's hints by name, one table for every tool (so a tool in any module gets them): reads say `readOnlyHint` and
/// `openWorldHint: false`; writes also `destructiveHint` and `idempotentHint`.
pub(crate) fn annotations(name: &str) -> Value {
    if READS.contains(&name) {
        return json!({ "readOnlyHint": true, "openWorldHint": false });
    }
    json!({ "readOnlyHint": false, "destructiveHint": DESTRUCTIVE.contains(&name), "idempotentHint": IDEMPOTENT.contains(&name), "openWorldHint": false })
}

pub(crate) fn text(v: Value) -> Vec<Value> {
    vec![json!({ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() })]
}

/// A list as `{items, found}` (`found`: how many there were; `truncated: true` when `limit` cut it), `rest` beside.
pub(crate) fn listed(items: Value, limit: usize, rest: Value) -> Value {
    let mut list = items.as_array().cloned().unwrap_or_default();
    let found = list.len();
    list.truncate(limit);
    let mut v = rest;
    v["items"] = json!(list);
    v["found"] = json!(found);
    if found > limit {
        v["truncated"] = json!(true);
    }
    v
}

pub(crate) fn arg_str(a: &Value, k: &str) -> Result<String, String> {
    a[k].as_str().map(str::to_string).ok_or_else(|| format!("missing '{k}'"))
}

pub(crate) use crate::args::place;

tokio::task_local! {
    /// The world a call said it means (its `world`): edits check it again once they hold the gate.
    static WANT: u64;
}

/// The world the running call means, if it said.
pub fn wanted() -> Option<u64> {
    WANT.try_with(|w| *w).ok()
}

/// Refused: the call means another world than the one open.
pub fn other_world(open: Option<u64>, want: u64) -> String {
    match open {
        Some(o) => format!("mapd has another world open ({}), not {}: nothing was done", crate::hex(o), crate::hex(want)),
        None => format!("no world is open (asked for {}): open it in the map app", crate::hex(want)),
    }
}

pub async fn call(app: &Shared, name: &str, a: Value) -> Result<Vec<Value>, String> {
    // Old names folded, arguments checked against the tool's schema (batch steps come through here too).
    let a = crate::args::check(name, &a)?;
    let r = match a["world"].as_str() {
        None => Box::pin(dispatch(app, name, a)).await,
        Some(w) => {
            let want = u64::from_str_radix(w.trim(), 16).map_err(|_| format!("world: a hash as world_overview gives it, not {w}"))?;
            if app.current() != Some(want) {
                return Err(other_world(app.current(), want));
            }
            WANT.scope(want, Box::pin(dispatch(app, name, a))).await
        }
    };
    unsaved_note(app, name, r)
}

/// A write that went through while the world could not be saved says so (`saved: false`, once, even where
/// the tool's own reply was built after the save): an error would invite a repeat of a change that stands.
fn unsaved_note(app: &Shared, name: &str, r: Result<Vec<Value>, String>) -> Result<Vec<Value>, String> {
    let write = !READS.contains(&name) && name != "focus_view";
    if !write || crate::batch::active() || !app.unsaved.load(std::sync::atomic::Ordering::Relaxed) {
        return r;
    }
    let mut out = r?;
    if let Some(t) = out.first_mut()
        && let Some(mut v) = t["text"].as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()).filter(Value::is_object)
        && v.get("saved").is_none()
    {
        v["saved"] = json!(false);
        v["warning"] = json!(format!("made, but not yet saved to disk ({}): mapd keeps it and saves it again every few seconds; don't repeat this call", app.save_error.lock().unwrap()));
        *t = text(v).remove(0);
    }
    Ok(out)
}

async fn dispatch(app: &Shared, name: &str, a: Value) -> Result<Vec<Value>, String> {
    if let Some(r) = crate::batch::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::notebook::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::build::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::works::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::sites::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::scatter::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::crossings::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::ground::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::design::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::towns::call(app, name, &a).await {
        return r;
    }
    match name {
        "world_overview" => {
            let mut v = app.worker.with(|ex| Ok(agent::overview(&ex.world, &ex.t0))).await?;
            v["world"] = json!(app.current().map(crate::hex));
            Ok(text(v))
        }
        "search_features" => {
            let q = a["query"].as_str().unwrap_or("").to_string();
            let kind = a["kind"].as_str().map(str::to_string);
            let limit = a["limit"].as_u64().unwrap_or(25) as usize;
            let hidden = a["include_hidden"].as_bool().unwrap_or(false);
            // (All the matches are found, so `found` can say how many there were.)
            let all = app.worker.with(move |ex| Ok(agent::search(&ex.world, &ex.t0, &q, kind.as_deref(), 100_000, hidden))).await?;
            Ok(text(listed(all["results"].clone(), limit, json!({}))))
        }
        "get_feature" => {
            let id = arg_str(&a, "id")?;
            app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| crate::args::no_such(&ex.world, &ex.t0, &id))).await.map(text)
        }
        "list_children" => {
            let id = arg_str(&a, "id")?;
            app.worker.with(move |ex| agent::children(&ex.world, &ex.t0, &id).ok_or_else(|| deleted_or(&ex.world, &ex.t0, &id, format!("no such feature, or it contains nothing: {id}")))).await.map(text)
        }
        "list_names" => {
            let (kind, inside, settlement) = (a["kind"].as_str().map(str::to_string), a["inside"].as_str().map(str::to_string), a["settlement"].as_str().map(str::to_string));
            let v = app.worker.with(move |ex| agent::names(&ex.world, &ex.t0, kind.as_deref(), inside.as_deref(), settlement.as_deref())).await?;
            Ok(text(listed(v["names"].clone(), usize::MAX, json!({}))))
        }
        "describe_location" => {
            let p = place(app, &a).await?;
            app.worker.with(move |ex| Ok(agent::describe(&ex.world, &ex.t0, p[0], p[1]))).await.map(text)
        }
        "features_near" => {
            let p = place(app, &a).await?;
            let r = a["radius_mi"].as_f64().unwrap_or(5.0) * 5280.0;
            let kinds: Vec<String> = a["kinds"].as_array().map(|v| v.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let limit = a["limit"].as_u64().unwrap_or(50) as usize;
            let hidden = a["include_hidden"].as_bool().unwrap_or(false);
            let mut v = app.worker.with(move |ex| Ok(agent::near(&ex.world, &ex.t0, p, r, &kinds, limit, hidden))).await?;
            let (items, found) = (v["results"].take(), v["found"].clone());
            v["items"] = items;
            if found.as_u64().unwrap_or(0) > limit as u64 {
                v["truncated"] = json!(true);
            }
            if let Some(m) = v.as_object_mut() {
                m.remove("results");
            }
            Ok(text(v))
        }
        "route" => {
            let from = crate::args::point(app, "from", &a["from"]).await?;
            let to = crate::args::point(app, "to", &a["to"]).await?;
            app.worker.with(move |ex| Ok(agent::route(&ex.world, &ex.t0, from, to))).await.map(text)
        }
        "list_roads" => {
            let near = if a["at"].is_null() { None } else { Some((place(app, &a).await?, a["radius_mi"].as_f64().unwrap_or(25.0) * 5280.0)) };
            let settlement = a["settlement"].as_str().map(str::to_string);
            let class = a["class"].as_str().map(str::to_string);
            let drawn = a["drawn"].as_bool().unwrap_or(false);
            let limit = a["limit"].as_u64().unwrap_or(50) as usize;
            let mut v = app
                .worker
                .with(move |ex| {
                    let li = match &settlement {
                        Some(id) => Some(agent::layout_of(&ex.world, &ex.t0, id).filter(|&l| l < ex.t0.settlements.len()).ok_or_else(|| format!("{id} is not a settlement"))?),
                        None => None,
                    };
                    Ok(agent::roads(&ex.world, &ex.t0, near, li, class.as_deref(), drawn, limit))
                })
                .await?;
            let (found, items) = (v["count"].as_u64().unwrap_or(0), v["roads"].take());
            if let Some(m) = v.as_object_mut() {
                m.remove("count");
                m.remove("shown");
                m.remove("roads");
                m.insert("items".into(), items);
                m.insert("found".into(), json!(found));
                if found > limit as u64 {
                    m.insert("truncated".into(), json!(true));
                }
            }
            Ok(text(v))
        }
        "get_battlemap" => {
            let p = place(app, &a).await?;
            let r = a["radius_squares"].as_f64().unwrap_or(12.0);
            app.worker
                .run(move |g| {
                    let ex = g.ex.as_mut().ok_or("no world is open")?;
                    let geom = &ex.world.geom;
                    let size = geom.tile_size_ft(geom.max_level);
                    let key = TileKey::surface(geom.max_level, (p[0] / size).floor().max(0.0) as u32, (p[1] / size).floor().max(0.0) as u32);
                    let origin = [key.x as f64 * size, key.y as f64 * size];
                    let chunk = ex.battlemap(key);
                    Ok::<Value, String>(agent::battlemap_summary(&ex.world, &chunk, origin, p, r))
                })
                .await
                .map(text)
        }
        "render_view" => {
            let p = place(app, &a).await?;
            let size = a["size_ft"].as_f64().unwrap_or(3000.0);
            let reply = app.ask_app(json!({ "type": "render", "x": p[0], "y": p[1], "size": size }), 45).await?;
            let png = reply["png"].as_str().ok_or_else(|| format!("the app could not render: {}", reply["error"].as_str().unwrap_or("no image")))?;
            let data = png.split(',').next_back().unwrap_or(png);
            Ok(vec![
                json!({ "type": "image", "data": data, "mimeType": "image/png" }),
                json!({ "type": "text", "text": format!("View centred at ({:.0}, {:.0}) ft, spanning about {size:.0} ft.", p[0], p[1]) }),
            ])
        }
        "focus_view" => {
            let p = place(app, &a).await?;
            let size = a["size_ft"].as_f64().unwrap_or(3000.0);
            if app.followers().is_empty() {
                return Err(app.no_app());
            }
            app.send(json!({ "type": "focus", "x": p[0], "y": p[1], "size": size, "world": app.current().map(crate::hex) }));
            Ok(text(json!({ "focused": [p[0].round(), p[1].round()], "size_ft": size })))
        }
        "rename_feature" => {
            let id = arg_str(&a, "id")?;
            let name = arg_str(&a, "name")?.trim().to_string();
            known(app, &id).await?;
            // A building drawn by hand keeps its name in its entry, whichever of its ids is given (as the app does).
            let q = id.clone();
            let drawn = app.worker.with(move |ex| Ok(agent::drawn_building_ids(&ex.world, &ex.t0, &q))).await?;
            app.edit("agent", 0, move |e| {
                if let Some((k, cid, bid)) = drawn.filter(|d| !name.is_empty() && e.created.get(d.0).is_some_and(|c| c.id == d.1)) {
                    e.renames.remove(&cid);
                    e.renames.remove(&bid);
                    e.created[k].name = name.clone();
                    return Ok(json!({ "tool": "rename_feature", "id": id, "name": name }));
                }
                if name.is_empty() {
                    e.renames.remove(&id);
                } else {
                    e.renames.insert(id.clone(), name.clone());
                }
                if let Some(c) = e.created.iter_mut().find(|c| c.id == id)
                    && !name.is_empty()
                {
                    c.name = name.clone();
                }
                Ok(json!({ "tool": "rename_feature", "id": id, "name": name }))
            })
            .await
            .map(text)
        }
        "annotate_feature" => {
            let id = arg_str(&a, "id")?;
            let t = arg_str(&a, "notes")?;
            let tags: Vec<String> = a["tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let append = a["append"].as_bool().unwrap_or(false);
            known(app, &id).await?;
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                let key = note_key(e, pair, &id);
                let n = e.notes.entry(key.clone()).or_insert_with(Note::default);
                n.text = if append && !n.text.is_empty() { format!("{}\n\n{t}", n.text) } else { t };
                for tag in tags {
                    if !n.tags.contains(&tag) {
                        n.tags.push(tag);
                    }
                }
                Ok(json!({ "tool": "annotate_feature", "id": key }))
            })
            .await
            .map(text)
        }
        "update_feature" => {
            let id = arg_str(&a, "id")?;
            if ["name", "notes", "tags"].iter().all(|k| a[k].is_null()) {
                return Err(crate::args::bad("name, notes, tags", "give at least one", &json!({})));
            }
            known(app, &id).await?;
            let name = a["name"].as_str().map(|s| s.trim().to_string());
            let notes = a["notes"].as_str().map(str::to_string);
            let tags: Option<Vec<String>> = a["tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect());
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                if let Some(n) = &name {
                    // A building drawn by hand keeps its name in its entry (as `rename_feature`).
                    let drawn = pair.as_ref().filter(|p| !n.is_empty() && e.created.get(p.0).is_some_and(|c| c.id == p.1));
                    if let Some((k, cid, bid)) = drawn {
                        e.renames.remove(cid);
                        e.renames.remove(bid);
                        e.created[*k].name = n.clone();
                    } else {
                        e.renames.insert(id.clone(), n.clone());
                        if let Some(c) = e.created.iter_mut().find(|c| c.id == id) {
                            c.name = n.clone();
                        }
                    }
                }
                if notes.is_some() || tags.is_some() {
                    let key = note_key(e, pair.clone(), &id);
                    let n = e.notes.entry(key).or_insert_with(Note::default);
                    if let Some(t) = notes {
                        n.text = t;
                    }
                    if let Some(t) = tags {
                        n.tags = t;
                    }
                }
                Ok(json!({ "tool": "update_feature", "id": id }))
            })
            .await
            .map(text)
        }
        "hide_feature" => {
            let id = arg_str(&a, "id")?;
            let hide = a["hidden"].as_bool().unwrap_or(true);
            known(app, &id).await?;
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                // (A building drawn by hand: its mark kept under its created id.)
                let key = match pair.filter(|p| e.created.get(p.0).is_some_and(|c| c.id == p.1)) {
                    Some((_, c, b)) => {
                        e.hidden.remove(&b);
                        c
                    }
                    None => id.clone(),
                };
                if hide {
                    e.hidden.insert(key.clone());
                } else {
                    e.hidden.remove(&key);
                }
                Ok(json!({ "tool": "hide_feature", "id": key, "hidden": hide }))
            })
            .await
            .map(text)
        }
        "create_feature" => create(app, a).await.map(text),
        _ => Err(format!("unknown tool: {name}")),
    }
}

/// The reply for an id that gave nothing: what it was deleted with (a deleted site), else `other`.
pub(crate) fn deleted_or(world: &worldgen::world::World, t0: &worldgen::t0::T0, id: &str, other: String) -> String {
    agent::deleted_note(world, t0, id).unwrap_or(other)
}

/// An id the world knows (so edits never pile up on typos).
pub(crate) async fn known(app: &Shared, id: &str) -> Result<(), String> {
    let id2 = id.to_string();
    app.worker.with(move |ex| agent::position(&ex.world, &ex.t0, &id2).map(|_| ()).ok_or_else(|| crate::args::no_such(&ex.world, &ex.t0, &id2))).await
}

/// A building drawn by hand behind an id (`agent::drawn_pair`: its place in `created`, created
/// and building ids), else None.
async fn drawn_pair(app: &Shared, id: &str) -> Result<Option<(usize, String, String)>, String> {
    let q = id.to_string();
    app.worker.with(move |ex| Ok(agent::drawn_pair(&ex.world, &ex.t0, &q))).await
}

/// The key a place's note is kept under: a building drawn by hand's created id (`pair`, while
/// `created` still holds it there), its note under its building id (written before) joined
/// onto it; else `id`.
fn note_key(e: &mut Edits, pair: Option<(usize, String, String)>, id: &str) -> String {
    match pair.filter(|p| e.created.get(p.0).is_some_and(|c| c.id == p.1)) {
        Some((_, c, b)) => {
            if let Some(old) = e.notes.remove(&b) {
                let joined = Note::join(e.notes.get(&c), Some(&old));
                e.notes.extend(joined.map(|n| (c.clone(), n)));
            }
            c
        }
        None => id.to_string(),
    }
}

async fn create(app: &Shared, a: Value) -> Result<Value, String> {
    let kind = arg_str(&a, "kind")?;
    if kind == "building" {
        return Err("buildings are drawn with create_building".into());
    }
    if kind == "castle" || kind == "wall" {
        return crate::works::place(app, &a, None, Some(&kind)).await;
    }
    let opt = |k: &str| a[k].as_str().map(str::to_string);
    let levels = match &a["levels"] {
        Value::Null => None,
        v => Some(v.as_u64().filter(|l| *l <= 255).ok_or("levels: a whole number")? as u8),
    };
    let mut c = Created { kind: kind.clone(), under: opt("under"), size: opt("size"), levels, theme: opt("theme"), ..Default::default() };
    c.check()?;
    let p = place(app, &a).await?;
    let asked = a["name"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    // On dry land inside the map, named (the local naming culture's) if no name was given.
    let n = app.with_edits(|e| e.created.len()).unwrap_or(0);
    c.id = format!("c:{n}");
    let (id, under) = (c.id.clone(), c.under.clone());
    // The same site again (a script run twice): that one, nothing added (checked again below,
    // at the spot the site would take).
    let at = p;
    (c.x, c.y) = (p[0], p[1]);
    let existing = app.with_edits(|e| e.existing_site(&c, at).map(|o| o.id.clone())).flatten();
    if let Some(id) = existing {
        return found(app, id).await;
    }
    let (kind2, asked_at) = (kind.clone(), p);
    let (p, name) = app.worker.with(move |ex| agent::creation_spot(&ex.world, &ex.t0, &kind, under.as_deref(), &id, p)).await?;
    // Put elsewhere than asked (clear of a river, a camp's clearing kept in its chunk): said so when it is more than 5 ft.
    let ft = (p[0] - asked_at[0]).hypot(p[1] - asked_at[1]);
    let moved = (ft > 5.0).then(|| {
        let why = if kind2 == "camp" { "kept clear of rivers (150 ft) and 70 ft inside its battlemap chunk" } else { "kept clear of rivers (150 ft)" };
        json!({ "x_ft": p[0].round(), "y_ft": p[1].round(), "ft": ft.round(), "why": why })
    });
    (c.x, c.y, c.name) = (p[0], p[1], asked.unwrap_or(name));
    let change = app
        .edit("agent", 0, move |e| {
            if let Some(o) = e.existing_site(&c, at) {
                return Ok(json!({ "existing": o.id }));
            }
            // (Another client may have created a site meanwhile: take the next id.)
            c.id = format!("c:{}", e.created.len());
            let reply = json!({ "tool": "create_feature", "id": c.id, "kind": c.kind, "name": c.name, "x_ft": p[0].round(), "y_ft": p[1].round() });
            e.created.push(c);
            Ok(reply)
        })
        .await?;
    if let Some(id) = change["existing"].as_str() {
        return found(app, id.to_string()).await;
    }
    let id = change["id"].as_str().unwrap_or_default().to_string();
    let mut v = app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "created, but it could not be read back".to_string())).await?;
    if let Some(m) = moved {
        v["moved"] = m;
    }
    Ok(v)
}

/// A site asked to be created that was already there (`Created::same_site`), as `get_feature`
/// has it (a building with its building id), marked `existing`.
pub(crate) async fn found(app: &Shared, id: String) -> Result<Value, String> {
    let mut v = app
        .worker
        .with(move |ex| {
            let mut v = agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "it could not be read back".to_string())?;
            if ex.world.file.edits.created.iter().any(|c| c.id == id && c.kind == "building")
                && let Some(li) = agent::layout_of(&ex.world, &ex.t0, &id)
            {
                v["building"] = json!(format!("b:{li}:0"));
            }
            Ok::<_, String>(v)
        })
        .await?;
    // (Its description says what it is, not that it was just made.)
    if let Some(d) = v["detail"].as_str().and_then(|d| d.strip_prefix("created ")) {
        v["detail"] = json!(format!("already here, not made again: {d}"));
    }
    v["existing"] = json!(true);
    v["note"] = json!("this was already here: its id is returned and nothing was added (change something to make a second one: another name or place)");
    Ok(v)
}
