//! The world's islets by its cities (R5 M20): rock in a wide river or just off a sea or lake
//! shore, made from T0 alone (the cities, their reach, the river curves, the drawn shore and the
//! network roads), once for the whole world (`islets`), so request order and the edits never
//! change them. They are shaped as hand islets are (`ground::Shaper`), known to `town::Shore`
//! and read by their city's plan (piers keep off them, a bridge's deck runs flat over them, one
//! may carry a watchtower: `town::islet_towers`).
//!
//! - River: half of the cities and metropolises with a river at least `RIVER_MIN_W_FT` wide in
//!   their reach get one (a metropolis up to two): on the river's middle line, its radius 15-45
//!   ft and at most 0.3 of the width, at least 150 ft from a network road's crossing.
//! - Coast: half of the cities on a sea or lake get 1-3, 120-300 ft out from the shore in water
//!   2-25 ft deep (`COAST_DEPTH_FT`).
//!
//! Each stands 3-8 ft above its water, inside its city's reach (less its radius and 40 ft).
use crate::core::rng::{hash3, unit};
use crate::t0::T0;
use crate::t0::settle::Tier;
use crate::town::geom::{add, dist, mul, seg_dist};

type P = [f64; 2];

/// One islet: its rock ring (world ft), its top's level and the water's under it (ft), middle,
/// radius (ft), whether a watchtower stands on it, its settlement (layout index), whether it is
/// in a river, and its ring's bounds.
#[derive(Clone, Debug)]
pub struct Islet {
    pub pts: Vec<P>,
    pub level: f32,
    pub water: f32,
    pub at: P,
    pub r: f64,
    pub tower: bool,
    pub settlement: u32,
    pub river: bool,
    pub bbox: [f64; 4],
}

/// The narrowest river (ft) that gets an islet.
pub const RIVER_MIN_W_FT: f64 = 100.0;
const RADIUS_FT: (f64, f64) = (15.0, 45.0);
const RISE_FT: (f64, f64) = (3.0, 8.0);
/// The water's depth (ft) under a coast islet's middle. (The plan's 6 ft is rare this near a
/// shore: the drawn sea bed shelves gently, mostly 0-5 ft deep within 300 ft of the shore.)
const COAST_DEPTH_FT: (f64, f64) = (2.0, 25.0);
/// The smallest islet that may carry a watchtower (ft).
pub const TOWER_MIN_R_FT: f64 = 25.0;
const SALT: u64 = 0x15_1e75;

/// The world's islets (worked out on first use, from T0 alone).
pub fn islets(t0: &T0) -> &[Islet] {
    t0.islets.get_or_init(|| generate(t0))
}

/// The islets whose rings' bounds meet `rect` (world ft: x0, y0, x1, y1).
pub fn near<'a>(t0: &'a T0, rect: [f64; 4]) -> impl Iterator<Item = &'a Islet> + 'a {
    islets(t0).iter().filter(move |i| i.bbox[2] >= rect[0] && i.bbox[0] <= rect[2] && i.bbox[3] >= rect[1] && i.bbox[1] <= rect[3])
}

/// The islets of settlement (layout) `index`.
pub fn of(t0: &T0, index: usize) -> impl Iterator<Item = &Islet> {
    islets(t0).iter().filter(move |i| i.settlement as usize == index)
}

/// The lattice (ft) of the T0 ground the terrain is built on (`T0::from_grids`).
fn lattice(cell_ft: f64) -> f64 {
    let mut l = 2.5;
    while l < cell_ft {
        l *= 2.0;
    }
    l
}

fn generate(t0: &T0) -> Vec<Islet> {
    let mut out: Vec<Islet> = Vec::new();
    let lattice = lattice(t0.cell_ft);
    for (index, s) in t0.settlements.iter().enumerate() {
        if s.tier < Tier::City {
            continue;
        }
        // (Keyed by its place, as its windmills are.)
        let (sx, sy) = (crate::core::round(s.x) as i64, crate::core::round(s.y) as i64);
        let roll = |k: i64| unit(hash3(s.seed ^ SALT, sx, sy, k));
        let river_want = if roll(0) >= 0.5 { 0 } else if s.tier == Tier::Metropolis { 1 + (roll(1) < 0.5) as usize } else { 1 };
        let coast_want = if s.tier == Tier::City && roll(2) < 0.5 { 1 + ((roll(3) * 3.0) as usize).min(2) } else { 0 };
        if river_want + coast_want == 0 {
            continue;
        }
        let c = [s.x, s.y];
        let reach = crate::town::reach(s);
        let rect = [s.x - reach, s.y - reach, s.x + reach, s.y + reach];
        let shore = crate::town::Shore::of_t0(t0, rect, lattice);
        let roads = road_chords(t0, rect);
        let make = |at: P, r: f64, water: f64, river: bool, k: i64, out: &mut Vec<Islet>| {
            let pts = crate::ground::islet_ring(hash3(s.seed ^ SALT ^ 0x7e, crate::core::round(at[0]) as i64, crate::core::round(at[1]) as i64, k), at, r);
            let rise = RISE_FT.0 + (RISE_FT.1 - RISE_FT.0) * roll(40 + k);
            let level = (crate::core::round((water + rise) * 10.0) * 0.1) as f32;
            let tower = r >= TOWER_MIN_R_FT && roll(60 + k) < 0.5;
            let bbox = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
            out.push(Islet { pts, level, water: water as f32, at, r, tower, settlement: index as u32, river, bbox });
        };
        let clear_of = |out: &[Islet], at: P, r: f64| out.iter().all(|o| dist(o.at, at) >= o.r + r + 150.0);

        // River: points on the middle line of a wide enough river in reach.
        if river_want > 0 {
            let mut cands: Vec<(P, f64, f64)> = Vec::new();
            for (ri, k) in t0.rivers.segments_near(rect[0], rect[1], rect[2], rect[3], 0.0) {
                let rc = &t0.rivers.rivers[ri as usize];
                let len = rc.s[k as usize + 1] - rc.s[k as usize];
                let n = (len / 40.0).ceil().clamp(2.0, 600.0) as usize;
                for j in 0..n {
                    let cp = rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft);
                    if cp.w >= RIVER_MIN_W_FT && dist(cp.p, c) <= reach - RADIUS_FT.0 - 40.0 {
                        cands.push((cp.p, cp.w, cp.z));
                    }
                }
            }
            // Where the network roads cross a river near the city.
            let crossings: Vec<P> = if cands.is_empty() { Vec::new() } else { roads.iter().map(|q| q.0).filter(|&q| shore.in_river(q)).collect() };
            let mut placed = 0;
            for a in 0..2 * river_want as i64 {
                if placed >= river_want || cands.is_empty() {
                    break;
                }
                let start = ((roll(10 + a) * cands.len() as f64) as usize).min(cands.len() - 1);
                let r0 = RADIUS_FT.0 + (RADIUS_FT.1 - RADIUS_FT.0) * roll(20 + a);
                for m in 0..cands.len() {
                    let (at, w, z) = cands[(start + m) % cands.len()];
                    let r = r0.min(0.3 * w);
                    let water = z.max(t0.sample_water(at[0], at[1]) as f64);
                    let ok = dist(at, c) <= reach - r - 40.0
                        && crossings.iter().all(|&q| dist(q, at) >= 150.0)
                        && clear_of(&out, at, r)
                        && water - shore.ground(at) <= crate::ground::ISLET_MAX_DEPTH_FT
                        && (0..12).all(|k| {
                            let t = std::f64::consts::TAU * k as f64 / 12.0;
                            shore.in_river(add(at, [(r * 1.15 + 6.0) * libm::cos(t), (r * 1.15 + 6.0) * libm::sin(t)]))
                        });
                    if ok {
                        make(at, r, water, true, a, &mut out);
                        placed += 1;
                        break;
                    }
                }
            }
        }

        // Coast: out along rays from the middle, past the first shore met.
        if coast_want > 0 {
            let still = |p: P| shore.wet(p) && !shore.in_river(p);
            let mut cands: Vec<(P, f64, f64)> = Vec::new();
            const RAYS: i64 = 48;
            for a in 0..RAYS {
                let t = std::f64::consts::TAU * (a as f64 + roll(100 + a)) / RAYS as f64;
                let dir = [libm::cos(t), libm::sin(t)];
                let (mut d, mut dry, mut shore_at) = (20.0, false, None);
                while d < reach {
                    let w = still(add(c, mul(dir, d)));
                    if w && dry {
                        shore_at = Some(d);
                        break;
                    }
                    dry |= !w;
                    d += 20.0;
                }
                let Some(d0) = shore_at else { continue };
                let r = RADIUS_FT.0 + (RADIUS_FT.1 - RADIUS_FT.0) * roll(200 + a);
                // The first spot out from the shore (120 to 300 ft) deep enough.
                let mut d = 120.0 + 30.0 * roll(300 + a);
                while d <= 300.0 {
                    let at = add(c, mul(dir, d0 + d));
                    d += 30.0;
                    if dist(at, c) > reach - r - 40.0 {
                        break;
                    }
                    let water = t0.sample_water(at[0], at[1]) as f64;
                    let depth = water - shore.ground(at);
                    let ring = |rad: f64, n: usize| {
                        (0..n).all(|k| {
                            let t = std::f64::consts::TAU * k as f64 / n as f64;
                            still(add(at, [rad * libm::cos(t), rad * libm::sin(t)]))
                        })
                    };
                    if !still(at) || !(COAST_DEPTH_FT.0..=COAST_DEPTH_FT.1).contains(&depth) || !ring(110.0, 12) || !ring(r * 1.15 + 10.0, 12) || roads.iter().any(|q| seg_dist(at, q.0, q.1) < r + 60.0) {
                        continue;
                    }
                    cands.push((at, r, water));
                    break;
                }
            }
            let start = (roll(4) * cands.len() as f64) as usize;
            let mut placed = 0;
            for m in 0..cands.len() {
                if placed >= coast_want {
                    break;
                }
                let (at, r, water) = cands[(start + m) % cands.len()];
                if clear_of(&out, at, r) {
                    make(at, r, water, false, 10 + m as i64, &mut out);
                    placed += 1;
                }
            }
        }
    }
    out
}

/// The network roads over `rect` as ~20-ft chords (world ft).
fn road_chords(t0: &T0, rect: [f64; 4]) -> Vec<(P, P)> {
    let mut out = Vec::new();
    for (ri, k) in t0.roads.segments_near(rect, 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(1.0, 64.0) as usize;
        let pts: Vec<P> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 5.0, t0.cell_ft).p).collect();
        out.extend(pts.windows(2).map(|q| (q[0], q[1])));
    }
    out
}
