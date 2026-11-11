//! MCP tools for battlemap objects put down and taken away by hand (`Edits::objects`,
//! `Edits::cleared`) and uploaded sprites (`Edits::sprites`, pictures in the asset store).

use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use worldgen::battlemap::{self, KindInfo, catalog};
use worldgen::world::{Clear, ObjKind, Placed, SpriteMeta};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{listed, schema, text};

const MAX_BATCH: usize = 500;

fn sprite_props() -> Value {
    json!({
        "name": { "type": "string" },
        "size_squares": { "type": "number", "minimum": 0.2, "maximum": 40, "default": 1, "description": "Squares across at scale 1" },
        "cover": { "oneOf": [{ "type": "string", "enum": COVERS }, { "type": "integer", "minimum": 0, "maximum": 3 }], "description": "none, half, three-quarters, full (or 0 to 3)" },
        "blocks_movement": { "type": "boolean" },
        "blocks_sight": { "type": "boolean" },
        "difficult": { "type": "boolean" },
        "height_ft": { "type": "number", "minimum": 0, "maximum": 500, "default": 3 },
    })
}

const COVERS: [&str; 4] = ["none", "half", "three-quarters", "full"];

pub fn list() -> Vec<Value> {
    let kind = json!({ "oneOf": [{ "type": "integer", "minimum": 0 }, { "type": "string" }], "description": "A built-in kind (id or name) or sprite (s:<asset> or name)" });
    let kinds = json!({ "type": "array", "items": { "oneOf": [{ "type": "integer", "minimum": 0 }, { "type": "string" }] }, "description": "Only these kinds" });
    let mut within = crate::args::area_schema("[x0, y0, x1, y1], [[x, y], ...] or {at, radius_ft}; only a circle takes generated objects", 640.0);
    within["oneOf"][1]["properties"]["kinds"] = kinds.clone();
    let mut restore_within = crate::args::area_schema("Clears touching a box, polygon or circle", 640.0);
    restore_within["oneOf"][1]["properties"]["kinds"] = kinds.clone();
    let ids = crate::args::ids_props("id");
    let mut remove = ids.clone();
    if let Some(m) = remove.as_object_mut() {
        m.insert("points".into(), json!({ "type": "array", "minItems": 1, "maxItems": MAX_BATCH, "items": { "type": "object", "properties": { "kind": kind.clone(), "x_ft": { "type": "number" }, "y_ft": { "type": "number" } }, "required": ["kind", "x_ft", "y_ft"] }, "description": "Generated objects, as get_battlemap lists them" }));
        m.insert("within".into(), within);
        m.insert("kinds".into(), kinds);
    }
    let mut restore = ids.clone();
    if let Some(m) = restore.as_object_mut() {
        m.insert("within".into(), restore_within);
    }
    let mut upload = sprite_props();
    if let Some(m) = upload.as_object_mut() {
        m.insert("data_base64".into(), json!({ "type": "string", "description": "The picture (png, jpeg, webp, gif or svg), base64, top-down" }));
        m.insert("path".into(), json!({ "type": "string" }));
        m.insert("asset".into(), json!({ "type": "string", "description": "An uploaded asset to change" }));
    }
    vec![
        json!({ "name": "list_sprites", "title": "Object kinds", "description": "What can be put on a battlemap: built-in kinds and uploaded sprites with id, tactical rules and uploaded. Returns {items, found}; limit (default 200, max 1000) cuts it and sets truncated.", "inputSchema": schema(json!({ "limit": crate::tools::limit_prop(200, 1000) }), &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "upload_sprite", "title": "Upload a sprite", "description": "Add a picture as an object kind for battlemaps (64 px per 5-ft square is plenty), with its name, size and tactical rules. Give data_base64 or path; or asset to change an uploaded one's rules. Returns its kind id (s:<asset>).", "inputSchema": schema(upload, &[]) }),
        json!({ "name": "place_objects", "title": "Put objects down", "description": "Put objects on the battlemap at world positions (ft): built-in kinds or uploaded sprites, up to 500 at once. They block, cover and hinder by their kind's rules. With an id (o:...), changes one put down before. Returns their ids.", "inputSchema": schema(json!({ "objects": { "type": "array", "minItems": 1, "maxItems": MAX_BATCH, "items": { "type": "object", "properties": { "id": { "type": "string", "description": "A placed object (o:...) to change" }, "kind": kind, "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "rot_deg": { "type": "number", "description": "Degrees clockwise (props that can turn)" }, "scale": { "type": "number", "minimum": 0.2, "maximum": 5, "default": 1 }, "variant": { "type": "integer", "minimum": 0, "maximum": 15, "description": "Look (most kinds have four); default by chance" } } } } }), &["objects"]) }),
        json!({ "name": "remove_objects", "title": "Take objects away", "description": "Take objects off the battlemap: placed ones by id or ids (o:...), generated ones by kind and place (points), or everything inside within (kinds narrows a circle). Returns the ids of the clears (restore_objects undoes them).", "inputSchema": schema(remove, &[]) }),
        json!({ "name": "restore_objects", "title": "Bring objects back", "description": "Bring back generated objects that were taken away: clear ids (x:...), or every clear touching an area (within).", "inputSchema": schema(restore, &[]) }),
    ]
}

/// An f32 rule as a number with two decimals (no f32 noise).
fn round2(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}

fn rules(i: &KindInfo) -> Value {
    let cover = ["none", "half", "three-quarters", "full"][i.cover.min(3) as usize];
    json!({
        "cover": cover,
        "blocks_movement": i.blocks_move,
        "blocks_sight": i.blocks_sight,
        "difficult": i.difficult,
        "height_ft": round2(i.height_ft),
        "size_squares": round2(i.radius * 2.0),
        "hazard": i.hazard.map(|h| h.effect),
    })
}

/// A built-in kind by id or name.
pub(crate) fn builtin(v: &Value) -> Option<u16> {
    match v {
        Value::Number(n) => n.as_u64().filter(|&k| k <= u16::MAX as u64 && battlemap::kind_of(k as u16).is_some()).map(|k| k as u16),
        Value::String(s) => {
            let s = s.trim().to_lowercase();
            catalog().find(|i| i.name == s || i.name.replace(' ', "_") == s).map(|i| i.id).or_else(|| s.parse().ok().filter(|&k: &u16| battlemap::kind_of(k).is_some()))
        }
        _ => None,
    }
}

/// A kind (built-in, or an uploaded sprite by `s:<asset>` or name).
pub(crate) fn kind_of(v: &Value, sprites: &std::collections::BTreeMap<String, SpriteMeta>) -> Result<ObjKind, String> {
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
    if let Some(x) = a["size_squares"].as_f64() {
        m.size = x as f32;
    }
    match &a["cover"] {
        Value::String(w) => m.cover = COVERS.iter().position(|c| c == w).unwrap_or(0) as u8,
        Value::Number(n) => m.cover = n.as_u64().unwrap_or(0) as u8,
        _ => {}
    }
    for (k, f) in [("blocks_movement", &mut m.blocks_move), ("blocks_sight", &mut m.blocks_sight), ("difficult", &mut m.difficult)] {
        if let Some(b) = a[k].as_bool() {
            *f = b;
        }
    }
    if let Some(h) = a["height_ft"].as_f64() {
        m.height_ft = h as f32;
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
        "list_sprites" => list_sprites(app, crate::tools::limit_of(a, 200)).map(text),
        "upload_sprite" => upload(app, a).await.map(text),
        "place_objects" => place(app, a).await.map(text),
        "remove_objects" => remove(app, a).await.map(text),
        "restore_objects" => restore(app, a).await.map(text),
        _ => return None,
    })
}

fn list_sprites(app: &Shared, limit: usize) -> Result<Value, String> {
    let e = app.with_edits(Clone::clone).unwrap_or_default();
    let built: Vec<Value> = catalog().map(|i| json!({ "kind": i.id, "name": i.name, "uploaded": false, "rules": rules(i) })).collect();
    let up: Vec<Value> = e
        .sprites
        .iter()
        .map(|(id, m)| {
            let kind = format!("s:{id}");
            let placed = e.objects.values().filter(|p| matches!(&p.kind, ObjKind::Sprite(s) if *s == kind)).count();
            let i = KindInfo { id: 0, name: "", radius: m.size / 2.0, blocks_move: m.blocks_move, blocks_sight: m.blocks_sight, cover: m.cover, difficult: m.difficult, height_ft: m.height_ft, hazard: None, feature: true };
            json!({ "kind": kind, "name": m.name, "uploaded": true, "picture": format!("/assets/{id}"), "rules": rules(&i), "placed": placed })
        })
        .collect();
    Ok(listed(json!(built.into_iter().chain(up).collect::<Vec<_>>()), limit, json!({})))
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
    let by_bytes = a["asset"].is_null();
    let r = app
        .edit("agent", 0, move |e| {
            let old = e.sprites.get(&id2).cloned();
            let mut m = old.clone().unwrap_or_default();
            sprite_fields(&mut m, &a2)?;
            if m.name.is_empty() {
                m.name = "Custom object".into();
            }
            let name = m.name.clone();
            let mut v = json!({ "tool": "upload_sprite", "id": format!("s:{id2}"), "name": name });
            // The same picture again: one name and rules per picture, the last upload's.
            if let (true, Some(o)) = (by_bytes, &old) {
                let mut was = serde_json::Map::new();
                let (cw, ow) = (|c: u8| ["none", "half", "three-quarters", "full"][c.min(3) as usize], &o.name);
                if o.name != m.name {
                    was.insert("name".into(), json!(ow));
                }
                if o.size != m.size {
                    was.insert("size_squares".into(), json!(round2(o.size)));
                }
                if o.cover != m.cover {
                    was.insert("cover".into(), json!(cw(o.cover)));
                }
                if o.blocks_move != m.blocks_move {
                    was.insert("blocks_movement".into(), json!(o.blocks_move));
                }
                if o.blocks_sight != m.blocks_sight {
                    was.insert("blocks_sight".into(), json!(o.blocks_sight));
                }
                if o.difficult != m.difficult {
                    was.insert("difficult".into(), json!(o.difficult));
                }
                if o.height_ft != m.height_ft {
                    was.insert("height_ft".into(), json!(round2(o.height_ft)));
                }
                v["existing"] = json!(true);
                if was.is_empty() {
                    v["note"] = json!("this picture was already uploaded with these name and rules");
                } else {
                    v["replaced"] = Value::Object(was);
                    v["note"] = json!("this picture was already uploaded: a picture has one name and rules, the last upload's (replaced shows what they were)");
                }
            }
            e.sprites.insert(id2.clone(), m);
            Ok(v)
        })
        .await?;
    let mut out = json!({ "kind": format!("s:{id}"), "asset": id });
    for k in ["existing", "replaced", "note"] {
        if !r[k].is_null() {
            out[k] = r[k].clone();
        }
    }
    Ok(out)
}

async fn bounds(app: &Shared) -> Result<[f64; 2], String> {
    app.worker.with(|ex| Ok([ex.world.geom.map_w_ft, ex.world.geom.map_h_ft])).await
}

/// One object of `place_objects` onto `e`: its id, whether it changed one put down before, and whether it was
/// already there (a new one of the same kind within half a foot, with the same turn, scale and any variant given).
fn put_one(e: &mut worldgen::world::Edits, o: &Value, w: f64, h: f64) -> Result<(String, bool, bool), String> {
    // One put down before (`id`): only the fields given change.
    let (id, old) = match &o["id"] {
        Value::Null => (new_id("o"), None),
        Value::String(id) => (id.clone(), Some(e.objects.get(id).cloned().ok_or_else(|| format!("id: no placed object {id}"))?)),
        v => return Err(crate::args::bad("id", "a placed object's id (o:...)", v)),
    };
    let kind = match (&o["kind"], &old) {
        (Value::Null, Some(p)) => p.kind.clone(),
        (Value::Null, None) => return Err("kind: required (or give the id of an object put down before)".into()),
        (k, _) => kind_of(k, &e.sprites).map_err(|m| format!("kind: {m}"))?,
    };
    let at = |k: &str, was: Option<f64>| if o[k].is_null() { was.ok_or_else(|| format!("{k}: required")) } else { num(o, k) };
    let (x, y) = (at("x_ft", old.as_ref().map(|p| p.x))?, at("y_ft", old.as_ref().map(|p| p.y))?);
    if !(0.0..w).contains(&x) || !(0.0..h).contains(&y) {
        return Err(format!("x_ft/y_ft: ({x}, {y}) is off the map, which is {w} by {h} ft"));
    }
    let variant = match (o["variant"].as_u64(), &old) {
        (Some(v), _) => v as u8,
        (None, Some(p)) => p.variant,
        (None, None) => (worldgen::core::hash::fnv64(id.as_bytes()) & 15) as u8,
    };
    let scale = match (o["scale"].as_f64().filter(|s| s.is_finite()), &old) {
        (Some(s), _) => s as f32,
        (None, Some(p)) => p.scale,
        (None, None) => 1.0,
    };
    // A turn given is drawn (`turned`); none keeps the one it had.
    let (rot, turned) = match (o["rot_deg"].as_f64().filter(|r| r.is_finite()), &old) {
        (Some(r), _) => (r.to_radians() as f32, true),
        (None, Some(p)) => (p.rot, p.turned),
        (None, None) => (0.0, false),
    };
    let changed = old.is_some();
    if old.is_none()
        && let Some((k, _)) = e.objects.iter().find(|(_, p)| {
            p.kind == kind && (p.x - x).hypot(p.y - y) <= 0.5 && (p.rot - rot).abs() < 1e-3 && (p.scale - scale).abs() < 1e-3 && (o["variant"].is_null() || p.variant == variant)
        })
    {
        return Ok((k.clone(), false, true));
    }
    e.objects.insert(id.clone(), Placed { kind, x, y, rot, scale, variant, turned });
    Ok((id, changed, false))
}

async fn place(app: &Shared, a: &Value) -> Result<Value, String> {
    let list = a["objects"].as_array().ok_or("objects: required")?.clone();
    let [w, h] = bounds(app).await?;
    app.edit("agent", 0, move |e| {
        let (mut ids, mut changed, mut existing) = (Vec::new(), Vec::new(), Vec::new());
        for (i, o) in list.iter().enumerate() {
            let (id, was, there) = put_one(e, o, w, h).map_err(|m| format!("objects[{i}].{m}"))?;
            if was {
                changed.push(id.clone());
            }
            if there {
                existing.push(id.clone());
            }
            ids.push(id);
        }
        let mut v = json!({ "tool": "place_objects", "ids": ids, "changed": changed });
        if !existing.is_empty() {
            v["existing"] = json!(existing);
            v["note"] = json!(format!("{} of these were already there (same kind, place, turn and scale): their ids are returned and nothing was added for them", existing.len()));
        }
        Ok(v)
    })
    .await
}

/// The kinds of a remove: those at the top and in a circle, as built-in ids.
fn kinds_of(a: &Value, within: &Value) -> Result<Vec<u16>, String> {
    let mut kinds = Vec::new();
    for (field, list) in [("kinds", &a["kinds"]), ("within.kinds", &within["kinds"])] {
        for (i, k) in list.as_array().into_iter().flatten().enumerate() {
            kinds.push(builtin(k).ok_or_else(|| format!("{field}[{i}]: no built-in kind {k}"))?);
        }
    }
    Ok(kinds)
}

async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = if a["id"].is_null() && a["ids"].is_null() { Vec::new() } else { crate::args::ids(a)? };
    let mut points = Vec::new();
    for (i, o) in a["points"].as_array().into_iter().flatten().enumerate() {
        let kind = builtin(&o["kind"]).ok_or_else(|| format!("points[{i}].kind: built-in kinds only (placed objects go by id), got {}", o["kind"]))?;
        points.push((kind, num(o, "x_ft").map_err(|m| format!("points[{i}].{m}"))?, num(o, "y_ft").map_err(|m| format!("points[{i}].{m}"))?));
    }
    let single = a["ids"].is_null();
    let area = if a["within"].is_null() { None } else { Some(crate::args::area(app, "within", &a["within"]).await?) };
    let kinds = kinds_of(a, &a["within"])?;
    if ids.is_empty() && points.is_empty() && area.is_none() {
        return Err("give id, ids, points or within".into());
    }
    app.edit("agent", 0, move |e| {
        let mut removed = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            e.objects.get(id).ok_or_else(|| format!("{}: no placed object {id}", crate::args::item(single, i)))?;
        }
        for id in &ids {
            e.objects.remove(id);
            removed.push(id.clone());
        }
        let mut clears = Vec::new();
        let mut there = Vec::new();
        for (kind, x, y) in points {
            // Already cleared there: a clear of that kind at the point (a quarter square), or a circle over it.
            let have = e.cleared.iter().find(|(_, c)| match c.kind {
                Some(k) => k == kind && (c.x - x).hypot(c.y - y) <= 1.25,
                None => (c.x - x).hypot(c.y - y) <= c.r.unwrap_or(0.0) && (c.kinds.is_empty() || c.kinds.contains(&kind)),
            });
            if let Some((id, _)) = have {
                there.push(id.clone());
                continue;
            }
            let id = new_id("x");
            e.cleared.insert(id.clone(), Clear { x, y, kind: Some(kind), ..Default::default() });
            clears.push(id);
        }
        let mut note = None;
        if let Some(area) = area {
            let inside: Vec<String> = e
                .objects
                .iter()
                .filter(|(_, p)| area.contains([p.x, p.y]) && (kinds.is_empty() || matches!(p.kind, ObjKind::Builtin(k) if kinds.contains(&k))))
                .map(|(k, _)| k.clone())
                .collect();
            for k in inside {
                e.objects.remove(&k);
                removed.push(k);
            }
            if let crate::args::Area::Circle([x, y], r) = area {
                let id = new_id("x");
                e.cleared.insert(id.clone(), Clear { x, y, kind: None, r: Some(r), kinds });
                clears.push(id);
            } else {
                note = Some("a box or polygon takes away the objects put down by hand; generated ones go by a circle or by points");
            }
        }
        let mut v = json!({ "tool": "remove_objects", "removed": removed, "clears": clears });
        if let Some(n) = note {
            v["note"] = json!(n);
        }
        if !there.is_empty() {
            v["existing"] = json!(there);
            let n = format!("{} of these points {} already cleared (their clear ids are in existing): nothing was added for them", there.len(), if there.len() == 1 { "was" } else { "were" });
            v["note"] = json!(match note {
                Some(o) => format!("{o}; {n}"),
                None => n,
            });
        }
        Ok(v)
    })
    .await
}

/// How far `p` is from the nearest side of the closed polygon `q`.
fn edge_dist(q: &[[f64; 2]], p: [f64; 2]) -> f64 {
    (0..q.len())
        .map(|i| {
            let (a, b) = (q[i], q[(i + 1) % q.len()]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dy * dy;
            let t = if l2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
            (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
        })
        .fold(f64::MAX, f64::min)
}

async fn restore(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = if a["id"].is_null() && a["ids"].is_null() { Vec::new() } else { crate::args::ids(a)? };
    let single = a["ids"].is_null();
    let area = if a["within"].is_null() { None } else { Some(crate::args::area(app, "within", &a["within"]).await?) };
    if ids.is_empty() && area.is_none() {
        return Err("give id, ids or within".into());
    }
    app.edit("agent", 0, move |e| {
        let mut gone = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            e.cleared.get(id).ok_or_else(|| format!("{}: no clear {id}", crate::args::item(single, i)))?;
        }
        for id in &ids {
            e.cleared.remove(id);
            gone.push(id.clone());
        }
        if let Some(area) = area {
            let touching: Vec<String> = e
                .cleared
                .iter()
                .filter(|(_, c)| match &area {
                    crate::args::Area::Circle(m, r) => (c.x - m[0]).hypot(c.y - m[1]) <= r + c.r.unwrap_or(0.0),
                    other => other.contains([c.x, c.y]) || edge_dist(&other.outline(), [c.x, c.y]) <= c.r.unwrap_or(0.0),
                })
                .map(|(k, _)| k.clone())
                .collect();
            for k in touching {
                e.cleared.remove(&k);
                gone.push(k);
            }
        }
        Ok(json!({ "tool": "restore_objects", "restored": gone }))
    })
    .await
}
