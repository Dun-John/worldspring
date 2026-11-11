//! MCP tools for the water's edge (R5 M20): rock islets (`raise_islet`, a plateau of kind `islet`) and hand quays, piers and
//! breakwaters (`Edits::wharves`, `q:<id>`), with a lighthouse at a breakwater's tip. The checks live in `worldgen::wharf`
//! (`islet_at`, `root_problem`, `lighthouse_lot`) so the app's own ops can call the same code.

use serde_json::{Value, json};
use worldgen::ground::{self, PlateauKind};
use worldgen::town::geom;
use worldgen::wharf;
use worldgen::world::{Created, CrossingMaterial, Edits, Wharf, WharfKind};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{limit_of, limit_prop, listed, schema, text};

pub fn list() -> Vec<Value> {
    vec![
        json!({ "name": "raise_islet", "title": "Raise a rock islet", "description": "A rock islet in a river, lake or the sea (water at most 120 ft deep), round at; a building, tower or castle may stand on it. With id (h:...), changes it. Returns id, level, water, outline.", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "An islet to change" },
            "at": crate::args::point_schema("A place in the water"),
            "radius_ft": { "type": "number", "minimum": 10, "maximum": 200, "default": 30 },
            "rise_ft": { "type": "number", "minimum": 1, "maximum": 30, "default": 5, "description": "Its top over the water" },
        }), &[]) }),
        json!({ "name": "place_wharf", "title": "Put a quay, pier or breakwater down", "description": "A quay (paving along a shore), pier (a deck) or breakwater (a stone mole; lighthouse: true builds one at its tip) along path (a pier's or breakwater's first point on the bank). With id (q:...), changes it. Returns id, water and bank.", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "A wharf to change" },
            "kind": { "type": "string", "enum": ["quay", "pier", "breakwater"] },
            "path": { "type": "array", "minItems": 2, "maxItems": 16, "items": crate::args::point_schema("A point") },
            "width_ft": { "type": "number", "minimum": 6, "maximum": 60, "description": "By kind: MCP.md" },
            "material": { "type": "string", "enum": ["timber", "stone"] },
            "lighthouse": { "type": "boolean" },
        }), &[]) }),
        json!({ "name": "list_wharves", "title": "Quays, piers and breakwaters", "description": "The wharves put down by hand, with the buildings on them. Returns {items, found}.", "inputSchema": schema(json!({
            "within": crate::args::area_light("Only inside: a box, polygon or circle {at, radius_ft}"),
            "limit": limit_prop(200, 2000),
        }), &[]) }),
        json!({ "name": "remove_wharves", "title": "Take wharves away", "description": "Take wharves off the map (q:..., all or none); their lighthouse goes too.", "inputSchema": schema(crate::args::ids_props("wharf id"), &[]) }),
    ]
}

/// Run a harbour tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "raise_islet" => islet(app, a).await.map(text),
        "place_wharf" => place(app, a).await.map(text),
        "list_wharves" => list_wharves(app, a).await.map(text),
        "remove_wharves" => remove(app, a).await.map(text),
        _ => return None,
    })
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn bbox(pts: &[[f64; 2]]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])])
}

fn num(a: &Value, k: &str) -> Result<Option<f64>, String> {
    match &a[k] {
        Value::Null => Ok(None),
        v => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| format!("{k}: a number")),
    }
}

// ---- islets -----------------------------------------------------------------------------------------------------

async fn islet(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().map(str::to_string);
    let (radius, rise) = (num(a, "radius_ft")?, num(a, "rise_ft")?);
    let at = match (&a["at"], &id) {
        (Value::Null, Some(_)) => None,
        (Value::Null, None) => return Err("at: needed (a place in the water: an id, point or [x, y])".into()),
        (v, _) => Some(crate::args::point(app, "at", v).await?),
    };
    let key = id.clone().unwrap_or_else(|| new_id("h"));
    let (p, w, sea, existing, changed) = app
        .worker
        .with({
            let key = key.clone();
            move |ex| {
                let (world, t0) = (&ex.world, &ex.t0);
                let plateaus = &world.file.edits.plateaus;
                let old = match &id {
                    Some(id) => Some(plateaus.get(id).filter(|p| p.kind == PlateauKind::Islet).ok_or_else(|| format!("no islet {id}"))?),
                    None => None,
                };
                let at = at.or_else(|| old.map(|p| geom::centroid(&p.pts))).ok_or("at: needed")?;
                let sea = world.params().sea_level_ft;
                // A change keeps what it is not told: the radius as the ring is, the rise over the water.
                let radius = radius.or_else(|| old.map(|p| (p.pts.iter().map(|q| geom::dist(*q, at)).sum::<f64>() / p.pts.len() as f64 / 0.97).round().clamp(10.0, 200.0))).unwrap_or(30.0);
                let rise = rise
                    .or_else(|| old.and_then(|p| wharf::water_at(world, t0, at).map(|w| round1(p.level as f64 - w.level as f64).clamp(1.0, 30.0))))
                    .unwrap_or(5.0);
                // The same islet again (a script run twice): its ring for this centre and radius lies within 5 ft of one there.
                if old.is_none() {
                    for (eid, ep) in plateaus.iter().filter(|(_, p)| p.kind == PlateauKind::Islet) {
                        let ring = ground::islet_ring(worldgen::core::rng::stream(0x15_1e7, eid), at, radius);
                        if ring.len() == ep.pts.len() && ring.iter().zip(&ep.pts).all(|(p, q)| geom::dist(*p, *q) <= 5.0) {
                            let w = wharf::water_at(world, t0, at).ok_or_else(|| format!("at: ({:.0}, {:.0}) is on dry land: an islet stands in a river, lake or the sea", at[0], at[1]))?;
                            if (ep.level as f64 - (w.level as f64 + rise)).abs() > 0.15 {
                                return Err(format!("{eid} is an islet there: give its id to change it"));
                            }
                            return Ok((ep.clone(), w, sea, Some(eid.clone()), false));
                        }
                    }
                }
                let (p, w) = wharf::islet_at(world, t0, &key, at, radius, rise)?;
                Ok::<_, String>((p, w, sea, None, old.is_some()))
            }
        })
        .await?;
    let b = bbox(&p.pts);
    let water = json!({ "kind": w.kind, "id": w.id, "name": w.name, "depth_ft": w.depth_ft.round() });
    if let Some(eid) = existing {
        return Ok(json!({
            "tool": "raise_islet", "id": eid, "changed": false, "existing": true, "level_ft": round1(p.level as f64 - sea), "water": water, "poly": p.pts, "bbox": b,
            "note": "an islet here was already there: nothing was added (give its id to change it)",
        }));
    }
    let reply = json!({ "tool": "raise_islet", "id": key, "changed": changed, "level_ft": round1(p.level as f64 - sea), "water": water, "poly": p.pts, "bbox": b });
    let k2 = key.clone();
    app.edit("agent", 0, move |e| {
        if !changed && e.plateaus.len() >= ground::MAX_PLATEAUS {
            return Err(format!("a world holds at most {} plateaus", ground::MAX_PLATEAUS));
        }
        e.plateaus.insert(k2, p);
        Ok(reply)
    })
    .await
}

// ---- wharves ----------------------------------------------------------------------------------------------------

fn kind_of(s: &str) -> Option<WharfKind> {
    match s {
        "quay" => Some(WharfKind::Quay),
        "pier" => Some(WharfKind::Pier),
        "breakwater" => Some(WharfKind::Breakwater),
        _ => None,
    }
}

/// The live buildings drawn on wharf `id` (a lighthouse).
fn on_wharf(e: &Edits, id: &str) -> Vec<usize> {
    (0..e.created.len()).filter(|&k| e.created[k].on.as_deref() == Some(id) && !e.created[k].removed && e.created[k].kind == "building").collect()
}

/// Retire the live buildings on `id` (as `delete_feature` does: what points at them is kept): `(retired, left_pointing)`.
pub fn retire(e: &mut Edits, id: &str) -> (Vec<Value>, Option<Value>) {
    let mut out = Vec::new();
    let (mut npcs, mut plots, mut notes) = (Vec::new(), Vec::new(), false);
    for k in on_wharf(e, id) {
        e.created[k].removed = true;
        let c = &e.created[k];
        out.push(json!({ "id": c.id, "name": c.name }));
        npcs.extend(e.npcs.iter().filter(|(_, n)| n.location.as_ref().is_some_and(|l| l.id == c.id)).map(|(k, _)| k.clone()));
        plots.extend(e.plots.iter().filter(|(_, p)| p.anchors.iter().any(|x| *x == c.id)).map(|(k, _)| k.clone()));
        notes |= e.notes.contains_key(&c.id);
    }
    let mut left = serde_json::Map::new();
    if !npcs.is_empty() {
        left.insert("npcs".into(), json!(npcs));
    }
    if !plots.is_empty() {
        left.insert("plots".into(), json!(plots));
    }
    if notes {
        left.insert("notes".into(), json!(true));
    }
    (out, (!left.is_empty()).then_some(Value::Object(left)))
}

fn near_path(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| geom::dist(*p, *q) <= 5.0)
}

/// A wharf's own description: what the app and `list_wharves` show.
fn wharf_json(e: &Edits, id: &str, w: &Wharf, sea: f64, level: Option<f32>) -> Value {
    let mut v = json!({ "id": id, "kind": w.kind.name(), "path": w.pts, "width_ft": w.width, "length_ft": w.length().round() });
    if w.kind != WharfKind::Quay {
        v["material"] = json!(if w.material == CrossingMaterial::Stone || w.kind == WharfKind::Breakwater { "stone" } else { "timber" });
    }
    if let Some(l) = level {
        v["level_ft"] = json!(round1(l as f64 - sea));
    }
    let on: Vec<&str> = on_wharf(e, id).into_iter().map(|k| e.created[k].id.as_str()).collect();
    if !on.is_empty() {
        v["buildings"] = json!(on);
    }
    v
}

async fn place(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().map(str::to_string);
    let old = match &id {
        Some(i) => Some(app.with_edits(|e| e.wharves.get(i).cloned()).ok_or("no world is open: open the map app first")?.ok_or_else(|| format!("no wharf {i}"))?),
        None => None,
    };
    let kind = match (a["kind"].as_str(), &old) {
        (Some(k), _) => Some(kind_of(k).ok_or_else(|| format!("kind: quay, pier or breakwater, not {k}"))?),
        (None, o) => o.as_ref().map(|w| w.kind),
    };
    if let (Some(k), Some(o)) = (kind, &old)
        && k != o.kind
    {
        return Err(format!("kind: {} is a {}: a wharf's kind does not change (remove it and put another down)", id.as_deref().unwrap_or(""), o.kind.name()));
    }
    let path = match a["path"].as_array() {
        Some(list) => {
            let mut pts = Vec::new();
            for (i, p) in list.iter().enumerate() {
                pts.push(crate::args::point(app, &format!("path[{i}]"), p).await?);
            }
            Some(pts)
        }
        None => None,
    };
    let material = match a["material"].as_str() {
        None => None,
        Some("timber") => Some(CrossingMaterial::Timber),
        Some("stone") => Some(CrossingMaterial::Stone),
        Some(m) => return Err(format!("material: timber or stone, not {m}")),
    };
    let (width, lighthouse) = (num(a, "width_ft")?, a["lighthouse"].as_bool());
    let Some(kind) = kind else { return Err("a new wharf needs kind and path".into()) };
    if lighthouse == Some(true) && kind != WharfKind::Breakwater {
        return Err(format!("lighthouse: only a breakwater has one (this is a {})", kind.name()));
    }
    let w = match &old {
        Some(o) => Wharf { kind, pts: path.clone().unwrap_or_else(|| o.pts.clone()), width: width.unwrap_or(o.width), material: material.unwrap_or(o.material) },
        None => Wharf { kind, pts: path.clone().ok_or("path: needed for a new wharf (2 to 16 points)")?, width: width.unwrap_or_else(|| kind.default_width()), material: material.unwrap_or(CrossingMaterial::Timber) },
    };
    let w = Wharf { material: if kind == WharfKind::Breakwater { CrossingMaterial::Stone } else { w.material }, ..w };
    let [mw, mh] = app.worker.with(|ex| Ok([ex.world.geom.map_w_ft, ex.world.geom.map_h_ft])).await?;
    if let Some(p) = w.problem(mw, mh) {
        return Err(p);
    }
    // The same wharf again (a script run twice): that one, nothing added.
    let repeat = match (&id, &path) {
        (None, Some(p)) => app.with_edits({
            let (p, k) = (p.clone(), kind);
            move |e| {
                e.wharves.iter().find(|(_, o)| o.kind == k && (near_path(&o.pts, &p) || (k == WharfKind::Quay && near_path(&o.pts, &p.iter().rev().copied().collect::<Vec<_>>())))).map(|(i, o)| (i.clone(), o.clone()))
            }
        }).flatten(),
        _ => None,
    };
    if let Some((eid, o)) = &repeat
        && (width.is_some_and(|x| (x - o.width).abs() > 1e-6) || material.is_some_and(|m| kind != WharfKind::Breakwater && m != o.material))
    {
        return Err(format!("{eid} already runs along this path (a {}, {} ft wide): give its id to change it", o.kind.name(), o.width));
    }
    let w = repeat.as_ref().map(|(_, o)| o.clone()).unwrap_or(w);
    // Where it stands: its root (a pier or breakwater starts on the bank), the water and bank there, its top.
    let (w2, check_root) = (w.clone(), repeat.is_none());
    let (info, level, sea) = app
        .worker
        .run(move |g| {
            let ex = g.ex.as_mut().ok_or("no world is open: open the map app (it connects to mapd and shares its world)")?;
            if check_root && let Some(p) = wharf::root_problem(ex, &w2) {
                return Err(p);
            }
            let (world, t0) = (&ex.world, &ex.t0);
            let p0 = w2.pts[0];
            let mut info = json!({});
            if let Some(r) = worldgen::agent::river_at(world, t0, p0, 300.0) {
                let (_, d) = r.bank_of(p0, 0.0);
                info["river"] = r.json(Some(p0));
                info["bank_ft"] = json!(d.round());
            } else if let Some(wa) = w2.pts.windows(2).flat_map(|s| (0..=(geom::dist(s[0], s[1]) / 10.0).ceil().max(1.0) as usize).map(move |k| geom::lerp(s[0], s[1], k as f64 / (geom::dist(s[0], s[1]) / 10.0).ceil().max(1.0)))).find_map(|p| wharf::water_over(world, t0, p)) {
                info["water"] = json!({ "kind": wa.kind, "id": wa.id, "name": wa.name });
            } else {
                info["water"] = Value::Null;
            }
            let wet = wharf::water(&w2, t0, 10.0) > worldgen::t0::hydro::DRY;
            Ok::<_, String>((info, wet.then(|| wharf::level(&w2, t0)), world.params().sea_level_ft))
        })
        .await?;
    let (retire_old, keep) = (id.is_some() && path.as_ref().is_some_and(|p| old.as_ref().is_some_and(|o| o.pts != *p)), repeat.as_ref().map(|r| r.0.clone()));
    let wid = keep.clone().or(id.clone()).unwrap_or_else(|| new_id("q"));
    let (w3, wid2, existing, is_change) = (w.clone(), wid.clone(), keep.is_some(), id.is_some());
    let mut retired = Vec::new();
    let mut left = None;
    let (mut v, had) = {
        let r = app
            .edit("agent", 0, move |e| {
                let had = !on_wharf(e, &wid2).is_empty();
                let (mut gone, mut left) = (Vec::new(), None);
                if !existing {
                    e.wharves.insert(wid2.clone(), w3.clone());
                    if retire_old || lighthouse == Some(false) {
                        (gone, left) = retire(e, &wid2);
                    }
                } else if lighthouse == Some(false) {
                    (gone, left) = retire(e, &wid2);
                }
                let mut v = json!({ "tool": "place_wharf", "id": wid2, "kind": kind.name(), "changed": is_change, "wharf": wharf_json(e, &wid2, &w3, sea, level) });
                if existing {
                    v["changed"] = json!(false);
                    v["existing"] = json!(true);
                    v["note"] = json!("a wharf of this kind along this path was already there: nothing was added (give its id to change it)");
                }
                v["_gone"] = json!(gone);
                v["_left"] = left.unwrap_or(Value::Null);
                Ok(json!({ "v": v, "had": had }))
            })
            .await?;
        (r["v"].clone(), r["had"].as_bool().unwrap_or(false))
    };
    for (k, x) in info.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    if let Some(g) = v.as_object_mut().and_then(|o| o.remove("_gone")).and_then(|g| g.as_array().cloned()) {
        retired = g;
    }
    if let Some(l) = v.as_object_mut().and_then(|o| o.remove("_left")).filter(|l| !l.is_null()) {
        left = Some(l);
    }
    // A lighthouse at the tip: asked for, or one that stood before a changed path.
    let want = kind == WharfKind::Breakwater && lighthouse.unwrap_or(retire_old && had);
    let mut made = Vec::new();
    if want {
        match lighthouse_at(app, &wid, &w).await {
            Ok(Some(c)) => made.push(c),
            Ok(None) => {}
            Err(e) if id.is_none() && !existing => {
                // (A new breakwater whose lighthouse can't stand: neither is made.)
                let wid3 = wid.clone();
                app.edit("agent", 0, move |ed| {
                    ed.wharves.remove(&wid3);
                    Ok(json!({}))
                })
                .await?;
                return Err(format!("lighthouse: {e}; nothing was put down"));
            }
            Err(e) => v["note"] = json!(format!("the lighthouse was not built: {e}")),
        }
    }
    if !made.is_empty() || !retired.is_empty() {
        v["buildings"] = json!(made);
    }
    if !retired.is_empty() {
        v["retired"] = json!(retired);
    }
    if let Some(l) = left {
        v["left_pointing"] = l;
    }
    Ok(v)
}

/// A lighthouse on breakwater `wid` (the wharf is in the world): its footprint at the tip, checked as a building, made as `on` it.
/// `Ok(None)` when one already stands.
async fn lighthouse_at(app: &Shared, wid: &str, w: &Wharf) -> Result<Option<String>, String> {
    if app.with_edits(|e| !on_wharf(e, wid).is_empty()).unwrap_or(false) {
        return Ok(None);
    }
    let poly = wharf::lighthouse_lot(w).ok_or("a breakwater has one")?;
    let cid = format!("c:{}", app.with_edits(|e| e.created.len()).unwrap_or(0));
    let (p, name) = app
        .worker
        .with({
            let (poly, cid) = (poly.clone(), cid.clone());
            move |ex| {
                let skip = worldgen::agent::spot_skip(&ex.world, &ex.t0, &cid);
                worldgen::agent::building_spot(&ex.world, &ex.t0, &poly, Some("lighthouse"), &cid, skip)
            }
        })
        .await?;
    let wid = wid.to_string();
    let r = app
        .edit("agent", 0, move |e| {
            let id = format!("c:{}", e.created.len());
            let c = Created { id: id.clone(), kind: "building".into(), x: p[0], y: p[1], name, poly, floors: Some(3), func: Some("lighthouse".into()), on: Some(wid), ..Default::default() };
            c.check()?;
            e.created.push(c);
            Ok(json!({ "id": id }))
        })
        .await?;
    Ok(r["id"].as_str().map(str::to_string))
}

async fn list_wharves(app: &Shared, a: &Value) -> Result<Value, String> {
    let area = if a["within"].is_null() { None } else { Some(crate::args::area(app, "within", &a["within"]).await?) };
    let limit = limit_of(a, 200);
    let v = app
        .worker
        .with(move |ex| {
            let sea = ex.world.params().sea_level_ft;
            let e = &ex.world.file.edits;
            let items: Vec<Value> = e
                .wharves
                .iter()
                .filter(|(_, w)| {
                    area.as_ref().is_none_or(|ar| {
                        let b = bbox(&w.pts);
                        let m = ar.middle();
                        w.pts.iter().any(|&q| ar.contains(q)) || (m[0] >= b[0] && m[0] <= b[2] && m[1] >= b[1] && m[1] <= b[3])
                    })
                })
                .map(|(id, w)| {
                    let wet = wharf::water(w, &ex.t0, 10.0) > worldgen::t0::hydro::DRY;
                    wharf_json(e, id, w, sea, wet.then(|| wharf::level(w, &ex.t0)))
                })
                .collect();
            Ok(json!(items))
        })
        .await?;
    Ok(listed(v, limit, json!({})))
}

async fn remove(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::args::ids(a)?;
    let single = a["ids"].is_null();
    app.edit("agent", 0, move |e| {
        for (i, id) in ids.iter().enumerate() {
            if !e.wharves.contains_key(id) {
                return Err(format!("{}: no wharf {id}", crate::args::item(single, i)));
            }
        }
        let (mut gone, mut npcs, mut plots, mut notes) = (Vec::new(), Vec::new(), Vec::new(), false);
        for id in &ids {
            e.wharves.remove(id);
            let (g, l) = retire(e, id);
            gone.extend(g);
            if let Some(l) = l {
                npcs.extend(l["npcs"].as_array().cloned().unwrap_or_default());
                plots.extend(l["plots"].as_array().cloned().unwrap_or_default());
                notes |= l["notes"].as_bool().unwrap_or(false);
            }
        }
        let mut v = json!({ "tool": "remove_wharves", "ids": ids });
        if !gone.is_empty() {
            v["buildings"] = json!(gone);
        }
        let mut left = serde_json::Map::new();
        if !npcs.is_empty() {
            left.insert("npcs".into(), json!(npcs));
        }
        if !plots.is_empty() {
            left.insert("plots".into(), json!(plots));
        }
        if notes {
            left.insert("notes".into(), json!(true));
        }
        if !left.is_empty() {
            v["left_pointing"] = Value::Object(left);
        }
        Ok(v)
    })
    .await
}
