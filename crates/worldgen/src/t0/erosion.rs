//! Fluvial erosion with tectonic uplift: the implicit stream-power solver of Braun & Willett
//! (2013), as used for large-scale terrain by Cordonnier et al. (2016). Iterating uplift +
//! erosion toward steady state yields dendritic valley networks, ridges and passes that
//! noise alone cannot. Heights are in model units; the caller rescales to feet.
//!
//! Flow directions are stochastic (steepest descent with per-iteration jitter), which removes
//! the parallel, grid-aligned valleys plain D8 produces on a regular grid.

use super::flood::{D8, neighbors, priority_flood};
use crate::core::rng::{mix64, unit};

/// Stream-power erodibility.
const K: f64 = 0.35;
// Drainage-area exponent m = 0.5 in E = K A^m S^n (n = 1), applied as a square root below.
const DT: f64 = 2.0;
/// Hillslope diffusion per iteration (keeps ridges from becoming knife-edges).
const KAPPA: f64 = 0.03;
/// Receiver choice weighs each downhill slope by a random factor in [1 - J, 1 + J].
const JITTER: f64 = 0.45;

/// `land[i]` cells erode; everything else is a fixed base-level outlet at height 0.
pub fn erode(w: usize, h: usize, land: &[bool], uplift: &[f64], iterations: usize, seed: u64, mut progress: impl FnMut(f64)) -> Vec<f64> {
    let n = w * h;
    let outlet: Vec<bool> = land.iter().map(|l| !l).collect();
    // Start from gentle multi-scale relief so early drainage is irregular, not grid-aligned.
    let mut relief = crate::core::noise::Fbm::new(seed, 5, 2.0, 0.55);
    let mut z: Vec<f64> = (0..n)
        .map(|i| {
            if !land[i] {
                return 0.0;
            }
            let (x, y) = ((i % w) as f64, (i / w) as f64);
            0.5 * (1.0 + relief.at(x / 18.0, y / 18.0))
        })
        .collect();
    let mut area = vec![0.0f64; n];
    let mut rec = vec![0u32; n];
    let mut dist = vec![1.0f64; n];
    let mut lap = vec![0.0f64; n];

    for it in 0..iterations {
        for i in 0..n {
            if land[i] {
                z[i] += uplift[i] * DT;
            }
        }

        // Route flow over the depression-filled surface; the flood order is a valid
        // downstream-first ordering for the implicit solve.
        let fl = priority_flood(w, h, &z, &outlet, 1e-7);
        // hash3(seed, i, it, k) = mix64(seed ^ mix64(i ^ inner[k])): the inner mixes depend
        // only on the iteration and neighbour slot, so they are computed once per iteration.
        let inner: [u64; 8] = std::array::from_fn(|k| mix64((it as u64) ^ mix64((k as u64) ^ 0x632b_e59b_d9b4_e019)));
        // (Only land cells: the solve and the drainage areas read no outlet's receiver.)
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if outlet[i] {
                    continue;
                }
                let (mut best, mut best_s, mut best_d) = (i, 0.0, 1.0);
                let mut take = |k: usize, nb: usize, d: f64| {
                    let drop = fl.filled[i] - fl.filled[nb];
                    if drop <= 0.0 {
                        return;
                    }
                    let jitter = 1.0 + JITTER * (2.0 * unit(mix64(seed ^ mix64((i as u64) ^ inner[k]))) - 1.0);
                    let s = drop / d * jitter;
                    if s > best_s {
                        (best, best_s, best_d) = (nb, s, d);
                    }
                };
                if x > 0 && y > 0 && x + 1 < w && y + 1 < h {
                    // Inside the grid every neighbour is there: slot k is the D8 slot.
                    for (k, &(dx, dy, d)) in D8.iter().enumerate() {
                        take(k, (i as isize + dy as isize * w as isize + dx as isize) as usize, d);
                    }
                } else {
                    for (k, (nb, d)) in neighbors(w, h, i).enumerate() {
                        take(k, nb, d);
                    }
                }
                rec[i] = best as u32;
                dist[i] = best_d;
            }
        }

        area.iter_mut().for_each(|a| *a = 1.0);
        for &i in fl.order.iter().rev() {
            let i = i as usize;
            let r = rec[i] as usize;
            if r != i {
                area[r] += area[i];
            }
        }

        for &i in &fl.order {
            let i = i as usize;
            let r = rec[i] as usize;
            if r == i {
                continue;
            }
            // A^M with M = 0.5: drainage areas are whole cell counts, for which sqrt gives
            // the same bits as pow(A, 0.5) (checked exhaustively up to 2^24), far faster.
            let f = K * DT * crate::core::sqrt(area[i]) / dist[i];
            z[i] = (z[i] + f * z[r]) / (1.0 + f);
        }

        // Hillslope diffusion (4-neighbor Laplacian, outlets fixed).
        for i in 0..n {
            if !land[i] {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let mut s = 0.0;
            for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                s += if nx < w && ny < h { z[ny * w + nx] } else { 0.0 };
            }
            lap[i] = s - 4.0 * z[i];
        }
        for i in 0..n {
            if land[i] {
                z[i] = (z[i] + KAPPA * lap[i]).max(0.0);
            }
        }
        progress((it + 1) as f64 / iterations as f64);
    }
    z
}
