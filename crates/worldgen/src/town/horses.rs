//! (R5 towns, item 114) A steppe town's horse pens and horse market: their battlemap props (ids 106-107,
//! the towns lane's range) and the park kind that holds them (`ParkKind::Horses`, "horses": a steppe
//! town's parks that would be formal gardens, or any park so set by hand).
//!
//! How (`plan`, a pure function of the park): its ground trodden bare (the battlemap's `URBAN_YARD`,
//! a camp clearing's dirt); a way in from the middle of each side along a street to the market
//! ground's middle (the park's deepest point), kept clear; 2-4 corrals of post-and-rail fence along its
//! sides (10 ft in from a street, 6 ft from a back, off the ways, 7 ft apart, 16 ft or more from the
//! middle; the bigger and the nearer a corner the likelier, its main street side last), each with a
//! gate toward the middle, a water trough along its back fence, a haystack or two in its back corners,
//! 2-6 horses turned every way; the market ground between them: hitching rails along its main street
//! sides and the corrals' fronts (horses tied at some, nose to the rail), troughs and carts by its size.
//! Nothing overlaps (footprints as `size` gives them), nothing stands on a way in or a gate's approach.

use super::geom::*;
use super::{Layout, Park, ParkKind};
use crate::battlemap::{Kind, KindInfo};
use crate::core::rng::{Pcg32, hash2};
use crate::park::ParkLayout;

const fn k(id: Kind, name: &'static str, radius: f32, blocks_move: bool, blocks_sight: bool, cover: u8, difficult: bool, height_ft: f32) -> KindInfo {
    KindInfo { id: id as u16, name, radius, blocks_move, blocks_sight, cover, difficult, height_ft, hazard: None, feature: true }
}

/// The horse park's props, for the battlemap's catalog.
pub(crate) const KINDS: [(Kind, KindInfo); 2] = [
    // (About 8 ft nose to tail.)
    (Kind::Horse, k(Kind::Horse, "horse", 0.8, true, false, 1, false, 5.5)),
    // (A rail on two posts, 10 ft long along x.)
    (Kind::HitchingRail, k(Kind::HitchingRail, "hitching rail", 1.0, false, false, 0, false, 3.5)),
];

/// A corral's gate (ft wide), and how far either side of a way in nothing stands (ft).
pub const GATE_FT: f64 = 10.0;
pub const WAY_HALF: f64 = 6.0;
/// How far in from its park's side a corral's fence stands (ft): a side along a street, a back.
const FRONT_SET: f64 = 10.0;
const BACK_SET: f64 = 6.0;
/// How far (ft) a corral keeps from the market ground's middle, and from another corral.
const MARKET_CLEAR: f64 = 16.0;
const CORRAL_GAP: f64 = 7.0;
/// A gate's approach kept clear (ft): inside, outside, either side of its middle.
const GATE_IN: f64 = 6.0;
const GATE_OUT: f64 = 9.0;
const GATE_HALF: f64 = 5.5;

/// A corral: its fence's corners (the first side along its park's side), the middle of its gate and
/// the side it is on (from corner `side` to the next).
#[derive(Clone, Debug)]
pub struct Corral {
    pub pts: [P; 4],
    pub gate: P,
    pub side: usize,
}

/// A horse park as laid out (world ft).
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub corrals: Vec<Corral>,
    /// The market ground's middle (the park's deepest point).
    pub centre: P,
    /// Ways in from the streets: from a side's middle (2 ft past it) to the middle, how far either side.
    pub ways: Vec<(P, P, f64)>,
    /// What is set out: where, turn (rad), kind, look, size.
    pub props: Vec<(P, f32, Kind, u8, f32)>,
}

/// How big a horse is set out (its sprite at size 1 reads as a pony beside the carts).
pub const HORSE_SCALE: f32 = 1.35;

/// A prop's footprint (ft along its turn, across), at size 1.
pub fn size(kind: Kind) -> (f64, f64) {
    match kind {
        // (A horse set out at `HORSE_SCALE`: about 8.4 by 3.2 ft.)
        Kind::Horse => (6.2, 2.4),
        Kind::HitchingRail => (10.4, 1.4),
        Kind::WaterTrough => (7.0, 3.0),
        Kind::Haystack => (7.6, 7.6),
        Kind::Cart => (12.0, 6.0),
        _ => (5.0, 5.0),
    }
}

fn boxed(c: P, u: P, l: f64, w: f64) -> [P; 4] {
    let (a, b) = (mul(u, 0.5 * l), mul([-u[1], u[0]], 0.5 * w));
    [sub(sub(c, a), b), sub(add(c, a), b), add(add(c, a), b), add(sub(c, a), b)]
}

/// A prop's footprint as a box (`size`).
pub fn foot(kind: Kind, at: P, rot: f32, scale: f32) -> [P; 4] {
    let (l, w) = size(kind);
    let r = rot as f64;
    boxed(at, [libm::cos(r), libm::sin(r)], l * scale as f64, w * scale as f64)
}

/// A segment as a box `w` ft wide.
pub fn seg_box(a: P, b: P, w: f64) -> [P; 4] {
    let l = dist(a, b);
    boxed(lerp(a, b, 0.5), mul(sub(b, a), 1.0 / l.max(1e-9)), l, w)
}

/// Whether two boxes come within `pad` ft of each other (by their separating axes: a hit may be
/// called a little early off a corner, never missed).
pub fn hit(a: &[P; 4], b: &[P; 4], pad: f64) -> bool {
    for q in [a, b] {
        for k in 0..2 {
            let e = sub(q[k + 1], q[k]);
            let ax = mul(e, 1.0 / len(e).max(1e-9));
            let pr = |r: &[P; 4]| r.iter().fold((f64::MAX, f64::MIN), |s, p| (s.0.min(dot(*p, ax)), s.1.max(dot(*p, ax))));
            let ((a0, a1), (b0, b1)) = (pr(a), pr(b));
            if a0 > b1 + pad || b0 > a1 + pad {
                return false;
            }
        }
    }
    true
}

/// A box's outline sampled every 4 ft or less (its corners among them).
fn samples(q: &[P; 4]) -> Vec<P> {
    let mut out = Vec::new();
    for k in 0..4 {
        let (a, b) = (q[k], q[(k + 1) % 4]);
        let n = crate::core::ceil(dist(a, b) / 4.0).max(1.0) as usize;
        out.extend((0..n).map(|j| lerp(a, b, j as f64 / n as f64)));
    }
    out
}

/// How deep inside `poly` a point lies (ft to its nearest side; -1 outside).
fn depth(poly: &[P], p: P) -> f64 {
    if !contains(poly, p) {
        return -1.0;
    }
    (0..poly.len()).map(|e| seg_dist(p, poly[e], poly[(e + 1) % poly.len()])).fold(f64::MAX, f64::min)
}

/// Whether `p` lies inside `poly` at least `set[e]` ft from each side `e`.
fn set_in(poly: &[P], set: &[f64], p: P) -> bool {
    contains(poly, p) && (0..poly.len()).all(|e| seg_dist(p, poly[e], poly[(e + 1) % poly.len()]) >= set[e] - 0.01)
}

/// Park `pk` laid out (`None` unless it is a horse park).
pub fn plan(pk: &Park) -> Option<Plan> {
    if pk.kind != ParkKind::Horses || pk.pts.len() < 3 {
        return None;
    }
    let poly = &pk.pts;
    let m = poly.len();
    let mut rng = Pcg32::new(hash2(pk.key, 0x4053_e5, 1), 41);
    let front = |e: usize| pk.fronts.get(e).copied().unwrap_or(false);
    // Each side: its ends, the way along it, the way in from it, its length.
    let sides: Vec<(P, P, P, P, f64)> = (0..m)
        .map(|e| {
            let (a, b) = (poly[e], poly[(e + 1) % m]);
            let l = dist(a, b);
            let u = mul(sub(b, a), 1.0 / l.max(1e-9));
            let mut n = [-u[1], u[0]];
            if !contains(poly, add(lerp(a, b, 0.5), mul(n, 0.5))) {
                n = mul(n, -1.0);
            }
            (a, b, u, n, l)
        })
        .collect();
    // The market ground's middle: the deepest point on a grid over the park.
    let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let step = ((x1 - x0).max(y1 - y0) / 60.0).max(3.0);
    let mut centre = centroid(poly);
    let mut deep = depth(poly, centre);
    let (nx, ny) = (((x1 - x0) / step) as usize, ((y1 - y0) / step) as usize);
    for j in 0..=ny {
        for i in 0..=nx {
            let p = [x0 + i as f64 * step, y0 + j as f64 * step];
            let d = depth(poly, p);
            if d > deep + 1e-9 {
                (centre, deep) = (p, d);
            }
        }
    }
    // (The middle of the deepest ground: a long park's runs along its length.)
    let mut sum: (P, f64) = ([0.0, 0.0], 0.0);
    for j in 0..=ny {
        for i in 0..=nx {
            let p = [x0 + i as f64 * step, y0 + j as f64 * step];
            if depth(poly, p) >= deep - 1.0 {
                sum = (add(sum.0, p), sum.1 + 1.0);
            }
        }
    }
    let mid = mul(sum.0, 1.0 / sum.1.max(1.0));
    if sum.1 > 0.0 && depth(poly, mid) >= deep - 1.5 {
        centre = mid;
    }
    if deep < 8.0 {
        return Some(Plan { centre, ..Plan::default() });
    }
    // The ways in: from the middle of every side along a street (16 ft or longer).
    let main = (0..m).filter(|&e| front(e)).max_by(|&a, &b| sides[a].4.total_cmp(&sides[b].4)).unwrap_or_else(|| (0..m).max_by(|&a, &b| sides[a].4.total_cmp(&sides[b].4)).unwrap_or(0));
    let mut ways: Vec<(P, P, f64)> = (0..m)
        .filter(|&e| front(e) && sides[e].4 >= 16.0)
        .map(|e| (sub(lerp(sides[e].0, sides[e].1, 0.5), mul(sides[e].3, 2.0)), centre, WAY_HALF))
        .collect();
    if ways.is_empty() {
        ways.push((sub(lerp(sides[main].0, sides[main].1, 0.5), mul(sides[main].3, 2.0)), centre, WAY_HALF));
    }
    let way_boxes: Vec<[P; 4]> = ways.iter().filter(|w| dist(w.0, w.1) > 0.5).map(|&(a, b, h)| seg_box(a, b, 2.0 * h)).collect();
    let set: Vec<f64> = (0..m).map(|e| if front(e) { FRONT_SET } else { BACK_SET }).collect();
    let wet = |p: P| pk.ground.water_near(p, 3.0);
    // Corrals: boxes along the sides, the best first, one a side, then a second.
    let a = area(poly).abs();
    let want = if a < 12_000.0 { 2 } else if a < 30_000.0 { 3 } else { 4 };
    // (A big park's corrals bigger, to half again as big.)
    let grow = (crate::core::sqrt(a) / 150.0).clamp(1.0, 1.5);
    let mut cands: Vec<(f64, usize, [P; 4])> = Vec::new();
    for e in 0..m {
        let (pa, _, u, n, le) = sides[e];
        if le < 26.0 {
            continue;
        }
        let s = set[e];
        for d in [38.0 * grow, 30.0 * grow, 22.0] {
            for w in [52.0 * grow, 40.0 * grow, 30.0] {
                if w > le {
                    continue;
                }
                let mut t0 = 0.0;
                while t0 + w <= le + 1e-9 {
                    let q = [add(pa, add(mul(u, t0), mul(n, s))), add(pa, add(mul(u, t0 + w), mul(n, s))), add(pa, add(mul(u, t0 + w), mul(n, s + d))), add(pa, add(mul(u, t0), mul(n, s + d)))];
                    let ok = samples(&q).iter().all(|&p| set_in(poly, &set, p) && !wet(p))
                        && !contains(&q, centre)
                        && (0..4).all(|k| seg_dist(centre, q[k], q[(k + 1) % 4]) >= MARKET_CLEAR)
                        && !way_boxes.iter().any(|b| hit(&q, b, 1.5));
                    if ok {
                        let corner = t0.min(le - t0 - w);
                        let score = -(d * w) / (150.0 * grow * grow) + if e == main { 30.0 } else { 0.0 } + if front(e) { 6.0 } else { 0.0 } + corner / 20.0 + rng.range(0.0, 4.0);
                        cands.push((score, e, q));
                    }
                    t0 += 4.0;
                }
            }
        }
    }
    cands.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut chosen: Vec<(usize, [P; 4])> = Vec::new();
    for per_side in [1, 2] {
        for (_, e, q) in &cands {
            if chosen.len() >= want {
                break;
            }
            if chosen.iter().filter(|c| c.0 == *e).count() < per_side && !chosen.iter().any(|c| hit(&c.1, q, CORRAL_GAP)) {
                chosen.push((*e, *q));
            }
        }
    }
    let mut out = Plan { centre, ways, ..Plan::default() };
    let mut gates: Vec<[P; 4]> = Vec::new();
    // Each corral's gate on its side nearest the middle (a side long enough), and what stands in it.
    for (_, q) in &chosen {
        let c = mean(q);
        let side = (1..4).filter(|&k| dist(q[k], q[(k + 1) % 4]) >= GATE_FT + 8.0).min_by(|&x, &y| dist(lerp(q[x], q[(x + 1) % 4], 0.5), centre).total_cmp(&dist(lerp(q[y], q[(y + 1) % 4], 0.5), centre))).unwrap_or(2);
        let (ga, gb) = (q[side], q[(side + 1) % 4]);
        let gl = dist(ga, gb);
        let f = rng.range(0.35, 0.65).clamp((0.5 * GATE_FT + 4.0) / gl, 1.0 - (0.5 * GATE_FT + 4.0) / gl);
        let gate = lerp(ga, gb, f);
        let mut o = [-(gb[1] - ga[1]) / gl, (gb[0] - ga[0]) / gl];
        if dot(o, sub(gate, c)) < 0.0 {
            o = mul(o, -1.0);
        }
        let g = seg_box(sub(gate, mul(o, GATE_IN)), add(gate, mul(o, GATE_OUT)), 2.0 * GATE_HALF);
        gates.push(g);
        out.corrals.push(Corral { pts: *q, gate, side });
    }
    let corrals = out.corrals.clone();
    for (k, cr) in corrals.iter().enumerate() {
        let q = &cr.pts;
        let (w, d) = (dist(q[0], q[1]), dist(q[0], q[3]));
        let (u, n) = (mul(sub(q[1], q[0]), 1.0 / w), mul(sub(q[3], q[0]), 1.0 / d));
        let c = mean(q);
        let inner = boxed(c, u, w - 2.0, d - 2.0);
        let within = |f: &[P; 4]| f.iter().all(|&p| contains(&inner, p));
        let mut mine: Vec<[P; 4]> = Vec::new();
        let put = |out: &mut Plan, mine: &mut Vec<[P; 4]>, at: P, rot: f32, kind: Kind, look: u8, scale: f32, pad: f64| -> bool {
            let f = foot(kind, at, rot, scale);
            if !within(&f) || mine.iter().any(|o| hit(o, &f, pad)) || gates.iter().any(|g| hit(g, &f, 0.0)) {
                return false;
            }
            mine.push(f);
            out.props.push((at, rot, kind, look, scale));
            true
        };
        // A trough along the back fence.
        let ut = libm::atan2(u[1], u[0]) as f32;
        for t in [rng.range(0.3, 0.7), 0.5, 0.25, 0.75] {
            let at = add(q[0], add(mul(u, t * w), mul(n, 3.0)));
            if put(&mut out, &mut mine, at, ut, Kind::WaterTrough, rng.below(4) as u8, 1.0, 0.8) {
                break;
            }
        }
        // Haystacks in the back corners (two in a big corral).
        let stacks = if w * d > 1300.0 { 2 } else { 1 };
        let mut laid = 0;
        for (j, corner) in [(q[0], u), (q[1], mul(u, -1.0))].into_iter().enumerate() {
            if laid >= stacks {
                break;
            }
            let sc = rng.range(0.9, 1.05) as f32;
            let at = add(corner.0, add(mul(corner.1, 5.4), mul(n, 5.4)));
            // (Square to the fence, turned a little: a round stack's footprint a box.)
            let rot = ut + rng.range(-0.1, 0.1) as f32;
            if put(&mut out, &mut mine, at, rot, Kind::Haystack, ((k + j) as u32 + rng.below(8)) as u8 & 7, sc, 0.8) {
                laid += 1;
            }
        }
        // Horses, 2-6, turned every way.
        let most = ((w * d / 230.0) as u32).clamp(2, 6);
        let least = (most / 2).max(2);
        let n_h = least + rng.below(most - least + 1);
        let mut got = 0;
        for _ in 0..120 {
            if got >= n_h {
                break;
            }
            let at = add(c, add(mul(u, rng.range(-0.5, 0.5) * w), mul(n, rng.range(-0.5, 0.5) * d)));
            let rot = rng.range(0.0, core::f64::consts::TAU) as f32;
            let look = rng.below(4) as u8;
            if put(&mut out, &mut mine, at, rot, Kind::Horse, look, HORSE_SCALE, 0.6) {
                got += 1;
            }
        }
        // Cover within `COVER_FT` of every spot (the battlemap's guarantee): a haystack, else a horse
        // (six at most), where none is near.
        let (ni, nj) = ((w / 8.0) as usize, (d / 8.0) as usize);
        for j in 0..=nj {
            for i in 0..=ni {
                let p = add(q[0], add(mul(u, (i as f64 * 8.0).min(w)), mul(n, (j as f64 * 8.0).min(d))));
                if covered(&out.props, p) {
                    continue;
                }
                let _ = RINGS.iter().any(|off| {
                    let at = add(p, *off);
                    if !same_chunk(at, p) {
                        return false;
                    }
                    let rot = rng.range(0.0, core::f64::consts::TAU) as f32;
                    if put(&mut out, &mut mine, at, ut, Kind::Haystack, rng.below(8) as u8, 0.95, 0.8) {
                        return true;
                    }
                    got < 6 && put(&mut out, &mut mine, at, rot, Kind::Horse, rng.below(4) as u8, HORSE_SCALE, 0.6) && {
                        got += 1;
                        true
                    }
                });
            }
        }
    }
    // The market ground: what stands on it stands off the street sides (8 ft), the backs (3 ft), the
    // water, the corrals (2 ft), the ways in and the gates' approaches.
    let mset: Vec<f64> = (0..m).map(|e| if front(e) { 8.0 } else { 3.0 }).collect();
    let corral_boxes: Vec<[P; 4]> = out.corrals.iter().map(|c| c.pts).collect();
    let mut placed: Vec<[P; 4]> = Vec::new();
    let free = |f: &[P; 4], placed: &[[P; 4]], pad: f64, skip: Option<usize>| {
        samples(f).iter().all(|&p| set_in(poly, &mset, p) && !pk.ground.water_near(p, 2.0))
            && !corral_boxes.iter().any(|c| hit(c, f, 2.0))
            && !way_boxes.iter().any(|b| hit(b, f, 1.0))
            && !gates.iter().any(|g| hit(g, f, 0.5))
            && !placed.iter().enumerate().any(|(i, o)| Some(i) != skip && hit(o, f, pad))
    };
    // Hitching rails: in runs of three along the street sides, 12 ft in (the main one's six, the others'
    // two), then one along each corral's front, 3.5 ft out.
    let mut rails: Vec<(P, P, P)> = Vec::new();
    let mut street: Vec<usize> = (0..m).filter(|&e| e != main && front(e)).collect();
    street.sort_by(|&x, &y| sides[y].4.total_cmp(&sides[x].4));
    street.insert(0, main);
    for (si, &e) in street.iter().enumerate() {
        let (pa, _, u, n, le) = sides[e];
        let rot = libm::atan2(u[1], u[0]) as f32;
        let (cap, mut laid, mut run) = (if si == 0 { 6 } else { 2 }, 0, 0);
        let mut t = rng.range(0.0, 12.0);
        while t + 10.4 <= le && laid < cap {
            let at = add(pa, add(mul(u, t + 5.2), mul(n, 12.0)));
            let f = foot(Kind::HitchingRail, at, rot, 1.0);
            if free(&f, &placed, 1.5, None) {
                placed.push(f);
                rails.push((at, u, n));
                out.props.push((at, rot, Kind::HitchingRail, rng.below(4) as u8, 1.0));
                laid += 1;
                run += 1;
                t += if run % 3 == 0 { 30.0 } else { 12.4 };
            } else {
                run = 0;
                t += 4.0;
            }
        }
    }
    for cr in &corrals {
        let (ga, gb) = (cr.pts[cr.side], cr.pts[(cr.side + 1) % 4]);
        let gl = dist(ga, gb);
        let u = mul(sub(gb, ga), 1.0 / gl);
        let mut o = [-u[1], u[0]];
        if dot(o, sub(cr.gate, mean(&cr.pts))) < 0.0 {
            o = mul(o, -1.0);
        }
        let g = dot(sub(cr.gate, ga), u);
        let mut laid = 0;
        let mut t = 6.2;
        while t + 6.2 <= gl && laid < 1 {
            if (t - g).abs() < 0.5 * GATE_FT + 5.2 + 3.0 {
                t += 2.0;
                continue;
            }
            let at = add(ga, add(mul(u, t), mul(o, 3.5)));
            let rot = libm::atan2(u[1], u[0]) as f32;
            let f = foot(Kind::HitchingRail, at, rot, 1.0);
            // (Its own corral 2.8 ft off: a foot nearer than the rest keep.)
            let ok = samples(&f).iter().all(|&p| set_in(poly, &mset, p)) && !corral_boxes.iter().any(|c| hit(c, &f, 1.5)) && !way_boxes.iter().any(|b| hit(b, &f, 1.0)) && !gates.iter().any(|b| hit(b, &f, 0.5)) && !placed.iter().any(|b| hit(b, &f, 1.5));
            if ok {
                placed.push(f);
                rails.push((at, u, o));
                out.props.push((at, rot, Kind::HitchingRail, rng.below(4) as u8, 1.0));
                laid += 1;
                t += 14.0;
            } else {
                t += 3.0;
            }
        }
    }
    // Horses tied at some rails, nose to it, on its open side.
    for &(at, u, o) in &rails {
        let r = rng.next_f64();
        let offs: &[f64] = if r < 0.45 { &[] } else if r < 0.85 { &[0.0] } else { &[-2.6, 2.6] };
        let jitter = if offs.len() == 1 { rng.range(-2.5, 2.5) } else { 0.0 };
        let own = placed.iter().position(|f| dist(mean(f), at) < 0.01);
        for &s in offs {
            let p = add(at, add(mul(o, 5.3), mul(u, s + jitter)));
            let rot = (libm::atan2(-o[1], -o[0]) + rng.range(-0.12, 0.12)) as f32;
            let f = foot(Kind::Horse, p, rot, HORSE_SCALE);
            let near_own = own.is_some_and(|i| hit(&placed[i], &f, 0.0));
            if !near_own && free(&f, &placed, 0.5, own) {
                placed.push(f);
                out.props.push((p, rot, Kind::Horse, rng.below(4) as u8, HORSE_SCALE));
            }
        }
    }
    // Troughs about the middle, then carts anywhere on the market ground, a haystack for every two.
    let troughs = (1 + (a / 25_000.0) as u32).min(3);
    let reach = (deep - 8.0).clamp(6.0, 30.0);
    let mut laid = 0;
    for _ in 0..40 {
        if laid >= troughs {
            break;
        }
        let (t, rr) = (rng.range(0.0, core::f64::consts::TAU), rng.range(8.0, reach.max(8.5)));
        let p = add(centre, [rr * libm::cos(t), rr * libm::sin(t)]);
        let rot = rng.range(0.0, core::f64::consts::PI) as f32;
        let f = foot(Kind::WaterTrough, p, rot, 1.0);
        if free(&f, &placed, 1.5, None) {
            placed.push(f);
            out.props.push((p, rot, Kind::WaterTrough, rng.below(4) as u8, 1.0));
            laid += 1;
        }
    }
    let carts = (1 + rng.below(2) + (a / 15_000.0) as u32).min(6);
    let mut laid = 0;
    for _ in 0..100 {
        if laid >= carts {
            break;
        }
        let p = [rng.range(x0, x1), rng.range(y0, y1)];
        let rot = rng.range(0.0, core::f64::consts::TAU) as f32;
        let f = foot(Kind::Cart, p, rot, 1.0);
        if free(&f, &placed, 1.5, None) {
            placed.push(f);
            out.props.push((p, rot, Kind::Cart, rng.below(4) as u8, 1.0));
            laid += 1;
        }
    }
    let stacks = laid / 2;
    let mut laid = 0;
    for _ in 0..60 {
        if laid >= stacks {
            break;
        }
        let p = [rng.range(x0, x1), rng.range(y0, y1)];
        let (rot, sc) = (rng.range(0.0, core::f64::consts::TAU) as f32, rng.range(0.9, 1.05) as f32);
        let f = foot(Kind::Haystack, p, rot, sc);
        if free(&f, &placed, 1.5, None) {
            placed.push(f);
            out.props.push((p, rot, Kind::Haystack, rng.below(8) as u8, sc));
            laid += 1;
        }
    }
    // Cover within `COVER_FT` of every spot of the market ground: haystacks and carts most, a horse led
    // to market, a trough.
    let mut j = y0;
    while j <= y1 {
        let mut i = x0;
        while i <= x1 {
            let p = [i, j];
            i += 10.0;
            if !contains(poly, p) || corral_boxes.iter().any(|c| contains(c, p)) || covered(&out.props, p) {
                continue;
            }
            // (A trough only where nothing bigger fits.)
            const FILL: [Kind; 5] = [Kind::Haystack, Kind::Cart, Kind::Horse, Kind::Haystack, Kind::Cart];
            let first = rng.below(5) as usize;
            let _ = (0..2).any(|pass| {
                RINGS.iter().any(|off| {
                (0..if pass == 0 { 5 } else { 1 }).any(|t| {
                    let kind = if pass == 0 { FILL[(first + t) % 5] } else { Kind::WaterTrough };
                    let at = add(p, *off);
                    if !same_chunk(at, p) {
                        return false;
                    }
                    let rot = rng.range(0.0, core::f64::consts::TAU) as f32;
                    let sc = match kind {
                        Kind::Haystack => 0.95,
                        Kind::Horse => HORSE_SCALE,
                        _ => 1.0,
                    };
                    let f = foot(kind, at, rot, sc);
                    if !free(&f, &placed, 1.5, None) {
                        return false;
                    }
                    placed.push(f);
                    out.props.push((at, rot, kind, rng.below(if kind == Kind::Haystack { 8 } else { 4 }) as u8, sc));
                    true
                })
                })
            });
        }
        j += 10.0;
    }
    Some(out)
}

/// How far (ft, either way along the world's axes) the battlemap's guarantee lets a spot be from
/// cover: its `MAX_COVER_GAP` squares, a square to spare.
const COVER_FT: f64 = (crate::battlemap::MAX_COVER_GAP as f64 - 1.0) * crate::battlemap::SQUARE_FT - 1.0;

/// Where to try a cover prop round a spot: on it, then 8, 16 and 22 ft out every eighth of a turn.
const RINGS: [P; 25] = {
    const D: f64 = 0.7071067811865476;
    [
        [0.0, 0.0],
        [8.0, 0.0], [8.0 * D, 8.0 * D], [0.0, 8.0], [-8.0 * D, 8.0 * D], [-8.0, 0.0], [-8.0 * D, -8.0 * D], [0.0, -8.0], [8.0 * D, -8.0 * D],
        [16.0, 0.0], [16.0 * D, 16.0 * D], [0.0, 16.0], [-16.0 * D, 16.0 * D], [-16.0, 0.0], [-16.0 * D, -16.0 * D], [0.0, -16.0], [16.0 * D, -16.0 * D],
        [22.0, 0.0], [22.0 * D, 22.0 * D], [0.0, 22.0], [-22.0 * D, 22.0 * D], [-22.0, 0.0], [-22.0 * D, -22.0 * D], [0.0, -22.0], [22.0 * D, -22.0 * D],
    ]
};

/// Whether two spots lie in one battlemap chunk (the guarantee reads each chunk's cover alone).
fn same_chunk(a: P, b: P) -> bool {
    let c = crate::battlemap::SQ as f64 * crate::battlemap::SQUARE_FT;
    let k = |v: f64| crate::core::floor(v / c);
    k(a[0]) == k(b[0]) && k(a[1]) == k(b[1])
}

/// Whether a prop giving cover stands within `COVER_FT` of `p` (either way along the axes), in its chunk.
fn covered(props: &[(P, f32, Kind, u8, f32)], p: P) -> bool {
    props.iter().any(|q| matches!(q.2, Kind::Horse | Kind::Cart | Kind::Haystack | Kind::WaterTrough) && (q.0[0] - p[0]).abs() <= COVER_FT && (q.0[1] - p[1]).abs() <= COVER_FT && same_chunk(q.0, p))
}

impl Plan {
    /// Ground the battlemap's guarantee keeps clear: the ways in, the gates' approaches.
    pub fn keep(&self) -> Vec<(P, P, f64)> {
        let mut out: Vec<(P, P, f64)> = self.ways.clone();
        for cr in &self.corrals {
            let c = mean(&cr.pts);
            let mut o = sub(cr.gate, c);
            let (ga, gb) = (cr.pts[cr.side], cr.pts[(cr.side + 1) % 4]);
            let s = sub(gb, ga);
            let n = mul([-s[1], s[0]], 1.0 / len(s).max(1e-9));
            o = if dot(n, o) < 0.0 { mul(n, -1.0) } else { n };
            out.push((sub(cr.gate, mul(o, GATE_IN)), add(cr.gate, mul(o, GATE_OUT)), 0.5 * GATE_FT));
        }
        out
    }

    /// The corrals' fences: each side whole but for the gap at its gate.
    pub fn fences(&self) -> Vec<(P, P)> {
        let mut out = Vec::new();
        for cr in &self.corrals {
            for k in 0..4 {
                let (a, b) = (cr.pts[k], cr.pts[(k + 1) % 4]);
                if k != cr.side {
                    out.push((a, b));
                    continue;
                }
                let l = dist(a, b);
                let u = mul(sub(b, a), 1.0 / l.max(1e-9));
                let g = dot(sub(cr.gate, a), u);
                out.push((a, add(a, mul(u, g - 0.5 * GATE_FT))));
                out.push((add(a, mul(u, g + 0.5 * GATE_FT)), b));
            }
        }
        out
    }
}

/// A horse park's battlemap layout (`park::layout`): what it sets out, the ground kept clear.
pub fn park_layout(pk: &Park) -> ParkLayout {
    let Some(pl) = plan(pk) else { return ParkLayout::default() };
    ParkLayout { keep: pl.keep(), props: pl.props, ..ParkLayout::default() }
}

/// The corrals' fences of a layout's horse parks reaching `near` (world ft: x0, y0, x1, y1), for
/// `rural::battle`.
pub fn bounds(l: &Layout, near: [f64; 4]) -> Vec<(P, P)> {
    let hits = |p: &Park| {
        let (x0, y0, x1, y1) = p.pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, q| (a.0.min(q[0]), a.1.min(q[1]), a.2.max(q[0]), a.3.max(q[1])));
        x0 <= near[2] && x1 >= near[0] && y0 <= near[3] && y1 >= near[1]
    };
    l.parks.iter().filter(|p| p.kind == ParkKind::Horses && hits(p)).filter_map(plan).flat_map(|pl| pl.fences()).collect()
}

/// Payload site-polygon kind of a corral's fence on the plan (`payload.rs`; its ground is a camp's
/// clearing's, kind 8).
pub const PLAN_FENCE: u32 = 34;

/// A horse park on the town's plan: its trodden ground, the corrals' fences as thin lines.
pub fn plan_polys(pk: &Park, push: &mut dyn FnMut(&[P], u32)) {
    let Some(pl) = plan(pk) else { return };
    push(&pk.pts, 8);
    for (a, b) in pl.fences() {
        if dist(a, b) > 0.5 {
            push(&seg_box(a, b, 1.6), PLAN_FENCE);
        }
    }
}
