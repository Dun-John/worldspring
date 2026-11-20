//! The world file (the only source of truth) and the resolved `World` context.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::{
    hash::{FastMap, fnv64},
    rng,
    tile::{FT_PER_MILE, WorldGeom},
};

/// Bump whenever generator output changes for an unchanged world file.
pub const GEN_VERSION: u32 = 66;

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
    /// How much of the plates' own mountain building is kept, 0..1 (0: mountains only where
    /// drawn; the plains keep their hills). Left out of the file at 1, so older worlds keep
    /// their hash.
    #[serde(skip_serializing_if = "is_one")]
    pub procedural_mountains: f64,
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
    /// Off: only the roads drawn in the sketch (with short spurs to the settlements beside
    /// them); settlements are placed without regard to roads and no waystations are made.
    /// Left out of the file when on, so older worlds keep their hash.
    #[serde(skip_serializing_if = "is_true")]
    pub generated_roads: bool,
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
            procedural_mountains: 1.0,
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
            generated_roads: true,
        }
    }
}

fn is_one(v: &f64) -> bool {
    *v == 1.0
}

fn is_true(v: &bool) -> bool {
    *v
}

impl WorldParams {
    pub fn biome_weight(&self, name: &str) -> f64 {
        self.biome_weights.get(name).copied().unwrap_or(1.0).max(0.0)
    }

    fn validate(&self) -> Result<(), String> {
        let checks: [(bool, &str); 14] = [
            ((150.0..=3000.0).contains(&self.width_mi), "width_mi must be 150–3000"),
            ((100.0..=3000.0).contains(&self.height_mi), "height_mi must be 100–3000"),
            ((0.05..=0.95).contains(&self.land_fraction), "land_fraction must be 0.05–0.95"),
            ((0.0..=3.0).contains(&self.ruggedness), "ruggedness must be 0–3"),
            ((0.0..=1.0).contains(&self.procedural_mountains), "procedural_mountains must be 0–1"),
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
    /// A mountain mass over a closed outline: rising from its edge (foothills `radius_ft` out)
    /// to its core, ridged along `trend` (strength: hills at 0.2 to high peaks at 1).
    Massif,
    /// Raises or lowers the land inside a closed outline by `delta_ft` (plateaus, basins),
    /// with an edge `radius_ft` wide.
    Elevation,
    /// A lake filling a closed outline (`level_ft`, else the lowest point of its shore; `salt`;
    /// optional `name`). It drains out of its lowest shore, or into a river drawn out of it.
    Lake,
    /// A volcano at the first point (`kind`, `activity`, strength: its size; optional `name`).
    Volcano,
    /// A named place (`name`): a closed outline is a region of its own (`kind`, else the land's
    /// kind inside it); a point names what lies there (a region, range, lake, island or sea;
    /// `kind` says which, else the most local), sharing it with other names in it.
    Region,
    /// A site at the first point (`kind`: ruin, tower, camp, waystation, cave, mine, lava tube
    /// or entrance; `under` a ruin or entrance; optional `name`).
    Site,
    /// A road along the line (`kind`: kings_road, road or track, default road; optional
    /// `name`), joining the settlements at and beside it; or (`kind` none) a line no planned
    /// road crosses.
    Road,
}

/// Volcano strokes' kinds and activities.
pub const VOLCANO_KINDS: [&str; 4] = ["strato", "shield", "cinder", "caldera"];
pub const VOLCANO_ACTIVITY: [&str; 3] = ["active", "dormant", "extinct"];

/// What settlements by the shore live by: never created without a remake.
pub const SHORE_KINDS: [&str; 2] = ["port", "fishing"];
/// Pin strokes' kinds (`t0::settle::SettleKind::key`).
pub const PIN_KINDS: [&str; 10] = ["port", "river", "mining", "fortress", "market", "farming", "fishing", "lumber", "herding", "oasis"];
/// Site strokes' kinds (as created sites').
pub const SITE_KINDS: [&str; 8] = ["ruin", "tower", "camp", "waystation", "cave", "mine", "lava_tube", "entrance"];
/// Road strokes' kinds: the road's class, or `none` (a line no planned road crosses).
pub const ROAD_KINDS: [&str; 4] = ["kings_road", "road", "track", "none"];
/// What a region stroke names or makes: area features' kinds.
pub const REGION_KINDS: [&str; 19] = [
    "region", "forest", "jungle", "taiga", "desert", "swamp", "plains", "tundra", "glacier", "blight", "ashlands", "range", "lake", "river", "island",
    "continent", "bay", "sea", "ocean",
];
/// The longest name a stroke may give (characters), and the most wards a pin names.
pub const NAME_MAX: usize = 80;
pub const WARDS_MAX: usize = 24;

/// The most points a sketch may hold in all (the cost of applying it grows with them).
pub const SKETCH_POINTS: usize = 200_000;

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
    /// The name of what it makes (else generated): a settlement, range, massif, river, lake,
    /// volcano, painted region, region or site; a road's (else it has none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Massifs: the direction their ridges run (degrees, 0 = east, 90 = south), else along
    /// the outline's long axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trend: Option<f64>,
    /// Elevation strokes: how far the land is raised (negative: lowered), ft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_ft: Option<f64>,
    /// Lakes: the water's level (ft above sea level).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_ft: Option<f64>,
    /// Lakes: a salt lake (no outflow).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub salt: bool,
    /// Volcanoes: `strato`, `shield`, `cinder` or `caldera` (default strato). Pins: what the
    /// settlement lives by (`PIN_KINDS`, else from its surroundings). Sites: `SITE_KINDS`.
    /// Regions: `REGION_KINDS`. Roads: `ROAD_KINDS`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Volcanoes: `active`, `dormant` or `extinct` (default dormant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    /// Pins: the realm's capital (else the first metropolis is).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub capital: bool,
    /// Pins: names for the settlement's districts, in order (the central one first).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wards: Vec<String>,
    /// Pins: added beside the world's own settlements (Edit › Places), not in place of one of
    /// its tier: it neither counts toward the tier's number nor keeps the others of its tier away
    /// (smaller ones keep the least room, 10 mi); none is placed within 3 mi of a settlement.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub added: bool,
    /// Pins: in place of the world's own settlement at its spot (within 3 mi), changed: the tier
    /// that settlement was placed at (`PIN_TIERS`; its `Settlement::planned`). The tier is placed
    /// as it would be without the pin, then the pin's settlement takes that one's place (and its
    /// number among them), so no other moves; with none there it stands beside them (as `added`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<String>,
    /// Pins: false keeps a waterside settlement where the pin stands, not moved onto its shore or
    /// beside its river (`settle::snap_to_water`); left out, it is moved as the world's own are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_water: Option<bool>,
    /// Sites: what lies beneath a ruin (dungeon, crypt, catacombs) or an entrance (any site).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<String>,
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
        if self.strokes.iter().map(|s| s.pts.len()).sum::<usize>() > SKETCH_POINTS {
            return Err(format!("a sketch has at most {SKETCH_POINTS} points in all"));
        }
        for s in &self.strokes {
            if s.pts.is_empty() || s.pts.len() > 5000 || s.pts.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
                return Err("a stroke needs 1–5000 finite points".into());
            }
            if !(s.radius_ft.is_finite() && s.radius_ft > 0.0) || !(0.0..=1.0).contains(&s.strength) {
                return Err("stroke radius must be positive and strength 0–1".into());
            }
            match s.tool {
                SketchTool::Biome if !s.biome.as_deref().is_some_and(|b| crate::model::biome::ALL.iter().any(|x| x.name() == b)) => {
                    return Err("a biome stroke needs a biome name".into());
                }
                SketchTool::Pin if !s.tier.as_deref().is_some_and(|t| PIN_TIERS.contains(&t)) => return Err("a pin needs a tier: metropolis, city, town or village".into()),
                SketchTool::Pin if !s.replaces.as_deref().is_none_or(|t| PIN_TIERS.contains(&t)) => return Err("a pin's replaces is a tier: metropolis, city, town or village".into()),
                SketchTool::Massif | SketchTool::Elevation | SketchTool::Lake if !s.closed || s.pts.len() < 3 => {
                    return Err("massif, elevation and lake strokes are closed outlines of 3 or more points".into());
                }
                SketchTool::Elevation if !s.delta_ft.is_some_and(|d| d.is_finite() && d.abs() <= 20_000.0) => return Err("an elevation stroke needs delta_ft (±20,000 ft at most)".into()),
                SketchTool::Volcano if !s.kind.as_deref().is_none_or(|k| VOLCANO_KINDS.contains(&k)) => return Err("a volcano's kind is strato, shield, cinder or caldera".into()),
                SketchTool::Volcano if !s.activity.as_deref().is_none_or(|a| VOLCANO_ACTIVITY.contains(&a)) => return Err("a volcano's activity is active, dormant or extinct".into()),
                SketchTool::Pin if !s.kind.as_deref().is_none_or(|k| PIN_KINDS.contains(&k)) => return Err(format!("a pin's kind is one of {}", PIN_KINDS.join(", "))),
                SketchTool::Site if !s.kind.as_deref().is_some_and(|k| SITE_KINDS.contains(&k)) => return Err(format!("a site needs a kind: {}", SITE_KINDS.join(", "))),
                SketchTool::Region if !s.name.as_deref().is_some_and(|n| !n.trim().is_empty()) => return Err("a region stroke needs a name".into()),
                SketchTool::Region if !s.kind.as_deref().is_none_or(|k| REGION_KINDS.contains(&k)) => return Err(format!("a region's kind is one of {}", REGION_KINDS.join(", "))),
                SketchTool::Region if s.closed && s.pts.len() < 3 => return Err("a region's outline needs 3 or more points".into()),
                SketchTool::Road if s.closed || s.pts.len() < 2 => return Err("a road is a line of 2 or more points".into()),
                SketchTool::Road if !s.kind.as_deref().is_none_or(|k| ROAD_KINDS.contains(&k)) => return Err(format!("a road's kind is one of {}", ROAD_KINDS.join(", "))),
                _ => {}
            }
            if let Some(u) = &s.under {
                use crate::model::under::UnderKind;
                let built = [UnderKind::Dungeon, UnderKind::Crypt, UnderKind::Catacombs];
                match (s.tool, s.kind.as_deref(), UnderKind::parse(u)) {
                    (SketchTool::Site, Some("ruin"), Some(k)) if built.contains(&k) => {}
                    (SketchTool::Site, Some("entrance"), Some(_)) => {}
                    _ => return Err("'under' is for ruin sites (dungeon, crypt, catacombs) and entrances (any site)".into()),
                }
            }
            if (s.capital || s.added || !s.wards.is_empty() || s.replaces.is_some() || s.on_water.is_some()) && s.tool != SketchTool::Pin {
                return Err("capital, added, wards, replaces and on_water are for pins".into());
            }
            if s.added && s.replaces.is_some() {
                return Err("a pin is added beside the world's settlements or replaces one of them, not both".into());
            }
            if s.wards.len() > WARDS_MAX || s.name.iter().chain(&s.wards).any(|n| n.chars().count() > NAME_MAX) {
                return Err(format!("names are at most {NAME_MAX} characters, and a pin names at most {WARDS_MAX} wards"));
            }
            if !s.trend.is_none_or(f64::is_finite) || !s.level_ft.is_none_or(|l| l.is_finite() && l.abs() <= 30_000.0) {
                return Err("trend and level_ft must be finite (level_ft ±30,000 ft at most)".into());
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
    pub designs: BTreeMap<String, crate::model::under::SiteDesign>,
    /// Crossings put down by hand (`v:<id>`): a bridge, a ford or a ferry from one point to
    /// another, drawn on the battlemap over what is there.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub crossings: BTreeMap<String, Crossing>,
    /// Generated buildings changed or taken away (`b:<layout>:<id>`), applied over the layout
    /// as generated (`town::layout`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub buildings: BTreeMap<String, BuildingEdit>,
    /// Towns and cities laid out anew by hand (`<layout>` → corners moved, wards set, walls on or
    /// off), applied between planning a town and building it (`town::base_layout`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub towns: BTreeMap<String, TownEdit>,
    /// Ground brushed by hand (`g:XXXXX:YYYYY`: block indices, zero-padded, x first), applied
    /// where battlemaps and the finest terrain read heights (`ground::Shaper`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ground: BTreeMap<String, crate::model::ground::GroundBlock>,
    /// Plateaus and pits drawn by hand (`h:<id>`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plateaus: BTreeMap<String, crate::model::ground::Plateau>,
    /// Water drawn by hand (`e:<id>`): inlets, basins, lakes, ponds and moats, each an outline
    /// and a level (`waters`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub waters: BTreeMap<String, crate::waters::Water>,
    /// Quays, piers and breakwaters built by hand (`q:<id>`): made ground along or out into the
    /// water, or a deck over it (`wharf`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub wharves: BTreeMap<String, Wharf>,
    /// The world's own settlements laid out in another culture (`<layout>` → 0 north, 1 west,
    /// 2 south, 3 dry, 4 steppe), applied where a settlement is laid out (`town::culture`); a
    /// village or town added by hand keeps its own in `Created::culture`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cultures: BTreeMap<String, u8>,
    /// (Roadmap 7, 161) The world's own towns and cities planned otherwise than rolled
    /// (`<layout>` → 0-100: how planned its streets are, `town::planning`: 0-29 grown, 30-69 a
    /// planned quarter beside a grown core, 70-100 planned throughout). Towns laid out by hand
    /// and villages ignore it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub planning: BTreeMap<String, u8>,
    /// (Roadmap 7, 189) The world's own towns and cities given a street pattern (`<layout>` →
    /// `grid`, `rings`, `radial` or `linear`; absent: auto, picked from its land and culture,
    /// `town::pattern`). Its planned part follows it, as strictly as its planning says.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub patterns: BTreeMap<String, String>,
    /// Overland hex grids (one key, `overland`): drawn over the map for a hex crawl, never
    /// generated from (the app draws them; play sessions keep which hexes are revealed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hexes: BTreeMap<String, HexGrid>,
    /// Towns laid out by hand as a street graph (`<layout>` the town's header, `<layout>/n<id>` a
    /// junction, `/e<id>` a street, `/k<id>` a block, `/r<id>` a row of lots along a street, `/a<id>`
    /// a painted area, `/m<id>` a wall line; `lanes/…` the world's lanes outside any town). Each
    /// value is one record of `town::hand` (positions in world ft), kept as JSON so that every
    /// record is one small op.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub streets: BTreeMap<String, Value>,
    /// Paired exits put down by hand (`j:<id>` → `links::Link`): a square on a level of a site
    /// underground, a sewer section or a building's cellar joined to another such square; each
    /// end shows as a `link` item on its own map (`links::dress`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub links: BTreeMap<String, crate::model::links::Link>,
    /// Roads drawn by hand on the ground (`rw:<id>`): a footpath, track, gravel road, paved road or
    /// cobbled street along a line (`paths`); the world's road network and towns stay as they are.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub paths: BTreeMap<String, crate::model::paths::Path>,
    /// Towns' members set by hand (`<layout>` of a town, a made town or a `place` → `Members`):
    /// drawn things (`c:<n>`) joined to it or kept out of it, and generated buildings
    /// (`b:<layout>:<id>`) pushed out of their town into a `place`. Joins within
    /// `town::members::JOIN_FT` are worked out, never stored (`town::members`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub members: BTreeMap<String, Members>,
    /// (Roadmap 6 R5) Stretches of the world's road network removed by hand (`rd:<8 hex>`,
    /// `lod::roads::road_ids`): drawn nowhere (the ground beneath shows, their bridges and fords
    /// go), while everything laid out on them stays (traffic, routes, towns' gates). An id that
    /// no longer names a stretch (a remade world) resolves nowhere.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed_roads: BTreeSet<String>,
    /// (Roadmap 6 R6) Fences and hedges drawn by hand (`f:<n>`): drawn on the battlemap only,
    /// across roads, stopping only at water; nothing is laid out round them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fences: BTreeMap<String, crate::model::fences::Fence>,
    /// (Roadmap 6 R6) Gaps cut by the eraser (`fg:<n>`), through every fence and hedge under
    /// them, generated or drawn; kept through remakes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fence_gaps: BTreeMap<String, crate::model::fences::FenceGap>,
}

/// A town's members set by hand: ids joined to it (`add`: drawn things `c:<n>` from anywhere,
/// or generated buildings `b:<layout>:<id>` pushed out into this place) and drawn things kept out
/// of it however near (`remove`: `c:<n>` only). An id is in at most one town's `add`
/// (`town::members::change` keeps it so).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Members {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub add: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub remove: BTreeSet<String>,
}

impl Members {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

/// An overland hex grid's settings: `size_mi` across the flats (centre to neighbouring centre),
/// flat-topped or pointy, its lines' colour (`#rrggbb`), width (px) and opacity (0-1), the
/// lattice moved by `ox_mi`/`oy_mi`, and hex numbers (`CCRR`) shown or not.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HexGrid {
    pub size_mi: f64,
    pub flat: bool,
    pub color: String,
    pub width: f64,
    pub opacity: f64,
    pub ox_mi: f64,
    pub oy_mi: f64,
    pub numbers: bool,
}

impl Default for HexGrid {
    fn default() -> Self {
        HexGrid { size_mi: 6.0, flat: true, color: "#2b241d".into(), width: 1.5, opacity: 0.35, ox_mi: 0.0, oy_mi: 0.0, numbers: true }
    }
}

mod townedit;
mod crossing;
mod created;
mod edits;

pub use townedit::*;
pub use crossing::*;
pub use created::*;
pub use edits::*;

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

/// The hash of a world file (any generator's: fields added since are left out when empty, so an
/// older file hashes as its own build hashed it); edits are layered on top of generation, so the
/// hash (which keys every cache) ignores them.
pub fn file_hash(file: &WorldFile) -> Result<u64, String> {
    let canonical = serde_json::to_string(&WorldFile { edits: Edits::default(), ..file.clone() }).map_err(|e| e.to_string())?;
    Ok(fnv64(canonical.as_bytes()))
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
        Ok(Self {
            hash: file_hash(&file)?,
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
