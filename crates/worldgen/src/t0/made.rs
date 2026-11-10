//! Settlements added by hand without remaking the world (`world::Created` of kind `village` or
//! `town`). Each is laid out by the world's own village and town generators from its entry
//! alone (its point, its id's seed, its culture and what it lives by) and joined to the nearest
//! road by a local track. Nothing the world generated reads them: T0's bytes, terrain tiles,
//! every network road and every other layout stay as they were. The battlemap lays the
//! settlement's pad and its track over the ground (`Ground`).

use super::T0;
use super::biome::Biome;
use super::roads::RoadClass;
use super::settle::{Reason, SettleKind, Settlement, Tier};
use crate::World;
use crate::core::noise::{fbm, smoothstep};
use crate::core::rng::{hash3, unit};
use crate::world::Created;

type P = [f64; 2];

fn dist(a: P, b: P) -> f64 {
    crate::core::sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]))
}

/// The farthest a created settlement may lie from the nearest road for a track to join it
/// (ft, straight line from its centre): 10 mi. Farther, it stands without one.
pub const TRACK_CAP_FT: f64 = 10.0 * 5280.0;
/// The track's routing lattice (ft; nodes at global index × step).
const STEP_FT: f64 = 250.0;
/// How far round the straight line the track may wander (ft).
const MARGIN_FT: f64 = 1.5 * 5280.0;
/// Where the track's ground eases back to the land beside it (ft past its edge).
const TRACK_EASE_FT: f64 = 15.0;

/// A settlement added by hand: its point of interest (layout `settlements.len() + poi`), the
/// settlement it is, and its track from the settlement's edge (`town::road_trim_radius`) to the
/// road, with the track's surface (ft) at each point; empty with none.
#[derive(Clone, Debug)]
pub struct Made {
    pub poi: usize,
    pub s: Settlement,
    pub track: Vec<P>,
    pub z: Vec<f32>,
}

/// The tier of a created settlement's kind.
pub fn tier_of(kind: &str) -> Tier {
    if kind == "town" { Tier::Town } else { Tier::Village }
}

/// The T0 cell (index) nearest a point.
fn cell_of(t0: &T0, x: f64, y: f64) -> usize {
    let (w, h) = (t0.height.w, t0.height.h);
    let i = ((x / t0.cell_ft).round().max(0.0) as usize).min(w - 1);
    let j = ((y / t0.cell_ft).round().max(0.0) as usize).min(h - 1);
    j * w + i
}

/// The naming culture at a point: its nearest settlement's (as the world's own sites take it).
pub fn culture_at(t0: &T0, x: f64, y: f64) -> u8 {
    let d2 = |s: &Settlement| (s.x - x) * (s.x - x) + (s.y - y) * (s.y - y);
    t0.settlements.iter().min_by(|a, b| d2(a).total_cmp(&d2(b))).map_or(0, |s| s.culture)
}

/// What a settlement of `tier` at a point lives by, from its ground as the world's own are
/// given theirs (no reason, never by the shore: farming where it would fish a river).
pub fn kind_at(world: &World, t0: &T0, x: f64, y: f64, tier: Tier) -> SettleKind {
    let k = cell_of(t0, x, y);
    let w = t0.height.w;
    let (i, j) = (k % w, k / w);
    let b = Biome::from_u8((t0.biome.get(i, j) & 0xff) as u8);
    let above = t0.height.get(i, j) as f64 - world.params().sea_level_ft;
    let ore = fbm(world.stream("t0.settle.ore"), i as f64 / 12.0, j as f64 / 12.0, 3, 2.0, 0.5);
    let c = t0.cell_ft;
    let on_river = !t0.rivers.segments_near(x - 0.5 * c, y - 0.5 * c, x + 0.5 * c, y + 0.5 * c, 0.0).is_empty();
    let near_river = on_river || !t0.rivers.segments_near(x - 1.5 * c, y - 1.5 * c, x + 1.5 * c, y + 1.5 * c, 0.0).is_empty();
    match super::settle::kind_of(Reason::None, tier, b, above, ore, false, near_river, on_river, true) {
        SettleKind::Fishing | SettleKind::Port => SettleKind::Farming,
        k => k,
    }
}

/// The settlement a created village or town is: drawn from `seed` (its id's), standing at its
/// point, its culture and what it lives by as stored on the entry (else from where it stands).
pub fn settlement(world: &World, t0: &T0, c: &Created, seed: u64) -> Settlement {
    let tier = tier_of(&c.kind);
    let (lo, hi) = super::settle::pop_range(tier);
    let population = (lo * libm::pow(hi / lo, unit(hash3(seed, 0x0070_6f70, 0, 0)))) as u32;
    let kind = c.lives_by.as_deref().and_then(SettleKind::parse).filter(|k| !matches!(k, SettleKind::Port | SettleKind::Fishing)).unwrap_or_else(|| kind_at(world, t0, c.x, c.y, tier));
    let culture = c.culture.unwrap_or_else(|| culture_at(t0, c.x, c.y));
    let r = crate::town::urban_radius(tier, population);
    let river = !t0.rivers.segments_near(c.x - r, c.y - r, c.x + r, c.y + r, 0.0).is_empty();
    Settlement {
        tier,
        kind,
        reason: Reason::None,
        cell: cell_of(t0, c.x, c.y),
        x: c.x,
        y: c.y,
        population,
        coastal: false,
        river,
        capital: false,
        seed,
        culture,
        pin: None,
        planned: tier,
        planned_pop: population,
        isolated: false,
    }
}

/// The nearest point of a network road within `TRACK_CAP_FT` of `c`: (point, the road's surface there).
fn nearest_road(t0: &T0, c: P) -> Option<(P, f64)> {
    let cap = TRACK_CAP_FT;
    let mut best: Option<(f64, P, f64)> = None;
    for (ri, k) in t0.roads.segments_near([c[0] - cap, c[1] - cap, c[0] + cap, c[1] + cap], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        for j in 0..=16 {
            let q = rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft);
            let d = dist(q.p, c);
            if d <= cap && best.is_none_or(|b| d < b.0) {
                best = Some((d, q.p, q.z));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

/// The track from settlement `s` to the nearest road: routed over the ground between them (on a
/// lattice of `STEP_FT`, round water, as gently as it can; never steeper than a track's grade
/// where it can help it), corners rounded, trimmed to the settlement's edge (its layout draws the
/// rest) and graded (`roads::fit_profile`) to meet the road at its level. None beyond
/// `TRACK_CAP_FT`, or where a road already reaches the settlement's edge.
pub fn track(world: &World, t0: &T0, s: &Settlement) -> (Vec<P>, Vec<f32>) {
    let c = [s.x, s.y];
    let trim = crate::town::road_trim_radius(s.tier, s.population);
    let none = (Vec::new(), Vec::new());
    let Some((end, end_z)) = nearest_road(t0, c) else { return none };
    if dist(end, c) <= trim + STEP_FT {
        return none;
    }
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
    let gmax = RoadClass::Track.max_grade();
    // The lattice round both ends (global indices, so a node's place is its index × step).
    let i0 = crate::core::floor((c[0].min(end[0]) - MARGIN_FT) / STEP_FT) as i64;
    let j0 = crate::core::floor((c[1].min(end[1]) - MARGIN_FT) / STEP_FT) as i64;
    let i1 = crate::core::ceil((c[0].max(end[0]) + MARGIN_FT) / STEP_FT) as i64;
    let j1 = crate::core::ceil((c[1].max(end[1]) + MARGIN_FT) / STEP_FT) as i64;
    let (w, h) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
    let at = |n: usize| [(i0 + (n % w) as i64) as f64 * STEP_FT, (j0 + (n / w) as i64) as f64 * STEP_FT];
    let ground: Vec<f64> = (0..w * h).map(&at).map(|p| t0.ground_at(p[0], p[1], lattice)).collect();
    let wet: Vec<bool> = (0..w * h).map(|n| (at(n), ground[n])).map(|(p, g)| p[0] < 0.0 || p[1] < 0.0 || p[0] > world.geom.map_w_ft || p[1] > world.geom.map_h_ft || t0.sample_water(p[0], p[1]) as f64 > g).collect();
    let node = |p: P| {
        let i = (crate::core::round(p[0] / STEP_FT) as i64 - i0).clamp(0, w as i64 - 1) as usize;
        let j = (crate::core::round(p[1] / STEP_FT) as i64 - j0).clamp(0, h as i64 - 1) as usize;
        j * w + i
    };
    let (start, goal) = (node(c), node(end));
    // Dijkstra over 16 directions: length, more on a climb, much more past a track's grade.
    const DIRS: [(i64, i64); 16] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)];
    let mut cost = vec![f64::INFINITY; w * h];
    let mut from = vec![u32::MAX; w * h];
    let mut heap = std::collections::BinaryHeap::new();
    cost[start] = 0.0;
    heap.push(std::cmp::Reverse((0u64, start as u32)));
    while let Some(std::cmp::Reverse((bits, n))) = heap.pop() {
        let n = n as usize;
        if f64::from_bits(bits) > cost[n] {
            continue;
        }
        if n == goal {
            break;
        }
        let (ni, nj) = ((n % w) as i64, (n / w) as i64);
        for (di, dj) in DIRS {
            let (mi, mj) = (ni + di, nj + dj);
            if mi < 0 || mj < 0 || mi >= w as i64 || mj >= h as i64 {
                continue;
            }
            let m = mj as usize * w + mi as usize;
            if wet[m] && m != goal {
                continue;
            }
            let len = STEP_FT * crate::core::sqrt((di * di + dj * dj) as f64);
            let g = (ground[m] - ground[n]).abs() / len / gmax;
            let k = if g <= 1.0 { 1.0 + 3.0 * g * g } else { 4.0 + 40.0 * (g - 1.0) };
            let v = cost[n] + len * k;
            if v < cost[m] {
                cost[m] = v;
                from[m] = n as u32;
                heap.push(std::cmp::Reverse((v.to_bits(), m as u32)));
            }
        }
    }
    if from[goal] == u32::MAX && goal != start {
        return none;
    }
    let mut path = vec![end];
    let mut n = from[goal];
    while n != u32::MAX && n as usize != start {
        path.push(at(n as usize));
        n = from[n as usize];
    }
    path.push(c);
    path.reverse();
    // Straight runs as one leg, corners rounded.
    let mut legs: Vec<P> = vec![path[0]];
    for k in 1..path.len() - 1 {
        let (a, b, d) = (*legs.last().unwrap(), path[k], path[k + 1]);
        let cr = (b[0] - a[0]) * (d[1] - b[1]) - (b[1] - a[1]) * (d[0] - b[0]);
        if cr.abs() > 1e-6 {
            legs.push(b);
        }
    }
    legs.push(*path.last().unwrap());
    let (mut pts, _) = super::roads::round_corners(&legs, &vec![0.0; legs.len()], 300.0, 40.0);
    // From the settlement's edge on.
    let Some(k) = pts.iter().position(|p| dist(*p, c) >= trim) else { return none };
    if k > 0 {
        let (a, b) = (pts[k - 1], pts[k]);
        let (da, db) = (dist(a, c), dist(b, c));
        let t = ((trim - da) / (db - da).max(1e-9)).clamp(0.0, 1.0);
        pts[k - 1] = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        pts.drain(..k - 1);
    }
    if pts.len() < 2 {
        return none;
    }
    let mut terrain: Vec<f64> = pts.iter().map(|p| t0.ground_at(p[0], p[1], lattice)).collect();
    *terrain.last_mut().unwrap() = end_z;
    let mut z = super::roads::fit_profile(&pts, &terrain, gmax);
    // Meet the road at its level, easing in over the last 200 ft.
    let off = end_z as f32 - *z.last().unwrap();
    let mut run = 0.0;
    for k in (0..pts.len()).rev() {
        if k + 1 < pts.len() {
            run += dist(pts[k + 1], pts[k]);
        }
        if run >= 200.0 {
            break;
        }
        z[k] += off * (1.0 - run / 200.0) as f32;
    }
    (pts, z)
}

/// The ground created settlements lay over a battlemap: each one's pad (the ground eased to the
/// smooth surface its layout was planned on, as `town::pad_weight` reaches) and its track (the
/// ground cut and filled to the track's surface, easing back over `TRACK_EASE_FT`).
pub struct Ground<'a> {
    t0: &'a T0,
    lattice: f64,
    pads: Vec<(P, f64)>,
    tracks: Vec<(&'a [P], &'a [f32])>,
}

impl<'a> Ground<'a> {
    /// Over `rect` (x0, y0, x1, y1 world ft); None where no created settlement reaches it.
    pub fn new(world: &World, t0: &'a T0, rect: [f64; 4]) -> Option<Ground<'a>> {
        let hit = |b: [f64; 4]| b[2] >= rect[0] && b[0] <= rect[2] && b[3] >= rect[1] && b[1] <= rect[3];
        let mut pads = Vec::new();
        let mut tracks = Vec::new();
        for m in &t0.made {
            let r = crate::town::reach(&m.s);
            let e = crate::town::pad_extent(r);
            if hit([m.s.x - e, m.s.y - e, m.s.x + e, m.s.y + e]) {
                pads.push(([m.s.x, m.s.y], r));
            }
            let pad = 0.5 * RoadClass::Track.width_ft() + TRACK_EASE_FT;
            if m.track.windows(2).any(|s| hit([s[0][0].min(s[1][0]) - pad, s[0][1].min(s[1][1]) - pad, s[0][0].max(s[1][0]) + pad, s[0][1].max(s[1][1]) + pad])) {
                tracks.push((m.track.as_slice(), m.z.as_slice()));
            }
        }
        if pads.is_empty() && tracks.is_empty() {
            return None;
        }
        Some(Ground { t0, lattice: world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1)), pads, tracks })
    }

    /// The nearest track point: (distance, the track's surface there).
    fn track_near(&self, p: P) -> Option<(f64, f64)> {
        let mut best: Option<(f64, f64)> = None;
        let reach = 0.5 * RoadClass::Track.width_ft() + TRACK_EASE_FT;
        for (pts, z) in &self.tracks {
            for k in 0..pts.len() - 1 {
                let (a, b) = (pts[k], pts[k + 1]);
                if p[0] < a[0].min(b[0]) - reach || p[0] > a[0].max(b[0]) + reach || p[1] < a[1].min(b[1]) - reach || p[1] > a[1].max(b[1]) + reach {
                    continue;
                }
                let ab = [b[0] - a[0], b[1] - a[1]];
                let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
                let d = dist(p, [a[0] + ab[0] * t, a[1] + ab[1] * t]);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, z[k] as f64 + (z[k + 1] - z[k]) as f64 * t));
                }
            }
        }
        best
    }

    /// The ground at `(x, y)` over `base` (the terrain's there).
    pub fn at(&self, x: f64, y: f64, base: f32) -> f32 {
        let mut h = base as f64;
        let w = self.pads.iter().fold(0.0f64, |m, &(c, r)| m.max(crate::town::pad_weight(dist([x, y], c), r)));
        if w > 0.0 {
            h += (self.t0.ground_at(x, y, self.lattice) - h) * w;
        }
        if let Some((d, z)) = self.track_near([x, y]) {
            let half = 0.5 * RoadClass::Track.width_ft();
            let k = 1.0 - smoothstep(half + 2.5, half + TRACK_EASE_FT, d);
            h += (z - h) * k;
        }
        h as f32
    }

    /// Whether `(x, y)` is on a track.
    pub fn on_track(&self, x: f64, y: f64) -> bool {
        self.track_near([x, y]).is_some_and(|(d, _)| d < 0.5 * RoadClass::Track.width_ft())
    }
}
