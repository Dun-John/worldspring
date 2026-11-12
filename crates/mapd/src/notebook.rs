//! MCP tools for the DM's notebook: NPCs (`n:<id>`) and plot points (`p:<id>`). Both are
//! authored by the user or agents, never generated, and live in the world file's edits.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::world::{Npc, NpcPlace, PLOT_STATUSES, Plot, STANCES};

use crate::Shared;
use crate::tools::{arg_str, known, listed, schema, text};

fn npc_props() -> Value {
    json!({
        "name": { "type": "string" },
        "appearance": { "type": "string", "description": "How they look and sound" },
        "mannerisms": { "type": "string", "description": "Habits, speech, quirks" },
        "stance": { "type": "string", "enum": STANCES, "description": "Attitude toward the players" },
        "attitude": { "type": "string" },
        "goals": { "type": "string" },
        "notes": { "type": "string" },
        "tags": { "type": "array", "items": { "type": "string" } },
        "status": { "type": "string" },
        "portrait": { "type": "string", "description": "An asset id" },
    })
}

fn place_props() -> Value {
    json!({
        "location": { "type": "string", "description": "A place id (empty: nowhere)" },
        "level": { "type": "integer", "minimum": 0, "description": "Level inside a building or site (0 lowest)" },
        "x_ft": { "type": "number", "description": "A point to stand at" },
        "y_ft": { "type": "number" },
        "at": crate::args::point_schema("Instead of location, x_ft, y_ft: an id (the location) or a point"),
    })
}

fn plot_props() -> Value {
    json!({
        "title": { "type": "string" },
        "text": { "type": "string" },
        "status": { "type": "string", "enum": PLOT_STATUSES },
        "anchors": { "type": "array", "items": { "type": "string" } },
        "npcs": { "type": "array", "items": { "type": "string" } },
        "tags": { "type": "array", "items": { "type": "string" } },
    })
}

/// A note's tags in one form: the quick picks lower-case, the others deduped ignoring case (the app's `canonTags`).
pub(crate) fn canon_tags(tags: Vec<String>) -> Vec<String> {
    const QUICK: [&str; 5] = ["treasure", "read-aloud", "trap", "secret", "monster"];
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim();
        if t.is_empty() || out.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            continue;
        }
        let k = t.to_lowercase();
        out.push(if QUICK.contains(&k.as_str()) { k } else { t.to_string() });
    }
    out
}

fn note_spot() -> Value {
    json!({
        "site": { "type": "string", "description": "Building, dungeon or sewer section id" },
        "level": { "type": "integer", "minimum": 0, "description": "Level index" },
        "at": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x, y] grid squares" },
        "room": { "type": "string", "description": "r: id (its centre)" },
    })
}

fn with(a: Value, b: Value) -> Value {
    let mut a = a;
    if let (Some(m), Value::Object(n)) = (a.as_object_mut(), b) {
        m.extend(n);
    }
    a
}

pub fn list() -> Vec<Value> {
    let id = json!({ "id": { "type": "string" } });
    let ids = crate::args::ids_props("id");
    let ro = json!({ "readOnlyHint": true });
    vec![
        json!({ "name": "list_npcs", "title": "NPCs", "description": "The world's NPCs, {items, found}; limit (default 100) cuts it and sets truncated. Filter by text, location (also its levels, rooms and, for a settlement, its buildings and sites), tag or stance.", "inputSchema": schema(json!({ "query": { "type": "string" }, "location": { "type": "string" }, "tag": { "type": "string" }, "stance": { "type": "string", "enum": STANCES }, "limit": crate::tools::limit_prop(100, 1000) }), &[]), "annotations": ro }),
        json!({ "name": "get_npc", "title": "NPC", "description": "An NPC in full: appearance, mannerisms, attitude toward the players, goals, notes, where they are and the plots they are in.", "inputSchema": schema(id.clone(), &["id"]), "annotations": ro }),
        json!({ "name": "create_npc", "title": "Create an NPC", "description": "Add an NPC, optionally placed somewhere (a building's id puts them inside it; the DM sees them there). Returns the NPC with its id.", "inputSchema": schema(with(npc_props(), place_props()), &["name"]) }),
        json!({ "name": "update_npc", "title": "Update an NPC", "description": "Change an NPC's fields (only those given).", "inputSchema": schema(with(id.clone(), npc_props()), &["id"]) }),
        json!({ "name": "delete_npc", "title": "Delete an NPC", "description": "Delete an NPC, or several (id or ids; all or none), and take them out of plots.", "inputSchema": schema(ids.clone(), &[]) }),
        json!({ "name": "place_npc", "title": "Place an NPC", "description": "Put an NPC somewhere: a place id (and level inside), or a point (at, or x_ft and y_ft); an empty location takes them off the map.", "inputSchema": schema(with(id.clone(), place_props()), &["id"]) }),
        json!({ "name": "list_plots", "title": "Plot points", "description": "The world's plot points (returns {items, found}, at most limit, default 100; truncated: true when cut): title, status, places and NPCs. Filter by text, status, place, NPC or tag.", "inputSchema": schema(json!({ "query": { "type": "string" }, "status": { "type": "string", "enum": PLOT_STATUSES }, "anchor": { "type": "string" }, "npc": { "type": "string" }, "tag": { "type": "string" }, "limit": crate::tools::limit_prop(100, 1000) }), &[]), "annotations": ro }),
        json!({ "name": "get_plot", "title": "Plot point", "description": "A plot point in full.", "inputSchema": schema(id.clone(), &["id"]), "annotations": ro }),
        json!({ "name": "create_plot", "title": "Create a plot point", "description": "Add a plot point tied to places and NPCs. Returns it with its id.", "inputSchema": schema(plot_props(), &["title"]) }),
        json!({ "name": "update_plot", "title": "Update a plot point", "description": "Change a plot point's fields (only those given; lists are replaced).", "inputSchema": schema(with(id.clone(), plot_props()), &["id"]) }),
        json!({ "name": "add_note_at", "title": "Add a note in a room", "description": "Put a note at a point in a building, dungeon or sewer level (site, level, at; or room). Returns it with its m: id.", "inputSchema": schema(with(note_spot(), json!({ "text": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } } })), &["text"]) }),
        json!({ "name": "move_note", "title": "Move a point note", "description": "Move an m: note (at, or room on its level).", "inputSchema": schema(with(json!({ "id": { "type": "string" } }), json!({ "at": note_spot()["at"], "room": note_spot()["room"] })), &["id"]) }),
        json!({ "name": "delete_note", "title": "Delete a note", "description": "Delete a note (place, r: or m: id).", "inputSchema": schema(id.clone(), &["id"]) }),
        json!({ "name": "list_notes", "title": "Notes", "description": "Notes on places, rooms and points: {items, found}, limit default 100. Filter by tag, site, level, kind, query.", "inputSchema": schema(json!({ "query": { "type": "string" }, "tag": { "type": "string" }, "site": { "type": "string" }, "level": { "type": "integer", "minimum": 0 }, "kind": { "type": "string", "enum": ["place", "room", "point"] }, "limit": crate::tools::limit_prop(100, 1000) }), &[]), "annotations": ro }),
        json!({ "name": "delete_plot", "title": "Delete a plot point", "description": "Delete a plot point, or several (id or ids; all or none).", "inputSchema": schema(ids, &[]) }),
    ]
}

/// A new id: `<prefix>:` and a time-and-chance suffix, so the app and an agent creating at the
/// same moment never collide.
pub(crate) fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let mut h = worldgen::core::rng::mix64(t ^ N.fetch_add(1, Ordering::Relaxed).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ std::process::id() as u64);
    let mut s = String::from(prefix);
    s.push(':');
    for _ in 0..8 {
        s.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(h % 36) as usize] as char);
        h /= 36;
    }
    s
}

fn strs(v: &Value) -> Option<Vec<String>> {
    v.as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect())
}

/// The fields given in `a`, onto `n`.
fn npc_fields(n: &mut Npc, a: &Value) -> Result<(), String> {
    for (k, f) in [("name", &mut n.name), ("appearance", &mut n.appearance), ("mannerisms", &mut n.mannerisms), ("goals", &mut n.goals), ("notes", &mut n.notes)] {
        if let Some(s) = a[k].as_str() {
            *f = s.to_string();
        }
    }
    if let Some(s) = a["attitude"].as_str() {
        n.attitude.text = s.to_string();
    }
    if let Some(s) = a["stance"].as_str() {
        n.attitude.stance = serde_json::from_value(json!(s)).map_err(|_| format!("stance must be one of {}", STANCES.join(", ")))?;
    }
    if let Some(t) = strs(&a["tags"]) {
        n.tags = t;
    }
    if let Some(s) = a["status"].as_str() {
        n.status = Some(s.trim().to_string()).filter(|s| !s.is_empty());
    }
    if let Some(s) = a["portrait"].as_str() {
        n.portrait = Some(s.trim().to_string()).filter(|s| !s.is_empty());
    }
    if n.name.trim().is_empty() {
        return Err("an NPC needs a name".into());
    }
    Ok(())
}

/// Where `a` puts an NPC: `Some(None)` off the map, `None` if it says nothing about it.
async fn npc_place(app: &Shared, a: &Value) -> Result<Option<Option<NpcPlace>>, String> {
    // `at`: an id is the location, a point is x_ft and y_ft; not together with those.
    let (mut location, mut at) = (a["location"].as_str().map(str::trim), None);
    let given = |k: &str| !a[k].is_null();
    if given("at") {
        if given("location") || given("x_ft") || given("y_ft") {
            return Err("at and location/x_ft/y_ft: give one place".into());
        }
        match &a["at"] {
            Value::String(id) => location = Some(id.trim()),
            p => at = Some(crate::args::point(app, "at", p).await?),
        }
    } else {
        at = match (a["x_ft"].as_f64(), a["y_ft"].as_f64()) {
            (Some(x), Some(y)) => Some(crate::args::point(app, "x_ft", &json!([x, y])).await?),
            (Some(_), None) => return Err("y_ft: needed with x_ft".into()),
            (None, Some(_)) => return Err("x_ft: needed with y_ft".into()),
            _ => None,
        };
    }
    let level = a["level"].as_u64().map(|l| l as usize);
    let id = match location {
        Some("") => return Ok(Some(None)),
        Some(id) if agent::is_authored(id) => return Err("a location is a place (a feature, building, district or site), not an NPC or plot".into()),
        Some(id) => {
            known(app, id).await?;
            id.to_string()
        }
        None => match at {
            // A point alone: the building, district or settlement there, else the nearest named place.
            Some(p) => app
                .worker
                .with(move |ex| {
                    let (w, t0) = (&ex.world, &ex.t0);
                    let hit = serde_json::to_value(worldgen::gazetteer::query(w, t0, p[0], p[1])).unwrap_or(Value::Null);
                    hit["id"]
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| hit["settlement"].as_u64().and_then(|li| agent::feature_of_layout(w, t0, li as usize)))
                        .or_else(|| agent::describe(w, t0, p[0], p[1])["nearby"][0]["id"].as_str().map(str::to_string))
                        .ok_or_else(|| "nothing named there: give a location id".to_string())
                })
                .await?,
            None if level.is_some() => return Err("a level needs a location".into()),
            None => return Ok(None),
        },
    };
    Ok(Some(Some(NpcPlace { id, level, x: at.map(|p| p[0]), y: at.map(|p| p[1]) })))
}

fn plot_fields(p: &mut Plot, a: &Value, npcs: &std::collections::BTreeMap<String, Npc>) -> Result<(), String> {
    if let Some(s) = a["title"].as_str() {
        p.title = s.to_string();
    }
    if let Some(s) = a["text"].as_str() {
        p.text = s.to_string();
    }
    if let Some(s) = a["status"].as_str() {
        p.status = serde_json::from_value(json!(s)).map_err(|_| format!("status must be one of {}", PLOT_STATUSES.join(", ")))?;
    }
    if let Some(v) = strs(&a["anchors"]) {
        p.anchors = v;
    }
    if let Some(v) = strs(&a["npcs"]) {
        if let Some(bad) = v.iter().find(|k| !npcs.contains_key(*k)) {
            return Err(format!("no such NPC: {bad}"));
        }
        p.npcs = v;
    }
    if let Some(t) = strs(&a["tags"]) {
        p.tags = t;
    }
    if p.title.trim().is_empty() {
        return Err("a plot point needs a title".into());
    }
    Ok(())
}

fn has(text: &str, q: &str) -> bool {
    text.to_lowercase().contains(q)
}

/// Run a notebook tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    let single = a["ids"].is_null();
    Some(match name {
        "list_npcs" | "get_npc" | "list_plots" | "get_plot" => read(app, name, a.clone()).await.map(text),
        "create_npc" | "update_npc" | "place_npc" => write_npc(app, name, a).await.map(text),
        "delete_npc" => match crate::args::ids(a) {
            Err(e) => Err(e),
            Ok(ids) => app
                .edit("agent", 0, move |e| {
                    for (i, id) in ids.iter().enumerate() {
                        if !e.npcs.contains_key(id) {
                            return Err(format!("{}: no such NPC: {id}", crate::args::item(single, i)));
                        }
                    }
                    let mut names = Vec::new();
                    for id in &ids {
                        if let Some(n) = e.npcs.remove(id) {
                            names.push(n.name);
                        }
                        for p in e.plots.values_mut() {
                            p.npcs.retain(|k| k != id);
                        }
                    }
                    Ok(json!({ "tool": "delete_npc", "ids": ids, "names": names }))
                })
                .await
                .map(text),
        },
        "add_note_at" | "move_note" => write_point(app, name, a).await.map(text),
        "delete_note" => delete_note(app, a).await.map(text),
        "list_notes" => list_notes(app, a.clone()).await.map(text),
        "create_plot" | "update_plot" => write_plot(app, name, a).await.map(text),
        "delete_plot" => match crate::args::ids(a) {
            Err(e) => Err(e),
            Ok(ids) => app
                .edit("agent", 0, move |e| {
                    for (i, id) in ids.iter().enumerate() {
                        if !e.plots.contains_key(id) {
                            return Err(format!("{}: no such plot point: {id}", crate::args::item(single, i)));
                        }
                    }
                    let names: Vec<String> = ids.iter().filter_map(|id| e.plots.remove(id)).map(|p| p.title).collect();
                    Ok(json!({ "tool": "delete_plot", "ids": ids, "names": names }))
                })
                .await
                .map(text),
        },
        _ => return None,
    })
}

async fn read(app: &Shared, name: &str, a: Value) -> Result<Value, String> {
    let name = name.to_string();
    app.worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let e = &w.file.edits;
            let q = a["query"].as_str().unwrap_or("").trim().to_lowercase();
            let tag = a["tag"].as_str();
            let tagged = |tags: &[String]| tag.is_none_or(|t| tags.iter().any(|x| x.eq_ignore_ascii_case(t)));
            match name.as_str() {
                "get_npc" => agent::npc_json(w, t0, a["id"].as_str().unwrap_or(""), false).ok_or_else(|| format!("no such NPC: {}", a["id"].as_str().unwrap_or(""))),
                "get_plot" => agent::plot_json(w, t0, a["id"].as_str().unwrap_or(""), false).ok_or_else(|| format!("no such plot point: {}", a["id"].as_str().unwrap_or(""))),
                "list_npcs" => {
                    // A settlement or site also holds its buildings, districts, towers and ways underground
                    // (and their levels and rooms); a settlement, the sites in it (`agent::at_place`).
                    let at = a["location"].as_str().map(|id| agent::at_place(w, t0, id));
                    let inside = |l: &str| at.as_ref().is_none_or(|f| f(l));
                    let stance = a["stance"].as_str();
                    let v: Vec<Value> = e
                        .npcs
                        .iter()
                        .filter(|(_, n)| q.is_empty() || [&n.name, &n.appearance, &n.mannerisms, &n.goals, &n.notes, &n.attitude.text].iter().any(|s| has(s, &q)))
                        .filter(|(_, n)| tagged(&n.tags) && stance.is_none_or(|s| serde_json::to_value(n.attitude.stance).ok().and_then(|v| v.as_str().map(|x| x == s)).unwrap_or(false)))
                        .filter(|(_, n)| at.is_none() || n.location.as_ref().is_some_and(|l| inside(&l.id)))
                        .filter_map(|(k, _)| agent::npc_json(w, t0, k, true))
                        .collect();
                    Ok(listed(json!(v), crate::tools::limit_of(&a, 100), json!({})))
                }
                _ => {
                    let status = a["status"].as_str();
                    let (anchor, npc) = (a["anchor"].as_str(), a["npc"].as_str());
                    let v: Vec<Value> = e
                        .plots
                        .iter()
                        .filter(|(_, p)| q.is_empty() || has(&p.title, &q) || has(&p.text, &q))
                        .filter(|(_, p)| tagged(&p.tags) && status.is_none_or(|s| serde_json::to_value(p.status).ok().and_then(|v| v.as_str().map(|x| x == s)).unwrap_or(false)))
                        .filter(|(_, p)| anchor.is_none_or(|x| p.anchors.iter().any(|y| y == x)) && npc.is_none_or(|x| p.npcs.iter().any(|y| y == x)))
                        .filter_map(|(k, _)| agent::plot_json(w, t0, k, true))
                        .collect();
                    Ok(listed(json!(v), crate::tools::limit_of(&a, 100), json!({})))
                }
            }
        })
        .await
}

async fn write_npc(app: &Shared, tool: &str, a: &Value) -> Result<Value, String> {
    let id = if tool == "create_npc" { new_id("n") } else { arg_str(a, "id")? };
    let place = npc_place(app, a).await?;
    if tool == "place_npc" && place.is_none() {
        return Err("give a location (empty to take them off the map), or x_ft and y_ft".into());
    }
    let (tool, a2) = (tool.to_string(), a.clone());
    let id2 = id.clone();
    let change = app.edit("agent", 0, move |e| {
        let mut n = match e.npcs.get(&id2) {
            Some(n) if tool != "create_npc" => n.clone(),
            None if tool != "create_npc" => return Err(format!("no such NPC: {id2}")),
            _ => Npc::default(),
        };
        if tool != "place_npc" {
            npc_fields(&mut n, &a2)?;
        }
        if let Some(p) = place {
            n.location = p;
        }
        let name = n.name.clone();
        // The same NPC again (a script run twice): that one, nothing added.
        if tool == "create_npc" && let Some((k, _)) = e.npcs.iter().find(|(_, o)| o.name.trim().to_lowercase() == name.trim().to_lowercase() && o.location == n.location) {
            return Ok(json!({ "existing": k }));
        }
        e.npcs.insert(id2.clone(), n);
        Ok(json!({ "tool": tool, "id": id2, "name": name }))
    })
    .await?;
    let existing = change["existing"].as_str().map(str::to_string);
    let id = existing.clone().unwrap_or(id);
    let mut v = app.worker.with(move |ex| agent::npc_json(&ex.world, &ex.t0, &id, false).ok_or_else(|| "saved, but it could not be read back".to_string())).await?;
    if existing.is_some() {
        v["existing"] = json!(true);
        v["note"] = json!("an NPC with this name and place was already here: its id is returned and nothing was added (give another name or place for a second one)");
    }
    Ok(v)
}

async fn write_plot(app: &Shared, tool: &str, a: &Value) -> Result<Value, String> {
    let id = if tool == "create_plot" { new_id("p") } else { arg_str(a, "id")? };
    for anchor in strs(&a["anchors"]).unwrap_or_default() {
        if agent::is_authored(&anchor) {
            return Err(format!("{anchor}: anchors are places; NPCs go in 'npcs'"));
        }
        known(app, &anchor).await?;
    }
    let (tool, a2) = (tool.to_string(), a.clone());
    let id2 = id.clone();
    let change = app.edit("agent", 0, move |e| {
        let mut p = match e.plots.get(&id2) {
            Some(p) if tool != "create_plot" => p.clone(),
            None if tool != "create_plot" => return Err(format!("no such plot point: {id2}")),
            _ => Plot::default(),
        };
        plot_fields(&mut p, &a2, &e.npcs)?;
        let title = p.title.clone();
        // The same plot point again (a script run twice): that one, nothing added.
        if tool == "create_plot" && let Some((k, _)) = e.plots.iter().find(|(_, o)| o.title.trim().to_lowercase() == title.trim().to_lowercase()) {
            return Ok(json!({ "existing": k }));
        }
        e.plots.insert(id2.clone(), p);
        Ok(json!({ "tool": tool, "id": id2, "name": title }))
    })
    .await?;
    let existing = change["existing"].as_str().map(str::to_string);
    let id = existing.clone().unwrap_or(id);
    let mut v = app.worker.with(move |ex| agent::plot_json(&ex.world, &ex.t0, &id, false).ok_or_else(|| "saved, but it could not be read back".to_string())).await?;
    if existing.is_some() {
        v["existing"] = json!(true);
        v["note"] = json!("a plot point with this title was already here: its id is returned and nothing was added (give another title for a second one)");
    }
    Ok(v)
}

/// Where `a` puts a point note: its site, level and grid point (rounded to 0.01), checked against
/// the level (inside its grid; the second is whether the point is on a room's floor or outside
/// the rooms). `room` (an `r:` id) is its centre; `fixed` is a site and level the note already has.
fn spot(w: &worldgen::world::World, t0: &worldgen::t0::T0, a: &Value, fixed: Option<(&str, usize)>) -> Result<(String, usize, [f64; 2], bool), String> {
    let (mut site, mut li) = (a["site"].as_str().map(str::to_string), a["level"].as_u64().map(|l| l as usize));
    let mut at = None;
    if let Some(r) = a["room"].as_str() {
        if !a["at"].is_null() {
            return Err("at and room: give one".into());
        }
        let (s, l, ri) = agent::split_room_id(r).and_then(|(s, l, ri)| Some((s, l, ri?))).ok_or_else(|| format!("room: an r: id (r:<site>:<level>:<room>), got {r}"))?;
        if site.as_deref().is_some_and(|x| x != s) || li.is_some_and(|x| x != l) {
            return Err(format!("room {r} is on level {l} of {s}, not the site and level given"));
        }
        let it = worldgen::interior::generate_id(w, t0, s).ok_or_else(|| format!("no such room: {r}"))?;
        let lv = it.levels.get(l).ok_or_else(|| format!("no such room: {r}"))?;
        let c = lv.rooms.get(ri).ok_or_else(|| format!("no such room: {r}"))?.center;
        let c = [c[0] as f64, c[1] as f64];
        // The centre itself when it lies in the room; else the room's square nearest to it.
        let mut best = c;
        if agent::room_at(&it, l, c) != Some(ri) {
            let d = |p: &[f64; 2]| (p[0] - c[0]) * (p[0] - c[0]) + (p[1] - c[1]) * (p[1] - c[1]);
            best = (0..it.nx * it.ny).filter(|&k| lv.cells[k] == ri as i16).map(|k| [(k % it.nx) as f64 + 0.5, (k / it.nx) as f64 + 0.5]).min_by(|a, b| d(a).total_cmp(&d(b))).unwrap_or(c);
        }
        (site, li, at) = (Some(s.to_string()), Some(l), Some(best));
    } else if let Some(p) = a["at"].as_array() {
        match p.iter().map(Value::as_f64).collect::<Vec<_>>().as_slice() {
            [Some(x), Some(y)] if x.is_finite() && y.is_finite() => at = Some([*x, *y]),
            _ => return Err("at: [x, y] in grid squares".into()),
        }
    }
    if let Some((s, l)) = fixed {
        if site.as_deref().is_some_and(|x| x != s) || li.is_some_and(|x| x != l) {
            return Err(format!("a note stays on its level ({s}, level {l}): delete it and add one there"));
        }
        (site, li) = (Some(s.to_string()), Some(l));
    }
    let (Some(site), Some(li)) = (site, li) else { return Err("give site and level, or a room".into()) };
    let Some(at) = at else { return Err("give at [x, y], or a room".into()) };
    let Some(it) = worldgen::interior::generate_id(w, t0, &site) else {
        if let Some(l) = site.strip_prefix("w:").filter(|l| l.parse::<u32>().is_ok()) {
            return Err(format!("{site} is a whole sewer network: a note goes in one of its 480-ft sections, w:{l}:<sx>:<sy> (or give room: an r: id)"));
        }
        return Err(if agent::position(w, t0, &site).is_some() {
            format!("{site} has no inside: point notes go in a building, dungeon or sewer level (annotate_feature puts a note on a place on the map)")
        } else {
            crate::args::no_such(w, t0, &site)
        });
    };
    if li >= it.levels.len() {
        return Err(format!("level: 0 to {}, got {li} ({site} has {} levels)", it.levels.len() - 1, it.levels.len()));
    }
    if !(0.0..it.nx as f64).contains(&at[0]) || !(0.0..it.ny as f64).contains(&at[1]) {
        return Err(format!("at: x 0 to {}, y 0 to {} (grid squares of this level), got [{}, {}]", it.nx, it.ny, at[0], at[1]));
    }
    let at = [(at[0] * 100.0).round() / 100.0, (at[1] * 100.0).round() / 100.0];
    Ok((site, li, at, agent::room_at(&it, li, at).is_some()))
}

pub(crate) async fn write_point(app: &Shared, tool: &str, a: &Value) -> Result<Value, String> {
    let (tool, a) = (tool.to_string(), a.clone());
    let add = tool == "add_note_at";
    let given = if add { None } else { Some(arg_str(&a, "id")?) };
    if let Some(id) = &given
        && agent::split_point_id(id).is_none()
    {
        return Err(format!("{id}: not a point note (m: id); a place's or room's note has no point to move"));
    }
    let (a2, g2) = (a.clone(), given.clone());
    let (id, site_li, on_room) = app
        .worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let fixed = match &g2 {
                Some(id) => {
                    w.file.edits.notes.get(id).ok_or_else(|| format!("no such note: {id}"))?;
                    agent::split_point_id(id).map(|(s, l, _)| (s, l))
                }
                None => None,
            };
            let (site, li, at, on_room) = spot(w, t0, &a2, fixed)?;
            let id = g2.unwrap_or_else(|| format!("m:{site}:{li}:{}", new_id("").trim_start_matches(':')));
            Ok((id, (site, li, at), on_room))
        })
        .await?;
    let (site, li, at) = site_li;
    if add && a["text"].as_str().is_none_or(|t| t.trim().is_empty()) {
        return Err("text: the note's text (not empty)".into());
    }
    let (id2, tags, text) = (id.clone(), canon_tags(strs(&a["tags"]).unwrap_or_default()), a["text"].as_str().map(str::to_string));
    app.edit("agent", 0, move |e| {
        if add {
            e.notes.insert(id2.clone(), worldgen::world::Note { text: text.unwrap_or_default(), tags, at: Some(at) });
        } else {
            e.notes.get_mut(&id2).ok_or_else(|| format!("no such note: {id2}"))?.at = Some(at);
        }
        Ok(json!({ "tool": tool, "id": id2 }))
    })
    .await?;
    let id2 = id.clone();
    let mut v = app
        .worker
        .with(move |ex| {
            let n = ex.world.file.edits.notes.get(&id2).ok_or("saved, but it could not be read back")?;
            agent::note_row(&ex.world, &ex.t0, &id2, n).ok_or_else(|| "saved, but it could not be read back".to_string())
        })
        .await?;
    v["site"] = json!(site);
    v["level"] = json!(li);
    v["on"] = json!(if on_room { "a room's floor" } else { "outside the rooms (inside the level's grid)" });
    Ok(v)
}

async fn delete_note(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = arg_str(a, "id")?;
    let id2 = id.clone();
    let name = app.worker.with(move |ex| Ok(agent::name_any(&ex.world, &ex.t0, &id2))).await?;
    // (A building drawn by hand keeps its note under its created id: the same key as annotate_feature.)
    let pair = crate::tools::drawn_pair(app, &id).await?;
    app.edit("agent", 0, move |e| {
        let key = crate::tools::note_key(e, pair, &id);
        let n = e.notes.remove(&key).or_else(|| e.notes.remove(&id)).ok_or_else(|| format!("no note on {id}"))?;
        Ok(json!({ "tool": "delete_note", "id": id, "name": name, "text": n.text, "tags": n.tags }))
    })
    .await
}

async fn list_notes(app: &Shared, a: Value) -> Result<Value, String> {
    app.worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let q = a["query"].as_str().unwrap_or("").trim().to_lowercase();
            let tag = a["tag"].as_str();
            let (kind, level) = (a["kind"].as_str(), a["level"].as_u64().map(|l| l as usize));
            let site = a["site"].as_str();
            let at = site.map(|s| agent::at_place(w, t0, s));
            // (Each site's inside is generated once for the whole list.)
            let mut cache = Default::default();
            let v: Vec<Value> = w
                .file
                .edits
                .notes
                .iter()
                // (Notes set aside with a place no longer on the map are left out, as in the app.)
                .filter(|(k, _)| !k.starts_with("gone:") && !k.starts_with("was:") && worldgen::world::layout_in_id(k).is_none_or(|l| l < worldgen::world::PARKED_LAYOUT))
                .filter(|(_, n)| tag.is_none_or(|t| n.tags.iter().any(|x| x.eq_ignore_ascii_case(t))))
                .filter(|(_, n)| q.is_empty() || n.text.to_lowercase().contains(&q))
                .filter(|(k, _)| at.as_ref().is_none_or(|f| f(k)))
                .filter(|(k, _)| level.is_none_or(|l| agent::site_level(k).is_some_and(|s| s.1 == l)))
                .filter_map(|(k, n)| agent::note_row_in(w, t0, k, n, &mut cache))
                .filter(|r| kind.is_none_or(|x| r["kind"] == x))
                .collect();
            Ok(listed(json!(v), crate::tools::limit_of(&a, 100), json!({})))
        })
        .await
}
