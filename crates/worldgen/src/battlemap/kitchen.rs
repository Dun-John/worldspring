//! A castle's kitchen garden and its smithy's anvil (`dress_bailey`, v63 castles): rows of
//! vegetables and herbs in a wattle fence by the kitchen, a beehive or two against it; an anvil
//! with its quench tub before the smithy. (Topiary and flowerbeds are a palace's: `palace::gardens`.)

use super::{Kind, KindInfo, SQUARE_FT, info, k};
use crate::core::rng::{hash2, unit};
use crate::town::Building;
use crate::town::geom::{add, area, centroid, dist, dot, lerp, mul, seg_dist, sub};

type P = [f64; 2];
type Prop = (P, f32, Kind, u8, f32);
/// Room for something `r` ft round at a place, clear of what is `placed` (and of the building at
/// `own` in the bailey's list by a little): `dress_bailey`'s `free`.
type Free<'a> = &'a dyn Fn(&[(P, f64)], P, f64, Option<usize>) -> bool;

/// The kinds (the castles lane's ids, 75-87), with their rules: rows and the fence are low, seen
/// across and stepped over (difficult); a skep and an anvil are stood round.
pub(super) const KINDS: [(Kind, KindInfo); 4] = [
    (Kind::VegRow, k(Kind::VegRow, "vegetable row", 1.0, false, false, 0, true, 1.5, true)),
    (Kind::WattleFence, k(Kind::WattleFence, "wattle fence", 0.55, false, false, 1, true, 3.0, true)),
    (Kind::Beehive, k(Kind::Beehive, "beehive", 0.45, true, false, 1, false, 3.0, true)),
    (Kind::Anvil, k(Kind::Anvil, "anvil", 0.6, true, false, 1, false, 3.0, true)),
];

/// A row's length (a `VegRow` at scale 1), a fence piece's, ft.
const ROW: f64 = 10.0;
const PIECE: f64 = 5.0;

fn along(d: P) -> f32 {
    libm::atan2(d[1], d[0]) as f32
}

/// A building's sides, its long sides first, each kind deepest in the bailey first: their ends, the
/// way along them and the way out of the building.
fn yard_sides(b: &Building, depth: &dyn Fn(P) -> f64) -> Vec<(P, P, P, P)> {
    let p = &b.poly;
    if p.len() != 4 {
        return Vec::new();
    }
    let mid = |e: usize| lerp(p[e], p[(e + 1) % 4], 0.5);
    let long = |e: usize| dist(p[e], p[(e + 1) % 4]) >= dist(p[(e + 1) % 4], p[(e + 2) % 4]) - 0.5;
    let mut es: Vec<usize> = (0..4).collect();
    es.sort_by(|&x, &y| long(y).cmp(&long(x)).then(depth(mid(y)).total_cmp(&depth(mid(x)))).then(y.cmp(&x)));
    es.into_iter()
        .map(|e| {
            let (a, q) = (p[e], p[(e + 1) % 4]);
            let u = mul(sub(q, a), 1.0 / dist(a, q).max(1e-9));
            let n0 = [-u[1], u[0]];
            let n = if dot(sub(mid(e), centroid(p)), n0) >= 0.0 { n0 } else { mul(n0, -1.0) };
            (a, q, u, n)
        })
        .collect()
}

/// An anvil with its quench tub before each smithy of the bailey's `ranges` (a castle's buildings
/// in it): on its yard side a little to one side of the middle, else its other sides, where there
/// is room.
pub(super) fn anvils(key: u64, ranges: &[&&Building], blds: &[&Building], depth: &dyn Fn(P) -> f64, free: Free, placed: &mut Vec<(P, f64)>, out: &mut Vec<Prop>) {
    let rad = info(Kind::Anvil as u16).radius as f64 * SQUARE_FT;
    for (si, b) in ranges.iter().filter(|b| b.func.is_some_and(|f| crate::town::catalog::CATALOG[f as usize].key == "blacksmith")).enumerate() {
        let Some(own) = blds.iter().position(|o| std::ptr::eq(*o, **b)) else { continue };
        let first = if unit(hash2(key, 500 + si as i64, 0x6b17)) < 0.5 { [0.3, 0.7] } else { [0.7, 0.3] };
        let spot = yard_sides(b, depth).into_iter().find_map(|(a, q, u, n)| {
            [first[0], first[1], 0.5, 0.15, 0.85].into_iter().map(|t| (add(lerp(a, q, t), mul(n, rad + 2.0)), u)).find(|&(p, _)| free(placed, p, rad, Some(own)))
        });
        if let Some((p, u)) = spot {
            placed.push((p, rad));
            out.push((p, along(u), Kind::Anvil, (hash2(key, 510 + si as i64, 1) >> 20) as u8 & 3, 1.0));
        }
    }
}

/// The kitchen garden: three to five rows each side of a walk (fewer, or one bed, where room is
/// short), a wattle fence round them open where the walk meets the side toward the kitchen, one or
/// two skeps against its far side; whether one was laid out. Its place: off the kitchen range (the
/// castle's biggest plain range among `ranges`), its yard side first, else as near the keep as
/// there is room (its kitchen is in its hall), square to the nearest stretch of curtain. Pieces of
/// fence each lie in one chunk.
#[allow(clippy::too_many_arguments)]
pub(super) fn garden(
    key: u64,
    ranges: &[&&Building],
    blds: &[&Building],
    keep_at: P,
    ring: &[P],
    spots: &[P],
    to_keep: &dyn Fn(P) -> f64,
    depth: &dyn Fn(P) -> f64,
    free: Free,
    placed: &mut Vec<(P, f64)>,
    out: &mut Vec<Prop>,
) -> bool {
    let roll = |k: i64| unit(hash2(key, 400 + k, 0x6b17));
    let most = 3 + (roll(1) * 3.0) as usize;
    let kitchen = ranges.iter().filter(|b| b.func.is_none() && b.poly.len() == 4).max_by(|a, b| area(&a.poly).abs().total_cmp(&area(&b.poly).abs()));
    let own = kitchen.and_then(|b| blds.iter().position(|o| std::ptr::eq(*o, **b)));
    let sides = kitchen.map_or(Vec::new(), |b| yard_sides(b, depth));
    let mut near: Vec<P> = spots.to_vec();
    near.sort_by(|&a, &b| to_keep(a).total_cmp(&to_keep(b)).then(a[1].total_cmp(&b[1])).then(a[0].total_cmp(&b[0])));
    let rn = ring.len();
    let mut found: Option<(P, P, P, usize, usize)> = None;
    for (rows, beds) in [(most, 2), (3, 2), (most.min(4), 1), (3, 1)] {
        let (hw, hd) = (if beds == 2 { ROW + 5.0 } else { 0.5 * ROW + 2.5 }, rows as f64 * 2.5 + 2.5);
        // (Room for the garden at `c`, `u` along its rows and `v` from the kitchen side to the far one.)
        let fits = |c: P, u: P, v: P, own: Option<usize>| {
            let at = |s: f64, t: f64| add(c, add(mul(u, s), mul(v, t)));
            let corners = [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)];
            corners.iter().all(|&(s, t)| free(placed, at(s, t), 3.0, own)) && {
                let (ns, nt) = (crate::core::ceil(2.0 * hw / 5.0) as i64, crate::core::ceil(2.0 * hd / 5.0) as i64);
                (0..=ns).all(|i| (0..=nt).all(|j| free(placed, at(-hw + 2.0 * hw * i as f64 / ns as f64, -hd + 2.0 * hd * j as f64 / nt as f64), 3.0, own)))
            }
        };
        // By the kitchen range, its fence a few feet off it.
        'by: for &(a, q, u, n) in &sides {
            let m = lerp(a, q, 0.5);
            for gap in [6.0, 9.0, 14.0] {
                for t in [0.0, -8.0, 8.0, -16.0, 16.0] {
                    let c = add(add(m, mul(n, gap + hd)), mul(u, t));
                    if fits(c, u, n, own) {
                        found = Some((c, u, n, rows, beds));
                        break 'by;
                    }
                }
            }
        }
        // Else as near the keep as it fits, its gate toward the keep.
        if found.is_none() {
            found = near.iter().find_map(|&c| {
                let e = (0..rn).min_by(|&x, &y| seg_dist(c, ring[x], ring[(x + 1) % rn]).total_cmp(&seg_dist(c, ring[y], ring[(y + 1) % rn])).then(x.cmp(&y)))?;
                let u = mul(sub(ring[(e + 1) % rn], ring[e]), 1.0 / dist(ring[e], ring[(e + 1) % rn]).max(1e-9));
                let v0 = [-u[1], u[0]];
                let v = if dot(sub(c, keep_at), v0) >= 0.0 { v0 } else { mul(v0, -1.0) };
                fits(c, u, v, None).then_some((c, u, v, rows, beds))
            });
        }
        if found.is_some() {
            break;
        }
    }
    let Some((c, u, v, rows, beds)) = found else { return false };
    let (hw, hd) = (if beds == 2 { ROW + 5.0 } else { 0.5 * ROW + 2.5 }, rows as f64 * 2.5 + 2.5);
    let at = |s: f64, t: f64| add(c, add(mul(u, s), mul(v, t)));
    let (ru, rv) = (along(u), along(v));
    // The rows: two crops to a bed, turn about.
    let a = (roll(10) * 4.0) as u8 & 3;
    let b = (a + 1 + (roll(11) * 3.0) as u8) & 3;
    let beds_at: &[(u8, f64)] = if beds == 2 { &[(0, -0.5 * (hw + 5.0) + 2.5), (2, 0.5 * (hw + 5.0) - 2.5)] } else { &[(0, 0.0)] };
    for &(side, s) in beds_at {
        for j in 0..rows {
            let crop = (if j % 2 == 0 { a } else { b } + side) & 3;
            out.push((at(s, (j as f64 + 0.5) * 5.0 - rows as f64 * 2.5), ru, Kind::VegRow, crop, 1.0));
        }
    }
    // The fence, piece by piece round it, open across the walk's end toward the kitchen.
    let look = (roll(30) * 4.0) as u8 & 3;
    let mut fence: Vec<Prop> = Vec::new();
    let n_s = crate::core::round(2.0 * hw / PIECE) as i64;
    let n_t = crate::core::round(2.0 * hd / PIECE) as i64;
    for i in 0..n_s {
        let s = -hw + (i as f64 + 0.5) * PIECE;
        if s.abs() > if beds == 2 { PIECE } else { 0.5 * PIECE } {
            fence.push((at(s, -hd), ru, Kind::WattleFence, look, 1.0));
        }
        fence.push((at(s, hd), ru, Kind::WattleFence, look, 1.0));
    }
    for j in 0..n_t {
        let t = -hd + (j as f64 + 0.5) * PIECE;
        for s in [-hw, hw] {
            fence.push((at(s, t), rv, Kind::WattleFence, look, 1.0));
        }
    }
    out.extend(fence.iter().flat_map(|pr| crate::park::seam_fit(pr, 3)));
    // Skeps against the far side, outside.
    let hives = if roll(40) < 0.5 { 2 } else { 1 };
    let first = if roll(41) < 0.5 { -1.0 } else { 1.0 };
    let hr = info(Kind::Beehive as u16).radius as f64 * SQUARE_FT;
    for i in 0..hives {
        let p = at(first * if i == 0 { 1.0 } else { -1.0 } * (hw - 4.0), hd + hr + 1.5);
        if free(placed, p, hr, None) {
            placed.push((p, hr));
            out.push((p, 0.0, Kind::Beehive, (hash2(key, 420, i) >> 20) as u8 & 3, 1.0));
        }
    }
    placed.push((c, libm::hypot(hw, hd) + 1.0));
    true
}
