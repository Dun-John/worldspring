//! The furniture catalog: items, their looks and sprites, design kinds and what each room holds.

use super::*;

/// Furniture: (kind, display name, cover, blocks movement, height ft).
pub const FURNITURE: &[(&str, &str, u8, bool, f32)] = &[
    ("bed", "bed", 1, false, 2.0),
    ("chest", "chest", 1, true, 2.5),
    ("wardrobe", "wardrobe", 3, true, 7.0),
    ("table", "table", 1, true, 3.0),
    ("long_table", "long table", 1, true, 3.0),
    ("desk", "desk", 1, true, 3.0),
    ("bench", "bench", 0, false, 1.5),
    ("pew", "pew", 1, false, 3.0),
    ("bar", "bar counter", 2, true, 4.0),
    ("counter", "counter", 2, true, 4.0),
    ("hearth", "hearth", 2, true, 5.0),
    ("oven", "oven", 2, true, 5.0),
    ("shelf", "shelves", 2, true, 7.0),
    ("bookcase", "bookcase", 3, true, 8.0),
    ("barrel", "barrel", 1, true, 3.5),
    ("keg_rack", "keg rack", 2, true, 5.0),
    ("crate", "crate stack", 2, true, 5.0),
    ("workbench", "workbench", 1, true, 3.0),
    ("forge", "forge", 2, true, 5.0),
    ("anvil", "anvil", 1, true, 3.0),
    ("weapon_rack", "weapon rack", 1, true, 6.0),
    ("altar", "altar", 2, true, 4.0),
    ("statue", "statue", 3, true, 9.0),
    ("sarcophagus", "sarcophagus", 2, true, 3.5),
    ("cage", "iron cage", 1, true, 7.0),
    ("throne", "throne", 1, true, 5.0),
    ("rug", "rug", 0, false, 0.0),
    ("couch", "couch", 1, true, 3.0),
    ("cauldron", "cauldron", 1, true, 3.0),
    ("alchemy_bench", "alchemy bench", 1, true, 3.5),
    ("telescope", "telescope", 1, true, 6.0),
    ("bell", "bell", 3, true, 6.0),
    ("bath", "bath", 1, true, 2.0),
    ("cot", "cot", 0, false, 1.5),
    ("pillar", "pillar", 3, true, 10.0),
    ("barricade", "barricade", 2, true, 4.0),
    ("display", "display table", 1, true, 3.0),
    ("vat", "brewing vat", 2, true, 6.0),
    ("chair", "chair", 0, false, 3.0),
    ("spiral_stair", "spiral stairs", 0, false, 0.0),
    ("rack", "rack", 1, true, 3.0),
    ("bucket", "slop bucket", 0, false, 1.0),
    ("booth_table", "booth table", 1, true, 3.0),
    ("booth_seat", "booth seat", 1, false, 3.5),
    ("trapdoor", "trapdoor (to the undercroft)", 0, false, 0.0),
    ("winch", "portcullis winch", 1, true, 4.0),
    ("sideboard", "sideboard", 1, true, 3.5),
    ("stage", "stage", 0, false, 2.0),
    // Great halls and ballrooms (`hall::furnish_hall`). The dais's steps are climbed, so it is
    // difficult ground like the underground dais; things hung on a wall or laid on the floor are
    // walked over; a standing banner is gone round but seen past.
    ("throne_dais", "throne dais", 1, false, 2.5),
    ("carpet_runner", "carpet runner", 0, false, 0.0),
    ("chair_of_office", "chair of office", 1, true, 5.0),
    ("high_table", "high table", 1, true, 3.0),
    ("tapestry", "tapestry", 0, false, 0.0),
    ("wall_banner", "wall banner", 0, false, 0.0),
    ("standing_banner", "standing banner", 0, true, 5.5),
    ("font", "font", 1, true, 3.5),
    ("cloister_well", "cloister well", 1, true, 3.5),
    ("dance_floor", "inlaid dance floor", 0, false, 0.0),
    ("harpsichord", "harpsichord", 1, true, 3.0),
    ("musicians_corner", "musicians' corner", 0, false, 3.0),
    ("refreshment_table", "refreshment table", 1, true, 3.0),
    // A palace court's gravel walks (laid on its lawn: walked over).
    ("garden_path", "gravel walk", 0, false, 0.0),
    // A low dais others stand on (`hall::is_platform`: the one piece another may overlap), climbed.
    ("low_dais", "low dais", 0, false, 1.0),
    // A temple's (`hall::Style::Nave`), each drawn for its god (`GOD_LOOKS`): the god's statue over
    // life size on its plinth, a tall stand of candles (gone round, seen past), the carved screen
    // across the nave (a wall to stand behind) and its gate (open: walked through as the floor is,
    // its sill flush, its beam overhead), a side chapel's altar.
    ("god_statue", "statue of the god", 3, true, 10.0),
    ("candle_stand", "candle stand", 0, true, 5.5),
    ("carved_screen", "carved screen", 2, true, 8.0),
    ("screen_gate", "screen gate", 0, false, 0.0),
    ("side_altar", "side altar", 2, true, 3.5),
    // A town hall's and a guild's (`hub::civic`): a strongbox, a counting desk with its ledgers, a
    // council table with its seats round it (the whole footprint blocks), hung on the wall a job board
    // and a guild's arms (`GUILD_ARMS`); a stable stall, walked into (its partitions half cover).
    ("strongbox", "strongbox", 1, true, 2.0),
    ("counting_desk", "counting desk", 1, true, 3.0),
    ("job_board", "job board", 0, false, 0.0),
    ("stable_stall", "stable stall", 1, false, 4.5),
    ("guild_arms", "guild's arms", 0, false, 0.0),
    ("council_table", "council table", 1, true, 3.0),
    // A roof terrace's (`furnish_terrace`): a dome on its drum (a wall to stand behind), a big clay
    // water jar, a planter of herbs (gone round), a washing line on its two posts (walked under).
    ("roof_dome", "dome", 3, true, 12.0),
    ("water_jar", "water jar", 1, true, 3.0),
    ("planter", "planter", 1, true, 2.0),
    ("washing_line", "washing line", 0, false, 0.0),
    // A yurt's (`furnish_yurt`): its hearth, a ring of stones round the fire (stepped over), cushions
    // round it; a low pallet with its bedding; a painted chest; a floor cushion.
    ("yurt_hearth", "hearth", 0, false, 1.5),
    ("yurt_pallet", "pallet and bedroll", 0, false, 1.0),
    ("painted_chest", "painted chest", 1, true, 2.5),
    ("floor_cushion", "floor cushion", 0, false, 0.5),
    // A tower mill's (`furnishing`): a pair of millstones in their wooden tun, the hopper over
    // them; the great spur wheel on the upright shaft; the sack hoist's trap and its winding drum.
    ("millstones", "millstones", 1, true, 3.5),
    ("spur_wheel", "great spur wheel", 1, true, 4.0),
    ("hoist_trapdoor", "sack hoist trapdoor", 0, false, 0.0),
    ("lighthouse_lamp", "lighthouse lamp", 2, true, 5.0),
    ("sack_hoist", "sack hoist", 1, true, 4.0),
];

/// A building's function key as its inside is built by: a hand-made hall is a town hall's and a chapel a shrine's (the nearest kinds
/// with a hub and furnishing of their own).
pub fn func_key(b: &town::Building) -> Option<&'static str> {
    b.func.map(|f| town::catalog::CATALOG[f as usize].key).map(|k| match k {
        "hall" => "town_hall",
        "chapel" => "shrine",
        k => k,
    })
}

/// A guild's arms' looks (`Item::variant` of `guild_arms`): the merchants', the craftsmen's, the
/// adventurers', the mages', the town's (a town hall's, a courthouse's).
pub const GUILD_ARMS: u8 = 5;

/// The arms a building of function `key` hangs (`GUILD_ARMS`): None for one that shows none (a
/// thieves' guild's).
pub fn arms_look(key: Option<&str>) -> Option<u8> {
    match key? {
        "merchant_guild" | "auction_house" => Some(0),
        "craft_guild" => Some(1),
        "adventurers_guild" => Some(2),
        "mages_guild" => Some(3),
        "thieves_guild" => None,
        _ => Some(4),
    }
}

/// The house colours a hall's banners fly (`Item::variant` of a wall or standing banner: the same
/// index on both, so a castle's banners can share one house).
pub const HOUSES: u8 = 8;

/// The looks of a temple's pieces (the god's statue, candle stands, the carved screen and its gate,
/// a side altar): one per kind of god (`town::temple::god_look`, `town::temple::GOD_KINDS`).
pub const GOD_LOOKS: u8 = 8;

/// Furniture with other than four looks (`Item::variant` 0 to looks - 1): the throne dais's are
/// three seatings (one throne, a pair, a throne and a lesser chair) in stone and gilt (0-2), then
/// on a timber platform (3-5); a banner's are its house's colours; a low dais is of stone (0) or
/// timber (1); a temple's pieces are its god's (`GOD_LOOKS`).
const LOOKS: &[(&str, u8)] = &[
    ("throne_dais", 6),
    // (A key's look is the colour of the lock it opens, 1 to `design::MAX_LOCK`.)
    ("key", design::MAX_LOCK + 1),
    ("wall_banner", HOUSES),
    ("standing_banner", HOUSES),
    ("low_dais", 2),
    ("god_statue", GOD_LOOKS),
    ("candle_stand", GOD_LOOKS),
    ("carved_screen", GOD_LOOKS),
    ("screen_gate", GOD_LOOKS),
    ("side_altar", GOD_LOOKS),
    ("guild_arms", GUILD_ARMS),
];

/// How many looks an item kind has (`Item::variant`): four but for `LOOKS`.
pub fn item_looks(kind: &str) -> u8 {
    LOOKS.iter().find(|l| l.0 == kind).map_or(4, |l| l.1)
}

/// (kind, display name, cover, blocks movement, height ft).
pub(super) fn item_info(kind: &'static str) -> (&'static str, u8, bool, f32) {
    // (Else an indoor prop, as underground: a mill's sacks.)
    FURNITURE
        .iter()
        .find(|f| f.0 == kind)
        .map(|f| (f.1, f.2, f.3, f.4))
        .or_else(|| crate::under::PROPS.iter().find(|p| p.0 == kind && INDOOR_PROPS.contains(&p.0)).map(|p| (p.1, p.2, p.3, p.4)))
        .unwrap_or(("furniture", 1, true, 3.0))
}

/// Clutter a building's design can hold besides its furniture (the designer's indoor props):
/// kinds of `under::PROPS`, with their rules, drawn as they are underground.
pub const INDOOR_PROPS: &[&str] = &[
    "sacks", "rubble", "timber", "bones", "skeleton", "skulls", "urn", "coffin", "effigy", "brazier", "candles", "sconce",
    "lantern", "banner", "chains", "cobweb", "bedroll", "tools", "powder", "hoard", "offering", "glyph", "dais", "well",
    "fountain", "iron_maiden", "stocks", "rat_nest", "trap", "collapsed_wall", "key", "ballista",
];

/// An item a building's design puts down, by kind or name: furniture (`FURNITURE`), else an
/// indoor prop (`INDOOR_PROPS`, with its hazard). Not uploaded pictures (`Sprites`).
pub fn building_item(kind: &str, x: usize, y: usize, w: usize, h: usize) -> Option<Item> {
    let at = |kind, name, cover, blocks_move, height_ft, hazard| Item { kind, name, x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover, blocks_move, height_ft, hazard, sprite: 0, back: None, variant: None };
    if let Some(f) = FURNITURE.iter().find(|f| f.0 == kind || f.1 == kind) {
        return Some(at(f.0, f.1, f.2, f.3, f.4, None));
    }
    let p = crate::under::PROPS.iter().find(|p| (p.0 == kind || p.1 == kind) && INDOOR_PROPS.contains(&p.0))?;
    Some(at(p.0, p.1, p.2, p.3, p.4, p.5))
}

/// Item kind `sprite`: an uploaded picture (`Item::sprite`).
pub const SPRITE: &str = "sprite";

/// The asset id of a design's item kind that names an uploaded picture (`s:<asset id>`).
pub fn sprite_asset(kind: &str) -> Option<&str> {
    kind.strip_prefix("s:").filter(|a| !a.is_empty() && a.len() <= 128)
}

/// Uploaded pictures put down as items while a site is built: each picture's rules
/// (`Edits::sprites`) and its place in `Interior::sprites`.
pub struct Sprites<'a> {
    metas: &'a std::collections::BTreeMap<String, crate::world::SpriteMeta>,
    used: Vec<String>,
}

impl<'a> Sprites<'a> {
    pub fn new(metas: &'a std::collections::BTreeMap<String, crate::world::SpriteMeta>) -> Self {
        Sprites { metas, used: Vec::new() }
    }

    /// Picture `asset` as an item. Its rules in the terms play mode reads items by
    /// (`play/vision.ts`): what blocks movement and stands 6 ft or more blocks sight; what
    /// doesn't block movement but stands 1 ft or more is difficult ground. So the height kept is
    /// moved across those lines as the picture's own rules say (a picture that blocks sight
    /// but not movement doesn't block sight indoors).
    pub fn item(&mut self, asset: &str, x: usize, y: usize, w: usize, h: usize) -> Item {
        let k = match self.used.iter().position(|a| a == asset) {
            Some(k) => k,
            None => {
                self.used.push(asset.to_string());
                self.used.len() - 1
            }
        };
        let m = self.metas.get(asset).cloned().unwrap_or_default();
        let h0 = if m.height_ft.is_finite() { m.height_ft.clamp(0.0, 100.0) } else { 3.0 };
        let height_ft = match (m.blocks_move, m.blocks_sight, m.difficult) {
            (true, true, _) => h0.max(6.0),
            (true, false, _) => h0.min(5.5),
            (false, _, true) => h0.max(1.0),
            (false, _, false) => h0.min(0.5),
        };
        Item { kind: SPRITE, name: "custom object", x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover: m.cover.min(3), blocks_move: m.blocks_move, height_ft, hazard: None, sprite: k as u16 + 1, back: None, variant: None }
    }

    /// `Interior::sprites`: the pictures used, in the order first put down.
    pub fn table(&self) -> Vec<InteriorSprite> {
        self.used
            .iter()
            .map(|a| {
                let name = self.metas.get(a).map(|m| m.name.trim()).filter(|n| !n.is_empty()).unwrap_or("custom object");
                InteriorSprite { asset: a.clone(), name: name.to_string() }
            })
            .collect()
    }
}

/// A design's item kind for item `f` of an interior: an uploaded picture as `s:<asset id>`.
pub fn design_kind(it: &Interior, f: &Item) -> String {
    match it.sprites.get((f.sprite as usize).wrapping_sub(1)) {
        Some(s) if f.sprite > 0 => format!("s:{}", s.asset),
        _ => f.kind.to_string(),
    }
}

#[derive(Clone, Copy)]
pub(super) enum Place {
    /// Against a wall (length along the wall, depth into the room).
    Wall(u16, u16),
    /// Free-standing, clear of the walls.
    Center(u16, u16),
    /// A one-square round table with a chair on each open side (four when the room allows).
    Seated,
    /// A table in a room corner with an L of bench seats along the two walls (2 × 2).
    Booth,
}

/// Furniture rules per room kind: (item, placement, how many: fixed plus one per `per`
/// squares of room, 0 = none).
pub(super) fn furnishing(kind: &str) -> &'static [(&'static str, Place, u8, u16)] {
    use Place::*;
    match kind {
        "common room" | "taproom" => &[("bar", Wall(3, 1), 1, 0), ("table", Seated, 2, 12), ("booth_table", Booth, 0, 24), ("hearth", Wall(2, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "brewhouse" => &[("vat", Center(2, 2), 1, 16), ("barrel", Wall(1, 1), 2, 8), ("workbench", Wall(2, 1), 1, 0)],
        "battlements" => &[("crate", Wall(1, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "kitchen" => &[("oven", Wall(2, 1), 1, 0), ("table", Center(2, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "storeroom" | "loft storage" => &[("crate", Wall(1, 1), 2, 6), ("barrel", Wall(1, 1), 1, 8), ("shelf", Wall(2, 1), 1, 0)],
        "keg cellar" | "wine cellar" => &[("keg_rack", Wall(2, 1), 2, 8), ("barrel", Wall(1, 1), 2, 6)],
        "bedroom" | "guest room" | "owner's quarters" | "chamber" => &[("bed", Wall(1, 2), 1, 0), ("chest", Wall(1, 1), 1, 0), ("wardrobe", Wall(1, 1), 1, 0)],
        "master bedroom" | "lord's chamber" | "bedchamber" => &[("bed", Wall(2, 2), 1, 0), ("wardrobe", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        // A one-room home's: its bed against a wall by the hearth.
        // A yurt's: its own (`furnish_yurt`).
        "yurt" => &[],
        // (R5 towns: a felt hall's: its own, `felt::furnish`.)
        "felt hall" | "felt sanctuary" | "felt common room" => &[],
        "hearth room" => &[("bed", Wall(1, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("table", Center(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "main room" | "living room" => &[("hearth", Wall(2, 1), 1, 0), ("table", Center(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("bench", Wall(2, 1), 1, 0)],
        "shop" => &[("counter", Wall(3, 1), 1, 0), ("shelf", Wall(2, 1), 2, 10), ("display", Center(2, 1), 0, 12)],
        "workshop" => &[("workbench", Wall(2, 1), 1, 8), ("shelf", Wall(2, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "forge" => &[("forge", Wall(2, 2), 1, 0), ("anvil", Center(1, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0), ("weapon_rack", Wall(2, 1), 1, 0), ("workbench", Wall(2, 1), 1, 0)],
        "warehouse floor" => &[("crate", Center(2, 2), 1, 10), ("shelf", Wall(2, 1), 1, 10), ("barrel", Wall(1, 1), 2, 0)],
        "office" | "archive" => &[("desk", Center(2, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 8), ("chest", Wall(1, 1), 1, 0)],
        "study" => &[("desk", Center(2, 1), 1, 0), ("bookcase", Wall(2, 1), 2, 0), ("rug", Center(2, 2), 1, 0)],
        "meeting room" => &[("long_table", Center(3, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 0)],
        "library" | "reading room" => &[("bookcase", Wall(2, 1), 2, 6), ("table", Center(2, 1), 1, 16)],
        "laboratory" => &[("alchemy_bench", Wall(2, 1), 2, 0), ("cauldron", Center(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "observatory" => &[("telescope", Center(2, 2), 1, 0), ("desk", Wall(2, 1), 1, 0), ("bookcase", Wall(1, 1), 1, 0)],
        // A lighthouse's (Roadmap 7 P8): its lamp put down by `builder` (the lamp room); the keeper's
        // watch room below it; wicks and spare glass on the store floors; the oil in casks.
        "lamp room" | "lamp gallery" => &[],
        "watch room" => &[("desk", Wall(2, 1), 1, 0), ("chair", Wall(1, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "wick store" => &[("shelf", Wall(2, 1), 1, 0), ("crate", Wall(1, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "oil store" => &[("barrel", Wall(1, 1), 3, 0), ("rack", Wall(2, 1), 1, 0)],
        "keeper's room" => &[("cot", Wall(1, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("table", Center(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "entry hall" | "foyer" => &[("rug", Center(2, 2), 1, 0), ("statue", Wall(1, 1), 1, 0)],
        "nave" => &[("pew", Center(3, 1), 2, 6), ("pillar", Wall(1, 1), 2, 0)],
        "sanctuary" | "chapel" => &[("altar", Wall(2, 1), 1, 0), ("statue", Wall(1, 1), 1, 0), ("pew", Center(2, 1), 0, 10)],
        "oratory" => &[("altar", Wall(2, 1), 1, 0), ("statue", Wall(1, 1), 1, 0)],
        "vestry" => &[("wardrobe", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("table", Center(1, 1), 1, 0)],
        "crypt" | "ossuary" => &[("sarcophagus", Center(1, 2), 1, 8), ("statue", Wall(1, 1), 1, 0)],
        "tomb chamber" => &[("sarcophagus", Center(1, 2), 1, 0)],
        "bell loft" => &[("bell", Center(2, 2), 1, 0)],
        "monk's cell" | "cell" | "holding cell" => &[("cot", Wall(1, 2), 1, 0), ("bucket", Wall(1, 1), 1, 0)],
        "great hall" | "mess hall" => &[("long_table", Center(4, 1), 1, 18), ("hearth", Wall(2, 1), 1, 0), ("throne", Wall(1, 1), 1, 0), ("pillar", Wall(1, 1), 0, 16)],
        "dining room" => &[("long_table", Center(3, 1), 1, 0), ("sideboard", Wall(2, 1), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "parlor" => &[("couch", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "gallery" => &[("statue", Wall(1, 1), 2, 8), ("bench", Center(2, 1), 1, 0)],
        "guardroom" | "armory" => &[("weapon_rack", Wall(2, 1), 1, 6), ("table", Center(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "barrack room" | "dormitory" => &[("cot", Wall(1, 2), 2, 5), ("chest", Wall(1, 1), 1, 8)],
        "dungeon" | "holding pen" | "beast pen" => &[("cage", Center(2, 2), 1, 12), ("chest", Wall(1, 1), 0, 0), ("barrel", Wall(1, 1), 1, 0)],
        "vault" => &[("chest", Wall(1, 1), 3, 4), ("strongbox", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "counting house" => &[("counter", Wall(3, 1), 1, 0), ("counting_desk", Center(2, 1), 1, 12), ("strongbox", Wall(1, 1), 1, 0), ("bench", Wall(2, 1), 0, 16)],
        "courtroom" => &[("counter", Wall(3, 1), 1, 0), ("desk", Center(2, 1), 1, 12), ("bench", Center(3, 1), 0, 12)],
        "baths" => &[("bath", Center(2, 2), 1, 12), ("bench", Wall(2, 1), 1, 0)],
        "infirmary" => &[("cot", Wall(1, 2), 2, 6), ("table", Center(2, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "arena floor" => &[("pillar", Center(1, 1), 2, 40), ("barricade", Center(2, 1), 2, 30), ("trapdoor", Center(1, 1), 1, 30), ("cage", Center(1, 1), 1, 0)],
        "stands" => &[("bench", Wall(3, 1), 2, 6)],
        "tower room" => &[("weapon_rack", Wall(1, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        // A tower mill's floors (`program`): flour sacks and meal bins; the millstones; the grain
        // bins and the sack hoist's trap; the great spur wheel and the hoist under the cap.
        "meal floor" => &[("sacks", Wall(1, 1), 2, 10), ("barrel", Wall(1, 1), 2, 0), ("workbench", Wall(2, 1), 1, 0)],
        "stone floor" => &[("millstones", Center(2, 2), 1, 0), ("sacks", Wall(1, 1), 2, 0), ("barrel", Wall(1, 1), 1, 0)],
        "bin floor" => &[("hoist_trapdoor", Center(1, 1), 1, 0), ("crate", Wall(1, 1), 3, 0), ("sacks", Wall(1, 1), 1, 0)],
        "dust floor" => &[("spur_wheel", Center(2, 2), 1, 0), ("sack_hoist", Wall(1, 1), 1, 0), ("sacks", Wall(1, 1), 1, 0)],
        "winch room" => &[("winch", Center(1, 1), 1, 0), ("weapon_rack", Wall(1, 1), 1, 0)],
        "wall walk" => &[],
        "hall" | "corridor" => &[],
        "screens passage" | "mural passage" | "landing" | "gate passage" | "open to below" | "roof" => &[],
        // (A roof terrace furnished on its own, round its domes: `furnish_terrace`.)
        TERRACE => &[],
        "entrance hall" => &[("bench", Wall(2, 1), 1, 0)],
        "minstrels' gallery" => &[("bench", Wall(2, 1), 1, 0)],
        "keep yard" => &[("barrel", Wall(1, 1), 2, 0), ("crate", Wall(1, 1), 1, 0)],
        "great chamber" => &[("bed", Wall(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("wardrobe", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        "garrison" => &[("cot", Wall(1, 2), 2, 5), ("weapon_rack", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 8)],
        "solar" => &[("couch", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("table", Center(1, 1), 1, 0)],
        "map room" => &[("long_table", Center(3, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "treasury" => &[("chest", Wall(1, 1), 3, 4), ("strongbox", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "guest chamber" => &[("bed", Wall(1, 2), 1, 0), ("wardrobe", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 1), 1, 0)],
        "servants' quarters" | "officers' quarters" => &[("cot", Wall(1, 2), 2, 6), ("chest", Wall(1, 1), 1, 8)],
        "pantry" | "buttery" => &[("shelf", Wall(2, 1), 1, 6), ("barrel", Wall(1, 1), 2, 0), ("crate", Wall(1, 1), 1, 0)],
        "wardrobe" => &[("wardrobe", Wall(1, 1), 2, 0), ("chest", Wall(1, 1), 1, 0)],
        "guard post" => &[("weapon_rack", Wall(1, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "tower top" => &[("crate", Wall(1, 1), 1, 0)],
        "torture chamber" => &[("rack", Center(2, 1), 1, 0), ("cage", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "prison block" => &[("cage", Wall(2, 2), 2, 3), ("cot", Wall(1, 2), 1, 2), ("bucket", Wall(1, 1), 1, 2)],
        "well room" => &[("bucket", Wall(1, 1), 2, 2), ("barrel", Wall(1, 1), 1, 2)],
        "warden's office" => &[("desk", Center(2, 1), 1, 0), ("bookcase", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        // A palace's and an estate's (its throne room, banqueting hall and ballroom furnished as
        // wholes: `hall`; these where that fails).
        "throne room" => &[("throne", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("pillar", Wall(1, 1), 0, 16)],
        "banqueting hall" => &[("long_table", Center(4, 1), 1, 18), ("hearth", Wall(2, 1), 1, 0), ("sideboard", Wall(2, 1), 1, 0)],
        "ballroom" => &[("bench", Wall(2, 1), 1, 12), ("statue", Wall(1, 1), 1, 0)],
        "audience chamber" => &[("chair_of_office", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("tapestry", Wall(2, 1), 1, 12), ("bench", Wall(2, 1), 1, 12)],
        "long gallery" => &[("statue", Wall(1, 1), 1, 8), ("tapestry", Wall(2, 1), 0, 8), ("bench", Wall(2, 1), 0, 10)],
        "royal chapel" => &[("altar", Wall(2, 1), 1, 0), ("statue", Wall(1, 1), 1, 0), ("pew", Center(3, 1), 1, 8), ("font", Center(1, 1), 1, 0)],
        // A temple's and a monastery's (their naves, sanctuaries, side chapels, shrines and refectories
        // furnished as wholes: `hall`; these where that fails).
        "side chapel" => &[("side_altar", Wall(2, 1), 1, 0), ("candle_stand", Wall(1, 1), 1, 0)],
        "shrine" => &[("altar", Wall(2, 1), 1, 0), ("god_statue", Wall(1, 1), 1, 0), ("candle_stand", Wall(1, 1), 1, 0)],
        "refectory" => &[("long_table", Center(4, 1), 1, 18), ("bench", Wall(2, 1), 1, 8), ("hearth", Wall(2, 1), 1, 0)],
        "chapter house" => &[("bench", Wall(2, 1), 2, 6), ("chair_of_office", Wall(1, 1), 1, 0)],
        "dorter" => &[("cot", Wall(1, 2), 2, 5), ("chest", Wall(1, 1), 1, 8)],
        "scriptorium" => &[("desk", Center(2, 1), 1, 8), ("bookcase", Wall(2, 1), 1, 8), ("shelf", Wall(2, 1), 1, 0)],
        "cloister walk" | "garth" | "inn yard" | "yard gallery" => &[],
        // (A palace's court: its fountain, walks and benches laid out with it, `hub::Plan::court_pieces`.)
        "courtyard" => &[],
        "antechamber" => &[("bench", Wall(2, 1), 1, 0), ("tapestry", Wall(2, 1), 1, 0), ("chair", Wall(1, 1), 1, 0)],
        "servants' hall" => &[("long_table", Center(3, 1), 1, 0), ("bench", Wall(2, 1), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "salon" => &[("couch", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("harpsichord", Wall(2, 1), 1, 0)],
        "stair hall" => &[("bench", Wall(2, 1), 1, 0)],
        // A town hall's and a guild's (`hub::civic`): the guild's arms (`arms_look`) where it shows
        // itself; a belfry's stair and its bell loft (the bell put down with the tower: `hub::civic`).
        "arcade" => &[("crate", Wall(1, 1), 1, 14), ("barrel", Wall(1, 1), 1, 18)],
        "council chamber" => &[("council_table", Center(4, 2), 1, 0), ("long_table", Center(3, 1), 1, 0), ("chair_of_office", Wall(1, 1), 1, 0), ("guild_arms", Wall(2, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 0)],
        "strongroom" | "warded vault" => &[("strongbox", Wall(1, 1), 2, 4), ("chest", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "sample room" => &[("display", Center(2, 1), 1, 10), ("shelf", Wall(2, 1), 1, 6), ("guild_arms", Wall(1, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "masterpiece gallery" => &[("display", Center(2, 1), 1, 10), ("statue", Wall(1, 1), 1, 0), ("guild_arms", Wall(1, 1), 1, 0), ("bench", Wall(2, 1), 1, 0)],
        "job-board room" => &[("job_board", Wall(2, 1), 1, 0), ("table", Center(2, 1), 0, 12), ("bench", Wall(2, 1), 1, 8), ("guild_arms", Wall(1, 1), 1, 0)],
        "trophy hall" => &[("statue", Wall(1, 1), 2, 8), ("guild_arms", Wall(2, 1), 1, 0), ("weapon_rack", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        "belfry stair" | "belfry" => &[],
        // A stables' stalls in a row along its walls, their mangers to the wall.
        "stables" => &[("stable_stall", Wall(1, 2), 2, 4), ("barrel", Wall(1, 1), 1, 0)],
        "tack room" => &[("rack", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "bunk room" => &[("cot", Wall(1, 2), 2, 4), ("chest", Wall(1, 1), 1, 8)],
        // A big inn's (`hub::inn`): the snug a drinking room of its own, the private dining room's
        // table for a party, the scullery's tubs and buckets, the best chamber's big bed by its
        // hearth, the innkeeper's takings in a strongbox.
        "snug" => &[("table", Seated, 1, 0), ("hearth", Wall(2, 1), 1, 0), ("bench", Wall(2, 1), 1, 0)],
        "private dining room" => &[("long_table", Center(3, 1), 1, 0), ("sideboard", Wall(2, 1), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "scullery" => &[("bucket", Wall(1, 1), 2, 0), ("barrel", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "best chamber" => &[("bed", Wall(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("wardrobe", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        "innkeeper's quarters" => &[("bed", Wall(1, 2), 1, 0), ("desk", Wall(2, 1), 1, 0), ("strongbox", Wall(1, 1), 1, 0), ("wardrobe", Wall(1, 1), 1, 0)],
        _ => &[("chest", Wall(1, 1), 1, 0)],
    }
}

/// Every kind of room in a building (those with furnishing rules), for the designer.
pub const ROOM_KINDS: &[&str] = &[
    "common room", "taproom", "brewhouse", "battlements", "kitchen", "storeroom", "loft storage", "keg cellar",
    "wine cellar", "bedroom", "guest room", "owner's quarters", "chamber", "master bedroom", "lord's chamber",
    "bedchamber", "main room", "hearth room", "living room", "shop", "workshop", "forge", "warehouse floor", "office", "archive",
    "study", "meeting room", "library", "reading room", "laboratory", "observatory", "entry hall", "foyer", "nave",
    "sanctuary", "chapel", "vestry", "crypt", "ossuary", "bell loft", "monk's cell", "cell", "holding cell",
    "great hall", "mess hall", "dining room", "parlor", "gallery", "guardroom", "armory", "barrack room", "dormitory",
    "dungeon", "holding pen", "beast pen", "vault", "counting house", "courtroom", "baths", "infirmary",
    "arena floor", "stands", "tower room", "winch room", "wall walk", "hall", "corridor", "solar", "map room",
    "treasury", "guest chamber", "servants' quarters", "officers' quarters", "pantry", "buttery", "wardrobe",
    "guard post", "tower top", "torture chamber", "prison block", "well room", "warden's office",
    "screens passage", "mural passage", "landing", "entrance hall", "minstrels' gallery", "open to below",
    "roof", "roof terrace", "keep yard", "gate passage", "great chamber", "garrison", "oratory", "throne room", "ballroom",
    "audience chamber", "long gallery", "banqueting hall", "royal chapel", "courtyard", "antechamber",
    "servants' hall", "salon", "stair hall", "side chapel", "shrine", "refectory", "chapter house", "dorter",
    "scriptorium", "cloister walk", "garth", "arcade", "council chamber", "strongroom", "sample room",
    "masterpiece gallery", "job-board room", "trophy hall", "bunk room", "warded vault", "belfry stair", "belfry",
    "stables", "tack room", "snug", "private dining room", "scullery", "best chamber", "innkeeper's quarters",
    "inn yard", "yard gallery", "yurt", "meal floor", "stone floor", "bin floor", "dust floor",
    // R5 towns
    "tomb chamber", "felt hall", "felt sanctuary", "felt common room",
    // Roadmap 7 P8: a lighthouse's, its oil store's
    "lamp room", "lamp gallery", "watch room", "wick store", "oil store", "keeper's room",
];

/// A kind of room in a building, by name.
pub fn room_kind(s: &str) -> Option<&'static str> {
    ROOM_KINDS.iter().copied().find(|k| *k == s)
}

// ---------------------------------------------------------------------------------------
// Generation.
