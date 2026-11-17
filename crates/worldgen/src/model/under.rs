//! What lies underground: the kinds of site and their sizes.

use serde::{Deserialize, Serialize};

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
