//! The `set_` tools of features, sites, settlements, buildings and the notebook: one tool makes a thing (no `id`) or
//! changes it (`id`), each mode running its family's handler under the handler's own name (so the changes keep their
//! `"tool"` labels, the app's history vocabulary). A mode's arguments are checked against its handler's schema
//! (`route::fit`); a call with parts for several handlers is one change (`batch::as_one`).

use serde_json::{Map, Value, json};

use crate::Shared;
use crate::route::fit;
use crate::tools::{Run, schema, text};

/// The public schema of a merged tool: the properties of `olds` (each with its renames: (ours, the handler's)), less
/// `drop`, plus `extra`.
fn union(olds: &[(&str, &[(&str, &str)])], drop: &[&str], extra: Value, required: &[&str]) -> Value {
    let mut props = Map::new();
    for (old, renames) in olds {
        let s = crate::read::old_schema(old);
        for (k, p) in s["properties"].as_object().into_iter().flatten() {
            let k = renames.iter().find(|(_, w)| w == k).map(|(n, _)| (*n).to_string()).unwrap_or_else(|| k.clone());
            if !drop.contains(&k.as_str()) {
                props.entry(k).or_insert_with(|| p.clone());
            }
        }
    }
    if let Value::Object(x) = extra {
        props.extend(x);
    }
    props.remove("world");
    schema(Value::Object(props), required)
}

const PATH: &[(&str, &str)] = &[("path", "pts")];
const NEAR: &[(&str, &str)] = &[("at", "near")];
const STAMP: &[(&str, &str)] = &[("stamp", "kind"), ("dry_run", "preview")];
const ROW: &[(&str, &str)] = &[("path", "along"), ("dry_run", "preview")];

pub fn list() -> Vec<Value> {
    let id = |what: &str| json!({ "id": { "type": "string", "description": what } });
    let dry = json!({ "dry_run": { "type": "boolean", "default": false, "description": "Only say what it would do (nothing changes)" } });
    let mut feature = union(&[("rename_feature", &[]), ("annotate_feature", &[]), ("update_feature", &[]), ("hide_feature", &[])], &["in_building", "way", "at"], json!({}), &["id"]);
    feature["properties"]["name"]["description"] = json!("Empty restores the original");
    let mut site = union(&[("create_feature", PATH), ("update_fortification", PATH)], &[], id("A site to change (castle or wall: its outline and parts; an entrance: in_building, way; any: name)"), &[]);
    site["properties"]["in_building"] = json!({ "type": ["string", "null"], "description": "An entrance in this building's cellar (b:<layout>:<n> or a drawn building's c: id); with id, null takes it out" });
    let settlement = union(&[("create_settlement", &[]), ("update_settlement", &[])], &[], id("A settlement to change"), &[]);
    let mut building = union(&[("create_building", NEAR), ("update_building", &[]), ("check_building_spot", &[]), ("place_stamp", STAMP)], &[], dry.clone(), &[]);
    building["properties"]["id"] = json!({ "type": "string", "description": "A building to change (b:<layout>:<n>, or a drawn one's c: id)" });
    building["properties"]["stamp"]["description"] = json!("A building of this kind fronting the nearest street at at (with size, facing)");
    let row = union(&[("create_building_row", ROW)], &[], json!({}), &["path"]);
    let parcel = union(&[("create_parcel", &[]), ("update_parcel", &[])], &[], json!({}), &[]);
    let mut npc = union(&[("create_npc", &[]), ("update_npc", &[])], &["location", "x_ft", "y_ft"], id("An NPC to change (n:...)"), &[]);
    npc["properties"]["at"] = json!({ "type": ["string", "object", "array", "null"], "description": "Where they are: a place id (with level inside), {x_ft, y_ft} or [x_ft, y_ft]; null takes them off the map" });
    let plot = union(&[("create_plot", &[]), ("update_plot", &[])], &[], json!({}), &[]);
    let note = union(&[("add_note_at", &[]), ("move_note", &[])], &[], id("A point note (m:...) to change or move"), &[]);
    vec![
        json!({ "name": "set_feature", "title": "Name, notes, hide", "description": "Rename anything named (a feature, building, district, tower, site, level l:, room r:, road stretch rd:, crossing v:; empty name restores the original), set or append its notes and tags (remove_tags takes some off), or hide it from labels and search (hidden). A building drawn by hand is one place: either of its ids.", "inputSchema": feature }),
        json!({ "name": "set_site", "title": "Make or change a site", "description": "Without id, make one (kind): ruin (under: dungeon, crypt, catacombs), tower, camp, waystation, cave, mine, lava_tube, entrance (in_building, way); underground ones take size, levels, theme; castle (poly or rect) or wall (path or around_patches), with gates, keep, moat... (remove_in_way removes buildings in the way). With id: a castle's or wall's outline and parts, an entrance's in_building and way, any site's name.", "inputSchema": site }),
        json!({ "name": "set_settlement", "title": "Make or change a settlement", "description": "Without id: add a village or town at a point (at, tier; name, culture, lives_by, link, blank). With id: change a settlement's tier, culture, lives_by, capital, name (regenerate remakes its layout).", "inputSchema": settlement }),
        json!({ "name": "set_building", "title": "Make or change a building", "description": "Without id, draw one: poly, rect or circle (or at with width_ft x depth_ft: the nearest clear spot on a street), or a stamp (house, inn, smithy...) at at fronting the nearest street (size, facing). With id, change any building (shape, func, floors, roof, material, shade, tint, name). dry_run: true says whether a footprint fits (its blockers) or where a stamp would land, changing nothing.", "inputSchema": building }),
        json!({ "name": "set_building_row", "title": "A row of buildings", "description": "A row of lots along a path (or a road) on one side, with kinds, depth and gaps; clear: true takes the world's own buildings away. dry_run: true only lists the lots.", "inputSchema": row }),
        json!({ "name": "set_parcel", "title": "Mark or change a plot of ground", "description": "Without id: a free plot of ground (poly) with a fill: lots left empty for buildings by hand, or open ground. With id (c:<n>): its outline, fill or name. Returns its id. (set_plot is a notebook plot point.)", "inputSchema": parcel }),
        json!({ "name": "set_npc", "title": "Make or change an NPC", "description": "Without id: add an NPC (name; appearance, mannerisms, attitude, goals, notes, portrait, stance, status, tags). With id: change the fields given. at puts them somewhere (a place id, and level inside it; or a point), null takes them off the map. Returns the NPC with its id.", "inputSchema": npc }),
        json!({ "name": "set_plot", "title": "Make or change a plot point", "description": "Without id: add a plot point (title; text, status, anchors: places, npcs, tags). With id: change the fields given (lists are replaced). Returns it with its id.", "inputSchema": plot }),
        json!({ "name": "set_note", "title": "Make or change a point note", "description": "Without id: a note at a point in a building, dungeon or sewer level (site, level, at as [x, y] squares; or room) with text and tags; returns its m: id. With id: move it (at, or room on its level) and change its text and tags. Notes on places are set_feature's.", "inputSchema": note }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    let r = match name {
        "set_feature" => feature(app, a).await,
        "set_site" => site(app, a).await,
        "set_settlement" => one(app, a, ("update_settlement", "create_settlement"), Run::Settlements).await,
        "set_building" => building(app, a).await,
        "set_building_row" => {
            let b = dry(a);
            match fit(&b, "create_building_row", ROW, "set_building_row") {
                Ok(b) => run(app, vec![(Run::Quick, "create_building_row", b)], dried(a)).await,
                Err(e) => Err(e),
            }
        }
        "set_parcel" => one(app, a, ("update_parcel", "create_parcel"), Run::Quick).await,
        "set_npc" => npc(app, a).await,
        "set_plot" => match foreign(a, "p:", "set_plot") {
            Err(e) => Err(e),
            Ok(()) => one(app, a, ("update_plot", "create_plot"), Run::Notebook).await,
        },
        "set_note" => note(app, a).await,
        _ => return None,
    };
    Some(r.map(text))
}

fn given(a: &Value, k: &str) -> bool {
    !a[k].is_null()
}

/// The args with `dry_run: false` left out (the handlers' `preview` is the same left out).
fn dry(a: &Value) -> Value {
    let mut b = a.clone();
    if b["dry_run"] == json!(false) {
        b.as_object_mut().map(|o| o.remove("dry_run"));
    }
    b
}

fn dried(a: &Value) -> bool {
    a["dry_run"] == json!(true)
}

/// Runs the parts as one change; the reply of one, or `{changes, results}` of several (`dry`: marked a dry run).
async fn run(app: &Shared, steps: Vec<(Run, &'static str, Value)>, dry: bool) -> Result<Value, String> {
    let n = steps.len();
    let mut out = crate::batch::as_one(app, steps).await.map_err(|(i, e)| if n > 1 && i != crate::batch::NOT_A_STEP { format!("{e}. Nothing was changed.") } else { e })?;
    let mut v = if out.len() == 1 { out.remove(0) } else { json!({ "changes": out.len(), "results": out }) };
    if dry && v.is_object() {
        v["dry_run"] = json!(true);
    }
    Ok(v)
}

/// A tool with an id (changes, the first handler) or without (makes, the second).
async fn one(app: &Shared, a: &Value, (change, make): (&'static str, &'static str), r: Run) -> Result<Value, String> {
    let (old, label) = if given(a, "id") { (change, "with id (a change)") } else { (make, "without id (a new one)") };
    let tool = match r {
        Run::Settlements => "set_settlement",
        Run::Quick => "set_parcel",
        _ => "set_plot",
    };
    let b = fit(a, old, &[], &format!("{tool} {label}"))?;
    run(app, vec![(r, old, b)], false).await
}

/// `<id> is a <family>: use <tool>` for an id of another family than the tool's.
fn foreign(a: &Value, prefix: &str, tool: &str) -> Result<(), String> {
    let Some(id) = a["id"].as_str() else { return Ok(()) };
    if id.starts_with(prefix) {
        return Ok(());
    }
    let (fam, use_) = match id.split(':').next().unwrap_or_default() {
        "n" => ("an NPC (n:)", "set_npc"),
        "p" => ("a plot point (p:)", "set_plot"),
        "m" => ("a point note (m:)", "set_note"),
        "b" => ("a building (b:)", "set_building"),
        _ => return Err(format!("{tool}: id is {prefix}..., not {id}")),
    };
    Err(format!("{id} is {fam}: use {use_}"))
}

/// The part of `a` under `keys` (with its id), for one handler.
fn part(a: &Value, keys: &[&str]) -> Value {
    let mut v = json!({ "id": a["id"] });
    for k in keys {
        if let Some(x) = a.get(*k) {
            v[*k] = x.clone();
        }
    }
    v
}

async fn feature(app: &Shared, a: &Value) -> Result<Value, String> {
    let has = |k: &str| given(a, k);
    let mut steps = Vec::new();
    let notes = ["notes", "tags", "remove_tags", "append"].into_iter().any(has);
    if has("append") || has("remove_tags") {
        steps.push((Run::Finds, "annotate_feature", part(a, &["notes", "tags", "remove_tags", "append"])));
        if has("name") {
            steps.insert(0, (Run::Finds, "rename_feature", part(a, &["name"])));
        }
    } else if notes {
        steps.push((Run::Finds, "update_feature", part(a, &["name", "notes", "tags"])));
    } else if has("name") {
        steps.push((Run::Finds, "rename_feature", part(a, &["name"])));
    }
    if has("hidden") {
        steps.push((Run::Finds, "hide_feature", part(a, &["hidden"])));
    }
    if steps.is_empty() {
        return Err("set_feature: give name, notes, tags, remove_tags or hidden".into());
    }
    run(app, steps, false).await
}

async fn site(app: &Shared, a: &Value) -> Result<Value, String> {
    let Some(id) = a["id"].as_str().map(str::to_string) else {
        let b = fit(a, "create_feature", PATH, "set_site without id (a new site)")?;
        return run(app, vec![(Run::Create, "create_feature", b)], false).await;
    };
    if given(a, "kind") {
        return Err("kind: only when making a site (no id); a site keeps its kind".into());
    }
    let id2 = id.clone();
    let kind = app.with_edits(move |e| e.created.iter().find(|c| c.id == id2).map(|c| c.kind.clone())).flatten();
    let keys: Vec<&str> = a.as_object().into_iter().flatten().filter(|(k, x)| !x.is_null() || *k == "in_building").map(|(k, _)| k.as_str()).filter(|k| !matches!(*k, "id" | "world")).collect();
    if matches!(kind.as_deref(), Some("castle" | "wall")) {
        let b = fit(a, "update_fortification", PATH, &format!("set_site on a {}", kind.unwrap_or_default()))?;
        return run(app, vec![(Run::Works, "update_fortification", b)], false).await;
    }
    if let Some(k) = keys.iter().find(|k| !matches!(**k, "name" | "in_building" | "way")) {
        let what = kind.map(|k| format!("a {k}")).unwrap_or_else(|| "a site".into());
        return Err(format!("set_site on {id} ({what}) takes name, in_building, way (an entrance's); not {k}"));
    }
    if keys.is_empty() {
        return Err("set_site with id: give name, in_building or way (a castle or wall: its outline and parts)".into());
    }
    run(app, vec![(Run::Finds, "update_feature", part(a, &["name", "in_building", "way"]))], false).await
}

/// The styling a footprint check doesn't weigh (left out of a dry run's check).
const STYLE: [&str; 8] = ["floors", "roof", "material", "shade", "tint", "shape", "structure", "name"];

async fn building(app: &Shared, a: &Value) -> Result<Value, String> {
    let b = dry(a);
    if given(a, "stamp") {
        let b = fit(&b, "place_stamp", STAMP, "set_building with stamp")?;
        return run(app, vec![(Run::Quick, "place_stamp", b)], dried(a)).await;
    }
    if dried(a) {
        let mut c = b.clone();
        if let Some(o) = c.as_object_mut() {
            o.remove("dry_run");
            for k in STYLE {
                o.remove(k);
            }
        }
        if ["at", "width_ft", "depth_ft"].into_iter().any(|k| given(&c, k)) {
            return Err("dry_run checks a footprint (poly, rect or circle) or a stamp; at with width_ft x depth_ft finds its own clear spot".into());
        }
        let c = fit(&c, "check_building_spot", &[], "set_building dry_run")?;
        return run(app, vec![(Run::Build, "check_building_spot", c)], true).await;
    }
    if given(a, "id") {
        let b = fit(&b, "update_building", &[], "set_building with id (a change)")?;
        return run(app, vec![(Run::Build, "update_building", b)], false).await;
    }
    let b = fit(&b, "create_building", NEAR, "set_building without id (a new one)")?;
    run(app, vec![(Run::Build, "create_building", b)], false).await
}

async fn npc(app: &Shared, a: &Value) -> Result<Value, String> {
    foreign(a, "n:", "set_npc")?;
    let mut b = a.clone();
    let o = b.as_object_mut().ok_or("arguments: an object")?;
    o.remove("world");
    // (at: null, off the map: the handler's empty location.)
    let placed = o.contains_key("at") || o.contains_key("level");
    if o.get("at").is_some_and(Value::is_null) {
        o.remove("at");
        o.insert("location".into(), json!(""));
    }
    if !given(a, "id") {
        if !given(a, "name") {
            return Err("name: required (to make an NPC; give id to change one)".into());
        }
        if o.get("location") == Some(&json!("")) {
            o.remove("location");
        }
        return run(app, vec![(Run::Notebook, "create_npc", b)], false).await;
    }
    let fields = o.keys().any(|k| !matches!(k.as_str(), "id" | "at" | "level" | "location"));
    if !fields && !placed {
        return Err("set_npc with id: give the fields to change, or at".into());
    }
    let old = if fields { "update_npc" } else { "place_npc" };
    run(app, vec![(Run::Notebook, old, b)], false).await
}

async fn note(app: &Shared, a: &Value) -> Result<Value, String> {
    foreign(a, "m:", "set_note")?;
    if !given(a, "id") {
        let b = fit(a, "add_note_at", &[], "set_note without id (a new note)")?;
        return run(app, vec![(Run::Notebook, "add_note_at", b)], false).await;
    }
    for k in ["site", "level"] {
        if given(a, k) {
            return Err(format!("{k}: only for a new note (no id); a note moves on its level (at, room)"));
        }
    }
    let mut steps = Vec::new();
    if given(a, "at") || given(a, "room") {
        steps.push((Run::Notebook, "move_note", part(a, &["at", "room"])));
    }
    if given(a, "text") || given(a, "tags") {
        let mut p = part(a, &["tags"]);
        if given(a, "text") {
            p["notes"] = a["text"].clone();
        }
        steps.push((Run::Finds, "update_feature", p));
    }
    if steps.is_empty() {
        return Err("set_note with id: give at, room, text or tags".into());
    }
    run(app, steps, false).await
}
