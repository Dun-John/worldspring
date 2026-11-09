//! MCP tools for buildings drawn by hand (`Created` sites of kind `building`): each is a site of
//! its own (`b:<layout>:0`), with an interior, a roof and a battlemap footprint like the world's.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::town::roof;
use worldgen::world::{BUILDING_HOMES, Created, MAX_FLOORS, ROOFS, STRUCTURES, TINTS};

use crate::Shared;
use crate::tools::{found, schema, text};

const SQUARE_FT: f64 = 5.0;

fn props() -> Value {
    json!({
        "poly": { "type": "array", "minItems": 3, "maxItems": 64, "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "description": "Corners [x_ft, y_ft] in order" },
        "rect": { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "width_ft": { "type": "number" }, "depth_ft": { "type": "number" }, "angle_deg": { "type": "number", "default": 0 } }, "required": ["x_ft", "y_ft", "width_ft", "depth_ft"], "description": "A rectangle about its middle" },
        "circle": { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "radius_ft": { "type": "number" }, "sides": { "type": "integer", "minimum": 8, "maximum": 32, "default": 16 } }, "required": ["x_ft", "y_ft", "radius_ft"], "description": "Or a round tower" },
        "snap": { "type": "boolean", "default": true, "description": "Snap corners to the 5-ft grid" },
        "name": { "type": "string" },
        "func": { "type": "string", "description": "A home or business key (docs/MCP.md); default house" },
        "floors": { "type": "integer", "minimum": 1, "maximum": MAX_FLOORS },
        "roof": { "type": "string", "enum": ([ROOFS.as_slice(), &["auto"]].concat()) },
        "tint": { "type": "string", "enum": ([TINTS.as_slice(), &["auto"]].concat()), "description": "Roof colour (auto: picked)" },
        "material": { "type": "string", "enum": ([roof::MATERIALS.as_slice(), &["auto"]].concat()) },
        "shape": { "type": "string", "enum": ([roof::SHAPES.as_slice(), &["auto"]].concat()) },
        "shade": { "oneOf": [{ "type": "integer", "minimum": 0, "maximum": 4 }, { "type": "string", "enum": ["auto"] }], "description": "Variant 0-4 of the material\'s colours (2 the usual); auto: picked" },
        "structure": { "type": "string", "enum": STRUCTURES },
    })
}

pub fn list() -> Vec<Value> {
    let mut update = props();
    if let Some(m) = update.as_object_mut() {
        m.insert("id".into(), json!({ "type": "string" }));
    }
    let mut create = props();
    if let Some(m) = create.as_object_mut() {
        m.insert("near".into(), crate::args::point_schema("Instead of a footprint: the nearest clear spot for width_ft x depth_ft, fronting a street"));
        m.insert("width_ft".into(), json!({ "type": "number", "minimum": 10, "maximum": 400, "description": "With near: along the street" }));
        m.insert("depth_ft".into(), json!({ "type": "number", "minimum": 10, "maximum": 400 }));
    }
    let mut check = props();
    if let Some(m) = check.as_object_mut() {
        m.retain(|k, _| ["poly", "rect", "circle", "snap", "func"].contains(&k.as_str()));
        m.insert("id".into(), json!({ "type": "string", "description": "A building being reshaped" }));
    }
    let mut remove = crate::args::ids_props("building id (b:<layout>:<id>)");
    if let Some(m) = remove.as_object_mut() {
        m.insert("within".into(), crate::args::area_schema("Buildings whose middle lies inside: [x0, y0, x1, y1], [[x, y], ...] or {at, radius_ft}", 1.0e6));
    }
    vec![
        json!({ "name": "create_building", "title": "Build", "description": "Put a building on the map, drawn by hand: a footprint (poly, rect or circle) or near a place with width_ft and depth_ft, on dry land clear of buildings, roads, streets, walls and squares. It gets an interior, roof and walls. Returns its id (c:<n>) and building id (b:<layout>:0). check_building_spot tests a spot first.", "inputSchema": schema(create, &[]) }),
        json!({ "name": "update_building", "title": "Change a building", "description": "Change a building drawn by hand (c:<n>) or one of the world's own (b:<layout>:<id>): footprint (checked like a new one), name, func, floors, roof, tint, material, shape, shade or structure. Fields left out stay; auto goes back to as generated.", "inputSchema": schema(update, &["id"]) }),
        json!({ "name": "check_building_spot", "title": "Check a building spot", "description": "Whether a footprint (poly, rect or circle) is clear for a building, without building it: its point and the name it would get, or why not (water, river, another building, street, road, wall, square).", "inputSchema": schema(check, &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "remove_buildings", "title": "Remove buildings", "description": "Take away the world's own buildings (delete_feature does drawn ones): by id or ids (b:<layout>:<id>) or every one whose middle lies inside within. Others keep their ids. restore_building brings them back.", "inputSchema": schema(remove, &[]) }),
        json!({ "name": "restore_building", "title": "Restore a building", "description": "Put one of the world's own buildings back as generated: undoes remove_buildings and any update_building change (its name stays as renamed).", "inputSchema": schema(crate::args::ids_props("building id"), &[]) }),
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
        "restore_building" => match crate::args::ids(a) {
            Ok(ids) => restore(app, ids, a["ids"].is_null()).await.map(text),
            Err(e) => Err(e),
        },
        _ => return None,
    })
}

/// A footprint checked without building it.
async fn check_spot(app: &Shared, a: &Value) -> Result<Value, String> {
    let poly = footprint(a)?.ok_or("give the footprint: poly, rect or circle")?;
    let func = a["func"].as_str().map(str::to_string);
    if let Some(f) = &func {
        known_func(f)?;
    }
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

/// A `func` the code knows: a business (`town::catalog`) or a home (`BUILDING_HOMES`); empty is none.
fn known_func(f: &str) -> Result<(), String> {
    let f = f.trim();
    if f.is_empty() || worldgen::town::catalog::index_of(f).is_some() || BUILDING_HOMES.contains(&f) {
        return Ok(());
    }
    let keys = worldgen::town::catalog::CATALOG.iter().map(|c| c.key).chain(BUILDING_HOMES);
    let hint = crate::args::suggest(f, keys.clone()).map(|h| format!(" (did you mean {h}?)")).unwrap_or_default();
    let few: Vec<&str> = ["inn", "tavern", "blacksmith", "temple", "castle"].into_iter().chain(BUILDING_HOMES.into_iter().take(3)).collect();
    Err(format!("func: a business or home such as {}, got {}{hint}; list_names shows the businesses", few.join(", "), json!(f)))
}

/// The `shade` argument: none given, `auto` (`Some(None)`) or a variant 0-4.
fn shade_arg(a: &Value) -> Result<Option<Option<u8>>, String> {
    match &a["shade"] {
        Value::Null => Ok(None),
        Value::String(s) if s == "auto" => Ok(Some(None)),
        v => match v.as_u64() {
            Some(n) if n <= 4 => Ok(Some(Some(n as u8))),
            _ => Err("shade: 0 to 4 (2 the usual), or auto".into()),
        },
    }
}

/// Change one of the world's own buildings (`Edits.buildings`; its name goes to `renames`).
async fn update_generated(app: &Shared, a: &Value, id: String) -> Result<Value, String> {
    let opt = |k: &str| a[k].as_str().map(|s| s.trim().to_string());
    if let Some(f) = opt("func") {
        known_func(&f)?;
    }
    let floors = match &a["floors"] {
        Value::Null => None,
        v => Some(v.as_u64().filter(|f| *f <= 255).ok_or("floors: a whole number")? as u8),
    };
    let change = agent::BuildingChange { func: opt("func"), floors, poly: footprint(a)?, roof: opt("roof"), tint: opt("tint"), material: opt("material"), shape: opt("shape"), shade: shade_arg(a)?, structure: opt("structure") };
    let name = opt("name");
    let id2 = id.clone();
    // Its inside designed by hand: new storeys follow (in the same change); a new footprint or a
    // ruin sets the design aside.
    let reshaped = change.poly.is_some() || change.structure.as_deref() == Some("ruin");
    let (edit, design, aside) = app
        .worker
        .with(move |ex| {
            let edit = agent::building_edit(&ex.world, &ex.t0, &id2, &change)?;
            let Some(d) = ex.world.file.edits.designs.get(&id2) else { return Ok((edit, None, false)) };
            if reshaped {
                return Ok((edit, None, true));
            }
            let Some(f) = change.floors else { return Ok((edit, None, false)) };
            let fit = worldgen::interior::design::refit(&ex.world, &ex.t0, &id2, d, Some(f))?;
            let ok = fit.as_ref().and_then(|d| worldgen::interior::design::problems_as(&ex.world, &ex.t0, &id2, d, Some(f)).ok()).is_some_and(|(_, p)| p.iter().all(|p| !p.blocking));
            Ok::<_, String>(if ok { (edit, fit, false) } else { (edit, None, true) })
        })
        .await?;
    let id3 = id.clone();
    app.edit("agent", 0, move |e| {
        match edit {
            Some(b) => e.buildings.insert(id3.clone(), b),
            None => e.buildings.remove(&id3),
        };
        if let Some(d) = design {
            e.designs.insert(id3.clone(), d);
        }
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
    if aside {
        v["note"] = json!("its inside was designed by hand for the building as it was: that design is set aside (the generated interior stands) until the footprint and storeys are as they were; set_site_design makes a new one");
    }
    Ok(v)
}

/// Take away buildings of the world's own: by ids, or every one with its middle in `within`.
async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let mut ids = if a["id"].is_null() && a["ids"].is_null() { Vec::new() } else { crate::args::ids(a)? };
    let single = a["ids"].is_null();
    let within = if a["within"].is_null() { None } else { Some(crate::args::area(app, "within", &a["within"]).await?) };
    if ids.is_empty() && within.is_none() {
        return Err("give id, ids or within".into());
    }
    let edits = app
        .worker
        .with(move |ex| {
            if let Some(area) = &within {
                ids.extend(agent::generated_buildings_in(&ex.world, &ex.t0, &area.outline()).into_iter().filter(|(_, at)| area.contains(*at)).map(|(id, _)| id));
            }
            ids.sort();
            ids.dedup();
            ids.into_iter().enumerate().map(|(i, id)| agent::building_removal(&ex.world, &ex.t0, &id).map(|e| (id, e)).map_err(|m| if single { format!("id: {m}") } else { format!("{}: {m}", crate::args::item(false, i)) })).collect::<Result<Vec<_>, String>>()
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

/// Some of the world's own buildings back as generated (all or none).
async fn restore(app: &Shared, ids: Vec<String>, single: bool) -> Result<Value, String> {
    let ids2 = ids.clone();
    app.edit("agent", 0, move |e| {
        if let Some((i, id)) = ids2.iter().enumerate().find(|(_, id)| !e.buildings.contains_key(*id)) {
            return Err(format!("{}: {id} is as generated already", crate::args::item(single, i)));
        }
        for id in &ids2 {
            e.buildings.remove(id);
        }
        Ok(json!({ "tool": "restore_building", "ids": ids2 }))
    })
    .await?;
    let ids2 = ids.clone();
    let mut got = app
        .worker
        .with(move |ex| ids2.iter().map(|id| agent::get(&ex.world, &ex.t0, id).ok_or_else(|| "restored, but it could not be read back".to_string())).collect::<Result<Vec<_>, String>>())
        .await?;
    if ids.len() == 1 {
        let mut v = got.remove(0);
        v["tool"] = json!("restore_building");
        return Ok(v);
    }
    Ok(json!({ "tool": "restore_building", "ids": ids, "restored": got }))
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

/// The key a `near` call is remembered by: the open world and the folded arguments as canonical JSON
/// (`serde_json` maps are sorted), `world` left out.
fn near_key(app: &Shared, a: &Value) -> Option<(u64, String)> {
    let mut a = a.clone();
    a.as_object_mut()?.remove("world");
    Some((app.current()?, a.to_string()))
}

/// The building an identical earlier `near` call made, if it still stands.
fn remembered(app: &Shared, key: &(u64, String)) -> Option<String> {
    let (id, at) = app.near_calls.lock().unwrap().iter().rev().find(|(k, _)| k == key)?.1.clone();
    app.with_edits(|e| e.created.iter().any(|c| c.id == id && !c.removed && c.kind == "building" && (c.x - at[0]).hypot(c.y - at[1]) < 1.0))?.then_some(id)
}

/// Create a building (`id` None) or change one.
async fn build(app: &Shared, a: &Value, id: Option<String>) -> Result<Value, String> {
    let poly = footprint(a)?;
    // The footprint's middle as asked for (before the grid): a move of more than 5 ft is said.
    let asked_mid = match &poly {
        Some(_) => {
            let mut raw = a.clone();
            raw["snap"] = json!(false);
            footprint(&raw)?.filter(|p| !p.is_empty()).map(|p| [p.iter().map(|q| q[0]).sum::<f64>() / p.len() as f64, p.iter().map(|q| q[1]).sum::<f64>() / p.len() as f64])
        }
        None => None,
    };
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
    let near_key = if creating && poly.is_none() && !a["near"].is_null() { near_key(app, a) } else { None };
    let (was_floors, was_ruin) = (c.floors, c.structure.as_deref() == Some("ruin"));
    let poly = match poly {
        None if creating && !a["near"].is_null() => {
            // The nearest clear lot near a place.
            let near = crate::args::point(app, "near", &a["near"]).await?;
            let size = |k: &str| a[k].as_f64().ok_or_else(|| format!("{k}: needed with near"));
            let (w, d) = (size("width_ft")?, size("depth_ft")?);
            // The same call again (a script run twice): mapd remembers this session's `near` calls; one
            // identical to an earlier one whose building still stands gives that building, nothing added.
            if let Some(id) = near_key.as_ref().and_then(|k| remembered(app, k)) {
                return found(app, id).await;
            }
            let (func, cid) = (a["func"].as_str().map(str::to_string), c.id.clone());
            let found = app
                .worker
                .with(move |ex| {
                    agent::find_spot(&ex.world, &ex.t0, near, w, d, func.as_deref(), &cid).map(|s| s.0)
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
        known_func(&f)?;
        c.func = Some(f).filter(|f| !f.is_empty());
    }
    if let Some(r) = opt("roof") {
        c.roof = Some(r).filter(|r| r != "auto");
    }
    if let Some(t) = opt("tint") {
        c.tint = Some(t).filter(|t| t != "auto");
    }
    if let Some(m) = opt("material") {
        c.material = Some(m).filter(|m| m != "auto");
    }
    if let Some(r) = opt("shape") {
        c.shape = Some(r).filter(|r| r != "auto");
    }
    if let Some(s) = shade_arg(a)? {
        c.shade = s;
    }
    if let Some(s) = opt("structure") {
        c.structure = Some(s).filter(|s| s != "roofed");
    }
    match &a["floors"] {
        Value::Null => {}
        v => c.floors = Some(v.as_u64().filter(|f| *f <= 255).ok_or("floors: a whole number")? as u8),
    }
    c.check()?;
    // Its inside designed by hand (`b:<layout>:0`): new storeys follow (in the same change); a new
    // footprint or a ruin sets the design aside.
    let mut design: Option<(String, worldgen::under::design::SiteDesign)> = None;
    let mut aside = false;
    if !creating {
        let reshaped = reshaped || (!was_ruin && c.structure.as_deref() == Some("ruin"));
        let floors = (c.floors != was_floors).then(|| c.floors.unwrap_or(1).max(1));
        let cid = c.id.clone();
        (design, aside) = app
            .worker
            .with(move |ex| {
                let Some(li) = agent::layout_of(&ex.world, &ex.t0, &cid) else { return Ok((None, false)) };
                let bid = format!("b:{li}:0");
                let Some(d) = ex.world.file.edits.designs.get(&bid) else { return Ok((None, false)) };
                if reshaped {
                    return Ok((None, true));
                }
                let Some(f) = floors else { return Ok((None, false)) };
                let fit = worldgen::interior::design::refit(&ex.world, &ex.t0, &bid, d, Some(f)).ok().flatten();
                let ok = fit.as_ref().and_then(|d| worldgen::interior::design::problems_as(&ex.world, &ex.t0, &bid, d, Some(f)).ok()).is_some_and(|(_, p)| p.iter().all(|p| !p.blocking));
                Ok(if ok { (fit.map(|d| (bid, d)), false) } else { (None, true) })
            })
            .await?;
    }
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
    let moved = asked_mid.map(|m| ((c.x - m[0]).hypot(c.y - m[1]), c.x, c.y)).filter(|(ft, ..)| *ft > 5.0).map(|(ft, x, y)| {
        let why = if a["snap"].as_bool().unwrap_or(true) { "the footprint was snapped to the 5-ft grid and its point set where it fits" } else { "its point was set where it fits" };
        json!({ "x_ft": x.round(), "y_ft": y.round(), "ft": ft.round(), "why": why })
    });
    let tool = if creating { "create_building" } else { "update_building" };
    let reply = json!({ "tool": tool, "id": c.id, "kind": "building", "name": c.name, "x_ft": c.x.round(), "y_ft": c.y.round() });
    let cid = c.id.clone();
    let mut reply = reply;
    let app2 = app.clone();
    let change = app.edit("agent", 0, move |e| {
        if creating {
            if let Some(o) = e.existing_site(&c, at) {
                return Ok(json!({ "existing": o.id }));
            }
            // (Another client may have created a site meanwhile: take the next id.)
            c.id = format!("c:{}", e.created.len());
            reply["id"] = json!(c.id);
            if let Some(k) = near_key {
                let mut m = app2.near_calls.lock().unwrap();
                m.push((k, (c.id.clone(), [c.x, c.y])));
                if m.len() > 500 {
                    m.remove(0);
                }
            }
            e.created.push(c);
        } else {
            let slot = e.created.iter_mut().find(|x| x.id == c.id && !x.removed).ok_or_else(|| format!("{} was deleted meanwhile", c.id))?;
            *slot = c;
            if let Some((bid, d)) = design {
                e.designs.insert(bid, d);
            }
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
            if aside {
                v["note"] = json!("its inside was designed by hand for the building as it was: that design is set aside (the generated interior stands) until the footprint and storeys are as they were; set_site_design makes a new one");
            }
            if let Some(m) = moved {
                v["moved"] = m;
            }
            Ok(v)
        })
        .await
}
