//! (Roadmap 6 R6) MCP tools for fences and hedges: drawn by hand (`Edits::fences`, `f:<n>`), a
//! generated run's kind and gates (`TownEdit::fences`, `fr:`/`fh:`), removed whole or cut by gaps
//! (`Edits::fence_gaps`, `fg:<n>`), all through `worldgen::fencecut`.

use serde_json::{Value, json};
use worldgen::fencecut::{self, ChangeOut, Op};
use worldgen::model::fences::{Fence, FenceGap, FenceKind, GAP_R_FT};
use worldgen::world::Edits;

use crate::Shared;
use crate::tools::{listed, schema, text};

pub fn list() -> Vec<Value> {
    // (Points as `point_schema` takes them, checked in `points`: the schemas stay short.)
    let kinds: Vec<&str> = FenceKind::ALL.iter().filter(|k| **k != FenceKind::GardenWall).map(|k| k.name()).collect();
    let props = json!({
        "id": { "type": "string", "description": "To change: f:, or fr:/fh: (kind, gates)" },
        "kind": { "type": "string", "enum": kinds, "description": "hedge blocks the way; stockade, high_wall block sight; post_and_rail no cover; the rest hard going, half cover" },
        "points": { "type": "array", "minItems": 2 },
        "closed": { "type": "boolean" },
        "gates": { "type": "array", "description": "Points for 5-ft gates" },
        "gaps": { "type": "array", "description": "Instead of a fence: cut gaps into fences, [{points, width_ft?: 5}]" },
    });
    vec![
        json!({ "name": "place_fence", "title": "Draw a fence", "description": "A fence or hedge along points (over roads, not water). With id, changes one. Returns its id. gaps alone cuts gaps into the fences there.", "inputSchema": schema(props, &[]) }),
        json!({ "name": "remove_fences", "title": "Take fences away", "description": "Fences and hedges off by id (fr:, fh:, f:; fg: a gap; restore: back), or cut along gaps.", "inputSchema": schema(json!({ "ids": { "type": "array", "minItems": 1 }, "gaps": { "type": "array", "description": "[{points, width_ft?: 5}]" }, "restore": { "type": "boolean" } }), &[]) }),
        json!({ "name": "list_fences", "title": "Fences and hedges", "description": "Drawn fences, gaps, changed runs; with at, all near.", "inputSchema": schema(json!({ "at": { "type": ["string", "object", "array"] }, "radius_ft": { "type": "number", "maximum": 2000 }, "limit": { "type": "integer", "minimum": 1 } }), &[]) }),
    ]
}

/// Run a fence tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        // (gaps alone: cut into the fences there, as `remove_fences` did.)
        "place_fence" if !a["gaps"].is_null() => match a.as_object().and_then(|o| o.keys().find(|k| !matches!(k.as_str(), "gaps" | "world"))) {
            Some(k) => Err(format!("gaps: alone (cut into the fences there), not with {k}")),
            None => remove(app, a).await.map(text),
        },
        "place_fence" => place(app, a).await.map(text),
        "remove_fences" => remove(app, a).await.map(text),
        "list_fences" => list_fences(app, a).await.map(text),
        _ => return None,
    })
}

/// A list of points (world ft) checked against the map.
async fn points(app: &Shared, field: &str, v: &Value) -> Result<Vec<[f64; 2]>, String> {
    let list = v.as_array().ok_or_else(|| format!("{field}: a list of points"))?;
    let [w, h] = crate::tools::args::map_size(app).await?;
    let mut out = Vec::with_capacity(list.len());
    for (i, p) in list.iter().enumerate() {
        let f = format!("{field}[{i}]");
        let xy = match p {
            Value::Array(a) if a.len() == 2 && a[0].is_number() && a[1].is_number() => [a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)],
            Value::Object(o) if o.get("x_ft").is_some_and(Value::is_number) && o.get("y_ft").is_some_and(Value::is_number) => [o["x_ft"].as_f64().unwrap_or(0.0), o["y_ft"].as_f64().unwrap_or(0.0)],
            _ => {
                out.push(crate::tools::args::point(app, &f, p).await?);
                continue;
            }
        };
        if !(0.0..w).contains(&xy[0]) || !(0.0..h).contains(&xy[1]) {
            return Err(format!("{f}: ({}, {}) is off the map", xy[0].round(), xy[1].round()));
        }
        out.push(xy);
    }
    Ok(out)
}

/// `out`'s refusals as a reply list.
fn refused(out: &ChangeOut) -> Vec<Value> {
    out.refused.iter().map(|r| json!({ "id": r.id, "why": r.why })).collect()
}

fn why(out: &ChangeOut) -> String {
    out.refused.iter().map(|r| if r.id.is_empty() { r.why.clone() } else { format!("{}: {}", r.id, r.why) }).collect::<Vec<_>>().join("; ")
}

/// Work `ops` out in turn on the world as it is and apply them as one edit (the app's sync sees it).
async fn run(app: &Shared, ops: Vec<(Vec<String>, Op)>, reply: impl FnOnce(&[ChangeOut]) -> Result<Value, String> + Send + 'static) -> Result<Value, String> {
    let _hold = if crate::tools::batch::active() { None } else { Some(app.gate.lock().await) };
    let outs = app.worker.with(move |ex| Ok(ops.iter().map(|(ids, op)| fencecut::change(&ex.world, &ex.t0, ids, op, false)).collect::<Vec<_>>())).await?;
    let v = reply(&outs)?;
    let change = v.clone();
    let apply = move |e: &mut Edits| {
        for o in &outs {
            o.apply(e);
        }
        Ok(change)
    };
    if _hold.is_some() { app.edit_now("agent", 0, apply).await } else { app.edit("agent", 0, apply).await }
}

async fn place(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().map(str::to_string);
    let kind = match &a["kind"] {
        Value::Null => None,
        Value::String(k) if k == "garden_wall" => return Err("kind: garden_wall is generated only: low_wall or high_wall".into()),
        Value::String(k) => Some(FenceKind::parse(k).ok_or("kind: hedgerow, hedge, dry_stone_wall, post_and_rail, wattle, low_wall, picket, iron_railing, stockade or high_wall")?),
        _ => return Err("kind: text".into()),
    };
    let pts = if a["points"].is_null() { None } else { Some(points(app, "points", &a["points"]).await?) };
    let gates = if a["gates"].is_null() { Vec::new() } else { points(app, "gates", &a["gates"]).await? };
    let closed = a["closed"].as_bool();
    match id.as_deref() {
        Some(g) if g.starts_with("fr:") || g.starts_with("fh:") => {
            if pts.is_some() || closed.is_some() {
                return Err("a generated run keeps its line: give kind and/or gates".into());
            }
            if kind.is_none() && gates.is_empty() {
                return Err("give kind and/or gates".into());
            }
            let g = g.to_string();
            run(app, vec![(vec![g.clone()], Op::Change(kind, gates))], move |outs| {
                let o = &outs[0];
                if o.done.is_empty() {
                    return Err(why(o));
                }
                let t = o.towns.values().flatten().next();
                let ch = worldgen::town::fence::parse(&g).and_then(|(_, l)| t.and_then(|t| t.fences.get(&l)));
                Ok(json!({ "tool": "place_fence", "id": g, "changed": true, "kind": ch.and_then(|c| c.kind).map(|k| k.name()), "gates": ch.map(|c| c.gates.clone()).unwrap_or_default() }))
            })
            .await
        }
        Some(f) if f.starts_with("f:") => {
            let f = f.to_string();
            let old: Fence = {
                let f = f.clone();
                app.worker.with(move |ex| ex.world.file.edits.fences.get(&f).cloned().ok_or_else(|| format!("no fence {f}"))).await?
            };
            let op = if pts.is_none() && closed.is_none() {
                if kind.is_none() && gates.is_empty() {
                    return Err("give kind, points, closed and/or gates".into());
                }
                Op::Change(kind, gates)
            } else {
                // (A new line drops its gates unless given again.)
                let mut n = Fence { kind: kind.unwrap_or(old.kind), pts: pts.unwrap_or(old.pts.clone()), closed: closed.unwrap_or(old.closed), gates: Vec::new() };
                for p in &gates {
                    let g = fencecut::along(&n.pts, n.closed, *p)?;
                    n.gates.push(g as f32);
                }
                n.gates.sort_by(f32::total_cmp);
                Op::Draw(n)
            };
            let key = f.clone();
            run(app, vec![(vec![f], op)], move |outs| {
                let o = &outs[0];
                match o.fences.get(&key) {
                    Some(Some(n)) => {
                        let mut v = fencecut::drawn_entry(&key, n);
                        v["tool"] = json!("place_fence");
                        v["changed"] = json!(true);
                        Ok(v)
                    }
                    _ => Err(why(o)),
                }
            })
            .await
        }
        Some(other) => Err(format!("id: {other} is not a fence (f:, fr:, fh:)")),
        None => {
            let pts = pts.ok_or("points: the fence's line, 2 or more points")?;
            let kind = kind.ok_or("kind: what it is (hedgerow, picket, low_wall, …)")?;
            let mut n = Fence { kind, pts, closed: closed.unwrap_or(false), gates: Vec::new() };
            for p in &gates {
                let g = fencecut::along(&n.pts, n.closed, *p)?;
                n.gates.push(g as f32);
            }
            n.gates.sort_by(f32::total_cmp);
            run(app, vec![(Vec::new(), Op::Draw(n))], move |outs| {
                let o = &outs[0];
                let Some((id, Some(n))) = o.fences.iter().next() else { return Err(why(o)) };
                let mut v = fencecut::drawn_entry(id, n);
                v["tool"] = json!("place_fence");
                v["changed"] = json!(false);
                Ok(v)
            })
            .await
        }
    }
}

async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let restore = a["restore"].as_bool().unwrap_or(false);
    let ids: Vec<String> = match &a["ids"] {
        Value::Null => Vec::new(),
        Value::Array(l) if l.iter().all(Value::is_string) => l.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => return Err("ids: a list of ids".into()),
    };
    if ids.len() > fencecut::MAX_IDS {
        return Err(format!("at most {} ids a call (got {}): nothing was changed", fencecut::MAX_IDS, ids.len()));
    }
    let mut gaps = Vec::new();
    if !a["gaps"].is_null() && !a["gaps"].is_array() {
        return Err("gaps: a list of {points, width_ft?}".into());
    }
    if let Some(list) = a["gaps"].as_array() {
        if restore {
            return Err("restore takes ids (fg: to drop a gap: ids without restore)".into());
        }
        for (i, g) in list.iter().enumerate() {
            let pts = points(app, &format!("gaps[{i}].points"), &g["points"]).await?;
            let w = match &g["width_ft"] {
                Value::Null => 2.0 * GAP_R_FT as f64,
                v => v.as_f64().filter(|w| (1.0..=20.0).contains(w)).ok_or(format!("gaps[{i}].width_ft: 1 to 20"))?,
            };
            gaps.push(FenceGap { pts, r: (0.5 * w) as f32 });
        }
    }
    if ids.is_empty() && gaps.is_empty() {
        return Err("give ids and/or gaps".into());
    }
    let mut ops = Vec::new();
    if !ids.is_empty() {
        ops.push((ids, if restore { Op::Restore } else { Op::Remove }));
    }
    if !gaps.is_empty() {
        ops.push((Vec::new(), Op::Gaps(gaps)));
    }
    let with_ids = !a["ids"].is_null();
    run(app, ops, move |outs| {
        let (by_id, cut) = if with_ids { (outs.first(), outs.get(1)) } else { (None, outs.first()) };
        let done: Vec<String> = by_id.map(|o| o.done.clone()).unwrap_or_default();
        let new: Vec<String> = cut.map(|o| o.done.clone()).unwrap_or_default();
        let refused: Vec<Value> = outs.iter().flat_map(refused).collect();
        if done.is_empty() && new.is_empty() {
            return Err(format!("nothing {}: {}", if restore { "restored" } else { "removed" }, outs.iter().map(why).collect::<Vec<_>>().join("; ")));
        }
        Ok(json!({ "tool": "remove_fences", (if restore { "restored" } else { "removed" }): done, "gaps": new, "refused": refused }))
    })
    .await
}

async fn list_fences(app: &Shared, a: &Value) -> Result<Value, String> {
    let limit = crate::tools::limit_of(a, 100);
    if a["at"].is_null() {
        let v = app.worker.with(|ex| Ok(fencecut::list(&ex.world, &ex.t0))).await?;
        let mut items: Vec<Value> = Vec::new();
        for k in ["drawn", "gaps", "changed"] {
            items.extend(v[k].as_array().cloned().unwrap_or_default());
        }
        return Ok(listed(json!(items), limit, json!({})));
    }
    let at = crate::tools::args::point(app, "at", &a["at"]).await?;
    let r = a["radius_ft"].as_f64().unwrap_or(200.0).clamp(0.0, fencecut::MAX_NEAR_FT);
    let items = app.worker.with(move |ex| Ok(fencecut::near(&ex.world, &ex.t0, at, r, usize::MAX))).await?;
    Ok(listed(json!(items), limit, json!({ "at": [at[0].round(), at[1].round()], "radius_ft": r })))
}
