//! Underground sites, generated on demand from an entrance on the surface (`Layout::entrances`):
//! a pure function of (world, layout, entrance). A site is a stack of levels on one 5-ft grid
//! anchored at the entrance and running the way the passage goes in, so it lies under the map
//! where it is (the surface shows through faintly). Same `Interior` model as buildings.
//!
//! - Dungeons and crypts are graph-first: rooms placed on the level, a spanning tree plus a few
//!   loops between them, corridors dug by least-cost paths that never cut through other rooms,
//!   doors where a corridor meets a room.
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

use serde::Serialize;

pub mod design;

use crate::World;
use crate::core::hash::FastSet;
use crate::core::rng::{Pcg32, hash3};
use crate::interior::{Door, Interior, Item, Level, Link, Room, SQUARE_FT, Wall};
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
}

#[allow(clippy::too_many_arguments)]
const fn built(key: &'static str, kind: UnderKind, first: &'static str, kinds: &'static [&'static str], passage: &'static str, rooms: (usize, u32), sprawl: bool, corridor: usize) -> Theme {
    Theme { key, kind, first, kinds, passage, rooms, sprawl, corridor, ledge: "" }
}

const fn natural(key: &'static str, kind: UnderKind, kinds: &'static [&'static str], ledge: &'static str) -> Theme {
    Theme { key, kind, first: "", kinds, passage: "", rooms: (0, 1), sprawl: false, corridor: 1, ledge }
}

/// Every theme a site can be given; each kind's first is the one it had before themes.
pub const THEMES: &[Theme] = &[
    built("dungeon", UnderKind::Dungeon, "entry hall", &["guard room", "barracks", "storeroom", "cell block", "torture chamber", "shrine", "armory", "well room", "pit room", "prison"], "corridor", (8, 5), false, 1),
    built("prison", UnderKind::Dungeon, "gatehouse", &["cell block", "cell block", "prison", "oubliette", "torture chamber", "guard room", "warden's office", "mess hall", "interrogation room"], "cell corridor", (10, 5), false, 1),
    built("temple", UnderKind::Dungeon, "narthex", &["sanctum", "shrine", "vestry", "priests' cells", "ritual pool", "reliquary", "ossuary", "chapel of rest"], "processional way", (7, 4), false, 2),
    built("wizard_lair", UnderKind::Dungeon, "foyer", &["library", "laboratory", "summoning circle", "specimen vault", "study", "menagerie", "golem workshop", "scrying room"], "hall", (7, 4), false, 1),
    built("bandit_hideout", UnderKind::Dungeon, "hidden entry", &["common room", "bunk room", "loot store", "armory", "kitchen", "captain's quarters", "storeroom", "lookout"], "tunnel", (7, 3), false, 1),
    built("dwarven_hall", UnderKind::Dungeon, "gate hall", &["great hall", "forge", "brewery", "armory", "barracks", "treasury", "ancestor hall", "storeroom", "mushroom farm"], "gallery", (9, 5), true, 2),
    built("goblin_warren", UnderKind::Dungeon, "warren mouth", &["den", "den", "nest", "cook pit", "refuse pit", "wolf pen", "shaman's hut", "storeroom"], "crawlway", (12, 6), false, 1),
    built("flooded_vault", UnderKind::Dungeon, "flooded stair", &["flooded hall", "cistern", "old vault", "sunken chapel", "treasury", "drowned barracks"], "flooded passage", (8, 4), false, 1),
    built("crypt", UnderKind::Crypt, "antechamber", &["burial hall", "ossuary", "tomb", "chapel of rest", "embalming room", "reliquary", "sealed tomb", "catacomb gallery"], "passage", (7, 4), false, 1),
    built("tomb", UnderKind::Crypt, "antechamber", &["tomb", "sealed tomb", "false tomb", "treasury", "guardian hall", "burial hall", "embalming room"], "passage", (5, 3), false, 2),
    built("ossuary", UnderKind::Crypt, "stair foot", &["ossuary", "ossuary", "bone chapel", "charnel pit", "skull gallery", "burial niches"], "catacomb passage", (8, 4), false, 1),
    built("catacombs", UnderKind::Catacombs, "stair foot", &["burial niches", "burial niches", "ossuary", "catacomb gallery", "charnel pit", "bone chapel", "reliquary", "flooded gallery"], "catacomb passage", (9, 5), false, 1),
    natural("cave", UnderKind::Cave, &["cavern", "grotto", "crystal grotto", "fungus grotto", "pool chamber", "bat roost"], "ledge"),
    natural("fungal", UnderKind::Cave, &["fungus forest", "spore grotto", "fungus grotto", "mycelium hall"], "ledge"),
    natural("crystal", UnderKind::Cave, &["crystal cavern", "geode", "crystal grotto", "cavern"], "crystal shelf"),
    natural("ice", UnderKind::Cave, &["ice cavern", "frozen pool", "icicle gallery"], "ice shelf"),
    natural("flooded", UnderKind::Cave, &["flooded cavern", "sump", "pool chamber"], "ledge"),
    natural("beast_den", UnderKind::Cave, &["lair", "nest chamber", "gnawing chamber", "bat roost", "cavern"], "ledge"),
    natural("mine", UnderKind::Mine, &[], ""),
    natural("lava_tube", UnderKind::LavaTube, &["lava tube", "tube gallery", "side pocket", "obsidian gallery"], "basalt shelf"),
];

/// Under a large keep: prisons, stores, old crypts and a way out.
const KEEP_DUNGEON: Theme = built(
    "keep",
    UnderKind::KeepDungeon,
    "landing",
    &["cell block", "prison", "oubliette", "torture chamber", "guard room", "armory", "storeroom", "barracks", "well room", "crypt of the old lords", "secret vault", "shrine"],
    "corridor",
    (11, 6),
    true,
    1,
);
/// Below a city's sewers.
const UNDERCROFT: Theme = built("undercroft", UnderKind::Sewer, "drain chamber", &["cistern", "overflow chamber", "smugglers' cache", "old vault", "rat warren", "flooded hall"], "culvert", (9, 5), false, 1);

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
    let own: Vec<&'static Theme> = THEMES.iter().filter(|t| t.kind == kind).collect();
    let theme = match spec.theme.and_then(|i| THEMES.get(i as usize)).filter(|t| t.kind == kind) {
        Some(t) => t,
        None if own.len() > 1 && rng.next_f64() >= 0.5 => own[1 + rng.below(own.len() as u32 - 1) as usize],
        None => own.first().copied().unwrap_or(&THEMES[0]),
    };
    (size, levels, theme)
}

/// A city's paved street lines (world ft): segment ends and half the paving's width.
pub fn street_segments(l: &Layout) -> Vec<(P, P, f64)> {
    l.roads.iter().filter(|r| r.1 >= 3).flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * *w))).collect()
}

/// Water stands over the ground here.
fn wet(t0: &T0, p: P) -> bool {
    t0.sample_water(p[0], p[1]) as f64 > t0.sample(p[0], p[1], 5.0)
}

/// Corner (world ft) of the sewer section holding a point.
pub fn sewer_section(p: P) -> P {
    [crate::core::floor(p[0] / SEWER_SECTION_FT) * SEWER_SECTION_FT, crate::core::floor(p[1] / SEWER_SECTION_FT) * SEWER_SECTION_FT]
}

/// Ways underground in a city: a sewer grate on a street in every section the paved streets
/// run far enough into, catacomb stairs in each graveyard and beside each mausoleum and
/// catacomb house.
pub fn city_entrances(l: &Layout, t0: &T0, seed: u64) -> Vec<Entrance> {
    let mut out = Vec::new();
    let segs = street_segments(l);
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
                        if inside(q) && !wet(t0, q) {
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
    let mut rng = Pcg32::new(hash3(world.stream("under"), layout as i64, k as i64, e.kind as i64), 91);
    // A created site's own choices (size, levels, theme); the rest from the rng.
    let spec = layout.checked_sub(t0.settlements.len()).and_then(|p| t0.created_site(p)).map(|c| c.spec).unwrap_or_default();
    let (size, depth, theme) = resolve(e.kind, spec, &mut rng);
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
    let mut rng = Pcg32::new(hash3(world.stream("under.sewer"), layout as i64, sx, sy), 91);
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
    let chosen = key.is_some_and(|k| SEWER_CELLARS.contains(&k)) || (b.func.is_none() && crate::core::rng::hash2(l.index as u64 ^ 0x5e3e, b.id as i64, 7) % 10 == 0);
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
    let link = keep.levels[0].links.iter().find(|k| k.to == id)?;
    let m = 14;
    let (nx, ny) = (keep.nx + 2 * m, keep.ny + 2 * m);
    let origin = sub(keep.origin, add(mul(keep.axis, m as f64 * SQUARE_FT), mul(keep.across, m as f64 * SQUARE_FT)));
    let entry = (link.y as usize + m) * nx + link.x as usize + m;
    let frame = Frame { origin, axis: keep.axis, across: keep.across, nx, ny };
    let mut rng = Pcg32::new(hash3(world.stream("under.keep"), layout as i64, bi as i64, 0), 93);
    let depth = if nx * ny > 6000 { 3 } else { 2 };
    let surface = keep.levels[0].elevation_ft;
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
    for d in 0..depth {
        let last = d + 1 == depth;
        let a = arrive.unwrap_or(0);
        let mut p = match kind {
            UnderKind::Sewer if d == 0 => sewer(t0, l, f.origin, nx, ny, arrive, last)?,
            UnderKind::Dungeon | UnderKind::Crypt | UnderKind::Catacombs | UnderKind::Sewer | UnderKind::KeepDungeon => dungeon(nx, ny, a, d, last, theme, area, rng),
            UnderKind::Cave => cave(nx, ny, a, last, theme, rng),
            UnderKind::Mine => mine(nx, ny, a, last, rng),
            UnderKind::LavaTube => lava_tube(nx, ny, a, d, last, theme, rng),
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
    })
}

// ---------------------------------------------------------------------------------------
// A level while it is laid out.

struct RoomDef {
    kind: &'static str,
    raise: f32,
}

struct Plan {
    nx: usize,
    /// Room per square, -1 rock.
    cells: Vec<i16>,
    rooms: Vec<RoomDef>,
    /// Doors between neighbouring squares (smaller index first).
    doors: FastSet<(usize, usize)>,
    natural: bool,
    /// Arrival square (from the surface or the level above) and the way down, if any.
    up: usize,
    down: Option<usize>,
    items: Vec<Item>,
    /// Squares with an item on them, and squares an item may not take (doorways, ways in
    /// and out).
    taken: Vec<bool>,
    blocked: Vec<bool>,
    /// Lines the floor follows (`Level::paths`).
    paths: Vec<[f32; 5]>,
    /// Ways to other sites (`Level::links`).
    links: Vec<Link>,
    /// Doors hidden in the wall (square pairs).
    secret: FastSet<(usize, usize)>,
}

impl Plan {
    fn new(nx: usize, ny: usize, up: usize, natural: bool) -> Plan {
        Plan {
            nx,
            cells: vec![-1; nx * ny],
            rooms: Vec::new(),
            doors: FastSet::default(),
            natural,
            up,
            down: None,
            items: Vec::new(),
            taken: vec![false; nx * ny],
            blocked: vec![false; nx * ny],
            paths: Vec::new(),
            links: Vec::new(),
            secret: FastSet::default(),
        }
    }

    fn room(&mut self, kind: &'static str, raise: f32) -> i16 {
        self.rooms.push(RoomDef { kind, raise });
        (self.rooms.len() - 1) as i16
    }

    fn door(&mut self, a: usize, b: usize) {
        self.doors.insert((a.min(b), a.max(b)));
    }

    /// Whether one can step between neighbouring squares a and b.
    fn passable(&self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.cells[a], self.cells[b]);
        ra >= 0 && rb >= 0 && (ra == rb || self.natural || self.doors.contains(&(a.min(b), a.max(b))))
    }

    /// Step distances from `start` (-1 unreachable), around `blocked` squares.
    fn distances(&self, start: usize, ny: usize, blocked: Option<&[bool]>) -> Vec<i32> {
        let nx = self.nx;
        let mut dist = vec![-1; nx * ny];
        let mut q = std::collections::VecDeque::new();
        dist[start] = 0;
        q.push_back(start);
        while let Some(k) = q.pop_front() {
            for n in neighbours(k, nx, ny) {
                if dist[n] < 0 && self.passable(k, n) && blocked.is_none_or(|b| !b[n]) {
                    dist[n] = dist[k] + 1;
                    q.push_back(n);
                }
            }
        }
        dist
    }

    /// Put an item without checks (ways in and out, lava): its squares are reserved.
    fn put(&mut self, it: Item) {
        for j in it.y as usize..(it.y + it.h) as usize {
            for i in it.x as usize..(it.x + it.w) as usize {
                self.taken[j * self.nx + i] = true;
                if it.blocks_move {
                    self.blocked[j * self.nx + i] = true;
                }
            }
        }
        self.items.push(it);
    }

    /// Place an item if its squares are free floor (of `room`, if given) and, if it blocks,
    /// every other floor square stays reachable from the arrival.
    fn place(&mut self, it: Item, room: Option<i16>, ny: usize) -> bool {
        let nx = self.nx;
        let (x, y, w, h) = (it.x as usize, it.y as usize, it.w as usize, it.h as usize);
        if x + w > nx || y + h > ny {
            return false;
        }
        let squares: Vec<usize> = (y..y + h).flat_map(|j| (x..x + w).map(move |i| j * nx + i)).collect();
        if squares.iter().any(|&k| self.cells[k] < 0 || self.taken[k] || room.is_some_and(|r| self.cells[k] != r)) {
            return false;
        }
        if it.blocks_move {
            let mut blocked = self.blocked.clone();
            for &k in &squares {
                blocked[k] = true;
            }
            let dist = self.distances(self.up, ny, Some(&blocked));
            if (0..nx * ny).any(|k| self.cells[k] >= 0 && !blocked[k] && dist[k] < 0) {
                return false;
            }
            self.blocked = blocked;
        }
        for &k in &squares {
            self.taken[k] = true;
        }
        self.items.push(it);
        true
    }

    /// Doorways stay clear.
    fn reserve_doors(&mut self) {
        let ds: Vec<(usize, usize)> = self.doors.iter().copied().collect();
        for (a, b) in ds {
            self.taken[a] = true;
            self.taken[b] = true;
        }
    }

    fn finish(self, nx: usize, ny: usize, z: i8, name: String, elevation_ft: f32) -> Level {
        let mut rooms: Vec<Room> = self.rooms.iter().map(|r| Room { kind: r.kind, squares: 0, raise_ft: r.raise, center: [0.0; 2] }).collect();
        let mut sum = vec![[0.0f64; 2]; rooms.len()];
        for k in 0..nx * ny {
            let r = self.cells[k];
            if r >= 0 {
                rooms[r as usize].squares += 1;
                sum[r as usize][0] += (k % nx) as f64 + 0.5;
                sum[r as usize][1] += (k / nx) as f64 + 0.5;
            }
        }
        for (r, s) in rooms.iter_mut().zip(&sum) {
            if r.squares > 0 {
                r.center = [(s[0] / r.squares as f64) as f32, (s[1] / r.squares as f64) as f32];
            }
        }
        // Walls on square edges: rock against floor (exterior), and between rooms where no
        // door is (built levels only). Unit edges, merged into runs.
        let cell = |i: isize, j: isize| {
            if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { self.cells[j as usize * nx + i as usize] }
        };
        let mut hs: Vec<(isize, isize, bool)> = Vec::new();
        let mut vs: Vec<(isize, isize, bool)> = Vec::new();
        let mut doors = Vec::new();
        for j in 0..=ny as isize {
            for i in 0..=nx as isize {
                // Edge above square (i, j): between (i, j-1) and (i, j).
                for (horizontal, a, b) in [(true, (i, j - 1), (i, j)), (false, (i - 1, j), (i, j))] {
                    if (horizontal && i >= nx as isize) || (!horizontal && j >= ny as isize) {
                        continue;
                    }
                    let (ra, rb) = (cell(a.0, a.1), cell(b.0, b.1));
                    if ra < 0 && rb < 0 || ra == rb {
                        continue;
                    }
                    let edge = (j, i, ra < 0 || rb < 0);
                    if !edge.2 {
                        let (ka, kb) = (a.1 as usize * nx + a.0 as usize, b.1 as usize * nx + b.0 as usize);
                        if self.doors.contains(&(ka.min(kb), ka.max(kb))) {
                            let (p0, p1) = if horizontal { ([i as f32, j as f32], [i as f32 + 1.0, j as f32]) } else { ([i as f32, j as f32], [i as f32, j as f32 + 1.0]) };
                            let kind = if self.secret.contains(&(ka.min(kb), ka.max(kb))) { "secret" } else { "door" };
                            doors.push(Door { a: p0, b: p1, kind, rooms: [ra, rb] });
                            continue;
                        }
                        if self.natural {
                            continue;
                        }
                    }
                    if horizontal {
                        hs.push(edge);
                    } else {
                        vs.push((i, j, edge.2));
                    }
                }
            }
        }
        let mut walls = Vec::new();
        for (runs, horizontal) in [(hs, true), (vs, false)] {
            // (line, position along it, exterior), sorted by line then position.
            let mut runs = runs;
            runs.sort();
            let mut k = 0;
            while k < runs.len() {
                let (line, start, ext) = runs[k];
                let mut end = start + 1;
                while k + 1 < runs.len() && runs[k + 1] == (line, end, ext) {
                    end += 1;
                    k += 1;
                }
                let (a, b) = if horizontal { ([start as f32, line as f32], [end as f32, line as f32]) } else { ([line as f32, start as f32], [line as f32, end as f32]) };
                walls.push(Wall { a, b, exterior: ext });
                k += 1;
            }
        }
        Level { z, name, elevation_ft, cells: self.cells, rooms, walls, doors, windows: Vec::new(), furniture: self.items, roof: false, has_stairs: false, natural: self.natural, paths: self.paths, links: self.links }
    }
}

fn neighbours(k: usize, nx: usize, ny: usize) -> impl Iterator<Item = usize> {
    let (i, j) = (k % nx, k / nx);
    [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten()
}

// ---------------------------------------------------------------------------------------
// Dungeons and crypts: rooms, a spanning tree plus loops, dug corridors.

#[derive(Clone, Copy)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Rect {
    fn center(&self) -> (f64, f64) {
        (self.x as f64 + self.w as f64 / 2.0, self.y as f64 + self.h as f64 / 2.0)
    }
    /// Overlaps `o` grown by one square of rock all round.
    fn touches(&self, o: &Rect) -> bool {
        self.x < o.x + o.w + 1 && o.x < self.x + self.w + 1 && self.y < o.y + o.h + 1 && o.y < self.y + self.h + 1
    }
}

fn dungeon(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let (ax, ay) = (arrive % nx, arrive / nx);
    let fit = |c: usize, len: usize, n: usize, rng: &mut Pcg32| {
        // A start so that [start, start + len) holds c and stays inside [1, n - 1).
        let lo = c.saturating_sub(len - 1).max(1);
        let hi = c.min(n - 1 - len);
        if hi <= lo { lo } else { lo + rng.below((hi - lo + 1) as u32) as usize }
    };
    let (w, h) = (4 + rng.below(3) as usize, 4 + rng.below(3) as usize);
    let mut rects = vec![Rect { x: fit(ax, w, nx, rng), y: fit(ay, h, ny, rng), w, h }];
    // The boss chamber first on the last level: big, as far from the arrival as it fits.
    let mut boss: Option<usize> = None;
    if last {
        let mut best: Option<(f64, Rect)> = None;
        for _ in 0..80 {
            let (w, h) = (8 + rng.below(3) as usize, 7 + rng.below(3) as usize);
            let r = Rect { x: 1 + rng.below((nx - w - 2) as u32) as usize, y: 1 + rng.below((ny - h - 2) as u32) as usize, w, h };
            if rects[0].touches(&r) {
                continue;
            }
            let (cx, cy) = r.center();
            let d = (cx - ax as f64).hypot(cy - ay as f64);
            if best.is_none_or(|(b, _)| d > b) {
                best = Some((d, r));
            }
        }
        if let Some((_, r)) = best {
            rects.push(r);
            boss = Some(1);
        }
    }
    let mut target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * area).round() as usize).max(3);
    if theme.sprawl {
        target = target.max(nx * ny / 140).min(40);
    }
    for _ in 0..(600.0 * area.max(1.0)) as usize {
        if rects.len() >= target {
            break;
        }
        let (lo, span) = if theme.sprawl { (4, 6) } else { (3, 5) };
        let (w, h) = (lo + rng.below(span) as usize, lo + rng.below(span) as usize);
        let r = Rect { x: 1 + rng.below((nx - w - 2) as u32) as usize, y: 1 + rng.below((ny - h - 2) as u32) as usize, w, h };
        if rects.iter().all(|o| !o.touches(&r)) {
            rects.push(r);
        }
    }
    let kinds = theme.kinds;
    for (ri, r) in rects.iter().enumerate() {
        let kind = if Some(ri) == boss {
            BOSS
        } else if ri == 0 {
            if depth == 0 { theme.first } else { "landing" }
        } else {
            kinds[rng.below(kinds.len() as u32) as usize]
        };
        let id = p.room(kind, 0.0);
        for j in r.y..r.y + r.h {
            for i in r.x..r.x + r.w {
                p.cells[j * nx + i] = id;
            }
        }
    }
    let passage = p.room(theme.passage, 0.0);
    // Spanning tree over the rooms (not the boss chamber), then loops; the boss chamber hangs
    // off its nearest room only.
    let n = rects.len();
    let cdist = |a: usize, b: usize| {
        let (ca, cb) = (rects[a].center(), rects[b].center());
        (ca.0 - cb.0).hypot(ca.1 - cb.1)
    };
    let normal: Vec<usize> = (0..n).filter(|&i| Some(i) != boss).collect();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut inside = vec![false; n];
    inside[0] = true;
    let mut mst_len = 0.0;
    for _ in 1..normal.len() {
        let mut best: Option<(f64, usize, usize)> = None;
        for &a in normal.iter().filter(|&&a| inside[a]) {
            for &b in normal.iter().filter(|&&b| !inside[b]) {
                let d = cdist(a, b);
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, a, b));
                }
            }
        }
        let Some((d, a, b)) = best else { break };
        inside[b] = true;
        mst_len += d;
        edges.push((a, b));
    }
    let avg = mst_len / edges.len().max(1) as f64;
    let mst_n = edges.len();
    for (ia, &a) in normal.iter().enumerate() {
        for &b in &normal[ia + 1..] {
            if !edges.contains(&(a, b)) && !edges.contains(&(b, a)) && cdist(a, b) < 1.3 * avg && rng.next_f64() < 0.2 {
                edges.push((a, b));
            }
        }
    }
    if let Some(bi) = boss {
        let near = normal.iter().copied().filter(|&a| a != 0 || normal.len() == 1).min_by(|&a, &b| cdist(a, bi).total_cmp(&cdist(b, bi))).unwrap_or(0);
        edges.push((near, bi));
    }
    // Some of the loops are walled off by secret doors.
    for (ei, (a, b)) in edges.into_iter().enumerate() {
        let made = dig(&mut p, nx, ny, a as i16, b as i16, passage, theme.corridor > 1);
        if ei >= mst_n && Some(b) != boss && rng.next_f64() < 0.4 {
            p.secret.extend(made);
        }
    }
    // Rooms the corridors failed to reach (boxed in): back to rock.
    let dist = p.distances(arrive, ny, None);
    for k in 0..nx * ny {
        if p.cells[k] >= 0 && dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    let alive = |p: &Plan, r: i16| p.cells.iter().any(|&c| c == r);
    if let Some(bi) = boss
        && !alive(&p, bi as i16)
    {
        // No way into the boss chamber was found: the farthest room takes its place.
        let far = (0..nx * ny).filter(|&k| p.cells[k] >= 0 && p.cells[k] != passage).max_by_key(|&k| dist[k]).map(|k| p.cells[k]);
        if let Some(r) = far {
            p.rooms[r as usize].kind = BOSS;
        }
    }
    if !last {
        // The way down: the middle of the room farthest from the arrival.
        let far = (0..nx * ny).filter(|&k| p.cells[k] >= 0 && p.cells[k] != passage && p.cells[k] != p.cells[arrive]).max_by_key(|&k| dist[k]);
        let room = far.map(|k| p.cells[k]).unwrap_or(p.cells[arrive]);
        let r = &rects[room as usize];
        let c = (r.y + r.h / 2) * nx + r.x + r.w / 2;
        p.down = Some(if p.cells[c] == room && c != arrive { c } else { far.unwrap_or(arrive) });
    }
    p
}

/// The sewers of a section: the ground under the paved streets, joined where runs meet only
/// outside the section by service passages; a sewage channel down the wider runs, an outfall
/// where a tunnel reaches water; the drain shaft down at the far end from the way in (the
/// grate, else the run nearest the section's middle). None if no street runs here.
fn sewer(t0: &T0, l: &Layout, origin: P, nx: usize, ny: usize, grate: Option<usize>, last: bool) -> Option<Plan> {
    let n = nx * ny;
    let mut p = Plan::new(nx, ny, 0, false);
    let tunnel = p.room("sewer tunnel", 0.0);
    let centre = |k: usize| [origin[0] + ((k % nx) as f64 + 0.5) * SQUARE_FT, origin[1] + ((k / nx) as f64 + 0.5) * SQUARE_FT];
    let mut is_wet: Vec<Option<bool>> = vec![None; n];
    let r = SEWER_REACH_FT;
    for (a, b, _) in street_segments(l) {
        let (x0, y0) = (a[0].min(b[0]) - r - origin[0], a[1].min(b[1]) - r - origin[1]);
        let (x1, y1) = (a[0].max(b[0]) + r - origin[0], a[1].max(b[1]) + r - origin[1]);
        let span = nx as f64 * SQUARE_FT;
        if x1 < 0.0 || y1 < 0.0 || x0 > span || y0 > span {
            continue;
        }
        let g = |p: P| [((p[0] - origin[0]) / SQUARE_FT) as f32, ((p[1] - origin[1]) / SQUARE_FT) as f32];
        let (ga, gb) = (g(a), g(b));
        p.paths.push([ga[0], ga[1], gb[0], gb[1], (r / SQUARE_FT) as f32]);
        let i0 = (x0 / SQUARE_FT).floor().max(0.0) as usize;
        let j0 = (y0 / SQUARE_FT).floor().max(0.0) as usize;
        let i1 = ((x1 / SQUARE_FT).ceil() as usize).min(nx - 1);
        let j1 = ((y1 / SQUARE_FT).ceil() as usize).min(ny - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let k = j * nx + i;
                if p.cells[k] >= 0 || town::geom::seg_dist(centre(k), a, b) > r {
                    continue;
                }
                if !*is_wet[k].get_or_insert_with(|| wet(t0, centre(k))) {
                    p.cells[k] = tunnel;
                }
            }
        }
    }
    // Where cellars and keep tunnels meet this section: their squares are sewer.
    let s_ft = SEWER_SECTION_FT;
    let here = [origin[0] / s_ft, origin[1] / s_ft];
    let mut joins: Vec<(usize, Link, &'static str, &'static str)> = Vec::new();
    let square_of = |q: P| {
        let (i, j) = (((q[0] - origin[0]) / SQUARE_FT).floor(), ((q[1] - origin[1]) / SQUARE_FT).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    for b in &l.buildings {
        let (c, bi) = (town::geom::centroid(&b.poly), b.id);
        if (c[0] / s_ft - here[0] - 0.5).abs() > 0.8 || (c[1] / s_ft - here[1] - 0.5).abs() > 0.8 {
            continue;
        }
        if let Some(q) = sewer_link_of(t0, l, b)
            && let Some(k) = square_of(q)
        {
            joins.push((k, Link { x: (k % nx) as u16, y: (k / nx) as u16, to: format!("b:{}:{bi}", l.index) }, "ladder", "ladder up into a cellar"));
        }
        if let Some(q) = keep_escape(t0, l, b)
            && let Some(k) = square_of(q)
        {
            joins.push((k, Link { x: (k % nx) as u16, y: (k / nx) as u16, to: format!("k:{}:{bi}", l.index) }, "tunnel", "tunnel to the keep's dungeons"));
        }
    }
    for (k, _, _, _) in &joins {
        p.cells[*k] = tunnel;
    }
    // Every street grate over this section is a way up (the first is the way in).
    let grates: Vec<usize> = l.entrances.iter().filter(|e| e.kind == UnderKind::Sewer && sewer_section(e.at) == origin).filter_map(|e| square_of(e.at)).collect();
    for &k in &grates {
        p.cells[k] = tunnel;
    }
    let arrive = match grate {
        Some(g) => {
            p.cells[g] = tunnel;
            g
        }
        None => {
            let mid = |k: usize| (k % nx).abs_diff(nx / 2).pow(2) + (k / nx).abs_diff(ny / 2).pow(2);
            (0..n).filter(|&k| p.cells[k] == tunnel).min_by_key(|&k| mid(k))?
        }
    };
    p.up = arrive;
    // Runs that meet only outside the section: each its own room for a moment, dug to the
    // grate's run by a service passage, then one sewer again.
    let mut comp = vec![-1i32; n];
    let mut comps: Vec<Vec<usize>> = Vec::new();
    for s in 0..n {
        if p.cells[s] != tunnel || comp[s] >= 0 {
            continue;
        }
        let id = comps.len() as i32;
        let mut stack = vec![s];
        comp[s] = id;
        let mut members = Vec::new();
        while let Some(k) = stack.pop() {
            members.push(k);
            for m in neighbours(k, nx, ny) {
                if p.cells[m] == tunnel && comp[m] < 0 {
                    comp[m] = id;
                    stack.push(m);
                }
            }
        }
        comps.push(members);
    }
    if comps.len() > 1 {
        let main = comp[arrive] as usize;
        let mut ids = vec![tunnel; comps.len()];
        for (c, members) in comps.iter().enumerate() {
            if c != main {
                ids[c] = p.room("sewer tunnel", 0.0);
                for &k in members {
                    p.cells[k] = ids[c];
                }
            }
        }
        let passage = p.room("service passage", 0.0);
        // Each run joins the grate's network as soon as it is dug to it (a later run may
        // reach the network only through it); rounds until no more join.
        let mut left: Vec<usize> = (0..comps.len()).filter(|&c| c != main).collect();
        loop {
            let before = left.len();
            left.retain(|&c| {
                let id = ids[c];
                let doors = p.doors.len();
                dig(&mut p, nx, ny, id, tunnel, passage, false);
                if p.doors.len() == doors {
                    return true;
                }
                for &k in &comps[c] {
                    p.cells[k] = tunnel;
                }
                false
            });
            if left.is_empty() || left.len() == before {
                break;
            }
        }
        // The doors dug between a run and its passage stay; any between two runs go.
        let doors: Vec<(usize, usize)> = p.doors.iter().copied().filter(|&(a, b)| p.cells[a] != p.cells[b]).collect();
        p.doors = doors.into_iter().collect();
    }
    let dist = p.distances(arrive, ny, None);
    for k in 0..n {
        if dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    // The drain shaft down: the farthest run from the grate, clear of the section's edge.
    if !last {
        let inner = |k: usize| (3..nx - 3).contains(&(k % nx)) && (3..ny - 3).contains(&(k / nx));
        // (Never on a grate or where a cellar or keep joins.)
        let kept = |k: usize| grates.contains(&k) || joins.iter().any(|j| j.0 == k);
        let far = |ok: &dyn Fn(usize) -> bool| (0..n).filter(|&k| dist[k] > 0 && p.cells[k] == tunnel && !kept(k) && ok(k)).max_by_key(|&k| dist[k]);
        // (Never on the section's outermost squares: the level below keeps a ring of rock.)
        let margin1 = |k: usize| (1..nx - 1).contains(&(k % nx)) && (1..ny - 1).contains(&(k / nx));
        p.down = far(&inner).or_else(|| far(&margin1));
        // A scrap of tunnel too small for a shaft: no sewer section here.
        p.down?;
    }
    // Away from the walls: floor on all four sides.
    let all_floor = |p: &Plan, k: usize| {
        let (i, j) = (k % nx, k / nx);
        i > 0 && j > 0 && i + 1 < nx && j + 1 < ny && neighbours(k, nx, ny).all(|m| p.cells[m] >= 0)
    };
    let keep = |p: &Plan, k: usize| k == p.up || Some(k) == p.down;
    // The other grates: ladders up to the street.
    let up = p.up;
    for &k in grates.iter().filter(|&&k| k != up) {
        if dist[k] >= 0 && !p.taken[k] && Some(k) != p.down {
            p.put(Item::new("exit", "ladder up to a street grate", k % nx, k / nx, 1, 1, 0, false, 0.0, None));
        }
    }
    // The ladders and tunnels where the cellars and keeps join (one per square).
    // (Two cellars facing across a street can meet at one square: the second ladder takes the
    // nearest free square of tunnel.)
    for (k, link, kind, name) in joins {
        if dist[k] < 0 {
            continue;
        }
        let mut seen = vec![false; n];
        let mut q = std::collections::VecDeque::from([k]);
        seen[k] = true;
        let mut spot = None;
        while let Some(m) = q.pop_front() {
            if !p.taken[m] && m != p.up && Some(m) != p.down {
                spot = Some(m);
                break;
            }
            for o in neighbours(m, nx, ny) {
                if !seen[o] && p.cells[o] >= 0 {
                    seen[o] = true;
                    q.push_back(o);
                }
            }
        }
        if let Some(m) = spot {
            p.put(Item::new(kind, name, m % nx, m / nx, 1, 1, 0, false, 0.0, None));
            p.links.push(Link { x: (m % nx) as u16, y: (m / nx) as u16, ..link });
        }
    }
    // Sewage down the middle of the wider runs; walkways along the walls.
    for k in 0..n {
        if p.cells[k] == tunnel && !keep(&p, k) && all_floor(&p, k) {
            p.put(Item::new("sewage", "sewage channel", k % nx, k / nx, 1, 1, 0, false, 0.0, Some("difficult terrain; swallowed: DC 11 Con save or poisoned for 1 hour")));
        }
    }
    // An outfall where a run reaches water.
    let mut outfalls = 0;
    for k in 0..n {
        if outfalls >= 2 || p.cells[k] != tunnel || p.taken[k] || keep(&p, k) {
            continue;
        }
        if neighbours(k, nx, ny).any(|m| p.cells[m] < 0 && *is_wet[m].get_or_insert_with(|| wet(t0, centre(m)))) {
            p.put(Item::new("outfall", "outfall grate", k % nx, k / nx, 1, 1, 0, false, 0.0, Some("strong current: DC 12 Str (Athletics) or swept 20 ft toward the water")));
            outfalls += 1;
        }
    }
    // Where runs leave the section (one mark per opening).
    for k in 0..n {
        let (i, j) = (k % nx, k / nx);
        let edge = i == 0 || j == 0 || i == nx - 1 || j == ny - 1;
        if !edge || p.cells[k] != tunnel || p.taken[k] || keep(&p, k) {
            continue;
        }
        let prev = if i == 0 || i == nx - 1 { (j > 0).then(|| k - nx) } else { (i > 0).then(|| k - 1) };
        if prev.is_some_and(|q| p.cells[q] >= 0) {
            continue;
        }
        p.put(Item::new("continues", "the sewer runs on (double-click to follow)", i, j, 1, 1, 0, false, 0.0, None));
    }
    Some(p)
}

/// Dig a corridor from room `a` to room `b`: least cost through rock (reusing corridors is
/// cheaper, turning and running beside other rooms dearer), never through another room;
/// `wide`: two squares wide where that touches no other room.
#[allow(clippy::too_many_arguments)]
fn dig(p: &mut Plan, nx: usize, ny: usize, a: i16, b: i16, passage: i16, wide: bool) -> Vec<(usize, usize)> {
    let n = nx * ny;
    // State: square × heading (0..4).
    let mut cost = vec![u32::MAX; n * 4];
    let mut came = vec![u32::MAX; n * 4];
    let mut heap = std::collections::BinaryHeap::new();
    for k in 0..n {
        if p.cells[k] == a {
            for h in 0..4 {
                cost[k * 4 + h] = 0;
                heap.push(std::cmp::Reverse((0u32, (k * 4 + h) as u32)));
            }
        }
    }
    let other_room = |c: i16| c >= 0 && c != a && c != b && c != passage;
    let near_other = |k: usize| neighbours(k, nx, ny).any(|m| other_room(p.cells[m]));
    let mut goal = None;
    while let Some(std::cmp::Reverse((c, s))) = heap.pop() {
        let s = s as usize;
        if c > cost[s] {
            continue;
        }
        let (k, h) = (s / 4, s % 4);
        if p.cells[k] == b {
            goal = Some(s);
            break;
        }
        let (i, j) = (k % nx, k / nx);
        for (nh, (di, dj)) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
            let (ni, nj) = (i as isize + di, j as isize + dj);
            // Keep a ring of rock round the edge.
            if ni < 1 || nj < 1 || ni >= nx as isize - 1 || nj >= ny as isize - 1 {
                continue;
            }
            let m = nj as usize * nx + ni as usize;
            let cm = p.cells[m];
            if other_room(cm) {
                continue;
            }
            let step = if cm == b || cm == a {
                1
            } else if cm == passage {
                2
            } else {
                4 + if near_other(m) { 6 } else { 0 }
            } + if nh != h && p.cells[k] != a { 3 } else { 0 };
            let nc = c + step;
            let ns = m * 4 + nh;
            if nc < cost[ns] {
                cost[ns] = nc;
                came[ns] = s as u32;
                heap.push(std::cmp::Reverse((nc, ns as u32)));
            }
        }
    }
    let Some(mut s) = goal else { return Vec::new() };
    let mut path = vec![s / 4];
    while came[s] != u32::MAX {
        s = came[s] as usize;
        path.push(s / 4);
    }
    path.reverse();
    let mut made = Vec::new();
    for w in path.windows(2) {
        let (k0, k1) = (w[0], w[1]);
        if p.cells[k1] < 0 {
            p.cells[k1] = passage;
        }
        if p.cells[k0] != p.cells[k1] {
            p.door(k0, k1);
            made.push((k0.min(k1), k0.max(k1)));
        }
    }
    if wide {
        // The second lane: beside each step (below a run along x, right of one along y).
        for w in path.windows(2) {
            let (k0, k1) = (w[0], w[1]);
            if p.cells[k1] != passage {
                continue;
            }
            let m = if k1.abs_diff(k0) == 1 { k1 + nx } else { k1 + 1 };
            let (i, j) = (m % nx, m / nx);
            let other = |c: i16| c >= 0 && c != passage;
            if m < n && i >= 1 && j >= 1 && i + 1 < nx && j + 1 < ny && p.cells[m] < 0 && !neighbours(m, nx, ny).any(|q| other(p.cells[q])) {
                p.cells[m] = passage;
            }
        }
    }
    made
}

// ---------------------------------------------------------------------------------------
// Natural levels: caves and lava tubes.

/// Cellular automaton cave grown around a winding spine from the arrival toward the far side.
fn cave(nx: usize, ny: usize, arrive: usize, last: bool, theme: &Theme, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut open = vec![false; n];
    let interior = |k: usize| {
        let (i, j) = (k % nx, k / nx);
        i >= 1 && j >= 1 && i + 1 < nx && j + 1 < ny
    };
    for k in 0..n {
        open[k] = interior(k) && rng.next_f64() < 0.45;
    }
    // The spine: a biased walk away from the arrival, two squares wide.
    let mut keep = vec![false; n];
    let (mut x, mut y) = ((arrive % nx) as f64, (arrive / nx) as f64);
    let toward = if ((arrive % nx) as f64) < nx as f64 / 2.0 { 1.0 } else { -1.0 };
    let mut heading: f64 = if toward > 0.0 { 0.0 } else { std::f64::consts::PI };
    for _ in 0..(nx * 2) {
        for dj in -1..=1 {
            for di in -1..=1 {
                let (i, j) = ((x as isize + di).clamp(1, nx as isize - 2), (y as isize + dj).clamp(1, ny as isize - 2));
                keep[j as usize * nx + i as usize] = true;
            }
        }
        heading += rng.range(-0.6, 0.6);
        // Pull back toward the far side and away from the edges.
        let back = if toward > 0.0 { 0.0 } else { std::f64::consts::PI };
        heading += 0.15 * libm::sin(back - heading);
        x = (x + libm::cos(heading)).clamp(2.0, nx as f64 - 3.0);
        y = (y + libm::sin(heading) + 0.04 * (ny as f64 / 2.0 - y)).clamp(2.0, ny as f64 - 3.0);
        if (toward > 0.0 && x >= nx as f64 - 4.0) || (toward < 0.0 && x <= 3.0) {
            break;
        }
    }
    for it in 0..5 {
        let mut next = open.clone();
        for k in 0..n {
            if !interior(k) {
                next[k] = false;
                continue;
            }
            let (i, j) = (k % nx, k / nx);
            let mut rock = 0;
            for dj in -1..=1isize {
                for di in -1..=1isize {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    let (a, b) = (i as isize + di, j as isize + dj);
                    if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || !open[b as usize * nx + a as usize] {
                        rock += 1;
                    }
                }
            }
            next[k] = if rock >= 5 {
                false
            } else if rock <= 3 {
                true
            } else {
                open[k]
            };
            if it < 3 && keep[k] {
                next[k] = true;
            }
        }
        open = next;
    }
    open[arrive] = true;
    natural_level(nx, ny, arrive, last, open, theme.kinds, theme.ledge, rng)
}

/// A meandering lava tube with side pockets; the lower tube carries a lava channel.
fn lava_tube(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut open = vec![false; n];
    let (ax, ay) = ((arrive % nx) as isize, (arrive / nx) as isize);
    let toward: isize = if (ax as usize) < nx / 2 { 1 } else { -1 };
    let (p1, p2, p3) = (rng.range(0.0, 6.28), rng.range(0.0, 6.28), rng.range(0.0, 6.28));
    let amp = (ny as f64 / 2.0 - 6.0).max(1.0);
    let centre = |x: f64| ny as f64 / 2.0 + amp * (0.6 * libm::sin(x * 0.11 + p1) + 0.4 * libm::sin(x * 0.27 + p2));
    let half = |x: f64| if depth > 0 { 2.5 } else { 1.5 } + 1.4 * (0.5 + 0.5 * libm::sin(x * 0.19 + p3));
    let carve = |open: &mut Vec<bool>, i: isize, j: isize| {
        if i >= 1 && j >= 1 && i < nx as isize - 1 && j < ny as isize - 1 {
            open[j as usize * nx + i as usize] = true;
        }
    };
    let (x0, x1) = if toward > 0 { (ax, nx as isize - 3) } else { (2, ax) };
    for i in x0..=x1 {
        let (c, w) = (centre(i as f64), half(i as f64));
        for j in (c - w).floor() as isize..=(c + w).ceil() as isize {
            if (j as f64 - c).abs() <= w {
                carve(&mut open, i, j);
            }
        }
    }
    // From the arrival to the tube.
    let c0 = centre(ax as f64).round() as isize;
    for j in ay.min(c0)..=ay.max(c0) {
        carve(&mut open, ax, j);
        carve(&mut open, ax + toward, j);
    }
    // Side pockets.
    for _ in 0..3 + rng.below(3) {
        let i = x0 + rng.below((x1 - x0).max(1) as u32) as isize;
        let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
        let (c, w) = (centre(i as f64), half(i as f64));
        let r = rng.range(1.8, 3.2);
        let pc = c + side * (w + r + 0.5);
        for j in (pc - r).floor() as isize..=(pc + r).ceil() as isize {
            for di in -(r.ceil() as isize)..=r.ceil() as isize {
                if ((di * di) as f64 + (j as f64 - pc) * (j as f64 - pc)) <= r * r {
                    carve(&mut open, i + di, j);
                }
            }
        }
        let (lo, hi) = (c.min(pc).round() as isize, c.max(pc).round() as isize);
        for j in lo..=hi {
            carve(&mut open, i, j);
        }
    }
    open[arrive] = true;
    let mut p = natural_level(nx, ny, arrive, last, open, theme.kinds, theme.ledge, rng);
    // The lower tube: a lava channel down its middle (never on the ways in, out or the
    // boss chamber's floor).
    if depth > 0 {
        for i in 1..nx as isize - 1 {
            let j = centre(i as f64).round() as isize;
            if j < 1 || j >= ny as isize - 1 {
                continue;
            }
            let k = j as usize * nx + i as usize;
            let r = p.cells[k];
            if r < 0 || p.taken[k] || Some(k) == p.down || k == p.up || p.rooms[r as usize].kind == BOSS {
                continue;
            }
            p.put(lava(i as usize, j as usize));
        }
    }
    p
}

/// Finish a natural level from its open squares: keep what the arrival reaches, the boss
/// chamber (last level) or the way down at the far end, chambers by nearest seed (some
/// raised as ledges).
fn natural_level(nx: usize, ny: usize, arrive: usize, last: bool, open: Vec<bool>, kinds: &[&'static str], ledge: &'static str, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut p = Plan::new(nx, ny, arrive, true);
    let floor = p.room("floor", 0.0);
    for k in 0..n {
        if open[k] {
            p.cells[k] = floor;
        }
    }
    let dist = p.distances(arrive, ny, None);
    for k in 0..n {
        if dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    let far = (0..n).filter(|&k| dist[k] >= 0).max_by_key(|&k| dist[k]).unwrap_or(arrive);
    let max_d = dist[far].max(0);
    // Chambers: nearest of a few seeds by walking distance.
    let cells: Vec<usize> = (0..n).filter(|&k| p.cells[k] >= 0).collect();
    let mut seeds = vec![arrive];
    for _ in 0..(cells.len() / 110 + 3) {
        seeds.push(cells[rng.below(cells.len() as u32) as usize]);
    }
    p.rooms.clear();
    let mut owner = vec![-1i16; n];
    let mut q = std::collections::VecDeque::new();
    for (si, &s) in seeds.iter().enumerate() {
        if owner[s] < 0 {
            let raise = if si == 0 { 0.0 } else { [0.0, 0.0, 0.0, 5.0, 5.0, 10.0][rng.below(6) as usize] };
            let kind = if raise > 0.0 { ledge } else { kinds[rng.below(kinds.len() as u32) as usize] };
            let id = p.room(kind, raise);
            owner[s] = id;
            q.push_back(s);
        }
    }
    while let Some(k) = q.pop_front() {
        for m in neighbours(k, nx, ny) {
            if p.cells[m] >= 0 && owner[m] < 0 {
                owner[m] = owner[k];
                q.push_back(m);
            }
        }
    }
    for k in 0..n {
        if p.cells[k] >= 0 {
            p.cells[k] = owner[k];
        }
    }
    // The arrival stands on the floor (not a ledge).
    let a = p.cells[arrive] as usize;
    p.rooms[a].raise = 0.0;
    if last {
        // The boss chamber: the far end. Squares at least `max_d - reach` steps in, joined to
        // the farthest square: every other square's shortest way in avoids them (a dead end).
        let reach = 12.min(max_d / 3);
        let t = max_d - reach;
        let boss = p.room(BOSS, 0.0);
        let mut q = std::collections::VecDeque::from([far]);
        let mut seen = vec![false; n];
        seen[far] = true;
        let mut chamber = Vec::new();
        while let Some(k) = q.pop_front() {
            chamber.push(k);
            for m in neighbours(k, nx, ny) {
                if !seen[m] && p.cells[m] >= 0 && dist[m] >= t {
                    seen[m] = true;
                    q.push_back(m);
                }
            }
        }
        for &k in &chamber {
            p.cells[k] = boss;
        }
        // Widen it into the rock where that touches nothing else.
        let (fx, fy) = ((far % nx) as isize, (far / nx) as isize);
        for _ in 0..3 {
            for j in fy - 5..=fy + 5 {
                for i in fx - 5..=fx + 5 {
                    if i < 1 || j < 1 || i >= nx as isize - 1 || j >= ny as isize - 1 || (i - fx) * (i - fx) + (j - fy) * (j - fy) > 25 {
                        continue;
                    }
                    let k = j as usize * nx + i as usize;
                    if p.cells[k] < 0 && neighbours(k, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == boss) && neighbours(k, nx, ny).any(|m| p.cells[m] == boss) {
                        p.cells[k] = boss;
                    }
                }
            }
        }
    } else {
        p.down = Some(far);
        let r = p.cells[far] as usize;
        p.rooms[r].raise = 0.0;
    }
    p
}

/// A haulage way with side drifts and stopes; a shaft at the end, or the boss chamber.
fn mine(nx: usize, ny: usize, arrive: usize, last: bool, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, true);
    let (ax, ay) = ((arrive % nx) as isize, (arrive / nx) as isize);
    let toward: isize = if (ax as usize) < nx / 2 { 1 } else { -1 };
    let end = if toward > 0 {
        nx as isize - (if last { 13 } else { 4 })
    } else if last {
        12
    } else {
        3
    };
    let haul = p.room("haulage way", 0.0);
    let ok = |i: isize, j: isize| i >= 1 && j >= 2 && i < nx as isize - 1 && j < ny as isize - 2;
    let mut y = ay;
    let mut x = ax;
    let mut main: Vec<(isize, isize)> = Vec::new();
    // Two squares wide (rails down the middle); jogs a square now and then.
    while x != end + toward {
        for dj in [0, -1] {
            if ok(x, y + dj) {
                p.cells[(y + dj) as usize * nx + x as usize] = haul;
            }
        }
        main.push((x, y));
        if rng.next_f64() < 0.12 {
            let ny_ = (y + if rng.next_f64() < 0.5 { 1 } else { -1 }).clamp(3, ny as isize - 4);
            for dj in [0, -1] {
                if ok(x, ny_ + dj) {
                    p.cells[(ny_ + dj) as usize * nx + x as usize] = haul;
                }
            }
            y = ny_;
        }
        x += toward;
    }
    // From the arrival square onto the haulage way.
    for j in ay.min(y - 1)..=ay.max(y) {
        if ok(ax, j) {
            p.cells[j as usize * nx + ax as usize] = haul;
        }
    }
    let free = |p: &Plan, i: isize, j: isize, own: i16| {
        ok(i, j) && neighbours(j as usize * nx + i as usize, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == own) && p.cells[j as usize * nx + i as usize] < 0
    };
    // Side drifts, kept a square of rock from everything else; a stope at some ends.
    let mut k = 4 + rng.below(4) as usize;
    while k + 6 < main.len() {
        let (mx, my) = main[k];
        let side: isize = if rng.next_f64() < 0.5 { 1 } else { -1 };
        let id = p.room("drift", 0.0);
        let (mut i, mut j) = (mx, if side > 0 { my + 1 } else { my - 2 });
        let len = 5 + rng.below(10) as isize;
        let mut dug = 0;
        let mut last_sq = None;
        for s in 0..len {
            // The first square opens off the haulage way.
            let fits = if s == 0 {
                ok(i, j) && p.cells[j as usize * nx + i as usize] < 0
            } else {
                free(&p, i, j, id)
                    || (ok(i, j)
                        && p.cells[j as usize * nx + i as usize] < 0
                        && neighbours(j as usize * nx + i as usize, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == id || (s == 1 && p.cells[m] == haul)))
            };
            if !fits {
                break;
            }
            p.cells[j as usize * nx + i as usize] = id;
            dug += 1;
            last_sq = Some((i, j));
            if rng.next_f64() < 0.2 {
                i += if rng.next_f64() < 0.5 { 1 } else { -1 };
                if !(ok(i, j) && p.cells[j as usize * nx + i as usize] < 0) {
                    break;
                }
                p.cells[j as usize * nx + i as usize] = id;
            }
            j += side;
        }
        if dug >= 4
            && rng.next_f64() < 0.6
            && let Some((ei, ej)) = last_sq
        {
            let kind = ["stope", "ore chamber", "powder store", "pump room", "collapsed stope"][rng.below(5) as usize];
            let st = p.room(kind, 0.0);
            let (w, h) = (3 + rng.below(3) as isize, 3 + rng.below(2) as isize);
            let (x0, y0) = (ei - w / 2, if side > 0 { ej + 1 } else { ej - h });
            let fits = (y0..y0 + h).all(|j| {
                (x0..x0 + w).all(|i| {
                    free(&p, i, j, st)
                        || (ok(i, j)
                            && p.cells[j as usize * nx + i as usize] < 0
                            && neighbours(j as usize * nx + i as usize, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == st || p.cells[m] == id))
                })
            });
            if fits {
                for j in y0..y0 + h {
                    for i in x0..x0 + w {
                        p.cells[j as usize * nx + i as usize] = st;
                    }
                }
            }
        }
        k += 5 + rng.below(5) as usize;
    }
    let (ex, ey) = *main.last().unwrap_or(&(ax, ay));
    if last {
        // The boss chamber at the end of the haulage way: the deep vein.
        let boss = p.room(BOSS, 0.0);
        let (w, h) = (9isize, 8isize);
        let x0 = if toward > 0 { ex + 1 } else { ex - w };
        let y0 = (ey - h / 2).clamp(1, ny as isize - 1 - h);
        for j in y0..y0 + h {
            for i in x0..x0 + w {
                if i >= 1 && i < nx as isize - 1 && p.cells[j as usize * nx + i as usize] < 0 {
                    p.cells[j as usize * nx + i as usize] = boss;
                }
            }
        }
        // Joined to the haulage way at its end.
        for dj in [0, -1] {
            let k = (ey + dj) as usize * nx + (ex + toward) as usize;
            if p.cells[k] < 0 {
                p.cells[k] = boss;
            }
        }
    } else {
        let shaft = p.room("shaft head", 0.0);
        for dj in [0, -1] {
            p.cells[(ey + dj) as usize * nx + ex as usize] = shaft;
        }
        p.down = Some(ey as usize * nx + ex as usize);
    }
    // Whatever the arrival can't reach goes back to rock.
    let dist = p.distances(arrive, ny, None);
    for k in 0..nx * ny {
        if dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    p
}

// ---------------------------------------------------------------------------------------
// Furnishing.

impl Item {
    #[allow(clippy::too_many_arguments)]
    pub fn new(kind: &'static str, name: &'static str, x: usize, y: usize, w: usize, h: usize, cover: u8, blocks_move: bool, height_ft: f32, hazard: Option<&'static str>) -> Item {
        Item { kind, name, x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover, blocks_move, height_ft, hazard, sprite: 0 }
    }
}

/// A square of a lava tube's lava channel.
fn lava(x: usize, y: usize) -> Item {
    Item::new("lava", "lava channel", x, y, 1, 1, 0, false, 0.0, Some("10d10 fire on entering or starting a turn in it"))
}

/// Props underground: (kind, name, cover, blocks movement, height ft, hazard).
pub const PROPS: &[(&str, &str, u8, bool, f32, Option<&str>)] = &[
    ("pillar", "pillar", 3, true, 15.0, None),
    ("sarcophagus", "sarcophagus", 2, true, 3.5, None),
    ("urn", "burial urn", 1, true, 3.0, None),
    ("brazier", "brazier", 1, true, 4.0, Some("knocked over: 1d6 fire to adjacent creatures")),
    ("bones", "scattered bones", 0, false, 0.5, None),
    ("altar", "altar", 2, true, 3.5, None),
    ("chest", "chest", 1, true, 2.5, None),
    ("hoard", "treasure hoard", 1, false, 2.0, None),
    ("dais", "dais and throne", 1, false, 2.5, None),
    ("table", "table", 1, true, 3.0, None),
    ("cot", "cot", 0, false, 1.5, None),
    ("crate", "crates", 1, true, 4.0, None),
    ("barrel", "barrels", 1, true, 4.0, None),
    ("weapon_rack", "weapon rack", 1, true, 6.0, None),
    ("cage", "iron cage", 1, true, 7.0, None),
    ("rack", "torture rack", 1, true, 3.0, None),
    ("well", "well", 1, true, 3.0, Some("40 ft shaft to water")),
    ("trap", "pressure plate", 0, false, 0.0, Some("hidden (DC 14 Perception); 2d10 piercing darts (DC 13 Dex half)")),
    ("pit", "pit", 0, false, 0.0, Some("10 ft deep: 1d6 bludgeoning (DC 12 Dex to catch the edge)")),
    ("rubble", "rubble", 1, false, 2.0, Some("difficult terrain")),
    ("stalagmite", "stalagmite", 2, true, 6.0, None),
    ("crystal", "crystal cluster", 1, true, 4.0, None),
    ("pool", "pool", 0, false, 0.0, Some("difficult terrain; 3 ft deep")),
    ("fungus", "glowing fungus", 0, false, 1.0, Some("dim light, 10 ft")),
    ("timber", "timber prop", 1, true, 8.0, None),
    ("rail", "rails", 0, false, 0.0, None),
    ("ore_cart", "ore cart", 1, true, 4.0, None),
    ("cave_in", "unstable ceiling", 0, false, 0.0, Some("loud noise: DC 12 Dex save or 2d6 bludgeoning, then difficult terrain")),
    ("basalt", "basalt column", 3, true, 10.0, None),
    ("obsidian", "obsidian shards", 0, false, 1.0, Some("difficult terrain; 1d4 slashing on falling prone")),
    ("vent", "steam vent", 0, false, 0.0, Some("erupts on a d6 roll of 6: 2d6 fire in 10 ft (DC 13 Dex half)")),
    ("niche", "burial niche", 0, false, 1.0, None),
    ("boulder", "boulder", 3, true, 6.0, None),
    ("rock_column", "rock column", 3, true, 20.0, None),
    ("mushroom", "giant mushrooms", 1, true, 6.0, Some("struck: spores, DC 12 Con save or poisoned for 1 minute")),
    ("web", "thick webs", 0, false, 0.0, Some("difficult terrain; DC 12 Dex save or restrained (DC 12 Str to break free)")),
    ("guano", "bat guano", 0, false, 0.0, Some("difficult terrain; open wounds: DC 11 Con save or sewer plague")),
    ("skeleton", "adventurer's remains", 0, false, 0.5, None),
    ("campfire", "cold campfire", 0, false, 0.5, None),
    ("moss", "damp moss", 0, false, 0.0, Some("slippery: DC 10 Dex save when dashing or fall prone")),
    ("ore_vein", "ore vein", 0, false, 0.0, None),
    ("tools", "mining tools", 0, false, 1.0, None),
    ("lantern", "hanging lantern", 0, false, 0.0, Some("bright light 15 ft, dim 15 ft beyond")),
    ("winch", "hoist winch", 2, true, 5.0, None),
    ("powder", "blasting powder kegs", 1, true, 3.0, Some("fire or lightning: explodes, 3d6 fire in 10 ft (DC 13 Dex half)")),
    ("bedroll", "bedroll", 0, false, 0.5, None),
    ("chains", "wall shackles", 0, false, 0.0, None),
    ("sconce", "torch sconce", 0, false, 0.0, Some("bright light 20 ft, dim 20 ft beyond")),
    ("statue", "statue", 3, true, 8.0, None),
    ("fountain", "dry fountain", 2, true, 3.0, None),
    ("iron_maiden", "iron maiden", 2, true, 7.0, None),
    ("stocks", "stocks", 1, true, 3.0, None),
    ("sacks", "grain sacks", 1, true, 3.0, None),
    ("bookshelf", "bookshelf", 3, true, 7.0, None),
    ("banner", "tattered banner", 0, false, 0.0, None),
    ("rug", "faded rug", 0, false, 0.0, None),
    ("coffin", "open coffin", 1, true, 2.0, None),
    ("skulls", "skull pile", 1, true, 2.5, None),
    ("candles", "candelabrum", 0, true, 5.0, Some("dim light 10 ft")),
    ("effigy", "stone effigy", 3, true, 6.0, None),
    ("cobweb", "cobwebs", 0, false, 0.0, None),
    ("offering", "offering bowl", 0, false, 0.5, None),
    ("debris", "flotsam heap", 1, false, 2.0, Some("difficult terrain")),
    ("pipe", "drain outlet", 0, false, 0.0, None),
    ("rat_nest", "rat nest", 0, false, 1.0, Some("a swarm of rats lairs here")),
    ("ladder", "ladder to a street grate", 0, false, 0.0, None),
    ("sulfur", "sulfur crust", 0, false, 0.0, Some("disturbed: DC 12 Con save or poisoned for 1 minute")),
    ("scorched", "scorched remains", 0, false, 0.5, None),
    ("glass_pool", "cooled glassy pool", 0, false, 0.0, Some("slippery: difficult terrain")),
    ("glyph", "ritual circle", 0, false, 0.0, Some("arcane: its purpose is the DM's")),
    ("nest", "beast's nest", 0, false, 1.5, None),
    ("ice", "ice formation", 2, true, 6.0, None),
    ("ice_sheet", "sheet ice", 0, false, 0.0, Some("slippery: DC 10 Dex save on dashing or fall prone")),
    ("forge", "forge", 2, true, 4.0, Some("hot coals: 1d6 fire on falling against it")),
    ("anvil", "anvil", 1, true, 3.0, None),
    ("long_table", "long table", 1, true, 3.0, None),
    ("desk", "desk", 1, true, 3.0, None),
    ("hearth", "hearth", 1, true, 3.0, None),
    ("keg_rack", "keg rack", 1, true, 5.0, None),
    ("vat", "vat", 1, true, 5.0, None),
    ("bed", "bed", 0, false, 2.0, None),
];

/// An item of `kind` at (x, y), w × h, with its tactical values (`PROPS`; anything else is clutter).
fn item(kind: &'static str, x: usize, y: usize, w: usize, h: usize) -> Item {
    let (name, cover, blocks, height, hazard) = PROPS.iter().find(|p| p.0 == kind).map(|p| (p.1, p.2, p.3, p.4, p.5)).unwrap_or(("clutter", 0, false, 1.0, None));
    Item::new(kind, name, x, y, w, h, cover, blocks, height, hazard)
}

#[derive(Clone, Copy)]
enum Spot {
    /// Against the room's edge.
    Edge,
    /// Anywhere in the room.
    Any,
}

/// What goes in a room: (item, w, h, where, how many: min, max).
fn kit(kind: &str) -> &'static [(&'static str, usize, usize, Spot, u32, u32)] {
    use Spot::*;
    match kind {
        // Dungeons.
        "guard room" => &[("table", 1, 2, Any, 1, 1), ("weapon_rack", 1, 1, Edge, 1, 2), ("chest", 1, 1, Edge, 0, 1), ("sconce", 1, 1, Edge, 1, 2), ("sacks", 1, 1, Edge, 0, 2)],
        "barracks" => &[("cot", 1, 2, Edge, 3, 6), ("chest", 1, 1, Edge, 1, 2), ("bedroll", 1, 2, Any, 0, 2), ("sconce", 1, 1, Edge, 0, 1)],
        "storeroom" => &[("crate", 1, 1, Edge, 2, 5), ("barrel", 1, 1, Edge, 1, 4), ("sacks", 1, 1, Edge, 1, 3)],
        "powder store" => &[("powder", 1, 1, Edge, 2, 4), ("crate", 1, 1, Edge, 1, 3), ("lantern", 1, 1, Edge, 0, 1)],
        "cell block" | "prison" => &[("cage", 2, 2, Edge, 1, 3), ("chains", 1, 1, Edge, 2, 4), ("bones", 1, 1, Any, 0, 2), ("stocks", 2, 1, Any, 0, 1)],
        "torture chamber" => &[("rack", 1, 2, Any, 1, 1), ("iron_maiden", 1, 1, Edge, 0, 1), ("chains", 1, 1, Edge, 1, 3), ("brazier", 1, 1, Edge, 1, 2), ("cage", 2, 2, Edge, 0, 1)],
        "shrine" => &[("altar", 2, 1, Edge, 1, 1), ("statue", 1, 1, Edge, 0, 2), ("candles", 1, 1, Any, 1, 2), ("brazier", 1, 1, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1)],
        "chapel of rest" => &[("altar", 2, 1, Edge, 1, 1), ("candles", 1, 1, Any, 1, 3), ("offering", 1, 1, Edge, 1, 2), ("urn", 1, 1, Edge, 0, 2)],
        "armory" => &[("weapon_rack", 1, 1, Edge, 2, 5), ("crate", 1, 1, Edge, 0, 2), ("sconce", 1, 1, Edge, 0, 1)],
        "well room" => &[("well", 1, 1, Any, 1, 1), ("barrel", 1, 1, Edge, 0, 2), ("moss", 1, 1, Any, 1, 3)],
        "pit room" => &[("pit", 2, 2, Any, 1, 1), ("bones", 1, 1, Any, 1, 2), ("chains", 1, 1, Edge, 0, 2)],
        "entry hall" => &[("statue", 1, 1, Edge, 1, 2), ("banner", 1, 1, Edge, 1, 2), ("sconce", 1, 1, Edge, 1, 2), ("fountain", 2, 2, Any, 0, 1)],
        "antechamber" => &[("effigy", 1, 2, Edge, 0, 2), ("candles", 1, 1, Any, 1, 2), ("cobweb", 1, 1, Edge, 1, 2)],
        "landing" => &[("sconce", 1, 1, Edge, 1, 2), ("pillar", 1, 1, Any, 0, 2), ("bookshelf", 1, 2, Edge, 0, 1)],
        "oubliette" => &[("pit", 2, 2, Any, 1, 1), ("chains", 1, 1, Edge, 1, 3), ("bones", 1, 1, Any, 1, 2)],
        "crypt of the old lords" => &[("sarcophagus", 1, 2, Any, 2, 4), ("effigy", 1, 2, Edge, 0, 2), ("candles", 1, 1, Any, 1, 2), ("cobweb", 1, 1, Edge, 1, 3)],
        "secret vault" => &[("chest", 1, 1, Edge, 2, 4), ("hoard", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 1, 2)],
        "corridor" => &[("trap", 1, 1, Any, 0, 2), ("sconce", 1, 1, Edge, 1, 4), ("chains", 1, 1, Edge, 0, 1)],
        // Crypts and catacombs.
        "burial hall" => &[("sarcophagus", 1, 2, Any, 2, 5), ("coffin", 1, 2, Any, 0, 2), ("candles", 1, 1, Any, 1, 2), ("urn", 1, 1, Edge, 1, 3)],
        "catacomb gallery" => &[("sarcophagus", 1, 2, Any, 1, 3), ("skulls", 1, 1, Edge, 1, 3), ("cobweb", 1, 1, Edge, 1, 3), ("urn", 1, 1, Edge, 0, 2)],
        "ossuary" => &[("skulls", 1, 1, Edge, 2, 5), ("bones", 1, 1, Any, 2, 5), ("urn", 1, 1, Edge, 1, 3)],
        "tomb" => &[("sarcophagus", 1, 2, Any, 1, 1), ("effigy", 1, 2, Edge, 0, 1), ("offering", 1, 1, Edge, 1, 2), ("urn", 1, 1, Edge, 1, 3), ("chest", 1, 1, Edge, 0, 1)],
        "sealed tomb" => &[("sarcophagus", 1, 2, Any, 1, 1), ("cobweb", 1, 1, Edge, 2, 4), ("skeleton", 1, 2, Any, 0, 1), ("chest", 1, 1, Edge, 0, 1)],
        "embalming room" => &[("table", 1, 2, Any, 1, 2), ("urn", 1, 1, Edge, 1, 3), ("candles", 1, 1, Any, 0, 1), ("coffin", 1, 2, Edge, 0, 1)],
        "reliquary" => &[("altar", 2, 1, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 2), ("offering", 1, 1, Any, 1, 2), ("candles", 1, 1, Any, 1, 2)],
        "passage" | "catacomb passage" => &[("trap", 1, 1, Any, 0, 2), ("cobweb", 1, 1, Edge, 1, 3), ("skulls", 1, 1, Edge, 0, 1)],
        "stair foot" => &[("urn", 1, 1, Edge, 0, 2), ("candles", 1, 1, Any, 0, 1), ("cobweb", 1, 1, Edge, 0, 2)],
        "burial niches" => &[("niche", 1, 1, Edge, 4, 9), ("bones", 1, 1, Any, 0, 2), ("cobweb", 1, 1, Edge, 0, 2)],
        "charnel pit" => &[("pit", 2, 2, Any, 1, 1), ("skulls", 1, 1, Edge, 1, 3), ("bones", 1, 1, Any, 2, 4)],
        "bone chapel" => &[("altar", 2, 1, Edge, 1, 1), ("skulls", 1, 1, Edge, 2, 5), ("candles", 1, 1, Any, 1, 3), ("urn", 1, 1, Edge, 1, 2)],
        "flooded gallery" | "flooded hall" => &[("pool", 2, 2, Any, 1, 3), ("niche", 1, 1, Edge, 0, 3), ("coffin", 1, 2, Any, 0, 1), ("debris", 1, 1, Any, 0, 2)],
        // Sewers and their undercrofts.
        "drain chamber" => &[("pipe", 1, 1, Edge, 1, 2), ("debris", 1, 1, Any, 0, 2)],
        "culvert" => &[("trap", 1, 1, Any, 0, 1), ("debris", 1, 1, Any, 0, 2), ("rat_nest", 1, 1, Edge, 0, 1)],
        "cistern" => &[("pool", 2, 2, Any, 2, 4), ("pillar", 1, 1, Any, 1, 4), ("pipe", 1, 1, Edge, 1, 2)],
        "overflow chamber" => &[("pool", 2, 2, Any, 1, 2), ("rubble", 1, 1, Any, 1, 3), ("debris", 1, 1, Any, 1, 3), ("pipe", 1, 1, Edge, 1, 2)],
        "smugglers' cache" => &[("crate", 1, 1, Edge, 2, 5), ("barrel", 1, 1, Edge, 1, 3), ("sacks", 1, 1, Edge, 1, 3), ("chest", 1, 1, Edge, 1, 1), ("bedroll", 1, 2, Any, 0, 2), ("lantern", 1, 1, Edge, 1, 1)],
        "old vault" => &[("chest", 1, 1, Edge, 1, 3), ("pillar", 1, 1, Any, 0, 2), ("statue", 1, 1, Edge, 0, 1), ("cobweb", 1, 1, Edge, 1, 2)],
        "rat warren" => &[("rat_nest", 1, 1, Any, 2, 4), ("bones", 1, 1, Any, 2, 5), ("rubble", 1, 1, Any, 1, 3), ("debris", 1, 1, Any, 0, 2)],
        "sewer tunnel" => &[("debris", 1, 1, Edge, 3, 7), ("pipe", 1, 1, Edge, 3, 7), ("rat_nest", 1, 1, Edge, 1, 3), ("rubble", 1, 1, Edge, 1, 4), ("bones", 1, 1, Edge, 1, 3), ("barrel", 1, 1, Edge, 0, 2)],
        "service passage" => &[("debris", 1, 1, Any, 0, 1), ("lantern", 1, 1, Edge, 0, 1)],
        // Caves.
        "cavern" => &[("stalagmite", 1, 1, Any, 2, 6), ("boulder", 2, 2, Any, 0, 2), ("rock_column", 1, 1, Any, 0, 2), ("rubble", 1, 1, Any, 0, 2), ("skeleton", 1, 2, Any, 0, 1), ("web", 1, 1, Edge, 0, 1)],
        "grotto" => &[("stalagmite", 1, 1, Any, 2, 5), ("rock_column", 1, 1, Any, 0, 1), ("moss", 1, 1, Any, 1, 3), ("boulder", 2, 2, Any, 0, 1)],
        "bat roost" => &[("guano", 2, 2, Any, 1, 3), ("stalagmite", 1, 1, Any, 1, 4), ("bones", 1, 1, Any, 0, 2)],
        "crystal grotto" => &[("crystal", 1, 1, Any, 3, 7), ("boulder", 2, 2, Any, 0, 1), ("rock_column", 1, 1, Any, 0, 1)],
        "fungus grotto" => &[("fungus", 1, 1, Any, 3, 6), ("mushroom", 1, 1, Any, 2, 5), ("moss", 1, 1, Any, 1, 2)],
        "pool chamber" => &[("pool", 2, 2, Any, 1, 2), ("stalagmite", 1, 1, Any, 1, 3), ("moss", 1, 1, Any, 1, 3)],
        "ledge" => &[("stalagmite", 1, 1, Any, 0, 2), ("boulder", 2, 2, Any, 0, 1), ("campfire", 1, 1, Any, 0, 1), ("bedroll", 1, 2, Any, 0, 1)],
        // Lava tubes.
        "lava tube" | "tube gallery" => &[("basalt", 1, 1, Any, 1, 3), ("scorched", 1, 2, Any, 0, 1), ("obsidian", 1, 1, Any, 0, 2), ("rubble", 1, 1, Any, 0, 2)],
        "basalt shelf" => &[("basalt", 1, 1, Any, 1, 2), ("sulfur", 1, 1, Any, 0, 2)],
        "side pocket" | "obsidian gallery" => &[("obsidian", 1, 1, Any, 2, 4), ("basalt", 1, 1, Any, 0, 2), ("vent", 1, 1, Any, 0, 1), ("sulfur", 1, 1, Any, 1, 3), ("glass_pool", 2, 2, Any, 0, 1)],
        // Mines.
        "haulage way" => &[("ore_cart", 1, 1, Any, 1, 2), ("timber", 1, 1, Edge, 4, 10), ("lantern", 1, 1, Edge, 2, 4), ("tools", 1, 1, Edge, 0, 2)],
        "drift" => &[("timber", 1, 1, Edge, 0, 3), ("ore_vein", 1, 1, Edge, 1, 3), ("cave_in", 1, 1, Any, 0, 1), ("tools", 1, 1, Edge, 0, 1)],
        "stope" | "ore chamber" => &[("ore_vein", 1, 1, Edge, 2, 4), ("ore_cart", 1, 1, Any, 0, 1), ("rubble", 1, 1, Any, 1, 3), ("timber", 1, 1, Edge, 1, 3), ("tools", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 0, 1)],
        "collapsed stope" => &[("rubble", 1, 1, Any, 3, 6), ("boulder", 2, 2, Any, 0, 2), ("cave_in", 1, 1, Any, 1, 1), ("skeleton", 1, 2, Any, 0, 1)],
        "pump room" => &[("pool", 2, 2, Any, 1, 1), ("barrel", 1, 1, Edge, 1, 2), ("winch", 1, 1, Edge, 0, 1)],
        "shaft head" => &[("winch", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 0, 1)],
        // Prisons.
        "gatehouse" => &[("weapon_rack", 1, 1, Edge, 1, 2), ("table", 1, 1, Any, 0, 1), ("sconce", 1, 1, Edge, 1, 2), ("chest", 1, 1, Edge, 0, 1)],
        "warden's office" => &[("desk", 2, 1, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 1), ("bookshelf", 1, 2, Edge, 0, 1), ("sconce", 1, 1, Edge, 1, 1)],
        "mess hall" => &[("long_table", 1, 3, Any, 1, 2), ("barrel", 1, 1, Edge, 1, 2), ("sacks", 1, 1, Edge, 0, 2), ("sconce", 1, 1, Edge, 1, 1)],
        "interrogation room" => &[("table", 1, 1, Any, 1, 1), ("chains", 1, 1, Edge, 1, 3), ("brazier", 1, 1, Edge, 1, 1), ("cage", 2, 2, Edge, 0, 1)],
        "cell corridor" => &[("chains", 1, 1, Edge, 1, 3), ("sconce", 1, 1, Edge, 1, 3), ("trap", 1, 1, Any, 0, 1)],
        // Temples.
        "narthex" => &[("statue", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 1, 1), ("fountain", 2, 2, Any, 0, 1), ("sconce", 1, 1, Edge, 1, 1)],
        "sanctum" => &[("altar", 2, 1, Edge, 1, 1), ("statue", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 1, 3), ("brazier", 1, 1, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1), ("pillar", 1, 1, Any, 0, 2)],
        "vestry" => &[("bookshelf", 1, 2, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 0, 1), ("table", 1, 2, Any, 0, 1)],
        "priests' cells" => &[("cot", 1, 2, Edge, 2, 4), ("chest", 1, 1, Edge, 0, 2), ("candles", 1, 1, Any, 0, 1)],
        "ritual pool" => &[("pool", 2, 2, Any, 1, 2), ("candles", 1, 1, Any, 1, 2), ("glyph", 2, 2, Any, 0, 1)],
        "processional way" => &[("statue", 1, 1, Edge, 0, 2), ("sconce", 1, 1, Edge, 1, 4), ("banner", 1, 1, Edge, 0, 2), ("trap", 1, 1, Any, 0, 1)],
        // Wizards' lairs.
        "foyer" => &[("statue", 1, 1, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1), ("sconce", 1, 1, Edge, 1, 2), ("bookshelf", 1, 2, Edge, 0, 1)],
        "hall" => &[("sconce", 1, 1, Edge, 1, 3), ("banner", 1, 1, Edge, 0, 2), ("trap", 1, 1, Any, 0, 1)],
        "library" => &[("bookshelf", 1, 2, Edge, 3, 6), ("desk", 2, 1, Any, 0, 1), ("table", 1, 2, Any, 0, 1), ("candles", 1, 1, Any, 1, 2)],
        "laboratory" => &[("table", 1, 2, Any, 1, 2), ("bookshelf", 1, 2, Edge, 0, 2), ("vat", 1, 1, Edge, 0, 2), ("crystal", 1, 1, Any, 0, 1), ("glyph", 2, 2, Any, 0, 1), ("candles", 1, 1, Any, 1, 1)],
        "summoning circle" => &[("glyph", 2, 2, Any, 1, 1), ("candles", 1, 1, Any, 2, 4), ("brazier", 1, 1, Edge, 0, 2)],
        "specimen vault" => &[("cage", 2, 2, Edge, 1, 3), ("vat", 1, 1, Edge, 1, 2), ("bones", 1, 1, Any, 0, 2)],
        "study" => &[("desk", 2, 1, Edge, 1, 1), ("bookshelf", 1, 2, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1), ("candles", 1, 1, Any, 1, 1)],
        "menagerie" => &[("cage", 2, 2, Edge, 2, 3), ("bones", 1, 1, Any, 1, 3), ("sacks", 1, 1, Edge, 0, 1)],
        "golem workshop" => &[("anvil", 1, 1, Any, 1, 1), ("forge", 1, 1, Edge, 0, 1), ("table", 1, 2, Any, 1, 1), ("statue", 1, 1, Edge, 1, 2), ("tools", 1, 1, Edge, 0, 1)],
        "scrying room" => &[("fountain", 2, 2, Any, 1, 1), ("candles", 1, 1, Any, 1, 2), ("crystal", 1, 1, Edge, 0, 2), ("rug", 2, 3, Any, 0, 1)],
        // Bandits.
        "hidden entry" => &[("crate", 1, 1, Edge, 1, 2), ("barrel", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 1, 1), ("trap", 1, 1, Any, 0, 1)],
        "common room" => &[("table", 1, 2, Any, 1, 2), ("barrel", 1, 1, Edge, 1, 3), ("campfire", 1, 1, Any, 0, 1), ("bedroll", 1, 2, Any, 0, 2), ("sacks", 1, 1, Edge, 0, 2)],
        "bunk room" => &[("bedroll", 1, 2, Edge, 3, 6), ("chest", 1, 1, Edge, 1, 2), ("lantern", 1, 1, Edge, 0, 1)],
        "loot store" => &[("chest", 1, 1, Edge, 2, 4), ("crate", 1, 1, Edge, 2, 4), ("sacks", 1, 1, Edge, 1, 3), ("hoard", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 0, 1)],
        "kitchen" => &[("hearth", 1, 1, Edge, 1, 1), ("barrel", 1, 1, Edge, 1, 3), ("sacks", 1, 1, Edge, 1, 2), ("table", 1, 2, Any, 1, 1)],
        "captain's quarters" => &[("bed", 1, 2, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 2), ("desk", 2, 1, Edge, 0, 1), ("rug", 2, 3, Any, 0, 1), ("weapon_rack", 1, 1, Edge, 0, 1)],
        "lookout" => &[("crate", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 1, 1), ("bedroll", 1, 2, Any, 0, 1)],
        "tunnel" => &[("lantern", 1, 1, Edge, 0, 2), ("debris", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 0, 1)],
        // Dwarven halls.
        "gate hall" => &[("statue", 1, 1, Edge, 2, 4), ("pillar", 1, 1, Any, 2, 4), ("banner", 1, 1, Edge, 1, 2), ("sconce", 1, 1, Edge, 1, 3)],
        "great hall" => &[("long_table", 1, 3, Any, 2, 4), ("pillar", 1, 1, Any, 2, 6), ("hearth", 1, 1, Edge, 0, 1), ("banner", 1, 1, Edge, 1, 3), ("statue", 1, 1, Edge, 0, 2)],
        "forge" => &[("forge", 1, 1, Edge, 1, 2), ("anvil", 1, 1, Any, 1, 2), ("barrel", 1, 1, Edge, 1, 2), ("tools", 1, 1, Edge, 1, 2), ("crate", 1, 1, Edge, 0, 2)],
        "brewery" => &[("vat", 1, 1, Any, 2, 3), ("keg_rack", 1, 2, Edge, 1, 2), ("barrel", 1, 1, Edge, 2, 4)],
        "treasury" => &[("chest", 1, 1, Edge, 2, 5), ("hoard", 1, 1, Any, 1, 2), ("statue", 1, 1, Edge, 0, 1), ("trap", 1, 1, Any, 1, 2)],
        "ancestor hall" => &[("statue", 1, 1, Edge, 2, 5), ("sarcophagus", 1, 2, Any, 1, 3), ("candles", 1, 1, Any, 1, 2), ("brazier", 1, 1, Edge, 0, 2)],
        "mushroom farm" => &[("mushroom", 1, 1, Any, 2, 5), ("fungus", 1, 1, Any, 2, 4), ("barrel", 1, 1, Edge, 0, 1)],
        "gallery" => &[("sconce", 1, 1, Edge, 1, 3), ("statue", 1, 1, Edge, 0, 1), ("pillar", 1, 1, Any, 0, 2)],
        // Goblin warrens.
        "warren mouth" => &[("bones", 1, 1, Any, 1, 2), ("rubble", 1, 1, Any, 1, 2), ("cage", 2, 2, Edge, 0, 1)],
        "den" => &[("bedroll", 1, 2, Any, 2, 4), ("bones", 1, 1, Any, 1, 3), ("sacks", 1, 1, Edge, 0, 2), ("campfire", 1, 1, Any, 0, 1)],
        "nest" => &[("nest", 1, 1, Any, 1, 2), ("bedroll", 1, 2, Any, 1, 3), ("bones", 1, 1, Any, 1, 2)],
        "cook pit" => &[("campfire", 1, 1, Any, 1, 1), ("barrel", 1, 1, Edge, 0, 2), ("bones", 1, 1, Any, 1, 3), ("sacks", 1, 1, Edge, 0, 1)],
        "refuse pit" => &[("pit", 2, 2, Any, 1, 1), ("debris", 1, 1, Any, 2, 4), ("bones", 1, 1, Any, 1, 3)],
        "wolf pen" => &[("nest", 1, 1, Any, 1, 2), ("bones", 1, 1, Any, 2, 4), ("chains", 1, 1, Edge, 1, 2), ("cage", 2, 2, Edge, 0, 1)],
        "shaman's hut" => &[("glyph", 2, 2, Any, 0, 1), ("skulls", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 1, 2), ("bones", 1, 1, Any, 1, 1), ("offering", 1, 1, Edge, 1, 1)],
        "crawlway" => &[("rubble", 1, 1, Any, 0, 2), ("bones", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 0, 1)],
        // Flooded vaults.
        "flooded stair" => &[("pool", 2, 2, Any, 1, 2), ("debris", 1, 1, Any, 0, 2), ("moss", 1, 1, Any, 1, 2)],
        "sunken chapel" => &[("altar", 2, 1, Edge, 1, 1), ("pool", 2, 2, Any, 1, 2), ("statue", 1, 1, Edge, 0, 1), ("moss", 1, 1, Any, 1, 2)],
        "drowned barracks" => &[("cot", 1, 2, Edge, 2, 4), ("pool", 2, 2, Any, 1, 2), ("debris", 1, 1, Any, 1, 2), ("skeleton", 1, 2, Any, 0, 1)],
        "flooded passage" => &[("pool", 2, 2, Any, 0, 2), ("moss", 1, 1, Any, 1, 2), ("debris", 1, 1, Any, 0, 1)],
        // Tombs and ossuaries.
        "false tomb" => &[("sarcophagus", 1, 2, Any, 1, 1), ("trap", 1, 1, Any, 1, 3), ("pit", 2, 2, Any, 0, 1), ("cobweb", 1, 1, Edge, 1, 2)],
        "guardian hall" => &[("statue", 1, 1, Edge, 2, 4), ("pillar", 1, 1, Any, 2, 4), ("trap", 1, 1, Any, 0, 2), ("sconce", 1, 1, Edge, 0, 2)],
        "skull gallery" => &[("skulls", 1, 1, Edge, 3, 6), ("niche", 1, 1, Edge, 2, 5), ("candles", 1, 1, Any, 0, 1)],
        // Themed caves.
        "fungus forest" => &[("mushroom", 1, 1, Any, 3, 6), ("fungus", 1, 1, Any, 3, 6), ("moss", 1, 1, Any, 1, 2)],
        "spore grotto" => &[("fungus", 1, 1, Any, 2, 4), ("mushroom", 1, 1, Any, 1, 3), ("web", 1, 1, Edge, 0, 1)],
        "mycelium hall" => &[("fungus", 1, 1, Any, 4, 8), ("mushroom", 1, 1, Any, 1, 3), ("stalagmite", 1, 1, Any, 0, 2)],
        "crystal cavern" => &[("crystal", 1, 1, Any, 4, 8), ("stalagmite", 1, 1, Any, 1, 3), ("rock_column", 1, 1, Any, 0, 1)],
        "geode" => &[("crystal", 1, 1, Any, 5, 9), ("boulder", 2, 2, Any, 0, 1)],
        "crystal shelf" => &[("crystal", 1, 1, Any, 1, 3)],
        "ice cavern" => &[("ice", 1, 1, Any, 2, 5), ("ice_sheet", 2, 2, Any, 1, 3), ("stalagmite", 1, 1, Any, 0, 2)],
        "frozen pool" => &[("ice_sheet", 2, 2, Any, 2, 4), ("ice", 1, 1, Any, 1, 3)],
        "icicle gallery" => &[("ice", 1, 1, Any, 4, 7), ("ice_sheet", 2, 2, Any, 0, 2)],
        "ice shelf" => &[("ice", 1, 1, Any, 1, 2), ("ice_sheet", 2, 2, Any, 0, 1)],
        "flooded cavern" => &[("pool", 2, 2, Any, 2, 4), ("stalagmite", 1, 1, Any, 1, 3), ("moss", 1, 1, Any, 1, 3)],
        "sump" => &[("pool", 2, 2, Any, 2, 3), ("debris", 1, 1, Any, 0, 2), ("moss", 1, 1, Any, 1, 2)],
        "lair" => &[("bones", 1, 1, Any, 3, 6), ("skeleton", 1, 2, Any, 1, 2), ("nest", 1, 1, Any, 1, 2), ("boulder", 2, 2, Any, 0, 1)],
        "nest chamber" => &[("nest", 1, 1, Any, 2, 4), ("bones", 1, 1, Any, 2, 4), ("guano", 2, 2, Any, 0, 1)],
        "gnawing chamber" => &[("bones", 1, 1, Any, 3, 6), ("skeleton", 1, 2, Any, 0, 2), ("skulls", 1, 1, Any, 0, 1)],
        _ => &[],
    }
}

fn furnish(p: &mut Plan, nx: usize, ny: usize, kind: UnderKind, rng: &mut Pcg32) {
    p.reserve_doors();
    let up = p.up;
    p.taken[up] = true;
    if let Some(d) = p.down {
        p.taken[d] = true;
    }
    // Rails down the haulage way, before anything stands on it.
    if kind == UnderKind::Mine {
        let rails: Vec<usize> =
            (0..nx * ny).filter(|&k| p.cells[k] >= 0 && p.rooms[p.cells[k] as usize].kind == "haulage way" && k >= nx && p.cells[k - nx] == p.cells[k] && !p.taken[k]).collect();
        for k in rails {
            p.put(item("rail", k % nx, k / nx, 1, 1));
        }
    }
    let rooms: Vec<(i16, &'static str)> = p.rooms.iter().enumerate().map(|(i, r)| (i as i16, r.kind)).collect();
    for (ri, rkind) in rooms {
        furnish_room(p, nx, ny, ri, rkind, kind, rng);
    }
}

/// Room `ri`'s kit (a boss chamber's set piece), where it fits around what is there.
fn furnish_room(p: &mut Plan, nx: usize, ny: usize, ri: i16, rkind: &str, kind: UnderKind, rng: &mut Pcg32) {
    let squares: Vec<usize> = (0..nx * ny).filter(|&k| p.cells[k] == ri).collect();
    if squares.is_empty() {
        return;
    }
    if rkind == BOSS {
        boss_chamber(p, nx, ny, ri, &squares, kind, rng);
        return;
    }
    let edge: Vec<usize> = squares.iter().copied().filter(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] != ri) || k % nx == 0 || k / nx == 0).collect();
    for &(what, w, h, spot, lo, hi) in kit(rkind) {
        // Big rooms (a sewer's whole network, a great cavern) get a kit per 80 squares.
        let n = (lo + rng.below(hi - lo + 1)) * (squares.len() as u32 / 80).clamp(1, 12);
        let mut placed = 0;
        for _ in 0..n * 12 {
            if placed >= n {
                break;
            }
            let pool = match spot {
                Spot::Edge if !edge.is_empty() => &edge,
                _ => &squares,
            };
            let k = pool[rng.below(pool.len() as u32) as usize];
            let (w, h) = if rng.next_f64() < 0.5 { (w, h) } else { (h, w) };
            if p.place(item(what, k % nx, k / nx, w, h), Some(ri), ny) {
                placed += 1;
            }
        }
    }
}

/// Pillars in rows, a dais and throne at the far end from the way in, a hoard beside it,
/// braziers; caves get bones and a hoard instead of masonry.
fn boss_chamber(p: &mut Plan, nx: usize, ny: usize, ri: i16, squares: &[usize], kind: UnderKind, rng: &mut Pcg32) {
    let dist = p.distances(p.up, ny, None);
    let far = squares.iter().copied().max_by_key(|&k| dist[k]).unwrap_or(squares[0]);
    let (fx, fy) = (far % nx, far / nx);
    let built = !matches!(kind, UnderKind::Cave | UnderKind::LavaTube);
    if built {
        // The dais against the far end.
        for (dx, dy) in [(0isize, 0isize), (-1, 0), (0, -1), (-1, -1), (-2, 0), (0, -2), (-2, -1), (-1, -2), (1, 0), (0, 1)] {
            let (x, y) = (fx as isize + dx, fy as isize + dy);
            if x >= 0 && y >= 0 && p.place(item("dais", x as usize, y as usize, 3, 2), Some(ri), ny) {
                break;
            }
        }
        if kind != UnderKind::Mine {
            let (x0, y0) = (squares.iter().map(|k| k % nx).min().unwrap_or(0), squares.iter().map(|k| k / nx).min().unwrap_or(0));
            let (x1, y1) = (squares.iter().map(|k| k % nx).max().unwrap_or(0), squares.iter().map(|k| k / nx).max().unwrap_or(0));
            if x1 >= x0 + 5 && y1 >= y0 + 5 {
                let mut x = x0 + 2;
                while x + 2 <= x1 {
                    for y in [y0 + 1, y1 - 1] {
                        p.place(item("pillar", x, y, 1, 1), Some(ri), ny);
                    }
                    x += 3;
                }
            }
        }
        for _ in 0..30 {
            let k = squares[rng.below(squares.len() as u32) as usize];
            if p.place(item("brazier", k % nx, k / nx, 1, 1), Some(ri), ny) && rng.next_f64() < 0.4 {
                break;
            }
        }
        // Banners on the walls, a ritual circle on the floor.
        let edge: Vec<usize> = squares.iter().copied().filter(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] != ri)).collect();
        let mut banners = 0;
        for _ in 0..20 {
            if banners >= 3 || edge.is_empty() {
                break;
            }
            let k = edge[rng.below(edge.len() as u32) as usize];
            if p.place(item("banner", k % nx, k / nx, 1, 1), Some(ri), ny) {
                banners += 1;
            }
        }
        if kind != UnderKind::Mine {
            for _ in 0..20 {
                let k = squares[rng.below(squares.len() as u32) as usize];
                if p.place(item("glyph", k % nx, k / nx, 2, 2), Some(ri), ny) {
                    break;
                }
            }
        }
    } else {
        for _ in 0..8 {
            let k = squares[rng.below(squares.len() as u32) as usize];
            let what = ["bones", "skeleton", "stalagmite", "boulder", if kind == UnderKind::LavaTube { "vent" } else { "pool" }][rng.below(5) as usize];
            let (w, h) = match what {
                "boulder" => (2, 2),
                "skeleton" => (1, 2),
                _ => (1, 1),
            };
            p.place(item(what, k % nx, k / nx, w, h), Some(ri), ny);
        }
    }
    // The hoard, as near the far end as fits.
    let mut near: Vec<usize> = squares.to_vec();
    near.sort_by_key(|&k| (k % nx).abs_diff(fx) + (k / nx).abs_diff(fy));
    let mut put = 0;
    for k in near {
        if put >= 2 {
            break;
        }
        if p.place(item(if put == 0 { "hoard" } else { "chest" }, k % nx, k / nx, 1, 1), Some(ri), ny) {
            put += 1;
        }
    }
}
