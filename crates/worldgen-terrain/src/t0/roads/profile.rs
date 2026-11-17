//! Road height profiles: fitted grades, meeting points and levelled profiles.

use super::*;

/// Road surface elevation along the path, limited to the class grade: the average of the
/// cut-only envelope (largest grade-limited profile below the terrain) and the fill-only
/// one (smallest above it). Both are exactly grade-limited, so their average is too, and it
/// balances cuttings through bumps against embankments over dips.
pub(super) fn profile(hg: &Plan, pts: &[[f64; 2]], gmax: f64, cell: f64) -> Vec<f32> {
    let terrain: Vec<f64> = pts.iter().map(|p| hg.z(*p)).collect();
    let ft: Vec<[f64; 2]> = pts.iter().map(|p| [p[0] * cell, p[1] * cell]).collect();
    fit_profile(&ft, &terrain, gmax)
}

/// Road surface heights for points (ft) over the given ground heights: as close to the
/// ground as the grade limit allows (the mean of the cut-only and fill-only envelopes).
pub fn fit_profile(pts: &[[f64; 2]], terrain: &[f64], gmax: f64) -> Vec<f32> {
    let lim: Vec<f64> = (0..pts.len())
        .map(|i| if i == 0 { 0.0 } else { gmax * dist(pts[i], pts[i - 1]) })
        .collect();
    let envelope = |sign: f64| {
        // sign = 1: cut-only (min-plus); sign = -1: fill-only (max-minus).
        let mut z: Vec<f64> = terrain.iter().map(|v| v * sign).collect();
        for i in 1..z.len() {
            z[i] = z[i].min(z[i - 1] + lim[i]);
        }
        for i in (1..z.len()).rev() {
            z[i - 1] = z[i - 1].min(z[i] + lim[i]);
        }
        z.into_iter().map(|v| v * sign).collect::<Vec<f64>>()
    };
    let (cut, fill) = (envelope(1.0), envelope(-1.0));
    // f32 rounding must not push a step over the limit: shave a hair off by re-clamping.
    let mut z: Vec<f32> = cut.iter().zip(&fill).map(|(a, b)| (0.5 * (a + b)) as f32).collect();
    for i in 1..z.len() {
        let l = (lim[i] * 0.999) as f32;
        z[i] = z[i].clamp(z[i - 1] - l, z[i - 1] + l);
    }
    z
}

/// Where network roads meet: their shared ends, and where one crosses another (or ends on its
/// middle). Each such place becomes a point of every road there (put in where it falls between
/// two), so their profiles can be made to agree on it (`level_profiles`). Returns per place the
/// roads and their point indices there. `wander` gets a point wherever `pts` does.
pub fn meeting_points(pts: &mut [Vec<[f64; 2]>], wander: &mut [Vec<f32>]) -> Vec<Vec<(usize, usize)>> {
    use crate::core::hash::{FastMap, FastSet};
    // Places (by 1-ft key), each with the roads at it: (road, segment, fraction along it).
    let mut places: Vec<([f64; 2], Vec<(usize, usize, f64)>)> = Vec::new();
    let mut at: FastMap<(i64, i64), usize> = FastMap::default();
    let mut add = |p: [f64; 2], r: usize, k: usize, t: f64| {
        let (kx, ky) = (crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64);
        let found = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (kx + dx, ky + dy))).filter_map(|key| at.get(&key).copied()).find(|&i| dist(places[i].0, p) <= 1.0);
        let i = found.unwrap_or_else(|| {
            places.push((p, Vec::new()));
            at.insert((kx, ky), places.len() - 1);
            places.len() - 1
        });
        places[i].1.push((r, k, t));
    };
    for (r, q) in pts.iter().enumerate() {
        if q.len() >= 2 {
            add(q[0], r, 0, 0.0);
            add(q[q.len() - 1], r, q.len() - 2, 1.0);
        }
    }
    // Crossings: segments binned on a coarse grid, each pair of roads' segments in a bin tried once.
    const BIN: f64 = 2_000.0;
    let mut bins: FastMap<(i64, i64), Vec<(usize, usize)>> = FastMap::default();
    for (r, q) in pts.iter().enumerate() {
        for k in 0..q.len().saturating_sub(1) {
            let (a, b) = (q[k], q[k + 1]);
            let (x0, x1) = (crate::core::floor(a[0].min(b[0]) / BIN) as i64, crate::core::floor(a[0].max(b[0]) / BIN) as i64);
            let (y0, y1) = (crate::core::floor(a[1].min(b[1]) / BIN) as i64, crate::core::floor(a[1].max(b[1]) / BIN) as i64);
            for by in y0..=y1 {
                for bx in x0..=x1 {
                    bins.entry((bx, by)).or_default().push((r, k));
                }
            }
        }
    }
    let mut keys: Vec<(i64, i64)> = bins.keys().copied().collect();
    keys.sort_unstable();
    let mut tried: FastSet<(usize, usize, usize, usize)> = FastSet::default();
    for key in keys {
        let list = &bins[&key];
        for (n, &(r1, k1)) in list.iter().enumerate() {
            for &(r2, k2) in &list[n + 1..] {
                if r1 == r2 || !tried.insert((r1, k1, r2, k2)) {
                    continue;
                }
                let (a, b, c, d) = (pts[r1][k1], pts[r1][k1 + 1], pts[r2][k2], pts[r2][k2 + 1]);
                let (e, f) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
                let den = e[0] * f[1] - e[1] * f[0];
                if den.abs() < 1e-9 {
                    continue;
                }
                let t = ((c[0] - a[0]) * f[1] - (c[1] - a[1]) * f[0]) / den;
                let u = ((c[0] - a[0]) * e[1] - (c[1] - a[1]) * e[0]) / den;
                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    let p = [a[0] + e[0] * t, a[1] + e[1] * t];
                    add(p, r1, k1, t);
                    add(p, r2, k2, u);
                }
            }
        }
    }
    // Each place on each road there: an existing point within a foot, else a new one.
    let mut cuts: Vec<Vec<(usize, f64, usize)>> = vec![Vec::new(); pts.len()];
    let mut meets: Vec<Vec<(usize, usize)>> = Vec::new();
    for (_, list) in &places {
        let roads: FastSet<usize> = list.iter().map(|e| e.0).collect();
        if roads.len() < 2 {
            continue;
        }
        let id = meets.len();
        meets.push(Vec::new());
        for &(r, k, t) in list {
            cuts[r].push((k, t, id));
        }
    }
    let point: Vec<[f64; 2]> = places.iter().filter(|(_, l)| l.iter().map(|e| e.0).collect::<FastSet<usize>>().len() >= 2).map(|(p, _)| *p).collect();
    for r in 0..pts.len() {
        if cuts[r].is_empty() {
            continue;
        }
        cuts[r].sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        let old = std::mem::take(&mut pts[r]);
        let oldw = std::mem::take(&mut wander[r]);
        let (mut np, mut nw) = (Vec::with_capacity(old.len() + cuts[r].len()), Vec::with_capacity(old.len() + cuts[r].len()));
        // Old point index → new; places on an old point are resolved once all are in.
        let mut map = vec![0usize; old.len()];
        let mut on_point: Vec<(usize, usize)> = Vec::new();
        let mut ci = 0;
        for k in 0..old.len() {
            map[k] = np.len();
            np.push(old[k]);
            nw.push(oldw[k]);
            while ci < cuts[r].len() && cuts[r][ci].0 == k {
                let (_, t, id) = cuts[r][ci];
                ci += 1;
                let next = (k + 1).min(old.len() - 1);
                let len = dist(old[k], old[next]);
                if t * len < 1.0 {
                    on_point.push((k, id));
                } else if (1.0 - t) * len < 1.0 {
                    on_point.push((next, id));
                } else {
                    let q = point[id];
                    if dist(*np.last().unwrap(), q) >= 1.0 {
                        np.push(q);
                        nw.push(oldw[k] + (oldw[next] - oldw[k]) * t as f32);
                    }
                    meets[id].push((r, np.len() - 1));
                }
            }
        }
        for (k, id) in on_point {
            meets[id].push((r, map[k]));
        }
        pts[r] = np;
        wander[r] = nw;
    }
    for m in &mut meets {
        m.sort_unstable();
        m.dedup();
    }
    meets.retain(|m| m.len() >= 2);
    meets
}

/// Profiles for the whole network: each road fitted to `terrain` within its grade
/// (`fit_profile`), never below `floor` (bridge decks), and every road at a meeting place
/// (`meeting_points`) at one level there: the ground, or the greatest road's own level where it
/// stands off the ground (the lesser ones ramp to it); a road whose route is too steep to reach
/// that from the places it already meets comes as near as it can. The greater roads' places
/// are levelled first. Bounds that fall away from each level and floor at the grade limit keep
/// each fit within its grade.
pub fn level_profiles(pts: &[Vec<[f64; 2]>], class: &[RoadClass], terrain: &[Vec<f64>], floor: &[Vec<f64>], meets: &[Vec<(usize, usize)>]) -> Vec<Vec<f32>> {
    let n = pts.len();
    let s: Vec<Vec<f64>> = pts
        .iter()
        .map(|q| {
            let mut acc = 0.0;
            let mut v = vec![0.0; q.len()];
            for i in 1..q.len() {
                acc += dist(q[i - 1], q[i]);
                v[i] = acc;
            }
            v
        })
        .collect();
    let g: Vec<f64> = class.iter().map(|c| 0.999 * c.max_grade()).collect();
    // Each road's floor, ramping down from every deck at the grade limit.
    let ramp: Vec<Vec<f64>> = (0..n)
        .map(|r| {
            let mut v = floor[r].clone();
            for i in 1..v.len() {
                v[i] = v[i].max(v[i - 1] - g[r] * (s[r][i] - s[r][i - 1]));
            }
            for i in (1..v.len()).rev() {
                v[i - 1] = v[i - 1].max(v[i] - g[r] * (s[r][i] - s[r][i - 1]));
            }
            v
        })
        .collect();
    // Each road's own fit, as it would stand with no one to meet.
    let free: Vec<Vec<f32>> = (0..n).map(|r| fit_profile(&pts[r], &terrain[r], class[r].max_grade())).collect();
    let mut pins: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let band = |pins: &[(usize, f64)], r: usize, i: usize| {
        let (mut lo, mut hi) = (ramp[r][i], f64::INFINITY);
        for &(j, z) in pins {
            let d = g[r] * (s[r][i] - s[r][j]).abs();
            lo = lo.max(z - d);
            hi = hi.min(z + d);
        }
        (lo, hi)
    };
    // The greater roads' places first, then west to east (a stable order).
    let mut order: Vec<usize> = (0..meets.len()).collect();
    let key = |m: &Vec<(usize, usize)>| (m.iter().map(|&(r, _)| class[r]).min().unwrap_or(RoadClass::Track), pts[m[0].0][m[0].1]);
    order.sort_by(|&a, &b| {
        let (ka, kb) = (key(&meets[a]), key(&meets[b]));
        ka.0.cmp(&kb.0).then(ka.1[0].total_cmp(&kb.1[0])).then(ka.1[1].total_cmp(&kb.1[1]))
    });
    for m in order {
        let (r0, i0) = meets[m][0];
        // The ground, unless the greatest roads there would all stand off it (on an embankment
        // up a slope too steep for them, say): then the nearest of their levels, and the lesser
        // roads ramp to it. Each road comes as near that as its grade allows from the places it
        // already meets (one whose route is too steep keeps the step to itself).
        let best = meets[m].iter().map(|&(r, _)| class[r]).min().unwrap_or(RoadClass::Track);
        let (mut a, mut b) = (f64::INFINITY, f64::MIN);
        for &(r, i) in meets[m].iter().filter(|&&(r, _)| class[r] == best) {
            let (lo, hi) = band(&pins[r], r, i);
            let v = (free[r][i] as f64).min(hi).max(lo);
            (a, b) = (a.min(v), b.max(v));
        }
        let level = terrain[r0][i0].max(a).min(b);
        for &(r, i) in &meets[m] {
            let (lo, hi) = band(&pins[r], r, i);
            pins[r].push((i, level.min(hi).max(lo)));
        }
    }
    // Each road's final profile, the greater roads first: a lesser road running alongside one
    // already done near where they meet (a Y's converging legs) keeps to its level there, so
    // no strip between them is cut or banked.
    const ALONG_FT: f64 = 600.0;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&r| (class[r], r));
    let mut done: Vec<Option<Vec<f32>>> = vec![None; n];
    let at_road = |q: usize, near: usize, p: [f64; 2], zq: &[f32]| -> Option<(f64, f64)> {
        // Nearest point of road q within ALONG_FT (along it) of its point `near`: (distance, level).
        let mut best: Option<(f64, f64)> = None;
        let k0 = s[q].partition_point(|&v| v < s[q][near] - 1.5 * ALONG_FT).saturating_sub(1);
        let k1 = s[q].partition_point(|&v| v <= s[q][near] + 1.5 * ALONG_FT).min(pts[q].len() - 1);
        for k in k0..k1 {
            let (a, b) = (pts[q][k], pts[q][k + 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
            let d = dist(p, [a[0] + dx * t, a[1] + dy * t]);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, zq[k] as f64 + (zq[k + 1] - zq[k]) as f64 * t));
            }
        }
        best
    };
    for &r in &order {
        for m in meets.iter().filter(|m| m.iter().any(|&(q, _)| q == r)) {
            let i = m.iter().find(|&&(q, _)| q == r).map(|e| e.1).unwrap_or(0);
            for &(q, j) in m.iter().filter(|&&(q, _)| q != r) {
                let Some(zq) = done[q].as_deref() else { continue };
                let half = 0.5 * (class[r].width_ft() + class[q].width_ft());
                for dir in [-1i64, 1] {
                    let mut k = i as i64 + dir;
                    while k >= 0 && (k as usize) < pts[r].len() && (s[r][k as usize] - s[r][i]).abs() <= ALONG_FT {
                        let ku = k as usize;
                        let (lo, hi) = band(&pins[r], r, ku);
                        let own = (free[r][ku] as f64).min(hi).max(lo);
                        match at_road(q, j, pts[r][ku], zq) {
                            // Too close for a bank between them (beds' sides fall 1 in 2).
                            Some((d, z)) if d - half <= 2.0 * (own - z).abs() + 10.0 => {
                                pins[r].push((ku, z.min(hi).max(lo)));
                            }
                            _ => break,
                        }
                        k += dir;
                    }
                }
            }
        }
        let fit = &free[r];
        let m = pts[r].len();
        let mut lo = ramp[r].clone();
        let mut hi = vec![f64::INFINITY; m];
        for &(i, z) in &pins[r] {
            lo[i] = lo[i].max(z);
            hi[i] = hi[i].min(z);
        }
        for i in 1..m {
            let l = g[r] * (s[r][i] - s[r][i - 1]);
            lo[i] = lo[i].max(lo[i - 1] - l);
            hi[i] = hi[i].min(hi[i - 1] + l);
        }
        for i in (1..m).rev() {
            let l = g[r] * (s[r][i] - s[r][i - 1]);
            lo[i - 1] = lo[i - 1].max(lo[i] - l);
            hi[i - 1] = hi[i - 1].min(hi[i] + l);
        }
        let mut z: Vec<f32> = (0..m).map(|i| (fit[i] as f64).min(hi[i]).max(lo[i]) as f32).collect();
        // f32 rounding must not push a step over the limit.
        for i in 1..m {
            let l = (g[r] * (s[r][i] - s[r][i - 1]) * 0.999) as f32;
            z[i] = z[i].clamp(z[i - 1] - l, z[i - 1] + l);
        }
        done[r] = Some(z);
    }
    done.into_iter().map(|z| z.unwrap_or_default()).collect()
}
