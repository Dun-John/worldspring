//! MCP tools for water drawn by hand (`Edits::waters`, `e:<id>`): inlets, basins, lakes, ponds
//! and moats, each an outline and a level (`worldgen::waters`).

use serde_json::{Value, json};
use worldgen::geoquery::Squares;
use worldgen::pipeline::Executor;
use worldgen::town::geom;
use worldgen::waters::{self, Water, WaterKind};
use worldgen::world::Edits;

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{listed, schema, text};

/// Points along an outline read for its default level, at most.
const LEVEL_SAMPLES: usize = 96;

pub fn list() -> Vec<Value> {
    let pt = crate::args::point_schema("A point: {x_ft, y_ft} or [x_ft, y_ft] (world ft)");
    let kinds: Vec<&str> = WaterKind::ALL.iter().map(|k| k.name()).collect();
    let props = |with_id: bool| {
        let mut p = json!({
            "poly": { "type": "array", "minItems": 3, "maxItems": waters::MAX_WATER_PTS, "items": pt.clone(), "description": "The outline (world ft, 5 miles across at most)" },
            "kind": { "type": "string", "enum": kinds.clone(), "description": "inlet and basin: the sea's, reaching in; lake, pond, moat: standing water (default lake)" },
            "level_ft": { "type": "number", "minimum": -20000, "maximum": 20000, "description": "Its surface, ft above the sea. Default: the sea's or a lake's where the outline reaches one, an inlet's or basin's the sea's, else 1 ft under the lowest ground on its shore" },
            "name": { "type": "string", "description": "Its name on the map (empty: none)" },
        });
        if with_id {
            p["id"] = json!({ "type": "string", "description": "The water to change (e:...)" });
        }
        p
    };
    vec![
        json!({ "name": "create_water", "title": "Draw water", "description": "Water inside an outline drawn by hand: an inlet or basin of the sea, a lake, pond or moat. The ground inside falls under its level (the battlemap's water, the map's at every zoom); an outline reaching the sea or a lake joins it; overlapping outlines are one water with one shore. Buildings, walls and crossings' ends keep out of it; a plateau or islet drawn over it stands dry. Returns its id (e:...).", "inputSchema": schema(props(false), &["poly"]) }),
        json!({ "name": "update_water", "title": "Change water", "description": "Change water drawn by hand (e:...): its outline, kind, level or name; what is not given stays.", "inputSchema": schema(props(true), &["id"]) }),
        json!({ "name": "delete_water", "title": "Take water away", "description": "Take water drawn by hand away (the ground goes back as it was), by id (e:...), with its name, note and hidden mark (no restore; the user's Undo in the app brings it back).", "inputSchema": schema(crate::args::ids_props("water id (e:...)"), &[]) }),
        json!({ "name": "list_waters", "title": "Water drawn by hand", "description": "The water drawn by hand (e:...): id, kind, level_ft, name, middle, bbox, area and outline. Returns {items, found}; limit (default 100) cuts it.", "inputSchema": schema(json!({ "limit": { "type": "integer", "minimum": 1 } }), &[]) }),
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

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// A water's entry in replies and lists.
fn water_json(e: &Edits, id: &str, w: &Water, sea: f64) -> Value {
    let b = w.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
    let mid = geom::centroid(&w.pts);
    let mut v = json!({
        "id": id, "kind": w.kind.name(), "level_ft": round1(w.level as f64 - sea), "x_ft": mid[0].round(), "y_ft": mid[1].round(),
        "bbox": b.map(f64::round), "area_sq_ft": geom::area(&w.pts).abs().round(), "points": w.pts.len(), "poly": w.pts,
    });
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
/// (the highest), else the sea's for an inlet or basin, else 1 ft under the lowest ground on its
/// shore as shaped by `others` (ft, the generator's datum). And what it joins.
fn default_level(ex: &mut Executor, pts: &[[f64; 2]], kind: WaterKind, others: &Edits) -> (f64, Option<&'static str>) {
    let sea = ex.world.params().sea_level_ft;
    let n = pts.len();
    // The corners and points between them, at most `LEVEL_SAMPLES`.
    let per = (LEVEL_SAMPLES / n).max(1);
    let probes: Vec<[f64; 2]> = (0..n).flat_map(|i| (0..per).map(move |s| geom::lerp(pts[i], pts[(i + 1) % n], s as f64 / per as f64))).take(LEVEL_SAMPLES.max(n)).collect();
    let (mut wet, mut low) = (f64::MIN, f64::MAX);
    for p in &probes {
        let sq = Squares::round(ex, [p[0], p[1], p[0], p[1]], 0);
        let g = sq.shaped(others, Some(&ex.t0))[0] as f64;
        let w = sq.still[0] as f64;
        if w > worldgen::t0::hydro::DRY as f64 && w > g {
            wet = wet.max(w);
        }
        low = low.min(g);
    }
    if wet > f64::MIN {
        return (wet, Some(if wet <= sea + 1.0 { "sea" } else { "lake" }));
    }
    if kind.of_sea() { (sea, None) } else { ((low - 1.0).max(sea), None) }
}

async fn set(app: &Shared, a: &Value, change: bool) -> Result<Value, String> {
    let id = if change { Some(a["id"].as_str().ok_or("id: the water to change (e:...)")?.to_string()) } else { None };
    let old = match &id {
        Some(id) => Some(app.with_edits(|e| e.waters.get(id).cloned()).flatten().ok_or_else(|| format!("no water {id}"))?),
        None => None,
    };
    let kind = match &a["kind"] {
        Value::Null => old.as_ref().map_or(WaterKind::Lake, |w| w.kind),
        Value::String(k) => WaterKind::parse(k).ok_or("kind: inlet, basin, lake, pond or moat")?,
        _ => return Err("kind: inlet, basin, lake, pond or moat".into()),
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
        None => return Err("poly: an outline, 3 or more points".into()),
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
    let keep = level_ft.is_none() && poly.is_none() && old.as_ref().is_some_and(|w| w.kind.of_sea() == kind.of_sea());
    let me = id.clone();
    let (sea, level, joins) = app
        .worker
        .run(move |g| {
            let ex = g.ex.as_mut().ok_or("no world is open: open the map app (it connects to mapd and shares its world)")?;
            let (mw, mh) = (ex.world.geom.map_w_ft, ex.world.geom.map_h_ft);
            if pts.iter().any(|p| !(0.0..mw).contains(&p[0]) || !(0.0..mh).contains(&p[1])) {
                return Err("the outline is off the map".to_string());
            }
            let sea = ex.world.params().sea_level_ft;
            let mut others = ex.world.file.edits.clone();
            if let Some(me) = &me {
                others.waters.remove(me);
            }
            let (found, joins) = default_level(ex, &pts, kind, &others);
            Ok::<_, String>((sea, level_ft.map_or(found, |l| l + sea), joins))
        })
        .await?;
    app.edit("agent", 0, move |e| {
        let old = match &id {
            Some(id) => Some(e.waters.get(id).cloned().ok_or_else(|| format!("no water {id}"))?),
            None => None,
        };
        let pts = poly.clone().unwrap_or_else(|| old.as_ref().map(|w| w.pts.clone()).unwrap_or_default());
        let level = if keep { old.as_ref().map_or(level, |w| w.level as f64) } else { level };
        let w = Water { pts, kind, level: ((level * 10.0).round() * 0.1) as f32 };
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
