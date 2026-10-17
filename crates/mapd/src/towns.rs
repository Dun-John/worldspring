//! MCP tools for the ward editor (`Edits.towns`): a town's plan (its patches and their corners)
//! read, and changed: corners moved, patches given another ward, lot size, merge or roll, a
//! temple's close, squares where streets meet, walls up or down (`worldgen::town::wards`).

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::town::wards;
use worldgen::world::{LOT_SIZES, SQUARE_KINDS, TOWN_WARDS};

use crate::Shared;
use crate::tools::{schema, text};

pub fn list() -> Vec<Value> {
    let pt = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    let town = json!({ "type": "string", "description": "The town or city (its feature id, as search_features gives it; villages have no wards)" });
    vec![
        json!({
            "name": "get_town_plan",
            "title": "A town's wards",
            "description": "A town's or city's plan as the ward editor sees it: its patches (each a ward cut into lots: patch number, middle, corners, ward, ward_generated when set by hand, district, neighbours, lots, merged_with, reroll, close when a temple's patch has an open close round the temple, close_generated when set by hand, close_refused: why one can't be set there or the one set isn't built) and their corners (number, where it stands, planned when moved, pinned on water or a river, gate, wall, junction where three streets or more meet so a square can open, square_refused where they meet but none can, square: the square there (crossroads, well, fountain or green) and square_ft how far out it is cut, square_generated when set by hand), max_move_ft (how far a corner may go from where it was laid out), the walls, and any part of its edit set aside. within limits it to the patches whose middle is in a rectangle.",
            "inputSchema": schema(json!({ "id": town, "within": { "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4, "description": "[x0_ft, y0_ft, x1_ft, y1_ft]" } }), &["id"]),
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "edit_town",
            "title": "Lay out a town anew",
            "description": "Change a town's or city's layout (as the ward editor does): move patch corners (moves: {corner, to [x_ft, y_ft] | by [dx_ft, dy_ft] | as_generated}), pull patches toward regular shapes (equalize: patch numbers), smooth corners round a point (relax: {at, radius_ft, amount 0-1}), set patches (patches: {patch, ward, lots, merge_with a neighbour or \"none\", reroll: true for a new roll, close: true to keep a temple's patch as an open close round the temple or false to build it up, as_generated}; \"auto\" puts a field back), open or close squares where streets meet (squares: {corner, kind: crossroads, well, fountain, green, none or auto}; only at a junction), put walls up or take them down (walls: true, false or \"auto\"), or put it back (reset: all, corners, patches or squares). Corners go only as far as keeps every patch round them convex: the reply says how many went all the way. Only the patches touched are built again; buildings that come out as they were keep their ids, new ones get new ids, and a business the town loses goes to a new building where one fits, else to the nearest home about a patch away (functions_lost lists the rest). dry_run reports without changing anything.",
            "inputSchema": schema(json!({
                "id": town,
                "moves": { "type": "array", "items": { "type": "object", "properties": { "corner": { "type": "integer" }, "to": pt, "by": pt, "as_generated": { "type": "boolean" } }, "required": ["corner"] } },
                "equalize": { "type": "array", "items": { "type": "integer" } },
                "relax": { "type": "object", "properties": { "at": pt, "radius_ft": { "type": "number" }, "amount": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5 } }, "required": ["at", "radius_ft"] },
                "patches": { "type": "array", "items": { "type": "object", "properties": {
                    "patch": { "type": "integer" },
                    "ward": { "type": "string", "enum": ([TOWN_WARDS.as_slice(), &["auto"]].concat()), "description": "empty: open ground (for buildings of your own)" },
                    "lots": { "type": "string", "enum": ([LOT_SIZES.as_slice(), &["auto"]].concat()) },
                    "merge_with": { "description": "A neighbouring patch whose district (ward and lots) it joins, with no street between; or \"none\"" },
                    "reroll": { "type": "boolean" },
                    "close": { "description": "A temple's patch: true opens a close round its temple (open ground with the temple, a cross before it and a hedged churchyard behind it, the houses round it kept), false builds it up; or \"auto\". Only where the town has its temple on that patch and no square opens at its corners" },
                    "as_generated": { "type": "boolean" },
                }, "required": ["patch"] } },
                "squares": { "type": "array", "items": { "type": "object", "properties": {
                    "corner": { "type": "integer", "description": "A corner where three streets or more meet (junction in get_town_plan)" },
                    "kind": { "type": "string", "enum": ([SQUARE_KINDS.as_slice(), &["none", "auto"]].concat()), "description": "The square opened there (the blocks' corners round it cut back), none for no square, auto as generated" },
                }, "required": ["corner", "kind"] } },
                "walls": { "description": "true, false or \"auto\" (as generated). Gates stay where the roads come in; a wall taken down leaves a street round the town" },
                "reset": { "type": "string", "enum": ["all", "corners", "patches", "squares"] },
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
    agent::layout_of(&ex.world, &ex.t0, id).filter(|&l| l < ex.t0.settlements.len()).ok_or_else(|| format!("{id} is not a settlement"))
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
    app.edit("agent", 0, move |e| {
        match edit {
            Some(t) => e.towns.insert(key, t),
            None => e.towns.remove(&key),
        };
        Ok(reply)
    })
    .await
}
