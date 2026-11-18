//! A town's outskirts' wards (`plan_town`'s phase after the town's own): shanty towns, farmland
//! and the wild beyond.

use super::*;

/// Shanty towns (watabou): outside patches touching the city grow outward one by one,
/// favouring the gates, the centre and the shore; then farmland to an irregular radius,
/// wild beyond.
///
/// Sets the outskirts patches' `ward` (`Slum`, `Farm` or none); `cells` are the plan's patches
/// (the town's first `n_inner`).
#[allow(clippy::too_many_arguments)]
pub(super) fn grow(
    cells: &[Labeled],
    wet: &[bool],
    n_inner: usize,
    pseed: u64,
    spacing: f64,
    r_out: f64,
    gate_pts: &[P],
    usable: impl Fn(usize) -> bool,
    neighbors: impl Fn(usize) -> Vec<usize>,
    ward: &mut [Option<Ward>],
) {
    let total = cells.len();
    let roll = |i: usize, stage: i64| seeds::roll(pseed, i, stage);
    let outer_ok = |i: usize| i >= n_inner && !wet[i] && cells[i].pts.len() >= 3;
    let mut slum = vec![false; total];
    let u = roll(0, 20);
    let mut budget = (crate::core::sqrt(n_inner as f64) * (1.0 + u * u * u)) as usize;
    let mut round = 0;
    while budget > 0 && round < 400 {
        round += 1;
        let mut cands: Vec<(usize, f64)> = Vec::new();
        for i in (0..total).filter(|&i| outer_ok(i) && !slum[i]) {
            let touching = neighbors(i).iter().filter(|&&j| usable(j) || slum[j]).count();
            if touching < 2 {
                continue;
            }
            let c = centroid(&cells[i].pts);
            let gate_d = gate_pts.iter().map(|g| dist(*g, c)).fold(f64::MAX, f64::min);
            let shore_d = if neighbors(i).iter().any(|&j| wet[j]) { 0.5 * spacing } else { f64::MAX };
            let nearest = (3.0 * len(c)).min(2.0 * gate_d).min(shore_d).max(1.0);
            cands.push((i, 1.0 / (nearest * nearest)));
        }
        if cands.is_empty() {
            break;
        }
        let total_w: f64 = cands.iter().map(|c| c.1).sum();
        let mut pick = unit(hash3(pseed, round, 22, 0x51)) * total_w;
        let mut chosen = cands[cands.len() - 1].0;
        for &(i, w) in &cands {
            if pick < w {
                chosen = i;
                break;
            }
            pick -= w;
        }
        slum[chosen] = true;
        budget -= 1;
    }
    let (a1, a2, f1, f2) = (roll(0, 30) * 0.15 + 0.1, roll(0, 31) * 0.1 + 0.05, roll(0, 32) * std::f64::consts::TAU, roll(0, 33) * std::f64::consts::TAU);
    for i in n_inner..total {
        if !outer_ok(i) {
            continue;
        }
        let c = centroid(&cells[i].pts);
        let th = libm::atan2(c[1], c[0]);
        let farmed = len(c) < (1.0 + a1 * libm::sin(th + f1) + a2 * libm::sin(2.0 * th + f2)) * r_out * 0.8;
        ward[i] = if slum[i] { Some(Ward::Slum) } else if farmed { Some(Ward::Farm) } else { None };
    }
}
