//! MCP tools for paired exits put down by hand (`Edits::links`, `j:<id>`): a square on a level of a
//! site underground, a sewer section or a building's cellar joined to another (`worldgen::links`).

use serde_json::{Value, json};
use worldgen::links::{self, End, Link, WAYS};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{listed, schema, text};

fn end_schema(description: &str) -> Value {
    json!({ "type": "object", "description": description, "properties": {
        "at": { "type": "string", "description": "A level (l:<site>:<level>, from the bottom), a room (r:<site>:<level>:<room>), or a site: u:/k: (top level), w:<layout>:<sx>:<sy> (sewer section), b: (cellar)" },
        "x": { "type": "integer", "minimum": 0, "maximum": 4095, "description": "Square as get_site_design's plan numbers them (a room or site alone: its free square nearest its middle)" },
        "y": { "type": "integer", "minimum": 0, "maximum": 4095 },
    }, "required": ["at"] })
}

pub fn list() -> Vec<Value> {
    vec![
        json!({ "name": "create_link", "title": "Link two places", "description": "Join a square of one place below ground to another's: a site underground (a level or room of u:/k:), a sewer section (w:) or a building's cellar (b:). Each end shows on its map as a way across (way: stairs, ladder, passage, trapdoor, rope, portal; default stairs) naming the far end; furniture gives way. Both ends must be floor, off the stairs, ways and other links. Returns its id (j:...) and both ends.", "inputSchema": schema(json!({
            "a": end_schema("One end"),
            "b": end_schema("The other end"),
            "way": { "type": "string", "enum": WAYS },
            "name": { "type": "string", "description": "Its own name (shown with the far end's)" },
        }), &["a", "b"]) }),
        json!({ "name": "list_links", "title": "Links", "description": "Links put down by hand: id, way, name and both ends (level, site, square, name). of: those with an end in a site or level; near: those with an end below a place (its sewer section, or a site or building within 100 ft); check: each with the problems that keep an end from showing (empty: both show).", "inputSchema": schema(json!({
            "of": { "type": "string", "description": "A site (u:, k:, w:, b:) or one of its levels (l:)" },
            "near": crate::args::point_schema("A place"),
            "check": { "type": "boolean", "default": false },
            "limit": crate::tools::limit_prop(200, 2000),
        }), &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "delete_link", "title": "Take links away", "description": "Take links put down by hand away, by id or ids (j:..., all or none): both ends go.", "inputSchema": schema(crate::args::ids_props("link id"), &[]) }),
    ]
}

/// Run a link tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "create_link" => create(app, a).await.map(text),
        "list_links" => list_links(app, a).await.map(text),
        "delete_link" => delete(app, a).await.map(text),
        _ => return None,
    })
}

/// End `field` (`{at, x?, y?}`) as a level and square that can take a link.
fn resolve(world: &worldgen::world::World, t0: &worldgen::t0::T0, field: &str, v: &Value) -> Result<End, String> {
    let at = v["at"].as_str().ok_or_else(|| format!("{field}.at: required"))?;
    let (site, li, room) = match worldgen::agent::split_room_id(at) {
        Some((site, li, room)) => (site.to_string(), Some(li), room),
        None => (at.to_string(), None, None),
    };
    if !matches!(site.as_bytes(), [b'u' | b'k' | b'w' | b'b', b':', ..]) {
        return Err(format!("{field}.at: {at} is not below ground (a site underground u:/k:, a sewer section w:, a building's cellar b:, or one of their levels or rooms)"));
    }
    let it = worldgen::interior::generate_id(world, t0, &site).ok_or_else(|| format!("{field}.at: {site} has no inside"))?;
    let li_given = li;
    let li = match li {
        Some(li) => li,
        None if site.starts_with("b:") => it.levels.iter().position(|l| l.z == -1).ok_or_else(|| format!("{field}.at: {site} has no cellar"))?,
        None => it.entry_level,
    };
    let lv = it.levels.get(li).ok_or_else(|| format!("{field}.at: {site} has no level {li} (levels 0-{}, from the bottom)", it.levels.len().saturating_sub(1)))?;
    let (x, y) = match (v["x"].as_u64(), v["y"].as_u64(), room) {
        (Some(x), Some(y), _) => (x as u16, y as u16),
        (None, None, Some(ri)) => {
            let r = lv.rooms.get(ri).ok_or_else(|| format!("{field}.at: level {li} of {site} has no room {ri}"))?;
            let d = |k: usize| {
                let (dx, dy) = ((k % it.nx) as f32 + 0.5 - r.center[0], (k / it.nx) as f32 + 0.5 - r.center[1]);
                dx * dx + dy * dy
            };
            let k = (0..it.nx * it.ny)
                .filter(|&k| lv.cells[k] == ri as i16 && links::spot_problem(&it, &site, li, (k % it.nx) as u16, (k / it.nx) as u16).is_none())
                .min_by(|&a, &b| d(a).total_cmp(&d(b)).then(a.cmp(&b)))
                .ok_or_else(|| format!("{field}.at: room {ri} has no free square"))?;
            ((k % it.nx) as u16, (k / it.nx) as u16)
        }
        // A site alone (no level, no square): the free square nearest the middle of its top level
        // (a cellar's, for a building), the lowest index among equals.
        (None, None, None) if li_given.is_none() => {
            let c = [it.nx as f32 / 2.0, it.ny as f32 / 2.0];
            let d = |k: usize| {
                let (dx, dy) = ((k % it.nx) as f32 + 0.5 - c[0], (k / it.nx) as f32 + 0.5 - c[1]);
                dx * dx + dy * dy
            };
            let k = (0..it.nx * it.ny)
                .filter(|&k| links::spot_problem(&it, &site, li, (k % it.nx) as u16, (k / it.nx) as u16).is_none())
                .min_by(|&a, &b| d(a).total_cmp(&d(b)).then(a.cmp(&b)))
                .ok_or_else(|| format!("{field}.at: {site} level {li} has no free square"))?;
            ((k % it.nx) as u16, (k / it.nx) as u16)
        }
        _ => return Err(format!("{field}: give x and y (or a room alone)")),
    };
    if let Some(p) = links::spot_problem(&it, &site, li, x, y) {
        return Err(format!("{field}: {p}"));
    }
    Ok(End { level: links::level_id(&site, li), x, y })
}

async fn create(app: &Shared, a: &Value) -> Result<Value, String> {
    let (ea, eb) = (a["a"].clone(), a["b"].clone());
    let (a_end, b_end) = app
        .worker
        .with(move |ex| {
            let a = resolve(&ex.world, &ex.t0, "a", &ea)?;
            let b = resolve(&ex.world, &ex.t0, "b", &eb)?;
            if a == b {
                return Err("a and b are the same square".to_string());
            }
            Ok((a, b))
        })
        .await?;
    let link = Link { a: a_end, b: b_end, way: a["way"].as_str().unwrap_or(WAYS[0]).to_string(), name: a["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string) };
    let id = new_id("j");
    let (id2, link2) = (id.clone(), link.clone());
    app.edit("agent", 0, move |e| {
        // (Another link put on either square meanwhile.)
        let taken = |x: &End| e.links.values().any(|l| l.a == *x || l.b == *x);
        if taken(&link2.a) || taken(&link2.b) {
            return Err("another link is on that square".into());
        }
        e.links.insert(id2.clone(), link2);
        Ok(json!({ "tool": "create_link", "id": id2 }))
    })
    .await?;
    app.worker.with(move |ex| Ok(links::link_json(&ex.world, &ex.t0, &id, &link))).await
}

async fn list_links(app: &Shared, a: &Value) -> Result<Value, String> {
    let near = match &a["near"] {
        Value::Null => None,
        v => Some(crate::args::point(app, "near", v).await?),
    };
    let of = a["of"].as_str().map(str::to_string);
    let check = a["check"].as_bool().unwrap_or(false);
    let limit = crate::tools::limit_of(a, 200);
    let items = app
        .worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let edits = &w.file.edits;
            let mut rows: Vec<(String, Value)> = match (&near, &of) {
                (Some(p), _) => worldgen::agent::links_near(w, t0, *p).into_iter().map(|v| (v["id"].as_str().unwrap_or("").to_string(), v)).collect(),
                (None, Some(id)) => links::of(edits, id).map(|(k, l)| (k.clone(), links::link_json(w, t0, k, l))).collect(),
                (None, None) => edits.links.iter().map(|(k, l)| (k.clone(), links::link_json(w, t0, k, l))).collect(),
            };
            if let (Some(_), Some(id)) = (&near, &of) {
                rows.retain(|(k, _)| links::of(edits, id).any(|(j, _)| j == k));
            }
            if check {
                for (k, v) in rows.iter_mut().take(limit) {
                    if let Some(l) = edits.links.get(k) {
                        v["problems"] = json!(links::problems(w, t0, k, l));
                    }
                }
            }
            Ok(rows.into_iter().map(|r| r.1).collect::<Vec<_>>())
        })
        .await?;
    Ok(listed(json!(items), limit, json!({})))
}

async fn delete(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::args::ids(a)?;
    let single = a["ids"].is_null();
    app.edit("agent", 0, move |e| {
        for (i, id) in ids.iter().enumerate() {
            if !e.links.contains_key(id) {
                return Err(format!("{}: no link {id}", crate::args::item(single, i)));
            }
        }
        for id in &ids {
            e.links.remove(id);
        }
        Ok(json!({ "tool": "delete_link", "ids": ids }))
    })
    .await
}
