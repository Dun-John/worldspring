//! Quays, piers and shores: cargo, works, bollards and armour along the water.

use super::*;

/// Cargo along a quay: stacks of crates and barrels, now and then a cart, every 22 to 45 ft down
/// its middle. A pure function of the quay (world ft).
pub(super) fn quay_cargo(poly: &[[f64; 2]], seed: u64, out: &mut Vec<([f64; 2], f32, Kind)>) {
    use crate::town::geom::{add, centroid, contains, dot, main_axis, mul, sub};
    let c = centroid(poly);
    let mut rng = Pcg32::new(hash2(seed ^ 0x9a1e, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64), 17);
    let eu = main_axis(poly);
    let ev = [-eu[1], eu[0]];
    let (mut u0, mut u1) = (f64::MAX, f64::MIN);
    for q in poly {
        let u = dot(sub(*q, c), eu);
        (u0, u1) = (u0.min(u), u1.max(u));
    }
    let mut u = u0 + rng.range(6.0, 20.0);
    while u < u1 - 6.0 {
        let base = add(c, mul(eu, u));
        if rng.next_f64() < 0.12 {
            // Sprites face south (+y) unturned: a cart lies along the quay.
            let rot = (libm::atan2(eu[1], eu[0]) - std::f64::consts::FRAC_PI_2) as f32;
            if contains(poly, base) {
                out.push((base, rot, Kind::Cart));
            }
        } else {
            for _ in 0..1 + rng.below(3) {
                let p = add(base, add(mul(eu, rng.range(-4.0, 4.0)), mul(ev, rng.range(-2.5, 2.5))));
                let kind = if rng.next_f64() < 0.55 { Kind::Crate } else { Kind::Barrel };
                let rot = rng.range(0.0, std::f64::consts::TAU) as f32;
                if contains(poly, p) {
                    out.push((p, rot, kind));
                }
            }
        }
        u += rng.range(22.0, 45.0);
    }
}

/// R5 M20: a quay's bollards every 20-30 ft along its water edge (side 0-1, as the town's quays
/// and those built by hand are laid; P2 160: a city's quay front's face, `edge`, a line along the
/// shore), 2 ft in; and on a city's quay, a crane at a quarter of the way along each 50 ft or more
/// of it, clear of the piers' roots, its jib out over the water.
pub(super) fn quay_works(poly: &[[f64; 2]], edge: &[[f64; 2]], seed: u64, piers: &[Vec<[f64; 2]>], cranes: bool, out: &mut Vec<([f64; 2], f32, Kind, u8, bool)>) {
    use crate::town::geom::{add, contains, dist, lerp, mul, sub};
    if poly.len() < 4 || edge.len() < 2 {
        return;
    }
    let el: f64 = edge.windows(2).map(|w| dist(w[0], w[1])).sum();
    if el < 10.0 {
        return;
    }
    // (The point `t` ft along the edge and the way in there: toward the quay's far side.)
    let at = |t: f64| -> ([f64; 2], [f64; 2]) {
        let mut t = t;
        for (i, w) in edge.windows(2).enumerate() {
            let d = dist(w[0], w[1]);
            if t <= d || i + 2 == edge.len() {
                let u = mul(sub(w[1], w[0]), 1.0 / d.max(1e-9));
                let p = lerp(w[0], w[1], (t / d.max(1e-9)).min(1.0));
                let mut v = [-u[1], u[0]];
                if !contains(poly, add(p, mul(v, 1.0))) {
                    v = mul(v, -1.0);
                }
                return (p, v);
            }
            t -= d;
        }
        (edge[0], [0.0, 0.0])
    };
    let (a, b) = (edge[0], edge[edge.len() - 1]);
    let roots: Vec<[f64; 2]> = piers.iter().filter_map(|p| crate::town::harbour::pier_ends(p)).map(|e| e.0).collect();
    let h = hash3(seed ^ 0xb011, crate::core::round(a[0]) as i64, crate::core::round(a[1]) as i64, crate::core::round(b[1]) as i64);
    let mut t = 6.0 + 8.0 * unit(h);
    let mut k = 0i64;
    while t < el - 4.0 {
        let (q, v) = at(t);
        let p = add(q, mul(v, 2.0));
        if roots.iter().all(|r| dist(*r, p) > 7.0) {
            out.push((p, 0.0, Kind::Bollard, (hash2(h, k, 1) >> 20) as u8 & 3, false));
        }
        k += 1;
        t += 20.0 + 10.0 * unit(hash2(h, k, 2));
    }
    if cranes && el >= 50.0 {
        let f = if unit(h ^ 9) < 0.5 { 0.25 } else { 0.75 };
        let (q, v) = at(f * el);
        let p = add(q, mul(v, 3.0));
        if roots.iter().all(|r| dist(*r, p) > 18.0) {
            out.push((p, libm::atan2(-v[1], -v[0]) as f32, Kind::DockCrane, (h >> 24) as u8 & 3, false));
        }
    }
}

/// P145: how far from the shore of water drawn by hand its bank is dressed (ft; each kind less or
/// as far), and how near a castle or wall a moat's bank is stone.
pub(super) const HAND_BANK_FT: f64 = 8.0;
/// P146: a quay's paving beside dug water of the sea: squares whose middle is this near its edge
/// (two squares: drawn with its smooth edge, about 7 ft of paving shows).
pub(super) const QUAY_FT: f64 = 9.5;
pub(super) const MOAT_STONE_FT: f64 = 60.0;

/// The walls and castle curtains (as pieces) within `MOAT_STONE_FT` of `rect`: the towns' walls,
/// castles' curtains and the castles and walls drawn by hand.
pub(super) fn guarded_near(world: &World, t0: &T0, rect: [f64; 4]) -> Vec<([f64; 2], [f64; 2])> {
    let m = MOAT_STONE_FT;
    let r = [rect[0] - m, rect[1] - m, rect[2] + m, rect[3] + m];
    let mut out = Vec::new();
    let ring = |p: &[[f64; 2]], closed: bool, out: &mut Vec<([f64; 2], [f64; 2])>| {
        for w in p.windows(2) {
            out.push((w[0], w[1]));
        }
        if closed && p.len() > 2 {
            out.push((p[p.len() - 1], p[0]));
        }
    };
    for l in crate::town::layouts_near(world, t0, r) {
        for w in &l.walls {
            ring(w, false, &mut out);
        }
        for &(c, rad) in &l.castles {
            let pts: Vec<[f64; 2]> = (0..24).map(|k| {
                let a = k as f64 * core::f64::consts::TAU / 24.0;
                [c[0] + rad * libm::cos(a), c[1] + rad * libm::sin(a)]
            }).collect();
            ring(&pts, true, &mut out);
        }
    }
    for c in world.file.edits.created.iter().filter(|c| matches!(c.kind.as_str(), "castle" | "wall" | "tower")) {
        if !c.poly.is_empty() {
            ring(&c.poly, c.kind != "wall", &mut out);
        } else {
            out.push(([c.x, c.y], [c.x, c.y]));
        }
    }
    out
}

/// P145: what stands along the shore of water drawn by hand, at points spaced by length along
/// its shore from its first point (so every chunk finds the same; each kept by the chunk holding
/// it, on its dry bank): reeds round a pond, pebbles round a lake, bollards round a basin, pebbles
/// and a few reeds along a stream. An inlet, a moat and dug still water have none. By look
/// (`waters::Form::look`, the sea at `sea` ft).
pub(super) fn hand_shore_things(edits: &crate::world::Edits, seed: u64, sea: f64, rect: [f64; 4], out: &mut Vec<([f64; 2], f32, Kind, u8, bool)>) {
    use crate::waters::WaterKind as W;
    for w in edits.waters.values() {
        let look = w.look(sea);
        let (step, out_ft) = match look.kind {
            W::Pond | W::Lake if look.dug => continue,
            W::Pond => (9.0, 3.5),
            W::Lake => (22.0, 3.5),
            // (On the quay's paving, near its edge.)
            W::Basin => (18.0, 4.0),
            W::Stream => (14.0, 3.0),
            W::Inlet | W::Moat => continue,
        };
        let sh = w.shore();
        let b = crate::ground::bbox(&sh);
        if b[2] + 10.0 < rect[0] || b[0] - 10.0 > rect[2] || b[3] + 10.0 < rect[1] || b[1] - 10.0 > rect[3] || w.problem().is_some() {
            continue;
        }
        let n = sh.len();
        // (Outward: to the right of the shore when its area is positive.)
        let sign = if crate::town::geom::area(&sh) > 0.0 { -1.0 } else { 1.0 };
        let h = hash2(seed ^ 0x5a0e_5001, crate::core::round(w.pts[0][0]) as i64, crate::core::round(w.pts[0][1]) as i64);
        let (mut along, mut next, mut k) = (0.0, step * unit(h), 0i64);
        for i in 0..n {
            let (a, c) = (sh[i], sh[(i + 1) % n]);
            let l = crate::town::geom::dist(a, c);
            if l < 1e-6 {
                continue;
            }
            let (ux, uy) = ((c[0] - a[0]) / l, (c[1] - a[1]) / l);
            while next <= along + l {
                let t = next - along;
                let r = hash2(h, k, 7);
                k += 1;
                next += step * (0.7 + 0.6 * unit(r));
                let p = [a[0] + ux * t + sign * uy * out_ft, a[1] + uy * t - sign * ux * out_ft];
                if p[0] < rect[0] || p[1] < rect[1] || p[0] >= rect[2] || p[1] >= rect[3] {
                    continue;
                }
                let v = (r >> 20) as u8 & 3;
                let kind = match look.kind {
                    W::Pond if unit(r ^ 1) < 0.75 => Kind::Reeds,
                    W::Lake if unit(r ^ 1) < 0.45 => Kind::RockSmall,
                    W::Basin => Kind::Bollard,
                    W::Stream if unit(r ^ 1) < 0.35 => Kind::RockSmall,
                    W::Stream if unit(r ^ 1) < 0.5 => Kind::Reeds,
                    _ => continue,
                };
                out.push((p, 0.0, kind, v, false));
            }
            along += l;
        }
    }
}

/// R5 M20: bollards along both sides of a pier's deck (`rect`: corners 0 and 3 at its root), every
/// 20 ft or so, staggered, 1.5 ft in from its edges.
pub(super) fn pier_bollards(d: &[[f64; 2]], seed: u64, out: &mut Vec<([f64; 2], f32, Kind, u8, bool)>) {
    use crate::town::geom::{add, dist, mul, sub};
    let Some((r, t)) = crate::town::harbour::pier_ends(d) else { return };
    let len = dist(r, t);
    let half = 0.5 * dist(d[0], d[3]);
    if len < 15.0 || half < 3.0 {
        return;
    }
    let u = mul(sub(t, r), 1.0 / len);
    let v = [-u[1], u[0]];
    let h = hash2(seed ^ 0xb0a1, crate::core::round(r[0]) as i64, crate::core::round(r[1]) as i64);
    let mut at = 8.0;
    let mut k = 0i64;
    while at < len - 2.0 {
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        out.push((add(add(r, mul(u, at)), mul(v, side * (half - 1.5))), 0.0, Kind::Bollard, (hash2(h, k, 0) >> 20) as u8 & 3, false));
        k += 1;
        at += 15.0;
    }
}

/// R5 M20: a breakwater's armour (`ShapeKind::Breakwater`): its edges in pieces about 30 ft long,
/// each in the chunk holding its middle, where the square 7 ft out from it (away from the line)
/// is under water.
#[allow(clippy::too_many_arguments)]
pub(super) fn armour(line: &[[f64; 2]], edges: &[Vec<[f64; 2]>], hh: &[f32], hw: &[f32], hx0: f64, hy0: f64, owns: &dyn Fn([f64; 2]) -> bool, local: &dyn Fn(&[f64; 2]) -> [f32; 2], shapes: &mut Vec<VectorShape>) {
    use crate::town::geom::{add, dist, lerp, mul, sub};
    let nearest = |p: [f64; 2]| {
        let mut best = (f64::MAX, line[0]);
        for w in line.windows(2) {
            let ab = sub(w[1], w[0]);
            let t = (crate::town::geom::dot(sub(p, w[0]), ab) / crate::town::geom::dot(ab, ab).max(1e-12)).clamp(0.0, 1.0);
            let q = add(w[0], mul(ab, t));
            let d = dist(p, q);
            if d < best.0 {
                best = (d, q);
            }
        }
        best.1
    };
    for e in edges {
        let total: f64 = e.windows(2).map(|w| dist(w[0], w[1])).sum();
        if total < 1.0 {
            continue;
        }
        let n = crate::core::ceil(total / 30.0).max(1.0) as usize;
        let at = |s: f64| {
            let mut s = s;
            for w in e.windows(2) {
                let l = dist(w[0], w[1]);
                if s <= l {
                    return lerp(w[0], w[1], s / l.max(1e-9));
                }
                s -= l;
            }
            *e.last().unwrap()
        };
        for k in 0..n {
            let (s0, s1) = (total * k as f64 / n as f64, total * (k + 1) as f64 / n as f64);
            let mid = at(0.5 * (s0 + s1));
            if !owns(mid) {
                continue;
            }
            let q = nearest(mid);
            let out = mul(sub(mid, q), 1.0 / dist(mid, q).max(1e-9));
            // (Past the made ground's eased edge.)
            // (Nearer by the chunk's edge, inside its halo.)
            let sq = |t: [f64; 2]| (((t[0] - hx0) / SQUARE_FT).floor(), ((t[1] - hy0) / SQUARE_FT).floor());
            let inside = |(i, j): (f64, f64)| i >= 0.0 && j >= 0.0 && i < HS as f64 && j < HS as f64;
            let (i, j) = Some(sq(add(mid, mul(out, 7.0)))).filter(|&q| inside(q)).unwrap_or_else(|| sq(add(mid, mul(out, 4.0))));
            if !inside((i, j)) {
                continue;
            }
            let h = j as usize * HS + i as usize;
            if hw[h] <= hh[h] {
                continue;
            }
            let m = 1 + crate::core::ceil((s1 - s0) / 6.0) as usize;
            // (Run so the water lies on the left-hand normal (-uy, ux), where the foam is drawn.)
            let d = sub(at(s1), at(s0));
            let flip = d[0] * out[1] - d[1] * out[0] < 0.0;
            let pts: Vec<[f32; 2]> = (0..=m).map(|j| if flip { m - j } else { j }).map(|j| local(&at(s0 + (s1 - s0) * j as f64 / m as f64))).collect();
            shapes.push(VectorShape { kind: ShapeKind::Breakwater, size: (9.0 / SQUARE_FT) as f32, pts });
        }
    }
}
