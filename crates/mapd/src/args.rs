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
    "nearest",
    "features_near",
    "list_roads",
    "get_battlemap",
    "render_view",
    "focus_view",
    "create_feature",
    "create_settlement",
    "ground_heights",
    "shape_ground",
];

/// Old names still accepted: (tool, old, canonical).
const ALIASES: &[(&str, &str, &str)] = &[
    ("list_names", "within", "inside"),
    ("annotate_feature", "text", "notes"),
    ("update_feature", "text", "notes"),
    ("create_npc", "text", "notes"),
    ("update_npc", "text", "notes"),
    ("create_plot", "notes", "text"),
    ("update_plot", "notes", "text"),
    ("upload_sprite", "blocks_move", "blocks_movement"),
    ("upload_sprite", "size", "size_squares"),
    ("remove_objects", "area", "within"),
    ("restore_objects", "area", "within"),
    ("remove_objects", "at", "points"),
    ("remove_buildings", "area", "within"),
    ("set_plateau", "outline", "poly"),
    ("create_feature", "outline", "poly"),
    ("create_building", "outline", "poly"),
    ("update_building", "outline", "poly"),
    ("check_building_spot", "outline", "poly"),
    ("create_feature", "path", "pts"),
    ("update_fortification", "path", "pts"),
    ("update_fortification", "outline", "poly"),
];

/// Keys a tool's code reads that its schema doesn't list (tools of files this module's owner
/// doesn't edit): (tool, key). None are known.
const EXTRA_KEYS: &[(&str, &str)] = &[];

/// The `at` / point schema: an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`.
pub fn point_schema(description: &str) -> Value {
    json!({
        "description": description,
        "oneOf": [
            { "type": "string" },
            { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" } }, "required": ["x_ft", "y_ft"] },
            { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
        ],
    })
}

/// The `at` property of the tools in `PLACE_TOOLS`.
pub fn at_props() -> Value {
    json!({ "at": point_schema("A place (ft from the map's top-left, x east, y south)") })
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
pub fn suggest<'a>(word: &str, names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
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
    let mut errs = Vec::new();
    for (t, old, new) in ALIASES {
        if *t != tool {
            continue;
        }
        if let Some(v) = obj.remove(*old).filter(|v| !v.is_null()) {
            match obj.get(*new).filter(|n| !n.is_null()) {
                Some(n) if !same(n, &v) => errs.push(format!("{old} and {new} disagree: give one")),
                Some(_) => {}
                None => {
                    obj.insert((*new).to_string(), v);
                }
            }
        }
    }
    if PLACE_TOOLS.contains(&tool) {
        if let Err(e) = fold_place(&mut obj) {
            errs.push(e);
        }
    }
    if matches!(tool, "create_building" | "update_building" | "check_building_spot") {
        if let Some(Value::String(f)) = obj.get_mut("func") {
            *f = f.trim().to_lowercase();
        }
    }
    let mut v = Value::Object(obj);
    let extra: Vec<&str> = EXTRA_KEYS.iter().filter(|(t, _)| *t == tool).map(|(_, k)| *k).collect();
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

/// Equal as JSON, numbers by value (`5` and `5.0` agree), also inside lists and objects.
pub fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, p)| y.get(k).is_some_and(|q| same(p, q))),
        _ => a == b,
    }
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
        (Some(id), None, None) => Some(id.clone()),
        (None, Some(x), Some(y)) => Some(json!({ "x_ft": x, "y_ft": y })),
        _ => None,
    };
    // `at` beside the old ways: the same place twice is fine, two places are not.
    if let Some(at) = obj.get("at") {
        return match old {
            Some(o) if same(&o, at) => Ok(()),
            _ => Err(format!("at and {} disagree: give one", if id.is_some() { "id" } else { "x_ft/y_ft" })),
        };
    }
    match old {
        Some(o) => {
            obj.insert("at".into(), o);
            Ok(())
        }
        None if x.is_some() => Err("y_ft: needed with x_ft".into()),
        None => Err("x_ft: needed with y_ft".into()),
    }
}

/// The ids a remover or restorer is given: `id` (one) or `ids` (a list), either; both only when the same.
pub fn ids(a: &Value) -> Result<Vec<String>, String> {
    let one = a["id"].as_str().map(str::to_string);
    let many: Option<Vec<String>> = a["ids"].as_array().map(|l| l.iter().filter_map(|v| v.as_str().map(str::to_string)).collect());
    let mut out = match (one, many) {
        (None, None) => return Err("give id or ids".into()),
        (Some(o), None) => vec![o],
        (None, Some(m)) => m,
        (Some(o), Some(m)) if m.len() == 1 && m[0] == o => m,
        (Some(_), Some(_)) => return Err("id and ids disagree: give one".into()),
    };
    let mut seen = std::collections::HashSet::new();
    out.retain(|i| seen.insert(i.clone()));
    if out.is_empty() {
        return Err(bad("ids", "at least one id", &json!([])));
    }
    Ok(out)
}

/// How an item of `ids` is named in an error: `id` when one was given as `id`, else `ids[i]`.
pub fn item(single: bool, i: usize) -> String {
    if single { "id".to_string() } else { format!("ids[{i}]") }
}

/// `id` and `ids`, for a remover's or restorer's schema.
pub fn ids_props(what: &str) -> Value {
    json!({
        "id": { "type": "string", "description": format!("One {what} (or ids)") },
        "ids": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "Several (all or none), or id" },
    })
}

/// The `within` schema: a box, a polygon or a circle (`max_radius_ft` its largest radius).
pub fn area_schema(description: &str, max_radius_ft: f64) -> Value {
    let num = json!({ "type": "number" });
    json!({
        "description": description,
        "oneOf": [
            { "type": "array", "title": "a box [x0, y0, x1, y1]", "items": num, "minItems": 4, "maxItems": 4 },
            { "type": "array", "title": "a polygon [[x, y], ...]", "items": { "type": "array", "items": num, "minItems": 2, "maxItems": 2 }, "minItems": 3 },
            { "type": "object", "title": "a circle {at, radius_ft}", "properties": {
                "at": point_schema("The middle"),
                "x_ft": num, "y_ft": num,
                "radius_ft": { "type": "number", "minimum": 0, "maximum": max_radius_ft },
            }, "required": ["radius_ft"] },
        ],
    })
}

/// A short `within` for read tools (the list tools' schemas stay small): the same three forms, checked by `area`.
pub fn area_light(description: &str) -> Value {
    json!({ "type": ["array", "object"], "description": description })
}

/// An area of the map (world ft).
#[derive(Clone, Debug)]
pub enum Area {
    Box([f64; 4]),
    Poly(Vec<[f64; 2]>),
    Circle([f64; 2], f64),
}

impl Area {
    pub fn contains(&self, p: [f64; 2]) -> bool {
        match self {
            Area::Box(b) => p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3],
            Area::Circle(c, r) => (p[0] - c[0]).hypot(p[1] - c[1]) <= *r,
            Area::Poly(q) => {
                let mut inside = false;
                let mut j = q.len() - 1;
                for i in 0..q.len() {
                    if (q[i][1] > p[1]) != (q[j][1] > p[1]) && p[0] < (q[j][0] - q[i][0]) * (p[1] - q[i][1]) / (q[j][1] - q[i][1]) + q[i][0] {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }

    /// The same area for worldgen's queries.
    pub fn region(&self) -> worldgen::geoquery::Region {
        use worldgen::geoquery::Region;
        match self {
            Area::Box(b) => Region::Box(*b),
            Area::Poly(q) => Region::Poly(q.clone()),
            Area::Circle(c, r) => Region::Circle(*c, *r),
        }
    }

    /// The middle: a box's or circle's centre, a polygon's corners' mean.
    pub fn middle(&self) -> [f64; 2] {
        match self {
            Area::Box(b) => [0.5 * (b[0] + b[2]), 0.5 * (b[1] + b[3])],
            Area::Circle(c, _) => *c,
            Area::Poly(q) => [q.iter().map(|p| p[0]).sum::<f64>() / q.len() as f64, q.iter().map(|p| p[1]).sum::<f64>() / q.len() as f64],
        }
    }

    /// A polygon that holds the whole area (its own, or a box round a circle).
    pub fn outline(&self) -> Vec<[f64; 2]> {
        match self {
            Area::Box(b) => vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]],
            Area::Poly(q) => q.clone(),
            Area::Circle(c, r) => vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] - r], [c[0] + r, c[1] + r], [c[0] - r, c[1] + r]],
        }
    }
}

/// An area from a `within` value (already checked against `area_schema`); `field` names it in errors.
pub async fn area(app: &Shared, field: &str, v: &Value) -> Result<Area, String> {
    match v {
        Value::Array(a) if a.len() == 4 && a.iter().all(Value::is_number) => {
            let n: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
            if n[2] <= n[0] || n[3] <= n[1] {
                return Err(bad(field, "a box [x0, y0, x1, y1] with x1 > x0 and y1 > y0", v));
            }
            Ok(Area::Box([n[0], n[1], n[2], n[3]]))
        }
        Value::Array(a) => {
            let q: Option<Vec<[f64; 2]>> = a.iter().map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect();
            q.filter(|q| q.len() >= 3).map(Area::Poly).ok_or_else(|| bad(field, "a polygon of 3 or more [x_ft, y_ft] corners", v))
        }
        Value::Object(o) => {
            let r = o.get("radius_ft").and_then(Value::as_f64).ok_or_else(|| format!("{field}.radius_ft: required"))?;
            if !(r > 0.0 && r.is_finite()) {
                return Err(bad(&format!("{field}.radius_ft"), "more than 0", &o["radius_ft"]));
            }
            let c = match (o.get("at"), o.get("x_ft"), o.get("y_ft")) {
                (Some(_), Some(_), _) | (Some(_), _, Some(_)) => return Err(format!("{field}: at and x_ft/y_ft: give one middle")),
                (Some(at), None, None) => point(app, &format!("{field}.at"), at).await?,
                (None, Some(x), Some(y)) => point(app, field, &json!({ "x_ft": x, "y_ft": y })).await?,
                (None, Some(_), None) => return Err(format!("{field}.y_ft: needed with x_ft")),
                (None, None, Some(_)) => return Err(format!("{field}.x_ft: needed with y_ft")),
                (None, None, None) => return Err(format!("{field}: a circle needs at (or x_ft and y_ft)")),
            };
            Ok(Area::Circle(c, r))
        }
        _ => Err(bad(field, "a box, a polygon or a circle {at, radius_ft}", v)),
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
        (Some(l), Some(h)) if l == h => Some(format!("{l} {}", if l == 1 { "item" } else { "items" })),
        (Some(l), Some(h)) => Some(format!("{l} to {h} items")),
        (Some(l), None) => Some(format!("at least {l} {}", if l == 1 { "item" } else { "items" })),
        (None, Some(h)) => Some(format!("at most {h} {}", if h == 1 { "item" } else { "items" })),
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
        // (A point's three forms carry no titles, to keep tools/list small: named here.)
        let point = forms.iter().any(|f| f["properties"]["x_ft"].is_object() && f["required"].is_array() && f["title"].is_null());
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
                        .or_else(|| point.then(|| match f["type"].as_str() {
                            Some("string") => "an id",
                            Some("object") => "{x_ft, y_ft}",
                            _ => "[x_ft, y_ft]",
                        }.to_string()))
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
            // (A null is as good as left out, except where the schema allows null: `in_building: null` means out.)
            obj.retain(|k, x| !x.is_null() || required.contains(&k.as_str()) || props.get(k).is_some_and(|p| p["type"].as_array().is_some_and(|t| t.iter().any(|t| t == "null"))));
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

/// What to say of an id that names no place: a notebook id that is not on the map says what it is.
pub fn no_such(world: &worldgen::world::World, t0: &worldgen::t0::T0, id: &str) -> String {
    let e = &world.file.edits;
    if let Some(n) = worldgen::agent::deleted_note(world, t0, id) {
        n
    } else if e.npcs.contains_key(id) {
        format!("{id} is an NPC with no location")
    } else if e.plots.contains_key(id) {
        format!("{id} is a plot point, not a place")
    } else if id.starts_with("rd:") {
        format!("no road stretch {id} (list_roads gives the ids; a stretch whose ends a remake or sketch change moved has a new one)")
    } else if id.starts_with("v:") {
        format!("no crossing {id} (list_crossings gives the ids)")
    } else {
        format!("no such feature: {id}")
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
                .with(move |ex| worldgen::agent::position(&ex.world, &ex.t0, &id).ok_or_else(|| no_such(&ex.world, &ex.t0, &id)))
                .await;
        }
        Value::Object(o) => (o.get("x_ft").and_then(Value::as_f64), o.get("y_ft").and_then(Value::as_f64)),
        Value::Array(a) if a.len() == 2 => (a[0].as_f64(), a[1].as_f64()),
        _ => return Err(bad(field, FORMS, v)),
    };
    let (Some(x), Some(y)) = xy else { return Err(bad(field, FORMS, v)) };
    let [w, h] = map_size(app).await?;
    if !(0.0..w).contains(&x) || !(0.0..h).contains(&y) {
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

/// Like `point`, and a string that is no id may be a place's name (case-insensitive, the whole name):
/// one place called that is used, and comes back as `{id, name, kind}` to echo; several are refused,
/// listing up to 5 with their ids and kinds; none is refused with a did-you-mean.
pub async fn point_named(app: &Shared, field: &str, v: &Value) -> Result<([f64; 2], Option<Value>), String> {
    let Value::String(s) = v else { return Ok((point(app, field, v).await?, None)) };
    let name = s.clone();
    let (field, field_out) = (field.to_string(), field);
    let r = app
        .worker
        .with(move |ex| {
            if let Some(p) = worldgen::agent::position(&ex.world, &ex.t0, &name) {
                return Ok((Some(p), None));
            }
            // Something shaped like an id (`b:3:7`) that is none is refused as before.
            if name.contains(':') && !name.contains(' ') {
                return Err(no_such(&ex.world, &ex.t0, &name));
            }
            let (exact, close) = worldgen::geoquery::places_named(&ex.world, &ex.t0, &name);
            let at = |m: &Value| match (m["x_ft"].as_f64(), m["y_ft"].as_f64()) {
                (Some(x), Some(y)) => Some([x, y]),
                _ => m["id"].as_str().and_then(|id| worldgen::agent::position(&ex.world, &ex.t0, id)),
            };
            let echo = |m: &Value| json!({ "id": m["id"], "name": m["name"], "kind": m["kind"] });
            match exact.as_slice() {
                [m] => Ok((at(m), Some(echo(m)))),
                [] => {
                    let fs = worldgen::agent::features(&ex.world, &ex.t0);
                    let mut near: Vec<String> = close.iter().filter_map(|m| m["name"].as_str().map(str::to_string)).take(3).collect();
                    if near.is_empty() {
                        near.extend(suggest(&name, fs.iter().map(|f| f.name.as_str())).map(str::to_string));
                    }
                    let hint = if near.is_empty() { String::new() } else { format!(" (did you mean {}?)", near.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(" or ")) };
                    Err(format!("{field}: no place with the id or name \"{name}\"{hint}"))
                }
                many => {
                    let list: Vec<String> = many.iter().take(5).map(|m| format!("{} ({})", m["id"].as_str().unwrap_or("?"), m["kind"].as_str().unwrap_or("?"))).collect();
                    Err(format!("{field}: {} places are called \"{name}\": {}{}; give an id", many.len(), list.join(", "), if many.len() > 5 { ", ..." } else { "" }))
                }
            }
        })
        .await?;
    match r {
        (Some(p), echo) => {
            let [w, h] = map_size(app).await?;
            if !(0.0..w).contains(&p[0]) || !(0.0..h).contains(&p[1]) {
                return Err(format!("{field_out}: ({}, {}) is off the map", num(p[0]), num(p[1])));
            }
            Ok((p, echo))
        }
        _ => Err(format!("{field_out}: that place has no position")),
    }
}
