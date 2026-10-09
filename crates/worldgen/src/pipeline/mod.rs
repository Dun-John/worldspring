//! Job keys, the dependency DAG, and a native executor.
//!
//! Every generated artifact is identified by a `JobKey` and is a pure function of the
//! world plus the artifacts `deps` names. Dependencies only ever point at coarser levels
//! or parent features, so the graph is acyclic and view history can never matter.

use std::collections::HashMap;
use std::fmt::Write;
use std::rc::Rc;

use crate::World;
use crate::core::{
    hash::fnv64,
    tile::TileKey,
};
use crate::lod::terrain_refine::{TerrainOut, terrain_tile};
use crate::payload::pack_terrain;
use crate::t0::T0;

pub const KIND_TERRAIN: u32 = 1;
pub const KIND_BATTLEMAP: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum JobKey {
    Terrain(TileKey),
    /// Battlemap chunk for a finest-level tile.
    Battlemap(TileKey),
}

impl JobKey {
    /// Flat encoding shared with JavaScript: [kind, layer << 8 | level, x, y].
    pub fn encode(&self) -> [u32; 4] {
        match self {
            JobKey::Terrain(k) => [KIND_TERRAIN, (k.layer as u32) << 8 | k.level as u32, k.x, k.y],
            JobKey::Battlemap(k) => [KIND_BATTLEMAP, (k.layer as u32) << 8 | k.level as u32, k.x, k.y],
        }
    }

    pub fn decode(v: [u32; 4]) -> Option<JobKey> {
        match v[0] {
            KIND_TERRAIN => Some(JobKey::Terrain(TileKey {
                layer: (v[1] >> 8) as u8,
                level: (v[1] & 0xff) as u8,
                x: v[2],
                y: v[3],
            })),
            KIND_BATTLEMAP => Some(JobKey::Battlemap(TileKey { layer: (v[1] >> 8) as u8, level: (v[1] & 0xff) as u8, x: v[2], y: v[3] })),
            _ => None,
        }
    }
}

pub fn deps(world: &World, job: &JobKey) -> Vec<JobKey> {
    match job {
        JobKey::Terrain(k) if k.level >= world.geom.first_refine_level => {
            vec![JobKey::Terrain(k.parent().expect("refined level has a parent"))]
        }
        JobKey::Terrain(_) => vec![],
        // The worker regenerates the finest terrain tile from its parent (cheap) rather than
        // shipping carved heights and river water around.
        JobKey::Battlemap(k) => vec![JobKey::Terrain(k.parent().expect("battlemaps live at the finest level"))],
    }
}

/// Native, single-threaded executor with an unbounded cache (tests, tools, `mapd`).
pub struct Executor {
    pub world: World,
    pub t0: T0,
    terrain: HashMap<TileKey, Rc<TerrainOut>>,
}

impl Executor {
    pub fn new(world: World) -> Self {
        let mut t0 = T0::generate(&world);
        t0.apply_edits(&world);
        Self { world, t0, terrain: HashMap::new() }
    }

    pub fn terrain(&mut self, key: TileKey) -> Rc<TerrainOut> {
        if let Some(t) = self.terrain.get(&key) {
            return t.clone();
        }
        let parent = match deps(&self.world, &JobKey::Terrain(key)).first() {
            Some(JobKey::Terrain(p)) => Some(self.terrain(*p)),
            _ => None,
        };
        let tile = Rc::new(terrain_tile(&self.world, &self.t0, &key, parent.as_deref().map(|p| p.padded.as_slice())));
        self.terrain.insert(key, tile.clone());
        tile
    }

    pub fn battlemap(&mut self, key: TileKey) -> crate::battlemap::Chunk {
        let tile = self.terrain(key);
        crate::battlemap::generate(&self.world, &self.t0, &key, &tile)
    }

    pub fn terrain_packed(&mut self, key: TileKey) -> Vec<u8> {
        let tile = self.terrain(key);
        pack_terrain(&self.world, &self.t0, &key, &tile)
    }
}

/// Tiles hashed by the determinism check: full level chains under a few fixed map points
/// plus a finest-level neighbor, so every refinement level is exercised.
pub fn det_sample_keys(world: &World) -> Vec<TileKey> {
    let g = &world.geom;
    let mut keys = Vec::new();
    for (fx, fy) in [(0.37, 0.41), (0.55, 0.62), (0.21, 0.73)] {
        let (x, y) = (fx * g.map_w_ft, fy * g.map_h_ft);
        for level in 0..=g.max_level {
            let size = g.tile_size_ft(level);
            keys.push(TileKey::surface(level, (x / size) as u32, (y / size) as u32));
        }
        let size = g.tile_size_ft(g.max_level);
        keys.push(TileKey::surface(g.max_level, (x / size) as u32 + 1, (y / size) as u32));
    }
    keys
}

/// One line per artifact: `name hash`. Native and WASM builds must print identical reports.
pub fn det_report(world_json: &str) -> Result<String, String> {
    let world = World::from_json(world_json)?;
    let keys = det_sample_keys(&world);
    let mut ex = Executor::new(world);
    let mut out = String::new();
    writeln!(out, "t0 {:016x}", fnv64(&ex.t0.to_bytes())).unwrap();
    writeln!(out, "overlay {:016x}", fnv64(ex.t0.overlay_json().as_bytes())).unwrap();
    for k in &keys {
        let bytes = ex.terrain_packed(*k);
        writeln!(out, "terrain/{}/{}/{} {:016x}", k.level, k.x, k.y, fnv64(&bytes)).unwrap();
    }
    let max_level = ex.world.geom.max_level;
    for k in keys.iter().filter(|k| k.level == max_level) {
        let chunk = ex.battlemap(*k);
        let bytes = crate::battlemap::pack(&ex.world, &chunk);
        writeln!(out, "battlemap/{}/{}/{} {:016x}", k.level, k.x, k.y, fnv64(&bytes)).unwrap();
    }
    // Buildings drawn by hand: their interiors.
    let first = ex.t0.settlements.len() + ex.t0.base_pois;
    for (k, c) in ex.world.file.edits.created.iter().enumerate().filter(|(_, c)| c.kind == "building" && !c.removed) {
        let id = format!("b:{}:0", first + k);
        let json = crate::interior::generate_id(&ex.world, &ex.t0, &id).map(|it| serde_json::to_string(&it).unwrap_or_default()).unwrap_or_default();
        writeln!(out, "interior/{} {} {:016x}", c.id, id, fnv64(json.as_bytes())).unwrap();
    }
    // Castles and walls drawn by hand: their layouts, first tower's interior and the battlemap
    // at their point.
    let works: Vec<(usize, String, [f64; 2])> = ex.world.file.edits.created.iter().enumerate().filter(|(_, c)| (c.kind == "castle" || c.kind == "wall") && !c.removed).map(|(k, c)| (first + k, c.id.clone(), [c.x, c.y])).collect();
    for (li, id, at) in works {
        let l = crate::town::layout(&ex.world, &ex.t0, li);
        let mut h = crate::core::hash::Fnv64::default();
        h.write(format!("{:?} {:?} {:?} {:?}", l.walls, l.towers, l.gate_towers, l.plazas).as_bytes());
        for b in &l.buildings {
            h.write(format!("{} {:?} {:?} {} {:?} {}", b.id, b.poly, b.func, b.floors, b.name, b.pad_ft).as_bytes());
        }
        let json = crate::interior::generate_id(&ex.world, &ex.t0, &format!("t:{li}:0")).map(|it| serde_json::to_string(&it).unwrap_or_default()).unwrap_or_default();
        let size = ex.world.geom.tile_size_ft(max_level);
        let key = TileKey::surface(max_level, (at[0] / size) as u32, (at[1] / size) as u32);
        let chunk = ex.battlemap(key);
        let bytes = crate::battlemap::pack(&ex.world, &chunk);
        writeln!(out, "works/{id} {:016x} {:016x} {:016x}", h.finish(), fnv64(json.as_bytes()), fnv64(&bytes)).unwrap();
    }
    // The world's own buildings edited (a world without edits): in the largest settlement, the
    // first residence taken away, the next made a three-storey smithy; its layout, the smithy's
    // interior and the battlemap where the first stood.
    if ex.world.file.edits.is_empty()
        && let Some(si) = (0..ex.t0.settlements.len()).max_by_key(|&i| ex.t0.settlements[i].population)
    {
        let l = crate::town::layout(&ex.world, &ex.t0, si);
        let homes: Vec<u32> = l.buildings.iter().filter(|b| b.structure == crate::town::Structure::Roofed && b.func.is_none()).map(|b| b.id).take(2).collect();
        if let [gone, smithy] = homes[..] {
            let (gone, smithy) = (format!("b:{si}:{gone}"), format!("b:{si}:{smithy}"));
            let change = crate::agent::BuildingChange { func: Some("blacksmith".into()), floors: Some(3), ..Default::default() };
            let edits = [(gone.clone(), crate::agent::building_removal(&ex.world, &ex.t0, &gone)), (smithy.clone(), crate::agent::building_edit(&ex.world, &ex.t0, &smithy, &change).and_then(|e| e.ok_or_else(String::new)))];
            if let [(_, Ok(a)), (_, Ok(b))] = &edits {
                let at = a.at;
                ex.world.file.edits.buildings.insert(gone.clone(), a.clone());
                ex.world.file.edits.buildings.insert(smithy.clone(), b.clone());
                let l = crate::town::layout(&ex.world, &ex.t0, si);
                let mut h = crate::core::hash::Fnv64::default();
                for b in &l.buildings {
                    h.write(format!("{} {:?} {:?} {} {:?}", b.id, b.poly, b.func, b.floors, b.name).as_bytes());
                }
                let json = crate::interior::generate_id(&ex.world, &ex.t0, &smithy).map(|it| serde_json::to_string(&it).unwrap_or_default()).unwrap_or_default();
                let size = ex.world.geom.tile_size_ft(max_level);
                let key = TileKey::surface(max_level, (at[0] / size) as u32, (at[1] / size) as u32);
                let chunk = ex.battlemap(key);
                let bytes = crate::battlemap::pack(&ex.world, &chunk);
                writeln!(out, "edited/{gone} {smithy} {:016x} {:016x} {:016x}", h.finish(), fnv64(json.as_bytes()), fnv64(&bytes)).unwrap();
                ex.world.file.edits.buildings.clear();
            }
        }
    }
    // The designer's steps on the first site underground: every room furnished again, doors
    // where needed, the plan as text, the site it builds and its problems.
    use crate::under::{UnderKind, design};
    let site = (0..crate::town::layout_count(&ex.t0))
        .find_map(|li| crate::town::layout(&ex.world, &ex.t0, li).entrances.iter().find(|e| e.kind != UnderKind::Sewer).map(|e| format!("u:{li}:{}", e.id)));
    if let Some(id) = site
        && let Ok(mut d) = design::design_of(&ex.world, &ex.t0, &id, true)
    {
        for li in 0..d.levels.len() {
            for ri in 0..d.levels[li].rooms.len() {
                d.furnish(li, ri, 7);
            }
        }
        d.add_doors(None);
        let text = design::to_text(&d, &|_, _| None).unwrap_or_default();
        let json = design::design_json(&ex.world, &ex.t0, &id, Some(&serde_json::to_string(&d).unwrap_or_default()), Some(r#"{"doors":null}"#));
        writeln!(out, "design/{id} {:016x} {:016x}", fnv64(text.as_bytes()), fnv64(json.as_bytes())).unwrap();
    }
    Ok(out)
}
