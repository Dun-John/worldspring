//! The MCP tools: read the world, look at it through the open app, and edit it. Every edit
//! is saved, logged and shown live in the app.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::core::tile::TileKey;
use worldgen::under::{MAX_LEVELS, SiteSize, THEMES, UnderKind};
use worldgen::world::{CREATABLE, Created, Note};

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
in the open map app. Created sites are generated like the world's own: a ruin's dungeon or crypt is fully playable. Battlemaps take objects put down or taken away by hand (place_objects, remove_objects), including uploaded sprites (upload_sprite). Many edits at once go in one batch. Every tool takes an optional world (the hash world_overview gives) and refuses to run if another world is open. The DM's notebook holds NPCs (list_npcs, create_npc, place_npc...) and plot points tied to places (list_plots, create_plot...); get_feature and describe_location show those at a place.";

pub(crate) fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn place_props() -> Value {
    json!({
        "id": { "type": "string", "description": "A feature, building, district or site id" },
        "x_ft": { "type": "number", "description": "Feet east of the map's left edge" },
        "y_ft": { "type": "number", "description": "Feet south of the map's top edge" },
    })
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
        { "name": "search_features", "title": "Search", "description": "Named features matching a query, by their current names (towns, ranges, rivers, ruins...; from three letters also districts and businesses such as inns and temples). Empty query with a kind lists that kind. Each result has its extent: bounding box (ft) and area (sq mi), or a river's length (mi).", "inputSchema": schema(json!({ "query": { "type": "string" }, "kind": { "type": "string", "description": "Only this kind (city, town, village, ruin, cave, range, forest, river, lake, building, district...)" }, "limit": { "type": "integer", "default": 25 }, "include_hidden": { "type": "boolean", "default": false } }), &["query"]), "annotations": { "readOnlyHint": true } },
        { "name": "get_feature", "title": "Feature details", "description": "A feature's details and relations: what and where it is, its extent (bounding box, area or length), its notes, the regions it lies in, nearest settlements, roads to other settlements, a settlement's districts, notable buildings and ways underground, a building's or site's levels and rooms.", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_children", "title": "Children", "description": "What a feature contains: a settlement's districts and businesses, a district's businesses, a building's or underground site's levels and rooms, a region's settlements and sites.", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_names", "title": "Names", "description": "Everything that can be renamed, each with its id, kind, current and generated name: every named feature (oceans, rivers, ranges, settlements, sites...); with settlement (a settlement's or site's id) its districts, businesses, towers and underground sites; with within (a building or site id) its levels and rooms.", "inputSchema": schema(json!({ "kind": { "type": "string", "description": "Only this kind (river, range, city, district, building, tower, underground, level, room...)" }, "settlement": { "type": "string" }, "within": { "type": "string" } }), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "describe_location", "title": "What is here", "description": "What is at a place: elevation, biome, the areas it lies in (land, sea, range, forest, lake...), the building or district or site there, named features nearby (within a tenth of their size, 2 to 10 mi, measured to their nearest edge or course), nearest settlements.", "inputSchema": schema(place_props(), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "features_near", "title": "Near a place", "description": "Everything named within a radius of a place, nearest first, measured to each feature's nearest edge or course (0 inside an area), with its distance, direction and extent. kinds limits it to some kinds (e.g. [\"river\", \"lake\", \"forest\"]); districts and businesses come when kinds asks for district or building, or with no kinds within a mile.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "default": 5 }, "kinds": { "type": "array", "items": { "type": "string" } }, "limit": { "type": "integer", "default": 50 }, "include_hidden": { "type": "boolean", "default": false } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "route", "title": "Route", "description": "Distance between two places by road (where roads join them) or overland, with D&D 5e travel days (normal, fast, slow pace).", "inputSchema": schema(json!({ "from": { "description": "An id, or {x_ft, y_ft}" }, "to": { "description": "An id, or {x_ft, y_ft}" } }), &["from", "to"]), "annotations": { "readOnlyHint": true } },
        { "name": "list_roads", "title": "Roads", "description": "The road network: named roads (drawn in the world's sketch) with the settlements along them, then the roads between settlements and junctions, longest first, each with its class (king's road, road, track), length and ends (a settlement's id and name, or the junction's point). Filter by a place (id or x_ft/y_ft) and radius_mi, by settlement (roads ending there), by class, or drawn roads only.", "inputSchema": schema(merge(place_props(), json!({ "radius_mi": { "type": "number", "default": 25 }, "settlement": { "type": "string", "description": "A settlement's id: only roads ending there" }, "class": { "type": "string", "enum": ["kings_road", "road", "track"] }, "drawn": { "type": "boolean", "default": false, "description": "Only roads drawn in the sketch" }, "limit": { "type": "integer", "default": 50 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "get_battlemap", "title": "Battlemap", "description": "The battlemap (a 640-ft square of 5-ft squares) at a place: elevation range, surfaces, buildings, and every kind of object with its tactical rules (cover, movement, sight, hazards); objects near the place listed by square.", "inputSchema": schema(merge(place_props(), json!({ "radius_squares": { "type": "number", "default": 12 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "render_view", "title": "Look at the map", "description": "A screenshot of the open map app at a place (PNG, 1536x1024). size_ft is the ground across the image: up to about 450 shows the painted battlemap with its 5-ft grid, 3000 a village or a town's streets, 20000 a city and its fields, 200000 a region.", "inputSchema": schema(merge(place_props(), json!({ "size_ft": { "type": "number", "default": 3000 } })), &[]), "annotations": { "readOnlyHint": true } },
        { "name": "focus_view", "title": "Show the user", "description": "Move the open map app's view to a place (the user sees it).", "inputSchema": schema(merge(place_props(), json!({ "size_ft": { "type": "number", "default": 3000 } })), &[]) },
        { "name": "rename_feature", "title": "Rename", "description": "Rename anything named: a feature, building, district, tower, underground site, or a site's level (l:<site>:<level>) or room (r:<site>:<level>:<room>), ids as list_names gives them. An empty name restores the original.", "inputSchema": schema(json!({ "id": { "type": "string" }, "name": { "type": "string" } }), &["id", "name"]) },
        { "name": "annotate_feature", "title": "Add notes", "description": "Set (or append to) a feature's notes: lore, hooks, secrets for the DM, with tags.", "inputSchema": schema(json!({ "id": { "type": "string" }, "text": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } }, "append": { "type": "boolean", "default": false } }), &["id", "text"]) },
        { "name": "create_feature", "title": "Create a site", "description": "Add a site at a place on dry land, generated like the world's own (map, battlemap, interiors, underground): ruin (over a dungeon, crypt or catacombs), tower, camp, waystation (roadside inn), cave, mine, lava_tube, or entrance (a bare way underground: stairs, a cave mouth, a mine adit or a skylight, by 'under'). Sites with something underground take its size, levels and theme (anything left out is chosen as for the world's own). Or a castle (an outline: curtain wall, towers, gatehouse, keep, yard buildings) or a wall (a line of wall with towers; gates where asked and where roads and streets cross it), drawn by poly/rect or pts instead of a place; the world's own buildings in the way are named, or taken away with remove_in_way. Returns its id and the ids of its ways underground (a castle: its keep's building id, towers and gates).", "inputSchema": schema(merge(merge(place_props(), crate::works::props()), json!({
            "kind": { "type": "string", "enum": CREATABLE.iter().filter(|k| **k != "building").collect::<Vec<_>>() },
            "name": { "type": "string" },
            "under": { "type": "string", "enum": UnderKind::CREATABLE.map(|k| k.key()), "description": "What lies beneath a ruin (dungeon, crypt, catacombs) or an entrance (any)" },
            "size": { "type": "string", "enum": SiteSize::NAMES, "description": "The site underground" },
            "levels": { "type": "integer", "minimum": 1, "maximum": MAX_LEVELS },
            "theme": { "type": "string", "enum": THEMES.iter().map(|t| t.key).collect::<Vec<_>>(), "description": theme_doc() },
        })), &["kind"]) },
        { "name": "update_feature", "title": "Update", "description": "Change a feature's name and/or notes in one go.", "inputSchema": schema(json!({ "id": { "type": "string" }, "name": { "type": "string" }, "notes": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } } }), &["id"]) },
        { "name": "hide_feature", "title": "Hide", "description": "Hide a feature from the map's labels and search (or show it again). It is still there.", "inputSchema": schema(json!({ "id": { "type": "string" }, "hidden": { "type": "boolean", "default": true } }), &["id"]) },
        { "name": "delete_feature", "title": "Delete a created site", "description": "Delete a site that was created, buildings drawn by hand too (the world's own buildings go with remove_buildings; other generated features can only be hidden).", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]) },
    ]);
    if let Some(a) = v.as_array_mut() {
        a.extend(crate::notebook::list());
        a.extend(crate::scatter::list());
        a.extend(crate::build::list());
        a.extend(crate::works::list());
        a.extend(crate::crossings::list());
        a.extend(crate::design::list());
        a.extend(crate::towns::list());
        a.extend(crate::batch::list());
        // Any tool can say which world it means.
        for t in a.iter_mut() {
            t["inputSchema"]["properties"]["world"] = json!({ "type": "string", "description": "The world meant (its hash, from world_overview): the call is refused if mapd has another world open" });
        }
    }
    v
}

pub(crate) fn text(v: Value) -> Vec<Value> {
    vec![json!({ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() })]
}

pub(crate) fn arg_str(a: &Value, k: &str) -> Result<String, String> {
    a[k].as_str().map(str::to_string).ok_or_else(|| format!("missing '{k}'"))
}

/// A place from `id` or `x_ft`/`y_ft` (or a value that is either).
async fn place(app: &Shared, a: &Value) -> Result<[f64; 2], String> {
    if let (Some(x), Some(y)) = (a["x_ft"].as_f64(), a["y_ft"].as_f64()) {
        return Ok([x, y]);
    }
    let id = a["id"].as_str().or_else(|| a.as_str()).ok_or("give an id, or x_ft and y_ft")?.to_string();
    app.worker.with(move |ex| agent::position(&ex.world, &ex.t0, &id).ok_or_else(|| format!("no such feature: {id}"))).await
}

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
    let Some(w) = a["world"].as_str() else { return Box::pin(dispatch(app, name, a)).await };
    let want = u64::from_str_radix(w.trim(), 16).map_err(|_| format!("world: a hash as world_overview gives it, not {w}"))?;
    if app.current() != Some(want) {
        return Err(other_world(app.current(), want));
    }
    WANT.scope(want, Box::pin(dispatch(app, name, a))).await
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
    if let Some(r) = crate::scatter::call(app, name, &a).await {
        return r;
    }
    if let Some(r) = crate::crossings::call(app, name, &a).await {
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
            let limit = a["limit"].as_u64().unwrap_or(25).clamp(1, 200) as usize;
            let hidden = a["include_hidden"].as_bool().unwrap_or(false);
            app.worker.with(move |ex| Ok(agent::search(&ex.world, &ex.t0, &q, kind.as_deref(), limit, hidden))).await.map(text)
        }
        "get_feature" => {
            let id = arg_str(&a, "id")?;
            app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| format!("no such feature: {id}"))).await.map(text)
        }
        "list_children" => {
            let id = arg_str(&a, "id")?;
            app.worker.with(move |ex| agent::children(&ex.world, &ex.t0, &id).ok_or_else(|| format!("no such feature, or it contains nothing: {id}"))).await.map(text)
        }
        "list_names" => {
            let (kind, within, settlement) = (a["kind"].as_str().map(str::to_string), a["within"].as_str().map(str::to_string), a["settlement"].as_str().map(str::to_string));
            app.worker.with(move |ex| agent::names(&ex.world, &ex.t0, kind.as_deref(), within.as_deref(), settlement.as_deref())).await.map(text)
        }
        "describe_location" => {
            let p = place(app, &a).await?;
            app.worker.with(move |ex| Ok(agent::describe(&ex.world, &ex.t0, p[0], p[1]))).await.map(text)
        }
        "features_near" => {
            let p = place(app, &a).await?;
            let r = a["radius_mi"].as_f64().unwrap_or(5.0).clamp(0.01, 500.0) * 5280.0;
            let kinds: Vec<String> = a["kinds"].as_array().map(|v| v.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let limit = a["limit"].as_u64().unwrap_or(50).clamp(1, 500) as usize;
            let hidden = a["include_hidden"].as_bool().unwrap_or(false);
            app.worker.with(move |ex| Ok(agent::near(&ex.world, &ex.t0, p, r, &kinds, limit, hidden))).await.map(text)
        }
        "route" => {
            let from = place(app, &a["from"]).await?;
            let to = place(app, &a["to"]).await?;
            app.worker.with(move |ex| Ok(agent::route(&ex.world, &ex.t0, from, to))).await.map(text)
        }
        "list_roads" => {
            let near = if a["id"].is_string() || a["x_ft"].is_number() { Some((place(app, &a).await?, a["radius_mi"].as_f64().unwrap_or(25.0).clamp(0.01, 3000.0) * 5280.0)) } else { None };
            let settlement = a["settlement"].as_str().map(str::to_string);
            let class = a["class"].as_str().map(str::to_string);
            let drawn = a["drawn"].as_bool().unwrap_or(false);
            let limit = a["limit"].as_u64().unwrap_or(50).clamp(1, 1000) as usize;
            app.worker
                .with(move |ex| {
                    let li = match &settlement {
                        Some(id) => Some(agent::layout_of(&ex.world, &ex.t0, id).filter(|&l| l < ex.t0.settlements.len()).ok_or_else(|| format!("{id} is not a settlement"))?),
                        None => None,
                    };
                    Ok(agent::roads(&ex.world, &ex.t0, near, li, class.as_deref(), drawn, limit))
                })
                .await
                .map(text)
        }
        "get_battlemap" => {
            let p = place(app, &a).await?;
            let r = a["radius_squares"].as_f64().unwrap_or(12.0).clamp(1.0, 64.0);
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
            let size = a["size_ft"].as_f64().unwrap_or(3000.0).clamp(50.0, 5_000_000.0);
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
            let name = a["name"].as_str().unwrap_or("").trim().to_string();
            known(app, &id).await?;
            app.edit("agent", 0, move |e| {
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
            let t = arg_str(&a, "text")?;
            let tags: Vec<String> = a["tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let append = a["append"].as_bool().unwrap_or(false);
            known(app, &id).await?;
            app.edit("agent", 0, move |e| {
                let n = e.notes.entry(id.clone()).or_insert_with(Note::default);
                n.text = if append && !n.text.is_empty() { format!("{}\n\n{t}", n.text) } else { t };
                for tag in tags {
                    if !n.tags.contains(&tag) {
                        n.tags.push(tag);
                    }
                }
                Ok(json!({ "tool": "annotate_feature", "id": id }))
            })
            .await
            .map(text)
        }
        "update_feature" => {
            let id = arg_str(&a, "id")?;
            known(app, &id).await?;
            let name = a["name"].as_str().map(|s| s.trim().to_string());
            let notes = a["notes"].as_str().map(str::to_string);
            let tags: Option<Vec<String>> = a["tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect());
            app.edit("agent", 0, move |e| {
                if let Some(n) = &name {
                    e.renames.insert(id.clone(), n.clone());
                    if let Some(c) = e.created.iter_mut().find(|c| c.id == id) {
                        c.name = n.clone();
                    }
                }
                if notes.is_some() || tags.is_some() {
                    let n = e.notes.entry(id.clone()).or_insert_with(Note::default);
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
            app.edit("agent", 0, move |e| {
                if hide {
                    e.hidden.insert(id.clone());
                } else {
                    e.hidden.remove(&id);
                }
                Ok(json!({ "tool": "hide_feature", "id": id, "hidden": hide }))
            })
            .await
            .map(text)
        }
        "delete_feature" => {
            let id = arg_str(&a, "id")?;
            app.edit("agent", 0, move |e| {
                let c = e
                    .created
                    .iter_mut()
                    .find(|c| c.id == id && !c.removed)
                    .ok_or_else(|| format!("{id} is not a created site (the world's own buildings go with remove_buildings; other generated features can be hidden with hide_feature)"))?;
                c.removed = true;
                Ok(json!({ "tool": "delete_feature", "id": id }))
            })
            .await
            .map(text)
        }
        "create_feature" => create(app, a).await.map(text),
        _ => Err(format!("unknown tool: {name}")),
    }
}

/// An id the world knows (so edits never pile up on typos).
pub(crate) async fn known(app: &Shared, id: &str) -> Result<(), String> {
    let id2 = id.to_string();
    app.worker.with(move |ex| agent::position(&ex.world, &ex.t0, &id2).map(|_| ()).ok_or_else(|| format!("no such feature: {id2}"))).await
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
    let (p, name) = app.worker.with(move |ex| agent::creation_spot(&ex.world, &ex.t0, &kind, under.as_deref(), &id, p)).await?;
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
    app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "created, but it could not be read back".to_string())).await
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
    v["existing"] = json!(true);
    v["note"] = json!("this was already here: its id is returned and nothing was added");
    Ok(v)
}
