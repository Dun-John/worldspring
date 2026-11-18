//! MCP tools for water drawn by hand (`Edits::waters`, `e:<id>`): a polygon, or a brushed line with
//! a width (a ring when closed), each with a depth, natural or dug, and a level; its look follows
//! (`worldgen::waters::Form::look`). `kind` (inlet, basin, lake, pond, moat, stream) is still taken
//! from older callers and kept on the record (`worldgen::waters`).

use serde_json::{Value, json};
use worldgen::geoquery::Squares;
use worldgen::pipeline::Executor;
use worldgen::town::geom;
use worldgen::waters::{self, Water, WaterKind};
use worldgen::world::Edits;

use crate::Shared;
use crate::args::round1;
use crate::notebook::new_id;
use crate::tools::{listed, schema, text};

/// Points along an outline read for its default level, at most.
const LEVEL_SAMPLES: usize = 96;

pub fn list() -> Vec<Value> {
    let pt = crate::args::point_schema("{x_ft, y_ft} or [x_ft, y_ft]");
    let kinds: Vec<&str> = WaterKind::ALL.iter().map(|k| k.name()).collect();
    let props = |with_id: bool| {
        let mut p = json!({
            "poly": { "type": "array", "minItems": 2, "maxItems": waters::MAX_WATER_PTS, "items": pt.clone(), "description": "Points (world ft): polygon 3+, brush 2+" },
            "shape": { "type": "string", "enum": ["polygon", "brush"], "description": "brush: a band width_ft wide along poly (default polygon)" },
            "width_ft": { "type": "number", "minimum": waters::STREAM_WIDTHS[0], "maximum": waters::BRUSH_WIDTHS[1], "description": "Brush width (default 10)" },
            "closed": { "type": "boolean", "description": "Brush comes round to its start: a ring" },
            "depth_ft": { "type": "number", "minimum": waters::DEPTHS[0], "maximum": waters::DEPTHS[1], "description": "Default 4" },
            "dug": { "type": "boolean", "description": "Straight sides, stone edge" },
            "kind": { "type": "string", "enum": kinds.clone(), "description": "Legacy fixed look; leave out" },
            "level_ft": { "type": "number", "minimum": -20000, "maximum": 20000, "description": "Surface, ft above the sea (default: the sea's or a lake's it reaches, else under its shore)" },
            "name": { "type": "string", "description": "Its name on the map (empty: none)" },
        });
        if with_id {
            p["id"] = json!({ "type": "string", "description": "The water to change (e:...)" });
        }
        p
    };
    vec![
        json!({ "name": "create_water", "title": "Draw water", "description": "Water drawn by hand: a polygon or brushed band. Its look follows it: the sea's at sea level (dug: a quay), an open band a stream running downhill, a ring a moat, else a pond or lake by depth. Returns id (e:...), look as kind.", "inputSchema": schema(props(false), &["poly"]) }),
        json!({ "name": "update_water", "title": "Change water", "description": "Change water drawn by hand (e:...); what is not given stays.", "inputSchema": schema(props(true), &["id"]) }),
        json!({ "name": "delete_water", "title": "Take water away", "description": "Take water drawn by hand away (the ground goes back as it was), by id (e:...), with its name, note and hidden mark (no restore; the user's Undo in the app brings it back).", "inputSchema": schema(crate::args::ids_props("water id (e:...)"), &[]) }),
        json!({ "name": "list_waters", "title": "Water drawn by hand", "description": "Water drawn by hand (e:...) as create_water returns it. Returns {items, found}; limit (default 100).", "inputSchema": schema(json!({ "limit": { "type": "integer", "minimum": 1 } }), &[]) }),
    ]
}

/// Run a water tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "create_water" => set(app, a, false).await.map(text),
        "update_water" => set(app, a, true).await.map(text),
        "delete_water" => delete(app, a).await.map(text),
        "list_waters" => list_waters(app, crate::tools::limit_of(a, 100)).await.map(text),
        _ => return None,
    })
}

/// A water's entry in replies and lists.
fn water_json(e: &Edits, id: &str, w: &Water, sea: f64) -> Value {
    // (Where it lies by its shore as shaped; `poly` the points as drawn.)
    let shore = w.shore();
    let b = shore.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
    let mid = geom::centroid(&shore);
    let mut v = json!({
        "id": id, "kind": w.look(sea).kind.name(), "shape": if w.is_line() { "brush" } else { "polygon" }, "depth_ft": w.depth_ft(), "dug": w.is_dug(),
        "level_ft": round1(w.level as f64 - sea), "x_ft": mid[0].round(), "y_ft": mid[1].round(),
        "bbox": b.map(f64::round), "area_sq_ft": geom::area(&shore).abs().round(), "points": w.pts.len(), "poly": w.pts,
    });
    if let Some(k) = w.kind {
        v["legacy_kind"] = json!(k.name());
    }
    if w.is_line() {
        v["line"] = json!(true);
        v["width_ft"] = json!(w.width_ft());
        v["closed"] = json!(w.closed);
        if let (Some(a), Some(z)) = (w.levels.first(), w.levels.last()) {
            v["level_ft"] = json!(round1(a.max(*z) as f64 - sea));
            v["falls_to_ft"] = json!(round1(a.min(*z) as f64 - sea));
        }
    }
    if let Some(n) = e.renames.get(id) {
        v["name"] = json!(n);
    }
    v
}

async fn list_waters(app: &Shared, limit: usize) -> Result<Value, String> {
    let items = app
        .worker
        .with(|ex| {
            let (e, sea) = (&ex.world.file.edits, ex.world.params().sea_level_ft);
            Ok(e.waters.iter().map(|(id, w)| water_json(e, id, w, sea)).collect::<Vec<_>>())
        })
        .await?;
    Ok(listed(json!(items), limit, json!({})))
}

/// The level an outline takes when none is given: the sea's or a lake's where it reaches one
/// (the highest), else the sea's for a legacy inlet or basin, else 1 ft under the lowest ground on
/// its shore (`Water::shore`) as shaped by `others` (ft, the generator's datum). And what it
/// joins. Water with no kind reaches the sea within `SEA_NEAR_FT` of its shore (read out there too)
/// and then stands at the sea's level exactly; else never within `waters::SEA_WITHIN_FT` of it
/// (its look the sea's only where it joins the sea: `waters::Form::look`).
fn default_level(ex: &mut Executor, w: &Water, others: &Edits) -> (f64, Option<&'static str>) {
    let legacy = w.legacy();
    let sea = ex.world.params().sea_level_ft;
    let shore = w.shore();
    let every = shore.len().div_ceil(LEVEL_SAMPLES).max(1);
    let sampled: Vec<[f64; 2]> = shore.iter().step_by(every).copied().collect();
    let (pts, n) = (sampled.as_slice(), sampled.len());
    // The corners and points between them, at most `LEVEL_SAMPLES`; with no kind, each also
    // `SEA_NEAR_FT` out from the shore (to the right of it when its area is positive).
    let per = (LEVEL_SAMPLES / n).max(1);
    let out = if legacy.is_none() { SEA_NEAR_FT * if geom::area(pts) > 0.0 { -1.0 } else { 1.0 } } else { 0.0 };
    let probes: Vec<([f64; 2], [f64; 2])> = (0..n)
        .flat_map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let l = geom::dist(a, b).max(1e-9);
            let o = [(b[1] - a[1]) / l * out, -(b[0] - a[0]) / l * out];
            (0..per).map(move |s| {
                let p = geom::lerp(a, b, s as f64 / per as f64);
                (p, [p[0] + o[0], p[1] + o[1]])
            })
        })
        .take(LEVEL_SAMPLES.max(n))
        .collect();
    let (mut wet, mut low) = (f64::MIN, f64::MAX);
    for (p, q) in &probes {
        for (k, r) in [p, q].into_iter().enumerate() {
            if k == 1 && out == 0.0 {
                continue;
            }
            let sq = Squares::round(ex, [r[0], r[1], r[0], r[1]], 0);
            let g = sq.shaped(others, Some(&ex.t0))[0] as f64;
            let w = sq.still[0] as f64;
            if w > worldgen::t0::hydro::DRY as f64 && w > g {
                wet = wet.max(w);
            }
            if k == 0 {
                low = low.min(g);
            }
        }
    }
    if wet > f64::MIN {
        let of_sea = wet <= sea + 1.0;
        return (if of_sea && legacy.is_none() { sea } else { wet }, Some(if of_sea { "sea" } else { "lake" }));
    }
    match legacy {
        Some(k) if k.of_sea() => (sea, None),
        Some(_) => ((low - 1.0).max(sea), None),
        None => ((low - 1.0).max(sea + SEA_CLEAR_FT), None),
    }
}

/// Water with no kind reaches the sea where the sea is this near its shore (ft).
const SEA_NEAR_FT: f64 = 4.0;
/// Water with no kind that doesn't reach the sea stands at least this over its level (ft).
const SEA_CLEAR_FT: f64 = 0.5;

/// A stream's levels along its line (`waters::stream_levels`), the ground as shaped by `others`.
fn stream_levels(ex: &mut Executor, w: &Water, others: &Edits) -> Vec<f32> {
    let sea = ex.world.params().sea_level_ft;
    let at: Vec<(f64, Option<f64>)> = waters::stream_samples(&w.centre())
        .iter()
        .map(|p| {
            let sq = Squares::round(ex, [p[0], p[1], p[0], p[1]], 0);
            let (g, still) = (sq.shaped(others, Some(&ex.t0))[0] as f64, sq.still[0] as f64);
            (g, (still > worldgen::t0::hydro::DRY as f64 && still > g).then_some(still))
        })
        .collect();
    waters::stream_levels(&at, sea)
}

async fn set(app: &Shared, a: &Value, change: bool) -> Result<Value, String> {
    let id = if change { Some(a["id"].as_str().ok_or("id: the water to change (e:...)")?.to_string()) } else { None };
    let old = match &id {
        Some(id) => Some(app.with_edits(|e| e.waters.get(id).cloned()).flatten().ok_or_else(|| format!("no water {id}"))?),
        None => None,
    };
    let given_kind = match &a["kind"] {
        Value::Null => None,
        Value::String(k) => Some(WaterKind::parse(k).ok_or("kind: inlet, basin, lake, pond, moat or stream (or leave it out)")?),
        _ => return Err("kind: inlet, basin, lake, pond, moat or stream (or leave it out)".into()),
    };
    let flag = |k: &str| match &a[k] {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        _ => Err(format!("{k}: true or false")),
    };
    let brush = match &a["shape"] {
        Value::Null => flag("line")?,
        Value::String(s) if s == "brush" => Some(true),
        Value::String(s) if s == "polygon" => Some(false),
        _ => return Err("shape: polygon or brush".into()),
    };
    let depth_ft = match &a["depth_ft"] {
        Value::Null => None,
        v => Some(v.as_f64().filter(|d| d.is_finite() && (waters::DEPTHS[0]..=waters::DEPTHS[1]).contains(d)).ok_or("depth_ft: 1 to 20 ft")?),
    };
    let dug = flag("dug")?;
    // (A legacy kind: given, or kept from the water changed unless its shape, depth or shore is.)
    let kind = given_kind.or_else(|| old.as_ref().filter(|_| brush.is_none() && depth_ft.is_none() && dug.is_none()).and_then(|w| w.kind));
    let line = match kind {
        Some(k) => brush.unwrap_or_else(|| old.as_ref().is_some_and(|w| w.line)) || k == WaterKind::Stream,
        None => brush.unwrap_or_else(|| old.as_ref().is_some_and(|w| w.is_line())),
    };
    if let Some(k) = kind
        && line
        && !k.may_be_line()
    {
        return Err("line: only a moat or a stream is drawn as a line (or leave kind out)".into());
    }
    let closed = line && kind != Some(WaterKind::Stream) && flag("closed")?.unwrap_or_else(|| old.as_ref().is_some_and(|w| w.closed));
    let width = match &a["width_ft"] {
        Value::Null if kind.is_some() => old.as_ref().filter(|w| w.kind == kind).and_then(|w| w.width),
        Value::Null => old.as_ref().filter(|w| w.is_line()).map(|w| w.width_ft().clamp(waters::BRUSH_WIDTHS[0], waters::BRUSH_WIDTHS[1])),
        v => Some(v.as_f64().filter(|w| w.is_finite()).ok_or("width_ft: a number of ft")?),
    };
    let width = width.filter(|_| line);
    // (Its depth and shore: given, else as it was, else 4 ft and natural; a legacy kind's own
    // unless given.)
    let depth = if kind.is_some() { depth_ft.or(old.as_ref().filter(|w| w.kind == kind).and_then(|w| w.depth)) } else { Some(depth_ft.or(old.as_ref().map(|w| w.depth_ft())).unwrap_or(waters::DEPTH_FT)) };
    let dug = if kind.is_some() { dug.or(old.as_ref().filter(|w| w.kind == kind).and_then(|w| w.dug)) } else { Some(dug.or(old.as_ref().map(|w| w.is_dug())).unwrap_or(false)) };
    let make = move |pts: Vec<[f64; 2]>, level: f32| {
        let mut w = match kind {
            Some(k) if line => Water::of_line(pts, k, level, width, closed),
            Some(k) => Water::outline(pts, k, level),
            None => Water::drawn(pts, level, line, width, closed, waters::DEPTH_FT, false),
        };
        w.depth = depth;
        w.dug = dug;
        w
    };
    let poly: Option<Vec<[f64; 2]>> = match a["poly"].as_array() {
        Some(o) => {
            let mut pts = Vec::new();
            for (i, p) in o.iter().enumerate() {
                pts.push(crate::args::point(app, &format!("poly[{i}]"), p).await?);
            }
            Some(waters::rounded(&pts))
        }
        None if a["poly"].is_null() => None,
        None => return Err("poly: an outline, 3 or more points (a line: 2 or more)".into()),
    };
    if poly.is_none() && old.is_none() {
        return Err("poly: the water's outline".into());
    }
    let level_ft = match &a["level_ft"] {
        Value::Null => None,
        v => Some(v.as_f64().filter(|l| l.is_finite() && l.abs() <= 20_000.0).ok_or("level_ft: within 20,000 ft of the sea")?),
    };
    let name = match &a["name"] {
        Value::Null => None,
        Value::String(n) => Some(n.trim().to_string()),
        _ => return Err("name: text".into()),
    };
    let pts = poly.clone().unwrap_or_else(|| old.as_ref().map(|w| w.pts.clone()).unwrap_or_default());
    // The level: given, kept (the outline and kind as they were), or found along the new shore.
    let shaped = make(pts.clone(), 0.0);
    let of_sea = |k: Option<WaterKind>| k.is_some_and(|k| k.of_sea());
    let keep = level_ft.is_none() && poly.is_none() && old.as_ref().is_some_and(|w| of_sea(w.legacy()) == of_sea(shaped.legacy()) && w.is_line() == line && w.closed == closed && w.width_ft() == shaped.width_ft() && w.flows() == shaped.flows());
    let me = id.clone();
    let (sea, level, joins, levels) = app
        .worker
        .with_mut(move |ex| {
            let (mw, mh) = (ex.world.geom.map_w_ft, ex.world.geom.map_h_ft);
            if pts.iter().any(|p| !(0.0..mw).contains(&p[0]) || !(0.0..mh).contains(&p[1])) {
                return Err("the outline is off the map".to_string());
            }
            let sea = ex.world.params().sea_level_ft;
            let mut others = ex.world.file.edits.clone();
            if let Some(me) = &me {
                others.waters.remove(me);
            }
            if let Some(err) = shaped.problem() {
                return Err(err);
            }
            let (found, joins) = default_level(ex, &shaped, &others);
            // (A stream's level falls along it, unless one is given: then it is flat.)
            let levels = if shaped.flows() && level_ft.is_none() && !keep { stream_levels(ex, &shaped, &others) } else { Vec::new() };
            let found = levels.iter().copied().reduce(f32::max).map_or(found, |l| l as f64);
            Ok::<_, String>((sea, level_ft.map_or(found, |l| l + sea), joins, levels))
        })
        .await?;
    app.edit("agent", 0, move |e| {
        let old = match &id {
            Some(id) => Some(e.waters.get(id).cloned().ok_or_else(|| format!("no water {id}"))?),
            None => None,
        };
        let pts = poly.clone().unwrap_or_else(|| old.as_ref().map(|w| w.pts.clone()).unwrap_or_default());
        let level = if keep { old.as_ref().map_or(level, |w| w.level as f64) } else { level };
        let level = ((level * 10.0).round() * 0.1) as f32;
        let mut w = make(pts, level);
        w.levels = if keep { old.as_ref().map_or(Vec::new(), |o| o.levels.clone()) } else { levels.clone() };
        if let Some(err) = w.problem() {
            return Err(err);
        }
        if id.is_none() {
            // The same outline again (a script run twice): that water, nothing added.
            if let Some((eid, ew)) = e.waters.iter().find(|(_, q)| q.pts.len() == w.pts.len() && q.pts.iter().zip(&w.pts).all(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= 1.0)) {
                if ew.kind != w.kind || level_ft.is_some() && (ew.level - w.level).abs() > 0.15 {
                    return Err(format!("{eid} has this outline: give its id to change it (update_water)"));
                }
                let mut v = water_json(e, eid, ew, sea);
                v["tool"] = json!("create_water");
                v["changed"] = json!(false);
                v["existing"] = json!(true);
                v["note"] = json!("water with this outline was already there: nothing was added");
                return Ok(v);
            }
            if e.waters.len() >= waters::MAX_WATERS {
                return Err(format!("a world holds at most {} water outlines", waters::MAX_WATERS));
            }
        }
        let key = id.clone().unwrap_or_else(|| new_id("e"));
        match name.as_deref() {
            Some("") => {
                e.renames.remove(&key);
            }
            Some(n) => {
                e.renames.insert(key.clone(), n.to_string());
            }
            None => {}
        }
        let mut v = water_json(e, &key, &w, sea);
        if !keep && level_ft.is_none() {
            v["joins"] = json!(joins);
        }
        v["tool"] = json!(if id.is_some() { "update_water" } else { "create_water" });
        v["changed"] = json!(id.is_some());
        e.waters.insert(key, w);
        Ok(v)
    })
    .await
}

async fn delete(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::args::ids(a)?;
    let single = a["ids"].is_null();
    app.edit("agent", 0, move |e| {
        for (i, id) in ids.iter().enumerate() {
            if !e.waters.contains_key(id) {
                return Err(format!("{}: no water {id}", crate::args::item(single, i)));
            }
        }
        for id in &ids {
            e.waters.remove(id);
            e.renames.remove(id);
            e.notes.remove(id);
            e.hidden.remove(id);
        }
        Ok(json!({ "tool": "delete_water", "ids": ids }))
    })
    .await
}
