//! Battlemap chunks: one finest-level tile = 128 × 128 five-foot squares.
//!
//! Everything here is the world's own detail at 5-ft scale (not a template): square heights
//! come from the L_max terrain plus deterministic micro-relief (knolls, outcrops, ditches,
//! dune ridges, hummocks), quantized into 5-ft tiers with cliff/slope edges; surfaces come
//! from biome and water; objects (vegetation, rocks, props, hazards) carry 5e tactical data.
//!
//! **Battlemap guarantee.** Rules are enforced on grid-aligned cells inside the chunk, so they
//! are local and deterministic yet hold for every window:
//! - every aligned 8 × 8 cell has ≥ 1 cover object (≥ half cover) → any 32 × 32 window holds ≥ 9;
//! - every aligned 16 × 16 cell spans ≥ 2 tiers → any 32 × 32 window contains one such cell;
//! - every aligned 16 × 16 cell has a hazard or prop cluster, or the chunk has atmosphere;
//! - no dry square is more than `MAX_COVER_GAP` squares from cover.
//! Open water far from shore is exempt (cells that are entirely deep water).

use serde::Serialize;

use crate::World;
use crate::core::noise::{fbm, smoothstep};
use crate::core::rng::{Pcg32, hash2, hash3, unit};
use crate::core::tile::{HALO, PADDED, TILE_N, TileKey};
use crate::lod::terrain_refine::TerrainOut;
use crate::t0::T0;
use crate::t0::biome::Biome;

pub const SQ: usize = TILE_N / 2;
/// Squares per edge including a one-square halo ring (for seamless rendering gradients).
pub const HS: usize = SQ + 2;
pub const SQUARE_FT: f64 = 5.0;
pub const TIER_FT: f64 = 5.0;
pub const MAX_COVER_GAP: i32 = 6;
const COVER_CELL: usize = 8;
const TIER_CELL: usize = 16;
/// Micro-relief feature grid (squares).
const RELIEF_CELL: i64 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Surface {
    Grass = 0,
    ForestFloor,
    DryGrass,
    Sand,
    Snow,
    Rock,
    Mud,
    Ash,
    Salt,
    Dirt,
    Shallow,
    Deep,
    Lava,
    Ice,
    /// King's road paving.
    Cobble,
    /// Packed-earth road or track.
    Road,
    /// Bridge deck.
    Planks,
    /// A building (its roof; interiors come with M5).
    Roof,
    /// Cropland.
    Field,
}

impl Surface {
    pub fn is_road(self) -> bool {
        matches!(self, Surface::Cobble | Surface::Road | Surface::Planks)
    }
}

/// Settlement ground under a square (`Chunk::urban`).
pub const URBAN_STREET: u8 = 1;
pub const URBAN_FIELD: u8 = 2;
pub const URBAN_PLAZA: u8 = 3;
pub const URBAN_GRAVEYARD: u8 = 4;
pub const URBAN_RUIN: u8 = 5;
/// Bridge deck or pier.
pub const URBAN_DECK: u8 = 6;
/// Town wall (a raised stone wall-walk) and towers.
pub const URBAN_WALL: u8 = 7;
pub const URBAN_TOWER: u8 = 8;
/// Ground at the mouth of a way underground (kept clear of plants and props).
pub const URBAN_ENTRANCE: u8 = 9;
/// Packed earth trodden bare (a camp's clearing).
pub const URBAN_YARD: u8 = 10;
/// Wall-walk and tower-top heights above the ground (ft).
const WALL_FT: f32 = 20.0;
const TOWER_FT: f32 = 30.0;
/// Eave height per storey (ft): a building's squares stand this much per floor above its pad.
const STOREY_FT: f32 = 10.0;

/// Bridge decks sit this far above the water they span.
pub const DECK_CLEARANCE_FT: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Atmosphere {
    None = 0,
    Mist,
    Fog,
    Fireflies,
    Snowfall,
    BlowingSand,
    Embers,
    HeatHaze,
    FallingLeaves,
    Drizzle,
}

/// Object kinds. Ids are stable (payload + renderer).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub enum Kind {
    TreeDeciduous = 1,
    TreeConifer,
    TreePalm,
    TreeJungle,
    TreeAcacia,
    TreeDead,
    TreeWillow,
    Bush,
    Thicket,
    FallenLog,
    Stump,
    Boulder,
    RockSmall,
    RockPile,
    Reeds,
    Cactus,
    TallGrass,
    Flowers,
    Mushrooms,
    Bones,
    Snowdrift,
    IceSpire,
    BasaltPillar,
    Obsidian,
    Brambles,
    Bog,
    Quicksand,
    ThinIce,
    LavaPool,
    SteamVent,
    Scree,
    Sinkhole,
    Barrel,
    Crate,
    Cart,
    MarketStall,
    Well,
    Haystack,
    Headstone,
    RuinWall,
    Statue,
    /// Camps (`town::Layout::props`).
    Tent,
    Campfire,
    Firewood,
    Bedroll,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Hazard {
    pub name: &'static str,
    pub effect: &'static str,
}

/// 5e tactical data for an object kind.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct KindInfo {
    pub id: u16,
    pub name: &'static str,
    /// Footprint radius in squares (at scale 1).
    pub radius: f32,
    pub blocks_move: bool,
    pub blocks_sight: bool,
    /// 0 none, 1 half, 2 three-quarters, 3 full.
    pub cover: u8,
    pub difficult: bool,
    pub height_ft: f32,
    pub hazard: Option<Hazard>,
    /// Counts toward the "hazard or prop" guarantee.
    pub feature: bool,
}

const fn k(id: Kind, name: &'static str, radius: f32, blocks_move: bool, blocks_sight: bool, cover: u8, difficult: bool, height_ft: f32, feature: bool) -> KindInfo {
    KindInfo { id: id as u16, name, radius, blocks_move, blocks_sight, cover, difficult, height_ft, hazard: None, feature }
}

const fn hz(id: Kind, name: &'static str, radius: f32, difficult: bool, effect: &'static str) -> KindInfo {
    KindInfo {
        id: id as u16,
        name,
        radius,
        blocks_move: false,
        blocks_sight: false,
        cover: 0,
        difficult,
        height_ft: 0.0,
        hazard: Some(Hazard { name, effect }),
        feature: true,
    }
}

pub const CATALOG: [KindInfo; 45] = [
    k(Kind::TreeDeciduous, "deciduous tree", 1.4, true, true, 2, false, 35.0, false),
    k(Kind::TreeConifer, "conifer", 1.1, true, true, 2, false, 45.0, false),
    k(Kind::TreePalm, "palm", 1.2, true, false, 1, false, 30.0, false),
    k(Kind::TreeJungle, "jungle tree", 1.8, true, true, 2, false, 70.0, false),
    k(Kind::TreeAcacia, "acacia", 1.6, true, false, 1, false, 20.0, false),
    k(Kind::TreeDead, "dead tree", 0.9, true, false, 1, false, 20.0, false),
    k(Kind::TreeWillow, "willow", 1.6, true, true, 2, false, 30.0, false),
    k(Kind::Bush, "bush", 0.6, false, false, 1, true, 4.0, false),
    k(Kind::Thicket, "thicket", 1.2, false, true, 2, true, 8.0, false),
    k(Kind::FallenLog, "fallen log", 1.5, false, false, 1, true, 3.0, false),
    k(Kind::Stump, "stump", 0.4, false, false, 1, true, 2.5, false),
    k(Kind::Boulder, "boulder", 0.9, true, true, 2, false, 6.0, false),
    k(Kind::RockSmall, "rocks", 0.3, false, false, 0, false, 1.0, true),
    k(Kind::RockPile, "rock pile", 0.8, false, false, 1, true, 3.0, false),
    k(Kind::Reeds, "reeds", 0.6, false, false, 0, true, 5.0, true),
    k(Kind::Cactus, "cactus", 0.5, true, false, 1, false, 8.0, false),
    k(Kind::TallGrass, "tall grass", 0.6, false, false, 0, false, 3.0, true),
    k(Kind::Flowers, "wildflowers", 0.5, false, false, 0, false, 1.0, true),
    k(Kind::Mushrooms, "mushroom ring", 0.5, false, false, 0, false, 0.5, true),
    k(Kind::Bones, "old bones", 0.5, false, false, 0, false, 1.0, true),
    k(Kind::Snowdrift, "snowdrift", 1.0, false, false, 1, true, 3.0, false),
    k(Kind::IceSpire, "ice spire", 0.7, true, true, 2, false, 10.0, false),
    k(Kind::BasaltPillar, "basalt pillar", 0.9, true, true, 3, false, 15.0, false),
    k(Kind::Obsidian, "obsidian outcrop", 0.8, true, false, 2, false, 6.0, false),
    hz(Kind::Brambles, "brambles", 1.3, true, "difficult terrain; 1d4 piercing per 5 ft moved (DC 11 Dex negates)"),
    hz(Kind::Bog, "bog", 1.6, true, "difficult terrain; DC 12 Str save or restrained until the end of the next turn"),
    hz(Kind::Quicksand, "quicksand", 1.2, true, "sink 1d4+1 ft; restrained; DC 10 + depth Str check to escape"),
    hz(Kind::ThinIce, "thin ice", 1.6, false, "breaks under >150 lb (DC 10 Dex); icy water: DC 10 Con save or 1 level of exhaustion"),
    hz(Kind::LavaPool, "lava", 1.2, true, "10d10 fire on entering or starting a turn in it"),
    hz(Kind::SteamVent, "steam vent", 0.5, false, "erupts on a d6 roll of 6: 2d6 fire in 10 ft (DC 13 Dex half)"),
    hz(Kind::Scree, "loose scree", 1.5, true, "difficult terrain; DC 10 Dex save when dashing or fall prone"),
    hz(Kind::Sinkhole, "sinkhole", 0.8, false, "hidden (DC 13 Wis (Perception)); 10 ft fall, 1d6 bludgeoning"),
    k(Kind::Barrel, "barrels", 0.5, false, false, 1, true, 4.0, true),
    k(Kind::Crate, "crates", 0.6, false, false, 1, true, 4.0, true),
    k(Kind::Cart, "cart", 1.3, true, false, 2, false, 5.0, true),
    k(Kind::MarketStall, "market stall", 1.2, true, false, 1, false, 8.0, true),
    k(Kind::Well, "well", 0.6, true, false, 1, false, 3.0, true),
    k(Kind::Haystack, "haystack", 0.8, false, true, 2, true, 7.0, true),
    k(Kind::Headstone, "headstone", 0.45, false, false, 1, false, 3.0, true),
    k(Kind::RuinWall, "ruined wall", 0.62, true, true, 2, false, 6.0, true),
    k(Kind::Statue, "statue on a plinth", 0.9, true, false, 2, false, 12.0, true),
    k(Kind::Tent, "tent", 1.2, true, true, 1, false, 7.0, true),
    KindInfo { hazard: Some(Hazard { name: "campfire", effect: "1d6 fire on entering or starting a turn in it" }), ..k(Kind::Campfire, "campfire", 0.55, false, false, 0, true, 1.5, true) },
    k(Kind::Firewood, "firewood stack", 0.7, false, false, 1, true, 3.0, true),
    k(Kind::Bedroll, "bedroll", 0.55, false, false, 0, false, 0.5, true),
];

/// Every kind, by id - 1.
const KINDS: [Kind; 45] = [
    Kind::TreeDeciduous, Kind::TreeConifer, Kind::TreePalm, Kind::TreeJungle, Kind::TreeAcacia, Kind::TreeDead, Kind::TreeWillow, Kind::Bush,
    Kind::Thicket, Kind::FallenLog, Kind::Stump, Kind::Boulder, Kind::RockSmall, Kind::RockPile, Kind::Reeds, Kind::Cactus, Kind::TallGrass,
    Kind::Flowers, Kind::Mushrooms, Kind::Bones, Kind::Snowdrift, Kind::IceSpire, Kind::BasaltPillar, Kind::Obsidian, Kind::Brambles, Kind::Bog,
    Kind::Quicksand, Kind::ThinIce, Kind::LavaPool, Kind::SteamVent, Kind::Scree, Kind::Sinkhole, Kind::Barrel, Kind::Crate, Kind::Cart,
    Kind::MarketStall, Kind::Well, Kind::Haystack, Kind::Headstone, Kind::RuinWall, Kind::Statue, Kind::Tent, Kind::Campfire, Kind::Firewood,
    Kind::Bedroll,
];

/// The kind with a catalog id.
pub fn kind_of(id: u16) -> Option<Kind> {
    KINDS.get((id as usize).checked_sub(1)?).copied()
}

pub fn info(kind: u16) -> &'static KindInfo {
    &CATALOG[(kind as usize).saturating_sub(1).min(CATALOG.len() - 1)]
}

#[derive(Clone, Copy, Debug)]
pub struct Object {
    pub kind: Kind,
    pub variant: u8,
    /// Position in squares within the chunk (0..128).
    pub x: f32,
    pub y: f32,
    pub rot: f32,
    pub scale: f32,
    /// An uploaded sprite: index + 1 into `Chunk::sprites` (0 = the built-in `kind`).
    pub sprite: u16,
}

/// An uploaded sprite used in a chunk (`Edits::sprites`).
#[derive(Clone, Debug)]
pub struct ChunkSprite {
    /// Its asset id.
    pub asset: String,
    pub name: String,
    pub info: KindInfo,
}

/// Packed kind ids from here up name `Chunk::sprites` (built-in ids stay below).
pub const SPRITE_KIND: u16 = 1000;

pub struct Chunk {
    pub key: TileKey,
    /// Ground height at square centers (ft), after micro-relief.
    pub height: Vec<f32>,
    /// Elevation tier (5-ft steps above sea level).
    pub tier: Vec<i16>,
    pub surface: Vec<Surface>,
    /// Bits: 1 cliff to the east, 2 cliff to the south, 4 slope edge east, 8 slope edge south,
    /// 16 difficult terrain.
    pub edges: Vec<u8>,
    pub objects: Vec<Object>,
    pub atmosphere: Atmosphere,
    pub water_level: Vec<f32>,
    /// Heights and water on the `HS`² grid with a one-square halo taken from the neighbours'
    /// (pure) terrain and micro-relief, so chunk edges shade seamlessly.
    pub halo_height: Vec<f32>,
    pub halo_water: Vec<f32>,
    /// Road class + 1 on the `HS`² grid (0 = none), bit 0x80 = bridge deck; the renderer
    /// filters it into smooth road edges.
    pub road_halo: Vec<u8>,
    /// Building on each square: index + 1 into `buildings` (0 = none).
    pub building: Vec<u16>,
    /// (settlement index, building index) of each building touching the chunk.
    pub buildings: Vec<(u32, u32)>,
    /// Settlement ground per square (`URBAN_*`, 0 = none).
    pub urban: Vec<u8>,
    /// Footprint of each building in `buildings` (chunk-local squares) and its storeys; the
    /// renderer draws roofs from these rather than from the square grid.
    /// With each: storeys, roof (style 0 pitched, 1 battlements, 2 cone | (tint + 1) << 2) and
    /// corner-tower side (squares).
    pub building_polys: Vec<(Vec<[f32; 2]>, u8, u8, u8)>,
    /// Height above the ground per halo square of what is drawn as a vector on top (roofs,
    /// walls, towers, bridge and pier decks; 0 elsewhere): tiers include it, the rendered
    /// ground does not.
    pub roof_lift: Vec<f32>,
    /// Walls, towers and decks drawn as vectors (chunk-local squares).
    pub structures: Vec<VectorShape>,
    /// Uploaded sprites the chunk's objects use.
    pub sprites: Vec<ChunkSprite>,
}

impl Chunk {
    /// An object's tactical rules (a built-in kind's, or an uploaded sprite's).
    pub fn info(&self, o: &Object) -> &KindInfo {
        match o.sprite {
            0 => info(o.kind as u16),
            s => &self.sprites[s as usize - 1].info,
        }
    }

    /// An object's name.
    pub fn name(&self, o: &Object) -> &str {
        match o.sprite {
            0 => info(o.kind as u16).name,
            s => &self.sprites[s as usize - 1].name,
        }
    }
}

/// A structure the renderer draws as a vector over the ground.
#[derive(Clone, Debug)]
pub struct VectorShape {
    pub kind: ShapeKind,
    /// Line width or tower radius (squares); unused for polygons.
    pub size: f32,
    pub pts: Vec<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// Curtain wall centre line.
    Wall = 0,
    /// Wall or gate tower (one point).
    Tower = 1,
    /// Bridge or pier deck outline, its first edge along the span; size is the style (0 pier,
    /// 1 timber bridge, 2 stone bridge).
    Deck = 2,
    /// Road bridge centre line.
    RoadBridge = 3,
    /// A way underground: the opening and a point one square in along the passage; size is
    /// the `UnderKind` (0 dungeon, 1 crypt, 2 cave, 3 mine, 4 lava tube, 5 sewer, 6 catacombs).
    Entrance = 4,
    /// A round feature raised a tier on paved ground: its centre and a point on its rim in the
    /// direction its steps come down (squares); size is the `PlazaFeature`.
    Dais = 5,
}

/// Object mix per biome: (kind, count per 100 squares, allowed in shallow water).
fn profile(b: Biome) -> &'static [(Kind, f64, bool)] {
    use Kind::*;
    match b {
        Biome::TemperateForest => &[(TreeDeciduous, 9.0, false), (Bush, 3.0, false), (FallenLog, 0.4, false), (Stump, 0.4, false), (Boulder, 0.3, false), (Flowers, 0.3, false), (Mushrooms, 0.3, false), (Brambles, 0.25, false)],
        Biome::TemperateRainforest => &[(TreeDeciduous, 7.0, false), (TreeConifer, 5.0, false), (Thicket, 1.5, false), (FallenLog, 0.8, false), (Mushrooms, 0.6, false), (Boulder, 0.4, false)],
        Biome::Taiga => &[(TreeConifer, 9.0, false), (Boulder, 0.6, false), (FallenLog, 0.5, false), (Bush, 1.0, false), (Snowdrift, 0.3, false)],
        Biome::Jungle => &[(TreeJungle, 8.0, false), (TreePalm, 2.0, false), (Thicket, 4.0, false), (Bush, 5.0, false), (Flowers, 0.6, false), (Quicksand, 0.08, false)],
        Biome::Grassland => &[(TallGrass, 3.0, false), (Bush, 0.6, false), (TreeDeciduous, 0.35, false), (Boulder, 0.3, false), (Flowers, 1.0, false), (Brambles, 0.12, false)],
        Biome::Steppe => &[(TallGrass, 2.0, false), (Boulder, 0.4, false), (Bush, 0.5, false), (RockSmall, 1.0, false), (Bones, 0.1, false)],
        Biome::Savanna => &[(TallGrass, 3.0, false), (TreeAcacia, 0.7, false), (Bush, 1.0, false), (Boulder, 0.3, false), (Bones, 0.1, false)],
        Biome::HotDesert => &[(Cactus, 0.35, false), (Boulder, 0.25, false), (RockSmall, 0.6, false), (Bones, 0.15, false), (Quicksand, 0.05, false)],
        Biome::ColdDesert => &[(Boulder, 0.8, false), (RockSmall, 2.0, false), (RockPile, 0.4, false), (Bush, 0.4, false)],
        Biome::Tundra => &[(Boulder, 0.6, false), (RockSmall, 1.0, false), (Snowdrift, 0.6, false), (Bush, 0.3, false), (ThinIce, 0.03, false)],
        Biome::Alpine => &[(Boulder, 1.6, false), (RockPile, 1.0, false), (RockSmall, 2.0, false), (TreeConifer, 0.4, false), (Scree, 0.3, false)],
        Biome::Ice => &[(Snowdrift, 2.0, false), (IceSpire, 0.3, false), (Boulder, 0.2, false), (ThinIce, 0.08, false)],
        Biome::Swamp => &[(TreeWillow, 2.5, false), (TreeDead, 1.5, false), (Reeds, 5.0, true), (Bog, 0.4, false), (Mushrooms, 0.4, false), (Bush, 1.0, false)],
        Biome::Volcanic => &[(BasaltPillar, 0.5, false), (Boulder, 1.0, false), (Obsidian, 0.4, false), (SteamVent, 0.15, false), (LavaPool, 0.06, false), (RockSmall, 1.5, false)],
        Biome::SaltFlat => &[(RockSmall, 0.3, false), (Bones, 0.08, false)],
        Biome::Ocean | Biome::Lake => &[(Reeds, 3.0, true), (Boulder, 0.3, false)],
    }
}

/// Cover object used by the guarantee when a cell lacks cover.
fn fallback_cover(b: Biome, rng: &mut Pcg32) -> Kind {
    use Kind::*;
    let opts: &[Kind] = match b {
        Biome::TemperateForest | Biome::TemperateRainforest => &[FallenLog, Boulder, Thicket, Bush],
        Biome::Taiga => &[Boulder, FallenLog, Bush],
        Biome::Jungle => &[Thicket, Bush, FallenLog],
        Biome::Grassland | Biome::Steppe | Biome::Savanna => &[Bush, Boulder, RockPile, Bush],
        Biome::HotDesert => &[Boulder, RockPile, Cactus],
        Biome::ColdDesert | Biome::Alpine => &[Boulder, RockPile],
        Biome::Tundra | Biome::Ice => &[Snowdrift, Boulder],
        Biome::Swamp => &[TreeDead, Bush, Thicket],
        Biome::Volcanic => &[Boulder, BasaltPillar, Obsidian],
        Biome::SaltFlat | Biome::Ocean | Biome::Lake => &[Boulder, RockPile],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

fn fallback_feature(b: Biome, rng: &mut Pcg32) -> Kind {
    use Kind::*;
    let opts: &[Kind] = match b {
        Biome::TemperateForest | Biome::TemperateRainforest => &[Mushrooms, Flowers, Brambles],
        Biome::Taiga => &[Mushrooms, Bones],
        Biome::Jungle => &[Flowers, Quicksand, Mushrooms],
        Biome::Grassland | Biome::Savanna | Biome::Steppe => &[Flowers, Bones, Brambles, RockSmall],
        Biome::HotDesert | Biome::SaltFlat => &[Bones, RockSmall, Quicksand],
        Biome::ColdDesert | Biome::Alpine => &[Scree, RockSmall, Bones],
        Biome::Tundra | Biome::Ice => &[ThinIce, Bones],
        Biome::Swamp => &[Bog, Mushrooms],
        Biome::Volcanic => &[SteamVent, Obsidian],
        Biome::Ocean | Biome::Lake => &[Reeds, RockSmall],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

fn atmosphere_for(b: Biome, h: u64) -> Atmosphere {
    let r = unit(h);
    match b {
        Biome::Swamp => if r < 0.5 { Atmosphere::Fog } else { Atmosphere::Fireflies },
        Biome::TemperateRainforest | Biome::Jungle => if r < 0.5 { Atmosphere::Mist } else { Atmosphere::Drizzle },
        Biome::TemperateForest => if r < 0.35 { Atmosphere::FallingLeaves } else if r < 0.5 { Atmosphere::Mist } else { Atmosphere::None },
        Biome::Taiga | Biome::Tundra | Biome::Ice => if r < 0.6 { Atmosphere::Snowfall } else { Atmosphere::None },
        Biome::Alpine => if r < 0.4 { Atmosphere::Snowfall } else if r < 0.6 { Atmosphere::Mist } else { Atmosphere::None },
        Biome::HotDesert => if r < 0.4 { Atmosphere::HeatHaze } else if r < 0.6 { Atmosphere::BlowingSand } else { Atmosphere::None },
        Biome::Volcanic => if r < 0.7 { Atmosphere::Embers } else { Atmosphere::HeatHaze },
        _ => Atmosphere::None,
    }
}

fn surface_for(b: Biome, slope: f64) -> Surface {
    if slope > 0.9 {
        return Surface::Rock;
    }
    match b {
        Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle => Surface::ForestFloor,
        Biome::Grassland => Surface::Grass,
        Biome::Steppe | Biome::Savanna => Surface::DryGrass,
        Biome::HotDesert => Surface::Sand,
        Biome::ColdDesert => Surface::Dirt,
        Biome::Tundra => if slope > 0.4 { Surface::Rock } else { Surface::Grass },
        Biome::Alpine => if slope > 0.5 { Surface::Rock } else { Surface::Snow },
        Biome::Ice => Surface::Snow,
        Biome::Swamp => Surface::Mud,
        Biome::Volcanic => Surface::Ash,
        Biome::SaltFlat => Surface::Salt,
        Biome::Ocean | Biome::Lake => Surface::Sand,
    }
}

/// Micro-relief feature from its grid cell (pure function of the cell): center (global
/// squares), radius (squares), height (ft, may be negative), steepness 0 smooth..1 cliff-edged,
/// elongation along `dir`, and an irregular outline (three harmonics of the angle).
struct Relief {
    cx: f64,
    cy: f64,
    r: f64,
    h: f64,
    steep: f64,
    dir: (f64, f64),
    elong: f64,
    wobble: [(f64, f64); 3],
}

/// Largest reach of a feature from its centre (squares): features come from the chunk's cells
/// and their neighbours, so they must not reach past the next cell (seams).
const RELIEF_REACH: f64 = 20.0;

fn relief_in_cell(seed: u64, gx: i64, gy: i64, biome: Biome) -> Vec<Relief> {
    let hsh = hash2(seed, gx, gy);
    let mut rng = Pcg32::new(hsh, 7);
    let (p_exist, h_lo, h_hi, steep, elong): (f64, f64, f64, f64, f64) = match biome {
        Biome::HotDesert | Biome::SaltFlat => (0.8, 4.0, 12.0, 0.2, 2.6),
        Biome::Alpine | Biome::ColdDesert | Biome::Volcanic => (0.85, 5.0, 14.0, 0.9, 1.3),
        Biome::Swamp => (0.7, 2.0, 5.0, 0.3, 1.3),
        Biome::Ocean | Biome::Lake => (0.0, 0.0, 0.0, 0.0, 1.0),
        _ => (0.65, 3.0, 11.0, 0.5, 1.5),
    };
    let mut out = Vec::new();
    // A main feature most of the time, sometimes a smaller companion.
    for (k, p) in [(0, p_exist), (1, 0.35 * p_exist)] {
        if rng.next_f64() > p {
            continue;
        }
        let ang = rng.range(0.0, std::f64::consts::TAU);
        let ditch = rng.next_f64() < 0.25;
        let sign = if ditch { -1.0 } else { 1.0 };
        // Mostly small knolls and hollows, now and then a broad mound.
        let u = rng.next_f64();
        let scale = if k == 0 { 1.0 } else { 0.6 };
        let mut r = (2.0 + 6.0 * u * u) * scale;
        let mut el = elong * rng.range(0.7, 1.4) * if ditch { rng.range(1.2, 2.0) } else { 1.0 };
        let wobble = [(rng.range(0.0, 0.2), rng.range(0.0, std::f64::consts::TAU)), (rng.range(0.0, 0.14), rng.range(0.0, std::f64::consts::TAU)), (rng.range(0.0, 0.08), rng.range(0.0, std::f64::consts::TAU))];
        let swell = 1.0 / (1.0 - wobble.iter().map(|w| w.0).sum::<f64>());
        // Keep within the reach: long features get narrower rather than longer.
        let reach = r * el.max(1.0) * swell + 1.0;
        if reach > RELIEF_REACH {
            let f = RELIEF_REACH / reach;
            r *= f.sqrt();
            el = (el * f.sqrt()).max(1.0);
        }
        out.push(Relief {
            cx: gx as f64 * RELIEF_CELL as f64 + rng.range(3.0, RELIEF_CELL as f64 - 3.0),
            cy: gy as f64 * RELIEF_CELL as f64 + rng.range(3.0, RELIEF_CELL as f64 - 3.0),
            r,
            h: sign * rng.range(h_lo, h_hi) * (0.55 + 0.45 * (r / 6.0).min(1.5)),
            steep: (steep + rng.range(-0.25, 0.25)).clamp(0.0, 1.0),
            dir: (libm::cos(ang), libm::sin(ang)),
            elong: el,
            wobble,
        });
    }
    out
}

fn relief_value(f: &Relief, x: f64, y: f64) -> f64 {
    let (dx, dy) = (x - f.cx, y - f.cy);
    let along = (dx * f.dir.0 + dy * f.dir.1) / f.elong;
    let across = -dx * f.dir.1 + dy * f.dir.0;
    // Irregular outline: the radius swells and pinches with the angle.
    let th = libm::atan2(across, along);
    let edge = 1.0 + f.wobble.iter().enumerate().map(|(k, (a, ph))| a * libm::cos((k + 1) as f64 * th + ph)).sum::<f64>();
    let d = crate::core::sqrt(along * along + across * across) / (f.r * edge);
    if d >= 1.0 {
        return 0.0;
    }
    // Smooth dome blended toward a flat-topped, sharp-edged mesa by steepness.
    let dome = 1.0 - smoothstep(0.0, 1.0, d);
    let mesa = 1.0 - smoothstep(0.78, 0.92, d);
    f.h * (dome + (mesa - dome) * f.steep)
}

/// The road surface (ft) at a point on a road: the nearest road piece within its width.
fn road_level(roads: &[crate::lod::roads::RoadPiece], p: [f64; 2]) -> Option<f32> {
    let mut best: Option<(f64, f64)> = None;
    for piece in roads {
        let reach = 0.5 * piece.class.width_ft() + SQUARE_FT;
        for seg in piece.pts.windows(2) {
            let (a, b) = (seg[0].p, seg[1].p);
            if p[0] < a[0].min(b[0]) - reach || p[0] > a[0].max(b[0]) + reach || p[1] < a[1].min(b[1]) - reach || p[1] > a[1].max(b[1]) + reach {
                continue;
            }
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
            let (ex, ey) = (a[0] + dx * t - p[0], a[1] + dy * t - p[1]);
            let d = crate::core::sqrt(ex * ex + ey * ey);
            if d < reach && best.is_none_or(|b| d < b.0) {
                best = Some((d, seg[0].z + (seg[1].z - seg[0].z) * t));
            }
        }
    }
    best.map(|b| b.1 as f32)
}

pub fn generate(world: &World, t0: &T0, key: &TileKey, tile: &TerrainOut) -> Chunk {
    let g = &world.geom;
    let sea = world.params().sea_level_ft;
    let (ox, oy) = g.tile_origin_ft(key);
    let (gsx, gsy) = (key.x as i64 * SQ as i64, key.y as i64 * SQ as i64);
    let n = SQ * SQ;
    let seed = world.stream("battlemap");

    // Square centers are odd lattice samples of the finest level (2.5 ft spacing). Square
    // (i, j) with i, j in -1..=SQ (halo included) is sample (2i + 1, 2j + 1) of the tile.
    let sidx = |i: i64, j: i64| ((2 * j + 1 + HALO as i64) as usize) * PADDED + (2 * i + 1 + HALO as i64) as usize;
    let mut hh = vec![0f32; HS * HS];
    let mut hw = vec![0f32; HS * HS];
    // Road class + 1 per halo square (0 = none); bridges where a road meets water.
    let mut road_h = vec![0u8; HS * HS];
    let mut lift = vec![0f32; HS * HS];
    for j in -1..=SQ as i64 {
        for i in -1..=SQ as i64 {
            let (x, y) = (ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT);
            let c = sidx(i, j);
            let h = (j + 1) as usize * HS + (i + 1) as usize;
            hh[h] = tile.padded[c];
            hw[h] = t0.sample_water(x, y).max(tile.river_water[c]);
            road_h[h] = tile.road_mask.get(c).copied().unwrap_or(0);
            if road_h[h] != 0 && hw[h] > hh[h] {
                // The deck carries the road at its own level (its embankments reach the
                // banks), at least clear of the water.
                let top = (hw[h] + DECK_CLEARANCE_FT).max(road_level(&tile.roads, [x, y]).unwrap_or(f32::MIN));
                lift[h] = top - hh[h];
                hh[h] = top;
                road_h[h] |= 0x80;
            }
        }
    }
    // Settlements: buildings (roofs at eave height), streets and plazas, fields.
    let mut bld_h = vec![0u16; HS * HS];
    let mut urb_h = vec![0u8; HS * HS];
    // Ground kept clear round a way underground (its own ground stays: graveyard grass, ruin
    // dirt, street cobbles).
    let mut ent_h = vec![false; HS * HS];
    let mut refs: Vec<(u32, u32)> = Vec::new();
    let mut polys: Vec<(Vec<[f32; 2]>, u8, u8, u8)> = Vec::new();
    let mut shapes: Vec<VectorShape> = Vec::new();
    let local = |p: &[f64; 2]| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32];
    let chunk_ft = SQ as f64 * SQUARE_FT;
    let owns = |p: [f64; 2]| p[0] >= ox && p[0] < ox + chunk_ft && p[1] >= oy && p[1] < oy + chunk_ft;
    let mut wells: Vec<[f64; 2]> = Vec::new();
    // Market stalls in blocks on town plazas (world ft, rotation, kind).
    let mut stalls: Vec<([f64; 2], f32, Kind)> = Vec::new();
    // Round raised features on town squares: (centre, radius ft, kind, steps angle).
    let mut features: Vec<([f64; 2], f64, PlazaFeature, f64)> = Vec::new();
    let mut in_town = vec![false; HS * HS];
    let mut ruins: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut statues: Vec<[f64; 2]> = Vec::new();
    // Props a layout sets out itself (a camp's tents and fire).
    let mut props: Vec<crate::town::Prop> = Vec::new();
    // Camps' clearings, levelled (outline, height ft).
    let mut yards: Vec<(Vec<[f64; 2]>, f32)> = Vec::new();
    // Ground before anything is raised on it (walls stand on the ground, not on roofs).
    let ground_h = hh.clone();
    let (hx0, hy0) = (ox - SQUARE_FT, oy - SQUARE_FT);
    let span = HS as f64 * SQUARE_FT;
    let center = |i: usize, j: usize| [hx0 + (i as f64 + 0.5) * SQUARE_FT, hy0 + (j as f64 + 0.5) * SQUARE_FT];
    let bbox = |poly: &[[f64; 2]]| poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    // Halo squares a polygon's bbox may cover.
    let range = |b: (f64, f64, f64, f64)| {
        let i0 = (((b.0 - hx0) / SQUARE_FT - 0.5).floor().max(0.0)) as usize;
        let j0 = (((b.1 - hy0) / SQUARE_FT - 0.5).floor().max(0.0)) as usize;
        let i1 = (((b.2 - hx0) / SQUARE_FT - 0.5).ceil()).min(HS as f64 - 1.0);
        let j1 = (((b.3 - hy0) / SQUARE_FT - 0.5).ceil()).min(HS as f64 - 1.0);
        (i0, j0, i1, j1)
    };
    let overlaps = |b: (f64, f64, f64, f64)| b.2 >= hx0 && b.0 <= hx0 + span && b.3 >= hy0 && b.1 <= hy0 + span;
    for l in crate::town::layouts_near(world, t0, [hx0, hy0, hx0 + span, hy0 + span]) {
        let stamp = |poly: &[[f64; 2]], val: u8, urb_h: &mut Vec<u8>| {
            let b = bbox(poly);
            if !overlaps(b) {
                return;
            }
            let (i0, j0, i1, j1) = range(b);
            if i1 < 0.0 || j1 < 0.0 {
                return;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    let h = j * HS + i;
                    // Anything more specific than a street wins (plazas, graveyards, ruins).
                    if (urb_h[h] == 0 || val != URBAN_STREET) && crate::town::geom::contains(poly, center(i, j)) {
                        urb_h[h] = val;
                    }
                }
            }
        };
        // Only streets are paved (stamped from the town's road lines below); yards and
        // gardens between buildings keep their natural ground, but no mounds or ditches.
        for d in &l.districts {
            let b = bbox(d);
            if !overlaps(b) {
                continue;
            }
            let (i0, j0, i1, j1) = range(b);
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    if crate::town::geom::contains(d, center(i, j)) {
                        in_town[j * HS + i] = true;
                    }
                }
            }
        }
        for d in l.bridges.iter().chain(&l.piers) {
            stamp(d, URBAN_DECK, &mut urb_h);
        }
        for y in &l.yards {
            stamp(&y.0, URBAN_YARD, &mut urb_h);
            yards.push(y.clone());
        }
        for p in &l.plazas {
            stamp(p, URBAN_PLAZA, &mut urb_h);
            if !l.site {
                wells.push(crate::town::geom::centroid(p));
                if l.tier >= crate::t0::settle::Tier::Town && overlaps(bbox(p)) {
                    let first = stalls.len();
                    // Lanes the features keep off: the town's streets and roads, and the
                    // world's roads across the square.
                    let mut lanes: Vec<([f64; 2], [f64; 2], f64)> = Vec::new();
                    for (pts, _, w) in &l.roads {
                        lanes.extend(pts.windows(2).map(|q| (q[0], q[1], 0.5 * w)));
                    }
                    for st in &l.streets {
                        lanes.extend(st.windows(2).map(|q| (q[0], q[1], 5.0)));
                    }
                    let b = bbox(p);
                    for (ri, k) in t0.roads.segments_near([b.0 - 50.0, b.1 - 50.0, b.2 + 50.0, b.3 + 50.0], 0.0) {
                        let rc = &t0.roads.roads[ri as usize];
                        for j in 0..16 {
                            let a = rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p;
                            let e = rc.eval(k as usize, (j + 1) as f64 / 16.0, 5.0, t0.cell_ft).p;
                            lanes.push((a, e, 12.0));
                        }
                    }
                    // A city's squares are laid out first, in a pattern round the well, and the
                    // market fits round them; a town's features go where the market leaves room.
                    let patterned = l.tier >= crate::t0::settle::Tier::City;
                    let ffirst = features.len();
                    if patterned {
                        plaza_features(p, &l.monuments, &[], &lanes, seed, true, &mut features);
                    }
                    let keep: Vec<([f64; 2], f64)> = features[ffirst..].iter().map(|f| (f.0, f.1)).collect();
                    market_blocks(p, &l.monuments, &keep, seed, &mut stalls);
                    if !patterned {
                        plaza_features(p, &l.monuments, &stalls[first..], &lanes, seed, false, &mut features);
                    }
                }
            }
        }
        for f in &l.fields {
            stamp(f, URBAN_FIELD, &mut urb_h);
        }
        // Ways underground: the opening's ground stays clear; the chunk holding it draws it.
        // `size` = kind + 16 × style (0 plain, 1 a ruin's broken stair, 2 a graveyard's
        // mausoleum headhouse).
        for e in &l.entrances {
            use crate::town::geom::{add, mul};
            let in_yard = l.buildings.iter().any(|b| b.structure == crate::town::Structure::Open && crate::town::geom::contains(&b.poly, e.at));
            let style = match e.kind {
                crate::under::UnderKind::Dungeon | crate::under::UnderKind::Crypt => 1u8,
                crate::under::UnderKind::Catacombs if in_yard => 2,
                _ => 0,
            };
            let clear = if style == 2 {
                // The headhouse (4 × 3 squares) and a square round it.
                let side = [-e.dir[1], e.dir[0]];
                let at = |a: f64, b: f64| add(e.at, add(mul(e.dir, a * SQUARE_FT), mul(side, b * SQUARE_FT)));
                vec![at(-2.5, -2.5), at(3.5, -2.5), at(3.5, 2.5), at(-2.5, 2.5)]
            } else {
                // A street grate needs no clearing; stairs a little; mouths in the wild more.
                let r = match e.kind {
                    crate::under::UnderKind::Sewer => 3.0,
                    crate::under::UnderKind::Catacombs => 9.0,
                    _ => 18.0,
                };
                crate::town::geom::circle(e.at, r, 16)
            };
            let b = bbox(&clear);
            if overlaps(b) {
                let (i0, j0, i1, j1) = range(b);
                if i1 >= 0.0 && j1 >= 0.0 {
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            if crate::town::geom::contains(&clear, center(i, j)) {
                                ent_h[j * HS + i] = true;
                            }
                        }
                    }
                }
            }
            if owns(e.at) {
                let inward = add(e.at, mul(e.dir, SQUARE_FT));
                shapes.push(VectorShape { kind: ShapeKind::Entrance, size: (e.kind as u8 + 16 * style) as f32, pts: vec![local(&e.at), local(&inward)] });
            }
        }
        // Streets drawn as roads (approaches to gates, main streets, bridge streets): street
        // ground wherever they run, inside the walls or out.
        for (pts, _, width) in &l.roads {
            let half = 0.5 * width;
            for seg in pts.windows(2) {
                let b = (seg[0][0].min(seg[1][0]) - half, seg[0][1].min(seg[1][1]) - half, seg[0][0].max(seg[1][0]) + half, seg[0][1].max(seg[1][1]) + half);
                if !overlaps(b) {
                    continue;
                }
                let (i0, j0, i1, j1) = range(b);
                if i1 < 0.0 || j1 < 0.0 {
                    continue;
                }
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let h = j * HS + i;
                        if urb_h[h] != URBAN_DECK && crate::town::geom::seg_dist(center(i, j), seg[0], seg[1]) < half {
                            urb_h[h] = URBAN_STREET;
                        }
                    }
                }
            }
        }
        if l.tier == crate::t0::settle::Tier::Village {
            // Village lanes (10 ft wide).
            for lane in &l.streets {
                for seg in lane.windows(2) {
                    let b = (seg[0][0].min(seg[1][0]) - 5.0, seg[0][1].min(seg[1][1]) - 5.0, seg[0][0].max(seg[1][0]) + 5.0, seg[0][1].max(seg[1][1]) + 5.0);
                    if !overlaps(b) {
                        continue;
                    }
                    let (i0, j0, i1, j1) = range(b);
                    if i1 < 0.0 || j1 < 0.0 {
                        continue;
                    }
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            if crate::town::geom::seg_dist(center(i, j), seg[0], seg[1]) < 5.0 && urb_h[j * HS + i] != URBAN_PLAZA {
                                urb_h[j * HS + i] = URBAN_STREET;
                            }
                        }
                    }
                }
            }
        }
        for (bi, b) in l.buildings.iter().enumerate() {
            let bb = bbox(&b.poly);
            if !overlaps(bb) {
                continue;
            }
            match b.structure {
                crate::town::Structure::Roofed => {}
                crate::town::Structure::Open => {
                    stamp(&b.poly, URBAN_GRAVEYARD, &mut urb_h);
                    continue;
                }
                crate::town::Structure::Ruin => {
                    stamp(&b.poly, URBAN_RUIN, &mut urb_h);
                    ruins.push(b.poly.clone());
                    continue;
                }
            }
            let (i0, j0, i1, j1) = range(bb);
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            let mut id = 0u16;
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    if crate::town::geom::contains(&b.poly, center(i, j)) {
                        if id == 0 {
                            refs.push((l.index, bi as u32));
                            let battlements = crate::interior::battlements(b);
                            polys.push((
                                b.poly.iter().map(|p| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32]).collect(),
                                b.floors,
                                roof_byte(b, battlements.is_some()),
                                crate::core::round(battlements.unwrap_or(0.0) / SQUARE_FT) as u8,
                            ));
                            id = refs.len().min(u16::MAX as usize) as u16;
                        }
                        let h = j * HS + i;
                        bld_h[h] = id;
                        lift[h] = b.pad_ft + STOREY_FT * b.floors as f32 - hh[h];
                        hh[h] = b.pad_ft + STOREY_FT * b.floors as f32;
                    }
                }
            }
        }
        // Walls (8 ft thick) and towers (22 ft across; 30 at gates): raised stone.
        let raise = |a: [f64; 2], b: [f64; 2], half: f64, val: u8, urb_h: &mut Vec<u8>, hh: &mut Vec<f32>, lift: &mut Vec<f32>| {
            let bb = (a[0].min(b[0]) - half, a[1].min(b[1]) - half, a[0].max(b[0]) + half, a[1].max(b[1]) + half);
            if !overlaps(bb) {
                return;
            }
            let (i0, j0, i1, j1) = range(bb);
            if i1 < 0.0 || j1 < 0.0 {
                return;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    let h = j * HS + i;
                    if bld_h[h] != 0 || hw[h] > hh[h] || urb_h[h] == URBAN_TOWER {
                        continue;
                    }
                    // Every square the drawn wall or tower touches stands on it.
                    if crate::town::geom::seg_dist(center(i, j), a, b) <= half + 1.5 {
                        urb_h[h] = val;
                        hh[h] = ground_h[h] + if val == URBAN_TOWER { TOWER_FT } else { WALL_FT };
                        lift[h] = hh[h] - ground_h[h];
                    }
                }
            }
        };
        for t in &l.towers {
            raise(*t, *t, 11.0, URBAN_TOWER, &mut urb_h, &mut hh, &mut lift);
        }
        for t in &l.gate_towers {
            raise(*t, *t, 15.0, URBAN_TOWER, &mut urb_h, &mut hh, &mut lift);
        }
        for w in &l.walls {
            for seg in w.windows(2) {
                raise(seg[0], seg[1], 4.0, URBAN_WALL, &mut urb_h, &mut hh, &mut lift);
            }
        }
        // The same structures as vectors, each drawn by the one chunk that owns its middle.
        for w in &l.walls {
            for seg in w.windows(2) {
                if owns([0.5 * (seg[0][0] + seg[1][0]), 0.5 * (seg[0][1] + seg[1][1])]) {
                    shapes.push(VectorShape { kind: ShapeKind::Wall, size: (8.0 / SQUARE_FT) as f32, pts: vec![local(&seg[0]), local(&seg[1])] });
                }
            }
        }
        for (ts, r) in [(&l.towers, 11.0), (&l.gate_towers, 15.0)] {
            for t in ts.iter() {
                if owns(*t) {
                    shapes.push(VectorShape { kind: ShapeKind::Tower, size: (r / SQUARE_FT) as f32, pts: vec![local(t)] });
                }
            }
        }
        // Deck style: 0 pier, 1 a village or town's timber bridge, 2 a city's stone bridge.
        let style = if l.tier >= crate::t0::settle::Tier::City { 2.0 } else { 1.0 };
        for (d, size) in l.bridges.iter().map(|d| (d, style)).chain(l.piers.iter().map(|d| (d, 0.0))) {
            if owns(crate::town::geom::centroid(d)) {
                shapes.push(VectorShape { kind: ShapeKind::Deck, size, pts: d.iter().map(local).collect() });
            }
        }
        props.extend(l.props.iter().copied());
        for m in &l.monuments {
            statues.push(crate::town::geom::centroid(m));
        }
    }
    // Decks over water become plank road squares (same machinery as road bridges).
    for h in 0..HS * HS {
        if urb_h[h] == URBAN_DECK && hw[h] > hh[h] - DECK_CLEARANCE_FT {
            let top = hh[h].max(hw[h] + DECK_CLEARANCE_FT);
            lift[h] += top - hh[h];
            hh[h] = top;
            road_h[h] = 0x80 | 3;
        }
    }
    // Road bridges as vectors: wherever a road bed crosses a river's centre line, a deck along
    // the road spanning the channel (its width over the crossing angle, plus a landing each
    // side). Both lines are pure functions of the world, so every chunk finds the same
    // crossing; the chunk owning it draws the bridge.
    for piece in &tile.roads {
        for rs in piece.pts.windows(2) {
            let (a, b) = (rs[0].p, rs[1].p);
            let (rx, ry) = (b[0] - a[0], b[1] - a[1]);
            let rl = crate::core::sqrt(rx * rx + ry * ry);
            if rl < 1e-6 {
                continue;
            }
            let u = [rx / rl, ry / rl];
            for river in &tile.pieces {
                for vs in river.pts.windows(2) {
                    let (c, d) = (vs[0].p, vs[1].p);
                    let (sx, sy) = (d[0] - c[0], d[1] - c[1]);
                    let den = rx * sy - ry * sx;
                    if den.abs() < 1e-9 {
                        continue;
                    }
                    let t = ((c[0] - a[0]) * sy - (c[1] - a[1]) * sx) / den;
                    let v = ((c[0] - a[0]) * ry - (c[1] - a[1]) * rx) / den;
                    if !(0.0..1.0).contains(&t) || !(0.0..1.0).contains(&v) {
                        continue;
                    }
                    let x = [a[0] + rx * t, a[1] + ry * t];
                    if !owns(x) {
                        continue;
                    }
                    let sl = crate::core::sqrt(sx * sx + sy * sy).max(1e-9);
                    let sin = (den / (rl * sl)).abs().max(0.35);
                    let w = vs[0].w + (vs[1].w - vs[0].w) * v;
                    let deck = piece.class.width_ft() + 4.0;
                    // Each end reaches past the river (its width there plus 4 ft of bank) at both
                    // corners, which matters when the road crosses at a slant; then 8 ft of landing.
                    let clear = |p: [f64; 2]| {
                        tile.pieces.iter().all(|r| {
                            r.pts.windows(2).all(|q| {
                                let (a, b) = (q[0].p, q[1].p);
                                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                                let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
                                let (ex, ey) = (p[0] - a[0] - dx * t, p[1] - a[1] - dy * t);
                                crate::core::sqrt(ex * ex + ey * ey) >= 0.5 * (q[0].w + (q[1].w - q[0].w) * t) + 4.0
                            })
                        })
                    };
                    let reach = |sign: f64| {
                        let mut t = 0.5 * w / sin;
                        let corner = |t: f64, side: f64| [x[0] + u[0] * sign * t - u[1] * side * 0.5 * deck, x[1] + u[1] * sign * t + u[0] * side * 0.5 * deck];
                        while t < 600.0 && !(clear(corner(t, 1.0)) && clear(corner(t, -1.0))) {
                            t += 2.0;
                        }
                        t + 8.0
                    };
                    let (fwd, back) = (reach(1.0), reach(-1.0));
                    let (p0, p1) = ([x[0] - u[0] * back, x[1] - u[1] * back], [x[0] + u[0] * fwd, x[1] + u[1] * fwd]);
                    shapes.push(VectorShape { kind: ShapeKind::RoadBridge, size: (deck / SQUARE_FT) as f32, pts: vec![local(&p0), local(&p1)] });
                }
            }
        }
    }
    let road_bed = hh.clone();
    let mut biome = vec![Biome::Grassland; n];
    let mut slope = vec![0f64; n];
    for j in 0..SQ {
        for i in 0..SQ {
            let k = j * SQ + i;
            let (x, y) = (ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT);
            biome[k] = Biome::from_u8(t0.sample_biome(x, y, t0.biome_warp(x, y))[0]);
            let c = sidx(i as i64, j as i64);
            let gx = (tile.padded[c + 1] - tile.padded[c - 1]) as f64 / 5.0;
            let gy = (tile.padded[c + PADDED] - tile.padded[c - PADDED]) as f64 / 5.0;
            slope[k] = crate::core::sqrt(gx * gx + gy * gy);
        }
    }

    // Micro-relief from global feature cells (dry ground only).
    let rseed = hash2(seed, 0x5e11, 1);
    let (cx0, cy0) = (gsx.div_euclid(RELIEF_CELL) - 1, gsy.div_euclid(RELIEF_CELL) - 1);
    let (cx1, cy1) = ((gsx + SQ as i64).div_euclid(RELIEF_CELL) + 1, (gsy + SQ as i64).div_euclid(RELIEF_CELL) + 1);
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            // Biome at the feature cell's true world position (not clamped into this chunk),
            // so a feature straddling two chunks is identical in both.
            let (wx, wy) = (((cx * RELIEF_CELL + RELIEF_CELL / 2) as f64 + 0.5) * SQUARE_FT, ((cy * RELIEF_CELL + RELIEF_CELL / 2) as f64 + 0.5) * SQUARE_FT);
            let b = Biome::from_u8(t0.sample_biome(wx, wy, t0.biome_warp(wx, wy))[0]);
            for f in relief_in_cell(rseed, cx, cy, b) {
                apply_relief(&f, &mut hh, &hw, gsx - 1, gsy - 1, HS);
            }
        }
    }
    // Micro-relief never lifts or digs a road bed, a building, a street, a field or the ground
    // at a way underground.
    let protected: Vec<bool> =
        (0..HS * HS).map(|h| road_h[h] != 0 || bld_h[h] != 0 || in_town[h] || ent_h[h] || matches!(urb_h[h], URBAN_STREET | URBAN_PLAZA | URBAN_WALL | URBAN_TOWER | URBAN_FIELD | URBAN_YARD)).collect();
    for h in 0..HS * HS {
        if protected[h] {
            hh[h] = road_bed[h];
        }
    }
    // A camp's clearing is level: the ground eases to one height from just past the rim to
    // three quarters of the way in (so the tents stand on one tier). That height is the mean
    // of the ground in it when the chunk holds it all (camps are placed so), else the layout's.
    for (poly, level) in &yards {
        let c = crate::town::geom::centroid(poly);
        let reach = poly.iter().map(|q| crate::town::geom::dist(*q, c)).fold(0.0, f64::max);
        let b = (c[0] - 1.2 * reach, c[1] - 1.2 * reach, c[0] + 1.2 * reach, c[1] + 1.2 * reach);
        if !overlaps(b) {
            continue;
        }
        let (i0, j0, i1, j1) = range(b);
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        let whole = b.0 >= ox && b.1 >= oy && b.2 < ox + chunk_ft && b.3 < oy + chunk_ft;
        let level = if whole {
            let inside: Vec<f32> = (j0..=j1 as usize).flat_map(|j| (i0..=i1 as usize).map(move |i| (i, j))).filter(|&(i, j)| crate::town::geom::contains(poly, center(i, j))).map(|(i, j)| hh[j * HS + i]).collect();
            if inside.is_empty() { *level } else { inside.iter().sum::<f32>() / inside.len() as f32 }
        } else {
            *level
        };
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let t = ((crate::town::geom::dist(center(i, j), c) / reach - 0.75) / 0.4).clamp(0.0, 1.0);
                let w = 1.0 - t * t * (3.0 - 2.0 * t);
                if w > 0.0 && road_h[h] == 0 && bld_h[h] == 0 {
                    hh[h] += (level - hh[h]) * w as f32;
                }
            }
        }
    }
    // Plaza features stand a tier above the paving (a fountain's rim, a planter, a cross's
    // steps): every square whose centre lies within the round.
    for &(fc, r, _, _) in &features {
        let b = (fc[0] - r, fc[1] - r, fc[0] + r, fc[1] + r);
        if !overlaps(b) {
            continue;
        }
        let (i0, j0, i1, j1) = range(b);
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                if crate::town::geom::dist(center(i, j), fc) <= r && road_h[h] == 0 && bld_h[h] == 0 {
                    hh[h] = road_bed[h] + (TIER_FT * 1.05) as f32;
                }
            }
        }
    }
    let road: Vec<u8> = (0..n).map(|k| road_h[(k / SQ + 1) * HS + k % SQ + 1]).collect();
    let interior = |grid: &[f32]| -> Vec<f32> { (0..n).map(|k| grid[(k / SQ + 1) * HS + k % SQ + 1]).collect() };
    let height = interior(&hh);
    let water_level = interior(&hw);
    let dominant = dominant_biome(&biome, &water_level, &height);

    let mut chunk = Chunk {
        key: *key,
        tier: vec![0; n],
        surface: vec![Surface::Grass; n],
        edges: vec![0; n],
        objects: Vec::new(),
        atmosphere: atmosphere_for(dominant, hash3(seed, gsx, gsy, 0xa7)),
        height,
        water_level,
        halo_height: hh,
        halo_water: hw,
        road_halo: road_h,
        building: (0..n).map(|k| bld_h[(k / SQ + 1) * HS + k % SQ + 1]).collect(),
        buildings: refs,
        urban: (0..n).map(|k| if ent_h[(k / SQ + 1) * HS + k % SQ + 1] { URBAN_ENTRANCE } else { urb_h[(k / SQ + 1) * HS + k % SQ + 1] }).collect(),
        building_polys: polys,
        roof_lift: lift,
        structures: shapes,
        sprites: Vec::new(),
    };
    retier(&mut chunk, sea);

    // Surfaces. Towns and cities pave their streets; villages have packed earth.
    let tier_here = crate::town::layouts_near(world, t0, [ox, oy, ox + SQ as f64 * SQUARE_FT, oy + SQ as f64 * SQUARE_FT]).iter().map(|l| l.tier).max();
    let paved = tier_here.is_some_and(|t| t >= crate::t0::settle::Tier::Town);
    let village = tier_here == Some(crate::t0::settle::Tier::Village);
    for k in 0..n {
        let depth = chunk.water_level[k] - chunk.height[k];
        chunk.surface[k] = if depth > 4.0 {
            Surface::Deep
        } else if depth > 0.0 {
            Surface::Shallow
        } else if depth > -3.0 && chunk.water_level[k] as f64 <= sea + 0.5 && slope[k] < 0.15 {
            Surface::Sand
        } else {
            surface_for(biome[k], slope[k])
        };
        if depth <= 0.0 {
            // The ground's own kind, under any keep-clear mark.
            match urb_h[(k / SQ + 1) * HS + k % SQ + 1] {
                URBAN_PLAZA => chunk.surface[k] = if village { Surface::Grass } else { Surface::Cobble },
                URBAN_STREET => chunk.surface[k] = if paved { Surface::Cobble } else { Surface::Road },
                URBAN_FIELD => chunk.surface[k] = Surface::Field,
                URBAN_GRAVEYARD => chunk.surface[k] = Surface::Grass,
                URBAN_DECK => chunk.surface[k] = if paved { Surface::Cobble } else { Surface::Road },
                URBAN_RUIN | URBAN_YARD => chunk.surface[k] = Surface::Dirt,
                URBAN_WALL | URBAN_TOWER => chunk.surface[k] = Surface::Rock,
                _ => {}
            }
        }
        if road[k] != 0 {
            chunk.surface[k] = if road[k] & 0x80 != 0 {
                Surface::Planks
            } else if road[k] == 1 {
                Surface::Cobble
            } else {
                Surface::Road
            };
        }
        if chunk.building[k] != 0 {
            chunk.surface[k] = Surface::Roof;
        }
    }

    place_objects(&mut chunk, &biome, &slope, seed, gsx, gsy);
    place_town_props(&mut chunk, seed, gsx, gsy);
    // Ruins: broken wall segments along the old outlines, rubble inside. Keyed by world
    // position so a ruin across a chunk border breaks the same way in both chunks.
    let wseed = hash2(seed, 0x2a11, 5);
    for poly in &ruins {
        let m = poly.len();
        for e in 0..m {
            let (a, b) = (poly[e], poly[(e + 1) % m]);
            let len = crate::town::geom::dist(a, b);
            let steps = (len / SQUARE_FT).floor().max(1.0) as usize;
            for s in 0..steps {
                let p = crate::town::geom::lerp(a, b, (s as f64 + 0.5) / steps as f64);
                let h = hash2(wseed, (p[0] * 2.0) as i64, (p[1] * 2.0) as i64);
                // Long runs survive, with breaches.
                let run = unit(hash2(wseed ^ 0x77, (p[0] / 20.0) as i64, (p[1] / 20.0) as i64));
                if run < 0.3 || unit(h) < 0.15 {
                    continue;
                }
                let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
                if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
                    continue;
                }
                let rot = libm::atan2(b[1] - a[1], b[0] - a[0]) as f32;
                let variant = (h >> 20) as u8 & 7;
                chunk.objects.push(Object { kind: Kind::RuinWall, variant, x: x as f32, y: y as f32, rot, scale: 1.0, sprite: 0 });
            }
        }
        let c = crate::town::geom::centroid(poly);
        let h = hash2(wseed, (c[0] * 2.0) as i64, (c[1] * 2.0) as i64);
        let (x, y) = ((c[0] - ox) / SQUARE_FT, (c[1] - oy) / SQUARE_FT);
        if unit(h) < 0.7 && x >= 0.0 && y >= 0.0 && x < SQ as f64 && y < SQ as f64 {
            chunk.objects.push(Object { kind: Kind::RockPile, variant: (h >> 20) as u8 & 7, x: x as f32, y: y as f32, rot: 0.0, scale: 1.3, sprite: 0 });
        }
    }
    for st in &statues {
        let (x, y) = ((st[0] - ox) / SQUARE_FT, (st[1] - oy) / SQUARE_FT);
        if x >= 0.0 && y >= 0.0 && x < SQ as f64 && y < SQ as f64 {
            chunk.objects.push(Object { kind: Kind::Statue, variant: 0, x: x as f32, y: y as f32, rot: 0.0, scale: 1.0, sprite: 0 });
        }
    }
    // A layout's own props: each in the chunk holding it, on dry ground off roads and roofs.
    for p in &props {
        let (x, y) = ((p.at[0] - ox) / SQUARE_FT, (p.at[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.building[k] != 0 || chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] {
            continue;
        }
        chunk.objects.push(Object { kind: p.kind, variant: p.variant, x: x as f32, y: y as f32, rot: p.rot, scale: p.scale, sprite: 0 });
    }
    // Plaza features: drawn by the chunk holding the centre; a planter's tree stands in it.
    for &(fc, r, kind, ang) in &features {
        let (x, y) = ((fc[0] - ox) / SQUARE_FT, (fc[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        // Drawn a little past the raised squares' centres, so the round covers them.
        let rv = (r + 2.0) / SQUARE_FT;
        let rim = [(x + rv * libm::cos(ang)) as f32, (y + rv * libm::sin(ang)) as f32];
        chunk.structures.push(VectorShape { kind: ShapeKind::Dais, size: kind as u8 as f32, pts: vec![[x as f32, y as f32], rim] });
        if kind == PlazaFeature::Planter {
            chunk.objects.push(Object { kind: Kind::TreeDeciduous, variant: (hash2(seed, x as i64, y as i64) >> 20) as u8 & 7, x: x as f32, y: y as f32, rot: 0.0, scale: 0.9, sprite: 0 });
        }
    }
    // The market: each stall in the chunk holding its centre, on open plaza ground.
    for (p, rot, kind) in stalls {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.urban[k] != URBAN_PLAZA || chunk.building[k] != 0 || chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] {
            continue;
        }
        let h = hash2(seed ^ 0x5a11, (p[0] * 2.0) as i64, (p[1] * 2.0) as i64);
        let o = Object { kind, variant: (h >> 20) as u8 & 7, x: x as f32, y: y as f32, rot, scale: 1.0, sprite: 0 };
        // The plaza's margin keeps stalls off buildings; across a chunk edge the neighbour's
        // squares are plaza too.
        let r = info(kind as u16).radius;
        let near_building = (-1i32..=1).any(|dy| {
            (-1i32..=1).any(|dx| {
                let (i, j) = ((o.x + dx as f32 * r) as i32, (o.y + dy as f32 * r) as i32);
                i >= 0 && j >= 0 && (i as usize) < SQ && (j as usize) < SQ && chunk.building[j as usize * SQ + i as usize] != 0
            })
        });
        if !near_building {
            chunk.objects.push(o);
        }
    }
    // A well at the heart of every plaza and green.
    for w in wells {
        let (i, j) = (((w[0] - ox) / SQUARE_FT).floor(), ((w[1] - oy) / SQUARE_FT).floor());
        if i >= 0.0 && j >= 0.0 && (i as usize) < SQ && (j as usize) < SQ {
            let k = j as usize * SQ + i as usize;
            if chunk.building[k] == 0 && chunk.water_level[k] <= chunk.height[k] {
                chunk.objects.push(Object { kind: Kind::Well, variant: 0, x: ((w[0] - ox) / SQUARE_FT) as f32, y: ((w[1] - oy) / SQUARE_FT) as f32, rot: 0.0, scale: 1.0, sprite: 0 });
            }
        }
    }
    enforce(&mut chunk, &biome, seed, gsx, gsy, sea);
    // Enforcement knolls may have spilled onto a road bed or a building; those keep their
    // level (on a street or plaza a raised terrace or dais is fine).
    let mut touched = false;
    for k in 0..n {
        let h = (k / SQ + 1) * HS + k % SQ + 1;
        let bed = road_bed[h];
        if (chunk.road_halo[h] != 0 || chunk.building[k] != 0) && chunk.height[k] != bed {
            chunk.height[k] = bed;
            touched = true;
        }
    }
    if touched {
        retier(&mut chunk, sea);
    }
    // Enforcement only edits the interior (≥ 7 squares from the edge); copy it back.
    for k in 0..n {
        chunk.halo_height[(k / SQ + 1) * HS + k % SQ + 1] = chunk.height[k];
    }
    apply_edits(&mut chunk, &world.file.edits, ox, oy);
    compute_edges(&mut chunk);
    chunk
}

/// Objects put down and taken away by hand (`Edits::objects`, `Edits::cleared`): generated
/// objects matching a clear go, then those put down in this chunk (by their centre) are added.
/// A roof as packed: style (0 hip, 1 battlements, 2 cone) | (tint + 1) << 2 (0: picked).
fn roof_byte(b: &crate::town::Building, battlements: bool) -> u8 {
    let style = if battlements {
        1
    } else if b.roof == Some(crate::town::RoofStyle::Cone) {
        2
    } else {
        0
    };
    style | b.tint.map_or(0, |t| (t + 1) << 2)
}

fn apply_edits(c: &mut Chunk, e: &crate::world::Edits, ox: f64, oy: f64) {
    use crate::world::ObjKind;
    let chunk_ft = SQ as f64 * SQUARE_FT;
    if !e.cleared.is_empty() {
        let reach = |cl: &crate::world::Clear| cl.r.unwrap_or(0.25 * SQUARE_FT);
        let near: Vec<&crate::world::Clear> = e
            .cleared
            .values()
            .filter(|cl| {
                let r = reach(cl);
                cl.x + r >= ox && cl.x - r < ox + chunk_ft && cl.y + r >= oy && cl.y - r < oy + chunk_ft
            })
            .collect();
        if !near.is_empty() {
            c.objects.retain(|o| {
                let (x, y) = (ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT);
                let kind = o.kind as u16;
                !near.iter().any(|cl| {
                    let d2 = (x - cl.x) * (x - cl.x) + (y - cl.y) * (y - cl.y);
                    let r = reach(cl);
                    match cl.kind {
                        Some(k) => k == kind && d2 <= r * r,
                        None => cl.r.is_some() && d2 <= r * r && (cl.kinds.is_empty() || cl.kinds.contains(&kind)),
                    }
                })
            });
        }
    }
    for p in e.objects.values() {
        if p.x < ox || p.y < oy || p.x >= ox + chunk_ft || p.y >= oy + chunk_ft {
            continue;
        }
        let (x, y) = (((p.x - ox) / SQUARE_FT) as f32, ((p.y - oy) / SQUARE_FT) as f32);
        let scale = if p.scale.is_finite() { p.scale.clamp(0.1, 10.0) } else { 1.0 };
        let rot = if p.rot.is_finite() { p.rot } else { 0.0 };
        let mut o = Object { kind: Kind::Crate, variant: p.variant, x, y, rot, scale, sprite: 0 };
        match &p.kind {
            ObjKind::Builtin(k) => match kind_of(*k) {
                Some(k) => o.kind = k,
                None => continue,
            },
            ObjKind::Sprite(_) => {
                let Some(id) = p.kind.sprite() else { continue };
                let at = match c.sprites.iter().position(|s| s.asset == id) {
                    Some(i) => i,
                    None => {
                        let m = e.sprites.get(id).cloned().unwrap_or_default();
                        let name = if m.name.is_empty() { "custom object".to_string() } else { m.name.clone() };
                        let info = KindInfo {
                            id: SPRITE_KIND + c.sprites.len() as u16,
                            name: "custom object",
                            radius: (m.size.clamp(0.1, 40.0) / 2.0),
                            blocks_move: m.blocks_move,
                            blocks_sight: m.blocks_sight,
                            cover: m.cover.min(3),
                            difficult: m.difficult,
                            height_ft: m.height_ft.max(0.0),
                            hazard: None,
                            feature: true,
                        };
                        c.sprites.push(ChunkSprite { asset: id.to_string(), name, info });
                        c.sprites.len() - 1
                    }
                };
                o.sprite = at as u16 + 1;
            }
        }
        c.objects.push(o);
    }
}

fn dominant_biome(biome: &[Biome], water: &[f32], height: &[f32]) -> Biome {
    let mut counts = [0usize; 17];
    for k in 0..biome.len() {
        if water[k] <= height[k] {
            counts[biome[k] as usize] += 1;
        }
    }
    let best = (0..17).max_by_key(|&b| (counts[b], 17 - b)).unwrap_or(8);
    Biome::from_u8(best as u8)
}

/// Apply a relief feature to a `dim`² grid whose (0, 0) is global square (gx0, gy0).
fn apply_relief(f: &Relief, height: &mut [f32], water: &[f32], gsx: i64, gsy: i64, dim: usize) {
    let reach = RELIEF_REACH + 1.0;
    let (x0, x1) = ((f.cx - reach) as i64 - gsx, (f.cx + reach) as i64 - gsx + 1);
    let (y0, y1) = ((f.cy - reach) as i64 - gsy, (f.cy + reach) as i64 - gsy + 1);
    for y in y0.max(0)..y1.min(dim as i64) {
        for x in x0.max(0)..x1.min(dim as i64) {
            let k = y as usize * dim + x as usize;
            if water[k] > height[k] - 1.0 {
                continue;
            }
            let v = relief_value(f, (gsx + x) as f64 + 0.5, (gsy + y) as f64 + 0.5);
            // Depressions never cut below the water table.
            height[k] = (height[k] as f64 + v).max(water[k] as f64 + 0.5) as f32;
        }
    }
}

fn retier(c: &mut Chunk, sea: f64) {
    for k in 0..c.height.len() {
        c.tier[k] = crate::core::floor((c.height[k] as f64 - sea) / TIER_FT) as i16;
    }
}

fn place_objects(c: &mut Chunk, biome: &[Biome], slope: &[f64], seed: u64, gsx: i64, gsy: i64) {
    let grove_seed = hash2(seed, 0x6007, 2);
    // Each (biome, kind) pair has its own jittered global lattice; objects belong to the
    // chunk that contains them, so placement is identical whichever chunk is asked.
    let mut kinds: Vec<(Biome, Kind, f64, bool)> = Vec::new();
    let mut seen = [false; 17];
    for &b in biome {
        if !seen[b as usize] {
            seen[b as usize] = true;
            for &(kd, dens, wet) in profile(b) {
                kinds.push((b, kd, dens, wet));
            }
        }
    }
    kinds.sort_by(|a, b| (a.0 as u8, a.1 as u16).cmp(&(b.0 as u8, b.1 as u16)));
    for (b, kind, dens, wet_ok) in kinds {
        // Twice the lattice density, thinned by a clustering field (keyed to world position):
        // things gather in patches and leave open ground between, at the same mean density.
        let spacing = 10.0 / crate::core::sqrt(dens);
        let cell = (spacing * 0.7).max(1.0);
        let lat_seed = hash3(seed, b as i64, kind as u16 as i64, 0x0b1);
        let clump_seed = hash3(grove_seed, b as i64, kind as u16 as i64, 0xc1);
        let (lx0, ly0) = (crate::core::floor(gsx as f64 / cell) as i64 - 1, crate::core::floor(gsy as f64 / cell) as i64 - 1);
        let (lx1, ly1) = (crate::core::floor((gsx + SQ as i64) as f64 / cell) as i64 + 1, crate::core::floor((gsy + SQ as i64) as f64 / cell) as i64 + 1);
        let is_tree = info(kind as u16).height_ft >= 15.0 && info(kind as u16).blocks_move;
        for ly in ly0..=ly1 {
            for lx in lx0..=lx1 {
                let h = hash2(lat_seed, lx, ly);
                let mut rng = Pcg32::new(h, 3);
                let px = (lx as f64 + rng.next_f64()) * cell;
                let py = (ly as f64 + rng.next_f64()) * cell;
                let (ix, iy) = (crate::core::floor(px) as i64 - gsx, crate::core::floor(py) as i64 - gsy);
                if ix < 0 || iy < 0 || ix >= SQ as i64 || iy >= SQ as i64 {
                    continue;
                }
                let k = iy as usize * SQ + ix as usize;
                if biome[k] != b || c.surface[k].is_road() || c.building[k] != 0 || c.urban[k] != 0 {
                    continue;
                }
                // Canopies and boulders keep clear of buildings (roofs stay visible).
                let reach = crate::core::ceil(info(kind as u16).radius as f64 * 1.25) as i64;
                let near_building = (-reach..=reach).any(|dy| {
                    (-reach..=reach).any(|dx| {
                        let (x, y) = (ix + dx, iy + dy);
                        x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && c.building[y as usize * SQ + x as usize] != 0
                    })
                });
                if near_building {
                    continue;
                }
                // Groves and glades for trees (one shared field, so species mix in a grove);
                // tighter patches of rocks and bushes.
                let clump = if is_tree { 0.5 + 0.5 * fbm(grove_seed, px / 34.0, py / 34.0, 3, 2.0, 0.5) } else { 0.5 + 0.5 * fbm(clump_seed, px / 13.0, py / 13.0, 3, 2.0, 0.55) };
                let p = 0.85 * 0.98 * smoothstep(0.32, 0.68, clump);
                if rng.next_f64() > p {
                    continue;
                }
                let depth = c.water_level[k] - c.height[k];
                let ok = if wet_ok { depth < 3.0 } else { depth <= 0.0 };
                if !ok || (is_tree && slope[k] > 0.8) {
                    continue;
                }
                c.objects.push(Object {
                    kind,
                    variant: (h >> 40) as u8 & 7,
                    x: (px - gsx as f64) as f32,
                    y: (py - gsy as f64) as f32,
                    rot: rng.range(0.0, std::f64::consts::TAU) as f32,
                    scale: rng.range(0.8, 1.25) as f32,
                    sprite: 0,
                });
            }
        }
    }
}

fn is_cover(o: &Object) -> bool {
    info(o.kind as u16).cover >= 1
}

/// Ground the guarantee counts: dry, bridge decks being exempt like the water they span (no
/// boulders on the bridge).
fn dry(c: &Chunk, k: usize) -> bool {
    c.water_level[k] <= c.height[k] && c.surface[k] != Surface::Planks
}

/// Where props may go: dry ground that is not a building, road bed or bridge.
fn placeable(c: &Chunk, k: usize) -> bool {
    dry(c, k) && c.building[k] == 0 && c.urban[k] != URBAN_ENTRANCE && c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] == 0
}

/// The 32x32 windows (top-left squares) whose dry ground is all one tier, leaving out mostly-water
/// ones: the battlemap guarantee as `tests/vital.rs` checks it.
fn flat_windows(c: &Chunk) -> Vec<(usize, usize)> {
    const W: usize = 32;
    let n = SQ - W + 1;
    // Per row, each run of W squares: lowest and highest dry tier, and how many are wet.
    let (mut lo, mut hi, mut wet) = (vec![i16::MAX; SQ * n], vec![i16::MIN; SQ * n], vec![0usize; SQ * n]);
    for y in 0..SQ {
        for x in 0..n {
            for k in y * SQ + x..y * SQ + x + W {
                if dry(c, k) {
                    lo[y * n + x] = lo[y * n + x].min(c.tier[k]);
                    hi[y * n + x] = hi[y * n + x].max(c.tier[k]);
                }
                wet[y * n + x] += (c.water_level[k] > c.height[k]) as usize;
            }
        }
    }
    let mut out = Vec::new();
    for wy in 0..n {
        for wx in 0..n {
            let (mut a, mut b, mut w) = (i16::MAX, i16::MIN, 0);
            for y in wy..wy + W {
                (a, b, w) = (a.min(lo[y * n + wx]), b.max(hi[y * n + wx]), w + wet[y * n + wx]);
            }
            if a == b && w * 2 < W * W {
                out.push((wx, wy));
            }
        }
    }
    out
}

fn enforce(c: &mut Chunk, biome: &[Biome], seed: u64, gsx: i64, gsy: i64, sea: f64) {
    let mut rng = Pcg32::new(hash3(seed, gsx, gsy, 0xe4f), 9);

    // Elevation: every 16x16 cell spans at least two tiers.
    let mut deferred = false;
    for cy in 0..SQ / TIER_CELL {
        for cx in 0..SQ / TIER_CELL {
            let cells: Vec<usize> = (0..TIER_CELL * TIER_CELL).map(|q| (cy * TIER_CELL + q / TIER_CELL) * SQ + cx * TIER_CELL + q % TIER_CELL).collect();
            if cells.iter().all(|&k| !dry(c, k)) {
                continue;
            }
            // Tiers of the ground you can stand on (a shallows bed doesn't count).
            let t0 = c.tier[*cells.iter().find(|&&k| dry(c, k)).unwrap()];
            if cells.iter().any(|&k| dry(c, k) && c.tier[k] != t0) {
                continue;
            }
            // Mostly open plaza paving: left to the windows below.
            let dry_n = cells.iter().filter(|&&k| dry(c, k)).count();
            if cells.iter().filter(|&&k| dry(c, k) && c.urban[k] == URBAN_PLAZA).count() * 2 >= dry_n {
                deferred = true;
                continue;
            }
            raise(c, &cells, &mut rng, gsx, gsy, sea);
        }
    }
    // Plaza paving answers to the guarantee itself rather than cell by cell: every 32x32 window
    // spans two tiers. A square's own features (laid out sparingly in a pattern on a city's
    // square: `plaza_features`) usually see to that; a window still flat gets one in its middle,
    // or anywhere in it if that fails.
    let mut tried: Vec<(usize, usize)> = Vec::new();
    for _ in 0..if deferred { 32 } else { 0 } {
        let Some(&(wx, wy)) = flat_windows(c).iter().find(|w| tried.iter().filter(|t| *t == *w).count() < 2) else { break };
        let span = if tried.contains(&(wx, wy)) { (0, 32) } else { (8, 16) };
        tried.push((wx, wy));
        let cells: Vec<usize> = (0..span.1 * span.1).map(|q| (wy + span.0 + q / span.1) * SQ + wx + span.0 + q % span.1).collect();
        raise(c, &cells, &mut rng, gsx, gsy, sea);
    }


    // Cover: every 8x8 cell has at least one cover object.
    for cy in 0..SQ / COVER_CELL {
        for cx in 0..SQ / COVER_CELL {
            let (x0, y0) = ((cx * COVER_CELL) as f32, (cy * COVER_CELL) as f32);
            let inside = |o: &Object| o.x >= x0 && o.x < x0 + COVER_CELL as f32 && o.y >= y0 && o.y < y0 + COVER_CELL as f32;
            let square = |q: usize| (cy * COVER_CELL + q / COVER_CELL) * SQ + cx * COVER_CELL + q % COVER_CELL;
            // Building walls are cover too.
            if c.objects.iter().any(|o| is_cover(o) && inside(o)) || (0..COVER_CELL * COVER_CELL).any(|q| c.building[square(q)] != 0) {
                continue;
            }
            let all: Vec<usize> = (0..COVER_CELL * COVER_CELL).map(square).filter(|&k| dry(c, k)).collect();
            let off_road: Vec<usize> = all.iter().copied().filter(|&k| placeable(c, k)).collect();
            let cells = if off_road.is_empty() { all } else { off_road };
            if cells.is_empty() {
                continue;
            }
            let k = cells[rng.below(cells.len() as u32) as usize];
            let kind = if c.urban[k] != 0 { town_cover(c.urban[k], &mut rng) } else { fallback_cover(biome[k], &mut rng) };
            push_at(c, kind, k, &mut rng);
        }
    }

    // Max gap: no dry square farther than MAX_COVER_GAP from cover.
    for _ in 0..64 {
        let dist = cover_distance(c);
        let far = (0..SQ * SQ).filter(|&k| dry(c, k) && dist[k] > MAX_COVER_GAP).max_by_key(|&k| (dist[k], std::cmp::Reverse(k)));
        match far {
            Some(k) => {
                // On a road, put the cover on the nearest verge instead.
                let k = if !placeable(c, k) { nearest_verge(c, k).unwrap_or(k) } else { k };
                let kind = if c.urban[k] != 0 { town_cover(c.urban[k], &mut rng) } else { fallback_cover(biome[k], &mut rng) };
                push_at(c, kind, k, &mut rng);
            }
            None => break,
        }
    }

    // Something to interact with: hazard/prop per 16x16 cell unless the chunk has atmosphere.
    if c.atmosphere == Atmosphere::None {
        for cy in 0..SQ / TIER_CELL {
            for cx in 0..SQ / TIER_CELL {
                let (x0, y0) = ((cx * TIER_CELL) as f32, (cy * TIER_CELL) as f32);
                let has = c.objects.iter().any(|o| {
                    info(o.kind as u16).feature && o.x >= x0 && o.x < x0 + TIER_CELL as f32 && o.y >= y0 && o.y < y0 + TIER_CELL as f32
                });
                if has {
                    continue;
                }
                let all: Vec<usize> =
                    (0..TIER_CELL * TIER_CELL).map(|q| (cy * TIER_CELL + q / TIER_CELL) * SQ + cx * TIER_CELL + q % TIER_CELL).filter(|&k| dry(c, k)).collect();
                let off_road: Vec<usize> = all.iter().copied().filter(|&k| placeable(c, k)).collect();
                let cells = if off_road.is_empty() { all } else { off_road };
                if cells.is_empty() {
                    continue;
                }
                let k = cells[rng.below(cells.len() as u32) as usize];
                let kind = if c.urban[k] != 0 { town_cover(c.urban[k], &mut rng) } else { fallback_feature(biome[k], &mut rng) };
                push_at(c, kind, k, &mut rng);
            }
        }
    }
}

/// Make a flat stretch (`cells`) span two tiers: a natural knoll on open ground, a round built
/// feature on paving, at a dry spot far enough inside the chunk that it is never clipped at the
/// chunk border (chunks are generated independently).
fn raise(c: &mut Chunk, cells: &[usize], rng: &mut Pcg32, gsx: i64, gsy: i64, sea: f64) {
    let interior = |k: usize| {
        let (x, y) = (k % SQ, k / SQ);
        x >= 7 && y >= 7 && x < SQ - 7 && y < SQ - 7
    };
    // Open country and fields take a natural knoll; streets and plazas a built feature.
    let good: Vec<usize> = cells.iter().copied().filter(|&k| placeable(c, k) && interior(k) && matches!(c.urban[k], 0 | URBAN_FIELD)).collect();
    let town: Vec<usize> = cells.iter().copied().filter(|&k| placeable(c, k) && interior(k)).collect();
    // Paved ground (streets, plazas) gets a round built feature (a dais, a fountain, a
    // planter, a market cross) clear of stalls and props, rather than a knoll.
    if good.is_empty() && !town.is_empty() {
        let mut taken = vec![false; SQ * SQ];
        for o in &c.objects {
            let r = info(o.kind as u16).radius * o.scale + 0.5;
            for y in (o.y - r).max(0.0) as usize..=((o.y + r) as usize).min(SQ - 1) {
                for x in (o.x - r).max(0.0) as usize..=((o.x + r) as usize).min(SQ - 1) {
                    taken[y * SQ + x] = true;
                }
            }
        }
        let base = c.tier[town[0]];
        let free = |k: usize| placeable(c, k) && interior(k) && matches!(c.urban[k], URBAN_STREET | URBAN_PLAZA) && !taken[k] && c.tier[k] == base;
        // Squares (offsets) within a round of radius r squares about a square's centre.
        let disc = |r: f64| -> Vec<(i64, i64)> {
            let n = r.floor() as i64;
            (-n..=n).flat_map(|dy| (-n..=n).map(move |dx| (dx, dy))).filter(|&(dx, dy)| ((dx * dx + dy * dy) as f64) <= r * r).collect()
        };
        let mut placed = None;
        'size: for r in [2.3, 1.5, 1.0, 0.5] {
            let offs = disc(r);
            let start = rng.below(town.len() as u32) as usize;
            for q in 0..town.len() {
                let k = town[(start + q) % town.len()];
                let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
                let ok = offs.iter().all(|&(dx, dy)| {
                    let (i, j) = (x + dx, y + dy);
                    i >= 0 && j >= 0 && (i as usize) < SQ && (j as usize) < SQ && free(j as usize * SQ + i as usize)
                });
                if ok {
                    placed = Some((k, r, offs));
                    break 'size;
                }
            }
        }
        if let Some((k, r, offs)) = placed {
            let top = ((base as f64 + 1.2) * TIER_FT + sea) as f32;
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            for (dx, dy) in offs {
                c.height[(y + dy) as usize * SQ + (x + dx) as usize] = top;
            }
            let kind = if r < 1.5 {
                if rng.next_f64() < 0.5 { PlazaFeature::Cross } else { PlazaFeature::Dais }
            } else {
                [PlazaFeature::Dais, PlazaFeature::Fountain, PlazaFeature::Planter, PlazaFeature::Cross][rng.below(4) as usize]
            };
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let rv = r as f32 + 0.45;
            let ang = rng.range(0.0, std::f64::consts::TAU) as f32;
            c.structures.push(VectorShape { kind: ShapeKind::Dais, size: kind as u8 as f32, pts: vec![[cx, cy], [cx + rv * ang.cos(), cy + rv * ang.sin()]] });
            if kind == PlazaFeature::Planter {
                c.objects.push(Object { kind: Kind::TreeDeciduous, variant: rng.below(8) as u8, x: cx, y: cy, rot: 0.0, scale: 0.9, sprite: 0 });
            }
            retier(c, sea);
            return;
        }
    }
    let pool: &[usize] = if !good.is_empty() { &good } else if !town.is_empty() { &town } else { cells };
    let spot = pool[rng.below(pool.len() as u32) as usize];
    let (sx, sy) = ((spot % SQ) as f64 + 0.5, (spot / SQ) as f64 + 0.5);
    let rise = TIER_FT * rng.range(1.2, 2.0);
    let f = Relief {
        cx: sx + gsx as f64,
        cy: sy + gsy as f64,
        r: rng.range(2.5, 3.5),
        h: rise,
        steep: rng.range(0.4, 0.9),
        dir: {
            let a = rng.range(0.0, std::f64::consts::TAU);
            (libm::cos(a), libm::sin(a))
        },
        elong: rng.range(1.0, 1.4),
        wobble: [(rng.range(0.0, 0.2), rng.range(0.0, 6.28)), (rng.range(0.0, 0.12), rng.range(0.0, 6.28)), (0.0, 0.0)],
    };
    apply_relief(&f, &mut c.height, &c.water_level, gsx, gsy, SQ);
    // Guarantee even if the relief was clipped: lift the spot itself a full tier.
    c.height[spot] = c.height[spot].max(((c.tier[spot] as f64 + 1.2) * TIER_FT + sea) as f32);
    retier(c, sea);
}

/// Round raised features on town squares (the battlemap draws each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlazaFeature {
    /// A round stone dais with steps (a speaker's platform).
    Dais = 0,
    /// A fountain: a raised stone basin with a central jet.
    Fountain = 1,
    /// A raised round planter with a tree in it.
    Planter = 2,
    /// A market cross on a stepped round plinth.
    Cross = 3,
}

/// A town plaza's market as a little town of its own: jittered seeds inside a wobbly oval round
/// the well make Voronoi cells, each pulled back from its neighbours so lanes run between them
/// (streets for players and passers-by). Each block's outline is rounded and wobbled, and its
/// stalls follow that curve shoulder to shoulder, facing out onto the lanes, with a gap to get
/// in; crates and barrels stack inside. The cell at the well (and a few others) stays an open
/// square, as does any cell holding a monument; stalls keep clear of the features (`keep`: centre
/// and radius, laid out first on a city's square). A pure function of the plaza (world ft), so
/// every chunk lays out the same market.
fn market_blocks(poly: &[[f64; 2]], monuments: &[Vec<[f64; 2]>], keep: &[([f64; 2], f64)], seed: u64, out: &mut Vec<([f64; 2], f32, Kind)>) {
    use crate::town::geom::{add, area, centroid, clip_plain, contains, dist, dot, len, main_axis, mul, sub};
    let c = centroid(poly);
    let mut rng = Pcg32::new(hash2(seed ^ 0x3a2c, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 11);
    // Blocks follow the plaza's long axis, a little askew.
    let eu0 = main_axis(poly);
    let skew = rng.range(-0.15, 0.15);
    let (cs, sn) = (libm::cos(skew), libm::sin(skew));
    let eu = [eu0[0] * cs - eu0[1] * sn, eu0[0] * sn + eu0[1] * cs];
    let ev = [-eu[1], eu[0]];
    let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for q in poly {
        let d = sub(*q, c);
        let (u, v) = (dot(d, eu), dot(d, ev));
        (u0, u1, v0, v1) = (u0.min(u), u1.max(u), v0.min(v), v1.max(v));
    }
    // On a great square the market keeps to a block round the well; the rest stays open.
    let (hu, hv) = (rng.range(110.0, 150.0), rng.range(80.0, 110.0));
    (u0, u1, v0, v1) = (u0.max(-hu), u1.min(hu), v0.max(-hv), v1.min(hv));
    if u1 - u0 < 40.0 || v1 - v0 < 30.0 {
        return;
    }
    let at = |u: f64, v: f64| add(c, add(mul(eu, u), mul(ev, v)));
    // Seeds on a jittered grid about 50 ft apart.
    let pitch = 50.0;
    let (nu, nv) = (((u1 - u0) / pitch).ceil() as i64, ((v1 - v0) / pitch).ceil() as i64);
    let mut seeds: Vec<[f64; 2]> = Vec::new();
    for j in 0..nv {
        for i in 0..nu {
            let u = u0 + (i as f64 + 0.5) * (u1 - u0) / nu as f64 + rng.range(-13.0, 13.0);
            let v = v0 + (j as f64 + 0.5) * (v1 - v0) / nv as f64 + rng.range(-13.0, 13.0);
            seeds.push(at(u, v));
        }
    }
    let well = (0..seeds.len()).min_by(|&a, &b| dist(seeds[a], c).total_cmp(&dist(seeds[b], c)));
    // The market's edge: a wobbly oval, not a box.
    let (mu, mv, ru, rv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0, (u1 - u0) / 2.0, (v1 - v0) / 2.0);
    let (ph1, ph2) = (rng.range(0.0, 6.28), rng.range(0.0, 6.28));
    let bounds: Vec<[f64; 2]> = (0..28)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / 28.0;
            let w = 1.0 + 0.07 * libm::sin(3.0 * t + ph1) + 0.04 * libm::sin(5.0 * t + ph2);
            at(mu + ru * w * libm::cos(t) * 1.12, mv + rv * w * libm::sin(t) * 1.12)
        })
        .collect();
    // Monuments on the square (a statue on its plinth) and its features keep a 9-ft walk round
    // them clear; a cell holding a monument stays open.
    let plinths: Vec<([f64; 2], f64)> = monuments.iter().map(|m| (centroid(m), m.iter().map(|q| dist(*q, centroid(m))).fold(0.0, f64::max))).collect();
    let clear = |p: [f64; 2], r: f64| plinths.iter().chain(keep).all(|&(mc, mr)| dist(p, mc) > mr + 9.0 + r);
    let half_lane = 5.0;
    for (i, &s) in seeds.iter().enumerate() {
        // Open squares: the well's cell and a few others (decided first, so the rng runs the
        // same whatever the cell's shape).
        let open = Some(i) == well || rng.next_f64() < 0.12;
        let mut cell = bounds.clone();
        for (j, &o) in seeds.iter().enumerate() {
            if i == j || dist(s, o) > 3.0 * pitch {
                continue;
            }
            let d = sub(o, s);
            let n = mul(d, 1.0 / len(d));
            cell = clip_plain(&cell, n, dot(add(s, mul(d, 0.5)), n) - half_lane);
            if cell.len() < 3 {
                break;
            }
        }
        if open || cell.len() < 3 || area(&cell).abs() < 250.0 || plinths.iter().any(|&(mc, _)| contains(&cell, mc)) {
            continue;
        }
        // Round the corners (Chaikin), then wobble the outline in and out about the middle.
        let mid = centroid(&cell);
        let mut outline = cell.clone();
        for _ in 0..3 {
            let m = outline.len();
            outline = (0..m).flat_map(|e| {
                let (a, b) = (outline[e], outline[(e + 1) % m]);
                [add(mul(a, 0.75), mul(b, 0.25)), add(mul(a, 0.25), mul(b, 0.75))]
            }).collect();
        }
        let (wa, wk, wp) = (rng.range(0.04, 0.1), 2.0 + rng.below(3) as f64, rng.range(0.0, 6.28));
        for q in &mut outline {
            let d = sub(*q, mid);
            let ang = libm::atan2(d[1], d[0]);
            *q = add(mid, mul(d, 1.0 + wa * libm::sin(wk * ang + wp)));
        }
        let ccw = area(&outline) > 0.0;
        let m = outline.len();
        let seg: Vec<f64> = (0..m).map(|e| dist(outline[e], outline[(e + 1) % m])).collect();
        let total: f64 = seg.iter().sum();
        let count = (total / 9.2).floor() as usize;
        if count < 3 {
            continue;
        }
        let step = total / count as f64;
        // A way in: two stalls left out somewhere round the ring.
        let gap = if count >= 8 { Some(rng.below(count as u32) as usize) } else { None };
        let (mut e, mut acc) = (0usize, 0.0);
        for k in 0..count {
            let want = (k as f64 + 0.5) * step;
            while e < m - 1 && acc + seg[e] < want {
                acc += seg[e];
                e += 1;
            }
            if gap.is_some_and(|g| k == g || k == (g + 1) % count) || rng.next_f64() < 0.04 {
                continue;
            }
            let (a, b) = (outline[e], outline[(e + 1) % m]);
            let t = mul(sub(b, a), 1.0 / seg[e].max(1e-9));
            let on = add(a, mul(t, want - acc));
            // Outward normal: the stall's front, onto the lane.
            let n = if ccw { [t[1], -t[0]] } else { [-t[1], t[0]] };
            let p = add(on, mul(n, -3.5));
            let fits = [-4.5, 4.5].iter().all(|&o| contains(poly, add(p, mul(t, o)))) && clear(p, 5.0);
            if !fits {
                continue;
            }
            // Sprites face south (+y) unturned.
            let rot = (libm::atan2(n[1], n[0]) - std::f64::consts::FRAC_PI_2 + rng.range(-0.08, 0.08)) as f32;
            let kind = if rng.next_f64() < 0.05 { Kind::Cart } else { Kind::MarketStall };
            out.push((p, rot, kind));
        }
        // Stock in the block's back.
        if area(&cell).abs() > 500.0 && contains(poly, mid) && clear(mid, 4.0) {
            for _ in 0..1 + rng.below(3) {
                let p = add(mid, [rng.range(-4.0, 4.0), rng.range(-4.0, 4.0)]);
                let kind = if rng.next_f64() < 0.5 { Kind::Crate } else { Kind::Barrel };
                out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind));
            }
        }
    }
}

/// Round raised features on a town square, on open paving: fountains, planters with a tree,
/// market crosses and speakers' daises, clear of the well, monuments, the market's stalls, the
/// square's edge, its streets and roads (`lanes`: segment and half width) and each other. A pure
/// function of the plaza (world ft).
///
/// A town scatters them (about one per 7,000 sq ft, up to six). A city lays them out sparingly in
/// a pattern round the well, square to the plaza's long axis (`patterned`): a pair of fountains on
/// the axis, four trees round the well, or fountains on the long axis and crosses (or daises) on
/// the short one. Every symmetric group goes in whole or not at all. Open paving must still span
/// two tiers in every 32-square window (`enforce`), so where some part of the square is more than
/// 70 ft from a raised feature, more trees go in, mirrored across both axes.
fn plaza_features(
    poly: &[[f64; 2]],
    monuments: &[Vec<[f64; 2]>],
    stalls: &[([f64; 2], f32, Kind)],
    lanes: &[([f64; 2], [f64; 2], f64)],
    seed: u64,
    patterned: bool,
    out: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>,
) {
    use crate::town::geom::{add, area, centroid, contains, dist, dot, main_axis, mul, seg_dist, sub};
    use std::f64::consts::TAU;
    let c = centroid(poly);
    let mut rng = Pcg32::new(hash2(seed ^ 0xf0a7, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 13);
    let first = out.len();
    let fits = |out: &[([f64; 2], f64, PlazaFeature, f64)], p: [f64; 2], r: f64| {
        (0..12).all(|k| {
            let t = TAU * k as f64 / 12.0;
            contains(poly, add(p, [(r + 6.0) * libm::cos(t), (r + 6.0) * libm::sin(t)]))
        }) && dist(p, c) > r + 22.0
            && monuments.iter().all(|m| {
                let mc = centroid(m);
                dist(p, mc) > r + 8.0 + m.iter().map(|q| dist(*q, mc)).fold(0.0, f64::max)
            })
            && stalls.iter().all(|s| dist(p, s.0) > r + 10.0)
            && lanes.iter().all(|&(a, b, w)| seg_dist(p, a, b) > r + w + 4.0)
            && out[first..].iter().all(|f| dist(p, f.0) > r + f.1 + 14.0)
    };
    if !patterned {
        let want = ((area(poly).abs() / 7000.0) as usize).min(6);
        let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        let mut placed = 0;
        for _ in 0..60 {
            if placed >= want {
                break;
            }
            let p = [rng.range(x0, x1), rng.range(y0, y1)];
            let roll = rng.next_f64();
            let (kind, r) = if roll < 0.35 {
                (PlazaFeature::Fountain, rng.range(9.0, 13.0))
            } else if roll < 0.65 {
                (PlazaFeature::Planter, rng.range(6.0, 9.0))
            } else if roll < 0.85 {
                (PlazaFeature::Cross, rng.range(5.0, 7.0))
            } else {
                (PlazaFeature::Dais, rng.range(7.0, 10.0))
            };
            let ang = rng.range(0.0, TAU);
            if fits(&out[..], p, r) {
                out.push((p, r, kind, ang));
                placed += 1;
            }
        }
        return;
    }
    // Patterned: a point `u` along the plaza's long axis from the well and `v` across it.
    let eu = main_axis(poly);
    let ev = [-eu[1], eu[0]];
    let at = |u: f64, v: f64| add(c, add(mul(eu, u), mul(ev, v)));
    // A symmetric group, whole or not at all; each faces the well (a dais's steps come down
    // toward it).
    let whole = |out: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>, pts: &[[f64; 2]], r: f64, kind: PlazaFeature| {
        let start = out.len();
        for &p in pts {
            if !fits(&out[..], p, r) {
                out.truncate(start);
                return false;
            }
            out.push((p, r, kind, libm::atan2(c[1] - p[1], c[0] - p[0])));
        }
        true
    };
    // How far the square reaches along each axis from the well.
    let (mut ru, mut rv) = (0.0f64, 0.0f64);
    for q in poly {
        let d = sub(*q, c);
        (ru, rv) = (ru.max(dot(d, eu).abs()), rv.max(dot(d, ev).abs()));
    }
    // Distances to try, nearest a preferred fraction of the reach first.
    let around = |reach: f64, frac: f64| {
        let mut ds: Vec<f64> = (0..).map(|k| 30.0 + 6.0 * k as f64).take_while(|&d| d < reach).collect();
        ds.sort_by(|a, b| (a - reach * frac).abs().total_cmp(&(b - reach * frac).abs()));
        ds
    };
    let (r_tree, r_fountain, r_cross, r_dais) = (rng.range(6.0, 8.0), rng.range(9.0, 12.0), rng.range(5.0, 6.5), rng.range(7.0, 9.0));
    let minor = if rng.next_f64() < 0.6 { (PlazaFeature::Cross, r_cross) } else { (PlazaFeature::Dais, r_dais) };
    let style = rng.below(3);
    for attempt in 0..3 {
        match (style + attempt) % 3 {
            // A fountain either side of the well on the long axis.
            0 => {
                for u in around(ru, 0.4) {
                    if whole(out, &[at(u, 0.0), at(-u, 0.0)], r_fountain, PlazaFeature::Fountain) {
                        break;
                    }
                }
            }
            // Four trees round the well, square to the axes.
            1 => {
                for d in around(ru.min(rv), 0.35) {
                    if whole(out, &[at(d, d), at(-d, d), at(d, -d), at(-d, -d)], r_tree, PlazaFeature::Planter) {
                        break;
                    }
                }
            }
            // Fountains on the long axis, crosses or daises on the short one.
            _ => {
                for u in around(ru, 0.45) {
                    if whole(out, &[at(u, 0.0), at(-u, 0.0)], r_fountain, PlazaFeature::Fountain) {
                        for v in around(rv, 0.45) {
                            if whole(out, &[at(0.0, v), at(0.0, -v)], minor.1, minor.0) {
                                break;
                            }
                        }
                        break;
                    }
                }
            }
        }
        if out.len() > first {
            break;
        }
    }
    // Reach: no point of the square farther than 70 ft (along the map's axes, as the battlemap's
    // windows run) from a raised feature. The worst-served spot gets a mirrored group of trees
    // nearby; a spot nothing fits near is left to `enforce`.
    let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let mut samples: Vec<[f64; 2]> = Vec::new();
    let mut y = y0 + 5.0;
    while y < y1 {
        let mut x = x0 + 5.0;
        while x < x1 {
            if contains(poly, [x, y]) {
                samples.push([x, y]);
            }
            x += 10.0;
        }
        y += 10.0;
    }
    let reach = |out: &[([f64; 2], f64, PlazaFeature, f64)], p: [f64; 2]| out[first..].iter().map(|f| (f.0[0] - p[0]).abs().max((f.0[1] - p[1]).abs())).fold(f64::MAX, f64::min);
    let mut given_up: Vec<[f64; 2]> = Vec::new();
    for _ in 0..16 {
        let worst = samples
            .iter()
            .filter(|&&s| given_up.iter().all(|g| dist(*g, s) > 40.0))
            .map(|&s| (reach(&out[..], s), s))
            .filter(|&(d, _)| d > 70.0)
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, s)) = worst else { break };
        let mut spots: Vec<[f64; 2]> = (0..81).map(|q| [s[0] + ((q % 9) as f64 - 4.0) * 8.0, s[1] + ((q / 9) as f64 - 4.0) * 8.0]).collect();
        spots.sort_by(|a, b| dist(*a, s).total_cmp(&dist(*b, s)));
        let placed = spots.into_iter().any(|p| {
            let d = sub(p, c);
            let (u, v) = (dot(d, eu), dot(d, ev));
            let near = r_tree + 7.0;
            let pts: Vec<[f64; 2]> = match (u.abs() < near, v.abs() < near) {
                (true, true) => return false,
                (true, false) => vec![at(0.0, v), at(0.0, -v)],
                (false, true) => vec![at(u, 0.0), at(-u, 0.0)],
                _ => vec![at(u, v), at(-u, v), at(u, -v), at(-u, -v)],
            };
            whole(out, &pts, r_tree, PlazaFeature::Planter)
        });
        if !placed {
            given_up.push(s);
        }
    }
}

/// Nearest dry, non-road square within a few squares (by ring), for roadside placements.
fn nearest_verge(c: &Chunk, k: usize) -> Option<usize> {
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    for r in 1..=4i64 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                    continue;
                }
                let nk = ny as usize * SQ + nx as usize;
                let road_bed = c.road_halo[(nk / SQ + 1) * HS + nk % SQ + 1] != 0;
                if !road_bed && c.building[nk] == 0 && c.water_level[nk] <= c.height[nk] {
                    return Some(nk);
                }
            }
        }
    }
    None
}

/// Cover in town (streets, plazas) or on fields.
fn town_cover(urban: u8, rng: &mut Pcg32) -> Kind {
    let opts: &[Kind] = match urban {
        URBAN_FIELD => &[Kind::Haystack],
        URBAN_PLAZA => &[Kind::Crate, Kind::Barrel, Kind::Cart],
        URBAN_GRAVEYARD => &[Kind::Headstone],
        URBAN_RUIN => &[Kind::RuinWall, Kind::RockPile],
        _ => &[Kind::Crate, Kind::Barrel, Kind::Cart],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

/// Street life: barrels and crates against walls, haystacks in fields (market stalls stand in
/// blocks: `market_blocks`).
/// Keyed by global square so the result does not depend on which chunk asks.
fn place_town_props(c: &mut Chunk, seed: u64, gsx: i64, gsy: i64) {
    let pseed = hash2(seed, 0x70e5, 3);
    for k in 0..SQ * SQ {
        let urb = c.urban[k];
        if urb == 0 || c.building[k] != 0 || c.water_level[k] > c.height[k] || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 {
            continue;
        }
        let (i, j) = (k % SQ, k / SQ);
        let h = hash2(pseed, gsx + i as i64, gsy + j as i64);
        let r = unit(h);
        let by_wall = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
            let (x, y) = (i as i64 + dx, j as i64 + dy);
            x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && c.building[y as usize * SQ + x as usize] != 0
        });
        let kind = match urb {
            URBAN_STREET if by_wall && r < 0.035 => Some(if (h >> 20) & 1 == 0 { Kind::Barrel } else { Kind::Crate }),
            // Headstones in rows (every other square, most of them).
            URBAN_GRAVEYARD if (gsx + i as i64) % 2 == 0 && (gsy + j as i64) % 3 == 0 && r < 0.8 => Some(Kind::Headstone),
            URBAN_FIELD if r < 0.004 => Some(Kind::Haystack),
            _ => None,
        };
        if let Some(kind) = kind {
            let mut rng = Pcg32::new(h, 7);
            push_at(c, kind, k, &mut rng);
        }
    }
}

fn push_at(c: &mut Chunk, kind: Kind, k: usize, rng: &mut Pcg32) {
    let mut o = Object {
        kind,
        variant: rng.below(8) as u8,
        x: (k % SQ) as f32 + rng.range(0.25, 0.75) as f32,
        y: (k / SQ) as f32 + rng.range(0.25, 0.75) as f32,
        rot: rng.range(0.0, std::f64::consts::TAU) as f32,
        scale: rng.range(0.85, 1.2) as f32,
        sprite: 0,
    };
    // Tall props draw above roofs, so they must not reach over a building (or past the chunk
    // edge, where a neighbour's building may stand): try it smaller and centred, else a crate.
    if info(kind as u16).height_ft >= 5.0 && !clear_of_buildings(c, &o) {
        o.x = (k % SQ) as f32 + 0.5;
        o.y = (k / SQ) as f32 + 0.5;
        o.scale = 0.85;
        if !clear_of_buildings(c, &o) {
            o.kind = Kind::Crate;
        }
    }
    c.objects.push(o);
}

/// Whether an object's footprint disc stays inside the chunk and off building squares.
fn clear_of_buildings(c: &Chunk, o: &Object) -> bool {
    let r = info(o.kind as u16).radius * o.scale;
    if o.x - r < 0.0 || o.y - r < 0.0 || o.x + r > SQ as f32 || o.y + r > SQ as f32 {
        return false;
    }
    for y in (o.y - r) as usize..=((o.y + r) as usize).min(SQ - 1) {
        for x in (o.x - r) as usize..=((o.x + r) as usize).min(SQ - 1) {
            // Nearest point of the square to the disc centre.
            let dx = o.x - o.x.clamp(x as f32, x as f32 + 1.0);
            let dy = o.y - o.y.clamp(y as f32, y as f32 + 1.0);
            if dx * dx + dy * dy < r * r && c.building[y * SQ + x] != 0 {
                return false;
            }
        }
    }
    true
}

/// Chebyshev-ish BFS distance (squares) from every square to the nearest cover object.
pub fn cover_distance(c: &Chunk) -> Vec<i32> {
    let mut dist = vec![i32::MAX; SQ * SQ];
    let mut queue = std::collections::VecDeque::new();
    for k in 0..SQ * SQ {
        if c.building[k] != 0 || matches!(c.urban[k], URBAN_WALL | URBAN_TOWER) {
            dist[k] = 0;
            queue.push_back(k);
        }
    }
    for o in c.objects.iter().filter(|o| c.info(o).cover >= 1) {
        let (x, y) = (o.x as i64, o.y as i64);
        if x >= 0 && y >= 0 && (x as usize) < SQ && (y as usize) < SQ {
            let k = y as usize * SQ + x as usize;
            if dist[k] != 0 {
                dist[k] = 0;
                queue.push_back(k);
            }
        }
    }
    while let Some(k) = queue.pop_front() {
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                continue;
            }
            let nk = ny as usize * SQ + nx as usize;
            if dist[nk] > dist[k] + 1 {
                dist[nk] = dist[k] + 1;
                queue.push_back(nk);
            }
        }
    }
    dist
}

fn compute_edges(c: &mut Chunk) {
    for j in 0..SQ {
        for i in 0..SQ {
            let k = j * SQ + i;
            let mut e = 0u8;
            for (bit_cliff, bit_slope, nk) in [(1u8, 4u8, (i + 1 < SQ).then(|| k + 1)), (2u8, 8u8, (j + 1 < SQ).then(|| k + SQ))] {
                // Building walls are implied by the building squares, and walls, towers and
                // decks are drawn as vectors: none of them are drawn as cliffs.
                let raised = |k: usize| c.building[k] != 0 || matches!(c.urban[k], URBAN_DECK | URBAN_WALL | URBAN_TOWER) || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] & 0x80 != 0;
                if let Some(nk) = nk.filter(|&nk| !raised(k) && !raised(nk)) {
                    if c.tier[nk] != c.tier[k] {
                        let dh = (c.height[nk] - c.height[k]).abs();
                        e |= if dh >= 4.0 { bit_cliff } else { bit_slope };
                    }
                }
            }
            if matches!(c.surface[k], Surface::Shallow | Surface::Mud) {
                e |= 16;
            }
            c.edges[k] = e;
        }
    }
    for o in &c.objects {
        let inf = c.info(o);
        if inf.difficult {
            let r = (inf.radius * o.scale) as i64;
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (o.x as i64 + dx, o.y as i64 + dy);
                    if x >= 0 && y >= 0 && (x as usize) < SQ && (y as usize) < SQ {
                        c.edges[y as usize * SQ + x as usize] |= 16;
                    }
                }
            }
        }
    }
}

/// Binary payload for the renderer (little-endian):
///   0 magic "BATL" | 4 level | 8 x | 12 y | 16 n_objects | 20 atmosphere | 24 sea f32 | 28 reserved
///   32 height f32[HS²] | tier i16[SQ²] | surface u8[SQ²] | edges u8[SQ²] | water f32[HS²]
///   (height and water include the one-square halo ring)
///   objects: (kind u16, variant u8, 0 u8, x f32, y f32, rot f32, scale f32) × n
///   road halo u8[HS²]: road class + 1 (0 = none), 0x80 = bridge deck
///   building u16[SQ²]: index + 1 of the building on each square (0 = none)
///   buildings: count u32, then per building: n u8, floors u8, roof u8 (style: 0 hip,
///   1 battlements, 2 cone; | (tint + 1) << 2, 0 = picked),
///   corner-tower side u8 (squares), then n × (x, y) f32 in
///   chunk squares (heights above are the ground; roofs are drawn from these outlines)
///   structures: count u32, then per shape: kind u8 (`ShapeKind`), n u8, 0, 0, size f32,
///   then n × (x, y) f32 in chunk squares (drawn as vectors; decks are not in the road halo)
///   raised u8[SQ² × 2]: per square, height (ft, capped 255) of what stands on the drawn
///   ground (roof, wall walk, tower top, deck) and its `URBAN_*` kind
///   sprites: count u32, then per uploaded sprite (object kind `SPRITE_KIND` + index): id length
///   u8, cover u8, flags u8 (1 blocks movement, 2 blocks sight, 4 difficult), 0, radius f32
///   (squares), height f32 (ft), then the asset id (ASCII)
pub fn pack(world: &World, c: &Chunk) -> Vec<u8> {
    let n = SQ * SQ;
    let mut out = Vec::with_capacity(32 + n * 12 + c.objects.len() * 20);
    for v in [0x4241_544cu32, c.key.level as u32, c.key.x, c.key.y, c.objects.len() as u32, c.atmosphere as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&(world.params().sea_level_ft as f32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    // Rendered ground: roofs are drawn as vectors, so their squares show the ground below.
    for (v, l) in c.halo_height.iter().zip(&c.roof_lift) {
        out.extend_from_slice(&(v - l).to_le_bytes());
    }
    for v in &c.tier {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend(c.surface.iter().map(|s| *s as u8));
    out.extend_from_slice(&c.edges);
    for v in &c.halo_water {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for o in &c.objects {
        let kind = if o.sprite == 0 { o.kind as u16 } else { SPRITE_KIND + o.sprite - 1 };
        out.extend_from_slice(&kind.to_le_bytes());
        out.push(o.variant);
        out.push(0);
        for v in [o.x, o.y, o.rot, o.scale] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend(c.road_halo.iter().map(|&r| if r & 0x80 != 0 { 0 } else { r }));
    for b in &c.building {
        out.extend_from_slice(&b.to_le_bytes());
    }
    out.extend_from_slice(&(c.building_polys.len() as u32).to_le_bytes());
    for (poly, floors, roof, tower) in &c.building_polys {
        out.extend_from_slice(&[poly.len().min(255) as u8, *floors, *roof, *tower]);
        for p in poly.iter().take(255) {
            out.extend_from_slice(&p[0].to_le_bytes());
            out.extend_from_slice(&p[1].to_le_bytes());
        }
    }
    out.extend_from_slice(&(c.structures.len() as u32).to_le_bytes());
    for sh in &c.structures {
        out.extend_from_slice(&[sh.kind as u8, sh.pts.len().min(255) as u8, 0, 0]);
        out.extend_from_slice(&sh.size.to_le_bytes());
        for p in sh.pts.iter().take(255) {
            out.extend_from_slice(&p[0].to_le_bytes());
            out.extend_from_slice(&p[1].to_le_bytes());
        }
    }
    for k in 0..n {
        let lift = c.roof_lift[(k / SQ + 1) * HS + k % SQ + 1];
        out.push(lift.round().clamp(0.0, 255.0) as u8);
        out.push(c.urban[k]);
    }
    out.extend_from_slice(&(c.sprites.len() as u32).to_le_bytes());
    for sp in &c.sprites {
        let i = &sp.info;
        let flags = i.blocks_move as u8 | (i.blocks_sight as u8) << 1 | (i.difficult as u8) << 2;
        out.extend_from_slice(&[sp.asset.len().min(255) as u8, i.cover, flags, 0]);
        out.extend_from_slice(&i.radius.to_le_bytes());
        out.extend_from_slice(&i.height_ft.to_le_bytes());
        out.extend_from_slice(&sp.asset.as_bytes()[..sp.asset.len().min(255)]);
    }
    out
}

/// Object catalog (tactical data) as JSON for the renderer and agents.
pub fn catalog_json() -> String {
    serde_json::to_string(&CATALOG.to_vec()).expect("serializable")
}
