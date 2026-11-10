//! (R5 towns, M18) A wall drawn by hand round a town's patches (`around_patches`), its gates on
//! streets (`street_gates`, a way into every ring), and its gates said in the reply: for
//! `create_feature` kind `wall` and `update_fortification` (`works.rs`).

use serde_json::{Map, Value, json};
use worldgen::agent;
use worldgen::town::geom::{centroid, dist};
use worldgen::town::handwall;
use worldgen::world::Created;

use crate::Shared;

type P = [f64; 2];

/// With `around_patches` (and `settlement`): the wall's line laid round those patches on the
/// town's plan, closed, its gates where main streets (with `street_gates`, every street) cross;
/// what the reply says of it. None without `around_patches`.
pub async fn line(app: &Shared, a: &Value, c: &mut Created) -> Result<Option<Map<String, Value>>, String> {
    let Some(arr) = a["around_patches"].as_array() else { return Ok(None) };
    if a["pts"].is_array() {
        return Err("give the wall's line (pts) or around_patches, not both".into());
    }
    let patches: Vec<usize> = arr.iter().map(|v| v.as_u64().map(|k| k as usize)).collect::<Option<_>>().ok_or("around_patches: patch numbers (get_town_plan)")?;
    let sid = a["settlement"].as_str().ok_or("around_patches needs settlement: the town's id")?.to_string();
    let all = a["street_gates"].as_bool().unwrap_or(false);
    let ring = app
        .worker
        .with(move |ex| {
            let i = agent::layout_of(&ex.world, &ex.t0, &sid).filter(|&l| l < ex.t0.settlements.len()).ok_or_else(|| format!("{sid} is not a settlement"))?;
            handwall::around_patches(&ex.world, &ex.t0, i, &patches, all).map(|r| (r, sid))
        })
        .await?;
    let (ring, sid) = ring;
    c.pts = ring.pts.clone();
    c.gates = ring.gates.clone();
    c.closed = true;
    let mut m = Map::new();
    m.insert("around".into(), json!({ "settlement": sid, "patches": arr, "plan_corners": ring.plan_corners }));
    if !ring.bare.is_empty() {
        m.insert("sides_without_street".into(), json!(ring.bare));
    }
    if !ring.enclosed.is_empty() {
        m.insert("enclosed_patches".into(), json!(ring.enclosed));
    }
    let closed: Vec<Value> = ring.crossings.iter().map(|x| json!({ "corner": x.corner, "x_ft": x.at[0].round(), "y_ft": x.at[1].round(), "street": if x.main { "main" } else { "side" } })).collect();
    m.insert("streets_closed".into(), json!(closed));
    Ok(Some(m))
}

/// A wall given by its line: with `street_gates`, a gate on each side street crossing it (a new
/// corner where none is within 20 ft); a closed one with no gate coming (none set, no road,
/// approach or main street crossing) gets one at the side street nearest its town's middle
/// (else its corner nearest it). What the reply says: the side streets closed.
pub async fn street_gates(app: &Shared, a: &Value, c: &mut Created) -> Result<Map<String, Value>, String> {
    let all = a["street_gates"].as_bool().unwrap_or(false);
    let (pts, gates, closed) = (c.pts.clone(), c.gates.clone(), c.closed);
    if pts.len() < 2 || pts.len() > worldgen::world::WALL_RING_MAX_CORNERS {
        return Ok(Map::new());
    }
    let (pts, gates, shut) = app
        .worker
        .with(move |ex| {
            let (mut pts, mut gates) = (pts, gates);
            // (A ring on a town's patch edges, as `around_patches` lays one: its streets by the plan.)
            if closed && all && let Some((more, crossings)) = worldgen::town::wallgates::ring_street_gates(&ex.world, &ex.t0, &pts) {
                gates.extend(more);
                gates.sort_unstable();
                gates.dedup();
                let shut: Vec<P> = crossings.iter().filter(|c| c.corner.is_none_or(|k| !gates.contains(&(k as u32)))).map(|c| c.at).collect();
                return Ok((pts, gates, shut));
            }
            let side = handwall::side_crossings(&ex.world, &ex.t0, &pts, closed);
            let shut: Vec<P> = side.clone();
            if all {
                for &at in &side {
                    handwall::add_gate(&mut pts, &mut gates, closed, at);
                }
            }
            if closed && !handwall::builds_gate(&ex.world, &ex.t0, &pts, &gates) {
                let mid = handwall::town_of_wall(&ex.t0, centroid(&pts)).map_or(centroid(&pts), |s| [s.x, s.y]);
                let mut near = side.clone();
                near.sort_by(|p, q| dist(*p, mid).total_cmp(&dist(*q, mid)));
                let mut done = false;
                for &at in near.iter().take(8) {
                    let (mut p2, mut g2) = (pts.clone(), gates.clone());
                    handwall::add_gate(&mut p2, &mut g2, closed, at);
                    if handwall::builds_gate(&ex.world, &ex.t0, &p2, &g2) {
                        (pts, gates, done) = (p2, g2, true);
                        break;
                    }
                }
                if !done {
                    let mut order: Vec<usize> = (0..pts.len()).collect();
                    order.sort_by(|&i, &j| dist(pts[i], mid).total_cmp(&dist(pts[j], mid)));
                    handwall::way_in_gate(&ex.world, &ex.t0, &pts, &mut gates, &order);
                }
            }
            Ok((pts, gates, shut))
        })
        .await?;
    c.pts = pts;
    c.gates = gates;
    let mut m = Map::new();
    m.insert("streets_closed".into(), json!(shut.iter().map(|q| json!({ "x_ft": q[0].round(), "y_ft": q[1].round(), "street": "side" })).collect::<Vec<_>>()));
    Ok(m)
}

/// The reply for a wall: its corners, length, closed, and each gate as built (on corner k or at a
/// point), with what `line`/`street_gates` said.
pub async fn reply(app: &Shared, v: &mut Value, more: Map<String, Value>) {
    let Some(id) = v["id"].as_str().map(str::to_string) else { return };
    let got = app
        .worker
        .with(move |ex| {
            let c = ex.world.file.edits.created.iter().find(|c| c.id == id).cloned().ok_or("gone")?;
            let li = agent::layout_of(&ex.world, &ex.t0, &id).ok_or("no layout")?;
            let l = worldgen::town::layout(&ex.world, &ex.t0, li);
            let mids = handwall::gates_of(&l);
            Ok((c, mids))
        })
        .await;
    let Ok((c, mids)) = got else { return };
    let n = c.pts.len();
    let sides = if c.closed { n } else { n.saturating_sub(1) };
    let length: f64 = (0..sides).map(|i| dist(c.pts[i], c.pts[(i + 1) % n])).sum();
    v["corners"] = json!(n);
    v["closed"] = json!(c.closed);
    v["length_ft"] = json!(length.round());
    let list: Vec<Value> = mids
        .iter()
        .map(|m| match (0..n).filter(|&k| dist(c.pts[k], *m) < 30.0).min_by(|&i, &j| dist(c.pts[i], *m).total_cmp(&dist(c.pts[j], *m))) {
            Some(k) => json!({ "corner": k, "x_ft": m[0].round(), "y_ft": m[1].round() }),
            None => json!({ "x_ft": m[0].round(), "y_ft": m[1].round() }),
        })
        .collect();
    v["gates"] = json!(list.len());
    v["gate_list"] = json!(list);
    // (The streets crossing it with no gate as built: closed.)
    for (k, mut x) in more {
        if k == "streets_closed"
            && let Some(a) = x.as_array_mut()
        {
            let at = |q: &Value| [q["x_ft"].as_f64().unwrap_or(0.0), q["y_ft"].as_f64().unwrap_or(0.0)];
            a.retain(|q| !mids.iter().any(|m| dist(*m, at(q)) < 40.0));
        }
        v[k.as_str()] = x;
    }
}
