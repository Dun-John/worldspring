//! `list_streets`: the streets, walls, squares and roads in an area of the map.

use serde_json::{Value, json};
use worldgen::agent;

use crate::Shared;
use crate::args::{area, area_light};
use crate::tools::{limit_of, limit_prop, listed, schema, text};

/// The widest area asked for (ft across): a city's streets run to thousands of points.
const MAX_ACROSS_FT: f64 = 3000.0;

pub fn list() -> Vec<Value> {
    vec![json!({
        "name": "list_streets",
        "title": "Streets in an area",
        "description": "Streets, walls, squares and world roads (rd:) in an area whose bounding box is at most 3,000 ft across, per settlement or site, with width, kind and line (cut to the area). Returns {items, found}.",
        "inputSchema": schema(json!({
            "within": area_light("Box [x0, y0, x1, y1], polygon [[x, y], ...] or circle {at, radius_ft}"),
            "what": { "type": "string", "enum": ["street", "wall", "square", "road"] },
            "limit": limit_prop(100, 1000),
        }), &["within"]),
    })]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    match name {
        "list_streets" => Some(streets(app, a).await.map(text)),
        _ => None,
    }
}

async fn streets(app: &Shared, a: &Value) -> Result<Value, String> {
    let within = area(app, "within", &a["within"]).await?;
    let o = within.outline();
    let rect = o.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let across = (rect[2] - rect[0]).max(rect[3] - rect[1]);
    if across > MAX_ACROSS_FT {
        return Err(format!("within: its bounding box may be at most {MAX_ACROSS_FT} ft across, this one is {} ft; ask for a smaller box, polygon or circle", across.round()));
    }
    let what = a["what"].as_str().map(str::to_string);
    let items = app
        .worker
        .with(move |ex| {
            let mut v = agent::streets_in(&ex.world, &ex.t0, rect, &|p| within.contains(p));
            if let Some(w) = &what {
                v.retain(|i| i["what"].as_str() == Some(w));
            }
            Ok(v)
        })
        .await?;
    Ok(listed(json!(items), limit_of(a, 100), json!({ "tool": "list_streets" })))
}
