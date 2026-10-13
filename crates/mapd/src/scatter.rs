//! MCP tools for battlemap objects put down and taken away by hand (`Edits::objects`,
//! `Edits::cleared`) and uploaded sprites (`Edits::sprites`, pictures in the asset store).

use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use worldgen::battlemap::{CATALOG, KindInfo};
use worldgen::world::{Clear, ObjKind, Placed, SpriteMeta};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{schema, text};

const MAX_BATCH: usize = 500;

fn sprite_props() -> Value {
    json!({
        "name": { "type": "string" },
        "size": { "type": "number", "description": "Squares across at scale 1 (default 1)" },
        "cover": { "type": "integer", "minimum": 0, "maximum": 3, "description": "0 none, 1 half, 2 three-quarters, 3 full" },
        "blocks_move": { "type": "boolean" },
        "blocks_sight": { "type": "boolean" },
        "difficult": { "type": "boolean", "description": "Difficult terrain over its spread" },
        "height_ft": { "type": "number" },
    })
}

pub fn list() -> Vec<Value> {
    let kind = json!({ "description": "A built-in kind (its id or name, e.g. 12 or \"boulder\"), or an uploaded sprite (\"s:<asset>\" or its name)" });
    let area = json!({ "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "radius_ft": { "type": "number" }, "kinds": { "type": "array", "description": "Only these kinds (ids or names)" } }, "required": ["x_ft", "y_ft", "radius_ft"] });
    let mut upload = sprite_props();
    if let Some(m) = upload.as_object_mut() {
        m.insert("data_base64".into(), json!({ "type": "string", "description": "The picture (png, jpeg, webp, gif or svg), base64; a top-down view, transparent round it" }));
        m.insert("path".into(), json!({ "type": "string", "description": "Or a picture file on this machine" }));
        m.insert("asset".into(), json!({ "type": "string", "description": "Or an asset already uploaded (to change its name or rules)" }));
    }
    vec![
        json!({ "name": "list_sprites", "title": "Object kinds", "description": "What can be put on a battlemap: the built-in kinds (trees, rocks, props, hazards) and uploaded sprites, each with its id and tactical rules; uploaded ones with how many are placed.", "inputSchema": schema(json!({}), &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "upload_sprite", "title": "Upload a sprite", "description": "Add a picture as an object kind for battlemaps (64 px per 5-ft square is plenty), with its name, size and tactical rules. Give data_base64 or path; or asset to change an uploaded one's rules. Returns its kind id (s:<asset>).", "inputSchema": schema(upload, &[]) }),
        json!({ "name": "place_objects", "title": "Put objects down", "description": "Put objects on the battlemap at world positions (ft): built-in kinds or uploaded sprites, up to 500 at once. They block, cover and hinder by their kind's rules in play. Give an id (o:...) to move, turn or change one put down before: only the fields given change. Returns their ids.", "inputSchema": schema(json!({ "objects": { "type": "array", "items": { "type": "object", "properties": { "id": { "type": "string", "description": "A placed object (o:...) to change instead of a new one" }, "kind": kind, "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "rot_deg": { "type": "number", "description": "Turned clockwise: drawn turned for props that can turn (not trees, plants or pools) and for uploaded sprites" }, "scale": { "type": "number", "default": 1 }, "variant": { "type": "integer", "description": "0-7 (built-in kinds' look; default by chance)" } } } } }), &["objects"]) }),
        json!({ "name": "remove_objects", "title": "Take objects away", "description": "Take objects off the battlemap: ids of placed ones (o:...); generated ones by kind and place (as get_battlemap lists them: kind, x_ft, y_ft); or everything in a circle (placed and generated, optionally of some kinds). Returns the ids of the clears (restore_objects undoes them).", "inputSchema": schema(json!({ "ids": { "type": "array", "items": { "type": "string" } }, "at": { "type": "array", "items": { "type": "object", "properties": { "kind": kind, "x_ft": { "type": "number" }, "y_ft": { "type": "number" } }, "required": ["kind", "x_ft", "y_ft"] } }, "area": area }), &[]) }),
        json!({ "name": "restore_objects", "title": "Bring objects back", "description": "Bring back generated objects that were taken away: clear ids (x:...), or every clear touching a circle.", "inputSchema": schema(json!({ "ids": { "type": "array", "items": { "type": "string" } }, "area": area }), &[]) }),
    ]
}

fn rules(i: &KindInfo) -> Value {
    let cover = ["none", "half", "three-quarters", "full"][i.cover.min(3) as usize];
    json!({
        "cover": cover,
        "blocks_movement": i.blocks_move,
        "blocks_sight": i.blocks_sight,
        "difficult": i.difficult,
        "height_ft": i.height_ft,
        "size_squares": i.radius * 2.0,
        "hazard": i.hazard.map(|h| h.effect),
    })
}

/// A built-in kind by id or name.
fn builtin(v: &Value) -> Option<u16> {
    match v {
        Value::Number(n) => n.as_u64().filter(|&k| k >= 1 && k as usize <= CATALOG.len()).map(|k| k as u16),
        Value::String(s) => {
            let s = s.trim().to_lowercase();
            CATALOG.iter().find(|i| i.name == s || i.name.replace(' ', "_") == s).map(|i| i.id).or_else(|| s.parse().ok().filter(|&k: &u16| k >= 1 && k as usize <= CATALOG.len()))
        }
        _ => None,
    }
}

/// A kind (built-in, or an uploaded sprite by `s:<asset>` or name).
fn kind_of(v: &Value, sprites: &std::collections::BTreeMap<String, SpriteMeta>) -> Result<ObjKind, String> {
    if let Some(k) = builtin(v) {
        return Ok(ObjKind::Builtin(k));
    }
    let s = v.as_str().map(str::trim).unwrap_or("");
    if let Some(id) = s.strip_prefix("s:") {
        return sprites.contains_key(id).then(|| ObjKind::Sprite(s.to_string())).ok_or_else(|| format!("no uploaded sprite {s} (upload_sprite first)"));
    }
    match sprites.iter().find(|(_, m)| m.name.eq_ignore_ascii_case(s)) {
        Some((id, _)) => Ok(ObjKind::Sprite(format!("s:{id}"))),
        None => Err(format!("no object kind '{s}' (list_sprites lists them)")),
    }
}

fn num(v: &Value, k: &str) -> Result<f64, String> {
    v[k].as_f64().filter(|x| x.is_finite()).ok_or_else(|| format!("missing '{k}'"))
}

fn sprite_fields(m: &mut SpriteMeta, a: &Value) -> Result<(), String> {
    if let Some(s) = a["name"].as_str() {
        m.name = s.trim().to_string();
    }
    if let Some(x) = a["size"].as_f64() {
        if !(0.2..=40.0).contains(&x) {
            return Err("size: 0.2 to 40 squares".into());
        }
        m.size = x as f32;
    }
    if let Some(c) = a["cover"].as_u64() {
        m.cover = c.min(3) as u8;
    }
    for (k, f) in [("blocks_move", &mut m.blocks_move), ("blocks_sight", &mut m.blocks_sight), ("difficult", &mut m.difficult)] {
        if let Some(b) = a[k].as_bool() {
            *f = b;
        }
    }
    if let Some(h) = a["height_ft"].as_f64() {
        m.height_ft = h.clamp(0.0, 500.0) as f32;
    }
    Ok(())
}

/// The asset id the app gives a picture: the first 16 bytes of its SHA-256, in hex.
fn asset_id(bytes: &[u8]) -> String {
    Sha256::digest(bytes)[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// Run a battlemap object tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "list_sprites" => list_sprites(app).map(text),
        "upload_sprite" => upload(app, a).await.map(text),
        "place_objects" => place(app, a).await.map(text),
        "remove_objects" => remove(app, a).await.map(text),
        "restore_objects" => restore(app, a).await.map(text),
        _ => return None,
    })
}

fn list_sprites(app: &Shared) -> Result<Value, String> {
    let e = app.with_edits(Clone::clone).unwrap_or_default();
    let built: Vec<Value> = CATALOG.iter().map(|i| json!({ "kind": i.id, "name": i.name, "rules": rules(i) })).collect();
    let up: Vec<Value> = e
        .sprites
        .iter()
        .map(|(id, m)| {
            let kind = format!("s:{id}");
            let placed = e.objects.values().filter(|p| matches!(&p.kind, ObjKind::Sprite(s) if *s == kind)).count();
            let i = KindInfo { id: 0, name: "", radius: m.size / 2.0, blocks_move: m.blocks_move, blocks_sight: m.blocks_sight, cover: m.cover, difficult: m.difficult, height_ft: m.height_ft, hazard: None, feature: true };
            json!({ "kind": kind, "name": m.name, "picture": format!("/assets/{id}"), "rules": rules(&i), "placed": placed })
        })
        .collect();
    Ok(json!({ "built_in": built, "uploaded": up }))
}

async fn upload(app: &Shared, a: &Value) -> Result<Value, String> {
    let bytes = if let Some(d) = a["data_base64"].as_str() {
        let d = d.trim();
        let d = d.split_once(";base64,").map(|(_, b)| b).unwrap_or(d);
        Some(base64::engine::general_purpose::STANDARD.decode(d.as_bytes()).map_err(|e| format!("data_base64: {e}"))?)
    } else if let Some(p) = a["path"].as_str() {
        Some(std::fs::read(p).map_err(|e| format!("{p}: {e}"))?)
    } else {
        None
    };
    let id = match bytes {
        Some(b) => {
            if b.len() > 16 << 20 {
                return Err("a sprite is at most 16 MB".into());
            }
            if crate::image_type(&b).is_none() {
                return Err("a sprite is a picture: png, jpeg, webp, gif or svg".into());
            }
            let id = asset_id(&b);
            if !app.store.has_asset(&id) {
                app.store.put_asset(&id, &b)?;
            }
            id
        }
        None => {
            let id = a["asset"].as_str().map(|s| s.trim().trim_start_matches("s:").to_string()).ok_or("give data_base64, path or asset")?;
            if !app.store.has_asset(&id) {
                return Err(format!("no asset {id}"));
            }
            id
        }
    };
    let a2 = a.clone();
    let id2 = id.clone();
    app.edit("agent", 0, move |e| {
        let mut m = e.sprites.get(&id2).cloned().unwrap_or_default();
        sprite_fields(&mut m, &a2)?;
        if m.name.is_empty() {
            m.name = "Custom object".into();
        }
        let name = m.name.clone();
        e.sprites.insert(id2.clone(), m);
        Ok(json!({ "tool": "upload_sprite", "id": format!("s:{id2}"), "name": name }))
    })
    .await?;
    Ok(json!({ "kind": format!("s:{id}"), "asset": id }))
}

async fn bounds(app: &Shared) -> Result<[f64; 2], String> {
    app.worker.with(|ex| Ok([ex.world.geom.map_w_ft, ex.world.geom.map_h_ft])).await
}

async fn place(app: &Shared, a: &Value) -> Result<Value, String> {
    let list = a["objects"].as_array().ok_or("missing 'objects'")?.clone();
    if list.is_empty() || list.len() > MAX_BATCH {
        return Err(format!("objects: 1 to {MAX_BATCH}"));
    }
    let [w, h] = bounds(app).await?;
    let ids = app
        .edit("agent", 0, move |e| {
            let (mut ids, mut changed) = (Vec::new(), Vec::new());
            for o in &list {
                // One put down before (`id`): only the fields given change.
                let (id, old) = match &o["id"] {
                    Value::Null => (new_id("o"), None),
                    Value::String(id) => (id.clone(), Some(e.objects.get(id).cloned().ok_or_else(|| format!("no placed object {id}"))?)),
                    v => return Err(format!("id: a placed object's id (o:...), not {v}")),
                };
                let kind = match (&o["kind"], &old) {
                    (Value::Null, Some(p)) => p.kind.clone(),
                    (Value::Null, None) => return Err("missing 'kind' (or give the id of an object put down before)".into()),
                    (k, _) => kind_of(k, &e.sprites)?,
                };
                let at = |k: &str, was: Option<f64>| if o[k].is_null() { was.ok_or_else(|| format!("missing '{k}'")) } else { num(o, k) };
                let (x, y) = (at("x_ft", old.as_ref().map(|p| p.x))?, at("y_ft", old.as_ref().map(|p| p.y))?);
                if !(0.0..w).contains(&x) || !(0.0..h).contains(&y) {
                    return Err(format!("({x}, {y}) is off the map"));
                }
                let variant = match (o["variant"].as_u64(), &old) {
                    (Some(v), _) => v.min(7) as u8,
                    (None, Some(p)) => p.variant,
                    (None, None) => (worldgen::core::hash::fnv64(id.as_bytes()) & 7) as u8,
                };
                let scale = match (o["scale"].as_f64().filter(|s| s.is_finite()), &old) {
                    (Some(s), _) => s.clamp(0.2, 5.0) as f32,
                    (None, Some(p)) => p.scale,
                    (None, None) => 1.0,
                };
                // A turn given is drawn (`turned`); none keeps the one it had.
                let (rot, turned) = match (o["rot_deg"].as_f64().filter(|r| r.is_finite()), &old) {
                    (Some(r), _) => (r.to_radians() as f32, true),
                    (None, Some(p)) => (p.rot, p.turned),
                    (None, None) => (0.0, false),
                };
                if old.is_some() {
                    changed.push(id.clone());
                }
                e.objects.insert(id.clone(), Placed { kind, x, y, rot, scale, variant, turned });
                ids.push(id);
            }
            Ok(json!({ "tool": "place_objects", "ids": ids, "changed": changed }))
        })
        .await?;
    Ok(ids)
}

/// A circle from `area`: (x, y, r, kinds).
fn area_of(v: &Value) -> Result<Option<(f64, f64, f64, Vec<u16>)>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let r = num(v, "radius_ft")?;
    if !(0.0..=640.0).contains(&r) {
        return Err("radius_ft: up to 640".into());
    }
    let mut kinds = Vec::new();
    for k in v["kinds"].as_array().into_iter().flatten() {
        kinds.push(builtin(k).ok_or_else(|| format!("no built-in kind {k}"))?);
    }
    Ok(Some((num(v, "x_ft")?, num(v, "y_ft")?, r, kinds)))
}

async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids: Vec<String> = a["ids"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
    let mut at = Vec::new();
    for o in a["at"].as_array().into_iter().flatten() {
        at.push((builtin(&o["kind"]).ok_or_else(|| format!("'at' takes built-in kinds (placed objects go by id): {}", o["kind"]))?, num(o, "x_ft")?, num(o, "y_ft")?));
    }
    let area = area_of(&a["area"])?;
    if ids.is_empty() && at.is_empty() && area.is_none() {
        return Err("give ids, at or area".into());
    }
    app.edit("agent", 0, move |e| {
        let mut removed = Vec::new();
        for id in &ids {
            e.objects.remove(id).ok_or_else(|| format!("no placed object {id}"))?;
            removed.push(id.clone());
        }
        let mut clears = Vec::new();
        for (kind, x, y) in at {
            let id = new_id("x");
            e.cleared.insert(id.clone(), Clear { x, y, kind: Some(kind), ..Default::default() });
            clears.push(id);
        }
        if let Some((x, y, r, kinds)) = area {
            let inside: Vec<String> = e
                .objects
                .iter()
                .filter(|(_, p)| (p.x - x).hypot(p.y - y) <= r && (kinds.is_empty() || matches!(p.kind, ObjKind::Builtin(k) if kinds.contains(&k))))
                .map(|(k, _)| k.clone())
                .collect();
            for k in inside {
                e.objects.remove(&k);
                removed.push(k);
            }
            let id = new_id("x");
            e.cleared.insert(id.clone(), Clear { x, y, kind: None, r: Some(r), kinds });
            clears.push(id);
        }
        Ok(json!({ "tool": "remove_objects", "removed": removed, "clears": clears }))
    })
    .await
}

async fn restore(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids: Vec<String> = a["ids"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
    let area = area_of(&a["area"])?;
    if ids.is_empty() && area.is_none() {
        return Err("give ids or area".into());
    }
    app.edit("agent", 0, move |e| {
        let mut gone = Vec::new();
        for id in &ids {
            e.cleared.remove(id).ok_or_else(|| format!("no clear {id}"))?;
            gone.push(id.clone());
        }
        if let Some((x, y, r, _)) = area {
            let touching: Vec<String> = e.cleared.iter().filter(|(_, c)| (c.x - x).hypot(c.y - y) <= r + c.r.unwrap_or(0.0)).map(|(k, _)| k.clone()).collect();
            for k in touching {
                e.cleared.remove(&k);
                gone.push(k);
            }
        }
        Ok(json!({ "tool": "restore_objects", "restored": gone }))
    })
    .await
}
