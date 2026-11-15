//! MCP tools for towns laid out by hand: a hand town's street plan read (`get_street_plan`) and
//! changed (`edit_streets`: the change function's ops, `worldgen::town::handedit`).

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::town::{hand, handedit};

use crate::Shared;
use crate::tools::{schema, text};

pub fn list() -> Vec<Value> {
    let town = json!({ "type": "string", "description": "The town or city (its id)" });
    // (The ops are checked by the change function's own request type, which names what is wrong: no property list here keeps tools/list short.)
    let op = json!({ "type": "object", "required": ["op"], "description": "{op, ...}: see the tool's description and MCP.md" });
    vec![
        json!({
            "name": "get_street_plan",
            "title": "A hand town's streets",
            "description": "A hand-laid town's junctions, streets, blocks, rows, areas, walls and problems (world ft). See MCP.md.",
            "inputSchema": schema(json!({ "id": town }), &["id"]),
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "edit_streets",
            "title": "Lay out streets by hand",
            "description": "Lay out a town's streets by hand: ops in order (or one as op), ids from get_street_plan, world ft: lay_out_by_hand, back_to_generated, add_street {points, width?, set_l?, set_r?, class?}, delete_street {street}, set_street {street, width?, set_l?, set_r?, class? main|street|quay|joint, bridge?}, move_junctions {moves: [{junction, to|by}]}, set_square {junction, square|none}, set_block {block, fill? {kind generated, ward, lots?, height? low|usual|tall, reroll?, close?} | {kind hand} | {kind open, open, style?} | {kind castle, gates? [[x, y]], keep_shape? square|round, moat? none|dry|wet}, district?, seed?, reroll?}, add_row {street, side l|r, depth, ward, lots?, gaps?}, remove_row {row}, add_area {poly, open}, remove_area {area}, add_wall {pts, closed?, spacing?, towers?, gates?, name?}, remove_wall {wall}, set_area {area, open}, set_wall {wall, gates? [[x, y]] (replaces the hand gates, each within 30 ft of the line), name? (empty: none), spacing?}. A refusal ends the list. dry_run: first op only. See MCP.md.",
            "inputSchema": schema(json!({
                "id": town,
                "ops": { "type": "array", "items": op.clone(), "minItems": 1 },
                "op": op,
                "dry_run": { "type": "boolean", "default": false },
            }), &["id"]),
        }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "get_street_plan" => plan(app, a).await.map(text),
        "edit_streets" => edit(app, a).await.map(text),
        _ => return None,
    })
}

fn town_index(ex: &worldgen::pipeline::Executor, id: &str) -> Result<usize, String> {
    agent::layout_of(&ex.world, &ex.t0, id).filter(|&l| worldgen::town::is_settlement(&ex.t0, l)).ok_or_else(|| format!("{id} is not a settlement"))
}

async fn plan(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().ok_or("missing 'id'")?.to_string();
    let mut v = app
        .worker
        .with(move |ex| {
            let li = town_index(ex, &id)?;
            if !hand::is_hand(&ex.world, li) {
                return Err(format!("{id} is not laid out by hand: edit_streets with op lay_out_by_hand does that"));
            }
            handedit::plan_json(&ex.world, &ex.t0, li)
        })
        .await?;
    v["tool"] = json!("get_street_plan");
    Ok(v)
}

async fn edit(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().ok_or("missing 'id'")?.to_string();
    let dry = a["dry_run"].as_bool().unwrap_or(false);
    let ops: Vec<Value> = match (a.get("ops"), a.get("op")) {
        (Some(_), Some(_)) => return Err("give ops (a list) or op (one), not both".into()),
        (Some(o), None) => o.as_array().cloned().ok_or("ops: a list of {op, …}")?,
        (None, Some(o)) => vec![o.clone()],
        (None, None) => return Err("give ops (a list of {op, …}) or op".into()),
    };
    if ops.is_empty() {
        return Err("ops is empty".into());
    }
    if ops.len() > 1 && crate::batch::active() {
        return Err("inside a batch, one op per edit_streets call (each op is built on the last one's result)".into());
    }
    if ops.len() > 1 && dry {
        return Err("dry_run takes one op (the next ones depend on the first)".into());
    }
    // Each change is computed and applied under one hold of the gate (the whole list), so no other
    // edit can land between a change and the plan it was computed on.
    let _hold = if crate::batch::active() || dry { None } else { Some(app.gate.lock().await) };
    let mut replies: Vec<Value> = Vec::new();
    for (n, op) in ops.into_iter().enumerate() {
        // (serde ignores extra fields on a variant with none.)
        if matches!(op["op"].as_str(), Some("lay_out_by_hand" | "back_to_generated")) && op.as_object().is_some_and(|o| o.len() > 1) {
            return Err(format!("op {n}: {} takes no fields", op["op"].as_str().unwrap_or("")));
        }
        let req = op.to_string();
        let town = id.clone();
        let (li, change, report) = app
            .worker
            .with(move |ex| {
                let li = town_index(ex, &town)?;
                let parsed: handedit::HandRequest = serde_json::from_str(&req).map_err(|e| e.to_string())?;
                let (ch, report) = handedit::change(&ex.world, &ex.t0, li, &parsed)?;
                Ok((li, ch, report))
            })
            .await
            .map_err(|e| if n == 0 { e } else { format!("op {n} ({}): {e} (ops before it were applied)", op["op"].as_str().unwrap_or("?")) })?;
        let mut report = report;
        report["layout"] = json!(li);
        if dry {
            report["dry_run"] = json!(true);
            return Ok(json!({ "tool": "edit_streets", "id": id, "dry_run": true, "results": [report] }));
        }
        let reply = report.clone();
        let apply = move |e: &mut worldgen::world::Edits| {
            change.apply(e);
            Ok(reply)
        };
        if _hold.is_some() {
            app.edit_now("agent", 0, apply).await?;
        } else {
            app.edit("agent", 0, apply).await?;
        }
        replies.push(report);
    }
    Ok(json!({ "tool": "edit_streets", "id": id, "results": replies }))
}
