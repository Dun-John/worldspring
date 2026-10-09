//! MCP tools for castles and walls drawn by hand (`Created` sites of kind `castle` and `wall`):
//! `create_feature` with those kinds comes here, and `update_fortification` changes one.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::world::{CASTLE_MAX_FT, CASTLE_MIN_FT, Created, STRUCTURES, WALL_MAX_FT};

use crate::Shared;
use crate::tools::{found, schema, text};

const SQUARE_FT: f64 = 5.0;

/// The fields of a castle or wall, for `create_feature` and `update_fortification`.
pub fn props() -> Value {
    let pt = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    json!({
        "poly": { "type": "array", "items": pt, "description": format!("A castle's outline: corners [x_ft, y_ft] in order, convex (3-32), {CASTLE_MIN_FT} to {CASTLE_MAX_FT} ft across. Its curtain wall stands just inside it") },
        "rect": { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "width_ft": { "type": "number" }, "depth_ft": { "type": "number" }, "angle_deg": { "type": "number", "default": 0 } }, "required": ["x_ft", "y_ft", "width_ft", "depth_ft"], "description": "Or a castle's outline as a rectangle about its middle (width along angle_deg, clockwise from east)" },
        "gate": { "type": "integer", "minimum": 0, "description": "A castle's gate: the side it is in (from corner gate to the next); else the side facing the nearest road" },
        "keep": { "type": "boolean", "default": true, "description": "A castle's keep (the castle itself, named as the site) in the yard" },
        "yard_buildings": { "type": "boolean", "default": true, "description": "Buildings along the inside of a castle's walls (barracks, stables, a smithy...)" },
        "structure": { "type": "string", "enum": STRUCTURES, "description": "A castle roofed (default) or in ruins (broken walls, fallen towers, roofless buildings)" },
        "pts": { "type": "array", "items": pt, "description": format!("A wall's line: corners [x_ft, y_ft] in order (2-64), each side at least 10 ft, at most {WALL_MAX_FT} ft in all. Towers stand on its corners and along long runs") },
        "gates": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "A wall's corners (indices into pts) that are gates; roads and streets crossing it get gates of their own" },
        "closed": { "type": "boolean", "default": false, "description": "A wall's last corner joins its first (a ring)" },
        "snap": { "type": "boolean", "default": true, "description": "Snap corners to the 5-ft grid" },
        "remove_in_way": { "type": "boolean", "default": false, "description": "Take away the world's own buildings in its way (else it is refused, naming them)" },
    })
}

pub fn list() -> Vec<Value> {
    let mut update = props();
    if let Some(m) = update.as_object_mut() {
        m.insert("id".into(), json!({ "type": "string", "description": "A castle or wall drawn by hand (its created id c:<n>)" }));
        m.insert("name".into(), json!({ "type": "string" }));
    }
    vec![json!({ "name": "update_fortification", "title": "Change a castle or wall", "description": "Change a castle or wall made with create_feature: a new outline (poly or rect) or line (pts), its gate or gates, keep, yard_buildings, structure, closed, name. Fields left out stay; it is checked like a new one. delete_feature takes it away.", "inputSchema": schema(update, &["id"]) })]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "update_fortification" => match a["id"].as_str() {
            Some(id) => place(app, a, Some(id.to_string()), None).await.map(text),
            None => Err("missing 'id'".into()),
        },
        _ => return None,
    })
}

/// Corners from `a[key]` (snapped to the grid unless `snap` is false).
fn corners(a: &Value, key: &str) -> Result<Option<Vec<[f64; 2]>>, String> {
    let snap = a["snap"].as_bool().unwrap_or(true);
    let grid = |v: f64| if snap { (v / SQUARE_FT).round() * SQUARE_FT } else { v };
    let Some(p) = a[key].as_array() else { return Ok(None) };
    let pts: Option<Vec<[f64; 2]>> = p.iter().map(|q| Some([grid(q.get(0)?.as_f64()?), grid(q.get(1)?.as_f64()?)])).collect();
    let mut pts = pts.ok_or_else(|| format!("{key}: corners as [x_ft, y_ft]"))?;
    pts.dedup();
    Ok(Some(pts))
}

/// A castle's outline from `rect`.
fn rect(a: &Value) -> Result<Option<Vec<[f64; 2]>>, String> {
    let r = &a["rect"];
    if !r.is_object() {
        return Ok(None);
    }
    let num = |k: &str| r[k].as_f64().filter(|v| v.is_finite()).ok_or_else(|| format!("rect: missing '{k}'"));
    let (x, y, w, d) = (num("x_ft")?, num("y_ft")?, num("width_ft")?, num("depth_ft")?);
    let ang = r["angle_deg"].as_f64().unwrap_or(0.0).to_radians();
    let (u, v) = ([ang.cos(), ang.sin()], [-ang.sin(), ang.cos()]);
    let snap = a["snap"].as_bool().unwrap_or(true) && ang.abs() < 1e-9;
    let corner = |s: f64, t: f64| {
        let p = [x + u[0] * s * w / 2.0 + v[0] * t * d / 2.0, y + u[1] * s * w / 2.0 + v[1] * t * d / 2.0];
        if snap { p.map(|c| (c / SQUARE_FT).round() * SQUARE_FT) } else { p }
    };
    Ok(Some(vec![corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]))
}

/// Create a castle or wall (`id` None, `kind` given) or change one: checked like a new one; the
/// world's own buildings in its way are taken away with it only with `remove_in_way`.
pub async fn place(app: &Shared, a: &Value, id: Option<String>, kind: Option<&str>) -> Result<Value, String> {
    let creating = id.is_none();
    let mut c = match &id {
        Some(id) => app
            .with_edits(|e| e.created.iter().find(|c| &c.id == id && !c.removed && (c.kind == "castle" || c.kind == "wall")).cloned())
            .ok_or("no world is open: open the map app first")?
            .ok_or_else(|| format!("{id} is not a castle or wall made by hand"))?,
        None => {
            let n = app.with_edits(|e| e.created.len()).unwrap_or(0);
            Created { id: format!("c:{n}"), kind: kind.unwrap_or("castle").into(), ..Default::default() }
        }
    };
    let castle = c.kind == "castle";
    if castle {
        if let Some(p) = corners(a, "poly")?.or(rect(a)?) {
            if c.gate.is_some_and(|g| g as usize >= p.len()) {
                c.gate = None;
            }
            c.poly = p;
        }
        match &a["gate"] {
            Value::Null => {}
            v => c.gate = Some(v.as_u64().filter(|g| *g < 1000).ok_or("gate: a side's index")? as u32),
        }
        for (k, slot) in [("keep", &mut c.keep), ("yard_buildings", &mut c.yard_buildings)] {
            if let Some(b) = a[k].as_bool() {
                *slot = (!b).then_some(false);
            }
        }
        if let Some(s) = a["structure"].as_str() {
            c.structure = Some(s.to_string()).filter(|s| s != "roofed");
        }
    } else {
        if let Some(p) = corners(a, "pts")? {
            c.pts = p;
            c.gates.retain(|g| (*g as usize) < c.pts.len());
        }
        if let Some(g) = a["gates"].as_array() {
            c.gates = g.iter().map(|v| v.as_u64().filter(|k| *k < 1000).map(|k| k as u32)).collect::<Option<Vec<_>>>().ok_or("gates: corner indices")?;
            c.gates.sort_unstable();
            c.gates.dedup();
        }
        if let Some(b) = a["closed"].as_bool() {
            c.closed = b;
        }
    }
    let pts = if castle { &c.poly } else { &c.pts };
    if pts.is_empty() {
        return Err(if castle { "give the castle's outline: poly or rect".into() } else { "give the wall's line: pts".into() });
    }
    // Its point (checked against the corners) before the generator puts it in the middle.
    let n = pts.len() as f64;
    (c.x, c.y) = (pts.iter().map(|q| q[0]).sum::<f64>() / n, pts.iter().map(|q| q[1]).sum::<f64>() / n);
    c.check()?;
    let asked = a["name"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let at = [c.x, c.y];
    if creating && let Some(id) = app.with_edits(|e| e.existing_site(&Created { name: asked.clone().unwrap_or_default(), ..c.clone() }, at).map(|o| o.id.clone())).flatten() {
        return found(app, id).await;
    }
    let probe = c.clone();
    let spot = app
        .worker
        .with(move |ex| {
            let skip = agent::layout_of(&ex.world, &ex.t0, &probe.id).filter(|_| !creating);
            agent::works_spot(&ex.world, &ex.t0, &probe, &probe.id, skip).map(|s| (s.at, s.name, s.in_way))
        })
        .await?;
    let (p, name, in_way) = spot;
    if !in_way.is_empty() && !a["remove_in_way"].as_bool().unwrap_or(false) {
        let ids: Vec<&str> = in_way.iter().map(|(id, _)| id.as_str()).take(20).collect();
        return Err(format!(
            "{} of the world's own buildings stand in the way ({}{}): pass remove_in_way: true to take them away with it",
            in_way.len(),
            ids.join(", "),
            if in_way.len() > ids.len() { ", ..." } else { "" }
        ));
    }
    (c.x, c.y) = (p[0], p[1]);
    if creating {
        c.name = name;
    }
    if let Some(n) = asked {
        c.name = n;
    }
    let tool = if creating { "create_feature" } else { "update_fortification" };
    let removed: Vec<String> = in_way.iter().map(|(id, _)| id.clone()).collect();
    let mut reply = json!({ "tool": tool, "id": c.id, "kind": c.kind, "name": c.name, "x_ft": c.x.round(), "y_ft": c.y.round() });
    if !removed.is_empty() {
        reply["removed"] = json!(removed);
    }
    let cid = c.id.clone();
    let change = app
        .edit("agent", 0, move |e| {
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
            for (id, at) in in_way {
                e.buildings.insert(id, worldgen::world::BuildingEdit { at, removed: true, ..Default::default() });
            }
            Ok(reply)
        })
        .await?;
    if let Some(id) = change["existing"].as_str() {
        return found(app, id.to_string()).await;
    }
    let id = if creating { change["id"].as_str().unwrap_or_default().to_string() } else { cid };
    let mut v = app
        .worker
        .with(move |ex| {
            let mut v = agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "made, but it could not be read back".to_string())?;
            if let Some(li) = agent::layout_of(&ex.world, &ex.t0, &id) {
                let l = worldgen::town::layout(&ex.world, &ex.t0, li);
                v["towers"] = json!(l.towers.len());
                v["gates"] = json!(l.gate_towers.len() / 2);
                if let Some(k) = l.buildings.iter().find(|b| b.func.is_some_and(|f| worldgen::town::catalog::CATALOG[f as usize].key == "castle")) {
                    v["keep"] = json!(format!("b:{li}:{}", k.id));
                }
                v["buildings"] = json!(l.buildings.len());
            }
            Ok::<_, String>(v)
        })
        .await?;
    v["tool"] = json!(tool);
    if !removed.is_empty() {
        v["removed"] = json!(removed);
    }
    Ok(v)
}
