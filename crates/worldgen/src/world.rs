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
pub const GEN_VERSION: u32 = 56;

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
                SketchTool::Biome if !s.biome.as_deref().is_some_and(|b| crate::t0::biome::ALL.iter().any(|x| x.name() == b)) => {
                    return Err("a biome stroke needs a biome name".into());
                }
                SketchTool::Pin if !s.tier.as_deref().is_some_and(|t| PIN_TIERS.contains(&t)) => return Err("a pin needs a tier: metropolis, city, town or village".into()),
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
                use crate::under::UnderKind;
                let built = [UnderKind::Dungeon, UnderKind::Crypt, UnderKind::Catacombs];
                match (s.tool, s.kind.as_deref(), UnderKind::parse(u)) {
                    (SketchTool::Site, Some("ruin"), Some(k)) if built.contains(&k) => {}
                    (SketchTool::Site, Some("entrance"), Some(_)) => {}
                    _ => return Err("'under' is for ruin sites (dungeon, crypt, catacombs) and entrances (any site)".into()),
                }
            }
            if (s.capital || !s.wards.is_empty()) && s.tool != SketchTool::Pin {
                return Err("capital and wards are for pins".into());
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
    pub designs: BTreeMap<String, crate::under::design::SiteDesign>,
    /// Crossings put down by hand (`v:<id>`): a bridge, a ford or a ferry from one point to
    /// another, drawn on the battlemap over what is there.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub crossings: BTreeMap<String, Crossing>,
    /// Generated buildings changed or taken away (`b:<layout>:<id>`), applied over the layout
    /// as generated (`town::layout`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub buildings: BTreeMap<String, BuildingEdit>,
}

/// A generated building changed by hand: removed, or given another function, storeys,
/// footprint, roof, tint or structure (each left out: as generated). `at` is the generated
/// footprint's centre: when the building at that id no longer stands there (the world was
/// generated again from a changed sketch and the town laid out anew), the edit is set aside.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BuildingEdit {
    pub at: [f64; 2],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
    /// A `town::catalog` function key or a home (`BUILDING_HOMES`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors: Option<u8>,
    /// A new footprint (world ft), as a drawn building's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poly: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<String>,
    /// `roofed` or `ruin`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<String>,
}

impl BuildingEdit {
    /// How far (ft) the building at its id may stand from `at` and still be the one edited.
    pub const SAME_AT_FT: f64 = 3.0;

    /// Whether its options make sense (as a drawn building's).
    pub fn check(&self) -> Result<(), String> {
        if !self.at[0].is_finite() || !self.at[1].is_finite() {
            return Err("at: the building's centre as generated".into());
        }
        if self.removed {
            return Ok(());
        }
        let c = crate::town::geom::centroid(&self.poly);
        let probe = Created {
            kind: "building".into(),
            x: if self.poly.is_empty() { self.at[0] } else { c[0] },
            y: if self.poly.is_empty() { self.at[1] } else { c[1] },
            poly: if self.poly.is_empty() { vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]].into_iter().map(|q| [q[0] + self.at[0] - 10.0, q[1] + self.at[1] - 10.0]).collect() } else { self.poly.clone() },
            floors: self.floors,
            func: self.func.clone(),
            roof: self.roof.clone(),
            tint: self.tint.clone(),
            structure: self.structure.clone(),
            ..Default::default()
        };
        probe.check()
    }

    /// Whether it changes nothing (every option as generated).
    pub fn is_noop(&self) -> bool {
        !self.removed && self.func.is_none() && self.floors.is_none() && self.poly.is_empty() && self.roof.is_none() && self.tint.is_none() && self.structure.is_none()
    }
}

/// A crossing put down by hand: from `a` to `b` (world ft, bank to bank), `width` ft across.
/// A bridge's deck spans the water (planks clear of it); a ford brings the bed up to wading
/// depth under stepping stones; a ferry runs a raft on a rope between two jetties.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Crossing {
    pub kind: CrossingKind,
    pub a: [f64; 2],
    pub b: [f64; 2],
    #[serde(default = "crossing_width")]
    pub width: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CrossingKind {
    Bridge,
    Ford,
    Ferry,
}

fn crossing_width() -> f64 {
    12.0
}

impl Crossing {
    /// Widths (ft) a crossing may have, and its length.
    pub const WIDTH: std::ops::RangeInclusive<f64> = 5.0..=40.0;
    pub const LENGTH: std::ops::RangeInclusive<f64> = 10.0..=2000.0;

    /// Length (ft), from a to b.
    pub fn length(&self) -> f64 {
        crate::core::sqrt((self.b[0] - self.a[0]) * (self.b[0] - self.a[0]) + (self.b[1] - self.a[1]) * (self.b[1] - self.a[1]))
    }

    /// Why it can't be put down, if so: its ends off the map or not finite, too short or long
    /// (a ferry needs room for its two jetties), too narrow or wide.
    pub fn problem(&self, map_w_ft: f64, map_h_ft: f64) -> Option<String> {
        let on = |p: [f64; 2]| p[0].is_finite() && p[1].is_finite() && (0.0..map_w_ft).contains(&p[0]) && (0.0..map_h_ft).contains(&p[1]);
        if !on(self.a) || !on(self.b) {
            return Some("both ends must be on the map".into());
        }
        let min = if self.kind == CrossingKind::Ferry { 2.0 * crate::battlemap::JETTY_FT + 20.0 } else { *Self::LENGTH.start() };
        let len = self.length();
        if len < min || len > *Self::LENGTH.end() {
            return Some(format!("a {} is {min:.0} to {:.0} ft long (this one is {len:.0})", self.kind.name(), Self::LENGTH.end()));
        }
        if !self.width.is_finite() || !Self::WIDTH.contains(&self.width) {
            return Some(format!("width: {:.0} to {:.0} ft", Self::WIDTH.start(), Self::WIDTH.end()));
        }
        None
    }
}

impl CrossingKind {
    pub fn name(self) -> &'static str {
        match self {
            CrossingKind::Bridge => "bridge",
            CrossingKind::Ford => "ford",
            CrossingKind::Ferry => "ferry",
        }
    }
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
    /// A site created again within this of one of its kind (with the same beneath) is that one.
    pub const SAME_SPOT_FT: f64 = 300.0;

    /// Whether `self`, being created (asked for at `asked`, put at its own point), is `other`, a
    /// live site already there: the same kind and beneath, within `SAME_SPOT_FT` of either point;
    /// a building, the same footprint (each corner within 2 ft), function and name (if `self`
    /// has one). Creating a site again (a script run twice, a world file opened again) then adds
    /// nothing; another building on that footprint is refused where it overlaps the first.
    pub fn same_site(&self, other: &Created, asked: [f64; 2]) -> bool {
        if other.removed || other.kind != self.kind {
            return false;
        }
        if self.kind == "building" {
            let named = self.name.trim().is_empty() || self.name.trim() == other.name.trim();
            return other.func == self.func
                && named
                && other.poly.len() == self.poly.len()
                && other.poly.iter().zip(&self.poly).all(|(a, b)| (a[0] - b[0]).abs() <= 2.0 && (a[1] - b[1]).abs() <= 2.0);
        }
        let near = |p: [f64; 2]| (other.x - p[0]).hypot(other.y - p[1]) <= Self::SAME_SPOT_FT;
        other.under_kind() == self.under_kind() && (near([self.x, self.y]) || near(asked))
    }

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
    /// `next` replacing these edits whole, keeping listed entries' places (`created`: later ids
    /// are indices): the entries `next` lacks stay, marked removed, as the ops that make it leave
    /// them wherever they are applied (the app's `keepPlaces`).
    pub fn keeping_places(&self, mut next: Edits) -> Edits {
        let n = next.created.len();
        if self.created.len() > n {
            next.created.extend(self.created[n..].iter().map(|c| Created { removed: true, ..c.clone() }));
        }
        next
    }

    /// The live site `c` (being created, asked for at `asked`) would duplicate, if any
    /// (`Created::same_site`).
    pub fn existing_site(&self, c: &Created, asked: [f64; 2]) -> Option<&Created> {
        self.created.iter().find(|o| c.same_site(o, asked))
    }

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
                "crossings" => self.crossings = std::mem::take(&mut part.crossings),
                "buildings" => self.buildings = std::mem::take(&mut part.buildings),
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
                "crossings" => patch(&mut self.crossings, p)?,
                "buildings" => patch(&mut self.buildings, p)?,
                _ => return Err(format!("{field}: not a keyed edits field")),
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.renames.is_empty() && self.notes.is_empty() && self.hidden.is_empty() && self.created.is_empty() && self.npcs.is_empty() && self.plots.is_empty()
            && self.objects.is_empty() && self.cleared.is_empty() && self.sprites.is_empty() && self.designs.is_empty() && self.crossings.is_empty()
            && self.buildings.is_empty()
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
            // (A list's entries in list order: an entry appended can only follow the one before it.)
            let keys: Vec<&String> = match shape {
                Shape::List => {
                    let mut k: Vec<&String> = eb.iter().map(|(k, _)| k).collect();
                    k.extend(ea.iter().map(|(k, _)| k).filter(|k| !eb.iter().any(|(j, _)| j == *k)));
                    k
                }
                _ => ea.iter().chain(&eb).map(|(k, _)| k).collect::<BTreeSet<_>>().into_iter().collect(),
            };
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
    ("crossings", Shape::Map),
    ("buildings", Shape::Map),
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
