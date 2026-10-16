//! MCP tools for crossings put down by hand (`Edits::crossings`): bridges, fords and ferries.

use serde_json::{Value, json};
use worldgen::world::{Crossing, CrossingKind, CrossingMaterial};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{schema, text};

pub fn list() -> Vec<Value> {
    let point = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x_ft, y_ft]" });
    vec![
        json!({ "name": "list_crossings", "title": "Crossings", "description": "The crossings put down by hand (bridges, fords, ferries): id, kind, ends (world ft), width, length and a bridge's material.", "inputSchema": schema(json!({}), &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "place_crossing", "title": "Put a crossing down", "description": "Put a bridge, ford or ferry on the map from one bank to the other (world ft), drawn on the battlemap over what is there: a bridge's deck clear of the water (walkable; timber planks, or old stone with material \"stone\"), a ford's bed brought up to wading depth under stepping stones, a ferry's jetties out from each point with a raft on a rope between. 10 to 2000 ft long (a ferry at least 68), 5 to 40 ft wide. With id, changes that crossing (fields left out stay). Returns its id (v:...).", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "A crossing to change" },
            "kind": { "type": "string", "enum": ["bridge", "ford", "ferry"] },
            "from": point,
            "to": point,
            "width_ft": { "type": "number", "description": "Deck, ford or raft width (default 12)" },
            "material": { "type": "string", "enum": ["timber", "stone"], "description": "A bridge's: timber planks (default) or old stone" },
        }), &[]) }),
        json!({ "name": "remove_crossings", "title": "Take crossings away", "description": "Take crossings put down by hand off the map, by id (v:...).", "inputSchema": schema(json!({ "ids": { "type": "array", "items": { "type": "string" } } }), &["ids"]) }),
    ]
}

/// Run a crossing tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "list_crossings" => list_crossings(app).map(text),
        "place_crossing" => place(app, a).await.map(text),
        "remove_crossings" => remove(app, a).await.map(text),
        _ => return None,
    })
}

fn describe(id: &str, c: &Crossing) -> Value {
    let mut v = json!({ "id": id, "kind": c.kind.name(), "from": c.a, "to": c.b, "width_ft": c.width, "length_ft": c.length().round() });
    if c.kind == CrossingKind::Bridge {
        v["material"] = json!(c.material.name());
    }
    v
}

fn list_crossings(app: &Shared) -> Result<Value, String> {
    Ok(json!(app.with_edits(|e| e.crossings.iter().map(|(id, c)| describe(id, c)).collect::<Vec<_>>()).unwrap_or_default()))
}

fn point(v: &Value, k: &str) -> Result<Option<[f64; 2]>, String> {
    if v[k].is_null() {
        return Ok(None);
    }
    match v[k].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()) {
        Some(p) if p.len() == 2 => Ok(Some([p[0], p[1]])),
        _ => Err(format!("{k}: [x_ft, y_ft]")),
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
    let (from, to, width) = (point(a, "from")?, point(a, "to")?, a["width_ft"].as_f64());
    let id = a["id"].as_str().map(str::to_string);
    let [w, h] = app.worker.with(|ex| Ok([ex.world.geom.map_w_ft, ex.world.geom.map_h_ft])).await?;
    app.edit("agent", 0, move |e| {
        let changed = id.is_some();
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
        let out = describe(&id, &c);
        e.crossings.insert(id.clone(), c.clone());
        Ok(json!({ "tool": "place_crossing", "id": id, "kind": c.kind.name(), "changed": changed, "crossing": out }))
    })
    .await
}

async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids: Vec<String> = a["ids"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
    if ids.is_empty() {
        return Err("give ids".into());
    }
    app.edit("agent", 0, move |e| {
        for id in &ids {
            e.crossings.remove(id).ok_or_else(|| format!("no crossing {id}"))?;
        }
        Ok(json!({ "tool": "remove_crossings", "ids": ids }))
    })
    .await
}
