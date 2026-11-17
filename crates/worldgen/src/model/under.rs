//! What lies underground: the kinds of site and their sizes.

use serde::{Deserialize, Serialize};

use crate::World;
use crate::core::rng::{Pcg32, hash2, hash3, unit};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnderKind {
    Dungeon,
    Crypt,
    Cave,
    Mine,
    LavaTube,
    Sewer,
    Catacombs,
    /// Below a large keep's cellar.
    KeepDungeon,
}

impl UnderKind {
    pub fn name(self) -> &'static str {
        match self {
            UnderKind::Dungeon => "dungeon",
            UnderKind::Crypt => "crypt",
            UnderKind::Cave => "cave",
            UnderKind::Mine => "mine",
            UnderKind::LavaTube => "lava tube",
            UnderKind::Sewer => "sewer",
            UnderKind::Catacombs => "catacombs",
            UnderKind::KeepDungeon => "deep dungeons",
        }
    }

    /// Kinds a created site can open onto (`Created::under`), by key.
    pub const CREATABLE: [UnderKind; 6] = [UnderKind::Dungeon, UnderKind::Crypt, UnderKind::Catacombs, UnderKind::Cave, UnderKind::Mine, UnderKind::LavaTube];

    /// Its key (`Created::under`): the name, snake_case.
    pub fn key(self) -> &'static str {
        match self {
            UnderKind::LavaTube => "lava_tube",
            UnderKind::KeepDungeon => "keep_dungeon",
            k => k.name(),
        }
    }

    pub fn parse(key: &str) -> Option<UnderKind> {
        Self::CREATABLE.into_iter().find(|k| k.key() == key)
    }

    /// What the opening looks like on the surface.
    pub fn entrance_name(self) -> &'static str {
        match self {
            UnderKind::Dungeon => "Dungeon stairs",
            UnderKind::Crypt => "Crypt stairs",
            UnderKind::Cave => "Cave mouth",
            UnderKind::Mine => "Mine adit",
            UnderKind::LavaTube => "Lava tube skylight",
            UnderKind::Sewer => "Sewer grate",
            UnderKind::Catacombs => "Catacomb stairs",
            UnderKind::KeepDungeon => "Stairs to the deep dungeons",
        }
    }

    /// The way between levels (down, up).
    pub(crate) fn ways(self) -> (&'static str, &'static str) {
        match self {
            UnderKind::Dungeon | UnderKind::Crypt | UnderKind::Catacombs | UnderKind::KeepDungeon => ("stairs down", "stairs up"),
            UnderKind::Sewer => ("drain shaft: ladder down 20 ft", "drain shaft: ladder up 20 ft"),
            UnderKind::Cave => ("steep passage down", "steep passage up"),
            UnderKind::Mine => ("shaft: ladder down 20 ft", "shaft: ladder up 20 ft"),
            UnderKind::LavaTube => ("collapse: climb down 20 ft", "collapse: climb up 20 ft"),
        }
    }
}

/// How big a site is: its grid (and room count) grows with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteSize {
    Small,
    Medium,
    Large,
    Huge,
}

impl SiteSize {
    pub const ALL: [SiteSize; 4] = [SiteSize::Small, SiteSize::Medium, SiteSize::Large, SiteSize::Huge];
    pub const NAMES: [&str; 4] = ["small", "medium", "large", "huge"];

    pub fn parse(s: &str) -> Option<SiteSize> {
        Self::NAMES.iter().position(|n| *n == s).map(|i| Self::ALL[i])
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// Grid scale (each side).
    pub(crate) fn scale(self) -> f64 {
        [0.75, 1.0, 1.3, 1.6][self as usize]
    }
}

/// A designed site.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SiteDesign {
    /// What it is (`UnderKind::key`): dungeon, crypt, catacombs, cave, mine, lava_tube.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// A cave's look from its mouth's biome (`Interior::look`), kept when the site is copied.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub look: u8,
    /// World ft of grid corner (0, 0) and the grid's x axis (as `Interior`).
    pub origin: [f64; 2],
    pub axis: [f64; 2],
    pub nx: u16,
    pub ny: u16,
    /// The way in on the top level: the square under the entrance on the surface.
    pub entry: [u16; 2],
    /// Levels bottom to top (as `Interior::levels`, so `l:<site>:<level>` ids hold).
    pub levels: Vec<DesignLevel>,
    /// A building's stair block (x, y, w, h: the same squares on every level it reaches).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stairs: Option<[u16; 4]>,
    /// A building's: what the design was made for (`interior::design::fingerprint`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fingerprint: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DesignLevel {
    pub name: String,
    pub elevation_ft: f32,
    /// Natural rock (caves, mines, lava tubes): rooms open into each other, no doors needed.
    pub natural: bool,
    /// Room per square (row-major), run-length encoded: room (-1 rock), count, room, count…
    pub cells: Vec<i32>,
    /// Rooms keep their index (`r:<site>:<level>:<room>` names hold); one with no squares is
    /// simply not there.
    pub rooms: Vec<DesignRoom>,
    /// Doors: x, y, side (0: between the square and the one east of it, 1: south; a building's
    /// also 2 west, 3 north), flags (1 secret; a building's door to the outside: 2 front, 4 back, 8 arch;
    /// a locked door's colour, 1-31: its low three bits shifted 4 up, the rest shifted 8 up (`lock_flags`); 128 a portcullis,
    /// between two rooms).
    pub doors: Vec<[u16; 4]>,
    /// Everything standing on the floor, the ways in, up and down too (kinds `exit`, `up`,
    /// `down`), in order.
    pub items: Vec<DesignItem>,
    /// A building's: the storey (-1 cellar, 0 ground floor, as `Level::z`), an open roof, the
    /// stair block reaching it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub z: i8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub roof: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_stairs: bool,
    /// Where the stairs come out on this level (a building's: the square one steps off its stair block onto, else its squares; a site's:
    /// the way up, else under the way down above): both must be floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<[u16; 2]>,
}

pub(crate) fn is_zero(z: &i8) -> bool {
    *z == 0
}

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

pub(crate) fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DesignRoom {
    pub kind: String,
    /// Raised floor (a ledge), ft.
    pub raise_ft: u16,
    /// Drawn round, octagonal, with an apse, chamfered or as a cavern (`interior::shape`; 0
    /// square): built so while its squares fit the shape.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub shape: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DesignItem {
    pub kind: String,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// The side its back is against, 0 north 1 east 2 south 3 west; absent: found from the walls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<u8>,
    /// One of its looks, 0-3 (more for some: `interior::item_looks`); absent: picked by position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<u8>,
}

//// A site's program: built levels (graph-first: dungeons, crypts) take their rooms, passages
/// and corridor width from it; natural levels (caves) their chamber kinds and ledges.
pub struct Theme {
    /// Id (`Created::theme`).
    pub key: &'static str,
    /// The kind of site it is for.
    pub kind: UnderKind,
    /// The room the level is entered by (top level).
    pub(crate) first: &'static str,
    pub(crate) kinds: &'static [&'static str],
    pub(crate) passage: &'static str,
    /// Room count: at least, and up to this many more (a medium site).
    pub(crate) rooms: (usize, u32),
    /// Corridors this many squares wide.
    pub(crate) corridor: usize,
    /// Natural levels: a raised chamber.
    pub(crate) ledge: &'static str,
    /// What lays out its levels.
    pub(crate) planner: Planner,
    /// How much water stands in it (laid by its planner: `Plan::water`).
    pub(crate) water: Wetness,
}

/// Which planner (`themes`) lays out a theme's levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Planner {
    /// Grown room by room with mixed kits, a mirrored core on the top level: guard rooms, cells, stores (`themes::mixed`).
    Mixed,
    /// Axial halls: gate hall, a pillared gallery with side wings, the great hall; a chasm one level in two
    /// (`themes::dwarven`).
    Dwarven,
    /// A round stair hall at each level's heart, mirrored spokes to round rooms; the summoning circle deepest
    /// (`themes::wizard`).
    Wizard,
    /// Burrows: twisting crawlways out to roundish dens, drawn as caverns (`themes::warren`).
    Warren,
    /// A cave front (entry, common room, lookout) with dug rooms grown behind it, the captain's locked
    /// (`themes::bandit`).
    Bandit,
    /// Ring passages round a round ritual chamber, rooms between them, the gates offset (`themes::cult`).
    Cult,
    /// A maze of 1-square passages on a lattice, dead-end side rooms, the stairs down (or the boss) in the heart room
    /// (`themes::labyrinth`).
    Labyrinth,
    /// A fort's halls (broken gatehouse, pillared hall, barracks, round tower cellars) with caverns breaking through,
    /// more cavern the deeper (`themes::fortress`).
    Fortress,
    /// Grown room by room, the top level mirrored: narthex, nave, chapels, apse sanctum (`themes::temple`).
    Temple,
    /// Grown round a guard ring: cell blocks off it, the offices; solitary cells and oubliettes below (`themes::prison`).
    Prison,
    /// Grown from the landing with prison and store kits: a cell wing, stores, the old lords' crypt (`themes::keep`).
    Keep,
    /// Grown from the drain chamber with store kits: cisterns (some flooded), caches, a rat warren (`themes::keep`).
    Undercroft,
    /// Niche-lined galleries in a grid of junctions, bigger rooms on and off it (`themes::crypt`).
    Crypt,
    /// A barrow's mirrored run: passage, antechamber, guardian hall, side chambers, the burial chamber (`themes::crypt`).
    Tomb,
    /// Great flooded halls and cisterns, vaults among flooded passages (`themes::vault`).
    Vault,
    /// Chambers of uneven size grown together, a square of rock between them, joined by squeezes and short tunnels;
    /// raised ledges, water by theme, islands, a chasm now and then (`themes::cave`).
    Cave,
    Mine,
    LavaTube,
}

#[allow(clippy::too_many_arguments)]
const fn built(key: &'static str, kind: UnderKind, first: &'static str, kinds: &'static [&'static str], passage: &'static str, rooms: (usize, u32), corridor: usize) -> Theme {
    Theme { key, kind, first, kinds, passage, rooms, corridor, ledge: "", planner: Planner::Mixed, water: Wetness::Dry }
}

const fn natural(key: &'static str, kind: UnderKind, kinds: &'static [&'static str], ledge: &'static str) -> Theme {
    let planner = match kind {
        UnderKind::Mine => Planner::Mine,
        UnderKind::LavaTube => Planner::LavaTube,
        _ => Planner::Cave,
    };
    Theme { key, kind, first: "", kinds, passage: "", rooms: (0, 1), corridor: 1, ledge, planner, water: Wetness::Dry }
}

/// Every theme a site can be given; each kind's first is the one it had before themes.
pub const THEMES: &[Theme] = &[
    Theme { planner: Planner::Mixed, ..built("dungeon", UnderKind::Dungeon, "entry hall", &["guard room", "barracks", "storeroom", "cell block", "torture chamber", "shrine", "armory", "well room", "pit room", "prison"], "corridor", (8, 5), 1) },
    Theme {
        planner: Planner::Prison,
        ..built("prison", UnderKind::Dungeon, "gatehouse", &["cell block", "cell block", "prison", "oubliette", "torture chamber", "guard room", "warden's office", "mess hall", "interrogation room"], "cell corridor", (10, 5), 1)
    },
    Theme { planner: Planner::Temple, ..built("temple", UnderKind::Dungeon, "narthex", &["sanctum", "shrine", "vestry", "priests' cells", "ritual pool", "reliquary", "ossuary", "chapel of rest"], "processional way", (7, 4), 2) },
    Theme { planner: Planner::Wizard, ..built("wizard_lair", UnderKind::Dungeon, "foyer", &["library", "laboratory", "summoning circle", "specimen vault", "study", "menagerie", "golem workshop", "scrying room"], "hall", (7, 4), 1) },
    Theme {
        planner: Planner::Bandit,
        ..built("bandit_hideout", UnderKind::Dungeon, "hidden entry", &["common room", "bunk room", "loot store", "armory", "kitchen", "captain's quarters", "storeroom", "lookout"], "tunnel", (9, 4), 1)
    },
    Theme { planner: Planner::Dwarven, ..built("dwarven_hall", UnderKind::Dungeon, "gate hall", &["great hall", "forge", "brewery", "armory", "barracks", "treasury", "ancestor hall", "storeroom", "mushroom farm"], "gallery", (9, 5), 2) },
    Theme {
        planner: Planner::Warren,
        ..built("goblin_warren", UnderKind::Dungeon, "warren mouth", &["den", "den", "nest", "cook pit", "refuse pit", "wolf pen", "shaman's hut", "storeroom"], "crawlway", (12, 6), 1)
    },
    Theme {
        planner: Planner::Vault,
        water: Wetness::Flooded,
        ..built("flooded_vault", UnderKind::Dungeon, "flooded stair", &["flooded hall", "cistern", "old vault", "sunken chapel", "treasury", "drowned barracks"], "flooded passage", (8, 4), 1)
    },
    Theme {
        planner: Planner::Cult,
        ..built("cult_sanctum", UnderKind::Dungeon, "cellar", &["ritual chamber", "dormitory", "robing room", "sacrifice pit", "cell", "shrine", "storeroom"], RING, (10, 4), 1)
    },
    Theme { planner: Planner::Labyrinth, ..built("labyrinth", UnderKind::Dungeon, "maze gate", &["bone pit", "treasure nook", "trapped end", "heart of the maze"], "maze passage", (6, 3), 1) },
    // (Drawn for a generated site only under a ruin, `resolve`.)
    Theme {
        planner: Planner::Fortress,
        ..built(FORTRESS, UnderKind::Dungeon, "broken gatehouse", &["pillared hall", "barracks", "armory", "storeroom", "guard room", "mess hall", "tower cellar", "shrine"], "corridor", (9, 4), 1)
    },
    Theme { planner: Planner::Crypt, ..built("crypt", UnderKind::Crypt, "antechamber", &["burial hall", "ossuary", "tomb", "chapel of rest", "embalming room", "reliquary", "sealed tomb", "catacomb gallery"], "passage", (7, 4), 1) },
    Theme { planner: Planner::Tomb, ..built("tomb", UnderKind::Crypt, "antechamber", &["tomb", "sealed tomb", "false tomb", "treasury", "guardian hall", "burial hall", "embalming room"], "passage", (5, 3), 2) },
    Theme { planner: Planner::Crypt, ..built("ossuary", UnderKind::Crypt, "stair foot", &["ossuary", "ossuary", "bone chapel", "charnel pit", "skull gallery", "burial niches"], "catacomb passage", (8, 4), 1) },
    Theme { planner: Planner::Crypt, ..built("catacombs", UnderKind::Catacombs, "stair foot", &["burial niches", "burial niches", "ossuary", "catacomb gallery", "charnel pit", "bone chapel", "reliquary", "flooded gallery"], "catacomb passage", (9, 5), 1) },
    natural("cave", UnderKind::Cave, &["cavern", "grotto", "crystal grotto", "fungus grotto", "pool chamber", "bat roost"], "ledge"),
    natural("fungal", UnderKind::Cave, &["fungus forest", "spore grotto", "fungus grotto", "mycelium hall"], "ledge"),
    natural("crystal", UnderKind::Cave, &["crystal cavern", "geode", "crystal grotto", "cavern"], "crystal shelf"),
    natural("ice", UnderKind::Cave, &["ice cavern", "frozen pool", "icicle gallery"], "ice shelf"),
    natural("flooded", UnderKind::Cave, &["flooded cavern", "sump", "pool chamber"], "ledge"),
    natural("beast_den", UnderKind::Cave, &["lair", "nest chamber", "gnawing chamber", "bat roost", "cavern"], "ledge"),
    natural("mine", UnderKind::Mine, &[], ""),
    natural("lava_tube", UnderKind::LavaTube, &["lava tube", "tube gallery", "side pocket", "obsidian gallery"], "basalt shelf"),
    // (Never drawn for a generated site: a temple on wet ground becomes it, and about 1 in 4 of the rest, `generate`.)
    Theme {
        planner: Planner::Temple,
        water: Wetness::Flooded,
        ..built(SUNKEN_TEMPLE, UnderKind::Dungeon, "narthex", &["sanctum", "shrine", "vestry", "priests' cells", "ritual pool", "reliquary", "sunken chapel", "chapel of rest"], "processional way", (7, 4), 2)
    },
];

/// A temple sunk in wet ground: flooded halls.
pub const SUNKEN_TEMPLE: &str = "sunken_temple";
/// A fort's halls with caves eating in: drawn for generated sites under ruins only.
pub const FORTRESS: &str = "collapsed_fortress";

/// How old the place over a site is, for its cave-ins (`themes::breach`): under a ruin (or a lone way down in the
/// wilds) about half the levels get one; under a living town, keep or city about one site in ten; a site made by hand
/// never.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Age {
    Ruin,
    Living,
    Made,
}

/// Under a large keep: prisons, stores, old crypts and a way out.
pub(crate) const KEEP_DUNGEON: Theme = Theme { planner: Planner::Keep, ..built(
    "keep",
    UnderKind::KeepDungeon,
    "landing",
    &["cell block", "prison", "oubliette", "torture chamber", "guard room", "armory", "storeroom", "barracks", "well room", "crypt of the old lords", "secret vault", "shrine"],
    "corridor",
    (11, 6),
    1,
) };
/// Below a city's sewers.
pub(crate) const UNDERCROFT: Theme = Theme {
    planner: Planner::Undercroft,
    ..built("undercroft", UnderKind::Sewer, "drain chamber", &["cistern", "overflow chamber", "smugglers' cache", "old vault", "rat warren", "flooded hall"], "culvert", (9, 5), 1)
};

/// A theme by key (`Created::theme`).
pub fn theme(key: &str) -> Option<usize> {
    THEMES.iter().position(|t| t.key == key)
}

/// Most levels a site has.
pub const MAX_LEVELS: u8 = 6;

/// What a created site asks for underground (anything left out is chosen as for a generated
/// one): its size, its levels (1 to `MAX_LEVELS`), its theme (index into `THEMES`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SiteSpec {
    pub size: Option<SiteSize>,
    pub levels: Option<u8>,
    pub theme: Option<u8>,
}

/// A generated site's grid (squares, medium size), and the sizes and levels it is chosen from.
pub(crate) fn ranges(kind: UnderKind) -> ((usize, usize), (SiteSize, SiteSize), (u8, u8)) {
    use SiteSize::*;
    match kind {
        UnderKind::Dungeon => ((44, 36), (Small, Large), (2, 4)),
        UnderKind::Crypt => ((36, 30), (Small, Medium), (1, 3)),
        UnderKind::Cave => ((60, 48), (Small, Large), (1, 3)),
        UnderKind::Mine => ((60, 40), (Medium, Large), (2, 4)),
        UnderKind::LavaTube => ((84, 30), (Small, Large), (2, 3)),
        UnderKind::Catacombs | UnderKind::Sewer | UnderKind::KeepDungeon => ((48, 40), (Medium, Large), (2, 3)),
    }
}

/// How cold the ground over a site is, for the themes that need it (ice): `Glacier` (the Ice biome: a cave there
/// is always ice), `Snowy` (Alpine, Tundra, and the high part of Taiga: ice keeps its usual odds), else `No`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Cold {
    #[default]
    No,
    Snowy,
    Glacier,
}

/// Taiga this far above the sea (ft) counts as snowy: about the highest third of its land (see `namecheck --ice`).
pub const TAIGA_SNOW_FT: f64 = 4750.0;

/// The ground's coldness from a T0 cell's biome (the packed `T0::biome` value) and height (ft, as stored in
/// `T0::height`, an `f32`). The one rule every caller shares (`T0::cold_at`; the namer, with its own grids).
pub fn cold_of(biome: u32, height_ft: f32, sea_ft: f64) -> Cold {
    use crate::t0::biome::Biome;
    match Biome::from_u8((biome & 0xff) as u8) {
        Biome::Ice => Cold::Glacier,
        Biome::Alpine | Biome::Tundra => Cold::Snowy,
        Biome::Taiga if height_ft as f64 - sea_ft > TAIGA_SNOW_FT => Cold::Snowy,
        _ => Cold::No,
    }
}

/// Choose what `spec` leaves open (size, levels, theme) for a site of `kind`: a generated
/// site keeps its kind's usual theme half the time. Ice only over cold ground (`cold`) unless `spec` asks for it:
/// off it, a drawn ice is drawn again among the others; in the Ice biome a cave is always ice.
pub fn resolve(kind: UnderKind, spec: SiteSpec, ruin: bool, cold: Cold, rng: &mut Pcg32) -> (SiteSize, usize, &'static Theme) {
    let (_, (s0, s1), (l0, l1)) = ranges(kind);
    let size = spec.size.unwrap_or_else(|| SiteSize::ALL[s0 as usize + rng.below((s1 as usize - s0 as usize + 1) as u32) as usize]);
    let levels = spec.levels.map(|l| l.clamp(1, MAX_LEVELS)).unwrap_or_else(|| l0 + rng.below((l1 - l0 + 1) as u32) as u8) as usize;
    let own: Vec<&'static Theme> = THEMES.iter().filter(|t| t.kind == kind && t.key != SUNKEN_TEMPLE && (ruin || t.key != FORTRESS)).collect();
    let theme = match spec.theme.and_then(|i| THEMES.get(i as usize)).filter(|t| t.kind == kind) {
        Some(t) => t,
        None if own.len() > 1 && rng.next_f64() >= 0.5 => own[1 + rng.below(own.len() as u32 - 1) as usize],
        None => own.first().copied().unwrap_or(&THEMES[0]),
    };
    let theme = if spec.theme.is_some() || (theme.key != "ice" && cold != Cold::Glacier) {
        theme
    } else if cold == Cold::No {
        let others: Vec<&'static Theme> = own.iter().copied().filter(|t| t.key != "ice").collect();
        others[rng.below(others.len() as u32) as usize]
    } else {
        own.iter().copied().find(|t| t.key == "ice").unwrap_or(theme)
    };
    (size, levels, theme)
}

/// The theme a generated wild site's way down (`kind`, its only entrance) comes to, as `generate`
/// draws it, from the site's seed alone (`town::rng_key` of a world's own site): so the site can
/// be named for it before anything is laid out. `ruin`: no roof stands at the site. (A temple
/// sunk on wet ground is still its temple here.)
pub fn wild_theme(world: &World, poi_seed: u64, kind: UnderKind, ruin: bool, cold: Cold) -> &'static str {
    let key = crate::core::rng::hash2(0x7369_7465_706f_6900, 0, poi_seed as i64) as i64;
    let mut rng = Pcg32::new(hash3(world.stream("under"), key, 0, kind as i64), 91);
    resolve(kind, SiteSpec::default(), ruin, cold, &mut rng).2.key
}

/// How much water a theme has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wetness {
    Dry,
    Wet,
    Flooded,
}

impl Wetness {
    /// The noise heights over which a square is shallow and deep water (`water_at`; over 1: never).
    pub fn bands(self) -> (f64, f64) {
        match self {
            Wetness::Dry => (2.0, 2.0),
            Wetness::Wet => (0.62, 0.72),
            Wetness::Flooded => (0.42, 0.56),
        }
    }
}

/// The cult's passages (the theme's passage kind): the rings, their gates, what is dug or grown off them. Kept clear
/// (no kit) and floored apart from the rooms between, so each ring and its gate stands out.
pub(crate) const RING: &str = "ring corridor";

/// What a generated ruin's way down comes to: a dungeon under the hold or its crypt, from the
/// site's seed alone (v63: placement asks it before the site is laid out).
pub fn ruin_under(world: &World, seed: u64) -> UnderKind {
    if unit(hash2(world.stream("site.under"), 0, seed as i64)) < 0.5 { UnderKind::Dungeon } else { UnderKind::Crypt }
}
