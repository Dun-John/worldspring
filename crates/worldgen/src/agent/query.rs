//! Lookups: overview, search, get, near, describe, children of a place.

use super::*;

/// A world overview: its size, the land, counts of named features, the largest settlements.
pub fn overview(world: &World, t0: &T0) -> Value {
    let fs = features(world, t0);
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for f in &fs {
        *counts.entry(f.kind).or_default() += 1;
    }
    let mut towns: Vec<&Feature> = fs.iter().filter(|f| SETTLEMENT_KINDS.contains(&f.kind)).collect();
    // "…, pop. 12,345, capital": the digits after "pop. " (thousands commas included).
    let pop = |f: &Feature| -> u64 {
        let d = f.detail.as_deref().and_then(|d| d.split("pop. ").nth(1)).unwrap_or("");
        d.chars().take_while(|c| c.is_ascii_digit() || *c == ',').filter(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0)
    };
    towns.sort_by_key(|f| std::cmp::Reverse(pop(f)));
    let p = world.params();
    json!({
        "seed": world.file.seed,
        "size_mi": [p.width_mi, p.height_mi],
        "map_origin": "x east and y south, in feet from the map's top-left corner (5,280 ft a mile)",
        "land": fs.iter().filter(|f| matches!(f.kind, "continent" | "island")).map(|f| json!({ "id": f.id, "kind": f.kind, "name": f.name })).collect::<Vec<_>>(),
        "feature_counts": counts,
        "largest_settlements": towns.iter().take(12).map(|f| json!({ "id": f.id, "name": f.name, "kind": f.kind, "detail": f.detail, "x_ft": f.x.round(), "y_ft": f.y.round() })).collect::<Vec<_>>(),
        "created_sites": world.file.edits.created.iter().filter(|c| !c.removed).count(),
        "notes": world.file.edits.notes.len(),
        "npcs": world.file.edits.npcs.len(),
        "plots": world.file.edits.plots.len(),
        "roads": if p.generated_roads { "generated and drawn" } else { "only drawn" },
        // (With the pieces of hand castles that were changed.)
        "buildings_changed": world.file.edits.buildings.len() + world.file.edits.created.iter().filter(|c| !c.removed).map(|c| c.pieces.len()).sum::<usize>(),
        // Changes to the world's own buildings that no longer apply (the town was laid out anew).
        "buildings_set_aside": town::set_aside(world, t0).into_iter().map(|(id, why)| json!({ "id": id, "why": why })).chain(town::pieces::set_aside(world, t0).into_iter().map(|(id, castle, why)| json!({ "id": id, "castle": castle, "why": why }))).collect::<Vec<_>>(),
        "designs_set_aside": crate::interior::design::set_aside(world, t0).into_iter().map(|(id, why)| json!({ "id": id, "why": why })).collect::<Vec<_>>(),
        "towns_set_aside": town::wards::set_aside(world, t0).into_iter().map(|(layout, why)| json!({ "layout": layout, "why": why })).collect::<Vec<_>>(),
        "places_removed": places_removed(world),
    })
}

/// Settlements taken back or removed in the app (`gone:<n>:<id>`), and settlements and sites of a
/// world upgraded to a generator that made them anew (`was:<version>:<id>`); both name nothing:
/// what was made on each, kept under that id: its name then, whether it has a note, how many NPCs
/// are there and plot points name it.
fn places_removed(world: &World) -> Vec<Value> {
    let e = &world.file.edits;
    let gone = |id: &&String| id.starts_with("gone:") || id.starts_with("was:");
    let mut ids: std::collections::BTreeSet<&String> = e.notes.keys().chain(e.renames.keys()).filter(gone).collect();
    ids.extend(e.npcs.values().filter_map(|n| n.location.as_ref().map(|l| &l.id)).filter(gone));
    ids.extend(e.plots.values().flat_map(|p| &p.anchors).filter(gone));
    ids.into_iter()
        .map(|id| {
            let npcs = e.npcs.values().filter(|n| n.location.as_ref().is_some_and(|l| &l.id == id)).count();
            let plots = e.plots.values().filter(|p| p.anchors.contains(id)).count();
            json!({ "id": id, "name": e.renames.get(id), "note": e.notes.contains_key(id), "npcs": npcs, "plots": plots })
        })
        .collect()
}

/// Named features (and, from three letters, districts and businesses) matching a query, by
/// their current names, each with its extent.
pub fn search(world: &World, t0: &T0, q: &str, kind: Option<&str>, limit: usize, include_hidden: bool) -> Value {
    search_with(world, t0, &SearchOpts { query: q.to_string(), kind: kind.map(str::to_string), tag: None, in_notes: false, hidden: include_hidden, within: None, under: None }, limit)
}

/// What `search_with` looks for: `query` in names (and notes with `in_notes`), `kind`, and only features
/// whose notes carry `tag`.
#[derive(Default)]
pub struct SearchOpts {
    pub query: String,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub in_notes: bool,
    pub hidden: bool,
    /// Only features with a point inside, or an extent overlapping (each hit says which: `inside`).
    pub within: Option<crate::geoquery::Region>,
    /// Underground sites too (`undersearch`), kept by these filters (None: not searched).
    pub under: Option<crate::undersearch::UnderOpts>,
}

pub fn search_with(world: &World, t0: &T0, o: &SearchOpts, limit: usize) -> Value {
    let ql = o.query.trim().to_lowercase();
    let (kind, include_hidden) = (o.kind.as_deref(), o.hidden);
    let edits = &world.file.edits;
    let hidden = &edits.hidden;
    let tag = o.tag.as_deref().map(str::to_lowercase);
    let shapes = shapes(world, t0);
    let fs = features(world, t0);
    let mut out: Vec<Value> = Vec::new();
    if tag.is_none() {
        out = fs
            .iter()
            .zip(&shapes)
            .filter(|(f, _)| (ql.is_empty() || f.name.to_lowercase().contains(&ql)) && kind.is_none_or(|k| f.kind == k) && (include_hidden || !hidden.contains(&f.id)))
            .take(limit)
            .map(|(f, s)| json!({ "id": f.id, "kind": f.kind, "name": f.name, "detail": f.detail, "x_ft": f.x.round(), "y_ft": f.y.round(), "extent": feature_extent(world, f, s) }))
            .collect();
        if ql.len() >= 3 && out.len() < limit && kind.is_none_or(|k| matches!(k, "building" | "district")) {
            for h in crate::gazetteer::search_buildings(world, t0, &ql, limit - out.len(), o.within.as_ref().map(|r| r.bbox()), include_hidden) {
                out.push(hit_json(world, t0, &h));
            }
        }
        // Towers that were renamed (a tower has no name of its own to search until it has one).
        if (!ql.is_empty() || kind == Some("tower")) && out.len() < limit && kind.is_none_or(|k| k == "tower") {
            for (id, name) in edits.renames.iter().filter(|(id, n)| id.starts_with("t:") && n.to_lowercase().contains(&ql) && (include_hidden || !hidden.contains(*id))) {
                if out.len() < limit && let Some(v) = tower_hit(world, t0, id, name) {
                    out.push(v);
                }
            }
        }
        // Road stretches renamed and crossings named (only those given a name).
        if (!ql.is_empty() || matches!(kind, Some("road" | "crossing"))) && out.len() < limit {
            out.extend(crate::roadids::search_hits(world, t0, &ql, kind, include_hidden, limit - out.len()));
        }
        // Sites below ground: by name, renamed levels and rooms, kind and theme words, what is on them.
        if let Some(u) = &o.under
            && kind.is_none_or(|k| k == "underground")
            && (!ql.is_empty() || kind.is_some() || u.narrows())
            && out.len() < limit
        {
            let mut u = u.clone();
            u.hidden = include_hidden;
            if u.within.is_none() {
                u.within = o.within.clone();
            }
            out.extend(crate::undersearch::search_underground(world, t0, &ql, &u).into_iter().take(limit - out.len()));
        }
    }
    // Notes: their tags, and with `in_notes` their words.
    if tag.is_some() || (o.in_notes && !ql.is_empty()) {
        let seen: Vec<String> = out.iter().filter_map(|v| v["id"].as_str().map(str::to_string)).collect();
        let road_ids: std::cell::OnceCell<Vec<String>> = std::cell::OnceCell::new();
        for (id, n) in &edits.notes {
            if out.len() >= limit || seen.contains(id) || (!include_hidden && hidden.contains(id)) {
                continue;
            }
            if tag.as_ref().is_some_and(|t| !n.tags.iter().any(|x| x.to_lowercase() == *t)) {
                continue;
            }
            let found = if tag.is_some() && ql.is_empty() { true } else { (o.in_notes && n.text.to_lowercase().contains(&ql)) || name_any(world, t0, id).is_some_and(|nm| nm.to_lowercase().contains(&ql)) };
            if !found {
                continue;
            }
            let v = if id.starts_with("rd:") {
                let ids = road_ids.get_or_init(|| crate::roadids::road_ids(t0));
                match crate::roadids::road_hit(world, t0, ids, &fs, id) {
                    Some(v) => v,
                    None => continue,
                }
            } else if let Some((f, s)) = fs.iter().zip(&shapes).find(|(f, _)| f.id == *id) {
                json!({ "id": f.id, "kind": f.kind, "name": f.name, "detail": f.detail, "x_ft": f.x.round(), "y_ft": f.y.round(), "extent": feature_extent(world, f, s) })
            } else if let Some(g) = get(world, t0, id) {
                let mut v = json!({ "id": id, "kind": g["kind"], "name": g["name"], "x_ft": g["x_ft"], "y_ft": g["y_ft"], "settlement": g["settlement"] });
                if let Some(e) = g.get("extent") {
                    v["extent"] = e.clone();
                }
                v
            } else {
                continue;
            };
            if kind.is_none_or(|k| v["kind"] == k) {
                out.push(v);
            }
        }
    }
    let room = limit.saturating_sub(out.len());
    out.extend(town::places::search(world, t0, &ql, kind, room, include_hidden));
    if let Some(rg) = &o.within {
        let at: std::collections::HashMap<&str, usize> = fs.iter().enumerate().map(|(i, f)| (f.id.as_str(), i)).collect();
        let ids: std::cell::OnceCell<Vec<String>> = std::cell::OnceCell::new();
        out.retain_mut(|v| {
            let id = v["id"].as_str().unwrap_or("");
            let inside = match (v["x_ft"].as_f64(), v["y_ft"].as_f64()) {
                (Some(x), Some(y)) if rg.contains([x, y]) => Some("point"),
                _ if id.starts_with("rd:") => {
                    let ids = ids.get_or_init(|| crate::roadids::road_ids(t0));
                    ids.iter().position(|r| r == id).filter(|&i| rg.hits_line(&t0.roads.roads[i].pts)).map(|_| "extent")
                }
                _ => match at.get(id) {
                    Some(&i) => shapes.get(i).filter(|s| rg.overlaps(s)).map(|_| "extent"),
                    None => v["extent"]["bbox_ft"].as_array().and_then(|b| {
                        let b: Vec<f64> = b.iter().filter_map(Value::as_f64).collect();
                        let q = rg.bbox();
                        (b.len() == 4 && b[2] >= q[0] && b[0] <= q[2] && b[3] >= q[1] && b[1] <= q[3] && (rg.contains([b[0], b[1]]) || rg.contains([b[2], b[3]]) || rg.contains([b[0], b[3]]) || rg.contains([b[2], b[1]]) || (b[0] <= rg.middle()[0] && rg.middle()[0] <= b[2] && b[1] <= rg.middle()[1] && rg.middle()[1] <= b[3]))).then_some("extent")
                    }),
                },
            };
            if let Some(w) = inside {
                v["inside"] = json!(w);
            }
            inside.is_some()
        });
        // Nearest the area's middle first (a hit with no point sorts last).
        let mid = rg.middle();
        let key = |v: &Value| match (v["x_ft"].as_f64(), v["y_ft"].as_f64()) {
            (Some(x), Some(y)) => town::geom::dist([x, y], mid),
            _ => f64::MAX,
        };
        out.sort_by(|a, b| key(a).total_cmp(&key(b)));
    }
    json!({ "results": out })
}

/// A renamed wall or gate tower (`t:<layout>:<k>`) as a search hit.
fn tower_hit(world: &World, t0: &T0, id: &str, name: &str) -> Option<Value> {
    let n: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    let (li, k) = (*n.first()?, *n.get(1)?);
    if li >= town::layout_count(t0) {
        return None;
    }
    let l = town::layout(world, t0, li);
    let (at, _, gate) = *crate::interior::towers(&l).get(k)?;
    let mut v = json!({ "id": id, "kind": "tower", "name": name, "function": if gate { "Gate tower" } else { "Wall tower" }, "x_ft": at[0].round(), "y_ft": at[1].round() });
    set_whereabouts(world, t0, &mut v, li);
    Some(v)
}

/// A district or building found in a layout, with its extent.
fn hit_json(world: &World, t0: &T0, h: &crate::gazetteer::Hit) -> Value {
    let mut v = with_rename(world, serde_json::to_value(h).unwrap_or(Value::Null));
    if let Some(e) = v["id"].as_str().and_then(|id| extent_of(world, t0, id)) {
        v["extent"] = e;
    }
    // The settlement as `get_feature` gives it ({id, name}; a layout's number is not an id).
    if let Some(li) = v["settlement"].as_u64() {
        set_whereabouts(world, t0, &mut v, li as usize);
    }
    v
}

fn with_rename(world: &World, mut v: Value) -> Value {
    if let Some(id) = v["id"].as_str()
        && let Some(n) = world.file.edits.renames.get(id)
    {
        v["name"] = json!(n);
    }
    if let Some(id) = v["district_id"].as_str()
        && let Some(n) = world.file.edits.renames.get(id)
    {
        v["district"] = json!(n);
    }
    v
}

/// Everything named within `radius` ft of a point (inside it, for an area), nearest first:
/// features, and districts and businesses when `kinds` asks for them (or, with no kinds,
/// within a mile). Each with its distance, direction and extent.
pub fn near(world: &World, t0: &T0, p: P, radius: f64, kinds: &[String], limit: usize, include_hidden: bool) -> Value {
    let hidden = &world.file.edits.hidden;
    let wants = |k: &str| kinds.is_empty() || kinds.iter().any(|w| w == k);
    let fs = features(world, t0);
    let mut out: Vec<(f64, Value)> = fs
        .iter()
        .zip(&shapes(world, t0))
        .filter(|(f, _)| wants(f.kind) && (include_hidden || !hidden.contains(&f.id)))
        .filter_map(|(f, s)| {
            let (d, at) = s.distance(p);
            (d <= radius).then(|| {
                let dir = if d > 0.0 { compass(p, at) } else { "here" };
                (d, json!({ "id": f.id, "kind": f.kind, "name": f.name, "detail": f.detail, "distance_mi": mi(d), "direction": dir, "x_ft": f.x.round(), "y_ft": f.y.round(), "extent": feature_extent(world, f, s) }))
            })
        })
        .collect();
    let towns = if kinds.is_empty() { radius <= MI } else { wants("district") || wants("building") };
    if towns {
        let r = radius.min(5.0 * MI);
        for h in crate::gazetteer::in_view(world, t0, [p[0] - r, p[1] - r, p[0] + r, p[1] + r], 2_000) {
            let mut v = hit_json(world, t0, &h);
            let (kind, id) = (v["kind"].as_str().unwrap_or("").to_string(), v["id"].as_str().unwrap_or("").to_string());
            let at = [v["x"].as_f64().unwrap_or(0.0), v["y"].as_f64().unwrap_or(0.0)];
            let d = geom::dist(at, p);
            if d <= radius && wants(&kind) && (include_hidden || !is_hidden(world, t0, &id)) {
                v["distance_mi"] = json!(mi(d));
                v["direction"] = json!(if d > 0.0 { compass(p, at) } else { "here" });
                out.push((d, v));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total = out.len();
    json!({ "x_ft": p[0].round(), "y_ft": p[1].round(), "radius_mi": mi(radius), "found": total, "results": out.into_iter().take(limit).map(|(_, v)| v).collect::<Vec<_>>() })
}

/// A feature's details: what it is, where, its notes, and how it relates to the world around
/// it (the regions it lies in, the nearest settlements, roads; a settlement's districts and
/// notable buildings; a building's floors; an underground site's levels).
pub fn get(world: &World, t0: &T0, id: &str) -> Option<Value> {
    if deleted_with(world, t0, id).is_some() {
        return None;
    }
    if id.starts_with("rd:") || world.file.edits.crossings.contains_key(id) {
        return way_json(world, t0, id);
    }
    // A link put down by hand: its ends, and what keeps either from showing.
    if let Some(l) = world.file.edits.links.get(id) {
        let mut v = crate::links::link_json(world, t0, id, l);
        v["kind"] = json!("link");
        v["problems"] = json!(crate::links::problems(world, t0, id, l));
        return Some(v);
    }
    let fs = features(world, t0);
    // (A building taken away: where it stood, while its place is in the world.)
    let here = || crate::world::layout_in_id(id).is_some_and(|li| li < town::layout_count(t0));
    let p = position(world, t0, id).or_else(|| world.file.edits.buildings.get(id).filter(|e| e.removed && here()).map(|e| e.at))?;
    let ground = t0.sample(p[0], p[1], t0.cell_ft);
    let mut v = json!({
        "id": id,
        "x_ft": p[0].round(),
        "y_ft": p[1].round(),
        "elevation_ft": (ground - world.params().sea_level_ft).round(),
        "hidden": is_hidden(world, t0, id),
        "notes": note_of(world, t0, id),
        "in": regions_at(&fs, &shapes(world, t0), p),
        "nearest_settlements": nearest_settlements(&fs, p, id, 3),
    });
    if let Some(e) = extent_of(world, t0, id) {
        v["extent"] = e;
    }
    if let Some(f) = fs.iter().find(|f| f.id == id) {
        v["kind"] = json!(f.kind);
        v["name"] = json!(f.name);
        v["detail"] = json!(f.detail);
    }
    let head = id.split(':').next().unwrap_or("");
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    if let Some(li) = layout_of(world, t0, id) {
        let l = town::layout(world, t0, li);
        v["layout"] = layout_summary(world, t0, &l);
        if town::is_settlement(t0, li) {
            v["roads_to"] = json!(roads_from(world, t0, li));
            if town::settlement(t0, li).is_some_and(|s| s.tier >= crate::t0::settle::Tier::Town) {
                v["layout"]["wards_edited"] = json!(town::wards::edit_of(world, t0, li).is_some());
            }
            v["sites"] = json!(sites_json(world, t0, li, false));
        } else {
            v["settlement"] = whereabouts(world, t0, li).0;
            // (A created site's ways underground, as its settlement's sites list has them; an entrance in a building says which.)
            v["underground"] = json!(l.entrances.iter().filter(|e| e.kind != crate::under::UnderKind::Sewer).map(|e| format!("u:{li}:{}", e.id)).collect::<Vec<_>>());
            if let Some(c) = crate::sitelinks::created_of(world, t0, li).filter(|c| c.in_building.is_some()) {
                v["in_building"] = in_building_json(world, t0, c);
                v["way"] = json!(if crate::sitelinks::trapdoor(c) { "trapdoor" } else { "stairs" });
            }
        }
    } else if head == "b" && nums.len() == 2 && nums[0] < town::layout_count(t0) {
        let l = town::layout(world, t0, nums[0]);
        let Some(b) = l.building(nums[1]) else {
            // Taken away (`remove_buildings`): what it was.
            let e = world.file.edits.buildings.get(id).filter(|e| e.removed)?;
            let g = town::base_layout(world, t0, nums[0]).building(nums[1]).cloned()?;
            v["kind"] = json!("building");
            v["removed"] = json!(true);
            v["function"] = json!(g.label());
            v["x_ft"] = json!(e.at[0].round());
            v["y_ft"] = json!(e.at[1].round());
            return Some(v);
        };
        v["kind"] = json!("building");
        v["name"] = json!(building_name(world, &l, b));
        v["function"] = json!(b.label());
        // (A hand castle's ruined piece keeps the storeys it was built with, for when it is roofed again: it has none now.)
        if b.structure == town::Structure::Ruin && town::pieces::piece_of(world, t0, id).is_some() {
            v["floors"] = json!(0);
            v["floors_note"] = json!("a ruin: a roofless shell has no storeys");
        } else {
            v["floors"] = json!(b.floors);
        }
        // The roof's material and shape (battlements and ruins have none of their own).
        if b.roof != Some(town::RoofStyle::Battlements) && b.structure != town::Structure::Ruin {
            let k = town::roof::look(&l, b);
            v["roof_material"] = json!(town::roof::MATERIALS[k.material as usize]);
            v["roof_shape"] = json!(town::roof::SHAPES[k.shape as usize]);
            v["roof_shade"] = json!(k.shade);
        }
        // The footprint: corners in order, and its long side's direction and size.
        let o = geom::obb(&b.poly);
        let r1 = |x: f64| (x * 10.0).round() / 10.0;
        v["footprint"] = json!({
            "poly": b.poly.iter().map(|p| [r1(p[0]), r1(p[1])]).collect::<Vec<_>>(),
            "angle_deg": r1(libm::atan2(o.axis[1], o.axis[0]).to_degrees().rem_euclid(180.0)),
            "length_ft": r1(o.long),
            "width_ft": r1(o.short),
            "area_sq_ft": geom::area(&b.poly).abs().round(),
        });
        // A keep's pieces (its footprint is their union).
        if !b.parts.is_empty() {
            v["footprint"]["pieces"] = b
                .parts
                .iter()
                .map(|p| {
                    let kind = match p.kind {
                        town::PartKind::Body => "body",
                        town::PartKind::Tower { round: true } => "round tower",
                        town::PartKind::Tower { round: false } if town::belfry::of(b).is_some() => "belfry",
                        town::PartKind::Tower { round: false } => "square tower",
                        town::PartKind::Forebuilding => "forebuilding",
                        town::PartKind::Wing if town::temple::is_temple(b) => "transept",
                        town::PartKind::Wing => "wing",
                        town::PartKind::Apse => "apse",
                    };
                    json!({ "kind": kind, "poly": p.poly.iter().map(|q| [r1(q[0]), r1(q[1])]).collect::<Vec<_>>() })
                })
                .collect();
        }
        // The ways down from its cellar: the sites of the entrances made in it (`create_feature in_building`).
        let down = crate::sitelinks::entrances_in(world, t0, nums[0], nums[1]);
        if !down.is_empty() {
            v["underground"] = json!(down.iter().map(|(el, _)| format!("u:{el}:0")).collect::<Vec<_>>());
        }
        v["structure"] = json!(match b.structure {
            town::Structure::Roofed => "roofed",
            town::Structure::Open => "open",
            town::Structure::Ruin => "ruin",
        });
        if world.file.edits.buildings.get(id).is_some() {
            v["edited"] = json!(true);
        }
        // Its inside designed by hand (get_site_design), or a design that no longer fits.
        if let Some(d) = world.file.edits.designs.get(id) {
            let fits = b.structure == town::Structure::Roofed && crate::interior::design::designed(world, t0, &l, nums[0], b, d).is_some();
            v["interior_designed"] = json!(if fits { "yes" } else { "set aside: it no longer fits the building (see world_overview)" });
        }
        set_whereabouts(world, t0, &mut v, nums[0]);
        if let Some(it) = crate::interior::generate_id(world, t0, id) {
            v["interior"] = interior_summary(world, &it);
        }
    } else if head == "d" && nums.len() == 2 {
        let l = town::layout(world, t0, nums[0]);
        let q = l.quarters.get(nums[1])?;
        v["kind"] = json!("district");
        v["name"] = json!(world.file.edits.renames.get(id).cloned().unwrap_or_else(|| q.name.clone()));
        v["district_kind"] = json!(q.kind.name());
        set_whereabouts(world, t0, &mut v, nums[0]);
    } else if matches!(head, "sq" | "gt") {
        // (R5 towns: a square, park or gate, `town::places`.)
        v.as_object_mut()?.extend(town::places::detail(world, t0, id)?.as_object()?.clone());
        set_whereabouts(world, t0, &mut v, nums[0]);
    } else if matches!(head, "u" | "w" | "k" | "t") {
        let it = crate::interior::generate_id(world, t0, id)?;
        v["kind"] = json!(it.function);
        v["name"] = json!(site_name(world, &it));
        v["interior"] = interior_summary(world, &it);
        if let Some(&li) = nums.first().filter(|&&li| li < town::layout_count(t0)) {
            set_whereabouts(world, t0, &mut v, li);
        }
    } else if let Some((site, li, own)) = split_point_id(id) {
        // A point note: its place path, the room it lies in and its spot.
        v["kind"] = json!("note");
        v["name"] = json!(name_any(world, t0, id));
        v["site"] = json!(site);
        v["level"] = json!(li);
        v["point"] = json!(own);
        if let Some(n) = world.file.edits.notes.get(id) {
            v["at"] = json!(n.at);
            if let Some(row) = note_row(world, t0, id, n) {
                v["place"] = row["place"].clone();
                v["room"] = row["room"].clone();
            }
        }
    } else if let Some((site, ..)) = split_room_id(id) {
        v["kind"] = json!(if head == "l" { "level" } else { "room" });
        v["name"] = json!(name_any(world, t0, id));
        v["site"] = json!(site);
        if let Some(li) = site.split(':').nth(1).and_then(|s| s.parse::<usize>().ok()).filter(|&li| li < town::layout_count(t0)) {
            v["settlement"] = whereabouts(world, t0, li).0;
        }
    }
    if world.file.edits.designs.contains_key(id) {
        v["designed"] = json!(true);
    }
    let (npcs, plots) = authored_at(world, t0, &[id.to_string()]);
    v["npcs"] = json!(npcs);
    v["plots"] = json!(plots);
    let links: Vec<Value> = crate::links::of(&world.file.edits, id).map(|(k, l)| crate::links::link_json(world, t0, k, l)).collect();
    if !links.is_empty() {
        v["links"] = json!(links);
    }
    Some(v)
}

/// A settlement's or site's layout: districts, notable buildings, ways underground.
/// A road stretch (`rd:`) or a crossing put down by hand (`v:`) for `get_feature`: what
/// `roadids` says of it, with its notes, whether hidden, the settlements nearest and (a
/// crossing) the river it crosses.
fn way_json(world: &World, t0: &T0, id: &str) -> Option<Value> {
    // Computed once for the call: every road lookup below reads them.
    let fs = features(world, t0);
    let generated: Option<String>;
    let mut v = match world.file.edits.crossings.get(id) {
        Some(c) => {
            let mut v = crate::roadids::crossing_json(&world.file.edits, id, c);
            v["crossing"] = v["kind"].take();
            v["kind"] = json!("crossing");
            let m = crate::roadids::crossing_mid(c);
            v["x_ft"] = json!(m[0].round());
            v["y_ft"] = json!(m[1].round());
            v["river"] = river_at(world, t0, m, 0.5 * c.length() + 500.0).map_or(Value::Null, |r| r.json(Some(m)));
            generated = Some(crate::roadids::crossing_generated(c));
            v
        }
        None => {
            let ids = crate::roadids::road_ids(t0);
            let v = crate::roadids::road_json(world, t0, &ids, &fs, id)?;
            let r = &t0.roads.roads[ids.iter().position(|x| x == id)?];
            generated = r.stroke.and_then(|s| crate::roadids::drawn_road(world, t0, &fs, s)).map(|f| fs[f].name.clone());
            v
        }
    };
    let p = [v["x_ft"].as_f64().unwrap_or(0.0), v["y_ft"].as_f64().unwrap_or(0.0)];
    v["generated"] = json!(generated);
    v["hidden"] = json!(is_hidden(world, t0, id));
    v["notes"] = note_of(world, t0, id);
    v["nearest_settlements"] = json!(nearest_settlements(&fs, p, id, 3));
    Some(v)
}

fn layout_summary(world: &World, t0: &T0, l: &town::Layout) -> Value {
    let notable: Vec<Value> = l
        .buildings
        .iter()
        .filter(|b| b.func.is_some())
        .take(60)
        .map(|b| json!({ "id": format!("b:{}:{}", l.index, b.id), "name": building_name(world, l, b), "function": b.label() }))
        .collect();
    let entrances: Vec<Value> = l
        .entrances
        .iter()
        .filter(|e| e.kind != crate::under::UnderKind::Sewer)
        .map(|e| json!({ "id": format!("u:{}:{}", l.index, e.id), "kind": e.kind.name(), "x_ft": e.at[0].round(), "y_ft": e.at[1].round() }))
        .collect();
    let sewers = l.entrances.iter().filter(|e| e.kind == crate::under::UnderKind::Sewer).count();
    // Flights of steps: one entry each (at the middle of its pieces), the highest first.
    let mut flights: Vec<(f32, [f64; 2], f64)> = Vec::new();
    for (k, st) in l.stairs.iter().enumerate() {
        if k > 0 && l.stairs[k - 1].flight == st.flight {
            continue;
        }
        let pieces: Vec<&town::Stair> = l.stairs[k..].iter().take_while(|p| p.flight == st.flight).collect();
        let mid = |p: &town::Stair| town::geom::lerp(p.pts[0], p.pts[p.pts.len() - 1], 0.5);
        let at = pieces.iter().fold([0.0, 0.0], |a, p| town::geom::add(a, town::geom::mul(mid(p), 1.0 / pieces.len() as f64)));
        flights.push((st.rise, at, st.pts.windows(2).map(|w| town::geom::dist(w[0], w[1])).sum()));
    }
    flights.sort_by(|a, b| b.0.total_cmp(&a.0));
    let stairs: Vec<Value> = flights.iter().take(40).map(|(rise, at, length)| json!({ "x_ft": at[0].round(), "y_ft": at[1].round(), "length_ft": length.round(), "rise_ft": (*rise as f64 * 10.0).round() / 10.0 })).collect();
    let _ = t0;
    // (A town laid out by hand: the size class its people make, as its feature's kind.)
    let tier = town::hand::size(world, l.index as usize).map_or(l.tier, |x| x.1);
    json!({
        "tier": tier.name(),
        "culture": town::culture::NAMES[l.culture as usize % 5],
        "buildings": l.buildings.len(),
        "walled": !l.walls.is_empty(),
        "districts": l.quarters.iter().enumerate().map(|(qi, q)| json!({ "id": format!("d:{}:{qi}", l.index), "name": world.file.edits.renames.get(&format!("d:{}:{qi}", l.index)).cloned().unwrap_or_else(|| q.name.clone()), "kind": q.kind.name() })).collect::<Vec<_>>(),
        "notable_buildings": notable,
        "underground": entrances,
        "sewer_grates": sewers,
        "stairs": stairs,
    })
}

/// A building's or site's name: renamed, its own, or what it is.
fn site_name(world: &World, it: &crate::interior::Interior) -> String {
    world.file.edits.renames.get(&it.id).cloned().or_else(|| it.name.clone()).unwrap_or_else(|| it.function.to_string())
}

/// A building's or site's levels with their rooms (counted by kind; renamed rooms by name).
fn interior_summary(world: &World, it: &crate::interior::Interior) -> Value {
    let renames = &world.file.edits.renames;
    json!({
        "id": it.id,
        "name": site_name(world, it),
        "function": it.function,
        "size_ft": [it.nx * 5, it.ny * 5],
        "levels": it.levels.iter().enumerate().map(|(li, lv)| {
            let mut rooms: std::collections::BTreeMap<&str, usize> = Default::default();
            for r in lv.rooms.iter().filter(|r| r.squares > 0) {
                *rooms.entry(r.kind).or_default() += 1;
            }
            let lid = format!("l:{}:{li}", it.id);
            let named: Vec<Value> = (0..lv.rooms.len())
                .filter_map(|ri| {
                    let rid = format!("r:{}:{li}:{ri}", it.id);
                    renames.get(&rid).map(|n| json!({ "id": rid, "name": n, "kind": lv.rooms[ri].kind }))
                })
                .collect();
            json!({
                "id": lid,
                "name": renames.get(&lid).unwrap_or(&lv.name),
                "rooms": rooms,
                "named_rooms": named,
                "hazards": lv.furniture.iter().filter_map(|f| f.hazard.map(|h| format!("{}: {h}", f.name))).collect::<std::collections::BTreeSet<_>>(),
                "ways_to_other_sites": lv.links.iter().map(|k| &k.to).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    })
}

/// Children of a feature: a settlement's districts and businesses, a district's businesses, a
/// building's or underground site's levels and rooms, a region's settlements and sites.
pub fn children(world: &World, t0: &T0, id: &str) -> Option<Value> {
    if deleted_note(world, t0, id).is_some() {
        return None;
    }
    let head = id.split(':').next().unwrap_or("");
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    if let Some(li) = layout_of(world, t0, id) {
        let l = town::layout(world, t0, li);
        // (Those pushed out into a place among the place's: `town::members`.)
        let mut businesses: Vec<Value> = l
            .buildings
            .iter()
            .filter(|b| b.func.is_some() && town::members::building_owner(world, t0, li, b.id).is_none())
            .map(|b| json!({ "id": format!("b:{}:{}", l.index, b.id), "name": building_name(world, &l, b), "function": b.label() }))
            .collect();
        for (id, cl, bi) in pushed_businesses(world, t0, li) {
            let Some(b) = cl.building(bi as usize) else { continue };
            businesses.push(json!({ "id": id, "name": building_name(world, &cl, b), "function": b.label() }));
        }
        let summary = layout_summary(world, t0, &l);
        let mut v = json!({ "districts": summary["districts"], "businesses": [], "underground": summary["underground"] });
        // A settlement's sites too: drawn buildings with a trade among its businesses.
        if is_town(world, t0, li) {
            for (id, cl) in drawn_businesses(world, t0, li) {
                let Some(b) = cl.building(0) else { continue };
                businesses.push(json!({ "id": id, "name": building_name(world, &cl, b), "function": b.label() }));
            }
            v["sites"] = json!(sites_json(world, t0, li, true));
        }
        v["businesses"] = json!(businesses);
        return Some(v);
    }
    if head == "d" && nums.len() == 2 && nums[0] < town::layout_count(t0) {
        let l = town::layout(world, t0, nums[0]);
        let q = l.quarters.get(nums[1])?;
        let businesses: Vec<Value> = l
            .buildings
            .iter()
            .filter(|b| b.func.is_some() && q.patches.iter().any(|poly| geom::contains(poly, geom::centroid(&b.poly))))
            .map(|b| json!({ "id": format!("b:{}:{}", l.index, b.id), "name": building_name(world, &l, b), "function": b.label() }))
            .collect();
        return Some(json!({ "businesses": businesses }));
    }
    if matches!(head, "b" | "u" | "w" | "k" | "t") {
        let it = crate::interior::generate_id(world, t0, id)?;
        return Some(interior_summary(world, &it));
    }
    // A region: the settlements and sites within it.
    let fs = features(world, t0);
    let f = fs.iter().find(|f| f.id == id)?;
    let reach = f.extent_ft * if matches!(f.kind, "continent" | "island") { 0.9 } else { 0.6 };
    let inside: Vec<Value> = fs
        .iter()
        .filter(|g| (SETTLEMENT_KINDS.contains(&g.kind) || SITE_KINDS.contains(&g.kind)) && geom::dist([g.x, g.y], [f.x, f.y]) < reach)
        .take(200)
        .map(|g| json!({ "id": g.id, "kind": g.kind, "name": g.name }))
        .collect();
    Some(json!({ "places": inside }))
}

/// The water of a lake or sea at a point: the smallest named body there (id, name).
pub(crate) fn water_body_at(world: &World, t0: &T0, q: P) -> Option<(String, String)> {
    let fs = features(world, t0);
    let shapes = shapes(world, t0);
    fs.iter()
        .zip(&shapes)
        .filter(|(f, s)| matches!(f.kind, "lake" | "salt_lake" | "sea" | "ocean" | "bay") && { let b = s.bbox(); q[0] >= b[0] && q[0] <= b[2] && q[1] >= b[1] && q[1] <= b[3] } && s.distance(q).0 == 0.0)
        .min_by(|a, b| a.1.area_sq_ft().total_cmp(&b.1.area_sq_ft()))
        .map(|(f, _)| (f.id.clone(), f.name.clone()))
}

/// What is at a point (on T0 ground; `geoquery::describe` adds the battlemap's): the regions, the
/// nearest settlements, the building or district or site there, and named features nearby (a tenth
/// of their size away, 2 to 10 mi).
pub fn describe(world: &World, t0: &T0, x: f64, y: f64) -> Value {
    let fs = features(world, t0);
    let shapes = shapes(world, t0);
    let p = [x, y];
    let sea = world.params().sea_level_ft;
    let ground = t0.sample(x, y, t0.cell_ft);
    let water = t0.sample_water(x, y) as f64;
    let biome = crate::t0::biome::Biome::from_u8(t0.sample_biome(x, y, t0.biome_warp(x, y))[0]).name();
    let here = crate::gazetteer::query(world, t0, x, y).map(|h| with_rename(world, serde_json::to_value(&h).unwrap_or(Value::Null)));
    let hidden = &world.file.edits.hidden;
    let mut near: Vec<(f64, P, &Feature)> = fs
        .iter()
        .zip(&shapes)
        .filter(|(f, _)| !hidden.contains(&f.id))
        .filter_map(|(f, s)| {
            let (d, at) = s.distance(p);
            // (What it lies in is listed under "in".)
            (d <= reach_ft(f) && !(d == 0.0 && matches!(**s, Shape::Area { .. }))).then_some((d, at, f))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let near: Vec<Value> = near
        .into_iter()
        .take(25)
        .map(|(d, at, f)| json!({ "id": f.id, "kind": f.kind, "name": f.name, "distance_mi": mi(d), "direction": if d > 0.0 { compass(p, at) } else { "here" } }))
        .collect();
    // The NPCs and plot points at what is here, or nearby.
    let mut ids: Vec<String> = near.iter().filter_map(|f| f["id"].as_str().map(str::to_string)).collect();
    if let Some(h) = &here {
        ids.extend(h["id"].as_str().map(str::to_string));
        ids.extend(h["settlement"].as_u64().and_then(|li| feature_of_layout(world, t0, li as usize)));
    }
    let (npcs, plots) = authored_at(world, t0, &ids);
    let mut out = json!({
        "x_ft": x.round(),
        "y_ft": y.round(),
        "elevation_ft": (ground - sea).round(),
        "under_water": water > ground,
        "npcs": npcs,
        "plots": plots,
        "biome": biome,
        "in": regions_at(&fs, &shapes, p),
        "here": here,
        "nearby": near,
        "nearest_settlements": nearest_settlements(&fs, p, "", 3),
    });
    let links = links_near(world, t0, p);
    if !links.is_empty() {
        out["links"] = json!(links);
    }
    out
}

/// The links put down by hand with an end below point `p`: in the sewer section under it, or in a
/// site, keep or building whose place is within 100 ft.
pub fn links_near(world: &World, t0: &T0, p: P) -> Vec<Value> {
    let links = &world.file.edits.links;
    if links.is_empty() {
        return Vec::new();
    }
    let s = crate::under::SEWER_SECTION_FT;
    let (sx, sy) = (crate::core::floor(p[0] / s) as i64, crate::core::floor(p[1] / s) as i64);
    let under = |site: &str| {
        let n: Vec<i64> = site.split(':').skip(1).filter_map(|v| v.parse().ok()).collect();
        match (site.as_bytes()[0], n.as_slice()) {
            (b'w', [_, x, y]) => (*x, *y) == (sx, sy),
            _ => position(world, t0, site).is_some_and(|q| geom::dist(p, q) <= 100.0),
        }
    };
    let at = |e: &crate::links::End| crate::links::site_of(&e.level).is_some_and(|(site, _)| under(site));
    links.iter().filter(|(_, l)| at(&l.a) || at(&l.b)).map(|(k, l)| crate::links::link_json(world, t0, k, l)).collect()
}

// ---------------------------------------------------------------------------------------
// Names: everything that can be renamed, with its generated name.

pub(super) fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A name as the Gazetteer and `list_names` show it: current and generated, where it is, and
/// whether it can be entered (a building or site with levels).
pub(super) fn name_entry(world: &World, id: String, kind: &str, generated: String, p: Option<P>, enter: bool) -> Value {
    let edits = &world.file.edits;
    let mut v = json!({ "id": id, "kind": kind, "name": edits.renames.get(&id).unwrap_or(&generated), "generated": generated });
    if let Some(p) = p {
        v["x_ft"] = json!(p[0].round());
        v["y_ft"] = json!(p[1].round());
    }
    if enter {
        v["enter"] = json!(true);
    }
    if edits.hidden.contains(&id) {
        v["hidden"] = json!(true);
    }
    v
}
