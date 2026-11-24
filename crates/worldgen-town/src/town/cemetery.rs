//! R5 towns (M21): cemeteries, a park kind (`ParkKind::Cemetery`) laid out by the culture group
//! (`culture % 5`, as `roof::look` and `wallstyle` read it: 0 North, 1 West, 2 South, 3 Dry, 4 Steppe).
//!
//! Where (`inner_pick`, `by_roads`): one at most, in a town now and then (a tenth), a city a
//! quarter, a metropolis half (villages keep their churchyards). One of a town's generated commons when it has two or more,
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
use crate::battle::{Kind, KindInfo};
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
    /// A gate pier: a short stretch of the wall either side of a gap with no lychgate (the South's,
    /// the dry lands'), drawn thicker than the wall.
    Pier,
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
    /// By a road: the path from the road's edge to the plot's edge at the gate (the main walk's width).
    pub link: Option<(P, P)>,
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
        if let Some(k) = &mut self.link {
            mv(&mut k.0);
            mv(&mut k.1);
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
    (Kind::Lychgate, k(Kind::Lychgate, "lychgate", 1.2, false, false, 1, false, 10.0)),
    // (25-50 ft across by its size: 35 ft at 1.)
    (Kind::Kurgan, k(Kind::Kurgan, "kurgan", 3.5, false, true, 3, true, 12.0)),
    (Kind::TombSlab, k(Kind::TombSlab, "tomb slab", 0.5, false, false, 1, true, 2.0)),
];

/// A kurgan's radius (ft) at size 1.
pub const KURGAN_R: f64 = 17.5;
/// A yew's crown's radius (ft, as drawn) at size 1.
pub const YEW_R: f64 = 6.5;

/// How many cemeteries a town of this tier has (by its plan's roll): one at most, a town's one
/// time in ten, a city's one in four, a metropolis's one in two.
fn wanted(s: &Settlement, plan: &TownPlan) -> usize {
    let r = plan.roll(0, 0xce1);
    let p = match s.tier {
        Tier::Town => 0.1,
        Tier::City => 0.25,
        Tier::Metropolis => 0.5,
        Tier::Village => 0.0,
    };
    (r < p) as usize
}

/// How far a cemetery by a road stands off its road's edge (ft): its wall clear of the road's
/// verge (the battlemap keeps walls a square and more off a road) and of the road's line as the
/// plan draws it (wider than the bed, every corner clear: `BEND` more at its middle for a bend), a
/// short path over to the gate.
pub const VERGE: f64 = 30.0;
/// How much farther than `VERGE` the plot's road side stands off its road at its middle (ft): room
/// for the road to bend toward a corner.
const BEND: f64 = 3.0;
/// How far a tomb's corners keep in from the boundary's line (ft): 5 ft off its wall's or hedge's face.
const TOMB_IN: f64 = 7.0;
/// How far a yew's crown keeps off the gate's gap and off a walk's edge (ft).
const YEW_GATE: f64 = 10.0;
const YEW_WALK: f64 = 2.5;
/// A gate pier's length along the wall (ft); the battlemap draws it thicker than the wall (`rural`).
pub const PIER_FT: f64 = 5.0;

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
/// road it came in by), `VERGE` ft off its edge, from 70 ft out from the gate to 1,100 ft, wholly outside
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
        let mut w = w;
        if let Some((_, wi, i)) = near.filter(|n| n.0 < 80.0) {
            let (_, q, ww) = &ways[wi];
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
                let c = add(p, mul(nrm, 0.5 * w + VERGE + BEND + 0.5 * deep));
                let poly = super::rect(c, t, along, deep);
                if clear_by_road(site, plan, l, &poly, &ways) {
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
        pk.spot = super::places::Spot::Road(rk as u32);
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

/// Whether a cemetery `poly` (local ft) beside a road stands
/// clear: outside the town, dry, gentle, off every way.
fn clear_by_road(site: &Site, plan: &TownPlan, l: &Layout, poly: &[P], ways: &[(u32, Vec<P>, f64)]) -> bool {
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
    // (Every road `VERGE` off and more, its own too where it bends: the plot's every corner clear.)
    for (_, pts, w) in ways {
        let need = 0.5 * w + VERGE;
        if pts.windows(2).any(|s| seg_gap(s[0], s[1]) < need) {
            return false;
        }
    }
    for (pts, _, w) in &l.roads {
        let need = 0.5 * w + VERGE;
        if pts.windows(2).any(|s| seg_gap(s[0], s[1]) < need) {
            return false;
        }
    }
    if l.streets.iter().any(|q| q.windows(2).any(|s| seg_gap(s[0], s[1]) < 15.0)) {
        return false;
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
        link: st.by_road.then(|| (sub(mid, mul(n, VERGE + BEND + 2.0)), mid)),
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
    // Tomb buildings along the walks, both sides, their doors toward them, 10 ft apart at least:
    // (width along, depth, most: fewer in a smaller plot). The places that fit, every 2 ft along
    // each walk's sides, taken in a rolled order so they spread over the plot (slab rows between).
    let area_ft = area(&boundary).abs();
    let tombs: Option<(f64, f64, usize)> = match group {
        1 if st.rich => Some((15.0, 20.0, (2 + (area_ft / 12_000.0) as usize).min(6))),
        2 => Some((15.0, 15.0, ((area_ft / 5_000.0) as usize).clamp(1, 4))),
        3 => Some((15.0, 15.0, ((area_ft / 2_500.0) as usize).clamp(2, 8))),
        _ => None,
    };
    if let Some((tw, td, most)) = tombs {
        let tgap = 10.0;
        let mut cands: Vec<(f64, Vec<P>)> = Vec::new();
        for (wi, &(p, q, w)) in walks.iter().enumerate() {
            let len_w = dist(p, q);
            let u = mul(sub(q, p), 1.0 / len_w.max(1e-9));
            for (si, side) in [1.0, -1.0].into_iter().enumerate() {
                let o = mul([-u[1], u[0]], side);
                let mut s = 2.0;
                while s + tw <= len_w {
                    // (A South tomb-house now and then deeper: a family's grander one.)
                    let d = if group == 2 && roll(wi as i64, (s * 4.0) as i64, 2) < 0.35 { td + 5.0 } else { td };
                    let door = add(add(p, mul(u, s + 0.5 * tw)), mul(o, 0.5 * w + DOOR_GAP));
                    let f = footprint(door, u, o, tw, d);
                    let fits = f.iter().all(|&c| depth(&boundary, c) >= TOMB_IN)
                        && dist(centroid(&f), gate) > 0.5 * (tw + d) + 12.0
                        && walks
                            .iter()
                            .enumerate()
                            .all(|(wj, &(a, b, ww))| wj == wi || poly_gap(&[a, b], &f) >= 0.5 * ww + 2.0 && !contains(&f, a));
                    if fits {
                        cands.push((roll(wi as i64 * 2 + si as i64, (s * 4.0) as i64, 12), f));
                    }
                    s += 2.0;
                }
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, f) in cands {
            if room.tombs.len() >= most {
                break;
            }
            if room.tombs.iter().all(|o2| poly_gap(o2, &f) >= tgap && !contains(o2, centroid(&f)) && !contains(&f, centroid(o2))) {
                room.tombs.push(f);
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
    // The West's yews: along the boundary every 30 ft or so, 6 ft in, and an avenue by the main walk;
    // their crowns `YEW_GATE` off the gate's gap, `YEW_WALK` off the walks.
    if group == 1 {
        let (g0, g1) = (sub(gate, mul(t, 0.5 * out.gap)), add(gate, mul(t, 0.5 * out.gap)));
        let yew_ok = |room: &Room, y: P, r: f64, edge: f64| {
            seg_dist(y, g0, g1) >= r + YEW_GATE && room.free(y, r, edge, YEW_WALK)
        };
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
                if roll(e as i64, k as i64, 7) < 0.8 && yew_ok(&room, y, YEW_R, -1.0) {
                    room.discs.push((y, YEW_R));
                    out.props.push((y, 0.0, Kind::Yew, rng.below(4) as u8, 1.0));
                }
            }
        }
        let (p, q, w) = walks[0];
        let lw = dist(p, q);
        let mut s = 30.0;
        while s < lw - 6.0 {
            for side in [1.0, -1.0] {
                let y = add(lerp(p, q, s / lw), mul(t, side * (0.5 * w + 0.9 * YEW_R + YEW_WALK + 0.5)));
                if yew_ok(&room, y, 0.9 * YEW_R, 2.0) {
                    room.discs.push((y, 0.9 * YEW_R));
                    out.props.push((y, 0.0, Kind::Yew, rng.below(4) as u8, 0.9));
                }
            }
            s += 36.0;
        }
    }
    // The North's standing stones: half a dozen and more (one to 2,500 sq ft or so), apart, in the open.
    if group == 0 {
        let want = (6 + rng.below(3) as usize).max((area_ft / 2_500.0) as usize).min(14);
        let mut set = 0;
        for k in 0..120 {
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
            if room.free(p, 2.5, 3.0, 3.0) && room.discs.iter().all(|&(q, _)| dist(p, q) > 14.0) {
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
        // (The North's rows wider apart, its graves farther apart along them, some gaps.)
        let (row, step, gaps) = match group {
            2 | 3 => (10.0, 5.0, 0.12),
            0 => (12.0, 7.0, 0.2),
            _ => (8.0, 5.0, 0.12),
        };
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
                    let skip = roll(j, k * 2 + si as i64, 10) < gaps;
                    let ok = if slab {
                        room.free(p, 1.5, 1.0, 1.0)
                            && room.free(add(p, mul(n, 2.5)), 1.0, 1.0, 1.0)
                            && room.free(sub(p, mul(n, 2.5)), 1.0, 1.0, 1.0)
                            && room.tombs.iter().all(|tb| depth_or_gap(tb, p) >= 4.0)
                    } else {
                        room.free(p, 1.2, 1.0, 1.0)
                    };
                    if ok && !skip {
                        if slab {
                            out.props
                                .push((p, (libm::atan2(t[1], t[0]) - std::f64::consts::FRAC_PI_2) as f32, Kind::TombSlab, rng.below(4) as u8, 1.0));
                        } else if group == 0 && roll(j, k * 2 + si as i64, 11) < 0.3 && room.free(p, 2.0, 1.0, 1.0) {
                            out.props.push((p, 0.0, Kind::Cairn, rng.below(4) as u8, 1.0));
                            room.discs.push((p, 2.0));
                        } else {
                            out.props.push((p, libm::atan2(t[1], t[0]) as f32, Kind::Headstone, rng.below(4) as u8, 1.0));
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
    cover_middle(&mut out.props, &mut room, group, &roll, &mut rng);
    out.tombs = room.tombs;
    Some(out.moved(o))
}

/// How far apart the points are that `cover_middle` looks at, and how far from cover it lets one be (ft):
/// any square of the plot then lies within the battlemap's `MAX_COVER_GAP` of something to hide behind.
const FILL_STEP: f64 = 8.0;
const FILL_R: f64 = 16.0;

/// Cover through a large cemetery's middle, where its rows (or the Steppe's few kurgans) leave open
/// ground: every point of an 8-ft lattice over the plot farther than `FILL_R` from what is set out
/// (or a tomb) gets the culture's own: a standing stone in the North, a yew in the West (a headstone
/// where none fits), a tomb slab in the South and the dry lands, a small kurgan on the Steppe (a
/// standing stone where none fits). A small cemetery's rows reach everywhere and it gets nothing.
fn cover_middle(props: &mut Vec<(P, f32, Kind, u8, f32)>, room: &mut Room, group: u8, roll: &dyn Fn(i64, i64, i64) -> f64, rng: &mut Pcg32) {
    let (x0, y0, x1, y1) = room.boundary.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let (nx, ny) = (((x1 - x0) / FILL_STEP) as i64 + 1, ((y1 - y0) / FILL_STEP) as i64 + 1);
    let at = |i: i64, j: i64| [x0 + (i as f64 + 0.5) * FILL_STEP, y0 + (j as f64 + 0.5) * FILL_STEP];
    let mut covered = vec![false; (nx * ny) as usize];
    let reach = (FILL_R / FILL_STEP).ceil() as i64 + 1;
    let cell = |p: P| (((p[0] - x0) / FILL_STEP).floor() as i64, ((p[1] - y0) / FILL_STEP).floor() as i64);
    let mark = |covered: &mut Vec<bool>, p: P| {
        let (ci, cj) = cell(p);
        for j in (cj - reach).max(0)..=(cj + reach).min(ny - 1) {
            for i in (ci - reach).max(0)..=(ci + reach).min(nx - 1) {
                if dist(at(i, j), p) <= FILL_R {
                    covered[(j * nx + i) as usize] = true;
                }
            }
        }
    };
    for p in props.iter() {
        mark(&mut covered, p.0);
    }
    for tb in &room.tombs {
        let (a0, b0, a1, b1) = tb.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        let ((i0, j0), (i1, j1)) = (cell([a0 - FILL_R, b0 - FILL_R]), cell([a1 + FILL_R, b1 + FILL_R]));
        for j in j0.max(0)..=j1.min(ny - 1) {
            for i in i0.max(0)..=i1.min(nx - 1) {
                if depth_or_gap(tb, at(i, j)) <= FILL_R {
                    covered[(j * nx + i) as usize] = true;
                }
            }
        }
    }
    let slab_turn = (libm::atan2(room.walks.first().map_or(1.0, |w| w.1[1] - w.0[1]), room.walks.first().map_or(0.0, |w| w.1[0] - w.0[0]))) as f32;
    for j in 0..ny {
        for i in 0..nx {
            let p0 = at(i, j);
            if covered[(j * nx + i) as usize] || !contains(room.boundary, p0) {
                continue;
            }
            // (At the lattice point, else a little off it.)
            let spots = [[0.0, 0.0], [3.0, 0.0], [-3.0, 0.0], [0.0, 3.0], [0.0, -3.0], [3.0, 3.0], [-3.0, -3.0], [3.0, -3.0], [-3.0, 3.0]];
            let big = roll(i, j, 13) < 0.5;
            let mut put = None;
            for d in spots {
                let p = add(p0, d);
                let want: [(Kind, f64, f64, f32); 2] = match group {
                    0 => [(Kind::StandingStone, 2.5, 3.0, 1.0), (Kind::StandingStone, 1.2, 1.0, 0.7)],
                    1 => [(Kind::Yew, 0.9 * YEW_R, 2.0, 0.9), (Kind::Headstone, 1.2, 1.0, 1.0)],
                    2 | 3 => [(Kind::TombSlab, 2.5, 1.0, 1.0), (Kind::TombSlab, 1.5, 1.0, 1.0)],
                    _ if big => [(Kind::Kurgan, 12.5, 3.0, (12.5 / KURGAN_R) as f32), (Kind::StandingStone, 1.2, 2.0, 0.7)],
                    _ => [(Kind::StandingStone, 1.2, 2.0, 0.7), (Kind::StandingStone, 1.2, 2.0, 0.7)],
                };
                if let Some(&(kind, r, _, size)) = want.iter().find(|w| room.free(p, w.1, w.2, w.2)) {
                    put = Some((p, kind, r, size));
                    break;
                }
            }
            let Some((p, kind, r, size)) = put else { continue };
            let turn = if kind == Kind::TombSlab { slab_turn } else { 0.0 };
            props.push((p, turn, kind, rng.below(4) as u8, size));
            room.discs.push((p, r));
            mark(&mut covered, p);
        }
    }
}

/// A cemetery as the battlemap and the tiles lay it out (`park::layout`): its walks as gravel paths
/// kept clear, what it sets out.
pub fn park_layout(pk: &Park) -> ParkLayout {
    let Some(pl) = plan(pk) else { return ParkLayout::default() };
    let lw = pl.walks.first().map_or(6.0, |w| w.2);
    ParkLayout {
        paths: pl.walks.iter().map(|&(a, b, w)| (vec![a, b], w)).chain(pl.link.map(|(a, b)| (vec![a, b], lw))).collect(),
        keep: pl.walks.iter().map(|&(a, b, w)| (a, b, 0.5 * w)).chain(pl.link.map(|(a, b)| (a, b, 0.5 * lw))).collect(),
        props: pl.props,
        ..ParkLayout::default()
    }
}

/// The boundary's runs (world ft): its line but for the gap at the gate (a wall or a hedge; none on
/// the Steppe), for `rural::battle`; last, a walled gap's two piers where no lychgate stands over it.
pub fn bounds(l: &Layout) -> Vec<(P, P, Bound)> {
    let mut out = Vec::new();
    let mut piers = Vec::new();
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
                let (mut s0, mut s1) = (s - 0.5 * pl.gap, s + 0.5 * pl.gap);
                // (The wall's ends meet the piers' outer ends.)
                if pl.bound == Bound::Wall && !pl.props.iter().any(|p| p.2 == Kind::Lychgate) && s0 - PIER_FT > 1.0 && s1 + PIER_FT < le - 1.0 {
                    piers.push((add(a, mul(u, s0 - PIER_FT)), add(a, mul(u, s0)), Bound::Pier));
                    piers.push((add(a, mul(u, s1)), add(a, mul(u, s1 + PIER_FT)), Bound::Pier));
                    (s0, s1) = (s0 - PIER_FT, s1 + PIER_FT);
                }
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
    out.extend(piers);
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
                deck: None,
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

// Roadmap 7 (193): a town's graveyard lot (a building "graveyard", open to the sky) laid to its plot.

/// How far a graveyard lot's own wall stands in from its outline (ft): 2.5 ft and more off a street's
/// edge (as `rural`'s runs keep off a way), within `rural`'s 3 ft of a house beside it, so the house's
/// own wall closes the lot there.
pub const LOT_WALL_IN: f64 = 2.75;
/// How far in its wall stands on its way in's side (ft): 5 ft and more off a street's paving at the
/// lot's edge, so the side walls run on to meet it (`rural` leaves a gap where a run comes within 5 ft
/// of a way across it).
const LOT_FRONT_IN: f64 = 6.0;
/// A lot's walk from its gate to the stair (ft wide), and the gate's gap in its wall.
const LOT_WALK_W: f64 = 5.0;
const LOT_GAP: f64 = 7.0;

/// A graveyard lot laid out (world ft): its frame from the way in (the longest side facing a street,
/// else the longest: `along` it, `inward` from it), the wall's line (`LOT_WALL_IN` in from the outline,
/// `LOT_FRONT_IN` on the way in's side)
/// and its gate (on the line, `LOT_GAP` wide), the walk from the lot's edge to the catacomb stair's
/// mouth, the stair (its mouth at `stair` facing the gate, running `inward`; under a mausoleum 3
/// squares wide and 4 deep when one fits inside the wall, else the plain stair; `fits` false when not
/// even that does), and the headstones in rows along the way in's side, 10 ft apart, 15 ft between
/// the rows, the walk and the stair left clear. `own`: the lot's own wall and rows (not a temple
/// close's churchyard nor a village's, which are laid with their close or village).
#[derive(Clone, Debug)]
pub struct Lot {
    pub along: P,
    pub inward: P,
    pub gate: P,
    pub wall: Vec<P>,
    pub bound: Bound,
    pub walk: (P, P, f64),
    pub stair: P,
    pub mausoleum: bool,
    pub fits: bool,
    pub stones: Vec<(P, u8)>,
    pub own: bool,
    /// Whether the way in faces a street (else the lot's longest side).
    pub street: bool,
}

/// The stair's footprint in its frame (ft from its mouth: back, front, half across), with a foot
/// round it; and how far its door's slabs reach out in front of the mouth.
fn stair_box(mausoleum: bool) -> (f64, f64, f64, f64) {
    let sq = crate::battle::SQUARE_FT;
    if mausoleum { (-1.5 * sq - 1.0, 2.5 * sq + 1.0, 1.5 * sq + 1.0, 2.5 * sq) } else { (-0.5 * sq - 1.0, 1.6 * sq + 1.0, 0.6 * sq + 1.0, 1.5 * sq) }
}

/// A graveyard lot's plan (None: not an open lot, or too small for a wall's line). A pure function of
/// the lot and the layout's blocks and buildings round it (the same from every chunk).
pub fn lot(l: &Layout, b: &Building) -> Option<Lot> {
    let pts = &b.poly;
    let m = pts.len();
    if b.structure != Structure::Open || m < 3 {
        return None;
    }
    let c = centroid(pts);
    if !contains(pts, c) {
        return None;
    }
    let ccw = area(pts) > 0.0;
    let el = |e: usize| dist(pts[e], pts[(e + 1) % m]);
    let outward = |e: usize| {
        let d = mul(sub(pts[(e + 1) % m], pts[e]), 1.0 / el(e).max(1e-9));
        if ccw { [d[1], -d[0]] } else { [-d[1], d[0]] }
    };
    // A side faces a street where the ground 4 ft out is on a street drawn as a road or a square, or
    // in no block and under no building.
    let bb = grow(bbox(pts), 10.0);
    let blds: Vec<&Vec<P>> = l.buildings.iter().filter(|o| o.id != b.id && boxes_meet(bbox(&o.poly), bb)).map(|o| &o.poly).collect();
    let blks: Vec<&Vec<P>> = l.blocks.iter().filter(|k| boxes_meet(bbox(k), bb)).collect();
    let squares: Vec<&Vec<P>> = l.plazas.iter().filter(|p| boxes_meet(bbox(&p.pts), bb)).map(|p| &p.pts).collect();
    let ways: Vec<(P, P, f64)> = l.roads.iter().flat_map(|(q, _, w)| q.windows(2).map(move |s| (s[0], s[1], 0.5 * w + 1.0))).filter(|&(a, e, h)| boxes_meet(grow(bbox(&[a, e]), h), bb)).collect();
    let street = |q: P| {
        ways.iter().any(|&(a, e, h)| seg_dist(q, a, e) < h) || squares.iter().any(|k| contains(k, q)) || (!blds.iter().any(|o| contains(o, q)) && !blks.iter().any(|k| contains(k, q)))
    };
    let faces = |e: usize| [0.25, 0.5, 0.75].iter().filter(|&&s| street(add(lerp(pts[e], pts[(e + 1) % m], s), mul(outward(e), 4.0)))).count() >= 2;
    let longest = |a: &usize, b: &usize| el(*a).total_cmp(&el(*b)).then(b.cmp(a));
    let front = (0..m).filter(|&e| el(e) >= 15.0 && faces(e)).max_by(longest);
    // (Else the longest side with open ground before it, a lane's or a yard's, else the longest.)
    let open = |e: usize| [0.25, 0.5, 0.75].iter().filter(|&&s| !blds.iter().any(|o| contains(o, add(lerp(pts[e], pts[(e + 1) % m], s), mul(outward(e), 4.0))))).count() >= 2;
    let e = front.or_else(|| (0..m).filter(|&e| el(e) >= 15.0 && open(e)).max_by(longest)).unwrap_or_else(|| (0..m).max_by(longest).unwrap_or(0));
    let wall = inset(pts, &(0..m).map(|k| if k == e { LOT_FRONT_IN } else { LOT_WALL_IN }).collect::<Vec<_>>());
    if wall.len() < 3 {
        return None;
    }
    let a = pts[e];
    let t = mul(sub(pts[(e + 1) % m], a), 1.0 / el(e).max(1e-9));
    let n = mul(outward(e), -1.0);
    // The way in across from the lot's middle, kept off the side's ends.
    let half = 0.5 * LOT_GAP + 1.5;
    let x = if el(e) > 2.0 * half { dot(sub(c, a), t).clamp(half, el(e) - half) } else { 0.5 * el(e) };
    let edge = add(a, mul(t, x));
    let gate = add(edge, mul(n, LOT_FRONT_IN));
    let deep = exit(&wall, add(gate, mul(n, 0.5)), n) + 0.5;
    // The stair on the walk, nearest the lot's middle that it fits inside the wall: the mausoleum
    // first, else the plain stair.
    let fits_at = |at: P, maus: bool| {
        let (a0, a1, s, door) = stair_box(maus);
        let p = |u: f64, v: f64| add(at, add(mul(n, u), mul(t, v)));
        [p(a0, -s), p(a1, -s), p(a1, s), p(a0, s), p(a0, 0.0), p(a1, 0.0), p(0.5 * (a0 + a1), -s), p(0.5 * (a0 + a1), s), p(-door, 0.0)].iter().all(|q| contains(&wall, *q))
    };
    let d0 = dot(sub(c, gate), n).clamp(0.0, deep.max(0.0));
    let steps = (deep / 2.5) as i64 * 2 + 1;
    let tries: Vec<f64> = (0..steps).map(|k| d0 + 2.5 * ((k + 1) / 2) as f64 * if k % 2 == 1 { 1.0 } else { -1.0 }).filter(|d| (0.0..=deep).contains(d)).collect();
    let on = |d: f64| add(gate, mul(n, d));
    let found = [true, false].iter().find_map(|&maus| tries.iter().find(|&&d| fits_at(on(d), maus)).map(|&d| (on(d), maus)));
    let (stair, mausoleum, fits) = match found {
        Some((at, maus)) => (at, maus, true),
        None => (on(d0), false, false),
    };
    let (a0, a1, s, door) = stair_box(mausoleum);
    let own = !l.plazas.iter().any(|p| p.kind == super::PlazaKind::TempleClose && contains(&p.pts, c)) && !l.crofts.iter().any(|p| contains(&p.pts, c));
    // (A lot too small across for a boundary and the stair gets none, not stubs.)
    let across = |d: P| wall.iter().map(|q| dot(*q, d)).fold((f64::MAX, f64::MIN), |r, v| (r.0.min(v), r.1.max(v)));
    let (sn, st) = (across(n), across(t));
    let small = (sn.1 - sn.0).min(st.1 - st.0) < 35.0;
    let bound = match l.culture % 5 {
        _ if small => Bound::None,
        4 => Bound::None,
        1 => Bound::Hedge,
        _ => Bound::Wall,
    };
    let mouth = sub(stair, mul(n, door));
    let mut stones = Vec::new();
    if own {
        // Rows along the way in's side; the walk and the stair (and a yard round it) left clear.
        let key = hash2(0x6a7e_5193, crate::core::round(pts[0][0]) as i64, crate::core::round(pts[0][1]) as i64);
        let (lo, hi) = wall.iter().map(|q| dot(sub(*q, edge), t)).fold((f64::MAX, f64::MIN), |r, v| (r.0.min(v), r.1.max(v)));
        let clear = |p: P| {
            let r = sub(p, stair);
            let (u, v) = (dot(r, n), dot(r, t));
            (u > a0 - 3.0 && u < a1 + 3.0 && v.abs() < s + 3.0) || seg_dist(p, edge, mouth) < 0.5 * LOT_WALK_W + 2.0
        };
        let mut j = 0i64;
        while 5.0 + 15.0 * j as f64 <= deep - 2.0 {
            let dj = 5.0 + 15.0 * j as f64;
            for side in [-1i64, 1] {
                let mut k = 0i64;
                while 5.0 + 10.0 * k as f64 <= (if side < 0 { -lo } else { hi }) {
                    let p = add(gate, add(mul(n, dj), mul(t, side as f64 * (5.0 + 10.0 * k as f64))));
                    let h = hash3(key, j, side * (k + 1), 0x57);
                    if contains(&wall, p) && depth(&wall, p) >= 2.0 && !clear(p) && unit(h) < 0.8 {
                        stones.push((p, (hash3(key, j, side * (k + 1), 0x58) % 4) as u8));
                    }
                    k += 1;
                }
            }
            j += 1;
        }
    }
    Some(Lot { along: t, inward: n, gate, wall, bound, walk: (edge, mouth, LOT_WALK_W), stair, mausoleum, fits, stones, own, street: front.is_some() })
}

/// The walls' (or hedges') runs round the layout's own graveyard lots near `near` (world ft): each
/// lot's wall line but for the gap at its gate and where a house's wall closes it (the line within
/// 3 ft of a roof); a run cut by a roof runs on to meet it (within a foot), for `rural::battle`.
pub fn lot_bounds(l: &Layout, near: [f64; 4]) -> Vec<(P, P, Bound)> {
    let mut out = Vec::new();
    for b in l.buildings.iter().filter(|b| b.structure == Structure::Open && boxes_meet(grow(bbox(&b.poly), 8.0), near)) {
        let Some(lp) = lot(l, b).filter(|p| p.own && p.bound != Bound::None) else { continue };
        let bb = grow(bbox(&b.poly), 10.0);
        let roofs: Vec<&Vec<P>> = l.buildings.iter().filter(|o| o.id != b.id && o.structure == Structure::Roofed && boxes_meet(bbox(&o.poly), bb)).map(|o| &o.poly).collect();
        let gap = |q: P| roofs.iter().map(|r| depth_or_gap(r, q)).fold(f64::MAX, f64::min);
        // The wall's loop in 0.25 ft cells: a cell stands unless it is in the gate's gap or within 3 ft
        // of a roof (a house's wall closes the lot there). Each run of standing cells is laid per
        // side; where a roof cut it, a stub runs on straight to the roof, to within half a foot.
        let m = lp.wall.len();
        let cum: Vec<f64> = (0..=m).scan(0.0, |s, e| {
            let v = *s;
            *s += dist(lp.wall[e % m], lp.wall[(e + 1) % m]);
            Some(v)
        }).collect();
        let per = cum[m];
        if per < 1.0 {
            continue;
        }
        let nc = (per / 0.25).ceil() as usize;
        let at = |s: f64| {
            let s = s.rem_euclid(per);
            let e = (0..m).rev().find(|&e| cum[e] <= s).unwrap_or(0);
            let (a, q) = (lp.wall[e], lp.wall[(e + 1) % m]);
            lerp(a, q, (s - cum[e]) / (cum[e + 1] - cum[e]).max(1e-9))
        };
        let cell = |i: usize| per * (i as f64 + 0.5) / nc as f64;
        let state = |i: usize| -> u8 {
            let p = at(cell(i % nc));
            if dist(p, lp.gate) < 0.5 * LOT_GAP { 2 } else if gap(p) < 3.0 { 1 } else { 0 }
        };
        let st: Vec<u8> = (0..nc).map(state).collect();
        let Some(start) = st.iter().position(|&x| x != 0) else {
            for e in 0..m {
                out.push((lp.wall[e], lp.wall[(e + 1) % m], lp.bound));
            }
            continue;
        };
        let mut i = start;
        let stop = start + nc;
        while i < stop {
            if st[i % nc] != 0 {
                i += 1;
                continue;
            }
            let i0 = i;
            while i < stop && st[i % nc] == 0 {
                i += 1;
            }
            let (s0, s1) = (per * i0 as f64 / nc as f64, per * i as f64 / nc as f64);
            // The run's pieces, a corner apart.
            let mut x = s0;
            while x < s1 - 1e-9 {
                let xm = x.rem_euclid(per);
                let e = (0..m).rev().find(|&e| cum[e] <= xm).unwrap_or(0);
                let to = (x + (cum[e + 1] - xm).max(1e-6)).min(s1);
                if to - x > 0.3 {
                    out.push((at(x), at(to - 1e-6), lp.bound));
                }
                x = to;
            }
            // (An end the roof cut; one by the gate's gap is left open.)
            for (end, before) in [(s0, st[(i0 + nc - 1) % nc]), (s1, st[i % nc])] {
                if before != 1 {
                    continue;
                }
                let p = at(end);
                let mut best: Option<(f64, P)> = None;
                for r in &roofs {
                    for k in 0..r.len() {
                        let (a, q) = (r[k], r[(k + 1) % r.len()]);
                        let ab = sub(q, a);
                        let tt = (dot(sub(p, a), ab) / dot(ab, ab).max(1e-9)).clamp(0.0, 1.0);
                        let c = add(a, mul(ab, tt));
                        let d = dist(p, c);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, c));
                        }
                    }
                }
                if let Some((d, c)) = best.filter(|&(d, _)| d > 0.6 && d < 8.0) {
                    out.push((p, add(p, mul(sub(c, p), (d - 0.4) / d)), lp.bound));
                }
            }
        }
    }
    out
}
