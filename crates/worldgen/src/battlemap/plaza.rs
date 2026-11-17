//! Squares, markets, parks and baileys dressed with props.

use super::*;

/// How far (squares, by the most of either way, to 12) each square is from one a lawn's knoll
/// mustn't touch: off the lawn and its gravel, a building, a road, ground within a foot of the water
/// below it (in a park, the water), a park's pond's or stream's banks (`park` bit 4, as `enforce`
/// has it), and the chunk's edge.
pub(super) fn lawn_room(c: &Chunk, park: &[u8]) -> Vec<u8> {
    let mut d = vec![u8::MAX; SQ * SQ];
    let mut queue = std::collections::VecDeque::new();
    for k in 0..SQ * SQ {
        let (x, y) = (k % SQ, k / SQ);
        let edge = x == 0 || y == 0 || x == SQ - 1 || y == SQ - 1;
        // (In a park only its water counts: the ground of a river's flat may stand a foot over the
        // water's level all across it.)
        let banks = park.get(k).is_some_and(|&p| p & 4 != 0);
        let shore = c.water_level[k] > c.height[k] - if park.get(k).is_some_and(|&p| p & 3 != 0) { 0.0 } else { 1.0 };
        if edge || banks || shore || !matches!(c.urban[k], URBAN_PARK | URBAN_PATH) || c.building[k] != 0 || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 {
            d[k] = if edge && matches!(c.urban[k], URBAN_PARK | URBAN_PATH) { 1 } else { 0 };
            queue.push_back(k);
        }
    }
    while let Some(k) = queue.pop_front() {
        if d[k] >= 12 {
            continue;
        }
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                    continue;
                }
                let n = ny as usize * SQ + nx as usize;
                if d[n] > d[k] + 1 {
                    d[n] = d[k] + 1;
                    queue.push_back(n);
                }
            }
        }
    }
    d
}

/// How a market fills its square: half-extents (ft, ranges) of the part round the well it keeps
/// to, the share of its cells left open, the spacing of stalls along a block's edge (ft) and the
/// share of stalls left out.
#[derive(Clone, Copy)]
pub(super) struct Market {
    hu: (f64, f64),
    hv: (f64, f64),
    open: f64,
    spacing: f64,
    drop: f64,
}

impl Market {
    pub(super) fn of(kind: crate::town::PlazaKind) -> Option<Market> {
        use crate::town::PlazaKind as K;
        match kind {
            K::Market => Some(Market { hu: (110.0, 150.0), hv: (80.0, 110.0), open: 0.12, spacing: 9.2, drop: 0.04 }),
            // Few stalls: wide lanes, many open cells, gaps along the rows.
            K::MarketSparse => Some(Market { hu: (100.0, 140.0), hv: (70.0, 100.0), open: 0.4, spacing: 11.5, drop: 0.2 }),
            // A market day's huddle round the well.
            K::MarketDay => Some(Market { hu: (55.0, 80.0), hv: (40.0, 60.0), open: 0.12, spacing: 9.2, drop: 0.06 }),
            _ => None,
        }
    }
}

/// A polygon's bounds (world ft): x0, y0, x1, y1.
pub(super) fn bounds(poly: &[[f64; 2]]) -> (f64, f64, f64, f64) {
    poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])))
}

/// A prop's footprint radius (ft).
fn foot(kind: Kind) -> f64 {
    info(kind as u16).radius as f64 * SQUARE_FT
}

/// Room for props on an open space (world ft): spots on the battlemap's squares inside it, each
/// kept clear of the lanes across it (streets and roads: centre line, half width), the buildings
/// round it (and on it: a temple on its close), its monuments and what is set out on it already
/// (centre, radius).
pub(super) struct Room<'a> {
    poly: &'a [[f64; 2]],
    c: [f64; 2],
    lanes: Vec<([f64; 2], [f64; 2], f64)>,
    blds: Vec<&'a [[f64; 2]]>,
    pub(super) taken: Vec<([f64; 2], f64)>,
    /// The spots, each with how far it is inside the outline, and (worked out when first asked:
    /// NaN till then) how far it is clear of the lanes (past their half width) and of buildings
    /// (ft; below 0 inside one).
    grid: Vec<([f64; 2], f64, std::cell::Cell<f64>, std::cell::Cell<f64>)>,
    seed: u64,
}

impl<'a> Room<'a> {
    pub(super) fn new(poly: &'a [[f64; 2]], l: &'a crate::town::Layout, lanes: &[([f64; 2], [f64; 2], f64)], taken: &[([f64; 2], f64)], seed: u64) -> Room<'a> {
        use crate::town::geom::{centroid, contains, dist};
        let (x0, y0, x1, y1) = bounds(poly);
        let near = |b: (f64, f64, f64, f64), m: f64| b.0 <= x1 + m && b.2 >= x0 - m && b.1 <= y1 + m && b.3 >= y0 - m;
        let lanes = lanes.iter().copied().filter(|&(a, b, w)| near((a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])), w + 15.0)).collect();
        let blds = l.buildings.iter().map(|b| b.poly.as_slice()).filter(|b| near(bounds(b), 15.0)).collect();
        let mut taken = taken.to_vec();
        for m in l.monuments.iter().filter(|m| near(bounds(m), 0.0)) {
            let mc = centroid(m);
            taken.push((mc, m.iter().map(|q| dist(*q, mc)).fold(0.0, f64::max) + 3.0));
        }
        let mut room = Room { poly, c: centroid(poly), lanes, blds, taken, grid: Vec::new(), seed };
        // Square centres (by their global index on the world's 5-ft grid, as every chunk lays its
        // squares).
        let cell = |v: f64| crate::core::floor(v / SQUARE_FT) as i64;
        for j in cell(y0)..=cell(y1) {
            for i in cell(x0)..=cell(x1) {
                let p = [(i as f64 + 0.5) * SQUARE_FT, (j as f64 + 0.5) * SQUARE_FT];
                if contains(poly, p) {
                    room.grid.push((p, room.depth(p), std::cell::Cell::new(f64::NAN), std::cell::Cell::new(f64::NAN)));
                }
            }
        }
        room
    }

    /// How far `p` is from the lanes, past their half width (ft).
    fn lane_gap(&self, p: [f64; 2]) -> f64 {
        self.lanes.iter().map(|&(a, b, w)| crate::town::geom::seg_dist(p, a, b) - w).fold(f64::MAX, f64::min)
    }

    /// How far `p` is from the buildings (ft; -1 inside one).
    fn bld_gap(&self, p: [f64; 2]) -> f64 {
        use crate::town::geom::{contains, seg_dist};
        self.blds.iter().map(|b| if contains(b, p) { -1.0 } else { (0..b.len()).map(|e| seg_dist(p, b[e], b[(e + 1) % b.len()])).fold(f64::MAX, f64::min) }).fold(f64::MAX, f64::min)
    }

    /// How far `p` lies inside the outline (ft; 0 outside it).
    fn depth(&self, p: [f64; 2]) -> f64 {
        use crate::town::geom::{contains, seg_dist};
        if !contains(self.poly, p) {
            return 0.0;
        }
        let m = self.poly.len();
        (0..m).map(|e| seg_dist(p, self.poly[e], self.poly[(e + 1) % m])).fold(f64::MAX, f64::min)
    }

    /// Whether a prop `r` ft round fits at `p` (off the lanes too, with `lanes`).
    fn free(&self, p: [f64; 2], r: f64, lanes: bool) -> bool {
        let nan = || std::cell::Cell::new(f64::NAN);
        self.fits(&(p, self.depth(p), nan(), nan()), r, lanes)
    }

    /// `free` for a spot of the grid (the cheap tests first; the gaps worked out once).
    fn fits(&self, (p, depth, lane, bld): &([f64; 2], f64, std::cell::Cell<f64>, std::cell::Cell<f64>), r: f64, lanes: bool) -> bool {
        let gap = |c: &std::cell::Cell<f64>, f: &dyn Fn([f64; 2]) -> f64| {
            if c.get().is_nan() {
                c.set(f(*p));
            }
            c.get()
        };
        *depth >= r + 1.0
            && self.taken.iter().all(|&(q, s)| (p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1]) >= (r + s + 2.0) * (r + s + 2.0))
            && (!lanes || gap(lane, &|p| self.lane_gap(p)) >= r + 1.0)
            && gap(bld, &|p| self.bld_gap(p)) >= r + 3.0
    }

    /// A little noise by spot (0–1), pure in the spot.
    fn jit(&self, p: [f64; 2], salt: i64) -> f64 {
        unit(hash3(self.seed, crate::core::round(p[0] * 2.0) as i64, crate::core::round(p[1] * 2.0) as i64, salt))
    }

    /// The free spot for a prop `r` ft round that scores best, taken; none if nothing fits.
    fn best(&mut self, r: f64, lanes: bool, score: impl Fn(&Room, [f64; 2]) -> f64) -> Option<[f64; 2]> {
        let mut best: Option<(f64, [f64; 2])> = None;
        for spot in &self.grid {
            if self.fits(spot, r, lanes) {
                let (p, s) = (spot.0, score(self, spot.0));
                if best.is_none_or(|b| s > b.0) {
                    best = Some((s, p));
                }
            }
        }
        let p = best?.1;
        self.taken.push((p, r));
        Some(p)
    }

    /// Lamp posts of one look round the square, spread apart: each at about `at` ft from the middle
    /// (or `at` ft in from the edge, with `edge`), as far as it can be from the others.
    fn lamps(&mut self, n: i64, at: f64, edge: bool, look: u8, out: &mut Vec<([f64; 2], f32, Kind, u8)>) {
        use crate::town::geom::dist;
        let mut lamps: Vec<[f64; 2]> = Vec::new();
        for k in 0..n {
            let spread = |r: &Room, p: [f64; 2]| {
                let off = if edge { r.depth(p) - at } else { dist(p, r.c) - at };
                -0.4 * off.abs() + 0.15 * lamps.iter().map(|q| dist(*q, p)).fold(150.0, f64::min) + r.jit(p, 70 + k)
            };
            if let Some(p) = self.best(foot(Kind::LampPost) + 2.0, true, spread) {
                lamps.push(p);
                out.push((p, 0.0, Kind::LampPost, look));
            }
        }
    }

    /// The direction (rad) of the outline's edge nearest `p`.
    fn along(&self, p: [f64; 2]) -> f32 {
        use crate::town::geom::seg_dist;
        let m = self.poly.len();
        let e = (0..m).min_by(|&a, &b| seg_dist(p, self.poly[a], self.poly[(a + 1) % m]).total_cmp(&seg_dist(p, self.poly[b], self.poly[(b + 1) % m]))).unwrap_or(0);
        let (a, b) = (self.poly[e], self.poly[(e + 1) % m]);
        libm::atan2(b[1] - a[1], b[0] - a[0]) as f32
    }
}

/// A lamp post's look for a square's set (one look for all): lit iron or two lanterns on a rich
/// square, else any but the unlit one, now and then that.
fn lamp_look(rng: &mut Pcg32, rich: bool) -> u8 {
    if rich {
        [0, 2][rng.below(2) as usize]
    } else if rng.next_f64() < 0.15 {
        3
    } else {
        rng.below(3) as u8
    }
}

/// A market cross by the well of a main square, 22 to 40 ft out along its long axis: on some
/// markets (a buttercross, mostly) and every civic square, its one landmark (taken in `room`).
/// Pure in the plaza.
pub(super) fn main_cross(pl: &crate::town::Plaza, room: &mut Room, seed: u64) -> Option<([f64; 2], f32, Kind, u8)> {
    use crate::town::PlazaKind as K;
    use crate::town::geom::{add, main_axis, mul};
    let c = room.c;
    let mut rng = Pcg32::new(hash2(seed ^ 0xc205, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 23);
    let chance = match pl.kind {
        K::Market | K::MarketSparse => 0.45,
        K::MarketDay => 0.3,
        _ => 1.0,
    };
    if rng.next_f64() >= chance {
        return None;
    }
    let look = if pl.kind.is_market() { [1, 1, 0, 3][rng.below(4) as usize] } else { [0, 3, 2][rng.below(3) as usize] };
    let u = main_axis(&pl.pts);
    let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
    // (A civic square's, its landmark, also further out or across the axis where a street
    // crosses the middle.)
    let (ds, axes): (&[f64], &[[f64; 2]]) = if pl.kind == K::Civic { (&[28.0, 34.0, 22.0, 40.0, 50.0, 60.0], &[u, [-u[1], u[0]]]) } else { (&[28.0, 34.0, 22.0, 40.0], &[u]) };
    for &a in axes {
        for &d in ds {
            for s in [side, -side] {
                let p = add(c, mul(a, d * s));
                if room.free(p, cross_ft() + 3.0, true) {
                    room.taken.push((p, cross_ft()));
                    return Some((p, 0.0, Kind::MarketCross, look));
                }
            }
        }
    }
    None
}

/// A main square's edge, once its market and features are laid out (taken in `room`): a notice
/// board (a civic square two), a trough or two along the edge, a city's lamp posts spread round it
/// (one look for all), and a civic square's stocks or pillory (a market day's now and then). Pure
/// in the plaza.
pub(super) fn dress_main(pl: &crate::town::Plaza, l: &crate::town::Layout, room: &mut Room, seed: u64, out: &mut Vec<([f64; 2], f32, Kind, u8)>) {
    use crate::town::PlazaKind as K;
    let c = room.c;
    let mut rng = Pcg32::new(hash2(seed ^ 0xd3a1, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 29);
    let civic = pl.kind == K::Civic;
    let city = l.tier >= crate::t0::settle::Tier::City;
    // Just inside the edge (`want` ft in), here or there.
    let edge = |want: f64, salt: i64| move |r: &Room, p: [f64; 2]| -(r.depth(p) - want).abs() + 3.0 * r.jit(p, salt);
    for k in 0..1 + civic as i64 {
        if let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(6.0, 10 + k)) {
            out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
        }
    }
    let trough = if city { [1, 3][rng.below(2) as usize] } else { [0, 2, 1][rng.below(3) as usize] };
    for k in 0..1 + (rng.next_f64() < 0.5) as i64 {
        if let Some(p) = room.best(foot(Kind::WaterTrough), true, edge(7.0, 20 + k)) {
            out.push((p, room.along(p), Kind::WaterTrough, trough));
        }
    }
    if city {
        let look = lamp_look(&mut rng, civic || l.tier == crate::t0::settle::Tier::Metropolis);
        room.lamps(4 + rng.below(3) as i64, 5.0, true, look, out);
    }
    if (civic && rng.next_f64() < 0.6) || (pl.kind == K::MarketDay && rng.next_f64() < 0.25) {
        let look = if civic { [0, 2, 3][rng.below(3) as usize] } else { [0, 3][rng.below(2) as usize] };
        if let Some(p) = room.best(foot(Kind::Stocks), true, edge(10.0, 40)) {
            out.push((p, 0.0, Kind::Stocks, look));
        }
    }
}

/// How much larger than its place a square's market cross is drawn (its steps and cross read from
/// afar; the squares keep room for it).
pub(super) const MARKET_CROSS_SCALE: f32 = 1.35;

/// A square's market cross's footprint radius (ft), as drawn.
pub(super) fn cross_ft() -> f64 {
    MARKET_CROSS_SCALE as f64 * foot(Kind::MarketCross)
}

/// A castle's well where its keep's court has no spot with 25 ft of room (`dress_bailey`): with
/// less room round it, in the court first, else anywhere in the bailey, by the keep. (v63: a well
/// in every castle, down to any free spot of the bailey.)
fn bailey_well(
    spots: &[[f64; 2]],
    placed: &[([f64; 2], f64)],
    well_r: f64,
    in_court: &dyn Fn([f64; 2]) -> bool,
    room: &dyn Fn([f64; 2]) -> f64,
    to_keep: &dyn Fn([f64; 2]) -> f64,
    free: &dyn Fn(&[([f64; 2], f64)], [f64; 2], f64, Option<usize>) -> bool,
) -> Option<[f64; 2]> {
    for court in [true, false] {
        for least in [18.0, 12.0, 6.0, 0.0] {
            let well = spots
                .iter()
                .copied()
                .filter(|&p| (!court || in_court(p)) && room(p) >= least && free(placed, p, well_r, None))
                .min_by(|&a, &b| (to_keep(a) - room(a)).total_cmp(&(to_keep(b) - room(b))).then(a[1].total_cmp(&b[1])).then(a[0].total_cmp(&b[0])));
            if well.is_some() {
                return well;
            }
        }
    }
    None
}

/// A castle's bailey: whether it is one of the castles with a garden yard (rolled per castle), and
/// how it is dressed (world ft: place, turn, kind, look, scale), a pure function of its outer
/// bailey (`pl`) and its castle in `l`, so every chunk sets out the same. Nothing stands in the
/// way from a gate to the next gate or to the keep, on a motte, or close by a wall, a tower or
/// another building:
/// - by most ranges, a store's group against its yard side (barrels, crates, a cart, firewood, a
///   trough);
/// - inside each gate, to one side of the way, a cart, a trough or barrels;
/// (The ways from gate to gate and to the keep go in `ways`: the guarantee keeps off them too.)
/// - a well in the keep's court;
/// - an anvil and its quench tub before each smithy (`kitchen::anvils`), a trough and hay before
///   each stables (`kitchen::stables`);
/// - in a garden yard, a kitchen garden against a range, the keep or the curtain (`kitchen::garden`: rows
///   in a wattle fence, skeps) and a few of the biome's trees planted in the yard, spaced out.
pub(super) fn dress_bailey(pl: &crate::town::Plaza, l: &crate::town::Layout, t0: &T0, seed: u64, out: &mut Vec<([f64; 2], f32, Kind, u8, f32)>, ways: &mut Vec<([f64; 2], [f64; 2])>) -> bool {
    use crate::town::geom::{add, centroid, contains, cross, dist, dot, lerp, mul, seg_dist, sub};
    let ring = &pl.pts;
    let rn = ring.len();
    if rn < 3 {
        return false;
    }
    let c0 = centroid(ring);
    let key = hash3(seed ^ 0xba11, crate::core::round(c0[0]) as i64, crate::core::round(c0[1]) as i64, 7);
    let roll = |k: i64| unit(hash2(key, k, 0x5d));
    // (Topiary is a palace's, `palace::gardens`: a castle's garden yard is a kitchen garden.)
    let garden = roll(0) < 0.8;
    let depth = |p: [f64; 2]| {
        let d = (0..rn).map(|k| seg_dist(p, ring[k], ring[(k + 1) % rn])).fold(f64::MAX, f64::min);
        if contains(ring, p) { d } else { -d }
    };
    let poly_d = |poly: &[[f64; 2]], p: [f64; 2]| {
        if contains(poly, p) {
            return 0.0;
        }
        let m = poly.len();
        (0..m).map(|k| seg_dist(p, poly[k], poly[(k + 1) % m])).fold(f64::MAX, f64::min)
    };
    // The castle's buildings, walls and towers round the bailey.
    let reach = ring.iter().map(|p| dist(*p, c0)).fold(0.0, f64::max) + 60.0;
    let near = |p: &[f64; 2]| dist(*p, c0) < reach;
    let blds: Vec<&crate::town::Building> = l.buildings.iter().filter(|b| b.poly.iter().any(near)).collect();
    let walls: Vec<([f64; 2], [f64; 2])> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).filter(|s| near(&s.0) || near(&s.1)).collect();
    let towers: Vec<([f64; 2], f64)> = l
        .towers
        .iter()
        .map(|t| (*t, if l.square_tower(*t).is_some() { 15.6 } else { 11.0 }))
        .chain(l.gate_towers.iter().map(|t| (*t, 15.0)))
        .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { 11.0 })))
        .filter(|t| near(&t.0))
        .collect();
    let keep = blds.iter().find(|b| !b.parts.is_empty() && crate::town::belfry::of(b).is_none() && contains(ring, centroid(&b.poly)));
    let ranges: Vec<&&crate::town::Building> =
        blds.iter().filter(|b| b.parts.is_empty() && b.ward == crate::town::catalog::Ward::Castle && contains(ring, centroid(&b.poly))).collect();
    let mottes: Vec<&crate::town::Motte> = l.mottes.iter().filter(|m| contains(ring, m.at)).collect();
    // The gates: pairs of gate towers a passage apart (the outer one on the ring, an inner one
    // inside it), their middles and the way in (toward the keep). (A castle set in the town by hand
    // has its towers among `extra_towers`.)
    let gt: Vec<[f64; 2]> = l.gate_towers.iter().copied().chain(l.extra_towers.iter().filter(|t| t.1).map(|t| t.0)).filter(|t| depth(*t) > -20.0).collect();
    let to = keep.map_or(c0, |k| centroid(&k.poly));
    let mut gates: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for a in 0..gt.len() {
        for b in a + 1..gt.len() {
            if (dist(gt[a], gt[b]) - 52.0).abs() < 0.5 {
                let m = lerp(gt[a], gt[b], 0.5);
                let u = mul(sub(gt[b], gt[a]), 1.0 / 52.0);
                let n = [-u[1], u[0]];
                gates.push((m, if dot(sub(to, m), n) >= 0.0 { n } else { mul(n, -1.0) }));
            }
        }
    }
    gates.sort_by(|a, b| depth(a.0).total_cmp(&depth(b.0)).then(a.0[0].total_cmp(&b.0[0])).then(a.0[1].total_cmp(&b.0[1])));
    // The ways: gate to gate, the last to the keep.
    let mut lanes: Vec<([f64; 2], [f64; 2])> = gates.windows(2).map(|g| (g[0].0, g[1].0)).collect();
    if let (Some(g), Some(k)) = (gates.last(), keep) {
        lanes.push((g.0, centroid(&k.poly)));
    }
    ways.extend(lanes.iter().copied());
    let inner_walls: Vec<([f64; 2], [f64; 2])> = walls.iter().copied().filter(|s| depth(s.0) > 20.0 && depth(s.1) > 20.0).collect();
    // The keep's court: inside the inner curtain if there is one (the way to its middle crosses
    // none of it).
    let ci = l.castles.get(1).map_or(c0, |c| c.0);
    let in_court = |p: [f64; 2]| {
        inner_walls.is_empty()
            || !inner_walls.iter().any(|&(a, b)| {
                let (o1, o2) = (cross(sub(ci, p), sub(a, p)), cross(sub(ci, p), sub(b, p)));
                let (o3, o4) = (cross(sub(b, a), sub(p, a)), cross(sub(b, a), sub(ci, a)));
                o1 * o2 < 0.0 && o3 * o4 < 0.0
            })
    };
    let mut placed: Vec<([f64; 2], f64)> = Vec::new();
    // Room for something `r` ft round at `p`: in the bailey, clear of buildings (but `own`, by a
    // little), walls, towers, the ways and gates, mottes and what is set out already.
    let free = |placed: &[([f64; 2], f64)], p: [f64; 2], r: f64, own: Option<usize>| {
        depth(p) >= r + 8.0
            && blds.iter().enumerate().all(|(i, b)| poly_d(&b.poly, p) >= r + if own == Some(i) { 1.5 } else { 6.0 })
            && walls.iter().all(|&(a, b)| seg_dist(p, a, b) >= r + 10.0)
            && towers.iter().all(|&(t, tr)| dist(p, t) >= tr + r + 8.0)
            && lanes.iter().all(|&(a, b)| seg_dist(p, a, b) >= r + 14.0)
            && gates.iter().all(|&(g, _)| dist(p, g) >= r + 20.0)
            && mottes.iter().all(|m| dist(p, m.at) >= m.foot_r + r + 6.0)
            && placed.iter().all(|&(q, qr)| dist(p, q) >= r + qr + 2.0)
    };
    let along = |d: [f64; 2]| libm::atan2(d[1], d[0]) as f32;
    let look = |a: i64, b: i64| (hash2(key, a, b) >> 20) as u8 & 7;
    // v63 castles: an anvil before each smithy.
    kitchen::anvils(key, &ranges, &blds, &depth, &free, &mut placed, out);
    kitchen::stables(key, &ranges, &blds, &depth, &free, &mut placed, out);
    // By the ranges, a store's group against the yard side.
    for (ri, b) in ranges.iter().enumerate() {
        if roll(10 + ri as i64) >= 0.7 || b.poly.len() != 4 {
            continue;
        }
        let bi = blds.iter().position(|o| std::ptr::eq(*o, **b));
        // (Its long side deepest in the bailey.)
        let side = (0..4).filter(|&e| dist(b.poly[e], b.poly[(e + 1) % 4]) >= dist(b.poly[(e + 1) % 4], b.poly[(e + 2) % 4]) - 0.5).max_by(|&x, &y| {
            depth(lerp(b.poly[x], b.poly[(x + 1) % 4], 0.5)).total_cmp(&depth(lerp(b.poly[y], b.poly[(y + 1) % 4], 0.5))).then(y.cmp(&x))
        });
        let Some(e) = side else { continue };
        let (a, q) = (b.poly[e], b.poly[(e + 1) % 4]);
        let el = dist(a, q);
        let u = mul(sub(q, a), 1.0 / el.max(1e-9));
        let n0 = [-u[1], u[0]];
        let n = if dot(sub(lerp(a, q, 0.5), centroid(&b.poly)), n0) >= 0.0 { n0 } else { mul(n0, -1.0) };
        let group: &[Kind] = match (roll(30 + ri as i64) * 5.0) as usize {
            0 => &[Kind::Barrel, Kind::Crate],
            1 => &[Kind::Cart],
            2 => &[Kind::Firewood, Kind::Firewood],
            3 => &[Kind::Crate, Kind::Barrel, Kind::Crate],
            _ => &[Kind::WaterTrough],
        };
        let t = 0.2 + 0.6 * roll(50 + ri as i64);
        for (gi, &kind) in group.iter().enumerate() {
            let rad = info(kind as u16).radius as f64 * SQUARE_FT;
            let p = add(add(a, mul(u, (el * t + gi as f64 * 6.0).min(el - rad))), mul(n, rad + 2.0));
            if free(&placed, p, rad, bi) {
                placed.push((p, rad));
                out.push((p, along(u), kind, look(70 + ri as i64, gi as i64), 1.0));
            }
        }
    }
    // Inside each gate, to one side of the way in.
    for (gi, &(g, n)) in gates.iter().enumerate() {
        let s = if roll(90 + gi as i64) < 0.5 { [-n[1], n[0]] } else { [n[1], -n[0]] };
        let kind = [Kind::Cart, Kind::WaterTrough, Kind::Barrel][(roll(95 + gi as i64) * 3.0) as usize % 3];
        let rad = info(kind as u16).radius as f64 * SQUARE_FT;
        for (fwd, aside) in [(18.0, 26.0), (26.0, 30.0), (14.0, 34.0)] {
            let p = add(add(g, mul(n, fwd)), mul(s, aside));
            if free(&placed, p, rad, None) {
                placed.push((p, rad));
                out.push((p, along(n), kind, look(99, gi as i64), 1.0));
                break;
            }
        }
    }
    // Spots on a 15-ft grid in the bailey, whole multiples of world ft (the same in every chunk).
    let (x0, y0, x1, y1) = ring.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    const GRID: f64 = 15.0;
    let mut spots: Vec<[f64; 2]> = Vec::new();
    let mut y = crate::core::ceil(y0 / GRID) * GRID;
    while y <= y1 {
        let mut x = crate::core::ceil(x0 / GRID) * GRID;
        while x <= x1 {
            if depth([x, y]) > 20.0 {
                spots.push([x, y]);
            }
            x += GRID;
        }
        y += GRID;
    }
    let to_keep = |p: [f64; 2]| keep.map_or(dist(p, c0), |k| poly_d(&k.poly, p));
    let room = |p: [f64; 2]| {
        let b = blds.iter().map(|b| poly_d(&b.poly, p)).fold(f64::MAX, f64::min);
        let w = walls.iter().map(|&(a, q)| seg_dist(p, a, q)).fold(f64::MAX, f64::min);
        b.min(w).min(80.0)
    };
    // A well in the keep's court: by the keep, with room round it.
    let well_r = info(Kind::Well as u16).radius as f64 * SQUARE_FT + 6.0;
    let well = spots
        .iter()
        .copied()
        .filter(|&p| in_court(p) && room(p) >= 25.0 && free(&placed, p, well_r, None))
        .min_by(|&a, &b| (to_keep(a) - room(a)).total_cmp(&(to_keep(b) - room(b))).then(a[1].total_cmp(&b[1])).then(a[0].total_cmp(&b[0])));
    let well = well.or_else(|| bailey_well(&spots, &placed, well_r, &in_court, &room, &to_keep, &free));
    // Last resort in a cramped bailey: a finer grid, bare well footprint, least clearances.
    let well = well.or_else(|| {
        let r = well_r - 6.0;
        let mut best: Option<([f64; 2], f64)> = None;
        let mut y = crate::core::ceil(y0 / 5.0) * 5.0;
        while y <= y1 {
            let mut x = crate::core::ceil(x0 / 5.0) * 5.0;
            while x <= x1 {
                let p = [x, y];
                let ok = depth(p) >= r + 3.0
                    && blds.iter().all(|b| poly_d(&b.poly, p) >= r + 1.0)
                    && walls.iter().all(|&(a, b)| seg_dist(p, a, b) >= r + 3.0)
                    && towers.iter().all(|&(t, tr)| dist(p, t) >= tr + r + 2.0)
                    && lanes.iter().all(|&(a, b)| seg_dist(p, a, b) >= r + 4.0)
                    && gates.iter().all(|&(g, _)| dist(p, g) >= r + 8.0)
                    && mottes.iter().all(|m| dist(p, m.at) >= m.foot_r + r + 2.0)
                    && placed.iter().all(|&(q, qr)| dist(p, q) >= r + qr);
                let score = room(p) - 0.1 * to_keep(p);
                if ok && best.map_or(true, |(_, s)| score > s) {
                    best = Some((p, score));
                }
                x += 5.0;
            }
            y += 5.0;
        }
        best.map(|b| b.0)
    });
    if let Some(p) = well {
        placed.push((p, well_r));
        out.push((p, 0.0, Kind::Well, (key >> 24) as u8 & 7, 1.0));
    }
    if !garden {
        return false;
    }
    // v63 castles: the kitchen garden.
    kitchen::garden(key, &ranges, &blds, keep.copied(), to, ring, &spots, &to_keep, &depth, &free, &mut placed, out);
    // A few planted trees, spaced out over the yard (the biome's own).
    let tree = greenery(land_biome(t0, c0)).0.first().copied().unwrap_or(Kind::TreeDeciduous);
    let want = 3 + (roll(300) * 4.0) as usize;
    let mut cand: Vec<[f64; 2]> = spots.iter().copied().filter(|&p| room(p) >= 14.0).collect();
    cand.sort_by_key(|p| (hash2(key, crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64), crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64));
    let mut trees: Vec<[f64; 2]> = Vec::new();
    for p in cand {
        if trees.len() >= want {
            break;
        }
        if free(&placed, p, 8.0, None) && trees.iter().all(|q| dist(*q, p) >= 40.0) {
            placed.push((p, 8.0));
            out.push((p, 0.0, tree, look(310, trees.len() as i64), 1.0));
            trees.push(p);
        }
    }
    true
}

/// The other open spaces dressed by what they are for (`town::squares`), each a pure function of
/// its plaza (world ft) and the layout round it, so every chunk sets out the same:
/// - a widened crossroads: a signpost as near its middle as the lanes leave room (a city's, now
///   and then, a market cross in the middle for the traffic to go round), a lamp or two, a trough,
///   a notice board, a stack of barrels or crates;
/// - a well square: the well on a stepped plinth in the middle, troughs by it, lamps, a notice
///   board, sometimes stocks;
/// - a fountain square: a fountain in the middle (raised a tier, as the main square's; a well on
///   its plinth where the square is too small), lamp posts spread round it, one look for all;
/// - a green: a well, a cross or a great tree in the middle, the biome's trees round its edge clear
///   of the lanes, its shrubs and plants, sometimes stocks;
/// - a temple's close: a standing cross before the temple, a hedge round its churchyard behind it
///   and headstones in rows square to it, the biome's trees round the close.
pub(super) fn dress_square(pl: &crate::town::Plaza, l: &crate::town::Layout, t0: &T0, lanes: &[([f64; 2], [f64; 2], f64)], seed: u64, features: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>, out: &mut Vec<([f64; 2], f32, Kind, u8)>) {
    use crate::town::PlazaKind as K;
    use crate::town::geom::{add, area, centroid, contains, dist, len, lerp, mul, sub};
    let poly = &pl.pts;
    let mut room = Room::new(poly, l, lanes, &[], seed ^ 0xd2e5);
    let c = room.c;
    let mut rng = Pcg32::new(hash2(seed ^ 0xd2e5, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 19);
    let city = l.tier >= crate::t0::settle::Tier::City;
    let reach = poly.iter().map(|q| dist(*q, c)).fold(0.0, f64::max);
    let outward = |p: [f64; 2]| libm::atan2(p[1] - c[1], p[0] - c[0]) as f32;
    let (trees, shrubs, plants, few) = greenery(land_biome(t0, c));
    let pick = |rng: &mut Pcg32, kinds: &[Kind]| kinds[rng.below(kinds.len() as u32) as usize];
    // Scores: nearest the middle; at a distance from it; just inside the edge.
    let middle = |salt: i64| move |r: &Room, p: [f64; 2]| -dist(p, r.c) + 2.0 * r.jit(p, salt);
    let ring = |at: f64, salt: i64| move |r: &Room, p: [f64; 2]| -(dist(p, r.c) - at).abs() + 3.0 * r.jit(p, salt);
    let edge = |want: f64, salt: i64| move |r: &Room, p: [f64; 2]| -(r.depth(p) - want).abs() + 3.0 * r.jit(p, salt);
    let cover = |room: &mut Room, rng: &mut Pcg32, out: &mut Vec<([f64; 2], f32, Kind, u8)>, salt: i64| {
        let kind = if rng.next_f64() < 0.5 { Kind::Barrel } else { Kind::Crate };
        if let Some(p) = room.best(foot(kind) + 1.0, true, edge(5.0, salt)) {
            out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8));
        }
    };
    // A well on a stepped round plinth in the middle.
    let well = |room: &mut Room, rng: &mut Pcg32, out: &mut Vec<([f64; 2], f32, Kind, u8)>, features: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>| {
        if !room.free(c, 7.0, false) {
            return false;
        }
        features.push((c, 6.0, PlazaFeature::Dais, rng.range(0.0, std::f64::consts::TAU)));
        room.taken.push((c, 9.0));
        out.push((c, 0.0, Kind::Well, rng.below(4) as u8));
        true
    };
    // Trees of the biome spread round a lawn, `inset` ft in from its edge.
    let grove = |room: &mut Room, rng: &mut Pcg32, out: &mut Vec<([f64; 2], f32, Kind, u8)>, n: i64, inset: f64| {
        let mut at: Vec<[f64; 2]> = Vec::new();
        let n = if few { n.min(1 + rng.below(2) as i64) } else { n };
        for k in 0..if trees.is_empty() { 0 } else { n } {
            let kind = pick(rng, trees);
            let spread = |r: &Room, p: [f64; 2]| -0.5 * (r.depth(p) - inset).abs() + 0.2 * at.iter().map(|q| dist(*q, p)).fold(100.0, f64::min) + 2.0 * r.jit(p, 10 + k);
            if let Some(p) = room.best(foot(kind), true, spread) {
                at.push(p);
                out.push((p, 0.0, kind, rng.below(4) as u8));
            }
        }
    };
    let start = out.len();
    match pl.kind {
        K::Crossroads => {
            if city && rng.next_f64() < 0.35 && room.free(c, cross_ft(), false) {
                room.taken.push((c, cross_ft()));
                out.push((c, 0.0, Kind::MarketCross, [0, 1, 3][rng.below(3) as usize]));
            } else if let Some(p) = room.best(foot(Kind::Signpost) + 1.0, true, middle(1)) {
                out.push((p, 0.0, Kind::Signpost, [1, 3][rng.below(2) as usize]));
            }
            let look = lamp_look(&mut rng, false);
            room.lamps(if city { 2 } else { (rng.next_f64() < 0.6) as i64 }, 0.6 * reach, false, look, out);
            // A trough where there's room, a notice board on some (on every one without a trough).
            let trough = room.best(foot(Kind::WaterTrough), true, ring(0.55 * reach, 20));
            if let Some(p) = trough {
                out.push((p, outward(p) + std::f32::consts::FRAC_PI_2, Kind::WaterTrough, rng.below(4) as u8));
            }
            if (trough.is_none() || rng.next_f64() < 0.5)
                && let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(4.0, 30))
            {
                out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
            }
            // At least two things to see: else by the edge, where the streets meet the houses, a
            // signpost (if none yet), a lamp post, a notice board.
            let signed = out[start..].iter().any(|o| matches!(o.2, Kind::Signpost | Kind::MarketCross));
            for (salt, kind) in [(45, Kind::Signpost), (46, Kind::LampPost), (47, Kind::NoticeBoard)] {
                if out.len() - start >= 2 || (kind == Kind::Signpost && signed) {
                    continue;
                }
                if let Some(p) = room.best(foot(kind) + 1.0, false, edge(3.0, salt)) {
                    let v = match kind {
                        Kind::Signpost => [1, 3][rng.below(2) as usize],
                        Kind::LampPost => look,
                        _ => rng.below(4) as u8,
                    };
                    out.push((p, 0.0, kind, v));
                }
            }
            for k in 0..1 + rng.below(2) as i64 {
                cover(&mut room, &mut rng, out, 40 + k);
            }
        }
        K::WellSquare => {
            well(&mut room, &mut rng, out, features);
            let trough = if city { [1, 3][rng.below(2) as usize] } else { [0, 2, 3][rng.below(3) as usize] };
            for k in 0..1 + (rng.next_f64() < 0.5) as i64 {
                if let Some(p) = room.best(foot(Kind::WaterTrough), false, ring(15.0, 10 + k)) {
                    out.push((p, outward(p) + std::f32::consts::FRAC_PI_2, Kind::WaterTrough, trough));
                }
            }
            let look = lamp_look(&mut rng, false);
            room.lamps(if city { 2 } else { (rng.next_f64() < 0.5) as i64 }, 0.65 * reach, false, look, out);
            if rng.next_f64() < 0.4
                && let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(4.0, 30))
            {
                out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
            }
            if rng.next_f64() < if city { 0.15 } else { 0.25 }
                && let Some(p) = room.best(foot(Kind::Stocks), true, edge(6.0, 31))
            {
                out.push((p, 0.0, Kind::Stocks, rng.below(4) as u8));
            }
            cover(&mut room, &mut rng, out, 40);
        }
        K::FountainSquare => {
            let fr = (0.3 * reach).clamp(7.0, 12.0);
            if room.free(c, fr + 2.0, false) {
                features.push((c, fr, PlazaFeature::Fountain, rng.range(0.0, std::f64::consts::TAU)));
                room.taken.push((c, fr + 4.0));
            } else {
                well(&mut room, &mut rng, out, features);
            }
            let look = lamp_look(&mut rng, city);
            room.lamps(3 + city as i64, fr + 0.5 * (reach - fr), false, look, out);
            if rng.next_f64() < 0.3
                && let Some(p) = room.best(foot(Kind::NoticeBoard), true, edge(4.0, 30))
            {
                out.push((p, 0.0, Kind::NoticeBoard, rng.below(4) as u8));
            }
        }
        K::Green => {
            // (A village's green has its well at the heart already, and its own tree by it.)
            let village = l.tier < crate::t0::settle::Tier::Town;
            let mid = if village { 1.0 } else { rng.next_f64() };
            if village || (mid < 0.4 && well(&mut room, &mut rng, out, features)) {
            } else if mid < 0.65 && room.free(c, cross_ft(), false) {
                room.taken.push((c, cross_ft() + 2.0));
                out.push((c, 0.0, Kind::MarketCross, [0, 2][rng.below(2) as usize]));
            } else if !trees.is_empty() && room.free(c, 9.0, true) {
                // A great tree in the middle.
                room.taken.push((c, 9.0));
                out.push((c, 0.0, pick(&mut rng, trees), rng.below(4) as u8));
            }
            if !village {
                grove(&mut room, &mut rng, out, ((area(poly).abs() / 3000.0) as i64).clamp(2, 6), 10.0);
            }
            for k in 0..if shrubs.is_empty() { 0 } else { 1 + rng.below(3) as i64 } {
                let kind = pick(&mut rng, shrubs);
                if let Some(p) = room.best(foot(kind), true, edge(4.0, 20 + k)) {
                    out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8));
                }
            }
            for k in 0..if plants.is_empty() { 0 } else { 2 + rng.below(3) as i64 } {
                let kind = pick(&mut rng, plants);
                if let Some(p) = room.best(foot(kind), true, |r: &Room, p: [f64; 2]| r.jit(p, 30 + k)) {
                    out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8));
                }
            }
            if rng.next_f64() < 0.3
                && let Some(p) = room.best(foot(Kind::Stocks), true, edge(6.0, 40))
            {
                out.push((p, 0.0, Kind::Stocks, rng.below(4) as u8));
            }
        }
        K::TempleClose => {
            // The temple and its churchyard: what stands on the close, roofed and open.
            let on = |s: crate::town::Structure| {
                l.buildings.iter().filter(|b| b.structure == s).map(|b| &b.poly).find(|b| {
                    let (x0, y0, x1, y1) = bounds(b);
                    contains(poly, [0.5 * (x0 + x1), 0.5 * (y0 + y1)])
                })
            };
            let temple = on(crate::town::Structure::Roofed);
            let tc = temple.map_or(c, |t| centroid(t));
            if let Some(t) = temple.filter(|t| t.len() >= 3) {
                // A standing cross before its front: of the hall's sides, the one toward the middle
                // of the town (as the close was laid out).
                let u = sub(t[1], t[0]);
                let u = mul(u, 1.0 / len(u).max(1e-9));
                let to = sub(l.center, tc);
                let front = [u, mul(u, -1.0), [-u[1], u[0]], [u[1], -u[0]]].into_iter().max_by(|a, b| crate::town::geom::dot(*a, to).total_cmp(&crate::town::geom::dot(*b, to))).unwrap_or(u);
                let ext = t.iter().map(|q| crate::town::geom::dot(sub(*q, tc), front)).fold(0.0, f64::max);
                let at = add(tc, mul(front, ext + 2.0 + cross_ft()));
                room.taken.push((at, cross_ft() + 2.0));
                out.push((at, 0.0, Kind::MarketCross, [0, 2][rng.below(2) as usize]));
            }
            if let Some(g) = on(crate::town::Structure::Open).filter(|g| g.len() == 4) {
                // A hedge round the churchyard 3 ft out (a steppe town's stones), with a gap for its gate
                // on the temple's side.
                let gc = centroid(g);
                let mid = |e: usize| lerp(g[e], g[(e + 1) % 4], 0.5);
                let gate = (0..4).min_by(|&a, &b| dist(mid(a), tc).total_cmp(&dist(mid(b), tc))).unwrap_or(0);
                for e in 0..4 {
                    let (a, b) = (g[e], g[(e + 1) % 4]);
                    let steps = (dist(a, b) / 5.0).floor().max(1.0) as usize;
                    for k in 0..=steps {
                        let q = lerp(a, b, k as f64 / steps as f64);
                        if e == gate && dist(q, mid(e)) < 6.0 {
                            continue;
                        }
                        let d = sub(q, gc);
                        let p = add(q, mul(d, 3.0 / len(d).max(1e-9)));
                        // (R5 towns, 114: a steppe town's a ring of stones, no hedge, as its cemetery's.)
                        let edge = if l.culture % 5 == 4 { Kind::RockSmall } else { Kind::Bush };
                        out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, edge, rng.below(4) as u8));
                    }
                }
                // Headstones in rows square to the churchyard, a path kept from its gate across.
                let (e0, e1) = (sub(g[1], g[0]), sub(g[3], g[0]));
                let (l0, l1) = (len(e0), len(e1));
                let (u0, u1) = (mul(e0, 1.0 / l0.max(1e-9)), mul(e1, 1.0 / l1.max(1e-9)));
                let (gm, way) = (mid(gate), sub(gc, mid(gate)));
                let way = mul(way, 1.0 / len(way).max(1e-9));
                let mut a = 4.5;
                while a <= l0 - 4.5 {
                    let mut b = 4.5;
                    while b <= l1 - 4.5 {
                        let p = add(g[0], add(mul(u0, a), mul(u1, b)));
                        let off = sub(p, gm);
                        let across = (off[0] * way[1] - off[1] * way[0]).abs();
                        if across > 3.5 && room.jit(p, 50) < 0.85 {
                            out.push((p, 0.0, Kind::Headstone, rng.below(4) as u8));
                        }
                        b += 8.0;
                    }
                    a += 5.0;
                }
                room.taken.push((gc, g.iter().map(|q| dist(*q, gc)).fold(0.0, f64::max) + 4.0));
            }
            let perimeter: f64 = (0..poly.len()).map(|e| dist(poly[e], poly[(e + 1) % poly.len()])).sum();
            grove(&mut room, &mut rng, out, ((perimeter / 50.0) as i64).clamp(3, 8), 8.0);
        }
        _ => {}
    }
    // Every square has its focus, however little room it leaves: a signpost at a crossroads, a
    // well at a well or fountain square, a tree or a well on a green.
    if out.len() == start && !(pl.kind == K::Green && l.tier < crate::t0::settle::Tier::Town) {
        let kind = match pl.kind {
            K::Crossroads => Kind::Signpost,
            K::Green if !trees.is_empty() => trees[0],
            _ => Kind::Well,
        };
        let p = room.best(foot(kind), false, middle(90)).unwrap_or(c);
        out.push((p, 0.0, kind, rng.below(4) as u8));
    }
}

/// A town plaza's market as a little town of its own: jittered seeds inside a wobbly oval round
/// the well make Voronoi cells, each pulled back from its neighbours so lanes run between them
/// (streets for players and passers-by). Each block's outline is rounded and wobbled, and its
/// stalls follow that curve shoulder to shoulder, facing out onto the lanes, with a gap to get
/// in; crates and barrels stack inside. The cell at the well (and a few others) stays an open
/// square, as does any cell holding a monument; stalls keep clear of the features (`keep`: centre
/// and radius, laid out first on a city's square). A pure function of the plaza (world ft), so
/// every chunk lays out the same market.
pub(super) fn market_blocks(poly: &[[f64; 2]], monuments: &[Vec<[f64; 2]>], keep: &[([f64; 2], f64)], mk: Market, seed: u64, out: &mut Vec<([f64; 2], f32, Kind)>) {
    use crate::town::geom::{add, area, centroid, clip_plain, contains, dist, dot, len, main_axis, mul, sub};
    let c = centroid(poly);
    let mut rng = Pcg32::new(hash2(seed ^ 0x3a2c, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 11);
    // Blocks follow the plaza's long axis, a little askew.
    let eu0 = main_axis(poly);
    let skew = rng.range(-0.15, 0.15);
    let (cs, sn) = (libm::cos(skew), libm::sin(skew));
    let eu = [eu0[0] * cs - eu0[1] * sn, eu0[0] * sn + eu0[1] * cs];
    let ev = [-eu[1], eu[0]];
    let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for q in poly {
        let d = sub(*q, c);
        let (u, v) = (dot(d, eu), dot(d, ev));
        (u0, u1, v0, v1) = (u0.min(u), u1.max(u), v0.min(v), v1.max(v));
    }
    // On a great square the market keeps to a block round the well; the rest stays open.
    let (hu, hv) = (rng.range(mk.hu.0, mk.hu.1), rng.range(mk.hv.0, mk.hv.1));
    (u0, u1, v0, v1) = (u0.max(-hu), u1.min(hu), v0.max(-hv), v1.min(hv));
    if u1 - u0 < 40.0 || v1 - v0 < 30.0 {
        return;
    }
    let at = |u: f64, v: f64| add(c, add(mul(eu, u), mul(ev, v)));
    // Seeds on a jittered grid about 50 ft apart.
    let pitch = 50.0;
    let (nu, nv) = (((u1 - u0) / pitch).ceil() as i64, ((v1 - v0) / pitch).ceil() as i64);
    let mut seeds: Vec<[f64; 2]> = Vec::new();
    for j in 0..nv {
        for i in 0..nu {
            let u = u0 + (i as f64 + 0.5) * (u1 - u0) / nu as f64 + rng.range(-13.0, 13.0);
            let v = v0 + (j as f64 + 0.5) * (v1 - v0) / nv as f64 + rng.range(-13.0, 13.0);
            seeds.push(at(u, v));
        }
    }
    let well = (0..seeds.len()).min_by(|&a, &b| dist(seeds[a], c).total_cmp(&dist(seeds[b], c)));
    // The market's edge: a wobbly oval, not a box.
    let (mu, mv, ru, rv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0, (u1 - u0) / 2.0, (v1 - v0) / 2.0);
    let (ph1, ph2) = (rng.range(0.0, 6.28), rng.range(0.0, 6.28));
    let bounds: Vec<[f64; 2]> = (0..28)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / 28.0;
            let w = 1.0 + 0.07 * libm::sin(3.0 * t + ph1) + 0.04 * libm::sin(5.0 * t + ph2);
            at(mu + ru * w * libm::cos(t) * 1.12, mv + rv * w * libm::sin(t) * 1.12)
        })
        .collect();
    // Monuments on the square (a statue on its plinth) and its features keep a 9-ft walk round
    // them clear; a cell holding a monument stays open.
    let plinths: Vec<([f64; 2], f64)> = monuments.iter().map(|m| (centroid(m), m.iter().map(|q| dist(*q, centroid(m))).fold(0.0, f64::max))).collect();
    let clear = |p: [f64; 2], r: f64| plinths.iter().chain(keep).all(|&(mc, mr)| dist(p, mc) > mr + 9.0 + r);
    let half_lane = 5.0;
    for (i, &s) in seeds.iter().enumerate() {
        // Open squares: the well's cell and a few others (decided first, so the rng runs the
        // same whatever the cell's shape).
        let open = Some(i) == well || rng.next_f64() < mk.open;
        let mut cell = bounds.clone();
        for (j, &o) in seeds.iter().enumerate() {
            if i == j || dist(s, o) > 3.0 * pitch {
                continue;
            }
            let d = sub(o, s);
            let n = mul(d, 1.0 / len(d));
            cell = clip_plain(&cell, n, dot(add(s, mul(d, 0.5)), n) - half_lane);
            if cell.len() < 3 {
                break;
            }
        }
        if open || cell.len() < 3 || area(&cell).abs() < 250.0 || plinths.iter().any(|&(mc, _)| contains(&cell, mc)) {
            continue;
        }
        // Round the corners (Chaikin), then wobble the outline in and out about the middle.
        let mid = centroid(&cell);
        let mut outline = cell.clone();
        for _ in 0..3 {
            let m = outline.len();
            outline = (0..m).flat_map(|e| {
                let (a, b) = (outline[e], outline[(e + 1) % m]);
                [add(mul(a, 0.75), mul(b, 0.25)), add(mul(a, 0.25), mul(b, 0.75))]
            }).collect();
        }
        let (wa, wk, wp) = (rng.range(0.04, 0.1), 2.0 + rng.below(3) as f64, rng.range(0.0, 6.28));
        for q in &mut outline {
            let d = sub(*q, mid);
            let ang = libm::atan2(d[1], d[0]);
            *q = add(mid, mul(d, 1.0 + wa * libm::sin(wk * ang + wp)));
        }
        let ccw = area(&outline) > 0.0;
        let m = outline.len();
        let seg: Vec<f64> = (0..m).map(|e| dist(outline[e], outline[(e + 1) % m])).collect();
        let total: f64 = seg.iter().sum();
        let count = (total / mk.spacing).floor() as usize;
        if count < 3 {
            continue;
        }
        let step = total / count as f64;
        // A way in: two stalls left out somewhere round the ring.
        let gap = if count >= 8 { Some(rng.below(count as u32) as usize) } else { None };
        let (mut e, mut acc) = (0usize, 0.0);
        for k in 0..count {
            let want = (k as f64 + 0.5) * step;
            while e < m - 1 && acc + seg[e] < want {
                acc += seg[e];
                e += 1;
            }
            if gap.is_some_and(|g| k == g || k == (g + 1) % count) || rng.next_f64() < mk.drop {
                continue;
            }
            let (a, b) = (outline[e], outline[(e + 1) % m]);
            let t = mul(sub(b, a), 1.0 / seg[e].max(1e-9));
            let on = add(a, mul(t, want - acc));
            // Outward normal: the stall's front, onto the lane.
            let n = if ccw { [t[1], -t[0]] } else { [-t[1], t[0]] };
            let p = add(on, mul(n, -3.5));
            let fits = [-4.5, 4.5].iter().all(|&o| contains(poly, add(p, mul(t, o)))) && clear(p, 5.0);
            if !fits {
                continue;
            }
            // Sprites face south (+y) unturned.
            let rot = (libm::atan2(n[1], n[0]) - std::f64::consts::FRAC_PI_2 + rng.range(-0.08, 0.08)) as f32;
            let kind = if rng.next_f64() < 0.05 { Kind::Cart } else { Kind::MarketStall };
            out.push((p, rot, kind));
        }
        // Stock in the block's back.
        if area(&cell).abs() > 500.0 && contains(poly, mid) && clear(mid, 4.0) {
            for _ in 0..1 + rng.below(3) {
                let p = add(mid, [rng.range(-4.0, 4.0), rng.range(-4.0, 4.0)]);
                let kind = if rng.next_f64() < 0.5 { Kind::Crate } else { Kind::Barrel };
                out.push((p, rng.range(0.0, std::f64::consts::TAU) as f32, kind));
            }
        }
    }
}

/// Round raised features on a town square, on open paving: fountains, planters with a tree,
/// market crosses and speakers' daises, clear of the well, monuments, the market's stalls, the
/// square's edge, its streets and roads (`lanes`: segment and half width) and each other. A pure
/// function of the plaza (world ft).
///
/// A town scatters them (about one per 7,000 sq ft, up to six). A city lays them out sparingly in
/// a pattern round the well, square to the plaza's long axis (`patterned`): a pair of fountains on
/// the axis, four trees round the well, or fountains on the long axis and crosses (or daises) on
/// the short one. Every symmetric group goes in whole or not at all. Open paving must still span
/// two tiers in every 32-square window (`enforce`), so where some part of the square is more than
/// 70 ft from a raised feature, more trees go in, mirrored across both axes. A civic square
/// (`civic`) has no pattern, its market cross for a landmark: its trees go by its edges.
#[allow(clippy::too_many_arguments)]
pub(super) fn plaza_features(
    poly: &[[f64; 2]],
    monuments: &[Vec<[f64; 2]>],
    stalls: &[([f64; 2], f32, Kind)],
    lanes: &[([f64; 2], [f64; 2], f64)],
    seed: u64,
    patterned: bool,
    civic: bool,
    out: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>,
) {
    use crate::town::geom::{add, area, centroid, contains, dist, dot, main_axis, mul, seg_dist, sub};
    use std::f64::consts::TAU;
    let c = centroid(poly);
    let mut rng = Pcg32::new(hash2(seed ^ 0xf0a7, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 13);
    let first = out.len();
    // (All must hold: most candidates fail at the rim, the lanes cost most.)
    let rim: [[f64; 2]; 12] = std::array::from_fn(|k| {
        let t = TAU * k as f64 / 12.0;
        [libm::cos(t), libm::sin(t)]
    });
    let fits = |out: &[([f64; 2], f64, PlazaFeature, f64)], p: [f64; 2], r: f64| {
        rim.iter().all(|u| contains(poly, add(p, [(r + 6.0) * u[0], (r + 6.0) * u[1]])))
            && dist(p, c) > r + 22.0
            && out[first..].iter().all(|f| dist(p, f.0) > r + f.1 + 14.0)
            && stalls.iter().all(|s| dist(p, s.0) > r + 10.0)
            && monuments.iter().all(|m| {
                let mc = centroid(m);
                dist(p, mc) > r + 8.0 + m.iter().map(|q| dist(*q, mc)).fold(0.0, f64::max)
            })
            && lanes.iter().all(|&(a, b, w)| seg_dist(p, a, b) > r + w + 4.0)
    };
    if !patterned {
        let want = ((area(poly).abs() / 7000.0) as usize).min(6);
        let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        let mut placed = 0;
        for _ in 0..60 {
            if placed >= want {
                break;
            }
            let p = [rng.range(x0, x1), rng.range(y0, y1)];
            let roll = rng.next_f64();
            let (kind, r) = if roll < 0.35 {
                (PlazaFeature::Fountain, rng.range(9.0, 13.0))
            } else if roll < 0.65 {
                (PlazaFeature::Planter, rng.range(6.0, 9.0))
            } else if roll < 0.85 {
                (PlazaFeature::Cross, rng.range(5.0, 7.0))
            } else {
                (PlazaFeature::Dais, rng.range(7.0, 10.0))
            };
            let ang = rng.range(0.0, TAU);
            if fits(&out[..], p, r) {
                out.push((p, r, kind, ang));
                placed += 1;
            }
        }
        return;
    }
    // Patterned: a point `u` along the plaza's long axis from the well and `v` across it.
    let eu = main_axis(poly);
    let ev = [-eu[1], eu[0]];
    let at = |u: f64, v: f64| add(c, add(mul(eu, u), mul(ev, v)));
    // A symmetric group, whole or not at all; each faces the well (a dais's steps come down
    // toward it).
    let whole = |out: &mut Vec<([f64; 2], f64, PlazaFeature, f64)>, pts: &[[f64; 2]], r: f64, kind: PlazaFeature| {
        let start = out.len();
        for &p in pts {
            if !fits(&out[..], p, r) {
                out.truncate(start);
                return false;
            }
            out.push((p, r, kind, libm::atan2(c[1] - p[1], c[0] - p[0])));
        }
        true
    };
    // How far the square reaches along each axis from the well.
    let (mut ru, mut rv) = (0.0f64, 0.0f64);
    for q in poly {
        let d = sub(*q, c);
        (ru, rv) = (ru.max(dot(d, eu).abs()), rv.max(dot(d, ev).abs()));
    }
    // Distances to try, nearest a preferred fraction of the reach first.
    let around = |reach: f64, frac: f64| {
        let mut ds: Vec<f64> = (0..).map(|k| 30.0 + 6.0 * k as f64).take_while(|&d| d < reach).collect();
        ds.sort_by(|a, b| (a - reach * frac).abs().total_cmp(&(b - reach * frac).abs()));
        ds
    };
    let (r_tree, r_fountain, r_cross, r_dais) = (rng.range(6.0, 8.0), rng.range(9.0, 12.0), rng.range(5.0, 6.5), rng.range(7.0, 9.0));
    let minor = if rng.next_f64() < 0.6 { (PlazaFeature::Cross, r_cross) } else { (PlazaFeature::Dais, r_dais) };
    let style = rng.below(3);
    // (A civic square has its cross for a landmark (`main_cross`) and trees by its edges, no more.)
    for attempt in (0..3).filter(|_| !civic) {
        match (style + attempt) % 3 {
            // A fountain either side of the well on the long axis.
            0 => {
                for u in around(ru, 0.4) {
                    if whole(out, &[at(u, 0.0), at(-u, 0.0)], r_fountain, PlazaFeature::Fountain) {
                        break;
                    }
                }
            }
            // Four trees round the well, square to the axes.
            1 => {
                for d in around(ru.min(rv), 0.35) {
                    if whole(out, &[at(d, d), at(-d, d), at(d, -d), at(-d, -d)], r_tree, PlazaFeature::Planter) {
                        break;
                    }
                }
            }
            // Fountains on the long axis, crosses or daises on the short one.
            _ => {
                for u in around(ru, 0.45) {
                    if whole(out, &[at(u, 0.0), at(-u, 0.0)], r_fountain, PlazaFeature::Fountain) {
                        for v in around(rv, 0.45) {
                            if whole(out, &[at(0.0, v), at(0.0, -v)], minor.1, minor.0) {
                                break;
                            }
                        }
                        break;
                    }
                }
            }
        }
        if out.len() > first {
            break;
        }
    }
    // Reach: no point of the square farther than 70 ft (along the map's axes, as the battlemap's
    // windows run) from a raised feature. The worst-served spot gets a mirrored group of trees
    // nearby; a spot nothing fits near is left to `enforce`.
    let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let mut samples: Vec<[f64; 2]> = Vec::new();
    let mut y = y0 + 5.0;
    while y < y1 {
        let mut x = x0 + 5.0;
        while x < x1 {
            if contains(poly, [x, y]) {
                samples.push([x, y]);
            }
            x += 10.0;
        }
        y += 10.0;
    }
    let reach = |out: &[([f64; 2], f64, PlazaFeature, f64)], p: [f64; 2]| out[first..].iter().map(|f| (f.0[0] - p[0]).abs().max((f.0[1] - p[1]).abs())).fold(f64::MAX, f64::min);
    let mut given_up: Vec<[f64; 2]> = Vec::new();
    let m = poly.len();
    let by_edge = |p: [f64; 2]| (0..m).map(|e| seg_dist(p, poly[e], poly[(e + 1) % m])).fold(f64::MAX, f64::min) <= r_tree + 14.0;
    for _ in 0..16 {
        let worst = samples
            .iter()
            .filter(|&&s| given_up.iter().all(|g| dist(*g, s) > 40.0))
            .map(|&s| (reach(&out[..], s), s))
            .filter(|&(d, _)| d > 70.0)
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, s)) = worst else { break };
        let mut spots: Vec<[f64; 2]> = (0..81).map(|q| [s[0] + ((q % 9) as f64 - 4.0) * 8.0, s[1] + ((q / 9) as f64 - 4.0) * 8.0]).filter(|&p| !civic || by_edge(p)).collect();
        spots.sort_by(|a, b| dist(*a, s).total_cmp(&dist(*b, s)));
        let placed = spots.into_iter().any(|p| {
            let d = sub(p, c);
            let (u, v) = (dot(d, eu), dot(d, ev));
            let near = r_tree + 7.0;
            let pts: Vec<[f64; 2]> = match (u.abs() < near, v.abs() < near) {
                (true, true) => return false,
                (true, false) => vec![at(0.0, v), at(0.0, -v)],
                (false, true) => vec![at(u, 0.0), at(-u, 0.0)],
                _ => vec![at(u, v), at(-u, v), at(u, -v), at(-u, -v)],
            };
            whole(out, &pts, r_tree, PlazaFeature::Planter)
        });
        if !placed {
            given_up.push(s);
        }
    }
}
