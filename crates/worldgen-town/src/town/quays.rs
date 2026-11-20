//! Quays and piers along a town's shore (`build_town`'s phase after the streets and approaches).

use super::*;

/// Quays and landings: city patches on the sea or a lake get a paved quay along the shore;
/// dock patches get piers every ~80 ft, square to the shore, starting at the quay. A pier is
/// built only over open water and clear of other piers. P2 160: a city's or metropolis's docks
/// on a sea or lake get quay fronts (`front`): the quay carried out to the water's edge.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(site: &Site, s: &Settlement, plan: &TownPlan, n_inner: usize, cells: &[Labeled], ward: &[Option<Ward>], usable: impl Fn(usize) -> bool, edge_v: impl Fn(usize, usize) -> (usize, usize), l: &mut Layout) {
    let mut pier_dirs: Vec<P> = Vec::new();
    let islets = site.islets();
    // (Of a city's quay fronts this share is timber, the rest stone: most none, some a few, a
    // few most.)
    let roll = unit(hash2(0x9a7e_0160, s.seed as i64, 1));
    let timber_share = if roll < 0.55 { 0.0 } else if roll < 0.9 { 0.3 } else { 0.75 };
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
            let mut quay = vec![a, b, sub(b, mul(out, 14.0)), sub(a, mul(out, 14.0))];
            let made = if s.tier >= Tier::City && ward[i] == Some(Ward::Docks) { front(site, s, l, &islets, &quay, out, timber_share) } else { None };
            if let Some(q) = made {
                // (The quay's paving reaches its face: no bare shore between the lots and the water.)
                quay = q.pts.clone();
                l.quays.push(q);
                l.plazas.push(Plaza { pts: quay, kind: PlazaKind::Quay, spot: places::Spot::None });
            } else if !site.wet(centroid(&quay)) && !site.near_river(centroid(&quay), 10.0) {
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

/// The farthest out (ft past the patch's side) a quay front is carried to find the water.
const FRONT_REACH_FT: f64 = 46.0;

/// P2 160: a quay front for the shore strip `quay` (`a`, `b` on the patch's side, then the lots'
/// side `out` of it 14 ft in): its face carried out along `out` to 2 ft past the farthest the
/// water's edge lies along it (at most `FRONT_REACH_FT`, never short of the side), its top the
/// lots' ground kept `wharf::MADE_ABOVE_FT` over the water. Only on standing water (no river
/// within 20 ft), the water found off at least half the side, clear of network roads, bridges and
/// islets; stone or, `timber_share` of them (by the side's place), timber.
fn front(site: &Site, s: &Settlement, l: &Layout, islets: &[Vec<P>], quay: &[P], out: P, timber_share: f64) -> Option<super::harbour::Quay> {
    let (a, b) = (quay[0], quay[1]);
    let el = dist(a, b);
    if el < 20.0 {
        return None;
    }
    let n = crate::core::ceil(el / 10.0) as usize;
    // (Out to 2 ft past the farthest edge of standing water; on a river, never into its channel:
    // to 1 ft short of the nearest.)
    let (mut far, mut bank, mut found) = (0.0f64, f64::MAX, 0);
    for k in 0..=n {
        let p = lerp(a, b, k as f64 / n as f64);
        let mut t = -12.0;
        while t <= FRONT_REACH_FT && !site.wet(add(p, mul(out, t))) {
            t += 2.0;
        }
        if t > FRONT_REACH_FT {
            continue;
        }
        let w = add(p, mul(out, t));
        if site.wet_standing(w) && !site.near_river(w, 20.0) {
            far = far.max(t + 2.0);
        } else {
            bank = bank.min(t - 1.0);
        }
        found += 1;
    }
    if 2 * found < n + 1 {
        return None;
    }
    let reach = if bank < f64::MAX { bank } else { far.min(FRONT_REACH_FT) };
    if reach < 0.0 {
        return None;
    }
    let pts = vec![add(a, mul(out, reach)), add(b, mul(out, reach)), quay[2], quay[3]];
    if site.deck_on_road(&pts) || l.bridges.iter().any(|d| poly_gap(d, &pts) < 10.0) || islets.iter().any(|i| poly_gap(i, &pts) < 5.0) {
        return None;
    }
    let (back, face) = (site.world(lerp(pts[2], pts[3], 0.5)), site.world(lerp(pts[0], pts[1], 0.5)));
    let level = crate::wharf::level(
        &crate::world::Wharf { kind: crate::world::WharfKind::Quay, pts: vec![back, face], width: reach + 14.0, material: crate::world::CrossingMaterial::Stone },
        site.t0,
    );
    let mid = lerp(a, b, 0.5);
    let timber = unit(hash2(s.seed ^ 0x9a7e_0161, crate::core::round(mid[0]) as i64, crate::core::round(mid[1]) as i64)) < timber_share;
    Some(super::harbour::Quay { pts, level, timber })
}
