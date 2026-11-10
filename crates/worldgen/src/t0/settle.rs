//! Settlements and points of interest at continent scale.
//!
//! Sites are scored for habitability (fertile biome, flat ground, fresh water, sheltered
//! harbours, river confluences; penalized at altitude) and placed tier by tier (metropolis →
//! city → town → village) with spacing, like central places. Cities weigh the water most; towns
//! and villages less, and their inland pulls too (springs, farmland, market spacing). Each gets a character (port,
//! river town, mining town, fortress, market, farming/fishing/lumber/herding village, oasis)
//! from its surroundings. Ruins and wizard towers are scattered in the wilds.

use serde::Serialize;

use super::biome::Biome;
use super::flood::neighbors;
use super::hydro::{Hydro, Mouth, NO_LAKE};
use crate::World;
use crate::core::noise::{fbm, smoothstep};
use crate::core::rng::{Pcg32, hash3, unit};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Village = 0,
    Town,
    City,
    Metropolis,
}

impl Tier {
    pub fn from_u8(v: u8) -> Tier {
        [Tier::Village, Tier::Town, Tier::City, Tier::Metropolis][(v as usize).min(3)]
    }
    pub fn name(self) -> &'static str {
        ["village", "town", "city", "metropolis"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettleKind {
    Port = 0,
    River,
    Mining,
    Fortress,
    Market,
    Farming,
    Fishing,
    Lumber,
    Herding,
    Oasis,
}

impl SettleKind {
    pub fn from_u8(v: u8) -> SettleKind {
        use SettleKind::*;
        [Port, River, Mining, Fortress, Market, Farming, Fishing, Lumber, Herding, Oasis][(v as usize).min(9)]
    }
    pub fn name(self) -> &'static str {
        ["port", "river town", "mining town", "fortress", "market town", "farming", "fishing", "lumber", "herding", "oasis"][self as usize]
    }
    /// A pin's `kind` (`world::PIN_KINDS`, in this order).
    pub fn parse(key: &str) -> Option<SettleKind> {
        crate::world::PIN_KINDS.iter().position(|&k| k == key).map(|i| SettleKind::from_u8(i as u8))
    }
}

/// Why a place stands where it does (the strongest of the site's reasons; step 5's economy reads it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    None = 0,
    /// Sheltered coast.
    Harbour,
    /// A big river meets the sea or a lake.
    Mouth,
    /// A river leaves a lake.
    Outlet,
    Confluence,
    /// Where a river becomes too small for boats.
    Head,
    /// The lowest point a big river can be bridged, or a ford or bridge on a road.
    Crossing,
    Ore,
    Timber,
    Fishing,
    /// The top of a pass on a king's road (towns).
    Pass,
    /// A road junction (towns).
    Crossroads,
    /// Farmland within reach.
    Food,
    /// A spring or wells: low, flat ground at the foot of higher ground, away from rivers and shores.
    Spring,
    /// A market far from the cities (towns): the farms round buy and sell there.
    Market,
}

impl Reason {
    pub fn from_u8(v: u8) -> Reason {
        use Reason::*;
        [None, Harbour, Mouth, Outlet, Confluence, Head, Crossing, Ore, Timber, Fishing, Pass, Crossroads, Food, Spring, Market][(v as usize).min(14)]
    }
    pub fn name(self) -> &'static str {
        ["none", "harbour", "mouth", "outlet", "confluence", "head of navigation", "crossing", "ore", "timber", "fishing", "pass", "crossroads", "food", "spring", "market"][self as usize]
    }
}

#[derive(Clone, Debug)]
pub struct Settlement {
    pub tier: Tier,
    pub kind: SettleKind,
    pub reason: Reason,
    /// T0 cell index and world position (ft).
    pub cell: usize,
    pub x: f64,
    pub y: f64,
    pub population: u32,
    pub coastal: bool,
    pub river: bool,
    pub capital: bool,
    /// Per-settlement seed for its layout.
    pub seed: u64,
    /// Naming culture (set with the gazetteer).
    pub culture: u8,
    /// The sketch stroke that pinned it, if any.
    pub pin: Option<u32>,
    /// The tier and population it was planned with (`roads::link_edges` and `route` read these, so
    /// the economy's re-tiering routes nothing again; a loaded world has its final ones here).
    pub planned: Tier,
    pub planned_pop: u32,
    /// Part of an isolated realm (`place_realms`): its walled town or city, or one of its hidden
    /// villages. No trade, ranking or roads but its own track.
    pub isolated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PoiKind {
    Ruin = 0,
    Tower,
    Camp,
    Waystation,
    /// Underground entrances (M6): a cave mouth on rugged ground or a sea cliff, a mine adit
    /// in the hills near a settlement, a lava-tube skylight on a volcano's flank.
    Cave,
    Mine,
    LavaTube,
    /// Created only: a bare way underground (a stairwell, cave mouth or grate).
    Entrance,
    /// Created only: a building drawn by hand (`world::Created::poly`).
    Building,
    /// Created only: a castle drawn by hand (its outline, `world::Created::poly`).
    Castle,
    /// Created only: a wall drawn by hand (its line, `world::Created::pts`).
    Wall,
}

impl PoiKind {
    /// A created site's or site stroke's kind (a ruin when unknown).
    pub fn parse(key: &str) -> PoiKind {
        match key {
            "tower" => PoiKind::Tower,
            "camp" => PoiKind::Camp,
            "waystation" => PoiKind::Waystation,
            "cave" => PoiKind::Cave,
            "mine" => PoiKind::Mine,
            "lava_tube" => PoiKind::LavaTube,
            "entrance" => PoiKind::Entrance,
            "building" => PoiKind::Building,
            "castle" => PoiKind::Castle,
            "wall" => PoiKind::Wall,
            _ => PoiKind::Ruin,
        }
    }

    pub fn from_u8(v: u8) -> PoiKind {
        const ALL: [PoiKind; 11] = [PoiKind::Ruin, PoiKind::Tower, PoiKind::Camp, PoiKind::Waystation, PoiKind::Cave, PoiKind::Mine, PoiKind::LavaTube, PoiKind::Entrance, PoiKind::Building, PoiKind::Castle, PoiKind::Wall];
        ALL[(v as usize).min(ALL.len() - 1)]
    }
}

#[derive(Clone, Debug)]
pub struct Poi {
    pub kind: PoiKind,
    pub x: f64,
    pub y: f64,
    pub seed: u64,
    /// The sketch stroke that placed it, if any (`world::SketchTool::Site`).
    pub stroke: Option<u32>,
}

pub struct Inputs<'a> {
    pub world: &'a World,
    pub w: usize,
    pub h: usize,
    pub cell_ft: f64,
    pub height: &'a [f64],
    pub land: &'a [bool],
    pub biome: &'a [u32],
    pub hydro: &'a Hydro,
    /// Settlements the sketch places (before any of their tier are chosen).
    pub pins: &'a [super::sketch::Pin],
    /// What each cell offers, worked out on the first `place` (it depends on nothing else here).
    pub ground: std::cell::OnceCell<Ground>,
}

/// Per cell: how good a site it is, and what water it stands by.
pub struct Ground {
    /// How good a site each cell is: `base` (fertile, flat, low) times its water (`water`, 0 to ~2.4:
    /// a sheltered coast 1, a river 0.5-1, a lake 0.6, a mouth up to 1.4 more) as `0.35 + water`, the
    /// weight cities give it; towns and villages weigh the water less (`place`).
    score: Vec<f64>,
    base: Vec<f32>,
    water: Vec<f32>,
    /// A spring's strength, 0-1 (`Reason::Spring`).
    pub(crate) spring: Vec<f32>,
    coastal: Vec<bool>,
    lakeside: Vec<bool>,
    pub(crate) on_river: Vec<bool>,
    /// Only sea and lake mouths make a harbour (a confluence is a river town).
    harbour: Vec<bool>,
    /// Per cell, the strength (0-255) of each site reason `Reason::Harbour..=Fishing` (index = the reason).
    pub(crate) site: Vec<[u8; 10]>,
    /// Farmland within a day's reach, 0-1.
    pub(crate) food: Vec<f32>,
    /// The strongest break-of-bulk or crossing reason, and the strongest industry, 0-1.
    pub(crate) bulk: Vec<f32>,
    pub(crate) industry: Vec<f32>,
    /// Where the river reasons (but crossings) come from: reason, cell, the cell before it on its river (a
    /// mouth's last land cell; else the cell again) and the river's strength there, 0-1.
    sources: Vec<(Reason, u32, u32, f32)>,
    /// The land masses: the mainland and the islands (`islands::label`).
    pub(crate) islands: super::islands::Islands,
}

impl Ground {
    /// Per cell: on a coast, a lake shore or a river.
    pub fn waterside(&self) -> Vec<bool> {
        (0..self.score.len()).map(|k| self.coastal[k] || self.lakeside[k] || self.on_river[k]).collect()
    }
}

/// What a village a cell off a previewed road (not on it) gains of the road's bonus.
const ROAD_NEAR: f64 = 0.6;
/// And on it: a village on the road itself needs no spur.
const ROAD_HERE: f64 = 1.4;
/// Timber away from a river (forest only), against 0.6 by one.
const TIMBER_INLAND: f64 = 0.4;
/// A spring's ground: the ground round rising this much (ft, mean over two cells) above it, from
/// none to full, then fading out again (mountain valleys).
const SPRING_RISE_FT: (f64, f64, f64, f64) = (40.0, 250.0, 600.0, 1_200.0);
/// A spring's ground lies at the foot: its height above the lowest within two cells (ft), from
/// full to none.
const SPRING_LOW_FT: (f64, f64) = (120.0, 400.0);
/// A spring's strength as a reason (`Reason::Spring`).
const SPRING_REASON: f64 = 0.7;
/// Market spacing (towns): the miles from the nearest city (or town placed before) at which a
/// site starts to gain and gains fully, and a market's strength as a reason.
const MARKET_MI: (f64, f64) = (45.0, 100.0);
const MARKET_REASON: f64 = 0.7;
/// How much of its water a tier weighs: `wet.0 + wet.1 * water` (cities `0.35 + water`).
const TOWN_WET: (f64, f64) = (0.5, 0.6);
const VILLAGE_WET: (f64, f64) = (0.55, 0.55);
/// Inland pulls for towns and villages: a spring (`1 + s * spring`), farmland (`f.0 + f.1 * food`)
/// and, for towns, market spacing (`1 + m * gap`).
const SPRING_GAIN: f64 = 1.0;
const FOOD_GAIN: (f64, f64) = (0.6, 0.8);
const MARKET_GAIN: f64 = 0.6;

/// How strong a market is `d` cells of `cell` ft from the nearest city: 0-1.
pub(crate) fn market_gap(d: f64, cell: f64) -> f64 {
    smoothstep(MARKET_MI.0 * 5280.0 / cell, MARKET_MI.1 * 5280.0 / cell, d)
}

pub(crate) fn fertility(b: Biome) -> f64 {
    match b {
        Biome::Grassland => 1.0,
        Biome::TemperateForest => 0.8,
        Biome::Savanna => 0.7,
        Biome::Steppe => 0.6,
        Biome::TemperateRainforest => 0.6,
        Biome::Jungle => 0.45,
        Biome::Taiga => 0.45,
        Biome::Swamp => 0.2,
        Biome::ColdDesert => 0.2,
        Biome::HotDesert => 0.12,
        Biome::Tundra => 0.15,
        Biome::Alpine => 0.1,
        Biome::Blight => 0.12,
        Biome::Ashland => 0.1,
        _ => 0.0,
    }
}

/// The per-cell context `place` reads (see `Ground`).
fn ground(inp: &Inputs) -> Ground {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let p = inp.world.params();
    let sea = p.sea_level_ft;
    let n = w * h;
    let big_river = 900_000.0;
    let river_q = super::hydro::river_q(p.river_density);
    // Per-cell context.
    let biome_of = |k: usize| Biome::from_u8((inp.biome[k] & 0xff) as u8);
    let slope_of = |k: usize| super::biome::local_slope(w, h, inp.height, k % w, k / w, cell);
    let mut score = vec![0.0f64; n];
    let mut base = vec![0.0f32; n];
    let mut water_at = vec![0.0f32; n];
    let mut coastal = vec![false; n];
    let mut lakeside = vec![false; n];
    let mut on_river = vec![false; n];
    // Where rivers meet the sea or a lake, and where they join: the best town sites.
    let mut mouth = vec![0.0f64; n];
    // Only sea and lake mouths make a harbour (a confluence is a river town).
    let mut harbour = vec![false; n];
    let mut site = vec![[0u8; 10]; n];
    let mut sources: Vec<(Reason, u32, u32, f32)> = Vec::new();
    // Raises one reason's strength round a cell (within 1 cell: the river's bank).
    let splat = |site: &mut Vec<[u8; 10]>, at: usize, reason: Reason, strength: f64, reach: i64| {
        let (mx, my) = ((at % w) as i64, (at / w) as i64);
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (x, y) = (mx + dx, my + dy);
                if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                    let fall = 1.0 - (crate::core::sqrt((dx * dx + dy * dy) as f64) / (reach as f64 + 1.0)).min(1.0);
                    let v = (strength * fall * 255.0) as u8;
                    let e = &mut site[y as usize * w + x as usize][reason as usize];
                    *e = (*e).max(v);
                }
            }
        }
    };
    // A king's road bridges up to 250 ft.
    let bridge_q = super::hydro::RIVER_Q * libm::pow(250.0 / 14.0, 1.0 / 0.6);
    // Where tributaries join (the cells their chains end on).
    let mut joined = vec![false; n];
    for r in &inp.hydro.rivers {
        if r.mouth == Mouth::Confluence
            && let (Some(&last), Some(&q)) = (r.cells.last(), r.q.last())
            && q as f64 >= river_q * 3.0
        {
            joined[last as usize] = true;
        }
    }
    // Boats go up the world's big river systems (those reaching the sea or a lake) as far as a
    // river carries a good share of what its greater systems carry at their mouths: a threshold
    // from the world's own discharges, not a fixed one, so most of them have a head.
    let mut mouths: Vec<f64> = inp.hydro.rivers.iter().filter(|r| matches!(r.mouth, Mouth::Ocean | Mouth::Lake)).map(|r| r.q.iter().fold(0.0f32, |a, &b| a.max(b)) as f64).filter(|&q| q >= big_river).collect();
    mouths.sort_by(|a, b| b.total_cmp(a));
    mouths.truncate(8);
    let nav_q = mouths.get(mouths.len() / 2).map_or(big_river, |&m| (0.25 * m).max(0.5 * big_river));
    let rivers = &inp.hydro.rivers;
    // Each chain's system (the chain reaching the sea, a lake or its end) and how many cells its
    // end lies up from that system's mouth, along the flow.
    let mut down: Vec<Option<(usize, usize)>> = vec![None; rivers.len()];
    for c in 0..rivers.len() {
        let mut path = vec![c];
        while let Some(&t) = path.last()
            && down[t].is_none()
            && path.len() < 256
        {
            match rivers[t].into.filter(|&p| p < rivers.len() && !path.contains(&p)) {
                Some(p) => path.push(p),
                None => {
                    down[t] = Some((t, 0));
                    break;
                }
            }
        }
        for k in (0..path.len()).rev() {
            let t = path[k];
            if down[t].is_some() {
                continue;
            }
            let Some(p) = rivers[t].into else { continue };
            let Some((root, d)) = down[p] else { continue };
            let pc = &rivers[p].cells;
            let at = rivers[t].cells.last().and_then(|e| pc.iter().rposition(|x| x == e)).unwrap_or(pc.len().saturating_sub(1));
            down[t] = Some((root, d + pc.len().saturating_sub(1 + at)));
        }
    }
    // (A head or crossing this near the mouth is the mouth's town.)
    let up = (crate::core::round(15.0 * 5280.0 / cell) as usize).max(4);
    for r in &inp.hydro.rivers {
        let (Some(&first), Some(&q)) = (r.cells.first(), r.q.last()) else { continue };
        let first = first as usize;
        let len = r.cells.len();
        if len >= 3 && (inp.hydro.lake_of[first] != NO_LAKE || neighbors(w, h, first).any(|(nb, _)| inp.hydro.lake_of[nb] != NO_LAKE)) {
            splat(&mut site, first, Reason::Outlet, 0.7, 1);
            sources.push((Reason::Outlet, first as u32, first as u32, 0.7));
        }
        let to_water = matches!(r.mouth, Mouth::Ocean | Mouth::Lake);
        if (q as f64) < big_river {
            continue;
        }
        // The lowest point a big river can be bridged: the last cell under the bridge limit; a
        // river never too wide for one is crossed below its lowest confluence (whose roads meet
        // there) or a little above its mouth.
        let at = if q as f64 > bridge_q {
            r.q.iter().rposition(|&v| (v as f64) <= bridge_q)
        } else if to_water || q as f64 >= 2.0 * big_river {
            (0..len.saturating_sub(up / 2)).rev().find(|&j| j > 0 && joined[r.cells[j - 1] as usize]).or(len.checked_sub(up / 2 + 1))
        } else {
            None
        };
        if let Some(i) = at
            && i > 0
            && i + 2 < len
        {
            splat(&mut site, r.cells[i] as usize, Reason::Crossing, 0.95, 2);
        }
    }
    // The head of navigation: in each big system reaching the sea or a lake, the point boats
    // reach farthest up (where a river big enough for them stops being so), well up from its
    // mouth. (A chain big from its first cell leaves a lake: boats go on across it.)
    let mut heads: Vec<Option<(usize, usize)>> = vec![None; rivers.len()];
    for (c, r) in rivers.iter().enumerate() {
        let Some((root, d)) = down[c] else { continue };
        let big = rivers[root].q.iter().any(|&v| v as f64 >= 1.5 * nav_q);
        if !big || !matches!(rivers[root].mouth, Mouth::Ocean | Mouth::Lake) {
            continue;
        }
        if let Some(i) = r.q.iter().position(|&v| v as f64 >= nav_q)
            && i > 0
        {
            let far = d + r.cells.len() - 1 - i;
            if far >= up && heads[root].is_none_or(|h| far > h.0) {
                heads[root] = Some((far, r.cells[i] as usize));
            }
        }
    }
    for &(_, at) in heads.iter().flatten() {
        splat(&mut site, at, Reason::Head, 0.95, 2);
        sources.push((Reason::Head, at as u32, at as u32, 0.95));
    }
    for r in &inp.hydro.rivers {
        let (Some(&last), Some(&q)) = (r.cells.last(), r.q.last()) else { continue };
        let strength = match r.mouth {
            Mouth::Ocean | Mouth::Lake => 1.0,
            Mouth::Confluence => 0.5 * smoothstep(0.5 * big_river, 3.0 * big_river, q as f64),
            _ => 0.0,
        } * smoothstep(river_q, big_river * 2.0, q as f64);
        if strength <= 0.0 {
            continue;
        }
        let kind = if r.mouth == Mouth::Confluence { Reason::Confluence } else { Reason::Mouth };
        let before = if r.mouth == Mouth::Confluence { last } else { r.cells[r.cells.len().saturating_sub(2)] };
        sources.push((kind, last, before, strength as f32));
        let (mx, my) = ((last as usize % w) as i64, (last as usize / w) as i64);
        for dy in -2i64..=2 {
            for dx in -2i64..=2 {
                let (x, y) = (mx + dx, my + dy);
                if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                    let k = y as usize * w + x as usize;
                    let fall = 1.0 - (crate::core::sqrt((dx * dx + dy * dy) as f64) / 3.0).min(1.0);
                    mouth[k] = mouth[k].max(strength * fall);
                    let reason = if r.mouth == Mouth::Confluence { Reason::Confluence } else { Reason::Mouth };
                    let e = &mut site[k][reason as usize];
                    *e = (*e).max((strength * fall * 255.0) as u8);
                    if matches!(r.mouth, Mouth::Ocean | Mouth::Lake) && strength * fall > 0.5 {
                        harbour[k] = true;
                    }
                }
            }
        }
    }
    // Farmland within a day's reach: the mean fertility of the cells round (a box sum).
    let reach = (crate::core::round(3.0 * 5280.0 / cell) as i64).clamp(1, 6);
    let mut food = vec![0.0f32; n];
    {
        let sw = w + 1;
        let mut sum = vec![0.0f64; sw * (h + 1)];
        for y in 0..h {
            let mut row = 0.0;
            for x in 0..w {
                let k = y * w + x;
                row += if inp.land[k] { fertility(biome_of(k)) } else { 0.0 };
                sum[(y + 1) * sw + x + 1] = sum[y * sw + x + 1] + row;
            }
        }
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = ((x as i64 - reach).max(0) as usize, ((x as i64 + reach + 1) as usize).min(w));
                let (y0, y1) = ((y as i64 - reach).max(0) as usize, ((y as i64 + reach + 1) as usize).min(h));
                let total = sum[y1 * sw + x1] - sum[y0 * sw + x1] - sum[y1 * sw + x0] + sum[y0 * sw + x0];
                food[y * w + x] = (total / ((2 * reach + 1) * (2 * reach + 1)) as f64) as f32;
            }
        }
    }
    let ore_seed = inp.world.stream("t0.settle.ore");
    let mut bulk = vec![0.0f32; n];
    let mut industry = vec![0.0f32; n];
    for k in 0..n {
        if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE {
            continue;
        }
        let (mut ocean_n, mut lake_n) = (0, 0);
        for (nb, _) in neighbors(w, h, k) {
            if !inp.land[nb] {
                ocean_n += 1;
            } else if inp.hydro.lake_of[nb] != NO_LAKE {
                lake_n += 1;
            }
        }
        // A coast is a coast on barren ground too (a pinned port there is still a port); the
        // ground only decides how good a site it is.
        coastal[k] = ocean_n > 0;
        lakeside[k] = lake_n > 0;
        let b = biome_of(k);
        let fert = fertility(b);
        let above = inp.height[k] - sea;
        let (cx, cy) = ((k % w) as f64, (k / w) as f64);
        // Industry: ore in the hills, timber where forest meets a river, fishing grounds.
        let ore = smoothstep(1_500.0, 2_500.0, above) * smoothstep(0.3, 0.45, fbm(ore_seed, cx / 12.0, cy / 12.0, 3, 2.0, 0.5));
        let forest = matches!(b, Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle);
        // (Logs float down a river; away from one, woodcutters and charcoal burners still live by it.)
        let timber = if !forest {
            0.0
        } else if inp.hydro.discharge[k] as f64 >= river_q || neighbors(w, h, k).any(|(nb, _)| inp.hydro.discharge[nb] as f64 >= river_q) {
            0.6
        } else {
            TIMBER_INLAND
        };
        if ocean_n > 0 {
            // A bay or inlet (sea on little of the ground round: land on three sides) shelters a
            // harbour; a bare coast or a headland is only fishing.
            let (mut sea_n, mut all) = (0, 0);
            for dy in -4i64..=4 {
                for dx in -4i64..=4 {
                    let (x, y) = (cx as i64 + dx, cy as i64 + dy);
                    if dx * dx + dy * dy <= 16 && x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                        all += 1;
                        sea_n += !inp.land[y as usize * w + x as usize] as i32;
                    }
                }
            }
            let shelter = 1.0 - smoothstep(0.22, 0.36, sea_n as f64 / all.max(1) as f64);
            site[k][Reason::Harbour as usize] = (0.8 * shelter * 255.0) as u8;
        }
        if ocean_n > 0 || lake_n > 0 {
            site[k][Reason::Fishing as usize] = (0.5 * 255.0) as u8;
        }
        site[k][Reason::Ore as usize] = (ore * 255.0) as u8;
        site[k][Reason::Timber as usize] = (timber * 255.0) as u8;
        if fert <= 0.0 {
            continue;
        }
        let flat = 1.0 - smoothstep(0.02, 0.15, slope_of(k));
        let mut water = 0.0f64;
        let q = inp.hydro.discharge[k] as f64;
        if ocean_n > 0 {
            // Sheltered coasts (a few sea neighbours, not a headland) make better harbours.
            water += if (2..=4).contains(&ocean_n) { 1.0 } else { 0.6 };
        }
        if q >= river_q {
            on_river[k] = true;
            water += 0.5 + 0.5 * smoothstep(river_q, big_river * 4.0, q);
        }
        if lake_n > 0 {
            water += 0.6;
        }
        water += 1.4 * mouth[k];
        let dry_fix = if matches!(b, Biome::HotDesert | Biome::ColdDesert) && water < 0.5 { 0.2 } else { 1.0 };
        // Farmland within reach, not the cell's own; a mining or timber place needs no farmland.
        let fed = 0.3 * fert + 0.7 * (1.4 * food[k] as f64).min(1.0);
        let fert = fed.max(0.6 * ore).max(0.5 * timber);
        let altitude = 1.0 - 0.8 * smoothstep(3_000.0, 8_000.0, above) * (1.0 - ore);
        base[k] = (fert * (0.35 + 0.65 * flat) * altitude * dry_fix) as f32;
        water_at[k] = water as f32;
        score[k] = base[k] as f64 * (0.35 + water);
    }
    // Springs: low, flat ground at the foot of higher ground (the ground within two cells lying
    // above it on the whole: a valley floor, the foot of a slope), away from rivers, lakes and the
    // sea (within a cell): where wells and springs water a village. One box sum, O(cells).
    let mut spring = vec![0.0f32; n];
    {
        let sw = w + 1;
        let mut sum = vec![0.0f64; sw * (h + 1)];
        for y in 0..h {
            let mut row = 0.0;
            for x in 0..w {
                row += inp.height[y * w + x].max(sea);
                sum[(y + 1) * sw + x + 1] = sum[y * sw + x + 1] + row;
            }
        }
        let wet = |k: usize| coastal[k] || lakeside[k] || on_river[k] || inp.hydro.lake_of[k] != NO_LAKE;
        for k in 0..n {
            if base[k] <= 0.0 || wet(k) || neighbors(w, h, k).any(|(nb, _)| wet(nb)) {
                continue;
            }
            let (x, y) = (k % w, k / w);
            let (x0, x1, y0, y1) = (x.saturating_sub(2), (x + 3).min(w), y.saturating_sub(2), (y + 3).min(h));
            let total = sum[y1 * sw + x1] - sum[y0 * sw + x1] - sum[y1 * sw + x0] + sum[y0 * sw + x0];
            let rise = total / ((x1 - x0) * (y1 - y0)) as f64 - inp.height[k];
            let flat = 1.0 - smoothstep(0.02, 0.10, slope_of(k));
            // (At the foot: little above the lowest ground round, not on a bench up the slope,
            // where its track would climb to it. 25 cells each, O(cells).)
            let mut lo = inp.height[k];
            for yy in y0..y1 {
                for xx in x0..x1 {
                    lo = lo.min(inp.height[yy * w + xx].max(sea));
                }
            }
            let foot = 1.0 - smoothstep(SPRING_LOW_FT.0, SPRING_LOW_FT.1, inp.height[k] - lo);
            // (Not between mountains: deep valleys have their rivers.)
            spring[k] = (flat * foot * smoothstep(SPRING_RISE_FT.0, SPRING_RISE_FT.1, rise) * (1.0 - smoothstep(SPRING_RISE_FT.2, SPRING_RISE_FT.3, rise))) as f32;
        }
    }
    for k in 0..n {
        let s = &site[k];
        let m = |rs: &[Reason]| rs.iter().map(|&r| s[r as usize]).max().unwrap_or(0) as f32 / 255.0;
        bulk[k] = m(&[Reason::Harbour, Reason::Mouth, Reason::Outlet, Reason::Confluence, Reason::Head, Reason::Crossing]);
        industry[k] = m(&[Reason::Ore, Reason::Timber, Reason::Fishing]);
    }
    let islands = super::islands::label(w, h, cell, inp.land, &inp.hydro.lake_of, inp.biome);
    Ground { score, base, water: water_at, spring, coastal, lakeside, on_river, harbour, site, food, bulk, industry, sources, islands }
}

/// What a place lives by: its reason's kind where the place allows, else a cascade on its ground
/// (`ore`: the ore noise at its cell; `hilly`: hill or mountain ground round it, for a mining
/// town).
#[allow(clippy::too_many_arguments)]
fn kind_of(reason: Reason, tier: Tier, b: Biome, above: f64, ore: f64, is_coast: bool, near_river: bool, on_river: bool, hilly: bool) -> SettleKind {
    let from_reason = match reason {
        Reason::Harbour | Reason::Mouth if is_coast => Some(if tier >= Tier::Town { SettleKind::Port } else { SettleKind::Fishing }),
        Reason::Outlet | Reason::Confluence | Reason::Head | Reason::Crossing if tier >= Tier::Town && near_river => Some(SettleKind::River),
        Reason::Ore => Some(SettleKind::Mining),
        Reason::Timber if tier <= Tier::Town => Some(SettleKind::Lumber),
        Reason::Fishing if tier == Tier::Village && is_coast => Some(SettleKind::Fishing),
        Reason::Pass => Some(SettleKind::Fortress),
        Reason::Crossroads | Reason::Market => Some(SettleKind::Market),
        Reason::Spring if matches!(b, Biome::HotDesert | Biome::ColdDesert) => Some(SettleKind::Oasis),
        Reason::Spring if tier == Tier::Village => Some(SettleKind::Farming),
        _ => None,
    };
    if let Some(kind) = from_reason {
        kind
    } else if is_coast {
        if tier >= Tier::Town { SettleKind::Port } else { SettleKind::Fishing }
    } else if above > 2_500.0 && ore > 0.1 && hilly {
        SettleKind::Mining
    } else if above > 2_500.0 && tier >= Tier::Town {
        SettleKind::Fortress
    } else if on_river && tier >= Tier::Town {
        SettleKind::River
    } else if tier >= Tier::City {
        SettleKind::Market
    } else if matches!(b, Biome::HotDesert | Biome::ColdDesert) {
        SettleKind::Oasis
    } else if matches!(b, Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle) {
        SettleKind::Lumber
    } else if matches!(b, Biome::Steppe | Biome::Savanna | Biome::Tundra) {
        SettleKind::Herding
    } else if on_river {
        SettleKind::Fishing
    } else {
        SettleKind::Farming
    }
}

/// The population range of a tier.
pub fn pop_range(tier: Tier) -> (f64, f64) {
    match tier {
        Tier::Metropolis => (25_000.0, 80_000.0),
        Tier::City => (5_000.0, 25_000.0),
        Tier::Town => (500.0, 5_000.0),
        Tier::Village => (50.0, 500.0),
    }
}

/// Move a generated settlement to another tier (the economy's re-tiering): a reason its new tier
/// doesn't weigh gives way to the strongest one it does (a city stands for more than its timber,
/// fishing, pass or crossroads), and its kind follows.
pub fn retier(inp: &Inputs, s: &mut Settlement, tier: Tier) {
    let (w, h) = (inp.w, inp.h);
    let Some(g) = inp.ground.get() else { return };
    let k = s.cell;
    s.tier = tier;
    let skip = |r: Reason| (tier < Tier::Town && matches!(r, Reason::Outlet | Reason::Head | Reason::Pass | Reason::Crossroads | Reason::Market)) || (tier >= Tier::City && matches!(r, Reason::Timber | Reason::Fishing | Reason::Pass | Reason::Crossroads | Reason::Spring | Reason::Market));
    if skip(s.reason) {
        let mut best = 0.3;
        s.reason = Reason::None;
        for r in [Reason::Harbour, Reason::Mouth, Reason::Outlet, Reason::Confluence, Reason::Head, Reason::Crossing, Reason::Ore] {
            let v = g.site[k][r as usize] as f64 / 255.0;
            if !skip(r) && v > best + 1e-9 {
                (s.reason, best) = (r, v);
            }
        }
        if !skip(Reason::Spring) && SPRING_REASON * g.spring[k] as f64 > best + 1e-9 {
            s.reason = Reason::Spring;
        }
        if s.reason == Reason::None && g.food[k] >= 0.5 {
            s.reason = Reason::Food;
        }
    }
    // A city grown on its trade alone lives by its market.
    if s.reason == Reason::None && tier >= Tier::City && tier > s.planned {
        s.reason = Reason::Crossroads;
    }
    let near_river = g.on_river[k] || neighbors(w, h, k).any(|(nb, _)| g.on_river[nb]);
    let (cx, cy) = ((k % w) as f64, (k / w) as f64);
    let ore = fbm(inp.world.stream("t0.settle.ore"), cx / 12.0, cy / 12.0, 3, 2.0, 0.5);
    let b = Biome::from_u8((inp.biome[k] & 0xff) as u8);
    s.kind = kind_of(s.reason, tier, b, inp.height[k] - inp.world.params().sea_level_ft, ore, s.coastal, near_river, g.on_river[k], true);
    s.river = g.on_river[k] || (near_river && s.kind == SettleKind::River);
}

/// Place settlements of the given tiers, keeping `existing` ones. `roads` (from
/// `roads::preview`) favours sites on roads and most of all at junctions, so later
/// tiers grow at the crossroads the earlier tiers' roads made. Pins asking for what their place
/// can't give (a port away from water, a river town off any river) say so in `conflicts`.
pub fn place(inp: &Inputs, existing: Vec<Settlement>, tiers: &[Tier], roads: Option<&super::roads::Preview>, conflicts: &mut Vec<super::sketch::Conflict>) -> Vec<Settlement> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let world = inp.world;
    let p = world.params();
    let sea = p.sea_level_ft;
    let n = w * h;

    let ore_seed = world.stream("t0.settle.ore");
    let biome_of = |k: usize| Biome::from_u8((inp.biome[k] & 0xff) as u8);
    let Ground { score, base, water, spring, coastal, lakeside, on_river, harbour, site, food, bulk, .. } = inp.ground.get_or_init(|| ground(inp));

    // How many of each tier, from land area (square miles) and density. At density 0 none are
    // chosen, but the sketch's pins still stand: what is placed by hand wins over the dial.
    let cell_mi = cell / 5280.0;
    let land_area = (0..n).filter(|&k| score[k] > 0.0).count() as f64 * cell_mi * cell_mi;
    let none = p.settlement_density <= 0.0;
    let villages = if none { 0 } else { (land_area / 3_500.0 * p.settlement_density).round().max(3.0) as usize };
    let quota = [
        (Tier::Metropolis, if none { 0 } else { (villages / 90).max(1) }, 180.0),
        (Tier::City, if none { 0 } else { (villages / 18).max(1) }, 90.0),
        (Tier::Town, if none { 0 } else { (villages / 5).max(2) }, 40.0),
        (Tier::Village, villages, 14.0),
    ];
    // (The world's villages and towns per sq mi: a big island's share.)
    let density = (villages as f64 / land_area.max(1.0), quota[2].1 as f64 / land_area.max(1.0));

    // Roads: a site on a road is better, a junction much better (within a cell of it; and
    // whether on its own cell).
    let road_at = |k: usize| -> (bool, bool, bool, bool) {
        let Some(rp) = roads else { return (false, false, false, false) };
        let (x, y) = ((k % w) as i64, (k / w) as i64);
        let (mut on, mut at, mut pass) = (false, false, false);
        let here = rp.usage[k] > 0 || rp.junction[k];
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                on |= rp.usage[j] > 0;
                at |= rp.junction[j];
                pass |= rp.pass[j];
            }
        }
        (on, at, pass, here)
    };
    // A ford or bridge where a road crosses a real river.
    let ford = |k: usize| inp.hydro.discharge[k] as f64 >= 3.0 * super::hydro::river_q(p.river_density);
    let road_bonus = |k: usize, tier: Tier| -> f64 {
        let (on, at, pass, here) = road_at(k);
        let (on_b, at_b) = if tier >= Tier::Town { (3.0, 5.0) } else { (2.0, 3.5) };
        // (Villages: on the road itself more, a cell off it less: a short spur from beside a road
        // meets it at a slant, or by a bridge.)
        let near = |b: f64| if tier >= Tier::Town { b } else if here { ROAD_HERE * b } else { 1.0 + ROAD_NEAR * (b - 1.0) };
        let (on_b, at_b) = (near(on_b), near(at_b));
        // Cities and towns also favour the sites where goods change hands (harbours, mouths,
        // confluences, heads of navigation, crossings) over plain farmland.
        // (Only cities: towns and villages live by what is round them, not by the water's trade.)
        let timber = site[k][Reason::Timber as usize] as f64 / 255.0;
        let sites = match tier {
            Tier::Metropolis | Tier::City => 1.0 + 2.0 * bulk[k] as f64,
            Tier::Town => 1.0 + 0.5 * timber,
            Tier::Village => 1.0 + 0.25 * timber,
        };
        // Ore draws miners to towns and villages however poor the ground; a pass, a town to
        // hold it; a ford on a road, a town or village by it.
        let ore = site[k][Reason::Ore as usize] as f64 / 255.0;
        let sites = sites
            * match tier {
                Tier::Town => (1.0 + 1.0 * ore) * if pass { 2.5 } else { 1.0 } * if on && ford(k) { 1.3 } else { 1.0 },
                Tier::Village => (1.0 + 1.0 * ore) * if on && ford(k) { 1.3 } else { 1.0 },
                _ => 1.0,
            };
        sites * if at { at_b } else if on { on_b } else { 1.0 }
    };

    let mut out = existing;
    // A pinned capital is the capital (else the first metropolis is).
    let pinned_capital = inp.pins.iter().any(|p| p.capital);
    for (tier, count, spacing_mi) in quota {
        if !tiers.contains(&tier) {
            continue;
        }
        // Each cell's draws are its own (its jitter here, its seed and size in `make`), so a
        // settlement added or taken away elsewhere changes no other's.
        let jitter = world.stream("t0.settle");
        // Market spacing (towns): how far each cell lies from the towns and up placed before this
        // tier (cities, realms, pins), in cells; O(cells × those few).
        let before: Vec<(f64, f64)> = out.iter().filter(|s| s.tier >= Tier::Town).map(|s| ((s.cell % w) as f64, (s.cell / w) as f64)).collect();
        let cities: Vec<(f64, f64)> = out.iter().filter(|s| s.tier >= Tier::City && !s.isolated).map(|s| ((s.cell % w) as f64, (s.cell / w) as f64)).collect();
        // (Cells to the nearest city; 0 with none.)
        let cities_d = |k: usize| {
            let (cx, cy) = ((k % w) as f64, (k / w) as f64);
            let d = cities.iter().map(|&(x, y)| (x - cx) * (x - cx) + (y - cy) * (y - cy)).fold(f64::INFINITY, f64::min);
            if d.is_finite() { crate::core::sqrt(d) } else { 0.0 }
        };
        let gap = |k: usize| {
            let (cx, cy) = ((k % w) as f64, (k / w) as f64);
            let d = before.iter().map(|&(x, y)| (x - cx) * (x - cx) + (y - cy) * (y - cy)).fold(f64::INFINITY, f64::min);
            if d.is_finite() { market_gap(crate::core::sqrt(d), cell) } else { 0.0 }
        };
        // What the tier weighs at a cell: cities the water most; towns and villages less, and
        // their inland pulls (springs, farmland, market spacing).
        let site_score = |k: usize| -> f64 {
            let (b, wv) = (base[k] as f64, water[k] as f64);
            match tier {
                Tier::Metropolis | Tier::City => score[k],
                Tier::Town => b * (TOWN_WET.0 + TOWN_WET.1 * wv) * (1.0 + SPRING_GAIN * spring[k] as f64) * (FOOD_GAIN.0 + FOOD_GAIN.1 * food[k] as f64) * (1.0 + MARKET_GAIN * gap(k)),
                Tier::Village => b * (VILLAGE_WET.0 + VILLAGE_WET.1 * wv) * (1.0 + SPRING_GAIN * spring[k] as f64) * (FOOD_GAIN.0 + FOOD_GAIN.1 * food[k] as f64),
            }
        };
        let mut cands: Vec<(f64, usize)> = (0..n).filter(|&k| score[k] > 0.05).map(|k| (site_score(k) * road_bonus(k, tier) * (0.55 + 0.45 * unit(hash3(jitter, k as i64, tier as i64, 0))), k)).collect();
        cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        // The top of a pass on a king's road draws a town to hold it however poor the ground
        // (rare: few routes top a saddle): those first, the highest first.
        if tier == Tier::Town
            && let Some(rp) = roads
        {
            let mut tops: Vec<(f64, usize)> = (0..n).filter(|&k| rp.pass[k] && inp.land[k] && score[k] > 0.0).map(|k| (inp.height[k], k)).collect();
            tops.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            cands.retain(|c| !rp.pass[c.1]);
            cands.splice(0..0, tops);
        }
        let spacing = spacing_mi * 5280.0 / cell;
        // A settlement of this tier at cell `k`, standing at (x, y) ft, drawn from `seed`.
        let make = |k: usize, seed: u64, mut x: f64, mut y: f64, capital: bool, pin: Option<&super::sketch::Pin>, conflicts: &mut Vec<super::sketch::Conflict>| {
            let (cx, cy) = ((k % w) as f64, (k / w) as f64);
            let (lo, hi) = pop_range(tier);
            let population = (lo * libm::pow(hi / lo, unit(hash3(seed, 0x706f_70, 0, 0)))) as u32;
            let b = biome_of(k);
            let above = inp.height[k] - sea;
            let ore = fbm(ore_seed, cx / 12.0, cy / 12.0, 3, 2.0, 0.5);
            let near_river = on_river[k] || neighbors(w, h, k).any(|(nb, _)| on_river[nb]);
            // The strongest reason at the site (ties go to the earlier in `Reason`'s order).
            let mut reason = Reason::None;
            let mut best = 0.3;
            let mut consider = |r: Reason, v: f64| {
                if v > best + 1e-9 {
                    (reason, best) = (r, v);
                }
            };
            for r in [Reason::Harbour, Reason::Mouth, Reason::Outlet, Reason::Confluence, Reason::Head, Reason::Crossing, Reason::Ore, Reason::Timber, Reason::Fishing] {
                // (Only a town's site weighs the river's heads, outlets and bridging points; a
                // city stands for more than its fishing or timber.)
                if (tier < Tier::Town && matches!(r, Reason::Outlet | Reason::Head | Reason::Crossing)) || (tier >= Tier::City && matches!(r, Reason::Timber | Reason::Fishing)) {
                    continue;
                }
                consider(r, site[k][r as usize] as f64 / 255.0);
            }
            if tier <= Tier::Town {
                let (on, at, pass, _) = road_at(k);
                if on && ford(k) {
                    consider(Reason::Crossing, 0.55);
                }
                if pass && tier == Tier::Town {
                    consider(Reason::Pass, 0.9);
                }
                if at && tier == Tier::Town {
                    consider(Reason::Crossroads, 0.75);
                }
                consider(Reason::Spring, SPRING_REASON * spring[k] as f64);
                if tier == Tier::Town {
                    consider(Reason::Market, MARKET_REASON * market_gap(cities_d(k), cell));
                }
            }
            if reason == Reason::None && food[k] >= 0.5 {
                reason = Reason::Food;
            }
            // A pass's town stands at the top of the pass (the highest such cell by its own), on
            // the king's road's way over.
            if reason == Reason::Pass
                && pin.is_none()
                && let Some(rp) = roads
            {
                let (px, py) = ((k % w) as i64, (k / w) as i64);
                let top = (-1i64..=1)
                    .flat_map(|dy| (-1i64..=1).map(move |dx| (px + dx, py + dy)))
                    .filter(|&(i, j)| i >= 0 && j >= 0 && i < w as i64 && j < h as i64 && rp.pass[j as usize * w + i as usize])
                    .max_by(|a, b| inp.height[a.1 as usize * w + a.0 as usize].total_cmp(&inp.height[b.1 as usize * w + b.0 as usize]).then(b.cmp(a)));
                if let Some((i, j)) = top {
                    (x, y) = (i as f64 * cell, j as f64 * cell);
                }
            }
            // A pin stands where it was drawn, not at its cell's centre: one drawn on the coast is
            // coastal when the sea is within reach of that point (the shore it then moves to).
            let pinned_coast = pin.is_some() && {
                let r = 1.6f64;
                let (pi, pj) = (crate::core::round(x / cell) as i64, crate::core::round(y / cell) as i64);
                (-2..=2i64).any(|dj| {
                    (-2..=2i64).any(|di| {
                        let (i, j) = (pi + di, pj + dj);
                        let (dx, dy) = (i as f64 - x / cell, j as f64 - y / cell);
                        i >= 0 && j >= 0 && i < w as i64 && j < h as i64 && dx * dx + dy * dy <= r * r && !inp.land[j as usize * w + i as usize]
                    })
                })
            };
            let mut is_coast = coastal[k] || harbour[k] || pinned_coast;
            let river = on_river[k] || (pin.is_some() && neighbors(w, h, k).any(|(nb, _)| on_river[nb]));
            let mut kind = kind_of(reason, tier, b, above, ore, is_coast, near_river, on_river[k], true);
            // What the pin says it lives by, where its place allows (a lakeside port stands on
            // its lake's shore).
            if let Some(want) = pin.and_then(|p| p.kind) {
                let lake = lakeside[k] || neighbors(w, h, k).any(|(nb, _)| lakeside[nb]);
                let ok = match want {
                    SettleKind::Port | SettleKind::Fishing => is_coast || lake || (want == SettleKind::Fishing && river),
                    SettleKind::River => river,
                    _ => true,
                };
                if ok {
                    kind = if want == SettleKind::Port && tier < Tier::Town { SettleKind::Fishing } else { want };
                    is_coast |= matches!(want, SettleKind::Port | SettleKind::Fishing) && !river;
                } else {
                    let p = pin.expect("asked");
                    let what = if want == SettleKind::River { "river town: no river runs here" } else { "port: no sea or lake shore here" };
                    conflicts.push(super::sketch::Conflict { stroke: p.stroke, message: format!("This settlement can't be a {what}"), x, y });
                }
            }
            Settlement {
                tier,
                kind,
                reason,
                cell: k,
                x,
                y,
                population,
                coastal: is_coast,
                river: on_river[k] || (near_river && kind == SettleKind::River),
                capital,
                seed,
                culture: 0,
                pin: pin.map(|p| p.stroke as u32),
                planned: tier,
                planned_pop: population,
                isolated: false,
            }
        };
        // Pinned ones first (they count toward the tier's quota), seeded by where they were
        // drawn (as the sketch's sites are), so taking another stroke away changes none. Those
        // added beside the world's own come after them (`Pin::added`).
        let pinned = |pin: &super::sketch::Pin, out: &[Settlement], conflicts: &mut Vec<super::sketch::Conflict>| {
            let seed = hash3(world.stream("t0.pin"), (pin.x / 10.0) as i64, (pin.y / 10.0) as i64, tier as i64);
            let capital = if pinned_capital { pin.capital } else { tier == Tier::Metropolis } && !out.iter().any(|s| s.capital);
            make(pin.cell, seed, pin.x, pin.y, capital, Some(pin), conflicts)
        };
        let start = out.len();
        let mut placed = 0;
        for pin in inp.pins.iter().filter(|p| p.tier == tier && !p.added) {
            let s = pinned(pin, &out, conflicts);
            out.push(s);
            placed += 1;
        }
        // Whether a settlement of this tier keeps its room from the others at cell `k`.
        let fits = |out: &[Settlement], k: usize| {
            let (cx, cy) = ((k % w) as f64, (k / w) as f64);
            out.iter().all(|s| {
                let (sx, sy) = ((s.cell % w) as f64, (s.cell / w) as f64);
                let d = crate::core::sqrt((sx - cx) * (sx - cx) + (sy - cy) * (sy - cy));
                // (One added beside the world's own keeps only the least room round it.)
                let added = s.pin.is_some_and(|st| inp.pins.iter().any(|p| p.stroke == st as usize && p.added));
                let need = if s.tier >= tier && !added { spacing } else { 10.0 * 5280.0 / cell };
                // (Nothing grows up beside an isolated realm: no town within sight of it.)
                let need = if s.isolated { need.max(if tier >= Tier::Town { REALM_CLEAR_MI } else { REALM_CLEAR_VILLAGE_MI } * 5280.0 / cell) } else { need };
                d >= need.max(10.0 * 5280.0 / cell)
            })
        };
        // Each island's share next (`islands::Islands::owed`; its pins count toward it), then the
        // rest by score.
        if !none && matches!(tier, Tier::Village | Tier::Town) {
            let mut put = |out: &mut Vec<Settlement>, k: usize| {
                let (cx, cy) = ((k % w) as f64, (k / w) as f64);
                let ok = fits(out, k);
                if ok {
                    out.push(make(k, hash3(world.stream("t0.settle.seed"), k as i64, tier as i64, 0), cx * cell, cy * cell, false, None, conflicts));
                }
                ok
            };
            placed += island_share(inp, tier, density, &cands, &mut out, count.saturating_sub(placed), &mut put);
        }
        for &(_, k) in &cands {
            if placed >= count {
                break;
            }
            let (cx, cy) = ((k % w) as f64, (k / w) as f64);
            if !fits(&out, k) {
                continue;
            }
            let capital = tier == Tier::Metropolis && !pinned_capital && !out.iter().any(|s| s.capital);
            out.push(make(k, hash3(world.stream("t0.settle.seed"), k as i64, tier as i64, 0), cx * cell, cy * cell, capital, None, conflicts));
            placed += 1;
        }
        for pin in inp.pins.iter().filter(|p| p.tier == tier && p.added) {
            // (Not on another settlement: drawn on one, it would be laid out over it.)
            if let Some(o) = out.iter().find(|s| crate::core::sqrt((s.x - pin.x) * (s.x - pin.x) + (s.y - pin.y) * (s.y - pin.y)) < 3.0 * 5280.0) {
                let message = format!("This settlement is within 3 miles of a {}: no settlement placed", o.tier.name());
                conflicts.push(super::sketch::Conflict { stroke: pin.stroke, message, x: pin.x, y: pin.y });
                continue;
            }
            let s = pinned(pin, &out, conflicts);
            out.push(s);
        }
        if tier == Tier::Village {
            island_fishing(inp, &mut out[start..]);
        }
    }
    out
}

/// Place island `tier`s owed (`islands::Islands::owed`) less those already there, at most
/// `budget`, island by island by their lowest cells: a small island's village on its most
/// sheltered shore, a port town first, the rest on the tier's candidates (`cands`, in order), then
/// on the island's other livable ground by score; never on barren ground. `put` places one where
/// it keeps its room. How many were placed.
fn island_share(inp: &Inputs, tier: Tier, density: (f64, f64), cands: &[(f64, usize)], out: &mut Vec<Settlement>, budget: usize, put: &mut dyn FnMut(&mut Vec<Settlement>, usize) -> bool) -> usize {
    let g = inp.ground.get().expect("ground");
    let isl = &g.islands;
    let n = inp.w * inp.h;
    let seed = inp.world.stream("t0.islands");
    let owed: Vec<super::islands::Owed> = (0..isl.parts.len() as u32).map(|c| isl.owed(c, seed, density)).collect();
    let want = |c: u32| if tier == Tier::Village { owed[c as usize].villages } else { owed[c as usize].towns };
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); isl.parts.len()];
    for k in 0..n {
        if let Some(c) = isl.island_at(k)
            && want(c) > 0
            && g.score[k] > 0.0
            && !super::islands::barren(Biome::from_u8((inp.biome[k] & 0xff) as u8))
        {
            cells[c as usize].push(k);
        }
    }
    let mut rank = vec![u32::MAX; n];
    for (i, &(_, k)) in cands.iter().enumerate() {
        rank[k] = i as u32;
    }
    let mut placed = 0;
    for c in 0..isl.parts.len() as u32 {
        let o = owed[c as usize];
        let mut have = out.iter().filter(|s| s.tier == tier && isl.of[s.cell] == c).count();
        if have >= want(c) {
            continue;
        }
        let cs = &mut cells[c as usize];
        cs.sort_by(|&a, &b| rank[a].cmp(&rank[b]).then(g.score[b].total_cmp(&g.score[a])).then(a.cmp(&b)));
        let shore = |k: usize| g.coastal[k] || g.harbour[k];
        // (On the shore first: a small island's only village, by its harbour; a port town.)
        let first_shore = if tier == Tier::Village { o.shore_village } else { o.port_town };
        if first_shore && have == 0 {
            let mut by: Vec<usize> = cs.iter().copied().filter(|&k| shore(k)).collect();
            if tier == Tier::Village {
                by.sort_by_key(|&k| std::cmp::Reverse(g.site[k][Reason::Harbour as usize]));
            }
            if placed >= budget || !by.into_iter().any(|k| put(out, k)) {
                continue;
            }
            (have, placed) = (have + 1, placed + 1);
        }
        for &k in cs.iter() {
            if have >= want(c) || placed >= budget {
                break;
            }
            if put(out, k) {
                (have, placed) = (have + 1, placed + 1);
            }
        }
    }
    placed
}

/// An island's generated villages on the shore live by the sea: by their harbour where it is
/// sheltered, else by fishing.
fn island_fishing(inp: &Inputs, placed: &mut [Settlement]) {
    let g = inp.ground.get().expect("ground");
    for s in placed.iter_mut().filter(|s| s.tier == Tier::Village && s.pin.is_none() && s.coastal && g.islands.island_at(s.cell).is_some()) {
        let k = s.cell;
        s.reason = if g.site[k][Reason::Harbour as usize] as f64 / 255.0 > 0.5 { Reason::Harbour } else { Reason::Fishing };
        s.kind = kind_of(s.reason, s.tier, Biome::from_u8((inp.biome[k] & 0xff) as u8), inp.height[k] - inp.world.params().sea_level_ft, 0.0, true, false, g.on_river[k], false);
    }
}

/// How far (miles) later towns and up keep from any part of an isolated realm, and the king's
/// roads and roads between them (`realm_crowded`): out of sight at the continent's view.
pub const REALM_CLEAR_MI: f64 = 100.0;
/// How far (miles) later villages keep from any part of an isolated realm.
pub const REALM_CLEAR_VILLAGE_MI: f64 = 40.0;
/// How far (miles) round a realm's town other king's roads and roads pay extra to pass, and
/// tracks (`roads::route`).
pub const REALM_AVOID_MI: f64 = 115.0;
pub const REALM_AVOID_TRACK_MI: f64 = 45.0;
/// How far (miles) a realm's town keeps from the cities placed before it and from the king's
/// roads between them as previewed: the clearance and its villages' reach (11 miles).
const REALM_CITY_MI: f64 = REALM_CLEAR_MI + 11.0;
const REALM_KINGS_MI: f64 = REALM_CLEAR_MI + 11.0;
/// How far (miles) a realm keeps from a site given up for one (`realm_crowded`).
const REALM_BANNED_MI: f64 = 60.0;

/// The realms' towns (their indices) that the king's roads and roads between the towns and up
/// as previewed (`usage`, which the network as built follows) pass closer than the clearance
/// to: their sites are given up (`place_realms`' `banned`).
pub fn realm_crowded(settlements: &[Settlement], usage: &super::roads::Preview, w: usize, cell: f64) -> Vec<usize> {
    let clear = REALM_CLEAR_MI * 5280.0;
    let realms: Vec<usize> = (0..settlements.len()).filter(|&i| settlements[i].isolated && settlements[i].tier >= Tier::Town).collect();
    let mut out = Vec::new();
    for &r in &realms {
        let parts: Vec<(f64, f64)> = settlements.iter().enumerate().filter(|&(i, s)| i == r || (s.isolated && s.tier == Tier::Village && super::roads::realm_of(settlements, i) == Some(r))).map(|(_, s)| ((s.cell % w) as f64 * cell, (s.cell / w) as f64 * cell)).collect();
        let near = usage.paths.iter().filter(|(class, _, _)| *class <= super::roads::RoadClass::Road).flat_map(|(_, path, _)| path.iter()).any(|&c| {
            let (x, y) = ((c as usize % w) as f64 * cell, (c as usize / w) as f64 * cell);
            parts.iter().any(|&(px, py)| (x - px) * (x - px) + (y - py) * (y - py) < clear * clear)
        });
        if near {
            out.push(settlements[r].cell);
        }
    }
    out
}

/// Ground a mountain realm stands on: grass or woods (not scree, snow or bare ground).
fn green(b: Biome) -> bool {
    matches!(b, Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle | Biome::Grassland | Biome::Steppe | Biome::Savanna)
}

/// A biome a forest realm hides in.
fn deep_forest(b: Biome) -> bool {
    matches!(b, Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle)
}

/// Isolated realms, on top of the tiers' counts: a walled city (rarely a large town) deep in a
/// large forest (a market) or on a flat spot high in the mountains (a fortress), well clear of
/// the cities placed so far and of every pin, with 2-4 hidden villages of its own round it in
/// the same forest or valley, on the side its way out leaves by (towards the nearest of the
/// king's roads previewed, `kings`). About one per 15 cities, the number drawn per world (some
/// have none); none at settlement density 0. Placed after the metropolises and cities, before
/// the other tiers, so they keep clear of them (`place`); none near a site given up (`banned`).
pub fn place_realms(inp: &Inputs, existing: Vec<Settlement>, kings: Option<&super::roads::Preview>, banned: &[usize]) -> Vec<Settlement> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let world = inp.world;
    let p = world.params();
    let sea = p.sea_level_ft;
    let n = w * h;
    let mut out = existing;
    let cities = out.iter().filter(|s| s.tier >= Tier::City && s.pin.is_none()).count();
    if p.settlement_density <= 0.0 || cities == 0 {
        return out;
    }
    let g = inp.ground.get_or_init(|| ground(inp));
    let stream = world.stream("t0.realm");
    // How many: a Poisson draw, a mean of one per 15 cities.
    let count = {
        let lambda = cities as f64 / 15.0;
        let u = unit(hash3(stream, 0, 0, 0));
        let (mut k, mut pk) = (0usize, libm::exp(-lambda));
        let mut cum = pk;
        while u > cum && k < 6 {
            k += 1;
            pk *= lambda / k as f64;
            cum += pk;
        }
        k
    };
    if count == 0 {
        return out;
    }
    let mi = 5280.0 / cell;
    let biome_of = |k: usize| Biome::from_u8((inp.biome[k] & 0xff) as u8);
    let slope_of = |k: usize| super::biome::local_slope(w, h, inp.height, k % w, k / w, cell);
    let dry = |k: usize| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE;
    // How deep in its forest each cell lies (cells to the nearest ground that isn't forest).
    let open: Vec<bool> = (0..n).map(|k| !(dry(k) && deep_forest(biome_of(k)))).collect();
    let depth = super::climate::distance_to(w, h, &open);
    // Flat enough for a walled town's plan: the ground round the cell as well as the cell itself.
    let around = |k: usize, f: &dyn Fn(usize) -> bool| {
        let (x, y) = ((k % w) as i64, (k / w) as i64);
        (-1i64..=1).all(|dy| (-1i64..=1).all(|dx| {
            let (i, j) = (x + dx, y + dy);
            i >= 0 && j >= 0 && i < w as i64 && j < h as i64 && f(j as usize * w + i as usize)
        }))
    };
    let flat = |k: usize, max: f64| around(k, &|v| dry(v) && slope_of(v) <= max);
    // How far (cells) each cell lies from the king's roads previewed (none: far from all).
    let kings_d: Vec<f64> = match kings {
        Some(pv) => {
            let mut on = vec![false; n];
            for (class, path, _) in &pv.paths {
                if *class == super::roads::RoadClass::KingsRoad {
                    for &c in path {
                        on[c as usize] = true;
                    }
                }
            }
            if on.iter().any(|&b| b) { super::climate::distance_to(w, h, &on) } else { Vec::new() }
        }
        None => Vec::new(),
    };
    let kings_at = |k: usize| kings_d.get(k).copied().unwrap_or(f64::INFINITY);
    let margin = 8.0 * mi;
    let inside = |k: usize| {
        let (x, y) = ((k % w) as f64, (k / w) as f64);
        x >= margin && y >= margin && x <= (w - 1) as f64 - margin && y <= (h - 1) as f64 - margin
    };
    let jitter = |k: usize, salt: i64| 0.75 + 0.25 * unit(hash3(stream, k as i64, salt, 1));
    // Forest sites: deep in a forest (5 miles and more from its edge), the deeper the better.
    let mut forest: Vec<(f64, usize)> =
        (0..n).filter(|&k| inside(k) && !open[k] && depth[k] >= 5.0 * mi && g.score[k] > 0.05 && !g.coastal[k] && flat(k, 0.10)).map(|k| (depth[k].min(20.0 * mi) * jitter(k, 0), k)).collect();
    // Mountain sites: 3,000 ft up and more on flat ground under grass or woods, the higher and
    // flatter the better, and better still in a basin (the ground rising 300 ft round it 3 miles
    // out).
    let basin = |k: usize| {
        let (x, y) = ((k % w) as f64, (k / w) as f64);
        let r = 3.0 * mi;
        let up = (0..16)
            .filter(|&a| {
                let ang = std::f64::consts::TAU * a as f64 / 16.0;
                let (i, j) = (crate::core::round(x + r * libm::cos(ang)), crate::core::round(y + r * libm::sin(ang)));
                i >= 0.0 && j >= 0.0 && i < w as f64 && j < h as f64 && inp.height[j as usize * w + i as usize] >= inp.height[k] + 300.0
            })
            .count();
        up as f64 / 16.0
    };
    let mut mountain: Vec<(f64, usize)> = (0..n)
        .filter(|&k| inside(k) && inp.height[k] - sea >= 3_000.0 && g.score[k] > 0.0 && !g.coastal[k] && flat(k, 0.08) && around(k, &|v| green(biome_of(v))))
        .map(|k| ((inp.height[k] - sea).min(9_000.0) / 1_000.0 * (1.2 - slope_of(k) / 0.08) * (0.6 + 0.8 * basin(k)) * jitter(k, 1), k))
        .collect();
    for c in [&mut forest, &mut mountain] {
        c.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    }
    let d_cells = |a: usize, b: usize| {
        let (dx, dy) = ((a % w) as f64 - (b % w) as f64, (a / w) as f64 - (b / w) as f64);
        crate::core::sqrt(dx * dx + dy * dy)
    };
    // Well clear of every settlement so far (another realm's more so), of the king's roads
    // between them and of every pin: a pin in the same place wins. (Not one added beside the
    // world's own, which moves nothing: refused or not, it leaves the realms as they were.)
    let clear = |k: usize, out: &[Settlement]| {
        let (x, y) = ((k % w) as f64 * cell, (k / w) as f64 * cell);
        kings_at(k) >= REALM_KINGS_MI * mi
            && out.iter().all(|s| d_cells(s.cell, k) >= if s.isolated { REALM_CITY_MI + 11.0 } else { REALM_CITY_MI } * mi)
            && banned.iter().all(|&b| d_cells(b, k) >= REALM_BANNED_MI * mi)
            && inp.pins.iter().all(|q| q.added || crate::core::sqrt((q.x - x) * (q.x - x) + (q.y - y) * (q.y - y)) >= 25.0 * 5280.0)
    };
    // The strongest reason at a site (as `place` weighs it for the tier).
    let reason_at = |k: usize, tier: Tier| {
        let (mut reason, mut best) = (Reason::None, 0.3);
        for r in [Reason::Harbour, Reason::Mouth, Reason::Outlet, Reason::Confluence, Reason::Head, Reason::Crossing, Reason::Ore, Reason::Timber, Reason::Fishing] {
            if (tier < Tier::Town && matches!(r, Reason::Outlet | Reason::Head | Reason::Crossing)) || (tier >= Tier::City && matches!(r, Reason::Timber | Reason::Fishing)) {
                continue;
            }
            let v = g.site[k][r as usize] as f64 / 255.0;
            if v > best + 1e-9 {
                (reason, best) = (r, v);
            }
        }
        if tier <= Tier::Town && SPRING_REASON * g.spring[k] as f64 > best + 1e-9 {
            reason = Reason::Spring;
        }
        if reason == Reason::None && g.food[k] >= 0.5 { Reason::Food } else { reason }
    };
    let ore_seed = world.stream("t0.settle.ore");
    let make = |k: usize, tier: Tier, population: u32, seed: u64, kind: Option<SettleKind>| {
        let reason = reason_at(k, tier);
        let near_river = g.on_river[k] || neighbors(w, h, k).any(|(nb, _)| g.on_river[nb]);
        let (cx, cy) = ((k % w) as f64, (k / w) as f64);
        let ore = fbm(ore_seed, cx / 12.0, cy / 12.0, 3, 2.0, 0.5);
        let kind = kind.unwrap_or_else(|| kind_of(reason, tier, biome_of(k), inp.height[k] - sea, ore, false, near_river, g.on_river[k], true));
        Settlement {
            tier,
            kind,
            reason,
            cell: k,
            x: cx * cell,
            y: cy * cell,
            population,
            coastal: false,
            river: g.on_river[k] || (near_river && kind == SettleKind::River),
            capital: false,
            seed,
            culture: 0,
            pin: None,
            planned: tier,
            planned_pop: population,
            isolated: true,
        }
    };
    for r in 0..count as i64 {
        let woods = unit(hash3(stream, r, 1, 0)) < 0.5;
        let order: [(bool, &Vec<(f64, usize)>); 2] = if woods { [(true, &forest), (false, &mountain)] } else { [(false, &mountain), (true, &forest)] };
        let Some((woods, k)) = order.iter().find_map(|&(wd, c)| c.iter().find(|&&(_, k)| clear(k, &out)).map(|&(_, k)| (wd, k))) else { break };
        let seed = hash3(stream, k as i64, 2, 0);
        // A city, rarely a large town.
        let (tier, population) = if unit(hash3(seed, 0x7469_6572, 0, 0)) < 0.2 {
            (Tier::Town, (3_000.0 + 2_000.0 * unit(hash3(seed, 0x706f_70, 0, 0))) as u32)
        } else {
            let (lo, hi) = pop_range(Tier::City);
            (Tier::City, (lo * libm::pow(hi / lo, 0.6 * unit(hash3(seed, 0x706f_70, 0, 0)))) as u32)
        };
        let kind = if woods { SettleKind::Market } else { SettleKind::Fortress };
        out.push(make(k, tier, population, seed, Some(kind)));
        // Its hidden villages: 5 to 11 miles out, in the same forest or valley (within 1,500 ft
        // of the town's height, on grass or woods), 5 miles apart (each named on the map), on the
        // side the realm's way out leaves by (towards the nearest king's road), so that they join
        // its track between the town and the world outside, never beyond the town.
        let want = 2 + (hash3(seed, 0x7669_6c, 0, 0) % 3) as usize;
        let hk = inp.height[k];
        let (kx, ky) = ((k % w) as i64, (k / w) as i64);
        let exit = {
            // (Down the distance to the king's roads, over 5 miles round the town.)
            let probe = |dx: f64, dy: f64| {
                let (i, j) = (crate::core::round(kx as f64 + dx * 5.0 * mi), crate::core::round(ky as f64 + dy * 5.0 * mi));
                if i >= 0.0 && j >= 0.0 && i < w as f64 && j < h as f64 { kings_at(j as usize * w + i as usize) } else { f64::INFINITY }
            };
            let (gx, gy) = (probe(1.0, 0.0) - probe(-1.0, 0.0), probe(0.0, 1.0) - probe(0.0, -1.0));
            let m = crate::core::sqrt(gx * gx + gy * gy);
            if m.is_finite() && m > 1e-9 { Some((-gx / m, -gy / m)) } else { None }
        };
        let reach = crate::core::ceil(11.0 * mi) as i64;
        let mut cands: Vec<(f64, usize)> = Vec::new();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (i, j) = (kx + dx, ky + dy);
                if i < 0 || j < 0 || i >= w as i64 || j >= h as i64 {
                    continue;
                }
                let v = j as usize * w + i as usize;
                let d = d_cells(v, k);
                if d < 5.0 * mi || d > 11.0 * mi || !dry(v) || g.score[v] <= 0.0 || g.coastal[v] || slope_of(v) > 0.15 {
                    continue;
                }
                // (Within 70 degrees of the way out.)
                if exit.is_some_and(|(ex, ey)| (dx as f64 * ex + dy as f64 * ey) < 0.34 * d) {
                    continue;
                }
                let fits = if woods { !open[v] && depth[v] >= 1.5 * mi } else { (inp.height[v] - hk).abs() <= 1_500.0 && inp.height[v] - sea >= 2_000.0 && green(biome_of(v)) };
                if fits {
                    cands.push((g.score[v] * jitter(v, 2), v));
                }
            }
        }
        cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut placed: Vec<usize> = Vec::new();
        for &(_, v) in &cands {
            if placed.len() >= want {
                break;
            }
            if placed.iter().any(|&q| d_cells(q, v) < 5.0 * mi) {
                continue;
            }
            let vseed = hash3(stream, v as i64, 3, 0);
            let (lo, hi) = pop_range(Tier::Village);
            out.push(make(v, Tier::Village, (lo * libm::pow(hi / lo, unit(hash3(vseed, 0x706f_70, 0, 0)))) as u32, vseed, None));
            placed.push(v);
        }
    }
    out
}

/// The ground and roads as they stand once everything is placed, for `verify`.
pub struct Placed<'a> {
    /// Standing water (sea or lake) at a point.
    pub wet: &'a dyn Fn(f64, f64) -> bool,
    /// The ground's height at a point (ft).
    pub ground: &'a dyn Fn(f64, f64) -> f64,
    /// Where roads cross rivers (bridges, fords, ferries).
    pub crossings: &'a [[f64; 2]],
    /// Every road: its class and points.
    pub roads: &'a [(super::roads::RoadClass, &'a [[f64; 2]])],
}

/// How far out (ft) `saddle_at` looks, in steps of a quarter mile.
const SADDLE_REACH: f64 = 3.0 * 5_280.0;

/// A saddle at `p` on a road running along `u` (unit): its high point along the road (the ground
/// falls 400 ft along it both ways within 3 miles, rising nowhere more than 60 ft above it) with
/// the ground rising 300 ft on both sides across it, and 1,000 ft up; and close up a notch (steep
/// flanks within a mile, the road climbing to it both ways within a mile and a half). The one
/// test for where a king's trial route tops a pass (`roads::preview`) and for a pass town where
/// it stands.
pub(crate) fn saddle_at(ground: &dyn Fn(f64, f64) -> f64, p: [f64; 2], u: [f64; 2], sea: f64) -> bool {
    let h0 = ground(p[0], p[1]);
    if h0 - sea < 1_000.0 {
        return false;
    }
    let steps = (SADDLE_REACH / 1_320.0) as i64;
    let at_t = |d: [f64; 2], t: i64| ground(p[0] + d[0] * t as f64 * 1_320.0, p[1] + d[1] * t as f64 * 1_320.0);
    let falls = |d: [f64; 2]| (1..=steps).all(|t| at_t(d, t) <= h0 + 60.0) && (2..=steps).any(|t| at_t(d, t) <= h0 - 400.0);
    let rises = |d: [f64; 2]| (1..=steps).any(|t| at_t(d, t) >= h0 + 300.0);
    let n = [-u[1], u[0]];
    // Close up, a notch between heights: steep ground 350 ft above the town on both flanks
    // within a mile, and the road climbing to it from both ways (the ground 200 ft below it
    // within a mile and a half), in steps of an eighth of a mile.
    let near = |d: [f64; 2], m: i64| (1..=m).map(move |t| ground(p[0] + d[0] * t as f64 * 660.0, p[1] + d[1] * t as f64 * 660.0));
    let flank = |d: [f64; 2]| near(d, 8).any(|z| z >= h0 + 350.0);
    let climb = |d: [f64; 2]| near(d, 12).any(|z| z <= h0 - 200.0);
    falls(u) && falls([-u[0], -u[1]]) && rises(n) && rises([-n[0], -n[1]]) && flank(n) && flank([-n[0], -n[1]]) && climb(u) && climb([-u[0], -u[1]])
}

/// Checks each settlement's reason where it finally stands (after `snap_to_water` and the roads
/// are built), not at the cell it was scored on: ore needs hill ground within a mile, a mouth its
/// river reaching the water within half a mile, a harbour a sheltered bay round the town itself,
/// a crossing a road over the river within half a mile, a pass the king's road over a saddle
/// through the town. One that fails gives way to the next reason that holds (else the kind
/// cascade), and its kind follows (a pin's own kind stays).
pub fn verify(inp: &Inputs, settlements: &mut [Settlement], at: &Placed) {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let sea = inp.world.params().sea_level_ft;
    let Some(g) = inp.ground.get() else { return };
    let ore_seed = inp.world.stream("t0.settle.ore");
    let centre = |k: u32| [(k as usize % w) as f64 * cell, (k as usize / w) as f64 * cell];
    let d2 = |a: [f64; 2], b: [f64; 2]| crate::core::sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]));
    let ray = |a: usize| {
        let ang = std::f64::consts::TAU * a as f64 / 48.0;
        [libm::cos(ang), libm::sin(ang)]
    };
    // Hill or mountain ground at the town: the ground rising 300 ft above it within a mile,
    // and the town itself 2,500 ft up (else a 450 ft rise: hills over a valley).
    let hilly = |c: [f64; 2]| {
        let h0 = (at.ground)(c[0], c[1]);
        let mut hi = h0;
        for j in -4i64..=4 {
            for i in -4i64..=4 {
                if i * i + j * j <= 16 {
                    hi = hi.max((at.ground)(c[0] + i as f64 * 1_320.0, c[1] + j as f64 * 1_320.0));
                }
            }
        }
        hi - h0 >= 300.0 && (h0 - sea >= 2_500.0 || hi - h0 >= 450.0)
    };
    // How sheltered a harbour the water by `c` is (0-1): of the ways out from it, how few reach
    // open water 1, 2 and 3 miles out (a bare coast looks out on it over half its sweep, a bay
    // only through its mouth), and the shore curving round the town (water a mile out over at
    // most a third of the sweep: a straight shore has it over half). None without water within
    // half a mile.
    let shelter = |c: [f64; 2]| {
        let shore = (0..48).any(|a| {
            let u = ray(a);
            (1..=26).any(|t| (at.wet)(c[0] + u[0] * t as f64 * 100.0, c[1] + u[1] * t as f64 * 100.0))
        });
        if !shore {
            return 0.0;
        }
        let open = (0..48)
            .filter(|&a| {
                let u = ray(a);
                [1.0, 2.0, 3.0].iter().all(|&m| (at.wet)(c[0] + u[0] * m * 5_280.0, c[1] + u[1] * m * 5_280.0))
            })
            .count();
        let mile = (0..48)
            .filter(|&a| {
                let u = ray(a);
                (at.wet)(c[0] + u[0] * 5_280.0, c[1] + u[1] * 5_280.0)
            })
            .count();
        (1.0 - smoothstep(0.2, 0.34, open as f64 / 48.0)).min(1.0 - smoothstep(0.3, 0.4, mile as f64 / 48.0))
    };
    // Where a mouth's river reaches its water: along from the cell before, the first wet point.
    let mouth_at = |k: u32, before: u32| {
        let (a, b) = (centre(before), centre(k));
        (0..=24).map(|i| i as f64 / 24.0).map(|t| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]).find(|p| (at.wet)(p[0], p[1])).unwrap_or(b)
    };
    // Roads with an end within reach of the town (its ways in): class and a point a mile out.
    let arms = |c: [f64; 2], reach: f64| -> Vec<(super::roads::RoadClass, [f64; 2])> {
        let mut out = Vec::new();
        for &(class, pts) in at.roads {
            if pts.len() < 2 {
                continue;
            }
            for flip in [false, true] {
                let p0 = if flip { pts[pts.len() - 1] } else { pts[0] };
                if d2(p0, c) > reach {
                    continue;
                }
                let mut run = 0.0;
                let mut far = p0;
                for i in 1..pts.len() {
                    let (a, b) = if flip { (pts[pts.len() - i], pts[pts.len() - 1 - i]) } else { (pts[i - 1], pts[i]) };
                    run += d2(a, b);
                    far = b;
                    if run >= 5_280.0 {
                        break;
                    }
                }
                out.push((class, far));
            }
        }
        out
    };
    // A pass: the king's road through the town (two ways in, or passing within 600 ft) at a
    // saddle (`saddle_at`).
    let saddle = |c: [f64; 2], reach: f64| {
        let unit = |d: [f64; 2]| {
            let l = crate::core::sqrt(d[0] * d[0] + d[1] * d[1]).max(1e-6);
            [d[0] / l, d[1] / l]
        };
        // Where the road is highest and which way it runs.
        let mut ways: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let kings: Vec<[f64; 2]> = arms(c, reach).into_iter().filter(|a| a.0 == super::roads::RoadClass::KingsRoad).map(|a| a.1).collect();
        for i in 0..kings.len() {
            for j in i + 1..kings.len() {
                let (a, b) = (unit([kings[i][0] - c[0], kings[i][1] - c[1]]), unit([kings[j][0] - c[0], kings[j][1] - c[1]]));
                if a[0] * b[0] + a[1] * b[1] <= -0.5 {
                    ways.push((c, unit([a[0] - b[0], a[1] - b[1]])));
                }
            }
        }
        for &(class, pts) in at.roads {
            if class != super::roads::RoadClass::KingsRoad {
                continue;
            }
            for k in 1..pts.len() {
                let (a, b) = (pts[k - 1], pts[k]);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l2 = dx * dx + dy * dy;
                let t = if l2 > 0.0 { (((c[0] - a[0]) * dx + (c[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                let p = [a[0] + t * dx, a[1] + t * dy];
                if d2(p, c) <= 600.0 {
                    // (The road's way over a mile either side, not one segment's.)
                    let (i, j) = (pts[..k].iter().rposition(|&q| d2(q, p) >= 2_640.0).unwrap_or(0), pts[k..].iter().position(|&q| d2(q, p) >= 2_640.0).map_or(pts.len() - 1, |e| k + e));
                    ways.push((p, unit([pts[j][0] - pts[i][0], pts[j][1] - pts[i][1]])));
                    break;
                }
            }
        }
        ways.iter().any(|&(p, u)| saddle_at(at.ground, p, u, sea))
    };
    // A road over a river within half a mile: a crossing on a road as built, or one in the town
    // a road runs through (two ways in).
    let on_road = |p: [f64; 2], tol: f64| {
        at.roads.iter().any(|&(_, pts)| {
            pts.windows(2).any(|q| {
                let (a, b) = (q[0], q[1]);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l2 = dx * dx + dy * dy;
                let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                d2(p, [a[0] + t * dx, a[1] + t * dy]) <= tol
            })
        })
    };
    let crossed = |c: [f64; 2], reach: f64| {
        at.crossings.iter().any(|&p| d2(p, c) <= 2_640.0 && (on_road(p, 400.0) || (d2(p, c) <= reach - 1_200.0 && arms(c, reach).len() >= 2)))
    };
    // (A market's distance to the nearest city, as the towns were placed: cells between cell centres.)
    let cities: Vec<usize> = settlements.iter().filter(|s| s.tier >= Tier::City && !s.isolated).map(|s| s.cell).collect();
    let cities_d = |k: usize| {
        let d = cities.iter().map(|&c| {
            let (dx, dy) = ((c % w) as f64 - (k % w) as f64, (c / w) as f64 - (k / w) as f64);
            dx * dx + dy * dy
        });
        let d = d.fold(f64::INFINITY, f64::min);
        if d.is_finite() { crate::core::sqrt(d) } else { 0.0 }
    };
    let near_source = |r: Reason, c: [f64; 2], d: f64| g.sources.iter().any(|&(sr, k, before, v)| sr == r && v >= 0.3 && d2(if r == Reason::Mouth { mouth_at(k, before) } else { centre(k) }, c) <= d);
    for s in settlements.iter_mut() {
        let c = [s.x, s.y];
        let k = s.cell;
        let reach = crate::town::urban_radius(s.tier, s.population) + 1_500.0;
        // What a reason is worth where the town stands (0: it does not hold there).
        let worth = |r: Reason| -> f64 {
            let v = if (r as usize) < 10 { g.site[k][r as usize] as f64 / 255.0 } else { 0.0 };
            match r {
                Reason::Harbour => v.min(0.8 * shelter(c)),
                // (A town stands back from its shore by a third of its radius.)
                Reason::Mouth => if near_source(Reason::Mouth, c, 3_000.0 + 0.33 * (reach - 1_500.0)) { v } else { 0.0 },
                Reason::Outlet | Reason::Confluence | Reason::Head => if near_source(r, c, 0.75 * cell) { v } else { 0.0 },
                // (A village's is a ford or bridge on its road; a city's only the river's bridging point.)
                Reason::Crossing if crossed(c, reach) => match s.tier {
                    Tier::Village => 0.55,
                    Tier::Town => v.max(0.55),
                    _ => v,
                },
                Reason::Crossing => 0.0,
                Reason::Ore => if hilly(c) { v } else { 0.0 },
                Reason::Pass => if s.tier == Tier::Town && saddle(c, reach) { 0.9 } else { 0.0 },
                // (A city's is its market: only the economy gives a city one, `retier`.)
                Reason::Crossroads => if s.tier >= Tier::City || (s.tier == Tier::Town && arms(c, reach).len() >= 3) { 0.75 } else { 0.0 },
                Reason::Food => if g.food[k] >= 0.5 { 1.0 } else { 0.0 },
                Reason::Spring => if s.tier <= Tier::Town { SPRING_REASON * g.spring[k] as f64 } else { 0.0 },
                Reason::Market => if s.tier == Tier::Town { MARKET_REASON * market_gap(cities_d(k), cell) } else { 0.0 },
                _ => v,
            }
        };
        if s.reason == Reason::None || worth(s.reason) > 0.3 {
            continue;
        }
        let was = s.reason;
        let mut reason = Reason::None;
        let mut best = 0.3;
        for r in [Reason::Harbour, Reason::Mouth, Reason::Outlet, Reason::Confluence, Reason::Head, Reason::Crossing, Reason::Ore, Reason::Timber, Reason::Fishing, Reason::Pass, Reason::Crossroads, Reason::Spring, Reason::Market] {
            // (The same reasons as `place` weighs for the tier; a village's crossing only on a road over a river.)
            if r == was || (s.tier < Tier::Town && matches!(r, Reason::Outlet | Reason::Head | Reason::Pass | Reason::Crossroads | Reason::Market)) || (s.tier >= Tier::City && matches!(r, Reason::Timber | Reason::Fishing | Reason::Pass | Reason::Crossroads | Reason::Spring | Reason::Market)) {
                continue;
            }
            let v = worth(r);
            if v > best + 1e-9 {
                (reason, best) = (r, v);
            }
        }
        if reason == Reason::None && g.food[k] >= 0.5 {
            reason = Reason::Food;
        }
        // (A city its trade grew from a town lives by its market: `retier`.)
        if reason == Reason::None && s.tier >= Tier::City && s.tier > s.planned {
            reason = Reason::Crossroads;
        }
        s.reason = reason;
        // (A realm's town keeps its kind: a fortress in the mountains, a market in the forest.)
        if (s.isolated && s.tier >= Tier::Town) || inp.pins.iter().any(|p| s.pin == Some(p.stroke as u32) && p.kind.is_some()) {
            continue;
        }
        let near_river = g.on_river[k] || neighbors(w, h, k).any(|(nb, _)| g.on_river[nb]);
        let (cx, cy) = ((k % w) as f64, (k / w) as f64);
        let ore = fbm(ore_seed, cx / 12.0, cy / 12.0, 3, 2.0, 0.5);
        let b = Biome::from_u8((inp.biome[k] & 0xff) as u8);
        s.kind = kind_of(reason, s.tier, b, inp.height[k] - sea, ore, s.coastal, near_river, g.on_river[k], hilly(c));
    }
}

/// Move each waterside settlement off its cell centre onto its water: ports and fishing
/// villages stand just inland of the real shoreline (by a third of their built-up radius, so
/// the town reaches the water), river towns just beside the river. `wet(x, y)` is standing
/// water (sea or lake); `river(x, y)` is the nearest river centreline point within reach:
/// (point, half width).
pub fn snap_to_water(settlements: &mut [Settlement], cell: f64, wet: &dyn Fn(f64, f64) -> bool, river: &dyn Fn(f64, f64) -> Option<([f64; 2], f64)>) {
    for s in settlements.iter_mut() {
        let r = crate::town::urban_radius(s.tier, s.population);
        let inland = 0.33 * r;
        // Nearest shore point by casting rays outward.
        let mut shore: Option<(f64, [f64; 2], [f64; 2])> = None;
        for a in 0..48 {
            let ang = std::f64::consts::TAU * a as f64 / 48.0;
            let u = [libm::cos(ang), libm::sin(ang)];
            let mut t = 25.0;
            while t < 2.2 * cell {
                let q = [s.x + u[0] * t, s.y + u[1] * t];
                if wet(q[0], q[1]) {
                    if shore.is_none_or(|b| t < b.0) {
                        shore = Some((t, q, u));
                    }
                    break;
                }
                t += 50.0;
            }
        }
        if s.coastal
            && let Some((_, q, u)) = shore
        {
            // Back inland from the shore until on dry ground.
            let mut back = inland;
            let mut c = [q[0] - u[0] * back, q[1] - u[1] * back];
            while wet(c[0], c[1]) && back < 3.0 * inland + 400.0 {
                back += 50.0;
                c = [q[0] - u[0] * back, q[1] - u[1] * back];
            }
            (s.x, s.y) = (c[0], c[1]);
            continue;
        }
        if s.river
            && let Some((p, hw)) = river(s.x, s.y)
        {
            let d = [s.x - p[0], s.y - p[1]];
            let dl = crate::core::sqrt(d[0] * d[0] + d[1] * d[1]).max(1e-6);
            let off = hw + 0.25 * r;
            (s.x, s.y) = (p[0] + d[0] / dl * off, p[1] + d[1] / dl * off);
        }
    }
}

/// Ruins and wizard towers, away from settlements; then cave mouths, mines and lava tubes
/// (`volcanoes`: T0 cell centre and cone radius ft).
/// Miles from each cell to the nearest of `own`'s settlements or a cell of the world's own roads
/// (`roads`: route usage per cell, empty with no roads): a two-pass 8-neighbour chamfer, so it
/// goes by the cells alone.
fn people_field(inp: &Inputs, own: &[&Settlement], roads: &[u8]) -> Vec<f32> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let mut d = vec![f32::MAX; w * h];
    for s in own {
        let (i, j) = (((s.x / cell).round().max(0.0) as usize).min(w - 1), ((s.y / cell).round().max(0.0) as usize).min(h - 1));
        d[j * w + i] = 0.0;
    }
    for (k, &u) in roads.iter().enumerate().take(w * h) {
        if u > 0 {
            d[k] = 0.0;
        }
    }
    let (a, b) = ((cell / 5280.0) as f32, (cell * std::f64::consts::SQRT_2 / 5280.0) as f32);
    let relax = |d: &mut [f32], x: usize, y: usize, nx: i64, ny: i64, c: f32| {
        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
            let v = d[ny as usize * w + nx as usize] + c;
            if v < d[y * w + x] {
                d[y * w + x] = v;
            }
        }
    };
    for y in 0..h {
        for x in 0..w {
            let (xi, yi) = (x as i64, y as i64);
            relax(&mut d, x, y, xi - 1, yi, a);
            relax(&mut d, x, y, xi, yi - 1, a);
            relax(&mut d, x, y, xi - 1, yi - 1, b);
            relax(&mut d, x, y, xi + 1, yi - 1, b);
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let (xi, yi) = (x as i64, y as i64);
            relax(&mut d, x, y, xi + 1, yi, a);
            relax(&mut d, x, y, xi, yi + 1, a);
            relax(&mut d, x, y, xi + 1, yi + 1, b);
            relax(&mut d, x, y, xi - 1, yi + 1, b);
        }
    }
    d
}

/// How willing a cave is to open this far (miles) from people: 0.2 at the 2-mi clear zone's edge
/// rising to 1.0 at 5 mi, 1.0 to 25 mi, falling fast to 0.05 at 35 mi and flat to 50, then 0.6
/// beyond (the wilds are their own draw, about a fifth of the caves).
fn cave_pull(mi: f64) -> f64 {
    const WILD: f64 = 0.6;
    if mi < 5.0 {
        0.2 + 0.8 * ((mi - 2.0) / 3.0).clamp(0.0, 1.0)
    } else if mi <= 25.0 {
        1.0
    } else if mi < 50.0 {
        if mi < 35.0 { 1.0 - 0.95 * (mi - 25.0) / 10.0 } else { 0.05 }
    } else {
        WILD
    }
}

pub fn place_pois(inp: &Inputs, settlements: &[Settlement], volcanoes: &[(f64, f64, f64)], roads: &[u8]) -> Vec<Poi> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let p = inp.world.params();
    // Each try draws from its own stream (where it looks, whether it takes the place, the seed),
    // so a try a settlement now blocks (or one more settlement's extra ruin) moves no other site.
    let stream = inp.world.stream("t0.poi");
    // How many there are, and which settlements mines are worked from, go by the world's own
    // settlements: one added beside them (`Pin::added`) only keeps sites off its ground.
    let own: Vec<&Settlement> = settlements.iter().filter(|s| !s.pin.is_some_and(|st| inp.pins.iter().any(|p| p.stroke == st as usize && p.added))).collect();
    let n_ruins = ((own.len() as f64) * 0.35 * p.poi_density).round() as usize;
    let n_towers = ((own.len() as f64) * 0.06 * p.poi_density).round().max(if p.poi_density > 0.0 { 1.0 } else { 0.0 }) as usize;
    let mut out: Vec<Poi> = Vec::new();
    let away = |x: f64, y: f64, out: &[Poi], min_s: f64, min_p: f64| {
        settlements.iter().all(|s| dist(s.x, s.y, x, y) >= min_s) && out.iter().all(|q| dist(q.x, q.y, x, y) >= min_p)
    };
    for (kind, count, min_s_mi) in [(PoiKind::Ruin, n_ruins, 6.0), (PoiKind::Tower, n_towers, 25.0)] {
        let mut tries = 0;
        let mut placed = 0;
        while placed < count && tries < count * 60 + 100 {
            tries += 1;
            let mut rng = Pcg32::new(hash3(stream, kind as i64, tries as i64, 0), 22);
            let k = rng.below((w * h) as u32) as usize;
            if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE {
                continue;
            }
            let b = Biome::from_u8((inp.biome[k] & 0xff) as u8);
            if matches!(b, Biome::Ice | Biome::SaltFlat) {
                continue;
            }
            // Ruins favour old, overgrown or buried places.
            if kind == PoiKind::Ruin && !matches!(b, Biome::Jungle | Biome::HotDesert | Biome::TemperateForest | Biome::Swamp) && rng.next_f64() < 0.5 {
                continue;
            }
            let (x, y) = ((k % w) as f64 * cell, (k / w) as f64 * cell);
            if !away(x, y, &out, min_s_mi * 5280.0, 8.0 * 5280.0) {
                continue;
            }
            let seed = rng.next_u32() as u64 | (rng.next_u32() as u64) << 32;
            if kind == PoiKind::Ruin && !dwarven_ground(inp, k, seed) {
                continue;
            }
            out.push(Poi { kind, x, y, seed, stroke: None });
            placed += 1;
        }
    }
    // Underground entrances, from their own stream (the surface sites above stay put).
    let stream = inp.world.stream("t0.poi.under");
    let sea = p.sea_level_ft;
    // Steepest drop to a neighbour (ft per cell): caves open in rugged ground and sea cliffs.
    let relief = |k: usize| {
        let (x, y) = (k % w, k / w);
        let mut r: f64 = 0.0;
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                r = r.max((inp.height[k] - inp.height[ny as usize * w + nx as usize].max(sea)).abs());
            }
        }
        r
    };
    let people = people_field(inp, &own, roads);
    let n_caves = ((own.len() as f64) * 0.2 * p.poi_density).round() as usize;
    let n_mines = ((own.len() as f64) * 0.1 * p.poi_density).round() as usize;
    for (kind, count) in [(PoiKind::Cave, n_caves), (PoiKind::Mine, n_mines)] {
        let (mut tries, mut placed) = (0, 0);
        while placed < count && tries < count * 600 + 600 {
            tries += 1;
            let mut rng = Pcg32::new(hash3(stream, kind as i64, tries as i64, 0), 23);
            let k = rng.below((w * h) as u32) as usize;
            if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE || inp.height[k] < sea + 20.0 {
                continue;
            }
            let b = Biome::from_u8((inp.biome[k] & 0xff) as u8);
            if matches!(b, Biome::SaltFlat | Biome::Swamp) || (b == Biome::Ice && kind == PoiKind::Mine) {
                continue;
            }
            let (x, y) = ((k % w) as f64 * cell, (k / w) as f64 * cell);
            let ok = match kind {
                PoiKind::Cave => relief(k) > 250.0 + 400.0 * rng.next_f64(),
                // Mines: hills, worked from a settlement a few miles off.
                _ => {
                    relief(k) > 150.0 && inp.height[k] > sea + 600.0 && own.iter().any(|s| (2.0 * 5280.0..15.0 * 5280.0).contains(&dist(s.x, s.y, x, y)))
                }
            };
            if !ok || !away(x, y, &out, 2.0 * 5280.0, 5.0 * 5280.0) {
                continue;
            }
            // Caves lean towards people (the try's own draw, so no other try moves).
            if kind == PoiKind::Cave && rng.next_f64() >= cave_pull(people[k] as f64) {
                continue;
            }
            out.push(Poi { kind, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32, stroke: None });
            placed += 1;
        }
    }
    // A lava tube on each volcano's flank, where it opens on dry land.
    for (v, &(vx, vy, r_ft)) in volcanoes.iter().enumerate() {
        let mut rng = Pcg32::new(hash3(stream, PoiKind::LavaTube as i64, v as i64, 0), 23);
        for _ in 0..12 {
            let a = rng.range(0.0, std::f64::consts::TAU);
            let d = r_ft * rng.range(0.7, 1.4);
            let (x, y) = (vx * cell + d * libm::cos(a), vy * cell + d * libm::sin(a));
            let (i, j) = ((x / cell).round(), (y / cell).round());
            if i < 0.0 || j < 0.0 || i >= w as f64 || j >= h as f64 {
                continue;
            }
            let k = j as usize * w + i as usize;
            if !inp.land[k] || inp.hydro.lake_of[k] != NO_LAKE || inp.height[k] < sea + 20.0 || !away(x, y, &out, 1.0 * 5280.0, 2.0 * 5280.0) {
                continue;
            }
            out.push(Poi { kind: PoiKind::LavaTube, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32, stroke: None });
            break;
        }
    }
    out
}

#[inline]
fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    crate::core::sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by))
}

/// v63: a ruin over a dwarven hall stands in hills or mountains; on flat ground 6 in 7 such tries
/// are passed over (the count of ruins stays: another try takes the place).
fn dwarven_ground(inp: &Inputs, k: usize, seed: u64) -> bool {
    let world = inp.world;
    // (Never a cave: the cold only matters to ice.)
    if crate::under::wild_theme(world, seed, crate::town::sites::ruin_under(world, seed), true, crate::under::Cold::No) != "dwarven_hall" {
        return true;
    }
    let (w, h, sea) = (inp.w, inp.h, world.params().sea_level_ft);
    let (x, y) = (k % w, k / w);
    let mut relief: f64 = 0.0;
    for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
            relief = relief.max((inp.height[k] - inp.height[ny as usize * w + nx as usize].max(sea)).abs());
        }
    }
    relief > 150.0 || inp.height[k] > sea + 1500.0 || crate::core::rng::unit(crate::core::rng::hash2(seed, 0x6477, 0)) < 1.0 / 7.0
}
