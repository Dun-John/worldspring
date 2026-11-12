//! The MCP tools: read the world, look at it through the open app, and edit it. Every edit
//! is saved, logged and shown live in the app.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::core::tile::TileKey;
use worldgen::under::{MAX_LEVELS, SiteSize, THEMES, UnderKind};
use worldgen::world::{CREATABLE, Created, Edits, Note};

use crate::Shared;

pub const INSTRUCTIONS: &str = "A procedurally generated fantasy world (D&D 5e scale: 5-ft squares). Coordinates are feet from the map's top-left corner, x east, y south. \
Find places with search_features, features_near or world_overview; inspect them with get_feature and list_children. A place (at) is an id, {x_ft, y_ft} or [x_ft, y_ft]. Ids carry their kind: c: a created site, b: a building, d: a district, u: a site underground, l:/r: its levels and rooms, \
n: an NPC, p: a plot point, o: a placed object, x: a clear, v: a crossing, rd: a road stretch, h: a plateau; get one wrong and search_features finds the right one. \
Edits (rename, annotate, hide, create_feature, create_building, place_crossing, place_objects, shape_ground, set_plateau, edit_town, set_site_design) are saved and appear live in the open app; \
many at once go in one batch. Roads and routes: list_roads, route, road_point (name a road by renaming its rd: stretches). Geometry: nearest (coast, cliff, river, road, flat, highest), search_features within an area. Read back hand work: list_objects, list_plateaus, list_designs. The DM's notebook holds NPCs and plot points tied to places (create_npc, create_plot). \
render_view and focus_view need the app open in a visible tab. Habits that avoid trouble: check_building_spot (or a dry_run where a tool has one) before a write; \
an error means nothing changed, except a save warning (saved: false), which is not to be repeated; the creates are safe to repeat (the same one again returns what is there); \
deletes come back with restore_feature, restore_objects and restore_building. Lists are cut by limit (found says how many there were). \
Out-of-range values are refused, never clamped. Every tool takes an optional world (the hash world_overview gives) and refuses to run if another world is open. \
Not possible: reshaping the sketch, drawing borders, founding or deleting settlements, moving a feature. The full reference is docs/MCP.md.";

/// Kinds search finds besides the features' own (layouts' parts, road stretches, crossings).
const SEARCH_KINDS: [&str; 9] = ["building", "district", "tower", "road", "crossing", "site", "underground", "level", "room"];

pub(crate) fn schema(props: Value, required: &[&str]) -> Value {
    let mut s = json!({ "type": "object", "properties": props });
    if !required.is_empty() {
        s["required"] = json!(required);
    }
    s
}

fn place_props() -> Value {
    crate::args::at_props()
}

pub(crate) fn merge(a: Value, b: Value) -> Value {
    let mut a = a;
    if let (Some(m), Value::Object(n)) = (a.as_object_mut(), b) {
        m.extend(n);
    }
    a
}

pub fn list() -> Value {
    let mut v = json!([
        { "name": "world_overview", "title": "World overview", "description": "The world's size, land masses, counts of named features and its largest settlements.", "inputSchema": schema(json!({}), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "search_features", "title": "Search", "description": "Named features matching a query (towns, ranges, rivers, ruins...; from three letters also districts, businesses and sites underground by name, kind or theme). Give a query, kind, tag, within or at. Returns {items, found}; limit cuts it and sets truncated.", "inputSchema": schema(json!({ "query": { "type": "string", "default": "" }, "kind": { "type": "string", "description": "Only this kind (city, ruin, river, building, underground...)" }, "tag": { "type": "string", "description": "Only features whose notes carry this tag" }, "in_notes": { "type": "boolean", "default": false, "description": "The query also matches notes" }, "within": crate::args::area_light("Only features inside (a point) or overlapping: a box, polygon or circle {at, radius_ft}"), "at": { "type": ["string", "array", "object"], "description": "Only within radius_ft of this place, nearest first" }, "radius_ft": { "type": "number", "default": 26400 }, "underground_kinds": { "type": "array", "items": { "type": "string" }, "description": "dungeon, crypt, catacombs, cave, mine, lava tube, sewer, keep dungeon" }, "levels_min": { "type": "integer" }, "levels_max": { "type": "integer" }, "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 25 }, "include_hidden": { "type": "boolean", "default": false } }), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "get_feature", "title": "Feature details", "description": "A feature's details and relations: where and what it is, extent, notes, regions, nearest settlements, roads, districts, notable buildings, ways underground, sites in it, levels and rooms; for anything in a layout, its settlement and site. NPCs and plots there.", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_children", "title": "Children", "description": "What a feature contains: a settlement's districts, businesses and sites, a building's or site's levels and rooms, a region's settlements and sites. Each list is cut to limit (default 200, max 2000); found gives the full count and truncated is true.", "inputSchema": schema(json!({ "id": { "type": "string" }, "limit": limit_prop(200, 2000) }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_names", "title": "Names", "description": "Everything that can be renamed, with id, kind, current and generated name; with settlement its districts, businesses, towers and sites; with inside (a building or site) its levels and rooms. Returns {items, found}; limit (default 200, max 2000) cuts it.", "inputSchema": schema(json!({ "kind": { "type": "string", "description": "Only this kind (river, city, building, site, room...)" }, "settlement": { "type": "string" }, "inside": { "type": "string" }, "function": { "type": "string", "description": "Only businesses of this function (inn, blacksmith...), in a settlement or site" }, "limit": limit_prop(200, 2000) }), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "describe_location", "title": "What is here", "description": "What is at a place: elevation, slope_deg, biome (the coarse map's, so a dry point can read ocean), water (a river or lake within 1000 ft), coast (the sea within 5 mi), areas, the building, district or site there, named features nearby, nearest settlements.", "inputSchema": schema(place_props(), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "features_near", "title": "Near a place", "description": "Everything named within a radius of a place, nearest first, with distance, direction and extent. Returns {items, found}; limit (default 50) cuts it. kinds limits it; districts and businesses only when asked for or within a mile.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "minimum": 0.01, "maximum": 500, "default": 5 }, "kinds": { "type": "array", "items": { "type": "string" } }, "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 50 }, "include_hidden": { "type": "boolean", "default": false } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "route", "title": "Route", "description": "Distance between two places by road (where roads join them) or overland, with D&D 5e travel days (normal, fast, slow pace). by_road lists the stretches (rd:) it takes and the named roads it follows.", "inputSchema": schema(json!({ "from": crate::args::point_schema("Where from: an id, a place's name, {x_ft, y_ft} or [x_ft, y_ft]"), "to": crate::args::point_schema("Where to: an id, a place's name, {x_ft, y_ft} or [x_ft, y_ft]") }), &["from", "to"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_roads", "title": "Roads", "description": "The road network: sketch-drawn roads, then roads between settlements and junctions, longest first, with id (rd:), class, length, ends and name. Returns {items, found, named}; limit cuts it. Filter by at and radius_mi, settlement, class or drawn.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "minimum": 0.01, "maximum": 3000, "default": 25 }, "settlement": { "type": "string", "description": "Only roads ending at this settlement" }, "class": { "type": "string", "description": "king's road (also kings road, kings_road), road or track; results say \"king's road\"" }, "drawn": { "type": "boolean", "default": false, "description": "Only roads drawn in the sketch" }, "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 50 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "get_battlemap", "title": "Battlemap", "description": "The battlemap (a 640-ft square of 5-ft squares) at a place: elevation range, surfaces, buildings, every kind of object with its tactical rules, and the objects near the place by square.", "inputSchema": schema(merge(place_props(), json!({ "radius_squares": { "type": "number", "minimum": 1, "maximum": 64, "default": 12 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "render_view", "title": "Look at the map", "description": "A screenshot of the open map app at a place (JPEG 1024x683; width_px 256-1536, format png). Needs the app open in a visible tab with the map generated. size_ft is the ground across: ~450 a battlemap, 3000 a village, 20000 a city.", "inputSchema": schema(merge(place_props(), json!({ "size_ft": { "type": "number", "minimum": 50, "maximum": 5000000, "default": 3000 }, "width_px": { "type": "integer", "minimum": 256, "maximum": 1536, "default": 1024, "description": "Pixels across (height 2/3 of it)" }, "format": { "type": "string", "enum": ["jpeg", "png"], "default": "jpeg" } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "focus_view", "title": "Show the user", "description": "Move the open map app's view to a place (the user sees it).", "inputSchema": schema(merge(place_props(), json!({ "size_ft": { "type": "number", "minimum": 50, "maximum": 5000000, "default": 3000 } })), &[]) },
        { "name": "rename_feature", "title": "Rename", "description": "Rename anything named: a feature, building, district, tower, site, a site's level (l:<site>:<level>) or room (r:<site>:<level>:<room>), a road stretch (rd:) or crossing (v:). Empty restores the original.", "inputSchema": schema(json!({ "id": { "type": "string" }, "name": { "type": "string" } }), &["id", "name"]) },
        { "name": "annotate_feature", "title": "Add notes", "description": "Set (or append to) a feature's notes: lore, hooks, secrets, with tags. A building drawn by hand is one place: either of its ids.", "inputSchema": schema(json!({ "id": { "type": "string" }, "notes": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } }, "remove_tags": { "type": "array", "items": { "type": "string" }, "description": "Tags to take off (notes may be left out when tags or remove_tags is given)" }, "append": { "type": "boolean", "default": false } }), &["id"]) },
        { "name": "create_feature", "title": "Create a site", "description": "Add a site on dry land: ruin (over a dungeon, crypt or catacombs), tower, camp, waystation, cave, mine, lava_tube, or entrance (a way down); something underground takes size, levels and theme. Or a castle (poly/rect) or a wall (pts or around_patches), drawn not placed; the world's own buildings in the way are named, or taken away with remove_in_way. Returns its id and underground ways.", "inputSchema": schema(merge(merge(place_props(), crate::works::props()), json!({
            "kind": { "type": "string", "enum": CREATABLE.iter().filter(|k| **k != "building").collect::<Vec<_>>() },
            "name": { "type": "string" },
            "under": { "type": "string", "enum": UnderKind::CREATABLE.map(|k| k.key()), "description": "Beneath a ruin (dungeon, crypt, catacombs) or an entrance (any)" },
            "size": { "type": "string", "enum": SiteSize::NAMES, "description": "The site underground" },
            "levels": { "type": "integer", "minimum": 1, "maximum": MAX_LEVELS },
            "theme": { "type": "string", "enum": THEMES.iter().map(|t| t.key).collect::<Vec<_>>(), "description": "Themes by kind: MCP.md" },
            "in_building": { "type": "string", "description": "Entrance in its cellar" },
            "way": { "type": "string", "enum": ["stairs", "trapdoor"] },
        })), &["kind"]) },
        { "name": "update_feature", "title": "Update", "description": "Change a feature's name and/or notes in one go. A building drawn by hand is one place: either of its ids. Also r: rooms, m: notes (at moves an m: note).", "inputSchema": schema(json!({ "id": { "type": "string" }, "name": { "type": "string" }, "notes": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } }, "in_building": { "type": ["string", "null"], "description": "Entrance: into this building; null: out" }, "way": { "type": "string" }, "at": {} }), &["id"]) },
        { "name": "hide_feature", "title": "Hide", "description": "Hide a feature from the map's labels and search (or show it again). It is still there. A building drawn by hand is one place: either of its ids (its business pin is hidden too).", "inputSchema": schema(json!({ "id": { "type": "string" }, "hidden": { "type": "boolean", "default": true } }), &["id"]) },
        { "name": "delete_feature", "title": "Delete a created site", "description": "Delete created sites and buildings drawn by hand (id or ids, all or none). Notes, NPCs and plots pointing at them are kept (left_pointing). restore_feature undoes it. The world's own buildings go with remove_buildings; other generated features can only be hidden.", "inputSchema": schema(crate::args::ids_props("created site (c:<n>, or a drawn building's b:<layout>:0)"), &[]) },
    ]);
    if let Some(a) = v.as_array_mut() {
        a.extend(crate::notebook::list());
        a.extend(crate::sites::list());
        a.extend(crate::scatter::list());
        a.extend(crate::build::list());
        a.extend(crate::works::list());
        a.extend(crate::crossings::list());
        a.extend(crate::hexes::list());
        a.extend(crate::ground::list());
        a.extend(crate::harbour::list());
        a.extend(crate::design::list());
        a.extend(crate::reads::list());
        a.extend(crate::streets::list());
        a.extend(crate::geo::list());
        a.extend(crate::towns::list());
        a.extend(crate::settlements::list());
        a.extend(crate::sketch::list());
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
            t["inputSchema"]["properties"]["world"] = json!({ "type": "string" });
        }
    }
    v
}

/// Tools that only read (or look): `readOnlyHint`.
const READS: &[&str] = &[
    "world_overview", "search_features", "get_feature", "list_children", "list_names", "describe_location", "features_near", "route", "list_roads", "get_battlemap", "render_view",
    "list_crossings", "list_wharves", "list_objects", "list_plateaus", "list_designs", "nearest", "road_point", "get_site_design", "ground_heights", "list_npcs", "get_npc", "list_plots", "get_plot", "list_notes", "list_sprites", "check_building_spot", "get_town_plan", "list_streets", "get_sketch",
];
/// Writes that delete, replace or reset work (`destructiveHint`); every other write only adds or adjusts.
const DESTRUCTIVE: &[&str] = &[
    "delete_feature", "delete_npc", "delete_plot", "delete_note", "delete_plateau", "remove_wharves", "remove_objects", "remove_buildings", "remove_crossings", "reset_site_design", "set_site_design", "edit_town", "batch",
];
/// Writes that leave the world the same when repeated with the same arguments (`idempotentHint`): setters, restorers,
/// `focus_view` and the creates that return what is already there.
const IDEMPOTENT: &[&str] = &[
    "focus_view", "rename_feature", "update_feature", "hide_feature", "create_feature", "restore_feature", "create_building", "update_building", "restore_building", "create_npc",
    "update_npc", "place_npc", "move_note", "create_plot", "update_plot", "place_objects", "restore_objects", "set_site_design", "reset_site_design", "set_plateau", "raise_islet", "update_fortification", "create_settlement", "update_settlement", "create_sketch_stroke",
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
    vec![json!({ "type": "text", "text": serde_json::to_string(&v).unwrap_or_default() })]
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

/// A `limit` property: whole numbers `1..=max`, `default` when left out (every list tool has one).
pub(crate) fn limit_prop(default: u64, max: u64) -> Value {
    json!({ "type": "integer", "minimum": 1, "maximum": max, "default": default })
}

/// The `limit` a call gave, else `default`.
pub(crate) fn limit_of(a: &Value, default: usize) -> usize {
    a["limit"].as_u64().map(|l| l as usize).unwrap_or(default)
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
    let before = app.changes.load(std::sync::atomic::Ordering::Relaxed);
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
    unsaved_note(app, name, r, app.changes.load(std::sync::atomic::Ordering::Relaxed) != before)
}

/// A write that went through while the world could not be saved says so (`saved: false`, once, even where
/// the tool's own reply was built after the save): an error would invite a repeat of a change that stands.
fn unsaved_note(app: &Shared, name: &str, r: Result<Vec<Value>, String>, changed: bool) -> Result<Vec<Value>, String> {
    let write = !READS.contains(&name) && name != "focus_view";
    if !write || !changed || crate::batch::active() || !app.unsaved.load(std::sync::atomic::Ordering::Relaxed) {
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
    if let Some(r) = crate::hexes::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::crossings::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::ground::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::harbour::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::design::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::towns::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::reads::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::streets::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::geo::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::settlements::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::sketch::call(app, name, &a).await {
        return r;
    }
    match name {
        "world_overview" => {
            let mut v = app.worker.with(|ex| Ok(agent::overview(&ex.world, &ex.t0))).await?;
            v["world"] = json!(app.current().map(crate::hex));
            Ok(text(v))
        }
        "search_features" => {
            let (q, kind, tag) = (a["query"].as_str().unwrap_or("").to_string(), a["kind"].as_str().map(str::to_string), a["tag"].as_str().map(|t| t.trim().to_string()));
            let narrows = !a["underground_kinds"].is_null() || !a["levels_min"].is_null() || !a["levels_max"].is_null();
            if q.trim().is_empty() && kind.is_none() && tag.is_none() && a["within"].is_null() && a["at"].is_null() && !narrows {
                return Err(crate::args::bad("query, kind, tag, within, at", "give at least one (world_overview lists the largest places; list_names everything renameable)", &json!({})));
            }
            if !a["within"].is_null() && !a["at"].is_null() {
                return Err("within, at: give one (within takes a circle {at, radius_ft} too)".into());
            }
            if !a["radius_ft"].is_null() && a["at"].is_null() {
                return Err("radius_ft: needs at".into());
            }
            let within = if a["within"].is_null() { None } else { Some(crate::args::area(app, "within", &a["within"]).await?) };
            let near = if a["at"].is_null() { None } else { Some(crate::args::point(app, "at", &a["at"]).await?) };
            let radius = a["radius_ft"].as_f64().unwrap_or(5.0 * 5280.0);
            if !(radius > 0.0) {
                return Err(crate::args::bad("radius_ft", "more than 0", &a["radius_ft"]));
            }
            // Underground filters: only sites below ground can meet them.
            let mut under = worldgen::undersearch::UnderOpts::from_json(&json!({ "kinds": a["underground_kinds"], "levels_min": a["levels_min"], "levels_max": a["levels_max"] })).map_err(|e| format!("underground_{e}"))?;
            if let (Some(lo), Some(hi)) = (under.levels_min, under.levels_max)
                && lo > hi
            {
                return Err(format!("levels_min: {lo} is more than levels_max {hi}"));
            }
            under.near = near;
            if narrows && kind.as_deref().is_some_and(|k| k != "underground") {
                return Err(format!("underground_kinds, levels_min, levels_max: only for sites underground (kind \"underground\" or none), not kind \"{}\"", kind.as_deref().unwrap_or("")));
            }
            let kind = if narrows { Some("underground".to_string()) } else { kind };
            if a["in_notes"].as_bool() == Some(true) && q.trim().is_empty() {
                return Err("in_notes: needs a query".into());
            }
            let limit = a["limit"].as_u64().unwrap_or(25) as usize;
            // Buildings and districts are matched by name only (3 letters or more), within an area too.
            let by_name_only = !a["within"].is_null() && q.trim().chars().count() < 3 && matches!(kind.as_deref(), Some("building" | "district"));
            let opts = agent::SearchOpts { query: q, kind, tag, in_notes: a["in_notes"].as_bool().unwrap_or(false), hidden: a["include_hidden"].as_bool().unwrap_or(false), within: within.map(|w| w.region()).or(near.map(|p| worldgen::geoquery::Region::Circle(p, radius))), under: Some(under) };
            // (All the matches are found, so `found` can say how many there were.)
            let all = app
                .worker
                .with(move |ex| {
                    let v = agent::search_with(&ex.world, &ex.t0, &opts, 100_000);
                    // A kind nothing has is a typo, not an empty answer (as list_names).
                    if let Some(k) = opts.kind.as_deref().filter(|_| v["results"].as_array().is_some_and(|r| r.is_empty())) {
                        let mut kinds: Vec<&str> = agent::features(&ex.world, &ex.t0).iter().map(|f| f.kind).collect();
                        kinds.extend(SEARCH_KINDS);
                        kinds.sort_unstable();
                        kinds.dedup();
                        if !kinds.contains(&k) {
                            let hint = crate::args::suggest(k, kinds.iter().copied()).map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default();
                            return Err(format!("kind: nothing of kind \"{k}\"{hint}; the kinds here: {}", kinds.join(", ")));
                        }
                    }
                    Ok(v)
                })
                .await?;
            let mut v = listed(all["results"].clone(), limit, json!({}));
            if by_name_only && v["found"] == 0 {
                v["note"] = json!("buildings and districts are found by a query of 3 or more letters of their name (within narrows those)");
            }
            Ok(text(v))
        }
        "get_feature" => {
            let id = arg_str(&a, "id")?;
            app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| crate::args::no_such(&ex.world, &ex.t0, &id))).await.map(text)
        }
        "list_children" => {
            let id = arg_str(&a, "id")?;
            let limit = limit_of(&a, 200);
            let mut v = app.worker.with(move |ex| agent::children(&ex.world, &ex.t0, &id).ok_or_else(|| deleted_or(&ex.world, &ex.t0, &id, format!("no such feature, or it contains nothing: {id}")))).await?;
            // Each list is cut to `limit`; `found` has the full counts of the ones that were cut.
            let mut found = serde_json::Map::new();
            for (k, l) in v.as_object_mut().into_iter().flatten() {
                if let Some(arr) = l.as_array_mut().filter(|x| x.len() > limit) {
                    found.insert(k.clone(), json!(arr.len()));
                    arr.truncate(limit);
                }
            }
            if !found.is_empty() {
                v["found"] = Value::Object(found);
                v["truncated"] = json!(true);
            }
            Ok(text(v))
        }
        "list_names" => {
            let (kind, inside, settlement) = (a["kind"].as_str().map(str::to_string), a["inside"].as_str().map(str::to_string), a["settlement"].as_str().map(str::to_string));
            let limit = limit_of(&a, 200);
            let function = a["function"].as_str().map(|f| f.trim().to_lowercase());
            if function.is_some() && settlement.is_none() {
                return Err(crate::args::bad("function", "used with settlement (a settlement's or site's id)", &a["function"]));
            }
            let v = app
                .worker
                .with(move |ex| {
                    let mut v = agent::names(&ex.world, &ex.t0, kind.as_deref(), inside.as_deref(), settlement.as_deref())?;
                    if let Some(f) = function.as_deref() {
                        let keys: Vec<&str> = worldgen::town::catalog::CATALOG.iter().map(|c| c.key).collect();
                        let label = worldgen::town::catalog::index_of(f).map(|i| worldgen::town::catalog::CATALOG[i as usize].name.to_lowercase());
                        if label.is_none() && !v["names"].as_array().into_iter().flatten().any(|n| n["function"].as_str().is_some_and(|x| x.to_lowercase() == f)) {
                            let hint = crate::args::suggest(f, keys.iter().copied()).map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default();
                            return Err(format!("function: no business \"{f}\"{hint}; the keys are as create_building's func"));
                        }
                        let want = label.unwrap_or_else(|| f.to_string());
                        if let Some(l) = v["names"].as_array_mut() {
                            l.retain(|n| n["function"].as_str().is_some_and(|x| x.to_lowercase() == want));
                        }
                    }
                    // A kind nothing has is a typo, not an empty answer.
                    if let Some(k) = kind.as_deref().filter(|_| v["names"].as_array().is_some_and(|n| n.is_empty())) {
                        let all = agent::names(&ex.world, &ex.t0, None, inside.as_deref(), settlement.as_deref())?;
                        let mut kinds: Vec<&str> = all["names"].as_array().into_iter().flatten().filter_map(|n| n["kind"].as_str()).collect();
                        // (Kinds that only a scoped list has are known too: settlement and inside.)
                        kinds.extend(["site", "district", "building", "tower", "underground", "level", "room", "road", "crossing"]);
                        kinds.sort_unstable();
                        kinds.dedup();
                        if !kinds.contains(&k) {
                            let hint = crate::args::suggest(k, kinds.iter().copied()).map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default();
                            return Err(format!("kind: no names of kind \"{k}\"{hint}; the kinds here: {}", kinds.join(", ")));
                        }
                    }
                    Ok(v)
                })
                .await?;
            Ok(text(listed(v["names"].clone(), limit, json!({}))))
        }
        "describe_location" => {
            let p = place(app, &a).await?;
            app.worker.with_mut(move |ex| Ok(worldgen::geoquery::describe(ex, p))).await.map(text)
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
            let (from, fe) = crate::args::point_named(app, "from", &a["from"]).await?;
            let (to, te) = crate::args::point_named(app, "to", &a["to"]).await?;
            let mut v = app.worker.with(move |ex| Ok(agent::route(&ex.world, &ex.t0, from, to))).await?;
            // A place given by name comes back with its id.
            if let Some(e) = fe {
                v["from_place"] = e;
            }
            if let Some(e) = te {
                v["to_place"] = e;
            }
            Ok(text(v))
        }
        "list_roads" => {
            let near = if a["at"].is_null() { None } else { Some((place(app, &a).await?, a["radius_mi"].as_f64().unwrap_or(25.0) * 5280.0)) };
            let settlement = a["settlement"].as_str().map(str::to_string);
            let class = a["class"].as_str().map(str::to_string);
            if let Some(c) = &class
                && !["king's road", "road", "track"].contains(&c.to_lowercase().replace('_', " ").replace("kings", "king's").as_str())
            {
                return Err(crate::args::bad("class", "king's road (or kings_road), road or track", &json!(c)));
            }
            let drawn = a["drawn"].as_bool().unwrap_or(false);
            let limit = a["limit"].as_u64().unwrap_or(50) as usize;
            let mut v = app
                .worker
                .with(move |ex| {
                    let li = match &settlement {
                        Some(id) => Some(agent::layout_of(&ex.world, &ex.t0, id).filter(|&l| worldgen::town::is_settlement(&ex.t0, l)).ok_or_else(|| format!("{id} is not a settlement"))?),
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
            let width = a["width_px"].as_u64().unwrap_or(1024);
            let png = a["format"].as_str() == Some("png");
            let reply = app.ask_app(json!({ "type": "render", "x": p[0], "y": p[1], "size": size, "width_px": width, "format": if png { "png" } else { "jpeg" } }), 45).await.map_err(|e| {
                format!("{e}; render_view needs the map app open in a visible tab, with the place generated (give it a few seconds and try again, or try a larger size_ft)")
            })?;
            let img = reply["image"].as_str().or_else(|| reply["png"].as_str()).ok_or_else(|| format!("the app could not render: {} (the tab must be visible and the map generated; try again, or a larger size_ft)", reply["error"].as_str().unwrap_or("no image")))?;
            let data = img.split(',').next_back().unwrap_or(img);
            // (Labelled by what the app sent, as its data URL says: an older app answers png whatever was asked.)
            let mime = img.strip_prefix("data:").and_then(|r| r.split(';').next()).filter(|m| m.starts_with("image/")).unwrap_or(if png { "image/png" } else { "image/jpeg" });
            Ok(vec![
                json!({ "type": "image", "data": data, "mimeType": mime }),
                json!({ "type": "text", "text": format!("View centred at ({:.0}, {:.0}) ft, spanning about {size:.0} ft; {width} x {} px.", p[0], p[1], (width as f64 * 2.0 / 3.0).round()) }),
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
            if a["notes"].is_null() && a["tags"].is_null() && a["remove_tags"].is_null() {
                return Err(crate::args::bad("notes, tags, remove_tags", "give at least one", &json!({})));
            }
            let given = |k: &str| a[k].as_array().is_some_and(|v| !v.is_empty());
            if a["notes"].is_null() && !given("tags") && !given("remove_tags") {
                return Err(if a["tags"].is_null() { "remove_tags: at least 1 item, or tags (or give notes)" } else { "tags: at least 1 item, or remove_tags (or give notes)" }.into());
            }
            let t = match a["notes"].as_str() {
                Some(t) => Some(t.to_string()),
                None if a["notes"].is_null() => None,
                None => return Err(crate::args::bad("notes", "text", &a["notes"])),
            };
            let tags: Vec<String> = a["tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let tags = crate::notebook::canon_tags(tags);
            let remove: Vec<String> = a["remove_tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(|t| t.to_string())).collect()).unwrap_or_default();
            if let Some(t) = tags.iter().find(|t| remove.iter().any(|r| r.eq_ignore_ascii_case(t))) {
                return Err(crate::args::bad("remove_tags", "not also in tags", &json!(t)));
            }
            let append = a["append"].as_bool().unwrap_or(false);
            known(app, &id).await?;
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                let key = note_key(e, pair, &id);
                let n = e.notes.entry(key.clone()).or_insert_with(Note::default);
                if let Some(t) = t {
                    n.text = if append && !n.text.is_empty() { format!("{}\n\n{t}", n.text) } else { t };
                }
                let gone = |t: &String| remove.iter().any(|r| r.eq_ignore_ascii_case(t));
                let removed: Vec<String> = n.tags.iter().filter(|t| gone(t)).cloned().collect();
                n.tags.retain(|t| !gone(t));
                n.tags.extend(tags);
                n.tags = crate::notebook::canon_tags(std::mem::take(&mut n.tags));
                let mut v = json!({ "tool": "annotate_feature", "id": key, "notes": n.text, "tags": n.tags });
                if !remove.is_empty() {
                    v["removed_tags"] = json!(removed);
                }
                Ok(v)
            })
            .await
            .map(text)
        }
        "update_feature" => {
            let id = arg_str(&a, "id")?;
            if id.starts_with("m:") && !a["name"].is_null() {
                return Err("name: a point note has no name (it is called after its room); give notes, tags or at".into());
            }
            // A point note (m:) moves with `at`, as move_note.
            let point = id.starts_with("m:") && !a["at"].is_null();
            if point {
                crate::notebook::write_point(app, "move_note", &json!({ "id": id, "at": a["at"] })).await?;
            }
            let moving = a.get("in_building").is_some() || !a["way"].is_null() || (!a["at"].is_null() && !point);
            if !moving && !point && ["name", "notes", "tags"].iter().all(|k| a[k].is_null()) {
                return Err(crate::args::bad("name, notes, tags", "give at least one", &json!({})));
            }
            known(app, &id).await?;
            let moved = if moving { Some(move_entrance(app, &a, &id).await?) } else { None };
            if let Some(m) = moved.as_ref().filter(|_| ["name", "notes", "tags"].iter().all(|k| a[k].is_null())) {
                return Ok(text(m.clone()));
            }
            if point && ["name", "notes", "tags"].iter().all(|k| a[k].is_null()) {
                return Ok(text(json!({ "tool": "update_feature", "id": id })));
            }
            let name = a["name"].as_str().map(|s| s.trim().to_string());
            let notes = a["notes"].as_str().map(str::to_string);
            let tags: Option<Vec<String>> = a["tags"].as_array().map(|v| crate::notebook::canon_tags(v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()));
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                if let Some(n) = &name {
                    // A building drawn by hand keeps its name in its entry (as `rename_feature`).
                    let drawn = pair.as_ref().filter(|p| !n.is_empty() && e.created.get(p.0).is_some_and(|c| c.id == p.1));
                    if let Some((k, cid, bid)) = drawn {
                        e.renames.remove(cid);
                        e.renames.remove(bid);
                        e.created[*k].name = n.clone();
                    } else if n.is_empty() {
                        e.renames.remove(&id);
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
            .map(|mut v| {
                if let Some(m) = moved {
                    v.as_object_mut().into_iter().for_each(|o| o.extend(m.as_object().cloned().unwrap_or_default()));
                }
                text(v)
            })
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
pub(crate) async fn drawn_pair(app: &Shared, id: &str) -> Result<Option<(usize, String, String)>, String> {
    let q = id.to_string();
    app.worker.with(move |ex| Ok(agent::drawn_pair(&ex.world, &ex.t0, &q))).await
}

/// The key a place's note is kept under: a building drawn by hand's created id (`pair`, while
/// `created` still holds it there), its note under its building id (written before) joined
/// onto it; else `id`.
pub(crate) fn note_key(e: &mut Edits, pair: Option<(usize, String, String)>, id: &str) -> String {
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
    if !a["in_building"].is_null() || !a["way"].is_null() {
        return create_in_building(app, a, kind).await;
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
        return found_spot(app, id, &c, at).await;
    }
    let (kind2, asked_at) = (kind.clone(), p);
    let (p, name, river) = app
        .worker
        .with(move |ex| {
            let (q, name) = agent::creation_spot(&ex.world, &ex.t0, &kind, under.as_deref(), &id, p)?;
            // (Where the river clearance alone puts it: what moved it, apart from a camp's chunk.)
            let r = worldgen::lod::rivers::clear_of_rivers(&ex.t0.rivers, p[0], p[1], 150.0, ex.t0.cell_ft);
            Ok((q, name, [r.0, r.1]))
        })
        .await?;
    // Put elsewhere than asked (clear of a river, a camp's clearing kept in its chunk): said so when it is more than 5 ft.
    let ft = (p[0] - asked_at[0]).hypot(p[1] - asked_at[1]);
    let moved = (ft > 5.0).then(|| {
        let by_river = (river[0] - asked_at[0]).hypot(river[1] - asked_at[1]) > 1.0;
        let by_chunk = kind2 == "camp" && (p[0] - river[0]).hypot(p[1] - river[1]) > 1.0;
        let why = match (by_river, by_chunk) {
            (true, true) => "kept clear of rivers (150 ft) and 70 ft inside its battlemap chunk",
            (true, false) => "kept clear of rivers (150 ft)",
            (false, true) => "a camp's clearing is kept 70 ft inside its battlemap chunk",
            (false, false) => "put where it fits",
        };
        json!({ "x_ft": p[0].round(), "y_ft": p[1].round(), "ft": ft.round(), "why": why })
    });
    (c.x, c.y, c.name) = (p[0], p[1], asked.unwrap_or(name));
    let c2 = c.clone();
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
        return found_spot(app, id.to_string(), &c2, at).await;
    }
    let id = change["id"].as_str().unwrap_or_default().to_string();
    let name = c2.name.clone();
    let mut v = app
        .worker
        .with(move |ex| {
            let mut v = agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "created, but it could not be read back".to_string())?;
            same_name_note(&mut v, agent::same_named(&ex.world, &ex.t0, &name, &id), &name);
            Ok(v)
        })
        .await?;
    if let Some(m) = moved {
        v["moved"] = m;
    }
    Ok(v)
}

/// `found` for a point site: which one it is, how far from the point asked, and the rule that made it the same.
async fn found_spot(app: &Shared, id: String, asked: &Created, at: [f64; 2]) -> Result<Value, String> {
    let mut v = found(app, id.clone()).await?;
    let (ox, oy) = (v["x_ft"].as_f64().unwrap_or(at[0]), v["y_ft"].as_f64().unwrap_or(at[1]));
    let ft = (ox - at[0]).hypot(oy - at[1]).round();
    let beneath = asked.under_kind().map(|k| format!(" over a {}", k.name())).unwrap_or_default();
    let what = v["name"].as_str().map(|n| format!("{id} ({n})")).unwrap_or_else(|| id.clone());
    let why = if let Some(b) = &asked.in_building {
        let b2 = b.clone();
        let named = app.worker.with(move |ex| Ok(agent::building_label(&ex.world, &ex.t0, &b2))).await.ok().flatten();
        let b = named.map(|n| format!("{b} ({n})")).unwrap_or_else(|| b.clone());
        format!("{what} is already an entrance{beneath} in {b}: one building has one entrance of each kind beneath; use another building or another `under`, or change that one")
    } else {
        format!(
            "{what} is a {} with the same beneath, {ft} ft from the point asked: a {}{beneath} within {} ft of another is the same site; \
             put it farther away, or in another building (names do not count for these)",
            asked.kind,
            asked.kind,
            Created::SAME_SPOT_FT
        )
    };
    v["existing_distance_ft"] = json!(ft);
    v["note"] = json!(format!("nothing was added: {why}"));
    Ok(v)
}

/// The building `raw` names for an entrance's cellar (`in_building`: `b:<layout>:<n>`, or the `c:` id of one drawn by hand): its id, name,
/// middle and footprint; refused when it is gone, a ruin, an open structure or a yurt.
async fn host_building(app: &Shared, rawq: String) -> Result<(String, String, [f64; 2], Vec<[f64; 2]>), String> {
    app.worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let id = if rawq.starts_with("c:") {
                match agent::drawn_pair(w, t0, &rawq) {
                    Some((k, _, b)) if !w.file.edits.created[k].removed => b,
                    Some(_) => return Err(format!("{rawq} was deleted (restore_building brings it back)")),
                    None => return Err(format!("{rawq}: no building drawn by hand has that id (in_building takes a building's id, b:<layout>:<n>)")),
                }
            } else {
                rawq.clone()
            };
            if let Some(n) = agent::deleted_note(w, t0, &id) {
                return Err(n);
            }
            let Some((li, bi)) = worldgen::sitelinks::parse_building(&id).filter(|(li, _)| *li < worldgen::town::layout_count(t0)) else {
                return Err(format!("in_building: {rawq} is no building id (b:<layout>:<n>; list_children of a settlement lists them)"));
            };
            let l = worldgen::town::layout(w, t0, li);
            let Some(b) = l.building(bi) else {
                let taken = w.file.edits.buildings.get(&id).is_some_and(|e| e.removed);
                return Err(if taken { format!("{id} was taken away (restore_building brings it back)") } else { format!("in_building: no building {id}") });
            };
            let name = agent::building_label(w, t0, &id).unwrap_or_else(|| b.label().to_string());
            match b.structure {
                worldgen::town::Structure::Ruin => return Err(format!("{id} ({name}) is a ruin: no cellar to put a way down in")),
                worldgen::town::Structure::Open => return Err(format!("{id} ({name}) is an open structure (a yard or ring), not a roofed building: no cellar")),
                worldgen::town::Structure::Roofed => {}
            }
            if !worldgen::interior::has_cellar(b) {
                return Err(format!("{id} ({name}) is a yurt (a round one-room home): no cellar"));
            }
            Ok((id, name, worldgen::town::geom::centroid(&b.poly), b.poly.clone()))
        })
        .await
}

/// An edit with the gate already held by the caller (inside a batch the batch stages it).
async fn edit_held(app: &Shared, f: impl FnOnce(&mut Edits) -> Result<Value, String>) -> Result<Value, String> {
    if crate::batch::active() { app.edit("agent", 0, f).await } else { app.edit_now("agent", 0, f).await }
}

/// A way down into building `bid` ({bname}) must not set aside the design drawn by hand for it: refused when that design has no room for it.
async fn design_room(app: &Shared, bid: &str, bname: &str) -> Result<(), String> {
    let b2 = bid.to_string();
    app.worker
        .with(move |ex| Ok(worldgen::interior::design::room_for_way_down(&ex.world, &ex.t0, &b2)))
        .await?
        .map_err(|why| format!("{bid} ({bname}) has a design drawn by hand and {why}: a way down would set it aside. set_site_design: add a cellar or free a square, then try again; nothing was changed"))
}

/// Add to `v`'s note that another feature has the name `name`, if one does (`other` its id).
fn same_name_note(v: &mut Value, other: Option<String>, name: &str) {
    let Some(o) = other else { return };
    let n = format!("another \"{name}\" exists: {o}");
    v["note"] = json!(match v["note"].as_str() {
        Some(t) => format!("{t}; {n}"),
        None => n,
    });
}

/// `update_feature` with `in_building` (and `way`, `at`) on an entrance: into a building's cellar (its point becomes the building's middle),
/// or, with null, back out (to `at`, else the nearest spot outside). Ids stay. Returns `{tool, id, in_building, way, moved, note?}`.
async fn move_entrance(app: &Shared, a: &Value, id: &str) -> Result<Value, String> {
    // The checks and the write happen under one hold of the gate (as `restore_feature`): nothing can be placed in between.
    let _hold = if crate::batch::active() { None } else { Some(app.gate.lock().await) };
    let into = a.get("in_building").cloned();
    let way = a["way"].as_str().map(str::to_string);
    let (idc, kind) = (id.to_string(), app.with_edits(|e| e.created.iter().find(|c| c.id == id && !c.removed).map(|c| c.kind.clone())).flatten());
    let Some(cur) = app.with_edits(|e| e.created.iter().find(|c| c.id == idc && !c.removed && c.kind == "entrance").cloned()).flatten() else {
        return Err(match (kind, into.is_some()) {
            (Some(k), _) => format!("in_building is for entrances (c:<n> of one made with create_feature); {id} is a {k}"),
            (None, true) => format!("in_building is for entrances (c:<n> of one made with create_feature); {id} is not one"),
            (None, false) => "way and at are for an entrance moved with in_building".to_string(),
        });
    };
    let (mut c, was) = (cur.clone(), (cur.x, cur.y));
    let (host, why);
    match &into {
        Some(Value::String(raw)) => {
            let (bid, bname, mid, poly) = host_building(app, raw.clone()).await?;
            if !a["at"].is_null() && a["at"].as_str() != Some(raw.as_str()) {
                let p = crate::args::point(app, "at", &a["at"]).await?;
                if !worldgen::town::geom::contains(&poly, p) {
                    return Err(format!("at: that point is not inside {bid} ({bname}); leave `at` out (the way down goes at the building's middle)"));
                }
            }
            if let Some(o) = app.with_edits(|e| e.created.iter().find(|o| o.id != idc && !o.removed && o.kind == "entrance" && o.in_building.as_deref() == Some(bid.as_str()) && o.under_kind() == cur.under_kind()).map(|o| o.id.clone())).flatten() {
                return Err(format!("{bid} ({bname}) already has an entrance of that kind beneath it ({o}): one per kind in a building"));
            }
            if cur.in_building.as_deref() != Some(bid.as_str()) {
                design_room(app, &bid, &bname).await?;
            }
            c.way = way.or(if cur.in_building.is_some() { cur.way.clone() } else { None });
            c.in_building = Some(bid.clone());
            (c.x, c.y) = (mid[0], mid[1]);
            why = format!("into the cellar of {bname} ({bid}), at its middle");
            host = Some((bid, bname));
        }
        Some(Value::Null) => {
            if cur.in_building.is_none() {
                return Err(format!("{id} is not in a building (in_building: null takes an entrance out of one)"));
            }
            if way.is_some() {
                return Err("way: goes with a building (give in_building), not with null".into());
            }
            let asked = if a["at"].is_null() { None } else { Some(crate::args::point(app, "at", &a["at"]).await?) };
            let from = asked.unwrap_or([cur.x, cur.y]);
            let (kind, under, cid) = (cur.kind.clone(), cur.under.clone(), cur.id.clone());
            let spot = app
                .worker
                .with(move |ex| match asked {
                    Some(p) => agent::creation_spot(&ex.world, &ex.t0, &kind, under.as_deref(), &cid, p).map(|r| r.0),
                    None => agent::entrance_out_spot(&ex.world, &ex.t0, from).ok_or_else(|| "no clear spot within 100 ft of the building: give `at`".to_string()),
                })
                .await?;
            c.in_building = None;
            c.way = None;
            (c.x, c.y) = (spot[0], spot[1]);
            why = if asked.is_some() { "out of the building, to the point given".to_string() } else { "out of the building, to the nearest clear spot outside it".to_string() };
            host = None;
        }
        Some(_) => return Err("in_building: a building's id, or null".into()),
        None => {
            if !a["at"].is_null() {
                return Err("at: goes with in_building: null (no feature is moved by a point alone)".into());
            }
            let Some(b) = cur.in_building.clone() else { return Err("way: needs in_building (the entrance is not in a building)".into()) };
            c.way = way;
            (c.x, c.y) = was;
            let bname = app.worker.with(move |ex| Ok::<_, String>(agent::building_label(&ex.world, &ex.t0, &b).unwrap_or_default())).await?;
            why = "the way down changed".to_string();
            host = cur.in_building.clone().map(|b| (b, bname));
        }
    }
    c.check()?;
    let (cc, ft) = (c.clone(), (c.x - was.0).hypot(c.y - was.1));
    edit_held(app, move |e| {
        let slot = e.created.iter_mut().find(|o| o.id == cc.id && !o.removed).ok_or_else(|| format!("{} was deleted meanwhile", cc.id))?;
        (slot.x, slot.y, slot.in_building, slot.way) = (cc.x, cc.y, cc.in_building, cc.way);
        Ok(json!({}))
    })
    .await?;
    // A design drawn by hand for the site: set aside when the site moved away from it, else dropped if it no longer builds.
    let idc = id.to_string();
    let aside = app
        .worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let Some(li) = agent::layout_of(w, t0, &idc) else { return Ok(None) };
            let uid = format!("u:{li}:0");
            let Some(d) = w.file.edits.designs.get(&uid) else { return Ok(None) };
            if worldgen::under::design::misplaced(w, t0, &uid) {
                return Ok(Some((uid, false)));
            }
            let blocking = d.problems(w, t0, &uid).map(|(_, p)| p.iter().any(|p| p.blocking)).unwrap_or(true);
            Ok::<_, String>(blocking.then_some((uid, true)))
        })
        .await?;
    let mut v = json!({ "tool": "update_feature", "id": id, "in_building": host.map(|(i, n)| json!({ "id": i, "name": n })), "way": c.way.clone().unwrap_or_else(|| "stairs".into()) });
    if c.in_building.is_none() {
        v["way"] = Value::Null;
    }
    v["moved"] = json!({ "x_ft": c.x.round(), "y_ft": c.y.round(), "ft": ft.round(), "why": why });
    if let Some((u, drop)) = aside {
        if drop {
            let u2 = u.clone();
            edit_held(app, move |e| {
                e.designs.remove(&u2);
                Ok(json!({}))
            })
            .await?;
        }
        v["note"] = json!(format!("{u} had a design drawn by hand: it was made for the entrance where it stood and is set aside{} (the generated site stands); set_site_design makes a new one", if drop { " and dropped, it no longer builds" } else { "" }));
    }
    Ok(v)
}

/// `create_feature` for an entrance in a building's cellar (`in_building`, `way`): its point is the building's middle.
async fn create_in_building(app: &Shared, a: Value, kind: String) -> Result<Value, String> {
    if kind != "entrance" {
        return Err(format!("in_building and way are for entrances (the kind is {kind}): only an entrance goes down from a cellar"));
    }
    let Some(raw) = a["in_building"].as_str().map(str::to_string) else {
        return Err("way: needs in_building (a building's id: b:<layout>:<n>, or c:<n> of one drawn by hand)".into());
    };
    let (rawq, mut asked_at) = (raw.clone(), None);
    let at_given = !a["at"].is_null() || !a["x_ft"].is_null();
    let at_same = a["at"].as_str().is_some_and(|s| s == raw);
    if at_given && !at_same {
        asked_at = Some(place(app, &a).await?);
    }
    // Everything is checked before anything is stored, and the checks and the write happen under one hold of the gate (as `restore_feature`).
    let _hold = if crate::batch::active() { None } else { Some(app.gate.lock().await) };
    let (bid, bname, mid, poly) = host_building(app, rawq).await?;
    if let Some(p) = asked_at
        && !worldgen::town::geom::contains(&poly, p)
    {
        return Err(format!("at: that point is not inside {bid} ({bname}); leave `at` out (the way down goes at the building's middle) or name a point inside it"));
    }
    let opt = |k: &str| a[k].as_str().map(str::to_string);
    let levels = match &a["levels"] {
        Value::Null => None,
        v => Some(v.as_u64().filter(|l| *l <= 255).ok_or("levels: a whole number")? as u8),
    };
    let mut c = Created { kind, under: opt("under"), size: opt("size"), levels, theme: opt("theme"), in_building: Some(bid.clone()), way: opt("way"), x: mid[0], y: mid[1], ..Default::default() };
    c.check()?;
    let n = app.with_edits(|e| e.created.len()).unwrap_or(0);
    c.id = format!("c:{n}");
    if let Some(id) = app.with_edits(|e| e.existing_site(&c, mid).map(|o| o.id.clone())).flatten() {
        drop(_hold);
        return found_spot(app, id, &c, mid).await;
    }
    design_room(app, &bid, &bname).await?;
    let asked = a["name"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let (cid, under) = (c.id.clone(), c.under.clone());
    let generated = app.worker.with(move |ex| Ok::<_, String>(agent::creation_spot_with(&ex.world, &ex.t0, "entrance", under.as_deref(), &cid, mid, agent::Overlap::Skip).map(|r| r.1).ok())).await?;
    c.name = asked.or(generated).unwrap_or_else(|| format!("Beneath {bname}"));
    let c2 = c.clone();
    let change = edit_held(app, move |e| {
        if let Some(o) = e.existing_site(&c, mid) {
            return Ok(json!({ "existing": o.id }));
        }
        c.id = format!("c:{}", e.created.len());
        let reply = json!({ "tool": "create_feature", "id": c.id, "kind": c.kind, "name": c.name, "x_ft": mid[0].round(), "y_ft": mid[1].round() });
        e.created.push(c);
        Ok(reply)
    })
    .await?;
    drop(_hold);
    if let Some(id) = change["existing"].as_str() {
        return found_spot(app, id.to_string(), &c2, mid).await;
    }
    let id = change["id"].as_str().unwrap_or_default().to_string();
    // (The cellar level that holds the stairs, and a point asked for that the middle replaced.)
    let (host, name) = (bid.clone(), c2.name.clone());
    let mut v = app
        .worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let mut v = agent::get(w, t0, &id).ok_or_else(|| "created, but it could not be read back".to_string())?;
            if let Some(el) = agent::layout_of(w, t0, &id)
                && let Some(l) = agent::way_down_level(w, t0, &host, el)
            {
                v["in_building"]["cellar_level"] = json!(l);
            }
            same_name_note(&mut v, agent::same_named(w, t0, &name, &id), &name);
            Ok(v)
        })
        .await?;
    if let Some(p) = asked_at {
        let ft = (p[0] - mid[0]).hypot(p[1] - mid[1]);
        if ft > 5.0 {
            v["moved"] = json!({ "x_ft": mid[0].round(), "y_ft": mid[1].round(), "ft": ft.round(), "why": format!("the way down goes at the middle of {bid} ({bname}), not at the point asked") });
        }
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
