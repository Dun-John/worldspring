//! MCP tools that read and name features: world_overview, search_features, get_feature, list_children, list_names, describe_location,
//! features_near, route, list_roads, get_battlemap, render_view, focus_view, and rename/annotate/update/hide_feature.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::core::tile::TileKey;
use worldgen::world::Note;

use crate::Shared;
use crate::tools::*;

const TOOLS: [&str; 16] = ["world_overview", "search_features", "get_feature", "list_children", "list_names", "describe_location", "features_near", "route", "list_roads", "get_battlemap", "render_view", "focus_view", "rename_feature", "annotate_feature", "update_feature", "hide_feature"];

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    TOOLS.contains(&name).then_some(())?;
    Some(run(app, name, a).await)
}

async fn run(app: &Shared, name: &str, a: &Value) -> Result<Vec<Value>, String> {
    match name {
        "world_overview" => {
            let mut v = app.worker.with(|ex| Ok(agent::overview(&ex.world, &ex.t0))).await?;
            v["world"] = json!(app.current().map(crate::hex));
            Ok(text(v))
        }
        "search_features" => {
            let (q, kind, tag) = (a["query"].as_str().unwrap_or("").to_string(), a["kind"].as_str().map(str::to_string), a["tag"].as_str().map(|t| t.trim().to_string()));
            let narrows = !a["underground_kinds"].is_null() || !a["levels_min"].is_null() || !a["levels_max"].is_null();
            if q.trim().is_empty() && kind.is_none() && tag.is_none() && a["within"].is_null() && a["at"].is_null() && !narrows {
                return Err(crate::tools::args::bad("query, kind, tag, within, at", "give at least one (world_overview lists the largest places; list (what names) everything renameable)", &json!({})));
            }
            if !a["within"].is_null() && !a["at"].is_null() {
                return Err("within, at: give one (within takes a circle {at, radius_ft} too)".into());
            }
            if !a["radius_ft"].is_null() && a["at"].is_null() {
                return Err("radius_ft: needs at".into());
            }
            let within = if a["within"].is_null() { None } else { Some(crate::tools::args::area(app, "within", &a["within"]).await?) };
            let near = if a["at"].is_null() { None } else { Some(crate::tools::args::point(app, "at", &a["at"]).await?) };
            let radius = a["radius_ft"].as_f64().unwrap_or(5.0 * 5280.0);
            if !(radius > 0.0) {
                return Err(crate::tools::args::bad("radius_ft", "more than 0", &a["radius_ft"]));
            }
            // Underground filters: only sites below ground can meet them.
            let mut under = worldgen::undersearch::UnderOpts::from_json(&json!({ "kinds": a["underground_kinds"], "levels_min": a["levels_min"], "levels_max": a["levels_max"] })).map_err(|e| format!("underground_{e}"))?;
            if let (Some(lo), Some(hi)) = (under.levels_min, under.levels_max)
                && lo > hi
            {
                return Err(format!("levels_min: {lo} is more than levels_max {hi}"));
            }
            under.near = near;
            if narrows && kind.as_deref().is_some_and(|k| k != "underground") {
                return Err(format!("underground_kinds, levels_min, levels_max: only for sites underground (kind \"underground\" or none), not kind \"{}\"", kind.as_deref().unwrap_or("")));
            }
            let kind = if narrows { Some("underground".to_string()) } else { kind };
            if a["in_notes"].as_bool() == Some(true) && q.trim().is_empty() {
                return Err("in_notes: needs a query".into());
            }
            let limit = a["limit"].as_u64().unwrap_or(25) as usize;
            // Buildings and districts are matched by name only (3 letters or more), within an area too.
            let by_name_only = !a["within"].is_null() && q.trim().chars().count() < 3 && matches!(kind.as_deref(), Some("building" | "district"));
            let opts = agent::SearchOpts { query: q, kind, tag, in_notes: a["in_notes"].as_bool().unwrap_or(false), hidden: a["include_hidden"].as_bool().unwrap_or(false), within: within.map(|w| w.region()).or(near.map(|p| worldgen::geoquery::Region::Circle(p, radius))), under: Some(under) };
            // (All the matches are found, so `found` can say how many there were.)
            let all = app
                .worker
                .with(move |ex| {
                    let v = agent::search_with(&ex.world, &ex.t0, &opts, 100_000);
                    // A kind nothing has is a typo, not an empty answer (as list_names).
                    if let Some(k) = opts.kind.as_deref().filter(|_| v["results"].as_array().is_some_and(|r| r.is_empty())) {
                        let mut kinds: Vec<&str> = agent::features(&ex.world, &ex.t0).iter().map(|f| f.kind).collect();
                        kinds.extend(SEARCH_KINDS);
                        kinds.sort_unstable();
                        kinds.dedup();
                        if !kinds.contains(&k) {
                            let hint = crate::tools::args::suggest(k, kinds.iter().copied()).map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default();
                            return Err(format!("kind: nothing of kind \"{k}\"{hint}; the kinds here: {}", kinds.join(", ")));
                        }
                    }
                    Ok(v)
                })
                .await?;
            let mut v = listed(all["results"].clone(), limit, json!({}));
            if by_name_only && v["found"] == 0 {
                v["note"] = json!("buildings and districts are found by a query of 3 or more letters of their name (within narrows those)");
            }
            Ok(text(v))
        }
        "get_feature" => {
            let id = arg_str(&a, "id")?;
            app.worker.with(move |ex| agent::get(&ex.world, &ex.t0, &id).ok_or_else(|| crate::tools::args::no_such(&ex.world, &ex.t0, &id))).await.map(text)
        }
        "list_children" => {
            let id = arg_str(&a, "id")?;
            let limit = limit_of(&a, 200);
            let mut v = app.worker.with(move |ex| agent::children(&ex.world, &ex.t0, &id).ok_or_else(|| deleted_or(&ex.world, &ex.t0, &id, format!("no such feature, or it contains nothing: {id}")))).await?;
            // Each list is cut to `limit`; `found` has the full counts of the ones that were cut.
            let mut found = serde_json::Map::new();
            for (k, l) in v.as_object_mut().into_iter().flatten() {
                if let Some(arr) = l.as_array_mut().filter(|x| x.len() > limit) {
                    found.insert(k.clone(), json!(arr.len()));
                    arr.truncate(limit);
                }
            }
            if !found.is_empty() {
                v["found"] = Value::Object(found);
                v["truncated"] = json!(true);
            }
            Ok(text(v))
        }
        "list_names" => {
            let (kind, inside, settlement) = (a["kind"].as_str().map(str::to_string), a["inside"].as_str().map(str::to_string), a["settlement"].as_str().map(str::to_string));
            let limit = limit_of(&a, 200);
            let function = a["function"].as_str().map(|f| f.trim().to_lowercase());
            if function.is_some() && settlement.is_none() {
                return Err(crate::tools::args::bad("function", "used with settlement (a settlement's or site's id)", &a["function"]));
            }
            let v = app
                .worker
                .with(move |ex| {
                    let mut v = agent::names(&ex.world, &ex.t0, kind.as_deref(), inside.as_deref(), settlement.as_deref())?;
                    if let Some(f) = function.as_deref() {
                        let keys: Vec<&str> = worldgen::town::catalog::CATALOG.iter().map(|c| c.key).collect();
                        let label = worldgen::town::catalog::index_of(f).map(|i| worldgen::town::catalog::CATALOG[i as usize].name.to_lowercase());
                        if label.is_none() && !v["names"].as_array().into_iter().flatten().any(|n| n["function"].as_str().is_some_and(|x| x.to_lowercase() == f)) {
                            let hint = crate::tools::args::suggest(f, keys.iter().copied()).map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default();
                            return Err(format!("function: no business \"{f}\"{hint}; the keys are as set_building's func"));
                        }
                        let want = label.unwrap_or_else(|| f.to_string());
                        if let Some(l) = v["names"].as_array_mut() {
                            l.retain(|n| n["function"].as_str().is_some_and(|x| x.to_lowercase() == want));
                        }
                    }
                    // A kind nothing has is a typo, not an empty answer.
                    if let Some(k) = kind.as_deref().filter(|_| v["names"].as_array().is_some_and(|n| n.is_empty())) {
                        let all = agent::names(&ex.world, &ex.t0, None, inside.as_deref(), settlement.as_deref())?;
                        let mut kinds: Vec<&str> = all["names"].as_array().into_iter().flatten().filter_map(|n| n["kind"].as_str()).collect();
                        // (Kinds that only a scoped list has are known too: settlement and inside.)
                        kinds.extend(["site", "district", "building", "tower", "underground", "level", "room", "road", "crossing"]);
                        kinds.sort_unstable();
                        kinds.dedup();
                        if !kinds.contains(&k) {
                            let hint = crate::tools::args::suggest(k, kinds.iter().copied()).map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default();
                            return Err(format!("kind: no names of kind \"{k}\"{hint}; the kinds here: {}", kinds.join(", ")));
                        }
                    }
                    Ok(v)
                })
                .await?;
            Ok(text(listed(v["names"].clone(), limit, json!({}))))
        }
        "describe_location" => {
            let p = place(app, &a).await?;
            let r = a["radius_ft"].as_f64().unwrap_or(15.0);
            app.worker.with_mut(move |ex| Ok(worldgen::geoquery::describe(ex, p, r))).await.map(text)
        }
        "features_near" => {
            let p = place(app, &a).await?;
            let r = a["radius_mi"].as_f64().unwrap_or(5.0) * 5280.0;
            let kinds: Vec<String> = a["kinds"].as_array().map(|v| v.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let limit = a["limit"].as_u64().unwrap_or(50) as usize;
            let hidden = a["include_hidden"].as_bool().unwrap_or(false);
            let mut v = app.worker.with(move |ex| Ok(agent::near(&ex.world, &ex.t0, p, r, &kinds, limit, hidden))).await?;
            let (items, found) = (v["results"].take(), v["found"].clone());
            v["items"] = items;
            if found.as_u64().unwrap_or(0) > limit as u64 {
                v["truncated"] = json!(true);
            }
            if let Some(m) = v.as_object_mut() {
                m.remove("results");
            }
            Ok(text(v))
        }
        "route" => {
            let (from, fe) = crate::tools::args::point_named(app, "from", &a["from"]).await?;
            let (to, te) = crate::tools::args::point_named(app, "to", &a["to"]).await?;
            let mut v = app.worker.with(move |ex| Ok(agent::route(&ex.world, &ex.t0, from, to))).await?;
            // A place given by name comes back with its id.
            if let Some(e) = fe {
                v["from_place"] = e;
            }
            if let Some(e) = te {
                v["to_place"] = e;
            }
            Ok(text(v))
        }
        "list_roads" => {
            let near = if a["at"].is_null() { None } else { Some((place(app, &a).await?, a["radius_mi"].as_f64().unwrap_or(25.0) * 5280.0)) };
            let settlement = a["settlement"].as_str().map(str::to_string);
            let class = a["class"].as_str().map(str::to_string);
            if let Some(c) = &class
                && !["king's road", "road", "track"].contains(&c.to_lowercase().replace('_', " ").replace("kings", "king's").as_str())
            {
                return Err(crate::tools::args::bad("class", "king's road (or kings_road), road or track", &json!(c)));
            }
            let drawn = a["drawn"].as_bool().unwrap_or(false);
            let limit = a["limit"].as_u64().unwrap_or(50) as usize;
            let mut v = app
                .worker
                .with(move |ex| {
                    let li = match &settlement {
                        Some(id) => Some(agent::layout_of(&ex.world, &ex.t0, id).filter(|&l| worldgen::town::is_settlement(&ex.t0, l)).ok_or_else(|| format!("{id} is not a settlement"))?),
                        None => None,
                    };
                    Ok(agent::roads(&ex.world, &ex.t0, near, li, class.as_deref(), drawn, limit))
                })
                .await?;
            let (found, items) = (v["count"].as_u64().unwrap_or(0), v["roads"].take());
            if let Some(m) = v.as_object_mut() {
                m.remove("count");
                m.remove("shown");
                m.remove("roads");
                m.insert("items".into(), items);
                m.insert("found".into(), json!(found));
                if found > limit as u64 {
                    m.insert("truncated".into(), json!(true));
                }
            }
            Ok(text(v))
        }
        "get_battlemap" => {
            let p = place(app, &a).await?;
            let r = a["radius_squares"].as_f64().unwrap_or(12.0);
            app.worker
                .run(move |g| {
                    let ex = g.ex.as_mut().ok_or("no world is open")?;
                    let geom = &ex.world.geom;
                    let size = geom.tile_size_ft(geom.max_level);
                    let key = TileKey::surface(geom.max_level, (p[0] / size).floor().max(0.0) as u32, (p[1] / size).floor().max(0.0) as u32);
                    let origin = [key.x as f64 * size, key.y as f64 * size];
                    let chunk = ex.battlemap(key);
                    Ok::<Value, String>(agent::battlemap_summary(&ex.world, &chunk, origin, p, r))
                })
                .await
                .map(text)
        }
        "render_view" => {
            let p = place(app, &a).await?;
            let size = a["size_ft"].as_f64().unwrap_or(3000.0);
            let width = a["width_px"].as_u64().unwrap_or(1024);
            let png = a["format"].as_str() == Some("png");
            // (An l: or r: id, or a b:, u: or k: one with a level, is looked at from inside.)
            let at = a["at"].as_str().unwrap_or("");
            let inside = match (agent::split_room_id(at), a["level"].as_u64()) {
                (Some((site, lv, _)), _) => Some(json!({ "id": site, "level": lv })),
                (None, Some(lv)) if matches!(at.get(..2), Some("b:" | "u:" | "k:")) => Some(json!({ "id": at, "level": lv })),
                (None, Some(_)) => return Err("level: needs at to be a building or site id (b:, u:, k:) or use an l: id".into()),
                _ => None,
            };
            let reply = app.ask_app(json!({ "type": "render", "x": p[0], "y": p[1], "size": size, "width_px": width, "format": if png { "png" } else { "jpeg" }, "inside": inside }), 45).await.map_err(|e| {
                format!("{e}; render_view needs the map app open in a visible tab, with the place generated (give it a few seconds and try again, or try a larger size_ft)")
            })?;
            let img = reply["image"].as_str().or_else(|| reply["png"].as_str()).ok_or_else(|| format!("the app could not render: {} (the tab must be visible and the map generated; try again, or a larger size_ft)", reply["error"].as_str().unwrap_or("no image")))?;
            let data = img.split(',').next_back().unwrap_or(img);
            // (Labelled by what the app sent, as its data URL says: an older app answers png whatever was asked.)
            let mime = img.strip_prefix("data:").and_then(|r| r.split(';').next()).filter(|m| m.starts_with("image/")).unwrap_or(if png { "image/png" } else { "image/jpeg" });
            Ok(vec![
                json!({ "type": "image", "data": data, "mimeType": mime }),
                json!({ "type": "text", "text": format!("View centred at ({:.0}, {:.0}) ft, spanning about {size:.0} ft; {width} x {} px.", p[0], p[1], (width as f64 * 2.0 / 3.0).round()) }),
            ])
        }
        "focus_view" => {
            let p = place(app, &a).await?;
            let size = a["size_ft"].as_f64().unwrap_or(3000.0);
            if app.followers().is_empty() {
                return Err(app.no_app());
            }
            app.send(json!({ "type": "focus", "x": p[0], "y": p[1], "size": size, "world": app.current().map(crate::hex) }));
            Ok(text(json!({ "focused": [p[0].round(), p[1].round()], "size_ft": size })))
        }
        "rename_feature" => {
            let id = arg_str(&a, "id")?;
            let name = arg_str(&a, "name")?.trim().to_string();
            known(app, &id).await?;
            // A building drawn by hand keeps its name in its entry, whichever of its ids is given (as the app does).
            let q = id.clone();
            let drawn = app.worker.with(move |ex| Ok(agent::drawn_building_ids(&ex.world, &ex.t0, &q))).await?;
            app.edit("agent", 0, move |e| {
                if let Some((k, cid, bid)) = drawn.filter(|d| !name.is_empty() && e.created.get(d.0).is_some_and(|c| c.id == d.1)) {
                    e.renames.remove(&cid);
                    e.renames.remove(&bid);
                    e.created[k].name = name.clone();
                    return Ok(json!({ "tool": "rename_feature", "id": id, "name": name }));
                }
                if name.is_empty() {
                    e.renames.remove(&id);
                } else {
                    e.renames.insert(id.clone(), name.clone());
                }
                if let Some(c) = e.created.iter_mut().find(|c| c.id == id)
                    && !name.is_empty()
                {
                    c.name = name.clone();
                }
                Ok(json!({ "tool": "rename_feature", "id": id, "name": name }))
            })
            .await
            .map(text)
        }
        "annotate_feature" => {
            let id = arg_str(&a, "id")?;
            if a["notes"].is_null() && a["tags"].is_null() && a["remove_tags"].is_null() {
                return Err(crate::tools::args::bad("notes, tags, remove_tags", "give at least one", &json!({})));
            }
            let given = |k: &str| a[k].as_array().is_some_and(|v| !v.is_empty());
            if a["notes"].is_null() && !given("tags") && !given("remove_tags") {
                return Err(if a["tags"].is_null() { "remove_tags: at least 1 item, or tags (or give notes)" } else { "tags: at least 1 item, or remove_tags (or give notes)" }.into());
            }
            let t = match a["notes"].as_str() {
                Some(t) => Some(t.to_string()),
                None if a["notes"].is_null() => None,
                None => return Err(crate::tools::args::bad("notes", "text", &a["notes"])),
            };
            let tags: Vec<String> = a["tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let tags = crate::edit::notebook::canon_tags(tags);
            let remove: Vec<String> = a["remove_tags"].as_array().map(|v| v.iter().filter_map(|t| t.as_str().map(|t| t.to_string())).collect()).unwrap_or_default();
            if let Some(t) = tags.iter().find(|t| remove.iter().any(|r| r.eq_ignore_ascii_case(t))) {
                return Err(crate::tools::args::bad("remove_tags", "not also in tags", &json!(t)));
            }
            let append = a["append"].as_bool().unwrap_or(false);
            known(app, &id).await?;
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                let key = note_key(e, pair, &id);
                let n = e.notes.entry(key.clone()).or_insert_with(Note::default);
                if let Some(t) = t {
                    n.text = if append && !n.text.is_empty() { format!("{}\n\n{t}", n.text) } else { t };
                }
                let gone = |t: &String| remove.iter().any(|r| r.eq_ignore_ascii_case(t));
                let removed: Vec<String> = n.tags.iter().filter(|t| gone(t)).cloned().collect();
                n.tags.retain(|t| !gone(t));
                n.tags.extend(tags);
                n.tags = crate::edit::notebook::canon_tags(std::mem::take(&mut n.tags));
                let mut v = json!({ "tool": "annotate_feature", "id": key, "notes": n.text, "tags": n.tags });
                if !remove.is_empty() {
                    v["removed_tags"] = json!(removed);
                }
                Ok(v)
            })
            .await
            .map(text)
        }
        "update_feature" => {
            let id = arg_str(&a, "id")?;
            if id.starts_with("m:") && !a["name"].is_null() {
                return Err("name: a point note has no name (it is called after its room); give notes, tags or at".into());
            }
            // A point note (m:) moves with `at`, as move_note.
            let point = id.starts_with("m:") && !a["at"].is_null();
            if point {
                crate::edit::notebook::write_point(app, "move_note", &json!({ "id": id, "at": a["at"] })).await?;
            }
            let moving = a.get("in_building").is_some() || !a["way"].is_null() || (!a["at"].is_null() && !point);
            if !moving && !point && ["name", "notes", "tags"].iter().all(|k| a[k].is_null()) {
                return Err(crate::tools::args::bad("name, notes, tags", "give at least one", &json!({})));
            }
            known(app, &id).await?;
            let moved = if moving { Some(move_entrance(app, &a, &id).await?) } else { None };
            if let Some(m) = moved.as_ref().filter(|_| ["name", "notes", "tags"].iter().all(|k| a[k].is_null())) {
                return Ok(text(m.clone()));
            }
            if point && ["name", "notes", "tags"].iter().all(|k| a[k].is_null()) {
                return Ok(text(json!({ "tool": "update_feature", "id": id })));
            }
            let name = a["name"].as_str().map(|s| s.trim().to_string());
            let notes = a["notes"].as_str().map(str::to_string);
            let tags: Option<Vec<String>> = a["tags"].as_array().map(|v| crate::edit::notebook::canon_tags(v.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()));
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                if let Some(n) = &name {
                    // A building drawn by hand keeps its name in its entry (as `rename_feature`).
                    let drawn = pair.as_ref().filter(|p| !n.is_empty() && e.created.get(p.0).is_some_and(|c| c.id == p.1));
                    if let Some((k, cid, bid)) = drawn {
                        e.renames.remove(cid);
                        e.renames.remove(bid);
                        e.created[*k].name = n.clone();
                    } else if n.is_empty() {
                        e.renames.remove(&id);
                    } else {
                        e.renames.insert(id.clone(), n.clone());
                        if let Some(c) = e.created.iter_mut().find(|c| c.id == id) {
                            c.name = n.clone();
                        }
                    }
                }
                if notes.is_some() || tags.is_some() {
                    let key = note_key(e, pair.clone(), &id);
                    let n = e.notes.entry(key).or_insert_with(Note::default);
                    if let Some(t) = notes {
                        n.text = t;
                    }
                    if let Some(t) = tags {
                        n.tags = t;
                    }
                }
                Ok(json!({ "tool": "update_feature", "id": id }))
            })
            .await
            .map(|mut v| {
                if let Some(m) = moved {
                    v.as_object_mut().into_iter().for_each(|o| o.extend(m.as_object().cloned().unwrap_or_default()));
                }
                text(v)
            })
        }
        "hide_feature" => {
            let id = arg_str(&a, "id")?;
            let hide = a["hidden"].as_bool().unwrap_or(true);
            if id.starts_with("w:") {
                return Err(format!("{id}: a city's sewers can't be hidden (their grates go with the streets)"));
            }
            known(app, &id).await?;
            let pair = drawn_pair(app, &id).await?;
            app.edit("agent", 0, move |e| {
                // (A building drawn by hand: its mark kept under its created id.)
                let key = match pair.filter(|p| e.created.get(p.0).is_some_and(|c| c.id == p.1)) {
                    Some((_, c, b)) => {
                        e.hidden.remove(&b);
                        c
                    }
                    None => id.clone(),
                };
                if hide {
                    e.hidden.insert(key.clone());
                } else {
                    e.hidden.remove(&key);
                }
                Ok(json!({ "tool": "hide_feature", "id": key, "hidden": hide }))
            })
            .await
            .map(text)
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}
