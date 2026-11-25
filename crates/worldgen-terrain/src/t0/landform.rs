//! Landforms from ruggedness (Roadmap 7 item 169): stepped plateaus and mesas in dry and upland
//! country, escarpments (long steps between levels, with a gentle back slope: cuestas), and
//! canyons where rivers cross uplands. Few and low at low ruggedness, many and bold at high.
//!
//! The shapes are made on the T0 grid, so every level and tile inherits them. Plateaus and
//! escarpments shape the ground before hydrology (rivers route over them); canyons are cut
//! after it along the rivers it found, keeping every river cell strictly below the one upstream.
//! Placement is a function of the seed and the coarse terrain; noise is keyed by position in
//! feet, so a landform is the same size on any map.
//!
//! The T0 grid can't hold a cliff (its cells are a mile across), so it also keeps a weight per
//! cell (`T0::landform`, 0 near rivers and lakes) and every refined level steps the ground again
//! with the same `Terrace` (`lod::terrain_refine`): each level halves the risers' width as its
//! samples halve, down to `SHARPEN_MIN_FT`, so the steps end as cliffs a few samples wide there.

use super::climate::Climate;
use super::hydro::{self, Hydro};
use crate::World;
use crate::core::noise::{Fbm, smoothstep};
use crate::core::rng::{self, Pcg32};

const MI: f64 = 5280.0;
/// Refined levels step the ground again down to this sample spacing (ft).
pub const SHARPEN_MIN_FT: f64 = 80.0;
/// Each refined level's riser, as a fraction of the step either side of its middle: 3/8 (a
/// gain of 2 at its middle) keeps a riser as many samples wide at every level.
pub const SHARPEN_RISER: f64 = 0.375;

/// The terrace levels: `step` ft apart from a slowly wandering base, the contours warped so
/// edges are ragged. A pure function of world position and height.
pub struct Terrace {
    pub step: f64,
    sea: f64,
    phase: Fbm,
    warp: Fbm,
}

impl Terrace {
    pub fn new(world: &World) -> Terrace {
        let p = world.params();
        let a = strength(p.ruggedness);
        Terrace { step: 220.0 + 380.0 * a, sea: p.sea_level_ft, phase: Fbm::new(world.stream("t0.landform.phase"), 2, 2.0, 0.5), warp: Fbm::new(world.stream("t0.landform.warp"), 3, 2.0, 0.5) }
    }

    /// The ground `hk` at (x, y) ft stepped: within `riser` of a band's middle it climbs to the
    /// next level, elsewhere it lies on one.
    #[inline]
    pub fn at(&mut self, x: f64, y: f64, hk: f64, riser: f64) -> f64 {
        let step = self.step;
        let base = self.sea + step * (0.5 + self.phase.at(x / (300.0 * MI), y / (300.0 * MI)));
        let wn = 0.45 * step * self.warp.at(x / (14.0 * MI), y / (14.0 * MI));
        let u = (hk + wn - base) / step;
        let lv = crate::core::floor(u);
        base + step * (lv + smoothstep(0.5 - riser, 0.5 + riser, u - lv)) - wn
    }
}

/// How strongly ruggedness asks for landforms: 0 below 0.2, 1 at 2.0, a little more at 2.5.
pub fn strength(ruggedness: f64) -> f64 {
    ((ruggedness - 0.2) / 1.8).clamp(0.0, 1.25)
}

fn slope_at(w: usize, h: usize, cell: f64, height: &[f64], k: usize) -> f64 {
    let (i, j) = (k % w, k / w);
    let (il, ir, ju, jd) = (i.saturating_sub(1), (i + 1).min(w - 1), j.saturating_sub(1), (j + 1).min(h - 1));
    let sx = (height[j * w + ir] - height[j * w + il]) / ((ir - il).max(1) as f64 * cell);
    let sy = (height[jd * w + i] - height[ju * w + i]) / ((jd - ju).max(1) as f64 * cell);
    crate::core::sqrt(sx * sx + sy * sy)
}

/// Plateaus (terraced uplands) and escarpments, before hydrology. `keep`: cells left alone
/// (drawn lakes and rivers, volcanoes). Returns the cells' terrace weights.
#[allow(clippy::too_many_arguments)]
pub fn shape(world: &World, w: usize, h: usize, cell: f64, height: &mut [f64], land: &[bool], clim: &Climate, keep: &[bool]) -> Vec<f32> {
    let p = world.params();
    let a = strength(p.ruggedness);
    let n = w * h;
    let mut weight = vec![0f32; n];
    if a <= 0.0 {
        return weight;
    }
    let (sea, top) = (p.sea_level_ft, p.max_elev_ft);
    let sea_mask: Vec<bool> = land.iter().map(|&l| !l).collect();
    let coast = super::climate::distance_to(w, h, &sea_mask);
    // Gentle country only, away from the coast and below the high ranges.
    let fit = |k: usize, hk: f64, slope: f64| {
        let above = hk - sea;
        let coast_ft = coast[k] * cell;
        smoothstep(1.5 * MI, 6.0 * MI, coast_ft) * (1.0 - smoothstep(0.45 * top, 0.65 * top, above)) * (1.0 - smoothstep(0.025, 0.07, slope))
    };
    let orig = height.to_vec();

    // Escarpments: long steps along the contours, the uphill side raised and falling back
    // gently behind its edge (a cuesta). Their edges are stepped with the terraces below.
    // (One chance per lattice cell 120 mi across, at a hashed spot in it: a scarp depends only on
    // its lattice cell, the seed and the ground round its spot, so drawing elsewhere on the map
    // or changing its size never moves it.)
    let lattice = 120.0 * MI;
    let scarps = world.stream("t0.landform.scarps");
    let chance = (0.55 * a).min(0.8);
    let mut sinuous = Fbm::new(world.stream("t0.landform.scarp_warp"), 3, 2.0, 0.5);
    let (lw, lh) = ((w as f64 * cell / lattice).ceil() as i64, (h as f64 * cell / lattice).ceil() as i64);
    for (gi, gj) in (0..lh).flat_map(|gj| (0..lw).map(move |gi| (gi, gj))) {
        let mut rng = Pcg32::new(rng::hash2(scarps, gi, gj), 1);
        if rng.next_f64() >= chance {
            continue;
        }
        let (px, py) = ((gi as f64 + rng.next_f64()) * lattice, (gj as f64 + rng.next_f64()) * lattice);
        let (pi, pj) = (crate::core::round(px / cell) as usize, crate::core::round(py / cell) as usize);
        if pi >= w || pj >= h {
            continue;
        }
        let k = pj * w + pi;
        let above = orig[k] - sea;
        if !(land[k] && !keep[k] && above > 300.0 && above < 0.4 * top && coast[k] * cell > 10.0 * MI && slope_at(w, h, cell, &orig, k) < 0.06) {
            continue;
        }
        let (ci, cj) = ((k % w) as i64, (k / w) as i64);
        // Coarse uphill direction (eight cells each way), else any.
        let r = 8i64;
        let hv = |i: i64, j: i64| orig[(j.clamp(0, h as i64 - 1) as usize) * w + i.clamp(0, w as i64 - 1) as usize];
        let (gx, gy) = (hv(ci + r, cj) - hv(ci - r, cj), hv(ci, cj + r) - hv(ci, cj - r));
        let gl = crate::core::sqrt(gx * gx + gy * gy);
        let (nx, ny) = if gl > 1.0 {
            (gx / gl, gy / gl)
        } else {
            let t = rng.range(0.0, std::f64::consts::TAU);
            (libm::cos(t), libm::sin(t))
        };
        let len = rng.range(40.0, 120.0) * MI * (0.7 + 0.3 * a);
        let rise = (250.0 + 450.0 * a) * rng.range(0.6, 1.2);
        let edge = (0.6 * cell).max(1_500.0);
        let back = 0.3 * len;
        let (cx, cy) = (ci as f64 * cell, cj as f64 * cell);
        let reach = 0.5 * len + back + 0.15 * len;
        let (i0, i1) = (((cx - reach) / cell).floor().max(0.0) as usize, (((cx + reach) / cell).ceil() as usize).min(w - 1));
        let (j0, j1) = (((cy - reach) / cell).floor().max(0.0) as usize, (((cy + reach) / cell).ceil() as usize).min(h - 1));
        // (Its own stretch of the wiggle noise, from its lattice cell.)
        let off = (rng::hash2(scarps ^ 0x5ca7_0ff5, gi, gj) % 100_000) as f64 * 0.0137;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let kk = j * w + i;
                if !land[kk] || keep[kk] {
                    continue;
                }
                let (dx, dy) = (i as f64 * cell - cx, j as f64 * cell - cy);
                let (u, v) = (dx * -ny + dy * nx, dx * nx + dy * ny);
                let v = v + 0.12 * len * sinuous.at(u / (0.25 * len) + off, off);
                let along = 1.0 - smoothstep(0.3 * len, 0.5 * len, crate::core::fabs(u));
                let lift = smoothstep(-edge, edge, v) * (1.0 - smoothstep(0.1 * back, back, v)) * along;
                if lift <= 0.0 {
                    continue;
                }
                let g = fit(kk, orig[kk], slope_at(w, h, cell, &orig, kk));
                height[kk] += rise * lift * g;
                let band = (1.0 - smoothstep(edge, 4.0 * edge, crate::core::fabs(v))) * along * g;
                weight[kk] = weight[kk].max(band as f32);
            }
        }
    }

    // Terraces: within plateau provinces (and on the escarpments' edges) the ground is stepped,
    // so sloping uplands become tablelands, dissected edges mesas and buttes.
    let riser = 0.24 - 0.16 * a.min(1.0);
    let threshold = 0.2 - 0.4 * a.min(1.0);
    let mut prov = Fbm::new(world.stream("t0.landform.province"), 3, 2.0, 0.5);
    let mut terrace = Terrace::new(world);
    for k in 0..n {
        if !land[k] || keep[k] {
            continue;
        }
        let (x, y) = ((k % w) as f64 * cell, (k / w) as f64 * cell);
        let hk = height[k];
        let above = orig[k] - sea;
        let mut m = weight[k] as f64;
        // (`fit`, its slope looked up only where the coast and the height leave it a say, and
        // the province noise only where the gate and the upland do: 0 otherwise, as multiplied.)
        let coast_ft = coast[k] * cell;
        let open = smoothstep(1.5 * MI, 6.0 * MI, coast_ft) * (1.0 - smoothstep(0.45 * top, 0.65 * top, above));
        if open > 0.0 {
            let gate = open * (1.0 - smoothstep(0.025, 0.07, slope_at(w, h, cell, &orig, k)));
            let dry = 1.0 - smoothstep(300.0, 900.0, clim.precip[k] as f64);
            let upland = (dry * smoothstep(100.0, 600.0, above)).max(0.9 * smoothstep(800.0, 2_500.0, above));
            let share = gate * upland;
            if share > 0.0 {
                m = m.max(share * smoothstep(threshold, threshold + 0.12, prov.at(x / (160.0 * MI), y / (160.0 * MI))));
            }
        }
        weight[k] = m as f32;
        if m <= 0.0 {
            continue;
        }
        // (Never below the sea.)
        height[k] = (hk + m * (terrace.at(x, y, hk, riser) - hk)).max(sea + 5.0);
    }
    weight
}

/// Canyons: rivers crossing uplands cut down into them, with steep walls. After hydrology,
/// on its rivers; every river cell stays strictly below the one upstream (and above its mouth).
/// Also clears the terrace weights near rivers and lakes (their water follows the T0 ground,
/// which refined levels must not step under it).
#[allow(clippy::too_many_arguments)]
pub fn incise(world: &World, w: usize, h: usize, cell: f64, height: &mut [f64], land: &[bool], clim: &Climate, hy: &Hydro, keep: &[bool], weight: &mut [f32]) {
    clear_water(w, h, cell, land, hy, weight);
    let p = world.params();
    let a = strength(p.ruggedness);
    if a <= 0.0 || hy.rivers.is_empty() {
        return;
    }
    let (sea, top) = (p.sea_level_ft, p.max_elev_ft);
    let q_river = hydro::river_q(p.river_density);
    let mut prov = Fbm::new(world.stream("t0.landform.canyons"), 2, 2.0, 0.5);
    let threshold = 0.15 - 0.35 * a.min(1.0);
    let deep = 300.0 + 800.0 * a;
    // (Out to two cells at least, and flat out to the cells beside the river: a floor the grid
    // holds, wide enough for the river's meanders and a road along it, not a notch between cells
    // whose sides the river's curve would climb.)
    let half = (1.4 * MI * (0.6 + 0.4 * a)).max(2.0 * cell);
    let flat = (cell / half).clamp(0.3, 0.8);
    let reach = crate::core::ceil(half / cell) as i64;
    let free = |k: usize| land[k] && hy.lake_of[k] == hydro::NO_LAKE && !keep[k];
    // Mains before the tributaries that join them.
    let mut order: Vec<usize> = Vec::with_capacity(hy.rivers.len());
    let mut done = vec![false; hy.rivers.len()];
    while order.len() < hy.rivers.len() {
        let before = order.len();
        for r in 0..hy.rivers.len() {
            if !done[r] && hy.rivers[r].into.is_none_or(|t| done[t]) {
                done[r] = true;
                order.push(r);
            }
        }
        if order.len() == before {
            order.extend((0..hy.rivers.len()).filter(|&r| !done[r]));
            break;
        }
    }
    let orig = height.to_vec();
    let mut cut = vec![0.0f64; w * h];
    for &r in &order {
        let river = &hy.rivers[r];
        let Some(&end) = river.cells.last() else { continue };
        // (Its mouth's water surface, not a lake's bed.)
        let end = end as usize;
        let end_h = if hy.lake_of[end] != hydro::NO_LAKE { hy.water[end] as f64 } else { height[end] }.max(sea);
        for (idx, &c) in river.cells.iter().enumerate() {
            let c = c as usize;
            if idx + 1 == river.cells.len() || !free(c) {
                continue;
            }
            let (x, y) = ((c % w) as f64 * cell, (c / w) as f64 * cell);
            let above = orig[c] - sea;
            let dry = 1.0 - smoothstep(400.0, 1_200.0, clim.precip[c] as f64);
            let gate = smoothstep(400.0, 1_800.0, above) * (1.0 - smoothstep(0.5 * top, 0.7 * top, above)) * (0.5 + 0.5 * dry) * smoothstep(threshold, threshold + 0.15, prov.at(x / (200.0 * MI), y / (200.0 * MI)));
            let size = smoothstep(1.0, 8.0, river.q[idx] as f64 / q_river);
            let d = (deep * gate * (0.35 + 0.65 * size)).min(0.6 * (height[c] - end_h).max(0.0));
            if d > cut[c] {
                cut[c] = d;
            }
        }
    }
    // Floors, then the walls round them: a narrow floor, steep sides up to the rim.
    let mut out = height.to_vec();
    for c in 0..w * h {
        if cut[c] <= 0.0 {
            continue;
        }
        let floor = orig[c] - cut[c];
        out[c] = out[c].min(floor);
        let (ci, cj) = ((c % w) as i64, (c / w) as i64);
        for dj in -reach..=reach {
            for di in -reach..=reach {
                let (i, j) = (ci + di, cj + dj);
                if (di == 0 && dj == 0) || i < 0 || j < 0 || i >= w as i64 || j >= h as i64 {
                    continue;
                }
                let k = j as usize * w + i as usize;
                if !free(k) || orig[k] <= floor {
                    continue;
                }
                let t = crate::core::sqrt((di * di + dj * dj) as f64) * cell / half;
                if t >= 1.0 {
                    continue;
                }
                let wall = floor + (orig[k] - floor) * smoothstep(flat, 1.0, t);
                if wall < out[k] {
                    out[k] = wall;
                }
            }
        }
    }
    // No wall or floor beside a lake drops below its surface (else the lake would spill into the
    // canyon); ground already lower there (a lake's outlet) is left as it was.
    for k in 0..w * h {
        if out[k] >= orig[k] || !land[k] || hy.lake_of[k] != hydro::NO_LAKE {
            continue;
        }
        let (ci, cj) = ((k % w) as i64, (k / w) as i64);
        let mut shore = f64::NEG_INFINITY;
        for (di, dj) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (i, j) = (ci + di, cj + dj);
            if i >= 0 && j >= 0 && i < w as i64 && j < h as i64 && hy.lake_of[j as usize * w + i as usize] != hydro::NO_LAKE {
                shore = shore.max(hy.water[j as usize * w + i as usize] as f64 + 2.0);
            }
        }
        out[k] = out[k].max(shore.min(orig[k]));
    }
    height.copy_from_slice(&out);
    downhill(w * h, height, &orig, land, hy, sea);
}

/// Every river step strictly downhill again after the cut (lake and sea cells excepted): the
/// rivers' cells in flow order (each after every cell flowing into it) lower the cells below
/// them, so one pass settles all: a cell is final before any step out of it is looked at.
fn downhill(n: usize, height: &mut [f64], orig: &[f64], land: &[bool], hy: &Hydro, sea: f64) {
    // Flow edges u -> d of every river (shared cells join them into one graph).
    // (Kept per river cell, `slot` its place in `next`: a list per map cell is a million of them.)
    let mut slot = vec![u32::MAX; n];
    let mut next: Vec<Vec<u32>> = Vec::new();
    let mut indeg = vec![0u32; n];
    let mut on = vec![false; n];
    for r in &hy.rivers {
        for pair in r.cells.windows(2) {
            let (u, d) = (pair[0] as usize, pair[1]);
            on[u] = true;
            on[d as usize] = true;
            if u != d as usize {
                if slot[u] == u32::MAX {
                    slot[u] = next.len() as u32;
                    next.push(Vec::new());
                }
                let out = &mut next[slot[u] as usize];
                if !out.contains(&d) {
                    out.push(d);
                    indeg[d as usize] += 1;
                }
            }
        }
    }
    let outs = |u: usize| -> &[u32] { if slot[u] == u32::MAX { &[] } else { &next[slot[u] as usize] } };
    // Kahn's order (ascending cell index among the ready ones: deterministic).
    let mut ready: std::collections::BinaryHeap<std::cmp::Reverse<u32>> = (0..n as u32).filter(|&k| on[k as usize] && indeg[k as usize] == 0).map(std::cmp::Reverse).collect();
    let mut seen = 0usize;
    let fix = |u: usize, height: &mut [f64]| {
        // (Hydrology left its rivers downhill: only steps out of a lowered cell need a look, and
        // the least drop f32 heights hold apart, so a flat run below a lowered cell, at a lake's
        // level say, sinks by next to nothing.)
        let hu = height[u];
        let gap = 0.001f64.max(crate::core::fabs(hu) * 5e-7);
        if hu >= orig[u] || hy.lake_of[u] != hydro::NO_LAKE {
            return;
        }
        for &d in outs(u) {
            let d = d as usize;
            if land[d] && hy.lake_of[d] == hydro::NO_LAKE && height[d] > sea && height[d] > hu - gap {
                height[d] = hu - gap;
            }
        }
    };
    let mut done = vec![false; n];
    while let Some(std::cmp::Reverse(u)) = ready.pop() {
        let u = u as usize;
        done[u] = true;
        seen += 1;
        fix(u, height);
        for &d in outs(u) {
            indeg[d as usize] -= 1;
            if indeg[d as usize] == 0 {
                ready.push(std::cmp::Reverse(d));
            }
        }
    }
    // (River nets are trees; a cycle would be a hydrology bug. Settle what's left in index order.)
    if seen < on.iter().filter(|&&o| o).count() {
        for u in 0..n {
            if on[u] && !done[u] {
                fix(u, height);
            }
        }
    }
    debug_assert!((0..n).all(|u| height[u] >= orig[u] || hy.lake_of[u] != hydro::NO_LAKE || outs(u).iter().all(|&d| {
        let d = d as usize;
        !land[d] || hy.lake_of[d] != hydro::NO_LAKE || height[d] <= sea || height[d] < height[u]
    })));
}

/// Terrace weights fade to 0 within about two miles (and at least two cells) of a river or lake.
fn clear_water(w: usize, h: usize, cell: f64, land: &[bool], hy: &Hydro, weight: &mut [f32]) {
    if weight.iter().all(|&m| m <= 0.0) {
        return;
    }
    let mut wet: Vec<bool> = (0..w * h).map(|k| land[k] && hy.lake_of[k] != hydro::NO_LAKE).collect();
    for r in &hy.rivers {
        for &c in &r.cells {
            wet[c as usize] = true;
        }
    }
    let d = super::climate::distance_to(w, h, &wet);
    let (near, far) = (1.0f64.max(0.5 * MI / cell), 2.5f64.max(2.0 * MI / cell));
    for k in 0..w * h {
        if weight[k] > 0.0 {
            weight[k] *= smoothstep(near, far, d[k]) as f32;
        }
    }
}

/// Terrace weights fade to 0 within about a mile and a half (and at least two cells) of the
/// polylines (x, y ft).
pub fn clear_lines(w: usize, h: usize, cell: f64, lines: &[&[[f64; 2]]], weight: &mut [f32]) {
    let mut on = vec![false; w * h];
    for pts in lines {
        for seg in pts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let steps = (crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])) / (0.5 * cell)).ceil().max(1.0) as usize;
            for t in 0..=steps {
                let f = t as f64 / steps as f64;
                let (i, j) = (crate::core::round((a[0] + (b[0] - a[0]) * f) / cell), crate::core::round((a[1] + (b[1] - a[1]) * f) / cell));
                if i >= 0.0 && j >= 0.0 && (i as usize) < w && (j as usize) < h {
                    on[j as usize * w + i as usize] = true;
                }
            }
        }
    }
    let d = super::climate::distance_to(w, h, &on);
    let (near, far) = (1.0f64.max(0.5 * MI / cell), 2.0f64.max(1.5 * MI / cell));
    for k in 0..w * h {
        if weight[k] > 0.0 {
            weight[k] *= smoothstep(near, far, d[k]) as f32;
        }
    }
}
