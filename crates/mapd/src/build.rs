//! MCP tools for buildings drawn by hand (`Created` sites of kind `building`): each is a site of
//! its own (`b:<layout>:0`), with an interior, a roof and a battlemap footprint like the world's.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::world::{BUILDING_HOMES, Created, MAX_FLOORS, ROOFS, STRUCTURES, TINTS};

use crate::Shared;
use crate::tools::{found, schema, text};

const SQUARE_FT: f64 = 5.0;

fn props() -> Value {
    json!({
        "poly": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "description": "The footprint: corners [x_ft, y_ft] in order (3-64; any simple shape, L-shapes too)" },
        "rect": { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "width_ft": { "type": "number" }, "depth_ft": { "type": "number" }, "angle_deg": { "type": "number", "default": 0 } }, "required": ["x_ft", "y_ft", "width_ft", "depth_ft"], "description": "Or a rectangle about its middle (width along angle_deg, clockwise from east)" },
        "circle": { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "radius_ft": { "type": "number" }, "sides": { "type": "integer", "minimum": 8, "maximum": 32, "default": 16 } }, "required": ["x_ft", "y_ft", "radius_ft"], "description": "Or a round tower" },
        "snap": { "type": "boolean", "default": true, "description": "Snap poly and unturned rect corners (and a circle's middle) to the 5-ft grid" },
        "name": { "type": "string", "description": "Else named for its trade (or what it is)" },
        "func": { "type": "string", "description": format!("What it is: a business (a catalog key as list_names shows businesses: inn, tavern, blacksmith, temple, castle, observatory...) or a home: {}. Default house; a round wizard_tower or observatory is a tower inside", BUILDING_HOMES.join(", ")) },
        "floors": { "type": "integer", "minimum": 1, "maximum": MAX_FLOORS },
        "roof": { "type": "string", "enum": ([ROOFS.as_slice(), &["auto"]].concat()), "description": "hip (pitched), battlements (a walkable roof), cone; auto: as its kind has it (battlements on castles and towers)" },
        "tint": { "type": "string", "enum": ([TINTS.as_slice(), &["auto"]].concat()), "description": "Roof colour (auto: picked)" },
        "structure": { "type": "string", "enum": STRUCTURES, "description": "roofed (default) or ruin (broken walls, no roof, no interior)" },
    })
}

pub fn list() -> Vec<Value> {
    let mut update = props();
    if let Some(m) = update.as_object_mut() {
        m.insert("id".into(), json!({ "type": "string", "description": "A building drawn by hand (its created id c:<n>) or one of the world's own (b:<layout>:<id>)" }));
    }
    let mut create = props();
    if let Some(m) = create.as_object_mut() {
        m.insert("near".into(), json!({ "description": "Instead of a footprint: an id or {x_ft, y_ft}; the nearest clear spot for width_ft x depth_ft, its front on a street and square to it, off squares" }));
        m.insert("width_ft".into(), json!({ "type": "number", "description": "With near: along the street (10-400)" }));
        m.insert("depth_ft".into(), json!({ "type": "number", "description": "With near: back from the street (10-400)" }));
    }
    let mut check = props();
    if let Some(m) = check.as_object_mut() {
        m.retain(|k, _| ["poly", "rect", "circle", "snap", "func"].contains(&k.as_str()));
        m.insert("id".into(), json!({ "type": "string", "description": "The building being reshaped, if any (it does not count against itself)" }));
    }
    let poly = json!({ "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 } });
    vec![
        json!({ "name": "create_building", "title": "Build", "description": "Put a building on the map, drawn by hand: a footprint (poly, rect or circle), or near a place with width_ft and depth_ft (the nearest clear lot on a street), on dry land clear of other buildings, roads, streets, walls and squares, with what it is, storeys, roof and roof colour. It gets an interior (enter_building / get_interior on its building id), a roof and walls on the battlemap. Returns its id (c:<n>, for update_building, delete_feature, notes and NPCs) and its building id (b:<layout>:0).", "inputSchema": schema(create, &[]) }),
        json!({ "name": "update_building", "title": "Change a building", "description": "Change a building: one drawn by hand (c:<n>) or one of the world's own (b:<layout>:<id>, as get_feature and list_children give them): a new footprint (poly, rect or circle; checked like a new one), name, func, floors, roof, tint or structure. Fields left out stay; for the world's own, auto (or an empty func) goes back to as generated.", "inputSchema": schema(update, &["id"]) }),
        json!({ "name": "check_building_spot", "title": "Check a building spot", "description": "Whether a footprint (poly, rect or circle) is clear for a building, without building it: its point and the name it would get, or why not (water, river, another building, street, road, wall, square).", "inputSchema": schema(check, &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "remove_buildings", "title": "Remove buildings", "description": "Take away buildings of the world's own (not drawn by hand: delete_feature does those): by ids (b:<layout>:<id>), or every one whose middle lies inside a polygon (within, [[x_ft, y_ft], ...]) such as a ward to clear for buildings of your own. Every other building keeps its id. restore_building brings one back.", "inputSchema": schema(json!({ "ids": { "type": "array", "items": { "type": "string" } }, "within": poly }), &[]) }),
        json!({ "name": "restore_building", "title": "Restore a building", "description": "Put one of the world's own buildings back as generated: undoes remove_buildings and any update_building change (its name stays as renamed).", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "create_building" => build(app, a, None).await.map(text),
        "update_building" => match a["id"].as_str() {
            Some(id) if id.starts_with("b:") => update_generated(app, a, id.to_string()).await.map(text),
            Some(id) => build(app, a, Some(id.to_string())).await.map(text),
            None => Err("missing 'id'".into()),
        },
        "check_building_spot" => check_spot(app, a).await.map(text),
        "remove_buildings" => remove(app, a).await.map(text),
        "restore_building" => match a["id"].as_str() {
            Some(id) => restore(app, id.to_string()).await.map(text),
            None => Err("missing 'id'".into()),
        },
        _ => return None,
    })
}

/// A footprint checked without building it.
async fn check_spot(app: &Shared, a: &Value) -> Result<Value, String> {
    let poly = footprint(a)?.ok_or("give the footprint: poly, rect or circle")?;
    let func = a["func"].as_str().map(str::to_string);
    let id = a["id"].as_str().unwrap_or("c:new").to_string();
    app.worker
        .with(move |ex| {
            let skip = agent::spot_skip(&ex.world, &ex.t0, &id);
            Ok(match agent::building_spot(&ex.world, &ex.t0, &poly, func.as_deref(), &id, skip) {
                Ok((p, name)) => json!({ "ok": true, "x_ft": p[0].round(), "y_ft": p[1].round(), "name": name }),
                Err(e) => json!({ "ok": false, "reason": e }),
            })
        })
        .await
}

/// Change one of the world's own buildings (`Edits.buildings`; its name goes to `renames`).
async fn update_generated(app: &Shared, a: &Value, id: String) -> Result<Value, String> {
    let opt = |k: &str| a[k].as_str().map(|s| s.trim().to_string());
    let floors = match &a["floors"] {
        Value::Null => None,
        v => Some(v.as_u64().filter(|f| *f <= 255).ok_or("floors: a whole number")? as u8),
    };
    let change = agent::BuildingChange { func: opt("func"), floors, poly: footprint(a)?, roof: opt("roof"), tint: opt("tint"), structure: opt("structure") };
    let name = opt("name");
    let id2 = id.clone();
    let edit = app.worker.with(move |ex| agent::building_edit(&ex.world, &ex.t0, &id2, &change)).await?;
    let id3 = id.clone();
    app.edit("agent", 0, move |e| {
        match edit {
            Some(b) => e.buildings.insert(id3.clone(), b),
            None => e.buildings.remove(&id3),
        };
        match name {
            Some(n) if !n.is_empty() => {
                e.renames.insert(id3.clone(), n);
            }
            Some(_) => {
                e.renames.remove(&id3);
            }
            None => {}
        }
        Ok(json!({ "tool": "update_building", "id": id3 }))
    })
    .await?;
    let mut v = app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "changed, but it could not be read back".to_string())).await?;
    v["tool"] = json!("update_building");
    Ok(v)
}

/// Take away buildings of the world's own: by ids, or every one with its middle in `within`.
async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let mut ids: Vec<String> = a["ids"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let within: Option<Vec<[f64; 2]>> = match a["within"].as_array() {
        Some(p) => Some(p.iter().map(|q| Some([q.get(0)?.as_f64()?, q.get(1)?.as_f64()?])).collect::<Option<Vec<_>>>().filter(|p| p.len() >= 3).ok_or("within: a polygon of 3 or more [x_ft, y_ft] corners")?),
        None => None,
    };
    if ids.is_empty() && within.is_none() {
        return Err("give ids or within".into());
    }
    let edits = app
        .worker
        .with(move |ex| {
            if let Some(p) = &within {
                ids.extend(agent::generated_buildings_in(&ex.world, &ex.t0, p).into_iter().map(|(id, _)| id));
            }
            ids.sort();
            ids.dedup();
            ids.into_iter().map(|id| agent::building_removal(&ex.world, &ex.t0, &id).map(|e| (id, e))).collect::<Result<Vec<_>, String>>()
        })
        .await?;
    let removed: Vec<String> = edits.iter().map(|(id, _)| id.clone()).collect();
    if removed.is_empty() {
        return Ok(json!({ "tool": "remove_buildings", "removed": [], "note": "no building of the world's own has its middle there" }));
    }
    let reply = json!({ "tool": "remove_buildings", "count": removed.len(), "removed": removed });
    app.edit("agent", 0, move |e| {
        for (id, b) in edits {
            e.buildings.insert(id, b);
        }
        Ok(reply)
    })
    .await
}

/// One of the world's own buildings back as generated.
async fn restore(app: &Shared, id: String) -> Result<Value, String> {
    let id2 = id.clone();
    app.edit("agent", 0, move |e| match e.buildings.remove(&id2) {
        Some(_) => Ok(json!({ "tool": "restore_building", "id": id2 })),
        None => Err(format!("{id2} is as generated already")),
    })
    .await?;
    let mut v = app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "restored, but it could not be read back".to_string())).await?;
    v["tool"] = json!("restore_building");
    Ok(v)
}

/// The footprint asked for, if any (world ft).
fn footprint(a: &Value) -> Result<Option<Vec<[f64; 2]>>, String> {
    let snap = a["snap"].as_bool().unwrap_or(true);
    let grid = |v: f64| if snap { (v / SQUARE_FT).round() * SQUARE_FT } else { v };
    let num = |o: &Value, k: &str| o[k].as_f64().filter(|v| v.is_finite()).ok_or_else(|| format!("missing '{k}'"));
    if let Some(p) = a["poly"].as_array() {
        let pts: Option<Vec<[f64; 2]>> = p.iter().map(|q| Some([grid(q.get(0)?.as_f64()?), grid(q.get(1)?.as_f64()?)])).collect();
        let mut pts = pts.ok_or("poly: corners as [x_ft, y_ft]")?;
        pts.dedup();
        return Ok(Some(pts));
    }
    let r = &a["rect"];
    if r.is_object() {
        let (x, y, w, d) = (num(r, "x_ft")?, num(r, "y_ft")?, num(r, "width_ft")?, num(r, "depth_ft")?);
        let ang = r["angle_deg"].as_f64().unwrap_or(0.0).to_radians();
        let (u, v) = ([ang.cos(), ang.sin()], [-ang.sin(), ang.cos()]);
        let turned = ang.abs() > 1e-9;
        let corner = |s: f64, t: f64| {
            let p = [x + u[0] * s * w / 2.0 + v[0] * t * d / 2.0, y + u[1] * s * w / 2.0 + v[1] * t * d / 2.0];
            if turned { p } else { p.map(grid) }
        };
        return Ok(Some(vec![corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]));
    }
    let c = &a["circle"];
    if c.is_object() {
        let (x, y, rad) = (grid(num(c, "x_ft")?), grid(num(c, "y_ft")?), num(c, "radius_ft")?);
        let n = c["sides"].as_u64().unwrap_or(16).clamp(8, 32) as usize;
        let pts = (0..n).map(|k| {
            let t = std::f64::consts::TAU * k as f64 / n as f64;
            [((x + rad * t.cos()) * 100.0).round() / 100.0, ((y + rad * t.sin()) * 100.0).round() / 100.0]
        });
        return Ok(Some(pts.collect()));
    }
    Ok(None)
}

/// Create a building (`id` None) or change one.
async fn build(app: &Shared, a: &Value, id: Option<String>) -> Result<Value, String> {
    let poly = footprint(a)?;
    let mut c = match &id {
        Some(id) => app
            .with_edits(|e| e.created.iter().find(|c| &c.id == id && !c.removed && c.kind == "building").cloned())
            .ok_or("no world is open: open the map app first")?
            .ok_or_else(|| format!("{id} is not a building drawn by hand"))?,
        None => {
            let n = app.with_edits(|e| e.created.len()).unwrap_or(0);
            Created { id: format!("c:{n}"), kind: "building".into(), ..Default::default() }
        }
    };
    let creating = id.is_none();
    let poly = match poly {
        None if creating && !a["near"].is_null() => {
            // The nearest clear lot near a place.
            let near = match (&a["near"], a["near"]["x_ft"].as_f64(), a["near"]["y_ft"].as_f64()) {
                (_, Some(x), Some(y)) => Ok([x, y]),
                (Value::String(s), ..) => Err(s.clone()),
                _ => return Err("near: an id or {x_ft, y_ft}".into()),
            };
            let size = |k: &str| a[k].as_f64().ok_or("near needs width_ft and depth_ft");
            let (w, d) = (size("width_ft")?, size("depth_ft")?);
            let (func, cid) = (a["func"].as_str().map(str::to_string), c.id.clone());
            let found = app
                .worker
                .with(move |ex| {
                    let p = match near {
                        Ok(p) => p,
                        Err(id) => agent::position(&ex.world, &ex.t0, &id).ok_or_else(|| format!("no such feature: {id}"))?,
                    };
                    agent::find_spot(&ex.world, &ex.t0, p, w, d, func.as_deref(), &cid).map(|s| s.0)
                })
                .await?;
            Some(found)
        }
        p => p,
    };
    if creating && poly.is_none() {
        return Err("give the footprint (poly, rect or circle), or near with width_ft and depth_ft".into());
    }
    let reshaped = poly.is_some();
    if let Some(p) = poly {
        let n = p.len().max(1) as f64;
        (c.x, c.y) = (p.iter().map(|q| q[0]).sum::<f64>() / n, p.iter().map(|q| q[1]).sum::<f64>() / n);
        c.poly = p;
    }
    let opt = |k: &str| a[k].as_str().map(|s| s.trim().to_string());
    if let Some(f) = opt("func") {
        c.func = Some(f).filter(|f| !f.is_empty());
    }
    if let Some(r) = opt("roof") {
        c.roof = Some(r).filter(|r| r != "auto");
    }
    if let Some(t) = opt("tint") {
        c.tint = Some(t).filter(|t| t != "auto");
    }
    if let Some(s) = opt("structure") {
        c.structure = Some(s).filter(|s| s != "roofed");
    }
    match &a["floors"] {
        Value::Null => {}
        v => c.floors = Some(v.as_u64().filter(|f| *f <= 255).ok_or("floors: a whole number")? as u8),
    }
    c.check()?;
    // The same building again (a script run twice): that one, nothing added (checked again
    // when it is added).
    let at = [c.x, c.y];
    let asked = opt("name").filter(|s| !s.is_empty());
    let probe = Created { name: asked.clone().unwrap_or_default(), ..c.clone() };
    if creating && let Some(id) = app.with_edits(|e| e.existing_site(&probe, at).map(|o| o.id.clone())).flatten() {
        return found(app, id).await;
    }
    // On dry land, clear of buildings, roads and walls; its point is the footprint's middle.
    if reshaped || creating {
        let (cid, poly, func) = (c.id.clone(), c.poly.clone(), c.func.clone());
        let (p, name) = app
            .worker
            .with(move |ex| {
                let skip = agent::spot_skip(&ex.world, &ex.t0, &cid);
                agent::building_spot(&ex.world, &ex.t0, &poly, func.as_deref(), &cid, skip)
            })
            .await?;
        (c.x, c.y) = (p[0], p[1]);
        if creating {
            c.name = name;
        }
    }
    if let Some(n) = asked {
        c.name = n;
    }
    let tool = if creating { "create_building" } else { "update_building" };
    let reply = json!({ "tool": tool, "id": c.id, "kind": "building", "name": c.name, "x_ft": c.x.round(), "y_ft": c.y.round() });
    let cid = c.id.clone();
    let mut reply = reply;
    let change = app.edit("agent", 0, move |e| {
        if creating {
            if let Some(o) = e.existing_site(&c, at) {
                return Ok(json!({ "existing": o.id }));
            }
            // (Another client may have created a site meanwhile: take the next id.)
            c.id = format!("c:{}", e.created.len());
            reply["id"] = json!(c.id);
            e.created.push(c);
        } else {
            let slot = e.created.iter_mut().find(|x| x.id == c.id && !x.removed).ok_or_else(|| format!("{} was deleted meanwhile", c.id))?;
            *slot = c;
        }
        Ok(reply)
    })
    .await?;
    if let Some(id) = change["existing"].as_str() {
        return found(app, id.to_string()).await;
    }
    let id = if creating { change["id"].as_str().unwrap_or_default().to_string() } else { cid };
    app.worker
        .with(move |ex| {
            let mut v = agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "built, but it could not be read back".to_string())?;
            if let Some(li) = agent::layout_of(&ex.world, &ex.t0, &id) {
                v["building"] = json!(format!("b:{li}:0"));
            }
            Ok(v)
        })
        .await
}
