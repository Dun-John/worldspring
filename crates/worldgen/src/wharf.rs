//! Quays, piers and breakwaters built by hand (`Edits::wharves`, `q:<id>`): their geometry, a
//! pure function of the wharf and T0 (so every chunk and tile that meets one finds the same).
//!
//! A quay's line is its water edge; its paving reaches `width` ft to the dry side of each side,
//! decided once from T0 at the side's middle. A breakwater is a strip `width` ft across along its
//! line, a pier a deck as wide. Quays and breakwaters are made ground (`ground::Shaper` raises the
//! water inside them to their level); a pier's deck is lifted over the water as a hand bridge's.

use crate::t0::T0;
use crate::town::geom;
use crate::world::{Crossing, CrossingKind, Wharf, WharfKind};
pub use crate::ground::wharf::{MADE_ABOVE_FT, water, level, dry_side, strips, decks, bounds};
use crate::ground::wharf::{unit, wetness};

type P = [f64; 2];

/// The wharves fit to build (`Wharf::problem`, the map's size aside, as `ground::Shaper` takes
/// them) whose bounds meet `rect` (world ft), by id.
pub fn near<'a>(world: &'a crate::world::World, rect: [f64; 4]) -> impl Iterator<Item = (&'a String, &'a Wharf)> + 'a {
    world.file.edits.wharves.iter().filter(move |(_, w)| {
        let b = bounds(w);
        b[2] >= rect[0] && b[0] <= rect[2] && b[3] >= rect[1] && b[1] <= rect[3] && w.problem(f64::MAX, f64::MAX).is_none()
    })
}

// R5 M20: shops on decks put down by hand.

/// A hand bridge's shops by default (ft): rows this deep either side of a lane this wide, each
/// lot this long along it; a lane is never narrower than `MIN_LANE_FT`; the ends of a bridge
/// (`END_CLEAR_FT` at each) are kept clear. Rows may be `MAX_SHOP_DEPTH_FT` deep at most; a bridge
/// keeps `bridge_lane` (rows that deep taken off its width) clear for its market street.
pub const SHOP_DEPTH_FT: f64 = 18.0;
pub const MAX_SHOP_DEPTH_FT: f64 = 40.0;
pub const SHOP_FRONT_FT: f64 = 20.0;
pub const SHOP_LANE_FT: f64 = 14.0;
pub const MIN_LANE_FT: f64 = 10.0;
pub const MAX_LANE_FT: f64 = 40.0;
pub const END_CLEAR_FT: f64 = 20.0;

/// Varied shops (`Rows::vary`): each lot's frontage is 0.7 to 1.6 of `SHOP_FRONT_FT` (14 to 32 ft), its depth 0.6 to 1
/// of the row's (the back line ragged), 1 to 3 storeys, and about one place in eight an open landing instead of a lot.
const VARY_FRONT: (f64, f64) = (0.7, 1.6);
const VARY_DEPTH_MIN: f64 = 0.6;
const GAP_CHANCE: f64 = 0.125;

/// The lane (ft) kept clear down the middle of hand bridge `c` (no building drawn by hand stands on it):
/// `width − 2·MAX_SHOP_DEPTH_FT`, at least `MIN_LANE_FT`.
pub fn bridge_lane(c: &crate::world::Crossing) -> f64 {
    (c.width - 2.0 * MAX_SHOP_DEPTH_FT).max(MIN_LANE_FT)
}

/// A key for the hashes that vary bridge `id`'s shops.
pub fn key_of(id: &str) -> u64 {
    crate::core::rng::stream(0x5b0b, id)
}

/// A shop lot: its footprint (world ft) and storeys.
pub type Lot = (Vec<P>, u8);

/// Shop lots (world ft rectangles) along both sides of hand bridge `c`'s deck, each up to `depth` across, against a
/// middle lane `lane` ft wide (at least `MIN_LANE_FT`), the `END_CLEAR_FT` at each end left clear: the left side's
/// lots from `a` to `b`, then the right's. Uniform lots `frontage` along when `key` is None; else varied
/// (`VARY_*`), a pure function of `key` and the lot's place along the deck (the first lot of the first side is
/// always as deep as `depth`). Empty on a ford or ferry, or a deck with no room for a row 8 ft deep.
pub fn bridge_sides(c: &crate::world::Crossing, lane: f64, depth: f64, frontage: f64, key: Option<u64>) -> [Vec<Lot>; 2] {
    let lane = lane.max(MIN_LANE_FT);
    let depth = depth.min(0.5 * (c.width - lane));
    let len = c.length();
    if c.kind != crate::world::CrossingKind::Bridge || depth < 8.0 || frontage < 8.0 || !frontage.is_finite() || len < 1.0 {
        return [Vec::new(), Vec::new()];
    }
    let usable = len - 2.0 * END_CLEAR_FT;
    let u = geom::sub(c.b, c.a);
    let (u, v) = ([u[0] / len, u[1] / len], [-u[1] / len, u[0] / len]);
    let mut out = [Vec::new(), Vec::new()];
    for (s, side) in [1.0, -1.0].into_iter().enumerate() {
        // (along from the first lot's start, frontage, depth, storeys)
        let mut cells: Vec<(f64, f64, f64, u8)> = Vec::new();
        let mut pos = 0.0;
        match key {
            None => {
                let n = if usable >= frontage { (usable / frontage).floor() as usize } else { 0 };
                cells.extend((0..n).map(|k| (k as f64 * frontage, frontage, depth, 2u8)));
                pos = n as f64 * frontage;
            }
            Some(key) => {
                for k in 0..200i64 {
                    let h = |w: i64| crate::core::rng::unit(crate::core::rng::hash3(key, s as i64, k, w));
                    let f = crate::core::round(SHOP_FRONT_FT * (VARY_FRONT.0 + (VARY_FRONT.1 - VARY_FRONT.0) * h(0)));
                    if k > 0 && h(1) < GAP_CHANCE {
                        let gap = crate::core::round(0.5 * f).max(8.0);
                        if pos + gap > usable {
                            break;
                        }
                        pos += gap;
                        continue;
                    }
                    if pos + f > usable {
                        break;
                    }
                    let d = if s == 0 && cells.is_empty() { depth } else { crate::core::round(depth * (VARY_DEPTH_MIN + (1.0 - VARY_DEPTH_MIN) * h(2))).max(8.0) };
                    cells.push((pos, f, d, 1 + ((h(3) * 3.0) as u8).min(2)));
                    pos += f;
                }
            }
        }
        let start = END_CLEAR_FT + 0.5 * (usable - pos);
        for (at, f, d, floors) in cells {
            let across = side * (0.5 * lane + 0.5 * d);
            let along = start + at + 0.5 * f;
            let m = [c.a[0] + u[0] * along + v[0] * across, c.a[1] + u[1] * along + v[1] * across];
            out[s].push((crate::town::rect(m, u, f, d), floors));
        }
    }
    out
}

/// `bridge_sides` uniform, as one list (the left side's lots, then the right's).
pub fn bridge_lots(c: &crate::world::Crossing, lane: f64, depth: f64, frontage: f64) -> Vec<Vec<P>> {
    bridge_sides(c, lane, depth, frontage, None).into_iter().flatten().map(|l| l.0).collect()
}

/// Whether a crossing from `a` to `b` (world ft) has water under it: a river's channel or standing
/// water somewhere along its middle (T0, every 5 ft from a tenth of the way to nine tenths). A
/// bridge with shops must (`shops_problem`).
pub fn crosses_water(t0: &T0, a: P, b: P) -> bool {
    let n = (geom::dist(a, b) / 5.0).ceil().max(1.0) as usize;
    (0..=n).any(|k| wetness(t0, geom::lerp(a, b, 0.1 + 0.8 * k as f64 / n as f64)) > 0.0)
}

/// Why bridge `c` can't carry shops where it stands (no water under it), if so.
pub fn shops_problem(t0: &T0, c: &crate::world::Crossing) -> Option<String> {
    (!crosses_water(t0, c.a, c.b)).then(|| format!("shops: a bridge with shops must cross water, and none is under this one from ({:.0}, {:.0}) to ({:.0}, {:.0}): put its ends on the two banks", c.a[0], c.a[1], c.b[0], c.b[1]))
}

/// Shop rows along a bridge: how deep, how long each lot, how wide the lane between.
#[derive(Clone, Copy, Debug)]
pub struct Rows {
    pub depth: f64,
    pub frontage: f64,
    pub lane: f64,
    /// `lane_ft` was given (else the lane is whatever the bridge's width leaves between the rows).
    pub lane_given: bool,
    /// Vary each lot's size, depth and storeys (`bridge_sides`), keyed by `key` (`key_of` the bridge's id).
    pub vary: bool,
    pub key: u64,
}

impl Rows {
    pub fn width(&self) -> f64 {
        self.lane + 2.0 * self.depth
    }

    /// The default rows.
    pub fn standard() -> Rows {
        Rows { depth: SHOP_DEPTH_FT, frontage: SHOP_FRONT_FT, lane: SHOP_LANE_FT, lane_given: false, vary: false, key: 0 }
    }
}

/// The trades shops take by turns along a bridge.
pub const TRADES: [&str; 6] = ["general_store", "tailor", "apothecary", "cobbler", "jeweler", "cartographer"];

/// A shop to build: footprint, trade and storeys.
pub struct Shop {
    pub poly: Vec<P>,
    pub func: &'static str,
    pub floors: u8,
}

/// The shop lots along bridge `c` as `rows` say, each with its trade (by turns), or why not (the rules mapd's
/// `place_crossing shops` and the app's Crossing mode share).
pub fn shop_lots(c: &crate::world::Crossing, rows: Rows) -> Result<Vec<Shop>, String> {
    if c.kind != crate::world::CrossingKind::Bridge {
        return Err(format!("shops: only a bridge has them (this is a {})", c.kind.name()));
    }
    if rows.depth > MAX_SHOP_DEPTH_FT + 1e-9 {
        return Err(format!("shops: rows are at most {MAX_SHOP_DEPTH_FT} ft deep"));
    }
    if c.width + 1e-9 < rows.width() {
        return Err(format!("shops: rows {} ft deep either side of a lane {} ft wide need width_ft {} or more, this one is {}", rows.depth, rows.lane, rows.width(), c.width));
    }
    // The lane between the rows: as asked, else what the width leaves. The deck keeps `bridge_lane` clear down its middle (no building stands on it).
    let lane = if rows.lane_given { rows.lane } else { (c.width - 2.0 * rows.depth).max(rows.lane) };
    let kept = bridge_lane(c);
    if kept > lane + 1e-9 {
        return Err(format!("shops: a {} ft wide bridge keeps {kept} ft clear down its middle (width_ft less {}): rows {} ft deep with a lane of {lane} ft would stand on it; give lane_ft {kept} or more, or a narrower width_ft", c.width, 2.0 * MAX_SHOP_DEPTH_FT, rows.depth));
    }
    let [l, r] = bridge_sides(c, lane, rows.depth, rows.frontage, rows.vary.then_some(rows.key));
    if l.is_empty() && r.is_empty() {
        return Err(format!("shops: a bridge {} ft long has no room for a row (20 ft are left clear at each end: it needs {} ft or more)", c.length().round(), rows.frontage + 2.0 * END_CLEAR_FT));
    }
    Ok(l.into_iter().chain(r).enumerate().map(|(k, (poly, floors))| Shop { poly, func: TRADES[k % TRADES.len()], floors }).collect())
}

/// Shops along bridge `c` (`id`) as `rows` say: each a drawn building `on` it, in a trade by turns. Their ids, or why not.
pub fn lay_shops(e: &mut crate::world::Edits, id: &str, c: &crate::world::Crossing, mut rows: Rows) -> Result<Vec<String>, String> {
    rows.key = key_of(id);
    let mut out = Vec::new();
    for s in shop_lots(c, rows)? {
        let m = geom::centroid(&s.poly);
        let cid = format!("c:{}", e.created.len());
        e.created.push(crate::world::Created { id: cid.clone(), kind: "building".into(), x: m[0], y: m[1], poly: s.poly, floors: Some(s.floors), func: Some(s.func.into()), on: Some(id.to_string()), ..Default::default() });
        out.push(cid);
    }
    Ok(out)
}

/// What a drawn building `on` stands on (`Created::on`): its deck's level (ft) and centre line
/// (world ft, the side its door faces). `v:<id>` a hand bridge (`battlemap::hand_deck_top`),
/// `q:<id>` a pier or breakwater (`level`, along its first to last point). `None` when there is
/// no such structure fit to build (a ford, a ferry, a quay; deleted).
pub fn deck_of(world: &crate::world::World, t0: &T0, on: &str) -> Option<(f32, [P; 2])> {
    let (map_w, map_h) = (world.geom.map_w_ft, world.geom.map_h_ft);
    if let Some(c) = world.file.edits.crossings.get(on).filter(|c| on.starts_with("v:") && c.kind == crate::world::CrossingKind::Bridge && c.problem(map_w, map_h).is_none()) {
        return Some((crate::battlemap::hand_deck_top(c, world, t0), [c.a, c.b]));
    }
    let w = world.file.edits.wharves.get(on).filter(|w| on.starts_with("q:") && w.kind != WharfKind::Quay && w.problem(map_w, map_h).is_none())?;
    Some((level(w, t0), [w.pts[0], *w.pts.last()?]))
}

/// `deck_of`'s level at `p` (world ft) along the deck: a hand bridge's deck climbs from end to end (`battlemap::hand_deck_ends`).
pub fn deck_level_at(world: &crate::world::World, t0: &T0, on: &str, p: P) -> Option<f32> {
    let (top, line) = deck_of(world, t0, on)?;
    let Some(c) = world.file.edits.crossings.get(on).filter(|_| on.starts_with("v:")) else { return Some(top) };
    let len = c.length().max(1e-6);
    let t = ((p[0] - line[0][0]) * (line[1][0] - line[0][0]) + (p[1] - line[0][1]) * (line[1][1] - line[0][1])) / (len * len);
    Some(crate::battlemap::hand_deck_ends(c, world, t0).level(t))
}

/// The hand bridge whose deck footprint `poly` (world ft) stands wholly on, if one: its id, and
/// when it is over the bridge's lane (`bridge_lane`, where no
/// building may stand), the lane's width. `None` when it is on no deck (or only partly on one).
pub fn deck_under(world: &crate::world::World, poly: &[P]) -> Option<(String, Option<f64>)> {
    if poly.len() < 3 {
        return None;
    }
    let mut probes: Vec<P> = vec![geom::centroid(poly)];
    for (k, &a) in poly.iter().enumerate() {
        let b = poly[(k + 1) % poly.len()];
        let n = (geom::dist(a, b) / 2.0).ceil().max(1.0) as usize;
        probes.extend((0..n).map(|s| geom::lerp(a, b, s as f64 / n as f64)));
    }
    let (map_w, map_h) = (world.geom.map_w_ft, world.geom.map_h_ft);
    for (id, c) in world.file.edits.crossings.iter().filter(|(_, c)| c.kind == crate::world::CrossingKind::Bridge && c.problem(map_w, map_h).is_none()) {
        let len = c.length();
        let u = [(c.b[0] - c.a[0]) / len, (c.b[1] - c.a[1]) / len];
        let frame = |p: P| {
            let d = geom::sub(p, c.a);
            (d[0] * u[0] + d[1] * u[1], -d[0] * u[1] + d[1] * u[0])
        };
        if !probes.iter().all(|&p| {
            let (along, across) = frame(p);
            (-0.5..=len + 0.5).contains(&along) && across.abs() <= 0.5 * c.width + 0.5
        }) {
            continue;
        }
        let lane = bridge_lane(c);
        let over = probes.iter().any(|&p| frame(p).1.abs() < 0.5 * lane - 0.5);
        return Some((id.clone(), over.then_some(lane)));
    }
    // A pier's or breakwater's deck: wholly within half its width of its line (no lane).
    for (id, w) in world.file.edits.wharves.iter().filter(|(_, w)| w.kind != WharfKind::Quay && w.problem(map_w, map_h).is_none()) {
        let reach = 0.5 * w.width + 0.5;
        if probes.iter().all(|&p| w.pts.windows(2).any(|s| geom::seg_dist(p, s[0], s[1]) <= reach)) {
            return Some((id.clone(), None));
        }
    }
    None
}

/// The water at a point (R5 M20 `raise_islet`, the app's islet tool): its kind (`river`, `lake`
/// or `sea`), the named body (id, name) when there is one, how deep it is (ft) and its level
/// (ft on the world's datum). `None` on dry land.
#[derive(Clone, Debug)]
pub struct WaterAt {
    pub kind: &'static str,
    pub id: Option<String>,
    pub name: Option<String>,
    pub depth_ft: f64,
    pub level: f32,
}

pub fn water_at(world: &crate::world::World, t0: &T0, p: P) -> Option<WaterAt> {
    water_over(world, t0, p).filter(|w| w.depth_ft > 0.0 || w.kind == "river")
}

/// `water_at` where T0's water level is above its ground or not (T0's shore is coarser than a battlemap's: its water may
/// lie a little under its ground at a square the battlemap draws wet).
pub fn water_over(world: &crate::world::World, t0: &T0, p: P) -> Option<WaterAt> {
    let still = t0.sample_water(p[0], p[1]);
    let river = t0.rivers.crossing_level(p, p, 0.0, t0.cell_ft, &crate::lod::rivers::CrossingCache::default()).map_or(crate::t0::hydro::DRY, |(z, _)| z as f32);
    let level = still.max(river);
    if level <= crate::t0::hydro::DRY {
        return None;
    }
    let depth_ft = (level as f64 - t0.ground_at(p[0], p[1], 5.0)).max(0.0);
    if river > still {
        let r = crate::agent::river_at(world, t0, p, 50.0);
        return Some(WaterAt { kind: "river", id: r.as_ref().and_then(|r| r.id.clone()), name: r.and_then(|r| r.name), depth_ft, level });
    }
    let body = crate::agent::water_body_at(world, t0, p);
    let kind = if (still as f64) <= world.params().sea_level_ft + 1.0 { "sea" } else { "lake" };
    Some(WaterAt { kind, id: body.as_ref().map(|b| b.0.clone()), name: body.map(|b| b.1), depth_ft, level })
}

/// A hand islet for `key` (`h:<id>`) round `at`: `ground::islet_for`, with the water it stands in.
pub fn islet_at(world: &crate::world::World, t0: &T0, key: &str, at: P, radius_ft: f64, rise_ft: f64) -> Result<(crate::ground::Plateau, WaterAt), String> {
    let p = crate::ground::islet_for(key, at, radius_ft, rise_ft, t0)?;
    let w = water_at(world, t0, at).ok_or_else(|| format!("at: ({:.0}, {:.0}) is on dry land: an islet stands in a river, lake or the sea", at[0], at[1]))?;
    Ok((p, w))
}

/// `islet_at` by the fine shore (as a battlemap draws the water at `at`: `ground::islet_for_fine`), for the agent's tool.
pub fn islet_at_fine(ex: &mut crate::pipeline::Executor, key: &str, at: P, radius_ft: f64, rise_ft: f64) -> Result<(crate::ground::Plateau, WaterAt), String> {
    let sq = crate::geoquery::Squares::round(ex, [at[0] - 3.0, at[1] - 3.0, at[0] + 3.0, at[1] + 3.0], 0);
    let fine = sq.at(crate::core::floor(at[0] / crate::battlemap::SQUARE_FT) as i64, crate::core::floor(at[1] / crate::battlemap::SQUARE_FT) as i64).map(|k| (sq.hw[k], sq.g[k]));
    let (world, t0) = (&ex.world, &ex.t0);
    let p = crate::ground::islet_for_fine(key, at, radius_ft, rise_ft, t0, fine)?;
    let w = water_over(world, t0, at).or_else(|| {
        fine.filter(|f| f.0 > f.1).map(|(hw, g)| WaterAt { kind: if (hw as f64) <= world.params().sea_level_ft + 1.0 { "sea" } else { "lake" }, id: None, name: None, depth_ft: (hw - g) as f64, level: hw })
    });
    let w = w.ok_or_else(|| format!("at: ({:.0}, {:.0}) is on dry land: an islet stands in a river, lake or the sea", at[0], at[1]))?;
    Ok((p, w))
}

/// Why a pier or breakwater can't start where it does: `pts[0]` and the point 5 ft behind it both wet, as a battlemap
/// draws the water (more than 5 ft into it). Names the bank to start from; a quay has no root.
pub fn root_problem(ex: &mut crate::pipeline::Executor, w: &Wharf) -> Option<String> {
    if w.kind == WharfKind::Quay || w.pts.len() < 2 {
        return None;
    }
    let (p, u) = (w.pts[0], unit(geom::sub(w.pts[1], w.pts[0])));
    let back = geom::sub(p, geom::mul(u, 5.0));
    let mut sq = crate::geoquery::Squares::round(ex, [p[0].min(back[0]) - 1.0, p[1].min(back[1]) - 1.0, p[0].max(back[0]) + 1.0, p[1].max(back[1]) + 1.0], 0);
    // (As the battlemap draws it: an islet's top, a plateau over the water and water drawn by hand count, so a pier may start on an islet.)
    sq.add_waters(&ex.world.file.edits);
    let ground = sq.shaped(&ex.world.file.edits, Some(&ex.t0));
    let wet_sq = |q: P| {
        let k = sq.at(crate::core::floor(q[0] / crate::battlemap::SQUARE_FT) as i64, crate::core::floor(q[1] / crate::battlemap::SQUARE_FT) as i64);
        k.is_some_and(|k| sq.hw[k] > ground[k])
    };
    if !wet_sq(p) || !wet_sq(back) {
        return None;
    }
    let (world, t0) = (&ex.world, &ex.t0);
    let r = |q: P| [crate::core::round(q[0]), crate::core::round(q[1])];
    let where_ = |q: P| format!("({}, {})", r(q)[0], r(q)[1]);
    if let Some(rv) = crate::agent::river_at(world, t0, p, 50.0).filter(|rv| rv.in_water()) {
        let (b, _) = rv.bank_of(p, 5.0);
        let what = rv.name.clone().map_or_else(|| "a river".to_string(), |n| format!("{n} ({})", rv.id.clone().unwrap_or_default()));
        return Some(format!("path[0]: {} is in the water of {what}, {:.0} ft wide there: a {} starts on the bank, at about {}", where_(p), rv.width_ft, w.kind.name(), where_(b)));
    }
    let what = water_over(world, t0, p).map_or_else(|| "the water".to_string(), |a| match (a.name, a.id) {
        (Some(n), Some(i)) => format!("the water of {n} ({i})"),
        _ => format!("a {}", a.kind),
    });
    // The nearest dry square within 600 ft (rings of 40 ft over a box).
    let big = crate::geoquery::Squares::round(ex, [p[0] - 600.0, p[1] - 600.0, p[0] + 600.0, p[1] + 600.0], 0);
    let dry = (1..=120).flat_map(|k| (0..16).map(move |a| (k as f64 * 5.0, a))).find_map(|(d, a)| {
        let t = std::f64::consts::TAU * a as f64 / 16.0;
        let q = [p[0] + d * libm::cos(t), p[1] + d * libm::sin(t)];
        let k = big.at(crate::core::floor(q[0] / crate::battlemap::SQUARE_FT) as i64, crate::core::floor(q[1] / crate::battlemap::SQUARE_FT) as i64)?;
        (big.hw[k] <= big.g[k]).then_some(q)
    });
    let near = dry.map_or_else(String::new, |q| format!(": the nearest dry ground is at about {}", where_(q)));
    Some(format!("path[0]: {} is in {what}: a {} starts on the bank{near}", where_(p), w.kind.name()))
}

/// The footprint of a lighthouse at the tip of breakwater `w` (a round tower, world ft), wholly
/// on its strip: at most 9 ft in radius, set back from the last point. `None` for a quay or pier.
pub fn lighthouse_lot(w: &Wharf) -> Option<Vec<P>> {
    if w.kind != WharfKind::Breakwater || w.pts.len() < 2 {
        return None;
    }
    let r = (0.5 * w.width - 1.5).clamp(4.5, 9.0);
    let (tip, u) = (*w.pts.last()?, unit(geom::sub(w.pts[w.pts.len() - 1], w.pts[w.pts.len() - 2])));
    let c = geom::sub(tip, geom::mul(u, r + 2.0));
    let c = [crate::core::round(c[0] * 2.0) * 0.5, crate::core::round(c[1] * 2.0) * 0.5];
    Some(geom::circle(c, r, 16))
}

/// A wharf's name before renames ("Quay", "Pier", "Breakwater").
pub fn generated_name(w: &Wharf) -> String {
    let n = w.kind.name();
    format!("{}{}", n[..1].to_uppercase(), &n[1..])
}

/// A wharf's middle: halfway along its path (by length).
pub fn mid(w: &Wharf) -> P {
    let half = 0.5 * w.length();
    let mut run = 0.0;
    for s in w.pts.windows(2) {
        let d = geom::dist(s[0], s[1]);
        if run + d >= half && d > 0.0 {
            return geom::lerp(s[0], s[1], (half - run) / d);
        }
        run += d;
    }
    w.pts.first().copied().unwrap_or([0.0, 0.0])
}

/// `Edits::follow_crossing`, here above the model crate (`world`).
pub trait FollowCrossing {
    fn follow_crossing(&mut self, id: &str, old: Option<&Crossing>) -> Vec<String>;
}

impl FollowCrossing for crate::world::Edits {
    /// (R5 M20) Shops follow their bridge: the drawn buildings `on` crossing `id` once it has
    /// changed (`old`: it as it was, if known). Gone, or no longer a bridge: they are removed.
    /// Its ends or width changed: they are laid out again on it (`wharf::bridge_lots`, as deep
    /// and as long as they were), each side's in order along it; those left without a lot are
    /// removed. Returns the ids of the shops changed.
    fn follow_crossing(&mut self, id: &str, old: Option<&Crossing>) -> Vec<String> {
        use crate::wharf;
        let shops: Vec<usize> = (0..self.created.len()).filter(|&k| self.created[k].on.as_deref() == Some(id) && !self.created[k].removed && self.created[k].kind == "building").collect();
        if shops.is_empty() {
            return Vec::new();
        }
        let new = self.crossings.get(id).filter(|c| c.kind == CrossingKind::Bridge && c.width.is_finite() && c.length() >= 1.0).cloned();
        let mut changed = Vec::new();
        let Some(new) = new else {
            for k in shops {
                self.created[k].removed = true;
                changed.push(self.created[k].id.clone());
            }
            return changed;
        };
        if old.is_some_and(|o| o.a == new.a && o.b == new.b && o.width == new.width && o.kind == new.kind) {
            return changed;
        }
        let was = old.filter(|o| o.length() >= 1.0).unwrap_or(&new);
        let (len, u) = (was.length(), [(was.b[0] - was.a[0]) / was.length(), (was.b[1] - was.a[1]) / was.length()]);
        let frame = |p: [f64; 2]| {
            let d = [p[0] - was.a[0], p[1] - was.a[1]];
            (d[0] * u[0] + d[1] * u[1], -d[0] * u[1] + d[1] * u[0])
        };
        let extent = |poly: &[[f64; 2]]| {
            let (mut a, mut c) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
            for &p in poly {
                let (x, y) = frame(p);
                (a, c) = ((a.0.min(x), a.1.max(x)), (c.0.min(y), c.1.max(y)));
            }
            (a.1 - a.0, c.1 - c.0, 0.5 * (a.0 + a.1), 0.5 * (c.0 + c.1))
        };
        let first = extent(&self.created[shops[0]].poly);
        let all: Vec<(f64, f64, f64, f64)> = shops.iter().map(|&k| extent(&self.created[k].poly)).collect();
        // Varied rows (lots of different frontage or depth) are laid out again varied, from the bridge's id; their deepest is the row's depth.
        let vary = all.iter().any(|e| (e.0 - first.0).abs() > 0.5 || (e.1 - first.1).abs() > 0.5);
        let (frontage, depth) = if !(first.0.is_finite() && first.0 >= 8.0 && first.1 >= 8.0 && len >= 1.0) {
            (wharf::SHOP_FRONT_FT, wharf::SHOP_DEPTH_FT)
        } else if vary {
            (wharf::SHOP_FRONT_FT, all.iter().fold(0.0f64, |m, e| m.max(e.1)).round())
        } else {
            (first.0, first.1)
        };
        let lane = (new.width - 2.0 * depth).max(wharf::MIN_LANE_FT);
        let lots = wharf::bridge_sides(&new, lane, depth, frontage, vary.then(|| wharf::key_of(id)));
        let per_side = [lots[0].len(), lots[1].len()];
        let mut sides: [Vec<(f64, usize)>; 2] = [Vec::new(), Vec::new()];
        for &k in &shops {
            let e = extent(&self.created[k].poly);
            sides[(e.3 < 0.0) as usize].push((e.2, k));
        }
        for (s, list) in sides.iter_mut().enumerate() {
            list.sort_by(|p, q| p.0.total_cmp(&q.0));
            for (n, &(_, k)) in list.iter().enumerate() {
                let c = &mut self.created[k];
                match lots[s].get(n).filter(|_| n < per_side[s]) {
                    Some((lot, _)) => {
                        let m = crate::core::geom::centroid(lot);
                        (c.poly, c.x, c.y) = (lot.clone(), m[0], m[1]);
                    }
                    None => c.removed = true,
                }
                changed.push(c.id.clone());
            }
        }
        changed
    }
}
