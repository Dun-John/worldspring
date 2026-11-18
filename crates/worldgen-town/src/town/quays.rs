//! Quays and piers along a town's shore (`build_town`'s phase after the streets and approaches).

use super::*;

/// Quays and landings: city patches on the sea or a lake get a paved quay along the shore;
/// dock patches get piers every ~80 ft, square to the shore, starting at the quay. A pier is
/// built only over open water and clear of other piers.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(site: &Site, plan: &TownPlan, n_inner: usize, cells: &[Labeled], ward: &[Option<Ward>], usable: impl Fn(usize) -> bool, edge_v: impl Fn(usize, usize) -> (usize, usize), l: &mut Layout) {
    let mut pier_dirs: Vec<P> = Vec::new();
    let islets = site.islets();
    for i in 0..n_inner {
        if !usable(i) {
            continue;
        }
        let c = &cells[i];
        let m = c.pts.len();
        let cen = centroid(&c.pts);
        for e in 0..m {
            let (va, vb) = edge_v(i, e);
            if c.labels[e] < 0 || !plan.shore.contains(&(va.min(vb), va.max(vb))) {
                continue;
            }
            let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
            let el = dist(a, b);
            if el < 1.0 {
                continue;
            }
            let u = mul(sub(b, a), 1.0 / el);
            let mut out = [-u[1], u[0]];
            if dot(out, sub(lerp(a, b, 0.5), cen)) < 0.0 {
                out = mul(out, -1.0);
            }
            let quay = vec![a, b, sub(b, mul(out, 14.0)), sub(a, mul(out, 14.0))];
            if !site.wet(centroid(&quay)) && !site.near_river(centroid(&quay), 10.0) {
                l.plazas.push(Plaza { pts: quay, kind: PlazaKind::Quay, spot: places::Spot::None });
            }
            if ward[i] != Some(Ward::Docks) {
                continue;
            }
            let n_piers = ((el - 60.0) / 80.0).floor().max(0.0) as usize + 1;
            for k in 0..n_piers {
                if el < 60.0 {
                    break;
                }
                let edge_pt = add(a, mul(u, 30.0 + (el - 60.0) * (k as f64 + 0.5) / n_piers as f64));
                // Anchor on the real shore: walk from the patch edge to the water's edge.
                let mut t = -60.0;
                while t < 60.0 && !site.wet(add(edge_pt, mul(out, t))) {
                    t += 4.0;
                }
                if t >= 60.0 || t <= -60.0 {
                    continue;
                }
                let base = add(edge_pt, mul(out, t - 4.0));
                // On a river a pier reaches a third of the way across at most (the village wharf's
                // rule), and leaves an open channel to the piers facing it from the other bank.
                let mid = add(base, mul(out, 20.0));
                let river = if site.wet_standing(mid) { None } else { site.nearest_river(mid).filter(|_| site.near_river(mid, 0.0)) };
                let mut length: f64 = 105.0;
                if let Some((_, _, hw)) = river {
                    length = length.min(0.66 * hw + 4.0);
                    if length < 24.0 {
                        continue;
                    }
                }
                let tip = add(base, mul(out, length));
                let over_water = (1..=8).all(|j| site.wet(add(base, mul(out, length * j as f64 / 8.0))));
                let clear = l.piers.iter().all(|pp| {
                    let pc = centroid(pp);
                    seg_dist(pc, base, tip) > 16.0
                });
                let pier = rect(add(base, mul(out, 0.5 * length)), out, length, 10.0);
                let mut free = !site.deck_on_road(&pier) && !l.bridges.iter().any(|d| overlaps_or_crosses(d, &pier));
                if let (true, Some((_, _, hw))) = (free, river) {
                    let channel = hw.max(40.0);
                    // Clear of every pier (both banks), of facing piers by the channel, of bridges
                    // and their approaches, fords and ferries by 30 ft.
                    free = l.piers.iter().zip(&pier_dirs).all(|(pp, &pd)| dot(pd, out) >= 0.0 || poly_gap(pp, &pier) >= channel)
                        && l.bridges.iter().all(|d| poly_gap(d, &pier) >= 30.0)
                        && !(0..=8).any(|j| site.near_network_road(add(base, mul(out, length * j as f64 / 8.0)), 36.0));
                }
                // No pier on any water touches or crosses another (6 ft between outlines): at a patch
                // corner the later one is dropped. (R5 M20: and none within 20 ft of an islet.)
                free = free && l.piers.iter().all(|pp| poly_gap(pp, &pier) >= 6.0) && islets.iter().all(|i| poly_gap(i, &pier) >= 20.0);
                if over_water && clear && free {
                    l.piers.push(pier);
                    pier_dirs.push(out);
                }
            }
        }
    }
}
