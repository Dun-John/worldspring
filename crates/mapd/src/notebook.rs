//! MCP tools for the DM's notebook: NPCs (`n:<id>`) and plot points (`p:<id>`). Both are
//! authored by the user or agents, never generated, and live in the world file's edits.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::world::{Npc, NpcPlace, PLOT_STATUSES, Plot, STANCES};

use crate::Shared;
use crate::tools::{arg_str, known, schema, text};

fn npc_props() -> Value {
    json!({
        "name": { "type": "string" },
        "appearance": { "type": "string", "description": "How they look and sound" },
        "mannerisms": { "type": "string", "description": "Habits, speech, quirks" },
        "stance": { "type": "string", "enum": STANCES, "description": "Attitude toward the players" },
        "attitude": { "type": "string", "description": "Why they feel that way, what would change it" },
        "goals": { "type": "string" },
        "notes": { "type": "string", "description": "Secrets and anything else for the DM" },
        "tags": { "type": "array", "items": { "type": "string" } },
        "status": { "type": "string", "description": "Free text: alive, dead, missing, imprisoned..." },
        "portrait": { "type": "string", "description": "An asset id (a picture uploaded to the app or to /assets)" },
    })
}

fn place_props() -> Value {
    json!({
        "location": { "type": "string", "description": "Where they are found: a feature, building, district or site id (empty: nowhere)" },
        "level": { "type": "integer", "description": "Inside a building or site: the level (0 = lowest; default the way in)" },
        "x_ft": { "type": "number", "description": "A point (ft) to stand at; alone, the place there is looked up" },
        "y_ft": { "type": "number" },
    })
}

fn plot_props() -> Value {
    json!({
        "title": { "type": "string" },
        "text": { "type": "string", "description": "What is going on, hooks, how it may play out" },
        "status": { "type": "string", "enum": PLOT_STATUSES },
        "anchors": { "type": "array", "items": { "type": "string" }, "description": "Ids of the places it is tied to" },
        "npcs": { "type": "array", "items": { "type": "string" }, "description": "Ids of the NPCs involved (n:...)" },
        "tags": { "type": "array", "items": { "type": "string" } },
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
    let ro = json!({ "readOnlyHint": true });
    vec![
        json!({ "name": "list_npcs", "title": "NPCs", "description": "The world's NPCs (written by the DM or agents): name, attitude, status, where they are. Filter by text, place (also on its levels and in its rooms; a settlement's id also finds those in its buildings and the sites standing in it; a building drawn by hand by either of its ids), tag or stance.", "inputSchema": schema(json!({ "query": { "type": "string" }, "location": { "type": "string" }, "tag": { "type": "string" }, "stance": { "type": "string", "enum": STANCES } }), &[]), "annotations": ro }),
        json!({ "name": "get_npc", "title": "NPC", "description": "An NPC in full: appearance, mannerisms, attitude toward the players, goals, notes, where they are and the plots they are in.", "inputSchema": schema(id.clone(), &["id"]), "annotations": ro }),
        json!({ "name": "create_npc", "title": "Create an NPC", "description": "Add an NPC, optionally placed somewhere (a building's id puts them inside it; the DM sees them there). Returns the NPC with its id.", "inputSchema": schema(with(npc_props(), place_props()), &["name"]) }),
        json!({ "name": "update_npc", "title": "Update an NPC", "description": "Change an NPC's fields (only those given).", "inputSchema": schema(with(id.clone(), npc_props()), &["id"]) }),
        json!({ "name": "delete_npc", "title": "Delete an NPC", "description": "Delete an NPC (and take them out of plots).", "inputSchema": schema(id.clone(), &["id"]) }),
        json!({ "name": "place_npc", "title": "Place an NPC", "description": "Put an NPC somewhere: a place id (and level inside), or a point; an empty location takes them off the map.", "inputSchema": schema(with(id.clone(), place_props()), &["id"]) }),
        json!({ "name": "list_plots", "title": "Plot points", "description": "The world's plot points: title, status, places and NPCs. Filter by text, status, place, NPC or tag.", "inputSchema": schema(json!({ "query": { "type": "string" }, "status": { "type": "string", "enum": PLOT_STATUSES }, "anchor": { "type": "string" }, "npc": { "type": "string" }, "tag": { "type": "string" } }), &[]), "annotations": ro }),
        json!({ "name": "get_plot", "title": "Plot point", "description": "A plot point in full.", "inputSchema": schema(id.clone(), &["id"]), "annotations": ro }),
        json!({ "name": "create_plot", "title": "Create a plot point", "description": "Add a plot point tied to places and NPCs. Returns it with its id.", "inputSchema": schema(plot_props(), &["title"]) }),
        json!({ "name": "update_plot", "title": "Update a plot point", "description": "Change a plot point's fields (only those given; lists are replaced).", "inputSchema": schema(with(id.clone(), plot_props()), &["id"]) }),
        json!({ "name": "delete_plot", "title": "Delete a plot point", "description": "Delete a plot point.", "inputSchema": schema(id, &["id"]) }),
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
    let at = match (a["x_ft"].as_f64(), a["y_ft"].as_f64()) {
        (Some(x), Some(y)) => Some([x, y]),
        _ => None,
    };
    let level = a["level"].as_u64().map(|l| l as usize);
    let id = match a["location"].as_str().map(str::trim) {
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
    Some(match name {
        "list_npcs" | "get_npc" | "list_plots" | "get_plot" => read(app, name, a.clone()).await.map(text),
        "create_npc" | "update_npc" | "place_npc" => write_npc(app, name, a).await.map(text),
        "delete_npc" => match arg_str(a, "id") {
            Err(e) => Err(e),
            Ok(id) => app
                .edit("agent", 0, move |e| {
                    let n = e.npcs.remove(&id).ok_or_else(|| format!("no such NPC: {id}"))?;
                    for p in e.plots.values_mut() {
                        p.npcs.retain(|k| *k != id);
                    }
                    Ok(json!({ "tool": "delete_npc", "id": id, "name": n.name }))
                })
                .await
                .map(text),
        },
        "create_plot" | "update_plot" => write_plot(app, name, a).await.map(text),
        "delete_plot" => match arg_str(a, "id") {
            Err(e) => Err(e),
            Ok(id) => app
                .edit("agent", 0, move |e| {
                    let p = e.plots.remove(&id).ok_or_else(|| format!("no such plot point: {id}"))?;
                    Ok(json!({ "tool": "delete_plot", "id": id, "name": p.title }))
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
                "get_npc" => agent::npc_json(w, t0, a["id"].as_str().unwrap_or(""), false).ok_or_else(|| "no such NPC".to_string()),
                "get_plot" => agent::plot_json(w, t0, a["id"].as_str().unwrap_or(""), false).ok_or_else(|| "no such plot point".to_string()),
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
                    Ok(json!({ "count": v.len(), "npcs": v }))
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
                    Ok(json!({ "count": v.len(), "plots": v }))
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
    app.edit("agent", 0, move |e| {
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
        e.npcs.insert(id2.clone(), n);
        Ok(json!({ "tool": tool, "id": id2, "name": name }))
    })
    .await?;
    app.worker.with(move |ex| agent::npc_json(&ex.world, &ex.t0, &id, false).ok_or_else(|| "saved, but it could not be read back".to_string())).await
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
    app.edit("agent", 0, move |e| {
        let mut p = match e.plots.get(&id2) {
            Some(p) if tool != "create_plot" => p.clone(),
            None if tool != "create_plot" => return Err(format!("no such plot point: {id2}")),
            _ => Plot::default(),
        };
        plot_fields(&mut p, &a2, &e.npcs)?;
        let title = p.title.clone();
        e.plots.insert(id2.clone(), p);
        Ok(json!({ "tool": tool, "id": id2, "name": title }))
    })
    .await?;
    app.worker.with(move |ex| agent::plot_json(&ex.world, &ex.t0, &id, false).ok_or_else(|| "saved, but it could not be read back".to_string())).await
}
