//! MCP tools for villages and towns added without remaking the world (`Created` kind `village`
//! or `town`; laid out by `worldgen::t0::made`), and for changing a settlement's tier, what it
//! lives by, whether it is the capital and its name (`update_settlement`): a made one at once, one
//! of the world's own through a sketch pin in its place (`Stroke::replaces`) and `regenerate`.

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::t0::made;
use worldgen::t0::settle::SettleKind;
use worldgen::world::{CREATED_SETTLEMENTS, Created, PIN_KINDS, PIN_TIERS};

use crate::Shared;
use crate::tools::{schema, text};

/// What a created settlement may live by: every pin kind but the shore's.
fn lives_by_kinds() -> Vec<&'static str> {
    PIN_KINDS.iter().copied().filter(|k| *k != "port" && *k != "fishing").collect()
}

pub fn list() -> Vec<Value> {
    vec![json!({ "name": "create_settlement", "title": "Add a village or town", "description": "Add a village or town at a place without remaking the world, laid out by the world's own generators and joined to the nearest road. Inland only: refused, with the reason, in water, on or near a shore, crowding another settlement or site, astride a road, on steep ground, or over 10 mi from a road. Culture and what it lives by come from the ground if not given. Returns its id (c:...) and get_feature's details. delete_feature takes it away and sets aside its edits.", "inputSchema": schema(json!({
        "tier": { "type": "string", "enum": CREATED_SETTLEMENTS },
        "at": crate::args::point_schema("Where its centre goes (a place)"),
        "name": { "type": "string", "minLength": 1, "maxLength": worldgen::world::NAME_MAX, "description": "If left out, named in its culture's style" },
        "culture": { "type": "integer", "minimum": 0, "maximum": 4, "description": "Naming culture: 0 cold north, 1 temperate west, 2 warm south, 3 dry lands, 4 steppe (default: nearest settlement's)" },
        "lives_by": { "type": "string", "enum": lives_by_kinds(), "description": "What it lives by (default: from its ground)" },
    }), &["tier", "at"]) }),
    json!({ "name": "update_settlement", "title": "Change a settlement", "description": "Change a settlement's tier, what it lives by, whether it is the realm's capital, or its name (Neverwinter a 2,000-person town, Luskan a fishing village). One added by hand (c:...) changes at once while it stays an inland village or town (else the world must be remade: refused, saying how). One of the world's own changes through a settlement pin in the sketch (pending: true until regenerate, or regenerate: true here), laid out anew in the same place: names, notes, NPCs and plots follow it, edits inside it are set aside.", "inputSchema": schema(json!({
        "id": { "type": "string", "description": "The settlement (a metropolis, city, town or village; c:... for one added by hand)" },
        "tier": { "type": "string", "enum": PIN_TIERS, "description": "Its new size (default: as it is)" },
        "lives_by": { "type": "string", "enum": PIN_KINDS, "description": "What it lives by (default: as it is if its tier stays, else from its land)" },
        "capital": { "type": "boolean", "description": "true: the realm's capital (the world's own settlements only)" },
        "name": { "type": "string", "minLength": 1, "maxLength": worldgen::world::NAME_MAX, "description": "Its new name (default: as it is named now)" },
        "regenerate": { "type": "boolean", "default": false, "description": "One of the world's own: remake the world at once (as regenerate)" },
    }), &["id"]), "annotations": { "openWorldHint": false, "idempotentHint": true } }),
    ]
}

/// Run a settlement tool (None: not one of these). `delete_feature` on a created village or town
/// is taken here too, to set aside the edits in it.
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "create_settlement" => create(app, a).await.map(text),
        "update_settlement" => update(app, a).await.map(text),
        "delete_feature" => {
            let ids = crate::args::ids(a).ok()?;
            let [id] = &ids[..] else { return None };
            let id = id.clone();
            let k = app.with_edits(|e| e.created.iter().position(|c| c.id == id && !c.removed && c.is_settlement())).flatten()?;
            delete(app, id, k).await.map(text)
        }
        "restore_feature" => {
            let ids = crate::args::ids(a).ok()?;
            let removed = |e: &worldgen::world::Edits, id: &String| e.created.iter().position(|c| &c.id == id && c.removed && c.is_settlement());
            let ks: Vec<Option<usize>> = app.with_edits(|e| ids.iter().map(|id| removed(e, id)).collect()).unwrap_or_default();
            if !ks.iter().any(Option::is_some) {
                return None;
            }
            if ids.len() != 1 {
                return Some(Err("restore_feature: a deleted village or town is restored on its own (one id per call), apart from other sites".into()));
            }
            restore(app, ids[0].clone(), ks[0]?).await.map(text)
        }
        _ => return None,
    })
}

/// The first created layout's index (`town::forget_created`'s `first`).
async fn first_created(app: &Shared) -> Result<usize, String> {
    app.worker.with(|ex| Ok(ex.t0.settlements.len() + ex.t0.base_pois)).await
}

async fn create(app: &Shared, a: &Value) -> Result<Value, String> {
    let tier = crate::tools::arg_str(a, "tier")?;
    if !CREATED_SETTLEMENTS.contains(&tier.as_str()) {
        return Err(format!("tier: {}, not {tier}", CREATED_SETTLEMENTS.join(" or ")));
    }
    let culture = match &a["culture"] {
        Value::Null => None,
        v => Some(v.as_u64().filter(|c| (*c as usize) < worldgen::t0::names::CULTURES.len()).ok_or_else(|| format!("culture: 0 to {}", worldgen::t0::names::CULTURES.len() - 1))? as u8),
    };
    let lives_by = a["lives_by"].as_str().map(str::to_string);
    if let Some(k) = &lives_by
        && !lives_by_kinds().contains(&k.as_str())
    {
        return Err(format!("lives_by: one of {} (ports and fishing villages need the world remade)", lives_by_kinds().join(", ")));
    }
    let at = crate::tools::place(app, a).await?;
    let asked = a["name"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let n = app.with_edits(|e| e.created.len()).unwrap_or(0);
    let mut c = Created { id: format!("c:{n}"), kind: tier.clone(), x: at[0], y: at[1], culture, lives_by, ..Default::default() };
    c.check()?;
    // The same settlement again (a script run twice): that one, nothing added.
    if let Some(id) = app.with_edits(|e| e.existing_site(&c, at).map(|o| o.id.clone())).flatten() {
        return crate::tools::found(app, id).await;
    }
    // Checked, and what it was not given taken from the ground where it stands.
    let probe = c.clone();
    c = app
        .worker
        .with(move |ex| {
            let mut c = probe;
            made::spot(&ex.world, &ex.t0, &c.kind, &c.id, at)?;
            let culture = *c.culture.get_or_insert_with(|| made::culture_at(&ex.t0, c.x, c.y));
            c.lives_by.get_or_insert_with(|| PIN_KINDS[made::kind_at(&ex.world, &ex.t0, c.x, c.y, made::tier_of(&c.kind)) as usize].to_string());
            c.name = asked.unwrap_or_else(|| made::name(&ex.world, &ex.t0, &c.id, culture));
            Ok(c)
        })
        .await?;
    let change = app
        .edit("agent", 0, move |e| {
            if let Some(o) = e.existing_site(&c, at) {
                return Ok(json!({ "existing": o.id }));
            }
            // (Another client may have created a site meanwhile: take the next id.)
            c.id = format!("c:{}", e.created.len());
            let reply = json!({ "tool": "create_settlement", "id": c.id, "kind": c.kind, "name": c.name, "x_ft": c.x.round(), "y_ft": c.y.round() });
            e.created.push(c);
            Ok(reply)
        })
        .await?;
    if let Some(id) = change["existing"].as_str() {
        return crate::tools::found(app, id.to_string()).await;
    }
    let id = change["id"].as_str().unwrap_or_default().to_string();
    app.worker.with(move |ex| read_back(ex, &id).ok_or_else(|| "created, but it could not be read back".to_string())).await
}

/// `get_feature`'s details of created settlement `id`, with its culture, what it lives by and its
/// track's length (null: no road within reach).
fn read_back(ex: &worldgen::pipeline::Executor, id: &str) -> Option<Value> {
    let mut v = agent::get(&ex.world, &ex.t0, id)?;
    let k = ex.world.file.edits.created.iter().position(|c| c.id == id)?;
    let c = &ex.world.file.edits.created[k];
    let m = ex.t0.made.iter().find(|m| m.poi == ex.t0.base_pois + k)?;
    let len: f64 = m.track.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum();
    v["culture"] = json!(c.culture);
    v["lives_by"] = json!(c.lives_by);
    v["population"] = json!(m.s.population);
    v["track_ft"] = if m.track.is_empty() { Value::Null } else { json!(len.round()) };
    Some(v)
}

/// `delete_feature` for created settlement `id` (at `k` in `created`): marked removed, and the
/// edits in its layout set aside (`made::set_aside`).
async fn delete(app: &Shared, id: String, k: usize) -> Result<Value, String> {
    let first = first_created(app).await?;
    app.edit("agent", 0, move |e| {
        let old = e.created.clone();
        let c = e.created.get_mut(k).filter(|c| c.id == id && !c.removed).ok_or_else(|| format!("{id} was deleted meanwhile"))?;
        c.removed = true;
        let (name, kind) = (c.name.clone(), c.kind.clone());
        // What pointed at it (the same account `delete_feature` gives for other created sites).
        let li = first + k;
        let here = |x: &str| x == id || worldgen::world::layout_in_id(x) == Some(li);
        let mut left = serde_json::Map::new();
        let npcs: Vec<&String> = e.npcs.iter().filter(|(_, n)| n.location.as_ref().is_some_and(|l| here(&l.id))).map(|(k, _)| k).collect();
        let plots: Vec<&String> = e.plots.iter().filter(|(_, p)| p.anchors.iter().any(|x| here(x))).map(|(k, _)| k).collect();
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
        let n = made::set_aside_changed(&old, e, first);
        let mut v = json!({ "id": id, "kind": kind, "name": name });
        let any = !left.is_empty();
        if any {
            v["left_pointing"] = Value::Object(left);
        }
        let mut reply = json!({ "tool": "delete_feature", "deleted": [v], "id": id, "set_aside": n });
        if any || n > 0 {
            reply["note"] = json!("what was made inside it (names, notes, buildings, NPCs' places, plot points) is set aside, moved off its layout number; restore_feature brings the settlement back, not those");
        }
        Ok(reply)
    })
    .await
}

/// `restore_feature` for a deleted village or town: the spot must be free by the checks a new one
/// meets; what was set aside when it was deleted stays set aside.
async fn restore(app: &Shared, id: String, k: usize) -> Result<Value, String> {
    let _hold = if crate::batch::active() { None } else { Some(app.gate.lock().await) };
    let c = app.with_edits(move |e| e.created.get(k).cloned()).flatten().ok_or_else(|| format!("{id} is not a created settlement"))?;
    let (probe, id) = (c.clone(), c.id.clone());
    app.worker
        .with(move |ex| made::spot(&ex.world, &ex.t0, &probe.kind, &probe.id, [probe.x, probe.y]).map(|_| ()).map_err(|e| format!("{} can't be restored, its place is taken: {e}", probe.id)))
        .await?;
    let id2 = id.clone();
    let change = move |e: &mut worldgen::world::Edits| {
        let c = e.created.get(k).cloned().filter(|c| c.id == id2 && c.removed).ok_or_else(|| format!("{id2} is not deleted"))?;
        if let Some(o) = e.existing_site(&c, [c.x, c.y]) {
            return Err(format!("{id2} can't be restored, its place is taken: {} is the same site", o.id));
        }
        e.created[k].removed = false;
        Ok(json!({ "tool": "restore_feature", "id": id2 }))
    };
    if crate::batch::active() {
        app.edit("agent", 0, change).await?;
    } else {
        app.edit_now("agent", 0, change).await?;
    }
    drop(_hold);
    let mut v = app.worker.with(move |ex| read_back(ex, &id).ok_or_else(|| "restored, but it could not be read back".to_string())).await?;
    v["tool"] = json!("restore_feature");
    v["note"] = json!("what was set aside when it was deleted (names, notes, buildings, NPCs' places, plot points) is not brought back");
    Ok(v)
}

/// What an `update_settlement` asks: tier, lives_by, capital and name, each if given.
struct Asked {
    tier: Option<String>,
    lives_by: Option<String>,
    capital: Option<bool>,
    name: Option<String>,
}

fn asked(a: &Value) -> Result<Asked, String> {
    for k in a.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
        if !["id", "tier", "lives_by", "capital", "name", "regenerate"].contains(&k.as_str()) {
            return Err(format!("{k}: unknown argument (update_settlement takes id, tier, lives_by, capital, name, regenerate)"));
        }
    }
    let word = |k: &str, of: &[&str]| match &a[k] {
        Value::Null => Ok(None),
        Value::String(w) if of.contains(&w.as_str()) => Ok(Some(w.clone())),
        v => Err(format!("{k}: one of {}, got {v}", of.join(", "))),
    };
    let flag = |k: &str| match &a[k] {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        v => Err(format!("{k}: true or false, got {v}")),
    };
    let name = match &a["name"] {
        Value::Null => None,
        Value::String(n) if !n.trim().is_empty() && n.trim().chars().count() <= worldgen::world::NAME_MAX => Some(n.trim().to_string()),
        v => return Err(format!("name: a name of 1 to {} characters, got {v}", worldgen::world::NAME_MAX)),
    };
    flag("regenerate")?;
    Ok(Asked { tier: word("tier", &PIN_TIERS)?, lives_by: word("lives_by", &PIN_KINDS)?, capital: flag("capital")?, name })
}

async fn update(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = crate::tools::arg_str(a, "id")?;
    let ask = asked(a)?;
    if ask.tier.is_none() && ask.lives_by.is_none() && ask.capital.is_none() && ask.name.is_none() {
        return Err("give what changes: tier, lives_by, capital or name".into());
    }
    if let Some(k) = app.with_edits(|e| e.created.iter().position(|c| c.id == id && !c.removed && c.is_settlement())).flatten() {
        return update_made(app, id, k, ask).await;
    }
    if crate::batch::active() {
        return Err("update_settlement can't change one of the world's own settlements in a batch: the sketch is changed (and the world remade) on its own".into());
    }
    let wid = id.clone();
    let lives = ask.lives_by.clone();
    // The settlement: its layout, pin, tier as placed, kind, name, cell centre, and a shore check.
    let (li, pin, tier, planned, kind, name, spot, capital, shore) = app
        .worker
        .with(move |ex| {
            let (w, t0) = (&ex.world, &ex.t0);
            let li = agent::layout_of(w, t0, &wid).filter(|&li| li < t0.settlements.len()).ok_or_else(|| format!("id: a settlement (a metropolis, city, town or village; search_features finds them), got {wid:?}"))?;
            let s = &t0.settlements[li];
            let name = w.file.edits.renames.get(&wid).cloned().or_else(|| t0.extra.as_ref()?.overlay.features.iter().find(|f| f.id == wid).map(|f| f.name.clone())).unwrap_or_default();
            let gw = t0.height.w;
            let spot = [(s.cell % gw) as f64 * t0.cell_ft, (s.cell / gw) as f64 * t0.cell_ft];
            // (A port or fishing village needs sea or lake within reach of its spot, as a pin does.)
            let shore = match lives.as_deref() {
                Some(k @ ("port" | "fishing")) => {
                    let wet = (1..=4).any(|r| (0..24).any(|q| {
                        let ang = std::f64::consts::TAU * q as f64 / 24.0;
                        let d = 0.4 * r as f64 * t0.cell_ft;
                        t0.sample_lake(spot[0] + d * ang.cos(), spot[1] + d * ang.sin()).1 >= 0.5
                    }));
                    wet || s.coastal || matches!(s.kind, SettleKind::Port | SettleKind::Fishing) || (k == "fishing" && s.river)
                }
                _ => true,
            };
            Ok((li, s.pin, s.tier.name().to_string(), s.planned.name().to_string(), PIN_KINDS[s.kind as usize].to_string(), name, spot, s.capital, shore))
        })
        .await?;
    if !shore {
        return Err(format!("lives_by: a port or fishing village stands on a sea or lake shore (fishing also on a river): {name} is inland. Leave lives_by out, or give another (river, mining, fortress, market, farming, lumber, herding, oasis)"));
    }
    let new_tier = ask.tier.clone().unwrap_or_else(|| tier.clone());
    // (What it lives by stays as it is, unless asked, while the new tier allows it: a port is no
    // village's, a fishing village no town's; else it is from its land.)
    let fits = new_tier == tier || !((kind == "port" && new_tier == "village") || (kind == "fishing" && new_tier != "village"));
    let new_kind = ask.lives_by.clone().or_else(|| fits.then(|| kind.clone()));
    let economy = (ask.tier.is_some() && new_tier != tier && ask.lives_by.is_none()).then(|| {
        if fits { format!("It keeps what it lives by ({kind}), which a {new_tier} can") } else { format!("What it lives by ({kind}) does not fit a {new_tier}: it is taken from its land") }
    });
    let fresh = crate::sketch::pin_stroke(spot, &new_tier);
    let (asked_name, asked_capital) = (ask.name.clone(), ask.capital);
    let given_tier = ask.tier.is_some();
    let (lives_given, asked_kind) = (ask.lives_by.is_some(), ask.lives_by.clone());
    let made_new = pin.is_none();
    let r = crate::sketch::put_pin(app, pin.map(|k| k as usize), spot, fresh, move |s| {
        if made_new && s.replaces.is_none() {
            // (In place of the world's own: as it was placed, and as it is named now.)
            s.replaces = Some(planned);
            s.name = Some(name);
            s.kind = new_kind.clone();
            s.capital = capital;
        }
        if given_tier {
            s.tier = Some(new_tier);
            if s.kind.is_none() {
                s.kind = new_kind.clone();
            }
        }
        if lives_given {
            s.kind = asked_kind;
        }
        if let Some(c) = asked_capital {
            s.capital = c;
        }
        if let Some(n) = asked_name {
            s.name = Some(n);
        }
    })
    .await?;
    let mut out = json!({ "id": id, "layout": li, "pin": r["stroke"]["id"], "stroke": r["stroke"], "pending": r["pending"] });
    if r["unchanged"] == json!(true) {
        out["unchanged"] = json!(true);
    }
    if let Some(e) = economy {
        out["economy"] = json!(e);
    }
    if a["regenerate"] == json!(true) {
        let g = crate::sketch::regenerate(app).await?;
        let now = app.worker.with(move |ex| Ok(agent::feature_of_layout(&ex.world, &ex.t0, li))).await?;
        out["pending"] = json!(false);
        out["now"] = json!(now);
        out["regenerate"] = g;
        out["note"] = json!("The world was remade: the settlement is now `now` (same layout number), with its names, notes, NPCs and plot points; changes inside it were set aside (regenerate's set_aside)");
    } else {
        out["note"] = json!("Stored as a pin in its place in the sketch (pin): regenerate remakes the world with it. The settlement keeps its place and layout number, and its names, notes, NPCs and plot points follow it; it is laid out anew, so changes inside it are set aside");
    }
    Ok(out)
}

/// `update_settlement` on a village or town added by hand (at `k` in `created`): changed at once
/// (laid out anew when its tier or kind changes: the edits in it set aside).
async fn update_made(app: &Shared, id: String, k: usize, ask: Asked) -> Result<Value, String> {
    let how = "delete_feature it and put a settlement pin there (create_sketch_stroke {tool: pin, at, tier, lives_by, capital, name}), then regenerate";
    if let Some(t) = ask.tier.as_deref().filter(|t| !CREATED_SETTLEMENTS.contains(t)) {
        return Err(format!("tier: a village or town added by hand stays a village or town ({t} needs the world remade: {how})"));
    }
    if ask.capital == Some(true) {
        return Err(format!("capital: a village or town added by hand can't be the capital (that needs the world remade: {how})"));
    }
    if let Some(l) = ask.lives_by.as_deref().filter(|l| !lives_by_kinds().contains(l)) {
        return Err(format!("lives_by: a village or town added by hand stands inland, so not {l} (a port or fishing village needs the world remade: {how})"));
    }
    let first = first_created(app).await?;
    let wid = id.clone();
    let change = app
        .edit("agent", 0, move |e| {
            let old = e.created.clone();
            let c = e.created.get_mut(k).filter(|c| c.id == wid && !c.removed).ok_or_else(|| format!("{wid} was deleted meanwhile"))?;
            let was = (c.kind.clone(), c.lives_by.clone(), c.name.clone());
            if let Some(t) = ask.tier {
                c.kind = t;
            }
            if let Some(l) = ask.lives_by {
                c.lives_by = Some(l);
            }
            if let Some(n) = &ask.name {
                c.name = n.clone();
            }
            let now = (c.kind.clone(), c.lives_by.clone(), c.name.clone());
            c.check()?;
            // (A name given wins over a rename made before.)
            let renamed = ask.name.is_some() && e.renames.remove(&wid).is_some();
            if now == was && !renamed {
                return Ok(json!({ "unchanged": true }));
            }
            let n = made::set_aside_changed(&old, e, first);
            Ok(json!({ "tool": "update_settlement", "id": wid, "kind": now.0, "lives_by": now.1, "name": now.2, "set_aside": n }))
        })
        .await?;
    let set_aside = change["set_aside"].clone();
    let rid = id.clone();
    let mut v = app.worker.with(move |ex| read_back(ex, &rid).ok_or_else(|| "changed, but it could not be read back".to_string())).await?;
    if change["unchanged"] == json!(true) {
        v["unchanged"] = json!(true);
    } else {
        v["set_aside"] = set_aside;
        v["pending"] = json!(false);
        v["note"] = json!("Changed at once (no remake): laid out anew if its tier or what it lives by changed, the changes made inside it set aside (set_aside counts them); its name, notes, NPCs and plot points stay on it");
    }
    Ok(v)
}
