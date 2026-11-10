//! Rounder curtains (item 104 slice 2c): a round-walled town's curtains (North, Dry, Steppe) bowed
//! out between their corners, and their corners rounded with arcs of 40-80 ft (Steppe's larger, its
//! corners drawn in further: a rough oval), once the town is laid, so each bow and each corner is cut
//! back to what fits: nothing laid (a building, block, square, park, field, road, bridge or pier),
//! no water, other wall, network road or the map's edge within `CLEAR` ft of the new line. What
//! `outline::lay` holds stays (the gates and the 26 ft to their towers, the corners of a castle's
//! patch, on the water or a river, and where a main street meets the wall); so does a stretch
//! between two held corners. The towers move onto the new line, each at its share of its stretch;
//! a run where nothing fits keeps the line `outline::round` laid.

use super::geom::*;
use super::{Layout, Site, TownPlan, Ward};
use crate::core::hash::FastMap;

/// The new line stands this far (ft) at least from anything laid, and a tower its reach and this much.
const CLEAR: f64 = 8.0;
const TOWER_CLEAR: f64 = 4.0;
/// A curtain's sag (share of its stretch, at most `SAG_MOST` ft), tried in turn; Steppe's first.
const SAG: [f64; 11] = [0.06, 0.05, 0.04, 0.035, 0.03, 0.025, 0.02, 0.015, 0.01, 0.005, 0.0];
const SAG_STEPPE: [f64; 11] = [0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.025, 0.02, 0.01, 0.005, 0.0];
const SAG_MOST: f64 = 16.0;
/// A corner's arc and how far it is drawn in toward the turn (both by the level's share), tried in
/// turn; the last is the arc `outline::round` laid.
const LEVELS: [f64; 8] = [1.0, 0.85, 0.7, 0.55, 0.4, 0.25, 0.12, 0.0];
const ARC: (f64, f64) = (40.0, 80.0);
const ARC_STEPPE: (f64, f64) = (60.0, 120.0);
const DRAW_IN: f64 = 12.0;
/// How far in a corner may come, drawn in and cut by its arc (ft).
const DEPTH: f64 = 13.0;
const DRAW_IN_STEPPE: f64 = 20.0;
/// Points along a bowed stretch (ft apart), and along the new line where it is tested.
const BODY_STEP: f64 = 25.0;
const TEST_STEP: f64 = 4.0;
const CELL: f64 = 48.0;
const PAD: f64 = 32.0;

fn unit(v: P) -> P {
    mul(v, 1.0 / len(v).max(1e-9))
}

fn same(a: P, b: P) -> bool {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() < 1e-6
}

fn left(u: P) -> P {
    [-u[1], u[0]]
}

/// What the new line keeps clear of, by `CELL`-ft cell: outlines (nothing within the clearance, nor
/// inside) and segments with their half widths.
struct Laid {
    polys: Vec<Vec<P>>,
    segs: Vec<(P, P, f64)>,
    grid: FastMap<(i64, i64), (Vec<u32>, Vec<u32>)>,
}

impl Laid {
    fn cells(lo: P, hi: P) -> impl Iterator<Item = (i64, i64)> {
        let (x0, y0) = (crate::core::floor((lo[0] - PAD) / CELL) as i64, crate::core::floor((lo[1] - PAD) / CELL) as i64);
        let (x1, y1) = (crate::core::floor((hi[0] + PAD) / CELL) as i64, crate::core::floor((hi[1] + PAD) / CELL) as i64);
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
    }

    fn poly(&mut self, p: &[P]) {
        if p.len() < 3 {
            return;
        }
        let (lo, hi) = p.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1])], [hi[0].max(q[0]), hi[1].max(q[1])]));
        let k = self.polys.len() as u32;
        self.polys.push(p.to_vec());
        for c in Self::cells(lo, hi) {
            self.grid.entry(c).or_default().0.push(k);
        }
    }

    fn seg(&mut self, a: P, b: P, half: f64) {
        let k = self.segs.len() as u32;
        self.segs.push((a, b, half));
        let (lo, hi) = ([a[0].min(b[0]) - half, a[1].min(b[1]) - half], [a[0].max(b[0]) + half, a[1].max(b[1]) + half]);
        for c in Self::cells(lo, hi) {
            self.grid.entry(c).or_default().1.push(k);
        }
    }

    /// How far `p` stands from what is laid (0 inside it), up to `cap` (under `PAD`).
    fn gap(&self, p: P, cap: f64) -> [f64; 2] {
        let key = (crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64);
        let Some((ps, ss)) = self.grid.get(&key) else { return [cap; 2] };
        let mut g = [cap; 2];
        for &k in ps {
            let q = &self.polys[k as usize];
            let m = q.len();
            g[0] = if contains(q, p) { 0.0 } else { (0..m).map(|e| seg_dist(p, q[e], q[(e + 1) % m])).fold(g[0], f64::min) };
        }
        for &k in ss {
            let (a, b, h) = self.segs[k as usize];
            g[1] = g[1].min((seg_dist(p, a, b) - h).max(0.0));
        }
        g
    }

    /// `p` as clear of what is laid as `q` was, or clear by `cap`: outlines and segments each.
    fn no_nearer(&self, p: P, q: P, cap: f64, slack: f64) -> bool {
        let (a, b) = (self.gap(p, cap), self.gap(q, cap));
        a[0] >= b[0].min(cap) - slack - 1e-6 && a[1] >= b[1].min(cap) - slack - 1e-6
    }

}

/// A run of a wall piece between two points that stay (its ends, `pts[0]` and the last), its free
/// corners between them.
struct Run {
    pts: Vec<P>,
}

/// The new line of a run at the levels given, its segments' owners (`Ok` a stretch, `Err` a corner),
/// each stretch's section (from the middle of the arc at its start, or the run's end, to that at its
/// end) as points of the line, and each corner's arc middle.
struct Line {
    pts: Vec<P>,
    owner: Vec<Result<usize, usize>>,
    sections: Vec<(usize, usize)>,
    mids: Vec<P>,
}

/// A stretch's sag profile over its length (`w` 0 to 1), with a level slope at an end that stays
/// (`fixed`), and its slope (per `w`) at each end.
fn profile(w: f64, fixed: (bool, bool)) -> f64 {
    let pi = std::f64::consts::PI;
    // (sin(pi w) sin(pi w / 2) peaks at 0.7698 at w = 0.6082.)
    const HALF_PEAK: f64 = 0.769800;
    match fixed {
        (false, false) => libm::sin(pi * w),
        (true, true) => {
            let s = libm::sin(pi * w);
            s * s
        }
        (true, false) => libm::sin(pi * w) * libm::sin(0.5 * pi * w) / HALF_PEAK,
        (false, true) => libm::sin(pi * w) * libm::sin(0.5 * pi * (1.0 - w)) / HALF_PEAK,
    }
}

fn end_slopes(fixed: (bool, bool)) -> (f64, f64) {
    let pi = std::f64::consts::PI;
    const HALF_PEAK: f64 = 0.769800;
    match fixed {
        (false, false) => (pi, -pi),
        (true, true) => (0.0, 0.0),
        (true, false) => (0.0, -pi / HALF_PEAK),
        (false, true) => (pi / HALF_PEAK, 0.0),
    }
}

/// The arc tangent to the lines in along `d1` and out along `d2` at `v`, of radius `rad`, as
/// `outline::round` lays it (10-degree steps), and its middle; `None` where the line hardly turns.
fn arc(v: P, d1: P, d2: P, rad: f64) -> Option<(Vec<P>, P)> {
    let th = libm::acos(dot(d1, d2).clamp(-1.0, 1.0));
    if th < 2f64.to_radians() || rad < 1.0 {
        return None;
    }
    let side = cross(d1, d2);
    let t = rad * libm::tan(0.5 * th);
    let t1 = sub(v, mul(d1, t));
    let toward = if side > 0.0 { [-d1[1], d1[0]] } else { [d1[1], -d1[0]] };
    let c = add(t1, mul(toward, rad));
    let a0 = libm::atan2(t1[1] - c[1], t1[0] - c[0]);
    let m = crate::core::ceil(th / 10f64.to_radians()).max(2.0) as usize;
    let sweep = if side > 0.0 { th } else { -th };
    let pts: Vec<P> = (0..=m).map(|i| {
        let a = a0 + sweep * i as f64 / m as f64;
        [c[0] + rad * libm::cos(a), c[1] + rad * libm::sin(a)]
    }).collect();
    Some((pts, add(c, mul(unit(sub(v, c)), rad))))
}

/// The run's shape: per stretch its outward side (`None`: the town is on neither side or both),
/// whether its ends stay, per corner its turn (radians), the 2a arc's radius there, the largest
/// arc and draw-in it may take.
struct Shape {
    out: Vec<Option<P>>,
    fixed: Vec<(bool, bool)>,
    rad0: Vec<f64>,
    arc_most: Vec<f64>,
    draw_most: Vec<f64>,
    sag_most: Vec<f64>,
}

fn build(run: &Run, sh: &Shape, sag: &[f64], lv: &[usize], levels: &[f64]) -> Line {
    let n = run.pts.len();
    // (Corners drawn in toward their turn: `lv` 0 the most.)
    let mut at: Vec<P> = run.pts.clone();
    for j in 1..n - 1 {
        let (u1, u2) = (unit(sub(run.pts[j], run.pts[j - 1])), unit(sub(run.pts[j + 1], run.pts[j])));
        let th = libm::acos(dot(u1, u2).clamp(-1.0, 1.0));
        let k = levels[lv[j - 1]];
        if th > 1f64.to_radians() && k > 0.0 {
            // (Drawn in and cut by its arc by `DEPTH` at most, all told.)
            let (sec, tn) = (1.0 / libm::cos(0.5 * th), libm::tan(0.5 * th));
            let room = 0.42 * dist(run.pts[j - 1], run.pts[j]).min(dist(run.pts[j], run.pts[j + 1]));
            let cut = (sh.arc_most[j - 1] * k).max(sh.rad0[j - 1]).min(room / tn.max(1e-9)) * (sec - 1.0);
            let e = (sh.draw_most[j - 1] * k).min(((DEPTH - cut) / sec).max(0.0));
            at[j] = add(run.pts[j], mul(unit(sub(u2, u1)), e * sec));
        }
    }
    // (Each stretch: its chord, sag and end directions.)
    let st: Vec<(P, P, f64, P, P)> = (0..n - 1)
        .map(|i| {
            let (a, b) = (at[i], at[i + 1]);
            let l = dist(a, b).max(1e-6);
            let u = unit(sub(b, a));
            let s = sh.out[i].map_or(0.0, |_| (sag[i] * l).min(sh.sag_most[i]));
            let o = sh.out[i].unwrap_or([0.0, 0.0]);
            let (s0, s1) = end_slopes(sh.fixed[i]);
            (u, o, s, unit(add(u, mul(o, s * s0 / l))), unit(add(u, mul(o, s * s1 / l))))
        })
        .collect();
    // (Each corner's arc, its tangent length at most 42% of the shorter stretch.)
    let mut arcs: Vec<Option<(Vec<P>, P, f64)>> = Vec::new();
    for j in 1..n - 1 {
        let (d1, d2) = (st[j - 1].4, st[j].3);
        let th = libm::acos(dot(d1, d2).clamp(-1.0, 1.0));
        let k = levels[lv[j - 1]];
        let tn = libm::tan(0.5 * th);
        // (The least: the arc `outline::round` laid, as it laid it.)
        let mut rad = if k > 0.0 { (sh.arc_most[j - 1] * k).max(sh.rad0[j - 1]) } else { sh.rad0[j - 1] };
        let room = 0.42 * dist(at[j - 1], at[j]).min(dist(at[j], at[j + 1]));
        if k > 0.0 && rad * tn > room {
            rad = room / tn.max(1e-9);
        }
        // (Nor cutting in, or bulging out, by more than `DEPTH` where the bows turn it further.)
        let sec = 1.0 / libm::cos(0.5 * th);
        if k > 0.0 && rad * (sec - 1.0) > DEPTH {
            rad = DEPTH / (sec - 1.0);
        }
        arcs.push(arc(at[j], d1, d2, rad).map(|(p, m)| (p, m, rad * tn)));
    }
    let mut line = Line { pts: vec![at[0]], owner: Vec::new(), sections: Vec::new(), mids: Vec::new() };
    let push = |line: &mut Line, p: P, o: Result<usize, usize>| {
        if !same(*line.pts.last().unwrap(), p) {
            line.pts.push(p);
            line.owner.push(o);
        }
    };
    for i in 0..n - 1 {
        let start = line.pts.len() - 1;
        let (u, o, s, _, _) = st[i];
        let l = dist(at[i], at[i + 1]);
        // (The second half of the arc at its start.)
        let t0 = if i > 0 {
            match &arcs[i - 1] {
                Some((pts, _, t)) => {
                    for p in &pts[pts.len() / 2..] {
                        push(&mut line, *p, Err(i - 1));
                    }
                    *t
                }
                None => 0.0,
            }
        } else {
            0.0
        };
        let t1 = if i + 1 < n - 1 { arcs[i].as_ref().map_or(0.0, |a| a.2) } else { 0.0 };
        if s > 0.0 {
            let (w0, w1) = (t0 / l, 1.0 - t1 / l);
            let m = crate::core::ceil((w1 - w0) * l / BODY_STEP).max(1.0) as usize;
            for k in 1..m {
                let w = w0 + (w1 - w0) * k as f64 / m as f64;
                push(&mut line, add(add(at[i], mul(u, w * l)), mul(o, s * profile(w, sh.fixed[i]))), Ok(i));
            }
        }
        // (The first half of the arc at its end, to its middle; else the stretch's end.)
        match (i + 1 < n - 1).then(|| arcs[i].as_ref()).flatten() {
            Some((pts, mid, _)) => {
                for p in &pts[..pts.len() / 2] {
                    push(&mut line, *p, Ok(i));
                }
                // (Its middle: a point of the line only where the arc's points don't have it.)
                let on = pts.len() % 2 == 1;
                push(&mut line, if on { pts[pts.len() / 2] } else { *mid }, Err(i));
                line.mids.push(*mid);
            }
            None => {
                push(&mut line, at[i + 1], Ok(i));
                if i + 1 < n - 1 {
                    line.mids.push(at[i + 1]);
                }
            }
        }
        line.sections.push((start, line.pts.len() - 1));
    }
    line
}

/// The point of `pts` nearest `q`, square to it (`None` where there is none but an end).
fn onto(pts: &[P], q: P) -> Option<P> {
    pts.windows(2)
        .filter_map(|s| {
            let d = sub(s[1], s[0]);
            let f = dot(sub(q, s[0]), d) / dot(d, d).max(1e-9);
            (0.0..=1.0).contains(&f).then(|| lerp(s[0], s[1], f))
        })
        .min_by(|x, y| dist(*x, q).total_cmp(&dist(*y, q)))
}

/// The point at share `f` of the way along `pts[a..=b]`.
fn along(pts: &[P], a: usize, b: usize, f: f64) -> P {
    let total: f64 = (a..b).map(|k| dist(pts[k], pts[k + 1])).sum();
    let mut want = f * total;
    for k in a..b {
        let d = dist(pts[k], pts[k + 1]);
        if want <= d || k + 1 == b {
            return lerp(pts[k], pts[k + 1], (want / d.max(1e-9)).clamp(0.0, 1.0));
        }
        want -= d;
    }
    pts[a]
}

/// Bows and rounds the town wall's curtains in `l` (round outlines only; `laid` the pieces and towers
/// `outline::lay` gave). Called once the town is laid, in local ft.
pub(super) fn curtains(site: &Site, plan: &TownPlan, mesh: &super::mesh::Mesh, ward: &[Option<Ward>], l: &mut Layout, laid: (&[Vec<P>], &[P])) {
    use super::wallstyle::Outline;
    let style = l.wall_style;
    if style.outline != Outline::Round || laid.0.is_empty() || l.walls.len() < laid.0.len() || (0..laid.0.len()).any(|p| l.walls[p] != laid.0[p]) {
        return;
    }
    let steppe = style.group % 5 == 4;
    // (The pieces as `wall_pieces_spaced` lays them, before `outline::round`: the same in number and
    // order, and so their towers.)
    let (mut raw, mut raw_towers) = (Vec::new(), Vec::new());
    for (chain, closed, along) in &plan.wall_runs {
        let corners: Vec<(P, bool)> = chain.iter().map(|&v| (mesh.pos[v], plan.gates.contains(&v))).collect();
        let (p, t, _, _) = super::wall_pieces_spaced(&corners, *closed, Some(along), super::SET);
        raw.extend(p);
        raw_towers.extend(t);
    }
    if raw.len() != laid.0.len() || raw_towers.len() != laid.1.len() {
        return;
    }
    // (What `outline::lay` holds.)
    let mut held: Vec<P> = (0..mesh.pos.len()).filter(|&v| plan.pinned[v]).map(|v| mesh.pos[v]).collect();
    for (i, f) in mesh.faces.iter().enumerate() {
        if ward.get(i).copied().flatten() == Some(Ward::Castle) {
            held.extend(f.iter().map(|&v| mesh.pos[v]));
        }
    }
    for &(a, b) in &plan.main_edges {
        held.push(mesh.pos[a]);
        held.push(mesh.pos[b]);
    }
    let keep = |p: P| held.iter().any(|q| same(*q, p));
    let inner: Vec<Vec<P>> = (0..plan.n_inner).map(|i| mesh.face_pts(i)).collect();
    let in_town = |p: P| inner.iter().any(|f| contains(f, p));
    // (What is laid, but for the town's wall: others' walls a wall's width off.)
    let mut obst = Laid { polys: Vec::new(), segs: Vec::new(), grid: FastMap::default() };
    // (A tower keeps clear of the buildings and the roads, as far as it did: `solid`.)
    let mut solid = Laid { polys: Vec::new(), segs: Vec::new(), grid: FastMap::default() };
    for p in l.buildings.iter().map(|b| &b.poly) {
        obst.poly(p);
        solid.poly(p);
    }
    // (Outside the town, its patches' blocks; inside it, the buildings on them.)
    for p in l.blocks.iter().filter(|b| b.len() >= 3 && !in_town(centroid(b))) {
        obst.poly(p);
    }
    for p in l.plazas.iter().map(|q| &q.pts) { obst.poly(p); }
    for p in l.parks.iter().map(|q| &q.pts).chain(l.crofts.iter().map(|c| &c.pts)) {
        obst.poly(p);
    }
    for p in l.fields.iter() { obst.poly(p); }
    for p in l.bridges.iter().chain(&l.piers).chain(l.grounds.iter().map(|g| &g.1)) {
        obst.poly(p);
    }
    // (Not the paved strip a wall stands on, between it and the blocks inside or out.)
    let on_wall = |p: P| laid.0.iter().any(|w| w.windows(2).any(|s| seg_dist(p, s[0], s[1]) < 27.0));
    for (pts, c, w) in &l.roads {
        for s in pts.windows(2) {
            if *c == 3 && on_wall(s[0]) && on_wall(s[1]) && on_wall(lerp(s[0], s[1], 0.5)) {
                continue;
            } else {
                obst.seg(s[0], s[1], 0.5 * w);
                solid.seg(s[0], s[1], 0.5 * w);
            }
        }
    }
    for w in l.walls.iter().skip(laid.0.len()) {
        for s in w.windows(2) {
            obst.seg(s[0], s[1], CLEAR);
        }
    }
    let half = 0.5 * style.thickness();
    let clear = CLEAR.max(half + 5.0);
    let t0 = site.t0;
    let (mw, mh) = (t0.height.w as f64 * t0.cell_ft, t0.height.h as f64 * t0.cell_ft);
    let open = |p: P| {
        let w = add(p, site.center);
        !super::Ground::near_water(site, p, CLEAR)
            && !site.near_network_road(p, CLEAR)
            && w[0] > 40.0
            && w[1] > 40.0
            && w[0] < mw - 40.0
            && w[1] < mh - 40.0
            && len(p) < site.reach - 20.0
    };
    let reach = if style.has_square() { 15.6 } else { 11.0 } + TOWER_CLEAR;
    let (arc_r, draw, sags) = if steppe { (ARC_STEPPE, DRAW_IN_STEPPE, &SAG_STEPPE) } else { (ARC, DRAW_IN, &SAG) };
    // (Every tower that isn't the town's (gate towers, castles', those added by hand).)
    let town_towers: Vec<P> = laid.1.to_vec();
    let others: Vec<P> = l.towers.iter().copied().filter(|t| !town_towers.iter().any(|o| same(*o, *t))).chain(l.gate_towers.iter().copied()).chain(l.extra_towers.iter().map(|e| e.0)).collect();
    let mut moved: Vec<(P, P)> = Vec::new();
    let mut new_walls: Vec<(usize, Vec<P>)> = Vec::new();
    'pieces: for (pi, rp) in raw.iter().enumerate() {
        let lp = &laid.0[pi];
        if rp.len() < 3 || !same(rp[0], lp[0]) || !same(rp[rp.len() - 1], lp[lp.len() - 1]) {
            continue;
        }
        // (Runs between the points that stay.)
        let mut runs: Vec<Run> = Vec::new();
        let mut cur = vec![rp[0]];
        for (k, &q) in rp.iter().enumerate().skip(1) {
            cur.push(q);
            if k + 1 == rp.len() || keep(q) {
                runs.push(Run { pts: std::mem::take(&mut cur) });
                cur.push(q);
            }
        }
        let mut out_pts: Vec<P> = Vec::new();
        let mut any = false;
        for run in &runs {
            let n = run.pts.len();
            // (The 2a line along this run: from its start to its end in the laid piece.)
            let (a, b) = (lp.iter().position(|q| same(*q, run.pts[0])), lp.iter().rposition(|q| same(*q, run.pts[n - 1])));
            let Some((a, b)) = a.zip(b).filter(|(a, b)| a < b) else { continue 'pieces };
            let old: Vec<P> = lp[a..=b].to_vec();
            let fixed_end = |k: usize| k == 0 || k == n - 1;
            let ends_held = keep(run.pts[0]) && keep(run.pts[n - 1]);
            if n == 2 && (ends_held || dist(run.pts[0], run.pts[1]) < 150.0) {
                out_pts.extend(old.iter().skip(if out_pts.is_empty() { 0 } else { 1 }));
                continue;
            }
            let mut sh = Shape { out: Vec::new(), fixed: Vec::new(), rad0: Vec::new(), arc_most: Vec::new(), draw_most: Vec::new(), sag_most: Vec::new() };
            for i in 0..n - 1 {
                let (p, q) = (run.pts[i], run.pts[i + 1]);
                let (u, mid) = (unit(sub(q, p)), lerp(p, q, 0.5));
                let (l_in, r_in) = (in_town(add(mid, mul(left(u), 4.0))), in_town(add(mid, mul(left(u), -4.0))));
                sh.out.push(match (l_in, r_in) {
                    (true, false) => Some(mul(left(u), -1.0)),
                    (false, true) => Some(left(u)),
                    _ => None,
                });
                sh.fixed.push((fixed_end(i), fixed_end(i + 1)));
                sh.sag_most.push(SAG_MOST);
            }
            for j in 1..n - 1 {
                let (p, v, q) = (run.pts[j - 1], run.pts[j], run.pts[j + 1]);
                let (u1, u2) = (unit(sub(v, p)), unit(sub(q, v)));
                let th = libm::acos(dot(u1, u2).clamp(-1.0, 1.0));
                // (The arc `outline::round` laid: from where it leaves the stretch before.)
                let t1 = old.iter().copied().filter(|x| !same(*x, v) && seg_dist(*x, p, v) < 1e-6 && dist(*x, v) < 0.5 * dist(p, v) + 1e-6).map(|x| dist(x, v)).fold(f64::MAX, f64::min);
                let on = old.iter().any(|x| same(*x, v));
                sh.rad0.push(if on || t1 == f64::MAX || th < 2f64.to_radians() { 0.0 } else { t1 / libm::tan(0.5 * th) });
                let short = dist(p, v).min(dist(v, q));
                sh.arc_most.push((0.3 * short).clamp(arc_r.0, arc_r.1));
                sh.draw_most.push(draw.min(0.05 * short));
            }
            let (ns, nc) = (n - 1, n - 2);
            let (mut sl, mut cl) = (vec![0usize; ns], vec![0usize; nc]);
            let last = (sags.len() - 1, LEVELS.len() - 1);
            let off_old = |p: P| old.windows(2).all(|s| seg_dist(p, s[0], s[1]) > 0.5);
            // (Clear of what is laid, or no nearer to it than the old line was.)
            let nearest_old = |p: P| {
                old.windows(2).map(|s| {
                    let d = sub(s[1], s[0]);
                    lerp(s[0], s[1], (dot(sub(p, s[0]), d) / dot(d, d).max(1e-9)).clamp(0.0, 1.0))
                }).min_by(|x, y| dist(*x, p).total_cmp(&dist(*y, p))).unwrap_or(p)
            };
            let fits = |p: P| !off_old(p) || (open(p) && obst.no_nearer(p, nearest_old(p), clear, 0.3));
            // (The line at the levels given, its towers, and which bows and corners don't fit.)
            let trial = |sl: &[usize], cl: &[usize]| {
            let sag: Vec<f64> = sl.iter().map(|&k| sags[k]).collect();
            let line = build(run, &sh, &sag, &cl, &LEVELS);
            let (mut bad_s, mut bad_c) = (vec![false; ns], vec![false; nc]);
            for (k, s) in line.pts.windows(2).enumerate() {
                let m = crate::core::ceil(dist(s[0], s[1]) / TEST_STEP).max(1.0) as usize;
                if (0..=m).map(|j| lerp(s[0], s[1], j as f64 / m as f64)).any(|p| !fits(p)) {
                    match line.owner[k] {
                        Ok(i) => bad_s[i] = true,
                        Err(j) => bad_c[j] = true,
                    }
                }
            }
            // (The towers along it: corner towers to their arc's middle, the rest at their share
            // of their stretch's section.)
            let mut tw: Vec<(usize, P)> = Vec::new();
            for (ti, t) in raw_towers.iter().enumerate() {
                let Some(i) = (0..ns).find(|&i| seg_dist(*t, run.pts[i], run.pts[i + 1]) < 1e-6) else { continue };
                let f = dot(sub(*t, run.pts[i]), unit(sub(run.pts[i + 1], run.pts[i]))) / dist(run.pts[i], run.pts[i + 1]).max(1e-9);
                let (i, f) = if f > 1.0 - 1e-6 && i + 1 < ns { (i + 1, 0.0) } else { (i, f) };
                if (i == 0 && f < 1e-6) || (i + 1 == ns && f > 1.0 - 1e-6) {
                    continue;
                }
                let (sa, sb) = line.sections[i];
                let old_at = laid.1[ti];
                // (On a stretch as `outline::round` left it, where that put it.)
                let kept = sag[i] == 0.0 && (i == 0 || cl[i - 1] == last.1) && (i + 1 == ns || cl[i] == last.1);
                let p = if kept { old_at } else if f < 1e-6 && i > 0 { line.mids[i - 1] } else { onto(&line.pts[sa..=sb], old_at).unwrap_or_else(|| along(&line.pts, sa, sb, f)) };
                let own: Result<usize, usize> = if f < 1e-6 { Err(i - 1) } else { Ok(i) };
                let ok = dist(p, old_at) < 0.5
                    || (solid.no_nearer(p, old_at, reach, 1.0)
                        && !super::Ground::near_water(site, p, 3.0)
                        && others.iter().chain(town_towers.iter().filter(|o| !same(**o, old_at))).all(|o| dist(*o, p) >= dist(*o, old_at).min(40.0)));
                if !ok {
                    match own {
                        Ok(i) => bad_s[i] = true,
                        Err(j) => bad_c[j] = true,
                    }
                }
                tw.push((ti, p));
            }
                (line, tw, bad_s, bad_c)
            };
            // (First the corners, the curtains straight: each that doesn't fit, or whose stretch
            // doesn't, cut back a step; then each curtain's bow, the corners as they are.)
            let mut done: Option<(Line, Vec<(usize, P)>)> = None;
            sl.iter_mut().for_each(|k| *k = last.0);
            for _ in 0..LEVELS.len() * 2 + 2 {
                let (_, _, bad_s, bad_c) = trial(&sl, &cl);
                let mut stepped = false;
                for j in 0..nc {
                    if (bad_c[j] || bad_s[j] || bad_s[j + 1]) && cl[j] < last.1 {
                        cl[j] += 1;
                        stepped = true;
                    }
                }
                if !stepped {
                    break;
                }
            }
            for i in 0..ns {
                for k in 0..=last.0 {
                    sl[i] = k;
                    let (_, _, bad_s, bad_c) = trial(&sl, &cl);
                    if !bad_s.iter().chain(&bad_c).any(|b| *b) {
                        break;
                    }
                }
            }
            let (line, tw, bad_s, bad_c) = trial(&sl, &cl);
            if !bad_s.iter().any(|b| *b) && !bad_c.iter().any(|b| *b) {
                done = Some((line, tw));
            }
            // (Nothing fits, or only `outline::round`'s line: that line, its towers where they are.)
            match done.filter(|_| sl.iter().any(|&k| k < last.0) || cl.iter().any(|&k| k < last.1)) {
                Some((line, tw)) => {
                    any = true;
                    out_pts.extend(line.pts.iter().skip(if out_pts.is_empty() { 0 } else { 1 }));
                    for (ti, p) in tw {
                        moved.push((laid.1[ti], p));
                    }
                }
                None => out_pts.extend(old.iter().skip(if out_pts.is_empty() { 0 } else { 1 })),
            }
        }
        if any {
            new_walls.push((pi, out_pts));
        }
    }
    for (pi, pts) in new_walls {
        l.walls[pi] = pts;
    }
    // (The towers moved, with their shapes and the way they face.)
    for (old, new) in moved {
        if same(old, new) {
            continue;
        }
        for t in l.towers.iter_mut().filter(|t| same(**t, old)) {
            *t = new;
        }
        let along = l
            .walls
            .iter()
            .flat_map(|w| w.windows(2))
            .filter(|s| seg_dist(new, s[0], s[1]) < 1.0)
            .max_by(|x, y| dist(x[0], x[1]).total_cmp(&dist(y[0], y[1])))
            .map(|s| unit(sub(s[1], s[0])));
        for f in l.tower_forms.iter_mut().filter(|f| same(f.0, old)) {
            f.0 = new;
            if let Some(u) = along {
                // (A D faces out, the way it did; the rest run along the wall.)
                f.2 = if f.1 & 15 == super::wallstyle::TowerShape::D as u8 {
                    let n = left(u);
                    if dot(n, f.2) < 0.0 { mul(n, -1.0) } else { n }
                } else {
                    u
                };
            }
        }
        for s in l.square_towers.iter_mut().filter(|s| same(s.0, old)) {
            s.0 = new;
            if let Some(u) = along {
                s.1 = u;
            }
        }
    }
}
