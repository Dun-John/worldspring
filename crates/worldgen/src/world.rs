//! The world file (the only source of truth) and the resolved `World` context.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::{
    hash::fnv64,
    rng,
    tile::{FT_PER_MILE, WorldGeom},
};

/// Bump whenever generator output changes for an unchanged world file.
pub const GEN_VERSION: u32 = 52;

/// Prevailing winds: latitude belts (trades, westerlies, polar easterlies) or one direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wind {
    Belts,
    FromWest,
    FromEast,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldParams {
    pub width_mi: f64,
    pub height_mi: f64,
    pub sea_level_ft: f64,
    pub max_elev_ft: f64,
    /// Fraction of the map that is land, 0.05..0.95 (matched exactly by thresholding).
    pub land_fraction: f64,
    /// Scales tectonic uplift and fine-detail amplitude, 0..3.
    pub ruggedness: f64,
    /// Tectonic plates, 4..40. More plates → more, shorter ranges.
    pub plate_count: u32,
    /// Erosion strength, 0..2 (0 = raw uplift, 2 = old, worn-down land).
    pub erosion: f64,
    /// Latitude of the map's top and bottom edges (degrees, south negative).
    pub lat_top: f64,
    pub lat_bottom: f64,
    pub wind: Wind,
    /// Added to every temperature (°C).
    pub temp_offset_c: f64,
    /// Multiplies precipitation, 0.2..3.
    pub moisture: f64,
    /// Number of volcanoes, 0..12.
    pub volcanoes: u32,
    /// Multiplies how many streams become mapped rivers, 0.25..4.
    pub river_density: f64,
    /// Multiplies how many settlements are placed, 0..3.
    pub settlement_density: f64,
    /// Multiplies how many ruins, towers and other points of interest are placed, 0..3.
    pub poi_density: f64,
    /// Per-biome weight by name (see `t0::biome::Biome::name`); missing = 1, 0 disables.
    pub biome_weights: BTreeMap<String, f64>,
}

impl Default for WorldParams {
    fn default() -> Self {
        Self {
            width_mi: 1200.0,
            height_mi: 900.0,
            sea_level_ft: 0.0,
            max_elev_ft: 14_000.0,
            land_fraction: 0.45,
            ruggedness: 1.0,
            plate_count: 14,
            erosion: 1.0,
            lat_top: 62.0,
            lat_bottom: 8.0,
            wind: Wind::Belts,
            temp_offset_c: 0.0,
            moisture: 1.0,
            volcanoes: 2,
            river_density: 1.0,
            settlement_density: 1.0,
            poi_density: 1.0,
            biome_weights: BTreeMap::new(),
        }
    }
}

impl WorldParams {
    pub fn biome_weight(&self, name: &str) -> f64 {
        self.biome_weights.get(name).copied().unwrap_or(1.0).max(0.0)
    }

    fn validate(&self) -> Result<(), String> {
        let checks: [(bool, &str); 13] = [
            ((150.0..=3000.0).contains(&self.width_mi), "width_mi must be 150–3000"),
            ((100.0..=3000.0).contains(&self.height_mi), "height_mi must be 100–3000"),
            ((0.05..=0.95).contains(&self.land_fraction), "land_fraction must be 0.05–0.95"),
            ((0.0..=3.0).contains(&self.ruggedness), "ruggedness must be 0–3"),
            ((4..=40).contains(&self.plate_count), "plate_count must be 4–40"),
            ((0.0..=2.0).contains(&self.erosion), "erosion must be 0–2"),
            ((-85.0..=85.0).contains(&self.lat_top) && (-85.0..=85.0).contains(&self.lat_bottom), "latitudes must be within ±85"),
            ((0.2..=3.0).contains(&self.moisture), "moisture must be 0.2–3"),
            (self.volcanoes <= 12, "volcanoes must be 0–12"),
            ((0.25..=4.0).contains(&self.river_density), "river_density must be 0.25–4"),
            ((1000.0..=30000.0).contains(&self.max_elev_ft), "max_elev_ft must be 1000–30000"),
            ((0.0..=3.0).contains(&self.settlement_density), "settlement_density must be 0–3"),
            ((0.0..=3.0).contains(&self.poi_density), "poi_density must be 0–3"),
        ];
        match checks.iter().find(|(ok, _)| !ok) {
            Some((_, msg)) => Err((*msg).to_string()),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldFile {
    pub gen_version: u32,
    /// u32 so it survives a round trip through JavaScript numbers.
    pub seed: u32,
    #[serde(default)]
    pub params: WorldParams,
    /// Drawn constraints the generator follows (coastlines, ranges, rivers, biomes, settlement
    /// pins). Part of the world: changing it regenerates.
    #[serde(default, skip_serializing_if = "Sketch::is_empty")]
    pub sketch: Sketch,
    /// User and agent edits layered on the generated world (never change generation, so
    /// they are not part of the world hash).
    #[serde(default, skip_serializing_if = "Edits::is_empty")]
    pub edits: Edits,
}

/// A sketch: strokes drawn over the map that steer generation (see `t0::sketch`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Sketch {
    #[serde(default)]
    pub strokes: Vec<Stroke>,
}

/// What a stroke does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SketchTool {
    /// Land: a closed outline (filled) or a brushed band. Once any land is drawn, the map is
    /// sea wherever none is drawn, so outlines are the coastline.
    Land,
    /// Sea: bays, straits, inland seas, cut out of the land.
    Sea,
    /// A mountain range along the line (strength: hills at 0.2 to high peaks at 1).
    Range,
    /// A river along the line, from its first point (the source) on; carved downhill.
    River,
    /// Paints a biome (`biome`) over the brushed band or closed outline.
    Biome,
    /// A settlement at the first point (`tier`, optional `name`).
    Pin,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stroke {
    pub tool: SketchTool,
    /// World ft, in drawing order.
    pub pts: Vec<[f64; 2]>,
    /// Land, sea and biome: the outline is filled rather than brushed along.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
    /// Brush radius (ft): band half-width, a range's half-width, a river's valley.
    #[serde(default = "default_radius")]
    pub radius_ft: f64,
    /// 0..1: a range's height, a river's size.
    #[serde(default = "default_strength")]
    pub strength: f64,
    /// Hard edges (exactly as drawn) rather than natural ones.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hard: bool,
    /// Biome strokes: the biome's name (`t0::biome::Biome::name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biome: Option<String>,
    /// Pins: `metropolis`, `city`, `town` or `village`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// Pins: the settlement's name (else generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn default_radius() -> f64 {
    5.0 * FT_PER_MILE
}

fn default_strength() -> f64 {
    0.7
}

pub const PIN_TIERS: [&str; 4] = ["metropolis", "city", "town", "village"];

impl Sketch {
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    fn validate(&self) -> Result<(), String> {
        if self.strokes.len() > 1000 {
            return Err("a sketch has at most 1000 strokes".into());
        }
        for s in &self.strokes {
            if s.pts.is_empty() || s.pts.len() > 5000 || s.pts.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
                return Err("a stroke needs 1–5000 finite points".into());
            }
            if !(s.radius_ft.is_finite() && s.radius_ft > 0.0) || !(0.0..=1.0).contains(&s.strength) {
                return Err("stroke radius must be positive and strength 0–1".into());
            }
            match s.tool {
                SketchTool::Biome if !s.biome.as_deref().is_some_and(|b| crate::t0::biome::ALL.iter().any(|x| x.name() == b)) => {
                    return Err("a biome stroke needs a biome name".into());
                }
                SketchTool::Pin if !s.tier.as_deref().is_some_and(|t| PIN_TIERS.contains(&t)) => return Err("a pin needs a tier: metropolis, city, town or village".into()),
                _ => {}
            }
        }
        Ok(())
    }
}

/// Edit log applied over generated content (by the user in the app, or by agents through
/// `mapd`). Renames, notes and hiding never change generation; created features add sites
/// (appended after the generated ones, so every generated id stays put); objects put down and
/// cleared change the battlemap chunks holding them (`battlemap::apply_edits`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Edits {
    /// Feature or building id → new name.
    #[serde(default)]
    pub renames: BTreeMap<String, String>,
    /// Feature or building id → notes (lore, hooks, DM notes).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub notes: BTreeMap<String, Note>,
    /// Ids hidden from labels, search and agents' listings (still generated).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub hidden: BTreeSet<String>,
    /// Sites added to the world, in order (never removed from the list: `removed` instead, so
    /// later ones keep their layout index).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<Created>,
    /// Non-player characters (`n:<id>`), written by the user or agents (never generated).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npcs: BTreeMap<String, Npc>,
    /// Plot points (`p:<id>`), tied to places and NPCs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plots: BTreeMap<String, Plot>,
    /// Objects put on the battlemap by hand (`o:<id>`): built-in kinds or uploaded sprites.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub objects: BTreeMap<String, Placed>,
    /// Generated objects taken away (`x:<id>`): one by kind and place, or all in a circle.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cleared: BTreeMap<String, Clear>,
    /// Uploaded sprites (by asset id) and their tactical rules.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sprites: BTreeMap<String, SpriteMeta>,
    /// Underground sites designed by hand (`u:<layout>:<k>` → the whole site), built instead of
    /// generated (`under::design`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub designs: BTreeMap<String, crate::under::design::SiteDesign>,
}

/// A battlemap object put down by hand, at a world position (ft).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Placed {
    pub kind: ObjKind,
    pub x: f64,
    pub y: f64,
    /// Radians (sprites that are drawn turned, and uploaded sprites).
    #[serde(default)]
    pub rot: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub variant: u8,
}

fn one() -> f32 {
    1.0
}

/// A built-in object kind (`battlemap::CATALOG` id) or an uploaded sprite (`s:<asset id>`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ObjKind {
    Builtin(u16),
    Sprite(String),
}

impl ObjKind {
    /// The asset id of an uploaded sprite.
    pub fn sprite(&self) -> Option<&str> {
        match self {
            ObjKind::Sprite(s) => s.strip_prefix("s:"),
            ObjKind::Builtin(_) => None,
        }
    }
}

/// Generated battlemap objects taken away: with `kind`, the one of that kind within a quarter
/// square of (x, y); else every one whose centre lies within `r` ft of it (of `kinds`, if
/// given). Objects put down by hand are removed by deleting their entry instead.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Clear {
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<u16>,
}

/// An uploaded sprite: its name and 5e tactical rules (as `battlemap::KindInfo`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SpriteMeta {
    pub name: String,
    /// Squares across at scale 1.
    pub size: f32,
    /// 0 none, 1 half, 2 three-quarters, 3 full.
    pub cover: u8,
    pub blocks_move: bool,
    pub blocks_sight: bool,
    pub difficult: bool,
    pub height_ft: f32,
}

impl Default for SpriteMeta {
    fn default() -> Self {
        Self { name: String::new(), size: 1.0, cover: 0, blocks_move: false, blocks_sight: false, difficult: false, height_ft: 3.0 }
    }
}

/// Notes on a feature.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// A site added to the world: a ruin (with its dungeon or crypt), tower, camp, roadside inn,
/// cave, mine, lava tube or a bare way underground, generated like the generated ones.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Created {
    /// `c:<n>`.
    pub id: String,
    /// `ruin`, `tower`, `camp`, `waystation`, `cave`, `mine`, `lava_tube` or `entrance`.
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub name: String,
    /// What lies beneath a ruin (`dungeon`, `crypt` or `catacombs`) or an entrance (those, or
    /// `cave`, `mine`, `lava_tube`); otherwise as generated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<String>,
    /// Its site underground: `small`, `medium`, `large` or `huge` (else as generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// Levels underground, 1 to `under::MAX_LEVELS` (else as generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<u8>,
    /// Theme underground (an `under::THEMES` key for its kind, else as generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// A building's footprint (world ft): a simple polygon, its corners within
    /// `BUILDING_REACH_FT` of (x, y).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poly: Vec<[f64; 2]>,
    /// A building's storeys above ground, 1 to `MAX_FLOORS` (else 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors: Option<u8>,
    /// What a building is: a `town::catalog` function key (`inn`, `blacksmith`…) or a home
    /// (`BUILDING_HOMES`); else a house.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func: Option<String>,
    /// A building's roof (`ROOFS`): else as its kind has it (battlements on keeps and towers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<String>,
    /// A building's roof colour (`TINTS`), else picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<String>,
    /// `roofed` (default) or `ruin` (broken walls, no roof).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<String>,
    /// Deleted (kept in the list so later sites keep their place).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
}

/// A non-player character. Every field is optional on the way in, so a partial entry from an
/// agent or an older app still reads.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Npc {
    pub name: String,
    pub appearance: String,
    pub mannerisms: String,
    pub attitude: Attitude,
    pub goals: String,
    pub notes: String,
    pub tags: Vec<String>,
    /// A picture in the asset store (its id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portrait: Option<String>,
    /// Where they are found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<NpcPlace>,
    /// Free text: alive, dead, missing, imprisoned...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// How an NPC feels about the players.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Attitude {
    pub stance: Stance,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Hostile,
    Unfriendly,
    #[default]
    Neutral,
    Friendly,
    Allied,
}

pub const STANCES: [&str; 5] = ["hostile", "unfriendly", "neutral", "friendly", "allied"];

/// Where an NPC is: a feature, building, district or site id; inside, a level and a point (ft).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NpcPlace {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

/// A plot point: what is going on, where (anchors: any ids) and who is involved (NPC ids).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Plot {
    pub title: String,
    pub text: String,
    pub status: PlotStatus,
    pub anchors: Vec<String>,
    pub npcs: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotStatus {
    #[default]
    Idea,
    Active,
    Resolved,
}

pub const PLOT_STATUSES: [&str; 3] = ["idea", "active", "resolved"];

/// The kinds of site that can be created.
pub const CREATABLE: [&str; 9] = ["ruin", "tower", "camp", "waystation", "cave", "mine", "lava_tube", "entrance", "building"];

/// Homes a building can be (`func`), as `town::catalog::RESIDENTIAL`: hovel, house, townhouse,
/// tenement, noble estate, farmhouse, wizard's tower.
pub const BUILDING_HOMES: [&str; 7] = ["hovel", "house", "townhouse", "tenement", "noble_estate", "farmhouse", "wizard_tower"];
/// A building's roof: pitched with hips, a walkable roof behind battlements, or a cone (a
/// pyramid on square plans).
pub const ROOFS: [&str; 3] = ["hip", "battlements", "cone"];
/// Roof colours (as the battlemap's `ROOF_TINTS`).
pub const TINTS: [&str; 5] = ["terracotta", "slate", "thatch", "shingle", "moss"];
pub const STRUCTURES: [&str; 2] = ["roofed", "ruin"];
pub const MAX_FLOORS: u8 = 8;
/// How far a building's corners may lie from its point (ft): its layout reaches
/// `town::sites::SITE_REACH_FT`.
pub const BUILDING_REACH_FT: f64 = 200.0;

impl Created {
    /// The kind of site underground (none for towers, camps and inns).
    pub fn under_kind(&self) -> Option<crate::under::UnderKind> {
        use crate::under::UnderKind;
        match self.kind.as_str() {
            "ruin" => Some(self.under.as_deref().and_then(UnderKind::parse).unwrap_or(UnderKind::Dungeon)),
            "entrance" => Some(self.under.as_deref().and_then(UnderKind::parse).unwrap_or(UnderKind::Dungeon)),
            "cave" => Some(UnderKind::Cave),
            "mine" => Some(UnderKind::Mine),
            "lava_tube" => Some(UnderKind::LavaTube),
            _ => None,
        }
    }

    /// Whether its options make sense: a kind that can be created, what lies beneath only for
    /// ruins (built sites) and entrances, size, levels and a theme of its site's kind only
    /// where there is a site underground.
    pub fn check(&self) -> Result<(), String> {
        use crate::under::{MAX_LEVELS, SiteSize, THEMES, UnderKind};
        if !CREATABLE.contains(&self.kind.as_str()) {
            return Err(format!("kind must be one of {}", CREATABLE.join(", ")));
        }
        if self.kind == "building" {
            return self.check_building();
        }
        if !self.poly.is_empty() || self.floors.is_some() || self.func.is_some() || self.roof.is_some() || self.tint.is_some() || self.structure.is_some() {
            return Err("footprint, floors, function, roof, tint and structure are for buildings".into());
        }
        if let Some(u) = &self.under {
            let built = [UnderKind::Dungeon, UnderKind::Crypt, UnderKind::Catacombs];
            match (self.kind.as_str(), UnderKind::parse(u)) {
                ("ruin", Some(k)) if built.contains(&k) => {}
                ("entrance", Some(_)) => {}
                ("ruin", _) => return Err("under a ruin: dungeon, crypt or catacombs".into()),
                ("entrance", _) => return Err(format!("under an entrance: {}", UnderKind::CREATABLE.map(|k| k.key()).join(", "))),
                _ => return Err("'under' is for ruins and entrances".into()),
            }
        }
        let site = self.under_kind();
        if site.is_none() && (self.size.is_some() || self.levels.is_some() || self.theme.is_some()) {
            return Err(format!("a {} has nothing underground: no size, levels or theme", self.kind));
        }
        if let Some(s) = &self.size
            && SiteSize::parse(s).is_none()
        {
            return Err(format!("size must be one of {}", SiteSize::NAMES.join(", ")));
        }
        if self.levels.is_some_and(|l| l == 0 || l > MAX_LEVELS) {
            return Err(format!("levels: 1 to {MAX_LEVELS}"));
        }
        if let (Some(t), Some(k)) = (&self.theme, site) {
            let own: Vec<&str> = THEMES.iter().filter(|th| th.kind == k).map(|th| th.key).collect();
            if !own.contains(&t.as_str()) {
                return Err(format!("themes for a {}: {}", k.name(), own.join(", ")));
            }
        }
        Ok(())
    }

    /// A building's options: a simple footprint of 3–64 corners near its point, at least
    /// 10 ft across; storeys, function, roof, tint and structure from their lists.
    fn check_building(&self) -> Result<(), String> {
        if self.under.is_some() || self.size.is_some() || self.levels.is_some() || self.theme.is_some() {
            return Err("a building has nothing underground: no under, size, levels or theme".into());
        }
        let p = &self.poly;
        if p.len() < 3 || p.len() > 64 || p.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) {
            return Err("a building's footprint needs 3–64 corners".into());
        }
        if p.iter().any(|q| (q[0] - self.x).hypot(q[1] - self.y) > BUILDING_REACH_FT) {
            return Err(format!("a building's corners must lie within {BUILDING_REACH_FT} ft of its point"));
        }
        if crate::town::geom::area(p).abs() < 100.0 || crate::town::geom::obb(p).short < 10.0 {
            return Err("a building must be at least 10 ft across".into());
        }
        if !simple(p) {
            return Err("a building's footprint must not cross itself".into());
        }
        if self.floors.is_some_and(|f| f == 0 || f > MAX_FLOORS) {
            return Err(format!("floors: 1 to {MAX_FLOORS}"));
        }
        if let Some(f) = &self.func
            && crate::town::catalog::index_of(f).is_none()
            && !BUILDING_HOMES.contains(&f.as_str())
        {
            return Err(format!("func: a business (list_names shows them; e.g. inn, tavern, blacksmith, temple, castle) or a home: {}", BUILDING_HOMES.join(", ")));
        }
        for (v, list, what) in [(&self.roof, &ROOFS[..], "roof"), (&self.tint, &TINTS[..], "tint"), (&self.structure, &STRUCTURES[..], "structure")] {
            if let Some(v) = v
                && !list.contains(&v.as_str())
            {
                return Err(format!("{what} must be one of {}", list.join(", ")));
            }
        }
        Ok(())
    }
}

/// Whether a polygon's sides cross only at shared corners.
fn simple(p: &[[f64; 2]]) -> bool {
    use crate::town::geom::{cross, sub};
    let n = p.len();
    let crosses = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (d1, d2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
        let (d3, d4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    };
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if crosses(p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

impl Edits {
    /// Replace the fields given (a JSON object of some of the edits' fields, each whole), the
    /// rest staying: a big world's live edits arrive field by field. Returns whether `created`
    /// was among them (only then is generation touched).
    pub fn set_fields(&mut self, json: &str) -> Result<bool, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let keys: Vec<String> = v.as_object().ok_or("edits fields: a JSON object")?.keys().cloned().collect();
        let mut part: Edits = serde_json::from_value(v).map_err(|e| e.to_string())?;
        for k in &keys {
            match k.as_str() {
                "renames" => self.renames = std::mem::take(&mut part.renames),
                "notes" => self.notes = std::mem::take(&mut part.notes),
                "hidden" => self.hidden = std::mem::take(&mut part.hidden),
                "created" => self.created = std::mem::take(&mut part.created),
                "npcs" => self.npcs = std::mem::take(&mut part.npcs),
                "plots" => self.plots = std::mem::take(&mut part.plots),
                "objects" => self.objects = std::mem::take(&mut part.objects),
                "cleared" => self.cleared = std::mem::take(&mut part.cleared),
                "sprites" => self.sprites = std::mem::take(&mut part.sprites),
                "designs" => self.designs = std::mem::take(&mut part.designs),
                _ => return Err(format!("no such edits field: {k}")),
            }
        }
        Ok(keys.iter().any(|k| k == "created"))
    }

    /// Change some entries of keyed fields: `{field: {set: {key: value…}, unset: [key…]}}`
    /// (a brush stroke in a world of thousands of objects sends only its own).
    pub fn patch_fields(&mut self, json: &str) -> Result<(), String> {
        fn patch<T: serde::de::DeserializeOwned>(m: &mut BTreeMap<String, T>, p: &Value) -> Result<(), String> {
            for (k, v) in p["set"].as_object().into_iter().flatten() {
                m.insert(k.clone(), serde_json::from_value(v.clone()).map_err(|e| format!("{k}: {e}"))?);
            }
            for k in p["unset"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                m.remove(k);
            }
            Ok(())
        }
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        for (field, p) in v.as_object().ok_or("edits patch: a JSON object")? {
            match field.as_str() {
                "renames" => patch(&mut self.renames, p)?,
                "notes" => patch(&mut self.notes, p)?,
                "npcs" => patch(&mut self.npcs, p)?,
                "plots" => patch(&mut self.plots, p)?,
                "objects" => patch(&mut self.objects, p)?,
                "cleared" => patch(&mut self.cleared, p)?,
                "sprites" => patch(&mut self.sprites, p)?,
                "designs" => patch(&mut self.designs, p)?,
                _ => return Err(format!("{field}: not a keyed edits field")),
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.renames.is_empty() && self.notes.is_empty() && self.hidden.is_empty() && self.created.is_empty() && self.npcs.is_empty() && self.plots.is_empty()
            && self.objects.is_empty() && self.cleared.is_empty() && self.sprites.is_empty() && self.designs.is_empty()
    }

    /// The NPCs found at `id` (placed there).
    pub fn npcs_at<'a>(&'a self, id: &'a str) -> impl Iterator<Item = (&'a String, &'a Npc)> + 'a {
        self.npcs.iter().filter(move |(_, n)| n.location.as_ref().is_some_and(|l| l.id == id))
    }

    /// The plots anchored at `id`.
    pub fn plots_at<'a>(&'a self, id: &'a str) -> impl Iterator<Item = (&'a String, &'a Plot)> + 'a {
        self.plots.iter().filter(move |(_, p)| p.anchors.iter().any(|a| a == id))
    }

    /// The ops that turn these edits into `to`.
    pub fn diff(&self, to: &Edits) -> Vec<EditOp> {
        let (a, b) = (serde_json::to_value(self).unwrap_or_default(), serde_json::to_value(to).unwrap_or_default());
        let mut ops = Vec::new();
        for &(field, shape) in EDIT_FIELDS {
            let (ea, eb) = (entries(&a, field, shape), entries(&b, field, shape));
            let keys: BTreeSet<&String> = ea.iter().chain(&eb).map(|(k, _)| k).collect();
            for key in keys {
                let (va, vb) = (ea.iter().find(|(k, _)| k == key).map(|e| &e.1), eb.iter().find(|(k, _)| k == key).map(|e| &e.1));
                if va == vb {
                    continue;
                }
                ops.push(match vb {
                    Some(v) => EditOp::Set { field: field.into(), key: key.clone(), value: v.clone() },
                    None => EditOp::Unset { field: field.into(), key: key.clone() },
                });
            }
        }
        ops
    }

    /// Apply one op; returns the op that undoes it. Nothing changes on an error.
    pub fn apply(&mut self, op: &EditOp) -> Result<EditOp, String> {
        let (field, key) = match op {
            EditOp::Set { field, key, .. } | EditOp::Unset { field, key } => (field.as_str(), key.clone()),
        };
        let &(_, shape) = EDIT_FIELDS.iter().find(|(f, _)| *f == field).ok_or_else(|| format!("no such edits field: {field}"))?;
        let mut v = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        let mut list = entries(&v, field, shape);
        let at = list.iter().position(|(k, _)| *k == key);
        let before = at.map(|i| list[i].1.clone());
        match (op, shape) {
            (EditOp::Set { value, .. }, Shape::List) => {
                let i = list_index(&key).ok_or_else(|| format!("{field} keys are <prefix>:<index>, not {key}"))?;
                if i > list.len() {
                    return Err(format!("{key}: {field} has only {} entries", list.len()));
                }
                if i == list.len() {
                    list.push((key.clone(), value.clone()));
                } else {
                    list[i].1 = value.clone();
                }
            }
            (EditOp::Set { value, .. }, _) => match at {
                Some(i) => list[i].1 = value.clone(),
                None => list.push((key.clone(), value.clone())),
            },
            // Listed entries keep their place (later ids are indices): unset marks one removed.
            (EditOp::Unset { .. }, Shape::List) => {
                if let Some(i) = at
                    && let Some(m) = list[i].1.as_object_mut()
                {
                    m.insert("removed".into(), Value::Bool(true));
                }
            }
            (EditOp::Unset { .. }, _) => list.retain(|(k, _)| *k != key),
        }
        put(&mut v, field, shape, list);
        *self = serde_json::from_value(v).map_err(|e| format!("{field} {key}: {e}"))?;
        let field = field.to_string();
        Ok(match before {
            Some(value) => EditOp::Set { field, key, value },
            None => EditOp::Unset { field, key },
        })
    }
}

/// One change to the edits: one entry of one field set or removed. The app and `mapd` exchange
/// these (not whole edits), so changes made at the same time to different entries all stay.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {
    Set { field: String, key: String, value: Value },
    Unset { field: String, key: String },
}

#[derive(Clone, Copy)]
enum Shape {
    /// An object keyed by id.
    Map,
    /// A sorted list of ids (an entry's value is `true`).
    Set,
    /// A list whose entries' ids are `<prefix>:<index>` (never shortened).
    List,
}

/// The edit fields and how their entries are keyed (mirrored in `app/src/sync/ops.ts`).
const EDIT_FIELDS: &[(&str, Shape)] = &[
    ("renames", Shape::Map),
    ("notes", Shape::Map),
    ("hidden", Shape::Set),
    ("created", Shape::List),
    ("npcs", Shape::Map),
    ("plots", Shape::Map),
    ("objects", Shape::Map),
    ("cleared", Shape::Map),
    ("sprites", Shape::Map),
    ("designs", Shape::Map),
];

fn list_index(key: &str) -> Option<usize> {
    key.rsplit(':').next()?.parse().ok()
}

/// A field's entries as (key, value), in order.
fn entries(v: &Value, field: &str, shape: Shape) -> Vec<(String, Value)> {
    match (shape, &v[field]) {
        (Shape::Map, Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        (Shape::Set, Value::Array(a)) => a.iter().filter_map(|k| Some((k.as_str()?.to_string(), Value::Bool(true)))).collect(),
        (Shape::List, Value::Array(a)) => a.iter().enumerate().map(|(i, e)| (e["id"].as_str().map(str::to_string).unwrap_or_else(|| format!("c:{i}")), e.clone())).collect(),
        _ => Vec::new(),
    }
}

fn put(v: &mut Value, field: &str, shape: Shape, list: Vec<(String, Value)>) {
    v[field] = match shape {
        Shape::Map => Value::Object(list.into_iter().collect()),
        Shape::Set => Value::Array(list.into_iter().map(|(k, _)| Value::String(k)).collect()),
        Shape::List => Value::Array(list.into_iter().map(|(_, e)| e).collect()),
    };
}

impl Default for WorldFile {
    fn default() -> Self {
        Self { gen_version: GEN_VERSION, seed: 1, params: WorldParams::default(), sketch: Sketch::default(), edits: Edits::default() }
    }
}

pub struct World {
    pub file: WorldFile,
    pub geom: WorldGeom,
    /// Hash of the canonical world file; keys every cache.
    pub hash: u64,
    pub seed: u64,
}

impl World {
    pub fn new(file: WorldFile) -> Result<Self, String> {
        if file.gen_version != GEN_VERSION {
            return Err(format!(
                "world file is for generator v{}, this is v{GEN_VERSION}",
                file.gen_version
            ));
        }
        let p = &file.params;
        p.validate()?;
        file.sketch.validate()?;
        let geom = WorldGeom::new(p.width_mi * FT_PER_MILE, p.height_mi * FT_PER_MILE);
        // Edits are layered on top of generation; the hash (which keys every cache) ignores them.
        let canonical = serde_json::to_string(&WorldFile { edits: Edits::default(), ..file.clone() }).map_err(|e| e.to_string())?;
        Ok(Self {
            hash: fnv64(canonical.as_bytes()),
            seed: rng::mix64(file.seed as u64 ^ 0x5eed_5eed_5eed_5eed),
            geom,
            file,
        })
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let file: WorldFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Self::new(file)
    }

    pub fn params(&self) -> &WorldParams {
        &self.file.params
    }

    /// Named sub-seed for one generator.
    pub fn stream(&self, name: &str) -> u64 {
        rng::stream(self.seed, name)
    }
}
