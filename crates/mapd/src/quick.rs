//! MCP tools for quick placement (Roadmap 6 P5): `place_stamp` (one building of a kind and size, fronting the nearest street),
//! `create_building_row` (drawn buildings lining a path, in one edit), `create_parcel` / `update_parcel` (a free plot with a fill).

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::placing;
use worldgen::town::geom;
use worldgen::world::{Created, LOT_SIZES, TOWN_WARDS};

use crate::Shared;
use crate::tools::{found, schema, text};

const BUILDING_KEYS: [&str; 8] = ["name", "floors", "roof", "tint", "material", "shape", "shade", "structure"];

fn points_schema(min: usize, max: usize, what: &str) -> Value {
    json!({ "type": "array", "minItems": min, "maxItems": max, "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "description": what })
}

fn fill_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "enum": ["generated", "hand", "open"], "description": "generated: lots of a ward; hand: left empty for buildings drawn by hand; open: open ground" },
            "ward": { "type": "string", "enum": TOWN_WARDS, "description": "generated: the ward (default common)" },
            "lots": { "type": "string", "enum": LOT_SIZES, "description": "generated: lot size (the ward's usual when left out)" },
            "reroll": { "type": "integer", "minimum": 0, "maximum": 1000, "description": "generated: lay the lots out again this many times" },
            "close": { "type": "boolean", "description": "generated, temple ward: a close round the temple or not" },
            "open": { "type": "string", "enum": ["plaza", "lawn", "park", "garden", "market", "cemetery", "farmland"], "description": "open: what the ground is" },
            "style": { "type": "string", "description": "open: the square's or park's kind" },
        },
        "description": "What the plot holds, as a hand block's fill (default generated, common)",
    })
}

pub fn list() -> Vec<Value> {
    let mut stamp = crate::build::props();
    if let Some(m) = stamp.as_object_mut() {
        m.retain(|k, _| BUILDING_KEYS.contains(&k.as_str()));
        m.insert("kind".into(), json!({ "type": "string", "description": "A business or home key (docs/MCP.md, the funcs catalog): house, shop-like trades, inn, temple, ..." }));
        m.insert("size".into(), json!({ "oneOf": [
            { "type": "string", "enum": placing::SIZES, "description": "small (0.75), usual (1) or large (1.4 times the kind's footprint)" },
            { "type": "object", "properties": { "width_ft": { "type": "number", "minimum": 10, "maximum": 400 }, "depth_ft": { "type": "number", "minimum": 10, "maximum": 400 } }, "required": ["width_ft", "depth_ft"], "description": "Or the footprint, width along the street" },
        ], "description": "Default usual" }));
        m.insert("at".into(), crate::args::point_schema("Where: an id, a name, {x_ft, y_ft} or [x_ft, y_ft]"));
        m.insert("facing".into(), json!({ "type": "number", "minimum": 0, "maximum": 360, "description": "The direction its front looks, degrees (0 = +x, 90 = +y): placed there turned that way; left out, it fronts the nearest street" }));
        m.insert("clear".into(), json!({ "type": "boolean", "default": false, "description": "Take away the world's own buildings in the way" }));
        m.insert("preview".into(), json!({ "type": "boolean", "default": false, "description": "Only say where it would land (nothing is made)" }));
    }
    let mut row = crate::build::props();
    if let Some(m) = row.as_object_mut() {
        m.retain(|_, _| false);
        m.insert("along".into(), json!({ "oneOf": [points_schema(2, 200, "A path [x_ft, y_ft] ..."), { "type": "string", "description": "A road stretch id (rd:..., list_roads)" }], "description": "The path to line: points, or a road stretch id" }));
        m.insert("side".into(), json!({ "type": "string", "enum": ["left", "right", "both"], "default": "both", "description": "Left or right of the direction of travel (left is [-dy, dx])" }));
        m.insert("depth".into(), json!({ "type": "number", "minimum": placing::DEPTH_RANGE.0, "maximum": placing::DEPTH_RANGE.1, "description": "Lot depth, ft (default by the ward's lot area, 22 to 60)" }));
        m.insert("kinds".into(), json!({ "type": "array", "minItems": 1, "maxItems": 24, "items": { "type": "string" }, "description": "Business or home keys to mix; left out, homes of the ward with now and then a business" }));
        m.insert("ward".into(), json!({ "type": "string", "enum": TOWN_WARDS, "default": "common", "description": "Sets lot widths, storeys and the mix" }));
        m.insert("gaps".into(), json!({ "type": "number", "minimum": 0, "maximum": 0.5, "default": 0.15, "description": "The share of slots left empty (alleys, yards)" }));
        m.insert("seed".into(), json!({ "type": "integer", "minimum": 0, "description": "Another mix of widths and kinds for the same path" }));
        m.insert("clear".into(), json!({ "type": "boolean", "default": false, "description": "Take away the world's own buildings in the way" }));
        m.insert("preview".into(), json!({ "type": "boolean", "default": false, "description": "Only list the lots (nothing is made)" }));
    }
    let mut parcel = json!({
        "poly": points_schema(3, 64, "Corners [x_ft, y_ft] in order, within 750 ft of their middle"),
        "fill": fill_schema(),
        "name": { "type": "string" },
        "snap": { "type": "boolean", "default": true, "description": "Snap corners to the 5-ft grid" },
    });
    let mut update = parcel.clone();
    if let Some(m) = update.as_object_mut() {
        m.insert("id".into(), json!({ "type": "string", "description": "A parcel (c:<n>)" }));
    }
    if let Some(m) = parcel.as_object_mut() {
        m.remove("id");
    }
    vec![
        json!({ "name": "place_stamp", "title": "Place a building", "description": "Set one building of a kind (house, inn, smithy, ...) and size (small, usual, large or width x depth) down at a point: fronting the nearest street, turned square to it and set back as lots are, clear of everything else (or turned to facing, moved to the nearest clear spot). clear: true takes away the world's own buildings it stands on. Returns the building (c:<n>, b:<layout>:0) with width_ft, depth_ft and facing_deg; preview: true only says where it would land.", "inputSchema": schema(stamp, &["kind", "at"]) }),
        json!({ "name": "create_building_row", "title": "Line a street with buildings", "description": "Draw a row of buildings along a path or a road stretch, on one or both sides: lots cut one after another (widths from the ward's lot area, a seeded mix of kinds and storeys, empty slots between), each fronting the path, set past its bed, clear of everything else. Up to 100 buildings, all in one edit; slots that do not fit are listed in refused. Repeating the call adds nothing. preview: true lists the lots only.", "inputSchema": schema(row, &["along"]) }),
        json!({ "name": "create_parcel", "title": "Mark a plot", "description": "A free plot of ground with a fill, drawn as an outline: generated lots of a ward, left empty for buildings by hand, or open ground. Returns its id (c:<n>); delete_feature removes it, update_parcel changes it. (create_plot is a notebook plot point.)", "inputSchema": schema(parcel, &["poly"]) }),
        json!({ "name": "update_parcel", "title": "Change a plot", "description": "Change a plot made with create_parcel: its outline, fill or name. Fields left out stay.", "inputSchema": schema(update, &["id"]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "place_stamp" => stamp(app, a).await.map(text),
        "create_building_row" => row(app, a).await.map(text),
        "create_parcel" => parcel(app, a, None).await.map(text),
        "update_parcel" => match a["id"].as_str() {
            Some(id) => parcel(app, a, Some(id.to_string())).await.map(text),
            None => Err("missing 'id'".into()),
        },
        _ => return None,
    })
}

async fn stamp(app: &Shared, a: &Value) -> Result<Value, String> {
    let kind = a["kind"].as_str().ok_or("missing 'kind'")?.trim().to_lowercase();
    crate::build::known_func(&kind)?;
    let (w, d) = match &a["size"] {
        Value::Object(o) => (o["width_ft"].as_f64().ok_or("size.width_ft: needed")?, o["depth_ft"].as_f64().ok_or("size.depth_ft: needed")?),
        s => placing::stamp_dims(&kind, s.as_str().unwrap_or("usual")).ok_or_else(|| format!("kind: {kind} has no stamp size (a business or home key)"))?,
    };
    if !(10.0..=400.0).contains(&w) || !(10.0..=400.0).contains(&d) {
        return Err("size: width_ft and depth_ft are 10 to 400 ft".into());
    }
    let at = crate::args::point(app, "at", &a["at"]).await?;
    let (facing, clear, preview) = (a["facing"].as_f64(), a["clear"].as_bool().unwrap_or(false), a["preview"].as_bool().unwrap_or(false));
    let n = app.with_edits(|e| e.created.len()).unwrap_or(0);
    let id = format!("c:{n}");
    let k2 = kind.clone();
    let s = app.worker.with(move |ex| placing::stamp(&ex.world, &ex.t0, at, &k2, w, d, facing, clear, &id)).await?;
    let poly: Vec<Value> = s.poly.iter().map(|p| json!(p)).collect();
    let extra = |v: &mut Value| {
        v["width_ft"] = json!(w);
        v["depth_ft"] = json!(d);
        if let Some(f) = s.facing {
            v["facing_deg"] = json!(f);
        }
        if s.moved_ft > 5.0 {
            v["moved_ft"] = json!(s.moved_ft);
        }
    };
    if preview {
        let mut v = json!({ "tool": "place_stamp", "preview": true, "kind": kind, "name": s.name, "x_ft": s.mid[0].round(), "y_ft": s.mid[1].round(), "poly": poly });
        extra(&mut v);
        if clear {
            v["would_remove"] = json!(s.removed.iter().map(|r| r.to_json()).collect::<Vec<_>>());
        }
        return Ok(v);
    }
    let mut args = json!({ "poly": poly, "snap": false, "func": kind, "clear": clear });
    for k in BUILDING_KEYS {
        if !a[k].is_null() {
            args[k] = a[k].clone();
        }
    }
    let mut v = crate::build::build(app, &args, None).await?;
    v["tool"] = json!("place_stamp");
    extra(&mut v);
    Ok(v)
}

/// A road stretch's line from its id.
async fn road_line(app: &Shared, id: String) -> Result<Vec<[f64; 2]>, String> {
    app.worker
        .with(move |ex| {
            let i = worldgen::roadids::road_index(&ex.t0, &id).ok_or_else(|| format!("along: no road stretch {id} (list_roads gives the ids)"))?;
            Ok(worldgen::roadids::line(&ex.t0.roads.roads[i]))
        })
        .await
}

async fn row(app: &Shared, a: &Value) -> Result<Value, String> {
    let line: Vec<[f64; 2]> = match &a["along"] {
        Value::String(id) => road_line(app, id.clone()).await?,
        Value::Array(p) => p.iter().map(|q| Some([q.get(0)?.as_f64()?, q.get(1)?.as_f64()?])).collect::<Option<Vec<_>>>().ok_or("along: points as [x_ft, y_ft]")?,
        _ => return Err("along: points or a road stretch id".into()),
    };
    let [mw, mh] = crate::args::map_size(app).await?;
    if line.iter().any(|p| !(0.0..mw).contains(&p[0]) || !(0.0..mh).contains(&p[1])) {
        return Err(format!("along: a point is off the map, which is {mw} by {mh} ft"));
    }
    let sides = match a["side"].as_str().unwrap_or("both") {
        "left" => vec![true],
        "right" => vec![false],
        _ => vec![true, false],
    };
    let kinds: Vec<String> = a["kinds"].as_array().map(|k| k.iter().filter_map(|s| s.as_str().map(|s| s.trim().to_lowercase())).collect()).unwrap_or_default();
    for k in &kinds {
        crate::build::known_func(k)?;
        if k.is_empty() {
            return Err("kinds: a business or home key".into());
        }
    }
    let ward = a["ward"].as_str().unwrap_or("common").to_string();
    let (depth, gaps, clear, seed, preview) = (a["depth"].as_f64(), a["gaps"].as_f64().unwrap_or(0.15), a["clear"].as_bool().unwrap_or(false), placing::seed_of(&a["seed"])?, a["preview"].as_bool().unwrap_or(false));
    let n = app.with_edits(|e| e.created.len()).unwrap_or(0);
    let out = app
        .worker
        .with(move |ex| {
            let ask = placing::RowAsk { line: &line, sides, depth, kinds, ward: &ward, gaps, clear, seed };
            let edits = &ex.world.file.edits;
            let existing = |poly: &[[f64; 2]], func: &str| {
                let probe = Created { kind: "building".into(), poly: poly.to_vec(), func: Some(func.to_string()), ..Default::default() };
                let c = geom::centroid(poly);
                edits.existing_site(&probe, c).map(|o| o.id.clone())
            };
            let out = placing::row(&ex.world, &ex.t0, &ask, n, &existing)?;
            // Names as create_building gives them (seeded by the id it will have).
            let mut k = 0;
            let names: Vec<String> = out
                .lots
                .iter()
                .map(|l| {
                    if l.existing.is_some() {
                        return String::new();
                    }
                    k += 1;
                    agent::spot_name(&ex.world, &ex.t0, &l.poly, Some(&l.func), &format!("c:{}", n + k - 1))
                })
                .collect();
            Ok((out, names))
        })
        .await?;
    let (out, names) = out;
    let lots_json = |ids: &[String]| -> Vec<Value> {
        out.lots
            .iter()
            .enumerate()
            .take(placing::ROW_MAX)
            .map(|(i, l)| {
                let mut v = json!({ "kind": l.func, "floors": l.floors, "x_ft": l.mid[0].round(), "y_ft": l.mid[1].round(), "poly": l.poly });
                if let Some(id) = ids.get(i).filter(|s| !s.is_empty()).or(l.existing.as_ref()) {
                    v["id"] = json!(id);
                }
                v
            })
            .collect()
    };
    let refused: Vec<Value> = out.refused.iter().map(|(p, why)| json!({ "x_ft": p[0], "y_ft": p[1], "why": why })).collect();
    let mut reply = json!({ "tool": "create_building_row", "length_ft": out.length_ft.round(), "count": out.lots.len(), "refused_count": out.refused_count });
    if out.lots.len() >= placing::ROW_MAX {
        reply["note"] = json!(format!("stopped at {} buildings: give the rest of the path in another call", placing::ROW_MAX));
    }
    if out.refused_count > 0 {
        reply["refused"] = json!(refused);
    }
    if clear {
        reply["removed"] = json!(out.removed.iter().map(|r| r.to_json()).collect::<Vec<_>>());
    }
    if preview {
        reply["preview"] = json!(true);
        reply["lots"] = json!(lots_json(&[]));
        return Ok(reply);
    }
    let new: Vec<Created> = out
        .lots
        .iter()
        .zip(&names)
        .filter(|(l, _)| l.existing.is_none())
        .map(|(l, name)| Created { kind: "building".into(), x: l.mid[0], y: l.mid[1], name: name.clone(), poly: l.poly.clone(), func: Some(l.func.clone()), floors: Some(l.floors), ..Default::default() })
        .collect();
    for c in &new {
        c.check()?;
    }
    let removed = out.removed;
    let change = app
        .edit("agent", 0, move |e| {
            let mut ids = Vec::new();
            for mut c in new {
                if let Some(o) = e.existing_site(&c, [c.x, c.y]) {
                    ids.push(o.id.clone());
                    continue;
                }
                c.id = format!("c:{}", e.created.len());
                ids.push(c.id.clone());
                e.created.push(c);
            }
            for r in removed {
                e.buildings.insert(r.id, worldgen::world::BuildingEdit { at: r.at, removed: true, ..Default::default() });
            }
            Ok(json!({ "ids": ids }))
        })
        .await?;
    // The ids in lot order (an existing lot keeps its own).
    let mut fresh = change["ids"].as_array().cloned().unwrap_or_default().into_iter().map(|v| v.as_str().unwrap_or_default().to_string());
    let ids: Vec<String> = out.lots.iter().map(|l| l.existing.clone().unwrap_or_else(|| fresh.next().unwrap_or_default())).collect();
    reply["ids"] = json!(ids);
    reply["lots"] = json!(lots_json(&ids));
    Ok(reply)
}

/// Build a `Created` plot's fill from the tool's `fill` object.
fn fill_of(f: &Value) -> Result<worldgen::town::hand::BlockFill, String> {
    let mut f = f.clone();
    if !f.is_object() {
        return Err(crate::args::bad("fill", "an object", &f));
    }
    if f["kind"].is_null() {
        f["kind"] = json!("generated");
    }
    if f["kind"] == "generated" && f["ward"].is_null() {
        f["ward"] = json!("common");
    }
    serde_json::from_value(f).map_err(|e| format!("fill: {e}"))
}

async fn parcel(app: &Shared, a: &Value, id: Option<String>) -> Result<Value, String> {
    let creating = id.is_none();
    let poly = crate::build::footprint(a)?;
    let mut c = match &id {
        Some(id) => app
            .with_edits(|e| e.created.iter().find(|c| &c.id == id && !c.removed && c.kind == "plot").cloned())
            .ok_or("no world is open: open the map app first")?
            .ok_or_else(|| format!("{id} is not a parcel (a plot made by create_parcel)"))?,
        None => {
            if poly.is_none() {
                return Err("give the outline: poly".into());
            }
            Created { id: String::new(), kind: "plot".into(), fill: Some(fill_of(&json!({}))?), ..Default::default() }
        }
    };
    if !a["fill"].is_null() {
        c.fill = Some(fill_of(&a["fill"])?);
    }
    if let Some(p) = poly {
        let [mw, mh] = crate::args::map_size(app).await?;
        if p.iter().any(|q| !(0.0..mw).contains(&q[0]) || !(0.0..mh).contains(&q[1])) {
            return Err(format!("poly: a corner is off the map, which is {mw} by {mh} ft"));
        }
        let n = p.len().max(1) as f64;
        (c.x, c.y) = (p.iter().map(|q| q[0]).sum::<f64>() / n, p.iter().map(|q| q[1]).sum::<f64>() / n);
        c.poly = p;
    }
    if let Some(n) = a["name"].as_str() {
        c.name = n.trim().to_string();
    }
    c.check()?;
    let other = c.clone();
    let mine = c.id.clone();
    let hit = app
        .with_edits(|e| e.created.iter().find(|o| o.kind == "plot" && !o.removed && o.id != mine && !(creating && other.same_site(o, [other.x, other.y])) && agent::polys_overlap_pub(&o.poly, &other.poly)).map(|o| o.id.clone()))
        .flatten();
    if let Some(o) = hit {
        return Err(format!("that outline meets the parcel {o}: change or delete_feature that first"));
    }
    if let Some(why) = app.worker.with({ let p = c.poly.clone(); move |ex| Ok(placing::parcel_blocker(&ex.world, &ex.t0, &p)) }).await? {
        return Err(why);
    }
    let change = app
        .edit("agent", 0, move |e| {
            if creating {
                if let Some(o) = e.existing_site(&c, [c.x, c.y]) {
                    return Ok(json!({ "id": o.id, "existing": true }));
                }
                c.id = format!("c:{}", e.created.len());
                let id = c.id.clone();
                e.created.push(c);
                Ok(json!({ "id": id }))
            } else {
                let slot = e.created.iter_mut().find(|x| x.id == c.id && !x.removed).ok_or_else(|| format!("{} was deleted meanwhile", c.id))?;
                let id = c.id.clone();
                *slot = c;
                Ok(json!({ "id": id }))
            }
        })
        .await?;
    let id = change["id"].as_str().unwrap_or_default().to_string();
    let mut v = if change["existing"].as_bool().unwrap_or(false) { found(app, id).await? } else { crate::tools::read_back(app, id).await? };
    v["tool"] = json!(if creating { "create_parcel" } else { "update_parcel" });
    Ok(v)
}
