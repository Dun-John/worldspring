//! Shaping a routed line: meanders, terrain following, simplifying, switchbacks,
//! rounded corners and fine routes in a corridor.

use super::*;

pub(super) fn passable_fn<'a>(inp: &'a Inputs) -> impl Fn(usize) -> bool + 'a {
    move |k| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE
}

/// Sways a realm village's spur (points in cells) sideways by up to about a third of a mile in waves a few
/// miles long, none at its ends (it leaves the village and joins the track as it did), none where the
/// ground there is wet or on a switchback leg. Short spurs sway less.
pub(super) fn meander_spur(pts: &mut Vec<[f64; 2]>, wander: &mut Vec<f32>, seed: u64, scale: f64, dry: &dyn Fn(i64, i64) -> bool) {
    const STEP: f64 = 0.08;
    const TAPER: f64 = 1.0;
    if pts.len() < 2 {
        return;
    }
    let (mut rp, mut rw): (Vec<[f64; 2]>, Vec<f32>) = (vec![pts[0]], vec![wander[0]]);
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let n = (dist(a, b) / STEP).ceil().max(1.0) as usize;
        for j in 1..=n {
            let t = j as f64 / n as f64;
            rp.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            rw.push(wander[k - 1] + (wander[k] - wander[k - 1]) * t as f32);
        }
    }
    let n = rp.len();
    if n < 3 {
        return;
    }
    let mut s = vec![0.0; n];
    for i in 1..n {
        s[i] = s[i - 1] + dist(rp[i - 1], rp[i]);
    }
    let total = s[n - 1];
    let size = smoothstep(0.2, 1.5, total);
    let mut out = rp.clone();
    for i in 1..n - 1 {
        let ease = smoothstep(0.0, TAPER, s[i].min(total - s[i]));
        if ease <= 0.0 || rw[i] < 0.5 {
            continue;
        }
        // (Not at a corner of the route: the sideways push would fold it.)
        // Looked at over a few lengths, so a turn just past the nearest one still holds it back.
        let m = rp[i];
        let mut cos = 1.0f64;
        for w in [20usize, 40, 70] {
            let (a, b) = (rp[i.saturating_sub(w)], rp[(i + w).min(n - 1)]);
            let (ux, uy, vx, vy) = (m[0] - a[0], m[1] - a[1], b[0] - m[0], b[1] - m[1]);
            cos = cos.min((ux * vx + uy * vy) / (crate::core::sqrt(ux * ux + uy * uy) * crate::core::sqrt(vx * vx + vy * vy)).max(1e-9));
        }
        let ease = ease * smoothstep(0.9, 0.99, cos);
        if ease <= 0.0 {
            continue;
        }
        let (a, b) = (rp[i.saturating_sub(20)], rp[(i + 20).min(n - 1)]);
        let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
        let tl = crate::core::sqrt(tx * tx + ty * ty).max(1e-9);
        let off = ease * size * scale * 0.6 * gradient2(seed, s[i] / 2.5, 0.37);
        for f in [1.0, 0.5] {
            let q = [rp[i][0] - ty / tl * off * f, rp[i][1] + tx / tl * off * f];
            if dry(crate::core::round(q[0]) as i64, crate::core::round(q[1]) as i64) {
                out[i] = q;
                break;
            }
        }
    }
    // Two light passes over the swayed stretches (1-2-1), so no step or corner is sharper than the road's.
    for _ in 0..2 {
        let prev = out.clone();
        for i in 1..n - 1 {
            if prev[i] == rp[i] && prev[i - 1] == rp[i - 1] && prev[i + 1] == rp[i + 1] {
                continue;
            }
            let q = [0.25 * prev[i - 1][0] + 0.5 * prev[i][0] + 0.25 * prev[i + 1][0], 0.25 * prev[i - 1][1] + 0.5 * prev[i][1] + 0.25 * prev[i + 1][1]];
            if dry(crate::core::round(q[0]) as i64, crate::core::round(q[1]) as i64) {
                out[i] = q;
            }
        }
    }
    *pts = out;
    *wander = rw;
}

/// Gives a road (points in cells, with their wander weights) the bends of a real one, baked
/// into its points so its profile and bed follow them: resampled every `STEP` cells, each
/// point is first offset sideways by gentle noise bends (wavelengths of about 3,700 and
/// 1,400 ft, straighter for a king's road, more winding for a track), then slides (up to
/// `REACH` cells) to ease the grades either side of it, kept smooth and near its bend: on a
/// hillside the road curves along the slope. Switchback legs (wander 0), the ends and water
/// stay as they are; wander is then 0 where the road bends here (the noise wander added
/// when it is drawn would ignore its profile and cut the hillside).
pub(super) fn follow_terrain(pts: &mut Vec<[f64; 2]>, wander: &mut Vec<f32>, hgrid: &Plan, class: RoadClass, cell: f64, seed: u64, dry: &dyn Fn(i64, i64) -> bool) {
    const STEP: f64 = 0.05;
    const REACH: f64 = 0.35;
    if pts.len() < 2 {
        return;
    }
    let gmax = class.max_grade();
    // Resample every `STEP` (switchback legs too: they stay where they are, but the profile
    // needs points along them to follow the ground).
    let (mut rp, mut rw): (Vec<[f64; 2]>, Vec<f32>) = (vec![pts[0]], vec![wander[0]]);
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let n = (dist(a, b) / STEP).ceil().max(1.0) as usize;
        for j in 1..=n {
            let t = j as f64 / n as f64;
            rp.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            rw.push(wander[k - 1] + (wander[k] - wander[k - 1]) * t as f32);
        }
    }
    let n = rp.len();
    if n < 3 {
        return;
    }
    let mut s = vec![0.0; n];
    for i in 1..n {
        s[i] = s[i - 1] + dist(rp[i - 1], rp[i]);
    }
    // Distance along the road to the nearest point that stays (an end or a switchback leg):
    // bends and sliding fade in over the first `EASE` cells from it.
    const EASE: f64 = 0.3;
    let stays = |i: usize| i == 0 || i == n - 1 || rw[i] < 0.5;
    let mut fixed = vec![f64::MAX; n];
    let mut last = f64::MIN;
    for i in 0..n {
        if stays(i) {
            last = s[i];
        }
        fixed[i] = s[i] - last;
    }
    let mut next = f64::MAX;
    for i in (0..n).rev() {
        if stays(i) {
            next = s[i];
        }
        fixed[i] = fixed[i].min(next - s[i]);
    }
    let ease: Vec<f64> = fixed.iter().map(|f| smoothstep(0.0, EASE, *f)).collect();
    // Bends: noise offsets along the line's normal.
    let amp = [0.75, 1.0, 1.3][class as usize];
    let normal = |i: usize| {
        let (a, b) = (rp[i.saturating_sub(1)], rp[(i + 1).min(n - 1)]);
        let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
        let tl = crate::core::sqrt(tx * tx + ty * ty).max(1e-9);
        [-ty / tl, tx / tl]
    };
    let mut orig = rp.clone();
    for i in 1..n - 1 {
        if ease[i] <= 0.0 {
            continue;
        }
        let off = ease[i] * amp * (0.045 * gradient2(seed, s[i] / 0.6, 0.31) + 0.012 * gradient2(seed ^ 0xb3, s[i] / 0.22, 0.77));
        let nrm = normal(i);
        for f in [1.0, 0.5] {
            let q = [rp[i][0] + nrm[0] * off * f, rp[i][1] + nrm[1] * off * f];
            if dry(crate::core::round(q[0]) as i64, crate::core::round(q[1]) as i64) {
                orig[i] = q;
                break;
            }
        }
    }
    let mut rp = orig.clone();
    let room: Vec<f64> = ease.iter().map(|e| REACH * e).collect();
    let hz = |p: [f64; 2]| hgrid.z(p);
    let mut z: Vec<f64> = rp.iter().map(|p| hz(*p)).collect();
    let grade = |za: f64, zb: f64, a: [f64; 2], b: [f64; 2]| (zb - za) / (dist(a, b) * cell).max(1.0);
    let cost = |g: f64| 100.0 * g * g + if g.abs() > gmax { 1_000.0 * (g.abs() - gmax) * (g.abs() - gmax) } else { 0.0 };
    for d in [0.08, 0.05, 0.03, 0.02, 0.01, 0.01] {
        for i in 1..n - 1 {
            if room[i] <= 0.0 {
                continue;
            }
            let (a, b) = (rp[i - 1], rp[i + 1]);
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let tl = crate::core::sqrt(tx * tx + ty * ty).max(1e-9);
            let nrm = [-ty / tl, tx / tl];
            let energy = |q: [f64; 2], zq: f64| {
                let bend = [a[0] - 2.0 * q[0] + b[0], a[1] - 2.0 * q[1] + b[1]];
                let off = dist(q, orig[i]) / REACH;
                cost(grade(z[i - 1], zq, a, q)) + cost(grade(zq, z[i + 1], q, b)) + (bend[0] * bend[0] + bend[1] * bend[1]) / (STEP * STEP) + 0.2 * off * off
            };
            let mut best = (energy(rp[i], z[i]), rp[i], z[i]);
            for sign in [-1.0, 1.0] {
                let q = [rp[i][0] + nrm[0] * d * sign, rp[i][1] + nrm[1] * d * sign];
                if dist(q, orig[i]) > room[i] || !dry(crate::core::round(q[0]) as i64, crate::core::round(q[1]) as i64) {
                    continue;
                }
                let zq = hz(q);
                let e = energy(q, zq);
                if e < best.0 {
                    best = (e, q, zq);
                }
            }
            (rp[i], z[i]) = (best.1, best.2);
        }
    }
    *pts = rp;
    *wander = rw.iter().zip(&ease).map(|(w, e)| if *e > 0.0 { 0.0 } else { *w }).collect();
}

/// Drop the part of a path inside radius `r` of `c` at its start (cut exactly at the circle).
pub(super) fn trim_start(pts: &mut Vec<[f64; 2]>, wander: &mut Vec<f32>, c: [f64; 2], r: f64) {
    let Some(k) = pts.iter().position(|p| dist(*p, c) >= r) else { return };
    if k == 0 || pts.len() - k < 1 {
        return;
    }
    let (a, b) = (pts[k - 1], pts[k]);
    let (da, db) = (dist(a, c), dist(b, c));
    let t = ((r - da) / (db - da).max(1e-9)).clamp(0.0, 1.0);
    pts[k - 1] = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    pts.drain(..k - 1);
    wander.drain(..k - 1);
    if let Some(w) = wander.first_mut() {
        *w = 0.0;
    }
}

/// Any-angle path from a grid path (string pulling): Douglas-Peucker that accepts a chord
/// when it stays within `tol` cells of the grid path, crosses only passable ground and is no steeper than the
/// class allows (or than the path itself was); chords are then resampled to at most one cell
/// per step so the grade check below sees the terrain between vertices.
pub(super) fn simplify(cells: &[[f64; 2]], tol: f64, w: usize, h: usize, hg: &Plan, gmax: f64, cell: f64, passable: impl Fn(usize) -> bool) -> Vec<[f64; 2]> {
    let steepest = |pts: &mut dyn Iterator<Item = [f64; 2]>| {
        let mut prev: Option<([f64; 2], f64)> = None;
        let mut worst = 0.0f64;
        for p in pts {
            let z = hg.z(p);
            if let Some((q, zq)) = prev {
                worst = worst.max((z - zq).abs() / (dist(p, q) * cell).max(1.0));
            }
            prev = Some((p, z));
        }
        worst
    };
    let clear = |a: [f64; 2], b: [f64; 2], path: &[[f64; 2]]| {
        let n = (dist(a, b) * 4.0).ceil().max(1.0) as usize;
        let sample = |s: usize| {
            let t = s as f64 / n as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        };
        let dry = (0..=n).all(|s| {
            let p = sample(s);
            passable((crate::core::round(p[1]) as usize).min(h - 1) * w + (crate::core::round(p[0]) as usize).min(w - 1))
        });
        dry && {
            let chord = steepest(&mut (0..=n).step_by(2).map(sample));
            chord <= gmax || chord <= steepest(&mut path.iter().copied())
        }
    };
    let mut keep = vec![false; cells.len()];
    keep[0] = true;
    *keep.last_mut().unwrap() = true;
    let mut stack = vec![(0usize, cells.len() - 1)];
    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (a, b) = (cells[i], cells[j]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = crate::core::sqrt(dx * dx + dy * dy).max(1e-9);
        let (mut worst, mut at) = (0.0, i + 1);
        for (k, p) in cells.iter().enumerate().take(j).skip(i + 1) {
            let d = ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / len;
            if d > worst {
                worst = d;
                at = k;
            }
        }
        if worst > tol || !clear(a, b, &cells[i..=j]) {
            keep[at] = true;
            stack.push((i, at));
            stack.push((at, j));
        }
    }
    let kept: Vec<[f64; 2]> = cells.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p).collect();
    let mut out = vec![kept[0]];
    for pair in kept.windows(2) {
        let n = dist(pair[0], pair[1]).ceil().max(1.0) as usize;
        for s in 1..=n {
            let t = s as f64 / n as f64;
            out.push([pair[0][0] + (pair[1][0] - pair[0][0]) * t, pair[0][1] + (pair[1][1] - pair[0][1]) * t]);
        }
    }
    out
}

/// A drawn road's segment (its cells, the ends in place, in cells): along its drawn `line`
/// between where the segment's ends fall on it, wherever that stretch is dry (`dry`, every
/// quarter cell) and the cells keep near it (within 3 cells); else each half of the cells is
/// tried the same way, down to single steps, and the cells left (round water) are `smooth`ed.
/// The line within a cell of either end is left out, so the road eases onto it from a junction
/// off it. Points at most a cell apart.
pub(super) fn along_drawn(cells: &[[f64; 2]], line: &[[f64; 2]], dry: &dyn Fn([f64; 2]) -> bool, smooth: &dyn Fn(&[[f64; 2]]) -> Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    /// From cell `i` to cell `j`: the line drawn between them (its points), or the cells.
    type Piece = (usize, usize, Option<Vec<[f64; 2]>>);
    fn fit(cells: &[[f64; 2]], line: &[[f64; 2]], s: &[f64], dry: &dyn Fn([f64; 2]) -> bool, i: usize, j: usize, out: &mut Vec<Piece>) {
        if j > i + 1 {
            let ((ti, _), (tj, _)) = (project(cells[i], line, s), project(cells[j], line, s));
            let near = cells[i..=j].iter().all(|c| project(*c, line, s).1 <= 3.0);
            let (lo, hi) = (ti.min(tj), ti.max(tj));
            if near && hi - lo > 2.5 {
                let mut part = sub_line(line, s, lo + 1.0, hi - 1.0);
                if ti > tj {
                    part.reverse();
                }
                let mut path = vec![cells[i]];
                path.extend_from_slice(&part);
                path.push(cells[j]);
                let wet = path.windows(2).any(|w| {
                    let n = (dist(w[0], w[1]) * 4.0).ceil().max(1.0) as usize;
                    (1..=n).any(|k| !dry([w[0][0] + (w[1][0] - w[0][0]) * k as f64 / n as f64, w[0][1] + (w[1][1] - w[0][1]) * k as f64 / n as f64]))
                });
                if !wet {
                    out.push((i, j, Some(part)));
                    return;
                }
            }
            let m = (i + j) / 2;
            fit(cells, line, s, dry, i, m, out);
            fit(cells, line, s, dry, m, j, out);
            return;
        }
        // (Runs of cells join up: `along_drawn` smooths them together.)
        match out.last_mut() {
            Some((_, end, None)) if *end == i => *end = j,
            _ => out.push((i, j, None)),
        }
    }
    let s = arc(line);
    let mut pieces = Vec::new();
    fit(cells, line, &s, dry, 0, cells.len() - 1, &mut pieces);
    let mut kept = vec![cells[0]];
    for (i, j, part) in pieces {
        match part {
            Some(part) => {
                kept.extend_from_slice(&part);
                kept.push(cells[j]);
            }
            None => kept.extend_from_slice(&smooth(&cells[i..=j])[1..]),
        }
    }
    let mut out = vec![kept[0]];
    for pair in kept.windows(2) {
        let n = dist(pair[0], pair[1]).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            out.push([pair[0][0] + (pair[1][0] - pair[0][0]) * t, pair[0][1] + (pair[1][1] - pair[0][1]) * t]);
        }
    }
    out
}

/// Replace too-steep stretches (on the plan between points, or on `ground` over any quarter
/// of one) with grade-limited paths on a 4× grid (T0 cell units in/out), or where that finds
/// none, an 8× grid on `ground` within `dry` (both at a point, in cells).
/// Also returns the wander weight per point (0 on switchback legs, which must stay put).
pub(super) fn switchbacks(
    hg: &Plan,
    cells: &[[f64; 2]],
    gmax: f64,
    cell: f64,
    passable: impl Fn(usize) -> bool,
    ground: &impl Fn([f64; 2]) -> f64,
    dry: &impl Fn([f64; 2]) -> bool,
) -> (Vec<[f64; 2]>, Vec<f32>) {
    const F: f64 = 4.0;
    const F_FINE: f64 = 8.0;
    // Legs are planned this far under the limit: the ground the profile meets is rougher than
    // the plan, and a route at the limit leaves no room (the profile would cut and fill by
    // a hundred feet).
    const SLACK: f64 = 0.85;
    // A route over this many times the straight run is too long.
    const LONG: f64 = 1.5;
    let at = |p: [f64; 2]| hg.z(p);
    // Steeper than the limit over any quarter of the stretch on the ground itself.
    let steep = |a: [f64; 2], b: [f64; 2]| {
        let q: Vec<f64> = (0..=4).map(|k| ground([a[0] + (b[0] - a[0]) * k as f64 / 4.0, a[1] + (b[1] - a[1]) * k as f64 / 4.0])).collect();
        let run = 0.25 * dist(a, b) * cell;
        q.windows(2).any(|w| (w[1] - w[0]).abs() > gmax * run)
    };
    let mut out: Vec<[f64; 2]> = vec![cells[0]];
    let mut wander: Vec<f32> = vec![1.0];
    let mut i = 0;
    while i + 1 < cells.len() {
        let (a, b) = (cells[i], cells[i + 1]);
        let run = crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])) * cell;
        let grade = (at(b) - at(a)).abs() / run;
        if grade <= gmax && !steep(a, b) {
            out.push(b);
            wander.push(1.0);
            i += 1;
            continue;
        }
        // Extend the steep stretch while it stays steep (up to 12 cells).
        let mut j = i + 1;
        while j + 1 < cells.len() && j - i < 12 {
            let (c, e) = (cells[j], cells[j + 1]);
            let r = crate::core::sqrt((e[0] - c[0]) * (e[0] - c[0]) + (e[1] - c[1]) * (e[1] - c[1])) * cell;
            if (at(e) - at(c)).abs() / r <= gmax && !steep(c, e) {
                break;
            }
            j += 1;
        }
        let on_plan = |p: [f64; 2]| hg.z(p);
        let on_cells = |p: [f64; 2]| passable((p[1].round() as usize).min(hg.ch - 1) * hg.cw + (p[0].round() as usize).min(hg.cw - 1));
        let corr = Corridor::new(hg, &on_plan, &on_cells, cells[i], cells[j], F);
        let mut route = fine_route_in(&corr, cells[i], cells[j], SLACK * gmax, &[1.0, 1.8], cell)
            .or_else(|| fine_route(hg, ground, dry, cells[i], cells[j], SLACK * gmax, &[1.0, 1.8], cell, F_FINE));
        // A climb that can only hold the grade by adding miles comes out as one long finger (out,
        // a U-turn, back beside itself) or a route far longer than the straight run. Spend the
        // grade instead: retry with each leg step allowed 1.2 to 1.8 times the planned grade (the
        // profile averages the bumps), taking the first route that has neither fault.
        if let Some(r) = route.clone() {
            let straight = dist(cells[i], cells[j]) * cell;
            let bad = |r: &[[f64; 2]]| route_len(r) * cell > LONG * straight || route_finger(r, cell);
            if bad(&r) {
                // (The profile can only climb its limit over the length: a route too short for
                // the climb would be cut and filled.)
                let climb = (at(cells[i]) - at(cells[j])).abs();
                // (Of those left, the one without a finger first, then the shortest.)
                let mut best: Option<((bool, f64), Vec<[f64; 2]>)> = None;
                for m in [1.2, 1.4, 1.6, 1.8] {
                    let Some(r2) = fine_route_in(&corr, cells[i], cells[j], SLACK * gmax, &[m], cell) else { continue };
                    let len = route_len(&r2);
                    if climb > 0.97 * gmax * len * cell {
                        continue;
                    }
                    if !bad(&r2) {
                        route = Some(r2);
                        best = None;
                        break;
                    }
                    let key = (route_finger(&r2, cell), len);
                    if best.as_ref().is_none_or(|b| key < b.0) {
                        best = Some((key, r2));
                    }
                }
                // None as good as wanted: one that is not a finger where the route is one (however
                // long), else the shortest, if it saves a good part of the length.
                if let Some(((finger, len), r2)) = best {
                    if (route_finger(&r, cell) && !finger) || len < 0.8 * route_len(&r) {
                        route = Some(r2);
                    }
                }
            }
        }
        // A finger that gains no height is cut across (the chord must be passable and keep to the
        // stepped grade); the climb that needs its length stays.
        if let Some(r) = route.as_mut() {
            for _ in 0..4 {
                let Some((fi, fj)) = finger_span(r, cell) else { break };
                let (p, q) = (r[fi], r[fj]);
                let len = dist(p, q) * cell;
                let n = (dist(p, q) * 4.0).ceil().max(1.0) as usize;
                if (at(p) - at(q)).abs() > 1.8 * SLACK * gmax * len || !(0..=n).all(|k| on_cells([p[0] + (q[0] - p[0]) * k as f64 / n as f64, p[1] + (q[1] - p[1]) * k as f64 / n as f64])) {
                    break;
                }
                r.drain(fi + 1..fj);
            }
        }
        match route {
            Some(route) => {
                // Drop collinear fine-grid points; keep the legs and hairpins.
                let mut legs: Vec<[f64; 2]> = vec![route[0]];
                for k in 1..route.len() - 1 {
                    let (p, q, r) = (legs[legs.len() - 1], route[k], route[k + 1]);
                    if ((q[0] - p[0]) * (r[1] - q[1]) - (q[1] - p[1]) * (r[0] - q[0])).abs() > 1e-9 {
                        legs.push(q);
                    }
                }
                legs.push(route[route.len() - 1]);
                if let Some(w) = wander.last_mut() {
                    *w = 0.0;
                }
                out.extend_from_slice(&legs[1..]);
                wander.extend(std::iter::repeat_n(0.0, legs.len() - 1));
            }
            None => {
                out.extend_from_slice(&cells[i + 1..=j]);
                wander.extend(std::iter::repeat_n(1.0, j - i));
            }
        }
        i = j;
    }
    // Corners (hairpins, and where a switchback stretch meets the road) turn on arcs; within a
    // stretch on wider ones, as far as its legs allow (a kink between legs reads as a bend).
    round_corners_by(&out, &wander, &|k| if wander[k] == 0.0 && k + 1 < wander.len() && wander[k + 1] == 0.0 { 0.6 / F } else { 0.06 }, 0.05)
}

/// A polyline (cells) with each corner turning more than 15° rounded by a circular arc
/// tangent to both sides (radius up to `r_max`, smaller where the sides are short or meet at
/// a narrow angle: a hairpin becomes a tight U-turn), and points at most `step` apart; `vals`
/// (one per point) are carried along (interpolated on lines, the corner's on its arc).
pub fn round_corners(pts: &[[f64; 2]], vals: &[f32], r_max: f64, step: f64) -> (Vec<[f64; 2]>, Vec<f32>) {
    round_corners_by(pts, vals, &|_| r_max, step)
}

/// `round_corners` with the largest radius given per corner (by point index).
fn round_corners_by(pts: &[[f64; 2]], vals: &[f32], r_max: &dyn Fn(usize) -> f64, step: f64) -> (Vec<[f64; 2]>, Vec<f32>) {
    let n = pts.len();
    if n < 3 {
        return (pts.to_vec(), vals.to_vec());
    }
    // Per corner: (distance cut back along each side, arc centre, radius); None: kept sharp.
    let fillet: Vec<Option<(f64, [f64; 2], f64)>> = (0..n)
        .map(|k| {
            if k == 0 || k == n - 1 {
                return None;
            }
            let (a, b) = (unit2([pts[k - 1][0] - pts[k][0], pts[k - 1][1] - pts[k][1]]), unit2([pts[k + 1][0] - pts[k][0], pts[k + 1][1] - pts[k][1]]));
            // Interior angle between the sides; a turn under 15° stays.
            let alpha = libm::acos(dot2(a, b).clamp(-1.0, 1.0));
            if alpha > 165f64.to_radians() || alpha < 1e-3 {
                return None;
            }
            let tan_half = libm::tan(0.5 * alpha);
            let t_max = 0.45 * dist(pts[k - 1], pts[k]).min(dist(pts[k], pts[k + 1]));
            let r = r_max(k).min(t_max * tan_half);
            let t = r / tan_half;
            let bis = unit2([a[0] + b[0], a[1] + b[1]]);
            let c = r / libm::sin(0.5 * alpha);
            Some((t, [pts[k][0] + bis[0] * c, pts[k][1] + bis[1] * c], r))
        })
        .collect();
    let mut out: Vec<[f64; 2]> = vec![pts[0]];
    let mut ov: Vec<f32> = vec![vals[0]];
    let line = |out: &mut Vec<[f64; 2]>, ov: &mut Vec<f32>, b: [f64; 2], vb: f32| {
        let (a, va) = (*out.last().unwrap(), *ov.last().unwrap());
        let m = (dist(a, b) / step).ceil().max(1.0) as usize;
        out.extend((1..=m).map(|i| [a[0] + (b[0] - a[0]) * i as f64 / m as f64, a[1] + (b[1] - a[1]) * i as f64 / m as f64]));
        ov.extend((1..=m).map(|i| va + (vb - va) * i as f32 / m as f32));
    };
    // Points closer than this to the last are left out (no slivers of segments).
    let min_gap = 0.05 * step;
    let toward = |from: [f64; 2], to: [f64; 2], d: f64| {
        let u = unit2([to[0] - from[0], to[1] - from[1]]);
        [from[0] + u[0] * d, from[1] + u[1] * d]
    };
    for k in 1..n {
        let Some((t, c, r)) = fillet[k] else {
            line(&mut out, &mut ov, pts[k], vals[k]);
            continue;
        };
        let (p0, p3) = (toward(pts[k], pts[k - 1], t), toward(pts[k], pts[k + 1], t));
        if dist(p0, *out.last().unwrap()) >= min_gap {
            line(&mut out, &mut ov, p0, vals[k]);
        }
        // Round the arc from p0 to p3 about c, the short way.
        let (a0, a1) = (libm::atan2(p0[1] - c[1], p0[0] - c[0]), libm::atan2(p3[1] - c[1], p3[0] - c[0]));
        let mut sweep = a1 - a0;
        while sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        }
        while sweep < -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }
        // At least a point every 15° (a tight hairpin is still round) and every `step`.
        let m = ((sweep.abs() * r) / step).max(sweep.abs() / 15f64.to_radians()).ceil().clamp(2.0, 60.0) as usize;
        for i in 1..=m {
            let a = a0 + sweep * i as f64 / m as f64;
            let q = [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)];
            if dist(q, *out.last().unwrap()) >= min_gap {
                out.push(q);
                ov.push(vals[k]);
            }
        }
    }
    (out, ov)
}

/// Length of a polyline (cells).
fn route_len(r: &[[f64; 2]]) -> f64 {
    r.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Whether a route (cells) runs out and comes back beside itself: two points over 1.5 mi apart
/// along it but within 0.6 mi of each other, with a leg between them longer than 0.6 mi
/// (compact switchback stacks have short legs between their hairpins).
fn route_finger(r: &[[f64; 2]], cell: f64) -> bool {
    finger_span(r, cell).is_some()
}

/// The first finger of a route (`route_finger`) as the indices of the point it leaves the route
/// at and the one it comes back to.
fn finger_span(r: &[[f64; 2]], cell: f64) -> Option<(usize, usize)> {
    const MI: f64 = 5280.0;
    let mut s: Vec<f64> = vec![0.0];
    for w in r.windows(2) {
        s.push(s[s.len() - 1] + dist(w[0], w[1]) * cell);
    }
    if s[s.len() - 1] < 1.5 * MI {
        return None;
    }
    // Hairpin-like corners (turning over 100 degrees across the nodes 250 ft either side).
    let at = |d: f64| {
        let k = s.partition_point(|&v| v < d).min(r.len() - 1);
        r[k]
    };
    let corners: Vec<f64> = (1..r.len() - 1)
        .filter(|&k| {
            let (p, a, b) = (r[k], at(s[k] - 500.0), at(s[k] + 500.0));
            let (u, v) = (sub2(p, a), sub2(b, p));
            let (lu, lv) = (crate::core::sqrt(dot2(u, u)), crate::core::sqrt(dot2(v, v)));
            lu > 0.0 && lv > 0.0 && dot2(u, v) < -0.17 * lu * lv
        })
        .map(|k| s[k])
        .collect();
    for i in 0..r.len() {
        for j in i + 1..r.len() {
            if s[j] - s[i] >= 1.5 * MI && dist(r[i], r[j]) * cell <= 0.6 * MI {
                let mut marks = vec![s[i]];
                marks.extend(corners.iter().copied().filter(|&c| c > s[i] && c < s[j]));
                marks.push(s[j]);
                if marks.windows(2).any(|w| w[1] - w[0] > 0.6 * MI) {
                    return Some((i, j));
                }
                break;
            }
        }
    }
    None
}

/// A* on a grid `f` times finer than T0 inside a corridor, over the `ground` heights at its
/// nodes where `ok` (both at a point in cells); each step's grade must be ≤ gmax (then
/// ≤ 1.8 gmax as a fallback). Returns points in T0 cell units.
#[allow(clippy::too_many_arguments)]
fn fine_route(hg: &Plan, ground: &impl Fn([f64; 2]) -> f64, ok: &impl Fn([f64; 2]) -> bool, a: [f64; 2], b: [f64; 2], gmax: f64, mults: &[f64], cell: f64, f: f64) -> Option<Vec<[f64; 2]>> {
    let corr = Corridor::new(hg, ground, ok, a, b, f);
    fine_route_in(&corr, a, b, gmax, mults, cell)
}

/// The fine grid a route between two points is searched on: heights and passability, which
/// do not depend on the grade limits tried over it.
struct Corridor {
    x0: f64,
    y0: f64,
    fw: usize,
    fh: usize,
    f: f64,
    hts: Vec<f64>,
    ok: Vec<bool>,
}

impl Corridor {
    fn new(hg: &Plan, ground: &impl Fn([f64; 2]) -> f64, ok: &impl Fn([f64; 2]) -> bool, a: [f64; 2], b: [f64; 2], f: f64) -> Corridor {
        let pad = 4.0;
        let (x0, y0) = ((a[0].min(b[0]) - pad).max(0.0), (a[1].min(b[1]) - pad).max(0.0));
        // (On whole cells, as the junctions are: an end at a settlement's own spot is put back on it
        // after, so roads still meet at one point.)
        let (x0, y0) = (crate::core::floor(x0), crate::core::floor(y0));
        let (x1, y1) = ((a[0].max(b[0]) + pad).min((hg.cw - 1) as f64), (a[1].max(b[1]) + pad).min((hg.ch - 1) as f64));
        let (fw, fh) = (((x1 - x0) * f) as usize + 1, ((y1 - y0) * f) as usize + 1);
        let to_world = |i: usize| [x0 + (i % fw) as f64 / f, y0 + (i / fw) as f64 / f];
        let hts: Vec<f64> = (0..fw * fh).map(|i| ground(to_world(i))).collect();
        let ok: Vec<bool> = (0..fw * fh).map(|i| ok(to_world(i))).collect();
        Corridor { x0, y0, fw, fh, f, hts, ok }
    }
}

fn fine_route_in(corr: &Corridor, a: [f64; 2], b: [f64; 2], gmax: f64, mults: &[f64], cell: f64) -> Option<Vec<[f64; 2]>> {
    let Corridor { x0, y0, fw, fh, f, ref hts, ref ok } = *corr;
    let to_world = |i: usize| [x0 + (i % fw) as f64 / f, y0 + (i / fw) as f64 / f];
    let idx = |p: [f64; 2]| (((p[1] - y0) * f).round() as usize).min(fh - 1) * fw + (((p[0] - x0) * f).round() as usize).min(fw - 1);
    let (s, t) = (idx(a), idx(b));
    let step = cell / f;
    const DIRS: [(i32, i32); 16] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)];
    // What turning from one heading to another costs (grid steps): enough that the route climbs
    // in long legs rather than zigzagging every step or two. (The states are a node and the
    // heading it was reached by, so each turn is costed as made.)
    const TURN: f64 = 2.5;
    let turn_cost: Vec<f64> = (0..256)
        .map(|k| {
            let ((ax, ay), (bx, by)) = (DIRS[k / 16], DIRS[k % 16]);
            TURN * (1.0 - (ax * bx + ay * by) as f64 / crate::core::sqrt(((ax * ax + ay * ay) * (bx * bx + by * by)) as f64))
        })
        .collect();
    for limit in mults.iter().map(|m| gmax * m) {
        let mut g = vec![f64::INFINITY; fw * fh * 16];
        let mut came = vec![u32::MAX; fw * fh * 16];
        let mut done = vec![false; fw * fh * 16];
        let mut heap = BinaryHeap::new();
        let (tx, ty) = ((t % fw) as f64, (t / fw) as f64);
        // (The start leaves in any heading.)
        let start = u32::MAX - 1;
        heap.push(Node { f: 0.0, i: start });
        while let Some(Node { i: st, .. }) = heap.pop() {
            let (i, d0) = if st == start { (s, usize::MAX) } else { (st as usize / 16, st as usize % 16) };
            if st != start && i == t {
                let mut path = vec![t];
                let mut cur = st as usize;
                while came[cur] != start {
                    cur = came[cur] as usize;
                    path.push(cur / 16);
                }
                path.push(s);
                path.reverse();
                let mut out: Vec<[f64; 2]> = path.into_iter().map(to_world).collect();
                let k = out.len() - 1;
                (out[0], out[k]) = (a, b);
                // (Not doubling back from there to the next point.)
                if out.len() > 2 && dot2(sub2(out[1], a), sub2(out[2], out[1])) < 0.0 {
                    out.remove(1);
                }
                let k = out.len() - 1;
                if out.len() > 2 && dot2(sub2(out[k - 1], out[k - 2]), sub2(b, out[k - 1])) < 0.0 {
                    out.remove(k - 1);
                }
                return Some(out);
            }
            let g0 = if st == start { 0.0 } else { g[st as usize] };
            if st != start {
                if done[st as usize] {
                    continue;
                }
                done[st as usize] = true;
            }
            let (ix, iy) = ((i % fw) as i32, (i / fw) as i32);
            for (d, (dx, dy)) in DIRS.into_iter().enumerate() {
                let (nx, ny) = (ix + dx, iy + dy);
                if nx < 0 || ny < 0 || nx >= fw as i32 || ny >= fh as i32 {
                    continue;
                }
                let nb = ny as usize * fw + nx as usize;
                if !ok[nb] {
                    continue;
                }
                let run = crate::core::sqrt((dx * dx + dy * dy) as f64) * step;
                let grade = (hts[nb] - hts[i]).abs() / run;
                if grade > limit {
                    continue;
                }
                // Turning costs; climbing costs a little extra.
                let turn = if d0 == usize::MAX { 0.0 } else { turn_cost[d0 * 16 + d] };
                let (ns, ng) = (nb * 16 + d, g0 + run * (1.0 + 2.0 * grade) + turn * step);
                if ng < g[ns] {
                    g[ns] = ng;
                    came[ns] = st;
                    let hd = dist([nx as f64, ny as f64], [tx, ty]) * step;
                    heap.push(Node { f: ng + hd, i: ns as u32 });
                }
            }
        }
    }
    None
}
