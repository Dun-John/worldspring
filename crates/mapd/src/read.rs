//! The read and view tools: world_overview, search, get, list, inspect, route, nearest, road_point, render_view and
//! focus_view. `get`, `list` and `inspect` each have views picked by one argument (`view` / `what`); a view's
//! arguments are checked against its own schema (`args::check`, through `view_of`), then it runs a family
//! module's read handler under the handler's own name (`VIEWS`: the one table of them).

use std::sync::OnceLock;

use serde_json::{Map, Value, json};

use crate::Shared;
use crate::tools::{Run, schema};

/// One view of `get`, `list` or `inspect`: `field` = `value` picks it (`default`: when left out); it runs the
/// handler `old` of `run` with the arguments renamed back (`renames`: (this view's name, the handler's)).
pub(crate) struct View {
    pub tool: &'static str,
    pub field: &'static str,
    pub value: &'static str,
    pub default: bool,
    pub old: &'static str,
    pub run: Run,
    pub renames: &'static [(&'static str, &'static str)],
    /// The handler's radius key and ft per unit: its place and radius are given here as a circle `within`.
    pub circle: Option<(&'static str, f64)>,
}

const fn v(tool: &'static str, field: &'static str, value: &'static str, old: &'static str, run: Run) -> View {
    View { tool, field, value, default: false, old, run, renames: &[], circle: None }
}

pub(crate) const VIEWS: &[View] = &[
    View { default: true, ..v("get", "view", "details", "get_feature", Run::Finds) },
    v("get", "view", "children", "list_children", Run::Finds),
    v("get", "view", "design", "get_site_design", Run::Design),
    v("get", "view", "town_plan", "get_town_plan", Run::Towns),
    v("get", "view", "street_plan", "get_street_plan", Run::StreetsEdit),
    v("list", "what", "names", "list_names", Run::Finds),
    View { circle: Some(("radius_mi", 5280.0)), ..v("list", "what", "roads", "list_roads", Run::Finds) },
    v("list", "what", "drawn_roads", "list_paths", Run::Paths),
    View { renames: &[("kind", "what")], ..v("list", "what", "streets", "list_streets", Run::Streets) },
    v("list", "what", "crossings", "list_crossings", Run::Crossings),
    v("list", "what", "wharves", "list_wharves", Run::Harbour),
    View { circle: Some(("radius_ft", 1.0)), ..v("list", "what", "fences", "list_fences", Run::Fences) },
    v("list", "what", "objects", "list_objects", Run::Reads),
    v("list", "what", "sprites", "list_sprites", Run::Scatter),
    v("list", "what", "ground", "list_plateaus", Run::Reads),
    v("list", "what", "waters", "list_waters", Run::Waters),
    v("list", "what", "designs", "list_designs", Run::Reads),
    View { renames: &[("at", "near")], ..v("list", "what", "links", "list_links", Run::Links) },
    View { renames: &[("at", "location")], ..v("list", "what", "npcs", "list_npcs", Run::Notebook) },
    v("list", "what", "plots", "list_plots", Run::Notebook),
    v("list", "what", "notes", "list_notes", Run::Notebook),
    v("list", "what", "sketch", "get_sketch", Run::Sketch),
    v("list", "what", "hex_grid", "get_hex_grid", Run::Hexes),
    View { default: true, ..v("inspect", "what", "location", "describe_location", Run::Finds) },
    v("inspect", "what", "battlemap", "get_battlemap", Run::Finds),
    v("inspect", "what", "heights", "ground_heights", Run::Ground),
    v("inspect", "what", "hex", "hex_at", Run::Hexes),
];

/// Old tools that are now another call, besides the views' (for `args::check`'s "now ..." on an unknown tool).
const MOVED: &[(&str, &str)] = &[
    ("search_features", "search"),
    ("features_near", "search {at, radius_ft}"),
    ("get_npc", "get {id}"),
    ("get_plot", "get {id}"),
    ("remove_paths", "remove {ids}"),
    ("rename_feature", "set_feature {id, name}"),
    ("annotate_feature", "set_feature {id, notes, tags}"),
    ("update_feature", "set_feature {id, name, notes, tags} (an entrance's in_building: set_site; an m: note's at: set_note)"),
    ("hide_feature", "set_feature {id, hidden}"),
    ("create_feature", "set_site {kind, at}"),
    ("update_fortification", "set_site {id}"),
    ("delete_feature", "remove {ids}"),
    ("restore_feature", "restore {ids}"),
    ("create_settlement", "set_settlement {at, tier}"),
    ("update_settlement", "set_settlement {id}"),
    ("create_building", "set_building"),
    ("update_building", "set_building {id}"),
    ("check_building_spot", "set_building {dry_run: true}"),
    ("place_stamp", "set_building {at, stamp}"),
    ("remove_buildings", "remove {ids} (or {what: \"buildings\", within})"),
    ("restore_building", "restore {ids}"),
    ("create_building_row", "set_building_row {path}"),
    ("create_parcel", "set_parcel {poly}"),
    ("update_parcel", "set_parcel {id}"),
    ("create_npc", "set_npc {name}"),
    ("update_npc", "set_npc {id}"),
    ("place_npc", "set_npc {id, at}"),
    ("delete_npc", "remove {ids}"),
    ("create_plot", "set_plot {title}"),
    ("update_plot", "set_plot {id}"),
    ("delete_plot", "remove {ids}"),
    ("add_note_at", "set_note {site, level, text}"),
    ("move_note", "set_note {id, at}"),
    ("delete_note", "remove {ids} (a place's note: remove {what: \"notes\", id})"),
    ("remove_objects", "remove {ids} (or {what: \"objects\", within, points, kinds})"),
    ("restore_objects", "restore {ids} (or {what: \"objects\", within})"),
    ("delete_plateau", "remove {ids}"),
    ("restore_islets", "restore {ids}"),
    ("delete_water", "remove {ids}"),
    ("remove_roads", "remove {ids} (restore {ids} brings them back)"),
    ("remove_fences", "remove {ids} (restore {ids} brings them back; gaps: place_fence {gaps})"),
    ("remove_wharves", "remove {ids}"),
    ("remove_crossings", "remove {ids}"),
    ("delete_link", "remove {ids}"),
    ("delete_sketch_stroke", "remove {ids}"),
];

/// What the old tool `old` is now: `list {what: "roads"}`.
pub(crate) fn moved(old: &str) -> Option<String> {
    if let Some(v) = VIEWS.iter().find(|v| v.old == old) {
        return Some(format!("{} {{{}: \"{}\"}}", v.tool, v.field, v.value));
    }
    MOVED.iter().find(|(o, _)| *o == old).map(|(_, n)| n.to_string())
}

/// The handlers' schemas (the old read tools': theirs here, and those in the family modules' lists), by name: a view's,
/// or a merged write tool's mode (`route::fit`).
pub(crate) fn old_schema(name: &str) -> Value {
    static S: OnceLock<Vec<Value>> = OnceLock::new();
    let all = S.get_or_init(|| {
        let mut v = old_finds();
        v.extend(crate::tools::own());
        for m in [
            crate::build::list(), crate::quick::list(), crate::works::list(), crate::sites::list(), crate::settlements::list(),
            crate::notebook::list(), crate::scatter::list(), crate::crossings::list(), crate::hexes::list(), crate::links::list(), crate::ground::list(), crate::waters::list(),
            crate::paths::list(), crate::fences::list(), crate::harbour::list(), crate::design::list(), crate::reads::list(), crate::streets::list(), crate::towns::list(),
            crate::streets_edit::list(), crate::sketch::list(),
        ] {
            v.extend(m);
        }
        v
    });
    all.iter().find(|t| t["name"] == name).map(|t| t["inputSchema"].clone()).unwrap_or_else(|| panic!("no schema for {name}"))
}

/// A view's own schema: its handler's, with this view's names (and `within` for a handler's place and radius).
fn view_schema(v: &View) -> Value {
    let old = old_schema(v.old);
    let mut props = old["properties"].as_object().cloned().unwrap_or_default();
    let mut required: Vec<String> = old["required"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(str::to_string)).collect();
    for (new, was) in v.renames {
        if let Some(p) = props.remove(*was) {
            props.insert((*new).to_string(), p);
        }
        for r in required.iter_mut().filter(|r| r == was) {
            *r = (*new).to_string();
        }
    }
    if let Some((radius, _)) = v.circle {
        props.remove("at");
        props.remove(radius);
        props.insert("within".into(), json!({ "type": "object", "description": "Only near: a circle {at, radius_ft}" }));
    }
    props.insert("world".into(), json!({ "type": "string" }));
    let req: Vec<&str> = required.iter().map(String::as_str).collect();
    schema(Value::Object(props), &req)
}

fn view_schemas() -> &'static Vec<Value> {
    static S: OnceLock<Vec<Value>> = OnceLock::new();
    S.get_or_init(|| VIEWS.iter().map(view_schema).collect())
}

/// For a tool with views, the view its arguments pick and that view's schema; the picking argument is taken out
/// (`args::check` puts it back). An argument only another view of the tool takes is refused, naming the view.
pub(crate) fn view_of(tool: &str, obj: &mut Map<String, Value>) -> Option<Result<(&'static View, &'static Value), String>> {
    let views: Vec<(usize, &'static View)> = VIEWS.iter().enumerate().filter(|(_, v)| v.tool == tool).collect();
    let field = views.first()?.1.field;
    let values = || views.iter().map(|(_, v)| v.value).collect::<Vec<_>>().join(", ");
    let picked = match obj.remove(field) {
        None | Some(Value::Null) => views.iter().find(|(_, v)| v.default).ok_or_else(|| format!("{field}: required, one of {}", values())),
        Some(Value::String(s)) => views.iter().find(|(_, v)| v.value == s).ok_or_else(|| {
            let hint = crate::args::suggest(&s, views.iter().map(|(_, v)| v.value)).map(|h| format!(" (did you mean {h}?)")).unwrap_or_default();
            format!("{field}: one of {}, got \"{s}\"{hint}", values())
        }),
        Some(x) => Err(crate::args::bad(field, &format!("one of {}", values()), &x)),
    };
    let (i, v) = match picked {
        Ok(p) => *p,
        Err(e) => return Some(Err(e)),
    };
    let s = &view_schemas()[i];
    let props = s["properties"].as_object().expect("view schema");
    let foreign = obj.keys().find(|k| !props.contains_key(*k) && views.iter().any(|(j, _)| view_schemas()[*j]["properties"].get(k.as_str()).is_some()));
    if let Some(k) = foreign {
        let mut takes: Vec<&str> = props.keys().map(String::as_str).filter(|k| *k != "world").collect();
        takes.sort_unstable();
        let takes = if takes.is_empty() { "no arguments".to_string() } else { takes.join(", ") };
        return Some(Err(format!("{field} '{}' takes {takes}; not {k}", v.value)));
    }
    Some(Ok((v, s)))
}

/// The public schema of a tool with views: every view's arguments, the picking one an enum saying which go with which.
fn union(tool: &str, about: &str, required: &[&str]) -> Value {
    let mut props = Map::new();
    let mut by = Vec::new();
    let mut field = "";
    for (v, s) in VIEWS.iter().zip(view_schemas()).filter(|(v, _)| v.tool == tool) {
        field = v.field;
        let mut keys: Vec<&str> = Vec::new();
        for (k, p) in s["properties"].as_object().into_iter().flatten() {
            if k != "world" {
                keys.push(k);
                props.entry(k.clone()).or_insert_with(|| p.clone());
            }
        }
        keys.sort_unstable();
        by.push(format!("{}{}: {}", v.value, if v.default { " (default)" } else { "" }, if keys.is_empty() { "-".to_string() } else { keys.join(", ") }));
    }
    let values: Vec<&str> = VIEWS.iter().filter(|v| v.tool == tool).map(|v| v.value).collect();
    let mut first = Map::new();
    first.insert(field.to_string(), json!({ "type": "string", "enum": values, "description": format!("{about}. Arguments by {field}: {}", by.join("; ")) }));
    first.extend(props);
    schema(Value::Object(first), required)
}

pub fn list() -> Vec<Value> {
    let old = old_finds();
    let find = |n: &str| old.iter().find(|t| t["name"] == n).cloned().expect("old read");
    let geo = crate::geo::list();
    let geo = |n: &str| geo.iter().find(|t| t["name"] == n).cloned().expect("geo read");
    let mut search = find("search_features");
    search["name"] = json!("search");
    search["description"] = json!("Named features: by query (towns, ranges, rivers, ruins...; from three letters also districts, businesses and sites underground by name, kind or theme), kind, tag or within. With at alone (and kinds), everything named within radius_ft of it, nearest first, with distance, direction and extent. Returns {items, found}; limit cuts it and sets truncated.");
    let p = &mut search["inputSchema"]["properties"];
    p["kinds"] = json!({ "type": "array", "items": { "type": "string" }, "description": "With at alone: only these kinds" });
    p["limit"] = json!({ "type": "integer", "minimum": 1, "maximum": 500, "description": "Default 25 (50 with at alone); at most 200 with a query, kind, tag or within" });
    p["radius_ft"] = json!({ "type": "number", "minimum": 50, "maximum": 2640000, "default": 26400 });
    vec![
        find("world_overview"),
        search,
        json!({ "name": "get", "title": "One thing", "description": "One thing by id: a feature's details and relations (an NPC or plot point in full by its n: or p: id), what it contains, a site's or interior's text plan, a settlement's ward plan or its hand-laid streets.", "inputSchema": union("get", "What to give", &["id"]) }),
        json!({ "name": "list", "title": "Lists", "description": "Many things of one kind as {items, found}, cut by limit (truncated: true): names to rename, roads, drawn roads, streets in an area, crossings, wharves, fences, objects and clears, sprites, ground (plateaus, brushed ground, islets), waters, designs, links, NPCs, plot points, notes, the sketch's strokes, the hex grid.", "inputSchema": union("list", "What to list", &["what"]) }),
        json!({ "name": "inspect", "title": "What is here", "description": "A place looked at: what is there (elevation, biome, water, coast, the building or site, things within radius_ft), its battlemap (surfaces, buildings, objects by square), the ground's heights per square, or its overland hex.", "inputSchema": union("inspect", "What to look at", &[]) }),
        find("route"),
        geo("nearest"),
        geo("road_point"),
        find("render_view"),
        find("focus_view"),
    ]
}

/// Runs a read tool: the views through their handlers; `search` as a query, or round a place with `at` alone.
pub async fn call(app: &Shared, name: &str, a: &Value) -> Result<Vec<Value>, String> {
    let (run, old, a) = match name {
        "get" | "list" | "inspect" => {
            let mut obj = a.as_object().cloned().unwrap_or_default();
            let (v, _) = view_of(name, &mut obj).expect("a tool with views")?;
            let mut a = Value::Object(obj);
            for (new, was) in v.renames {
                if let Some(x) = a.as_object_mut().and_then(|o| o.remove(*new)) {
                    a[*was] = x;
                }
            }
            if let Some((radius, per)) = v.circle
                && let Some(w) = a.as_object_mut().and_then(|o| o.remove("within"))
            {
                let r = w["radius_ft"].as_f64().ok_or_else(|| crate::args::bad("within", "a circle {at, radius_ft}", &w))?;
                a["at"] = if w["at"].is_null() { json!({ "x_ft": w["x_ft"], "y_ft": w["y_ft"] }) } else { w["at"].clone() };
                a[radius] = json!(r / per);
            }
            // (An NPC's or plot point's details are the notebook's.)
            match (v.value, a["id"].as_str().and_then(|id| id.get(..2))) {
                ("details", Some("n:")) => (Run::Notebook, "get_npc", a),
                ("details", Some("p:")) => (Run::Notebook, "get_plot", a),
                _ => (v.run, v.old, a),
            }
        }
        "search" => {
            let given = |k: &str| !a[k].is_null() && !(k == "query" && a[k].as_str().is_some_and(|q| q.trim().is_empty()));
            let around = given("at") && !["query", "kind", "tag", "in_notes", "within", "underground_kinds", "levels_min", "levels_max"].into_iter().any(given);
            let mut a = a.clone();
            if around {
                if let Some(o) = a.as_object_mut() {
                    o.remove("query");
                    if let Some(r) = o.remove("radius_ft").and_then(|r| r.as_f64()) {
                        o.insert("radius_mi".into(), json!(r / 5280.0));
                    }
                }
                (Run::Finds, "features_near", a)
            } else {
                if !a["kinds"].is_null() {
                    return Err("kinds: only with at alone (a search round a place); with a query give kind".into());
                }
                if a["limit"].as_u64().is_some_and(|l| l > 200) {
                    return Err(crate::args::bad("limit", "1 to 200 with a query, kind, tag or within", &a["limit"]));
                }
                (Run::Finds, "search_features", a)
            }
        }
        "world_overview" | "route" | "render_view" | "focus_view" => (Run::Finds, name, a.clone()),
        "nearest" | "road_point" => (Run::Geo, name, a.clone()),
        _ => return Err(format!("unknown tool: {name}")),
    };
    Box::pin(crate::tools::run(app, run, old, &a)).await
}

/// The schemas of the reads `finds` runs (world_overview ... focus_view), as they were listed.
fn old_finds() -> Vec<Value> {
    use crate::tools::{limit_prop, merge};
    let place_props = crate::args::at_props;
    let v = json!([
        { "name": "world_overview", "title": "World overview", "description": "The world's size, land masses, counts of named features and its largest settlements.", "inputSchema": schema(json!({}), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "search_features", "title": "Search", "description": "Named features matching a query (towns, ranges, rivers, ruins...; from three letters also districts, businesses and sites underground by name, kind or theme). Give a query, kind, tag, within or at. Returns {items, found}; limit cuts it and sets truncated.", "inputSchema": schema(json!({ "query": { "type": "string", "default": "" }, "kind": { "type": "string", "description": "Only this kind (city, ruin, river, building, underground...)" }, "tag": { "type": "string", "description": "Only features whose notes carry this tag" }, "in_notes": { "type": "boolean", "default": false, "description": "The query also matches notes" }, "within": crate::args::area_light("Only features inside (a point) or overlapping: a box, polygon or circle {at, radius_ft}"), "at": { "type": ["string", "array", "object"], "description": "Only within radius_ft of this place, nearest first" }, "radius_ft": { "type": "number", "default": 26400 }, "underground_kinds": { "type": "array", "items": { "type": "string" }, "description": "dungeon, crypt, catacombs, cave, mine, lava tube, sewer, keep dungeon" }, "levels_min": { "type": "integer" }, "levels_max": { "type": "integer" }, "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 25 }, "include_hidden": { "type": "boolean", "default": false } }), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "get_feature", "title": "Feature details", "description": "A feature's details and relations: where and what it is, extent, notes, regions, nearby settlements and roads, what it holds (districts, buildings, ways underground, sites, members, levels, rooms), its settlement and site; NPCs and plots there.", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_children", "title": "Children", "description": "What a feature contains: a settlement's districts, businesses and sites, a building's or site's levels and rooms, a region's settlements and sites. Each list is cut to limit (default 200, max 2000); found gives the full count and truncated is true.", "inputSchema": schema(json!({ "id": { "type": "string" }, "limit": limit_prop(200, 2000) }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_names", "title": "Names", "description": "Everything that can be renamed, with id, kind, current and generated name; with settlement its districts, businesses, towers and sites; with inside (a building or site) its levels and rooms. Returns {items, found}; limit (default 200, max 2000) cuts it.", "inputSchema": schema(json!({ "kind": { "type": "string", "description": "Only this kind (river, city, building, site, room...)" }, "settlement": { "type": "string" }, "inside": { "type": "string" }, "function": { "type": "string", "description": "Only businesses of this function (inn, blacksmith...), in a settlement or site" }, "limit": limit_prop(200, 2000) }), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "describe_location", "title": "What is here", "description": "What is at a place: elevation, slope_deg, biome (the coarse map's: a dry point can read ocean), water (a river or lake within 1000 ft), coast (the sea within 5 mi), areas, the building, district or site there, named features nearby, nearest settlements, and things: everything within radius_ft, topmost first.", "inputSchema": schema(merge(place_props(), json!({ "radius_ft": { "type": "number", "minimum": 0, "maximum": 500, "default": 15 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "features_near", "title": "Near a place", "description": "Everything named within a radius of a place, nearest first, with distance, direction and extent. Returns {items, found}; limit (default 50) cuts it. kinds limits it; districts and businesses only when asked for or within a mile.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "minimum": 0.01, "maximum": 500, "default": 5 }, "kinds": { "type": "array", "items": { "type": "string" } }, "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 50 }, "include_hidden": { "type": "boolean", "default": false } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "route", "title": "Route", "description": "Distance between two places by road (where roads join them) or overland, with D&D 5e travel days (normal, fast, slow pace).", "inputSchema": schema(json!({ "from": crate::args::point_schema("Where from: an id, a place's name, {x_ft, y_ft} or [x_ft, y_ft]"), "to": crate::args::point_schema("Where to: an id, a place's name, {x_ft, y_ft} or [x_ft, y_ft]") }), &["from", "to"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_roads", "title": "Roads", "description": "The road network: drawn roads, then stretches between settlements and junctions, with id (rd:), class, length, ends and name. Returns {items, found, named}; limit cuts it. Filter by at and radius_mi, settlement, class or drawn.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "minimum": 0.01, "maximum": 3000, "default": 25 }, "settlement": { "type": "string", "description": "Only roads ending at this settlement" }, "class": { "type": "string", "description": "king's road (also kings road, kings_road), road or track; results say \"king's road\"" }, "drawn": { "type": "boolean", "default": false, "description": "Only roads drawn in the sketch" }, "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 50 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "get_battlemap", "title": "Battlemap", "description": "The battlemap (a 640-ft square of 5-ft squares) at a place: elevation range, surfaces, buildings, every kind of object with its tactical rules, and the objects near the place by square.", "inputSchema": schema(merge(place_props(), json!({ "radius_squares": { "type": "number", "minimum": 1, "maximum": 64, "default": 12 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "render_view", "title": "Look at the map", "description": "A screenshot of the open map app at a place (JPEG 1024x683; width_px 256-1536, format png); needs the app in a visible tab. size_ft is the ground across: ~450 a battlemap, 3000 a village, 20000 a city. An l: or r: id in at shows that level inside (plan, doors, locks, furniture), as does a b:, u: or k: id with level; the map goes back after.", "inputSchema": schema(merge(place_props(), json!({ "level": { "type": "integer", "minimum": 0, "description": "With at b:, u: or k:: show this level inside, 0 the lowest" }, "size_ft": { "type": "number", "minimum": 50, "maximum": 5000000, "default": 3000 }, "width_px": { "type": "integer", "minimum": 256, "maximum": 1536, "default": 1024, "description": "Pixels across (height 2/3 of it)" }, "format": { "type": "string", "enum": ["jpeg", "png"], "default": "jpeg" } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "focus_view", "title": "Show the user", "description": "Move the open map app's view to a place (the user sees it).", "inputSchema": schema(merge(place_props(), json!({ "size_ft": { "type": "number", "minimum": 50, "maximum": 5000000, "default": 3000 } })), &[]) },
    ]);
    v.as_array().cloned().unwrap_or_default()
}
