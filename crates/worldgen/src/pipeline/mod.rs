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
    // Flights of steps on the steep streets of every city.
    {
        let mut h = crate::core::hash::Fnv64::default();
        let mut n = 0;
        // The highest flight's settlement and middle (the first highest).
        let mut top: Option<(usize, f32, [f64; 2])> = None;
        for si in (0..ex.t0.settlements.len()).filter(|&i| ex.t0.settlements[i].tier >= crate::town::stairs::STAIR_TIER) {
            let l = crate::town::layout(&ex.world, &ex.t0, si);
            h.write(format!("{si} {:?}", l.stairs).as_bytes());
            n += l.stairs.len();
            for st in &l.stairs {
                if top.is_none_or(|t| st.rise > t.1) {
                    top = Some((si, st.rise, crate::town::geom::lerp(st.pts[0], st.pts[st.pts.len() - 1], 0.5)));
                }
            }
        }
        writeln!(out, "stairs {n} {:016x}", h.finish()).unwrap();
        // Drawn: the battlemap round its middle and the tile above it (the hatch).
        if let Some((si, _, p)) = top {
            let g = ex.world.geom.clone();
            for level in [max_level, max_level - 2] {
                let size = g.tile_size_ft(level);
                let k = TileKey::surface(level, (p[0] / size) as u32, (p[1] / size) as u32);
                let bytes = if level == max_level {
                    let chunk = ex.battlemap(k);
                    crate::battlemap::pack(&ex.world, &chunk)
                } else {
                    ex.terrain_packed(k)
                };
                writeln!(out, "stairs/{si}/{}/{}/{} {:016x}", k.level, k.x, k.y, fnv64(&bytes)).unwrap();
            }
        }
    }
    // Objects put down by hand (a world without edits): barrels turned and crates not, on the
    // first battlemap; the chunk packed.
    if ex.world.file.edits.is_empty()
        && let Some(k) = keys.iter().find(|k| k.level == max_level)
    {
        let size = ex.world.geom.tile_size_ft(max_level);
        let (ox, oy) = (k.x as f64 * size, k.y as f64 * size);
        for i in 0..4 {
            let (kind, turned) = if i % 2 == 0 { (crate::battlemap::Kind::Barrel, true) } else { (crate::battlemap::Kind::Crate, false) };
            let p = crate::world::Placed { kind: crate::world::ObjKind::Builtin(kind as u16), x: ox + 100.0 + i as f64 * 37.5, y: oy + 220.0, rot: 0.7 * i as f32, scale: 1.0, variant: i as u8, turned };
            ex.world.file.edits.objects.insert(format!("o:det{i}"), p);
        }
        let chunk = ex.battlemap(*k);
        let bytes = crate::battlemap::pack(&ex.world, &chunk);
        writeln!(out, "placed/{}/{}/{} {:016x}", k.level, k.x, k.y, fnv64(&bytes)).unwrap();
        ex.world.file.edits.objects.clear();
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
    // Sites created by hand (not buildings or works): their layouts and the first site underground
    // beneath each.
    for (k, c) in ex.world.file.edits.created.iter().enumerate().filter(|(_, c)| !matches!(c.kind.as_str(), "building" | "castle" | "wall") && !c.removed) {
        let li = first + k;
        let l = crate::town::layout(&ex.world, &ex.t0, li);
        let mut h = crate::core::hash::Fnv64::default();
        h.write(format!("{:?} {:?}", l.plazas, l.entrances.iter().map(|e| (e.at, e.dir, e.kind as u8, e.id)).collect::<Vec<_>>()).as_bytes());
        for b in &l.buildings {
            h.write(format!("{} {:?} {:?} {} {:?} {}", b.id, b.poly, b.func, b.floors, b.name, b.pad_ft).as_bytes());
        }
        let under = l.entrances.first().and_then(|e| crate::interior::generate_id(&ex.world, &ex.t0, &format!("u:{li}:{}", e.id))).map(|it| serde_json::to_string(&it).unwrap_or_default()).unwrap_or_default();
        writeln!(out, "created/{} {li} {:016x} {:016x}", c.id, h.finish(), fnv64(under.as_bytes())).unwrap();
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
    // The ward editor on the largest settlement (a world without edits): a corner moved, a ward
    // made military and another rolled again; the change's report, the layout, the first new
    // building's interior and the battlemap at the corner.
    if ex.world.file.edits.is_empty()
        && let Some(si) = (0..ex.t0.settlements.len()).max_by_key(|&i| ex.t0.settlements[i].population)
        && let Ok(plan) = crate::town::wards::plan_json(&ex.world, &ex.t0, si)
    {
        let corner = plan["corners"].as_array().into_iter().flatten().find(|c| c["pinned"].is_null() && c["gate"].is_null() && c["wall"].is_null()).cloned();
        let wards: Vec<u64> = plan["patches"].as_array().into_iter().flatten().filter(|p| p["in_town"] == true && p["ward"] == "common").filter_map(|p| p["patch"].as_u64()).take(2).collect();
        if let (Some(c), [a, b]) = (corner, &wards[..]) {
            let req = format!(r#"{{"moves":[{{"corner":{},"by":[24,16]}}],"patches":[{{"patch":{a},"ward":"military"}},{{"patch":{b},"reroll":true}}]}}"#, c["corner"]);
            let change = crate::town::wards::change_json(&ex.world, &ex.t0, si, &req);
            let v: serde_json::Value = serde_json::from_str(&change).unwrap_or_default();
            if let Ok(e) = serde_json::from_value::<crate::world::TownEdit>(v["edit"].clone()) {
                ex.world.file.edits.towns.insert(crate::town::wards::key(si), e);
                let l = crate::town::layout(&ex.world, &ex.t0, si);
                let mut h = crate::core::hash::Fnv64::default();
                h.write(format!("{:?} {:?} {:?} {:?} {:?} {:?}", l.walls, l.towers, l.gate_towers, l.extra_towers, l.roads, l.stairs).as_bytes());
                for b in &l.buildings {
                    h.write(format!("{} {:?} {:?} {} {:?} {}", b.id, b.poly, b.func, b.floors, b.name, b.pad_ft).as_bytes());
                }
                let fresh = l.buildings.iter().find(|b| b.id >= crate::town::wards::NEW_ID && b.structure == crate::town::Structure::Roofed).map(|b| format!("b:{si}:{}", b.id)).unwrap_or_default();
                let json = crate::interior::generate_id(&ex.world, &ex.t0, &fresh).map(|it| serde_json::to_string(&it).unwrap_or_default()).unwrap_or_default();
                let at = [c["at"][0].as_f64().unwrap_or(0.0), c["at"][1].as_f64().unwrap_or(0.0)];
                let size = ex.world.geom.tile_size_ft(max_level);
                let key = TileKey::surface(max_level, (at[0] / size) as u32, (at[1] / size) as u32);
                let chunk = ex.battlemap(key);
                let bytes = crate::battlemap::pack(&ex.world, &chunk);
                writeln!(out, "towns/{si} {fresh} {:016x} {:016x} {:016x} {:016x}", fnv64(change.as_bytes()), h.finish(), fnv64(json.as_bytes()), fnv64(&bytes)).unwrap();
                ex.world.file.edits.towns.clear();
            }
        }
    }
    // Ground shaped by hand (a world without edits): in the largest settlement, a sheer plateau
    // across the chunk corner nearest a building that straddles a chunk edge, under it, and a
    // mound brushed on the edge beside it; the four battlemaps, the tiles above the corner at the
    // three finest levels and that building's interior.
    if ex.world.file.edits.is_empty()
        && let Some(si) = (0..ex.t0.settlements.len()).max_by_key(|&i| ex.t0.settlements[i].population)
    {
        let size = ex.world.geom.tile_size_ft(max_level);
        let l = crate::town::layout(&ex.world, &ex.t0, si);
        let snap = |v: f64| crate::core::round(v / size) * size;
        let straddles = |b: &crate::town::Building| {
            let (x0, x1) = b.poly.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[0]), a.1.max(p[0])));
            let (y0, y1) = b.poly.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
            crate::core::floor(x0 / size) != crate::core::floor(x1 / size) || crate::core::floor(y0 / size) != crate::core::floor(y1 / size)
        };
        let at = |b: &crate::town::Building| crate::town::geom::centroid(&b.poly);
        let near = l.buildings.iter().filter(|b| b.structure == crate::town::Structure::Roofed && straddles(b)).min_by(|a, b| {
            let (p, q) = (at(a), at(b));
            crate::town::geom::dist(p, [snap(p[0]), snap(p[1])]).total_cmp(&crate::town::geom::dist(q, [snap(q[0]), snap(q[1])]))
        });
        if let Some(b) = near {
            let c = at(b);
            let corner = [snap(c[0]), snap(c[1])];
            let half = 80.0;
            let outline = [[-half, -half], [half, -half], [half, half], [-half, half]].map(|d| [corner[0] + d[0], corner[1] + d[1]]);
            let mut e = crate::world::Edits::default();
            if let Ok(p) = crate::ground::plateau_for(&outline, b.pad_ft as f64 + 8.0, 0.0) {
                e.plateaus.insert("h:det".into(), p);
            }
            let dab = [[corner[0], corner[1] + 2.0 * half]];
            let stroke = crate::ground::Stroke { mode: crate::ground::Brush::Raise, path: &dab, radius_ft: 30.0, height_ft: 10.0, hardness: 0.5, strength: 1.0 };
            let _ = crate::ground::stamp_brush(&mut e.ground, &stroke, &|_, _| 0.0);
            ex.world.file.edits = e;
            let (kx, ky) = ((corner[0] / size) as u32, (corner[1] / size) as u32);
            let mut h = crate::core::hash::Fnv64::default();
            for (x, y) in [(kx - 1, ky - 1), (kx, ky - 1), (kx - 1, ky), (kx, ky)] {
                let chunk = ex.battlemap(TileKey::surface(max_level, x, y));
                h.write(&crate::battlemap::pack(&ex.world, &chunk));
            }
            let mut t = crate::core::hash::Fnv64::default();
            for level in max_level - 2..=max_level {
                let s = ex.world.geom.tile_size_ft(level);
                t.write(&ex.terrain_packed(TileKey::surface(level, (corner[0] / s) as u32, (corner[1] / s) as u32)));
            }
            let id = format!("b:{si}:{}", b.id);
            let json = crate::interior::generate_id(&ex.world, &ex.t0, &id).map(|it| serde_json::to_string(&it).unwrap_or_default()).unwrap_or_default();
            writeln!(out, "ground/{id} {:016x} {:016x} {:016x}", h.finish(), t.finish(), fnv64(json.as_bytes())).unwrap();
            ex.world.file.edits = crate::world::Edits::default();
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
        // Its props given a side and a look by hand, through the text plan.
        pose(&mut d);
        let text = design::to_text(&d, &|_, _| None).unwrap_or_default();
        let json = design::from_text(&text, &d).map(|(p, _)| design::design_json(&ex.world, &ex.t0, &id, Some(&serde_json::to_string(&p).unwrap_or_default()), None)).unwrap_or_else(|e| e);
        writeln!(out, "posed/{id} {:016x} {:016x}", fnv64(text.as_bytes()), fnv64(json.as_bytes())).unwrap();
    }
    // A building's interior designed: its first room of every level furnished again, the plan
    // as text, then built again a storey taller, with its problems.
    let house = crate::town::layout(&ex.world, &ex.t0, 0).buildings.iter().find(|b| b.structure == crate::town::Structure::Roofed && b.floors >= 2).map(|b| (format!("b:0:{}", b.id), b.floors));
    if let Some((id, floors)) = house
        && let Ok(mut d) = design::design_of(&ex.world, &ex.t0, &id, true)
    {
        for li in 0..d.levels.len() {
            d.furnish(li, 0, 7);
        }
        let text = design::to_text(&d, &|_, _| None).unwrap_or_default();
        let json = design::design_json(&ex.world, &ex.t0, &id, Some(&serde_json::to_string(&d).unwrap_or_default()), Some(&format!(r#"{{"refit":{}}}"#, floors + 1)));
        writeln!(out, "design/{id} {:016x} {:016x}", fnv64(text.as_bytes()), fnv64(json.as_bytes())).unwrap();
        pose(&mut d);
        let text = design::to_text(&d, &|_, _| None).unwrap_or_default();
        let json = design::design_json(&ex.world, &ex.t0, &id, Some(&serde_json::to_string(&d).unwrap_or_default()), None);
        writeln!(out, "posed/{id} {:016x} {:016x}", fnv64(text.as_bytes()), fnv64(json.as_bytes())).unwrap();
    }
    Ok(out)
}

/// Every third prop of a design given a side to stand against and every other a look, as by
/// hand (the det check's designed sites and buildings).
fn pose(d: &mut crate::under::design::SiteDesign) {
    for (k, f) in d.levels.iter_mut().flat_map(|l| l.items.iter_mut()).enumerate() {
        if k % 3 == 0 {
            f.back = Some((k / 3 % 4) as u8);
        }
        if k % 2 == 0 {
            f.variant = Some((k / 2 % 4) as u8);
        }
    }
}
