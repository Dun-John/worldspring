//! MCP tools for villages and towns added without remaking the world (`Created` kind `village`
//! or `town`; laid out by `worldgen::t0::made`).

use serde_json::{Value, json};
use worldgen::agent;
use worldgen::t0::made;
use worldgen::world::{CREATED_SETTLEMENTS, Created, PIN_KINDS};

use crate::Shared;
use crate::tools::{schema, text};

/// What a created settlement may live by: every pin kind but the shore's.
fn lives_by_kinds() -> Vec<&'static str> {
    PIN_KINDS.iter().copied().filter(|k| *k != "port" && *k != "fishing").collect()
}

pub fn list() -> Vec<Value> {
    vec![json!({ "name": "create_settlement", "title": "Add a village or town", "description": "Add a village or town at a place without remaking the world: laid out by the world's own village and town generators (streets, buildings, businesses, battlemaps, interiors), with a track to the nearest road (within 10 mi; farther, none). Nothing else in the world changes. Inland only: refused in water, in a river, on or near a shore (ports and fishing villages need the world remade), crowding another settlement or site, astride a road or on steep ground, with the reason. Culture and what it lives by are taken from the ground if not given. Returns its id (c:...) and get_feature's details. delete_feature takes it away, and sets aside the edits made in it.", "inputSchema": schema(json!({
        "tier": { "type": "string", "enum": CREATED_SETTLEMENTS },
        "id": { "type": "string", "description": "A feature to put it at (its position)" },
        "x_ft": { "type": "number", "description": "Feet east of the map's left edge" },
        "y_ft": { "type": "number", "description": "Feet south of the map's top edge" },
        "name": { "type": "string", "description": "If left out, named in its culture's style" },
        "culture": { "type": "integer", "minimum": 0, "maximum": 4, "description": "Naming culture: 0 cold north, 1 temperate west, 2 warm south, 3 dry lands, 4 open steppe (default: the nearest settlement's)" },
        "lives_by": { "type": "string", "enum": lives_by_kinds(), "description": "What it lives by (default: from its ground)" },
    }), &["tier"]) })]
}

/// Run a settlement tool (None: not one of these). `delete_feature` on a created village or town
/// is taken here too, to set aside the edits in it.
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "create_settlement" => create(app, a).await.map(text),
        "delete_feature" => {
            let id = a["id"].as_str()?.to_string();
            let k = app.with_edits(|e| e.created.iter().position(|c| c.id == id && !c.removed && c.is_settlement())).flatten()?;
            delete(app, id, k).await.map(text)
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
        let n = made::set_aside_changed(&old, e, first);
        Ok(json!({ "tool": "delete_feature", "id": id, "set_aside": n }))
    })
    .await
}
