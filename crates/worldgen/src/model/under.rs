//! What lies underground: the kinds of site and their sizes.

use serde::Serialize;

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
