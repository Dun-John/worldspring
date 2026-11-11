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
    None
}
