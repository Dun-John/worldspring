//! A ruin's plan at site zoom (v63 underground): payload site polygons drawn instead of the
//! roofless box, so a plan reads as the battlemap below it. The same broken walls as
//! `generate`'s ruins (the same runs and breaches, keyed by world position), its rubble heap,
//! a few fallen stones by the breaches, and the floor between.

use crate::core::rng::{hash2, unit};
use crate::town::geom::{P, add, centroid, dist, lerp, mul, sub};

// v63 underground: payload site-polygon kinds (`payload::pack_sites`, drawn in `gen/ruinPlan.ts`).
/// The floor inside the old walls (pale, unoutlined).
pub const FLOOR: u32 = 28;
/// A stretch of wall still standing.
pub const WALL: u32 = 29;
/// The rubble heap inside, and stones fallen by a breach.
pub const RUBBLE: u32 = 30;

/// Wall thickness (ft) on the plan.
const WALL_FT: f64 = 3.0;

/// The pieces (kind, polygon in world ft) of a ruined building's plan, in drawing order.
/// `seed` is the battlemap stream (`world.stream("battlemap")`).
pub fn footprint(seed: u64, poly: &[P]) -> Vec<(u32, Vec<P>)> {
    let sq = super::SQUARE_FT;
    let wseed = hash2(seed, 0x2a11, 5);
    let mut out = vec![(FLOOR, poly.to_vec())];
    if poly.len() < 3 {
        return out;
    }
    let c = centroid(poly);
    let m = poly.len();
    let mut stones: Vec<P> = Vec::new();
    for e in 0..m {
        let (a, b) = (poly[e], poly[(e + 1) % m]);
        let len = dist(a, b);
        if len < 1e-6 {
            continue;
        }
        let steps = (len / sq).floor().max(1.0) as usize;
        let along = mul(sub(b, a), 1.0 / len);
        // Inward: the side the middle is on.
        let mut inward = [-along[1], along[0]];
        if (c[0] - a[0]) * inward[0] + (c[1] - a[1]) * inward[1] < 0.0 {
            inward = mul(inward, -1.0);
        }
        // Each step stands or not as on the battlemap; standing steps join into runs.
        let mut run: Option<usize> = None;
        for s in 0..=steps {
            let stands = s < steps && {
                let p = lerp(a, b, (s as f64 + 0.5) / steps as f64);
                let h = hash2(wseed, (p[0] * 2.0) as i64, (p[1] * 2.0) as i64);
                let r = unit(hash2(wseed ^ 0x77, (p[0] / 20.0) as i64, (p[1] / 20.0) as i64));
                if !(r < 0.3 || unit(h) < 0.15) {
                    true
                } else {
                    // A breach in a standing run leaves a stone or two outside it.
                    if r >= 0.3 && unit(h >> 8) < 0.7 {
                        stones.push(add(p, mul(inward, -(0.8 + 2.5 * unit(h >> 16)))));
                    }
                    false
                }
            };
            match (stands, run) {
                (true, None) => run = Some(s),
                (false, Some(s0)) => {
                    out.push((WALL, wall_piece(wseed, a, b, steps, s0, s, inward)));
                    run = None;
                }
                _ => {}
            }
        }
    }
    // The heap where the roof fell (as the battlemap's rock pile, when it has one).
    let h = hash2(wseed, (c[0] * 2.0) as i64, (c[1] * 2.0) as i64);
    if unit(h) < 0.7 {
        out.push((RUBBLE, blob(h, c, 4.0)));
    }
    for (k, s) in stones.iter().enumerate() {
        let hs = hash2(wseed ^ 0x51, (s[0] * 2.0) as i64, (s[1] * 2.0) as i64) ^ k as u64;
        out.push((RUBBLE, blob(hs, *s, 1.0 + 0.8 * unit(hs >> 4))));
    }
    out
}

/// A standing run of steps s0..s1 along edge a-b, its broken ends slanted a little.
fn wall_piece(wseed: u64, a: P, b: P, steps: usize, s0: usize, s1: usize, inward: P) -> Vec<P> {
    let n = steps as f64;
    let (t0, t1) = (s0 as f64 / n, s1 as f64 / n);
    let along = mul(sub(b, a), 1.0 / dist(a, b));
    // A broken end (not the building's corner) slants by up to a foot.
    let slant = |t: f64, k: u64| {
        if t <= 0.0 || t >= 1.0 {
            0.0
        } else {
            let p = lerp(a, b, t);
            (unit(hash2(wseed ^ k, (p[0] * 2.0) as i64, (p[1] * 2.0) as i64)) - 0.5) * 2.0
        }
    };
    let (p0, p1) = (lerp(a, b, t0), lerp(a, b, t1));
    let q0 = add(add(p0, mul(inward, WALL_FT)), mul(along, slant(t0, 0x31)));
    let q1 = add(add(p1, mul(inward, WALL_FT)), mul(along, slant(t1, 0x32)));
    vec![p0, p1, q1, q0]
}

/// A small rough stone or heap: a lumpy heptagon of radius r round c.
fn blob(h: u64, c: P, r: f64) -> Vec<P> {
    let a0 = unit(h >> 3) * std::f64::consts::TAU;
    (0..7)
        .map(|k| {
            let a = a0 + std::f64::consts::TAU * k as f64 / 7.0;
            let rr = r * (0.8 + 0.2 * unit(h >> (5 + 4 * k)));
            add(c, [rr * libm::cos(a), rr * libm::sin(a)])
        })
        .collect()
}
