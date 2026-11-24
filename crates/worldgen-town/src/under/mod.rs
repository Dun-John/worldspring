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
//! - Mines are an adit and a railed haulage way, drifts along curved ore veins with stopes, a winze and a lift shaft.
//! - Lava tubes are a meandering tube with side pockets (cave chambers grown off it); the lower tube carries a lava
//!   channel.
//! - Sewers run under a city's paved streets, section by section (`SEWER_SECTION_FT` squares
//!   of the world, each entered from a street grate): the tunnels are the ground under the
//!   streets, a sewage channel down the wider runs, service passages joining runs that meet
//!   only outside the section, an outfall where a tunnel reaches water; an undercroft below.
//! - Catacombs open from graveyards, mausoleums and catacomb houses: burial galleries.
//!
//! Levels are joined by a way down and a way up on the same square. Every site ends in a boss
//! chamber on its deepest level, a dead end: nothing else on the level is reached through it.
//! Guarantees are checked in tests/vital/underground.rs.
//!
//! Layout: this file holds the kinds, themes and sizes, the entrances and `site`, which asks each theme's planner
//! (`Theme::planner`) for a level; `plan` is the level while it is laid out (`Plan`, `finish`), `kit` the tools every
//! planner shares (digging, props, furnishing), `themes/*` one planner per kind of level, `design` the hand designer.

pub mod design;
pub mod surface;
mod boss;
pub use boss::PERILS;
mod fungi;
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
pub use crate::model::under::*;

/// Room kind of the final chamber.
pub const BOSS: &str = "boss chamber";
/// Depth between levels (ft).
const LEVEL_FT: f32 = 20.0;
/// Side (ft) of a sewer section: a city's sewers are entered section by section, each a square
/// of the world (aligned to multiples of this), so neighbouring sections line up.
pub const SEWER_SECTION_FT: f64 = 480.0;
/// How far (ft) the sewer reaches either side of a paved street's line.
pub const SEWER_REACH_FT: f64 = 6.0;
/// Squares round a section its plan looks out over (`Level::rim`, and the lines in `Level::paths`):
/// past the drawn floor's widest blur and the walls a channel looks to (`underField.ts`), so the two
/// sides of a border work out the same floor and channel.
pub const SEWER_RIM: usize = 5;

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

/// Iron drain grates in the paved streets over sewer section `origin` (corner, world ft): each
/// in the gutter of a street's paving (1.25 ft in from its edge, one side or the other by a
/// hash; on the street's line where the paving is too narrow for the gutter's square and the
/// squares round it to be paved), with the street's way (unit), every 55 ft or so
/// along it from a phase of its own (a hash of its ends: the same seen from any section), at
/// most six, 40 ft clear of each other and of the street grates down, over dry ground, clear
/// of towers and gates, of a junction with another street, of buildings, bridges and piers.
/// From the layout alone: the sewer section lights the floor under them
/// and the battlemap draws them in the street, each working it out for itself. Cities only.
pub fn drain_grates(t0: &T0, l: &Layout, origin: P) -> Vec<(P, P)> {
    if l.site || l.tier < crate::t0::settle::Tier::City {
        return Vec::new();
    }
    let s = SEWER_SECTION_FT;
    let inside = |p: P| p[0] >= origin[0] + 15.0 && p[0] < origin[0] + s - 15.0 && p[1] >= origin[1] + 15.0 && p[1] < origin[1] + s - 15.0;
    let ways: Vec<P> = l.entrances.iter().filter(|e| e.kind == UnderKind::Sewer).map(|e| e.at).collect();
    let towers = crate::interior::towers(l);
    let clear = |q: P| towers.iter().all(|&(t, r, gate)| town::geom::dist(q, t) >= r + if gate { 22.0 } else { 5.0 });
    let segs = street_segments(l);
    // The squares the battlemap paves for the streets: centre within the street's half width.
    let paved = |c: P| segs.iter().any(|(a, b, h)| town::geom::seg_dist(c, *a, *b) < *h);
    let square_c = |q: P| [crate::core::floor(q[0] / SQUARE_FT) * SQUARE_FT + 0.5 * SQUARE_FT, crate::core::floor(q[1] / SQUARE_FT) * SQUARE_FT + 0.5 * SQUARE_FT];
    // Under a building, a bridge or a pier (a deck, not paving), on a plank quay or on a wall.
    let off_ground = |q: P| {
        l.walls.iter().any(|w| w.windows(2).any(|e| town::geom::seg_dist(q, e[0], e[1]) < 8.0))
            || l.buildings.iter().any(|b| town::geom::contains(&b.poly, q) || (0..b.poly.len()).any(|k| town::geom::seg_dist(q, b.poly[k], b.poly[(k + 1) % b.poly.len()]) < 2.0))
            || l.bridges.iter().chain(&l.piers).any(|d| town::geom::contains(d, q) || (0..d.len()).any(|k| town::geom::seg_dist(q, d[k], d[(k + 1) % d.len()]) < 3.0))
            || l.quays.iter().filter(|k| k.timber).any(|k| town::geom::contains(&k.band(), q))
    };
    const STEP: f64 = 55.0;
    let mut out: Vec<(P, P)> = Vec::new();
    for (si, &(a, b, half)) in segs.iter().enumerate() {
        if a[0].max(b[0]) < origin[0] || a[0].min(b[0]) > origin[0] + s || a[1].max(b[1]) < origin[1] || a[1].min(b[1]) > origin[1] + s {
            continue;
        }
        let len = town::geom::dist(a, b);
        if len < 20.0 {
            continue;
        }
        let dir = mul(sub(b, a), 1.0 / len);
        let norm = [-dir[1], dir[0]];
        let key = (crate::core::round(a[0] + b[0]) as i64, crate::core::round(a[1] + b[1]) as i64);
        let phase = crate::core::rng::unit(hash3(l.roof_seed ^ 0xd7a1, key.0, key.1, 0x96a7)) * STEP;
        let mut t = phase.max(8.0);
        let mut n = 0i64;
        while t < len - 8.0 {
            let on_line = add(a, mul(dir, t));
            t += STEP;
            n += 1;
            let side = if crate::core::rng::unit(hash3(l.roof_seed ^ 0x6c37, key.0, key.1, n)) < 0.5 { -1.0 } else { 1.0 };
            // The gutter if its square and the four beside it are paved (the paving's edge is
            // ragged to a square), else the street's line, if its square is.
            let gutter = half - 1.25;
            let spot = [gutter, 0.0].into_iter().find_map(|o| {
                let q = add(on_line, mul(norm, side * o));
                let c = square_c(q);
                let ring = o == 0.0 || [[SQUARE_FT, 0.0], [-SQUARE_FT, 0.0], [0.0, SQUARE_FT], [0.0, -SQUARE_FT]].iter().all(|d| paved(add(c, *d)));
                (ring && paved(c) && paved(q)).then_some(q)
            });
            let Some(q) = spot else { continue };
            let apart = |o: &P| town::geom::dist(*o, q) >= 40.0;
            // Not on another street's paving (a junction's open centre), 2 ft clear of it.
            let junction = segs.iter().enumerate().any(|(sj, (a2, b2, h2))| sj != si && town::geom::seg_dist(q, *a2, *b2) < *h2 + 2.0);
            if out.len() >= 6 || !inside(q) || !ways.iter().all(apart) || !out.iter().all(|(o, _)| apart(o)) || !clear(q) || junction || off_ground(q) || wet(t0, q) {
                continue;
            }
            out.push((q, dir));
        }
    }
    out
}

/// Ways underground in a city: a sewer grate on a street in every section the paved streets
/// run far enough into (never under a tower or in a gate's passage, nor by a palace's
/// forecourt), catacomb stairs in each graveyard and beside each mausoleum and catacomb house.
/// (`_seed`: none of it drawn at random since Roadmap 7, 193.)
pub fn city_entrances(l: &Layout, t0: &T0, _seed: u64) -> Vec<Entrance> {
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
    for b in &l.buildings {
        let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
        if let Some(lp) = town::cemetery::lot(&l, b) {
            // (Roadmap 7, 193) The stair on the lot's walk, square to its way in, its mouth toward
            // the gate and the site running in under the lot from it; a mausoleum over it where one
            // fits inside the wall (`cemetery::lot`).
            out.push(Entrance { at: lp.stair, dir: lp.inward, kind: UnderKind::Catacombs, id: 0 });
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
    let made = layout.checked_sub(t0.settlements.len()).and_then(|p| t0.created_site(p));
    let spec = made.map(|c| c.spec).unwrap_or_default();
    // (A ruin: no roof standing over it, out in the wilds.)
    let ruin = !town::is_settlement(t0, layout) && l.buildings.iter().all(|b| b.structure == town::Structure::Ruin);
    let age = if made.is_some() { Age::Made } else if ruin { Age::Ruin } else { Age::Living };
    let (size, depth, theme) = resolve(e.kind, spec, ruin, t0.cold_of_layout(world, layout), &mut rng);
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
    let caves = (age, hash3(world.stream("under"), town::rng_key(t0, layout), k as i64, 0xb4ea));
    let mut it = site(t0, &l, e.kind, format!("u:{layout}:{k}"), (layout as u32, k as u32), frame, depth, Some(entry.1 * nx + entry.0), surface, (theme, s * s), caves, &[], &mut rng)?;
    if e.kind == UnderKind::Cave {
        it.look = look_of(t0, e.at);
    }
    crate::sitelinks::dress(world, t0, layout, &mut it);
    Some(it)
}

/// A cave's look from the surface biome at its mouth (`Interior::look`): 1 green (rainforest, temperate forest, jungle,
/// swamp), 2 dry (deserts, savanna, salt flat), 3 cold (tundra, taiga), 0 today's rock (the rest).
pub fn look_of(t0: &T0, at: [f64; 2]) -> u8 {
    use crate::t0::biome::Biome;
    let (w, h) = (t0.biome.w, t0.biome.h);
    let (i, j) = (((at[0] / t0.cell_ft).round().max(0.0) as usize).min(w - 1), ((at[1] / t0.cell_ft).round().max(0.0) as usize).min(h - 1));
    match Biome::from_u8((t0.biome.get(i, j) & 0xff) as u8) {
        Biome::TemperateRainforest | Biome::TemperateForest | Biome::Jungle | Biome::Swamp => 1,
        Biome::HotDesert | Biome::ColdDesert | Biome::Savanna | Biome::SaltFlat => 2,
        Biome::Tundra | Biome::Taiga => 3,
        _ => 0,
    }
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
    let caves = (Age::Living, hash3(world.stream("under.sewer"), town::rng_key(t0, layout), sx, sy ^ 0xb4ea));
    // (R3: and the cellars of the buildings drawn by hand part of the city: `member_links`.)
    let members = member_links(world, t0, &l);
    let mut it = site(t0, &l, UnderKind::Sewer, format!("w:{layout}:{sx}:{sy}"), (layout as u32, 0), frame, 2, grate, surface, (&UNDERCROFT, 1.0), caves, &members, &mut rng)?;
    it.stone = sewer_stone(&l);
    Some(it)
}

/// The stonework of a city's sewer walkways (`Interior::stone`), the same in every section: bits
/// 0-1 the kind (0 running-bond brick, 1 dressed flags, 2 rough rubble), bit 2 warm stone (buff
/// brick, sandstone). By the layout's culture (`town::roof`: 0 north, 1 west, 2 south, 3 dry,
/// 4 steppe) and tier: small towns lay rubble, the great cities dressed stone.
pub fn sewer_stone(l: &Layout) -> u8 {
    let culture = l.culture % 5;
    let (brick, flags, rubble) = match culture {
        0 => (2, 6, 3),
        1 => (7, 3, 1),
        2 => (5, 4, 1),
        3 => (1, 6, 3),
        _ => (3, 2, 5),
    };
    let tier = l.tier as u32;
    let rubble = match tier {
        0 | 1 => rubble * 2,
        2 => rubble,
        _ => 0,
    };
    let flags = flags + if tier >= 3 { 3 } else { 0 };
    let x = crate::core::rng::unit(hash3(l.roof_seed, 0x5e57, 0, 0x7e3a)) * (brick + flags + rubble) as f64;
    let kind = if x < brick as f64 { 0 } else if x < (brick + flags) as f64 { 1 } else { 2 };
    kind | if matches!(culture, 2 | 3) { 4 } else { 0 }
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
    link_point(t0, l, l.index as usize, b)
}

/// R3: where the cellar of building `b`, drawn by hand on layout `ml` and part of city `tl`
/// (`town::members`), meets `tl`'s sewers: as `sewer_link_of`, on `tl`'s paved streets within
/// reach of it (else none: no sewer is dug out to the outskirts for it).
pub fn member_link(t0: &T0, tl: &Layout, ml: &Layout, b: &town::Building) -> Option<P> {
    if tl.site || tl.tier < crate::t0::settle::Tier::City {
        return None;
    }
    link_point(t0, tl, ml.index as usize, b)
}

/// The cellars of the buildings drawn by hand part of city `l` (`town::members`) that meet its
/// sewers: (where, `b:<layout>:0`). (The sewers read the drawn things, parent → child, as the
/// cellars read the city; neither is read by a town's layout.)
pub fn member_links(world: &World, t0: &T0, l: &Layout) -> Vec<(P, String)> {
    if l.site || l.tier < crate::t0::settle::Tier::City || world.file.edits.created.is_empty() {
        return Vec::new();
    }
    let first = t0.settlements.len() + t0.base_pois;
    town::members::members_of(world, t0, l.index as usize)
        .into_iter()
        .filter(|m| world.file.edits.created.get(m.0 - first).is_some_and(|c| c.kind == "building"))
        .filter_map(|(li, _)| {
            let ml = town::layout(world, t0, li);
            Some((member_link(t0, l, &ml, ml.building(0)?)?, format!("b:{li}:0")))
        })
        .collect()
}

/// The cellar of building `b` of drawn layout `l` where it meets the sewers of the city it is
/// part of: (where, the city's layout).
pub fn member_link_of(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<(P, usize)> {
    let c = l.index.checked_sub((t0.settlements.len() + t0.base_pois) as u32).and_then(|k| world.file.edits.created.get(k as usize))?;
    if c.kind != "building" {
        return None;
    }
    let o = town::members::owner(world, t0, l.index as usize).filter(|o| town::is_settlement(t0, o.town))?;
    Some((member_link(t0, &town::layout(world, t0, o.town), l, b)?, o.town))
}

/// Where building `b` (rolled by layout `owner`'s key) meets the sewers of streets layout `l`.
fn link_point(t0: &T0, l: &Layout, owner: usize, b: &town::Building) -> Option<P> {
    // (R5 towns: nor a steppe town's round felt hall, of one storey with no cellar; R5 M20 nor a
    // house on a deck over the water.)
    if b.structure != Structure::Roofed || town::roof::felt_built(b).is_some() || b.deck.is_some() {
        return None;
    }
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
    let chosen = key.is_some_and(|k| SEWER_CELLARS.contains(&k)) || (b.func.is_none() && crate::core::rng::hash2(town::rng_key(t0, owner) as u64 ^ 0x5e3e, b.id as i64, 7) % 10 == 0);
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
    let caves = (Age::Living, hash3(world.stream("under.keep"), town::rng_key(t0, layout), bi as i64, 0xb4ea));
    site(t0, &l, UnderKind::KeepDungeon, id, (layout as u32, bi as u32), frame, depth, Some(entry), surface, (&KEEP_DUNGEON, 1.0), caves, &[], &mut rng)
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
    (age, caves): (Age, u64),
    members: &[(P, String)],
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
            _ if kind == UnderKind::Sewer && d == 0 => sewer(t0, l, f.origin, nx, ny, arrive, last, members)?,
            Planner::Mixed => mixed(nx, ny, a, d, last, theme, area, rng),
            Planner::Dwarven => dwarven(nx, ny, a, d, last, theme, area, rng),
            Planner::Wizard => wizard(nx, ny, a, d, last, theme, area, rng),
            Planner::Warren => warren(nx, ny, a, d, last, theme, area, rng),
            Planner::Bandit => bandit(nx, ny, a, d, last, theme, area, rng),
            Planner::Cult => cult(nx, ny, a, d, last, theme, area, rng),
            Planner::Labyrinth => labyrinth(nx, ny, a, d, last, theme, area, rng),
            Planner::Fortress => fortress(nx, ny, a, d, last, theme, area, rng),
            Planner::Temple => temple(nx, ny, a, d, last, theme, area, rng),
            Planner::Prison => prison(nx, ny, a, d, last, theme, area, rng),
            // (A keep's dungeons are big when three levels deep.)
            Planner::Keep => keep(nx, ny, a, d, last, theme, if depth >= 3 { 1.69 } else { 1.0 }, rng),
            Planner::Undercroft => undercroft(nx, ny, a, last, theme, rng),
            Planner::Crypt => crypt(nx, ny, a, d, last, theme, area, rng),
            Planner::Tomb => tomb(nx, ny, a, d, last, theme, area, rng),
            Planner::Vault => vault(nx, ny, a, d, last, theme, area, rng),
            Planner::Cave => cave(nx, ny, a, last, theme, rng),
            Planner::Mine => mine(nx, ny, a, d, last, plans.last().and_then(|q| q.lift), rng),
            Planner::LavaTube => lava_tube(nx, ny, a, d, last, theme, rng),
        };
        // The ways in: the surface on the top level (if it opens there), the way up below.
        let here = p.up;
        if d == 0 && kind == UnderKind::KeepDungeon {
            // Up into the keep's cellar; and an old escape tunnel out to the sewers.
            p.put(Item::new("up", "stairs up to the keep", here % nx, here / nx, 1, 1, 0, false, 0.0, None));
            p.links.push(Link { x: (here % nx) as u16, y: (here / nx) as u16, to: format!("b:{settlement}:{building}"), id: String::new(), far: None });
            if let Some(q) = l.building(building as usize).and_then(|b| keep_escape(t0, l, b)) {
                let g = sub(q, f.origin);
                let at = [crate::town::geom::dot(g, f.axis) / SQUARE_FT, crate::town::geom::dot(g, f.across) / SQUARE_FT];
                let free = (0..nx * ny).filter(|&k| p.cells[k] >= 0 && !p.taken[k]).min_by(|&a, &b| {
                    let dd = |k: usize| ((k % nx) as f64 + 0.5 - at[0]).powi(2) + ((k / nx) as f64 + 0.5 - at[1]).powi(2);
                    dd(a).total_cmp(&dd(b))
                });
                if let Some(k) = free {
                    p.put(Item::new("tunnel", "escape tunnel to the sewers", k % nx, k / nx, 1, 1, 0, false, 0.0, Some("hidden: DC 15 Perception to find")));
                    p.links.push(Link { x: (k % nx) as u16, y: (k / nx) as u16, to: sewer_id(settlement as usize, q), id: String::new(), far: None });
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
        // A cave-in by age (from a hash of the site, not the rng: sites without one come out as before). Under a ruin
        // about half the levels, under a living place one site in ten (on one of its levels; a city's undercroft, not
        // its sewers); never in a made site, a labyrinth, a collapsed fortress (its own), caves, mines or lava tubes.
        let level = |salt: i64| crate::core::rng::unit(hash3(caves, d as i64, salt, 0xb4ea));
        let chance = match age {
            Age::Ruin => level(0) < 0.5,
            Age::Living => crate::core::rng::unit(hash3(caves, 0, 1, 0xb4ea)) < 0.1 && (if kind == UnderKind::Sewer { d == 1 } else { (caves % depth as u64) as usize == d }),
            Age::Made => false,
        };
        if chance && !matches!(theme.planner, Planner::Labyrinth | Planner::Fortress | Planner::Cave | Planner::Mine | Planner::LavaTube) && !(kind == UnderKind::Sewer && d == 0) {
            let r = 4.5 * crate::core::sqrt(area).clamp(0.8, 1.3) * (0.85 + 0.3 * level(2));
            breach(&mut p, ny, hash3(caves, d as i64, 3, 0xb4ea), r);
        }
        if p.passage.is_some() {
            // (A keep's dungeons are big when three levels deep.)
            let size = if kind == UnderKind::KeepDungeon { if depth >= 3 { 1.69 } else { 1.0 } } else { area };
            let (first, lk) = locks.get_or_insert_with(|| (d, Locks::new(size, rng)));
            guard(&mut p, &mut plans[*first..], nx, ny, d, depth, lk, rng);
        }
        // (Raised floors reached without a climb before water and a chasm keep every square reached.)
        p.ramp_ledges(ny);
        if !p.water.is_empty() {
            // (Its planner's water, once the locks and keys are in: off doorways, every square still reached.)
            p.reserve_doors();
            let (w, seed) = (std::mem::take(&mut p.water), hash3(rng.next_u32() as u64, d as i64, 0, 0x3a7e));
            let span = if rng.next_f64() < 0.5 { Span::Bridge } else { Span::Stones };
            let at = |k: usize| (w[k][0] as f64, w[k][1] as f64);
            // (Lava: stone bridges and stepping stones over it.)
            let items = if p.molten { lava_at(&Ground::of_plan(&p), seed, span, &at) } else { water_at(&Ground::of_plan(&p), seed, span, &at) };
            p.put_all(items);
        }
        if let Some((r, rope)) = p.chasm {
            // (A chasm across a room (a cave's: across the level), once the locks and keys are in: crossed by rope bridges
            // or its stone bridge.)
            p.reserve_doors();
            let seed = hash3(rng.next_u32() as u64, d as i64, 0, 0xc4a5);
            let items = chasm_in(&Ground::of_plan(&p), (r >= 0).then_some(r), Span::Bridge, seed, rope);
            p.put_all(items);
            // (The stone bridge's deck: its squares with the chasm beside them, end to end.)
            let gulf: crate::core::hash::FastSet<usize> = p.items.iter().filter(|f| f.kind == "chasm").flat_map(|f| (f.y as usize..(f.y + f.h) as usize).flat_map(move |j| (f.x as usize..(f.x + f.w) as usize).map(move |i| j * nx + i))).collect();
            let over: Vec<usize> = p.bridge.iter().copied().filter(|&k| neighbours(k, nx, ny).any(|m| gulf.contains(&m))).collect();
            if let (Some(&a), Some(&b)) = (over.iter().min(), over.iter().max()) {
                let (x0, x1, y0, y1) = (a % nx, b % nx, a / nx, b / nx);
                p.put(item("stone_bridge", x0.min(x1), y0.min(y1), x0.abs_diff(x1) + 1, y0.abs_diff(y1) + 1));
            }
        }
        p.theme = theme.key;
        furnish(&mut p, nx, ny, kind, rng);
        // (Every raised floor reached without a climb.)
        p.ramp_ledges(ny);
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
        stone: 0,
        look: 0,
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
        yard: Vec::new(),
    })
}
