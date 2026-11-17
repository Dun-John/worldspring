//! Junction squares: branches fitted and rejoined at shared ends, junctions closed.

use super::*;

/// How far (ft) a junction keeps from where a road crosses a river's meander belt, beyond
/// `WET_STRAY` times the belt's half width (the channel wanders within it, and a road pushed out
/// of it crosses where it runs through).
pub(super) const JUNCTION_WET: f64 = 0.5 * 5280.0;
pub(super) const WET_STRAY: f64 = 1.5;
/// Junctions closer than this (ft) along a road are merged.
const JUNCTION_GAP: f64 = 0.5 * 5280.0;
/// Two legs within 35° of straight make a through road.
pub(super) const THROUGH: f64 = -0.819;
/// Within 70°: a through road where no straighter pair can take the other roads' joins.
const BENT: f64 = -0.342;
/// How far a branch's join may slide along the through road, in its offset from it.
const SKEW_MAX: f64 = 2.5;
/// How far off the through road (cells) a branch's meeting best starts from (`fit_branch`).
const WIDE_OFF: f64 = 0.3;

/// What squaring junctions needs: settlements (centre, trim radius), the cell (ft), dry land and
/// the ground (ft), and where roads cross rivers' meander belts (with each belt's half width).
struct Square<'a> {
    towns: &'a [([f64; 2], f64)],
    cell: f64,
    dry: &'a dyn Fn(f64, f64) -> bool,
    ground: &'a dyn Fn([f64; 2]) -> f64,
    /// Where a line crosses rivers' meander belts (each with the belt's half width).
    crossings: &'a dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>,
    wet: Vec<([f64; 2], f64)>,
    /// Short branches from a settlement may be laid anew to take a junction clear of a river.
    relay: bool,
}

impl Square<'_> {
    /// Far enough from every river crossing.
    fn clear(&self, p: [f64; 2]) -> bool {
        self.wet.iter().all(|&(c, hw)| dist(c, p) >= JUNCTION_WET + WET_STRAY * hw)
    }
    /// Where a road end would count as a settlement's (`end_key`), with some room.
    fn in_town(&self, p: [f64; 2]) -> bool {
        self.towns.iter().any(|&(c, r)| dist(c, p) < 1.2 * r + 100.0)
    }
    /// A ramp from `z0` to `z1` along `curve` that the class can climb and that stays near the
    /// ground (the profile is fitted again later; this only keeps a new stretch off a ridge).
    fn ramp_ok(&self, curve: &[[f64; 2]], z0: f32, z1: f32, gmax: f64) -> bool {
        let s = arc(curve);
        let len = s[s.len() - 1].max(1.0);
        if ((z1 - z0).abs() as f64) > gmax * len + 5.0 {
            return false;
        }
        curve.iter().zip(&s).all(|(c, &u)| ((self.ground)(*c) - (z0 as f64 + (z1 - z0) as f64 * u / len)).abs() <= 25.0 + gmax * u.min(len - u))
    }
}

pub(super) fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// A road the other way round.
pub(super) fn flipped(r: &RoadPath) -> RoadPath {
    let (pts, z, wander) = (r.pts.iter().rev().copied().collect(), r.z.iter().rev().copied().collect(), r.wander.iter().rev().copied().collect());
    RoadPath { class: r.class, pts, z, wander, stroke: r.stroke, spine: r.spine }
}

/// A value per point (`s`: the line's `arc`) at `t` along it.
pub(super) fn val_at(v: &[f32], s: &[f64], t: f64) -> f32 {
    let k = s.partition_point(|&x| x < t).clamp(1, s.len() - 1);
    let f = ((t - s[k - 1]) / (s[k] - s[k - 1]).max(1e-9)).clamp(0.0, 1.0);
    v[k - 1] + (v[k] - v[k - 1]) * f as f32
}

/// A road cut at `cuts` (distance along it, the point there; in order): the pieces, each
/// starting and ending exactly on its cut points.
pub(super) fn split_road(r: &RoadPath, s: &[f64], cuts: &[(f64, [f64; 2])]) -> Vec<RoadPath> {
    let n = r.pts.len();
    let mut bounds: Vec<(f64, [f64; 2], f32, f32)> = vec![(0.0, r.pts[0], r.z[0], r.wander[0])];
    bounds.extend(cuts.iter().map(|&(t, q)| (t, q, val_at(&r.z, s, t), val_at(&r.wander, s, t))));
    bounds.push((s[n - 1], r.pts[n - 1], r.z[n - 1], r.wander[n - 1]));
    bounds
        .windows(2)
        .map(|b| {
            let (lo, hi) = (b[0], b[1]);
            let mid = (0..n).filter(|&k| s[k] > lo.0 + 5.0 && s[k] < hi.0 - 5.0);
            let pts = std::iter::once(lo.1).chain(mid.clone().map(|k| r.pts[k])).chain(std::iter::once(hi.1)).collect();
            let z = std::iter::once(lo.2).chain(mid.clone().map(|k| r.z[k])).chain(std::iter::once(hi.2)).collect();
            let wander = std::iter::once(lo.3).chain(mid.map(|k| r.wander[k])).chain(std::iter::once(hi.3)).collect();
            RoadPath { class: r.class, pts, z, wander, stroke: r.stroke, spine: r.spine }
        })
        .collect()
}

/// A branch's way in from `p` (leaving along `h`) to `q` on the road (`n`: the road's normal on
/// `p`'s side): a cubic to a point a little out from `q` (0.6 of `p`'s offset, at most 0.35 cell),
/// arriving square, then straight in. Both ends included, points ~60 ft apart or more.
fn square_curve(p: [f64; 2], h: [f64; 2], q: [f64; 2], n: [f64; 2], cell: f64) -> Vec<[f64; 2]> {
    let d = (0.6 * dot2(n, sub2(p, q))).min(0.35 * cell);
    let a = [q[0] + n[0] * d, q[1] + n[1] * d];
    let span = dist(p, a);
    let (c1, c2) = ([p[0] + h[0] * 0.4 * span, p[1] + h[1] * 0.4 * span], [a[0] + n[0] * 0.4 * span, a[1] + n[1] * 0.4 * span]);
    let step = (0.05 * cell).max(60.0);
    let m = (span / step).ceil().clamp(3.0, 30.0) as usize;
    let mut out: Vec<[f64; 2]> = (0..=m)
        .map(|i| {
            let t = i as f64 / m as f64;
            let u = 1.0 - t;
            let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            [w0 * p[0] + w1 * c1[0] + w2 * c2[0] + w3 * a[0], w0 * p[1] + w1 * c1[1] + w2 * c2[1] + w3 * a[1]]
        })
        .collect();
    let k = (d / step).ceil().max(1.0) as usize;
    out.extend((1..=k).map(|i| {
        let f = i as f64 / k as f64;
        [a[0] + (q[0] - a[0]) * f, a[1] + (q[1] - a[1]) * f]
    }));
    out
}

/// Where a branch meets the through road: `reach` ft along the branch (from the junction) it
/// leaves its line at `p` and curves in to `q`, `at` along the through road, arriving square.
#[derive(Clone)]
struct Fit {
    reach: f64,
    at: f64,
    q: [f64; 2],
    p: [f64; 2],
    /// From `p` to `q`.
    curve: Vec<[f64; 2]>,
    z_p: f32,
    /// On the left of the through road (as it runs).
    left: bool,
    clear: bool,
    /// From well off the road (`WIDE_OFF`).
    wide: bool,
}

/// How branch `b` (running out from the junction) best meets through road `t` (`ts`: its arc):
/// the nearest reach whose point falls on the road at least 0.15 cell off it, meeting it where it
/// falls (or slid along it, up to 1.3 cell and first towards where the branch comes from, to keep
/// clear of river crossings), or at `fixed` along it. Best first: clear of the through road's
/// river crossings (`JUNCTION_WET`), then out of their meander belts, met where it falls before
/// slid further than the branch's point stands off the road (its way in swings across); then
/// slid more than `SKEW_MAX` times that (it runs alongside, a lens of two roads); then in a belt;
/// then a shorter reach and a shorter slide. One from where the branch stands `WIDE_OFF` or more
/// off the road (and clear of the branch's own crossings) is taken over one from nearer it unless
/// it ranks worse than a slide: from nearer, at a narrow fork, the way in turns so short that the
/// branch still leaves at a slant. No meeting doubles back, crosses more rivers than the branch
/// did, enters water or a settlement, or climbs too steeply.
fn fit_branch(sq: &Square, t: &RoadPath, ts: &[f64], b: &RoadPath, fixed: Option<f64>, min_reach: f64) -> Option<Fit> {
    let cell = sq.cell;
    let bs = arc(&b.pts);
    let (blen, total) = (bs[bs.len() - 1], ts[ts.len() - 1]);
    let gmax = b.class.max_grade().min(t.class.max_grade());
    let t_wet = (sq.crossings)(&t.pts);
    // (Clear of where the through road crosses rivers, and of any crossing on the way in; a
    // branch's own crossing further out is no matter: a road leaving one along the bank.)
    let far = |q: [f64; 2], w: &[([f64; 2], f64)], m: f64| w.iter().all(|&(c, hw)| dist(c, q) >= m + WET_STRAY * hw);
    let mut best: Option<(u8, Fit)> = None;
    // (The best meeting from where the branch stands well off the road: one from nearer it, at a
    // narrow fork, turns in so short that it still leaves at a slant. Taken unless worse.)
    let mut wide: Option<(u8, Fit)> = None;
    // (Nor where it meets the road near where the branch itself crosses a river.)
    let mut b_wet: Option<Vec<([f64; 2], f64)>> = None;
    // (A short branch from a settlement to a junction at a river crossing may be laid anew from
    // near the settlement's edge, slid as far as it takes: nothing else would take it clear.)
    let most = if sq.relay && !sq.clear(b.pts[0]) && sq.in_town(b.pts[b.pts.len() - 1]) { blen } else { 0.5 * blen };
    // (A lesser road out to a settlement near the junction may swing in from further along it,
    // past its half: one bending off a better road at a narrow fork meets it square that way.)
    let end_in_town = b.class > t.class && sq.in_town(b.pts[b.pts.len() - 1]);
    let late: Vec<f64> = if end_in_town && most < blen { (1..=6).map(|k| blen - 0.05 * k as f64 * cell).filter(|&r| r > most && r > 0.4 * cell).rev().collect() } else { Vec::new() };
    for reach in [0.4, 0.55, 0.7, 0.9, 1.15, 1.5, 2.0, 2.5].iter().map(|f| (f * cell).max(min_reach)).take_while(|&r| r <= most).chain(late) {
        let p = point_at(&b.pts, &bs, reach);
        let (at0, off) = project(p, &t.pts, ts);
        if off < 0.15 * cell {
            continue;
        }
        let is_wide = off >= WIDE_OFF * cell;
        // Heading at `p` towards the junction.
        let h = unit2(sub2(point_at(&b.pts, &bs, reach - 0.03 * cell), p));
        let z_p = val_at(&b.z, &bs, reach);
        let mut wet_in: Option<usize> = None;
        // Slid towards where the branch comes from first, at each distance.
        let tan0 = unit2(sub2(point_at(&t.pts, ts, at0 + 40.0), point_at(&t.pts, ts, at0 - 40.0)));
        let back = if dot2(h, tan0) > 0.0 { -1.0 } else { 1.0 };
        let slides: &[f64] = if fixed.is_some() { &[0.0] } else { &[0.0, 0.15, -0.15, 0.3, -0.3, 0.5, -0.5, 0.75, -0.75, 1.0, -1.0, 1.3, -1.3] };
        for &d in slides {
            let at = fixed.unwrap_or(at0 + back * d * cell);
            if at < 0.15 * cell || at > total - 0.15 * cell {
                continue;
            }
            let q = point_at(&t.pts, ts, at);
            let slid = if fixed.is_none() { (at - at0).abs() / off } else { 0.0 };
            let rank = |clear: bool, belt: bool| match (slid > SKEW_MAX && most < blen, clear, belt) {
                (false, true, _) => (slid > 1.0) as u8,
                (false, false, true) => 2 + (slid > 1.0) as u8,
                (true, true, _) => 4,
                (true, false, true) => 5,
                _ => 6 + (slid > 1.0) as u8 + (slid > SKEW_MAX) as u8,
            };
            let r0 = rank(far(q, &t_wet, JUNCTION_WET), far(q, &t_wet, 300.0));
            if best.as_ref().is_some_and(|b| b.0 <= r0) && (!is_wide || wide.as_ref().is_some_and(|b| b.0 <= r0)) || sq.in_town(q) {
                continue;
            }
            let tan = unit2(sub2(point_at(&t.pts, ts, at + 40.0), point_at(&t.pts, ts, at - 40.0)));
            let left = tan[0] * (p[1] - q[1]) - tan[1] * (p[0] - q[0]) > 0.0;
            let n = if left { [-tan[1], tan[0]] } else { [tan[1], -tan[0]] };
            if dot2(unit2(sub2(q, p)), h) < -0.1 || dot2(n, sub2(p, q)) < 0.12 * cell {
                continue;
            }
            let curve = square_curve(p, h, q, n, cell);
            let k = curve.len() - 1;
            // Dry, out of settlements, and on its own side of the road until it meets it.
            if !curve.iter().all(|c| (sq.dry)(c[0], c[1]) && !sq.in_town(*c)) || !curve[..k - 1].iter().all(|c| dot2(n, sub2(*c, q)) > 5.0) {
                continue;
            }
            // (Laid anew, it may not cross the road it meets on the way: the road can bend.)
            if most == blen && curve[..k - 1].windows(2).any(|w| crosses(w[0], w[1], &t.pts)) {
                continue;
            }
            if !sq.ramp_ok(&curve, z_p, val_at(&t.z, ts, at), gmax) {
                continue;
            }
            // (No more river crossings than the branch had from `p` in: a way in across a river
            // that its line keeps on one side of puts the branch over it and back.)
            let wet_in = *wet_in.get_or_insert_with(|| (sq.crossings)(&sub_line(&b.pts, &bs, 0.0, reach)).len());
            let mut wet = (sq.crossings)(&curve);
            if wet.len() > if most < blen { wet_in } else { 0 } {
                continue;
            }
            wet.extend_from_slice(&t_wet);
            let (clear, belt) = (far(q, &wet, JUNCTION_WET), far(q, &wet, 300.0));
            let r = rank(clear, belt);
            let is_wide = is_wide && far(q, b_wet.get_or_insert_with(|| (sq.crossings)(&b.pts)), JUNCTION_WET);
            let fit = Fit { reach, at, q, p, curve, z_p, left, clear, wide: is_wide };
            if is_wide && wide.as_ref().is_none_or(|b| r < b.0) {
                if r == 0 {
                    return Some(fit);
                }
                wide = Some((r, fit.clone()));
            }
            if best.as_ref().is_none_or(|b| r < b.0) {
                best = Some((r, fit));
            }
        }
    }
    match (wide, best) {
        (Some(w), Some(b)) if w.0 <= b.0.max(1) => Some(w.1),
        (_, b) => b.map(|b| b.1),
    }
}

/// The branch with its end from `fit.p` replaced by the curve in to `fit.q` (running out from
/// the junction, like `b`), the level easing from `z_q` there to the branch's at `p`.
fn rejoined(b: &RoadPath, fit: &Fit, z_q: f32) -> RoadPath {
    let bs = arc(&b.pts);
    let k = fit.curve.len() - 1;
    let mut pts: Vec<[f64; 2]> = fit.curve.iter().rev().copied().collect();
    (pts[0], pts[k]) = (fit.q, fit.p);
    let mut z: Vec<f32> = (0..=k).map(|i| z_q + (fit.z_p - z_q) * i as f32 / k as f32).collect();
    let mut wander = vec![0.0f32; k + 1];
    let rest = (0..b.pts.len()).filter(|&i| bs[i] > fit.reach + 5.0);
    pts.extend(rest.clone().map(|i| b.pts[i]));
    z.extend(rest.clone().map(|i| b.z[i]));
    wander.extend(rest.map(|i| b.wander[i]));
    RoadPath { class: b.class, pts, z, wander, stroke: b.stroke, spine: b.spine }
}

/// Road ends by point (exact): (road, at its start).
fn ends_by_point(roads: &[RoadPath]) -> std::collections::BTreeMap<(u64, u64), Vec<(usize, bool)>> {
    let key = |p: [f64; 2]| (p[0].to_bits(), p[1].to_bits());
    let mut ends: std::collections::BTreeMap<(u64, u64), Vec<(usize, bool)>> = Default::default();
    for (ri, r) in roads.iter().enumerate() {
        if r.pts.len() >= 2 {
            ends.entry(key(r.pts[0])).or_default().push((ri, true));
            ends.entry(key(r.pts[r.pts.len() - 1])).or_default().push((ri, false));
        }
    }
    ends
}

/// `ends_by_point` kept up to date as roads change: only the roads whose end points moved are
/// taken out of the map and put back (the same map a rebuild gives).
struct EndIndex {
    keys: Vec<Option<((u64, u64), (u64, u64))>>,
    map: std::collections::BTreeMap<(u64, u64), Vec<(usize, bool)>>,
}

impl EndIndex {
    fn new(roads: &[RoadPath]) -> EndIndex {
        EndIndex { keys: roads.iter().map(Self::road_keys).collect(), map: ends_by_point(roads) }
    }

    fn road_keys(r: &RoadPath) -> Option<((u64, u64), (u64, u64))> {
        let key = |p: [f64; 2]| (p[0].to_bits(), p[1].to_bits());
        (r.pts.len() >= 2).then(|| (key(r.pts[0]), key(r.pts[r.pts.len() - 1])))
    }

    fn refresh(&mut self, roads: &[RoadPath]) {
        if roads.len() < self.keys.len() {
            *self = EndIndex::new(roads);
            return;
        }
        for ri in 0..roads.len() {
            let new = Self::road_keys(&roads[ri]);
            if ri < self.keys.len() && self.keys[ri] == new {
                continue;
            }
            if ri < self.keys.len() {
                if let Some((a, b)) = self.keys[ri] {
                    self.take(a, ri);
                    self.take(b, ri);
                }
                self.keys[ri] = new;
            } else {
                self.keys.push(new);
            }
            if let Some((a, b)) = new {
                self.put(a, ri, true);
                self.put(b, ri, false);
            }
        }
    }

    fn take(&mut self, k: (u64, u64), ri: usize) {
        if let Some(l) = self.map.get_mut(&k) {
            l.retain(|e| e.0 != ri);
            if l.is_empty() {
                self.map.remove(&k);
            }
        }
    }

    fn put(&mut self, k: (u64, u64), ri: usize, start: bool) {
        let l = self.map.entry(k).or_default();
        let at = l.partition_point(|e| (e.0, !e.1) < (ri, !start));
        l.insert(at, (ri, start));
    }
}

/// Junctions away from settlements become the joins of real roads. Grid routes meet at a cell,
/// often at a sharp V (two roads leaving a few tens of degrees apart, so travel between them
/// doubles back), and roads sharing a bridge part right on the bank. At each junction:
/// - the straightest pair of roads (within 35° of straight; the better class first) is the
///   through road: one line, its bend at the junction rounded (`fillet`), otherwise as it was
///   (but a pair as near straight that every branch meets from well off it comes first);
/// - every other road (a branch) leaves its line some way out and curves in to meet the through
///   road square, where its point there falls on it (`fit_branch`): the join slides along the
///   through road towards where the branch comes from, so nothing runs alongside and doubles
///   back. Joins are kept `JUNCTION_WET` from river crossings (sliding further, or reaching
///   further out along the branch), so roads sharing a bridge stay one road off it;
/// - two branches meeting it on opposite sides within `JUNCTION_GAP` of each other meet it at one
///   point (a crossroads); two on one side that close join each other further out (the lesser
///   ends on the greater, and that junction is squared in its turn);
/// - where a branch cannot meet the straightest pair, the next pair takes it, then pairs bending
///   up to 70°; at a junction standing in a meander belt, pairs crossing the river there come
///   last (the road along the bank runs on, the one crossing leaves it);
/// - where no pair can (equal roads parting in open ground) the fork stays.
///
/// Then junctions closer than `JUNCTION_GAP` along a road are merged into one (and squared again).
/// Points in ft; nothing is reshaped within a settlement's trim radius, over water or where the
/// new stretch could not keep to its grade (levels ease in as the profile is fitted later).
pub(super) fn square_junctions(
    roads: &mut Vec<RoadPath>,
    towns: &[([f64; 2], f64)],
    cell: f64,
    dry: &dyn Fn(f64, f64) -> bool,
    ground: &dyn Fn([f64; 2]) -> f64,
    crossings: &dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>,
    only: Option<&[[f64; 2]]>,
) {
    // (A road's crossings are looked up again only where its line changed.)
    let mut hits: Vec<(Vec<[f64; 2]>, Vec<([f64; 2], f64)>)> = Vec::new();
    let mut river_hits = |roads: &[RoadPath]| -> Vec<([f64; 2], f64)> {
        let mut out = Vec::new();
        let mut next = Vec::with_capacity(roads.len());
        for (i, r) in roads.iter().enumerate() {
            let entry = match hits.get_mut(i) {
                Some(e) if e.0 == r.pts => std::mem::take(e),
                _ => (r.pts.clone(), crossings(&r.pts)),
            };
            out.extend_from_slice(&entry.1);
            next.push(entry);
        }
        hits = next;
        out
    };
    unshare(roads, towns, ground, dry, crossings, true);
    let mut sq = Square { towns, cell, dry, ground, crossings, wet: river_hits(roads), relay: only.is_some() };
    // Points already squared (or left as they are) are not looked at again; with `only`, every
    // junction but those.
    let mut settled: std::collections::BTreeSet<(u64, u64)> = Default::default();
    if let Some(only) = only {
        settled.extend(ends_by_point(roads).into_keys().filter(|k| !only.iter().any(|q| (q[0].to_bits(), q[1].to_bits()) == *k)));
    }
    let mut index = EndIndex::new(roads);
    for round in 0..4 {
        index.refresh(roads);
        loop {
            let Some((k, list)) = index.map.iter().find(|(k, l)| l.len() >= 3 && !settled.contains(k)).map(|(k, l)| (*k, l.clone())) else { break };
            settled.insert(k);
            let j = [f64::from_bits(k.0), f64::from_bits(k.1)];
            for q in square_at(roads, j, &list, &sq) {
                settled.insert((q[0].to_bits(), q[1].to_bits()));
            }
            index.refresh(roads);
        }
        if round == 3 || !close_junctions(roads, &sq) {
            break;
        }
        sq.wet = river_hits(roads);
    }
}

/// Two roads leaving a junction (away from settlements) along one line, side by side (the grid
/// routed both along the same cells), become one road out to where they part: the better one
/// (class, then length) carries both, its first stretch taking the better class, and the other
/// leaves it there (that junction is squared in its turn). So does one that wanders off first
/// and comes back to run along the other (two roads down one valley from a pass, each with its
/// own switchbacks); of only two roads there, the stretch to where they part goes. One climbing
/// beside the other out of the junction (a switchback's leg above the other's line) rides it
/// instead, leaving it where a straight ramp in its grade reaches its own line further on; before
/// squaring (`mixed`) a lesser one beside a better one does too (else squaring takes the lesser
/// one's line through and bends the better one to it, a hook). One pair at a time, at most 200.
pub(super) fn unshare(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], ground: &dyn Fn([f64; 2]) -> f64, dry: &dyn Fn(f64, f64) -> bool, crossings: &dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>, mixed: bool) {
    const TOL: f64 = 150.0;
    const STEP: f64 = 50.0;
    const LATE: f64 = 4.0 * 5280.0;
    const BESIDE: f64 = 1_900.0;
    let mut tried: std::collections::BTreeSet<(u64, u64)> = Default::default();
    for _ in 0..200 {
        let ends = ends_by_point(roads);
        let mut done = false;
        for (k, list) in &ends {
            let j = [f64::from_bits(k.0), f64::from_bits(k.1)];
            if list.len() < 2 || tried.contains(k) || towns.iter().any(|&(t, r)| dist(t, j) < r) || list.iter().any(|e| roads[e.0].stroke.is_some()) {
                continue;
            }
            // (Two ends at a settlement's edge are its ways in, not a spike.)
            if list.len() == 2 && towns.iter().any(|&(t, r)| dist(t, j) < 2.0 * r + 1_000.0) {
                continue;
            }
            let mut ids: Vec<usize> = list.iter().map(|e| e.0).collect();
            ids.sort_unstable();
            ids.dedup();
            if ids.len() != list.len() {
                continue;
            }
            let legs: Vec<RoadPath> = list.iter().map(|&(ri, start)| if start { roads[ri].clone() } else { flipped(&roads[ri]) }).collect();
            let arcs: Vec<Vec<f64>> = legs.iter().map(|l| arc(&l.pts)).collect();
            let mut found = None;
            'pairs: for x in 0..legs.len() {
                for y in x + 1..legs.len() {
                    let key = |i: usize| (legs[i].class, !legs[i].spine, -arcs[i][arcs[i].len() - 1]);
                    let (c, o) = if key(x) <= key(y) { (x, y) } else { (y, x) };
                    let (os, cs) = (&arcs[o], &arcs[c]);
                    let (olen, clen) = (os[os.len() - 1], cs[cs.len() - 1]);
                    // (One that wanders off first, a loop or a switchback, and comes back to run
                    // along the other a good way from near the junction is laid on it all the same.)
                    let head = sub_line(&legs[c].pts, cs, 0.0, clen.min(2.0 * LATE + 5_000.0));
                    let head_near = NearLine::new(&head, TOL, 4.0 * TOL);
                    let near = |t: f64| head_near.near(point_at(&legs[o].pts, os, t));
                    let mut shared = 0.0;
                    while shared + STEP < olen && near(shared + STEP) {
                        shared += STEP;
                    }
                    if shared < 300.0 {
                        let mut from = shared + STEP;
                        while from < LATE && from + STEP < olen && !near(from) {
                            from += STEP;
                        }
                        let mut end = from;
                        while from < LATE && end + STEP < olen && near(end + STEP) {
                            end += STEP;
                        }
                        // (Not where the other's way there is much the longer.)
                        let via = project(point_at(&legs[o].pts, os, from), &legs[c].pts, cs).0;
                        if end - from >= 1_500.0 && via <= 1.5 * from + 500.0 {
                            shared = end;
                        }
                    }
                    // (Beside it out of the junction, a fine step or so off and a good way: rides
                    // it to the end of that, then leaves it on a ramp to its own line.)
                    if shared < 300.0 && (legs[o].class == legs[c].class || mixed) {
                        let beside = |t: f64| near_line(point_at(&legs[o].pts, os, t), &head, BESIDE);
                        let mut end = 0.0;
                        while end + STEP < olen && end < LATE && beside(end + STEP) {
                            end += STEP;
                        }
                        let at = project(point_at(&legs[o].pts, os, end), &legs[c].pts, cs).0;
                        let q = point_at(&legs[c].pts, cs, at);
                        // (Its junction where they part clear of river crossings.)
                        let clear = || crossings(&head).iter().chain(crossings(&legs[o].pts).iter()).all(|&(x, hw)| dist(x, q) >= JUNCTION_WET + WET_STRAY * hw);
                        if end >= 3_000.0 && at > 200.0 && at < clen - 300.0 && clear() {
                            let zq = val_at(&legs[c].z, cs, at) as f64;
                            let gmax = 0.85 * legs[o].class.max_grade();
                            let mut t = end + 2.0 * STEP;
                            while t < (end + 3.0 * 5280.0).min(olen - 300.0) {
                                let p = point_at(&legs[o].pts, os, t);
                                let (d, zp) = (dist(q, p), val_at(&legs[o].z, os, t) as f64);
                                let n = (d / 100.0).ceil().max(2.0) as usize;
                                let ramp: Vec<[f64; 2]> = (0..=n).map(|i| [q[0] + (p[0] - q[0]) * i as f64 / n as f64, q[1] + (p[1] - q[1]) * i as f64 / n as f64]).collect();
                                let fits = d > 600.0
                                    && (zp - zq).abs() <= gmax * d
                                    && at + d <= 1.6 * t + 1_000.0
                                    && ramp.iter().enumerate().all(|(i, r)| {
                                        let u = d * i as f64 / n as f64;
                                        dry(r[0], r[1]) && (ground(*r) - (zq + (zp - zq) * u / d)).abs() <= 25.0 + legs[o].class.max_grade() * u.min(d - u)
                                    })
                                    && !ramp[1..n].windows(2).any(|w| crosses(w[0], w[1], &legs[c].pts));
                                if fits {
                                    let (z0, z1) = (zq as f32, zp as f32);
                                    found = Some((c, o, t, at, ramp[1..n].iter().enumerate().map(|(i, r)| (*r, z0 + (z1 - z0) * (i + 1) as f32 / n as f32)).collect()));
                                    break 'pairs;
                                }
                                t += 2.0 * STEP;
                            }
                        }
                    }
                            // (One along the other all the way keeps only its last bit, to where it ends.)
                    if shared < 300.0 {
                        continue;
                    }
                    let at = project(point_at(&legs[o].pts, os, shared), &legs[c].pts, cs).0;
                    if at > 200.0 && at < clen - 300.0 {
                        found = Some((c, o, shared, at, Vec::new()));
                        break 'pairs;
                    }
                }
            }
            tried.insert(*k);
            let Some((c, o, shared, at, ramp)) = found else { continue };
            let (lc, lo) = (&legs[c], &legs[o]);
            let q = point_at(&lc.pts, &arcs[c], at);
            let mut pieces = split_road(lc, &arcs[c], &[(at, q)]);
            pieces[0].class = lc.class.min(lo.class);
            // (Two roads only, out and back along one line: the stretch to where they part
            // leads nowhere and goes.)
            if list.len() == 2 {
                pieces.remove(0);
            }
            let os = &arcs[o];
            let rest: Vec<usize> = (0..lo.pts.len()).filter(|&i| os[i] > shared + 5.0).collect();
            let mut pts = vec![q];
            let mut z = vec![val_at(&lc.z, &arcs[c], at)];
            let mut wander = vec![0.0f32];
            pts.extend(ramp.iter().map(|r| r.0));
            z.extend(ramp.iter().map(|r| r.1));
            wander.extend(ramp.iter().map(|_| 0.0f32));
            pts.extend(rest.iter().map(|&i| lo.pts[i]));
            z.extend(rest.iter().map(|&i| lo.z[i]));
            wander.extend(rest.iter().map(|&i| lo.wander[i]));
            pieces.push(RoadPath { class: lo.class, pts, z, wander, stroke: None, spine: lo.spine });
            if pieces.iter().any(|p| p.pts.len() < 2 || road_len(p) < 1.0) {
                continue;
            }
            let mut gone = vec![list[c].0, list[o].0];
            gone.sort_unstable_by(|x, y| y.cmp(x));
            for g in gone {
                roads.remove(g);
            }
            roads.extend(pieces);
            done = true;
            break;
        }
        if !done {
            return;
        }
    }
}

/// Squares one junction (`square_junctions`); returns the points branches now meet the through
/// road at (none where it is left as it was).
fn square_at(roads: &mut Vec<RoadPath>, j: [f64; 2], list: &[(usize, bool)], sq: &Square) -> Vec<[f64; 2]> {
    let cell = sq.cell;
    let mut ids: Vec<usize> = list.iter().map(|e| e.0).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() != list.len() || list.iter().any(|e| roads[e.0].stroke.is_some()) || sq.towns.iter().any(|&(t, r)| dist(t, j) < r) {
        return Vec::new();
    }
    let legs: Vec<RoadPath> = list.iter().map(|&(ri, start)| if start { roads[ri].clone() } else { flipped(&roads[ri]) }).collect();
    let dir: Vec<[f64; 2]> = legs
        .iter()
        .map(|l| {
            let s = arc(&l.pts);
            unit2(sub2(point_at(&l.pts, &s, (0.4 * cell).min(0.45 * s[s.len() - 1])), j))
        })
        .collect();
    // The through road: of pairs near straight, the best class, then the straightest; where
    // a branch cannot meet it, the next, then pairs bending up to 70° (the straightest first).
    // A junction standing in a river's meander belt: pairs crossing it there come last (a road
    // along the bank keeps on through, the one that crosses leaves it).
    let wet: Vec<bool> = legs
        .iter()
        .map(|l| {
            let s = arc(&l.pts);
            (sq.crossings)(&sub_line(&l.pts, &s, 0.0, (0.5 * cell).min(s[s.len() - 1]))).iter().any(|&(c, hw)| dist(c, j) < WET_STRAY * hw + 300.0)
        })
        .collect();
    let mut pairs: Vec<((u8, u8, RoadClass, RoadClass, f64), usize, usize)> = Vec::new();
    for a in 0..legs.len() {
        for b in a + 1..legs.len() {
            let c = dot2(dir[a], dir[b]);
            // (A realm's track through: its villages' spurs leave it, whatever lines up with a spur.)
            let w = (wet[a] || wet[b]) as u8 + 2 * !(legs[a].spine && legs[b].spine) as u8;
            if c <= THROUGH {
                pairs.push(((w, 0, legs[a].class.max(legs[b].class), legs[a].class.min(legs[b].class), c), a, b));
            } else if c <= BENT {
                pairs.push(((w, 1, RoadClass::KingsRoad, RoadClass::KingsRoad, c), a, b));
            }
        }
    }
    pairs.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(Ordering::Equal).then((x.1, x.2).cmp(&(y.1, y.2))));
    // (Of pairs as near straight, one its branches meet from well off it, square, comes first:
    // at a narrow fork the two roads close together are the better through road.)
    let mut first: Option<(usize, usize, RoadPath, Vec<f64>, Vec<Fit>)> = None;
    let mut group = None;
    let found = pairs.iter().find_map(|&(key, a, b)| {
        if group.is_some_and(|g| g != (key.0, key.1)) {
            return None;
        }
        let mut t = flipped(&legs[a]);
        let joint = t.pts.len() - 1;
        t.pts.extend_from_slice(&legs[b].pts[1..]);
        t.z.extend_from_slice(&legs[b].z[1..]);
        t.wander.extend_from_slice(&legs[b].wander[1..]);
        t.class = legs[a].class.min(legs[b].class);
        t.spine = legs[a].spine && legs[b].spine;
        t.wander[joint] = 0.0;
        fillet(&mut t, joint, cell, sq.dry);
        let ts = arc(&t.pts);
        let fits: Option<Vec<Fit>> = (0..legs.len()).filter(|&i| i != a && i != b).map(|i| fit_branch(sq, &t, &ts, &legs[i], None, 0.0)).collect();
        let fits = fits?;
        // (Not where that makes a better road a branch: it would leave its own line square
        // and turn back to it, a step; the lesser one meets it as it can.)
        let worse = legs[a].class.max(legs[b].class);
        if fits.iter().all(|f| f.wide && f.clear) && (0..legs.len()).filter(|&i| i != a && i != b).all(|i| legs[i].class >= worse) {
            return Some((a, b, t, ts, fits));
        }
        if first.is_none() {
            group = Some((key.0, key.1));
            first = Some((a, b, t, ts, fits));
        }
        None
    });
    let Some((a, b, t, ts, mut fits)) = found.or(first) else { return Vec::new() };
    let at_j = project(j, &t.pts, &ts).0;
    let branches: Vec<usize> = (0..legs.len()).filter(|&i| i != a && i != b).collect();
    // Branches near each other: opposite sides meet at one point, one side join further out.
    // (`feeds[i]`: branch i ends on branch feeds[i] at that one's `p`, which splits it.)
    let mut feeds: Vec<Option<usize>> = vec![None; fits.len()];
    let mut order: Vec<usize> = (0..fits.len()).collect();
    order.sort_by(|&x, &y| fits[x].at.total_cmp(&fits[y].at).then(x.cmp(&y)));
    for w in 0..order.len().saturating_sub(1) {
        let (x, y) = (order[w], order[w + 1]);
        if feeds[x].is_some() || feeds[y].is_some() || (fits[x].at - fits[y].at).abs() >= JUNCTION_GAP || fits[x].at == fits[y].at {
            continue;
        }
        let (bx, by) = (&legs[branches[x]], &legs[branches[y]]);
        if fits[x].left != fits[y].left {
            // The better road keeps its join; equal ones meet halfway.
            let at = match bx.class.cmp(&by.class) {
                Ordering::Less => fits[x].at,
                Ordering::Greater => fits[y].at,
                Ordering::Equal => 0.5 * (fits[x].at + fits[y].at),
            };
            if let (Some(fx), Some(fy)) = (fit_branch(sq, &t, &ts, bx, Some(at), fits[x].reach), fit_branch(sq, &t, &ts, by, Some(at), fits[y].reach)) {
                if fx.left != fy.left {
                    (fits[x], fits[y]) = (fx, fy);
                    continue;
                }
            }
        } else if same_side(sq, &t, &ts, &legs, &branches, &mut fits, &mut feeds, x, y) {
            continue;
        }
        // Else the lesser (class, then shorter) meets the road far enough from the other.
        let (keep, mv) = if (bx.class, road_len(bx)) <= (by.class, road_len(by)) { (x, y) } else { (y, x) };
        // (On the side the branch comes from.)
        let from = project(fits[mv].p, &t.pts, &ts).0;
        let away = if from >= fits[keep].at { 1.0 } else { -1.0 };
        if let Some(f) = fit_branch(sq, &t, &ts, &legs[branches[mv]], Some(fits[keep].at + away * (JUNCTION_GAP + 0.05 * cell)), 0.0) {
            if f.clear || !fits[mv].clear {
                fits[mv] = f;
            }
        }
    }
    // The through road cut where branches meet it.
    let mut cuts: Vec<(f64, [f64; 2])> = fits.iter().zip(&feeds).filter(|(_, f)| f.is_none()).map(|(f, _)| (f.at, f.q)).collect();
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
    cuts.dedup_by(|x, y| x.0 == y.0);
    let mut pieces = split_road(&t, &ts, &cuts);
    // Each piece's class: the road it mostly was, and the branches' whose traffic now runs along
    // most of it.
    let mut lo = 0.0f64;
    for (k, p) in pieces.iter_mut().enumerate() {
        let hi = if k < cuts.len() { cuts[k].0 } else { ts[ts.len() - 1] };
        let over = |x: f64, y: f64| (hi.min(x.max(y)) - lo.max(x.min(y))).max(0.0);
        let mut class = if over(0.0, at_j) >= 0.5 * (hi - lo) { legs[a].class } else { legs[b].class };
        for (i, f) in fits.iter().enumerate() {
            if feeds[i].is_none() && over(f.at, at_j) > 0.5 * (hi - lo) {
                class = class.min(legs[branches[i]].class);
            }
        }
        p.class = class;
        lo = hi;
    }
    let mut out: Vec<RoadPath> = pieces;
    let mut met = Vec::new();
    for (i, f) in fits.iter().enumerate() {
        let leg = &legs[branches[i]];
        match feeds[i] {
            Some(_) => {
                let z_q = { let m = &legs[branches[feeds[i].unwrap()]]; val_at(&m.z, &arc(&m.pts), fits[feeds[i].unwrap()].reach) };
                out.push(rejoined(leg, f, z_q));
            }
            None => {
                let z_q = val_at(&t.z, &ts, f.at);
                let r = rejoined(leg, f, z_q);
                met.push(f.q);
                // A branch another feeds into is cut where that one meets it.
                if let Some(less) = feeds.iter().position(|x| *x == Some(i)) {
                    let rs = arc(&r.pts);
                    let at = project(fits[less].q, &r.pts, &rs).0;
                    out.extend(split_road(&r, &rs, &[(at, fits[less].q)]));
                } else {
                    out.push(r);
                }
            }
        }
    }
    let mut gone: Vec<usize> = ids;
    gone.sort_unstable_by(|x, y| y.cmp(x));
    for i in gone {
        roads.remove(i);
    }
    roads.extend(out);
    met
}

/// Two branches meeting the through road on one side, near each other: the lesser (class, then
/// shorter) ends on the greater, which meets the road and is met far enough out from it (that
/// junction is squared in its turn). Returns whether they could.
#[allow(clippy::too_many_arguments)]
fn same_side(sq: &Square, t: &RoadPath, ts: &[f64], legs: &[RoadPath], branches: &[usize], fits: &mut [Fit], feeds: &mut [Option<usize>], x: usize, y: usize) -> bool {
    let cell = sq.cell;
    let (bx, by) = (&legs[branches[x]], &legs[branches[y]]);
    let (main, less) = if (bx.class, road_len(bx)) <= (by.class, road_len(by)) { (x, y) } else { (y, x) };
    let Some(fm) = fit_branch(sq, t, ts, &legs[branches[main]], None, fits[main].reach.max(JUNCTION_GAP + 0.1 * cell)) else { return false };
    let lb = &legs[branches[less]];
    let ls = arc(&lb.pts);
    let reach = fits[less].reach;
    let link = [point_at(&lb.pts, &ls, reach), fm.p];
    let n = (dist(link[0], link[1]) / (0.05 * cell).max(60.0)).ceil().max(1.0) as usize;
    let line: Vec<[f64; 2]> = (0..=n).map(|i| i as f64 / n as f64).map(|f| [link[0][0] + (link[1][0] - link[0][0]) * f, link[0][1] + (link[1][1] - link[0][1]) * f]).collect();
    let z_main = val_at(&legs[branches[main]].z, &arc(&legs[branches[main]].pts), fm.reach);
    let z_less = val_at(&lb.z, &ls, reach);
    if fm.clear && line.iter().all(|c| (sq.dry)(c[0], c[1]) && !sq.in_town(*c)) && sq.ramp_ok(&line, z_less, z_main, lb.class.max_grade()) && reach < 0.45 * ls[ls.len() - 1] {
        fits[main] = fm;
        fits[less] = Fit { reach, at: f64::NAN, q: link[1], p: link[0], curve: line, z_p: z_less, left: fits[main].left, clear: true, wide: true };
        feeds[less] = Some(main);
        return true;
    }
    false
}

/// Junctions (away from settlements) joined by a road shorter than `JUNCTION_GAP` become one,
/// halfway along it: each other road there runs on along it to that point. One such pair at a
/// time, shortest first; returns whether any were merged.
fn close_junctions(roads: &mut Vec<RoadPath>, sq: &Square) -> bool {
    let mut merged = false;
    loop {
        let ends = ends_by_point(roads);
        let key = |p: [f64; 2]| (p[0].to_bits(), p[1].to_bits());
        let junction = |k: &(u64, u64)| ends[k].len() >= 3 && !sq.towns.iter().any(|&(t, r)| dist(t, [f64::from_bits(k.0), f64::from_bits(k.1)]) < r);
        let mut short: Vec<(f64, usize)> = roads
            .iter()
            .enumerate()
            .filter(|(_, r)| r.pts.len() >= 2 && r.stroke.is_none())
            .filter(|(_, r)| {
                let (a, b) = (key(r.pts[0]), key(r.pts[r.pts.len() - 1]));
                a != b && junction(&a) && junction(&b)
            })
            .map(|(i, r)| (road_len(r), i))
            .filter(|&(l, _)| l < JUNCTION_GAP)
            .collect();
        short.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        let mut done = false;
        for (len, i) in short {
            let r = &roads[i];
            let (a, b) = (key(r.pts[0]), key(r.pts[r.pts.len() - 1]));
            // Not where another road joins the same two (merging would make it a loop).
            let others: Vec<(usize, bool)> = ends[&a].iter().chain(&ends[&b]).copied().filter(|e| e.0 != i).collect();
            let mut seen: Vec<usize> = others.iter().map(|e| e.0).collect();
            seen.sort_unstable();
            if seen.windows(2).any(|w| w[0] == w[1]) || others.iter().any(|e| roads[e.0].stroke.is_some()) {
                continue;
            }
            let s = arc(&r.pts);
            let m = point_at(&r.pts, &s, 0.5 * len);
            if !(sq.dry)(m[0], m[1]) || !sq.clear(m) {
                continue;
            }
            let halves = split_road(r, &s, &[(0.5 * len, m)]);
            // From each end: the road carrying on along it runs on to the middle; the others
            // end there on a chord from 0.25 cell back (squared again after).
            let to_mid = [halves[0].clone(), flipped(&halves[1])];
            let mut gone = vec![i];
            let mut add = Vec::new();
            let mut ok = true;
            for (side, k) in [a, b].iter().enumerate() {
                let half = &to_mid[side];
                let o = half.pts[0];
                let along = unit2(sub2(point_at(&half.pts, &arc(&half.pts), (0.1 * sq.cell).min(0.5 * len)), o));
                let ins: Vec<(usize, bool, RoadPath)> =
                    ends[k].iter().filter(|e| e.0 != i).map(|&(ri, start)| (ri, start, if start { flipped(&roads[ri]) } else { roads[ri].clone() })).collect();
                let into = |x: &RoadPath| {
                    let s = arc(&x.pts);
                    let l = s[s.len() - 1];
                    unit2(sub2(o, point_at(&x.pts, &s, l - (0.3 * sq.cell).min(0.45 * l))))
                };
                let main = (0..ins.len()).max_by(|&p, &q| dot2(into(&ins[p].2), along).total_cmp(&dot2(into(&ins[q].2), along)).then(q.cmp(&p))).unwrap_or(0);
                for (n, (ri, start, mut x)) in ins.into_iter().enumerate() {
                    if n == main {
                        x.pts.extend_from_slice(&half.pts[1..]);
                        x.z.extend_from_slice(&half.z[1..]);
                        x.wander.extend_from_slice(&half.wander[1..]);
                    } else {
                        let s = arc(&x.pts);
                        let l = s[s.len() - 1];
                        let back = l - (0.25 * sq.cell).min(0.45 * l);
                        let (p, zp, zm) = (point_at(&x.pts, &s, back), val_at(&x.z, &s, back), half.z[half.z.len() - 1]);
                        let keep: Vec<usize> = (0..x.pts.len()).filter(|&q| s[q] < back - 5.0).collect();
                        let c = (dist(p, m) / (0.05 * sq.cell).max(60.0)).ceil().max(1.0) as usize;
                        let chord: Vec<[f64; 2]> = (0..=c).map(|q| [p[0] + (m[0] - p[0]) * q as f64 / c as f64, p[1] + (m[1] - p[1]) * q as f64 / c as f64]).collect();
                        ok &= chord.iter().all(|q| (sq.dry)(q[0], q[1]) && !sq.in_town(*q));
                        let mut pts: Vec<[f64; 2]> = keep.iter().map(|&q| x.pts[q]).collect();
                        let mut z: Vec<f32> = keep.iter().map(|&q| x.z[q]).collect();
                        let mut wander: Vec<f32> = keep.iter().map(|&q| x.wander[q]).collect();
                        pts.extend_from_slice(&chord);
                        z.extend((0..=c).map(|q| zp + (zm - zp) * q as f32 / c as f32));
                        wander.extend(std::iter::repeat_n(0.0, c + 1));
                        let last = pts.len() - 1;
                        pts[last] = m;
                        (x.pts, x.z, x.wander) = (pts, z, wander);
                    }
                    add.push(if start { flipped(&x) } else { x });
                    gone.push(ri);
                }
            }
            if !ok {
                continue;
            }
            gone.sort_unstable_by(|x, y| y.cmp(x));
            for g in gone {
                roads.remove(g);
            }
            roads.extend(add);
            done = true;
            break;
        }
        if !done {
            return merged;
        }
        merged = true;
    }
}
