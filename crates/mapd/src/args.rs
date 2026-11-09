//! One vocabulary and argument checks for every tool: old names are folded into the canonical
//! ones, then the arguments are checked against the tool's own `inputSchema` (the one truth)
//! before anything runs; and the one place parser (`at`, `near`, `from`, `to`).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{Map, Value, json};

use crate::Shared;

/// Tools that take a place: `at` (an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`), or, as before, `id` or
/// `x_ft` and `y_ft` beside the other arguments.
const PLACE_TOOLS: &[&str] = &[
    "describe_location",
    "features_near",
    "list_roads",
    "get_battlemap",
    "render_view",
    "focus_view",
    "create_feature",
    "ground_heights",
    "shape_ground",
];

/// Old names still accepted: (tool, old, canonical).
const ALIASES: &[(&str, &str, &str)] = &[
    ("list_names", "within", "inside"),
    ("annotate_feature", "text", "notes"),
    ("update_feature", "text", "notes"),
];

/// Keys a tool's code reads that its schema doesn't list (tools of files this module's owner
/// doesn't edit): (tool, key). None are known.
const EXTRA_KEYS: &[(&str, &str)] = &[];

/// The `at` / point schema: an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`.
pub fn point_schema(description: &str) -> Value {
    json!({
        "description": description,
        "oneOf": [
            { "type": "string", "title": "an id" },
            { "type": "object", "title": "{x_ft, y_ft}", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" } }, "required": ["x_ft", "y_ft"] },
            { "type": "array", "title": "[x_ft, y_ft]", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
        ],
    })
}

/// The `at` property of the tools in `PLACE_TOOLS`.
pub fn at_props() -> Value {
    json!({ "at": point_schema("A place: a feature, building, district or site id, {x_ft, y_ft} or [x_ft, y_ft] (feet from the map's top-left corner, x east, y south)") })
}

/// `field: rule, got X` (X as JSON, cut short).
pub fn bad(field: &str, rule: &str, got: &Value) -> String {
    format!("{field}: {rule}, got {}", cut(got))
}

fn cut(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 60 {
        format!("{}...", s.chars().take(57).collect::<String>())
    } else {
        s
    }
}

fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut diag = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let up = row[j];
            row[j] = (diag + usize::from(a[i - 1] != b[j - 1])).min(row[j] + 1).min(row[j - 1] + 1);
            diag = up;
        }
    }
    row[b.len()]
}

/// The nearest of `names` to `word`: within 2 edits, or sharing a prefix of 3 or more letters.
fn suggest<'a>(word: &str, names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let word = word.to_lowercase();
    names
        .into_iter()
        .filter_map(|n| {
            let d = distance(&word, &n.to_lowercase());
            let prefix = word.len() >= 3 && (n.to_lowercase().starts_with(&word) || (n.len() >= 3 && word.starts_with(&n.to_lowercase())));
            (d <= 2 || prefix).then_some((d, n))
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, n)| n)
}

/// Each tool's `inputSchema`, by name (from `tools::list`, once).
fn schemas() -> &'static HashMap<String, Value> {
    static S: OnceLock<HashMap<String, Value>> = OnceLock::new();
    S.get_or_init(|| {
        let list = crate::tools::list();
        list.as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| Some((t["name"].as_str()?.to_string(), t["inputSchema"].clone())))
            .collect()
    })
}

/// A tool's arguments, folded to the canonical names and checked against its schema.
pub fn check(tool: &str, args: &Value) -> Result<Value, String> {
    let Some(schema) = schemas().get(tool) else {
        let hint = suggest(tool, schemas().keys().map(String::as_str))
            .map(|s| format!(" (did you mean {s}?)"))
            .unwrap_or_default();
        return Err(format!("unknown tool '{tool}'{hint}"));
    };
    let mut obj = match args {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        v => return Err(bad("arguments", "an object", v)),
    };
    for (t, old, new) in ALIASES {
        if *t != tool {
            continue;
        }
        if let Some(v) = obj.remove(*old).filter(|v| !v.is_null()) {
            match obj.get(*new).filter(|n| !n.is_null()) {
                Some(n) if *n != v => return Err(format!("{old} and {new} disagree: give one")),
                Some(_) => {}
                None => {
                    obj.insert((*new).to_string(), v);
                }
            }
        }
    }
    if PLACE_TOOLS.contains(&tool) {
        fold_place(&mut obj)?;
    }
    let mut v = Value::Object(obj);
    let extra: Vec<&str> = EXTRA_KEYS.iter().filter(|(t, _)| *t == tool).map(|(_, k)| *k).collect();
    let mut errs = Vec::new();
    walk(schema, &mut v, "", &mut errs, &extra);
    if errs.is_empty() {
        return Ok(v);
    }
    let more = errs.len().saturating_sub(5);
    errs.truncate(5);
    let mut msg = errs.join("; ");
    if more > 0 {
        msg.push_str(&format!("; and {more} more"));
    }
    Err(msg)
}

/// `id` or `x_ft` and `y_ft` (the old ways to give a place) into `at`.
fn fold_place(obj: &mut Map<String, Value>) -> Result<(), String> {
    for k in ["id", "x_ft", "y_ft", "at"] {
        if obj.get(k).is_some_and(Value::is_null) {
            obj.remove(k);
        }
    }
    let (id, x, y) = (obj.remove("id"), obj.remove("x_ft"), obj.remove("y_ft"));
    let old = match (&id, &x, &y) {
        (None, None, None) => return Ok(()),
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => return Err("id and x_ft/y_ft: give one place, or at".into()),
        (None, Some(_), None) => return Err(bad("y_ft", "needed with x_ft", &Value::Null)),
        (None, None, Some(_)) => return Err(bad("x_ft", "needed with y_ft", &Value::Null)),
        (Some(id), None, None) => id.clone(),
        (None, Some(x), Some(y)) => json!({ "x_ft": x, "y_ft": y }),
    };
    match obj.get("at") {
        Some(at) if *at != old => Err(format!("at and {} disagree: give one", if id.is_some() { "id" } else { "x_ft/y_ft" })),
        Some(_) => Ok(()),
        None => {
            obj.insert("at".into(), old);
            Ok(())
        }
    }
}

fn type_name(t: &str) -> &str {
    match t {
        "string" => "a string",
        "number" => "a number",
        "integer" => "a whole number",
        "boolean" => "true or false",
        "array" => "a list",
        "object" => "an object",
        "null" => "null",
        o => o,
    }
}

fn is_type(t: &str, v: &mut Value) -> bool {
    match t {
        "string" => v.is_string(),
        "number" => v.is_number(),
        "integer" => {
            if v.is_i64() || v.is_u64() {
                return true;
            }
            match v.as_f64() {
                Some(f) if f.fract() == 0.0 && f.abs() < 9e15 => {
                    *v = json!(f as i64);
                    true
                }
                _ => false,
            }
        }
        "boolean" => v.is_boolean(),
        "array" => v.is_array(),
        "object" => v.is_object(),
        "null" => v.is_null(),
        _ => true,
    }
}

fn types_of(s: &Value) -> Vec<&str> {
    match &s["type"] {
        Value::String(t) => vec![t.as_str()],
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() { key.to_string() } else { format!("{path}.{key}") }
}

fn num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 9e15 { format!("{}", f as i64) } else { format!("{f}") }
}

fn range(s: &Value, unit: &str) -> Option<String> {
    let (lo, hi) = (s["minimum"].as_f64(), s["maximum"].as_f64());
    let (lo, hi) = (lo.map(num), hi.map(num));
    match (lo, hi) {
        (Some(l), Some(h)) => Some(format!("{l} to {h}{unit}")),
        (Some(l), None) => Some(format!("at least {l}{unit}")),
        (None, Some(h)) => Some(format!("at most {h}{unit}")),
        _ => None,
    }
}

fn items_range(s: &Value) -> Option<String> {
    let (lo, hi) = (s["minItems"].as_u64(), s["maxItems"].as_u64());
    match (lo, hi) {
        (Some(l), Some(h)) if l == h => Some(format!("{l} items")),
        (Some(l), Some(h)) => Some(format!("{l} to {h} items")),
        (Some(l), None) => Some(format!("at least {l} items")),
        (None, Some(h)) => Some(format!("at most {h} items")),
        _ => None,
    }
}

/// Checks `v` against `s`: type, enum, range, items, unknown and missing keys; collects the errors.
/// Nulls of optional keys are dropped (as if not given) and whole-number floats made integers.
fn walk(s: &Value, v: &mut Value, path: &str, errs: &mut Vec<String>, extra: &[&str]) {
    let name = if path.is_empty() { "arguments" } else { path };
    if let Some(forms) = s["oneOf"].as_array().or_else(|| s["anyOf"].as_array()) {
        let mut all = Vec::new();
        for f in forms {
            let (mut c, mut e) = (v.clone(), Vec::new());
            walk(f, &mut c, path, &mut e, &[]);
            if e.is_empty() {
                *v = c;
                return;
            }
            all.push((f, e));
        }
        // The one form of the value's own type tells what is wrong with it; else name the forms.
        let same: Vec<_> = all.iter().filter(|(f, _)| types_of(f).iter().any(|t| is_type(t, &mut v.clone()))).collect();
        if let [(_, e)] = same[..] {
            errs.extend(e.iter().cloned());
        } else {
            let names: Vec<String> = forms
                .iter()
                .map(|f| {
                    f["title"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| types_of(f).iter().map(|t| type_name(t)).collect::<Vec<_>>().join(" or "))
                })
                .collect();
            let (last, head) = names.split_last().expect("forms");
            let rule = if head.is_empty() { last.clone() } else { format!("{} or {last}", head.join(", ")) };
            errs.push(bad(name, &rule, v));
        }
        return;
    }
    let types = types_of(s);
    if !types.is_empty() && !types.iter().any(|t| is_type(t, v)) {
        let rule = types.iter().map(|t| type_name(t)).collect::<Vec<_>>().join(" or ");
        errs.push(bad(name, &rule, v));
        return;
    }
    if let Some(en) = s["enum"].as_array()
        && !en.contains(v)
    {
        let names: Vec<String> = en.iter().map(|e| e.as_str().map(str::to_string).unwrap_or_else(|| e.to_string())).collect();
        let shown = if names.len() > 14 {
            format!("{}, ...", names[..14].join(", "))
        } else {
            names.join(", ")
        };
        let hint = v
            .as_str()
            .and_then(|w| suggest(w, names.iter().map(String::as_str)))
            .map(|h| format!(" (did you mean {h}?)"))
            .unwrap_or_default();
        errs.push(format!("{}{hint}", bad(name, &format!("one of {shown}"), v)));
        return;
    }
    if let Some(f) = v.as_f64()
        && (s["minimum"].as_f64().is_some_and(|m| f < m) || s["maximum"].as_f64().is_some_and(|m| f > m))
    {
        errs.push(bad(name, &range(s, "").unwrap_or_default(), v));
        return;
    }
    match v {
        Value::Array(list) => {
            if s["minItems"].as_u64().is_some_and(|m| (list.len() as u64) < m) || s["maxItems"].as_u64().is_some_and(|m| list.len() as u64 > m) {
                errs.push(bad(name, &items_range(s).unwrap_or_default(), &json!(list.len())));
                return;
            }
            if s["items"].is_object() {
                for (i, item) in list.iter_mut().enumerate() {
                    walk(&s["items"], item, &format!("{path}[{i}]"), errs, &[]);
                }
            }
        }
        Value::Object(obj) => {
            let Some(props) = s["properties"].as_object() else { return };
            let required: Vec<&str> = s["required"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            obj.retain(|k, x| !x.is_null() || required.contains(&k.as_str()));
            let unknown: Vec<String> = obj.keys().filter(|k| !props.contains_key(*k) && !extra.contains(&k.as_str())).cloned().collect();
            for k in unknown {
                let hint = suggest(&k, props.keys().map(String::as_str).filter(|p| !obj.contains_key(*p)))
                    .map(|h| format!(" (did you mean {h}?)"))
                    .unwrap_or_default();
                errs.push(format!("{}: unknown argument{hint}", join(path, &k)));
            }
            for k in &required {
                if !obj.contains_key(*k) {
                    errs.push(format!("{}: required", join(path, k)));
                }
            }
            for (k, x) in obj.iter_mut() {
                if let Some(ps) = props.get(k) {
                    walk(ps, x, &join(path, k), errs, &[]);
                }
            }
        }
        _ => {}
    }
}

/// The map's size in feet.
async fn map_size(app: &Shared) -> Result<[f64; 2], String> {
    app.worker.with(|ex| Ok([ex.world.geom.map_w_ft, ex.world.geom.map_h_ft])).await
}

/// A point from a value: an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`; `field` names it in errors. Off the map is refused.
pub async fn point(app: &Shared, field: &str, v: &Value) -> Result<[f64; 2], String> {
    const FORMS: &str = "an id, {x_ft, y_ft} or [x_ft, y_ft]";
    let xy = match v {
        Value::String(id) => {
            let id = id.clone();
            return app
                .worker
                .with(move |ex| worldgen::agent::position(&ex.world, &ex.t0, &id).ok_or_else(|| format!("no such feature: {id}")))
                .await;
        }
        Value::Object(o) => (o.get("x_ft").and_then(Value::as_f64), o.get("y_ft").and_then(Value::as_f64)),
        Value::Array(a) if a.len() == 2 => (a[0].as_f64(), a[1].as_f64()),
        _ => return Err(bad(field, FORMS, v)),
    };
    let (Some(x), Some(y)) = xy else { return Err(bad(field, FORMS, v)) };
    let [w, h] = map_size(app).await?;
    if !(0.0..=w).contains(&x) || !(0.0..=h).contains(&y) {
        return Err(format!("{field}: ({}, {}) is off the map, which is {} by {} ft", num(x), num(y), num(w), num(h)));
    }
    Ok([x, y])
}

/// The place a call gives: `at`, else (as before) `id` or `x_ft` and `y_ft` beside its other arguments.
pub async fn place(app: &Shared, a: &Value) -> Result<[f64; 2], String> {
    let mut o = a.as_object().cloned().unwrap_or_default();
    fold_place(&mut o)?;
    match o.get("at") {
        Some(at) => point(app, "at", at).await,
        None => Err("at: an id, {x_ft, y_ft} or [x_ft, y_ft] is needed".into()),
    }
}
