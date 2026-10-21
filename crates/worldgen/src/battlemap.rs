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
use crate::t0::roads::CrossingKind;

/// A park laid out (`town::Park`): what a park's chunks and tiles draw.
pub use crate::park::layout as park_layout;

pub const SQ: usize = TILE_N / 2;
/// Squares per edge including a one-square halo ring (for seamless rendering gradients).
pub const HS: usize = SQ + 2;
pub const SQUARE_FT: f64 = 5.0;
pub const TIER_FT: f64 = 5.0;
pub const MAX_COVER_GAP: i32 = 6;
/// In a castle's yard, whose curtains, towers and buildings are its cover: no dry square of it more
/// than this from cover, and a window of it (most of its squares) holding any of them needs no more
/// (`cover_gap`, `works_cover`).
pub const YARD_COVER_GAP: i32 = 12;
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
    /// A flight of steps on a steep paved street (cities and up).
    Stairs,
    /// A park's gravel paths.
    Gravel,
}

impl Surface {
    pub fn is_road(self) -> bool {
        matches!(self, Surface::Cobble | Surface::Road | Surface::Planks | Surface::Stairs)
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
/// A lawn in town (a green, a temple's close, a park): the ground's own, with no wild scatter.
pub const URBAN_PARK: u8 = 11;
/// A park's gravel paths and rounds (kept clear of what the guarantee adds).
pub const URBAN_PATH: u8 = 12;
/// Wall-walk and tower-top heights above the ground (ft).
const WALL_FT: f32 = 20.0;
const TOWER_FT: f32 = 30.0;
/// A palace forecourt's low garden wall and its gate's piers (ft).
const GARDEN_WALL_FT: f32 = 4.0;
const PIER_FT: f32 = 8.0;
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
    /// Squares and streets.
    WaterTrough,
    NoticeBoard,
    Signpost,
    Stocks,
    LampPost,
    MarketCross,
    /// Parks and gardens.
    Fountain,
    Pond,
    Bench,
    Flowerbed,
    Hedge,
    Topiary,
    Gazebo,
    Bandstand,
    GardenStatue,
    Footbridge,
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

pub const CATALOG: [KindInfo; 61] = [
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
    k(Kind::WaterTrough, "water trough", 0.75, false, false, 1, true, 2.5, true),
    k(Kind::NoticeBoard, "notice board", 0.6, true, false, 1, false, 7.0, true),
    k(Kind::Signpost, "signpost", 0.55, false, false, 0, false, 9.0, true),
    k(Kind::Stocks, "stocks", 0.75, true, false, 1, false, 3.5, true),
    k(Kind::LampPost, "lamp post", 0.5, false, false, 0, false, 10.0, true),
    k(Kind::MarketCross, "market cross", 1.0, false, false, 1, true, 15.0, true),
    k(Kind::Fountain, "fountain", 1.0, true, false, 1, false, 5.0, true),
    k(Kind::Pond, "pond", 2.0, false, false, 0, true, 0.0, true),
    k(Kind::Bench, "bench", 0.55, false, false, 1, true, 3.0, true),
    k(Kind::Flowerbed, "flowerbed", 1.0, false, false, 0, true, 1.0, true),
    k(Kind::Hedge, "hedge", 0.6, true, true, 2, false, 6.0, true),
    k(Kind::Topiary, "topiary", 0.55, true, false, 1, false, 6.0, true),
    k(Kind::Gazebo, "gazebo", 1.6, false, false, 1, false, 12.0, true),
    k(Kind::Bandstand, "bandstand", 2.0, false, false, 1, false, 14.0, true),
    k(Kind::GardenStatue, "garden statue", 0.7, true, false, 2, false, 10.0, true),
    k(Kind::Footbridge, "footbridge", 1.6, false, false, 0, false, 3.0, true),
];

/// Every kind, by id - 1.
const KINDS: [Kind; 61] = [
    Kind::TreeDeciduous, Kind::TreeConifer, Kind::TreePalm, Kind::TreeJungle, Kind::TreeAcacia, Kind::TreeDead, Kind::TreeWillow, Kind::Bush,
    Kind::Thicket, Kind::FallenLog, Kind::Stump, Kind::Boulder, Kind::RockSmall, Kind::RockPile, Kind::Reeds, Kind::Cactus, Kind::TallGrass,
    Kind::Flowers, Kind::Mushrooms, Kind::Bones, Kind::Snowdrift, Kind::IceSpire, Kind::BasaltPillar, Kind::Obsidian, Kind::Brambles, Kind::Bog,
    Kind::Quicksand, Kind::ThinIce, Kind::LavaPool, Kind::SteamVent, Kind::Scree, Kind::Sinkhole, Kind::Barrel, Kind::Crate, Kind::Cart,
    Kind::MarketStall, Kind::Well, Kind::Haystack, Kind::Headstone, Kind::RuinWall, Kind::Statue, Kind::Tent, Kind::Campfire, Kind::Firewood,
    Kind::Bedroll, Kind::WaterTrough, Kind::NoticeBoard, Kind::Signpost, Kind::Stocks, Kind::LampPost, Kind::MarketCross, Kind::Fountain, Kind::Pond,
    Kind::Bench, Kind::Flowerbed, Kind::Hedge, Kind::Topiary, Kind::Gazebo, Kind::Bandstand, Kind::GardenStatue, Kind::Footbridge,
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
    /// Put down by hand and turned (`Placed::turned`): drawn at `rot`.
    pub turned: bool,
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
    /// 16 difficult terrain, 32 a park's pond or stream or the ground by it.
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
    /// corner-tower side (squares; a castle's keep with no pieces its top in the high two bits,
    /// `town::plain_keep_top`; 255 a keep of pieces).
    pub building_polys: Vec<(Vec<[f32; 2]>, u8, u8, u8)>,
    /// A keep's pieces with its footprint in `building_polys` (`part_code`, outline; chunk-local
    /// squares); a palace's court (`COURT_PART`, its corners each by the footprint's in the same
    /// place); a manor's cross wing and main range (`CROSS_WING_PART`, `MAIN_RANGE_PART`); empty for
    /// every other building.
    pub building_parts: Vec<Vec<(u8, Vec<[f32; 2]>)>>,
    /// Height above the ground per halo square of what is drawn as a vector on top (roofs,
    /// walls, towers, bridge and pier decks; 0 elsewhere): tiers include it, the rendered
    /// ground does not.
    pub roof_lift: Vec<f32>,
    /// Walls, towers and decks drawn as vectors (chunk-local squares).
    pub structures: Vec<VectorShape>,
    /// Uploaded sprites the chunk's objects use.
    pub sprites: Vec<ChunkSprite>,
    /// A castle's yard per square (bits 1 a bailey, 2 a garden yard's, 4 on a motte above its toe;
    /// empty: none here): where the guarantee counts its walls, towers and buildings as cover.
    pub yard: Vec<u8>,
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
    /// Curtain wall segment, then `[flags, 0]` (`WALL_BARE_A`, `WALL_BARE_B`, `WALL_RUINED`).
    Wall = 0,
    /// Wall or gate tower: its centre, then `[flags, 0]` (`WALL_GATE`, `WALL_RUINED`).
    Tower = 1,
    /// Bridge or pier deck outline, its first edge along the span; size is the style (0 pier,
    /// 1 timber bridge, 2 stone bridge, 3 old stone bridge).
    Deck = 2,
    /// Road bridge centre line, then `[material, 0]` (0 timber, 1 old stone).
    RoadBridge = 3,
    /// A way underground: the opening and a point one square in along the passage; size is
    /// the `UnderKind` (0 dungeon, 1 crypt, 2 cave, 3 mine, 4 lava tube, 5 sewer, 6 catacombs).
    Entrance = 4,
    /// A round feature raised a tier on paved ground: its centre and a point on its rim in the
    /// direction its steps come down (squares); size is the `PlazaFeature`.
    Dais = 5,
    /// A ford: stepping stones across shallow water, its line from bank to bank; size is the
    /// road's width (squares).
    Ford = 6,
    /// A ferry: a rope from landing to landing, a jetty at each end and the raft; size is the
    /// raft's width (squares).
    Ferry = 7,
    /// A flight of steps' axis from the bottom up, then (how far up the flight its bottom lies,
    /// the depth of a tread); size is the width it covers (squares). Every chunk it crosses has
    /// it: the ground draws the treads square across it.
    Stairs = 8,
    /// A gatehouse over the passage between two gate towers: their centres; size is the style
    /// (0 standing, 1 a ruin's).
    Gate = 9,
    /// A palace forecourt's low garden wall: a straight stretch of it; size is its thickness.
    GardenWall = 10,
    /// A gate pier: its centre, then `[angle, 0]` (the way its sides run, radians); size is half its
    /// side.
    GatePier = 11,
    /// A forecourt's gate: the ends of its opening (the piers' inner faces), then a point inside,
    /// the way its leaves stand open; size is the style (0 wrought iron, 1 timber).
    ForecourtGate = 12,
    /// A gravel walk across a palace court's lawn: its four corners.
    GravelWalk = 13,
}

/// Flags on a wall's or tower's last point (`[flags, 0]`): the wall's first or last end stands
/// bare (no tower on it), a gate tower, a ruin's works.
const WALL_BARE_A: u8 = 1;
const WALL_BARE_B: u8 = 2;
const WALL_GATE: u8 = 1;
const WALL_RUINED: u8 = 4;
/// A square tower (`Layout::square_towers`); the flags' second value is the way its sides run
/// (radians).
const WALL_SQUARE: u8 = 8;

/// The code of a palace's court among a building's pieces (`Chunk::building_parts`, after a keep's
/// `part_code`s): open to the sky, the roof a range along each side round it.
pub const COURT_PART: u8 = 5;
/// The codes of a manor's cross wing over its family end and its main range
/// (`interior::cross_wing_of`): roofed apart, the wing's ridge across the range's (a cross gable).
pub const CROSS_WING_PART: u8 = 6;
pub const MAIN_RANGE_PART: u8 = 7;

/// A keep piece's code in the payload: 0 body, 1 round tower, 2 square tower, 3 forebuilding, 4
/// wing (its top, `town::keep_tops` with its donjon's end, in the high four bits).
fn part_code(kind: crate::town::PartKind) -> u8 {
    use crate::town::PartKind as K;
    match kind {
        K::Body => 0,
        K::Tower { round: true } => 1,
        K::Tower { round: false } => 2,
        K::Forebuilding => 3,
        K::Wing => 4,
    }
}

/// The gates in a layout's walls as pairs of gate towers (world ft; `towers` as
/// `interior::towers` gives them): two gate towers 36 to 54 ft apart (a passage at least 6 ft
/// wide between them), each beyond the other's wall (the towers of two gates side by side face
/// each other along one stretch) and the walls running on from them opposite ways (a castle's
/// outer and inner gates are not one gatehouse). Paired in order, each with its nearest; a pure function of the
/// layout, so every chunk finds the same.
fn gateways(l: &crate::town::Layout, towers: &[([f64; 2], f64, bool)]) -> Vec<([f64; 2], [f64; 2])> {
    let gates: Vec<[f64; 2]> = towers.iter().filter(|t| t.2).map(|t| t.0).collect();
    // Which way the wall runs from each gate tower, where a stretch of it ends there.
    let away: Vec<Option<[f64; 2]>> = gates
        .iter()
        .map(|&g| {
            let at = |p: [f64; 2]| (p[0] - g[0]) * (p[0] - g[0]) + (p[1] - g[1]) * (p[1] - g[1]) < 0.25;
            l.walls.iter().filter(|w| w.len() >= 2).find_map(|w| {
                let n = w.len();
                if at(w[0]) {
                    Some([w[1][0] - g[0], w[1][1] - g[1]])
                } else if at(w[n - 1]) {
                    Some([w[n - 2][0] - g[0], w[n - 2][1] - g[1]])
                } else {
                    None
                }
            })
        })
        .collect();
    let mut used = vec![false; gates.len()];
    let mut out = Vec::new();
    for i in 0..gates.len() {
        if used[i] {
            continue;
        }
        let mut best: Option<(f64, usize)> = None;
        for j in i + 1..gates.len() {
            let v = [gates[j][0] - gates[i][0], gates[j][1] - gates[i][1]];
            let d = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]);
            let beyond = |a: Option<[f64; 2]>, s: f64| a.is_none_or(|a| s * (a[0] * v[0] + a[1] * v[1]) < 0.0);
            // (The wall runs on from each the opposite way: not two gates of two curtains, one
            // behind the other.)
            let apart = match (away[i], away[j]) {
                (Some(a), Some(b)) => a[0] * b[0] + a[1] * b[1] < 0.0,
                _ => true,
            };
            if used[j] || !(36.0..=54.0).contains(&d) || (away[i].is_none() && away[j].is_none()) || !beyond(away[i], 1.0) || !beyond(away[j], -1.0) || !apart {
                continue;
            }
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, j));
            }
        }
        if let Some((_, j)) = best {
            used[i] = true;
            used[j] = true;
            out.push((gates[i], gates[j]));
        }
    }
    out
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
        Biome::Blight => &[(TreeDead, 7.0, false), (FallenLog, 1.0, false), (Stump, 0.6, false), (Mushrooms, 0.9, false), (Brambles, 0.3, false), (Bog, 0.2, false), (Bones, 0.12, false)],
        Biome::Ashland => &[(TreeDead, 0.6, false), (Stump, 0.5, false), (Boulder, 0.6, false), (RockPile, 0.2, false), (Obsidian, 0.15, false), (Bones, 0.08, false)],
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
        Biome::Blight => &[TreeDead, FallenLog, Boulder],
        Biome::Ashland => &[Boulder, RockPile, TreeDead],
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
        Biome::Blight => &[Mushrooms, Bog, Bones],
        Biome::Ashland => &[Bones, Obsidian],
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
        Biome::Blight => if r < 0.6 { Atmosphere::Fog } else { Atmosphere::Mist },
        Biome::Ashland => if r < 0.3 { Atmosphere::Embers } else { Atmosphere::None },
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
        Biome::Blight => Surface::Mud,
        Biome::Ashland => Surface::Ash,
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

/// A ferry's jetties (ft), from the ends of its line back to its banks.
pub const JETTY_FT: f64 = 24.0;
pub const JETTY_WIDTH_FT: f64 = 9.0;

/// A city street's line nearest a square where the ground was shaped by hand (`key`: layout,
/// street and segment in the chunk's `layouts_near`), for steps on slopes made by hand.
#[derive(Clone)]
struct HandStreet {
    key: (u32, u32, u32),
    a: [f64; 2],
    b: [f64; 2],
    width: f64,
    d: f64,
}

/// A street square steps up once shaping leaves it climbing at least `g_on` along its street (and
/// `HAND_MARGIN` more than it did); a flight's square goes back to paving once its slope is below
/// `g_off` (and that much gentler than it was, or on a plateau). Steeper than `HAND_CLIFF` it
/// stays a rock face.
const HAND_MARGIN: f64 = 0.03;
const HAND_CLIFF: f64 = 1.0;

/// The most flights of steps' axes the ground draws treads for in one chunk (the renderer's
/// `STAIR_AXES`).
pub const MAX_STAIR_AXES: usize = 64;

/// A road crossing as drawn: along the road from bank to bank (the run over the channel at
/// the crossing angle, extended until both corners are 4 ft clear of every river piece): a
/// bridge deck with an 8-ft landing each side, a ford's stepping stones, or a ferry's line
/// between its jetties' ends 14 ft out in the water (`JETTY_FT` back to the banks), its raft
/// (centre, length, width ft) on the line's sagging rope 30% of the way over. Pure functions
/// of the world, so every chunk finds the same.
struct CrossingShape {
    kind: ShapeKind,
    a: [f64; 2],
    b: [f64; 2],
    /// Width (ft): the deck's, the road's, the raft's.
    size: f64,
    raft: Option<([f64; 2], f64, f64)>,
    /// Merged into another, or given way to it (`merge_bridges`).
    dropped: bool,
    /// A bridge of old stone (else timber): put down so by hand.
    stone: bool,
}

/// Stepping stones along a ford, this far apart (ft).
const FORD_STEP_FT: f64 = 3.5;

fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]).max(1e-9);
    [v[0] / l, v[1] / l]
}

fn crossing_shape(x: &crate::lod::roads::RiverCrossing, tile: &TerrainOut) -> CrossingShape {
    let (u, deck) = (x.u, x.class.width_ft() + 4.0);
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
        let mut t = x.half_span();
        let corner = |t: f64, side: f64| [x.at[0] + u[0] * sign * t - u[1] * side * 0.5 * deck, x.at[1] + u[1] * sign * t + u[0] * side * 0.5 * deck];
        while t < 600.0 && !(clear(corner(t, 1.0)) && clear(corner(t, -1.0))) {
            t += 2.0;
        }
        t
    };
    let (fwd, back) = (reach(1.0), reach(-1.0));
    let (kind, size, extra) = match x.kind {
        CrossingKind::Bridge => (ShapeKind::RoadBridge, deck, 8.0),
        CrossingKind::Ford => (ShapeKind::Ford, x.class.width_ft(), 0.0),
        CrossingKind::Ferry => (ShapeKind::Ferry, x.class.width_ft().max(12.0), -(JETTY_FT - 10.0) - 4.0),
    };
    let (a, b) = ([x.at[0] - u[0] * (back + extra), x.at[1] - u[1] * (back + extra)], [x.at[0] + u[0] * (fwd + extra), x.at[1] + u[1] * (fwd + extra)]);
    let raft = (kind == ShapeKind::Ferry).then(|| raft_on(a, b, size));
    CrossingShape { kind, a, b, size, raft, dropped: false, stone: false }
}

/// A ferry's raft (centre, length, width ft) on its rope from a to b: a quadratic whose control
/// point is the middle moved 4% of the length to the right of the line (its point at t is
/// 2t(1 - t) of that out), 30% of the way over.
fn raft_on(a: [f64; 2], b: [f64; 2], size: f64) -> ([f64; 2], f64, f64) {
    let u = unit2([b[0] - a[0], b[1] - a[1]]);
    let (t, len) = (0.3, crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])));
    let off = 2.0 * t * (1.0 - t) * 0.04 * len;
    let at = [a[0] + (b[0] - a[0]) * t - u[1] * off, a[1] + (b[1] - a[1]) * t + u[0] * off];
    let w = (1.2 * size).max(14.0);
    (at, 1.6 * w, w)
}

/// A crossing put down by hand as decks (world ft polygons) for the map's site tiles: a
/// bridge's; a ferry's jetties, raft and rope (a ford has none).
pub fn hand_decks(c: &crate::world::Crossing) -> Vec<Vec<[f64; 2]>> {
    use crate::world::CrossingKind as K;
    let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
    let mid = |p: [f64; 2], q: [f64; 2]| [0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1])];
    match c.kind {
        K::Bridge => vec![crate::town::rect(mid(c.a, c.b), u, c.length(), c.width)],
        K::Ford => vec![],
        K::Ferry => {
            let (a, b) = ([c.a[0] + u[0] * JETTY_FT, c.a[1] + u[1] * JETTY_FT], [c.b[0] - u[0] * JETTY_FT, c.b[1] - u[1] * JETTY_FT]);
            let (at, l, w) = raft_on(a, b, c.width);
            vec![
                crate::town::rect(mid(c.a, a), u, JETTY_FT, JETTY_WIDTH_FT),
                crate::town::rect(mid(b, c.b), u, JETTY_FT, JETTY_WIDTH_FT),
                crate::town::rect(mid(a, b), u, c.length() - 2.0 * JETTY_FT, 1.5),
                crate::town::rect(at, u, l, w),
            ]
        }
    }
}

/// A crossing put down by hand as drawn (`CrossingShape`), and a bridge's deck level (ft): clear
/// of the highest water under it (rivers, lakes, the sea), else level with the lower end. A
/// ferry's line runs between its jetties' ends, `JETTY_FT` out from the points given. Pure
/// functions of the world, so every chunk finds the same.
fn hand_crossing(c: &crate::world::Crossing, t0: &T0) -> (CrossingShape, f32) {
    use crate::world::CrossingKind as K;
    let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
    let (kind, a, b) = match c.kind {
        K::Bridge => (ShapeKind::RoadBridge, c.a, c.b),
        K::Ford => (ShapeKind::Ford, c.a, c.b),
        K::Ferry => (ShapeKind::Ferry, [c.a[0] + u[0] * JETTY_FT, c.a[1] + u[1] * JETTY_FT], [c.b[0] - u[0] * JETTY_FT, c.b[1] - u[1] * JETTY_FT]),
    };
    let raft = (kind == ShapeKind::Ferry).then(|| raft_on(a, b, c.width));
    let mut top = f32::MIN;
    if kind == ShapeKind::RoadBridge {
        let river = t0.rivers.crossing_level(c.a, c.b, 0.0, t0.cell_ft, &crate::lod::rivers::CrossingCache::default());
        let n = (c.length() / 10.0).ceil().max(1.0) as usize;
        let at = |k: usize| [c.a[0] + (c.b[0] - c.a[0]) * k as f64 / n as f64, c.a[1] + (c.b[1] - c.a[1]) * k as f64 / n as f64];
        let still = (0..=n).map(|k| at(k)).map(|p| t0.sample_water(p[0], p[1])).fold(crate::t0::hydro::DRY, f32::max);
        let water = river.map_or(still, |(z, _)| still.max(z as f32));
        top = if water > crate::t0::hydro::DRY {
            water + DECK_CLEARANCE_FT
        } else {
            t0.ground_at(c.a[0], c.a[1], 5.0).min(t0.ground_at(c.b[0], c.b[1], 5.0)) as f32
        };
    }
    (CrossingShape { kind, a, b, size: c.width, raft, dropped: false, stone: c.material == crate::world::CrossingMaterial::Stone }, top)
}

/// Bridge decks that overlap (a road crossing a meander twice, two roads at a confluence)
/// become one: along the same line, one bridge from end to end; else the greater road's
/// (then the longer) stays and the other is dropped. In a stable order (by crossing point),
/// so every chunk decides alike.
fn merge_bridges(crossings: &[crate::lod::roads::RiverCrossing], shapes: &mut [CrossingShape]) {
    let mut order: Vec<usize> = (0..shapes.len()).filter(|&i| shapes[i].kind == ShapeKind::RoadBridge).collect();
    order.sort_by(|&p, &q| crossings[p].at[0].total_cmp(&crossings[q].at[0]).then(crossings[p].at[1].total_cmp(&crossings[q].at[1])));
    let seg_dist = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
        let (ex, ey) = (a[0] + dx * t - p[0], a[1] + dy * t - p[1]);
        crate::core::sqrt(ex * ex + ey * ey)
    };
    let len = |c: &CrossingShape| crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
    for (n, &i) in order.iter().enumerate() {
        for &j in &order[n + 1..] {
            if shapes[i].dropped || shapes[j].dropped {
                continue;
            }
            let (a, b) = (&shapes[i], &shapes[j]);
            let near = 0.5 * (a.size + b.size);
            let d = seg_dist(a.a, b.a, b.b).min(seg_dist(a.b, b.a, b.b)).min(seg_dist(b.a, a.a, a.b)).min(seg_dist(b.b, a.a, a.b));
            let crosses = {
                let (r, q) = ([a.b[0] - a.a[0], a.b[1] - a.a[1]], [b.b[0] - b.a[0], b.b[1] - b.a[1]]);
                let den = r[0] * q[1] - r[1] * q[0];
                den.abs() > 1e-9 && {
                    let t = ((b.a[0] - a.a[0]) * q[1] - (b.a[1] - a.a[1]) * q[0]) / den;
                    let v = ((b.a[0] - a.a[0]) * r[1] - (b.a[1] - a.a[1]) * r[0]) / den;
                    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&v)
                }
            };
            if d >= near && !crosses {
                continue;
            }
            let u = unit2([a.b[0] - a.a[0], a.b[1] - a.a[1]]);
            let ub = unit2([b.b[0] - b.a[0], b.b[1] - b.a[1]]);
            if (u[0] * ub[0] + u[1] * ub[1]).abs() > 0.95 && d < 0.5 * near {
                // One bridge over both: from the earliest end to the latest along the first.
                let ts = [a.a, a.b, b.a, b.b].map(|p| (p[0] - a.a[0]) * u[0] + (p[1] - a.a[1]) * u[1]);
                let (lo, hi) = (ts.iter().cloned().fold(f64::MAX, f64::min), ts.iter().cloned().fold(f64::MIN, f64::max));
                let o = a.a;
                let size = a.size.max(b.size);
                let s = &mut shapes[i];
                (s.a, s.b, s.size) = ([o[0] + u[0] * lo, o[1] + u[1] * lo], [o[0] + u[0] * hi, o[1] + u[1] * hi], size);
                shapes[j].dropped = true;
            } else {
                let keep_i = (crossings[i].class, -len(&shapes[i])) <= (crossings[j].class, -len(&shapes[j]));
                shapes[if keep_i { j } else { i }].dropped = true;
            }
        }
    }
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
    // Where roads cross rivers, and how (see `RiverCrossing`).
    let crossings = crate::lod::roads::river_crossings(&tile.roads, &tile.pieces);
    let mut crossing_shapes: Vec<CrossingShape> = crossings.iter().map(|x| crossing_shape(x, tile)).collect();
    merge_bridges(&crossings, &mut crossing_shapes);
    let x_ft = |i: i64| ox + (i as f64 + 0.5) * SQUARE_FT;
    let y_ft = |j: i64| oy + (j as f64 + 0.5) * SQUARE_FT;
    // Ground shaped by hand, applied as heights are read (so whatever stands on the ground
    // follows it); `plat_w` is the plateaus' weight per halo square (they are kept flat), `moved`
    // where the shaping changed the ground (the slope is measured on the shaped ground there).
    let shaper = crate::ground::Shaper::new(&world.file.edits, [ox - SQUARE_FT, oy - SQUARE_FT, ox + (SQ + 1) as f64 * SQUARE_FT, oy + (SQ + 1) as f64 * SQUARE_FT], SQUARE_FT);
    let (mut plat_w, mut moved) = if shaper.is_some() { (vec![0f32; HS * HS], vec![false; HS * HS]) } else { (Vec::new(), Vec::new()) };
    for j in -1..=SQ as i64 {
        for i in -1..=SQ as i64 {
            let (x, y) = (ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT);
            let c = sidx(i, j);
            let h = (j + 1) as usize * HS + (i + 1) as usize;
            hh[h] = tile.padded[c];
            hw[h] = t0.sample_water(x, y).max(tile.river_water[c]);
            if let Some(s) = &shaper {
                (hh[h], plat_w[h]) = s.at_square(gsx + i, gsy + j, hh[h], hw[h]);
                moved[h] = hh[h] != tile.padded[c] || plat_w[h] > 0.0;
            }
            road_h[h] = tile.road_mask.get(c).copied().unwrap_or(0);
            // A ford or a ferry leaves the water as it is (wading, or the raft).
            if road_h[h] != 0 && hw[h] > hh[h] && crossings.iter().any(|x| x.kind != CrossingKind::Bridge && x.covers([x_ft(i), y_ft(j)])) {
                road_h[h] = 0;
            }
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
    // Squares on a flight of steps (only where a flight comes near the chunk).
    let mut stair_h: Option<Vec<bool>> = None;
    // Where the ground was shaped by hand round a city's streets: the street line nearest each such
    // square (steps follow the slopes made by hand; empty when nothing is shaped here).
    let mut hand_st: Vec<Option<HandStreet>> = Vec::new();
    let mut refs: Vec<(u32, u32)> = Vec::new();
    let mut polys: Vec<(Vec<[f32; 2]>, u8, u8, u8)> = Vec::new();
    let mut poly_parts: Vec<Vec<(u8, Vec<[f32; 2]>)>> = Vec::new();
    let mut shapes: Vec<VectorShape> = Vec::new();
    let local = |p: &[f64; 2]| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32];
    let chunk_ft = SQ as f64 * SQUARE_FT;
    let owns = |p: [f64; 2]| p[0] >= ox && p[0] < ox + chunk_ft && p[1] >= oy && p[1] < oy + chunk_ft;
    let mut wells: Vec<[f64; 2]> = Vec::new();
    // Market stalls in blocks on town plazas (world ft, rotation, kind).
    let mut stalls: Vec<([f64; 2], f32, Kind)> = Vec::new();
    // Props set out on squares by what they are for (world ft, rotation, kind, variant).
    let mut dressing: Vec<([f64; 2], f32, Kind, u8)> = Vec::new();
    // Temples' churchyards in closes (their headstones are the close's dressing).
    let mut churchyards: Vec<Vec<[f64; 2]>> = Vec::new();
    // The capital's palace's court's squares (halo-indexed): a lawn, nothing added on it but what it
    // sets out.
    let mut court_h = vec![false; HS * HS];
    // Round raised features on town squares: (centre, radius ft, kind, steps angle).
    let mut features: Vec<([f64; 2], f64, PlazaFeature, f64)> = Vec::new();
    let mut in_town = vec![false; HS * HS];
    let mut ruins: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut statues: Vec<[f64; 2]> = Vec::new();
    // Props a layout sets out itself (a camp's tents and fire).
    let mut props: Vec<crate::town::Prop> = Vec::new();
    // Gates' passages (their towers' centres, world ft): kept clear of generated objects.
    let mut gates: Vec<([f64; 2], [f64; 2])> = Vec::new();
    // Parks (`park_layout`): which squares are a park's (1 a common, 2 a formal garden) and which
    // park (an index into `park_biomes`, from 1: the biome its lawn takes, the same in every chunk),
    // their ponds, streams (centre line and water's level), raised tiers (middle, top radius), swells
    // (middle, radius, rise), formal terraces (outline, level), sunk parterres, what they set out,
    // ground the guarantee keeps clear, and each park's outline, benches' look and paths (for the
    // benches the guarantee adds).
    let mut park_h = vec![0u8; HS * HS];
    let mut park_ix = vec![0u8; HS * HS];
    let mut park_biomes: Vec<Biome> = Vec::new();
    let mut swells: Vec<crate::park::Swell> = Vec::new();
    let mut terraces: Vec<(Vec<[f64; 2]>, f32)> = Vec::new();
    let mut park_keep: Vec<([f64; 2], [f64; 2], f64)> = Vec::new();
    let mut ponds: Vec<crate::park::Pond> = Vec::new();
    let mut streams: Vec<Vec<([f64; 2], f32)>> = Vec::new();
    let mut mounds: Vec<([f64; 2], f64)> = Vec::new();
    let mut sunk: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut park_props: Vec<([f64; 2], f32, Kind, u8, f32)> = Vec::new();
    let mut park_meta: Vec<ParkMeta> = Vec::new();
    // Camps' clearings, levelled (outline, height ft).
    let mut yards: Vec<(Vec<[f64; 2]>, f32)> = Vec::new();
    // A shell keep's motte: the ground raised to its top round the keep, easing down to the ground
    // round it (never lowered); a gravel path up it to the shell's gate (`Motte::way`).
    // Castles' baileys: grass (squares marked).
    let mut motte_path = vec![false; HS * HS];
    let mut bailey_h = vec![false; HS * HS];
    // (What the guarantee may set out where: 1 a bailey's, 2 a garden yard's; 4 on a motte (but its
    // toe), nothing.)
    let mut bailey_kind = vec![0u8; HS * HS];
    // A bailey's dressing (`dress_bailey`: world ft, rotation, kind, look, scale).
    let mut bailey_props: Vec<([f64; 2], f32, Kind, u8, f32)> = Vec::new();
    let mut bailey_ways: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for l in crate::town::layouts_near(world, t0, [ox - SQUARE_FT, oy - SQUARE_FT, ox + (SQ + 1) as f64 * SQUARE_FT, oy + (SQ + 1) as f64 * SQUARE_FT]) {
        for m in &l.mottes {
            let (a, b) = (crate::town::geom::add(m.at, crate::town::geom::mul(m.way, m.top_r - 8.0)), crate::town::geom::add(m.at, crate::town::geom::mul(m.way, m.foot_r + 6.0)));
            for j in 0..HS {
                for i in 0..HS {
                    let p = [ox + (i as f64 - 0.5) * SQUARE_FT, oy + (j as f64 - 0.5) * SQUARE_FT];
                    if crate::town::geom::seg_dist(p, a, b) < 4.0 {
                        motte_path[j * HS + i] = true;
                    }
                    let d = crate::town::geom::dist(p, m.at);
                    // (The mound, but its toe, where the guarantee's cover may stand round it: what
                    // stands on a square by it not reaching past its last 8 ft.)
                    if d < m.foot_r - 4.0 {
                        bailey_kind[j * HS + i] |= 4;
                    }
                    if d >= m.foot_r {
                        continue;
                    }
                    let t = ((d - m.top_r) / (m.foot_r - m.top_r)).clamp(0.0, 1.0);
                    let k = (1.0 - t * t * (3.0 - 2.0 * t)) as f32;
                    let h = j * HS + i;
                    if hw[h] <= hh[h] {
                        hh[h] = hh[h].max(hh[h] + (m.top - hh[h]) * k);
                    }
                }
            }
        }
    }
    // Ground before anything is raised on it (walls stand on the ground, not on roofs).
    let ground_h = hh.clone();
    let (hx0, hy0) = (ox - SQUARE_FT, oy - SQUARE_FT);
    let span = HS as f64 * SQUARE_FT;
    let center = |i: usize, j: usize| [hx0 + (i as f64 + 0.5) * SQUARE_FT, hy0 + (j as f64 + 0.5) * SQUARE_FT];
    let bbox = bounds;
    // Halo squares a polygon's bbox may cover.
    let range = |b: (f64, f64, f64, f64)| {
        let i0 = (((b.0 - hx0) / SQUARE_FT - 0.5).floor().max(0.0)) as usize;
        let j0 = (((b.1 - hy0) / SQUARE_FT - 0.5).floor().max(0.0)) as usize;
        let i1 = (((b.2 - hx0) / SQUARE_FT - 0.5).ceil()).min(HS as f64 - 1.0);
        let j1 = (((b.3 - hy0) / SQUARE_FT - 0.5).ceil()).min(HS as f64 - 1.0);
        (i0, j0, i1, j1)
    };
    let overlaps = |b: (f64, f64, f64, f64)| b.2 >= hx0 && b.0 <= hx0 + span && b.3 >= hy0 && b.1 <= hy0 + span;
    for (li, l) in crate::town::layouts_near(world, t0, [hx0, hy0, hx0 + span, hy0 + span]).into_iter().enumerate() {
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
        for pl in &l.plazas {
            use crate::town::PlazaKind as K;
            let p = &pl.pts;
            // A town's green and a temple's close are lawns; a village's green keeps the ground
            // round it as it is.
            let lawn = pl.kind.is_lawn() && l.tier >= crate::t0::settle::Tier::Town;
            stamp(p, if lawn { URBAN_PARK } else { URBAN_PLAZA }, &mut urb_h);
            // A castle's bailey: an open yard of grass, dressed sparingly (`dress_bailey`).
            if pl.kind == K::Bailey {
                let (i0, j0, i1, j1) = range(bbox(p));
                if overlaps(bbox(p)) && i1 >= 0.0 && j1 >= 0.0 {
                    let garden = dress_bailey(pl, &l, t0, seed, &mut bailey_props, &mut bailey_ways);
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            if crate::town::geom::contains(p, center(i, j)) {
                                bailey_h[j * HS + i] = true;
                                bailey_kind[j * HS + i] |= if garden { 2 } else { 1 };
                            }
                        }
                    }
                }
                continue;
            }
            // A quay takes cargo, no well or market.
            if pl.kind == K::Quay {
                if overlaps(bbox(p)) {
                    quay_cargo(p, seed, &mut stalls);
                }
                continue;
            }
            // The main square and a village's green have a well at the heart; the other squares
            // are dressed by what they are for (`dress_square`).
            let main = pl.kind.is_market() || pl.kind == K::Civic || (pl.kind == K::Green && !lawn);
            if !l.site {
                if main {
                    wells.push(crate::town::geom::centroid(p));
                }
                if l.tier >= crate::t0::settle::Tier::Town && overlaps(bbox(p)) {
                    let first = stalls.len();
                    // Lanes the features keep off: the town's streets and roads, and the
                    // world's roads across the square. Only those near it (features stand in
                    // the square and are far smaller than 200 ft: farther lanes never stop one).
                    let b = bbox(p);
                    let near = |q: &[[f64; 2]], w: f64| {
                        let m = w + 200.0;
                        q[0][0].min(q[1][0]) <= b.2 + m && q[0][0].max(q[1][0]) >= b.0 - m && q[0][1].min(q[1][1]) <= b.3 + m && q[0][1].max(q[1][1]) >= b.1 - m
                    };
                    let mut lanes: Vec<([f64; 2], [f64; 2], f64)> = Vec::new();
                    for (pts, _, w) in &l.roads {
                        lanes.extend(pts.windows(2).filter(|q| near(q, 0.5 * w)).map(|q| (q[0], q[1], 0.5 * w)));
                    }
                    for st in &l.streets {
                        lanes.extend(st.windows(2).filter(|q| near(q, 5.0)).map(|q| (q[0], q[1], 5.0)));
                    }
                    for (ri, k) in t0.roads.segments_near([b.0 - 50.0, b.1 - 50.0, b.2 + 50.0, b.3 + 50.0], 0.0) {
                        let rc = &t0.roads.roads[ri as usize];
                        for j in 0..16 {
                            let a = rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p;
                            let e = rc.eval(k as usize, (j + 1) as f64 / 16.0, 5.0, t0.cell_ft).p;
                            lanes.push((a, e, 12.0));
                        }
                    }
                    let ffirst = features.len();
                    if !main {
                        dress_square(pl, &l, t0, &lanes, seed, &mut features, &mut dressing);
                        continue;
                    }
                    // A city's squares are laid out first, in a pattern round the well, and the
                    // market fits round them; a town's features go where the market leaves room.
                    let patterned = l.tier >= crate::t0::settle::Tier::City;
                    if patterned {
                        plaza_features(p, &l.monuments, &[], &lanes, seed, true, pl.kind == K::Civic, &mut features);
                    }
                    let mut keep: Vec<([f64; 2], f64)> = features[ffirst..].iter().map(|f| (f.0, f.1)).collect();
                    let mut room = Room::new(p, &l, &lanes, &keep, seed ^ 0xd3a1);
                    room.taken.push((crate::town::geom::centroid(p), 6.0));
                    // A market cross by the well, which the stalls keep clear of.
                    let cross = main_cross(pl, &mut room, seed);
                    if let Some(x) = cross {
                        keep.push((x.0, cross_ft()));
                        dressing.push(x);
                    }
                    if let Some(m) = Market::of(pl.kind) {
                        market_blocks(p, &l.monuments, &keep, m, seed, &mut stalls);
                    }
                    if !patterned {
                        // (Clear of the market cross as of a monument.)
                        let mut stands = l.monuments.clone();
                        stands.extend(cross.map(|x| crate::town::geom::circle(x.0, cross_ft(), 8)));
                        plaza_features(p, &stands, &stalls[first..], &lanes, seed, false, false, &mut features);
                    }
                    // Then round its edge: notices, troughs, lamps; a civic square's stocks.
                    room.taken.extend(features[ffirst..].iter().map(|f| (f.0, f.1 + 2.0)));
                    room.taken.extend(stalls[first..].iter().map(|s| (s.0, info(s.2 as u16).radius as f64 * SQUARE_FT + 3.0)));
                    dress_main(pl, &l, &mut room, seed, &mut dressing);
                }
            }
        }
        // Parks: a lawn (a formal garden's mown), gravel paths and rounds on it; its pond, stream
        // and raised tier, round features raised a tier and what it sets out (laid out whole by
        // every chunk it reaches, so they agree).
        for pk in &l.parks {
            let b = bbox(&pk.pts);
            if !overlaps((b.0 - 20.0, b.1 - 20.0, b.2 + 20.0, b.3 + 20.0)) {
                continue;
            }
            let pb = land_biome(t0, crate::town::geom::centroid(&pk.pts));
            let lay = park_layout(pk, pb, l.tier >= crate::t0::settle::Tier::City, seed);
            stamp(&pk.pts, URBAN_PARK, &mut urb_h);
            park_biomes.push(pb);
            let (i0, j0, i1, j1) = range(b);
            if i1 >= 0.0 && j1 >= 0.0 {
                let mark = if pk.kind == crate::town::ParkKind::Formal { 2 } else { 1 };
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        if crate::town::geom::contains(&pk.pts, center(i, j)) {
                            park_h[j * HS + i] = mark;
                            park_ix[j * HS + i] = park_biomes.len().min(255) as u8;
                        }
                    }
                }
            }
            // A square is gravel where a path's or a round's edge reaches past its middle (a path a
            // little wider than drawn, so a slanting one stays joined), but under a gazebo.
            let reserved = |p: [f64; 2]| lay.reserved.iter().any(|&(c, r)| crate::town::geom::dist(p, c) < r);
            let segs = lay.paths.iter().flat_map(|(pts, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w + 0.6)));
            for (a, e, half) in segs.chain(lay.rounds.iter().map(|&(c, r)| (c, c, r))) {
                let (i0, j0, i1, j1) = range((a[0].min(e[0]) - half, a[1].min(e[1]) - half, a[0].max(e[0]) + half, a[1].max(e[1]) + half));
                if i1 < 0.0 || j1 < 0.0 {
                    continue;
                }
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let h = j * HS + i;
                        if urb_h[h] == URBAN_PARK && park_h[h] != 0 && crate::town::geom::seg_dist(center(i, j), a, e) < half && !reserved(center(i, j)) {
                            urb_h[h] = URBAN_PATH;
                        }
                    }
                }
            }
            ponds.extend(lay.pond.clone());
            if !lay.stream.is_empty() {
                streams.push(lay.stream.clone());
            }
            mounds.extend(lay.mound);
            swells.extend(lay.swells.iter().copied());
            terraces.extend(lay.level.map(|v| (pk.pts.clone(), v)));
            park_keep.extend(lay.keep.iter().copied());
            sunk.extend(lay.sunk.iter().cloned());
            park_meta.push(ParkMeta {
                poly: pk.pts.clone(),
                bench: lay.bench,
                segs: lay.paths.iter().flat_map(|(pts, _)| pts.windows(2).map(|q| (q[0], q[1]))).collect(),
                rounds: lay.rounds.clone(),
                axis: lay.axis,
            });
            for &(c, r, kind) in &lay.features {
                features.push((c, r, kind, unit(hash3(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 0x9a5)) * std::f64::consts::TAU));
            }
            park_props.extend(lay.props.iter().copied());
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
        let hand = shaper.is_some() && l.tier >= crate::town::stairs::STAIR_TIER;
        for (ri, (pts, _, width)) in l.roads.iter().enumerate() {
            let half = 0.5 * width;
            for (si, seg) in pts.windows(2).enumerate() {
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
                        let d = crate::town::geom::seg_dist(center(i, j), seg[0], seg[1]);
                        if urb_h[h] != URBAN_DECK && d < half {
                            urb_h[h] = URBAN_STREET;
                            // An own square the shaping reaches (or its neighbours'), off the plazas.
                            if hand
                                && (1..=SQ).contains(&i)
                                && (1..=SQ).contains(&j)
                                && [h, h - 1, h + 1, h - HS, h + HS].iter().any(|&q| moved[q])
                                && hand_st.get(h).is_none_or(|r| r.as_ref().is_none_or(|r| d < r.d))
                                && !l.plazas.iter().any(|p| crate::town::geom::contains(&p.pts, center(i, j)))
                            {
                                if hand_st.is_empty() {
                                    hand_st = vec![None; HS * HS];
                                }
                                hand_st[h] = Some(HandStreet { key: (li as u32, ri as u32, si as u32), a: seg[0], b: seg[1], width: *width, d });
                            }
                        }
                    }
                }
            }
        }
        // Flights of steps: the squares of the street under them, square at their ends (a round
        // end would reach past the landing), never on a plaza (the street stamp paves over plazas).
        for st in l.stairs.iter().filter(|_| l.tier >= crate::town::stairs::STAIR_TIER) {
            let half = 0.5 * st.width;
            let last = st.pts.len() - 2;
            for (k, seg) in st.pts.windows(2).enumerate() {
                let b = (seg[0][0].min(seg[1][0]) - half, seg[0][1].min(seg[1][1]) - half, seg[0][0].max(seg[1][0]) + half, seg[0][1].max(seg[1][1]) + half);
                if !overlaps(b) {
                    continue;
                }
                let (i0, j0, i1, j1) = range(b);
                if i1 < 0.0 || j1 < 0.0 {
                    continue;
                }
                let plazas: Vec<&Vec<[f64; 2]>> = l.plazas.iter().map(|p| &p.pts).filter(|p| {
                    let q = bbox(p);
                    q.0 <= b.2 && q.2 >= b.0 && q.1 <= b.3 && q.3 >= b.1
                }).collect();
                let (u, len) = (crate::town::geom::sub(seg[1], seg[0]), crate::town::geom::dist(seg[0], seg[1]).max(1e-9));
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let c = center(i, j);
                        let t = crate::town::geom::dot(crate::town::geom::sub(c, seg[0]), u) / len;
                        if (k == 0 && t < 0.0) || (k == last && t > len) || crate::town::geom::seg_dist(c, seg[0], seg[1]) >= half {
                            continue;
                        }
                        if !plazas.iter().any(|p| crate::town::geom::contains(p, c)) {
                            stair_h.get_or_insert_with(|| vec![false; HS * HS])[j * HS + i] = true;
                        }
                    }
                }
            }
        }
        if l.tier >= crate::town::stairs::STAIR_TIER && !l.stairs.is_empty() {
            for ax in crate::town::stairs::axes(&l.stairs) {
                let half = 0.5 * ax.width;
                let b = (ax.a[0].min(ax.b[0]) - half, ax.a[1].min(ax.b[1]) - half, ax.a[0].max(ax.b[0]) + half, ax.a[1].max(ax.b[1]) + half);
                if overlaps(b) {
                    let pts = vec![local(&ax.a), local(&ax.b), [(ax.s0 / SQUARE_FT) as f32, (ax.tread / SQUARE_FT) as f32]];
                    shapes.push(VectorShape { kind: ShapeKind::Stairs, size: (ax.width / SQUARE_FT) as f32, pts });
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
        // The capital's palace's court and forecourt (`interior::court_plan`), where the chunk meets
        // them: the forecourt paved, nothing added on it, its walls and the gate's piers raised with
        // the town's walls below.
        let mut courts: Vec<(u32, crate::interior::Court)> = Vec::new();
        let mut fore_walls: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let mut fore_piers: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let mut fore_gates: Vec<[[f64; 2]; 3]> = Vec::new();
        let mut walks: Vec<[[f64; 2]; 4]> = Vec::new();
        for b in l.buildings.iter().filter(|b| crate::town::palace::is_palace(b)) {
            let bb = bbox(&b.poly);
            if !overlaps((bb.0 - 40.0, bb.1 - 40.0, bb.2 + 40.0, bb.3 + 40.0)) {
                continue;
            }
            let Some(c) = crate::interior::court_plan(world, t0, &l, b) else { continue };
            if let Some(fc) = &c.forecourt {
                stamp(&fc.yard, URBAN_PLAZA, &mut urb_h);
                let (fi0, fj0, fi1, fj1) = range(bbox(&fc.yard));
                if fi1 >= 0.0 && fj1 >= 0.0 {
                    for j in fj0..=fj1 as usize {
                        for i in fi0..=fi1 as usize {
                            if crate::town::geom::contains(&fc.yard, center(i, j)) {
                                court_h[j * HS + i] = true;
                            }
                        }
                    }
                }
                fore_walls.extend(fc.walls.iter().copied());
                fore_piers.extend(fc.piers.iter().map(|&p| (p, fc.along)));
                fore_gates.push(fc.gate);
            }
            courts.push((b.id, c));
        }
        for b in &l.buildings {
            let bb = bbox(&b.poly);
            if !overlaps(bb) {
                continue;
            }
            match b.structure {
                crate::town::Structure::Roofed => {}
                crate::town::Structure::Open => {
                    stamp(&b.poly, URBAN_GRAVEYARD, &mut urb_h);
                    // A close's churchyard has its rows laid along it (`dress_square`).
                    let c = crate::town::geom::centroid(&b.poly);
                    if l.plazas.iter().any(|p| p.kind == crate::town::PlazaKind::TempleClose && crate::town::geom::contains(&p.pts, c)) {
                        churchyards.push(b.poly.clone());
                    }
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
            // (Its ground floor rides on ground shaped by hand: `ground::pad_shift`.)
            let mut pad = b.pad_ft;
            // (The capital's palace's court open to the sky: not the building's. A lawn with gravel
            // walks, its fountain and its benches, as its ground floor has them:
            // `interior::court_plan`.)
            let court = courts.iter().position(|c| c.0 == b.id).map(|k| courts.swap_remove(k).1);
            if let Some(c) = &court {
                stamp(&c.court, URBAN_PARK, &mut urb_h);
                let (ci0, cj0, ci1, cj1) = range(bbox(&c.court));
                if ci1 >= 0.0 && cj1 >= 0.0 {
                    for j in cj0..=cj1 as usize {
                        for i in ci0..=ci1 as usize {
                            if crate::town::geom::contains(&c.court, center(i, j)) {
                                court_h[j * HS + i] = true;
                            }
                        }
                    }
                }
                // (The walks drawn as the ground floor has them, straight-edged on their squares: on
                // the lawn, walked over; the fountain its size there, a square's a basin.)
                for (kind, q, back) in &c.pieces {
                    let m = crate::town::geom::mean(q);
                    match *kind {
                        "garden_path" => walks.push(*q),
                        "fountain" => park_props.push((m, 0.0, Kind::Fountain, 0, (0.5 * crate::town::geom::dist(q[0], q[1]) / SQUARE_FT).max(0.75) as f32)),
                        "bench" => dressing.push((m, crate::park::facing(back.map_or([0.0, 1.0], |d| [-d[0], -d[1]])), Kind::Bench, 0)),
                        _ => {}
                    }
                }
            }
            let court = court.map(|c| c.court);
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    if crate::town::geom::contains(&b.poly, center(i, j)) && !court.as_ref().is_some_and(|q| crate::town::geom::contains(q, center(i, j))) {
                        if id == 0 {
                            pad += crate::ground::pad_shift(&world.file.edits, t0, &b.poly, b.pad_ft);
                            refs.push((l.index, b.id));
                            let battlements = crate::interior::battlements(b);
                            // (A castle's keep with no pieces: its top in the high two bits.)
                            let top = if battlements.is_some() { crate::town::plain_keep_top(b, world.stream("keep.tops")) } else { 0 };
                            polys.push((
                                b.poly.iter().map(|p| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32]).collect(),
                                b.floors,
                                roof_byte(b, battlements.is_some()),
                                crate::core::round(battlements.unwrap_or(0.0) / SQUARE_FT) as u8 | top << 6,
                            ));
                            // (Each piece's code, how it is topped above it: `town::keep_tops`.)
                            let tops = crate::town::keep_tops(b, world.stream("keep.tops"));
                            let mut parts: Vec<(u8, Vec<[f32; 2]>)> = b.parts.iter().zip(&tops).map(|(p, &top)| (part_code(p.kind) | top << 4, p.poly.iter().map(local).collect())).collect();
                            parts.extend(court.as_ref().map(|q| (COURT_PART, q.iter().map(local).collect())));
                            // (A manor: its main range and its cross wing, roofed apart.)
                            if let Some((main, wing)) = crate::interior::cross_wing_of(world, t0, &l, b) {
                                parts.push((MAIN_RANGE_PART, main.iter().map(local).collect()));
                                parts.push((CROSS_WING_PART, wing.iter().map(local).collect()));
                            }
                            poly_parts.push(parts);
                            id = refs.len().min(u16::MAX as usize) as u16;
                        }
                        let h = j * HS + i;
                        bld_h[h] = id;
                        lift[h] = pad + STOREY_FT * b.floors as f32 - hh[h];
                        hh[h] = pad + STOREY_FT * b.floors as f32;
                    }
                }
            }
        }
        // Walls (8 ft thick) and towers (22 ft across; 30 at gates): raised stone.
        // (`square`: a square tower, its sides along it.)
        let raise = |a: [f64; 2], b: [f64; 2], half: f64, val: u8, ft: f32, square: Option<[f64; 2]>, urb_h: &mut Vec<u8>, hh: &mut Vec<f32>, lift: &mut Vec<f32>| {
            let reach = if square.is_some() { half * std::f64::consts::SQRT_2 } else { half };
            let bb = (a[0].min(b[0]) - reach, a[1].min(b[1]) - reach, a[0].max(b[0]) + reach, a[1].max(b[1]) + reach);
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
                    let on = match square {
                        Some(u) => {
                            let d = crate::town::geom::sub(center(i, j), a);
                            crate::town::geom::dot(d, u).abs() <= half + 1.5 && crate::town::geom::cross(u, d).abs() <= half + 1.5
                        }
                        None => crate::town::geom::seg_dist(center(i, j), a, b) <= half + 1.5,
                    };
                    if on {
                        urb_h[h] = val;
                        hh[h] = ground_h[h] + ft;
                        lift[h] = hh[h] - ground_h[h];
                    }
                }
            }
        };
        for t in &l.towers {
            raise(*t, *t, 11.0, URBAN_TOWER, TOWER_FT, l.square_tower(*t), &mut urb_h, &mut hh, &mut lift);
        }
        for t in &l.gate_towers {
            raise(*t, *t, 15.0, URBAN_TOWER, TOWER_FT, None, &mut urb_h, &mut hh, &mut lift);
        }
        for &(t, gate) in &l.extra_towers {
            raise(t, t, if gate { 15.0 } else { 11.0 }, URBAN_TOWER, TOWER_FT, if gate { None } else { l.square_tower(t) }, &mut urb_h, &mut hh, &mut lift);
        }
        for w in &l.walls {
            for seg in w.windows(2) {
                raise(seg[0], seg[1], 4.0, URBAN_WALL, WALL_FT, None, &mut urb_h, &mut hh, &mut lift);
            }
        }
        // A palace's forecourt: its low garden wall, the gate's piers (`interior::FORE_WALL_FT`,
        // `FORE_PIER_FT`); the gate's opening between them clear.
        for &(a, b) in &fore_walls {
            raise(a, b, 0.5 * crate::interior::FORE_WALL_FT, URBAN_WALL, GARDEN_WALL_FT, None, &mut urb_h, &mut hh, &mut lift);
        }
        for &(p, u) in &fore_piers {
            raise(p, p, 0.5 * crate::interior::FORE_PIER_FT, URBAN_WALL, PIER_FT, Some(u), &mut urb_h, &mut hh, &mut lift);
        }
        // The same structures as vectors, each drawn by the one chunk that owns its middle. Walls
        // and towers carry a point of flags (`WALL_*`): a wall's ends standing bare (no tower on
        // them: where a ruin's stretch fell), a gate tower, a ruin's works.
        let towers = crate::interior::towers(&l);
        let ruined = !l.buildings.is_empty() && l.buildings.iter().all(|b| b.structure == crate::town::Structure::Ruin);
        let ruin_bit = if ruined { WALL_RUINED } else { 0 };
        let near = |p: [f64; 2], q: [f64; 2], r: f64| (p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1]) <= r * r;
        let bare = |p: [f64; 2], wi: usize| {
            !towers.iter().any(|t| near(p, t.0, t.1 + 1.0)) && !l.walls.iter().enumerate().any(|(k, w)| k != wi && (near(p, w[0], 1.0) || near(p, w[w.len() - 1], 1.0)))
        };
        for (wi, w) in l.walls.iter().enumerate() {
            let n = w.len();
            for (k, seg) in w.windows(2).enumerate() {
                if owns([0.5 * (seg[0][0] + seg[1][0]), 0.5 * (seg[0][1] + seg[1][1])]) {
                    let ends = if k == 0 && bare(seg[0], wi) { WALL_BARE_A } else { 0 } | if k + 2 == n && bare(seg[1], wi) { WALL_BARE_B } else { 0 };
                    let flags = [(ends | ruin_bit) as f32, 0.0];
                    shapes.push(VectorShape { kind: ShapeKind::Wall, size: (8.0 / SQUARE_FT) as f32, pts: vec![local(&seg[0]), local(&seg[1]), flags] });
                }
            }
        }
        for &(t, r, gate) in &towers {
            if owns(t) {
                // (A square tower: its flag, and the way its sides run as an angle.)
                let square = if gate { None } else { l.square_tower(t) };
                let flags = match square {
                    Some(u) => [(WALL_SQUARE | ruin_bit) as f32, libm::atan2(u[1], u[0]) as f32],
                    None => [(if gate { WALL_GATE } else { 0 } | ruin_bit) as f32, 0.0],
                };
                shapes.push(VectorShape { kind: ShapeKind::Tower, size: (r / SQUARE_FT) as f32, pts: vec![local(&t), flags] });
            }
        }
        for &(a, b) in &fore_walls {
            if owns([0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]) {
                shapes.push(VectorShape { kind: ShapeKind::GardenWall, size: (crate::interior::FORE_WALL_FT / SQUARE_FT) as f32, pts: vec![local(&a), local(&b)] });
            }
        }
        for &(p, u) in &fore_piers {
            if owns(p) {
                shapes.push(VectorShape { kind: ShapeKind::GatePier, size: (0.5 * crate::interior::FORE_PIER_FT / SQUARE_FT) as f32, pts: vec![local(&p), [libm::atan2(u[1], u[0]) as f32, 0.0]] });
            }
        }
        // (The capital's gate wrought iron, timber in the odd palace.)
        for g in &fore_gates {
            if owns([0.5 * (g[0][0] + g[1][0]), 0.5 * (g[0][1] + g[1][1])]) {
                let timber = unit(hash3(seed, crate::core::round(g[0][0]) as i64, crate::core::round(g[0][1]) as i64, 0x6a7e)) < 0.25;
                shapes.push(VectorShape { kind: ShapeKind::ForecourtGate, size: timber as u8 as f32, pts: g.iter().map(local).collect() });
            }
        }
        for q in &walks {
            if owns(crate::town::geom::mean(q)) {
                shapes.push(VectorShape { kind: ShapeKind::GravelWalk, size: 0.0, pts: q.iter().map(local).collect() });
            }
        }
        for (g, h) in gateways(&l, &towers) {
            gates.push((g, h));
            if owns([0.5 * (g[0] + h[0]), 0.5 * (g[1] + h[1])]) {
                shapes.push(VectorShape { kind: ShapeKind::Gate, size: ruined as u8 as f32, pts: vec![local(&g), local(&h)] });
            }
        }
        // Deck style: 0 pier, 1 a village or town's timber bridge, 2 a city's stone bridge, 3 an
        // old stone bridge (some of either, by where the deck stands).
        let city = l.tier >= crate::t0::settle::Tier::City;
        let style = |d: &Vec<[f64; 2]>| {
            let c = crate::town::geom::centroid(d);
            let old = unit(hash3(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 0x01d5)) < if city { 0.35 } else { 0.30 };
            if old { 3.0 } else if city { 2.0 } else { 1.0 }
        };
        for (d, size) in l.bridges.iter().map(|d| (d, style(d))).chain(l.piers.iter().map(|d| (d, 0.0))) {
            if owns(crate::town::geom::centroid(d)) {
                shapes.push(VectorShape { kind: ShapeKind::Deck, size, pts: d.iter().map(local).collect() });
            }
        }
        props.extend(l.props.iter().copied());
        for m in &l.monuments {
            statues.push(crate::town::geom::centroid(m));
        }
    }
    // Crossings put down by hand (`Edits::crossings`) near this chunk, over whatever is there:
    // a bridge's planks wherever the ground is below its deck, a ford's bed brought up to
    // wading depth, a ferry's jetties and raft (with the generated ferries' below).
    let hand: Vec<(CrossingShape, f32)> = world
        .file
        .edits
        .crossings
        .values()
        .filter(|c| {
            let r = 0.5 * c.width + JETTY_FT + 40.0;
            overlaps((c.a[0].min(c.b[0]) - r, c.a[1].min(c.b[1]) - r, c.a[0].max(c.b[0]) + r, c.a[1].max(c.b[1]) + r)) && c.problem(g.map_w_ft, g.map_h_ft).is_none()
        })
        .map(|c| hand_crossing(c, t0))
        .collect();
    for (c, top) in hand.iter().filter(|(c, _)| c.kind != ShapeKind::Ferry) {
        let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
        let len = crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
        let r = 0.5 * c.size + SQUARE_FT;
        let (i0, j0, i1, j1) = range((c.a[0].min(c.b[0]) - r, c.a[1].min(c.b[1]) - r, c.a[0].max(c.b[0]) + r, c.a[1].max(c.b[1]) + r));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let p = center(i, j);
                let (dx, dy) = (p[0] - c.a[0], p[1] - c.a[1]);
                let (along, across) = (dx * u[0] + dy * u[1], -dx * u[1] + dy * u[0]);
                if along < 0.0 || along > len || across.abs() > 0.5 * c.size {
                    continue;
                }
                if c.kind == ShapeKind::RoadBridge && hh[h] < *top - 0.5 {
                    lift[h] += *top - hh[h];
                    hh[h] = *top;
                    road_h[h] = 0x80 | 3;
                } else if c.kind == ShapeKind::Ford && hw[h] > hh[h] && road_h[h] & 0x80 == 0 {
                    hh[h] = hh[h].max(hw[h] - crate::lod::terrain_refine::FORD_DEPTH_FT);
                }
            }
        }
    }
    // A ferry's jetties and raft are decks too: planks over the water (the raft afloat).
    for c in crossing_shapes.iter().chain(hand.iter().map(|(c, _)| c)).filter(|c| c.kind == ShapeKind::Ferry) {
        let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
        let mut decks: Vec<([f64; 2], f64, f64, f32)> = vec![
            ([c.a[0] - u[0] * 0.5 * JETTY_FT, c.a[1] - u[1] * 0.5 * JETTY_FT], JETTY_FT, JETTY_WIDTH_FT, DECK_CLEARANCE_FT),
            ([c.b[0] + u[0] * 0.5 * JETTY_FT, c.b[1] + u[1] * 0.5 * JETTY_FT], JETTY_FT, JETTY_WIDTH_FT, DECK_CLEARANCE_FT),
        ];
        if let Some((at, l, w)) = c.raft {
            decks.push((at, l, w, 1.0));
        }
        for (at, l, w, above) in decks {
            let r = 0.5 * l.max(w) + SQUARE_FT;
            let (i0, j0, i1, j1) = range((at[0] - r, at[1] - r, at[0] + r, at[1] + r));
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    let h = j * HS + i;
                    let p = center(i, j);
                    let (dx, dy) = (p[0] - at[0], p[1] - at[1]);
                    let (along, across) = (dx * u[0] + dy * u[1], -dx * u[1] + dy * u[0]);
                    if along.abs() <= 0.5 * l && across.abs() <= 0.5 * w && hw[h] > hh[h] {
                        let top = hw[h] + above;
                        lift[h] += top - hh[h];
                        hh[h] = top;
                        road_h[h] = 0x80 | 3;
                    }
                }
            }
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
    // Road crossings as vectors (`CrossingShape`); the chunk owning each draws it, except a
    // ford, whose stepping stones each chunk lays where they fall in it: every ~3.5 ft along
    // the road's centre line, a little to either side in turn, only on water.
    for (x, c) in crossings.iter().zip(&crossing_shapes) {
        if c.kind == ShapeKind::Ford {
            let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
            let len = crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
            let mut stones = Vec::new();
            // Only across the channel at the crossing (a slanting road would lay a long line).
            let mid = (x.at[0] - c.a[0]) * u[0] + (x.at[1] - c.a[1]) * u[1];
            for m in 0..(len / FORD_STEP_FT) as usize {
                let t = (m as f64 + 0.5) * FORD_STEP_FT;
                if (t - mid).abs() > x.half_span() + 4.0 {
                    continue;
                }
                let jit = |salt: i64| unit(hash3(seed, m as i64, salt, (x.at[0] * 0.5) as i64 ^ (x.at[1] * 0.5) as i64)) - 0.5;
                let side = if m % 2 == 0 { 0.7 } else { -0.7 } + 0.6 * jit(1);
                let along = t + 0.8 * jit(2);
                let p = [c.a[0] + u[0] * along - u[1] * side, c.a[1] + u[1] * along + u[0] * side];
                if !owns(p) {
                    continue;
                }
                let (i, j) = (((p[0] - hx0) / SQUARE_FT) as usize, ((p[1] - hy0) / SQUARE_FT) as usize);
                let h = j.min(HS - 1) * HS + i.min(HS - 1);
                if hw[h] > hh[h] {
                    stones.push(local(&p));
                }
            }
            if !stones.is_empty() {
                shapes.push(VectorShape { kind: ShapeKind::Ford, size: (c.size / SQUARE_FT) as f32, pts: stones });
            }
            continue;
        }
        if !owns(x.at) || c.dropped {
            continue;
        }
        let mut pts = vec![local(&c.a), local(&c.b)];
        if let Some(raft) = c.raft {
            pts.push(local(&raft.0));
        }
        if c.kind == ShapeKind::RoadBridge {
            // Old stone on some, more often on the greater roads (by where it crosses); else timber.
            let rate = [0.45, 0.35, 0.15][x.class as usize];
            let old = unit(hash3(seed, crate::core::round(x.at[0]) as i64, crate::core::round(x.at[1]) as i64, 0x0b5d)) < rate;
            pts.push([old as u8 as f32, 0.0]);
        }
        shapes.push(VectorShape { kind: c.kind, size: (c.size / SQUARE_FT) as f32, pts });
    }
    // Those put down by hand: drawn by the chunk owning their middle; a ford's stones (along
    // its whole line, on water) by each chunk where they fall.
    for (c, _) in &hand {
        let mid = [0.5 * (c.a[0] + c.b[0]), 0.5 * (c.a[1] + c.b[1])];
        if c.kind == ShapeKind::Ford {
            let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
            let len = crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
            let key = (mid[0] * 0.5) as i64 ^ (mid[1] * 0.5) as i64;
            let mut stones = Vec::new();
            for m in 0..(len / FORD_STEP_FT) as usize {
                let jit = |salt: i64| unit(hash3(seed, m as i64, salt, key)) - 0.5;
                let side = if m % 2 == 0 { 0.7 } else { -0.7 } + 0.6 * jit(1);
                let along = (m as f64 + 0.5) * FORD_STEP_FT + 0.8 * jit(2);
                let p = [c.a[0] + u[0] * along - u[1] * side, c.a[1] + u[1] * along + u[0] * side];
                if !owns(p) {
                    continue;
                }
                let (i, j) = (((p[0] - hx0) / SQUARE_FT) as usize, ((p[1] - hy0) / SQUARE_FT) as usize);
                let h = j.min(HS - 1) * HS + i.min(HS - 1);
                if hw[h] > hh[h] {
                    stones.push(local(&p));
                }
            }
            if !stones.is_empty() {
                shapes.push(VectorShape { kind: ShapeKind::Ford, size: (c.size / SQUARE_FT) as f32, pts: stones });
            }
        } else if owns(mid) {
            let mut pts = vec![local(&c.a), local(&c.b)];
            if let Some(raft) = c.raft {
                pts.push(local(&raft.0));
            }
            if c.kind == ShapeKind::RoadBridge {
                pts.push([c.stone as u8 as f32, 0.0]);
            }
            shapes.push(VectorShape { kind: c.kind, size: (c.size / SQUARE_FT) as f32, pts });
        }
    }
    let road_bed = hh.clone();
    let mut biome = vec![Biome::Grassland; n];
    let mut slope = vec![0f64; n];
    // Steps on slopes made by hand, by street segment and way up: (segment start, its direction,
    // street width, the squares' places along it).
    let mut hand_runs: std::collections::BTreeMap<((u32, u32, u32), bool), ([f64; 2], [f64; 2], f64, Vec<f64>)> = std::collections::BTreeMap::new();
    for j in 0..SQ {
        for i in 0..SQ {
            let k = j * SQ + i;
            let (x, y) = (ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT);
            biome[k] = Biome::from_u8(t0.sample_biome(x, y, t0.biome_warp(x, y))[0]);
            let c = sidx(i as i64, j as i64);
            let h = (j + 1) * HS + i + 1;
            let plain = || ((tile.padded[c + 1] - tile.padded[c - 1]) as f64 / 5.0, (tile.padded[c + PADDED] - tile.padded[c - PADDED]) as f64 / 5.0);
            let (gx, gy) = match &shaper {
                Some(s) if [h, h - 1, h + 1, h - HS, h + HS].iter().any(|&q| moved[q]) => {
                    let at = |dx: f64, dy: f64, c: usize| s.at_point(x + dx, y + dy, tile.padded[c], hw[h]) as f64;
                    ((at(2.5, 0.0, c + 1) - at(-2.5, 0.0, c - 1)) / 5.0, (at(0.0, 2.5, c + PADDED) - at(0.0, -2.5, c - PADDED)) / 5.0)
                }
                _ => plain(),
            };
            slope[k] = crate::core::sqrt(gx * gx + gy * gy);
            // Steps on a city street's slope made by hand, or paving again where shaping levelled a
            // flight, judged by the climb along the street (square by square: a pure function of
            // the square, so chunks agree).
            if let Some(Some(st)) = hand_st.get(h) {
                let len = crate::town::geom::dist(st.a, st.b).max(1e-9);
                let u = [(st.b[0] - st.a[0]) / len, (st.b[1] - st.a[1]) / len];
                let (px, py) = plain();
                let (along, was) = ((gx * u[0] + gy * u[1]).abs(), (px * u[0] + py * u[1]).abs());
                let prm = crate::town::stairs::StairParams::DEFAULT;
                let flight = stair_h.as_ref().is_some_and(|s| s[h]);
                // (A flight by the slope's whole grade: its square's nearest line may be a street
                // crossing it. A plateau's top is flat, steps or not before.)
                if flight && slope[k] < prm.g_off && (plat_w[h] >= 0.5 || slope[k] < crate::core::sqrt(px * px + py * py) - HAND_MARGIN) {
                    stair_h.as_mut().expect("a flight")[h] = false;
                } else if !flight && along >= prm.g_on && along >= was + HAND_MARGIN && slope[k] < HAND_CLIFF && urb_h[h] == URBAN_STREET && road_h[h] == 0 && hw[h] <= hh[h] {
                    stair_h.get_or_insert_with(|| vec![false; HS * HS])[h] = true;
                    let t = crate::town::geom::dot([x - st.a[0], y - st.a[1]], u);
                    let up = gx * u[0] + gy * u[1] > 0.0;
                    hand_runs.entry((st.key, up)).or_insert((st.a, u, st.width, Vec::new())).3.push(t);
                }
            }
        }
    }

    // Their treads, square across the street from the bottom up, in step across chunks and across
    // a street's strips side by side (counted along the way up from the world's origin). Each run
    // (a segment's steps, split where they stop for more than a square) is `[along, across]`
    // ranges in world ft on its way up `w` and the normal `[-w.y, w.x]`; runs on one way up side
    // by side or end to end are drawn as one axis.
    let mut runs: Vec<([f64; 2], [f64; 2], [f64; 2])> = Vec::new();
    for (&(_, up), (a, u, width, ts)) in &hand_runs {
        let w = if up { *u } else { [-u[0], -u[1]] };
        let off = crate::town::geom::dot(*a, [-w[1], w[0]]);
        let mut ts = ts.clone();
        ts.sort_by(f64::total_cmp);
        let along = |t: f64| crate::town::geom::dot([a[0] + u[0] * t, a[1] + u[1] * t], w);
        let mut first = 0;
        for k in 1..=ts.len() {
            if k == ts.len() || ts[k] - ts[k - 1] > 1.5 * SQUARE_FT {
                let (s0, s1) = (along(ts[first]), along(ts[k - 1]));
                runs.push((w, [s0.min(s1), s0.max(s1)], [off - 0.5 * width, off + 0.5 * width]));
                first = k;
            }
        }
    }
    let mut group: Vec<usize> = (0..runs.len()).collect();
    fn root(g: &[usize], mut i: usize) -> usize {
        while g[i] != i {
            i = g[i];
        }
        i
    }
    let gap = |a: [f64; 2], b: [f64; 2]| (a[0] - b[1]).max(b[0] - a[1]);
    for i in 0..runs.len() {
        for j in i + 1..runs.len() {
            let (p, q) = (&runs[i], &runs[j]);
            if (p.0[0] - q.0[0]).abs() < 1e-3 && (p.0[1] - q.0[1]).abs() < 1e-3 && gap(p.1, q.1) <= 1.5 * SQUARE_FT && gap(p.2, q.2) <= 1.0 {
                let (a, b) = (root(&group, i), root(&group, j));
                group[a.max(b)] = a.min(b);
            }
        }
    }
    let tread = crate::town::stairs::TREAD;
    for r in (0..runs.len()).filter(|&r| root(&group, r) == r) {
        let w = runs[r].0;
        let nrm = [-w[1], w[0]];
        let (mut s, mut o) = ([f64::MAX, f64::MIN], [f64::MAX, f64::MIN]);
        for q in (0..runs.len()).filter(|&q| root(&group, q) == r) {
            s = [s[0].min(runs[q].1[0]), s[1].max(runs[q].1[1])];
            o = [o[0].min(runs[q].2[0]), o[1].max(runs[q].2[1])];
        }
        let (lo, hi, mid) = (s[0] - 0.75 * SQUARE_FT, s[1] + 0.75 * SQUARE_FT, 0.5 * (o[0] + o[1]));
        let at = |t: f64| [w[0] * t + nrm[0] * mid, w[1] * t + nrm[1] * mid];
        let s0 = lo - crate::core::floor(lo / tread) * tread;
        shapes.push(VectorShape { kind: ShapeKind::Stairs, size: ((o[1] - o[0]) / SQUARE_FT) as f32, pts: vec![local(&at(lo)), local(&at(hi)), [(s0 / SQUARE_FT) as f32, (tread / SQUARE_FT) as f32]] });
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
    // Plateaus are flat: micro-relief fades out over them (empty when nothing is shaped here).
    for (h, &w) in plat_w.iter().enumerate() {
        if w > 0.0 {
            hh[h] = road_bed[h] + (hh[h] - road_bed[h]) * (1.0 - w);
        }
    }
    // A camp's clearing is level: the ground eases to one height from just past the rim to
    // three quarters of the way in (so the tents stand on one tier). That height is the mean
    // of the ground in it when the chunk holds it all (camps are placed so), else the layout's
    // (moved as a building's pad is by ground shaped by hand).
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
            *level + crate::ground::pad_shift(&world.file.edits, t0, poly, *level)
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
    // Parks' ground where their features stand, each square by its place alone (chunks agree): a
    // common's swells (easing out over their radius); a formal garden laid level (eased to the
    // streets over 24 ft in, cut or filled no more than a quarter of the way in, to 8 ft), its raised
    // features then standing on the terrace.
    let at_ease = |h: usize| road_h[h] == 0 && bld_h[h] == 0;
    let mut terraced = vec![false; if terraces.is_empty() { 0 } else { HS * HS }];
    for sw in &swells {
        let (c, r) = (sw.at, sw.reach());
        let (i0, j0, i1, j1) = range((c[0] - r, c[1] - r, c[0] + r, c[1] + r));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                if at_ease(h) && hw[h] <= hh[h] {
                    hh[h] += sw.lift(center(i, j));
                }
            }
        }
    }
    for (pts, level) in &terraces {
        let (i0, j0, i1, j1) = range(bbox(pts));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let din = crate::park::depth(pts, center(i, j));
                if din > 0.0 && at_ease(h) && hw[h] <= hh[h] {
                    // (Cut or filled no more than a quarter of the way in, to 8 ft: where the ground
                    // drawn strays from the plan, the terrace eases to it.)
                    let cap = (0.25 * din).min(8.0) as f32;
                    let target = level.clamp(hh[h] - cap, hh[h] + cap);
                    hh[h] += (target - hh[h]) * smoothstep(0.0, 24.0, din) as f32;
                    hw[h] = hw[h].min(hh[h] - 4.0);
                    terraced[h] = true;
                }
            }
        }
    }
    // Plaza features stand a tier above the paving (a fountain's rim, a planter, a cross's
    // steps), a formal garden's above its terrace: every square whose centre lies within the round.
    let floor = if terraced.is_empty() { Vec::new() } else { hh.clone() };
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
                    hh[h] = if terraced.get(h).is_some_and(|&t| t) { floor[h] } else { road_bed[h] } + (TIER_FT * 1.05) as f32;
                }
            }
        }
    }
    // Then by how far it is from the water's edge, a pond dug to its level (0.3 ft deep at its rim,
    // 0.2 ft deeper a foot in, to 3.5), its banks from 0.3 ft above the water (rising a tenth of a
    // foot a foot) eased out to the ground round it over its band, and no steeper than 0.35 from there
    // out to 30 ft past it, the water's level held a square past the rim (further out, well under the
    // ground: no wet shore drawn); a stream's bed below its water's level along it, its banks cut back
    // no steeper than three in ten for 40 ft and banked up where the ground beside it lies low; a
    // formal garden's parterres sunk inside their hedges; a raised tier 6 ft up, easing out over 20 ft
    // (no steeper than one in two).
    // (A park's own water and the two squares round it: `edges` bit 32, where the ground's drawing
    // keeps its dry fringe off the water's bed.)
    let mut park_wet = vec![false; HS * HS];
    for pond in &ponds {
        let (pts, level) = (&pond.pts, pond.level);
        let b = bbox(pts);
        let r = pond.band;
        const CONE: f64 = crate::park::POND_CONE;
        let (i0, j0, i1, j1) = range((b.0 - r - CONE, b.1 - r - CONE, b.2 + r + CONE, b.3 + r + CONE));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let p = center(i, j);
                let edge = (0..pts.len()).map(|e| crate::town::geom::seg_dist(p, pts[e], pts[(e + 1) % pts.len()])).fold(f64::MAX, f64::min);
                let inside = crate::town::geom::contains(pts, p);
                // (Past the band, other water's ground is left as it is.)
                if !at_ease(h) || (!inside && (edge >= r + CONE || (edge >= r && hw[h] > hh[h] - 1.0))) {
                    continue;
                }
                park_wet[h] |= inside || edge <= 2.0 * SQUARE_FT;
                // (Its own water there, whatever level a sea or river nearby would give it.)
                if inside {
                    hh[h] = level - (0.3 + 0.2 * edge).min(3.5) as f32;
                    hw[h] = level;
                } else {
                    hh[h] = crate::park::pond_bank(hh[h] as f64, edge, level as f64, r) as f32;
                    // (Within the band, 4 ft under the ground, never the land's no-water level: the
                    // water's surface between squares then meets the ground along the rim, not in
                    // steps at the squares.)
                    hw[h] = if edge <= SQUARE_FT {
                        level
                    } else if edge < r {
                        hh[h] - 4.0
                    } else {
                        hw[h].min(hh[h] - 4.0)
                    };
                }
            }
        }
    }
    for pts in &streams {
        let half = 0.5 * crate::park::STREAM_FT;
        const BANK: f64 = crate::park::STREAM_BANK;
        let b = pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, q| (a.0.min(q.0[0]), a.1.min(q.0[1]), a.2.max(q.0[0]), a.3.max(q.0[1])));
        let r = half + BANK;
        let (i0, j0, i1, j1) = range((b.0 - r, b.1 - r, b.2 + r, b.3 + r));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let p = center(i, j);
                if !at_ease(h) || hw[h] > hh[h] {
                    continue;
                }
                // The nearest point of its line, and the water's level there.
                let mut best: Option<(f64, f32)> = None;
                for s in pts.windows(2) {
                    let (q, t) = crate::park::project(s[0].0, s[1].0, p);
                    let d = crate::town::geom::dist(p, q);
                    if best.is_none_or(|x| d < x.0) {
                        best = Some((d, s[0].1 + (s[1].1 - s[0].1) * t as f32));
                    }
                }
                let Some((d, wl)) = best.filter(|x| x.0 < r) else { continue };
                park_wet[h] |= d < half + 2.0 * SQUARE_FT;
                // (The highest its water stands within a square and a half more than that: the water of
                // the square beside it upstream, on a falling stream.)
                let reach = d + 1.5 * SQUARE_FT;
                let mut up = wl;
                for s in pts.windows(2) {
                    let (q, t) = crate::park::project(s[0].0, s[1].0, p);
                    let dq = crate::town::geom::dist(p, q);
                    let ab = crate::town::geom::dist(s[0].0, s[1].0).max(1e-9);
                    if dq < reach {
                        let dt = crate::core::sqrt(reach * reach - dq * dq) / ab;
                        for tt in [(t - dt).max(0.0), (t + dt).min(1.0)] {
                            up = up.max(s[0].1 + (s[1].1 - s[0].1) * tt as f32);
                        }
                    }
                }
                if d < half {
                    hh[h] = hh[h].min(wl - (0.4 + 0.8 * (1.0 - d / half)) as f32);
                    hw[h] = wl;
                } else {
                    // (Cut back three in ten, and banked up where the ground beside it lies low: its
                    // water never stands above its banks.)
                    hh[h] = hh[h].min(crate::park::stream_bank(wl as f64, d - half) as f32);
                    // (0.3 ft over the water beside it out to 15 ft from its edge, then down a tenth of
                    // a foot a foot.)
                    if d < half + 25.0 {
                        hh[h] = hh[h].max(up + (0.3 - 0.1 * (d - half - 15.0).max(0.0)) as f32);
                    }
                    hw[h] = if d < half + SQUARE_FT { wl.min(hh[h] - 0.1) } else { hh[h] - 4.0 };
                }
            }
        }
    }
    for q in &sunk {
        let (i0, j0, i1, j1) = range(bbox(q));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        let (a, b) = crate::park::SUNK_BANK;
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let din = crate::park::depth(q, center(i, j));
                if din > 0.0 && at_ease(h) && hw[h] <= hh[h] {
                    let w = 0.5 * ((din - a) / (b - a)).clamp(0.0, 1.0) + 0.5 * smoothstep(a, b, din);
                    hh[h] -= (crate::park::SUNK_FT * w) as f32;
                    hw[h] = hw[h].min(hh[h] - 4.0);
                }
            }
        }
    }
    for &(c, r) in &mounds {
        let reach = r + 20.0;
        let (i0, j0, i1, j1) = range((c[0] - reach, c[1] - reach, c[0] + reach, c[1] + reach));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let d = crate::town::geom::dist(center(i, j), c);
                if at_ease(h) && d < reach && hw[h] <= hh[h] {
                    hh[h] += (6.0 * (1.0 - smoothstep(r, reach, d))) as f32;
                }
            }
        }
    }
    let road: Vec<u8> = (0..n).map(|k| road_h[(k / SQ + 1) * HS + k % SQ + 1]).collect();
    let interior = |grid: &[f32]| -> Vec<f32> { (0..n).map(|k| grid[(k / SQ + 1) * HS + k % SQ + 1]).collect() };
    let height = interior(&hh);
    let water_level = interior(&hw);
    let dominant = dominant_biome(&biome, &water_level, &height);
    // (A park's path doesn't go on under the water, nor over a step steeper than a cliff's grade
    // to a square beside it: it stops at the edge, but where the step is a raised feature's.)
    let on_feature = |h: usize| features.iter().any(|f| crate::town::geom::dist(center(h % HS, h / HS), f.0) <= f.1);
    let cliff = |h: usize| [1, HS].iter().any(|&o| [h + o, h - o].iter().any(|&nh| (hh[nh] - hh[h]).abs() > 2.75 && !on_feature(nh)));
    let urban: Vec<u8> = (0..n)
        .map(|k| {
            let h = (k / SQ + 1) * HS + k % SQ + 1;
            match urb_h[h] {
                _ if ent_h[h] => URBAN_ENTRANCE,
                URBAN_PATH if hw[h] > hh[h] || cliff(h) => URBAN_PARK,
                u => u,
            }
        })
        .collect();

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
        urban,
        building_polys: polys,
        building_parts: poly_parts,
        roof_lift: lift,
        structures: shapes,
        sprites: Vec::new(),
        yard: Vec::new(),
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
            // (A path's square cut at a cliff is the park's lawn.)
            let mark = match urb_h[(k / SQ + 1) * HS + k % SQ + 1] {
                URBAN_PATH if chunk.urban[k] != URBAN_PATH => URBAN_PARK,
                u => u,
            };
            match mark {
                // A castle's bailey: grass where anything grows (else the ground's own); a path up
                // a motte, gravel.
                URBAN_PLAZA if motte_path[(k / SQ + 1) * HS + k % SQ + 1] => chunk.surface[k] = Surface::Gravel,
                URBAN_PLAZA if bailey_h[(k / SQ + 1) * HS + k % SQ + 1] => {
                    if matches!(chunk.surface[k], Surface::ForestFloor | Surface::Mud | Surface::Grass) {
                        chunk.surface[k] = Surface::Grass;
                    }
                }
                // A village green and a graveyard keep the ground round them (no odd patch
                // in a forest); a town's plaza is paved.
                URBAN_PLAZA if !village => chunk.surface[k] = Surface::Cobble,
                // (Not at the mouth of a way underground on the flight: it keeps the street.)
                URBAN_STREET if chunk.urban[k] == URBAN_STREET && stair_h.as_ref().is_some_and(|s| s[(k / SQ + 1) * HS + k % SQ + 1]) => chunk.surface[k] = Surface::Stairs,
                URBAN_STREET => chunk.surface[k] = if paved { Surface::Cobble } else { Surface::Road },
                URBAN_FIELD => chunk.surface[k] = Surface::Field,
                URBAN_DECK => chunk.surface[k] = if paved { Surface::Cobble } else { Surface::Road },
                URBAN_RUIN | URBAN_YARD => chunk.surface[k] = Surface::Dirt,
                URBAN_WALL | URBAN_TOWER => chunk.surface[k] = Surface::Rock,
                URBAN_PATH => chunk.surface[k] = Surface::Gravel,
                // (A formal garden's lawn is mown grass, whatever grows round the town; a common's is
                // the ground of the land round its middle, whatever the shore beside it.)
                // (A palace's court a mown lawn too.)
                URBAN_PARK if park_h[(k / SQ + 1) * HS + k % SQ + 1] == 2 || court_h[(k / SQ + 1) * HS + k % SQ + 1] => chunk.surface[k] = Surface::Grass,
                URBAN_PARK if park_ix[(k / SQ + 1) * HS + k % SQ + 1] != 0 => {
                    chunk.surface[k] = surface_for(park_biomes[park_ix[(k / SQ + 1) * HS + k % SQ + 1] as usize - 1], slope[k]);
                }
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

    // Road squares (bridges and fords too) two squares round the chunk, and the chunk's
    // streets: scatter keeps its footprint off them, across chunk edges too.
    let road_near: Vec<bool> = (0..RN * RN)
        .map(|h| {
            let (i, j) = ((h % RN) as i64 - 2, (h / RN) as i64 - 2);
            let inside = i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64;
            tile.road_mask.get(sidx(i, j)).is_some_and(|&r| r != 0) || (inside && chunk.urban[j as usize * SQ + i as usize] == URBAN_STREET)
        })
        .collect();
    place_objects(&mut chunk, &biome, &slope, &road_near, seed, gsx, gsy);
    let rowless: Vec<bool> = if churchyards.is_empty() {
        Vec::new()
    } else {
        (0..n).map(|k| churchyards.iter().any(|y| crate::town::geom::contains(y, [ox + ((k % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((k / SQ) as f64 + 0.5) * SQUARE_FT]))).collect()
    };
    place_town_props(&mut chunk, seed, gsx, gsy, &rowless);
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
                chunk.objects.push(Object { kind: Kind::RuinWall, variant, x: x as f32, y: y as f32, rot, scale: 1.0, sprite: 0, turned: false });
            }
        }
        let c = crate::town::geom::centroid(poly);
        let h = hash2(wseed, (c[0] * 2.0) as i64, (c[1] * 2.0) as i64);
        let (x, y) = ((c[0] - ox) / SQUARE_FT, (c[1] - oy) / SQUARE_FT);
        if unit(h) < 0.7 && x >= 0.0 && y >= 0.0 && x < SQ as f64 && y < SQ as f64 {
            chunk.objects.push(Object { kind: Kind::RockPile, variant: (h >> 20) as u8 & 7, x: x as f32, y: y as f32, rot: 0.0, scale: 1.3, sprite: 0, turned: false });
        }
    }
    for st in &statues {
        let (x, y) = ((st[0] - ox) / SQUARE_FT, (st[1] - oy) / SQUARE_FT);
        if x >= 0.0 && y >= 0.0 && x < SQ as f64 && y < SQ as f64 {
            chunk.objects.push(Object { kind: Kind::Statue, variant: 0, x: x as f32, y: y as f32, rot: 0.0, scale: 1.0, sprite: 0, turned: false });
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
        chunk.objects.push(Object { kind: p.kind, variant: p.variant, x: x as f32, y: y as f32, rot: p.rot, scale: p.scale, sprite: 0, turned: false });
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
            chunk.objects.push(Object { kind: Kind::TreeDeciduous, variant: (hash2(seed, x as i64, y as i64) >> 20) as u8 & 7, x: x as f32, y: y as f32, rot: 0.0, scale: 0.9, sprite: 0, turned: false });
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
        // A stall's look is its awning and its goods, rolled apart (sixteen).
        let looks = if kind == Kind::MarketStall { 15 } else { 7 };
        let o = Object { kind, variant: (h >> 20) as u8 & looks, x: x as f32, y: y as f32, rot, scale: 1.0, sprite: 0, turned: false };
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
    // What a square is dressed with: each in the chunk holding it, on open ground (paving, a lawn,
    // a street through the square), never on a roof, a road or the water (its layout keeps clear
    // of buildings); a market cross a little larger than its place, to read as a cross from afar.
    for (p, rot, kind, variant) in dressing {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.building[k] != 0 || chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] || !matches!(chunk.urban[k], URBAN_PLAZA | URBAN_STREET | URBAN_PARK | URBAN_GRAVEYARD) {
            continue;
        }
        let scale = if kind == Kind::MarketCross { MARKET_CROSS_SCALE } else { 1.0 };
        chunk.objects.push(Object { kind, variant, x: x as f32, y: y as f32, rot, scale, sprite: 0, turned: false });
    }
    // What a park sets out: each in the chunk holding it, on its lawn (what stands in a gravel round
    // or across a stream on the gravel or the water too, reeds in the water), never on a roof, a road
    // or reaching over a building.
    for (p, rot, kind, variant, scale) in park_props {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        let wet_ok = matches!(kind, Kind::Reeds | Kind::Footbridge);
        let gravel_ok = matches!(kind, Kind::Fountain | Kind::GardenStatue | Kind::Footbridge);
        if chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || (!wet_ok && chunk.water_level[k] > chunk.height[k]) || !(chunk.urban[k] == URBAN_PARK || (gravel_ok && chunk.urban[k] == URBAN_PATH)) {
            continue;
        }
        let o = Object { kind, variant, x: x as f32, y: y as f32, rot, scale, sprite: 0, turned: false };
        if off_buildings(&chunk, &o) {
            chunk.objects.push(o);
        }
    }
    // What a bailey is dressed with: each in the chunk holding it, on the bailey, off the roofs.
    for (p, rot, kind, variant, scale) in bailey_props {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] || chunk.urban[k] != URBAN_PLAZA || bailey_kind[(k / SQ + 1) * HS + k % SQ + 1] & 3 == 0 {
            continue;
        }
        let o = Object { kind, variant, x: x as f32, y: y as f32, rot, scale, sprite: 0, turned: false };
        if off_buildings(&chunk, &o) {
            chunk.objects.push(o);
        }
    }
    // A well at the heart of every main square and village green.
    for w in wells {
        let (i, j) = (((w[0] - ox) / SQUARE_FT).floor(), ((w[1] - oy) / SQUARE_FT).floor());
        if i >= 0.0 && j >= 0.0 && (i as usize) < SQ && (j as usize) < SQ {
            let k = j as usize * SQ + i as usize;
            if chunk.building[k] == 0 && chunk.water_level[k] <= chunk.height[k] {
                chunk.objects.push(Object { kind: Kind::Well, variant: 0, x: ((w[0] - ox) / SQUARE_FT) as f32, y: ((w[1] - oy) / SQUARE_FT) as f32, rot: 0.0, scale: 1.0, sprite: 0, turned: false });
            }
        }
    }
    // Nothing generated stands on a wall or a tower, or in a gate's passage (a cart under the
    // portcullis); nor does what the guarantee adds below (`keep_off`: those squares, the ones next
    // to a wall or a tower, and a square's raised features with the walk round them).
    // (A footprint `r` ft round reaching into a gate's passage.)
    let in_gate = |p: [f64; 2], r: f64| {
        gates.iter().any(|&(g, h)| {
            let u = unit2([h[0] - g[0], h[1] - g[1]]);
            let len = crate::core::sqrt((h[0] - g[0]) * (h[0] - g[0]) + (h[1] - g[1]) * (h[1] - g[1]));
            let (dx, dy) = (p[0] - g[0], p[1] - g[1]);
            let (along, across) = (dx * u[0] + dy * u[1], -dx * u[1] + dy * u[0]);
            along > 12.0 - r && along < len - 12.0 + r && across.abs() < 14.0 + r
        })
    };
    let blocked = |urban: &[u8], o: &Object| on_wall(urban, o) || in_gate([ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT], (info(o.kind as u16).radius * o.scale) as f64 * SQUARE_FT);
    {
        let urban = &chunk.urban;
        chunk.objects.retain(|o| !blocked(urban, o));
    }
    // A close's churchyard and its hedge (a square round it): nothing added, no knoll over it.
    let churchyard: Vec<bool> = if churchyards.is_empty() {
        Vec::new()
    } else {
        (0..n)
            .map(|k| {
                let p = [ox + ((k % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((k / SQ) as f64 + 0.5) * SQUARE_FT];
                churchyards.iter().any(|y| crate::town::geom::contains(y, p) || (0..y.len()).any(|e| crate::town::geom::seg_dist(p, y[e], y[(e + 1) % y.len()]) < 1.5 * SQUARE_FT))
            })
            .collect()
    };
    let keep_off: Vec<bool> = (0..n)
        .map(|k| {
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            let wall = |x: i64, y: i64| x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && matches!(chunk.urban[y as usize * SQ + x as usize], URBAN_WALL | URBAN_TOWER);
            let p = [ox + (x as f64 + 0.5) * SQUARE_FT, oy + (y as f64 + 0.5) * SQUARE_FT];
            (-1..=1).any(|dy| (-1..=1).any(|dx| wall(x + dx, y + dy)))
                || in_gate(p, 2.5)
                || features.iter().any(|f| crate::town::geom::dist(p, f.0) < f.1 + 4.0)
                || churchyard.get(k).is_some_and(|&y| y)
                || court_h[(y as usize + 1) * HS + x as usize + 1]
                || chunk.urban[k] == URBAN_PATH
                || park_keep.iter().any(|&(a, b, r)| crate::town::geom::seg_dist(p, a, b) < r + 2.5)
                || bailey_kind[(y as usize + 1) * HS + x as usize + 1] & 4 != 0
                || (bailey_kind[(y as usize + 1) * HS + x as usize + 1] & 3 != 0 && bailey_ways.iter().any(|&(a, b)| crate::town::geom::seg_dist(p, a, b) < 8.0))
        })
        .collect();
    // (The baileys' squares, as `bailey_kind` has them; empty: none here.)
    let bailey: Vec<u8> = if bailey_kind.iter().all(|&v| v == 0) { Vec::new() } else { (0..n).map(|k| bailey_kind[(k / SQ + 1) * HS + k % SQ + 1]).collect() };
    // The parks' squares (empty: none here): the low bits 1 a common's, 2 a formal garden's (what
    // the guarantee puts down there is a park's); 4 by a pond or stream (no knoll: its banks stay
    // eased, by the band and 8 ft more); 8 gravel at a path's or round's edge, clear of features,
    // where cover may go when the lawn is out of reach (a big round's middle); 16 off the parks but
    // within 8 squares of one (no knoll or feature off a lawn there).
    let park: Vec<u8> = if park_h.iter().all(|&v| v == 0) {
        Vec::new()
    } else {
        let half = 0.5 * crate::park::STREAM_FT;
        let rmax: Vec<f64> = ponds.iter().map(|pd| pd.pts.iter().map(|&q| crate::town::geom::dist(q, pd.centre)).fold(0.0, f64::max)).collect();
        let mut marks: Vec<u8> = (0..n)
            .map(|k| {
                let kind = park_h[(k / SQ + 1) * HS + k % SQ + 1];
                if kind == 0 {
                    return 0;
                }
                let (x, y) = (k % SQ, k / SQ);
                let p = [ox + (x as f64 + 0.5) * SQUARE_FT, oy + (y as f64 + 0.5) * SQUARE_FT];
                let by_pond = ponds.iter().zip(&rmax).any(|(pd, &rmax)| {
                    let reach = pd.band + 8.0;
                    crate::town::geom::dist(p, pd.centre) < rmax + reach
                        && (crate::town::geom::contains(&pd.pts, p) || (0..pd.pts.len()).any(|e| crate::town::geom::seg_dist(p, pd.pts[e], pd.pts[(e + 1) % pd.pts.len()]) < reach))
                });
                let by_stream = streams.iter().any(|s| s.windows(2).any(|w| crate::town::geom::seg_dist(p, w[0].0, w[1].0) < half + crate::park::STREAM_BANK + 8.0));
                let lawn = |dx: i64, dy: i64| {
                    let (i, j) = (x as i64 + dx, y as i64 + dy);
                    i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && chunk.urban[j as usize * SQ + i as usize] == URBAN_PARK
                };
                let edge = chunk.urban[k] == URBAN_PATH
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| lawn(dx, dy))
                    && !features.iter().any(|f| crate::town::geom::dist(p, f.0) < f.1 + 4.0)
                    && !in_gate(p, 2.5);
                kind | if by_pond || by_stream { 4 } else { 0 } | if edge { 8 } else { 0 }
            })
            .collect();
        // (16: off the parks, within 8 squares of one, either way.)
        let near: Vec<bool> = (0..n)
            .map(|k| {
                let x = k % SQ;
                (x.saturating_sub(8)..=(x + 8).min(SQ - 1)).any(|i| marks[k - x + i] & 3 != 0)
            })
            .collect();
        for k in 0..n {
            let (x, y) = (k % SQ, k / SQ);
            if marks[k] & 3 == 0 && (y.saturating_sub(8)..=(y + 8).min(SQ - 1)).any(|j| near[j * SQ + x]) {
                marks[k] |= 16;
            }
        }
        marks
    };
    // Where the ground is shaped, the guarantee's draws are keyed by cell (a mound in one corner
    // moves nothing elsewhere) and plateaus get no knolls (`shaped`: empty when nothing is).
    let shaped: Vec<bool> = if shaper.is_some() { (0..n).map(|k| plat_w[(k / SQ + 1) * HS + k % SQ + 1] >= 0.5).collect() } else { Vec::new() };
    let before = if shaped.is_empty() { Vec::new() } else { chunk.height.clone() };
    let set_out = chunk.objects.len();
    chunk.yard = bailey.clone();
    enforce(&mut chunk, &biome, seed, gsx, gsy, sea, &shaped, &keep_off, &park, &bailey);
    // A bench the guarantee put in a park is one of the park's, facing its nearest path, or the
    // middle of the gravel round it stands at; one out on the lawn, more than 12 ft from either, is a
    // shrub instead. Its flowerbeds lie square to a formal garden's axis.
    for o in chunk.objects[set_out..].iter_mut().filter(|o| matches!(o.kind, Kind::Bench | Kind::Flowerbed)) {
        let p = [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
        let Some(m) = park_meta.iter().find(|m| crate::town::geom::contains(&m.poly, p)) else { continue };
        if o.kind == Kind::Flowerbed {
            if let Some(a) = m.axis {
                // (The quarter turn of the axis nearest its own.)
                let q = std::f32::consts::FRAC_PI_2;
                o.rot = a + q * crate::core::round(((o.rot - a) / q) as f64) as f32;
            }
            continue;
        }
        o.variant = m.bench;
        let path = m.segs.iter().map(|&(a, b)| crate::park::project(a, b, p).0).map(|q| (crate::town::geom::dist(q, p), q));
        let round = m.rounds.iter().map(|&(c, r)| ((crate::town::geom::dist(c, p) - r).abs(), c));
        match path.chain(round).min_by(|a, b| a.0.total_cmp(&b.0)) {
            Some((d, q)) if d <= 12.0 => {
                if crate::town::geom::dist(q, p) > 1.0 {
                    o.rot = crate::park::facing(crate::town::geom::sub(q, p));
                }
            }
            _ => (o.kind, o.variant) = (Kind::Bush, o.variant % 4),
        }
    }
    // A path square the guarantee left at a step steeper than a cliff's grade (a knoll beside it)
    // is the park's lawn: the path stops at the edge (but at a raised feature's step).
    if !park.is_empty() {
        // (Raised features' rounds, centre and radius squared plus half a square, in squares.)
        let raised: Vec<([f32; 2], f32)> = chunk
            .structures
            .iter()
            .filter(|s| s.kind == ShapeKind::Dais)
            .map(|s| {
                let (dx, dy) = (s.pts[1][0] - s.pts[0][0], s.pts[1][1] - s.pts[0][1]);
                let r = crate::core::sqrt((dx * dx + dy * dy) as f64) as f32 + 0.5;
                (s.pts[0], r * r)
            })
            .collect();
        let feature = |k: usize| {
            let (x, y) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
            raised.iter().any(|&(c, r2)| (x - c[0]) * (x - c[0]) + (y - c[1]) * (y - c[1]) <= r2)
        };
        let cut: Vec<usize> = (0..n)
            .filter(|&k| chunk.urban[k] == URBAN_PATH)
            .filter(|&k| {
                let (x, y) = (k % SQ, k / SQ);
                [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                    let (i, j) = (x as i64 + dx, y as i64 + dy);
                    i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                        let nk = j as usize * SQ + i as usize;
                        (chunk.height[nk] - chunk.height[k]).abs() > 2.75 && !feature(nk) && !feature(k)
                    }
                })
            })
            .collect();
        for k in cut {
            let h = (k / SQ + 1) * HS + k % SQ + 1;
            chunk.urban[k] = URBAN_PARK;
            chunk.surface[k] = match (park_h[h], park_ix[h]) {
                (2, _) => Surface::Grass,
                (_, ix) if ix != 0 => surface_for(park_biomes[ix as usize - 1], slope[k]),
                _ => chunk.surface[k],
            };
        }
    }
    // What it added in a park stands wholly on its lawn: one reaching over the edge (a statue over the
    // street) stands in the middle of its square, smaller, or is a bush there (the cover kept).
    let p = |o: &Object| [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
    let over = |o: &Object, poly: &[[f64; 2]]| crate::park::depth(poly, p(o)) < (info(o.kind as u16).radius * o.scale) as f64 * SQUARE_FT;
    for o in chunk.objects[set_out..].iter_mut().filter(|o| matches!(o.kind, Kind::GardenStatue | Kind::Topiary | Kind::Flowerbed | Kind::Bench)) {
        if let Some(m) = park_meta.iter().find(|m| crate::town::geom::contains(&m.poly, p(o)))
            && over(o, &m.poly)
        {
            (o.scale, o.x, o.y) = (0.85, o.x.floor() + 0.5, o.y.floor() + 0.5);
            if over(o, &m.poly) {
                (o.kind, o.variant) = (Kind::Bush, o.variant % 4);
            }
        }
    }
    // What it added that reaches a wall, a tower or a gate's passage (a cart by it) stands in the
    // middle of its square, smaller (a crate on paving), or goes.
    for k in (set_out..chunk.objects.len()).rev() {
        if !blocked(&chunk.urban, &chunk.objects[k]) {
            continue;
        }
        let o = &mut chunk.objects[k];
        let q = (o.y.max(0.0) as usize).min(SQ - 1) * SQ + (o.x.max(0.0) as usize).min(SQ - 1);
        (o.scale, o.x, o.y) = (0.85, o.x.floor() + 0.5, o.y.floor() + 0.5);
        if chunk.surface[q].is_road() {
            o.kind = Kind::Crate;
        }
        if blocked(&chunk.urban, &chunk.objects[k]) {
            chunk.objects.remove(k);
        }
    }
    // Enforcement knolls may have spilled onto a road bed or a building; those keep their
    // level (on a street or plaza a raised terrace or dais is fine), as does a plateau.
    let mut touched = false;
    for k in (0..shaped.len()).filter(|&k| shaped[k]) {
        if chunk.height[k] != before[k] {
            chunk.height[k] = before[k];
            touched = true;
        }
    }
    for k in 0..n {
        let h = (k / SQ + 1) * HS + k % SQ + 1;
        let bed = road_bed[h];
        if (chunk.road_halo[h] != 0 || chunk.building[k] != 0 || churchyard.get(k).is_some_and(|&y| y)) && chunk.height[k] != bed {
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
    for k in 0..n {
        if park_wet[(k / SQ + 1) * HS + k % SQ + 1] {
            chunk.edges[k] |= 32;
        }
    }
    chunk
}

/// Whether an object's footprint disc touches a square of the chunk for which `hit` holds.
fn touches_square(o: &Object, hit: impl Fn(usize) -> bool) -> bool {
    let r = info(o.kind as u16).radius * o.scale;
    let span = |v: f32| ((v - r).max(0.0) as usize, ((v + r).max(0.0) as usize).min(SQ - 1));
    let ((x0, x1), (y0, y1)) = (span(o.x), span(o.y));
    (y0..=y1).any(|y| {
        (x0..=x1).any(|x| {
            let (dx, dy) = (o.x - o.x.clamp(x as f32, x as f32 + 1.0), o.y - o.y.clamp(y as f32, y as f32 + 1.0));
            dx * dx + dy * dy < r * r && hit(y * SQ + x)
        })
    })
}

/// Whether an object's footprint touches a wall or tower square (it would be drawn over them).
fn on_wall(urban: &[u8], o: &Object) -> bool {
    touches_square(o, |k| matches!(urban[k], URBAN_WALL | URBAN_TOWER))
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
        let mut o = Object { kind: Kind::Crate, variant: p.variant, x, y, rot, scale, sprite: 0, turned: p.turned };
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
    const N: usize = crate::t0::biome::ALL.len();
    let mut counts = [0usize; N];
    for k in 0..biome.len() {
        if water[k] <= height[k] {
            counts[biome[k] as usize] += 1;
        }
    }
    let best = (0..N).max_by_key(|&b| (counts[b], N - b)).unwrap_or(8);
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

/// Side of `place_objects`' road grid: the chunk and two squares round it.
const RN: usize = SQ + 4;

fn place_objects(c: &mut Chunk, biome: &[Biome], slope: &[f64], road_near: &[bool], seed: u64, gsx: i64, gsy: i64) {
    let grove_seed = hash2(seed, 0x6007, 2);
    // Each (biome, kind) pair has its own jittered global lattice; objects belong to the
    // chunk that contains them, so placement is identical whichever chunk is asked.
    let mut kinds: Vec<(Biome, Kind, f64, bool)> = Vec::new();
    let mut seen = [false; crate::t0::biome::ALL.len()];
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
                // Canopies and boulders keep clear of buildings (roofs stay visible), and
                // nothing overhangs a road or street: no square of one within the footprint.
                let r = info(kind as u16).radius as f64;
                let reach = crate::core::ceil(r * 1.25) as i64;
                let (ox, oy) = (px - gsx as f64, py - gsy as f64);
                let blocked = (-reach..=reach).any(|dy| {
                    (-reach..=reach).any(|dx| {
                        let (x, y) = (ix + dx, iy + dy);
                        if x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && c.building[y as usize * SQ + x as usize] != 0 {
                            return true;
                        }
                        if x < -2 || y < -2 || x >= SQ as i64 + 2 || y >= SQ as i64 + 2 || !road_near[(y + 2) as usize * RN + (x + 2) as usize] {
                            return false;
                        }
                        let ex = (x as f64 - ox).max(ox - (x + 1) as f64).max(0.0);
                        let ey = (y as f64 - oy).max(oy - (y + 1) as f64).max(0.0);
                        ex * ex + ey * ey < 1.21 * r * r
                    })
                });
                if blocked {
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
                    turned: false,
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

/// No road bed on this square or round it, nor a street round open ground (a rock or a bush
/// here doesn't overhang the way; a town's props still stand by its streets).
fn clear_of_roads(c: &Chunk, k: usize) -> bool {
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    (-1..=1).all(|dy| {
        (-1..=1).all(|dx| {
            let (nx, ny) = (x + dx, y + dy);
            let inside = nx >= 0 && ny >= 0 && nx < SQ as i64 && ny < SQ as i64;
            c.road_halo[(ny + 1) as usize * HS + (nx + 1) as usize] == 0 && !(inside && c.urban[k] == 0 && c.urban[ny as usize * SQ + nx as usize] == URBAN_STREET)
        })
    })
}

/// The squares of `all` clear of roads (and off what is set out there already, a square's raised
/// features and their steps), else those off the road bed, else all of them; never one in
/// `keep_off` (walls, towers, gate passages and the squares next to them; raised features).
fn off_road_first(c: &Chunk, all: Vec<usize>, keep_off: &[bool]) -> Vec<usize> {
    // Squares under an object's footprint, within the bounds of `all`.
    let (x0, y0, x1, y1) = all.iter().fold((SQ, SQ, 0, 0), |a, &k| (a.0.min(k % SQ), a.1.min(k / SQ), a.2.max(k % SQ), a.3.max(k / SQ)));
    let w = (x1 + 1).saturating_sub(x0);
    let mut busy = vec![false; w * (y1 + 1).saturating_sub(y0)];
    for o in &c.objects {
        let r = c.info(o).radius * o.scale;
        if o.x + r < x0 as f32 || o.y + r < y0 as f32 || o.x - r > (x1 + 1) as f32 || o.y - r > (y1 + 1) as f32 {
            continue;
        }
        for y in (o.y - r).max(y0 as f32) as usize..=((o.y + r) as usize).min(y1) {
            for x in (o.x - r).max(x0 as f32) as usize..=((o.x + r) as usize).min(x1) {
                let (dx, dy) = (o.x - (x as f32 + 0.5), o.y - (y as f32 + 0.5));
                if dx * dx + dy * dy < (r + 0.5) * (r + 0.5) {
                    busy[(y - y0) * w + x - x0] = true;
                }
            }
        }
    }
    let open = |k: usize| !busy[(k / SQ - y0) * w + k % SQ - x0];
    let daises: Vec<([f64; 2], f64)> = c
        .structures
        .iter()
        .filter(|s| s.kind == ShapeKind::Dais)
        .map(|s| {
            let (d, (dx, dy)) = ([s.pts[0][0] as f64, s.pts[0][1] as f64], ((s.pts[1][0] - s.pts[0][0]) as f64, (s.pts[1][1] - s.pts[0][1]) as f64));
            (d, crate::core::sqrt(dx * dx + dy * dy) + 1.0)
        })
        .collect();
    let off_dais = |k: usize| {
        let (x, y) = ((k % SQ) as f64 + 0.5, (k / SQ) as f64 + 0.5);
        daises.iter().all(|&(d, r)| (x - d[0]) * (x - d[0]) + (y - d[1]) * (y - d[1]) > r * r)
    };
    let clear: Vec<usize> = all.iter().copied().filter(|&k| placeable(c, k) && clear_of_roads(c, k) && off_dais(k) && open(k) && !keep_off[k]).collect();
    if !clear.is_empty() {
        return clear;
    }
    let off_road: Vec<usize> = all.iter().copied().filter(|&k| placeable(c, k) && !keep_off[k]).collect();
    if off_road.is_empty() { all.into_iter().filter(|&k| !keep_off[k]).collect() } else { off_road }
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

/// On an open square, the squares of `cells` by its edge (a house or street within two squares),
/// where a stack of crates stands against a front rather than out on the paving; else all.
fn by_fronts(c: &Chunk, cells: Vec<usize>) -> Vec<usize> {
    if cells.iter().filter(|&&k| c.urban[k] == URBAN_PLAZA).count() * 2 <= cells.len() {
        return cells;
    }
    let edge: Vec<usize> = cells
        .iter()
        .copied()
        .filter(|&k| {
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            (-2..=2).any(|dy| {
                (-2..=2).any(|dx| {
                    let (i, j) = (x + dx, y + dy);
                    i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                        let q = j as usize * SQ + i as usize;
                        c.building[q] != 0 || c.urban[q] == URBAN_STREET
                    }
                })
            })
        })
        .collect();
    if edge.is_empty() { cells } else { edge }
}

/// The 32x32 windows `enforce` fills: those mostly dry with fewer than three cover objects and
/// buildings in them (every building with a square in a window, as `tests/vital.rs` counts); with
/// `feature`, those with no hazard or prop in them. What doesn't change while objects are added
/// (wet squares, the buildings in each window) is worked out once; the objects as they come.
struct Windows {
    feature: bool,
    /// Objects counted so far (objects are only added while `enforce` fills windows).
    counted: usize,
    /// Counted objects by square, and its summed-area table; the summed-area table of wet squares.
    at: Vec<u32>,
    cover: Vec<u32>,
    wet: Vec<u32>,
    /// Buildings in each window, once counted (up to three; `u8::MAX`: not yet).
    blds: Vec<u8>,
    /// The summed-area tables of a castle's yard's squares and of walls, towers and buildings
    /// (empty: no yard here, or `feature`): `works_cover`.
    yard: Vec<u32>,
    works: Vec<u32>,
}

impl Windows {
    const W: usize = 32;

    fn new(c: &Chunk, feature: bool) -> Self {
        let mut wet = vec![0u32; (SQ + 1) * (SQ + 1)];
        for y in 0..SQ {
            for x in 0..SQ {
                let k = y * SQ + x;
                let (i, up, left, diag) = ((y + 1) * (SQ + 1) + x + 1, y * (SQ + 1) + x + 1, (y + 1) * (SQ + 1) + x, y * (SQ + 1) + x);
                wet[i] = (c.water_level[k] > c.height[k]) as u32 + wet[up] + wet[left] - wet[diag];
            }
        }
        let n = SQ - Self::W + 1;
        let (mut yard, mut works) = (Vec::new(), Vec::new());
        if !feature && !c.yard.is_empty() {
            (yard, works) = (vec![0u32; (SQ + 1) * (SQ + 1)], vec![0u32; (SQ + 1) * (SQ + 1)]);
            for y in 0..SQ {
                for x in 0..SQ {
                    let k = y * SQ + x;
                    let (i, up, left, diag) = ((y + 1) * (SQ + 1) + x + 1, y * (SQ + 1) + x + 1, (y + 1) * (SQ + 1) + x, y * (SQ + 1) + x);
                    yard[i] = (c.yard[k] & 3 != 0) as u32 + yard[up] + yard[left] - yard[diag];
                    works[i] = (c.building[k] != 0 || matches!(c.urban[k], URBAN_WALL | URBAN_TOWER)) as u32 + works[up] + works[left] - works[diag];
                }
            }
        }
        Windows { feature, counted: 0, at: vec![0; SQ * SQ], cover: vec![0; (SQ + 1) * (SQ + 1)], wet, blds: vec![u8::MAX; n * n], yard, works }
    }

    fn sum(t: &[u32], x: usize, y: usize) -> u32 {
        let w = Self::W;
        t[(y + w) * (SQ + 1) + x + w] + t[y * (SQ + 1) + x] - t[y * (SQ + 1) + x + w] - t[(y + w) * (SQ + 1) + x]
    }

    /// The first thin window (by row, then column) that `skip` passes over.
    fn first(&mut self, c: &Chunk, skip: impl Fn((usize, usize)) -> bool) -> Option<(usize, usize)> {
        let w = Self::W;
        if c.objects.len() > self.counted {
            for o in c.objects[self.counted..].iter().filter(|o| if self.feature { c.info(o).feature } else { c.info(o).cover >= 1 }) {
                // (One centred off the chunk, past its top or left edge, is not in it.)
                let (x, y) = (o.x as usize, o.y as usize);
                if o.x >= 0.0 && o.y >= 0.0 && x < SQ && y < SQ {
                    self.at[y * SQ + x] += 1;
                }
            }
            self.counted = c.objects.len();
            for y in 0..SQ {
                for x in 0..SQ {
                    let (i, up, left, diag) = ((y + 1) * (SQ + 1) + x + 1, y * (SQ + 1) + x + 1, (y + 1) * (SQ + 1) + x, y * (SQ + 1) + x);
                    self.cover[i] = self.at[y * SQ + x] + self.cover[up] + self.cover[left] - self.cover[diag];
                }
            }
        }
        let need = if self.feature { 1 } else { 3 };
        let n = SQ - w + 1;
        for wy in 0..n {
            for wx in 0..n {
                let have = Self::sum(&self.cover, wx, wy) as usize;
                let wet = Self::sum(&self.wet, wx, wy) as usize;
                if wet * 2 >= w * w || have >= need || skip((wx, wy)) {
                    continue;
                }
                // (A castle's yard holding its curtains, towers or buildings: `works_cover`.)
                if !self.yard.is_empty() && Self::sum(&self.works, wx, wy) > 0 && Self::sum(&self.yard, wx, wy) as usize * 2 >= w * w - wet {
                    continue;
                }
                if !self.feature {
                    let q = wy * n + wx;
                    if self.blds[q] == u8::MAX {
                        let mut seen: Vec<u16> = Vec::new();
                        'scan: for y in wy..wy + w {
                            for x in wx..wx + w {
                                let b = c.building[y * SQ + x];
                                if b != 0 && !seen.contains(&b) {
                                    seen.push(b);
                                    if seen.len() >= need {
                                        break 'scan;
                                    }
                                }
                            }
                        }
                        self.blds[q] = seen.len() as u8;
                    }
                    if have + self.blds[q] as usize >= need {
                        continue;
                    }
                }
                return Some((wx, wy));
            }
        }
        None
    }
}

/// A park as the guarantee's finishing touches need it: its outline, its benches' look, its paths
/// (segments) and gravel rounds, and a formal garden's axis.
struct ParkMeta {
    poly: Vec<[f64; 2]>,
    bench: u8,
    segs: Vec<([f64; 2], [f64; 2])>,
    rounds: Vec<([f64; 2], f64)>,
    axis: Option<f32>,
}

/// `shaped`: empty for ground as generated (one stream for the chunk); else every draw is keyed
/// by its own cell (global), and squares marked (on a plateau) get no knoll or built feature.
/// `park`: the parks' squares (1 a common's, 2 a formal garden's; empty: none).
#[allow(clippy::too_many_arguments)]
fn enforce(c: &mut Chunk, biome: &[Biome], seed: u64, gsx: i64, gsy: i64, sea: f64, shaped: &[bool], keep_off: &[bool], park: &[u8], bailey: &[u8]) {
    let mut chunk_rng = Pcg32::new(hash3(seed, gsx, gsy, 0xe4f), 9);
    let keyed = !shaped.is_empty();
    let cell_rng = |phase: i64, x: i64, y: i64| Pcg32::new(hash3(seed, x, y, 0xe4f ^ (phase << 16)), 9);
    // What goes down for cover, and as a feature: a park's own, the town's, else the biome's.
    let in_park = |k: usize| park.get(k).copied().unwrap_or(0) & 3;
    let mut room: Option<Vec<u8>> = None;
    // (A castle's bailey: `yard_kind`.)
    let yard = |k: usize| bailey.get(k).copied().unwrap_or(0) & 3;
    let yard_major = |cells: &[usize]| !bailey.is_empty() && cells.iter().filter(|&&k| yard(k) != 0).count() * 2 > cells.len();
    let cover_kind = |c: &Chunk, k: usize, rng: &mut Pcg32| match in_park(k) {
        _ if yard(k) != 0 => yard_kind(c, k, yard(k) == 2, biome[k], false, rng),
        0 if !matches!(c.urban[k], 0 | URBAN_PARK) => town_cover(c.urban[k], rng),
        0 => fallback_cover(biome[k], rng),
        p => park_cover(p & 3 == 2, biome[k], false, rng),
    };
    let feature_kind = |c: &Chunk, k: usize, rng: &mut Pcg32| match in_park(k) {
        _ if yard(k) != 0 => yard_kind(c, k, yard(k) == 2, biome[k], true, rng),
        0 if !matches!(c.urban[k], 0 | URBAN_PARK) => town_cover(c.urban[k], rng),
        0 => fallback_feature(biome[k], rng),
        p => park_cover(p & 3 == 2, biome[k], true, rng),
    };

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
            // Mostly open plaza paving, or a park's: left to the windows below (a knoll or a feature
            // where a window is flat, not one to every cell).
            let dry_n = cells.iter().filter(|&&k| dry(c, k)).count();
            if cells.iter().filter(|&&k| dry(c, k) && (c.urban[k] == URBAN_PLAZA || in_park(k) != 0)).count() * 2 >= dry_n {
                deferred = true;
                continue;
            }
            if keyed && cells.iter().any(|&k| shaped[k]) {
                continue;
            }
            let mut own;
            let rng = if keyed {
                own = cell_rng(1, gsx / TIER_CELL as i64 + cx as i64, gsy / TIER_CELL as i64 + cy as i64);
                &mut own
            } else {
                &mut chunk_rng
            };
            raise(c, &cells, rng, gsx, gsy, sea, shaped, park, bailey, &mut room);
        }
    }
    // Plaza paving answers to the guarantee itself rather than cell by cell: every 32x32 window
    // spans two tiers. A square's own features (laid out sparingly in a pattern on a city's
    // square: `plaza_features`) usually see to that; a window still flat gets one in its middle,
    // or anywhere in it if that fails.
    // (In a park, the window is any flat one, by lot, and its middle tried is half again as wide:
    // knolls taken from the first window on, row by row, stood in rows. A window where nothing could
    // be raised doesn't count against the 48.)
    let mut tried: Vec<(usize, usize)> = Vec::new();
    let (mut raised, mut tries) = (0, 0);
    while deferred && raised < 48 && tries < 96 {
        tries += 1;
        let open: Vec<(usize, usize)> = flat_windows(c).into_iter().filter(|w| tried.iter().filter(|t| *t == w).count() < 2).collect();
        let Some(&first) = open.first() else { break };
        let in_park = |w: &(usize, usize)| (0..32 * 32).filter(|&q| in_park((w.1 + q / 32) * SQ + w.0 + q % 32) != 0).count() * 2 >= 32 * 32;
        // (By each window's own lot, keyed by where it lies: a mound shaped elsewhere moves no choice.)
        let lot = |w: &(usize, usize)| hash3(seed, gsx + w.0 as i64, gsy + w.1 as i64, 0x9a7 + raised as i64);
        let (wx, wy) = if in_park(&first) { *open.iter().min_by_key(|w| (lot(w), **w)).expect("a window") } else { first };
        let span = match (tried.contains(&(wx, wy)), in_park(&(wx, wy))) {
            (true, _) => (0, 32),
            (false, true) => (4, 24),
            (false, false) => (8, 16),
        };
        tried.push((wx, wy));
        let cells: Vec<usize> = (0..span.1 * span.1).map(|q| (wy + span.0 + q / span.1) * SQ + wx + span.0 + q % span.1).collect();
        let mut own;
        let rng = if keyed {
            own = cell_rng(2 + span.0 as i64, gsx + wx as i64, gsy + wy as i64);
            &mut own
        } else {
            &mut chunk_rng
        };
        raised += raise(c, &cells, rng, gsx, gsy, sea, shaped, park, bailey, &mut room) as usize;
    }


    // Cover: every 8x8 cell has at least one cover object (but open squares: there the windows and
    // the gap below see to it, so crates aren't strewn one to a cell over the paving).
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
            if all.iter().filter(|&&k| c.urban[k] == URBAN_PLAZA).count() * 2 > all.len() {
                continue;
            }
            let cells = off_road_first(c, all, keep_off);
            if cells.is_empty() {
                continue;
            }
            let mut own;
            let rng = if keyed {
                own = cell_rng(4, gsx / COVER_CELL as i64 + cx as i64, gsy / COVER_CELL as i64 + cy as i64);
                &mut own
            } else {
                &mut chunk_rng
            };
            place_in(c, &cells, rng, &cover_kind);
        }
    }

    // Every 32x32 window that is mostly dry holds three cover objects or buildings: a window
    // whose cells' cover lies outside it (by water, or in a corner of each cell) gets more in its
    // middle.
    // (Only what is put down counts against the 64: a window with nowhere for anything doesn't.)
    let mut tried = vec![0u8; (SQ - 31) * (SQ - 31)];
    let mut windows = Windows::new(c, false);
    let (mut filled, mut tries) = (0, 0);
    while filled < 64 && tries < 4096 {
        tries += 1;
        let Some((wx, wy)) = windows.first(c, |w| tried[w.1 * (SQ - 31) + w.0] >= 2) else { break };
        let span = if tried[wy * (SQ - 31) + wx] > 0 { (0, 32) } else { (8, 16) };
        tried[wy * (SQ - 31) + wx] += 1;
        let all: Vec<usize> = (0..span.1 * span.1).map(|q| (wy + span.0 + q / span.1) * SQ + wx + span.0 + q % span.1).filter(|&k| dry(c, k) && c.building[k] == 0).collect();
        if all.is_empty() {
            continue;
        }
        let cells = by_fronts(c, off_road_first(c, all, keep_off));
        if cells.is_empty() {
            continue;
        }
        let mut own;
        let rng = if keyed {
            own = cell_rng(7 + span.0 as i64, gsx + wx as i64, gsy + wy as i64);
            &mut own
        } else {
            &mut chunk_rng
        };
        let before = c.objects.len();
        if place_in(c, &cells, rng, &cover_kind) {
            filled += 1;
            yard_group(c, before, rng, keep_off, bailey);
        }
    }

    // Max gap: no dry square farther than MAX_COVER_GAP from cover (YARD_COVER_GAP in a castle's
    // yard: `cover_gap`; a square with no room for any near it, off walls, gates and raised
    // features, is given up).
    let mut given_up: Vec<usize> = Vec::new();
    for _ in 0..160 {
        let dist = cover_distance(c);
        let far = (0..SQ * SQ).filter(|&k| dry(c, k) && dist[k] > cover_gap(c, k) && !given_up.contains(&k)).max_by_key(|&k| (dist[k] - cover_gap(c, k), std::cmp::Reverse(k)));
        match far {
            Some(k) => {
                let mut own;
                let rng = if keyed {
                    own = cell_rng(5, gsx + (k % SQ) as i64, gsy + (k / SQ) as i64);
                    &mut own
                } else {
                    &mut chunk_rng
                };
                // On a road, put the cover on the nearest verge instead.
                let (k, edge) = if !placeable(c, k) || keep_off[k] {
                    match nearest_verge(c, k, keep_off) {
                        Some(v) => (v, false),
                        // (In a park's gravel round, its nearest edge: a statue in a formal garden, a
                        // bench on a common, off the path's middle.)
                        None => match gravel_edge(c, k, park) {
                            Some(v) => (v, true),
                            None => {
                                given_up.push(k);
                                continue;
                            }
                        },
                    }
                } else {
                    (k, false)
                };
                let kind = if edge { if in_park(k) & 3 == 2 { Kind::GardenStatue } else { Kind::Bench } } else { cover_kind(c, k, rng) };
                let before = c.objects.len();
                if !push_at(c, kind, k, rng) {
                    given_up.push(k);
                } else {
                    yard_group(c, before, rng, keep_off, bailey);
                }
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
                // (Not one to every cell of a castle's yard: the windows below see to it there.)
                if yard_major(&all) {
                    continue;
                }
                let cells = off_road_first(c, all, keep_off);
                if cells.is_empty() {
                    continue;
                }
                let mut own;
                let rng = if keyed {
                    own = cell_rng(6, gsx / TIER_CELL as i64 + cx as i64, gsy / TIER_CELL as i64 + cy as i64);
                    &mut own
                } else {
                    &mut chunk_rng
                };
                place_in(c, &cells, rng, &feature_kind);
            }
        }
        // And in every mostly dry 32x32 window (whose cells' props may lie outside it).
        let mut tried = vec![0u8; (SQ - 31) * (SQ - 31)];
        let mut windows = Windows::new(c, true);
        let (mut filled, mut tries) = (0, 0);
        while filled < 64 && tries < 4096 {
            tries += 1;
            let Some((wx, wy)) = windows.first(c, |w| tried[w.1 * (SQ - 31) + w.0] >= 2) else { break };
            let span = if tried[wy * (SQ - 31) + wx] > 0 { (0, 32) } else { (8, 16) };
            tried[wy * (SQ - 31) + wx] += 1;
            let all: Vec<usize> = (0..span.1 * span.1).map(|q| (wy + span.0 + q / span.1) * SQ + wx + span.0 + q % span.1).filter(|&k| dry(c, k) && c.building[k] == 0).collect();
            // (In a castle's yard, by the buildings and the streets at first.)
            let cells = if span.0 > 0 && yard_major(&all) { by_fronts(c, off_road_first(c, all, keep_off)) } else { off_road_first(c, all, keep_off) };
            if cells.is_empty() {
                continue;
            }
            let mut own;
            let rng = if keyed {
                own = cell_rng(9 + span.0 as i64, gsx + wx as i64, gsy + wy as i64);
                &mut own
            } else {
                &mut chunk_rng
            };
            filled += place_in(c, &cells, rng, &feature_kind) as usize;
        }
    }
}

/// What the guarantee put down at `c.objects[at]` in a castle's yard, grouped: a store or two
/// beside it (barrels and crates by a cart or a stack of them, firewood by firewood, a barrel by a
/// trough), not strewn one by one.
fn yard_group(c: &mut Chunk, at: usize, rng: &mut Pcg32, keep_off: &[bool], bailey: &[u8]) {
    let Some(o) = c.objects.get(at) else { return };
    let (x, y) = (o.x as i64, o.y as i64);
    let yard = |k: usize| bailey.get(k).copied().unwrap_or(0) & 3 != 0;
    if x < 0 || y < 0 || x >= SQ as i64 || y >= SQ as i64 || !yard(y as usize * SQ + x as usize) {
        return;
    }
    let with: &[Kind] = match o.kind {
        Kind::Cart => &[Kind::Barrel, Kind::Crate],
        Kind::Crate | Kind::Barrel => &[Kind::Crate, Kind::Barrel],
        Kind::Firewood => &[Kind::Firewood],
        Kind::WaterTrough => &[Kind::Barrel],
        Kind::Haystack => &[Kind::Firewood],
        _ => &[],
    };
    let mut n = 0;
    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
        if n >= with.len() {
            break;
        }
        let (i, j) = (x + dx, y + dy);
        if i < 0 || j < 0 || i >= SQ as i64 || j >= SQ as i64 {
            continue;
        }
        let k = j as usize * SQ + i as usize;
        let busy = c.objects.iter().any(|q| q.x as i64 == i && q.y as i64 == j);
        if yard(k) && placeable(c, k) && !keep_off[k] && !busy && push_at(c, with[n], k, rng) {
            n += 1;
        }
    }
}

/// Something (`kind_of` the square) put down on one of `cells`, from a random one on, the first
/// where it fits off the buildings (at most 16 tried). Whether it did.
fn place_in(c: &mut Chunk, cells: &[usize], rng: &mut Pcg32, kind_of: &dyn Fn(&Chunk, usize, &mut Pcg32) -> Kind) -> bool {
    if cells.is_empty() {
        return false;
    }
    let start = rng.below(cells.len() as u32) as usize;
    for q in 0..cells.len().min(16) {
        let k = cells[(start + q) % cells.len()];
        let kind = kind_of(c, k, rng);
        if push_at(c, kind, k, rng) {
            return true;
        }
    }
    false
}

/// Make a flat stretch (`cells`) span two tiers: a natural knoll on open ground, a round built
/// feature on paving and in a formal garden (a raised planter or a basin there), at a dry spot far
/// enough inside the chunk that it is never clipped at the chunk border (chunks are generated
/// independently).
/// Squares marked in `shaped` (a plateau) are left alone; `park` as `enforce` has it; `room` the
/// lawns' room (`lawn_room`) while nothing has been raised since it was worked out.
/// Whether anything was raised.
#[allow(clippy::too_many_arguments)]
fn raise(c: &mut Chunk, cells: &[usize], rng: &mut Pcg32, gsx: i64, gsy: i64, sea: f64, shaped: &[bool], park: &[u8], bailey: &[u8], room: &mut Option<Vec<u8>>) -> bool {
    let raised = raise_on(c, cells, rng, gsx, gsy, sea, shaped, park, bailey, room);
    if raised {
        *room = None;
    }
    raised
}

#[allow(clippy::too_many_arguments)]
fn raise_on(c: &mut Chunk, cells: &[usize], rng: &mut Pcg32, gsx: i64, gsy: i64, sea: f64, shaped: &[bool], park: &[u8], bailey: &[u8], room_cache: &mut Option<Vec<u8>>) -> bool {
    // (Never on a motte: it is its own rise. `bailey` bit 4.)
    let interior = |k: usize| {
        let (x, y) = (k % SQ, k / SQ);
        x >= 7 && y >= 7 && x < SQ - 7 && y < SQ - 7 && !shaped.get(k).is_some_and(|&s| s) && !bailey.get(k).is_some_and(|&b| b & 4 != 0)
    };
    let yard = |k: usize| bailey.get(k).copied().unwrap_or(0) & 3;
    let garden = |k: usize| park.get(k).is_some_and(|&p| p & 3 == 2);
    // A lawn's (a green's, a close's, a park's) knoll is a gentle hillock, broad where there is room,
    // all of it on the lawn (no house or street lifted, off the water's edge and a park's pond's or
    // stream's banks, inside the chunk).
    let lawn = |c: &Chunk, k: usize| matches!(c.urban[k], URBAN_PARK | URBAN_PATH);
    let room = if cells.iter().any(|&k| lawn(c, k)) { room_cache.get_or_insert_with(|| lawn_room(c, park)).clone() } else { Vec::new() };
    // Open country, fields and lawns take a natural knoll; streets, plazas and formal gardens a
    // built feature.
    // (Off a park's lawn, a knoll or feature stays 8 squares away from it: its slopes would rise on
    // the lawn and its paths. `park` bit 16.)
    let by_park = |k: usize| park.get(k).is_some_and(|&p| p & 19 != 0);
    let good: Vec<usize> = cells
        .iter()
        .copied()
        .filter(|&k| placeable(c, k) && interior(k) && matches!(c.urban[k], 0 | URBAN_FIELD | URBAN_PARK) && !garden(k) && (c.urban[k] != URBAN_PARK || room[k] >= 7))
        .filter(|&k| c.urban[k] == URBAN_PARK || !by_park(k))
        .collect();
    let town: Vec<usize> = cells.iter().copied().filter(|&k| placeable(c, k) && interior(k) && (lawn(c, k) || !by_park(k))).collect();
    // Last resort: any dry square inside the chunk off the shaping (never one in the water: lifted,
    // it stood up as a spike; never near the border: the knoll would stop at the seam).
    let last: Vec<usize> = cells.iter().copied().filter(|&k| dry(c, k) && interior(k) && (lawn(c, k) || !by_park(k))).collect();
    // Paved ground (streets, plazas) gets a round built feature (a dais, a fountain, a
    // planter, a market cross) clear of stalls and props, rather than a knoll.
    if good.is_empty() && !town.is_empty() {
        let free = |c: &Chunk, k: usize| placeable(c, k) && interior(k) && (matches!(c.urban[k], URBAN_STREET | URBAN_PLAZA) || (garden(k) && c.urban[k] == URBAN_PARK));
        let kind = |k: usize, r: f64, rng: &mut Pcg32| {
            // (A castle's yard: a raised planter, in a garden yard now and then a fountain.)
            if yard(k) != 0 {
                if yard(k) == 2 && r >= 1.5 && rng.next_f64() < 0.3 { PlazaFeature::Fountain } else { PlazaFeature::Planter }
            } else if garden(k) {
                if r < 1.5 || rng.next_f64() < 0.7 { PlazaFeature::Planter } else { PlazaFeature::Fountain }
            } else if r < 1.5 {
                if rng.next_f64() < 0.5 { PlazaFeature::Cross } else { PlazaFeature::Dais }
            } else {
                [PlazaFeature::Dais, PlazaFeature::Fountain, PlazaFeature::Planter, PlazaFeature::Cross][rng.below(4) as usize]
            }
        };
        if built_feature(c, &town, rng, sea, &free, &kind) {
            return true;
        }
    }
    let pool: &[usize] = if !good.is_empty() {
        &good
    } else if !town.is_empty() {
        &town
    } else if !last.is_empty() {
        &last
    } else {
        return false;
    };
    let mut spot = pool[rng.below(pool.len() as u32) as usize];
    // (On a lawn with no room for a whole knoll, the square with the most room round it.)
    if lawn(c, spot) && room[spot] < 7 {
        let most = pool.iter().filter(|&&k| lawn(c, k)).map(|&k| room[k]).max().unwrap_or(0);
        let best: Vec<usize> = pool.iter().copied().filter(|&k| lawn(c, k) && room[k] == most).collect();
        spot = best[rng.below(best.len() as u32) as usize];
        // (None but by the water's edge, a house or a road: no knoll here, the window round it tried
        // whole instead. In a park, a knoll cut off at its edge would rise in a step there: one needs
        // room for most of its slopes, else on its lawn a rocky outcrop with a boulder (a common's, where
        // one fits) or a raised planter with a tree.)
        let in_park = |k: usize| park.get(k).is_some_and(|&p| p & 3 != 0);
        if in_park(spot) && most < 6 {
            // (Its own park's lawn: a formal garden's beside a common's is another's.)
            let kind = park[spot] & 3;
            let lawns: Vec<usize> = pool.iter().copied().filter(|&k| park[k] & 3 == kind && c.urban[k] == URBAN_PARK).collect();
            let free = |c: &Chunk, k: usize| placeable(c, k) && interior(k) && c.urban[k] == URBAN_PARK && park[k] & 3 == kind && park[k] & 4 == 0;
            if lawns.is_empty() {
                return false;
            }
            return (!garden(spot) && outcrop(c, &lawns, rng, sea, &free)) || built_feature(c, &lawns, rng, sea, &free, &|_, _, _| PlazaFeature::Planter);
        }
        if most < 2 {
            return false;
        }
    }
    let (sx, sy) = ((spot % SQ) as f64 + 0.5, (spot / SQ) as f64 + 0.5);
    if lawn(c, spot) {
        // (Its reach, `r × elong × swell`, inside the room round it: 8.6 squares broad, 6 small; with
        // no room even for that, it is kept to the lawn's own squares.)
        let broad = room.get(spot).is_some_and(|&r| r >= 10);
        let f = Relief {
            cx: sx + gsx as f64,
            cy: sy + gsy as f64,
            r: if broad { rng.range(4.5, 6.0) } else { rng.range(3.5, 4.2) },
            h: TIER_FT * if broad { rng.range(1.25, 1.45) } else { rng.range(1.1, 1.2) },
            steep: rng.range(0.0, 0.1),
            dir: {
                let a = rng.range(0.0, std::f64::consts::TAU);
                (libm::cos(a), libm::sin(a))
            },
            elong: rng.range(1.0, 1.2),
            wobble: [(rng.range(0.0, 0.1), rng.range(0.0, 6.28)), (rng.range(0.0, 0.06), rng.range(0.0, 6.28)), (0.0, 0.0)],
        };
        // (A park's dry lawn rises whatever the water's level below it, as `lawn_room` has it.)
        let water: Vec<f32> = (0..SQ * SQ)
            .map(|k| if park.get(k).is_some_and(|&p| p & 3 != 0) && c.water_level[k] <= c.height[k] { c.water_level[k].min(c.height[k] - 1.5) } else { c.water_level[k] })
            .collect();
        if room.get(spot).is_some_and(|&r| r >= 7) {
            apply_relief(&f, &mut c.height, &water, gsx, gsy, SQ);
        } else {
            let before = c.height.clone();
            apply_relief(&f, &mut c.height, &water, gsx, gsy, SQ);
            for k in 0..SQ * SQ {
                if !matches!(c.urban[k], URBAN_PARK | URBAN_PATH) || c.building[k] != 0 || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 {
                    c.height[k] = before[k];
                }
            }
        }
        c.height[spot] = c.height[spot].max(((c.tier[spot] as f64 + 1.2) * TIER_FT + sea) as f32);
        retier(c, sea);
        return true;
    }
    // (A rounded knoll, a tier and a little more: never a flat-topped mesa. Its reach, `r × elong ×
    // swell`, under 7 squares: inside the margin it stands off the chunk's edge.)
    let rise = TIER_FT * rng.range(1.2, 1.5);
    let f = Relief {
        cx: sx + gsx as f64,
        cy: sy + gsy as f64,
        r: rng.range(3.0, 3.6),
        h: rise,
        steep: rng.range(0.1, 0.4),
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
    true
}

/// A round built feature raised a tier (`kind` by the square and its radius in squares) on one of
/// `cands`, the largest round that fits (2.3, 1.5, 1 or half a square) on `free` squares of the first
/// one's tier clear of what stands there; a planter with a tree in it. Whether one was.
fn built_feature(c: &mut Chunk, cands: &[usize], rng: &mut Pcg32, sea: f64, free: &dyn Fn(&Chunk, usize) -> bool, kind: &dyn Fn(usize, f64, &mut Pcg32) -> PlazaFeature) -> bool {
    let mut taken = vec![false; SQ * SQ];
    for o in &c.objects {
        let r = info(o.kind as u16).radius * o.scale + 0.5;
        for y in (o.y - r).max(0.0) as usize..=((o.y + r) as usize).min(SQ - 1) {
            for x in (o.x - r).max(0.0) as usize..=((o.x + r) as usize).min(SQ - 1) {
                taken[y * SQ + x] = true;
            }
        }
    }
    let base = c.tier[cands[0]];
    let fits = |k: usize| free(c, k) && !taken[k] && c.tier[k] == base;
    // Squares (offsets) within a round of radius r squares about a square's centre.
    let disc = |r: f64| -> Vec<(i64, i64)> {
        let n = r.floor() as i64;
        (-n..=n).flat_map(|dy| (-n..=n).map(move |dx| (dx, dy))).filter(|&(dx, dy)| ((dx * dx + dy * dy) as f64) <= r * r).collect()
    };
    let mut placed = None;
    'size: for r in [2.3, 1.5, 1.0, 0.5] {
        let offs = disc(r);
        let start = rng.below(cands.len() as u32) as usize;
        for q in 0..cands.len() {
            let k = cands[(start + q) % cands.len()];
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            let ok = offs.iter().all(|&(dx, dy)| {
                let (i, j) = (x + dx, y + dy);
                i >= 0 && j >= 0 && (i as usize) < SQ && (j as usize) < SQ && fits(j as usize * SQ + i as usize)
            });
            if ok {
                placed = Some((k, r, offs));
                break 'size;
            }
        }
    }
    let Some((k, r, offs)) = placed else { return false };
    let top = ((base as f64 + 1.2) * TIER_FT + sea) as f32;
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    let kind = kind(k, r, rng);
    for &(dx, dy) in &offs {
        c.height[(y + dy) as usize * SQ + (x + dx) as usize] = top;
    }
    let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
    let rv = r as f32 + 0.45;
    let ang = rng.range(0.0, std::f64::consts::TAU);
    c.structures.push(VectorShape { kind: ShapeKind::Dais, size: kind as u8 as f32, pts: vec![[cx, cy], [cx + rv * libm::cos(ang) as f32, cy + rv * libm::sin(ang) as f32]] });
    if kind == PlazaFeature::Planter {
        c.objects.push(Object { kind: Kind::TreeDeciduous, variant: rng.below(8) as u8, x: cx, y: cy, rot: 0.0, scale: 0.9, sprite: 0, turned: false });
    }
    retier(c, sea);
    true
}

/// A rocky outcrop on a common's lawn, on one of `cands` where its round (1.5, 1 or half a square,
/// eased out a square and a half more, a little out of round) lies on `free` squares a square or
/// more off the gravel: the ground rising in a dome to a tier over its foot, bare rock within half a square of its
/// round's edge, a boulder on its top and two or three rock piles about it; nothing else that blocks
/// the way within two squares of its round. Away from another (12 squares) where it can be. With no
/// room for one, a lone boulder on a knob of rock a square across. Whether one was.
fn outcrop(c: &mut Chunk, cands: &[usize], rng: &mut Pcg32, sea: f64, free: &dyn Fn(&Chunk, usize) -> bool) -> bool {
    let off_gravel = |c: &Chunk, k: usize| {
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        (-1..=1).all(|dy| (-1..=1).all(|dx| {
            let (i, j) = (x + dx, y + dy);
            !(i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64) || c.urban[j as usize * SQ + i as usize] != URBAN_PATH
        }))
    };
    let lone = |c: &Chunk, k: usize| {
        let (x, y) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
        !c.objects.iter().any(|o| o.kind == Kind::Boulder && o.scale > 1.0 && (o.x - x) * (o.x - x) + (o.y - y) * (o.y - y) < 144.0)
    };
    let base = c.tier[cands[0]];
    // (Its round on one tier; out to all the dome reaches and a square more (the most its outline
    // swells), the park's lawn: no street, square, quay, gravel, water or bank, and nothing standing
    // where its boulder goes.)
    let fits = |c: &Chunk, k: usize, r: f64, reach: f64| {
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        let out = reach * 1.25 + 1.0;
        let n = crate::core::ceil(out) as i64;
        let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
        let bare = c.objects.iter().all(|o| {
            let gap = (r + 0.6) as f32 + info(o.kind as u16).radius * o.scale;
            (o.x - cx) * (o.x - cx) + (o.y - cy) * (o.y - cy) >= gap * gap
        });
        bare && (-n..=n).all(|dy| (-n..=n).all(|dx| {
            let (i, j) = (x + dx, y + dy);
            let d2 = (dx * dx + dy * dy) as f64;
            d2 > out * out
                || (i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                    let q = j as usize * SQ + i as usize;
                    free(c, q) && off_gravel(c, q) && (d2 > r * r || c.tier[q] == base)
                })
        }))
    };
    // (Its boulder blocks the way: nothing else that does stands within two squares of its round, so
    // no square is shut in between them.)
    let clear = |c: &Chunk, k: usize, r: f64| {
        let (x, y) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
        c.objects.iter().all(|o| {
            let reach = (r + 2.0) as f32 + info(o.kind as u16).radius * o.scale;
            !info(o.kind as u16).blocks_move || (o.x - x) * (o.x - x) + (o.y - y) * (o.y - y) >= reach * reach
        })
    };
    let start = rng.below(cands.len() as u32) as usize;
    let mut placed = None;
    // (Room for its round and a broad ease round it first, apart from another; else a narrower ease.
    // Their sizes rolled, so a row of them along a strip of lawn isn't one size over and over.)
    let sizes = [[1.5, 1.0, 0.5], [1.0, 1.5, 0.5], [1.0, 0.5, 1.5]][rng.below(3) as usize];
    'look: for (spaced, ease) in [(true, 1.5), (false, 1.5), (false, 0.5)] {
        for r in sizes {
            for q in 0..cands.len() {
                let k = cands[(start + q) % cands.len()];
                if (!spaced || lone(c, k)) && fits(c, k, r, r + ease) && clear(c, k, r) {
                    placed = Some((k, r, ease));
                    break 'look;
                }
            }
        }
    }
    let Some((k, r, ease)) = placed else {
        // (No room for one: a lone boulder on a knob of rock a square across, the knob hidden under it,
        // where the lawn runs three squares round it (and no gravel a square past that) and nothing
        // stands within a square and a half.)
        let knob = (0..cands.len()).map(|q| cands[(start + q) % cands.len()]).find(|&k| {
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            (-3..=3).all(|dy| (-3..=3).all(|dx| {
                let (i, j) = (x + dx, y + dy);
                i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                    let q = j as usize * SQ + i as usize;
                    free(c, q) && off_gravel(c, q)
                }
            })) && clear(c, k, 0.0)
                && c.objects.iter().all(|o| {
                    let gap = 1.5 + info(o.kind as u16).radius * o.scale;
                    (o.x - cx) * (o.x - cx) + (o.y - cy) * (o.y - cy) >= gap * gap
                })
        });
        let Some(k) = knob else { return false };
        c.height[k] = c.height[k].max(((c.tier[k] as f64 + 1.2) * TIER_FT + sea) as f32);
        c.surface[k] = Surface::Rock;
        let (cx, cy) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
        c.objects.push(Object { kind: Kind::Boulder, variant: rng.below(8) as u8, x: cx, y: cy, rot: rng.range(0.0, std::f64::consts::TAU) as f32, scale: rng.range(1.35, 1.5) as f32, sprite: 0, turned: false });
        retier(c, sea);
        return true;
    };
    let (x, y) = ((k % SQ) as f64 + 0.5, (k / SQ) as f64 + 0.5);
    let top = ((base as f64 + 1.2) * TIER_FT + sea) as f32;
    let (a, b) = (rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU));
    let reach = r + ease;
    let n = crate::core::ceil(reach * 1.25) as i64;
    for dy in -n..=n {
        for dx in -n..=n {
            let (i, j) = (k as i64 % SQ as i64 + dx, k as i64 / SQ as i64 + dy);
            if i < 0 || j < 0 || i >= SQ as i64 || j >= SQ as i64 {
                continue;
            }
            let q = j as usize * SQ + i as usize;
            let (px, py) = (i as f64 + 0.5 - x, j as f64 + 0.5 - y);
            let d = crate::core::sqrt(px * px + py * py);
            let th = libm::atan2(py, px);
            let wob = 1.0 + 0.15 * libm::sin(2.0 * th + a) + 0.1 * libm::sin(3.0 * th + b);
            let t = d / (reach * wob);
            if t >= 1.0 || !free(c, q) {
                continue;
            }
            let ease = (t * t * (3.0 - 2.0 * t)) as f32;
            c.height[q] = c.height[q].max(top - (top - c.height[q]) * ease);
            if d <= (r - 0.5) * wob {
                c.surface[q] = Surface::Rock;
            }
        }
    }
    let (cx, cy) = ((x + rng.range(-0.2, 0.2)) as f32, (y + rng.range(-0.2, 0.2)) as f32);
    c.objects.push(Object { kind: Kind::Boulder, variant: rng.below(8) as u8, x: cx, y: cy, rot: rng.range(0.0, std::f64::consts::TAU) as f32, scale: ((r + 0.6) / 0.9 * rng.range(0.9, 1.05)) as f32, sprite: 0, turned: false });
    for m in 0..2 + rng.below(2) {
        let t = a + std::f64::consts::TAU * (m as f64 + rng.range(0.0, 0.4)) / 3.0;
        let d = r + rng.range(0.1, 0.6);
        let kind = if rng.next_f64() < 0.6 { Kind::RockPile } else { Kind::RockSmall };
        let (ox, oy) = (x + d * libm::cos(t), y + d * libm::sin(t));
        if ox > 0.5 && oy > 0.5 && ox < SQ as f64 - 0.5 && oy < SQ as f64 - 0.5 {
            c.objects.push(Object { kind, variant: rng.below(4) as u8, x: ox as f32, y: oy as f32, rot: rng.range(0.0, std::f64::consts::TAU) as f32, scale: rng.range(0.6, 0.9) as f32, sprite: 0, turned: false });
        }
    }
    retier(c, sea);
    true
}

/// How far (squares, by the most of either way, to 12) each square is from one a lawn's knoll
/// mustn't touch: off the lawn and its gravel, a building, a road, ground within a foot of the water
/// below it (in a park, the water), a park's pond's or stream's banks (`park` bit 4, as `enforce`
/// has it), and the chunk's edge.
fn lawn_room(c: &Chunk, park: &[u8]) -> Vec<u8> {
    let mut d = vec![u8::MAX; SQ * SQ];
    let mut queue = std::collections::VecDeque::new();
    for k in 0..SQ * SQ {
        let (x, y) = (k % SQ, k / SQ);
        let edge = x == 0 || y == 0 || x == SQ - 1 || y == SQ - 1;
        // (In a park only its water counts: the ground of a river's flat may stand a foot over the
        // water's level all across it.)
        let banks = park.get(k).is_some_and(|&p| p & 4 != 0);
        let shore = c.water_level[k] > c.height[k] - if park.get(k).is_some_and(|&p| p & 3 != 0) { 0.0 } else { 1.0 };
        if edge || banks || shore || !matches!(c.urban[k], URBAN_PARK | URBAN_PATH) || c.building[k] != 0 || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 {
            d[k] = if edge && matches!(c.urban[k], URBAN_PARK | URBAN_PATH) { 1 } else { 0 };
            queue.push_back(k);
        }
    }
    while let Some(k) = queue.pop_front() {
        if d[k] >= 12 {
            continue;
        }
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                    continue;
                }
                let n = ny as usize * SQ + nx as usize;
                if d[n] > d[k] + 1 {
                    d[n] = d[k] + 1;
                    queue.push_back(n);
                }
            }
        }
    }
    d
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

/// How a market fills its square: half-extents (ft, ranges) of the part round the well it keeps
/// to, the share of its cells left open, the spacing of stalls along a block's edge (ft) and the
/// share of stalls left out.
#[derive(Clone, Copy)]
struct Market {
    hu: (f64, f64),
    hv: (f64, f64),
    open: f64,
    spacing: f64,
    drop: f64,
}

impl Market {
    fn of(kind: crate::town::PlazaKind) -> Option<Market> {
        use crate::town::PlazaKind as K;
        match kind {
            K::Market => Some(Market { hu: (110.0, 150.0), hv: (80.0, 110.0), open: 0.12, spacing: 9.2, drop: 0.04 }),
            // Few stalls: wide lanes, many open cells, gaps along the rows.
            K::MarketSparse => Some(Market { hu: (100.0, 140.0), hv: (70.0, 100.0), open: 0.4, spacing: 11.5, drop: 0.2 }),
            // A market day's huddle round the well.
            K::MarketDay => Some(Market { hu: (55.0, 80.0), hv: (40.0, 60.0), open: 0.12, spacing: 9.2, drop: 0.06 }),
            _ => None,
        }
    }
}

/// Cargo along a quay: stacks of crates and barrels, now and then a cart, every 22 to 45 ft down
/// its middle. A pure function of the quay (world ft).
fn quay_cargo(poly: &[[f64; 2]], seed: u64, out: &mut Vec<([f64; 2], f32, Kind)>) {
    use crate::town::geom::{add, centroid, contains, dot, main_axis, mul, sub};
    let c = centroid(poly);
    let mut rng = Pcg32::new(hash2(seed ^ 0x9a1e, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 17);
    let eu = main_axis(poly);
    let ev = [-eu[1], eu[0]];
    let (mut u0, mut u1) = (f64::MAX, f64::MIN);
    for q in poly {
        let u = dot(sub(*q, c), eu);
        (u0, u1) = (u0.min(u), u1.max(u));
    }
    let mut u = u0 + rng.range(6.0, 20.0);
    while u < u1 - 6.0 {
        let base = add(c, mul(eu, u));
        if rng.next_f64() < 0.12 {
            // Sprites face south (+y) unturned: a cart lies along the quay.
            let rot = (libm::atan2(eu[1], eu[0]) - std::f64::consts::FRAC_PI_2) as f32;
            if contains(poly, base) {
                out.push((base, rot, Kind::Cart));
            }
        } else {
            for _ in 0..1 + rng.below(3) {
                let p = add(base, add(mul(eu, rng.range(-4.0, 4.0)), mul(ev, rng.range(-2.5, 2.5))));
                let kind = if rng.next_f64() < 0.55 { Kind::Crate } else { Kind::Barrel };
                let rot = rng.range(0.0, std::f64::consts::TAU) as f32;
                if contains(poly, p) {
                    out.push((p, rot, kind));
                }
            }
        }
        u += rng.range(22.0, 45.0);
    }
}

/// A polygon's bounds (world ft): x0, y0, x1, y1.
fn bounds(poly: &[[f64; 2]]) -> (f64, f64, f64, f64) {
    poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])))
}

/// A prop's footprint radius (ft).
fn foot(kind: Kind) -> f64 {
    info(kind as u16).radius as f64 * SQUARE_FT
}

/// Room for props on an open space (world ft): spots on the battlemap's squares inside it, each
/// kept clear of the lanes across it (streets and roads: centre line, half width), the buildings
/// round it (and on it: a temple on its close), its monuments and what is set out on it already
/// (centre, radius).
struct Room<'a> {
    poly: &'a [[f64; 2]],
    c: [f64; 2],
    lanes: Vec<([f64; 2], [f64; 2], f64)>,
    blds: Vec<&'a [[f64; 2]]>,
    taken: Vec<([f64; 2], f64)>,
    /// The spots, each with how far it is inside the outline, and (worked out when first asked:
    /// NaN till then) how far it is clear of the lanes (past their half width) and of buildings
    /// (ft; below 0 inside one).
    grid: Vec<([f64; 2], f64, std::cell::Cell<f64>, std::cell::Cell<f64>)>,
    seed: u64,
}

impl<'a> Room<'a> {
    fn new(poly: &'a [[f64; 2]], l: &'a crate::town::Layout, lanes: &[([f64; 2], [f64; 2], f64)], taken: &[([f64; 2], f64)], seed: u64) -> Room<'a> {
        use crate::town::geom::{centroid, contains, dist};
        let (x0, y0, x1, y1) = bounds(poly);
        let near = |b: (f64, f64, f64, f64), m: f64| b.0 <= x1 + m && b.2 >= x0 - m && b.1 <= y1 + m && b.3 >= y0 - m;
        let lanes = lanes.iter().copied().filter(|&(a, b, w)| near((a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])), w + 15.0)).collect();
        let blds = l.buildings.iter().map(|b| b.poly.as_slice()).filter(|b| near(bounds(b), 15.0)).collect();
        let mut taken = taken.to_vec();
        for m in l.monuments.iter().filter(|m| near(bounds(m), 0.0)) {
            let mc = centroid(m);
            taken.push((mc, m.iter().map(|q| dist(*q, mc)).fold(0.0, f64::max) + 3.0));
        }
        let mut room = Room { poly, c: centroid(poly), lanes, blds, taken, grid: Vec::new(), seed };
        // Square centres (by their global index on the world's 5-ft grid, as every chunk lays its
        // squares).
        let cell = |v: f64| crate::core::floor(v / SQUARE_FT) as i64;
        for j in cell(y0)..=cell(y1) {
            for i in cell(x0)..=cell(x1) {
                let p = [(i as f64 + 0.5) * SQUARE_FT, (j as f64 + 0.5) * SQUARE_FT];
                if contains(poly, p) {
                    room.grid.push((p, room.depth(p), std::cell::Cell::new(f64::NAN), std::cell::Cell::new(f64::NAN)));
                }
            }
        }
        room
    }

    /// How far `p` is from the lanes, past their half width (ft).
    fn lane_gap(&self, p: [f64; 2]) -> f64 {
        self.lanes.iter().map(|&(a, b, w)| crate::town::geom::seg_dist(p, a, b) - w).fold(f64::MAX, f64::min)
    }

    /// How far `p` is from the buildings (ft; -1 inside one).
    fn bld_gap(&self, p: [f64; 2]) -> f64 {
        use crate::town::geom::{contains, seg_dist};
        self.blds.iter().map(|b| if contains(b, p) { -1.0 } else { (0..b.len()).map(|e| seg_dist(p, b[e], b[(e + 1) % b.len()])).fold(f64::MAX, f64::min) }).fold(f64::MAX, f64::min)
    }

    /// How far `p` lies inside the outline (ft; 0 outside it).
    fn depth(&self, p: [f64; 2]) -> f64 {
        use crate::town::geom::{contains, seg_dist};
        if !contains(self.poly, p) {
            return 0.0;
        }
        let m = self.poly.len();
        (0..m).map(|e| seg_dist(p, self.poly[e], self.poly[(e + 1) % m])).fold(f64::MAX, f64::min)
    }

    /// Whether a prop `r` ft round fits at `p` (off the lanes too, with `lanes`).
    fn free(&self, p: [f64; 2], r: f64, lanes: bool) -> bool {
        let nan = || std::cell::Cell::new(f64::NAN);
        self.fits(&(p, self.depth(p), nan(), nan()), r, lanes)
    }

    /// `free` for a spot of the grid (the cheap tests first; the gaps worked out once).
    fn fits(&self, (p, depth, lane, bld): &([f64; 2], f64, std::cell::Cell<f64>, std::cell::Cell<f64>), r: f64, lanes: bool) -> bool {
        let gap = |c: &std::cell::Cell<f64>, f: &dyn Fn([f64; 2]) -> f64| {
            if c.get().is_nan() {
                c.set(f(*p));
            }
            c.get()
        };
        *depth >= r + 1.0
            && self.taken.iter().all(|&(q, s)| (p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1]) >= (r + s + 2.0) * (r + s + 2.0))
            && (!lanes || gap(lane, &|p| self.lane_gap(p)) >= r + 1.0)
            && gap(bld, &|p| self.bld_gap(p)) >= r + 3.0
    }

    /// A little noise by spot (0–1), pure in the spot.
    fn jit(&self, p: [f64; 2], salt: i64) -> f64 {
        unit(hash3(self.seed, crate::core::round(p[0] * 2.0) as i64, crate::core::round(p[1] * 2.0) as i64, salt))
    }

    /// The free spot for a prop `r` ft round that scores best, taken; none if nothing fits.
    fn best(&mut self, r: f64, lanes: bool, score: impl Fn(&Room, [f64; 2]) -> f64) -> Option<[f64; 2]> {
        let mut best: Option<(f64, [f64; 2])> = None;
        for spot in &self.grid {
            if self.fits(spot, r, lanes) {
                let (p, s) = (spot.0, score(self, spot.0));
                if best.is_none_or(|b| s > b.0) {
                    best = Some((s, p));
                }
            }
        }
        let p = best?.1;
        self.taken.push((p, r));
        Some(p)
    }

    /// Lamp posts of one look round the square, spread apart: each at about `at` ft from the middle
    /// (or `at` ft in from the edge, with `edge`), as far as it can be from the others.
    fn lamps(&mut self, n: i64, at: f64, edge: bool, look: u8, out: &mut Vec<([f64; 2], f32, Kind, u8)>) {
        use crate::town::geom::dist;
        let mut lamps: Vec<[f64; 2]> = Vec::new();
        for k in 0..n {
            let spread = |r: &Room, p: [f64; 2]| {
                let off = if edge { r.depth(p) - at } else { dist(p, r.c) - at };
                -0.4 * off.abs() + 0.15 * lamps.iter().map(|q| dist(*q, p)).fold(150.0, f64::min) + r.jit(p, 70 + k)
            };
            if let Some(p) = self.best(foot(Kind::LampPost) + 2.0, true, spread) {
                lamps.push(p);
                out.push((p, 0.0, Kind::LampPost, look));
            }
        }
    }

    /// The direction (rad) of the outline's edge nearest `p`.
    fn along(&self, p: [f64; 2]) -> f32 {
        use crate::town::geom::seg_dist;
        let m = self.poly.len();
        let e = (0..m).min_by(|&a, &b| seg_dist(p, self.poly[a], self.poly[(a + 1) % m]).total_cmp(&seg_dist(p, self.poly[b], self.poly[(b + 1) % m]))).unwrap_or(0);
        let (a, b) = (self.poly[e], self.poly[(e + 1) % m]);
        libm::atan2(b[1] - a[1], b[0] - a[0]) as f32
    }
}

/// A lamp post's look for a square's set (one look for all): lit iron or two lanterns on a rich
/// square, else any but the unlit one, now and then that.
fn lamp_look(rng: &mut Pcg32, rich: bool) -> u8 {
    if rich {
        [0, 2][rng.below(2) as usize]
    } else if rng.next_f64() < 0.15 {
        3
    } else {
        rng.below(3) as u8
    }
}

/// A market cross by the well of a main square, 22 to 40 ft out along its long axis: on some
/// markets (a buttercross, mostly) and every civic square, its one landmark (taken in `room`).
/// Pure in the plaza.
fn main_cross(pl: &crate::town::Plaza, room: &mut Room, seed: u64) -> Option<([f64; 2], f32, Kind, u8)> {
    use crate::town::PlazaKind as K;
    use crate::town::geom::{add, main_axis, mul};
    let c = room.c;
    let mut rng = Pcg32::new(hash2(seed ^ 0xc205, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 23);
    let chance = match pl.kind {
        K::Market | K::MarketSparse => 0.45,
        K::MarketDay => 0.3,
        _ => 1.0,
    };
    if rng.next_f64() >= chance {
        return None;
    }
    let look = if pl.kind.is_market() { [1, 1, 0, 3][rng.below(4) as usize] } else { [0, 3, 2][rng.below(3) as usize] };
    let u = main_axis(&pl.pts);
    let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
    // (A civic square's, its landmark, also further out or across the axis where a street
    // crosses the middle.)
    let (ds, axes): (&[f64], &[[f64; 2]]) = if pl.kind == K::Civic { (&[28.0, 34.0, 22.0, 40.0, 50.0, 60.0], &[u, [-u[1], u[0]]]) } else { (&[28.0, 34.0, 22.0, 40.0], &[u]) };
    for &a in axes {
        for &d in ds {
            for s in [side, -side] {
                let p = add(c, mul(a, d * s));
                if room.free(p, cross_ft() + 3.0, true) {
                    room.taken.push((p, cross_ft()));
                    return Some((p, 0.0, Kind::MarketCross, look));
                }
            }
        }
    }
    None
}

/// A main square's edge, once its market and features are laid out (taken in `room`): a notice
/// board (a civic square two), a trough or two along the edge, a city's lamp posts spread round it
/// (one look for all), and a civic square's stocks or pillory (a market day's now and then). Pure
/// in the plaza.
fn dress_main(pl: &crate::town::Plaza, l: &crate::town::Layout, room: &mut Room, seed: u64, out: &mut Vec<([f64; 2], f32, Kind, u8)>) {
    use crate::town::PlazaKind as K;
    let c = room.c;
    let mut rng = Pcg32::new(hash2(seed ^ 0xd3a1, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 29);
    let civic = pl.kind == K::Civic;
    let city = l.tier >= crate::t0::settle::Tier::City;
    // Just inside the edge (`want` ft in), here or there.
    let edge = |want: f64, salt: i64| move |r: &Room, p: [f64; 2]| -(r.depth(p) - want).abs() + 3.0 * r.jit(p, salt);
    for k in 0..1 + civic as i64 {
        if let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(6.0, 10 + k)) {
            out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
        }
    }
    let trough = if city { [1, 3][rng.below(2) as usize] } else { [0, 2, 1][rng.below(3) as usize] };
    for k in 0..1 + (rng.next_f64() < 0.5) as i64 {
        if let Some(p) = room.best(foot(Kind::WaterTrough), true, edge(7.0, 20 + k)) {
            out.push((p, room.along(p), Kind::WaterTrough, trough));
        }
    }
    if city {
        let look = lamp_look(&mut rng, civic || l.tier == crate::t0::settle::Tier::Metropolis);
        room.lamps(4 + rng.below(3) as i64, 5.0, true, look, out);
    }
    if (civic && rng.next_f64() < 0.6) || (pl.kind == K::MarketDay && rng.next_f64() < 0.25) {
        let look = if civic { [0, 2, 3][rng.below(3) as usize] } else { [0, 3][rng.below(2) as usize] };
        if let Some(p) = room.best(foot(Kind::Stocks), true, edge(10.0, 40)) {
            out.push((p, 0.0, Kind::Stocks, look));
        }
    }
}

/// The biome of the land at `p` (world ft): where the map there is the sea's or a lake's (a
/// town on the shore), the nearest land's round it, out to about a T0 cell, else grassland.
pub(crate) fn land_biome(t0: &T0, p: [f64; 2]) -> Biome {
    let at = |q: [f64; 2]| Biome::from_u8(t0.sample_biome(q[0], q[1], t0.biome_warp(q[0], q[1]))[0]);
    let wet = |b: Biome| matches!(b, Biome::Ocean | Biome::Lake);
    std::iter::once(p)
        .chain([300.0, 1000.0, 2500.0, 4500.0, 7000.0].into_iter().flat_map(|r| {
            (0..8).map(move |k| {
                let t = std::f64::consts::TAU * k as f64 / 8.0;
                [p[0] + r * libm::cos(t), p[1] + r * libm::sin(t)]
            })
        }))
        .map(at)
        .find(|&b| !wet(b))
        .unwrap_or(Biome::Grassland)
}

/// How much larger than its place a square's market cross is drawn (its steps and cross read from
/// afar; the squares keep room for it).
const MARKET_CROSS_SCALE: f32 = 1.35;

/// A square's market cross's footprint radius (ft), as drawn.
fn cross_ft() -> f64 {
    MARKET_CROSS_SCALE as f64 * foot(Kind::MarketCross)
}

/// What grows on a town's green or close in a biome: its trees, shrubs and small plants (palms
/// in the desert, acacias on the savanna, conifers in the north, none of a kind where the biome
/// has none), and whether its trees are few (one or two broadleaves on the steppe's grass).
pub(crate) fn greenery(b: Biome) -> (&'static [Kind], &'static [Kind], &'static [Kind], bool) {
    use Kind::*;
    match b {
        Biome::TemperateForest => (&[TreeDeciduous], &[Bush], &[Flowers, Mushrooms], false),
        Biome::TemperateRainforest => (&[TreeDeciduous, TreeConifer], &[Bush, Thicket], &[Mushrooms], false),
        Biome::Grassland => (&[TreeDeciduous], &[Bush], &[Flowers, TallGrass], false),
        Biome::Steppe => (&[TreeDeciduous], &[Bush], &[TallGrass], true),
        Biome::Savanna => (&[TreeAcacia], &[Bush], &[TallGrass], false),
        Biome::Jungle => (&[TreePalm, TreeJungle], &[Bush, Thicket], &[Flowers], false),
        Biome::HotDesert | Biome::SaltFlat => (&[TreePalm], &[Cactus], &[], false),
        Biome::Taiga | Biome::Alpine | Biome::Tundra | Biome::ColdDesert | Biome::Ice => (&[TreeConifer], &[Bush], &[], false),
        Biome::Swamp => (&[TreeWillow], &[Bush], &[Reeds], false),
        Biome::Blight | Biome::Ashland | Biome::Volcanic => (&[TreeDead], &[], &[], false),
        Biome::Ocean | Biome::Lake => (&[TreeDeciduous], &[Bush], &[], false),
    }
}

/// A castle's bailey: whether it is one of the castles with a garden yard (rolled per castle), and
/// how it is dressed (world ft: place, turn, kind, look, scale), a pure function of its outer
/// bailey (`pl`) and its castle in `l`, so every chunk sets out the same. Nothing stands in the
/// way from a gate to the next gate or to the keep, on a motte, or close by a wall, a tower or
/// another building:
/// - by most ranges, a store's group against its yard side (barrels, crates, a cart, firewood, a
///   trough);
/// - inside each gate, to one side of the way, a cart, a trough or barrels;
/// (The ways from gate to gate and to the keep go in `ways`: the guarantee keeps off them too.)
/// - a well in the keep's court;
/// - in a garden yard, a plot by the keep (flowerbeds square to it, topiary at its corners, a
///   bench on each long side facing in, now and then a fountain at its middle, a short hedge
///   behind it, each piece wholly inside one chunk) and a few of the biome's trees planted in the
///   yard, spaced out.
fn dress_bailey(pl: &crate::town::Plaza, l: &crate::town::Layout, t0: &T0, seed: u64, out: &mut Vec<([f64; 2], f32, Kind, u8, f32)>, ways: &mut Vec<([f64; 2], [f64; 2])>) -> bool {
    use crate::town::geom::{add, centroid, contains, cross, dist, dot, len, lerp, mul, seg_dist, sub};
    let ring = &pl.pts;
    let rn = ring.len();
    if rn < 3 {
        return false;
    }
    let c0 = centroid(ring);
    let key = hash3(seed ^ 0xba11, crate::core::round(c0[0]) as i64, crate::core::round(c0[1]) as i64, 7);
    let roll = |k: i64| unit(hash2(key, k, 0x5d));
    let garden = roll(0) < 0.45;
    let depth = |p: [f64; 2]| {
        let d = (0..rn).map(|k| seg_dist(p, ring[k], ring[(k + 1) % rn])).fold(f64::MAX, f64::min);
        if contains(ring, p) { d } else { -d }
    };
    let poly_d = |poly: &[[f64; 2]], p: [f64; 2]| {
        if contains(poly, p) {
            return 0.0;
        }
        let m = poly.len();
        (0..m).map(|k| seg_dist(p, poly[k], poly[(k + 1) % m])).fold(f64::MAX, f64::min)
    };
    // The castle's buildings, walls and towers round the bailey.
    let reach = ring.iter().map(|p| dist(*p, c0)).fold(0.0, f64::max) + 60.0;
    let near = |p: &[f64; 2]| dist(*p, c0) < reach;
    let blds: Vec<&crate::town::Building> = l.buildings.iter().filter(|b| b.poly.iter().any(near)).collect();
    let walls: Vec<([f64; 2], [f64; 2])> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).filter(|s| near(&s.0) || near(&s.1)).collect();
    let towers: Vec<([f64; 2], f64)> = l
        .towers
        .iter()
        .map(|t| (*t, if l.square_tower(*t).is_some() { 15.6 } else { 11.0 }))
        .chain(l.gate_towers.iter().map(|t| (*t, 15.0)))
        .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { 11.0 })))
        .filter(|t| near(&t.0))
        .collect();
    let keep = blds.iter().find(|b| !b.parts.is_empty() && contains(ring, centroid(&b.poly)));
    let ranges: Vec<&&crate::town::Building> =
        blds.iter().filter(|b| b.parts.is_empty() && b.ward == crate::town::catalog::Ward::Castle && contains(ring, centroid(&b.poly))).collect();
    let mottes: Vec<&crate::town::Motte> = l.mottes.iter().filter(|m| contains(ring, m.at)).collect();
    // The gates: pairs of gate towers a passage apart (the outer one on the ring, an inner one
    // inside it), their middles and the way in (toward the keep). (A castle set in the town by hand
    // has its towers among `extra_towers`.)
    let gt: Vec<[f64; 2]> = l.gate_towers.iter().copied().chain(l.extra_towers.iter().filter(|t| t.1).map(|t| t.0)).filter(|t| depth(*t) > -20.0).collect();
    let to = keep.map_or(c0, |k| centroid(&k.poly));
    let mut gates: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for a in 0..gt.len() {
        for b in a + 1..gt.len() {
            if (dist(gt[a], gt[b]) - 52.0).abs() < 0.5 {
                let m = lerp(gt[a], gt[b], 0.5);
                let u = mul(sub(gt[b], gt[a]), 1.0 / 52.0);
                let n = [-u[1], u[0]];
                gates.push((m, if dot(sub(to, m), n) >= 0.0 { n } else { mul(n, -1.0) }));
            }
        }
    }
    gates.sort_by(|a, b| depth(a.0).total_cmp(&depth(b.0)).then(a.0[0].total_cmp(&b.0[0])).then(a.0[1].total_cmp(&b.0[1])));
    // The ways: gate to gate, the last to the keep.
    let mut lanes: Vec<([f64; 2], [f64; 2])> = gates.windows(2).map(|g| (g[0].0, g[1].0)).collect();
    if let (Some(g), Some(k)) = (gates.last(), keep) {
        lanes.push((g.0, centroid(&k.poly)));
    }
    ways.extend(lanes.iter().copied());
    let inner_walls: Vec<([f64; 2], [f64; 2])> = walls.iter().copied().filter(|s| depth(s.0) > 20.0 && depth(s.1) > 20.0).collect();
    // The keep's court: inside the inner curtain if there is one (the way to its middle crosses
    // none of it).
    let ci = l.castles.get(1).map_or(c0, |c| c.0);
    let in_court = |p: [f64; 2]| {
        inner_walls.is_empty()
            || !inner_walls.iter().any(|&(a, b)| {
                let (o1, o2) = (cross(sub(ci, p), sub(a, p)), cross(sub(ci, p), sub(b, p)));
                let (o3, o4) = (cross(sub(b, a), sub(p, a)), cross(sub(b, a), sub(ci, a)));
                o1 * o2 < 0.0 && o3 * o4 < 0.0
            })
    };
    let mut placed: Vec<([f64; 2], f64)> = Vec::new();
    // Room for something `r` ft round at `p`: in the bailey, clear of buildings (but `own`, by a
    // little), walls, towers, the ways and gates, mottes and what is set out already.
    let free = |placed: &[([f64; 2], f64)], p: [f64; 2], r: f64, own: Option<usize>| {
        depth(p) >= r + 8.0
            && blds.iter().enumerate().all(|(i, b)| poly_d(&b.poly, p) >= r + if own == Some(i) { 1.5 } else { 6.0 })
            && walls.iter().all(|&(a, b)| seg_dist(p, a, b) >= r + 10.0)
            && towers.iter().all(|&(t, tr)| dist(p, t) >= tr + r + 8.0)
            && lanes.iter().all(|&(a, b)| seg_dist(p, a, b) >= r + 14.0)
            && gates.iter().all(|&(g, _)| dist(p, g) >= r + 20.0)
            && mottes.iter().all(|m| dist(p, m.at) >= m.foot_r + r + 6.0)
            && placed.iter().all(|&(q, qr)| dist(p, q) >= r + qr + 2.0)
    };
    let along = |d: [f64; 2]| libm::atan2(d[1], d[0]) as f32;
    let look = |a: i64, b: i64| (hash2(key, a, b) >> 20) as u8 & 7;
    // By the ranges, a store's group against the yard side.
    for (ri, b) in ranges.iter().enumerate() {
        if roll(10 + ri as i64) >= 0.7 || b.poly.len() != 4 {
            continue;
        }
        let bi = blds.iter().position(|o| std::ptr::eq(*o, **b));
        // (Its long side deepest in the bailey.)
        let side = (0..4).filter(|&e| dist(b.poly[e], b.poly[(e + 1) % 4]) >= dist(b.poly[(e + 1) % 4], b.poly[(e + 2) % 4]) - 0.5).max_by(|&x, &y| {
            depth(lerp(b.poly[x], b.poly[(x + 1) % 4], 0.5)).total_cmp(&depth(lerp(b.poly[y], b.poly[(y + 1) % 4], 0.5))).then(y.cmp(&x))
        });
        let Some(e) = side else { continue };
        let (a, q) = (b.poly[e], b.poly[(e + 1) % 4]);
        let el = dist(a, q);
        let u = mul(sub(q, a), 1.0 / el.max(1e-9));
        let n0 = [-u[1], u[0]];
        let n = if dot(sub(lerp(a, q, 0.5), centroid(&b.poly)), n0) >= 0.0 { n0 } else { mul(n0, -1.0) };
        let group: &[Kind] = match (roll(30 + ri as i64) * 5.0) as usize {
            0 => &[Kind::Barrel, Kind::Crate],
            1 => &[Kind::Cart],
            2 => &[Kind::Firewood, Kind::Firewood],
            3 => &[Kind::Crate, Kind::Barrel, Kind::Crate],
            _ => &[Kind::WaterTrough],
        };
        let t = 0.2 + 0.6 * roll(50 + ri as i64);
        for (gi, &kind) in group.iter().enumerate() {
            let rad = info(kind as u16).radius as f64 * SQUARE_FT;
            let p = add(add(a, mul(u, (el * t + gi as f64 * 6.0).min(el - rad))), mul(n, rad + 2.0));
            if free(&placed, p, rad, bi) {
                placed.push((p, rad));
                out.push((p, along(u), kind, look(70 + ri as i64, gi as i64), 1.0));
            }
        }
    }
    // Inside each gate, to one side of the way in.
    for (gi, &(g, n)) in gates.iter().enumerate() {
        let s = if roll(90 + gi as i64) < 0.5 { [-n[1], n[0]] } else { [n[1], -n[0]] };
        let kind = [Kind::Cart, Kind::WaterTrough, Kind::Barrel][(roll(95 + gi as i64) * 3.0) as usize % 3];
        let rad = info(kind as u16).radius as f64 * SQUARE_FT;
        for (fwd, aside) in [(18.0, 26.0), (26.0, 30.0), (14.0, 34.0)] {
            let p = add(add(g, mul(n, fwd)), mul(s, aside));
            if free(&placed, p, rad, None) {
                placed.push((p, rad));
                out.push((p, along(n), kind, look(99, gi as i64), 1.0));
                break;
            }
        }
    }
    // Spots on a 15-ft grid in the bailey, whole multiples of world ft (the same in every chunk).
    let (x0, y0, x1, y1) = ring.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    const GRID: f64 = 15.0;
    let mut spots: Vec<[f64; 2]> = Vec::new();
    let mut y = crate::core::ceil(y0 / GRID) * GRID;
    while y <= y1 {
        let mut x = crate::core::ceil(x0 / GRID) * GRID;
        while x <= x1 {
            if depth([x, y]) > 20.0 {
                spots.push([x, y]);
            }
            x += GRID;
        }
        y += GRID;
    }
    let to_keep = |p: [f64; 2]| keep.map_or(dist(p, c0), |k| poly_d(&k.poly, p));
    let room = |p: [f64; 2]| {
        let b = blds.iter().map(|b| poly_d(&b.poly, p)).fold(f64::MAX, f64::min);
        let w = walls.iter().map(|&(a, q)| seg_dist(p, a, q)).fold(f64::MAX, f64::min);
        b.min(w).min(80.0)
    };
    // A well in the keep's court: by the keep, with room round it.
    let well_r = info(Kind::Well as u16).radius as f64 * SQUARE_FT + 6.0;
    let well = spots
        .iter()
        .copied()
        .filter(|&p| in_court(p) && room(p) >= 25.0 && free(&placed, p, well_r, None))
        .min_by(|&a, &b| (to_keep(a) - room(a)).total_cmp(&(to_keep(b) - room(b))).then(a[1].total_cmp(&b[1])).then(a[0].total_cmp(&b[0])));
    if let Some(p) = well {
        placed.push((p, well_r));
        out.push((p, 0.0, Kind::Well, (key >> 24) as u8 & 7, 1.0));
    }
    if !garden {
        return false;
    }
    // The garden plot by the keep, square to the nearest stretch of curtain.
    let (pw, pd) = (35.0 + 15.0 * roll(200), 25.0 + 10.0 * roll(201));
    let plot = spots
        .iter()
        .copied()
        .filter_map(|c| {
            let k = (0..rn).min_by(|&x, &y| seg_dist(c, ring[x], ring[(x + 1) % rn]).total_cmp(&seg_dist(c, ring[y], ring[(y + 1) % rn])).then(x.cmp(&y)))?;
            let e = sub(ring[(k + 1) % rn], ring[k]);
            let u = mul(e, 1.0 / len(e).max(1e-9));
            let v = [-u[1], u[0]];
            let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(a, b)| add(c, add(mul(u, a * 0.5 * pw), mul(v, b * 0.5 * pd))));
            let ok = in_court(c)
                && free(&placed, c, 0.5 * pd + 4.0, None)
                && (0..4).all(|k| (0..=4).all(|j| free(&placed, lerp(corners[k], corners[(k + 1) % 4], j as f64 / 4.0), 12.0, None)));
            ok.then_some((c, u, v))
        })
        .min_by(|a, b| to_keep(a.0).total_cmp(&to_keep(b.0)).then(a.0[1].total_cmp(&b.0[1])).then(a.0[0].total_cmp(&b.0[0])));
    if let Some((c, u, v)) = plot {
        let rot = along(u);
        // Flowerbeds in a grid, its middle left for a fountain now and then.
        let fountain = roll(210) < 0.35 && pw >= 40.0;
        let cols = if pw >= 45.0 { 3 } else { 2 };
        for i in 0..cols {
            for j in 0..2 {
                if fountain && cols == 3 && i == 1 {
                    continue;
                }
                let (s, t) = ((i as f64 + 0.5) / cols as f64 - 0.5, (j as f64 + 0.5) / 2.0 - 0.5);
                let p = add(c, add(mul(u, s * (pw - 10.0)), mul(v, t * (pd - 10.0))));
                out.push((p, rot, Kind::Flowerbed, look(220 + i as i64, j as i64), 1.0));
            }
        }
        if fountain {
            out.push((c, rot, Kind::Fountain, (key >> 28) as u8 & 7, 1.0));
        }
        // Topiary at its corners; a bench on each long side facing in.
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let p = add(c, add(mul(u, a * (0.5 * pw - 2.5)), mul(v, b * (0.5 * pd - 2.5))));
            out.push((p, 0.0, Kind::Topiary, (key >> 32) as u8 & 7, 1.0));
        }
        for b in [-1.0, 1.0] {
            out.push((add(c, mul(v, b * (0.5 * pd + 3.0))), crate::park::facing(mul(v, -b)), Kind::Bench, (key >> 36) as u8 & 7, 1.0));
        }
        // A short hedge behind it, open at both ends, each piece wholly inside one chunk.
        let side = if roll(230) < 0.5 { 1.0 } else { -1.0 };
        let pieces = crate::core::floor((pw - 5.0) / 5.0) as i64;
        for k in 0..pieces {
            let p = add(c, add(mul(u, (k as f64 + 0.5) * 5.0 - 0.5 * pieces as f64 * 5.0), mul(v, side * (0.5 * pd + 9.0))));
            let pr = (p, rot, Kind::Hedge, (key >> 40) as u8 & 3, 1.0);
            if crate::park::seam_inside(&pr) && free(&placed, p, 3.0, None) {
                out.push(pr);
            }
        }
        placed.push((c, 0.5 * pw.max(pd) + 14.0));
    }
    // A few planted trees, spaced out over the yard (the biome's own).
    let tree = greenery(land_biome(t0, c0)).0.first().copied().unwrap_or(Kind::TreeDeciduous);
    let want = 3 + (roll(300) * 4.0) as usize;
    let mut cand: Vec<[f64; 2]> = spots.iter().copied().filter(|&p| room(p) >= 14.0).collect();
    cand.sort_by_key(|p| (hash2(key, crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64), crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64));
    let mut trees: Vec<[f64; 2]> = Vec::new();
    for p in cand {
        if trees.len() >= want {
            break;
        }
        if free(&placed, p, 8.0, None) && trees.iter().all(|q| dist(*q, p) >= 40.0) {
            placed.push((p, 8.0));
            out.push((p, 0.0, tree, look(310, trees.len() as i64), 1.0));
            trees.push(p);
        }
    }
    true
}

/// The other open spaces dressed by what they are for (`town::squares`), each a pure function of
/// its plaza (world ft) and the layout round it, so every chunk sets out the same:
/// - a widened crossroads: a signpost as near its middle as the lanes leave room (a city's, now
///   and then, a market cross in the middle for the traffic to go round), a lamp or two, a trough,
///   a notice board, a stack of barrels or crates;
/// - a well square: the well on a stepped plinth in the middle, troughs by it, lamps, a notice
///   board, sometimes stocks;
/// - a fountain square: a fountain in the middle (raised a tier, as the main square's; a well on
///   its plinth where the square is too small), lamp posts spread round it, one look for all;
/// - a green: a well, a cross or a great tree in the middle, the biome's trees round its edge clear
///   of the lanes, its shrubs and plants, sometimes stocks;
/// - a temple's close: a standing cross before the temple, a hedge round its churchyard behind it
///   and headstones in rows square to it, the biome's trees round the close.
fn dress_square(pl: &crate::town::Plaza, l: &crate::town::Layout, t0: &T0, lanes: &[([f64; 2], [f64; 2], f64)], seed: u64, features: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>, out: &mut Vec<([f64; 2], f32, Kind, u8)>) {
    use crate::town::PlazaKind as K;
    use crate::town::geom::{add, area, centroid, contains, dist, len, lerp, mul, sub};
    let poly = &pl.pts;
    let mut room = Room::new(poly, l, lanes, &[], seed ^ 0xd2e5);
    let c = room.c;
    let mut rng = Pcg32::new(hash2(seed ^ 0xd2e5, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 19);
    let city = l.tier >= crate::t0::settle::Tier::City;
    let reach = poly.iter().map(|q| dist(*q, c)).fold(0.0, f64::max);
    let outward = |p: [f64; 2]| libm::atan2(p[1] - c[1], p[0] - c[0]) as f32;
    let (trees, shrubs, plants, few) = greenery(land_biome(t0, c));
    let pick = |rng: &mut Pcg32, kinds: &[Kind]| kinds[rng.below(kinds.len() as u32) as usize];
    // Scores: nearest the middle; at a distance from it; just inside the edge.
    let middle = |salt: i64| move |r: &Room, p: [f64; 2]| -dist(p, r.c) + 2.0 * r.jit(p, salt);
    let ring = |at: f64, salt: i64| move |r: &Room, p: [f64; 2]| -(dist(p, r.c) - at).abs() + 3.0 * r.jit(p, salt);
    let edge = |want: f64, salt: i64| move |r: &Room, p: [f64; 2]| -(r.depth(p) - want).abs() + 3.0 * r.jit(p, salt);
    let cover = |room: &mut Room, rng: &mut Pcg32, out: &mut Vec<([f64; 2], f32, Kind, u8)>, salt: i64| {
        let kind = if rng.next_f64() < 0.5 { Kind::Barrel } else { Kind::Crate };
        if let Some(p) = room.best(foot(kind) + 1.0, true, edge(5.0, salt)) {
            out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8));
        }
    };
    // A well on a stepped round plinth in the middle.
    let well = |room: &mut Room, rng: &mut Pcg32, out: &mut Vec<([f64; 2], f32, Kind, u8)>, features: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>| {
        if !room.free(c, 7.0, false) {
            return false;
        }
        features.push((c, 6.0, PlazaFeature::Dais, rng.range(0.0, std::f64::consts::TAU)));
        room.taken.push((c, 9.0));
        out.push((c, 0.0, Kind::Well, rng.below(4) as u8));
        true
    };
    // Trees of the biome spread round a lawn, `inset` ft in from its edge.
    let grove = |room: &mut Room, rng: &mut Pcg32, out: &mut Vec<([f64; 2], f32, Kind, u8)>, n: i64, inset: f64| {
        let mut at: Vec<[f64; 2]> = Vec::new();
        let n = if few { n.min(1 + rng.below(2) as i64) } else { n };
        for k in 0..if trees.is_empty() { 0 } else { n } {
            let kind = pick(rng, trees);
            let spread = |r: &Room, p: [f64; 2]| -0.5 * (r.depth(p) - inset).abs() + 0.2 * at.iter().map(|q| dist(*q, p)).fold(100.0, f64::min) + 2.0 * r.jit(p, 10 + k);
            if let Some(p) = room.best(foot(kind), true, spread) {
                at.push(p);
                out.push((p, 0.0, kind, rng.below(4) as u8));
            }
        }
    };
    let start = out.len();
    match pl.kind {
        K::Crossroads => {
            if city && rng.next_f64() < 0.35 && room.free(c, cross_ft(), false) {
                room.taken.push((c, cross_ft()));
                out.push((c, 0.0, Kind::MarketCross, [0, 1, 3][rng.below(3) as usize]));
            } else if let Some(p) = room.best(foot(Kind::Signpost) + 1.0, true, middle(1)) {
                out.push((p, 0.0, Kind::Signpost, [1, 3][rng.below(2) as usize]));
            }
            let look = lamp_look(&mut rng, false);
            room.lamps(if city { 2 } else { (rng.next_f64() < 0.6) as i64 }, 0.6 * reach, false, look, out);
            // A trough where there's room, a notice board on some (on every one without a trough).
            let trough = room.best(foot(Kind::WaterTrough), true, ring(0.55 * reach, 20));
            if let Some(p) = trough {
                out.push((p, outward(p) + std::f32::consts::FRAC_PI_2, Kind::WaterTrough, rng.below(4) as u8));
            }
            if (trough.is_none() || rng.next_f64() < 0.5)
                && let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(4.0, 30))
            {
                out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
            }
            // At least two things to see: else by the edge, where the streets meet the houses, a
            // signpost (if none yet), a lamp post, a notice board.
            let signed = out[start..].iter().any(|o| matches!(o.2, Kind::Signpost | Kind::MarketCross));
            for (salt, kind) in [(45, Kind::Signpost), (46, Kind::LampPost), (47, Kind::NoticeBoard)] {
                if out.len() - start >= 2 || (kind == Kind::Signpost && signed) {
                    continue;
                }
                if let Some(p) = room.best(foot(kind) + 1.0, false, edge(3.0, salt)) {
                    let v = match kind {
                        Kind::Signpost => [1, 3][rng.below(2) as usize],
                        Kind::LampPost => look,
                        _ => rng.below(4) as u8,
                    };
                    out.push((p, 0.0, kind, v));
                }
            }
            for k in 0..1 + rng.below(2) as i64 {
                cover(&mut room, &mut rng, out, 40 + k);
            }
        }
        K::WellSquare => {
            well(&mut room, &mut rng, out, features);
            let trough = if city { [1, 3][rng.below(2) as usize] } else { [0, 2, 3][rng.below(3) as usize] };
            for k in 0..1 + (rng.next_f64() < 0.5) as i64 {
                if let Some(p) = room.best(foot(Kind::WaterTrough), false, ring(15.0, 10 + k)) {
                    out.push((p, outward(p) + std::f32::consts::FRAC_PI_2, Kind::WaterTrough, trough));
                }
            }
            let look = lamp_look(&mut rng, false);
            room.lamps(if city { 2 } else { (rng.next_f64() < 0.5) as i64 }, 0.65 * reach, false, look, out);
            if rng.next_f64() < 0.4
                && let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(4.0, 30))
            {
                out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
            }
            if rng.next_f64() < if city { 0.15 } else { 0.25 }
                && let Some(p) = room.best(foot(Kind::Stocks), true, edge(6.0, 31))
            {
                out.push((p, 0.0, Kind::Stocks, rng.below(4) as u8));
            }
            cover(&mut room, &mut rng, out, 40);
        }
        K::FountainSquare => {
            let fr = (0.3 * reach).clamp(7.0, 12.0);
            if room.free(c, fr + 2.0, false) {
                features.push((c, fr, PlazaFeature::Fountain, rng.range(0.0, std::f64::consts::TAU)));
                room.taken.push((c, fr + 4.0));
            } else {
                well(&mut room, &mut rng, out, features);
            }
            let look = lamp_look(&mut rng, city);
            room.lamps(3 + city as i64, fr + 0.5 * (reach - fr), false, look, out);
            if rng.next_f64() < 0.3
                && let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(4.0, 30))
            {
                out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
            }
        }
        K::Green => {
            let mid = rng.next_f64();
            if mid < 0.4 && well(&mut room, &mut rng, out, features) {
            } else if mid < 0.65 && room.free(c, cross_ft(), false) {
                room.taken.push((c, cross_ft() + 2.0));
                out.push((c, 0.0, Kind::MarketCross, [0, 2][rng.below(2) as usize]));
            } else if !trees.is_empty() && room.free(c, 9.0, true) {
                // A great tree in the middle.
                room.taken.push((c, 9.0));
                out.push((c, 0.0, pick(&mut rng, trees), rng.below(4) as u8));
            }
            grove(&mut room, &mut rng, out, ((area(poly).abs() / 3000.0) as i64).clamp(2, 6), 10.0);
            for k in 0..if shrubs.is_empty() { 0 } else { 1 + rng.below(3) as i64 } {
                let kind = pick(&mut rng, shrubs);
                if let Some(p) = room.best(foot(kind), true, edge(4.0, 20 + k)) {
                    out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8));
                }
            }
            for k in 0..if plants.is_empty() { 0 } else { 2 + rng.below(3) as i64 } {
                let kind = pick(&mut rng, plants);
                if let Some(p) = room.best(foot(kind), true, |r: &Room, p: [f64; 2]| r.jit(p, 30 + k)) {
                    out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8));
                }
            }
            if rng.next_f64() < 0.3
                && let Some(p) = room.best(foot(Kind::Stocks), true, edge(6.0, 40))
            {
                out.push((p, 0.0, Kind::Stocks, rng.below(4) as u8));
            }
        }
        K::TempleClose => {
            // The temple and its churchyard: what stands on the close, roofed and open.
            let on = |s: crate::town::Structure| {
                l.buildings.iter().filter(|b| b.structure == s).map(|b| &b.poly).find(|b| {
                    let (x0, y0, x1, y1) = bounds(b);
                    contains(poly, [0.5 * (x0 + x1), 0.5 * (y0 + y1)])
                })
            };
            let temple = on(crate::town::Structure::Roofed);
            let tc = temple.map_or(c, |t| centroid(t));
            if let Some(t) = temple.filter(|t| t.len() >= 3) {
                // A standing cross before its front: of the hall's sides, the one toward the middle
                // of the town (as the close was laid out).
                let u = sub(t[1], t[0]);
                let u = mul(u, 1.0 / len(u).max(1e-9));
                let to = sub(l.center, tc);
                let front = [u, mul(u, -1.0), [-u[1], u[0]], [u[1], -u[0]]].into_iter().max_by(|a, b| crate::town::geom::dot(*a, to).total_cmp(&crate::town::geom::dot(*b, to))).unwrap_or(u);
                let ext = t.iter().map(|q| crate::town::geom::dot(sub(*q, tc), front)).fold(0.0, f64::max);
                let at = add(tc, mul(front, ext + 2.0 + cross_ft()));
                room.taken.push((at, cross_ft() + 2.0));
                out.push((at, 0.0, Kind::MarketCross, [0, 2][rng.below(2) as usize]));
            }
            if let Some(g) = on(crate::town::Structure::Open).filter(|g| g.len() == 4) {
                // A hedge round the churchyard 3 ft out, with a gap for its gate on the temple's side.
                let gc = centroid(g);
                let mid = |e: usize| lerp(g[e], g[(e + 1) % 4], 0.5);
                let gate = (0..4).min_by(|&a, &b| dist(mid(a), tc).total_cmp(&dist(mid(b), tc))).unwrap_or(0);
                for e in 0..4 {
                    let (a, b) = (g[e], g[(e + 1) % 4]);
                    let steps = (dist(a, b) / 5.0).floor().max(1.0) as usize;
                    for k in 0..=steps {
                        let q = lerp(a, b, k as f64 / steps as f64);
                        if e == gate && dist(q, mid(e)) < 6.0 {
                            continue;
                        }
                        let d = sub(q, gc);
                        let p = add(q, mul(d, 3.0 / len(d).max(1e-9)));
                        out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, Kind::Bush, rng.below(4) as u8));
                    }
                }
                // Headstones in rows square to the churchyard, a path kept from its gate across.
                let (e0, e1) = (sub(g[1], g[0]), sub(g[3], g[0]));
                let (l0, l1) = (len(e0), len(e1));
                let (u0, u1) = (mul(e0, 1.0 / l0.max(1e-9)), mul(e1, 1.0 / l1.max(1e-9)));
                let (gm, way) = (mid(gate), sub(gc, mid(gate)));
                let way = mul(way, 1.0 / len(way).max(1e-9));
                let mut a = 4.5;
                while a <= l0 - 4.5 {
                    let mut b = 4.5;
                    while b <= l1 - 4.5 {
                        let p = add(g[0], add(mul(u0, a), mul(u1, b)));
                        let off = sub(p, gm);
                        let across = (off[0] * way[1] - off[1] * way[0]).abs();
                        if across > 3.5 && room.jit(p, 50) < 0.85 {
                            out.push((p, 0.0, Kind::Headstone, rng.below(4) as u8));
                        }
                        b += 8.0;
                    }
                    a += 5.0;
                }
                room.taken.push((gc, g.iter().map(|q| dist(*q, gc)).fold(0.0, f64::max) + 4.0));
            }
            let perimeter: f64 = (0..poly.len()).map(|e| dist(poly[e], poly[(e + 1) % poly.len()])).sum();
            grove(&mut room, &mut rng, out, ((perimeter / 50.0) as i64).clamp(3, 8), 8.0);
        }
        _ => {}
    }
    // Every square has its focus, however little room it leaves: a signpost at a crossroads, a
    // well at a well or fountain square, a tree or a well on a green.
    if out.len() == start {
        let kind = match pl.kind {
            K::Crossroads => Kind::Signpost,
            K::Green if !trees.is_empty() => trees[0],
            _ => Kind::Well,
        };
        let p = room.best(foot(kind), false, middle(90)).unwrap_or(c);
        out.push((p, 0.0, kind, rng.below(4) as u8));
    }
}

/// A town plaza's market as a little town of its own: jittered seeds inside a wobbly oval round
/// the well make Voronoi cells, each pulled back from its neighbours so lanes run between them
/// (streets for players and passers-by). Each block's outline is rounded and wobbled, and its
/// stalls follow that curve shoulder to shoulder, facing out onto the lanes, with a gap to get
/// in; crates and barrels stack inside. The cell at the well (and a few others) stays an open
/// square, as does any cell holding a monument; stalls keep clear of the features (`keep`: centre
/// and radius, laid out first on a city's square). A pure function of the plaza (world ft), so
/// every chunk lays out the same market.
fn market_blocks(poly: &[[f64; 2]], monuments: &[Vec<[f64; 2]>], keep: &[([f64; 2], f64)], mk: Market, seed: u64, out: &mut Vec<([f64; 2], f32, Kind)>) {
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
    let (hu, hv) = (rng.range(mk.hu.0, mk.hu.1), rng.range(mk.hv.0, mk.hv.1));
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
        let open = Some(i) == well || rng.next_f64() < mk.open;
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
        let count = (total / mk.spacing).floor() as usize;
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
            if gap.is_some_and(|g| k == g || k == (g + 1) % count) || rng.next_f64() < mk.drop {
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
/// 70 ft from a raised feature, more trees go in, mirrored across both axes. A civic square
/// (`civic`) has no pattern, its market cross for a landmark: its trees go by its edges.
#[allow(clippy::too_many_arguments)]
fn plaza_features(
    poly: &[[f64; 2]],
    monuments: &[Vec<[f64; 2]>],
    stalls: &[([f64; 2], f32, Kind)],
    lanes: &[([f64; 2], [f64; 2], f64)],
    seed: u64,
    patterned: bool,
    civic: bool,
    out: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>,
) {
    use crate::town::geom::{add, area, centroid, contains, dist, dot, main_axis, mul, seg_dist, sub};
    use std::f64::consts::TAU;
    let c = centroid(poly);
    let mut rng = Pcg32::new(hash2(seed ^ 0xf0a7, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 13);
    let first = out.len();
    // (All must hold: most candidates fail at the rim, the lanes cost most.)
    let rim: [[f64; 2]; 12] = std::array::from_fn(|k| {
        let t = TAU * k as f64 / 12.0;
        [libm::cos(t), libm::sin(t)]
    });
    let fits = |out: &[([f64; 2], f64, PlazaFeature, f64)], p: [f64; 2], r: f64| {
        rim.iter().all(|u| contains(poly, add(p, [(r + 6.0) * u[0], (r + 6.0) * u[1]])))
            && dist(p, c) > r + 22.0
            && out[first..].iter().all(|f| dist(p, f.0) > r + f.1 + 14.0)
            && stalls.iter().all(|s| dist(p, s.0) > r + 10.0)
            && monuments.iter().all(|m| {
                let mc = centroid(m);
                dist(p, mc) > r + 8.0 + m.iter().map(|q| dist(*q, mc)).fold(0.0, f64::max)
            })
            && lanes.iter().all(|&(a, b, w)| seg_dist(p, a, b) > r + w + 4.0)
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
    // (A civic square has its cross for a landmark (`main_cross`) and trees by its edges, no more.)
    for attempt in (0..3).filter(|_| !civic) {
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
    let m = poly.len();
    let by_edge = |p: [f64; 2]| (0..m).map(|e| seg_dist(p, poly[e], poly[(e + 1) % m])).fold(f64::MAX, f64::min) <= r_tree + 14.0;
    for _ in 0..16 {
        let worst = samples
            .iter()
            .filter(|&&s| given_up.iter().all(|g| dist(*g, s) > 40.0))
            .map(|&s| (reach(&out[..], s), s))
            .filter(|&(d, _)| d > 70.0)
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, s)) = worst else { break };
        let mut spots: Vec<[f64; 2]> = (0..81).map(|q| [s[0] + ((q % 9) as f64 - 4.0) * 8.0, s[1] + ((q / 9) as f64 - 4.0) * 8.0]).filter(|&p| !civic || by_edge(p)).collect();
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

/// The nearest square (within 8) at a park's gravel edge (`park` bit 8) that is off the buildings.
fn gravel_edge(c: &Chunk, k: usize, park: &[u8]) -> Option<usize> {
    if park.is_empty() {
        return None;
    }
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    for r in 0..=8i64 {
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (x + dx, y + dy);
                if (dx.abs() != r && dy.abs() != r) || nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                    continue;
                }
                let nk = ny as usize * SQ + nx as usize;
                if park[nk] & 8 != 0 && c.building[nk] == 0 && c.water_level[nk] <= c.height[nk] {
                    return Some(nk);
                }
            }
        }
    }
    None
}

/// Nearest dry, non-road square within a few squares (by ring), for roadside placements (off
/// `keep_off`).
fn nearest_verge(c: &Chunk, k: usize, keep_off: &[bool]) -> Option<usize> {
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    // The nearest square clear of the road, else the nearest one just off it.
    let mut verge = None;
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
                if !road_bed && !keep_off[nk] && c.building[nk] == 0 && c.water_level[nk] <= c.height[nk] {
                    if clear_of_roads(c, nk) {
                        return Some(nk);
                    }
                    verge = verge.or(Some(nk));
                }
            }
        }
    }
    verge
}

/// What the guarantee puts down in a park for cover (or, with `feature`, to see): a formal garden's
/// topiary and statues (beds); a common's shrubs and the biome's trees (wildflowers, tall grass).
/// No benches: a bench stands by a path (the park's own, or the max gap's at a round's edge).
fn park_cover(formal: bool, b: Biome, feature: bool, rng: &mut Pcg32) -> Kind {
    let (trees, shrubs, _, _) = greenery(b);
    let shrub = shrubs.first().copied().unwrap_or(Kind::Bush);
    let opts: Vec<Kind> = match (formal, feature) {
        (true, false) => vec![Kind::Topiary, Kind::Topiary, Kind::GardenStatue],
        (true, true) => vec![Kind::Flowerbed],
        (false, false) => vec![shrub, shrub, trees.first().copied().unwrap_or(Kind::TreeDeciduous)],
        (false, true) => vec![Kind::Flowers, Kind::TallGrass],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

/// What the guarantee sets out in a castle's bailey, as cover or (`feature`) a prop: by a building
/// (within three squares), stores (barrels, crates, firewood); out in the yard a cart, a trough or a
/// haystack (in a garden yard, one of the biome's trees planted for the haystack), as a prop a
/// trough or a cart.
fn yard_kind(c: &Chunk, k: usize, garden: bool, b: Biome, feature: bool, rng: &mut Pcg32) -> Kind {
    // (By a wall or a tower, small stores: nothing reaching it.)
    let by_building = within(k, 3, |q| c.building[q] != 0 || matches!(c.urban[q], URBAN_WALL | URBAN_TOWER));
    let tree = greenery(b).0.first().copied().unwrap_or(Kind::TreeDeciduous);
    let opts: &[Kind] = match (by_building, garden, feature) {
        (true, _, _) => &[Kind::Barrel, Kind::Crate, Kind::Firewood],
        (false, true, false) => &[Kind::Cart, Kind::WaterTrough, tree],
        (false, false, false) => &[Kind::Cart, Kind::WaterTrough, Kind::Haystack],
        (false, _, true) => &[Kind::WaterTrough, Kind::Cart],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

/// Whether any square within `r` (each way) of square `k` is one `of`.
fn within(k: usize, r: i64, of: impl Fn(usize) -> bool) -> bool {
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    (-r..=r).any(|dy| {
        (-r..=r).any(|dx| {
            let (i, j) = (x + dx, y + dy);
            i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && of(j as usize * SQ + i as usize)
        })
    })
}

/// Cover in town (streets, plazas) or on fields (a lawn takes the biome's own).
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
/// `rowless`: graveyard squares whose headstones are laid otherwise (empty: none).
fn place_town_props(c: &mut Chunk, seed: u64, gsx: i64, gsy: i64, rowless: &[bool]) {
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
            URBAN_GRAVEYARD if (gsx + i as i64) % 2 == 0 && (gsy + j as i64) % 3 == 0 && r < 0.8 && !rowless.get(k).is_some_and(|&x| x) => Some(Kind::Headstone),
            URBAN_FIELD if r < 0.004 => Some(Kind::Haystack),
            _ => None,
        };
        if let Some(kind) = kind {
            let mut rng = Pcg32::new(h, 7);
            push_at(c, kind, k, &mut rng);
        }
    }
}

/// Put a `kind` down in square `k`, unless no way of it fits off the buildings (false).
fn push_at(c: &mut Chunk, kind: Kind, k: usize, rng: &mut Pcg32) -> bool {
    let mut o = Object {
        kind,
        variant: rng.below(8) as u8,
        x: (k % SQ) as f32 + rng.range(0.25, 0.75) as f32,
        y: (k / SQ) as f32 + rng.range(0.25, 0.75) as f32,
        rot: rng.range(0.0, std::f64::consts::TAU) as f32,
        scale: rng.range(0.85, 1.2) as f32,
        sprite: 0,
        turned: false,
    };
    // Nothing reaches over a building; tall props draw above roofs, so not past the chunk edge
    // either (where a neighbour's building may stand): try it smaller and centred, a tall one
    // else as a crate, else nothing.
    let tall = info(kind as u16).height_ft >= 5.0;
    let fits = |c: &Chunk, o: &Object| if info(o.kind as u16).height_ft >= 5.0 { clear_of_buildings(c, o) } else { off_buildings(c, o) };
    if !fits(c, &o) {
        o.x = (k % SQ) as f32 + 0.5;
        o.y = (k / SQ) as f32 + 0.5;
        o.scale = 0.85;
        if !fits(c, &o) {
            // (On a park's lawn or gravel, a bush; else a crate, or in a lane one square wide, barrels.)
            let park = matches!(c.urban[k], URBAN_PARK | URBAN_PATH);
            let tries: &[Kind] = if park { &[Kind::Bush] } else if tall { &[Kind::Crate, Kind::Barrel] } else { &[Kind::Barrel] };
            let Some(&small) = tries.iter().find(|&&kd| kd != kind && off_buildings(c, &Object { kind: kd, ..o })) else { return false };
            o.kind = small;
        }
    }
    c.objects.push(o);
    true
}

/// Whether an object's footprint disc touches no building square of the chunk.
fn off_buildings(c: &Chunk, o: &Object) -> bool {
    !touches_square(o, |k| c.building[k] != 0)
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

/// How far square `k` may be from cover (`cover_distance`): `MAX_COVER_GAP`, in a castle's yard
/// `YARD_COVER_GAP`.
pub fn cover_gap(c: &Chunk, k: usize) -> i32 {
    if c.yard.get(k).is_some_and(|&y| y & 3 != 0) { YARD_COVER_GAP } else { MAX_COVER_GAP }
}

/// Whether the 32x32 window at (`wx`, `wy`) is a castle's yard's (half its squares not under water
/// at least) holding its cover: a wall, a tower or a building on any of its squares.
pub fn works_cover(c: &Chunk, wx: usize, wy: usize) -> bool {
    const W: usize = 32;
    if c.yard.is_empty() {
        return false;
    }
    let (mut yard, mut wet, mut works) = (0, 0, false);
    for y in wy..wy + W {
        for x in wx..wx + W {
            let k = y * SQ + x;
            yard += (c.yard[k] & 3 != 0) as usize;
            wet += (c.water_level[k] > c.height[k]) as usize;
            works |= c.building[k] != 0 || matches!(c.urban[k], URBAN_WALL | URBAN_TOWER);
        }
    }
    works && yard * 2 >= W * W - wet
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
///   objects: (kind u16, variant u8, turned u8 (1: put down by hand and drawn at rot,
///   `Placed::turned`; 0 otherwise), x f32, y f32, rot f32, scale f32) × n
///   road halo u8[HS²]: road class + 1 (0 = none), 0x80 = bridge deck
///   building u16[SQ²]: index + 1 of the building on each square (0 = none)
///   buildings: count u32, then per building: n u8, floors u8, roof u8 (style: 0 hip,
///   1 battlements, 2 cone; | (tint + 1) << 2, 0 = picked),
///   corner-tower side u8 (squares; 255: a keep of pieces), then n × (x, y) f32 in
///   chunk squares (heights above are the ground; roofs are drawn from these outlines); a keep
///   of pieces then has its pieces: count u8, then per piece `part_code` u8, n u8, n × (x, y) f32
///   structures: count u32, then per shape: kind u8 (`ShapeKind`), n u8, 0, 0, size f32,
///   then n × (x, y) f32 in chunk squares (drawn as vectors; decks are not in the road halo; a
///   flight of steps' third point is its start up the flight and its tread, in squares)
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
        // Drawn at `rot` (`Placed::turned`); 0 for generated objects, so their bytes are as before.
        out.push(o.turned as u8);
        for v in [o.x, o.y, o.rot, o.scale] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend(c.road_halo.iter().map(|&r| if r & 0x80 != 0 { 0 } else { r }));
    for b in &c.building {
        out.extend_from_slice(&b.to_le_bytes());
    }
    out.extend_from_slice(&(c.building_polys.len() as u32).to_le_bytes());
    for (k, (poly, floors, roof, tower)) in c.building_polys.iter().enumerate() {
        let parts = c.building_parts.get(k).filter(|p| !p.is_empty());
        out.extend_from_slice(&[poly.len().min(255) as u8, *floors, *roof, if parts.is_some() { 255 } else { *tower }]);
        for p in poly.iter().take(255) {
            out.extend_from_slice(&p[0].to_le_bytes());
            out.extend_from_slice(&p[1].to_le_bytes());
        }
        // A keep's pieces (its corner-tower byte 255): count u8, then per piece code u8, n u8, n ×
        // (x, y) f32.
        if let Some(parts) = parts {
            out.push(parts.len().min(255) as u8);
            for (code, pts) in parts.iter().take(255) {
                out.extend_from_slice(&[*code, pts.len().min(255) as u8]);
                for p in pts.iter().take(255) {
                    out.extend_from_slice(&p[0].to_le_bytes());
                    out.extend_from_slice(&p[1].to_le_bytes());
                }
            }
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
