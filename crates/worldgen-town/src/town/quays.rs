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
            if s.tier >= Tier::City && ward[i].is_some_and(|w| w != Ward::Farm) {
                // (Laid along the whole shore at once below, `fronts`; out on the right of a->b.
                // Roadmap 7: a ward whose lots don't front the water (a castle, temple close,
                // estate, barracks, park) carries the chain on with a narrow back, so the town's
                // shore keeps one quay.)
                let right = u[1] * out[0] - u[0] * out[1] > 0.0;
                let back = if ward[i].is_some_and(fronts_water) { FRONT_BACK_FT } else { NARROW_BACK_FT };
                sides.push(if right { Side { a, b, va, vb, out, back } } else { Side { a: b, b: a, va: vb, vb: va, out, back } });
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
    /// How far behind it the quay's back lies (ft): the lots' fronts, `FRONT_BACK_FT`, or
    /// `NARROW_BACK_FT` along a ward whose lots don't front the water.
    back: f64,
}

/// The farthest out (ft past the patch's side) a quay front is carried to find the water.
const FRONT_REACH_FT: f64 = 100.0;
/// The farthest back (ft behind the patch's side) a front looks for the water's edge where the
/// side lies out in the water.
const FRONT_INLAND_FT: f64 = 160.0;
/// The shore's samples along the sides (ft, about).
const FRONT_STEP_FT: f64 = 5.0;
/// The lots' fronts behind the patch's side (ft): the quay's back.
const FRONT_BACK_FT: f64 = 14.0;
/// The quay's back behind the side of a ward whose lots don't front the water (ft).
const NARROW_BACK_FT: f64 = 4.0;
/// The longest stretch (ft) with no water in reach a front is carried over, its face
/// interpolated between the water's edge either side (a small headland, a slipway).
const FRONT_BRIDGE_FT: f64 = 100.0;
/// The face past the water's edge (ft) on the sea or a lake; on a river it stops 1 ft short.
const FACE_OUT_FT: f64 = 3.0;
/// A quay's end eases its face back onto the shore over this many samples.
const FRONT_TAPER: usize = 3;
/// A stone or timber section of a quay, about (ft): a front shorter than 1.5 of them is one
/// material throughout.
const FRONT_PIECE_FT: f64 = 400.0;

/// The least depth of a quay from its face's water's edge to its back (ft).
const FRONT_MIN_DEPTH_FT: f64 = 20.0;

/// How far behind the patch's side a quay's back lies (ft) for a back `back` behind it and the
/// water's edge `t` out from it (behind it where negative): `back`, or `FRONT_MIN_DEPTH_FT`
/// behind the edge where the side lies out in the water.
fn back_depth(back: f64, t: f64) -> f64 {
    f64::max(back, FRONT_MIN_DEPTH_FT - t)
}

/// One shore sample (`chain_fronts`): its point, side, place along that side (ft), side's normal,
/// the quay's back behind it (ft).
type Sample = (P, usize, f64, P, f64);

/// P2 160: the quay fronts along a city's built shore (`sides`): the sides chained where they
/// meet, and along each chain the water's edge found every ~5 ft (`FRONT_STEP_FT`, walking out
/// along the shore's smoothed normal to where the water starts and holds, or back inland where
/// the side lies out in the water). A front is a run of samples with the water in reach (over
/// gaps of up to `FRONT_BRIDGE_FT`): its face traces the edge, smoothed and `FACE_OUT_FT` past it
/// (on a river 1 ft short of the channel), eased onto the shore at its ends; its back the lots'
/// fronts, at least `FRONT_MIN_DEPTH_FT` behind the edge. Streets meet it; bridges, islets and
/// river mouths end it. Stone or timber along it, a long one in sections of ~400 ft
/// (`timber_share` of them), each its own front and quay plaza. The sides of a chain with no
/// front keep the strip plaza where water lies off them.
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
    let mut fronted = vec![false; sides.len()];
    for chain in &chains {
        if chain_fronts(site, s, l, islets, sides, chain, timber_share, &mut covered) {
            for &i in chain {
                fronted[i] = true;
            }
        }
    }
    // (The rest of each side of a chain no front took: the strip where water lies off it. A
    // chain with a front keeps it alone, no square strips beside it.)
    for (i, sd) in sides.iter().enumerate() {
        let el = dist(sd.a, sd.b);
        if el < 1.0 || fronted[i] {
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
            let rest = vec![p0, p1, sub(p1, mul(sd.out, sd.back)), sub(p0, mul(sd.out, sd.back))];
            let by_water = cov.is_empty() || (0..=15).any(|j| site.wet(add(lerp(p0, p1, 0.5), mul(sd.out, 10.0 * j as f64))));
            if g1 - g0 >= 10.0 && by_water && !site.wet(centroid(&rest)) && !site.near_river(centroid(&rest), 10.0) {
                l.plazas.push(Plaza { pts: rest, kind: PlazaKind::Quay, spot: places::Spot::None });
            }
        }
    }
}

/// The fronts along one chain of sides (`fronts`); records the stretch of each side they take.
/// Roadmap 7: one front along the whole chain, but where a bridge, an islet or a river mouth
/// ends it, or more than `FRONT_BRIDGE_FT` of it has no water in reach; streets and roads meet
/// the quay, they don't cut it. Whether it laid any front.
#[allow(clippy::too_many_arguments)]
fn chain_fronts(site: &Site, s: &Settlement, l: &mut Layout, islets: &[Vec<P>], sides: &[Side], chain: &[usize], timber_share: f64, covered: &mut [Vec<(f64, f64)>]) -> bool {
    let lens: Vec<f64> = chain.iter().map(|&i| dist(sides[i].a, sides[i].b)).collect();
    let total: f64 = lens.iter().sum();
    if total < 30.0 {
        return false;
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
            (lerp(sd.a, sd.b, at / lens[j].max(1e-9)), chain[j], at, sd.out, sd.back)
        })
        .collect();
    // (The shore's normal and the back's depth: the sides' smoothed over 20 ft either way, so
    // corners round.)
    let reach = crate::core::ceil(20.0 / step) as i64;
    let mut normal: Vec<P> = Vec::with_capacity(n + 1);
    let mut back: Vec<f64> = Vec::with_capacity(n + 1);
    for k in 0..=n as i64 {
        let (mut v, mut d, mut w) = ([0.0, 0.0], 0.0, 0.0);
        for j in (k - reach).max(0)..=(k + reach).min(n as i64) {
            let wj = (reach + 1 - (j - k).abs()) as f64;
            v = add(v, mul(smp[j as usize].3, wj));
            d += smp[j as usize].4 * wj;
            w += wj;
        }
        normal.push(mul(v, 1.0 / dist(v, [0.0, 0.0]).max(1e-9)));
        back.push(d / w);
    }
    // (The water's edge along the normal: where water starts and holds 20 ft, or to the reach;
    // where the patch's side lies out in the water, back inland to where the ground starts and
    // holds 6 ft. And whether it is a river's.)
    let walk: Vec<Option<(f64, bool)>> = (0..=n)
        .map(|k| {
            let (p, nn) = (smp[k].0, normal[k]);
            let at = |t: f64| site.wet(add(p, mul(nn, t)));
            let mut t = -12.0;
            let (mut lo, mut hi) = if at(t) {
                while t > -FRONT_INLAND_FT && (at(t) || at(t - 6.0)) {
                    t -= 2.0;
                }
                if at(t) || at(t - 6.0) {
                    return None;
                }
                (t, t + 2.0)
            } else {
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
                (start? - 2.0, start?)
            };
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
    // (Where a front must end: by a bridge, an islet, a river crossing its back or meeting its
    // face, across a river's mouth, where the sea gives way to a river round a corner. Roadmap 7:
    // where it does so along a straight shore, it runs on, its face easing from the one's to the
    // other's.)
    let hard: Vec<bool> = (0..=n)
        .map(|k| {
            let Some((t, river)) = walk[k] else {
                let (p, b) = (smp[k].0, face_at(k, -back[k]));
                return (site.wet(face_at(k, -12.0)) && site.near_river(p, 0.0)) || site.near_river(b, 0.0) || l.bridges.iter().any(|d| poly_gap(d, &[b]) < 10.0);
            };
            let b = face_at(k, -back_depth(back[k], t));
            if site.near_river(b, 0.0) || l.bridges.iter().any(|d| poly_gap(d, &[b]) < 10.0) {
                return true;
            }
            // (Where the sea gives way to a river round a corner, the mouth's: a band carried round
            // it would fold.)
            let bent = dot(normal[k.saturating_sub(4)], normal[(k + 4).min(n)]) < 0.82;
            if bent && (k.saturating_sub(2)..=(k + 2).min(n)).any(|j| walk[j].is_some_and(|w| w.1 != river)) {
                return true;
            }
            let (f, m) = (face_at(k, t + if river { -1.0 } else { FACE_OUT_FT }), face_at(k, 0.5 * (t - back_depth(back[k], t))));
            site.near_river(f, if river { 0.0 } else { 4.0 }) || [f, m].iter().any(|&q| l.bridges.iter().any(|d| poly_gap(d, &[q]) < 10.0)) || islets.iter().any(|i| poly_gap(i, &[f]) < 5.0)
        })
        .collect();
    // (The water's edge where it is in reach.)
    let good = |k: usize| !hard[k] && walk[k].is_some();
    let span = crate::core::round(FRONT_BRIDGE_FT / step) as usize;
    let mut any = false;
    let mut k = 0;
    while k <= n {
        if !good(k) {
            k += 1;
            continue;
        }
        // (A run from good sample to good sample over gaps of at most `span`, the edge
        // interpolated across them.)
        let (k0, mut k1) = (k, k);
        let mut raw: Vec<f64> = vec![walk[k].map_or(0.0, |w| w.0)];
        let mut river: Vec<bool> = vec![walk[k].is_some_and(|w| w.1)];
        let mut j = k + 1;
        while j <= n && !hard[j] && j - k1 <= span {
            if good(j) {
                let (t0, t1) = (raw[raw.len() - 1], walk[j].map_or(0.0, |w| w.0));
                let gap = j - k1;
                let r0 = river[river.len() - 1];
                for g in 1..gap {
                    raw.push(t0 + (t1 - t0) * g as f64 / gap as f64);
                    river.push(r0);
                }
                raw.push(t1);
                river.push(walk[j].is_some_and(|w| w.1));
                k1 = j;
            }
            j += 1;
        }
        if k1 - k0 >= 7 {
            any |= run_front(site, s, l, &smp, &normal, &back, &raw, &river, (k0, k1), timber_share, covered);
        }
        k = k1 + 1;
    }
    any
}

/// One front along samples `k0..=k1` of a chain (`chain_fronts`): its face smoothed along the
/// water's edge, split into stone and timber stretches; `river` per sample, whether its water is
/// a river's (the face 1 ft short of the channel there, else `FACE_OUT_FT` past the edge, eased
/// from the one to the other over ~20 ft either way).
#[allow(clippy::too_many_arguments)]
fn run_front(site: &Site, s: &Settlement, l: &mut Layout, smp: &[Sample], normal: &[P], back_ft: &[f64], raw: &[f64], river: &[bool], (k0, k1): (usize, usize), timber_share: f64, covered: &mut [Vec<(f64, f64)>]) -> bool {
    let m = raw.len();
    debug_assert_eq!(m, k1 - k0 + 1);
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
    let sm = smooth(raw, 3);
    let on_river = |i: usize| sm[i].min(raw[i]) - 1.0;
    let f: Vec<f64> = if river.iter().all(|&r| r) {
        (0..m).map(on_river).collect()
    } else {
        let f1: Vec<f64> = (0..m).map(|i| f64::max(sm[i] + FACE_OUT_FT, raw[i] + 1.0)).collect();
        let f2 = smooth(&f1, 2);
        let share = smooth(&river.iter().map(|&r| if r { 1.0 } else { 0.0 }).collect::<Vec<f64>>(), 4);
        (0..m)
            .map(|i| {
                // (Eased back onto the shore at its ends: the quay meets the beach there.)
                let e = (i.min(m - 1 - i) as f64 / FRONT_TAPER as f64).min(1.0);
                let fi = f64::max(f2[i], raw[i] + 0.5);
                let sea = (raw[i] + (fi - raw[i]) * e).min(FRONT_REACH_FT + 5.0);
                let w = if river[i] { 1.0 } else { f64::min(1.0, 2.0 * share[i]) };
                sea + (on_river(i) - sea) * w
            })
            .collect()
    };
    // (Its back: the lots' fronts, or `FRONT_MIN_DEPTH_FT` behind the water's edge, smoothed.)
    let bk = smooth(&(0..m).map(|i| back_depth(back_ft[k0 + i], sm[i].min(raw[i]))).collect::<Vec<f64>>(), 3);
    // (The face in order along the shore: a sample where it would turn back is left out.)
    let face: Vec<P> = (0..m).map(|i| add(smp[k0 + i].0, mul(normal[k0 + i], f[i]))).collect();
    let mut idx: Vec<usize> = Vec::new();
    for i in 0..m {
        let k = k0 + i;
        let tan = [-normal[k][1], normal[k][0]];
        if idx.last().is_some_and(|&j| dot(sub(face[i], face[j]), tan) < 1.0) {
            continue;
        }
        // (Nor turn sharper than ~50 degrees on the last step, where the shore runs askew of the side.)
        if let [.., h, j] = idx[..] {
            let (u, v) = (sub(face[j], face[h]), sub(face[i], face[j]));
            if dot(u, v) < 0.6 * dist(u, [0.0, 0.0]) * dist(v, [0.0, 0.0]) {
                continue;
            }
        }
        idx.push(i);
    }
    let q = idx.len();
    if q < 4 {
        return false;
    }
    // (Roadmap 7: the back square behind the face, as deep as the lots' fronts lie behind it there,
    // smoothed, so the outline is an offset of the face and never askew of it or folded.)
    let inward: Vec<P> = (0..q)
        .map(|a| {
            let t = sub(face[idx[(a + 3).min(q - 1)]], face[idx[a.saturating_sub(3)]]);
            let tl = dist(t, [0.0, 0.0]).max(1e-9);
            let n = [-t[1] / tl, t[0] / tl];
            if dot(n, normal[k0 + idx[a]]) > 0.0 { mul(n, -1.0) } else { n }
        })
        .collect();
    let depth = smooth_list(
        &(0..q)
            .map(|a| {
                let i = idx[a];
                let back = sub(smp[k0 + i].0, mul(normal[k0 + i], bk[i]));
                dot(sub(back, face[i]), inward[a]).max(FRONT_MIN_DEPTH_FT)
            })
            .collect::<Vec<f64>>(),
        3,
    );
    // (The back in order too.)
    let mut kept: Vec<(usize, P, P)> = Vec::new();
    for a in 0..q {
        let (i, back) = (idx[a], add(face[idx[a]], mul(inward[a], depth[a])));
        let tan = [-inward[a][1], inward[a][0]];
        let tan = if dot(tan, [-normal[k0 + i][1], normal[k0 + i][0]]) < 0.0 { mul(tan, -1.0) } else { tan };
        if kept.last().is_some_and(|&(_, _, lb)| dot(sub(back, lb), tan) < 0.5) {
            continue;
        }
        kept.push((i, face[i], back));
    }
    if kept.len() < 4 {
        return false;
    }
    round_ends(&mut kept);
    // (Roadmap 7: none whose outline crosses itself, as round a river mouth's corner.)
    let outline: Vec<P> = kept.iter().map(|q| q.1).chain(kept.iter().rev().map(|q| q.2)).collect();
    if folds(&outline) {
        return false;
    }
    // (The stretch of each side it takes.)
    let mut last_side = usize::MAX;
    for &(i, ..) in &kept {
        let (_, side, at, ..) = smp[k0 + i];
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
            width: f[mi.0] + bk[mi.0],
            material: crate::world::CrossingMaterial::Stone,
            shore: false,
            piers: 0.0,
        },
        site.t0,
    );
    // (Roadmap 7: one material along it, or on a long front sections of ~400 ft, by their
    // places; the same kind next to it joins on, a change of kind a square joint.)
    let len = (kept.len() - 1) as f64 * FRONT_STEP_FT;
    let np = if len < 1.5 * FRONT_PIECE_FT { 1 } else { (crate::core::round(len / FRONT_PIECE_FT) as usize).clamp(1, kept.len() - 1) };
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
    true
}

/// Whether closed outline `p` crosses itself (two of its sides not next to each other cross).
fn folds(p: &[P]) -> bool {
    let n = p.len();
    let side = |a: P, b: P, c: P| cross(sub(b, a), sub(c, a));
    let boxes: Vec<[f64; 4]> = (0..n).map(|i| bbox(&[p[i], p[(i + 1) % n]])).collect();
    (0..n).any(|i| {
        (i + 2..n).any(|j| {
            let (bi, bj) = (boxes[i], boxes[j]);
            if (i == 0 && j == n - 1) || bi[2] < bj[0] || bj[2] < bi[0] || bi[3] < bj[1] || bj[3] < bi[1] {
                return false;
            }
            let (a, b, c, d) = (p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]);
            side(a, b, c) * side(a, b, d) < -1e-9 && side(c, d, a) * side(c, d, b) < -1e-9
        })
    })
}

/// How far along from each end a front's back corner rounds (ft, `round_ends`).
const END_ROUND_FT: f64 = 10.0;
/// The share of its width a front keeps at its very end (`round_ends`).
const END_KEEP: f64 = 0.35;

/// `v` smoothed with a tent of radius `r` samples.
fn smooth_list(v: &[f64], r: i64) -> Vec<f64> {
    let m = v.len() as i64;
    (0..m)
        .map(|i| {
            let (mut sum, mut w) = (0.0, 0.0);
            for j in (i - r).max(0)..=(i + r).min(m - 1) {
                let wj = (r + 1 - (j - i).abs()) as f64;
                sum += v[j as usize] * wj;
                w += wj;
            }
            sum / w
        })
        .collect()
}

/// A front's ends rounded off along the shore (`run_front`): over `END_ROUND_FT` from each end
/// its back closes toward the face in a quarter ellipse, keeping `END_KEEP` of the width at the
/// end, so the quay comes to a blunt, round-cornered end on the beach.
fn round_ends(kept: &mut [(usize, P, P)]) {
    let n = kept.len();
    let run: f64 = kept.windows(2).map(|w| dist(w[0].1, w[1].1)).sum();
    for end in 0..2 {
        let at = |j: usize| if end == 0 { j } else { n - 1 - j };
        // (Roadmap 7: a short round on the back corner, the end square across the shore, no
        // lens's point.)
        let reach = END_ROUND_FT.min(0.3 * run);
        let mut d = 0.0;
        for j in 0..n / 2 {
            if j > 0 {
                d += dist(kept[at(j - 1)].1, kept[at(j)].1);
            }
            if d >= reach {
                break;
            }
            let u = 1.0 - d / reach;
            let e = crate::core::sqrt(1.0 - u * u).max(END_KEEP);
            let (i, face, back) = kept[at(j)];
            kept[at(j)] = (i, face, add(face, mul(sub(back, face), e)));
        }
    }
}

/// How far behind a quay front's back its apron is carried at most to the houses (ft, `aprons`).
const APRON_FT: f64 = 60.0;

/// Roadmap 7: the paving behind each quay front carried on to the houses (or the street, a
/// square) behind it, where they stand within `APRON_FT` of its back: the lots' front ground on
/// the water is paved, no strip of beach between the quay and the town. Square to the back, as
/// deep as the nearest of its neighbours (no fingers between houses); none where open ground, a
/// park, a field, a close or a wall lies behind it. After the houses, so after those dropped off
/// the bands (`generate`). Yards: paved, nothing set out on them.
pub(super) fn aprons(site: &Site, l: &mut Layout) {
    let mut out: Vec<Vec<P>> = Vec::new();
    for q in &l.quays {
        let h = q.pts.len() / 2;
        if h < 2 {
            continue;
        }
        let bb = bbox(&q.pts);
        let near = |o: [f64; 4]| o[2] >= bb[0] - APRON_FT - 5.0 && o[0] <= bb[2] + APRON_FT + 5.0 && o[3] >= bb[1] - APRON_FT - 5.0 && o[1] <= bb[3] + APRON_FT + 5.0;
        let houses: Vec<(&Vec<P>, [f64; 4])> = l.buildings.iter().map(|b| (&b.poly, bbox(&b.poly))).filter(|(_, o)| near(*o)).collect();
        let paved: Vec<(&Vec<P>, [f64; 4])> = l.plazas.iter().filter(|p| !p.kind.is_lawn() && p.kind != PlazaKind::Bailey && p.pts != q.pts).map(|p| (&p.pts, bbox(&p.pts))).filter(|(_, o)| near(*o)).collect();
        let open: Vec<(&Vec<P>, [f64; 4])> = l.parks.iter().map(|p| &p.pts).chain(l.fields.iter()).chain(l.plazas.iter().filter(|p| p.kind.is_lawn() || p.kind == PlazaKind::Bailey).map(|p| &p.pts)).map(|p| (p, bbox(p))).filter(|(_, o)| near(*o)).collect();
        let inside = |set: &[(&Vec<P>, [f64; 4])], p: P| set.iter().any(|(poly, o)| p[0] >= o[0] && p[0] <= o[2] && p[1] >= o[1] && p[1] <= o[3] && geom::contains(poly, p));
        // (The stretches of street, road and wall that can come that near a point behind the
        // front, picked once: every point asked about is within `APRON_FT` of the front's box.)
        let reaches = |a: P, b: P, d: f64| near([a[0].min(b[0]) - d, a[1].min(b[1]) - d, a[0].max(b[0]) + d, a[1].max(b[1]) + d]);
        let ways: Vec<(P, P, f64)> = l.streets.iter().flat_map(|s| s.windows(2).map(|w| (w[0], w[1], 4.0))).chain(l.roads.iter().flat_map(|r| r.0.windows(2).map(|w| (w[0], w[1], 0.5 * r.2)))).filter(|&(a, b, d)| reaches(a, b, d)).collect();
        let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).filter(|&(a, b)| reaches(a, b, 8.0)).collect();
        let on_way = |p: P| ways.iter().any(|&(a, b, d)| seg_dist(p, a, b) < d);
        let walled = |p: P| walls.iter().any(|&(a, b)| seg_dist(p, a, b) < 8.0) || l.castles.iter().any(|&(c, r)| dist(c, p) < r + 8.0);
        // (How deep the apron goes behind each back point: to the first house, street or square,
        // or none.)
        let deep: Vec<(P, P, f64)> = (0..h)
            .map(|i| {
                let (f, b) = (q.pts[i], q.pts[2 * h - 1 - i]);
                let dd = dist(f, b);
                if dd < 1.0 {
                    return (b, [0.0, 0.0], 0.0);
                }
                let v = mul(sub(b, f), 1.0 / dd);
                let mut t = 1.0;
                while t <= APRON_FT {
                    let p = add(b, mul(v, t));
                    if inside(&open, p) || walled(p) || site.wet(p) || site.near_river(p, 4.0) {
                        return (b, v, 0.0);
                    }
                    if inside(&houses, p) {
                        return (b, v, t - 1.0);
                    }
                    if inside(&paved, p) || on_way(p) {
                        return (b, v, t);
                    }
                    t += 1.0;
                }
                (b, v, 0.0)
            })
            .collect();
        let e: Vec<f64> = (0..h).map(|i| if deep[i].2 <= 0.0 { 0.0 } else { (i.saturating_sub(1)..=(i + 1).min(h - 1)).map(|j| deep[j].2).filter(|&d| d > 0.0).fold(f64::INFINITY, f64::min) }).collect();
        // (In runs of back points it reaches 3 ft or more behind, each one plaza; a run whose
        // outline would cross itself (round a tight bend) is laid in quads.)
        let mut i = 0;
        while i < h {
            if e[i] < 3.0 {
                i += 1;
                continue;
            }
            let i0 = i;
            while i + 1 < h && e[i + 1] >= 3.0 {
                i += 1;
            }
            let i1 = i;
            i += 1;
            if i1 == i0 {
                continue;
            }
            let back: Vec<P> = (i0..=i1).map(|j| deep[j].0).collect();
            let far: Vec<P> = (i0..=i1).map(|j| add(deep[j].0, mul(deep[j].1, e[j]))).collect();
            let poly: Vec<P> = back.iter().copied().chain(far.iter().rev().copied()).collect();
            if !folds(&poly) {
                out.push(poly);
            } else {
                for j in 0..back.len() - 1 {
                    let quad = vec![back[j], back[j + 1], far[j + 1], far[j]];
                    if !folds(&quad) {
                        out.push(quad);
                    }
                }
            }
        }
    }
    l.plazas.extend(out.into_iter().map(|pts| Plaza { pts, kind: PlazaKind::Yard, spot: places::Spot::None }));
}

/// The farthest (ft) a street or lane is carried on to meet a quay front ahead of it (`lanes`).
const LANE_REACH_FT: f64 = 155.0;
/// A lane carried on to the quay (ft wide, `lanes`).
const LANE_FT: f64 = 9.0;

/// Roadmap 7: a street or lane whose end points at a quay front within `LANE_REACH_FT`, with no
/// other way, square, park, wall or water between, is carried on to the quay as a paved lane,
/// the lot houses in its way left off (none past a keep or a building of parts: that one stays
/// short). After the houses (`generate`).
pub(super) fn lanes(site: &Site, l: &mut Layout) {
    if l.quays.is_empty() {
        return;
    }
    // (The fronts' boxes: a point outside one, by a hair for rounding, is not on its front, and a
    // way's end farther than the reach from every one meets none.)
    let boxes: Vec<[f64; 4]> = l.quays.iter().map(|q| geom::grow(bbox(&q.pts), 1e-6)).collect();
    let within = |o: &[f64; 4], p: P, d: f64| p[0] >= o[0] - d && p[0] <= o[2] + d && p[1] >= o[1] - d && p[1] <= o[3] + d;
    let in_quay = |l: &Layout, p: P| l.quays.iter().zip(&boxes).any(|(q, o)| within(o, p, 0.0) && geom::contains(&q.pts, p));
    let ways: Vec<Vec<P>> = l.streets.iter().cloned().chain(l.roads.iter().filter(|r| r.1 >= 3).map(|r| r.0.clone())).collect();
    let mut laid: Vec<(P, P)> = Vec::new();
    for (wi, w) in ways.iter().enumerate() {
        if w.len() < 2 {
            continue;
        }
        for (e, p) in [(w[0], w[1]), (w[w.len() - 1], w[w.len() - 2])] {
            if !boxes.iter().any(|o| within(o, e, LANE_REACH_FT + 1.0)) {
                continue;
            }
            let d = dist(e, p);
            if d < 1.0 || l.quays.iter().any(|q| poly_gap(&q.pts, &[e]) <= 8.0) {
                continue;
            }
            let u = mul(sub(e, p), 1.0 / d);
            let at = |t: f64| add(e, mul(u, t));
            let Some(t1) = (2..=31).map(|j| 5.0 * j as f64).find(|&t| t <= LANE_REACH_FT && in_quay(l, at(t))) else { continue };
            // (Just into the quay: its back found to the foot.)
            let (mut lo, mut hi) = (t1 - 5.0, t1);
            for _ in 0..4 {
                let m = 0.5 * (lo + hi);
                if in_quay(l, at(m)) {
                    hi = m;
                } else {
                    lo = m;
                }
            }
            let end = at(hi + 1.0);
            let blocked = (1..crate::core::ceil(hi / 2.5) as usize).any(|j| {
                let t = 2.5 * j as f64;
                let q = at(t);
                (t >= 10.0 && ways.iter().enumerate().any(|(wj, w2)| wj != wi && w2.windows(2).any(|s| seg_dist(q, s[0], s[1]) < 4.0)))
                    || site.wet(q)
                    || site.near_river(q, 4.0)
                    || l.walls.iter().any(|ww| ww.windows(2).any(|s| seg_dist(q, s[0], s[1]) < 8.0))
                    || l.castles.iter().any(|&(c, r)| dist(c, q) < r + 8.0)
                    || l.parks.iter().any(|pk| geom::contains(&pk.pts, q))
                    || l.fields.iter().any(|f| geom::contains(f, q))
                    || l.plazas.iter().any(|pl| pl.kind != PlazaKind::Quay && pl.kind != PlazaKind::Yard && geom::contains(&pl.pts, q))
                    || l.buildings.iter().any(|b| (b.keep.is_some() || !b.parts.is_empty()) && poly_gap(&b.poly, &[q]) < 0.5 * LANE_FT)
            });
            // (Nor beside one already carried on.)
            if blocked || laid.iter().any(|&(a, b)| seg_dist(e, a, b) < 20.0 || seg_dist(end, a, b) < 20.0) {
                continue;
            }
            l.buildings.retain(|b| b.keep.is_some() || !b.parts.is_empty() || seg_poly_gap(e, end, &b.poly) >= 0.5 * LANE_FT);
            laid.push((e, end));
        }
    }
    for (a, b) in laid {
        l.roads.push((vec![a, b], 3, LANE_FT));
    }
}

/// How near segment `a`-`b` comes to convex polygon `poly` (0 where it crosses or lies inside).
fn seg_poly_gap(a: P, b: P, poly: &[P]) -> f64 {
    let n = poly.len();
    if geom::contains(poly, a) || geom::contains(poly, b) {
        return 0.0;
    }
    let mut g = f64::INFINITY;
    for i in 0..n {
        let (c, d) = (poly[i], poly[(i + 1) % n]);
        let s = |p: P, q: P, r: P| cross(sub(q, p), sub(r, p));
        if s(a, b, c) * s(a, b, d) < 0.0 && s(c, d, a) * s(c, d, b) < 0.0 {
            return 0.0;
        }
        g = g.min(seg_dist(c, a, b)).min(seg_dist(a, c, d)).min(seg_dist(b, c, d));
    }
    g
}
