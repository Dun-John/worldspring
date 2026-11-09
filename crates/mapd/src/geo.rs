//! Geometry reads: `nearest` (the nearest coast, cliff, river, road or flat ground) and `road_point`
//! (a point along a road). The work is in `worldgen::geoquery`; thresholds are defined there.

use serde_json::{Value, json};
use worldgen::geoquery::{self, NEAREST_DEFAULT_MI, NEAREST_MAX_MI};

use crate::Shared;
use crate::args::{at_props, bad, place, point_named, point_schema};
use crate::tools::{schema, text};

const MI: f64 = 5280.0;

pub fn list() -> Vec<Value> {
    vec![
        json!({ "name": "nearest", "title": "Nearest coast, cliff, river, road or flat", "description": "The nearest coast, cliff, river, road or flat ground to a place within within_mi (default 5, at most 25): its point, distance, direction and what it is. found is false where there is none. Thresholds: MCP.md.", "inputSchema": schema(crate::tools::merge(at_props(), json!({
            "what": { "type": "string", "enum": ["coast", "cliff", "river", "road", "flat"] },
            "within_mi": { "type": "number", "minimum": 0.1, "maximum": NEAREST_MAX_MI, "default": NEAREST_DEFAULT_MI },
        })), &["what"]) }),
        json!({ "name": "road_point", "title": "A point along a road", "description": "A point at_mi or fraction along a road: road (an rd: id or a road's name) from one end (from: a place near it; default the first), or from and to along the road route takes. Returns x_ft, y_ft, heading_deg and the stretch there; past the length is refused with it.", "inputSchema": schema(json!({
            "road": { "type": "string", "description": "An rd: id, or the name of a drawn or renamed road" },
            "from": point_schema("An end of the road (a place near it), or where a route starts: an id, a place's name, {x_ft, y_ft} or [x_ft, y_ft]"),
            "to": point_schema("Where a route ends (instead of road)"),
            "at_mi": { "type": "number", "minimum": 0 },
            "fraction": { "type": "number", "minimum": 0, "maximum": 1 },
        }), &[]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    let r = match name {
        "nearest" => nearest(app, a).await,
        "road_point" => road_point(app, a).await,
        _ => return None,
    };
    Some(r.map(text))
}

async fn nearest(app: &Shared, a: &Value) -> Result<Value, String> {
    let p = place(app, a).await?;
    let what = a["what"].as_str().ok_or_else(|| bad("what", "coast, cliff, river, road or flat", &a["what"]))?.to_string();
    let mi = a["within_mi"].as_f64().unwrap_or(NEAREST_DEFAULT_MI);
    if !(0.1..=NEAREST_MAX_MI).contains(&mi) {
        return Err(bad("within_mi", &format!("0.1 to {NEAREST_MAX_MI} miles"), &a["within_mi"]));
    }
    let mut v = app.worker.with(move |ex| geoquery::nearest(&ex.world, &ex.t0, p, &what, mi * MI)).await?;
    v["from"] = json!({ "x_ft": p[0].round(), "y_ft": p[1].round() });
    v["within_mi"] = json!(mi);
    Ok(v)
}

async fn road_point(app: &Shared, a: &Value) -> Result<Value, String> {
    let (at_mi, fraction) = (a["at_mi"].as_f64(), a["fraction"].as_f64());
    if at_mi.is_some() == fraction.is_some() {
        return Err("at_mi, fraction: give one".into());
    }
    let road = a["road"].as_str().map(str::to_string);
    let mut echo = serde_json::Map::new();
    let from = if a["from"].is_null() {
        None
    } else {
        let (p, e) = point_named(app, "from", &a["from"]).await?;
        if let Some(e) = e {
            echo.insert("from_place".into(), e);
        }
        Some(p)
    };
    let to = match (&road, a["to"].is_null()) {
        (Some(_), false) => return Err("road and to: give one (a road, or from and to for the way route takes)".into()),
        (None, true) => return Err("road, or from and to: give one".into()),
        (None, false) => {
            if from.is_none() {
                return Err("from: needed with to".into());
            }
            let (p, e) = point_named(app, "to", &a["to"]).await?;
            if let Some(e) = e {
                echo.insert("to_place".into(), e);
            }
            Some(p)
        }
        _ => None,
    };
    let mut v = app
        .worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let mut way = match (&road, from, to) {
                (Some(r), _, _) => geoquery::road_way(w, t0, r)?,
                (None, Some(f), Some(t)) => geoquery::route_way(w, t0, f, t)?,
                _ => return Err("road, or from and to: give one".to_string()),
            };
            // Walked from the end nearest `from` (the first end by default).
            if let (Some(_), Some(f)) = (&road, from) {
                let (s, e) = way.ends(t0);
                let d = |q: [f64; 2]| ((q[0] - f[0]).powi(2) + (q[1] - f[1]).powi(2)).sqrt();
                if d(e) < d(s) {
                    way = way.reversed();
                }
            }
            let len = way.length_ft(t0);
            let d = at_mi.map(|m| m * MI).or(fraction.map(|f| f * len)).unwrap_or(0.0);
            let (s, e) = way.ends(t0);
            let Some(pt) = geoquery::point_along(w, t0, &way, d) else {
                return Err(format!("at_mi: {:.2} is past the end: the road is {:.2} mi long ({} ft)", d / MI, len / MI, len.round()));
            };
            let mut r = json!({ "length_mi": (len / MI * 100.0).round() / 100.0, "stretches": way.pieces.len(), "from": worldgen::roadids::end_json(w, t0, s), "to": worldgen::roadids::end_json(w, t0, e) });
            if let Some(n) = &way.name {
                r["name"] = json!(n);
            }
            Ok(json!({ "road": r, "at_mi": (d / MI * 100.0).round() / 100.0, "fraction": ((d / len.max(1.0)) * 1000.0).round() / 1000.0, "x_ft": pt["x_ft"], "y_ft": pt["y_ft"], "heading_deg": pt["heading_deg"], "stretch": pt["stretch"] }))
        })
        .await?;
    for (k, e) in echo {
        v[k] = e;
    }
    Ok(v)
}
