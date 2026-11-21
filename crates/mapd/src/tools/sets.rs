//! The `set_` tools (every family's but the hex grid's): one tool makes a thing (no `id`) or
//! changes it (`id`), each mode running its family's handler under the handler's own name (so the changes keep their
//! `"tool"` labels, the app's history vocabulary). A mode's arguments are checked against its handler's schema
//! (`route::fit`); a call with parts for several handlers is one change (`batch::as_one`).

use serde_json::{Map, Value, json};

use crate::Shared;
use crate::tools::route::fit;
use worldgen::ground::PlateauKind;

use crate::tools::{Run, merge, schema, text};

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
    let mut out = vec![
        json!({ "name": "set_feature", "title": "Name, notes, hide", "description": "Rename anything named (a feature, building, district, tower, site, level l:, room r:, road stretch rd:, crossing v:; empty name restores the original), set or append its notes and tags (remove_tags takes some off), or hide it from labels and search (hidden). A building drawn by hand is one place: either of its ids.", "inputSchema": feature }),
        json!({ "name": "set_site", "title": "Make or change a site", "description": "Without id, make one (kind): ruin (under: dungeon, crypt, catacombs), tower, camp, waystation, cave, mine, lava_tube, entrance (in_building, way); underground ones take size, levels, theme; castle (poly or rect) or wall (path or around_patches), with gates, keep, moat... (remove_in_way removes buildings in the way). With id: a castle's or wall's outline and parts, an entrance's in_building and way, any site's name.", "inputSchema": site }),
        json!({ "name": "set_settlement", "title": "Make or change a settlement", "description": "Without id: add a village or town at a point (at, tier; name, culture, lives_by, link, blank). With id: change a settlement's tier, culture, lives_by, capital, name (regenerate remakes its layout).", "inputSchema": settlement }),
        json!({ "name": "set_building", "title": "Make or change a building", "description": "Without id, draw one: poly, rect or circle (or at with width_ft x depth_ft: the nearest clear spot on a street), or a stamp (house, inn, smithy...) at at fronting the nearest street (size, facing). With id, change any building (shape, func, floors, roof, material, shade, tint, name). dry_run: true says whether a footprint fits (its blockers) or where a stamp would land, changing nothing.", "inputSchema": building }),
        json!({ "name": "set_building_row", "title": "A row of buildings", "description": "A row of lots along a path (or a road) on one side, with kinds, depth and gaps; clear: true takes the world's own buildings away. dry_run: true only lists the lots.", "inputSchema": row }),
        json!({ "name": "set_parcel", "title": "Mark or change a plot of ground", "description": "Without id: a free plot of ground (poly) with a fill: lots left empty for buildings by hand, or open ground. With id (c:<n>): its outline, fill or name. Returns its id. (set_plot is a notebook plot point.)", "inputSchema": parcel }),
        json!({ "name": "set_npc", "title": "Make or change an NPC", "description": "Without id: add an NPC (name; appearance, mannerisms, attitude, goals, notes, portrait, stance, status, tags). With id: change the fields given. at puts them somewhere (a place id, and level inside it; or a point), null takes them off the map. Returns the NPC with its id.", "inputSchema": npc }),
        json!({ "name": "set_plot", "title": "Make or change a plot point", "description": "Without id: add a plot point (title; text, status, anchors: places, npcs, tags). With id: change the fields given (lists are replaced). Returns it with its id.", "inputSchema": plot }),
        json!({ "name": "set_note", "title": "Make or change a point note", "description": "Without id: a note at a point in a building, dungeon or sewer level (site, level, at as [x, y] squares; or room) with text and tags; returns its m: id. With id: move it (at, or room on its level) and change its text and tags. Notes on places are set_feature's.", "inputSchema": note }),
    ];
    out.extend(more());
    out
}

const POINTS: &[(&str, &str)] = &[("path", "points")];

/// The tools of the families that make and change one handler's thing each (objects, sprites, ground, water, roads,
/// fences, crossings, wharves, links, sketch strokes, designs).
fn more() -> Vec<Value> {
    let id = |what: &str| json!({ "id": { "type": "string", "description": what } });
    let kind = json!({ "kind": { "type": "string", "enum": GROUND.iter().map(|g| g.0).collect::<Vec<_>>(), "description": "brush: mode, at or path, radius_ft, width_ft, strength, hardness, height_ft. plateau (a pit when it digs): poly or around, level_ft or rise_ft, edge_ft, floor, margin_ft. hill: at or from+to or around, peak_ft, crown_ft, radius_ft, rx_ft, ry_ft, angle_deg, width_ft, profile. islet: at, radius_ft, rise_ft. With id it is the id's own (leave it out)" } });
    let ground = union(&[("shape_ground", &[]), ("set_plateau", &[]), ("set_hill", &[]), ("raise_islet", &[])], &[], merge(kind, id("A plateau, pit, hill or islet to change (h:...)")), &[]);
    let water = union(&[("create_water", &[]), ("update_water", &[])], &[], id("Water drawn by hand to change (e:...)"), &[]);
    let road = union(&[("place_path", PATH)], &[], json!({}), &[]);
    let fence = union(&[("place_fence", POINTS)], &[], json!({}), &[]);
    let stroke = union(&[("create_sketch_stroke", &[]), ("update_sketch_stroke", &[])], &[], id("A stroke to change (sk:...)"), &[]);
    let mut design = union(&[("set_site_design", &[]), ("reset_site_design", &[])], &[], json!({ "reset": { "type": "boolean", "default": false, "description": "Drop the design: generated again as it was (id alone)" } }), &["id"]);
    design["properties"]["text"]["description"] = json!("The text plan (get with view design gives it); not with reset");
    let same = |old: &str| union(&[(old, &[])], &[], json!({}), &[]);
    let mut objects = crate::read::old_schema("place_objects");
    objects["properties"].as_object_mut().map(|o| o.remove("world"));
    vec![
        json!({ "name": "set_objects", "title": "Put objects down", "description": "Put objects on the battlemap at world positions (ft): built-in kinds or uploaded sprites (list what sprites), up to 500 at once; each one with an id (o:...) changes one put down before. They block, cover and hinder by their kind's rules. Returns their ids.", "inputSchema": objects }),
        json!({ "name": "set_sprite", "title": "Add or change a sprite", "description": "Add a picture as an object kind for battlemaps (64 px per 5-ft square is plenty): name, data_base64 or path, size_squares, height_ft and its rules (cover, blocks_movement, blocks_sight, difficult); or asset to change an uploaded one's rules. Returns its kind id (s:<asset>).", "inputSchema": same("upload_sprite") }),
        json!({ "name": "set_ground", "title": "Shape the ground", "description": "The battlemap's ground by kind: brush (raise, lower, smooth or restore with a round brush: a dab at at or a stroke along path), plateau (an outline levelled at one height; a pit when it digs), hill (round, ridge, knoll or mountain), islet (a rock islet in water). With id (h:...), changes that plateau, hill or islet. Returns its id (h:...) or, for a brush, the blocks changed.", "inputSchema": ground }),
        json!({ "name": "set_water", "title": "Draw or change water", "description": "Without id: water drawn by hand, a polygon (poly) or brushed band; its look follows it (the sea's at sea level, dug: a quay; an open band a stream; a ring a moat; else a pond or lake by depth). With id (e:...): change it; what is not given stays. Returns its id and look.", "inputSchema": water }),
        json!({ "name": "set_road", "title": "Draw or change a road", "description": "A road on the ground along path: on the map and battlemap, walked like a road, a bridge over water; scatter and the world's own town buildings keep off it. With id (rw:...), changes one. Returns its rw: id and crossings.", "inputSchema": road }),
        json!({ "name": "set_fence", "title": "Draw or change a fence", "description": "A fence or hedge along path (over roads, not water): kind, closed, gates. With id, changes one. gaps alone cuts gaps into the fences there (generated ones too). Returns its id.", "inputSchema": fence }),
        json!({ "name": "set_crossing", "title": "Put a crossing down", "description": "A bridge, ford or ferry from one bank to the other (from, to: world ft), 10-2000 ft long (a ferry 68+), 5-120 ft wide (a ford or ferry 40 at most); a bridge may carry shops. With id (v:...), changes that crossing. Returns its id, shops and the river's banks; an end in the water is refused.", "inputSchema": same("place_crossing") }),
        json!({ "name": "set_wharf", "title": "Put a quay, pier or breakwater down", "description": "kind quay (paving along a shore; shore: true follows the water's edge near path), pier (a deck) or breakwater (a stone mole; lighthouse: true builds one at its tip) along path, a pier's or breakwater's first point on the bank. With id (q:...), changes one. Returns its id.", "inputSchema": same("place_wharf") }),
        json!({ "name": "set_link", "title": "Link two places", "description": "Join a square of one place below ground (a) to another's (b): a site underground (a level or room of u:/k:), a sewer section (w:) or a building's cellar (b:). Each end shows on its map as a way across (way: stairs, ladder, passage, trapdoor, rope, portal; default stairs) naming the far end. Returns its id (j:...).", "inputSchema": same("create_link") }),
        json!({ "name": "set_sketch_stroke", "title": "Draw or change a sketch stroke", "description": "Without id: add a stroke to the world's sketch (tool: land, sea, range, river, forest, biome, road, settlement...: an outline or path, or a pin at at). With id (sk:...): change its points and options; what is not given stays. Stored at once; regenerate remakes the world from the sketch. Returns its id.", "inputSchema": stroke }),
        json!({ "name": "set_design", "title": "Design a site", "description": "Change an underground site or a building's interior from a text plan: read it with get (view design; format: true gives the full format), change it, send it back (text). Only what the text gives changes. dry_run: true says what would change. reset: true drops the design (generated again as it was).", "inputSchema": design }),
    ]
}

/// set_ground's kinds: (kind, handler, the module that runs it).
const GROUND: [(&str, &str, Run); 4] = [("brush", "shape_ground", Run::Ground), ("plateau", "set_plateau", Run::Ground), ("hill", "set_hill", Run::Ground), ("islet", "raise_islet", Run::Harbour)];

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
        "set_objects" => via(app, a, Run::Scatter, "place_objects", &[], "set_objects").await,
        "set_sprite" => via(app, a, Run::Scatter, "upload_sprite", &[], "set_sprite").await,
        "set_ground" => ground(app, a).await,
        "set_water" => {
            let (old, label) = if given(a, "id") { ("update_water", "set_water with id (a change)") } else { ("create_water", "set_water without id (new water)") };
            via(app, a, Run::Waters, old, &[], label).await
        }
        "set_road" => via(app, a, Run::Paths, "place_path", PATH, "set_road").await,
        "set_fence" => via(app, a, Run::Fences, "place_fence", POINTS, "set_fence").await,
        "set_crossing" => via(app, a, Run::Crossings, "place_crossing", &[], "set_crossing").await,
        "set_wharf" => via(app, a, Run::Harbour, "place_wharf", &[], "set_wharf").await,
        "set_link" => via(app, a, Run::Links, "create_link", &[], "set_link").await,
        "set_sketch_stroke" => {
            let (old, label) = if given(a, "id") { ("update_sketch_stroke", "set_sketch_stroke with id (a change)") } else { ("create_sketch_stroke", "set_sketch_stroke without id (a new stroke)") };
            via(app, a, Run::Sketch, old, &[], label).await
        }
        "set_design" => design(app, a).await,
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
    let mut out = crate::tools::batch::as_one(app, steps).await.map_err(|(i, e)| if n > 1 && i != crate::tools::batch::NOT_A_STEP { format!("{e}. Nothing was changed.") } else { e })?;
    let mut v = if out.len() == 1 { out.remove(0) } else { json!({ "changes": out.len(), "results": out }) };
    if dry && v.is_object() {
        v["dry_run"] = json!(true);
    }
    Ok(v)
}

/// One handler under its own name, with our names (`renames`) for its arguments.
async fn via(app: &Shared, a: &Value, r: Run, old: &'static str, renames: &[(&str, &str)], label: &str) -> Result<Value, String> {
    let b = fit(a, old, renames, label)?;
    run(app, vec![(r, old, b)], false).await
}

async fn ground(app: &Shared, a: &Value) -> Result<Value, String> {
    let mut b = a.clone();
    let asked = b.as_object_mut().and_then(|o| o.remove("kind")).and_then(|k| k.as_str().map(str::to_string));
    let kind = match a["id"].as_str() {
        None => asked.ok_or("kind: required without id (brush, plateau, hill, islet)")?,
        Some(id) => {
            let q = id.to_string();
            let own = app.with_edits(move |e| e.plateaus.get(&q).map(|p| if p.hill.is_some() { "hill" } else if p.kind == PlateauKind::Islet { "islet" } else { "plateau" })).flatten();
            let own = own.ok_or_else(|| format!("no such {id} (h: ground shaped by hand: a plateau, pit, hill or islet)"))?;
            if let Some(k) = asked.filter(|k| k != own) {
                return Err(format!("{id} is a {own}: kind {own} (or leave kind out), not {k}"));
            }
            own.to_string()
        }
    };
    let (_, old, r) = GROUND.iter().find(|g| g.0 == kind).ok_or_else(|| format!("kind: brush, plateau, hill or islet (got {kind})"))?;
    via(app, &b, *r, old, &[], &format!("kind '{kind}'")).await
}

async fn design(app: &Shared, a: &Value) -> Result<Value, String> {
    let mut b = a.clone();
    b.as_object_mut().map(|o| o.remove("reset"));
    if a["reset"] == json!(true) {
        return via(app, &b, Run::Design, "reset_site_design", &[], "set_design with reset").await;
    }
    via(app, &b, Run::Design, "set_site_design", &[], "set_design").await
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
