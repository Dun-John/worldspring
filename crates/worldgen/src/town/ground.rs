//! The ground under a town plan: height and river columns, shores, and river samples.

use super::*;
pub use crate::lod::plan_ground::{PlanGround, VALLEY_CELL, Shore, PlanColumns};

pub(super) const RIVER_CELL: f64 = 250.0;
/// Network road chords near a settlement are binned by this (`Site::near_network_road`, margins
/// up to 10 ft).
pub(super) const ROAD_CELL: f64 = 100.0;

/// Shoreline points of standing water (sea or lake) near the settlement: where the wet test
/// changes between neighbouring points of a `step` grid, refined by bisection. With each:
/// the unit normal pointing into the water.
pub(super) fn shore_points(site: &Site, step: f64, within: f64) -> Vec<(P, P)> {
    let n = (2.0 * within / step).ceil() as i64;
    let at = |i: i64, j: i64| [-within + i as f64 * step, -within + j as f64 * step];
    let wet_grid: Vec<bool> = (0..=n).flat_map(|j| (0..=n).map(move |i| (i, j))).map(|(i, j)| {
        let p = at(i, j);
        len(p) < within && site.wet_standing(p)
    }).collect();
    let idx = |i: i64, j: i64| (j * (n + 1) + i) as usize;
    let mut out: Vec<(P, P)> = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            for (di, dj) in [(1i64, 0i64), (0, 1)] {
                let (i2, j2) = (i + di, j + dj);
                if i2 > n || j2 > n || wet_grid[idx(i, j)] == wet_grid[idx(i2, j2)] {
                    continue;
                }
                let (mut dry, mut wetp) = if wet_grid[idx(i, j)] { (at(i2, j2), at(i, j)) } else { (at(i, j), at(i2, j2)) };
                if len(dry) >= within || len(wetp) >= within {
                    continue;
                }
                for _ in 0..6 {
                    let m = lerp(dry, wetp, 0.5);
                    if site.wet_standing(m) { wetp = m } else { dry = m }
                }
                let q = lerp(dry, wetp, 0.5);
                // Normal from the local wet gradient (8 samples around the point).
                let mut g = [0.0, 0.0];
                for k in 0..8 {
                    let ang = std::f64::consts::TAU * k as f64 / 8.0;
                    let d = [libm::cos(ang), libm::sin(ang)];
                    if site.wet_standing(add(q, mul(d, 0.5 * step))) {
                        g = add(g, d);
                    }
                }
                let gl = len(g);
                if gl > 1e-6 {
                    out.push((q, mul(g, 1.0 / gl)));
                }
            }
        }
    }
    out
}

/// Mesh vertices joined to `v` by an edge.
pub(super) fn adj_vertices(mesh: &Mesh, v: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for face in &mesh.faces {
        let m = face.len();
        for k in 0..m {
            if face[k] == v {
                for u in [face[(k + m - 1) % m], face[(k + 1) % m]] {
                    if !out.contains(&u) {
                        out.push(u);
                    }
                }
            }
        }
    }
    out.sort_unstable();
    out
}

/// Points along the river centreline every `every` ft within `within` of the centre:
/// (point, unit tangent, half width).
pub(super) fn river_samples(site: &Site, every: f64, within: f64) -> Vec<(P, P, f64)> {
    let mut out = Vec::new();
    let mut acc = 0.5 * every;
    for &(a, b, hw) in &site.river {
        let l = dist(a, b);
        if l < 1e-9 {
            continue;
        }
        let t = mul(sub(b, a), 1.0 / l);
        while acc <= l {
            let p = add(a, mul(t, acc));
            if len(p) < within {
                out.push((p, t, hw));
            }
            acc += every;
        }
        acc -= l;
    }
    out
}

pub(super) fn river_pieces(t0: &T0, center: P, r: f64) -> Vec<(P, P, f64)> {
    let mut out = Vec::new();
    for (ri, k) in t0.rivers.segments_near(center[0] - r, center[1] - r, center[0] + r, center[1] + r, 0.0) {
        let rc = &t0.rivers.rivers[ri as usize];
        let len = rc.s[k as usize + 1] - rc.s[k as usize];
        // Fine enough to follow meanders and wiggles to a few feet (banks, quays, piers).
        let n = (len / 20.0).ceil().clamp(2.0, 1200.0) as usize;
        let mut prev: Option<(P, f64)> = None;
        for j in 0..=n {
            let cp = rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft);
            let p = sub(cp.p, center);
            if let Some((q, w)) = prev {
                out.push((q, p, 0.5 * w.max(cp.w)));
            }
            prev = Some((p, cp.w));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
