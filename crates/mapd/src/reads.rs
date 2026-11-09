//! Read-backs of hand work, world-wide: placed objects and clears, plateaus and brushed ground, designed sites and
//! buildings. Each list is `{items, found}` cut by `limit`, filtered by `within`.

use serde_json::{Value, json};
use worldgen::battlemap;
use worldgen::ground;
use worldgen::world::{ObjKind, SpriteMeta};

use crate::Shared;
use crate::args::{Area, area, area_light};
use crate::tools::{limit_of, limit_prop, listed, schema, text};

pub fn list() -> Vec<Value> {
    let within = area_light("Only inside: a box [x0, y0, x1, y1], a polygon [[x, y], ...] or a circle {at, radius_ft}");
    vec![
        json!({ "name": "list_objects", "title": "Objects put down", "description": "Objects put on battlemaps by hand (o:) and generated ones taken away (x:), world-wide, with ids, kinds and places. Returns {items, found}.", "inputSchema": schema(json!({
            "within": within.clone(),
            "kinds": { "type": "array", "items": { "oneOf": [{ "type": "integer", "minimum": 0 }, { "type": "string" }] }, "description": "Only these kinds" },
            "which": { "type": "string", "enum": ["objects", "clears", "both"], "default": "both" },
            "limit": limit_prop(200, 2000),
        }), &[]) }),
        json!({ "name": "list_plateaus", "title": "Plateaus and brushed ground", "description": "Plateaus and pits (h:) with outline, level and edge, and brushed ground blocks (g:) with place and size, world-wide. Returns {items, found}.", "inputSchema": schema(json!({ "within": within.clone(), "limit": limit_prop(100, 1000) }), &[]) }),
        json!({ "name": "list_designs", "title": "Designed sites and buildings", "description": "Underground sites (u:) and buildings (b:) with a design made by hand: id, name, what, levels, place. Returns {items, found}.", "inputSchema": schema(json!({ "within": within, "limit": limit_prop(100, 1000) }), &[]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    let r = match name {
        "list_objects" => objects(app, a).await,
        "list_plateaus" => plateaus(app, a).await,
        "list_designs" => designs(app, a).await,
        _ => return None,
    };
    Some(r.map(text))
}

async fn within(app: &Shared, a: &Value) -> Result<Option<Area>, String> {
    if a["within"].is_null() {
        return Ok(None);
    }
    area(app, "within", &a["within"]).await.map(Some)
}

/// A kind as named in results: a built-in's name, or an uploaded sprite's `s:<asset>`.
fn kind_name(k: &ObjKind, sprites: &std::collections::BTreeMap<String, SpriteMeta>) -> Value {
    match k {
        ObjKind::Builtin(id) => json!(battlemap::info(*id).name),
        ObjKind::Sprite(s) => json!(sprites.get(s.trim_start_matches("s:")).map(|m| format!("{} ({s})", m.name)).unwrap_or_else(|| s.clone())),
    }
}

async fn objects(app: &Shared, a: &Value) -> Result<Value, String> {
    let area = within(app, a).await?;
    let which = a["which"].as_str().unwrap_or("both").to_string();
    let limit = limit_of(a, 200);
    let sprites = app.with_edits(|e| e.sprites.clone()).unwrap_or_default();
    let mut wanted: Vec<ObjKind> = Vec::new();
    for (i, k) in a["kinds"].as_array().into_iter().flatten().enumerate() {
        wanted.push(crate::scatter::kind_of(k, &sprites).map_err(|e| format!("kinds[{i}]: {e}"))?);
    }
    let (mut objects, clears) = app
        .with_edits(|e| {
    let near = |x: f64, y: f64| area.as_ref().is_none_or(|ar| ar.contains([x, y]));
    let mut objects = Vec::new();
    if which != "clears" {
        for (id, p) in e.objects.iter().filter(|(_, p)| near(p.x, p.y) && (wanted.is_empty() || wanted.contains(&p.kind))) {
            let mut v = json!({ "id": id, "what": "object", "kind": kind_name(&p.kind, &sprites), "x_ft": p.x.round(), "y_ft": p.y.round(), "variant": p.variant });
            if p.turned || p.rot != 0.0 {
                v["rot_deg"] = json!((p.rot as f64).to_degrees().rem_euclid(360.0).round());
            }
            if (p.scale - 1.0).abs() > 1e-3 {
                v["scale"] = json!((p.scale as f64 * 100.0).round() / 100.0);
            }
            if let ObjKind::Sprite(s) = &p.kind {
                v["sprite"] = json!(s);
            }
            objects.push(v);
        }
    }
    let mut clears = Vec::new();
    if which != "objects" {
        let builtin_wanted: Vec<u16> = wanted.iter().filter_map(|k| if let ObjKind::Builtin(b) = k { Some(*b) } else { None }).collect();
        for (id, c) in e.cleared.iter().filter(|(_, c)| {
            // A clear of all kinds matches any built-in kind asked for; clears hold built-in kinds only (kinds naming only sprites: none).
            near(c.x, c.y) && (wanted.is_empty() || (!builtin_wanted.is_empty() && ((c.kind.is_none() && c.kinds.is_empty()) || c.kind.is_some_and(|k| builtin_wanted.contains(&k)) || c.kinds.iter().any(|k| builtin_wanted.contains(k)))))
        }) {
            let mut v = json!({ "id": id, "what": "clear", "x_ft": c.x.round(), "y_ft": c.y.round() });
            if let Some(k) = c.kind {
                v["kind"] = json!(battlemap::info(k).name);
            }
            if let Some(r) = c.r {
                v["radius_ft"] = json!(r.round());
            }
            if !c.kinds.is_empty() {
                v["kinds"] = json!(c.kinds.iter().map(|k| battlemap::info(*k).name).collect::<Vec<_>>());
            }
            clears.push(v);
        }
    }
    (objects, clears)
        })
        .ok_or("no world is open")?;
    let rest = json!({ "objects": objects.len(), "clears": clears.len() });
    objects.extend(clears);
    Ok(listed(json!(objects), limit, rest))
}

fn bbox(pts: &[[f64; 2]]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])])
}

async fn plateaus(app: &Shared, a: &Value) -> Result<Value, String> {
    let area = within(app, a).await?;
    let limit = limit_of(a, 100);
        let v = app
        .worker
        .with(move |ex| {
            let sea = ex.world.params().sea_level_ft;
            // Inside when the middle or any outline point is.
            let hits = |mid: [f64; 2], pts: &[[f64; 2]]| area.as_ref().is_none_or(|ar| ar.contains(mid) || pts.iter().any(|&q| ar.contains(q)));
            let mut items = Vec::new();
            for (id, p) in &ex.world.file.edits.plateaus {
                let b = bbox(&p.pts);
                let mid = worldgen::town::geom::centroid(&p.pts);
                if hits(mid, &p.pts) {
                    items.push(json!({ "id": id, "what": "plateau", "level_ft": ((p.level as f64 - sea) * 10.0).round() / 10.0, "edge_ft": p.edge, "x_ft": mid[0].round(), "y_ft": mid[1].round(), "bbox": b.map(f64::round), "poly": p.pts }));
                }
            }
            let side = ground::BLOCK as f64 * battlemap::SQUARE_FT;
            for (id, g) in &ex.world.file.edits.ground {
                let Some((bx, by)) = ground::parse_key(id) else { continue };
                let b = [bx as f64 * side, by as f64 * side, (bx + 1) as f64 * side, (by + 1) as f64 * side];
                if hits([0.5 * (b[0] + b[2]), 0.5 * (b[1] + b[3])], &[[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]) {
                    let up = g.add.iter().copied().max().unwrap_or(0).max(0);
                    let down = g.add.iter().copied().min().unwrap_or(0).min(0);
                    items.push(json!({
                        "id": id, "what": "ground", "x_ft": b[0], "y_ft": b[1], "size_ft": side, "squares_shaped": g.add.iter().filter(|&&v| v != 0).count(),
                        "raised_ft": up as f64 / 10.0, "lowered_ft": -down as f64 / 10.0,
                    }));
                }
            }
            Ok(json!(items))
        })
        .await?;
    Ok(listed(v, limit, json!({})))
}

async fn designs(app: &Shared, a: &Value) -> Result<Value, String> {
    let area = within(app, a).await?;
    let limit = limit_of(a, 100);
    let v = app
        .worker
        .with(move |ex| {
            let mut items = Vec::new();
            for (id, d) in &ex.world.file.edits.designs {
                let at = worldgen::agent::position(&ex.world, &ex.t0, id);
                if let Some(ar) = &area
                    && !at.is_some_and(|p| ar.contains(p))
                {
                    continue;
                }
                let mut v = json!({ "id": id, "name": worldgen::agent::name_any(&ex.world, &ex.t0, id), "what": if id.starts_with("b:") { "building" } else { "site" }, "kind": d.kind, "levels": d.levels.len() });
                match at {
                    Some(p) => {
                        v["x_ft"] = json!(p[0].round());
                        v["y_ft"] = json!(p[1].round());
                    }
                    None => v["gone"] = json!(true),
                }
                items.push(v);
            }
            Ok(json!(items))
        })
        .await?;
    Ok(listed(v, limit, json!({})))
}
