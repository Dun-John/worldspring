//! MCP tools for crossings put down by hand (`Edits::crossings`): bridges, fords and ferries.

use serde_json::{Value, json};
use worldgen::roadids::crossing_json;
use worldgen::world::{Crossing, CrossingKind, CrossingMaterial};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{listed, schema, text};

pub fn list() -> Vec<Value> {
    vec![
        json!({ "name": "list_crossings", "title": "Crossings", "description": "The crossings put down by hand (bridges, fords, ferries) with id, kind, ends, width, length, material, name and notes. Returns {items, found}; limit (default 200) cuts it and sets truncated.", "inputSchema": schema(json!({ "limit": crate::tools::limit_prop(200, 2000) }), &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "place_crossing", "title": "Put a crossing down", "description": "Put a bridge, ford or ferry from one bank to the other (world ft), 10 to 2000 ft long (a ferry at least 68), 5 to 40 ft wide. With id, changes that crossing. Returns its id (v:...) and the river's banks there; an end in the water is refused.", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "A crossing to change" },
            "kind": { "type": "string", "enum": ["bridge", "ford", "ferry"] },
            "from": crate::args::point_schema("One bank (a place)"),
            "to": crate::args::point_schema("The other bank (a place); the crossing"),
            "width_ft": { "type": "number", "minimum": 5, "maximum": 40, "default": 12, "description": "Deck, ford or raft width" },
            "material": { "type": "string", "enum": ["timber", "stone"] },
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
    let [w, h] = app.worker.with(|ex| Ok([ex.world.geom.map_w_ft, ex.world.geom.map_h_ft])).await?;
    let mut v = app.edit("agent", 0, move |e| {
        let changed = id.is_some();
        // The same crossing again (a script run twice): one of that kind with both ends within 5 ft of this one's, either way round.
        if let (None, Some(k), Some(f), Some(t)) = (&id, kind, from, to) {
            let near = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) <= 5.0;
            if let Some((eid, c)) = e.crossings.iter().find(|(_, c)| (near(c.a, f) && near(c.b, t)) || (near(c.a, t) && near(c.b, f))) {
                let differs = c.kind != k || (k == CrossingKind::Bridge && material.is_some_and(|m| m != c.material)) || width.is_some_and(|w| (w - c.width).abs() > 1e-6);
                if differs {
                    let m = if c.kind == CrossingKind::Bridge { format!("{:?}", c.material).to_lowercase() } else { "timber".to_string() };
                    return Err(format!("{eid} already joins these banks (a {}, {m}, {} ft): give its id to change it", c.kind.name(), c.width));
                }
                return Ok(json!({
                    "tool": "place_crossing", "id": eid, "kind": c.kind.name(), "changed": false, "existing": true,
                    "note": "a crossing of this kind between these ends was already there: nothing was added (give its id to change it)",
                    "crossing": crossing_json(e, eid, c),
                }));
            }
        }
        let (id, mut c) = match id {
            Some(id) => {
                let c = e.crossings.get(&id).cloned().ok_or_else(|| format!("no crossing {id}"))?;
                (id, c)
            }
            None => {
                let (Some(kind), Some(a), Some(b)) = (kind, from, to) else { return Err("a new crossing needs kind, from and to".into()) };
                (new_id("v"), Crossing { kind, a, b, width: 12.0, material: CrossingMaterial::Timber })
            }
        };
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
        Ok(json!({ "tool": "place_crossing", "id": id, "kind": c.kind.name(), "changed": changed, "crossing": out }))
    })
    .await?;
    banks(app, &mut v).await?;
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
        for id in &ids {
            e.crossings.remove(id);
        }
        Ok(json!({ "tool": "remove_crossings", "ids": ids }))
    })
    .await
}
