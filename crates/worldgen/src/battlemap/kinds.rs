//! The object catalog: kinds, hazards and their 5e data, and the JSON the app reads.

use super::*;

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
    // v63 countryside
    FruitTree = 62,
    WaterWheel = 63,
    // v63 castles: a castle's kitchen garden and smithy (`kitchen.rs`).
    VegRow = 75,
    WattleFence = 76,
    Beehive = 77,
    Anvil = 78,
    // R5 towns: a cemetery's (`town::cemetery`).
    Cairn = 100,
    StandingStone = 101,
    Yew = 102,
    Lychgate = 103,
    Kurgan = 104,
    TombSlab = 105,
    // R5 towns: a steppe town's horse pens and market (`town::horses`).
    Horse = 106,
    HitchingRail = 107,
    // R5 towns: a star wall's bastion (`town::wallside::bastion_props`).
    Ballista = 108,
    ShotStones = 109,
    // R5 M20: a harbour's (`town::harbour`).
    Rowboat = 140,
    FishingBoat = 141,
    Bollard = 142,
    DockCrane = 143,
    NetRack = 144,
    Buoy = 145,
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

pub(super) const fn k(id: Kind, name: &'static str, radius: f32, blocks_move: bool, blocks_sight: bool, cover: u8, difficult: bool, height_ft: f32, feature: bool) -> KindInfo {
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

pub const CATALOG: [KindInfo; 63] = [
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
    // v63 countryside
    k(Kind::FruitTree, "fruit tree", 1.0, false, false, 1, true, 15.0, false),
    k(Kind::WaterWheel, "water wheel", 1.4, true, false, 2, false, 14.0, true),
];

/// What a rock floor keeps off: every tree and the ground's plants.
pub(super) const TREES_AND_COVER: [Kind; 16] = [
    Kind::TreeDeciduous, Kind::TreeConifer, Kind::TreePalm, Kind::TreeJungle, Kind::TreeAcacia, Kind::TreeDead, Kind::TreeWillow, Kind::Bush,
    Kind::Thicket, Kind::FallenLog, Kind::Stump, Kind::Reeds, Kind::Cactus, Kind::TallGrass, Kind::Flowers, Kind::Brambles,
];

/// Every kind, by id - 1.
const KINDS: [Kind; 63] = [
    Kind::TreeDeciduous, Kind::TreeConifer, Kind::TreePalm, Kind::TreeJungle, Kind::TreeAcacia, Kind::TreeDead, Kind::TreeWillow, Kind::Bush,
    Kind::Thicket, Kind::FallenLog, Kind::Stump, Kind::Boulder, Kind::RockSmall, Kind::RockPile, Kind::Reeds, Kind::Cactus, Kind::TallGrass,
    Kind::Flowers, Kind::Mushrooms, Kind::Bones, Kind::Snowdrift, Kind::IceSpire, Kind::BasaltPillar, Kind::Obsidian, Kind::Brambles, Kind::Bog,
    Kind::Quicksand, Kind::ThinIce, Kind::LavaPool, Kind::SteamVent, Kind::Scree, Kind::Sinkhole, Kind::Barrel, Kind::Crate, Kind::Cart,
    Kind::MarketStall, Kind::Well, Kind::Haystack, Kind::Headstone, Kind::RuinWall, Kind::Statue, Kind::Tent, Kind::Campfire, Kind::Firewood,
    Kind::Bedroll, Kind::WaterTrough, Kind::NoticeBoard, Kind::Signpost, Kind::Stocks, Kind::LampPost, Kind::MarketCross, Kind::Fountain, Kind::Pond,
    Kind::Bench, Kind::Flowerbed, Kind::Hedge, Kind::Topiary, Kind::Gazebo, Kind::Bandstand, Kind::GardenStatue, Kind::Footbridge,
    // v63 countryside
    Kind::FruitTree, Kind::WaterWheel,
];

/// Kinds past `CATALOG`, with explicit ids (each lane's range, `docs/lane-common.md`).
static MORE: [&[(Kind, KindInfo)]; 5] = [
    // v63 castles
    &kitchen::KINDS,
    // R5 towns
    &crate::town::cemetery::KINDS,
    &crate::town::horses::KINDS,
    &crate::town::wallside::KINDS,
    // R5 M20
    &crate::town::harbour::KINDS,
];

fn more() -> impl Iterator<Item = &'static (Kind, KindInfo)> {
    MORE.iter().flat_map(|t| t.iter())
}

/// Every kind's rules (`CATALOG`, then the kinds past it).
pub fn catalog() -> impl Iterator<Item = &'static KindInfo> {
    CATALOG.iter().chain(more().map(|m| &m.1))
}

/// The kind with a catalog id.
pub fn kind_of(id: u16) -> Option<Kind> {
    KINDS.get((id as usize).checked_sub(1)?).copied().or_else(|| more().find(|m| m.1.id == id).map(|m| m.0))
}

pub fn info(kind: u16) -> &'static KindInfo {
    let i = (kind as usize).saturating_sub(1);
    if i < CATALOG.len() {
        return &CATALOG[i];
    }
    more().find(|m| m.1.id == kind).map_or(&CATALOG[CATALOG.len() - 1], |m| &m.1)
}

/// Object catalog (tactical data) as JSON for the renderer and agents.
pub fn catalog_json() -> String {
    serde_json::to_string(&catalog().collect::<Vec<_>>()).expect("serializable")
}
