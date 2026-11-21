//! MCP tools for roads drawn by hand (`Edits::paths`, `rw:<id>`): footpaths, tracks, gravel roads,
//! paved roads and cobbled streets drawn on the ground along a line (`worldgen::paths`).

use serde_json::{Value, json};
use worldgen::paths::{self, Path, PathKind, PathLine};
use worldgen::world::{CrossingKind, Edits};

use crate::Shared;
use crate::args::round1;
use crate::notebook::new_id;
use crate::tools::{listed, schema, text};

pub fn list() -> Vec<Value> {
    let pt = crate::args::point_schema("[x_ft, y_ft] or {x_ft, y_ft}");
    let kinds: Vec<&str> = PathKind::ALL.iter().map(|k| k.name()).collect();
    let props = json!({
        "id": { "type": "string", "description": "A road to change (rw:...); what is not given stays" },
        "pts": { "type": "array", "minItems": 2, "maxItems": paths::MAX_PATH_PTS, "items": pt, "description": "Its line (world ft): a smooth curve through them" },
        "type": { "type": "string", "enum": kinds, "description": "Width 4, 10, 14, 18, 20 ft; default track" },
        "width_ft": { "type": "number", "minimum": paths::MIN_WIDTH_FT, "maximum": paths::MAX_WIDTH_FT },
        "name": { "type": "string" },
    });
    vec![
        json!({ "name": "place_path", "title": "Draw a road", "description": "A road on the ground along a line: on the map and battlemap, walked like a road, a bridge over water; scatter and the world's own town buildings keep off it. The network and towns stay. With id, changes one. Returns its rw: id and crossings.", "inputSchema": schema(props, &[]) }),
        json!({ "name": "remove_roads", "title": "Take roads away", "description": "Roads off the map by id (restore: back): stretches (rd:, mt:), town streets (tr:), park paths (pp:), drawn roads (rw:), added streets (ts:); ids from inspect. Ground shows; buildings, gates, traffic stay. A stretch takes its towns' approaches (also).", "inputSchema": schema(json!({ "ids": { "type": "array", "items": { "type": "string" }, "minItems": 1 }, "restore": { "type": "boolean" } }), &["ids"]) }),
        json!({ "name": "list_paths", "title": "Roads drawn by hand", "description": "Roads drawn by hand with their crossings.", "inputSchema": schema(json!({ "limit": { "type": "integer", "minimum": 1 } }), &[]) }),
    ]
}

/// Run a road tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "place_path" => set(app, a, !a["id"].is_null()).await.map(text),
        "remove_roads" => remove_roads(app, a).await.map(text),
        "list_paths" => list_paths(app, crate::tools::limit_of(a, 100)).await.map(text),
        _ => return None,
    })
}

/// A road's entry in replies and lists (`crossings`: where it runs over water, if worked out).
fn path_json(id: &str, p: &Path, crossings: Option<Vec<Value>>) -> Value {
    let (len, mid) = paths::length_and_mid(p);
    let line = p.line();
    let b = line.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
    let mut v = json!({
        "id": id,
        "type": p.kind.name(),
        "what": p.kind.label(),
        "width_ft": round1(p.width_ft()),
        "name": p.name.clone().unwrap_or_default(),
        "length_ft": len.round(),
        "middle": { "x_ft": mid[0].round(), "y_ft": mid[1].round() },
        "bbox": [b[0].round(), b[1].round(), b[2].round(), b[3].round()],
        "pts": p.pts,
    });
    if let Some(c) = crossings {
        v["crossings"] = json!(c);
    }
    v
}

fn crossings_json(ex: &worldgen::pipeline::Executor, p: &Path) -> Vec<Value> {
    paths::crossings(&ex.world, &ex.t0, p)
        .iter()
        .map(|c| {
            let len = ((c.b[0] - c.a[0]).powi(2) + (c.b[1] - c.a[1]).powi(2)).sqrt();
            json!({ "kind": if c.kind == CrossingKind::Bridge { "bridge" } else { "ford" }, "material": if c.kind == CrossingKind::Bridge { c.material.name() } else { "" }, "a": [c.a[0].round(), c.a[1].round()], "b": [c.b[0].round(), c.b[1].round()], "length_ft": len.round() })
        })
        .collect()
}

async fn list_paths(app: &Shared, limit: usize) -> Result<Value, String> {
    let items = app.worker.with(|ex| Ok(ex.world.file.edits.paths.iter().map(|(id, p)| path_json(id, p, Some(crossings_json(ex, p)))).collect::<Vec<_>>())).await?;
    Ok(listed(json!(items), limit, json!({})))
}

async fn set(app: &Shared, a: &Value, update: bool) -> Result<Value, String> {
    let id: Option<String> = if update { Some(a["id"].as_str().ok_or("id: the road to change (rw:...)")?.to_string()) } else { None };
    let kind = match &a["type"] {
        Value::Null => None,
        Value::String(s) => Some(PathKind::parse(s).ok_or("type: footpath, track, gravel, paved or cobbled")?),
        _ => return Err("type: footpath, track, gravel, paved or cobbled".into()),
    };
    let width = match &a["width_ft"] {
        Value::Null => None,
        v => Some(v.as_f64().filter(|w| w.is_finite() && (paths::MIN_WIDTH_FT..=paths::MAX_WIDTH_FT).contains(w)).ok_or(format!("width_ft: {} to {} ft", paths::MIN_WIDTH_FT, paths::MAX_WIDTH_FT))?),
    };
    let pts: Option<Vec<[f64; 2]>> = match a["pts"].as_array() {
        Some(o) => {
            let mut pts = Vec::new();
            for (i, p) in o.iter().enumerate() {
                pts.push(crate::args::point(app, &format!("pts[{i}]"), p).await?);
            }
            Some(paths::rounded(&pts))
        }
        None if a["pts"].is_null() => None,
        None => return Err("pts: a line, 2 or more points".into()),
    };
    if pts.is_none() && id.is_none() {
        return Err("pts: the road's line, 2 or more points".into());
    }
    let name = match &a["name"] {
        Value::Null => None,
        Value::String(n) => Some(n.trim().to_string()),
        _ => return Err("name: text".into()),
    };
    let (mw, mh) = app.worker.with(|ex| Ok((ex.world.geom.map_w_ft, ex.world.geom.map_h_ft))).await?;
    if pts.as_ref().is_some_and(|pts| pts.iter().any(|p| !(0.0..mw).contains(&p[0]) || !(0.0..mh).contains(&p[1]))) {
        return Err("the line is off the map".into());
    }
    let made = app
        .edit("agent", 0, move |e| {
            let old = match &id {
                Some(id) => Some(e.paths.get(id).cloned().ok_or_else(|| format!("no road {id}"))?),
                None => None,
            };
            let kind = kind.or(old.as_ref().map(|o| o.kind)).unwrap_or_default();
            // (A width that was its old type's own follows a new type.)
            let width = match (width, &old) {
                (Some(w), _) => Some(w),
                (None, Some(o)) if o.kind == kind => o.width,
                (None, Some(o)) => o.width.filter(|w| (*w - o.kind.width_ft()).abs() > 0.05),
                (None, None) => None,
            }
            .filter(|w| (*w - kind.width_ft()).abs() > 0.05);
            let name = match name.as_deref() {
                Some("") => None,
                Some(n) => Some(n.to_string()),
                None => old.as_ref().and_then(|o| o.name.clone()),
            };
            let p = Path { pts: pts.clone().unwrap_or_else(|| old.as_ref().map(|o| o.pts.clone()).unwrap_or_default()), kind, width, name };
            if let Some(err) = p.problem() {
                return Err(err);
            }
            if id.is_none() {
                // (The same line again, a script run twice: that road, nothing added.)
                if let Some((eid, ep)) = e.paths.iter().find(|(_, q)| q.pts.len() == p.pts.len() && q.pts.iter().zip(&p.pts).all(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= 1.0)) {
                    if ep.kind != p.kind || (ep.width_ft() - p.width_ft()).abs() > 0.05 {
                        return Err(format!("{eid} runs along this line: give its id to change it"));
                    }
                    return Ok(json!({ "tool": "place_path", "id": eid, "kind": ep.kind.name(), "path": ep, "added": false }));
                }
                if e.paths.len() >= paths::MAX_PATHS {
                    return Err(format!("a world holds at most {} roads drawn by hand", paths::MAX_PATHS));
                }
            }
            let key = id.clone().unwrap_or_else(|| new_id("rw"));
            let v = json!({ "tool": "place_path", "id": key, "kind": p.kind.name(), "changed": id.is_some(), "path": p, "added": true });
            e.paths.insert(key.clone(), p);
            Ok(v)
        })
        .await?;
    let key = made["id"].as_str().unwrap_or_default().to_string();
    let p: Path = serde_json::from_value(made["path"].clone()).map_err(|e| e.to_string())?;
    let added = made["added"].as_bool().unwrap_or(true);
    let crossings = {
        let p = p.clone();
        app.worker.with(move |ex| Ok(crossings_json(ex, &p))).await?
    };
    let mut v = path_json(&key, &p, Some(crossings));
    v["tool"] = json!("place_path");
    v["changed"] = json!(update);
    if !update && !added {
        v["existing"] = json!(true);
        v["note"] = json!("a road along this line was already there: nothing was added");
    }
    Ok(v)
}

/// (Roadmap 6 R5) Roads, streets and paths taken off the map or brought back (`roadcut::cut`),
/// worked out and applied under one hold of the gate, as one edit (the app's sync sees it).
async fn remove_roads(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::args::ids(a)?;
    let restore = a["restore"].as_bool().unwrap_or(false);
    if ids.len() > worldgen::roadcut::MAX_IDS {
        return Err(format!("at most {} ids a call (got {}): nothing was changed", worldgen::roadcut::MAX_IDS, ids.len()));
    }
    let _hold = if crate::batch::active() { None } else { Some(app.gate.lock().await) };
    let out = app.worker.with(move |ex| Ok(worldgen::roadcut::cut(&ex.world, &ex.t0, &ids, restore, false))).await?;
    let refused: Vec<Value> = out.refused.iter().map(|r| json!({ "id": r.id, "why": r.why })).collect();
    if out.done.is_empty() {
        let why: Vec<String> = out.refused.iter().map(|r| format!("{}: {}", r.id, r.why)).collect();
        return Err(format!("nothing {}: {}", if restore { "restored" } else { "removed" }, why.join("; ")));
    }
    let reply = json!({ "tool": "remove_roads", (if restore { "restored" } else { "removed" }): out.done, "also": out.also, "refused": refused });
    let change = reply.clone();
    let apply = move |e: &mut Edits| {
        out.apply(e);
        Ok(change)
    };
    if _hold.is_some() { app.edit_now("agent", 0, apply).await } else { app.edit("agent", 0, apply).await }
}
