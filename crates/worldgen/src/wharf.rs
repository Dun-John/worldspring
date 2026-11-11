//! Quays, piers and breakwaters built by hand (`Edits::wharves`, `q:<id>`): their geometry, a
//! pure function of the wharf and T0 (so every chunk and tile that meets one finds the same).
//!
//! A quay's line is its water edge; its paving reaches `width` ft to the dry side of each side,
//! decided once from T0 at the side's middle. A breakwater is a strip `width` ft across along its
//! line, a pier a deck as wide. Quays and breakwaters are made ground (`ground::Shaper` raises the
//! water inside them to their level); a pier's deck is lifted over the water as a hand bridge's.

use crate::t0::T0;
use crate::town::geom;
use crate::world::{Wharf, WharfKind};

type P = [f64; 2];

/// The ground ft above the water a quay's or breakwater's top stands at least, and at most.
pub const MADE_ABOVE_FT: (f32, f32) = (3.0, 10.0);

/// The highest water (ft) under a wharf's line: standing water every 10 ft along it, and any river
/// it crosses or comes within `margin` ft of. `hydro::DRY` where none.
pub fn water(w: &Wharf, t0: &T0, margin: f64) -> f32 {
    let cache = crate::lod::rivers::CrossingCache::default();
    let mut top = crate::t0::hydro::DRY;
    for s in w.pts.windows(2) {
        let n = (geom::dist(s[0], s[1]) / 10.0).ceil().max(1.0) as usize;
        for k in 0..=n {
            let p = geom::lerp(s[0], s[1], k as f64 / n as f64);
            top = top.max(t0.sample_water(p[0], p[1]));
        }
        if let Some((z, _)) = t0.rivers.crossing_level(s[0], s[1], margin, t0.cell_ft, &cache) {
            top = top.max(z as f32);
        }
    }
    top
}

/// The level (ft) of a wharf's top: a quay's or breakwater's the bank's ground at `pts[0]`, kept
/// `MADE_ABOVE_FT` over the water; a pier's deck clear of the water as a hand bridge's
/// (`battlemap::DECK_CLEARANCE_FT`). On dry ground, the ground at `pts[0]`.
pub fn level(w: &Wharf, t0: &T0) -> f32 {
    let bank = t0.ground_at(w.pts[0][0], w.pts[0][1], 5.0) as f32;
    let water = water(w, t0, margin(w));
    if water <= crate::t0::hydro::DRY {
        return bank;
    }
    match w.kind {
        WharfKind::Pier => water + crate::battlemap::DECK_CLEARANCE_FT,
        _ => bank.clamp(water + MADE_ABOVE_FT.0, water + MADE_ABOVE_FT.1),
    }
}

fn unit(v: P) -> P {
    geom::mul(v, 1.0 / geom::len(v).max(1e-9))
}

/// How far from its line (ft) a wharf's water is looked for.
fn margin(w: &Wharf) -> f64 {
    match w.kind {
        WharfKind::Quay => 10.0,
        _ => 0.5 * w.width,
    }
}

/// How wet T0 is at `p`: in a river's channel, very; else the standing water's depth there
/// (negative on dry land).
fn wetness(t0: &T0, p: P) -> f64 {
    let q = crate::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 0.0, t0.cell_ft);
    if (q.0 - p[0]).abs() + (q.1 - p[1]).abs() > 0.01 {
        return 1000.0;
    }
    let water = t0.sample_water(p[0], p[1]);
    if water <= crate::t0::hydro::DRY { -1000.0 } else { water as f64 - t0.ground_at(p[0], p[1], 5.0) }
}

/// The unit normal toward the dry side of the side a–b (a quay's), from T0 at its middle: the
/// left one (`[-u.y, u.x]`) unless the right is drier.
pub fn dry_side(t0: &T0, a: P, b: P, width: f64) -> P {
    let u = unit(geom::sub(b, a));
    let n = [-u[1], u[0]];
    let (m, d) = (geom::lerp(a, b, 0.5), (0.5 * width).max(10.0));
    let (left, right) = (wetness(t0, geom::add(m, geom::mul(n, d))), wetness(t0, geom::add(m, geom::mul(n, -d))));
    if right < left { [-n[0], -n[1]] } else { n }
}

/// The made ground of a quay or breakwater (world ft polygons; a pier has none): a quay's strip
/// from its line to the dry side, a breakwater's along its line, each side one quadrilateral and
/// each joint closed (a quay's by the wedge between its sides' strips, a breakwater's by a round).
pub fn strips(w: &Wharf, t0: &T0) -> Vec<Vec<P>> {
    let mut out = Vec::new();
    match w.kind {
        WharfKind::Pier => {}
        WharfKind::Quay => {
            let sides: Vec<P> = w.pts.windows(2).map(|s| dry_side(t0, s[0], s[1], w.width)).collect();
            for (s, n) in w.pts.windows(2).zip(&sides) {
                let o = geom::mul(*n, w.width);
                out.push(vec![s[0], s[1], geom::add(s[1], o), geom::add(s[0], o)]);
            }
            for k in 1..sides.len() {
                let p = w.pts[k];
                out.push(vec![p, geom::add(p, geom::mul(sides[k - 1], w.width)), geom::add(p, geom::mul(sides[k], w.width))]);
            }
        }
        WharfKind::Breakwater => {
            out.extend(decks(w));
        }
    }
    out.retain(|p| geom::area(p).abs() > 1.0);
    out
}

/// A pier's (or breakwater's) deck along its line, `width` ft across: one rectangle per side and
/// a round at each joint (world ft polygons).
pub fn decks(w: &Wharf) -> Vec<Vec<P>> {
    let mut out: Vec<Vec<P>> = w.pts.windows(2).map(|s| crate::town::rect(geom::lerp(s[0], s[1], 0.5), geom::sub(s[1], s[0]), geom::dist(s[0], s[1]), w.width)).collect();
    for &p in &w.pts[1..w.pts.len().saturating_sub(1)] {
        out.push(geom::circle(p, 0.5 * w.width, 12));
    }
    out
}

/// The bounds (world ft: x0, y0, x1, y1) a wharf's ground and decks keep within.
pub fn bounds(w: &Wharf) -> [f64; 4] {
    let r = w.width + 1.0;
    w.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - r), b[1].min(p[1] - r), b[2].max(p[0] + r), b[3].max(p[1] + r)])
}

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
/// (`END_CLEAR_FT` at each) are kept clear.
pub const SHOP_DEPTH_FT: f64 = 18.0;
pub const SHOP_FRONT_FT: f64 = 20.0;
pub const SHOP_LANE_FT: f64 = 14.0;
pub const MIN_LANE_FT: f64 = 10.0;
pub const END_CLEAR_FT: f64 = 20.0;

/// The lane (ft) left down the middle of hand bridge `c` with rows `depth` deep either side:
/// `width − 2·depth`, at least `MIN_LANE_FT`.
pub fn bridge_lane(c: &crate::world::Crossing, depth: f64) -> f64 {
    (c.width - 2.0 * depth).max(MIN_LANE_FT)
}

/// Shop lots (world ft rectangles) along both sides of hand bridge `c`'s deck, each `frontage`
/// along the bridge and up to `depth` across, against a middle lane `lane` ft wide (at least
/// `MIN_LANE_FT`), the `END_CLEAR_FT` at each end left clear: first the left side's from `a` to
/// `b`, then the right's. None on a ford or ferry, or a deck with no room for a row 8 ft deep.
pub fn bridge_lots(c: &crate::world::Crossing, lane: f64, depth: f64, frontage: f64) -> Vec<Vec<P>> {
    let lane = lane.max(MIN_LANE_FT);
    let depth = depth.min(0.5 * (c.width - lane));
    let len = c.length();
    if c.kind != crate::world::CrossingKind::Bridge || depth < 8.0 || frontage < 8.0 || !frontage.is_finite() || len < 1.0 {
        return Vec::new();
    }
    let usable = len - 2.0 * END_CLEAR_FT;
    let n = if usable >= frontage { (usable / frontage).floor() as usize } else { 0 };
    let u = geom::sub(c.b, c.a);
    let (u, v) = ([u[0] / len, u[1] / len], [-u[1] / len, u[0] / len]);
    let start = END_CLEAR_FT + 0.5 * (usable - n as f64 * frontage);
    let mut out = Vec::with_capacity(2 * n);
    for side in [1.0, -1.0] {
        let across = side * (0.5 * lane + 0.5 * depth);
        for k in 0..n {
            let along = start + (k as f64 + 0.5) * frontage;
            let m = [c.a[0] + u[0] * along + v[0] * across, c.a[1] + u[1] * along + v[1] * across];
            out.push(crate::town::rect(m, u, frontage, depth));
        }
    }
    out
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
}

impl Rows {
    pub fn width(&self) -> f64 {
        self.lane + 2.0 * self.depth
    }

    /// The default rows.
    pub fn standard() -> Rows {
        Rows { depth: SHOP_DEPTH_FT, frontage: SHOP_FRONT_FT, lane: SHOP_LANE_FT, lane_given: false }
    }
}

/// The trades shops take by turns along a bridge.
pub const TRADES: [&str; 6] = ["general_store", "tailor", "apothecary", "cobbler", "jeweler", "cartographer"];

/// The shop lots along bridge `c` as `rows` say, each with its trade (by turns), or why not (the rules mapd's
/// `place_crossing shops` and the app's Crossing mode share).
pub fn shop_lots(c: &crate::world::Crossing, rows: Rows) -> Result<Vec<(Vec<P>, &'static str)>, String> {
    if c.kind != crate::world::CrossingKind::Bridge {
        return Err(format!("shops: only a bridge has them (this is a {})", c.kind.name()));
    }
    if c.width + 1e-9 < rows.width() {
        return Err(format!("shops: rows {} ft deep either side of a lane {} ft wide need width_ft {} or more, this one is {}", rows.depth, rows.lane, rows.width(), c.width));
    }
    // The lane between the rows: as asked, else what the width leaves. The deck keeps `bridge_lane` clear down its middle (no building stands on it).
    let lane = if rows.lane_given { rows.lane } else { (c.width - 2.0 * rows.depth).max(rows.lane) };
    let kept = bridge_lane(c, SHOP_DEPTH_FT);
    if kept > lane + 1e-9 {
        return Err(format!("shops: a {} ft wide bridge keeps {kept} ft clear down its middle (width_ft less {}): rows {} ft deep with a lane of {lane} ft would stand on it; give lane_ft {kept} or more, or a narrower width_ft", c.width, 2.0 * SHOP_DEPTH_FT, rows.depth));
    }
    let lots = bridge_lots(c, lane, rows.depth, rows.frontage);
    if lots.is_empty() {
        return Err(format!("shops: a bridge {} ft long has no room for a row (20 ft are left clear at each end: it needs {} ft or more)", c.length().round(), rows.frontage + 2.0 * END_CLEAR_FT));
    }
    Ok(lots.into_iter().enumerate().map(|(k, lot)| (lot, TRADES[k % TRADES.len()])).collect())
}

/// Shops along bridge `c` (`id`) as `rows` say: each a drawn building `on` it, in a trade by turns. Their ids, or why not.
pub fn lay_shops(e: &mut crate::world::Edits, id: &str, c: &crate::world::Crossing, rows: Rows) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (lot, trade) in shop_lots(c, rows)? {
        let m = geom::centroid(&lot);
        let cid = format!("c:{}", e.created.len());
        e.created.push(crate::world::Created { id: cid.clone(), kind: "building".into(), x: m[0], y: m[1], poly: lot, floors: Some(2), func: Some(trade.into()), on: Some(id.to_string()), ..Default::default() });
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
        return Some((crate::battlemap::hand_deck_top(c, t0), [c.a, c.b]));
    }
    let w = world.file.edits.wharves.get(on).filter(|w| on.starts_with("q:") && w.kind != WharfKind::Quay && w.problem(map_w, map_h).is_none())?;
    Some((level(w, t0), [w.pts[0], *w.pts.last()?]))
}

/// The hand bridge whose deck footprint `poly` (world ft) stands wholly on, if one: its id, and
/// when it is over the bridge's lane (`bridge_lane` with rows `SHOP_DEPTH_FT` deep, where no
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
        let lane = bridge_lane(c, SHOP_DEPTH_FT);
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

/// Why a pier or breakwater can't start where it does: `pts[0]` and the point 5 ft behind it both wet, as a battlemap
/// draws the water (more than 5 ft into it). Names the bank to start from; a quay has no root.
pub fn root_problem(ex: &mut crate::pipeline::Executor, w: &Wharf) -> Option<String> {
    if w.kind == WharfKind::Quay || w.pts.len() < 2 {
        return None;
    }
    let (p, u) = (w.pts[0], unit(geom::sub(w.pts[1], w.pts[0])));
    let back = geom::sub(p, geom::mul(u, 5.0));
    let sq = crate::geoquery::Squares::round(ex, [p[0].min(back[0]) - 1.0, p[1].min(back[1]) - 1.0, p[0].max(back[0]) + 1.0, p[1].max(back[1]) + 1.0], 0);
    let wet_sq = |q: P| {
        let k = sq.at(crate::core::floor(q[0] / crate::battlemap::SQUARE_FT) as i64, crate::core::floor(q[1] / crate::battlemap::SQUARE_FT) as i64);
        k.is_some_and(|k| sq.hw[k] > sq.g[k])
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
