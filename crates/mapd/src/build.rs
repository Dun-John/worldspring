//! MCP tools for buildings drawn by hand (`Created` sites of kind `building`): each is a site of
//! its own (`b:<layout>:0`), with an interior, a roof and a battlemap footprint like the world's.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::world::{BUILDING_HOMES, Created, MAX_FLOORS, ROOFS, STRUCTURES, TINTS};

use crate::Shared;
use crate::tools::{schema, text};

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
        m.insert("id".into(), json!({ "type": "string", "description": "The building's created id (c:<n>)" }));
    }
    vec![
        json!({ "name": "create_building", "title": "Build", "description": "Put a building on the map, drawn by hand: a footprint (poly, rect or circle) on dry land clear of other buildings, roads, streets and walls, with what it is, storeys, roof and roof colour. It gets an interior (enter_building / get_interior on its building id), a roof and walls on the battlemap. Returns its id (c:<n>, for update_building, delete_feature, notes and NPCs) and its building id (b:<layout>:0).", "inputSchema": schema(props(), &[]) }),
        json!({ "name": "update_building", "title": "Change a building", "description": "Change a building drawn by hand: a new footprint (poly, rect or circle; checked like a new one), name, func, floors, roof, tint or structure. Fields left out stay.", "inputSchema": schema(update, &["id"]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "create_building" => build(app, a, None).await.map(text),
        "update_building" => match a["id"].as_str() {
            Some(id) => build(app, a, Some(id.to_string())).await.map(text),
            None => Err("missing 'id'".into()),
        },
        _ => return None,
    })
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
    if creating && poly.is_none() {
        return Err("give the footprint: poly, rect or circle".into());
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
    let asked = opt("name").filter(|s| !s.is_empty());
    // On dry land, clear of buildings, roads and walls; its point is the footprint's middle.
    if reshaped || creating {
        let (cid, poly, func) = (c.id.clone(), c.poly.clone(), c.func.clone());
        let (p, name) = app
            .worker
            .with(move |ex| {
                let skip = agent::layout_of(&ex.world, &ex.t0, &cid);
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
