//! Underground sites, generated on demand from an entrance on the surface (`Layout::entrances`):
//! a pure function of (world, layout, entrance). A site is a stack of levels on one 5-ft grid
//! anchored at the entrance and running the way the passage goes in, so it lies under the map
//! where it is (the surface shows through faintly). Same `Interior` model as buildings.
//!
//! - Dungeons, dwarven halls, wizards' lairs, temples, prisons, crypts, tombs, flooded vaults, keep
//!   dungeons, undercrofts and the bandits' cellars are grown room by room (`grow`); crypts,
//!   ossuaries and catacombs as niche-lined galleries in a grid of junctions. Goblin warrens are
//!   burrowed: twisting crawlways out to roundish dens (drawn as caverns), as is a bandit
//!   hideout's cave front. Cult sanctums are rings round a ritual chamber, their gates offset.
//! - Caves are cellular automata grown around a winding spine, kept to the part reachable from
//!   the arrival, split into chambers (some raised as ledges).
//! - Mines are a haulage way with side drifts and stopes, a shaft to the next level.
//! - Lava tubes are a meandering tube with side pockets; the lower tube carries a lava channel.
//! - Sewers run under a city's paved streets, section by section (`SEWER_SECTION_FT` squares
//!   of the world, each entered from a street grate): the tunnels are the ground under the
//!   streets, a sewage channel down the wider runs, service passages joining runs that meet
//!   only outside the section, an outfall where a tunnel reaches water; an undercroft below.
//! - Catacombs open from graveyards, mausoleums and catacomb houses: burial galleries.
//!
//! Levels are joined by a way down and a way up on the same square. Every site ends in a boss
//! chamber on its deepest level, a dead end: nothing else on the level is reached through it.
//! Guarantees are checked in tests/vital.rs.
//!
//! Layout: this file holds the kinds, themes and sizes, the entrances and `site`, which asks each theme's planner
//! (`Theme::planner`) for a level; `plan` is the level while it is laid out (`Plan`, `finish`), `kit` the tools every
//! planner shares (digging, props, furnishing), `themes/*` one planner per kind of level, `design` the hand designer.

use serde::Serialize;

pub mod design;
mod grow;
mod kit;
mod plan;
mod themes;

pub use kit::{Ground, PROPS, SPANS, Span, TERRAIN, Wetness, blocked_squares, chasm, rubble_slope, water, water_at};
use kit::*;
use plan::*;
use themes::*;

use crate::World;
use crate::core::rng::{Pcg32, hash3};
use crate::interior::{Interior, Item, Level, Link, SQUARE_FT};
use crate::t0::T0;
use crate::town::{
    self, Entrance, Layout, Structure,
    geom::{P, add, mul, sub},
};

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
    fn ways(self) -> (&'static str, &'static str) {
        match self {
            UnderKind::Dungeon | UnderKind::Crypt | UnderKind::Catacombs | UnderKind::KeepDungeon => ("stairs down", "stairs up"),
            UnderKind::Sewer => ("drain shaft: ladder down 20 ft", "drain shaft: ladder up 20 ft"),
            UnderKind::Cave => ("steep passage down", "steep passage up"),
            UnderKind::Mine => ("shaft: ladder down 20 ft", "shaft: ladder up 20 ft"),
            UnderKind::LavaTube => ("collapse: climb down 20 ft", "collapse: climb up 20 ft"),
        }
    }
}

/// Room kind of the final chamber.
pub const BOSS: &str = "boss chamber";
/// Depth between levels (ft).
const LEVEL_FT: f32 = 20.0;
/// Side (ft) of a sewer section: a city's sewers are entered section by section, each a square
/// of the world (aligned to multiples of this), so neighbouring sections line up.
pub const SEWER_SECTION_FT: f64 = 480.0;
/// How far (ft) the sewer reaches either side of a paved street's line.
pub const SEWER_REACH_FT: f64 = 6.0;

//// A site's program: built levels (graph-first: dungeons, crypts) take their rooms, passages
/// and corridor width from it; natural levels (caves) their chamber kinds and ledges.
pub struct Theme {
    /// Id (`Created::theme`).
    pub key: &'static str,
    /// The kind of site it is for.
    pub kind: UnderKind,
    /// The room the level is entered by (top level).
    first: &'static str,
    kinds: &'static [&'static str],
    passage: &'static str,
    /// Room count: at least, and up to this many more (a medium site).
    rooms: (usize, u32),
    /// Fill a big grid: more and larger rooms (a keep's sprawling dungeons, dwarven halls).
    sprawl: bool,
    /// Corridors this many squares wide.
    corridor: usize,
    /// Natural levels: a raised chamber.
    ledge: &'static str,
    /// What lays out its levels.
    planner: Planner,
    /// How much water stands in it (laid by its planner: `Plan::water`).
    water: Wetness,
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
    Cave,
    Mine,
    LavaTube,
}

#[allow(clippy::too_many_arguments)]
const fn built(key: &'static str, kind: UnderKind, first: &'static str, kinds: &'static [&'static str], passage: &'static str, rooms: (usize, u32), sprawl: bool, corridor: usize) -> Theme {
    Theme { key, kind, first, kinds, passage, rooms, sprawl, corridor, ledge: "", planner: Planner::Mixed, water: Wetness::Dry }
}

const fn natural(key: &'static str, kind: UnderKind, kinds: &'static [&'static str], ledge: &'static str) -> Theme {
    let planner = match kind {
        UnderKind::Mine => Planner::Mine,
        UnderKind::LavaTube => Planner::LavaTube,
        _ => Planner::Cave,
    };
    Theme { key, kind, first: "", kinds, passage: "", rooms: (0, 1), sprawl: false, corridor: 1, ledge, planner, water: Wetness::Dry }
}

/// Every theme a site can be given; each kind's first is the one it had before themes.
pub const THEMES: &[Theme] = &[
    Theme { planner: Planner::Mixed, ..built("dungeon", UnderKind::Dungeon, "entry hall", &["guard room", "barracks", "storeroom", "cell block", "torture chamber", "shrine", "armory", "well room", "pit room", "prison"], "corridor", (8, 5), false, 1) },
    Theme {
        planner: Planner::Prison,
        ..built("prison", UnderKind::Dungeon, "gatehouse", &["cell block", "cell block", "prison", "oubliette", "torture chamber", "guard room", "warden's office", "mess hall", "interrogation room"], "cell corridor", (10, 5), false, 1)
    },
    Theme { planner: Planner::Temple, ..built("temple", UnderKind::Dungeon, "narthex", &["sanctum", "shrine", "vestry", "priests' cells", "ritual pool", "reliquary", "ossuary", "chapel of rest"], "processional way", (7, 4), false, 2) },
    Theme { planner: Planner::Wizard, ..built("wizard_lair", UnderKind::Dungeon, "foyer", &["library", "laboratory", "summoning circle", "specimen vault", "study", "menagerie", "golem workshop", "scrying room"], "hall", (7, 4), false, 1) },
    Theme {
        planner: Planner::Bandit,
        ..built("bandit_hideout", UnderKind::Dungeon, "hidden entry", &["common room", "bunk room", "loot store", "armory", "kitchen", "captain's quarters", "storeroom", "lookout"], "tunnel", (9, 4), false, 1)
    },
    Theme { planner: Planner::Dwarven, ..built("dwarven_hall", UnderKind::Dungeon, "gate hall", &["great hall", "forge", "brewery", "armory", "barracks", "treasury", "ancestor hall", "storeroom", "mushroom farm"], "gallery", (9, 5), true, 2) },
    Theme {
        planner: Planner::Warren,
        ..built("goblin_warren", UnderKind::Dungeon, "warren mouth", &["den", "den", "nest", "cook pit", "refuse pit", "wolf pen", "shaman's hut", "storeroom"], "crawlway", (12, 6), false, 1)
    },
    Theme {
        planner: Planner::Vault,
        water: Wetness::Flooded,
        ..built("flooded_vault", UnderKind::Dungeon, "flooded stair", &["flooded hall", "cistern", "old vault", "sunken chapel", "treasury", "drowned barracks"], "flooded passage", (8, 4), false, 1)
    },
    Theme {
        planner: Planner::Cult,
        ..built("cult_sanctum", UnderKind::Dungeon, "cellar", &["ritual chamber", "dormitory", "robing room", "sacrifice pit", "cell", "shrine", "storeroom"], "ring passage", (10, 4), false, 1)
    },
    Theme { planner: Planner::Crypt, ..built("crypt", UnderKind::Crypt, "antechamber", &["burial hall", "ossuary", "tomb", "chapel of rest", "embalming room", "reliquary", "sealed tomb", "catacomb gallery"], "passage", (7, 4), false, 1) },
    Theme { planner: Planner::Tomb, ..built("tomb", UnderKind::Crypt, "antechamber", &["tomb", "sealed tomb", "false tomb", "treasury", "guardian hall", "burial hall", "embalming room"], "passage", (5, 3), false, 2) },
    Theme { planner: Planner::Crypt, ..built("ossuary", UnderKind::Crypt, "stair foot", &["ossuary", "ossuary", "bone chapel", "charnel pit", "skull gallery", "burial niches"], "catacomb passage", (8, 4), false, 1) },
    Theme { planner: Planner::Crypt, ..built("catacombs", UnderKind::Catacombs, "stair foot", &["burial niches", "burial niches", "ossuary", "catacomb gallery", "charnel pit", "bone chapel", "reliquary", "flooded gallery"], "catacomb passage", (9, 5), false, 1) },
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
        ..built(SUNKEN_TEMPLE, UnderKind::Dungeon, "narthex", &["sanctum", "shrine", "vestry", "priests' cells", "ritual pool", "reliquary", "sunken chapel", "chapel of rest"], "processional way", (7, 4), false, 2)
    },
];

/// A temple sunk in wet ground: flooded halls.
pub const SUNKEN_TEMPLE: &str = "sunken_temple";

/// Under a large keep: prisons, stores, old crypts and a way out.
const KEEP_DUNGEON: Theme = Theme { planner: Planner::Keep, ..built(
    "keep",
    UnderKind::KeepDungeon,
    "landing",
    &["cell block", "prison", "oubliette", "torture chamber", "guard room", "armory", "storeroom", "barracks", "well room", "crypt of the old lords", "secret vault", "shrine"],
    "corridor",
    (11, 6),
    true,
    1,
) };
/// Below a city's sewers.
const UNDERCROFT: Theme = Theme {
    planner: Planner::Undercroft,
    ..built("undercroft", UnderKind::Sewer, "drain chamber", &["cistern", "overflow chamber", "smugglers' cache", "old vault", "rat warren", "flooded hall"], "culvert", (9, 5), false, 1)
};

/// A theme by key (`Created::theme`).
pub fn theme(key: &str) -> Option<usize> {
    THEMES.iter().position(|t| t.key == key)
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
    fn scale(self) -> f64 {
        [0.75, 1.0, 1.3, 1.6][self as usize]
    }
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
fn ranges(kind: UnderKind) -> ((usize, usize), (SiteSize, SiteSize), (u8, u8)) {
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

/// Choose what `spec` leaves open (size, levels, theme) for a site of `kind`: a generated
/// site keeps its kind's usual theme half the time.
fn resolve(kind: UnderKind, spec: SiteSpec, rng: &mut Pcg32) -> (SiteSize, usize, &'static Theme) {
    let (_, (s0, s1), (l0, l1)) = ranges(kind);
    let size = spec.size.unwrap_or_else(|| SiteSize::ALL[s0 as usize + rng.below((s1 as usize - s0 as usize + 1) as u32) as usize]);
    let levels = spec.levels.map(|l| l.clamp(1, MAX_LEVELS)).unwrap_or_else(|| l0 + rng.below((l1 - l0 + 1) as u32) as u8) as usize;
    let own: Vec<&'static Theme> = THEMES.iter().filter(|t| t.kind == kind && t.key != SUNKEN_TEMPLE).collect();
    let theme = match spec.theme.and_then(|i| THEMES.get(i as usize)).filter(|t| t.kind == kind) {
        Some(t) => t,
        None if own.len() > 1 && rng.next_f64() >= 0.5 => own[1 + rng.below(own.len() as u32 - 1) as usize],
        None => own.first().copied().unwrap_or(&THEMES[0]),
    };
    (size, levels, theme)
}

/// A `u:` site's size, read back from its grid (None for sewer sections and keep dungeons).
pub fn site_size(it: &Interior) -> Option<SiteSize> {
    let kind = UnderKind::CREATABLE.into_iter().find(|k| k.name() == it.function).filter(|_| it.id.starts_with("u:"))?;
    let s = it.nx as f64 / ranges(kind).0.0 as f64;
    SiteSize::ALL.into_iter().min_by(|a, b| (a.scale() - s).abs().total_cmp(&(b.scale() - s).abs()))
}

/// A city's paved street lines (world ft): segment ends and half the paving's width.
pub fn street_segments(l: &Layout) -> Vec<(P, P, f64)> {
    l.roads.iter().filter(|r| r.1 >= 3).flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * *w))).collect()
}

/// Water stands over the ground here.
fn wet(t0: &T0, p: P) -> bool {
    t0.sample_water(p[0], p[1]) as f64 > t0.sample(p[0], p[1], 5.0)
}

/// Wet ground: a swamp, standing water (a lake, the sea) within about 80 ft, or a river within 120 ft of its bank.
pub fn wet_ground(t0: &T0, p: P) -> bool {
    const R: f64 = 80.0;
    const D: f64 = R * std::f64::consts::FRAC_1_SQRT_2;
    let ring = [[0.0, 0.0], [R, 0.0], [-R, 0.0], [0.0, R], [0.0, -R], [D, D], [D, -D], [-D, D], [-D, -D]];
    if ring.iter().any(|o| wet(t0, add(p, *o))) {
        return true;
    }
    let (x, y) = crate::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 120.0, t0.cell_ft);
    if (x - p[0]).abs() + (y - p[1]).abs() > 0.5 {
        return true;
    }
    t0.sample_biome(p[0], p[1], t0.biome_warp(p[0], p[1]))[0] == crate::t0::biome::Biome::Swamp as u8
}

/// Corner (world ft) of the sewer section holding a point.
pub fn sewer_section(p: P) -> P {
    [crate::core::floor(p[0] / SEWER_SECTION_FT) * SEWER_SECTION_FT, crate::core::floor(p[1] / SEWER_SECTION_FT) * SEWER_SECTION_FT]
}

/// Ways underground in a city: a sewer grate on a street in every section the paved streets
/// run far enough into (never under a tower or in a gate's passage, nor by a palace's
/// forecourt), catacomb stairs in each graveyard and beside each mausoleum and catacomb house.
pub fn city_entrances(l: &Layout, t0: &T0, seed: u64) -> Vec<Entrance> {
    let mut out = Vec::new();
    let segs = street_segments(l);
    // Clear of the towers and the gatehouses between them (a gate's passage reaches 26 ft from
    // each of its towers, 11 ft either side of the wall).
    let towers = crate::interior::towers(l);
    // (Nor in a palace's forecourt or its gate: 10 ft clear of it.)
    let near_yard = |q: P| l.forecourts.iter().any(|f| f.yard.len() == 4 && (town::geom::contains(&f.yard, q) || (0..4).any(|k| town::geom::seg_dist(q, f.yard[k], f.yard[(k + 1) % 4]) < 10.0)));
    let clear = |q: P| towers.iter().all(|&(t, r, gate)| town::geom::dist(q, t) >= r + if gate { 22.0 } else { 5.0 }) && !near_yard(q);
    if !segs.is_empty() {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (a, b, _) in &segs {
            x0 = x0.min(a[0]).min(b[0]);
            y0 = y0.min(a[1]).min(b[1]);
            x1 = x1.max(a[0]).max(b[0]);
            y1 = y1.max(a[1]).max(b[1]);
        }
        let s = SEWER_SECTION_FT;
        let (i0, j0, i1, j1) = ((x0 / s).floor() as i64, (y0 / s).floor() as i64, (x1 / s).floor() as i64, (y1 / s).floor() as i64);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let (ox, oy) = (i as f64 * s, j as f64 * s);
                let c = [ox + 0.5 * s, oy + 0.5 * s];
                let inside = |p: P| p[0] >= ox + 20.0 && p[0] < ox + s - 20.0 && p[1] >= oy + 20.0 && p[1] < oy + s - 20.0;
                let mut total = 0.0;
                // Candidate grates: on each street, the point nearest the middle and the
                // street's middle.
                let mut cands: Vec<(f64, P)> = Vec::new();
                for (a, b, _) in &segs {
                    let m = town::geom::lerp(*a, *b, 0.5);
                    if !inside(m) {
                        continue;
                    }
                    total += town::geom::dist(*a, *b);
                    let ab = sub(*b, *a);
                    let t = (town::geom::dot(sub(c, *a), ab) / town::geom::dot(ab, ab).max(1e-9)).clamp(0.0, 1.0);
                    for q in [add(*a, mul(ab, t)), m] {
                        if inside(q) && !wet(t0, q) && clear(q) {
                            cands.push((town::geom::dist(q, c), q));
                        }
                    }
                }
                if total < 200.0 {
                    continue;
                }
                // The grate nearest the middle first, then up to three more, 140 ft apart.
                cands.sort_by(|x, y| x.0.total_cmp(&y.0));
                let mut chosen: Vec<P> = Vec::new();
                for (_, q) in cands {
                    if chosen.len() < 4 && chosen.iter().all(|o| town::geom::dist(*o, q) >= 140.0) {
                        chosen.push(q);
                    }
                }
                for at in chosen {
                    out.push(Entrance { at, dir: [1.0, 0.0], kind: UnderKind::Sewer, id: 0 });
                }
            }
        }
    }
    let mut rng = Pcg32::new(seed ^ 0xca7a, 17);
    for b in &l.buildings {
        let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
        let c = town::geom::centroid(&b.poly);
        if b.structure == Structure::Open && town::geom::contains(&b.poly, c) {
            // A mausoleum over the stair among the graves, square to the headstone rows (the
            // battlemap grid): 4 squares deep, 3 wide, its door at `at` (a square's centre).
            // It runs along the yard's long side, whichever way fits.
            let sq = crate::battlemap::SQUARE_FT;
            let at = [crate::core::floor(c[0] / sq) * sq + 0.5 * sq, crate::core::floor(c[1] / sq) * sq + 0.5 * sq];
            let (x0, y0, x1, y1) = b.poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
            let flip = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
            let mut dirs = if x1 - x0 >= y1 - y0 { [[flip, 0.0], [0.0, flip]] } else { [[0.0, flip], [flip, 0.0]] }.to_vec();
            dirs.extend(dirs.clone().iter().map(|d| mul(*d, -1.0)));
            let fits = |d: P| {
                let side = [-d[1], d[0]];
                [(-1.5, -1.5), (2.5, -1.5), (2.5, 1.5), (-1.5, 1.5)].iter().all(|&(a, s)| town::geom::contains(&b.poly, add(at, add(mul(d, a * sq), mul(side, s * sq)))))
            };
            let dir = dirs.iter().copied().find(|d| fits(*d)).unwrap_or(dirs[0]);
            out.push(Entrance { at, dir, kind: UnderKind::Catacombs, id: 0 });
        } else if matches!(key, Some("catacombs" | "mausoleum")) {
            // A stair beside the house, running in under it.
            let m = b.poly.len();
            for e in 0..m {
                let (p, q) = (b.poly[e], b.poly[(e + 1) % m]);
                let d = sub(q, p);
                let len = town::geom::len(d);
                if len < 1e-6 {
                    continue;
                }
                let mut n = [d[1] / len, -d[0] / len];
                let mid = town::geom::lerp(p, q, 0.5);
                if town::geom::contains(&b.poly, add(mid, mul(n, 1.0))) {
                    n = mul(n, -1.0);
                }
                let at = add(mid, mul(n, 9.0));
                let near = |o: &town::Building| o.poly.iter().any(|v| town::geom::dist(*v, at) < 80.0);
                if !wet(t0, at) && !l.buildings.iter().any(|o| near(o) && town::geom::contains(&o.poly, at)) {
                    out.push(Entrance { at, dir: mul(n, -1.0), kind: UnderKind::Catacombs, id: 0 });
                    break;
                }
            }
        }
    }
    out
}

/// The site behind entrance `k` of layout `layout` (id `u:<layout>:<k>`); a sewer grate opens
/// onto its section (`sewer_site`).
pub fn generate(world: &World, t0: &T0, layout: usize, k: usize) -> Option<Interior> {
    if layout >= town::layout_count(t0) {
        return None;
    }
    let l = town::layout(world, t0, layout);
    let e = *l.entrance(k)?;
    if e.kind == UnderKind::Sewer {
        let o = sewer_section(e.at);
        return sewer_site(world, t0, layout, (o[0] / SEWER_SECTION_FT) as i64, (o[1] / SEWER_SECTION_FT) as i64);
    }
    let mut rng = Pcg32::new(hash3(world.stream("under"), town::rng_key(t0, layout), k as i64, e.kind as i64), 91);
    // A created site's own choices (size, levels, theme); the rest from the rng.
    let spec = layout.checked_sub(t0.settlements.len()).and_then(|p| t0.created_site(p)).map(|c| c.spec).unwrap_or_default();
    let (size, depth, theme) = resolve(e.kind, spec, &mut rng);
    // A temple on wet ground lies sunken, and about 1 in 4 of the rest (unless its maker chose the theme).
    // (A hash of the site, not the rng: the draws after it stay as they were.)
    let share = crate::core::rng::unit(hash3(world.stream("under"), town::rng_key(t0, layout), k as i64, 0x5e4c)) < 0.25;
    let theme = if spec.theme.is_none() && theme.key == "temple" && (share || wet_ground(t0, e.at)) { THEMES.iter().find(|t| t.key == SUNKEN_TEMPLE).unwrap_or(theme) } else { theme };
    let ((bx, by), _, _) = ranges(e.kind);
    let s = size.scale();
    let (nx, ny) = ((bx as f64 * s).round() as usize, (by as f64 * s).round() as usize);
    // The entrance square sits near one end; the site runs away from it along `dir`.
    let entry = (2usize, ny / 2);
    let axis = e.dir;
    let across = [-axis[1], axis[0]];
    let origin = sub(e.at, add(mul(axis, (entry.0 as f64 + 0.5) * SQUARE_FT), mul(across, (entry.1 as f64 + 0.5) * SQUARE_FT)));
    let frame = Frame { origin, axis, across, nx, ny };
    let surface = t0.sample(e.at[0], e.at[1], 20.0) as f32;
    site(t0, &l, e.kind, format!("u:{layout}:{k}"), (layout as u32, k as u32), frame, depth, Some(entry.1 * nx + entry.0), surface, (theme, s * s), &mut rng)
}

/// The sewers under section (`sx`, `sy`) of the world (`SEWER_SECTION_FT` squares), id
/// `w:<layout>:<sx>:<sy>`: entered from its street grate if it has one, else from a
/// neighbouring section. None where no paved street of the layout runs.
pub fn sewer_site(world: &World, t0: &T0, layout: usize, sx: i64, sy: i64) -> Option<Interior> {
    if layout >= town::layout_count(t0) {
        return None;
    }
    let l = town::layout(world, t0, layout);
    let s = SEWER_SECTION_FT;
    let origin = [sx as f64 * s, sy as f64 * s];
    let n = (s / SQUARE_FT) as usize;
    let grates: Vec<usize> = l
        .entrances
        .iter()
        .filter(|e| e.kind == UnderKind::Sewer && sewer_section(e.at) == origin)
        .map(|e| {
            let (i, j) = (((e.at[0] - origin[0]) / SQUARE_FT) as usize, ((e.at[1] - origin[1]) / SQUARE_FT) as usize);
            j.min(n - 1) * n + i.min(n - 1)
        })
        .collect();
    let grate = grates.first().copied();
    let mut rng = Pcg32::new(hash3(world.stream("under.sewer"), town::rng_key(t0, layout), sx, sy), 91);
    let surface = t0.sample(origin[0] + 0.5 * s, origin[1] + 0.5 * s, 20.0) as f32;
    let frame = Frame { origin, axis: [1.0, 0.0], across: [0.0, 1.0], nx: n, ny: n };
    site(t0, &l, UnderKind::Sewer, format!("w:{layout}:{sx}:{sy}"), (layout as u32, 0), frame, 2, grate, surface, (&UNDERCROFT, 1.0), &mut rng)
}

/// Kinds of house whose cellar has a way into the sewers (when it stands on a street).
const SEWER_CELLARS: &[&str] = &["tavern", "inn", "alehouse", "smugglers_den", "thieves_guild", "black_market", "gambling_den", "warehouse", "temple", "bank", "pawnbroker", "alchemist", "castle", "palace"];

/// The sewer section id holding a world point.
pub fn sewer_id(layout: usize, q: P) -> String {
    let o = sewer_section(q);
    format!("w:{layout}:{}:{}", (o[0] / SEWER_SECTION_FT) as i64, (o[1] / SEWER_SECTION_FT) as i64)
}

/// The nearest point on a paved street to `c` within `reach` ft, over dry ground.
fn street_point(t0: &T0, l: &Layout, c: P, reach: f64) -> Option<P> {
    let mut best: Option<(f64, P)> = None;
    for r in l.roads.iter().filter(|r| r.1 >= 3) {
        for s in r.0.windows(2) {
            let (a, b) = (s[0], s[1]);
            if c[0] < a[0].min(b[0]) - reach || c[0] > a[0].max(b[0]) + reach || c[1] < a[1].min(b[1]) - reach || c[1] > a[1].max(b[1]) + reach {
                continue;
            }
            let ab = sub(b, a);
            let t = (crate::town::geom::dot(sub(c, a), ab) / crate::town::geom::dot(ab, ab).max(1e-9)).clamp(0.0, 1.0);
            let q = add(a, mul(ab, t));
            let d = crate::town::geom::dist(q, c);
            if d <= reach && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, q));
            }
        }
    }
    best.map(|(_, q)| q).filter(|&q| !wet(t0, q))
}

/// Where building `b`'s cellar meets the sewers (a point on the street outside it), for
/// taverns, warehouses, temples, smugglers' dens and the like and one house in ten, in cities.
pub fn sewer_link_of(t0: &T0, l: &Layout, b: &town::Building) -> Option<P> {
    if l.site || l.tier < crate::t0::settle::Tier::City {
        return None;
    }
    if b.structure != Structure::Roofed {
        return None;
    }
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
    let chosen = key.is_some_and(|k| SEWER_CELLARS.contains(&k)) || (b.func.is_none() && crate::core::rng::hash2(town::rng_key(t0, l.index as usize) as u64 ^ 0x5e3e, b.id as i64, 7) % 10 == 0);
    if !chosen {
        return None;
    }
    let c = town::geom::centroid(&b.poly);
    let half = b.poly.iter().map(|p| town::geom::dist(*p, c)).fold(0.0, f64::max);
    street_point(t0, l, c, half + 20.0)
}

/// Where a large keep's deep dungeons break out into the sewers: the street nearest it.
pub fn keep_escape(t0: &T0, l: &Layout, b: &town::Building) -> Option<P> {
    if l.site || l.tier < crate::t0::settle::Tier::City {
        return None;
    }
    if !crate::interior::has_deep_dungeon(b) {
        return None;
    }
    street_point(t0, l, town::geom::centroid(&b.poly), 400.0)
}

/// A large keep's deep dungeons (id `k:<layout>:<building>`): under the keep, entered by the
/// stairs down from its cellar, with an escape tunnel to the city's sewers.
pub fn keep_dungeon(world: &World, t0: &T0, layout: usize, bi: usize) -> Option<Interior> {
    if layout >= town::layout_count(t0) {
        return None;
    }
    let l = town::layout(world, t0, layout);
    if !crate::interior::has_deep_dungeon(l.building(bi)?) {
        return None;
    }
    let keep = crate::interior::generate(world, t0, layout, bi)?;
    let id = format!("k:{layout}:{bi}");
    // (On its deepest cellar: the first level, or a designed keep's deeper one.)
    let (cellar, link) = keep.levels.iter().find_map(|lv| Some((lv, lv.links.iter().find(|k| k.to == id)?)))?;
    let m = 14;
    let (nx, ny) = (keep.nx + 2 * m, keep.ny + 2 * m);
    let origin = sub(keep.origin, add(mul(keep.axis, m as f64 * SQUARE_FT), mul(keep.across, m as f64 * SQUARE_FT)));
    let entry = (link.y as usize + m) * nx + link.x as usize + m;
    let frame = Frame { origin, axis: keep.axis, across: keep.across, nx, ny };
    let mut rng = Pcg32::new(hash3(world.stream("under.keep"), town::rng_key(t0, layout), bi as i64, 0), 93);
    let depth = if nx * ny > 6000 { 3 } else { 2 };
    let surface = cellar.elevation_ft;
    site(t0, &l, UnderKind::KeepDungeon, id, (layout as u32, bi as u32), frame, depth, Some(entry), surface, (&KEEP_DUNGEON, 1.0), &mut rng)
}

/// Where a site's grid lies: corner (world ft), unit axes, size in squares.
#[derive(Clone, Copy)]
struct Frame {
    origin: P,
    axis: P,
    across: P,
    nx: usize,
    ny: usize,
}

/// Lay out a site's levels from the way in (`entry`; a sewer section without a grate picks
/// its own), furnish them, and assemble the `Interior`. `theme` with `area`, how much more
/// ground than a medium site it covers (room counts grow with it).
#[allow(clippy::too_many_arguments)]
fn site(
    t0: &T0,
    l: &Layout,
    kind: UnderKind,
    id: String,
    (settlement, building): (u32, u32),
    f: Frame,
    depth: usize,
    entry: Option<usize>,
    surface: f32,
    (theme, area): (&'static Theme, f64),
    rng: &mut Pcg32,
) -> Option<Interior> {
    let (nx, ny) = (f.nx, f.ny);
    let (down_name, up_name) = kind.ways();
    let mut plans: Vec<Plan> = Vec::with_capacity(depth);
    let mut arrive = entry;
    // (Built levels' locks: started on the first, so the levels before it draw as they did.)
    let mut locks: Option<(usize, Locks)> = None;
    for d in 0..depth {
        let last = d + 1 == depth;
        let a = arrive.unwrap_or(0);
        let mut p = match theme.planner {
            // (A city's own sewers, not from a theme: its top level follows the streets.)
            _ if kind == UnderKind::Sewer && d == 0 => sewer(t0, l, f.origin, nx, ny, arrive, last)?,
            Planner::Mixed => mixed(nx, ny, a, d, last, theme, area, rng),
            Planner::Dwarven => dwarven(nx, ny, a, d, last, theme, area, rng),
            Planner::Wizard => wizard(nx, ny, a, d, last, theme, area, rng),
            Planner::Warren => warren(nx, ny, a, d, last, theme, area, rng),
            Planner::Bandit => bandit(nx, ny, a, d, last, theme, area, rng),
            Planner::Cult => cult(nx, ny, a, d, last, theme, area, rng),
            Planner::Temple => temple(nx, ny, a, d, last, theme, area, rng),
            Planner::Prison => prison(nx, ny, a, d, last, theme, area, rng),
            // (A keep's dungeons are big when three levels deep.)
            Planner::Keep => keep(nx, ny, a, d, last, theme, if depth >= 3 { 1.69 } else { 1.0 }, rng),
            Planner::Undercroft => undercroft(nx, ny, a, last, theme, rng),
            Planner::Crypt => crypt(nx, ny, a, d, last, theme, area, rng),
            Planner::Tomb => tomb(nx, ny, a, d, last, theme, area, rng),
            Planner::Vault => vault(nx, ny, a, d, last, theme, area, rng),
            Planner::Cave => cave(nx, ny, a, last, theme, rng),
            Planner::Mine => mine(nx, ny, a, last, rng),
            Planner::LavaTube => lava_tube(nx, ny, a, d, last, theme, rng),
        };
        // The ways in: the surface on the top level (if it opens there), the way up below.
        let here = p.up;
        if d == 0 && kind == UnderKind::KeepDungeon {
            // Up into the keep's cellar; and an old escape tunnel out to the sewers.
            p.put(Item::new("up", "stairs up to the keep", here % nx, here / nx, 1, 1, 0, false, 0.0, None));
            p.links.push(Link { x: (here % nx) as u16, y: (here / nx) as u16, to: format!("b:{settlement}:{building}") });
            if let Some(q) = l.building(building as usize).and_then(|b| keep_escape(t0, l, b)) {
                let g = sub(q, f.origin);
                let at = [crate::town::geom::dot(g, f.axis) / SQUARE_FT, crate::town::geom::dot(g, f.across) / SQUARE_FT];
                let free = (0..nx * ny).filter(|&k| p.cells[k] >= 0 && !p.taken[k]).min_by(|&a, &b| {
                    let dd = |k: usize| ((k % nx) as f64 + 0.5 - at[0]).powi(2) + ((k / nx) as f64 + 0.5 - at[1]).powi(2);
                    dd(a).total_cmp(&dd(b))
                });
                if let Some(k) = free {
                    p.put(Item::new("tunnel", "escape tunnel to the sewers", k % nx, k / nx, 1, 1, 0, false, 0.0, Some("hidden: DC 15 Perception to find")));
                    p.links.push(Link { x: (k % nx) as u16, y: (k / nx) as u16, to: sewer_id(settlement as usize, q) });
                }
            }
        } else if d > 0 {
            p.put(Item::new("up", up_name, here % nx, here / nx, 1, 1, 0, false, 0.0, None));
        } else if entry.is_some() {
            p.put(Item::new("exit", kind.entrance_name(), here % nx, here / nx, 1, 1, 0, false, 0.0, None));
        }
        if let Some(dn) = p.down {
            p.put(Item::new("down", down_name, dn % nx, dn / nx, 1, 1, 0, false, 0.0, None));
        }
        if p.passage.is_some() {
            // (A keep's dungeons are big when three levels deep.)
            let size = if kind == UnderKind::KeepDungeon { if depth >= 3 { 1.69 } else { 1.0 } } else { area };
            let (first, lk) = locks.get_or_insert_with(|| (d, Locks::new(size, rng)));
            guard(&mut p, &mut plans[*first..], nx, ny, d, depth, lk, rng);
        }
        if !p.water.is_empty() {
            // (Its planner's water, once the locks and keys are in: off doorways, every square still reached.)
            p.reserve_doors();
            let (w, seed) = (std::mem::take(&mut p.water), hash3(rng.next_u32() as u64, d as i64, 0, 0x3a7e));
            let span = if rng.next_f64() < 0.5 { Span::Bridge } else { Span::Stones };
            let items = water_at(&Ground::of_plan(&p), seed, span, &|k| (w[k][0] as f64, w[k][1] as f64));
            p.put_all(items);
        }
        if let Some((r, rope)) = p.chasm {
            // (A chasm across a room, once the locks and keys are in: crossed by rope bridges or its stone bridge.)
            p.reserve_doors();
            let seed = hash3(rng.next_u32() as u64, d as i64, 0, 0xc4a5);
            let items = chasm_in(&Ground::of_plan(&p), Some(r), Span::Bridge, seed, rope);
            p.put_all(items);
        }
        furnish(&mut p, nx, ny, kind, rng);
        arrive = p.down.or(arrive);
        plans.push(p);
    }
    let levels: Vec<Level> = plans
        .into_iter()
        .enumerate()
        .rev()
        .map(|(d, p)| {
            let name = if d + 1 == depth { format!("Level {} · the deep", d + 1) } else { format!("Level {} · {} ft down", d + 1, (d + 1) * LEVEL_FT as usize) };
            p.finish(nx, ny, -(d as i8) - 1, name, surface - LEVEL_FT * (d + 1) as f32)
        })
        .collect();
    let entry_level = levels.len() - 1;
    Some(Interior {
        id,
        settlement,
        building,
        name: None,
        function: kind.name(),
        theme: (!matches!(kind, UnderKind::Sewer | UnderKind::KeepDungeon)).then_some(theme.key),
        origin: f.origin,
        axis: f.axis,
        across: f.across,
        nx,
        ny,
        levels,
        entry_level,
        stairs: [0; 4],
        sprites: Vec::new(),
        outlines: Vec::new(),
    })
}
