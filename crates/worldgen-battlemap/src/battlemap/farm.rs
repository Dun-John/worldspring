//! v63 countryside: where a settlement's haystacks stand on the battlemap. Not strewn over its
//! fields: one to three in a stack yard at the back of some crofts, two or three at a farmstead's
//! (a manor's) yard corner, and on some pastures and fallows a cluster of two or three inside the
//! field by its gate (where it comes nearest a lane or a road, else by the farmstead it touches),
//! to the side of the way in. None on arable, in orchards or copses, or out on wild ground; a field
//! touching no lane or farmstead has none. Each keyed by its place (world ft), so a chunk's share
//! never depends on which chunk asks.

use crate::core::rng::{hash3, unit};
use crate::town::Layout;
use crate::town::fields::FieldUse;
use crate::town::geom::{add, centroid, contains, dist, lerp, mul, seg_dist, sub};
use crate::town::village::PlotKind;

type P = [f64; 2];

/// How near a lane or a road a field's gate is looked for (ft), as `town::rural`'s gate.
const GATE_WAY_FT: f64 = 45.0;
/// How near a farmstead (a croft, a yard, a building) a field without a gate keeps its hay (ft).
const FARMSTEAD_FT: f64 = 15.0;
/// A village lane's width (ft; `town::village`'s).
const LANE_W: f64 = 10.0;
/// A stack's room from its plot's outline (ft).
const EDGE_FT: f64 = 5.0;

fn key(p: P, salt: i64) -> u64 {
    hash3(0x4a75_7ac5, crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64, salt)
}

fn outline_dist(poly: &[P], p: P) -> f64 {
    (0..poly.len()).map(|k| seg_dist(p, poly[k], poly[(k + 1) % poly.len()])).fold(f64::MAX, f64::min)
}

fn unit_v(v: P) -> P {
    let l = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]).max(1e-9);
    [v[0] / l, v[1] / l]
}

/// The haystacks of layout `l` whose middle lies in `rect` (world ft): middle, rotation, scale (a
/// big rick about 8 ft across, the rest about 5-6 ft).
pub(super) fn stacks(l: &Layout, rect: [f64; 4], out: &mut Vec<(P, f32, f32)>) {
    let near = [rect[0] - 300.0, rect[1] - 300.0, rect[2] + 300.0, rect[3] + 300.0];
    let hits = |poly: &[P]| {
        let b = poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        b[0] <= near[2] && near[0] <= b[2] && b[1] <= near[3] && near[1] <= b[3]
    };
    let inside = |p: P| p[0] >= rect[0] && p[0] < rect[2] && p[1] >= rect[1] && p[1] < rect[3];
    let in_building = |p: P, r: f64| l.buildings.iter().any(|b| contains(&b.poly, p) || outline_dist(&b.poly, p) < r);
    let mut put = |pts: &[(P, f64)], seed: u64, plot: &[P], keep: &dyn Fn(P) -> bool| {
        for (j, &(p, scale)) in pts.iter().enumerate() {
            if !contains(plot, p) || outline_dist(plot, p) < EDGE_FT || in_building(p, 6.0) || !keep(p) {
                continue;
            }
            if inside(p) {
                let rot = unit(seed ^ (j as u64 + 1) * 0x9e37) * std::f64::consts::TAU;
                out.push((p, rot as f32, scale as f32));
            }
        }
    };
    // A group of `n` along `t` from `at` (a step of about 8 ft, jittered across by `n_in`), the
    // first the big one when `big`.
    let group = |at: P, t: P, n_in: P, n: usize, big: bool, seed: u64| -> Vec<(P, f64)> {
        (0..n)
            .map(|j| {
                let h = seed.rotate_left(7 * j as u32 + 3);
                let along = 8.0 * j as f64 + 1.5 * (unit(h) - 0.5);
                let across = 3.0 * (unit(h ^ 0x51) - 0.5);
                let scale = if big && j == 0 { 1.0 + 0.1 * unit(h ^ 0x77) } else { 0.65 + 0.12 * unit(h ^ 0x77) };
                (add(at, add(mul(t, along), mul(n_in, across))), scale)
            })
            .collect()
    };

    // Stack yards: at the back of some crofts, in a manor's yard's far corner.
    for p in l.crofts.iter().filter(|p| p.pts.len() >= 4 && hits(&p.pts)) {
        let c = centroid(&p.pts);
        let h = key(c, 1);
        match p.kind {
            PlotKind::Croft => {
                if unit(h) >= 0.45 {
                    continue;
                }
                // (Its front is `pts[0]`-`pts[1]`, its back `pts[3]`-`pts[2]`.)
                let (b0, b1) = (p.pts[3], p.pts[2]);
                let t = unit_v(sub(b1, b0));
                let n_in = unit_v(sub(c, lerp(b0, b1, 0.5)));
                let r = unit(h ^ 0x3);
                let n = if r < 0.4 { 1 } else if r < 0.8 { 2 } else { 3 };
                let room = dist(b0, b1) - 2.0 * EDGE_FT - 8.0 * (n as f64 - 1.0);
                if room <= 0.0 {
                    continue;
                }
                let start = add(add(b0, mul(t, EDGE_FT + 1.0 + room * unit(h ^ 0x5))), mul(n_in, 9.0));
                put(&group(start, t, n_in, n, unit(h ^ 0x9) < 0.5, h), h, &p.pts, &|_| true);
            }
            PlotKind::ManorYard | PlotKind::Farmyard => {
                if unit(h) >= 0.8 {
                    continue;
                }
                // (The corner farthest from the yard's buildings.)
                let homes: Vec<P> = l.buildings.iter().map(|b| centroid(&b.poly)).filter(|&q| contains(&p.pts, q)).collect();
                let far = |q: P| homes.iter().map(|&m| dist(q, m)).fold(f64::MAX, f64::min);
                let Some(&corner) = p.pts.iter().max_by(|a, b| far(**a).total_cmp(&far(**b))) else { continue };
                let n_in = unit_v(sub(c, corner));
                let at = add(corner, mul(n_in, 13.0));
                let t = [-n_in[1], n_in[0]];
                let n = 2 + (unit(h ^ 0x3) < 0.5) as usize;
                put(&group(sub(at, mul(t, 4.0 * (n as f64 - 1.0))), t, n_in, n, true, h), h, &p.pts, &|_| true);
            }
            _ => {}
        }
    }

    // Hay by a pasture's or a fallow's gate.
    let ways: Vec<(P, P, f64)> = l
        .roads
        .iter()
        .flat_map(|(pts, _, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w)))
        .chain(l.streets.iter().flat_map(|st| st.windows(2).map(|q| (q[0], q[1], 0.5 * LANE_W))))
        .collect();
    for (i, f) in l.fields.iter().enumerate() {
        let Some(info) = l.field_info.get(i) else { continue };
        if !matches!(info.kind, FieldUse::Pasture | FieldUse::Fallow) || f.len() < 3 || !hits(f) {
            continue;
        }
        let c = centroid(f);
        let h = key(c, 2);
        if unit(h) >= 0.55 {
            continue;
        }
        // The outline's nearest point to a lane or a road (every 4 ft along it), else to a farmstead.
        let n = f.len();
        let fb = f.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        let by = |a: P, b: P, m: f64| a[0].min(b[0]) - m <= fb[2] && fb[0] <= a[0].max(b[0]) + m && a[1].min(b[1]) - m <= fb[3] && fb[1] <= a[1].max(b[1]) + m;
        let mine: Vec<(P, P, f64)> = ways.iter().copied().filter(|&(a, b, half)| by(a, b, half + GATE_WAY_FT)).collect();
        let homes: Vec<&Vec<P>> = l.crofts.iter().map(|p| &p.pts).chain(l.buildings.iter().map(|b| &b.poly)).filter(|p| p.iter().any(|q| by(*q, *q, FARMSTEAD_FT))).collect();
        let mut gate: Option<(f64, P, usize)> = None;
        let mut home: Option<(f64, P, usize)> = None;
        for k in 0..n {
            let (a, b) = (f[k], f[(k + 1) % n]);
            let m = (dist(a, b) / 4.0).ceil().max(1.0) as usize;
            for j in 0..=m {
                let p = lerp(a, b, j as f64 / m as f64);
                for &(s, e, half) in &mine {
                    let d = seg_dist(p, s, e) - half;
                    if d < GATE_WAY_FT && gate.is_none_or(|g| d < g.0) {
                        gate = Some((d, p, k));
                    }
                }
                for poly in &homes {
                    let d = if contains(poly, p) { 0.0 } else { outline_dist(poly, p) };
                    if d < FARMSTEAD_FT && home.is_none_or(|g| d < g.0) {
                        home = Some((d, p, k));
                    }
                }
            }
        }
        let Some((_, g, k)) = gate.or(home) else { continue };
        let t = unit_v(sub(f[(k + 1) % n], f[k]));
        let mut n_in = [-t[1], t[0]];
        if !contains(f, add(g, mul(n_in, 2.0))) {
            n_in = [t[1], -t[0]];
        }
        // To one side of the way in (off the gate's gap), 12 ft in.
        let count = 2 + (unit(h ^ 0x3) < 0.4) as usize;
        let side = if unit(h ^ 0x5) < 0.5 { 1.0 } else { -1.0 };
        let t = mul(t, side);
        let at = add(g, add(mul(n_in, 12.0), mul(t, 13.0)));
        let copse = crate::town::rural::copse(l, i);
        let keep = |p: P| copse.is_none_or(|(cc, r)| dist(p, cc) > r + 8.0);
        put(&group(at, t, n_in, count, unit(h ^ 0x9) < 0.6, h), h, f, &keep);
    }
}
