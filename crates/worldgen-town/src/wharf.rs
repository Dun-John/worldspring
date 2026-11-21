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
pub use crate::ground::wharf::{MADE_ABOVE_FT, SHORE_PIECE_FT, water, level, dry_side, strips, decks, bounds, shore_normals, shore_back, shore_pieces, shore_piers};
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
        return Some((crate::battle::hand_deck_top(c, world, t0), [c.a, c.b]));
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
    Some(crate::battle::hand_deck_ends(c, world, t0).level(t))
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

// R7 173: the shore wharf brush.

/// A shore quay's samples (ft apart), how far either side of a stroke its water's edge is looked
/// for, and how far out past the edge its face stands (as a town's quay fronts').
pub const SHORE_STEP_FT: f64 = 5.0;
pub const SHORE_REACH_FT: f64 = 60.0;
/// How far from a stroke the shore is looked for, to say how far off it is.
pub const SHORE_FAR_FT: f64 = 600.0;
pub const SHORE_FACE_OUT_FT: f64 = 3.0;

/// `p` resampled every `step` ft along it (its ends kept).
fn resample(p: &[P], step: f64) -> Vec<P> {
    let len: f64 = p.windows(2).map(|s| geom::dist(s[0], s[1])).sum();
    let n = crate::core::ceil(len / step).max(1.0) as usize;
    let step = len / n as f64;
    let (mut out, mut k, mut run) = (Vec::with_capacity(n + 1), 0, 0.0);
    for j in 0..=n {
        let at = j as f64 * step;
        while k + 2 < p.len() && run + geom::dist(p[k], p[k + 1]) < at {
            run += geom::dist(p[k], p[k + 1]);
            k += 1;
        }
        let d = geom::dist(p[k], p[k + 1]).max(1e-9);
        out.push(geom::lerp(p[k], p[k + 1], ((at - run) / d).clamp(0.0, 1.0)));
    }
    out
}

/// `v` averaged over `r` samples either way (weights falling off linearly; the ends held).
fn smooth_pts(v: &[P], r: usize) -> Vec<P> {
    let n = v.len();
    (0..n)
        .map(|i| {
            if i == 0 || i + 1 == n {
                return v[i];
            }
            let r = r.min(i).min(n - 1 - i) as i64;
            let (mut sum, mut w) = ([0.0, 0.0], 0.0);
            for j in -r..=r {
                let wj = (r + 1 - j.abs()) as f64;
                sum = geom::add(sum, geom::mul(v[(i as i64 + j) as usize], wj));
                w += wj;
            }
            geom::mul(sum, 1.0 / w)
        })
        .collect()
}

/// The water a shore quay is snapped to, as the map draws it: sea and lake over the drawn
/// ground (`town::Shore`, hand water, islets and plateaus too), and river channels.
pub struct ShoreWater<'a> {
    shore: crate::town::Shore<'a>,
}

impl<'a> ShoreWater<'a> {
    /// Over `rect` (x0, y0, x1, y1 world ft).
    pub fn new(world: &'a crate::world::World, t0: &'a T0, rect: [f64; 4]) -> ShoreWater<'a> {
        ShoreWater { shore: crate::town::Shore::new(world, t0, rect) }
    }

    /// Whether `p` (world ft) is in the water.
    pub fn wet(&self, p: P) -> bool {
        self.shore.wet(p) || self.shore.in_river(p)
    }
}

/// The water's edge across `p` along `n` (toward the water): the distance (ft, from `-reach` to
/// `reach`) where the land gives way to water that holds 16 ft (or to the reach). `None` when it
/// is wet at `-reach` or dry all the way.
fn edge_across(water: &ShoreWater, p: P, n: P, reach: f64) -> Option<f64> {
    let wet = |t: f64| water.wet(geom::add(p, geom::mul(n, t)));
    if wet(-reach) {
        return None;
    }
    let (mut t, mut start) = (-reach, None::<f64>);
    while t < reach {
        t += 2.0;
        if !wet(t) {
            start = None;
        } else if start.is_none() {
            start = Some(t);
        } else if start.is_some_and(|s0| t - s0 >= 16.0) {
            break;
        }
    }
    let (mut lo, mut hi) = (start? - 2.0, start?);
    for _ in 0..5 {
        let m = 0.5 * (lo + hi);
        if wet(m) {
            hi = m;
        } else {
            lo = m;
        }
    }
    Some(hi)
}

/// The water's edge along `line` (`edge_across` at each point, along its normals toward the
/// water, `out` the side: 1 left, -1 right), short gaps (up to 30 ft) filled in between their
/// neighbours: each run of it found (4 points or more), in order along `line`.
fn trace_edge(water: &ShoreWater, line: &[P], out: f64, reach: f64) -> Vec<Vec<P>> {
    let normal: Vec<P> = shore_normals(line).into_iter().map(|q| geom::mul(q, out)).collect();
    let mut t: Vec<Option<f64>> = line.iter().zip(&normal).map(|(&p, &n)| edge_across(water, p, n, reach)).collect();
    let n = t.len();
    let mut k = 0;
    while k < n {
        if t[k].is_some() {
            k += 1;
            continue;
        }
        let k0 = k;
        while k < n && t[k].is_none() {
            k += 1;
        }
        if k0 > 0 && k < n && k - k0 <= 6 {
            let (a, b) = (t[k0 - 1].unwrap(), t[k].unwrap());
            for (j, tj) in t.iter_mut().enumerate().take(k).skip(k0) {
                *tj = Some(a + (b - a) * (j + 1 - k0) as f64 / (k + 1 - k0) as f64);
            }
        }
    }
    let (mut runs, mut k) = (Vec::new(), 0);
    while k < n {
        let k0 = k;
        while k < n && t[k].is_some() {
            k += 1;
        }
        if k - k0 >= 4 {
            runs.push((k0..k).map(|k| geom::add(line[k], geom::mul(normal[k], t[k].unwrap()))).collect());
        }
        k = k.max(k0 + 1);
    }
    runs
}

/// How far (ft) from `line` the water's edge is found nearest (on rays every 22.5 degrees from
/// every fourth point, 5 ft steps), up to `max`.
fn nearest_shore(water: &ShoreWater, line: &[P], max: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for &p in line.iter().step_by(4).chain(line.last()) {
        let here = water.wet(p);
        for r in 0..16 {
            let a = r as f64 * core::f64::consts::TAU / 16.0;
            let d = [libm::cos(a), libm::sin(a)];
            let mut t = 5.0;
            while t <= best.unwrap_or(max).min(max) {
                if water.wet(geom::add(p, geom::mul(d, t))) != here {
                    best = Some(t);
                    break;
                }
                t += 5.0;
            }
        }
    }
    best
}

/// R7 173: the pieces of a quay brushed along the shore: `stroke` (world ft, as dragged near the
/// water's edge) snapped to the edge of the water the map draws (`ShoreWater`) within
/// `SHORE_REACH_FT` of it, smoothed, and its face set `SHORE_FACE_OUT_FT` out over the water: for
/// each part of the stroke that follows the shore, in order along it, the points of a shore quay
/// (`Wharf::shore`), every ~`SHORE_STEP_FT`, the dry side on the left. Or why none (no shore near
/// it, too short, too long).
pub fn shore_line(world: &crate::world::World, t0: &T0, stroke: &[P]) -> Result<Vec<Vec<P>>, String> {
    let stroke: Vec<P> = stroke.iter().copied().filter(|p| p[0].is_finite() && p[1].is_finite()).collect();
    let len: f64 = stroke.windows(2).map(|s| geom::dist(s[0], s[1])).sum();
    if stroke.len() < 2 || len < 20.0 {
        return Err("drag at least 20 ft along the water's edge".into());
    }
    if len > 1.5 * Wharf::MAX_LENGTH_FT {
        return Err(format!("a quay is at most {:.0} ft long (this stroke is {len:.0})", Wharf::MAX_LENGTH_FT));
    }
    let b = stroke.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let pad = SHORE_FAR_FT + 20.0;
    let water = ShoreWater::new(world, t0, [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad]);
    // (The stroke steadied over 20 ft either way, and which side of it the water lies: where more
    // of it is wet 30 ft off, else 60 ft.)
    let line = smooth_pts(&resample(&stroke, SHORE_STEP_FT), 4);
    let normal = shore_normals(&line);
    let wet_off = |s: f64| line.iter().zip(&normal).filter(|(p, n)| water.wet(geom::add(**p, geom::mul(**n, s)))).count();
    let (mut left, mut right) = (wet_off(30.0), wet_off(-30.0));
    if left == right {
        (left, right) = (left + wet_off(SHORE_REACH_FT), right + wet_off(-SHORE_REACH_FT));
    }
    let too_far = || match nearest_shore(&water, &line, SHORE_FAR_FT) {
        Some(d) if d > SHORE_REACH_FT => format!("draw along the water's edge: the nearest shore is {:.0} ft from the stroke", 5.0 * crate::core::round(d / 5.0)),
        Some(_) => "draw along the water's edge (on it, a little into the water or onto the land), not across it".into(),
        None => format!("draw along the water's edge: there is no shore within {SHORE_FAR_FT:.0} ft of the stroke"),
    };
    if left == 0 && right == 0 {
        return Err(too_far());
    }
    let out = if left >= right { 1.0 } else { -1.0 };
    // (First the edge across the stroke; then across that edge smoothed, which follows the
    // shore's own bends. Pieces in stroke order: `shore_face` turns each so its dry side is on
    // the left.)
    let mut pieces = Vec::new();
    for e1 in trace_edge(&water, &line, out, SHORE_REACH_FT) {
        let e1 = smooth_pts(&resample(&e1, SHORE_STEP_FT), 4);
        for e2 in trace_edge(&water, &e1, out, 25.0) {
            if let Some(pts) = shore_face(&e2, out) {
                pieces.push(pts);
            }
        }
    }
    if pieces.is_empty() {
        return Err(too_far());
    }
    for pts in &pieces {
        let run: f64 = pts.windows(2).map(|s| geom::dist(s[0], s[1])).sum();
        if run > Wharf::MAX_LENGTH_FT {
            return Err(format!("a quay is at most {:.0} ft long (this one along the shore is {run:.0})", Wharf::MAX_LENGTH_FT));
        }
        if pts.len() > Wharf::MAX_SHORE_PTS {
            return Err(format!("a quay along the shore has at most {} points", Wharf::MAX_SHORE_PTS));
        }
    }
    Ok(pieces)
}

/// A shore quay's face from the water's edge `e2` (traced with the water on side `out`): smoothed,
/// the dry side on the left, `SHORE_FACE_OUT_FT` out, in order along the shore, every
/// ~`SHORE_STEP_FT`. None when shorter than 20 ft.
fn shore_face(e2: &[P], out: f64) -> Option<Vec<P>> {
    let mut edge = smooth_pts(&resample(&smooth_pts(e2, 2), SHORE_STEP_FT), 3);
    // (The dry side on the left: the water on the right.)
    if out > 0.0 {
        edge.reverse();
    }
    let normal = shore_normals(&edge);
    let face = smooth_pts(&edge.iter().zip(&normal).map(|(&p, &n)| geom::sub(p, geom::mul(n, SHORE_FACE_OUT_FT))).collect::<Vec<P>>(), 1);
    // (In order along the shore: a point that would turn back is left out.)
    let mut kept: Vec<P> = Vec::with_capacity(face.len());
    for (k, &f) in face.iter().enumerate() {
        let tan = [normal[k][1], -normal[k][0]];
        if kept.last().is_some_and(|&l| geom::dot(geom::sub(f, l), tan) < 1.0) {
            continue;
        }
        kept.push(f);
    }
    if kept.len() < 2 {
        return None;
    }
    let mut pts: Vec<P> = resample(&kept, SHORE_STEP_FT).into_iter().map(|p| [crate::core::round(p[0] * 10.0) * 0.1, crate::core::round(p[1] * 10.0) * 0.1]).collect();
    pts.dedup_by(|a, b| geom::dist(*a, *b) < Wharf::MIN_SHORE_SIDE_FT);
    let run: f64 = pts.windows(2).map(|s| geom::dist(s[0], s[1])).sum();
    (pts.len() >= 4 && run >= 20.0).then_some(pts)
}

/// The cell (ft) of the shore brush's hint (`shore_hint`), and how many cells a side its tile has.
pub const SHORE_HINT_CELL_FT: f64 = 4.0;
pub const SHORE_HINT_CELLS: usize = 64;

/// R7 173: the water's edge the shore brush snaps to (`ShoreWater`) over the hint tile (i, j)
/// (`SHORE_HINT_CELLS` cells of `SHORE_HINT_CELL_FT` a side, from `i * side, j * side`): its
/// pieces (x0, y0, x1, y1 world ft), each across one cell between two crossings of the edge on
/// the cell's sides (placed to a foot).
pub fn shore_hint(world: &crate::world::World, t0: &T0, i: i64, j: i64) -> Vec<[f64; 4]> {
    let (c, n) = (SHORE_HINT_CELL_FT, SHORE_HINT_CELLS);
    let side = c * n as f64;
    let (x0, y0) = (i as f64 * side, j as f64 * side);
    let water = ShoreWater::new(world, t0, [x0 - c, y0 - c, x0 + side + c, y0 + side + c]);
    let at = |a: usize, b: usize| [(i * n as i64 + a as i64) as f64 * c, (j * n as i64 + b as i64) as f64 * c];
    let wet: Vec<bool> = (0..=n).flat_map(|b| (0..=n).map(move |a| (a, b))).map(|(a, b)| water.wet(at(a, b))).collect();
    let w = |a: usize, b: usize| wet[b * (n + 1) + a];
    // (Where the edge crosses the cell side p-q, wet `wp` at p: bisected.)
    let cross = |p: P, q: P, wp: bool| {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..3 {
            let m = 0.5 * (lo + hi);
            if water.wet(geom::lerp(p, q, m)) == wp {
                lo = m;
            } else {
                hi = m;
            }
        }
        geom::lerp(p, q, 0.5 * (lo + hi))
    };
    let mut out = Vec::new();
    for b in 0..n {
        for a in 0..n {
            let corners = [(a, b), (a + 1, b), (a + 1, b + 1), (a, b + 1)];
            let mut hits = Vec::with_capacity(4);
            for k in 0..4 {
                let (p, q) = (corners[k], corners[(k + 1) % 4]);
                if w(p.0, p.1) != w(q.0, q.1) {
                    hits.push(cross(at(p.0, p.1), at(q.0, q.1), w(p.0, p.1)));
                }
            }
            for h in hits.chunks_exact(2) {
                out.push([crate::core::round(h[0][0]), crate::core::round(h[0][1]), crate::core::round(h[1][0]), crate::core::round(h[1][1])]);
            }
        }
    }
    out
}
