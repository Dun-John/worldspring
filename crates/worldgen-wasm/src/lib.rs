//! WASM bindings used by the browser workers. Thin by design: all logic lives in `worldgen`
//! so the native build (tests, `mapd`) runs exactly the same code.

use wasm_bindgen::prelude::*;
use worldgen::World;
use worldgen::core::{hash::fnv64, tile::TileKey};
use worldgen::lod::terrain_refine::terrain_tile;
use worldgen::payload::pack_terrain;
use worldgen::pipeline::{JobKey, deps};
use worldgen::t0::T0;

#[wasm_bindgen]
pub struct Ctx {
    world: World,
    t0: Option<T0>,
}

#[wasm_bindgen]
impl Ctx {
    #[wasm_bindgen(constructor)]
    pub fn new(world_json: &str) -> Result<Ctx, JsError> {
        let world = World::from_json(world_json).map_err(|e| JsError::new(&e))?;
        Ok(Ctx { world, t0: None })
    }

    /// World geometry (map size, levels, tile layout) as JSON.
    pub fn geom_json(&self) -> String {
        let mut v = serde_json_value(&self.world.geom);
        v.push_str(&format!(
            ",\"sea_level_ft\":{},\"world_hash\":\"{:016x}\"}}",
            self.world.params().sea_level_ft,
            self.world.hash
        ));
        v
    }

    /// Generate T0 and keep it; returns the serialized blob for other workers.
    /// `progress(stage, fraction)` is called as generation advances.
    pub fn gen_t0(&mut self, progress: &js_sys::Function) -> Vec<u8> {
        let mut report = |stage: &str, f: f64| {
            let _ = progress.call2(&JsValue::NULL, &JsValue::from_str(stage), &JsValue::from_f64(f));
        };
        let mut t0 = T0::generate_with_progress(&self.world, &mut report);
        let bytes = t0.to_bytes();
        t0.apply_edits(&self.world);
        self.t0 = Some(t0);
        bytes
    }

    /// Named features and river polylines (JSON) from the T0 generated in this context.
    pub fn overlay_json(&self) -> String {
        self.t0.as_ref().map(T0::overlay_json).unwrap_or_else(|| "{}".into())
    }

    pub fn load_t0(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        let mut t0 = T0::from_bytes(bytes).map_err(|e| JsError::new(&e))?;
        t0.apply_edits(&self.world);
        self.t0 = Some(t0);
        Ok(())
    }

    /// Replace the world file's edits (live, from the app or an agent through `mapd`): created
    /// sites are re-applied and their cached layouts forgotten; the rest needs nothing here.
    pub fn set_edits(&mut self, edits_json: &str) -> Result<(), JsError> {
        let edits: worldgen::world::Edits = serde_json::from_str(edits_json).map_err(|e| JsError::new(&e.to_string()))?;
        self.world.file.edits = edits;
        if let Some(t0) = &mut self.t0 {
            t0.apply_edits(&self.world);
            worldgen::town::forget_from(t0.settlements.len() + t0.base_pois);
        }
        Ok(())
    }

    /// Replace only the edits fields given (a JSON object of whole fields): a big world's live
    /// edits arrive field by field. Created sites are re-applied only when they are among them.
    pub fn set_edit_fields(&mut self, fields_json: &str) -> Result<(), JsError> {
        let created = self.world.file.edits.set_fields(fields_json).map_err(|e| JsError::new(&e))?;
        if created && let Some(t0) = &mut self.t0 {
            t0.apply_edits(&self.world);
            worldgen::town::forget_from(t0.settlements.len() + t0.base_pois);
        }
        Ok(())
    }

    /// Change some entries of keyed edits fields (`Edits::patch_fields`): never created sites,
    /// so nothing else needs doing here.
    pub fn patch_edits(&mut self, patch_json: &str) -> Result<(), JsError> {
        self.world.file.edits.patch_fields(patch_json).map_err(|e| JsError::new(&e))
    }

    /// Dependencies of an encoded job, flattened as 4 u32 per dependency.
    pub fn deps(&self, kind: u32, layer_level: u32, x: u32, y: u32) -> Vec<u32> {
        match JobKey::decode([kind, layer_level, x, y]) {
            Some(job) => deps(&self.world, &job).iter().flat_map(|d| d.encode()).collect(),
            None => vec![],
        }
    }

    /// Generate the battlemap chunk for a finest-level tile; `parent` is the parent terrain
    /// tile's padded heights.
    pub fn gen_battlemap(&self, level: u8, x: u32, y: u32, parent: Vec<f32>) -> Result<Vec<u8>, JsError> {
        let t0 = self.t0.as_ref().ok_or_else(|| JsError::new("T0 not loaded"))?;
        let key = TileKey::surface(level, x, y);
        let tile = terrain_tile(&self.world, t0, &key, Some(&parent));
        let chunk = worldgen::battlemap::generate(&self.world, t0, &key, &tile);
        Ok(worldgen::battlemap::pack(&self.world, &chunk))
    }

    /// What is at a world position: a building or a settlement (JSON, or `null`).
    pub fn query_json(&self, x: f64, y: f64) -> String {
        match &self.t0 {
            Some(t0) => worldgen::gazetteer::query_json(&self.world, t0, x, y),
            None => "null".into(),
        }
    }

    /// Districts and named buildings matching a search string (JSON array); `rect`
    /// (x0, y0, x1, y1 in ft) limits the search to that area.
    pub fn search_json(&self, q: &str, rect: Option<Vec<f64>>) -> String {
        let rect = rect.filter(|r| r.len() == 4).map(|r| [r[0], r[1], r[2], r[3]]);
        match &self.t0 {
            Some(t0) => worldgen::gazetteer::search_json(&self.world, t0, q, rect),
            None => "[]".into(),
        }
    }

    /// An interior by id (`b:<settlement>:<building>` or `t:<settlement>:<tower>`), JSON or
    /// `null` for what can't be entered.
    pub fn interior_json(&self, id: &str) -> String {
        match &self.t0 {
            Some(t0) => worldgen::interior::interior_json(&self.world, t0, id),
            None => "null".into(),
        }
    }

    /// An underground site as a design, for the designer: `design` (JSON) or the site's own,
    /// changed by `action` (doors where needed, a room furnished, back to the generated site),
    /// with what it builds and its problems (`under::design::design_json`).
    pub fn design_json(&self, id: &str, design: Option<String>, action: Option<String>) -> String {
        match &self.t0 {
            Some(t0) => worldgen::under::design::design_json(&self.world, t0, id, design.as_deref(), action.as_deref()),
            None => r#"{"error":"the world is still being made"}"#.into(),
        }
    }

    /// A place by id (any feature, building, district or site): `{id, name, x, y}` or `null`.
    pub fn place_json(&self, id: &str) -> String {
        let p = self.t0.as_ref().and_then(|t0| worldgen::agent::place(&self.world, t0, id));
        p.map(|v| v.to_string()).unwrap_or_else(|| "null".into())
    }

    /// What can be renamed in a layout (`scope` its index: districts, businesses, towers, sites
    /// underground) or in a building or site (`scope` its id: levels and rooms), JSON array.
    pub fn names_json(&self, scope: &str) -> String {
        match &self.t0 {
            Some(t0) => worldgen::agent::names_json(&self.world, t0, scope),
            None => "[]".into(),
        }
    }

    /// Where a site of `kind` (over `under`, if given) can be created near (x, y), with a name
    /// for it: `{x, y, name}` or `{error}` (`agent::creation_spot`).
    pub fn creation_spot_json(&self, kind: &str, under: Option<String>, id: &str, x: f64, y: f64) -> String {
        match &self.t0 {
            Some(t0) => worldgen::agent::creation_spot_json(&self.world, t0, kind, under.as_deref(), id, [x, y]),
            None => "null".into(),
        }
    }

    /// Whether a building drawn by hand (footprint `[[x, y], …]` JSON, ft) can stand there, its
    /// point and a name for it: `{x, y, name}` or `{error}` (`agent::building_spot`; `id`: its
    /// created id, so reshaping one skips itself).
    pub fn building_spot_json(&self, poly: &str, func: Option<String>, id: &str) -> String {
        let Ok(poly) = serde_json::from_str::<Vec<[f64; 2]>>(poly) else { return r#"{"error":"bad footprint"}"#.into() };
        match &self.t0 {
            Some(t0) => worldgen::agent::building_spot_json(&self.world, t0, &poly, func.as_deref(), id),
            None => "null".into(),
        }
    }

    /// A generated building's edit with a change made (`{func, floors, poly, roof, tint,
    /// structure}`, or `{remove: true}`): `{edit}` (null: as generated) or `{error}`
    /// (`agent::building_edit_json`).
    pub fn building_edit_json(&self, id: &str, change: &str) -> String {
        match &self.t0 {
            Some(t0) => worldgen::agent::building_edit_json(&self.world, t0, id, change),
            None => "null".into(),
        }
    }

    /// The generated buildings with their middle inside a polygon (`[[x, y], …]` JSON, ft):
    /// `[{id, at}]` (`agent::generated_buildings_in`).
    pub fn buildings_in_json(&self, poly: &str) -> String {
        let Ok(poly) = serde_json::from_str::<Vec<[f64; 2]>>(poly) else { return "[]".into() };
        match &self.t0 {
            Some(t0) => worldgen::agent::generated_buildings_in_json(&self.world, t0, &poly),
            None => "null".into(),
        }
    }

    /// Districts and businesses inside a rectangle (x0, y0, x1, y1 in ft), JSON array.
    pub fn in_view_json(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> String {
        match &self.t0 {
            Some(t0) => worldgen::gazetteer::in_view_json(&self.world, t0, [x0, y0, x1, y1]),
            None => "[]".into(),
        }
    }

    /// A settlement's named districts for labels (JSON array).
    pub fn districts_json(&self, settlement: u32) -> String {
        match &self.t0 {
            Some(t0) => worldgen::gazetteer::districts_json(&self.world, t0, settlement as usize),
            None => "[]".into(),
        }
    }

    /// Generate one terrain tile. `parent` is the parent's padded heights for refined levels.
    pub fn gen_terrain(&self, level: u8, x: u32, y: u32, parent: Option<Vec<f32>>) -> Result<Vec<u8>, JsError> {
        let t0 = self.t0.as_ref().ok_or_else(|| JsError::new("T0 not loaded"))?;
        let key = TileKey::surface(level, x, y);
        let tile = terrain_tile(&self.world, t0, &key, parent.as_deref());
        Ok(pack_terrain(&self.world, t0, &key, &tile))
    }
}

/// Object kinds with 5e tactical data (JSON), for the renderer and agents.
#[wasm_bindgen]
pub fn battlemap_catalog_json() -> String {
    worldgen::battlemap::catalog_json()
}

/// What an underground site can be given in the designer: props, room kinds, themes
/// (`under::design::catalog_json`).
#[wasm_bindgen]
pub fn under_catalog_json() -> String {
    worldgen::under::design::catalog_json()
}

/// What a building drawn by hand can be (`agent::building_funcs_json`).
#[wasm_bindgen]
pub fn building_funcs_json() -> String {
    worldgen::agent::building_funcs_json()
}

/// The default world file (current generator version, default parameters) as JSON.
/// A quick look at a world while sketching (`T0::preview`), packed as `[w u32][h u32]
/// [n u32][the sketch's conflicts as JSON, n bytes][RGBA, w * h * 4 bytes]` (little-endian).
#[wasm_bindgen]
pub fn sketch_preview(world_json: &str, width: u32) -> Result<Vec<u8>, JsError> {
    let world = World::from_json(world_json).map_err(|e| JsError::new(&e))?;
    let p = T0::preview(&world, width as usize);
    let conflicts = serde_json::to_string(&p.conflicts).map_err(|e| JsError::new(&e.to_string()))?;
    let mut out = Vec::with_capacity(12 + conflicts.len() + p.rgba.len());
    for v in [p.w as u32, p.h as u32, conflicts.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(conflicts.as_bytes());
    out.extend_from_slice(&p.rgba);
    Ok(out)
}

#[wasm_bindgen]
pub fn default_world_json(seed: u32) -> String {
    serde_json::to_string(&worldgen::WorldFile { seed, ..Default::default() }).expect("serializable")
}

/// Determinism report (see `worldgen::pipeline::det_report`).
#[wasm_bindgen]
pub fn det_report(world_json: &str) -> Result<String, JsError> {
    worldgen::pipeline::det_report(world_json).map_err(|e| JsError::new(&e))
}

#[wasm_bindgen]
pub fn content_hash(bytes: &[u8]) -> String {
    format!("{:016x}", fnv64(bytes))
}

/// Serialize to a JSON object string without the closing brace, so fields can be appended.
fn serde_json_value<T: serde::Serialize>(v: &T) -> String {
    let mut s = serde_json::to_string(v).expect("serializable");
    s.pop();
    s
}
