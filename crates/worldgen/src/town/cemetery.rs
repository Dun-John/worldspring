//! R5 towns (M21): cemeteries, a park kind (`ParkKind::Cemetery`) laid out by the culture group
//! (`culture % 5`, as `roof::look` and `wallstyle` read it: 0 North, 1 West, 2 South, 3 Dry, 4 Steppe).
//!
//! Where (`inner_pick`, `by_roads`): a city one or two, a metropolis two, a town now and then one
//! (villages keep their churchyards). One of a town's generated commons when it has two or more,
//! the rest on open ground beside a road out of town (outside its walls and patches), as close to
//! the town as the ground allows; a park set to a cemetery by hand is one whatever the counts.
//!
//! How (`plan`, a pure function of the park): a low boundary 4.5 ft in from the plot's edge (a
//! dry-stone wall, a West hedge; none on the Steppe) with a gap where the way in comes from the
//! street (the longest side facing one), a lychgate over the gap in the North and the West; a main
//! walk in from the gate, cross walks square to it; graves in rows square to the main walk:
//! - North: headstones, cairns among them, a few standing stones;
//! - West: headstones, yews along the boundary and the main walk, mausoleums for a rich town;
//! - South: family tomb-houses in rows along the walks, tomb slabs between;
//! - Dry: small domed tombs along the walks, tomb slabs between;
//! - Steppe: kurgans (big round mounds, a few stones at the foot), a track in, little else.
//!
//! Mausoleums, tomb-houses and domed tombs are buildings (`raise`: function "tomb", one tomb
//! chamber, its door on the footprint's first side, toward its walk), laid once the layout is in
//! world ft, so the battlemap's props (`park_layout`, from the same park) and the buildings agree.

use super::geom::*;
use super::wards::Changes;
use super::{Building, Layout, Park, ParkKind, Site, Structure, TownPlan, Ward, wallstyle};
use crate::battlemap::{Kind, KindInfo};
use crate::core::rng::{Pcg32, hash2, hash3, unit};
use crate::park::ParkLayout;
use crate::t0::settle::{Settlement, Tier};

/// How a cemetery is laid out: its culture group, whether its town is rich (mausoleums), a city's
/// (wider walks), and the patch it is on (beside a road out of town: the outskirts' patch holding it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub group: u8,
    pub rich: bool,
    pub city: bool,
    pub patch: u32,
    pub by_road: bool,
}

impl Style {
    pub(super) fn of(s: &Settlement, patch: usize, by_road: bool) -> Style {
        let rich = wallstyle::rich_poor(s.tier, wallstyle::wealth(s)).0 || s.capital || s.tier == Tier::Metropolis;
        Style {
            group: s.culture % 5,
            rich,
            city: s.tier >= Tier::City,
            patch: patch as u32,
            by_road,
        }
    }
}

/// A cemetery's boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Wall,
    Hedge,
    None,
}

/// A cemetery laid out (world ft).
#[derive(Clone, Debug)]
pub struct Plan {
    /// The boundary's line (`BOUND_IN` in from the plot's edge), what it is, and its gap at the gate
    /// (`gate`, on the line; `inward` into the plot, `along` the gate's side; `gap` ft wide).
    pub boundary: Vec<P>,
    pub bound: Bound,
    pub gate: P,
    pub inward: P,
    pub along: P,
    pub gap: f64,
    /// Walks: centre line ends and width (ft); the main walk first, from the plot's edge.
    pub walks: Vec<(P, P, f64)>,
    /// The tomb buildings' footprints (their first side, corners 0 to 1, is the door's, toward a walk).
    pub tombs: Vec<Vec<P>>,
    /// What is set out: where, turn (rad: a lychgate's and a slab's long side along it), kind, look, size.
    pub props: Vec<(P, f32, Kind, u8, f32)>,
}

impl Plan {
    /// The same, every point moved by `o`.
    fn moved(mut self, o: P) -> Plan {
        let mv = |p: &mut P| *p = add(*p, o);
        self.boundary.iter_mut().for_each(mv);
        mv(&mut self.gate);
        for w in &mut self.walks {
            mv(&mut w.0);
            mv(&mut w.1);
        }
        self.tombs.iter_mut().flatten().for_each(mv);
        for p in &mut self.props {
            mv(&mut p.0);
        }
        self
    }
}

/// The most a common made a cemetery may hold (sq ft, its patch as planned).
const INNER_MAX: f64 = 90_000.0;

/// How far a cemetery by a road keeps off the town wall's planned line (ft: past its towers and bastions).
const WALL_OFF: f64 = 70.0;

/// How far the boundary stands in from the plot's edge (ft: a street's verge, as rural runs keep).
pub const BOUND_IN: f64 = 4.5;
/// How far a tomb's door side stands off its walk's edge (ft).
pub const DOOR_GAP: f64 = 2.5;

const fn k(id: Kind, name: &'static str, radius: f32, blocks_move: bool, blocks_sight: bool, cover: u8, difficult: bool, height_ft: f32) -> KindInfo {
    KindInfo {
        id: id as u16,
        name,
        radius,
        blocks_move,
        blocks_sight,
        cover,
        difficult,
        height_ft,
        hazard: None,
        feature: true,
    }
}

/// The cemetery props (ids 100-105, the towns lane's range), for the battlemap's catalog.
pub(crate) const KINDS: [(Kind, KindInfo); 6] = [
    (Kind::Cairn, k(Kind::Cairn, "cairn", 0.4, true, false, 1, false, 4.0)),
    (Kind::StandingStone, k(Kind::StandingStone, "standing stone", 0.3, true, false, 2, false, 8.0)),
    (Kind::Yew, k(Kind::Yew, "yew", 1.6, true, true, 2, false, 20.0)),
    (Kind::Lychgate, k(Kind::Lychgate, "lychgate", 1.0, false, false, 1, false, 10.0)),
    // (25-50 ft across by its size: 35 ft at 1.)
    (Kind::Kurgan, k(Kind::Kurgan, "kurgan", 3.5, false, true, 3, true, 12.0)),
    (Kind::TombSlab, k(Kind::TombSlab, "tomb slab", 0.5, false, false, 1, true, 2.0)),
];

/// A kurgan's radius (ft) at size 1.
pub const KURGAN_R: f64 = 17.5;

/// How many cemeteries a town of this tier has (by its plan's roll): a city one or two, a
/// metropolis two, a town now and then one.
fn wanted(s: &Settlement, plan: &TownPlan) -> usize {
    let r = plan.roll(0, 0xce1);
    match s.tier {
        Tier::Town => (r < 0.6) as usize,
        Tier::City => 1 + (r < 0.5) as usize,
        Tier::Metropolis => 2,
        Tier::Village => 0,
    }
}

/// How many cemeteries the town wants, and the generated common (if it has two or more, none
/// set by hand, on a slope, the palace's or bigger than `INNER_MAX`) that is one of them.
pub(super) fn inner_pick(s: &Settlement, plan: &TownPlan, ward: &[Option<Ward>], ch: &Changes) -> (usize, Option<usize>) {
    let want = wanted(s, plan);
    let cands: Vec<usize> = (0..plan.n_inner)
        .filter(|&i| {
            plan.ward[i] == Some(Ward::Park) && ward[i] == Some(Ward::Park) && !plan.steep[i] && plan.palace != Some(i) && !ch.park.contains_key(&i) && !ch.ward.contains_key(&i)
        })
        .collect();
    // (Not the biggest: a patch of `INNER_MAX` sq ft at most.)
    let small = cands.iter().copied().filter(|&i| area(&plan.mesh.face_pts(i)).abs() <= INNER_MAX);
    let pick = if want > 0 && cands.len() >= 2 {
        small.max_by(|&a, &b| plan.roll(a, 0xce2).total_cmp(&plan.roll(b, 0xce2)).then(b.cmp(&a)))
    } else {
        None
    };
    (want, pick)
}

/// Lays `want` cemeteries on open ground beside the roads out of town (local ft, once the fields
/// are picked: those under one go): each a rectangle along the road (its approach, then the world's
/// road it came in by), 2 ft off its edge, from 70 ft out from the gate to 1,100 ft, wholly outside
/// the town's patches and `WALL_OFF` off its walls' planned line, dry, on gentle
/// ground, clear of every building, road, square, park and bastion; the roads taken in an order
/// rolled from the plan, one each.
pub(super) fn by_roads(site: &Site, s: &Settlement, plan: &TownPlan, l: &mut Layout, want: usize) {
    if want == 0 {
        return;
    }
    let (along, deep) = match s.tier {
        Tier::Metropolis => (220.0, 150.0),
        Tier::City => (180.0, 120.0),
        _ => (130.0, 90.0),
    };
    let f = 0.85 + 0.3 * plan.roll(0, 0xce3);
    let (along, deep) = (along * f, deep * f);
    let mut order: Vec<usize> = (0..l.roads.len()).filter(|&k| l.roads[k].1 < 3 && l.roads[k].0.len() >= 2).collect();
    order.sort_by(|&a, &b| {
        unit(hash3(plan.pseed, a as i64, 0xce4, 0))
            .total_cmp(&unit(hash3(plan.pseed, b as i64, 0xce4, 0)))
            .then(a.cmp(&b))
    });
    let ways = world_ways(site);
    let mut laid = 0;
    for &rk in &order {
        if laid >= want {
            break;
        }
        let (pts, _, w) = l.roads[rk].clone();
        // (From the gate out: the approach ends at its gate; then on along the world's road it
        // came in by, away from the town.)
        let mut line: Vec<P> = pts.clone();
        line.reverse();
        let end = pts[0];
        let near = ways
            .iter()
            .enumerate()
            .flat_map(|(wi, (_, q, _))| q.iter().enumerate().map(move |(i, p)| (dist(*p, end), wi, i)))
            .min_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
        let mut own = None;
        let mut w = w;
        if let Some((_, wi, i)) = near.filter(|n| n.0 < 80.0) {
            let (ri, q, ww) = &ways[wi];
            let away = |j: usize| len(q[j]);
            let up = i + 1 < q.len() && (i == 0 || away(i + 1) >= away(i - 1));
            let mut run = 0.0;
            let mut j = i;
            loop {
                let nj = if up { j + 1 } else { j.wrapping_sub(1) };
                if nj >= q.len() || run > 1100.0 {
                    break;
                }
                run += dist(q[j], q[nj]);
                line.push(q[nj]);
                j = nj;
            }
            own = Some(*ri);
            w = w.max(*ww);
        }
        let total: f64 = line.windows(2).map(|q| dist(q[0], q[1])).sum();
        let at = |s: f64| -> (P, P) {
            let mut acc = 0.0;
            for q in line.windows(2) {
                let d = dist(q[0], q[1]);
                if acc + d >= s && d > 1e-9 {
                    return (lerp(q[0], q[1], (s - acc) / d), mul(sub(q[1], q[0]), 1.0 / d));
                }
                acc += d;
            }
            let n = line.len();
            (line[n - 1], mul(sub(line[n - 1], line[n - 2]), 1.0 / dist(line[n - 1], line[n - 2]).max(1e-9)))
        };
        let mut found: Option<(Vec<P>, Vec<bool>)> = None;
        let mut s0 = 70.0 + 0.5 * along;
        while found.is_none() && s0 + 0.5 * along <= total.min(1100.0) {
            let (p, _) = at(s0);
            // (The road's way over the rectangle's length: a chord, so a bend doesn't tilt it.)
            let (a, b) = (at(s0 - 0.5 * along).0, at(s0 + 0.5 * along).0);
            let t = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
            for side in [1.0, -1.0] {
                let nrm = mul([-t[1], t[0]], side);
                let c = add(p, mul(nrm, 0.5 * w + 2.0 + 0.5 * deep));
                let poly = super::rect(c, t, along, deep);
                if clear_by_road(site, plan, l, &poly, rk, &ways, own) {
                    // (The side facing the road: corners 0-1 lie at -v, `rect`.)
                    let v = [-t[1], t[0]];
                    let road_side = if dot(v, nrm) > 0.0 { 0 } else { 2 };
                    found = Some((poly, (0..4).map(|e| e == road_side).collect()));
                    break;
                }
            }
            s0 += 10.0;
        }
        let Some((poly, fronts)) = found else { continue };
        // (The fields under it, and 6 ft round, go.)
        let keep: Vec<bool> = l
            .fields
            .iter()
            .map(|fld| poly_gap(fld, &poly) >= 6.0 && !contains(&poly, centroid(fld)) && !fld.iter().any(|q| contains(&poly, *q)))
            .collect();
        let mut it = keep.iter();
        l.fields.retain(|_| *it.next().unwrap());
        if !l.field_info.is_empty() {
            let mut it = keep.iter();
            l.field_info.retain(|_| *it.next().unwrap());
        }
        let c = centroid(&poly);
        let patch = (plan.n_inner..plan.mesh.faces.len())
            .find(|&f| plan.mesh.faces[f].len() >= 3 && contains(&plan.mesh.face_pts(f), c))
            .unwrap_or(0);
        let key = hash3(plan.pseed, rk as i64, laid as i64, 0xce5);
        let mut pk = super::park(site, &poly, ParkKind::Cemetery, fronts, key);
        pk.cemetery = Some(Style::of(s, patch, true));
        l.parks.push(pk);
        laid += 1;
    }
}

/// The world's roads round the town (local ft): each piece's road, its points in the road's order
/// (every 20 ft or so), its width.
fn world_ways(site: &Site) -> Vec<(u32, Vec<P>, f64)> {
    let t0 = site.t0;
    let reach = 3000.0;
    let c = site.center;
    let mut segs = t0.roads.segments_near([c[0] - reach, c[1] - reach, c[0] + reach, c[1] + reach], 0.0);
    segs.sort();
    segs.dedup();
    let mut out: Vec<(u32, Vec<P>, f64)> = Vec::new();
    let mut last: Option<(u32, u32)> = None;
    for (ri, k) in segs {
        let rc = &t0.roads.roads[ri as usize];
        if k as usize + 1 >= rc.s.len() {
            continue;
        }
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(1.0, 400.0) as usize;
        let pts: Vec<P> = (0..=n).map(|j| sub(rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p, c)).collect();
        match (last, out.last_mut()) {
            (Some((lr, lk)), Some(cur)) if lr == ri && lk + 1 == k => cur.1.extend(pts.into_iter().skip(1)),
            _ => out.push((ri, pts, rc.class.width_ft())),
        }
        last = Some((ri, k));
    }
    out
}

/// Whether a cemetery `poly` (local ft) beside road `rk` (on along the world's road `own`) stands
/// clear: outside the town, dry, gentle, off every way.
fn clear_by_road(site: &Site, plan: &TownPlan, l: &Layout, poly: &[P], rk: usize, ways: &[(u32, Vec<P>, f64)], own: Option<u32>) -> bool {
    let o = obb(poly);
    let c = centroid(poly);
    let mut samples: Vec<P> = poly.to_vec();
    let (nu, nv) = ((o.long / 20.0).ceil() as usize, (o.short / 20.0).ceil() as usize);
    for j in 0..=nv {
        for i in 0..=nu {
            samples.push(lerp(
                lerp(poly[0], poly[1], i as f64 / nu as f64),
                lerp(poly[3], poly[2], i as f64 / nu as f64),
                j as f64 / nv as f64,
            ));
        }
    }
    if samples.iter().any(|&p| plan.in_town(p) || site.wet(p) || site.near_river(p, 25.0)) || site.grade(poly) > 0.1 {
        return false;
    }
    let near = |q: &[P], gap: f64| poly_gap(q, poly) < gap || contains(poly, q[0]) || contains(q, c);
    let seg_gap = |a: P, b: P| poly_gap(&[a, b], poly);
    // (Off the wall's line as planned, built or not, its towers and bastions with it: the same place
    // whether its walls are taken down or put up by hand.)
    let mp = &plan.mesh.pos;
    for (chain, closed, _) in &plan.wall_runs {
        let n = chain.len();
        let segs = if *closed { n } else { n.saturating_sub(1) };
        if (0..segs).any(|k| seg_gap(mp[chain[k]], mp[chain[(k + 1) % n]]) < WALL_OFF) {
            return false;
        }
    }
    if l.castles.iter().any(|&(cc, r)| dist(cc, c) < r + 0.5 * o.long + 60.0) {
        return false;
    }
    if l.buildings.iter().any(|b| near(&b.poly, 10.0)) {
        return false;
    }
    for (ri, pts, w) in ways {
        let need = 0.5 * w + if Some(*ri) == own { 1.5 } else { 6.0 };
        if pts.windows(2).any(|s| seg_gap(s[0], s[1]) < need) {
            return false;
        }
    }
    for (k, (pts, _, w)) in l.roads.iter().enumerate() {
        let need = 0.5 * w + if k == rk { 1.5 } else { 6.0 };
        if pts.windows(2).any(|s| seg_gap(s[0], s[1]) < need) {
            return false;
        }
    }
    let polys = l
        .plazas
        .iter()
        .map(|p| &p.pts)
        .chain(l.parks.iter().map(|p| &p.pts))
        .chain(&l.bridges)
        .chain(&l.piers)
        .chain(&l.blocks);
    !polys.into_iter().any(|q| q.len() >= 3 && near(q, 10.0))
}

/// How far a point stands from a polygon (0 inside).
fn depth_or_gap(poly: &[P], p: P) -> f64 {
    if contains(poly, p) {
        return 0.0;
    }
    let m = poly.len();
    (0..m).map(|e| seg_dist(p, poly[e], poly[(e + 1) % m])).fold(f64::MAX, f64::min)
}

/// How far `p` is inside `poly` (0 outside).
fn depth(poly: &[P], p: P) -> f64 {
    if !contains(poly, p) {
        return 0.0;
    }
    let m = poly.len();
    (0..m).map(|e| seg_dist(p, poly[e], poly[(e + 1) % m])).fold(f64::MAX, f64::min)
}

/// How far along `d` (unit) from `p` the ray first meets the outline of `poly` (0: never).
fn exit(poly: &[P], p: P, d: P) -> f64 {
    let m = poly.len();
    let mut best = f64::MAX;
    for e in 0..m {
        let (a, b) = (poly[e], poly[(e + 1) % m]);
        let ab = sub(b, a);
        let den = cross(d, ab);
        if den.abs() < 1e-12 {
            continue;
        }
        let ap = sub(a, p);
        let (t, u) = (cross(ap, ab) / den, cross(ap, d) / den);
        if t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) {
            best = best.min(t);
        }
    }
    if best == f64::MAX { 0.0 } else { best }
}

/// A footprint whose first side (door) runs along `a` from `door - a·w/2`, its depth `d` toward `o`,
/// wound counter-clockwise.
fn footprint(door: P, a: P, o: P, w: f64, d: f64) -> Vec<P> {
    let a = if cross(a, o) > 0.0 { a } else { mul(a, -1.0) };
    let (p0, p1) = (sub(door, mul(a, 0.5 * w)), add(door, mul(a, 0.5 * w)));
    vec![p0, p1, add(p1, mul(o, d)), add(p0, mul(o, d))]
}

/// What is laid out so far, for keeping things apart.
struct Room<'a> {
    boundary: &'a [P],
    walks: &'a [(P, P, f64)],
    gate: P,
    tombs: Vec<Vec<P>>,
    discs: Vec<(P, f64)>,
}

impl Room<'_> {
    /// Whether a disc of radius `r` at `p` stands clear: `edge` ft inside the boundary, `walk` ft off the
    /// walks' edges, off the tombs by a foot and a half, off what stands by half a foot, 6 ft off the gate.
    fn free(&self, p: P, r: f64, edge: f64, walk: f64) -> bool {
        depth(self.boundary, p) >= r + edge
            && self.walks.iter().all(|&(a, b, w)| seg_dist(p, a, b) >= 0.5 * w + r + walk)
            && dist(p, self.gate) >= r + 6.0
            && self.tombs.iter().all(|t| depth_or_gap(t, p) >= r + 1.5)
            && self.discs.iter().all(|&(q, rq)| dist(p, q) >= r + rq + 0.5)
    }
}

/// Cemetery `pk` laid out (None: not a cemetery, or too small for one).
pub fn plan(pk: &Park) -> Option<Plan> {
    if pk.kind != ParkKind::Cemetery || pk.pts.len() < 3 {
        return None;
    }
    // (One laid without its look, as no generated one is: the West's.)
    let st = pk.cemetery.unwrap_or(Style {
        group: 1,
        rich: false,
        city: false,
        patch: 0,
        by_road: false,
    });
    // (Laid out about its first corner, whole feet: far from the origin, products of world feet
    // lose a small footprint's middle.)
    let o = [crate::core::round(pk.pts[0][0]), crate::core::round(pk.pts[0][1])];
    let local: Vec<P> = pk.pts.iter().map(|q| sub(*q, o)).collect();
    let pts = &local;
    let m = pts.len();
    let boundary = inset(pts, &vec![BOUND_IN; m]);
    if boundary.len() < 3 {
        return None;
    }
    let el = |e: usize| dist(pts[e], pts[(e + 1) % m]);
    // The way in: the longest side facing a street, else the longest.
    let front = (0..m)
        .filter(|&e| pk.fronts.get(e).copied().unwrap_or(false) && el(e) >= 20.0)
        .max_by(|&a, &b| el(a).total_cmp(&el(b)).then(b.cmp(&a)));
    let e = front.unwrap_or_else(|| (0..m).max_by(|&a, &b| el(a).total_cmp(&el(b)).then(b.cmp(&a))).unwrap_or(0));
    let (a, b) = (pts[e], pts[(e + 1) % m]);
    let t = mul(sub(b, a), 1.0 / el(e).max(1e-9));
    let mid = lerp(a, b, 0.5);
    let mut n = [-t[1], t[0]];
    if dot(n, sub(centroid(pts), mid)) < 0.0 {
        n = mul(n, -1.0);
    }
    let gate = add(mid, mul(n, BOUND_IN));
    let deep = exit(&boundary, add(gate, mul(n, 0.5)), n) + 0.5;
    let mut rng = Pcg32::new(hash2(pk.key, 0xce3e, 7), 41);
    let roll = |i: i64, j: i64, s: i64| unit(hash3(pk.key, i, j, s));
    let group = st.group % 5;
    let bound = match group {
        4 => Bound::None,
        1 if !st.rich || roll(0, 0, 1) < 0.6 => Bound::Hedge,
        _ => Bound::Wall,
    };
    let wm = match (group, st.city) {
        (4, _) => 5.0,
        (_, true) => 8.0,
        _ => 6.0,
    };
    let mut out = Plan {
        boundary: boundary.clone(),
        bound,
        gate,
        inward: n,
        along: t,
        gap: wm + 2.0,
        walks: Vec::new(),
        tombs: Vec::new(),
        props: Vec::new(),
    };
    if deep < 30.0 {
        return Some(out.moved(o));
    }
    // Walks: the main one from the plot's edge in to 4 ft short of the boundary, cross walks
    // square to it (none on the Steppe), evenly along it, from boundary to boundary but 3 ft.
    out.walks.push((mid, add(gate, mul(n, deep - 4.0)), wm));
    if group != 4 {
        let crosses = (((deep - 30.0) / 60.0) as usize).min(4);
        for k in 0..crosses {
            let c = add(gate, mul(n, deep * (k + 1) as f64 / (crosses + 1) as f64));
            let (l0, l1) = (exit(&boundary, c, mul(t, -1.0)), exit(&boundary, c, t));
            if l0 + l1 >= 40.0 {
                out.walks.push((sub(c, mul(t, l0 - 3.0)), add(c, mul(t, l1 - 3.0)), 5.0));
            }
        }
    }
    let walks = out.walks.clone();
    let mut room = Room {
        boundary: &boundary,
        walks: &walks,
        gate,
        tombs: Vec::new(),
        discs: Vec::new(),
    };
    // Tomb buildings along the walks, both sides, their doors toward them: (width along, depth, gap, most).
    let area_ft = area(&boundary).abs();
    let tombs: Option<(f64, f64, f64, usize)> = match group {
        1 if st.rich => Some((15.0, 20.0, 10.0, (2 + (area_ft / 12_000.0) as usize).min(6))),
        2 => Some((15.0, 15.0, 3.0, usize::MAX)),
        3 => Some((15.0, 15.0, 7.0, usize::MAX)),
        _ => None,
    };
    if let Some((tw, td, tgap, most)) = tombs {
        for (wi, &(p, q, w)) in walks.iter().enumerate() {
            let len_w = dist(p, q);
            let u = mul(sub(q, p), 1.0 / len_w.max(1e-9));
            for side in [1.0, -1.0] {
                let o = mul([-u[1], u[0]], side);
                let mut s = 2.0;
                while s + tw <= len_w && room.tombs.len() < most {
                    // (A South tomb-house now and then deeper: a family's grander one.)
                    let d = if group == 2 && roll(wi as i64, (s * 4.0) as i64, 2) < 0.35 { td + 5.0 } else { td };
                    let door = add(add(p, mul(u, s + 0.5 * tw)), mul(o, 0.5 * w + DOOR_GAP));
                    let f = footprint(door, u, o, tw, d);
                    let fits = f.iter().all(|&c| depth(&boundary, c) >= 3.5)
                        && dist(centroid(&f), gate) > 0.5 * (tw + d) + 12.0
                        && walks
                            .iter()
                            .enumerate()
                            .all(|(wj, &(a, b, ww))| wj == wi || poly_gap(&[a, b], &f) >= 0.5 * ww + 2.0 && !contains(&f, a))
                        && room.tombs.iter().all(|o2| poly_gap(o2, &f) >= tgap && !contains(o2, centroid(&f)));
                    if fits {
                        room.tombs.push(f);
                        s += tw + tgap;
                    } else {
                        s += 2.0;
                    }
                }
            }
        }
    }
    // The Steppe's kurgans: big round mounds, apart, off the track, a few stones at the foot.
    if group == 4 {
        let (x0, y0, x1, y1) = boundary.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| {
            (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1]))
        });
        let mut cands: Vec<(f64, P)> = Vec::new();
        let (nx, ny) = (((x1 - x0) / 12.0) as i64, ((y1 - y0) / 12.0) as i64);
        for j in 0..=ny {
            for i in 0..=nx {
                let p = [x0 + (i as f64 + 0.25 + 0.5 * roll(i, j, 3)) * 12.0, y0 + (j as f64 + 0.25 + 0.5 * roll(i, j, 4)) * 12.0];
                cands.push((roll(i, j, 5), p));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut kurgans = 0;
        for (h, p) in cands {
            if kurgans >= 8 {
                break;
            }
            let r = 12.5 + 12.5 * unit(hash2(pk.key, (h * 1e9) as i64, 6));
            if room.free(p, r, 3.0, 3.0) {
                room.discs.push((p, r));
                out.props.push((p, 0.0, Kind::Kurgan, rng.below(4) as u8, (r / KURGAN_R) as f32));
                kurgans += 1;
                let stones = 1 + rng.below(3);
                let a0 = rng.range(0.0, std::f64::consts::TAU);
                for s in 0..stones {
                    let a = a0 + s as f64 * 2.2 + rng.range(-0.3, 0.3);
                    let q = add(p, [libm::cos(a) * (r + 2.5), libm::sin(a) * (r + 2.5)]);
                    if depth(&boundary, q) >= 2.0 && walks.iter().all(|&(wa, wb, ww)| seg_dist(q, wa, wb) >= 0.5 * ww + 2.0) {
                        out.props.push((q, 0.0, Kind::StandingStone, rng.below(4) as u8, 0.7));
                        room.discs.push((q, 1.0));
                    }
                }
            }
        }
    }
    // The West's yews: along the boundary every 30 ft or so, 6 ft in, and an avenue by the main walk.
    if group == 1 {
        let bm = boundary.len();
        for e in 0..bm {
            let (p, q) = (boundary[e], boundary[(e + 1) % bm]);
            let le = dist(p, q);
            let steps = (le / 30.0).floor() as usize;
            for k in 0..steps {
                let at = lerp(p, q, (k as f64 + 0.5) / steps as f64);
                let mut inn = [-(q[1] - p[1]) / le, (q[0] - p[0]) / le];
                if !contains(&boundary, add(at, mul(inn, 1.0))) {
                    inn = mul(inn, -1.0);
                }
                let y = add(at, mul(inn, 6.0));
                if roll(e as i64, k as i64, 7) < 0.8 && room.free(y, 4.0, 1.0, 2.0) {
                    room.discs.push((y, 4.0));
                    out.props.push((y, 0.0, Kind::Yew, rng.below(4) as u8, 1.0));
                }
            }
        }
        let (p, q, w) = walks[0];
        let lw = dist(p, q);
        let mut s = 20.0;
        while s < lw - 6.0 {
            for side in [1.0, -1.0] {
                let y = add(lerp(p, q, s / lw), mul(t, side * (0.5 * w + 5.0)));
                if room.free(y, 3.0, 2.0, 1.5) {
                    room.discs.push((y, 3.0));
                    out.props.push((y, 0.0, Kind::Yew, rng.below(4) as u8, 0.9));
                }
            }
            s += 36.0;
        }
    }
    // The North's standing stones: three to five, apart, in the open.
    if group == 0 {
        let want = 3 + rng.below(3) as usize;
        let mut set = 0;
        for k in 0..60 {
            if set >= want {
                break;
            }
            let p = add(
                gate,
                add(
                    mul(n, roll(k, 0, 8) * deep),
                    mul(t, (roll(k, 0, 9) - 0.5) * 2.0 * exit(&boundary, add(gate, mul(n, 0.5 * deep)), t).max(1.0)),
                ),
            );
            if room.free(p, 2.5, 3.0, 3.0) && room.discs.iter().all(|&(q, _)| dist(p, q) > 20.0) {
                room.discs.push((p, 2.5));
                out.props.push((p, 0.0, Kind::StandingStone, rng.below(4) as u8, 1.0));
                set += 1;
            }
        }
    }
    // Graves in rows square to the main walk: headstones (a North row's now and then a cairn), tomb
    // slabs in the South and the dry lands, along each row out from the main walk.
    if group != 4 {
        let slab = matches!(group, 2 | 3);
        let (row, step) = if slab { (10.0, 5.0) } else { (8.0, 5.0) };
        let reach = |s: f64, d: P| exit(&boundary, add(gate, mul(n, s)), d);
        let mut j = 0;
        while 6.0 + j as f64 * row < deep {
            let s = 6.0 + j as f64 * row;
            for (si, side) in [1.0, -1.0].into_iter().enumerate() {
                let d = mul(t, side);
                let far = reach(s, d);
                let mut k = 0;
                while 0.5 * wm + 3.5 + k as f64 * step < far {
                    let a = 0.5 * wm + 3.5 + k as f64 * step;
                    let p = add(add(gate, mul(n, s)), mul(d, a));
                    let skip = roll(j, k * 2 + si as i64, 10) < 0.12;
                    let ok = if slab {
                        room.free(p, 1.5, 1.0, 1.0) && room.free(add(p, mul(n, 2.5)), 1.0, 1.0, 1.0) && room.free(sub(p, mul(n, 2.5)), 1.0, 1.0, 1.0)
                    } else {
                        room.free(p, 1.2, 1.0, 1.0)
                    };
                    if ok && !skip {
                        if slab {
                            out.props
                                .push((p, (libm::atan2(t[1], t[0]) - std::f64::consts::FRAC_PI_2) as f32, Kind::TombSlab, rng.below(4) as u8, 1.0));
                        } else if group == 0 && roll(j, k * 2 + si as i64, 11) < 0.1 && room.free(p, 2.0, 1.0, 1.0) {
                            out.props.push((p, 0.0, Kind::Cairn, rng.below(4) as u8, 1.0));
                            room.discs.push((p, 2.0));
                        } else {
                            out.props.push((p, 0.0, Kind::Headstone, rng.below(4) as u8, 1.0));
                        }
                    }
                    k += 1;
                }
            }
            j += 1;
        }
    }
    // The North's and the West's lychgate over the gap at the way in.
    if matches!(group, 0 | 1) {
        out.props.push((gate, libm::atan2(t[1], t[0]) as f32, Kind::Lychgate, rng.below(4) as u8, 1.0));
    }
    out.tombs = room.tombs;
    Some(out.moved(o))
}

/// A cemetery as the battlemap and the tiles lay it out (`park::layout`): its walks as gravel paths
/// kept clear, what it sets out.
pub fn park_layout(pk: &Park) -> ParkLayout {
    let Some(pl) = plan(pk) else { return ParkLayout::default() };
    ParkLayout {
        paths: pl.walks.iter().map(|&(a, b, w)| (vec![a, b], w)).collect(),
        keep: pl.walks.iter().map(|&(a, b, w)| (a, b, 0.5 * w)).collect(),
        props: pl.props,
        ..ParkLayout::default()
    }
}

/// The boundary's runs (world ft): its line but for the gap at the gate (a wall or a hedge; none on
/// the Steppe), for `rural::battle`.
pub fn bounds(l: &Layout) -> Vec<(P, P, Bound)> {
    let mut out = Vec::new();
    for pk in l.parks.iter().filter(|p| p.kind == ParkKind::Cemetery) {
        let Some(pl) = plan(pk) else { continue };
        if pl.bound == Bound::None {
            continue;
        }
        let m = pl.boundary.len();
        for e in 0..m {
            let (a, b) = (pl.boundary[e], pl.boundary[(e + 1) % m]);
            let le = dist(a, b);
            if le < 0.5 {
                continue;
            }
            let u = mul(sub(b, a), 1.0 / le);
            let s = dot(sub(pl.gate, a), u);
            let off = cross(u, sub(pl.gate, a)).abs();
            if off < 0.5 && s > -0.5 * pl.gap && s < le + 0.5 * pl.gap {
                let (s0, s1) = (s - 0.5 * pl.gap, s + 0.5 * pl.gap);
                if s0 > 0.5 {
                    out.push((a, add(a, mul(u, s0)), pl.bound));
                }
                if s1 < le - 0.5 {
                    out.push((add(a, mul(u, s1)), b, pl.bound));
                }
            } else {
                out.push((a, b, pl.bound));
            }
        }
    }
    out
}

/// A cemetery's tomb (raised by `raise` alone: never a function a town edited by hand lost).
pub fn is_tomb(b: &Building) -> bool {
    b.func.is_some_and(|f| super::catalog::CATALOG[f as usize].key == "tomb")
}

/// The tomb buildings of the layout's cemeteries (world ft, once the layout stands there): function
/// "tomb", one storey, on the cemetery's ward. Ids after every other building's; in a town edited by
/// hand, a tomb as generated keeps its id, a new one takes one of its patch's (`wards::NEW_ID`).
pub(super) fn raise(site: &Site, l: &mut Layout, generated: Option<&Layout>) {
    let Some(fi) = super::catalog::index_of("tomb") else { return };
    let center = l.center;
    let mut next = l.buildings.iter().map(|b| b.id + 1).max().unwrap_or(0);
    let mut raised: Vec<Building> = Vec::new();
    let mut per: Vec<(u32, u32)> = Vec::new();
    for pk in l.parks.iter().filter(|p| p.kind == ParkKind::Cemetery) {
        let Some(pl) = plan(pk) else { continue };
        let patch = pk.cemetery.map_or(0, |c| c.patch);
        for poly in pl.tombs {
            let local: Vec<P> = poly.iter().map(|q| sub(*q, center)).collect();
            let was = generated.and_then(|g| {
                g.buildings
                    .iter()
                    .find(|o| o.func == Some(fi as u16) && o.poly.len() == poly.len() && o.poly.iter().zip(&poly).all(|(p, q)| dist(*p, *q) < 0.01))
            });
            let id = match (generated, was) {
                (_, Some(o)) => o.id,
                (Some(_), None) => {
                    let n = match per.iter_mut().find(|x| x.0 == patch) {
                        Some(x) => {
                            x.1 += 1;
                            x.1
                        }
                        None => {
                            per.push((patch, 0));
                            0
                        }
                    };
                    super::wards::NEW_ID + patch * super::wards::PER_PATCH + 9000 + n
                }
                (None, None) => {
                    next += 1;
                    next - 1
                }
            };
            raised.push(Building {
                poly,
                ward: Ward::Park,
                func: Some(fi as u16),
                residential: 1,
                name: None,
                floors: 1,
                pad_ft: site.pad(&local),
                structure: Structure::Roofed,
                roof: None,
                tint: None,
                material: None,
                shape: None,
                shade: None,
                id,
                parts: Vec::new(),
                keep: None,
            });
        }
    }
    for b in raised {
        let at = l.buildings.partition_point(|o| o.id < b.id);
        if l.buildings.get(at).is_none_or(|o| o.id != b.id) {
            l.buildings.insert(at, b);
        }
    }
}
