//! Quays and piers along a town's shore (`build_town`'s phase after the streets and approaches).

use super::*;

/// Quays and landings: city patches on the sea or a lake get a paved quay along the shore;
/// dock patches get piers every ~80 ft, square to the shore, starting at the quay. A pier is
/// built only over open water and clear of other piers. P2 160: a city's or metropolis's built
/// wards on a sea or lake get quay fronts along the shore line (`fronts`, `fronts_water`).
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(site: &Site, s: &Settlement, plan: &TownPlan, n_inner: usize, cells: &[Labeled], ward: &[Option<Ward>], usable: impl Fn(usize) -> bool, edge_v: impl Fn(usize, usize) -> (usize, usize), l: &mut Layout) {
    let mut pier_dirs: Vec<P> = Vec::new();
    let islets = site.islets();
    // (Of a city's quay fronts this share is timber, the rest stone: about a third overall, a
    // town more or less.)
    let roll = unit(hash2(0x9a7e_0160, s.seed as i64, 1));
    let timber_share = if roll < 0.3 { 0.2 } else if roll < 0.7 { 0.4 } else { 0.6 };
    let mut sides: Vec<Side> = Vec::new();
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
            if s.tier >= Tier::City && ward[i].is_some_and(fronts_water) {
                // (Laid along the whole shore at once below, `fronts`; out on the right of a->b.)
                let right = u[1] * out[0] - u[0] * out[1] > 0.0;
                sides.push(if right { Side { a, b, va, vb, out } } else { Side { a: b, b: a, va: vb, vb: va, out } });
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
    fronts(site, s, l, &islets, &sides, timber_share);
}

/// P2 160: wards whose shore sides get quay fronts: the docks and every built ward whose lots
/// front the water (not a castle, temple close, noble estates, barracks, park or open country).
fn fronts_water(w: Ward) -> bool {
    matches!(w, Ward::Docks | Ward::Plaza | Ward::Merchant | Ward::Craft | Ward::Common | Ward::Slum)
}


/// A city shore side whose ward fronts the water (`lay`): `a`->`b` (plan vertices `va`, `vb`), the
/// water's side `out` on its right.
struct Side {
    a: P,
    b: P,
    va: usize,
    vb: usize,
    out: P,
}

/// The farthest out (ft past the patch's side) a quay front is carried to find the water.
const FRONT_REACH_FT: f64 = 100.0;
/// The shore's samples along the sides (ft, about).
const FRONT_STEP_FT: f64 = 5.0;
/// The lots' fronts behind the patch's side (ft): the quay's back.
const FRONT_BACK_FT: f64 = 14.0;
/// The face past the water's edge (ft) on the sea or a lake; on a river it stops 1 ft short.
const FACE_OUT_FT: f64 = 3.0;
/// A quay's end eases its face back onto the shore over this many samples.
const FRONT_TAPER: usize = 3;
/// A stone or timber stretch of a quay, about (ft).
const FRONT_PIECE_FT: f64 = 150.0;

/// One shore sample (`chain_fronts`): its point, side, place along that side (ft), side's normal.
type Sample = (P, usize, f64, P);

/// P2 160: the quay fronts along a city's built shore (`sides`): the sides chained where they
/// meet, and along each chain the water's edge found every ~5 ft (`FRONT_STEP_FT`, walking out
/// along the shore's smoothed normal to where the water starts and holds). A front is a run of
/// samples with the water in reach: its face traces the edge, smoothed and `FACE_OUT_FT` past it
/// (on a river 1 ft short of the channel), eased onto the shore at its ends; its back the lots'
/// fronts. Clear of network roads, bridges, islets and river mouths. Stone and timber in
/// stretches of ~150 ft (`timber_share` of them), each its own front and quay plaza. The parts of
/// the sides no front takes keep the 14 ft strip plaza where water lies off them.
fn fronts(site: &Site, s: &Settlement, l: &mut Layout, islets: &[Vec<P>], sides: &[Side], timber_share: f64) {
    if sides.is_empty() {
        return;
    }
    // (Chains: from the sides nothing leads into, then round closed shores.)
    let mut used = vec![false; sides.len()];
    let mut chains: Vec<Vec<usize>> = Vec::new();
    for pass in 0..2 {
        for i in 0..sides.len() {
            if used[i] || (pass == 0 && sides.iter().any(|q| q.vb == sides[i].va)) {
                continue;
            }
            let mut chain = vec![i];
            used[i] = true;
            while let Some(k) = (0..sides.len()).find(|&k| !used[k] && sides[k].va == sides[*chain.last().unwrap()].vb) {
                used[k] = true;
                chain.push(k);
            }
            chains.push(chain);
        }
    }
    let mut covered: Vec<Vec<(f64, f64)>> = vec![Vec::new(); sides.len()];
    for chain in &chains {
        chain_fronts(site, s, l, islets, sides, chain, timber_share, &mut covered);
    }
    // (The rest of each side: the strip where water lies off it.)
    for (i, sd) in sides.iter().enumerate() {
        let el = dist(sd.a, sd.b);
        if el < 1.0 {
            continue;
        }
        let u = mul(sub(sd.b, sd.a), 1.0 / el);
        let mut cov = covered[i].clone();
        cov.sort_by(|x, y| x.0.total_cmp(&y.0));
        let (mut gaps, mut at) = (Vec::new(), 0.0);
        for &(c0, c1) in &cov {
            if c0 > at {
                gaps.push((at, c0));
            }
            at = f64::max(at, c1);
        }
        if at < el {
            gaps.push((at, el));
        }
        for (g0, g1) in gaps {
            let (p0, p1) = (add(sd.a, mul(u, g0)), add(sd.a, mul(u, g1)));
            let rest = vec![p0, p1, sub(p1, mul(sd.out, FRONT_BACK_FT)), sub(p0, mul(sd.out, FRONT_BACK_FT))];
            let by_water = cov.is_empty() || (0..=15).any(|j| site.wet(add(lerp(p0, p1, 0.5), mul(sd.out, 10.0 * j as f64))));
            if g1 - g0 >= 10.0 && by_water && !site.wet(centroid(&rest)) && !site.near_river(centroid(&rest), 10.0) {
                l.plazas.push(Plaza { pts: rest, kind: PlazaKind::Quay, spot: places::Spot::None });
            }
        }
    }
}

/// The fronts along one chain of sides (`fronts`); records the stretch of each side they take.
#[allow(clippy::too_many_arguments)]
fn chain_fronts(site: &Site, s: &Settlement, l: &mut Layout, islets: &[Vec<P>], sides: &[Side], chain: &[usize], timber_share: f64, covered: &mut [Vec<(f64, f64)>]) {
    let lens: Vec<f64> = chain.iter().map(|&i| dist(sides[i].a, sides[i].b)).collect();
    let total: f64 = lens.iter().sum();
    if total < 30.0 {
        return;
    }
    let n = crate::core::ceil(total / FRONT_STEP_FT) as usize;
    let step = total / n as f64;
    let smp: Vec<Sample> = (0..=n)
        .map(|k| {
            let mut at = k as f64 * step;
            let mut j = 0;
            while j + 1 < chain.len() && at > lens[j] {
                at -= lens[j];
                j += 1;
            }
            let sd = &sides[chain[j]];
            let at = at.min(lens[j]);
            (lerp(sd.a, sd.b, at / lens[j].max(1e-9)), chain[j], at, sd.out)
        })
        .collect();
    // (The shore's normal: the sides' smoothed over 20 ft either way, so corners round.)
    let reach = crate::core::ceil(20.0 / step) as i64;
    let normal: Vec<P> = (0..=n as i64)
        .map(|k| {
            let mut v = [0.0, 0.0];
            for j in (k - reach).max(0)..=(k + reach).min(n as i64) {
                v = add(v, mul(smp[j as usize].3, (reach + 1 - (j - k).abs()) as f64));
            }
            mul(v, 1.0 / dist(v, [0.0, 0.0]).max(1e-9))
        })
        .collect();
    // (The water's edge along the normal: where water starts and holds 20 ft, or to the reach;
    // and whether it is a river's.)
    let walk: Vec<Option<(f64, bool)>> = (0..=n)
        .map(|k| {
            let (p, nn) = (smp[k].0, normal[k]);
            let at = |t: f64| site.wet(add(p, mul(nn, t)));
            let mut t = -12.0;
            if at(t) {
                return None;
            }
            let mut start: Option<f64> = None;
            while t < FRONT_REACH_FT {
                t += 2.0;
                if !at(t) {
                    start = None;
                } else if start.is_none() {
                    start = Some(t);
                } else if start.is_some_and(|s0| t - s0 >= 20.0) {
                    break;
                }
            }
            let (mut lo, mut hi) = (start? - 2.0, start?);
            for _ in 0..3 {
                let m = 0.5 * (lo + hi);
                if at(m) {
                    hi = m;
                } else {
                    lo = m;
                }
            }
            let w = add(p, mul(nn, hi + 1.0));
            Some((hi, !(site.wet_standing(w) && !site.near_river(w, 20.0))))
        })
        .collect();
    let face_at = |k: usize, f: f64| add(smp[k].0, mul(normal[k], f));
    let ok: Vec<bool> = (0..=n)
        .map(|k| {
            let Some((t, river)) = walk[k] else { return false };
            if t < -6.0 || (k.saturating_sub(2)..=(k + 2).min(n)).any(|j| walk[j].is_some_and(|w| w.1 != river)) {
                return false;
            }
            let (f, m, b) = (face_at(k, t + if river { -1.0 } else { FACE_OUT_FT }), face_at(k, 0.5 * (t - FRONT_BACK_FT)), face_at(k, -FRONT_BACK_FT));
            !site.near_river(f, if river { 0.0 } else { 4.0 })
                && !site.near_river(b, 0.0)
                && [f, m, b].iter().all(|&q| !site.near_network_road(q, 6.0) && l.bridges.iter().all(|d| poly_gap(d, &[q]) >= 10.0))
                && islets.iter().all(|i| poly_gap(i, &[f]) >= 5.0)
        })
        .collect();
    let mut k = 0;
    while k <= n {
        if !ok[k] {
            k += 1;
            continue;
        }
        let k0 = k;
        while k <= n && ok[k] {
            k += 1;
        }
        if k - 1 - k0 >= 6 {
            run_front(site, s, l, &smp, &normal, &walk, (k0, k - 1), timber_share, covered);
        }
    }
}

/// One front along samples `k0..=k1` of a chain (`chain_fronts`): its face smoothed along the
/// water's edge, split into stone and timber stretches.
#[allow(clippy::too_many_arguments)]
fn run_front(site: &Site, s: &Settlement, l: &mut Layout, smp: &[Sample], normal: &[P], walk: &[Option<(f64, bool)>], (k0, k1): (usize, usize), timber_share: f64, covered: &mut [Vec<(f64, f64)>]) {
    let river = walk[k0].is_some_and(|w| w.1);
    let raw: Vec<f64> = (k0..=k1).map(|k| walk[k].map_or(0.0, |w| w.0)).collect();
    let m = raw.len();
    let smooth = |v: &[f64], r: i64| -> Vec<f64> {
        (0..m as i64)
            .map(|i| {
                let (mut sum, mut w) = (0.0, 0.0);
                for j in (i - r).max(0)..=(i + r).min(m as i64 - 1) {
                    let wj = (r + 1 - (j - i).abs()) as f64;
                    sum += v[j as usize] * wj;
                    w += wj;
                }
                sum / w
            })
            .collect()
    };
    let sm = smooth(&raw, 3);
    let f: Vec<f64> = if river {
        (0..m).map(|i| sm[i].min(raw[i]) - 1.0).collect()
    } else {
        let f1: Vec<f64> = (0..m).map(|i| f64::max(sm[i] + FACE_OUT_FT, raw[i] + 1.0)).collect();
        let f2 = smooth(&f1, 2);
        (0..m)
            .map(|i| {
                // (Eased back onto the shore at its ends: the quay meets the beach there.)
                let e = (i.min(m - 1 - i) as f64 / FRONT_TAPER as f64).min(1.0);
                let fi = f64::max(f2[i], raw[i] + 0.5);
                (raw[i] + (fi - raw[i]) * e).min(FRONT_REACH_FT + 5.0)
            })
            .collect()
    };
    // (Face and back in order along the shore: a sample where either would turn back is left out.)
    let mut kept: Vec<(usize, P, P)> = Vec::new();
    for i in 0..m {
        let k = k0 + i;
        let (face, back) = (add(smp[k].0, mul(normal[k], f[i])), sub(smp[k].0, mul(normal[k], FRONT_BACK_FT)));
        let tan = [-normal[k][1], normal[k][0]];
        if let Some(&(_, lf, lb)) = kept.last() {
            if dot(sub(face, lf), tan) < 1.0 || dot(sub(back, lb), tan) < 0.5 {
                continue;
            }
        }
        kept.push((i, face, back));
    }
    if kept.len() < 4 {
        return;
    }
    // (The stretch of each side it takes.)
    let mut last_side = usize::MAX;
    for &(i, ..) in &kept {
        let (_, side, at, _) = smp[k0 + i];
        if side != last_side {
            covered[side].push((at, at));
            last_side = side;
        } else if let Some(c) = covered[side].last_mut() {
            c.1 = c.1.max(at);
        }
    }
    let mi = kept[kept.len() / 2];
    let level = crate::wharf::level(
        &crate::world::Wharf {
            kind: crate::world::WharfKind::Quay,
            pts: vec![site.world(mi.2), site.world(mi.1)],
            width: f[mi.0] + FRONT_BACK_FT,
            material: crate::world::CrossingMaterial::Stone,
        },
        site.t0,
    );
    // (Stone and timber stretches of ~150 ft, by their places; the same kind next to it joins on.)
    let len = (kept.len() - 1) as f64 * FRONT_STEP_FT;
    let np = (crate::core::round(len / FRONT_PIECE_FT) as usize).clamp(1, kept.len() - 1);
    let cut = |j: usize| j * (kept.len() - 1) / np;
    let mut pieces: Vec<(usize, usize, bool)> = Vec::new();
    for j in 0..np {
        let (a, b) = (cut(j), cut(j + 1));
        let mid = kept[(a + b) / 2].1;
        let timber = unit(hash2(s.seed ^ 0x9a7e_0161, crate::core::round(mid[0]) as i64, crate::core::round(mid[1]) as i64)) < timber_share;
        match pieces.last_mut() {
            Some(p) if p.2 == timber => p.1 = b,
            _ => pieces.push((a, b, timber)),
        }
    }
    for (a, b, timber) in pieces {
        let mut pts: Vec<P> = kept[a..=b].iter().map(|q| q.1).collect();
        pts.extend(kept[a..=b].iter().rev().map(|q| q.2));
        l.quays.push(super::harbour::Quay { pts: pts.clone(), level, timber });
        l.plazas.push(Plaza { pts, kind: PlazaKind::Quay, spot: places::Spot::None });
    }
}
