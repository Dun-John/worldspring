//! `remove` and `restore`: ids of any family, each routed by its prefix to its family's handler (`family`). Ids of
//! several families are one change (`batch::as_one`: one save, one undo, all or none). Also `fit`, which checks a
//! merged tool's arguments against the handler a mode of it runs (`sets`).

use serde_json::{Map, Value, json};

use crate::Shared;
use crate::tools::{Run, schema, text};

/// A family's handler for its ids: `each`, one id per call; `ids_only`, it takes `ids` (never `id`); `flag`, an
/// argument it takes besides (`restore: true` for roads and fences).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Fam {
    run: Run,
    old: &'static str,
    each: bool,
    ids_only: bool,
    flag: Option<&'static str>,
}

const fn fam(run: Run, old: &'static str) -> Fam {
    Fam { run, old, each: false, ids_only: false, flag: None }
}

const ROADS: [&str; 6] = ["rd", "mt", "tr", "pp", "rw", "ts"];
const FENCES: [&str; 4] = ["fr", "fh", "f", "fg"];

/// What a family that has no restore is called, for the refusal.
fn for_good(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "n" => "an NPC",
        "p" => "a plot point",
        "m" | "r" => "a note",
        "o" => "a placed object",
        "h" | "g" => "ground shaped by hand",
        "e" => "water drawn by hand",
        "q" => "a wharf",
        "v" => "a crossing",
        "j" => "a link",
        "sk" => "a sketch stroke",
        _ => return None,
    })
}

/// The handler that takes `id` away (or brings it back: `restore`).
async fn family(app: &Shared, id: &str, restore: bool) -> Result<Fam, String> {
    let prefix = id.split(':').next().unwrap_or_default();
    let roads = ROADS.contains(&prefix);
    let fences = FENCES.contains(&prefix);
    // (A village or town added by hand: its handler sets the edits inside it aside, and brings them back.)
    let q = id.to_string();
    let settlement = prefix == "c" && app.with_edits(move |e| e.created.iter().any(|c| c.id == q && c.removed == restore && c.is_settlement())).unwrap_or(false);
    if restore {
        return Ok(match prefix {
            // (A building drawn by hand is a created site; the world's own are buildings.)
            "c" if settlement => Fam { each: true, ..fam(Run::Settlements, "restore_feature") },
            "c" => fam(Run::Sites, "restore_feature"),
            "b" if crate::tools::drawn_pair(app, id).await?.is_some() => fam(Run::Sites, "restore_feature"),
            "b" => fam(Run::Build, "restore_building"),
            "x" => fam(Run::Scatter, "restore_objects"),
            "i" => fam(Run::Ground, "restore_islets"),
            _ if roads => Fam { ids_only: true, flag: Some("restore"), ..fam(Run::Paths, "remove_roads") },
            _ if fences => Fam { ids_only: true, flag: Some("restore"), ..fam(Run::Fences, "remove_fences") },
            _ => {
                return Err(match for_good(prefix) {
                    Some(what) => format!("{id} can't be restored: {what} is deleted for good (the app's Undo can bring it back)"),
                    None => not_here(app, id, "restored").await,
                });
            }
        });
    }
    Ok(match prefix {
        "c" if settlement => Fam { each: true, ..fam(Run::Settlements, "delete_feature") },
        "c" => fam(Run::Sites, "delete_feature"),
        "b" if crate::tools::drawn_pair(app, id).await?.is_some() => fam(Run::Sites, "delete_feature"),
        "b" => fam(Run::Build, "remove_buildings"),
        "n" => fam(Run::Notebook, "delete_npc"),
        "p" => fam(Run::Notebook, "delete_plot"),
        "m" | "r" => Fam { each: true, ..fam(Run::Notebook, "delete_note") },
        "o" => fam(Run::Scatter, "remove_objects"),
        "x" => return Err(format!("{id} is a clear (generated objects taken away): restore brings them back")),
        "h" | "i" | "g" => fam(Run::Ground, "delete_plateau"),
        "e" => fam(Run::Waters, "delete_water"),
        "q" => fam(Run::Harbour, "remove_wharves"),
        "v" => fam(Run::Crossings, "remove_crossings"),
        "j" => fam(Run::Links, "delete_link"),
        "sk" => fam(Run::Sketch, "delete_sketch_stroke"),
        _ if roads => Fam { ids_only: true, ..fam(Run::Paths, "remove_roads") },
        _ if fences => Fam { ids_only: true, ..fam(Run::Fences, "remove_fences") },
        _ => return Err(not_here(app, id, "removed").await),
    })
}

/// An id no family takes: one the map knows can only be hidden; else what the prefix means.
async fn not_here(app: &Shared, id: &str, done: &str) -> String {
    let q = id.to_string();
    let done = done.to_string();
    app.worker
        .with(move |ex| {
            Ok::<_, String>(match worldgen::agent::position(&ex.world, &ex.t0, &q) {
                Some(_) => format!("{q} can't be {done}: generated features stay (set_feature {{id, hidden: true}} hides one)"),
                None => crate::tools::args::no_such(&ex.world, &ex.t0, &q),
            })
        })
        .await
        .unwrap_or_else(|e| e)
}

pub fn list() -> Vec<Value> {
    let ids = json!({ "type": "array", "items": { "type": "string" }, "minItems": 1, "maxItems": 5000, "description": "Ids of any family (all or none)" });
    let mut remove = crate::read::old_schema("remove_objects")["properties"].as_object().cloned().unwrap_or_default();
    remove.insert("ids".into(), ids.clone());
    remove.insert("id".into(), json!({ "type": "string" }));
    remove.insert("what".into(), json!({ "type": "string", "enum": ["buildings", "objects", "notes"], "description": "Instead of ids alone: buildings (the world's own inside within, with ids), objects (generated ones: within, points, kinds; with ids), notes (the notes on these places' ids)" }));
    let mut restore = Map::new();
    restore.insert("ids".into(), ids);
    restore.insert("id".into(), json!({ "type": "string" }));
    restore.insert("what".into(), json!({ "type": "string", "enum": ["objects"], "description": "objects: every clear touching within" }));
    restore.insert("within".into(), crate::read::old_schema("restore_objects")["properties"]["within"].clone());
    vec![
        json!({ "name": "remove", "title": "Take away", "description": "Take things away by id (id or ids, any families, all or none, one change): created sites and drawn buildings (c:, b:), the world's own buildings (b:), NPCs (n:), plot points (p:), notes (m:, r:), objects (o:), ground (h:, i:), water (e:), roads (rd: mt: tr: pp: rw: ts:), fences (fr: fh: f: fg:), wharves (q:), crossings (v:), links (j:), sketch strokes (sk:). what: buildings / objects take within (objects also points, kinds); notes the notes on places. Generated features can only be hidden (set_feature).", "inputSchema": schema(Value::Object(remove), &[]) }),
        json!({ "name": "restore", "title": "Bring back", "description": "Bring back what remove took (id or ids, all or none, one change): created sites and drawn buildings (c:, b:), the world's own buildings (b:, as generated), clears (x:), generated islets (i:), roads and fences by their ids; or what: objects with within (every clear touching it). NPCs, notes, water, wharves, crossings, links, strokes and ground shaped by hand are deleted for good.", "inputSchema": schema(Value::Object(restore), &[]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    let restore = match name {
        "remove" => false,
        "restore" => true,
        _ => return None,
    };
    Some(run(app, a, restore).await.map(text))
}

/// A handler's reply as the merged tool gives it; several (from several families): `{results, changes}`.
fn reply(mut out: Vec<Value>) -> Value {
    if out.len() == 1 {
        return out.remove(0);
    }
    json!({ "changes": out.len(), "results": out })
}

async fn run(app: &Shared, a: &Value, restore: bool) -> Result<Value, String> {
    let tool = if restore { "restore" } else { "remove" };
    let given = |k: &str| !a[k].is_null();
    if !a["id"].is_null() && !a["ids"].is_null() {
        return Err("give id or ids, not both".into());
    }
    let single = a["ids"].is_null();
    let ids: Vec<String> = if given("id") || given("ids") { crate::tools::args::ids(a)? } else { Vec::new() };
    if let Some(what) = a["what"].as_str() {
        // The world's own buildings, generated objects and clears by place; a place's note.
        let (run, old) = match (what, restore) {
            ("buildings", _) => (Run::Build, "remove_buildings"),
            ("objects", false) => (Run::Scatter, "remove_objects"),
            ("objects", true) => (Run::Scatter, "restore_objects"),
            (_, _) => (Run::Notebook, "delete_note"),
        };
        if old == "delete_note" {
            if ids.is_empty() {
                return Err("what 'notes': give the places' id or ids".into());
            }
            let rest: Vec<&String> = a.as_object().into_iter().flatten().map(|(k, _)| k).filter(|k| !matches!(k.as_str(), "what" | "id" | "ids" | "world")).collect();
            if let Some(k) = rest.first() {
                return Err(format!("what 'notes' takes id, ids; not {k}"));
            }
            let steps = ids.iter().map(|id| (run, old, json!({ "id": id }))).collect();
            return crate::tools::batch::as_one(app, steps).await.map(reply).map_err(|(i, e)| at_step(&ids, i, e));
        }
        let mut b = a.clone();
        b.as_object_mut().map(|o| o.remove("what"));
        let b = fit(&b, old, &[], &format!("what '{what}'"))?;
        return crate::tools::batch::as_one(app, vec![(run, old, b)]).await.map(reply).map_err(|(_, e)| e);
    }
    for k in ["within", "points", "kinds"] {
        if given(k) {
            return Err(format!("{k}: with what (\"buildings\" or \"objects\"{}), not with ids alone", if restore { "; restore: \"objects\"" } else { "" }));
        }
    }
    if ids.is_empty() {
        return Err(format!("{tool}: give id or ids (or what)"));
    }
    // The ids by family, in the order first given; one call for each family (or each id, where it takes one).
    let mut groups: Vec<(Fam, Vec<String>)> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let f = family(app, id, restore).await.map_err(|e| format!("{}: {e}", crate::tools::args::item(single, i)))?;
        match groups.iter_mut().find(|(g, _)| *g == f && !f.each) {
            Some((_, list)) => list.push(id.clone()),
            None => groups.push((f, vec![id.clone()])),
        }
    }
    let one = groups.len() == 1;
    let steps: Vec<(Run, &'static str, Value)> = groups
        .iter()
        .map(|(f, list)| {
            let mut v = if f.each || (single && !f.ids_only) { json!({ "id": list[0] }) } else { json!({ "ids": list }) };
            if let Some(flag) = f.flag {
                v[flag] = json!(true);
            }
            (f.run, f.old, v)
        })
        .collect();
    let lists: Vec<String> = groups.iter().map(|(_, l)| l.join(", ")).collect();
    crate::tools::batch::as_one(app, steps).await.map(reply).map_err(|(i, e)| if one { e } else { at_step(&lists, i, e) })
}

/// A part's error, naming the ids it had (`batch::NOT_A_STEP`: no part's).
fn at_step(parts: &[String], i: usize, e: String) -> String {
    match parts.get(i) {
        Some(ids) => format!("{ids}: {e}. Nothing was changed."),
        None => e,
    }
}

/// A merged tool's arguments for the handler `old` that one of its modes runs: renamed to the handler's names
/// (`renames`: (this tool's, the handler's)) and checked against its schema. An argument it doesn't take is
/// refused as `<label> takes a, b; not k`.
pub(crate) fn fit(a: &Value, old: &str, renames: &[(&str, &str)], label: &str) -> Result<Value, String> {
    let mut obj = a.as_object().cloned().unwrap_or_default();
    obj.remove("world");
    for (new, was) in renames {
        if let Some(x) = obj.remove(*new) {
            obj.insert((*was).to_string(), x);
        }
    }
    let s = crate::read::old_schema(old);
    let props = s["properties"].as_object().cloned().unwrap_or_default();
    let ours = |k: &str| renames.iter().find(|(_, w)| *w == k).map(|(n, _)| (*n).to_string()).unwrap_or_else(|| k.to_string());
    if let Some(k) = obj.keys().find(|k| !props.contains_key(*k)) {
        let mut takes: Vec<String> = props.keys().filter(|k| *k != "world").map(|k| ours(k)).collect();
        takes.sort_unstable();
        return Err(format!("{label} takes {}; not {}", takes.join(", "), ours(k)));
    }
    let mut v = Value::Object(obj);
    crate::tools::args::walk_all(&s, &mut v).map_err(|e| {
        // (The first error's field in this tool's name.)
        let head = e.split([':', '[', '.']).next().unwrap_or_default();
        format!("{}{}", ours(head), &e[head.len()..])
    })?;
    Ok(v)
}
