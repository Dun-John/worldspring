//! MCP tools for castles and walls drawn by hand (`Created` sites of kind `castle` and `wall`):
//! `create_feature` with those kinds comes here, and `update_fortification` changes one.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::town::pieces;
use worldgen::world::{BuildingEdit, CASTLE_MAX_FT, CASTLE_MIN_FT, CastlePiece, Created, STRUCTURES, WALL_MAX_FT, WALL_RING_MAX_FT};

use crate::Shared;
use crate::tools::{found, schema, text};

const SQUARE_FT: f64 = 5.0;

/// The fields of a castle or wall, for `create_feature` and `update_fortification`.
pub fn props() -> Value {
    let pt = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    json!({
        "poly": { "type": "array", "items": pt, "minItems": 3, "maxItems": 32, "description": format!("A castle's outline: convex corners in order, {CASTLE_MIN_FT} to {CASTLE_MAX_FT} ft across") },
        "rect": { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" }, "width_ft": { "type": "number" }, "depth_ft": { "type": "number" }, "angle_deg": { "type": "number", "default": 0 } }, "required": ["x_ft", "y_ft", "width_ft", "depth_ft"], "description": "Instead of poly: a rectangle about its middle (angle_deg clockwise from east)" },
        "gate": { "type": "integer", "minimum": 0, "description": "A castle's gate: its side (corner gate to the next); else facing a road" },
        "keep": { "type": "boolean", "default": true, "description": "A castle's keep in the yard" },
        "yard_buildings": { "type": "boolean", "default": true, "description": "Barracks, stables, a smithy... along the inside of the walls" },
        "structure": { "type": "string", "enum": STRUCTURES, "description": "Roofed (default) or in ruins" },
        "keep_func": { "type": "string", "description": "The keep's business (a list_names key) or home; auto: the castle itself" },
        "keep_name": { "type": "string", "description": "The keep's own name; empty: the castle's" },
        "pts": { "type": "array", "items": pt, "minItems": 2, "maxItems": 256, "description": format!("A wall's line: corners in order, sides 10 ft or more, {WALL_MAX_FT} ft in all ({WALL_RING_MAX_FT} closed)") },
        "settlement": { "type": "string", "description": "With around_patches: the town (its id)" },
        "around_patches": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "minItems": 1, "maxItems": 256, "description": "Instead of pts: a closed wall round these patches of the settlement's plan (get_town_plan); give it again after moving corners" },
        "street_gates": { "type": "boolean", "default": false, "description": "A gate on every street crossing the wall (else only roads and main streets)" },
        "gates": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "maxItems": 256, "description": "Corners (indices into its line) that are gates" },
        "closed": { "type": "boolean", "default": false, "description": "A wall's last corner joins its first (a ring)" },
        "snap": { "type": "boolean", "default": true, "description": "Snap corners to the 5-ft grid" },
        "remove_in_way": { "type": "boolean", "default": false, "description": "Take away the world's buildings in its way (else refused)" },
    })
}

pub fn list() -> Vec<Value> {
    let mut update = props();
    if let Some(m) = update.as_object_mut() {
        m.insert("id".into(), json!({ "type": "string", "description": "A castle or wall drawn by hand (its created id c:<n>)" }));
        m.insert("name".into(), json!({ "type": "string" }));
    }
    vec![json!({ "name": "update_fortification", "title": "Change a castle or wall", "description": "Change a castle or wall made with create_feature: outline (poly or rect), line (pts), gate or gates, keep, keep_func, keep_name, yard_buildings, structure, closed, name. Fields left out stay; checked like a new one. delete_feature takes it away.", "inputSchema": schema(update, &["id"]) })]
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
    let mut ring_report = serde_json::Map::new();
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
        keep_args(a, &mut c)?;
    } else {
        let (before, was_closed) = (c.pts.clone(), c.closed);
        if let Some(b) = a["closed"].as_bool() {
            c.closed = b;
        }
        let ringed = match crate::handwalls::line(app, a, &mut c).await? {
            Some(r) => {
                ring_report = r;
                true
            }
            None => false,
        };
        if !ringed && let Some(p) = corners(a, "pts")? {
            c.pts = p;
            c.gates.retain(|g| (*g as usize) < c.pts.len());
        }
        if let Some(g) = a["gates"].as_array() {
            let hand = g.iter().map(|v| v.as_u64().filter(|k| *k < 1000).map(|k| k as u32)).collect::<Option<Vec<_>>>().ok_or("gates: corner indices")?;
            if ringed { c.gates.extend(hand) } else { c.gates = hand }
            c.gates.sort_unstable();
            c.gates.dedup();
        }
        if !ringed && (creating || c.pts != before || c.closed != was_closed || a["street_gates"].is_boolean()) {
            ring_report = crate::handwalls::street_gates(app, a, &mut c).await?;
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
            agent::works_spot(&ex.world, &ex.t0, &probe, &probe.id, skip).map(|s| (s.at, s.name, s.in_way, s.in_way_names))
        })
        .await?;
    let (p, name, in_way, names) = spot;
    if !in_way.is_empty() && !a["remove_in_way"].as_bool().unwrap_or(false) {
        let ids: Vec<String> = in_way.iter().map(|(id, _)| names.get(id).map_or_else(|| id.clone(), |n| format!("{id} ({n})"))).take(20).collect();
        // (A ring round patches: most stand where one district's lots run across a patch edge.)
        let bare = ring_report.get("sides_without_street").map(|s| format!(" (the ring's sides from corners {s} run where lots cross a patch edge with no street)")).unwrap_or_default();
        return Err(format!(
            "{} of the world's own buildings stand in the way ({}{}){bare}: pass remove_in_way: true to take them away with it",
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
    let cid2 = id.clone();
    let mut v = app
        .worker
        .with(move |ex| {
            let mut v = agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| "made, but it could not be read back".to_string())?;
            if let Some(li) = agent::layout_of(&ex.world, &ex.t0, &id) {
                let l = worldgen::town::layout(&ex.world, &ex.t0, li);
                v["towers"] = json!(l.towers.len());
                v["gates"] = json!(l.gate_towers.len() / 2);
                if let Some(k) = worldgen::town::pieces::keep_index(&l).map(|i| &l.buildings[i]) {
                    v["keep"] = json!(format!("b:{li}:{}", k.id));
                }
                v["buildings"] = json!(l.buildings.len());
            }
            if castle {
                let aside = aside_of(ex, &cid2);
                if !aside.is_empty() {
                    v["pieces_set_aside"] = json!(aside);
                }
            }
            Ok::<_, String>(v)
        })
        .await?;
    v["tool"] = json!(tool);
    if !castle {
        crate::handwalls::reply(app, &mut v, ring_report).await;
    }
    if !removed.is_empty() {
        v["removed"] = json!(removed);
    }
    Ok(v)
}

/// `keep_func` and `keep_name` on a castle: its keep's piece edit (`Created::pieces`).
fn keep_args(a: &Value, c: &mut Created) -> Result<(), String> {
    let (func, name) = (a["keep_func"].as_str().map(str::trim), a["keep_name"].as_str().map(str::trim));
    if func.is_none() && name.is_none() {
        return Ok(());
    }
    if c.keep == Some(false) {
        return Err("keep_func and keep_name are for a castle with a keep (it was made with keep: false)".into());
    }
    // (Where the keep stood when changed: kept for reading; the keep's edit follows the keep.)
    let n = c.poly.len().max(1) as f64;
    let at = if c.poly.is_empty() { [c.x, c.y] } else { [c.poly.iter().map(|q| q[0]).sum::<f64>() / n, c.poly.iter().map(|q| q[1]).sum::<f64>() / n] };
    let p = c.pieces.entry(CastlePiece::KEEP.to_string()).or_insert_with(|| CastlePiece { edit: BuildingEdit { at, ..Default::default() }, name: None });
    if let Some(f) = func {
        p.edit.func = Some(f.to_string()).filter(|f| !f.is_empty() && f != "auto");
    }
    if let Some(n) = name {
        p.name = Some(n.to_string()).filter(|n| !n.is_empty());
    }
    if p.is_noop() {
        c.pieces.remove(CastlePiece::KEEP);
    }
    Ok(())
}

/// A castle's piece edits that find no building now, as the overview lists them (they stay stored).
fn aside_of(ex: &worldgen::pipeline::Executor, castle: &str) -> Vec<Value> {
    pieces::set_aside(&ex.world, &ex.t0).into_iter().filter(|(_, c, _)| c == castle).map(|(id, _, why)| json!({ "id": id, "why": why, "kept": true })).collect()
}

/// A castle piece's edit stored on its castle (`None`: as built), found by its `c:` id.
async fn store_piece(app: &Shared, castle: String, key: String, piece: Option<CastlePiece>, tool: &'static str, id: String) -> Result<Value, String> {
    app.edit("agent", 0, move |e| {
        let c = e.created.iter_mut().find(|c| c.id == castle && !c.removed).ok_or_else(|| format!("{castle} was deleted meanwhile"))?;
        match piece {
            Some(p) => drop(c.pieces.insert(key, p)),
            None => drop(c.pieces.remove(&key)),
        }
        Ok(json!({ "tool": tool, "id": id }))
    })
    .await
}

/// A castle piece read back after a change, with its castle.
async fn piece_reply(app: &Shared, id: String, tool: &'static str, castle: (String, String)) -> Result<Value, String> {
    let id2 = id.clone();
    let c2 = castle.0.clone();
    let (mut v, aside) = app
        .worker
        .with(move |ex| agent::get(&ex.world, &ex.t0, &id2).map(|v| (v, aside_of(ex, &c2))).ok_or_else(|| "changed, but it could not be read back".to_string()))
        .await?;
    if !aside.is_empty() {
        v["pieces_set_aside"] = json!(aside);
    }
    v["tool"] = json!(tool);
    v["id"] = json!(id);
    v["castle"] = json!({ "id": castle.0, "name": castle.1 });
    Ok(v)
}

/// `update_building` on a piece of a castle made by hand (its keep or a yard building): `None`
/// when `id` is not one. The edit is kept on the castle (by its `c:` id).
pub async fn update_piece(app: &Shared, id: &str, change: &agent::BuildingChange, name: Option<&str>, args: &Value) -> Result<Option<Value>, String> {
    let (id2, change, name) = (id.to_string(), change.clone(), name.map(str::to_string));
    // (Asked for in words update_building has none for: answered, not skipped.)
    let stray = if args["removed"].as_bool() == Some(true) {
        Some(format!("{id} is a piece of a castle: update_building does not take it away (remove_buildings does, a yard building only; the keep goes with update_fortification keep: false)"))
    } else if args.get("footprint").is_some() {
        Some(format!("{id} is a piece of a castle: its footprint is the castle's (redraw the castle with update_fortification); update_building has no footprint argument"))
    } else {
        None
    };
    let r = app
        .worker
        .with(move |ex| {
            let Some(p) = pieces::piece_of(&ex.world, &ex.t0, &id2) else { return Ok(None) };
            if let Some(s) = stray {
                return Err(s);
            }
            let e = pieces::piece_edit(&ex.world, &p, &change, name.as_deref())?;
            Ok(Some((p.castle.clone(), p.castle_name.clone(), p.key.clone(), p.what(), e)))
        })
        .await?;
    let Some((castle, castle_name, key, what, e)) = r else { return Ok(None) };
    store_piece(app, castle.clone(), key, e, "update_building", id.to_string()).await?;
    let mut v = piece_reply(app, id.to_string(), "update_building", (castle, castle_name)).await?;
    v["piece"] = json!(what);
    Ok(Some(v))
}

/// Castle pieces among `ids` (yard buildings to take away): (id, castle, key, edit), each
/// checked; the rest are left in `ids`.
pub fn piece_removals(ex: &worldgen::pipeline::Executor, ids: &mut Vec<String>) -> Result<Vec<(String, String, String, CastlePiece)>, String> {
    let mut out = Vec::new();
    let mut rest = Vec::new();
    for id in ids.drain(..) {
        match pieces::piece_of(&ex.world, &ex.t0, &id) {
            Some(p) => out.push((id, p.castle.clone(), p.key.clone(), pieces::piece_removal(&p)?)),
            None => rest.push(id),
        }
    }
    *ids = rest;
    Ok(out)
}

/// `restore_building` on a castle piece: its edit taken off the castle. `None` when `id` is not
/// one.
pub async fn restore_piece(app: &Shared, id: &str) -> Result<Option<Value>, String> {
    let id2 = id.to_string();
    let r = app.worker.with(move |ex| Ok(pieces::piece_of(&ex.world, &ex.t0, &id2).map(|p| (p.castle.clone(), p.castle_name.clone(), p.key.clone())))).await?;
    let Some((castle, castle_name, key)) = r else { return Ok(None) };
    let (c2, k2, id3) = (castle.clone(), key.clone(), id.to_string());
    app.edit("agent", 0, move |e| {
        let c = e.created.iter_mut().find(|c| c.id == c2 && !c.removed).ok_or_else(|| format!("{c2} was deleted meanwhile"))?;
        match c.pieces.remove(&k2) {
            Some(_) => Ok(json!({ "tool": "restore_building", "id": id3 })),
            None => Err(format!("{id3} is as built already")),
        }
    })
    .await?;
    piece_reply(app, id.to_string(), "restore_building", (castle, castle_name)).await.map(Some)
}
