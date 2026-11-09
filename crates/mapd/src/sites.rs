//! Deleting created sites (`delete_feature`) and bringing them back (`restore_feature`). A deleted
//! site keeps its `c:` entry (`removed`), so its numbers, its layout and everything that points at
//! it stay as they were; only listings and lookups skip it.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::world::Created;

use crate::Shared;
use crate::tools::{schema, text};

pub fn list() -> Vec<Value> {
    vec![json!({
        "name": "restore_feature",
        "title": "Restore deleted sites",
        "description": "Bring back sites deleted with delete_feature (a drawn building by either of its ids): id or ids, all or none in one change. Refused, naming what is in the way, when the spot is taken now (by the checks a new one meets). NPCs, plots, notes and a hand design that pointed at it are still there. Returns each site as get_feature does.",
        "inputSchema": schema(crate::args::ids_props("deleted site (c:<n>, or a building's b:<layout>:0)"), &[]),
    })]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "delete_feature" => delete(app, a).await.map(text),
        "restore_feature" => restore(app, a).await.map(text),
        _ => return None,
    })
}

/// A created site asked for: its id as given, its `c:` id, its layout, a drawn building's `b:` id, and the site.
struct Site {
    given: String,
    cid: String,
    li: usize,
    bid: Option<String>,
    site: Created,
}

/// The created sites behind `ids` (`c:<n>`, or a drawn building's `b:<layout>:0`), deleted ones too.
async fn resolve(app: &Shared, ids: Vec<String>, single: bool) -> Result<Vec<Site>, String> {
    app.worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let edits = &w.file.edits;
            let base = t0.settlements.len() + t0.base_pois;
            ids.iter()
                .enumerate()
                .map(|(i, id)| {
                    let at = crate::args::item(single, i);
                    let (k, bid) = match agent::drawn_pair(w, t0, id) {
                        Some((k, _, b)) => (k, Some(b)),
                        None => match edits.created.iter().position(|c| &c.id == id) {
                            Some(k) => (k, None),
                            None if agent::generated_building(t0, id).is_some() => {
                                return Err(format!("{at}: {id} is one of the world's own buildings: they go with remove_buildings (restore_building brings them back)"));
                            }
                            None if agent::position(w, t0, id).is_some() => {
                                return Err(format!("{at}: {id} is not a created site (other generated features can be hidden with hide_feature)"));
                            }
                            None => return Err(format!("{at}: {}", crate::args::no_such(w, t0, id))),
                        },
                    };
                    let site = edits.created[k].clone();
                    Ok(Site { given: id.clone(), cid: site.id.clone(), li: base + k, bid, site })
                })
                .collect()
        })
        .await
}

async fn delete(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::args::ids(a)?;
    let sites = resolve(app, ids.clone(), a["ids"].is_null()).await?;
    let mut seen = std::collections::HashSet::new();
    let sites: Vec<Site> = sites.into_iter().filter(|s| seen.insert(s.cid.clone())).collect();
    app.edit("agent", 0, move |e| {
        for s in &sites {
            if e.created.iter().any(|c| c.id == s.cid && c.removed) {
                return Err(format!("{} is already deleted (restore_feature brings it back)", s.given));
            }
        }
        let mut out = Vec::new();
        let mut any = false;
        for s in &sites {
            let Some(c) = e.created.iter_mut().find(|c| c.id == s.cid) else { return Err(format!("{} was deleted meanwhile", s.given)) };
            c.removed = true;
            let (name, kind) = (c.name.clone(), c.kind.clone());
            // What still points at it is kept, for a restore.
            let here = |id: &str| id == s.cid || s.bid.as_deref() == Some(id) || worldgen::world::layout_in_id(id) == Some(s.li);
            let npcs: Vec<&String> = e.npcs.iter().filter(|(_, n)| n.location.as_ref().is_some_and(|l| here(&l.id))).map(|(k, _)| k).collect();
            let plots: Vec<&String> = e.plots.iter().filter(|(_, p)| p.anchors.iter().any(|x| here(x))).map(|(k, _)| k).collect();
            let mut left = serde_json::Map::new();
            if !npcs.is_empty() {
                left.insert("npcs".into(), json!(npcs));
            }
            if !plots.is_empty() {
                left.insert("plots".into(), json!(plots));
            }
            if e.notes.keys().any(|k| here(k)) {
                left.insert("notes".into(), json!(true));
            }
            if e.designs.keys().any(|k| here(k)) {
                left.insert("design".into(), json!(true));
            }
            let renames = e.renames.keys().filter(|k| here(k)).count();
            if renames > 0 {
                left.insert("renames".into(), json!(renames));
            }
            let mut v = json!({ "id": s.cid, "kind": kind, "name": name });
            if let Some(b) = &s.bid {
                v["building"] = json!(b);
            }
            if !left.is_empty() {
                any = true;
                v["left_pointing"] = Value::Object(left);
            }
            out.push(v);
        }
        let mut reply = json!({ "tool": "delete_feature", "deleted": out });
        if out_len_one(&reply) {
            reply["id"] = reply["deleted"][0]["id"].clone();
        }
        if any {
            reply["note"] = json!("what still points at a deleted site is kept as it is, for restore_feature");
        }
        Ok(reply)
    })
    .await
}

fn out_len_one(v: &Value) -> bool {
    v["deleted"].as_array().is_some_and(|a| a.len() == 1)
}

async fn restore(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::args::ids(a)?;
    let sites = resolve(app, ids, a["ids"].is_null()).await?;
    let mut seen = std::collections::HashSet::new();
    let sites: Vec<Site> = sites.into_iter().filter(|s| seen.insert(s.cid.clone())).collect();
    for s in &sites {
        if !s.site.removed {
            return Err(format!("{} is not deleted", s.given));
        }
    }
    // The spot must be free now, by the checks a new one of its kind meets; the check and the
    // un-delete happen under one hold of the gate, so nothing can be placed in between.
    let _hold = if crate::batch::active() { None } else { Some(app.gate.lock().await) };
    let copies: Vec<(String, usize, Created)> = sites.iter().map(|s| (s.given.clone(), s.li, s.site.clone())).collect();
    app.worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            for (given, li, c) in &copies {
                let r: Result<(), String> = match c.kind.as_str() {
                    "building" => agent::building_spot(w, t0, &c.poly, c.func.as_deref(), &c.id, Some((*li, None))).map(|_| ()),
                    "castle" | "wall" => agent::works_spot(w, t0, c, &c.id, Some(*li)).and_then(|s| {
                        if s.in_way.is_empty() { Ok(()) } else { Err(format!("the world's own buildings are in the way: {}", s.in_way.iter().map(|b| b.0.as_str()).collect::<Vec<_>>().join(", "))) }
                    }),
                    _ => agent::creation_spot(w, t0, &c.kind, c.under.as_deref(), &c.id, [c.x, c.y]).map(|_| ()),
                };
                r.map_err(|e| format!("{given} can't be restored, its place is taken: {e}"))?;
            }
            Ok(())
        })
        .await?;
    let ids2: Vec<(String, String)> = sites.iter().map(|s| (s.given.clone(), s.cid.clone())).collect();
    let change = move |e: &mut worldgen::world::Edits| {
        for (given, cid) in &ids2 {
            let k = e.created.iter().position(|c| &c.id == cid).ok_or_else(|| format!("{given} is not a created site"))?;
            if !e.created[k].removed {
                return Err(format!("{given} is not deleted"));
            }
            let c = e.created[k].clone();
            if let Some(o) = e.existing_site(&c, [c.x, c.y]) {
                return Err(format!("{given} can't be restored, its place is taken: {} is the same site", o.id));
            }
            e.created[k].removed = false;
        }
        Ok(json!({ "tool": "restore_feature", "ids": ids2.iter().map(|p| p.1.clone()).collect::<Vec<_>>() }))
    };
    if crate::batch::active() {
        app.edit("agent", 0, change).await?;
    } else {
        app.edit_now("agent", 0, change).await?;
    }
    drop(_hold);
    let want: Vec<(String, Option<String>)> = sites.iter().map(|s| (s.cid.clone(), s.bid.clone())).collect();
    let mut got = app
        .worker
        .with(move |ex| {
            want.iter()
                .map(|(id, bid)| {
                    let mut v = agent::get(&ex.world, &ex.t0, id).ok_or_else(|| "restored, but it could not be read back".to_string())?;
                    if let Some(b) = bid {
                        v["building"] = json!(b);
                    }
                    Ok::<_, String>(v)
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .await?;
    if got.len() == 1 {
        let mut v = got.remove(0);
        v["tool"] = json!("restore_feature");
        return Ok(v);
    }
    Ok(json!({ "tool": "restore_feature", "ids": sites.iter().map(|s| s.cid.clone()).collect::<Vec<_>>(), "restored": got }))
}
