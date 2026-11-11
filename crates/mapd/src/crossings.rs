//! MCP tools for crossings put down by hand (`Edits::crossings`): bridges, fords and ferries.

use serde_json::{Value, json};
use worldgen::roadids::crossing_json;
use worldgen::wharf;
use worldgen::world::{Crossing, CrossingKind, CrossingMaterial, Edits};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{listed, schema, text};

pub fn list() -> Vec<Value> {
    vec![
        json!({ "name": "list_crossings", "title": "Crossings", "description": "The crossings put down by hand (bridges, fords, ferries) with id, kind, ends, width, length, material, name and notes. Returns {items, found}; limit (default 200) cuts it and sets truncated.", "inputSchema": schema(json!({ "limit": crate::tools::limit_prop(200, 2000) }), &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "place_crossing", "title": "Put a crossing down", "description": "Put a bridge, ford or ferry from one bank to the other (world ft), 10-2000 ft long (a ferry 68+), 5-80 ft wide (a ford or ferry 40 at most); a bridge may carry shops. With id, changes that crossing. Returns its id (v:...), shops and the river's banks; an end in the water is refused.", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "A crossing to change" },
            "kind": { "type": "string", "enum": ["bridge", "ford", "ferry"] },
            "from": crate::args::point_schema("One bank (a place)"),
            "to": crate::args::point_schema("The other bank (a place); the crossing"),
            "width_ft": { "type": "number", "minimum": 5, "maximum": 80, "default": 12, "description": "Deck, ford or raft width (a ford or ferry at most 40)" },
            "material": { "type": "string", "enum": ["timber", "stone"] },
            "shops": { "type": ["boolean", "object"], "description": "A bridge's shops: true or {depth_ft, frontage_ft, lane_ft}; false takes them off", "properties": {
                "depth_ft": { "type": "number", "minimum": 10, "maximum": 30 }, "frontage_ft": { "type": "number", "minimum": 12, "maximum": 40 }, "lane_ft": { "type": "number", "minimum": 10, "maximum": 30 } } },
        }), &[]) }),
        json!({ "name": "remove_crossings", "title": "Take crossings away", "description": "Take crossings put down by hand off the map, by id or ids (v:..., all or none).", "inputSchema": schema(crate::args::ids_props("crossing id"), &[]) }),
    ]
}

/// Run a crossing tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "list_crossings" => list_crossings(app, crate::tools::limit_of(a, 200)).map(text),
        "place_crossing" => place(app, a).await.map(text),
        "remove_crossings" => remove(app, a).await.map(text),
        _ => return None,
    })
}

fn list_crossings(app: &Shared, limit: usize) -> Result<Value, String> {
    Ok(listed(json!(app.with_edits(|e| e.crossings.iter().map(|(id, c)| crossing_json(e, id, c)).collect::<Vec<_>>()).unwrap_or_default()), limit, json!({})))
}

/// An end in a river's water is refused, naming the bank on its side.
/// Only one clearly in the water (more than `tol` ft past the bank, a fixed 5) is: fords, ferries and app-placed ends a few ft in pass.
async fn dry_end(app: &Shared, k: &str, p: Option<[f64; 2]>, tol: f64) -> Result<(), String> {
    let Some(p) = p else { return Ok(()) };
    let k = k.to_string();
    app.worker
        .with(move |ex| match worldgen::agent::river_at(&ex.world, &ex.t0, p, 50.0).filter(|r| 0.5 * r.width_ft - r.dist_ft > tol) {
            Some(r) => {
                let (b, _) = r.bank_of(p, 5.0);
                let what = r.name.clone().map_or_else(|| "a river".to_string(), |n| format!("{n} ({})", r.id.clone().unwrap_or_default()));
                Err(format!("{k}: ({}, {}) is in the water of {what}, {} ft wide there: its bank on that side is at about ({}, {}); put the end there or farther out", p[0].round(), p[1].round(), r.width_ft.round(), b[0].round(), b[1].round()))
            }
            None => Ok(()),
        })
        .await
}

/// Where the banks are for a crossing's reply: the river it crosses (id, name, width there, the
/// banks across it, the one on `from`'s side first) and how far each end is from its bank (ft;
/// beyond it, as an end should be). Null where no river is near.
async fn banks(app: &Shared, v: &mut Value) -> Result<(), String> {
    let pt = |x: &Value| Some([x[0].as_f64()?, x[1].as_f64()?]);
    let (Some(a), Some(b)) = (pt(&v["crossing"]["from"]), pt(&v["crossing"]["to"])) else { return Ok(()) };
    let r = app
        .worker
        .with(move |ex| {
            let m = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
            Ok(worldgen::agent::river_at(&ex.world, &ex.t0, m, 0.5 * geom_len(a, b) + 1000.0).map(|r| {
                let (fa, da) = r.bank_of(a, 0.0);
                let (fb, db) = r.bank_of(b, 0.0);
                let mut o = json!({ "river": r.json(Some(a)), "from_bank_ft": da.round(), "to_bank_ft": db.round() });
                if geom_len(fa, fb) < 1.0 {
                    o["note"] = json!("both ends are on the same side of the river");
                }
                o
            }))
        })
        .await?;
    match r {
        Some(o) => {
            for (k, x) in o.as_object().into_iter().flatten() {
                v[k] = x.clone();
            }
        }
        None => v["river"] = Value::Null,
    }
    Ok(())
}

fn geom_len(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

async fn end(app: &Shared, a: &Value, k: &str) -> Result<Option<[f64; 2]>, String> {
    match &a[k] {
        Value::Null => Ok(None),
        v => crate::args::point(app, k, v).await.map(Some),
    }
}

/// The `shops` argument: not given, `false` (`Some(None)`: take them off), or the rows.
fn shops_arg(a: &Value) -> Result<Option<Option<wharf::Rows>>, String> {
    let get = |o: &Value, k: &str, d: f64, r: std::ops::RangeInclusive<f64>| -> Result<f64, String> {
        match &o[k] {
            Value::Null => Ok(d),
            v => v.as_f64().filter(|x| r.contains(x)).ok_or_else(|| format!("shops.{k}: {} to {}", r.start(), r.end())),
        }
    };
    match &a["shops"] {
        Value::Null => Ok(None),
        Value::Bool(false) => Ok(Some(None)),
        v => Ok(Some(Some(wharf::Rows {
            depth: get(v, "depth_ft", wharf::SHOP_DEPTH_FT, 10.0..=30.0)?,
            frontage: get(v, "frontage_ft", wharf::SHOP_FRONT_FT, 12.0..=40.0)?,
            lane: get(v, "lane_ft", wharf::SHOP_LANE_FT, 10.0..=30.0)?,
            lane_given: !v["lane_ft"].is_null(),
        }))),
    }
}

/// Retire the shops on bridge `id`: their ids.
fn retire_shops(e: &mut Edits, id: &str) -> Vec<String> {
    let mut out = Vec::new();
    for c in e.created.iter_mut().filter(|c| c.on.as_deref() == Some(id) && !c.removed && c.kind == "building") {
        c.removed = true;
        out.push(c.id.clone());
    }
    out
}

/// The ids of the shops (buildings drawn on) bridge `id` now.
fn live_shops(e: &Edits, id: &str) -> Vec<String> {
    e.created.iter().filter(|c| c.on.as_deref() == Some(id) && !c.removed && c.kind == "building").map(|c| c.id.clone()).collect()
}

/// Each new shop checked as a building (named for its trade): one that cannot stand is retired. The ones skipped, with why.
async fn vet_shops(app: &Shared, ids: &[String]) -> Result<Vec<Value>, String> {
    let list: Vec<(String, Vec<[f64; 2]>, Option<String>)> = app
        .with_edits(|e| e.created.iter().filter(|c| ids.contains(&c.id) && c.name.is_empty()).map(|c| (c.id.clone(), c.poly.clone(), c.func.clone())).collect())
        .unwrap_or_default();
    let checked = app
        .worker
        .with(move |ex| {
            Ok(list
                .into_iter()
                .map(|(id, poly, func)| {
                    let skip = worldgen::agent::spot_skip(&ex.world, &ex.t0, &id);
                    let r = worldgen::agent::building_spot(&ex.world, &ex.t0, &poly, func.as_deref(), &id, skip).map(|s| s.1);
                    (id, r)
                })
                .collect::<Vec<_>>())
        })
        .await?;
    if checked.iter().all(|(_, r)| r.is_ok()) {
        let names: Vec<(String, String)> = checked.into_iter().filter_map(|(i, r)| r.ok().map(|n| (i, n))).collect();
        app.edit("agent", 0, move |e| {
            for (i, n) in names {
                if let Some(c) = e.created.iter_mut().find(|c| c.id == i) {
                    c.name = n;
                }
            }
            Ok(json!({}))
        })
        .await?;
        return Ok(Vec::new());
    }
    let skipped: Vec<Value> = checked.iter().filter_map(|(i, r)| r.as_ref().err().map(|why| json!({ "id": i, "why": why }))).collect();
    app.edit("agent", 0, move |e| {
        for (i, r) in checked {
            if let Some(c) = e.created.iter_mut().find(|c| c.id == i) {
                match r {
                    Ok(n) => c.name = n,
                    Err(_) => c.removed = true,
                }
            }
        }
        Ok(json!({}))
    })
    .await?;
    Ok(skipped)
}

async fn place(app: &Shared, a: &Value) -> Result<Value, String> {
    let kind = match a["kind"].as_str() {
        None => None,
        Some("bridge") => Some(CrossingKind::Bridge),
        Some("ford") => Some(CrossingKind::Ford),
        Some("ferry") => Some(CrossingKind::Ferry),
        Some(k) => return Err(format!("kind: bridge, ford or ferry, not {k}")),
    };
    let material = match a["material"].as_str() {
        None => None,
        Some("timber") => Some(CrossingMaterial::Timber),
        Some("stone") => Some(CrossingMaterial::Stone),
        Some(m) => return Err(format!("material: timber or stone, not {m}")),
    };
    let (from, to, width) = (end(app, a, "from").await?, end(app, a, "to").await?, a["width_ft"].as_f64());
    let shops = shops_arg(a)?;
    let rows = shops.flatten();
    let id = a["id"].as_str().map(str::to_string);
    // A repeat of a crossing already there is reported by `edit` below whatever its ends stand in.
    let repeat = match (&id, kind, from, to) {
        (None, Some(_), Some(f), Some(t)) => app
            .with_edits(move |e| {
                let near = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) <= 5.0;
                e.crossings.values().any(|c| (near(c.a, f) && near(c.b, t)) || (near(c.a, t) && near(c.b, f)))
            })
            .unwrap_or(false),
        _ => false,
    };
    if !repeat {
        let tol = 5.0;
        dry_end(app, "from", from, tol).await?;
        dry_end(app, "to", to, tol).await?;
    }
    // Shops only on a bridge over water (`wharf::shops_problem`): its ends as given, else as they stand.
    if rows.is_some() {
        let ends = match (from, to) {
            (Some(f), Some(t)) => Some((f, t)),
            _ => id.as_ref().and_then(|i| app.with_edits(|e| e.crossings.get(i).map(|c| (from.unwrap_or(c.a), to.unwrap_or(c.b)))).flatten()),
        };
        if let Some((a, b)) = ends {
            let c = Crossing { kind: CrossingKind::Bridge, a, b, width: 50.0, material: CrossingMaterial::Timber };
            if let Some(p) = app.worker.with(move |ex| Ok(wharf::shops_problem(&ex.t0, &c))).await? {
                return Err(p);
            }
        }
    }
    let [w, h] = app.worker.with(|ex| Ok([ex.world.geom.map_w_ft, ex.world.geom.map_h_ft])).await?;
    let mut v = app.edit("agent", 0, move |e| {
        let changed = id.is_some();
        // The same crossing again (a script run twice): one of that kind with both ends within 5 ft of this one's, either way round.
        if let (None, Some(k), Some(f), Some(t)) = (&id, kind, from, to) {
            let near = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) <= 5.0;
            let hit = e.crossings.iter().find(|(_, c)| (near(c.a, f) && near(c.b, t)) || (near(c.a, t) && near(c.b, f))).map(|(i, c)| (i.clone(), c.clone()));
            if let Some((eid, c)) = hit {
                let differs = c.kind != k || (k == CrossingKind::Bridge && material.is_some_and(|m| m != c.material)) || width.is_some_and(|w| (w - c.width).abs() > 1e-6);
                if differs {
                    let m = if c.kind == CrossingKind::Bridge { format!("{:?}", c.material).to_lowercase() } else { "timber".to_string() };
                    return Err(format!("{eid} already joins these banks (a {}, {m}, {} ft): give its id to change it", c.kind.name(), c.width));
                }
                let mut out = json!({
                    "tool": "place_crossing", "id": eid, "kind": c.kind.name(), "changed": false, "existing": true,
                    "note": "a crossing of this kind between these ends was already there: nothing was added (give its id to change it)",
                    "crossing": crossing_json(e, &eid, &c),
                });
                match rows {
                    Some(r) if live_shops(e, &eid).is_empty() => out["shops"] = json!(wharf::lay_shops(e, &eid, &c, r)?),
                    Some(_) => out["shops"] = json!(live_shops(e, &eid)),
                    None if shops.is_some() => out["shops_retired"] = json!(retire_shops(e, &eid)),
                    None => {}
                }
                return Ok(out);
            }
        }
        let (id, mut c) = match id {
            Some(id) => {
                let c = e.crossings.get(&id).cloned().ok_or_else(|| format!("no crossing {id}"))?;
                (id, c)
            }
            None => {
                let (Some(kind), Some(a), Some(b)) = (kind, from, to) else { return Err("a new crossing needs kind, from and to".into()) };
                (new_id("v"), Crossing { kind, a, b, width: rows.filter(|_| kind == CrossingKind::Bridge).map_or(12.0, |r| r.width()), material: CrossingMaterial::Timber })
            }
        };
        let was = changed.then(|| c.clone());
        if let Some(k) = kind {
            c.kind = k;
        }
        c.a = from.unwrap_or(c.a);
        c.b = to.unwrap_or(c.b);
        c.width = width.unwrap_or(c.width);
        // Only a bridge has a material.
        c.material = if c.kind == CrossingKind::Bridge { material.unwrap_or(c.material) } else { CrossingMaterial::Timber };
        if let Some(p) = c.problem(w, h) {
            return Err(p);
        }
        let out = crossing_json(e, &id, &c);
        e.crossings.insert(id.clone(), c.clone());
        let mut v = json!({ "tool": "place_crossing", "id": id, "kind": c.kind.name(), "changed": changed, "crossing": out });
        match shops {
            Some(Some(r)) => {
                let gone = retire_shops(e, &id);
                v["shops"] = json!(wharf::lay_shops(e, &id, &c, r)?);
                if !gone.is_empty() {
                    v["shops_retired"] = json!(gone);
                }
            }
            Some(None) => v["shops_retired"] = json!(retire_shops(e, &id)),
            // (The shops follow their bridge: laid out again on its new ends and width, or retired.)
            None if changed => {
                let moved = e.follow_crossing(&id, was.as_ref());
                let (gone, laid): (Vec<String>, Vec<String>) = moved.into_iter().partition(|m| e.created.iter().any(|c| &c.id == m && c.removed));
                if !laid.is_empty() {
                    v["shops_relaid"] = json!(laid);
                }
                if !gone.is_empty() {
                    v["shops_retired"] = json!(gone);
                }
            }
            None => {}
        }
        Ok(v)
    })
    .await?;
    banks(app, &mut v).await?;
    // The new shops are checked as buildings (what stands on the bridge's ends or crosses it); one that cannot stand is dropped.
    let fresh: Vec<String> = v["shops"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect();
    if !fresh.is_empty() {
        let skipped = vet_shops(app, &fresh).await?;
        if !skipped.is_empty() {
            let out: Vec<&str> = skipped.iter().filter_map(|s| s["id"].as_str()).collect();
            v["shops"] = json!(fresh.iter().filter(|f| !out.contains(&f.as_str())).collect::<Vec<_>>());
            v["shops_skipped"] = json!(skipped);
        }
    }
    Ok(v)
}

async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::args::ids(a)?;
    let single = a["ids"].is_null();
    app.edit("agent", 0, move |e| {
        for (i, id) in ids.iter().enumerate() {
            if !e.crossings.contains_key(id) {
                return Err(format!("{}: no crossing {id}", crate::args::item(single, i)));
            }
        }
        let mut gone = Vec::new();
        for id in &ids {
            e.crossings.remove(id);
            gone.extend(e.follow_crossing(id, None));
        }
        let mut v = json!({ "tool": "remove_crossings", "ids": ids });
        if !gone.is_empty() {
            v["shops_retired"] = json!(gone);
        }
        Ok(v)
    })
    .await
}
