//! MCP tools for the ward editor (`Edits.towns`): a town's plan (its patches and their corners)
//! read, and changed: corners moved, patches given another ward, lot size, merge or roll, a
//! temple's close, a park's garden, a district, squares where streets meet, walls up or down
//! (`worldgen::town::wards`).

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::town::wards;
use worldgen::world::{LOT_SIZES, PARK_KINDS, SQUARE_KINDS, TOWN_WARDS};

use crate::Shared;
use crate::tools::{schema, text};

pub fn list() -> Vec<Value> {
    let pt = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    let tower_ref = json!({ "oneOf": [{ "type": "integer", "minimum": 0 }, { "type": "string" }] });
    let corners = json!({ "type": "array", "items": { "type": "integer", "minimum": 0 } });
    let walls = json!({ "type": "object", "properties": {
        "built": { "oneOf": [{ "type": "boolean" }, { "type": "string", "enum": ["auto"] }] },
        "gates_add": corners.clone(),
        "gates_remove": corners,
        "towers_add": { "type": "array", "items": { "oneOf": [{ "type": "integer", "minimum": 0 }, { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }] } },
        "towers_remove": { "type": "array", "items": tower_ref },
    } });
    let town = json!({ "type": "string", "description": "The town or city (its id; villages have no wards)" });
    vec![
        json!({
            "name": "get_town_plan",
            "title": "A town's wards",
            "description": "A town's or city's plan as the ward editor sees it: patches (number, middle, corners, ward, lots, district, neighbours, merge, reroll, close, park; what was set by hand or refused), corners (number, place, pinned, gate, wall, junction where a square can open, square), districts, max_move_ft and the walls. within limits it to patches whose middle is in a rectangle. See MCP.md > Towns laid out anew.",
            "inputSchema": schema(json!({ "id": town, "within": { "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4, "description": "[x0_ft, y0_ft, x1_ft, y1_ft]" } }), &["id"]),
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "edit_town",
            "title": "Lay out a town anew",
            "description": "Change a town's or city's layout as the ward editor does; give any of: moves (corners to or by a place), equalize (patch numbers), relax (corners round a point), patches (ward, lots, merge_with, reroll, close, park, district; \"auto\" puts a field back), squares (where streets meet), walls, reset. Corners go only as far as keeps every patch convex. The reply says how many corners went all the way, patches_changed (each patch and what), districts_changed, functions_lost, renames_carried and renames_lost. dry_run reports without changing. See MCP.md > Towns laid out anew.",
            "inputSchema": schema(json!({
                "id": town,
                "moves": { "type": "array", "items": { "type": "object", "properties": { "corner": { "type": "integer", "minimum": 0 }, "to": pt, "by": pt, "as_generated": { "type": "boolean" } }, "required": ["corner"] } },
                "equalize": { "type": "array", "items": { "type": "integer", "minimum": 0 } },
                "relax": { "type": "object", "properties": { "at": crate::args::point_schema("The middle: an id, {x_ft, y_ft} or [x_ft, y_ft]"), "radius_ft": { "type": "number", "minimum": 1, "maximum": 5000 }, "amount": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5 } }, "required": ["at", "radius_ft"] },
                "patches": { "type": "array", "items": { "type": "object", "properties": {
                    "patch": { "type": "integer", "minimum": 0 },
                    "ward": { "type": "string", "enum": ([TOWN_WARDS.as_slice(), &["auto"]].concat()), "description": "empty: open ground (for buildings of your own)" },
                    "lots": { "type": "string", "enum": ([LOT_SIZES.as_slice(), &["auto"]].concat()) },
                    "merge_with": { "oneOf": [{ "type": "integer", "minimum": 0 }, { "type": "string", "enum": ["none"] }], "description": "A neighbouring patch whose district it joins, with no street between; or \"none\"" },
                    "reroll": { "type": "boolean" },
                    "close": { "oneOf": [{ "type": "boolean" }, { "type": "string", "enum": ["auto"] }], "description": "A temple's patch: true keeps a close round the temple, false builds it up, \"auto\" as generated" },
                    "park": { "type": "string", "enum": ([PARK_KINDS.as_slice(), &["auto"]].concat()), "description": "A park's patch (ward park): its layout; \"auto\" as generated" },
                    "district": { "type": "string", "description": "A district's id (d:<layout>:<n>), a name (that district, else a new one) or \"auto\"" },
                    "as_generated": { "type": "boolean" },
                }, "required": ["patch"] } },
                "squares": { "type": "array", "items": { "type": "object", "properties": {
                    "corner": { "type": "integer", "minimum": 0, "description": "A corner where three streets or more meet (junction in get_town_plan)" },
                    "kind": { "type": "string", "enum": ([SQUARE_KINDS.as_slice(), &["none", "auto"]].concat()), "description": "The square opened there; none for no square, auto as generated" },
                }, "required": ["corner", "kind"] } },
                "walls": { "oneOf": [{ "type": "boolean" }, { "type": "string", "enum": ["auto"] }, walls], "description": "true, false or \"auto\" (as generated), or {built, gates_add, gates_remove (corners), towers_add ([corner, next corner, f 0.05-0.95] or a corner), towers_remove (corner or t:<layout>:<n>)}; the reply's wall says what was added, removed and refused (why)" },
                "reset": { "type": "string", "enum": ["all", "corners", "patches", "squares", "walls"] },
                "dry_run": { "type": "boolean", "default": false },
            }), &["id"]),
        }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "get_town_plan" => plan(app, a).await.map(text),
        "edit_town" => edit(app, a).await.map(text),
        _ => return None,
    })
}

/// The layout index of the town `id` names.
fn town_index(ex: &worldgen::pipeline::Executor, id: &str) -> Result<usize, String> {
    agent::layout_of(&ex.world, &ex.t0, id).filter(|&l| worldgen::town::is_settlement(&ex.t0, l)).ok_or_else(|| format!("{id} is not a settlement"))
}

async fn plan(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().ok_or("missing 'id'")?.to_string();
    let within: Option<Vec<f64>> = a["within"].as_array().map(|r| r.iter().filter_map(Value::as_f64).collect());
    let mut v = app.worker.with(move |ex| wards::plan_json(&ex.world, &ex.t0, town_index(ex, &id)?)).await?;
    if let Some(r) = within {
        let [x0, y0, x1, y1] = r[..] else { return Err("within: [x0_ft, y0_ft, x1_ft, y1_ft]".into()) };
        let inside = |p: &Value| p["at"][0].as_f64().is_some_and(|x| x >= x0 && x <= x1) && p["at"][1].as_f64().is_some_and(|y| y >= y0 && y <= y1);
        let patches: Vec<Value> = v["patches"].as_array().into_iter().flatten().filter(|p| inside(p)).cloned().collect();
        let used: std::collections::BTreeSet<u64> = patches.iter().flat_map(|p| p["corners"].as_array().into_iter().flatten().filter_map(Value::as_u64)).collect();
        let corners: Vec<Value> = v["corners"].as_array().into_iter().flatten().filter(|c| c["corner"].as_u64().is_some_and(|k| used.contains(&k))).cloned().collect();
        v["patches"] = json!(patches);
        v["corners"] = json!(corners);
    }
    Ok(v)
}

async fn edit(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().ok_or("missing 'id'")?.to_string();
    let dry = a["dry_run"].as_bool().unwrap_or(false);
    let mut req = a.clone();
    if let Some(m) = req.as_object_mut() {
        for k in ["id", "dry_run", "world"] {
            m.remove(k);
        }
    }
    // (relax's middle is a place like any other: an id, {x_ft, y_ft} or [x_ft, y_ft].)
    if let Some(at) = req["relax"].get("at").cloned() {
        req["relax"]["at"] = json!(crate::args::point(app, "relax.at", &at).await?);
    }
    let req: wards::TownRequest = serde_json::from_value(req).map_err(|e| e.to_string())?;
    let town = id.clone();
    let (li, edit, mut report) = app
        .worker
        .with(move |ex| {
            let li = town_index(ex, &id)?;
            let (edit, report) = wards::change(&ex.world, &ex.t0, li, &req)?;
            Ok((li, edit, report))
        })
        .await?;
    report["tool"] = json!("edit_town");
    report["id"] = json!(town);
    report["layout"] = json!(li);
    if dry {
        report["dry_run"] = json!(true);
        report["edited"] = json!(edit.is_some());
        return Ok(report);
    }
    let key = wards::key(li);
    let reply = report.clone();
    // (R5 towns: renames and notes go with the buildings that take their place.)
    let carried: Vec<(String, String)> = report["renames_carried"].as_array().into_iter().flatten().filter_map(|c| Some((c["from"].as_str()?.to_string(), c["to"].as_str()?.to_string()))).collect();
    app.edit("agent", 0, move |e| {
        match edit {
            Some(t) => e.towns.insert(key, t),
            None => e.towns.remove(&key),
        };
        for (from, to) in carried {
            if let Some(n) = e.renames.remove(&from) {
                e.renames.insert(to.clone(), n);
            }
            if let Some(n) = e.notes.remove(&from) {
                e.notes.insert(to, n);
            }
        }
        Ok(reply)
    })
    .await
}
