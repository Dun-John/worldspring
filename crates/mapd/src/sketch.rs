//! MCP tools for the world's sketch (the strokes that steer generation: `WorldFile.sketch`,
//! `t0::sketch`) and `regenerate`, which remakes the world with it natively, its edits following
//! their places (`t0::remap::remake`), and tells the open app.
//!
//! The sketch is part of the world (its hash), so changes to it wait in `<hash>/sketch.json`
//! until `regenerate` makes the world they describe. A stroke's address is `sk:` and a hash of
//! the stroke as stored: it stays while other strokes come and go (an index would shift), and a
//! changed stroke gets a new one. Nothing in a world file keeps a stroke's index (generation
//! reads them afresh from the sketch), so removing one shifts no stored reference.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use worldgen::core::hash::fnv64;
use worldgen::world::{NAME_MAX, PIN_KINDS, PIN_TIERS, ROAD_KINDS, Sketch, SketchTool, Stroke, WARDS_MAX};
use worldgen::{World, WorldFile};

use crate::tools::{schema, text};
use crate::{Shared, hex};

const MI: f64 = 5280.0;
/// The tools strokes are added with (`forest`: a biome stroke of temperate forest).
const TOOLS: [&str; 8] = ["land", "sea", "range", "river", "forest", "biome", "road", "pin"];
/// The app's brush radii (mi) by tool.
fn default_radius_mi(tool: &str) -> f64 {
    match tool {
        "land" => 15.0,
        "sea" => 10.0,
        "range" => 15.0,
        "river" => 3.0,
        _ => 20.0,
    }
}

/// Sketch changes not regenerated yet: the strokes as they will be, each with the index of the
/// open world's stroke it came from (None: added).
#[derive(Clone, Default, Serialize, Deserialize)]
struct Pending {
    strokes: Vec<Stroke>,
    from: Vec<Option<usize>>,
}

/// What `regenerate` left for an app that opens the world it remade (`<new>/remade_from.json`).
#[derive(Serialize, Deserialize)]
struct RemadeFrom {
    from: String,
    gone: Vec<worldgen::t0::remap::Gone>,
}

pub fn stroke_id(s: &Stroke) -> String {
    let h = fnv64(serde_json::to_string(s).unwrap_or_default().as_bytes());
    format!("sk:{:08x}", (h ^ h >> 32) as u32)
}

fn bad(field: &str, rule: &str, got: &Value) -> String {
    let mut g = got.to_string();
    if g.len() > 60 {
        g = format!("{}...", g.chars().take(57).collect::<String>());
    }
    format!("{field}: {rule}, got {g}")
}

fn biomes() -> Vec<&'static str> {
    worldgen::t0::biome::ALL.iter().map(|b| b.name()).collect()
}

fn stroke_props(with_tool: bool) -> Value {
    let pt = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    let mut p = json!({
        "path": { "type": "array", "items": pt.clone(), "minItems": 2, "maxItems": 5000, "description": "The line or outline, [[x_ft, y_ft], ...] (every tool but pin): 2+ points, 3+ for an outline" },
        "at": { "oneOf": [{ "type": "string" }, { "type": "object", "properties": { "x_ft": { "type": "number" }, "y_ft": { "type": "number" } }, "required": ["x_ft", "y_ft"] }, pt], "description": "A pin's place: {x_ft, y_ft}, [x_ft, y_ft] or a feature's id" },
        "closed": { "type": "boolean", "description": "land, sea, forest, biome: the path is an outline, filled (default true); false: a band brushed along it, radius_mi wide each side" },
        "radius_mi": { "type": "number", "minimum": 0.1, "maximum": 200, "description": "Brush half-width: a band's, a range's, a river's valley (defaults: land 15, sea 10, range 15, river 3, forest and biome 20)" },
        "strength": { "type": "number", "minimum": 0, "maximum": 1, "description": "range: its height (0.2 hills to 1 high peaks); river: its size (default 0.7)" },
        "hard": { "type": "boolean", "description": "land, sea, forest, biome: edges exactly as drawn, not natural ones" },
        "biome": { "type": "string", "enum": biomes(), "description": "biome: the biome painted (forest: temperate_forest unless given)" },
        "name": { "type": "string", "maxLength": NAME_MAX, "description": "The name of what it makes: a settlement, range, river, forest, region; a road's (else it has none). Empty: none" },
        "kind": { "type": "string", "enum": ROAD_KINDS, "description": "road: its class (default road); none: a line no planned road crosses" },
        "tier": { "type": "string", "enum": PIN_TIERS, "description": "pin: the settlement's size" },
        "lives_by": { "type": "string", "enum": PIN_KINDS, "description": "pin: what it lives by (default: from its land)" },
        "capital": { "type": "boolean", "description": "pin: the realm's capital" },
        "wards": { "type": "array", "items": { "type": "string" }, "maxItems": WARDS_MAX, "description": "pin: names for its districts, the central one first" },
        "added": { "type": "boolean", "description": "pin: beside the world's own settlements (default true), as Edit › Places adds them; false: in place of one of its tier (a sketch pin)" },
        "replaces": { "type": "string", "enum": PIN_TIERS, "description": "pin: in place of the world's own settlement at its spot (within 3 mi), changed: the tier that one was placed at. Nothing else moves; update_settlement makes such pins" },
    });
    if with_tool {
        p["tool"] = json!({ "type": "string", "enum": TOOLS, "description": "What the stroke makes: land, sea (cut out of land), range (mountains along the line), river (from its first point down), forest, biome (a painted biome), road, pin (a settlement)" });
    } else {
        p["id"] = json!({ "type": "string", "description": "The stroke's id (sk:..., from get_sketch)" });
    }
    p
}

pub fn list() -> Vec<Value> {
    let ro = json!({ "readOnlyHint": true, "openWorldHint": false });
    let pending = "Stored at once, but the world is not remade until regenerate (results say pending: true).";
    vec![
        json!({ "name": "get_sketch", "title": "The sketch", "description": "The world's sketch: the strokes that steer generation (land, sea, ranges, massifs, rivers, lakes, biomes, volcanoes, settlement pins, sites, regions, roads), each with its id (sk:..., for update_sketch_stroke and delete_sketch_stroke), its tool, points (ft), and its options (a pin's tier, lives_by, capital, name, added). With changes not regenerated yet, the sketch as they leave it, pending: true, and what changed (added, changed, removed). Ids are kept while other strokes come and go; a changed stroke gets a new one.", "inputSchema": schema(json!({
            "tool": { "type": "string", "description": "Only strokes of this tool (land, sea, range, river, biome, pin, road, massif, elevation, lake, volcano, site, region)" },
            "points": { "type": "boolean", "default": true, "description": "false: each stroke's point count and bounding box instead of its points" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 100 },
        }), &[]), "annotations": ro }),
        json!({ "name": "create_sketch_stroke", "title": "Draw in the sketch", "description": format!("Add a stroke to the world's sketch: land or sea (an outline, or a band along a path), a mountain range or river along a path, a forest or biome painted over an outline, a road along a path (with a name and class; it joins the settlements at and beside it), or a settlement pin (tier, lives_by, capital, name). {pending} The same stroke again adds nothing (existing: true)."), "inputSchema": schema(stroke_props(true), &["tool"]), "annotations": { "openWorldHint": false, "idempotentHint": true } }),
        json!({ "name": "update_sketch_stroke", "title": "Change a stroke", "description": format!("Change a sketch stroke: its points (path, or a pin's at) and options; what is not given stays. Returns its new id. {pending}"), "inputSchema": schema(stroke_props(false), &["id"]), "annotations": { "openWorldHint": false } }),
        json!({ "name": "delete_sketch_stroke", "title": "Erase strokes", "description": format!("Take strokes out of the sketch (all or none). A settlement pin's settlement is taken away by regenerate, and what was made on it set aside (gone:... in the notes), never handed to a neighbour. {pending}"), "inputSchema": schema(json!({
            "ids": { "type": "array", "items": { "type": "string" }, "minItems": 1 },
            "id": { "type": "string" },
        }), &[]), "annotations": { "destructiveHint": true, "openWorldHint": false } }),
        json!({ "name": "regenerate", "title": "Remake the world", "description": "Remake the world with the sketch's pending changes (a few seconds). Its edits follow their places: names, notes and changes on places that come out the same stay on them; changes on places laid out differently, and on settlements whose pins were taken away, are set aside (kept under ids that name nothing; `vanished` shows what they say, and world_overview's places_removed lists them later; get_feature does not read them). The world gets a new hash. An open map app loads the remade world (asking first if it has work not saved). Returns how many changes were kept, set aside and moved, the settlements before and after, those the new pins made, what the sketch could not follow, and how long it took.", "inputSchema": schema(json!({}), &[]), "annotations": { "destructiveHint": true, "openWorldHint": false } }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    if !matches!(name, "get_sketch" | "create_sketch_stroke" | "update_sketch_stroke" | "delete_sketch_stroke" | "regenerate") {
        return None;
    }
    if let Err(e) = known_args(name, a) {
        return Some(Err(e));
    }
    if name != "get_sketch" && crate::batch::active() {
        return Some(Err(format!("{name} can't go in a batch: the sketch is changed (and the world remade) on its own")));
    }
    Some(
        match name {
            "get_sketch" => get(app, a).await,
            "create_sketch_stroke" => create(app, a).await,
            "update_sketch_stroke" => update(app, a).await,
            "delete_sketch_stroke" => delete(app, a).await,
            _ => regenerate(app).await,
        }
        .map(text),
    )
}

/// Arguments the tool doesn't have are refused (with a near name when there is one).
fn known_args(name: &str, a: &Value) -> Result<(), String> {
    let tools = list();
    let Some(t) = tools.iter().find(|t| t["name"] == name) else { return Ok(()) };
    let props = t["inputSchema"]["properties"].as_object().cloned().unwrap_or_default();
    let Some(args) = a.as_object() else { return Ok(()) };
    let near = |k: &str| props.keys().find(|p| p.starts_with(k) || k.starts_with(p.as_str()) || edit_distance(p, k) <= 2).cloned();
    let errs: Vec<String> = args
        .iter()
        .filter(|(k, v)| k.as_str() != "world" && !props.contains_key(k.as_str()) && !v.is_null())
        .map(|(k, _)| match near(k) {
            Some(p) => format!("{k}: unknown argument (did you mean {p}?)"),
            None => format!("{k}: unknown argument"),
        })
        .take(5)
        .collect();
    if errs.is_empty() { Ok(()) } else { Err(errs.join("; ")) }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let cur = row[j];
            row[j] = (prev + usize::from(a[i - 1] != b[j - 1])).min(row[j] + 1).min(row[j - 1] + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The open world (hash, file), and the sketch as its pending changes leave it (or as it is).
fn state(app: &Shared) -> Result<(u64, WorldFile, Pending, bool), String> {
    let (hash, file) = app.world.lock().unwrap().clone().ok_or("no world is open: open the map app first")?;
    if let Some(want) = crate::tools::wanted()
        && want != hash
    {
        return Err(crate::tools::other_world(Some(hash), want));
    }
    let p = load_pending(app, hash).filter(|p| p.strokes.len() == p.from.len());
    let has = p.is_some();
    let p = p.unwrap_or_else(|| Pending { strokes: file.sketch.strokes.clone(), from: (0..file.sketch.strokes.len()).map(Some).collect() });
    Ok((hash, file, p, has))
}

fn load_pending(app: &Shared, hash: u64) -> Option<Pending> {
    serde_json::from_str(&std::fs::read_to_string(app.store.world_path(hash, "sketch.json")).ok()?).ok()
}

/// Keep the pending changes (none left: the file goes). True when some are pending.
fn save_pending(app: &Shared, hash: u64, file: &WorldFile, p: &Pending) -> Result<bool, String> {
    let path = app.store.world_path(hash, "sketch.json");
    let same = p.strokes == file.sketch.strokes && p.from.iter().enumerate().all(|(i, f)| *f == Some(i));
    if same {
        let _ = std::fs::remove_file(&path);
        return Ok(false);
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string(p).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| format!("could not save the sketch's changes to {}: {e}", path.display()))?;
    Ok(true)
}

fn r1(v: f64) -> f64 {
    v.round()
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn is_point(s: &Stroke) -> bool {
    matches!(s.tool, SketchTool::Pin | SketchTool::Volcano | SketchTool::Site) || (s.tool == SketchTool::Region && !s.closed)
}

/// A stroke as the tools give it.
fn item(s: &Stroke, index: usize, points: bool) -> Value {
    let mut v = json!({ "id": stroke_id(s), "index": index, "tool": serde_json::to_value(s.tool).unwrap_or_default() });
    let pts: Vec<[f64; 2]> = s.pts.iter().map(|p| [r1(p[0]), r1(p[1])]).collect();
    if is_point(s) {
        v["at"] = json!(pts[0]);
    } else if points {
        v["path"] = json!(pts);
    } else {
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &pts {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        v["point_count"] = json!(pts.len());
        v["bbox"] = json!([lo[0], lo[1], hi[0], hi[1]]);
    }
    if s.closed {
        v["closed"] = json!(true);
    }
    if !is_point(s) && s.tool != SketchTool::Road {
        v["radius_mi"] = json!(r2(s.radius_ft / MI));
    }
    if matches!(s.tool, SketchTool::Range | SketchTool::River | SketchTool::Massif | SketchTool::Volcano) {
        v["strength"] = json!(r2(s.strength));
    }
    if s.hard {
        v["hard"] = json!(true);
    }
    let opt = |v: &mut Value, k: &str, o: &Option<String>| {
        if let Some(x) = o {
            v[k] = json!(x);
        }
    };
    opt(&mut v, "biome", &s.biome);
    opt(&mut v, "name", &s.name);
    if s.tool == SketchTool::Pin {
        opt(&mut v, "tier", &s.tier);
        opt(&mut v, "lives_by", &s.kind);
        v["capital"] = json!(s.capital);
        v["added"] = json!(s.added);
        opt(&mut v, "replaces", &s.replaces);
        if !s.wards.is_empty() {
            v["wards"] = json!(s.wards);
        }
    } else {
        opt(&mut v, "kind", &s.kind);
    }
    opt(&mut v, "activity", &s.activity);
    opt(&mut v, "under", &s.under);
    for (k, x) in [("trend_deg", s.trend), ("delta_ft", s.delta_ft), ("level_ft", s.level_ft)] {
        if let Some(x) = x {
            v[k] = json!(x);
        }
    }
    if s.salt {
        v["salt"] = json!(true);
    }
    v
}

/// What the pending changes do to the world's sketch: strokes added, changed, and the open
/// world's strokes removed (their ids).
fn changes(file: &WorldFile, p: &Pending) -> Value {
    let applied = &file.sketch.strokes;
    let added = p.from.iter().filter(|f| f.is_none()).count();
    let changed = p.from.iter().zip(&p.strokes).filter(|(f, s)| f.is_some_and(|i| applied.get(i) != Some(*s))).count();
    let removed: Vec<String> = (0..applied.len()).filter(|i| !p.from.contains(&Some(*i))).map(|i| stroke_id(&applied[i])).collect();
    json!({ "added": added, "changed": changed, "removed": removed })
}

async fn get(app: &Shared, a: &Value) -> Result<Value, String> {
    let (_, file, p, pending) = state(app)?;
    let points = a["points"].as_bool().unwrap_or(true);
    let limit = match &a["limit"] {
        Value::Null => 100,
        v => v.as_u64().filter(|l| (1..=1000).contains(l)).ok_or_else(|| bad("limit", "1 to 1000", v))? as usize,
    };
    if !a["points"].is_null() && !a["points"].is_boolean() {
        return Err(bad("points", "true or false", &a["points"]));
    }
    let tool = a["tool"].as_str();
    let all: Vec<Value> = p
        .strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| tool.is_none_or(|t| serde_json::to_value(s.tool).ok().and_then(|v| v.as_str().map(|x| x == t)).unwrap_or(false)))
        .map(|(i, s)| {
            let mut v = item(s, i, points);
            match p.from[i] {
                None => v["pending"] = json!("added"),
                Some(k) if file.sketch.strokes.get(k) != Some(s) => v["pending"] = json!("changed"),
                _ => {}
            }
            v
        })
        .collect();
    let mut all = all;
    settlements_of_pins(app, &p, &mut all).await;
    let found = all.len();
    let mut out = json!({ "items": all.into_iter().take(limit).collect::<Vec<_>>(), "found": found, "pending": pending });
    if found > limit {
        out["truncated"] = json!(true);
    }
    if pending {
        out["changes"] = changes(&file, &p);
        out["note"] = json!("The sketch has changes not regenerated yet: regenerate remakes the world with them");
    }
    Ok(out)
}

/// Each pin's `settlement`: the one its stroke made in the open world, or for a pin not
/// regenerated yet that replaces one (`update_settlement`), the settlement at its spot.
async fn settlements_of_pins(app: &Shared, p: &Pending, items: &mut [Value]) {
    let asks: Vec<(usize, Option<usize>, Option<[f64; 2]>)> = items
        .iter()
        .enumerate()
        .filter_map(|(k, v)| {
            let i = v["index"].as_u64()? as usize;
            let s = p.strokes.get(i).filter(|s| s.tool == SketchTool::Pin)?;
            Some((k, p.from[i], s.replaces.is_some().then(|| s.pts[0])))
        })
        .collect();
    if asks.is_empty() {
        return;
    }
    let ids = app
        .worker
        .with(move |ex| {
            let t0 = &ex.t0;
            let near = |at: [f64; 2]| t0.settlements.iter().enumerate().filter(|(_, s)| s.pin.is_none()).map(|(li, s)| (li, (s.x - at[0]).hypot(s.y - at[1]))).filter(|(_, d)| *d < 3.0 * MI).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(li, _)| li);
            Ok(asks
                .iter()
                .map(|&(k, from, spot)| {
                    // (Its own settlement, the stroke made already; else the one it is to replace.)
                    let li = from.and_then(|i| t0.settlements.iter().position(|s| s.pin == Some(i as u32))).or_else(|| spot.and_then(near));
                    (k, li.and_then(|li| worldgen::agent::feature_of_layout(&ex.world, t0, li)))
                })
                .collect::<Vec<_>>())
        })
        .await
        .unwrap_or_default();
    for (k, id) in ids {
        if let Some(id) = id {
            items[k]["settlement"] = json!(id);
        }
    }
}

/// Put a pin for a settlement of the open world in the pending sketch (`update_settlement`): the
/// applied pin it came from (`from`, its index) as it is pending, else a pending pin replacing the
/// one at `spot` (within 3 mi), else `fresh`; changed by `change`. The stroke, its id, and whether
/// it was the same already.
pub(crate) async fn put_pin(app: &Shared, from: Option<usize>, spot: [f64; 2], fresh: Stroke, change: impl FnOnce(&mut Stroke)) -> Result<Value, String> {
    let _one = app.gate.lock().await;
    let (hash, file, mut p, _) = state(app)?;
    let at = match from {
        Some(k) => Some(p.from.iter().position(|f| *f == Some(k)).ok_or("its pin is taken out of the sketch by changes not regenerated yet (get_sketch lists them): put it back, or regenerate first")?),
        None => p.strokes.iter().zip(&p.from).position(|(s, f)| f.is_none() && s.tool == SketchTool::Pin && s.replaces.is_some() && (s.pts[0][0] - spot[0]).hypot(s.pts[0][1] - spot[1]) < 3.0 * MI),
    };
    let mut s = at.map_or(fresh, |i| p.strokes[i].clone());
    change(&mut s);
    let i = match at {
        Some(i) if p.strokes[i] == s => {
            let mut out = json!({ "stroke": item(&s, i, true), "unchanged": true });
            out["pending"] = json!(load_pending(app, hash).is_some());
            return Ok(out);
        }
        Some(i) => {
            p.strokes[i] = s.clone();
            i
        }
        None => {
            p.strokes.push(s.clone());
            p.from.push(None);
            p.strokes.len() - 1
        }
    };
    check(&file, &p.strokes)?;
    let pending = save_pending(app, hash, &file, &p)?;
    app.store.log(hash, json!({ "time": now(), "author": "agent", "change": { "tool": "update_settlement", "stroke": stroke_id(&s) } }));
    Ok(json!({ "stroke": item(&s, i, true), "pending": pending }))
}

/// A pin stroke at `at` (as `create_sketch_stroke` makes one).
pub(crate) fn pin_stroke(at: [f64; 2], tier: &str) -> Stroke {
    Stroke {
        tool: SketchTool::Pin,
        pts: vec![[at[0].round(), at[1].round()]],
        closed: false,
        radius_ft: default_radius_mi("pin") * MI,
        strength: 0.7,
        hard: false,
        biome: None,
        tier: Some(tier.to_string()),
        name: None,
        trend: None,
        delta_ft: None,
        level_ft: None,
        salt: false,
        kind: None,
        activity: None,
        capital: false,
        wards: vec![],
        added: false,
        under: None,
        replaces: None,
    }
}

/// The map's size (ft).
fn size(file: &WorldFile) -> [f64; 2] {
    [file.params.width_mi * MI, file.params.height_mi * MI]
}

fn off_map(field: &str, p: &Value, wh: [f64; 2]) -> String {
    bad(field, &format!("on the map (0 ≤ x_ft < {:.0} and 0 ≤ y_ft < {:.0})", wh[0], wh[1]), p)
}

/// A point given as {x_ft, y_ft}, [x_ft, y_ft] or a feature's id.
async fn point(app: &Shared, field: &str, v: &Value, wh: [f64; 2]) -> Result<[f64; 2], String> {
    let p = match v {
        Value::String(id) => {
            let (id, f) = (id.clone(), field.to_string());
            app.worker.with(move |ex| worldgen::agent::position(&ex.world, &ex.t0, &id).ok_or_else(|| format!("{f}: no feature {id}"))).await?
        }
        Value::Array(xy) if xy.len() == 2 && xy.iter().all(Value::is_number) => [xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)],
        Value::Object(o) if o.get("x_ft").is_some_and(Value::is_number) && o.get("y_ft").is_some_and(Value::is_number) => [o["x_ft"].as_f64().unwrap_or(0.0), o["y_ft"].as_f64().unwrap_or(0.0)],
        _ => return Err(bad(field, "{x_ft, y_ft}, [x_ft, y_ft] or a feature's id", v)),
    };
    if !(0.0..wh[0]).contains(&p[0]) || !(0.0..wh[1]).contains(&p[1]) {
        return Err(off_map(field, v, wh));
    }
    Ok(p)
}

fn path(v: &Value, wh: [f64; 2]) -> Result<Vec<[f64; 2]>, String> {
    let Some(list) = v.as_array() else { return Err(bad("path", "a list of [x_ft, y_ft]", v)) };
    if list.len() > 5000 {
        return Err(bad("path", "at most 5000 points", &json!(list.len())));
    }
    list.iter()
        .enumerate()
        .map(|(i, p)| {
            let f = format!("path[{i}]");
            let xy = p.as_array().filter(|a| a.len() == 2).and_then(|a| Some([a[0].as_f64()?, a[1].as_f64()?])).ok_or_else(|| bad(&f, "[x_ft, y_ft]", p))?;
            if !(0.0..wh[0]).contains(&xy[0]) || !(0.0..wh[1]).contains(&xy[1]) {
                return Err(off_map(&f, p, wh));
            }
            Ok([xy[0].round(), xy[1].round()])
        })
        .collect()
}

/// The tool a stroke is changed as (`forest` is a biome stroke).
fn tool_key(s: &Stroke) -> &'static str {
    match s.tool {
        SketchTool::Land => "land",
        SketchTool::Sea => "sea",
        SketchTool::Range => "range",
        SketchTool::River => "river",
        SketchTool::Biome => "biome",
        SketchTool::Road => "road",
        SketchTool::Pin => "pin",
        _ => "other",
    }
}

/// Set a stroke's points and options from the arguments (`tool`: what it is, as asked).
async fn shape(app: &Shared, s: &mut Stroke, tool: &str, a: &Value, wh: [f64; 2], new: bool) -> Result<(), String> {
    let only = |k: &str, tools: &[&str]| -> Result<(), String> {
        if !a[k].is_null() && !tools.contains(&tool) {
            return Err(bad(k, &format!("only for {}", tools.join(", ")), &a[k]));
        }
        Ok(())
    };
    only("closed", &["land", "sea", "forest", "biome"])?;
    only("radius_mi", &["land", "sea", "range", "river", "forest", "biome"])?;
    only("strength", &["range", "river"])?;
    only("hard", &["land", "sea", "forest", "biome"])?;
    only("biome", &["forest", "biome"])?;
    only("kind", &["road"])?;
    for k in ["tier", "lives_by", "capital", "wards", "added", "replaces", "at"] {
        only(k, &["pin"])?;
    }
    if tool == "other" {
        // (Strokes the tools don't draw: their points and name can change.)
        for k in ["closed", "radius_mi", "strength", "hard", "biome", "kind"] {
            if !a[k].is_null() {
                return Err(bad(k, "not for this stroke (only path and name)", &a[k]));
            }
        }
    }
    if tool == "pin" {
        if !a["path"].is_null() {
            return Err(bad("path", "a pin has a place, at, not a path", &a["path"]));
        }
        if !a["at"].is_null() {
            let p = point(app, "at", &a["at"], wh).await?;
            s.pts = vec![[p[0].round(), p[1].round()]];
        } else if new {
            return Err("at: a pin needs its place ({x_ft, y_ft}, [x_ft, y_ft] or a feature's id)".into());
        }
    } else if !a["path"].is_null() {
        s.pts = path(&a["path"], wh)?;
    } else if new {
        return Err(format!("path: a {tool} stroke needs its points, [[x_ft, y_ft], ...]"));
    }
    if let Some(c) = a["closed"].as_bool() {
        s.closed = c;
    } else if !a["closed"].is_null() {
        return Err(bad("closed", "true or false", &a["closed"]));
    }
    let need = if s.closed { 3 } else if tool == "pin" { 1 } else { 2 };
    if s.pts.len() < need {
        return Err(bad("path", &format!("{need} or more points{}", if s.closed { " for an outline" } else { "" }), &json!(s.pts.len())));
    }
    if !a["radius_mi"].is_null() {
        let r = a["radius_mi"].as_f64().filter(|r| (0.1..=200.0).contains(r)).ok_or_else(|| bad("radius_mi", "0.1 to 200", &a["radius_mi"]))?;
        s.radius_ft = r * MI;
    }
    if !a["strength"].is_null() {
        s.strength = a["strength"].as_f64().filter(|r| (0.0..=1.0).contains(r)).ok_or_else(|| bad("strength", "0 to 1", &a["strength"]))?;
    }
    if !a["hard"].is_null() {
        s.hard = a["hard"].as_bool().ok_or_else(|| bad("hard", "true or false", &a["hard"]))?;
    }
    let word = |k: &str, of: &[&str]| -> Result<Option<String>, String> {
        match &a[k] {
            Value::Null => Ok(None),
            Value::String(w) if of.contains(&w.as_str()) => Ok(Some(w.clone())),
            v => Err(bad(k, &format!("one of {}", of.join(", ")), v)),
        }
    };
    if let Some(b) = word("biome", &biomes())? {
        s.biome = Some(b);
    }
    if tool == "biome" && s.biome.is_none() {
        return Err(format!("biome: a biome stroke needs its biome: one of {}", biomes().join(", ")));
    }
    if let Some(k) = word("kind", &ROAD_KINDS)? {
        s.kind = Some(k);
    }
    if let Some(t) = word("tier", &PIN_TIERS)? {
        s.tier = Some(t);
    }
    if tool == "pin" && s.tier.is_none() {
        return Err(format!("tier: a pin needs its tier: one of {}", PIN_TIERS.join(", ")));
    }
    if let Some(k) = word("lives_by", &PIN_KINDS)? {
        s.kind = Some(k);
    }
    for (k, f) in [("capital", &mut s.capital as &mut bool), ("added", &mut s.added)] {
        if !a[k].is_null() {
            *f = a[k].as_bool().ok_or_else(|| bad(k, "true or false", &a[k]))?;
        }
    }
    if let Some(t) = word("replaces", &PIN_TIERS)? {
        s.replaces = Some(t);
        // (In place of one, so not beside them, unless asked: then refused.)
        if a["added"].is_null() {
            s.added = false;
        }
    }
    if !a["wards"].is_null() {
        let w = a["wards"].as_array().filter(|w| w.len() <= WARDS_MAX).ok_or_else(|| bad("wards", &format!("a list of up to {WARDS_MAX} names"), &a["wards"]))?;
        s.wards = w.iter().enumerate().map(|(i, n)| n.as_str().map(|n| n.trim().to_string()).filter(|n| !n.is_empty() && n.chars().count() <= NAME_MAX).ok_or_else(|| bad(&format!("wards[{i}]"), &format!("a name of 1 to {NAME_MAX} characters"), n))).collect::<Result<_, _>>()?;
    }
    match &a["name"] {
        Value::Null => {}
        Value::String(n) if n.trim().chars().count() <= NAME_MAX => s.name = Some(n.trim().to_string()).filter(|n| !n.is_empty()),
        v => return Err(bad("name", &format!("a name of at most {NAME_MAX} characters"), v)),
    }
    if s.tool == SketchTool::Road && s.kind.as_deref() == Some("none") && s.name.is_some() {
        return Err("name: a road of kind none is a line no planned road crosses: it has no name".into());
    }
    Ok(())
}

/// A stroke's place in the sketch by its id (the first, should two be the same).
fn find(p: &Pending, field: &str, id: &Value) -> Result<usize, String> {
    let s = id.as_str().ok_or_else(|| bad(field, "a stroke's id (sk:..., from get_sketch)", id))?;
    p.strokes.iter().position(|x| stroke_id(x) == s).ok_or_else(|| bad(field, "a stroke in the sketch (get_sketch lists them)", id))
}

/// Check the sketch as the generator will (`Sketch::validate`, through `World::new`).
fn check(file: &WorldFile, strokes: &[Stroke]) -> Result<(), String> {
    World::new(WorldFile { sketch: Sketch { strokes: strokes.to_vec() }, edits: Default::default(), ..file.clone() }).map(|_| ())
}

fn pending_note(out: &mut Value, pending: bool) {
    out["pending"] = json!(pending);
    out["note"] = json!(if pending { "Stored in the sketch: the world is remade with it by regenerate" } else { "The sketch is as the world was made: nothing to regenerate" });
}

/// A pin far out at sea in the world as it is (no land within 25 mi, as the generator looks for
/// it): what `regenerate` would say, said now. None elsewhere, or for another tool.
async fn sea_warning(app: &Shared, s: &Stroke, strokes: &[Stroke]) -> Option<String> {
    if s.tool != SketchTool::Pin || s.pts.is_empty() {
        return None;
    }
    let p = s.pts[0];
    let far = app
        .worker
        .with(move |ex| {
            let t = &ex.t0;
            let (i, j) = (((p[0] / t.cell_ft) as usize).min(t.coast.w - 1), ((p[1] / t.cell_ft) as usize).min(t.coast.h - 1));
            Ok(t.coast.data[j * t.coast.w + i] as f64 > 25.0 * MI)
        })
        .await
        .ok()?;
    let land = strokes.iter().any(|x| x.tool == SketchTool::Land);
    far.then(|| {
        format!(
            "This pin is far out at sea (no land within 25 mi): regenerate would place no settlement here{}",
            if land { ", unless land the sketch draws reaches it" } else { ". Move it onto land (at), or draw land under it" }
        )
    })
}

/// The first range or massif stored: it lowers the generated mountains map-wide.
fn range_warning(s: &Stroke, strokes: &[Stroke]) -> Option<&'static str> {
    let big = |x: &Stroke| matches!(x.tool, SketchTool::Range | SketchTool::Massif);
    (big(s) && !strokes.iter().any(|x| big(x) && x != s)).then_some("This is the world's first range or massif: drawn ranges replace the generated mountains across the whole world, so the remake changes features, names and settlements far from it. Changes on places that vanish are set aside, and regenerate lists them")
}

/// The warnings a stored stroke earns, in `out["warning"]`.
async fn warn(app: &Shared, out: &mut Value, s: &Stroke, strokes: &[Stroke]) {
    let w: Vec<String> = sea_warning(app, s, strokes).await.into_iter().chain(range_warning(s, strokes).map(String::from)).collect();
    if !w.is_empty() {
        out["warning"] = json!(w.join(". "));
    }
}

async fn create(app: &Shared, a: &Value) -> Result<Value, String> {
    let _one = app.gate.lock().await;
    let (hash, file, mut p, _) = state(app)?;
    let tool = a["tool"].as_str().filter(|t| TOOLS.contains(t)).ok_or_else(|| bad("tool", &format!("one of {}", TOOLS.join(", ")), &a["tool"]))?;
    let stored = match tool {
        "land" => SketchTool::Land,
        "sea" => SketchTool::Sea,
        "range" => SketchTool::Range,
        "river" => SketchTool::River,
        "forest" | "biome" => SketchTool::Biome,
        "road" => SketchTool::Road,
        _ => SketchTool::Pin,
    };
    let area = matches!(tool, "land" | "sea" | "forest" | "biome");
    let mut s = Stroke {
        tool: stored,
        pts: vec![],
        closed: area,
        radius_ft: default_radius_mi(tool) * MI,
        strength: 0.7,
        hard: false,
        biome: (tool == "forest").then(|| "temperate_forest".to_string()),
        tier: None,
        name: None,
        trend: None,
        delta_ft: None,
        level_ft: None,
        salt: false,
        kind: (tool == "road").then(|| "road".to_string()),
        activity: None,
        capital: false,
        wards: vec![],
        added: tool == "pin",
        under: None,
        replaces: None,
    };
    shape(app, &mut s, tool, a, size(&file), true).await?;
    let id = stroke_id(&s);
    if let Some(i) = p.strokes.iter().position(|x| *x == s) {
        let mut out = json!({ "id": id, "existing": true, "stroke": item(&s, i, true), "note": "This stroke is in the sketch already: nothing was added" });
        out["pending"] = json!(load_pending(app, hash).is_some());
        return Ok(out);
    }
    p.strokes.push(s.clone());
    p.from.push(None);
    check(&file, &p.strokes)?;
    let pending = save_pending(app, hash, &file, &p)?;
    app.store.log(hash, json!({ "time": now(), "author": "agent", "change": { "tool": "create_sketch_stroke", "id": id } }));
    let mut out = json!({ "id": id, "stroke": item(&s, p.strokes.len() - 1, true) });
    pending_note(&mut out, pending);
    warn(app, &mut out, &s, &p.strokes).await;
    Ok(out)
}

async fn update(app: &Shared, a: &Value) -> Result<Value, String> {
    let _one = app.gate.lock().await;
    let (hash, file, mut p, _) = state(app)?;
    let i = find(&p, "id", &a["id"])?;
    let mut s = p.strokes[i].clone();
    let tool = match (tool_key(&s), s.biome.as_deref()) {
        ("biome", Some("temperate_forest")) => "forest",
        (t, _) => t,
    };
    shape(app, &mut s, tool, a, size(&file), false).await?;
    let id = stroke_id(&s);
    if s == p.strokes[i] {
        let mut out = json!({ "id": id, "stroke": item(&s, i, true), "unchanged": true });
        out["pending"] = json!(load_pending(app, hash).is_some());
        return Ok(out);
    }
    p.strokes[i] = s.clone();
    check(&file, &p.strokes)?;
    let pending = save_pending(app, hash, &file, &p)?;
    app.store.log(hash, json!({ "time": now(), "author": "agent", "change": { "tool": "update_sketch_stroke", "id": a["id"], "now": id } }));
    let mut out = json!({ "id": id, "was": a["id"], "stroke": item(&s, i, true) });
    pending_note(&mut out, pending);
    warn(app, &mut out, &s, &p.strokes).await;
    Ok(out)
}

async fn delete(app: &Shared, a: &Value) -> Result<Value, String> {
    let _one = app.gate.lock().await;
    let (hash, file, mut p, _) = state(app)?;
    let ids: Vec<(String, Value)> = match (&a["ids"], &a["id"]) {
        (Value::Array(l), _) => l.iter().enumerate().map(|(k, v)| (format!("ids[{k}]"), v.clone())).collect(),
        (Value::Null, Value::Null) => return Err("ids: give the strokes' ids (or one id)".into()),
        (Value::Null, v) => vec![("id".into(), v.clone())],
        (v, _) => return Err(bad("ids", "a list of stroke ids", v)),
    };
    let mut at: Vec<usize> = ids.iter().map(|(f, v)| find(&p, f, v)).collect::<Result<_, _>>()?;
    at.sort_unstable();
    at.dedup();
    let pins = at.iter().filter(|&&i| p.strokes[i].tool == SketchTool::Pin && p.from[i].is_some()).count();
    let removed: Vec<String> = at.iter().map(|&i| stroke_id(&p.strokes[i])).collect();
    for &i in at.iter().rev() {
        p.strokes.remove(i);
        p.from.remove(i);
    }
    check(&file, &p.strokes)?;
    let pending = save_pending(app, hash, &file, &p)?;
    app.store.log(hash, json!({ "time": now(), "author": "agent", "change": { "tool": "delete_sketch_stroke", "ids": removed } }));
    let mut out = json!({ "removed": removed });
    if pins > 0 {
        out["pins_removed"] = json!(pins);
    }
    pending_note(&mut out, pending);
    if pins > 0 && pending {
        out["note"] = json!("Stored in the sketch: regenerate remakes the world with it, and takes the pins' settlements away (what was made on them is set aside)");
    }
    Ok(out)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) async fn regenerate(app: &Shared) -> Result<Value, String> {
    let _one = app.gate.lock().await;
    let (old, file, p, pending) = state(app)?;
    if !pending {
        return Ok(json!({ "regenerated": false, "pending": false, "note": "The sketch has no changes to regenerate: the world is as it was made" }));
    }
    // (The open world's pins not kept: their settlements go on purpose.)
    let removed: Vec<usize> = (0..file.sketch.strokes.len()).filter(|i| file.sketch.strokes[*i].tool == SketchTool::Pin && !p.from.contains(&Some(*i))).collect();
    let applied = file.sketch.strokes.clone();
    let remade = WorldFile { sketch: Sketch { strokes: p.strokes.clone() }, ..file };
    let apps = app.followers().len();
    let swap = app.clone();
    // The remake runs on the generator thread: calls made meanwhile wait for it, and see the old
    // world or the new one, which goes into place there with mapd's copy of the file.
    let r = app.worker.run(move |g| g.remake(remade, &removed, |hash, f| *swap.world.lock().unwrap() = Some((hash, f.clone())))).await?;
    let saved = app.store.save(r.hash, &r.file);
    let mark = RemadeFrom { from: hex(old), gone: r.gone.clone() };
    let _ = std::fs::write(app.store.world_path(r.hash, "remade_from.json"), serde_json::to_string(&mark).unwrap_or_default());
    let _ = std::fs::remove_file(app.store.world_path(old, "sketch.json"));
    let change = json!({ "tool": "regenerate", "from": hex(old), "to": hex(r.hash), "kept": r.kept, "parked": r.parked + r.aside, "removed": r.removed });
    app.store.log(old, json!({ "time": now(), "author": "agent", "change": change }));
    app.store.log(r.hash, json!({ "time": now(), "author": "agent", "change": change }));
    // (To the apps showing the world it was remade from.)
    app.send(json!({ "type": "remade", "world": hex(old), "file": r.file, "gone": r.gone, "kept": r.kept, "parked": r.parked + r.aside, "removed": r.removed }));
    // What the new pins made, and what the sketch can't follow.
    let strokes = p.strokes.clone();
    // (New pins, and pins changed: one put on a settlement to change it again makes it anew.)
    let added: Vec<usize> = p.from.iter().enumerate().filter(|&(i, f)| strokes[i].tool == SketchTool::Pin && f.is_none_or(|j| applied.get(j) != Some(&strokes[i]))).map(|(i, _)| i).collect();
    let (made, conflicts) = app
        .worker
        .with(move |ex| {
            let made: Vec<Value> = added
                .iter()
                .map(|&k| {
                    let li = ex.t0.settlements.iter().position(|s| s.pin == Some(k as u32));
                    let id = li.and_then(|li| worldgen::agent::feature_of_layout(&ex.world, &ex.t0, li));
                    let name = id.as_ref().and_then(|id| ex.t0.extra.as_ref()?.overlay.features.iter().find(|f| f.id == *id).map(|f| f.name.clone()));
                    json!({ "stroke": stroke_id(&strokes[k]), "id": id, "name": name })
                })
                .collect();
            let conflicts: Vec<Value> = ex.t0.extra.as_ref().map_or_else(Vec::new, |e| e.overlay.conflicts.iter().map(|c| json!({ "stroke": strokes.get(c.stroke).map(stroke_id), "message": c.message, "x_ft": r1(c.x), "y_ft": r1(c.y) })).collect());
            Ok((made, conflicts))
        })
        .await?;
    let mut out = json!({
        "regenerated": true,
        "world": hex(r.hash),
        "was": hex(old),
        "seconds": (r.seconds * 10.0).round() / 10.0,
        "settlements": r.after,
        "settlements_before": r.before,
        "kept": r.kept,
        "set_aside": r.parked + r.removed + r.aside,
        "moved": r.moved,
        "changed": r.changed,
        "gone": r.gone.iter().map(|g| json!({ "id": g.id, "name": g.name })).collect::<Vec<_>>(),
        "made": made,
        "pending": false,
        "app": if apps > 0 { "the open map app loads the remade world (asking first if it has work not saved)" } else { "no map app shows this world: it opens the remade one when it connects" },
    });
    if r.removed > 0 {
        out["removed"] = json!(r.removed);
    }
    if !r.vanished.is_empty() {
        out["vanished"] = json!(r.vanished.iter().map(|v| {
            let mut e = json!({ "id": v.id, "kind": v.kind, "name": v.name, "kept_as": v.at, "what": v.what });
            if let Some(n) = &v.renamed {
                e["renamed_to"] = json!(n);
            }
            if let Some(n) = &v.note {
                e["note"] = json!(n);
            }
            if v.hidden {
                e["hidden"] = json!(true);
            }
            if !v.other.is_empty() {
                e["other"] = json!(v.other);
            }
            e
        }).collect::<Vec<_>>());
    }
    let c = &r.changed;
    let mut note = if r.all_stayed {
        String::from("Your changes: all stayed on their places, none had to move or be set aside")
    } else {
        format!(
            "Your changes: {} stayed on their places or followed them to the remade world's numbers (kept), {} moved with a place to its successor, {} set aside under ids that name nothing (set_aside: places laid out differently, taken away, or gone; the vanished list shows what they say, and world_overview's places_removed lists them later)",
            r.kept, r.moved, r.parked + r.removed + r.aside
        )
    };
    note += &format!(
        ". The world: {} settlements before, {} now; {} of the old ones stand where they stood, {} named features are named otherwise ({} of {} kept their ids). Any stroke can redraw the map: the generator works it whole",
        r.before, r.after, c.settlements_in_place, c.features_renamed, c.features_same_id, c.features_before
    );
    out["note"] = json!(note);
    if !conflicts.is_empty() {
        out["conflicts"] = json!(conflicts);
    }
    if let Err(e) = saved {
        app.unsaved.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("mapd: could not save: {e}");
        out["saved"] = json!(false);
        out["warning"] = json!(format!("the world was remade, but could not be saved to disk: {e}. mapd keeps it and tries again every few seconds"));
    }
    Ok(out)
}

/// A tab that says hello with the world mapd's open one was remade from (by `regenerate`, with
/// no app open, or one that didn't load it): told to load the remade one, unless it asks mapd
/// to follow its own (`take`) or brings edits to replace mapd's (`replace`).
pub fn redirect(app: &Shared, hash: u64, take: bool, replace: bool) -> Option<Value> {
    let cur = app.current()?;
    if cur == hash {
        // (An app has the remade world: the world before is anyone's to open again.)
        let _ = std::fs::remove_file(app.store.world_path(cur, "remade_from.json"));
        return None;
    }
    if take || replace {
        return None;
    }
    let mark: RemadeFrom = serde_json::from_str(&std::fs::read_to_string(app.store.world_path(cur, "remade_from.json")).ok()?).ok()?;
    if mark.from != hex(hash) {
        return None;
    }
    let file = app.world.lock().unwrap().as_ref().map(|(_, f)| f.clone())?;
    Some(json!({ "type": "remade", "world": hex(hash), "file": file, "gone": mark.gone }))
}
