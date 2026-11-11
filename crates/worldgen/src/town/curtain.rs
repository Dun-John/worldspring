//! Rounder curtains (item 104 slices 2c, 2e): a round-walled town's curtains (North, Dry, Steppe)
//! bowed out between their corners as the wall is laid, before the outskirts: each stretch a Hermite
//! curve through its two corners, leaving a free corner along the corner's bisector (so the wall runs
//! on round it, no kink) with a tangent of `PULL` times the stretch, Steppe's longer (a rough oval).
//! Outward only: a concave corner, one that stays, a gate's end and a stretch the town is on neither
//! side of all leave along the stretch. Each bow is cut back to what fits: no water, network road,
//! road or street, bridge, castle patch, other piece of the wall or the map's edge within `CLEAR` ft,
//! nor nearer the outskirts' other sides than the old line. What `outline::lay` holds stays (the
//! gates and the 26 ft to their towers, the corners of a castle's patch, on the water or a river, and
//! where a main street meets the wall). The towers move onto the new line; where a corner's two
//! stretches both stay straight it keeps `outline::round`'s arc. The outskirts' blocks step back from
//! each bow (`bow`'s map: the patches' insets on those sides, `build_town`), a strip of open ground
//! between them and the wall where it bows least.

use super::geom::*;
use super::{Site, TownPlan, Ward};
use crate::core::hash::FastMap;

/// The new line stands this far (ft) at least from anything laid, and a tower its reach and this much.
const CLEAR: f64 = 8.0;
const TOWER_CLEAR: f64 = 4.0;
/// A stretch's tangent length at its free corners (share of the stretch), tried in turn; and how far
/// it may bow at most (ft).
const PULL: [f64; 10] = [2.4, 2.0, 1.6, 1.3, 1.0, 0.7, 0.45, 0.25, 0.1, 0.0];
const PULL_STEPPE: [f64; 10] = [2.7, 2.3, 1.9, 1.5, 1.1, 0.75, 0.45, 0.25, 0.1, 0.0];
/// And a bulge on top (share of the stretch, sin^2: level at both ends, so no kink at a corner), with
/// each tangent.
const SAG: [f64; 10] = [0.06, 0.055, 0.05, 0.04, 0.03, 0.02, 0.012, 0.006, 0.0, 0.0];
const SAG_STEPPE: [f64; 10] = [0.09, 0.08, 0.07, 0.055, 0.04, 0.03, 0.02, 0.01, 0.0, 0.0];
const BOW_MOST: f64 = 45.0;
const BOW_MOST_STEPPE: f64 = 70.0;
/// Points along a bowed stretch (ft apart, at least), and along the new line where it is tested.
const BODY_STEP: f64 = 10.0;
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
    fn new() -> Laid {
        Laid { polys: Vec::new(), segs: Vec::new(), grid: FastMap::default() }
    }

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

/// The Hermite curve from `a` to `b` leaving along `ta` and arriving along `tb` (unit), its tangents
/// `m` times the chord, at `t` (0 to 1).
fn hermite(a: P, b: P, ta: P, tb: P, m: f64, t: f64) -> P {
    let k = dist(a, b) * m;
    let (t2, t3) = (t * t, t * t * t);
    let (h00, h10, h01, h11) = (2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2);
    [h00 * a[0] + h10 * k * ta[0] + h01 * b[0] + h11 * k * tb[0], h00 * a[1] + h10 * k * ta[1] + h01 * b[1] + h11 * k * tb[1]]
}

/// The point of the line `pts` nearest `p`.
fn nearest_on(pts: &[P], p: P) -> P {
    pts.windows(2)
        .map(|s| {
            let d = sub(s[1], s[0]);
            lerp(s[0], s[1], (dot(sub(p, s[0]), d) / dot(d, d).max(1e-9)).clamp(0.0, 1.0))
        })
        .min_by(|x, y| dist(*x, p).total_cmp(&dist(*y, p)))
        .unwrap_or(p)
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

/// Bows the town wall's curtains out (round outlines only), in `pieces` and `towers` as `outline::lay`
/// gave them (local ft), before the outskirts are laid; `roads` and `bridges` those laid so far. Returns
/// how far each wall side (its mesh corners, the lower first) bows out at most, for the outskirts.
#[allow(clippy::too_many_arguments)]
pub(super) fn bow(
    site: &Site,
    plan: &TownPlan,
    mesh: &super::mesh::Mesh,
    ward: &[Option<Ward>],
    style: &super::wallstyle::WallStyle,
    roads: &[(Vec<P>, u8, f64)],
    bridges: &[Vec<P>],
    pieces: &mut [Vec<P>],
    towers: &mut [P],
    gate_towers: &[P],
    (gate_vs, hand): (&[usize], &super::wallgates::WallChanges),
) -> FastMap<(usize, usize), f64> {
    let mut bows: FastMap<(usize, usize), f64> = FastMap::default();
    if style.outline != super::wallstyle::Outline::Round || pieces.is_empty() {
        return bows;
    }
    let steppe = style.group % 5 == 4;
    let (pulls, sags, most) = if steppe { (&PULL_STEPPE, &SAG_STEPPE, BOW_MOST_STEPPE) } else { (&PULL, &SAG, BOW_MOST) };
    let last = pulls.len() - 1;
    // (The pieces as `wall_pieces_spaced` lays them, before `outline::round`: the same in number and
    // order, and so their towers; and the wall's sides, by their mesh corners.)
    let (mut raw, mut raw_towers, mut sides) = (Vec::new(), Vec::new(), Vec::new());
    {
        let (p, t, _) = super::wallgates::pieces(plan, mesh, gate_vs, hand);
        raw.extend(p);
        raw_towers.extend(t);
    }
    for (chain, closed, _) in &plan.wall_runs {
        let m = chain.len();
        for k in 0..if *closed { m } else { m.saturating_sub(1) } {
            sides.push((chain[k], chain[(k + 1) % m]));
        }
    }
    if raw.len() != pieces.len() || raw_towers.len() != towers.len() {
        return bows;
    }
    // (What `outline::lay` holds.)
    let mut held: Vec<P> = (0..mesh.pos.len()).filter(|&v| plan.pinned[v]).map(|v| mesh.pos[v]).collect();
    for (i, f) in mesh.faces.iter().enumerate() {
        if ward.get(i).copied().flatten() == Some(Ward::Castle) {
            held.extend(f.iter().map(|&v| mesh.pos[v]));
        }
    }
    let castled = held.len();
    for &(a, b) in &plan.main_edges {
        held.push(mesh.pos[a]);
        held.push(mesh.pos[b]);
    }
    let keep = |p: P| held.iter().any(|q| same(*q, p));
    // (A castle's side stays straight: it is the castle's curtain.)
    let castle_side = |a: P, b: P| [a, b].iter().all(|p| held[..castled].iter().any(|q| same(*q, *p)));
    let inner: Vec<Vec<P>> = (0..plan.n_inner).map(|i| mesh.face_pts(i)).collect();
    let in_town = |p: P| inner.iter().any(|f| contains(f, p));
    // (What is there to keep clear of: castle patches, bridges, the roads and streets, and the
    // outskirts' other sides, their blocks 7 ft off them.)
    let mut obst = Laid::new();
    for (i, f) in mesh.faces.iter().enumerate() {
        if ward.get(i).copied().flatten() == Some(Ward::Castle) && f.len() >= 3 {
            obst.poly(&mesh.face_pts(i));
        }
    }
    for b in bridges {
        obst.poly(b);
    }
    for (pts, _, w) in roads {
        for s in pts.windows(2) {
            obst.seg(s[0], s[1], 0.5 * w);
        }
    }
    for path in &plan.paths {
        for s in path.windows(2) {
            obst.seg(mesh.pos[s[0]], mesh.pos[s[1]], 9.0);
        }
    }
    for (k, (e, class)) in plan.ends.iter().enumerate() {
        if let Some(Some(g)) = plan.gate_of.get(k) {
            let curve = super::approach_curve(*e, mesh.pos[*g], plan.heading_of.get(k).copied().flatten());
            let half = 0.5 * [24.0, 16.0, 10.0][(*class as usize).min(2)];
            for s in curve.windows(2) {
                obst.seg(s[0], s[1], half);
            }
        }
    }
    // (Past the outskirts the wall's sides are on: those patches don't step back from the wall, and
    // the wall's face keeps 3 ft off their blocks.)
    let mut far = Laid::new();
    let on_wall = |a: usize, b: usize| sides.iter().any(|&(p, q)| (p == a && q == b) || (p == b && q == a));
    for (i, f) in mesh.faces.iter().enumerate().skip(plan.n_inner) {
        let m = f.len();
        if f.len() >= 3 && !plan.wet[i] && !(0..m).any(|e| on_wall(f[e], f[(e + 1) % m])) {
            far.poly(&inset(&mesh.face_pts(i), &vec![7.0; m]));
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
    let laid_towers: Vec<P> = towers.to_vec();
    let laid_pieces: Vec<Vec<P>> = pieces.to_vec();
    'pieces: for (pi, rp) in raw.iter().enumerate() {
        let lp = &laid_pieces[pi];
        if rp.len() < 2 || !same(rp[0], lp[0]) || !same(rp[rp.len() - 1], lp[lp.len() - 1]) {
            continue;
        }
        // (Runs between the points that stay.)
        let mut runs: Vec<Vec<P>> = Vec::new();
        let mut cur = vec![rp[0]];
        for (k, &q) in rp.iter().enumerate().skip(1) {
            cur.push(q);
            if k + 1 == rp.len() || keep(q) {
                runs.push(std::mem::take(&mut cur));
                cur.push(q);
            }
        }
        // (The wall's other pieces: the new line keeps a wall's clearance off them.)
        let mut walls = Laid::new();
        for w in laid_pieces.iter().enumerate().filter(|(pj, _)| *pj != pi).map(|x| x.1) {
            for s in w.windows(2) {
                walls.seg(s[0], s[1], CLEAR);
            }
        }
        let mut out_pts: Vec<P> = Vec::new();
        let mut any = false;
        let mut moved: Vec<(usize, P)> = Vec::new();
        let mut piece_bows: Vec<((usize, usize), f64)> = Vec::new();
        let mut at = 0usize;
        for pts in &runs {
            let n = pts.len();
            // (The wall either side of the run, past a corner that stays: where it turns in there, the
            // run leaves along the turn's bisector all the same.)
            let (before, after) = (at.checked_sub(1).map(|k| rp[k]), rp.get(at + n).copied());
            at += n - 1;
            // (The 2a line along this run: from its start to its end in the laid piece.)
            let (a, b) = (lp.iter().position(|q| same(*q, pts[0])), lp.iter().rposition(|q| same(*q, pts[n - 1])));
            let Some((a, b)) = a.zip(b).filter(|(a, b)| a < b) else {
                continue 'pieces;
            };
            let old: Vec<P> = lp[a..=b].to_vec();
            let keep_old = |out_pts: &mut Vec<P>| out_pts.extend(old.iter().skip(if out_pts.is_empty() { 0 } else { 1 }));
            // (Each stretch's way and the side the town is on: 1 its left, -1 its right, 0 neither or both.)
            let u: Vec<P> = (0..n - 1).map(|i| unit(sub(pts[i + 1], pts[i]))).collect();
            let side: Vec<f64> = (0..n - 1)
                .map(|i| {
                    let mid = lerp(pts[i], pts[i + 1], 0.5);
                    match (in_town(add(mid, mul(left(u[i]), 4.0))), in_town(add(mid, mul(left(u[i]), -4.0)))) {
                        (true, false) => 1.0,
                        (false, true) => -1.0,
                        _ => 0.0,
                    }
                })
                .collect();
            if side.iter().all(|s| *s == 0.0) {
                keep_old(&mut out_pts);
                continue;
            }
            // (Each corner where `outline::round` left it: its arc's middle; the run's ends as they are.)
            let q: Vec<P> = (0..n).map(|j| if j == 0 || j + 1 == n { pts[j] } else { nearest_on(&old, pts[j]) }).collect();
            let on_old = |p: P| old.windows(2).any(|s| seg_dist(p, s[0], s[1]) <= 0.5);
            let nearest_old = |p: P| nearest_on(&old, p);
            let fits = |p: P| {
                let o = nearest_old(p);
                on_old(p) || (open(p) && obst.no_nearer(p, o, clear, 0.3) && walls.no_nearer(p, o, clear, 0.3) && far.gap(p, clear)[0] >= half + 3.0)
            };
            // (The corners the line passes outside of: a concave one, the town on the same side
            // throughout, no deeper than the most.)
            let mut gone = vec![false; n];
            let mut kept_in = vec![false; n];
            let bridge = |gone: &mut Vec<bool>| loop {
                let mut changed = false;
                for j in 1..n - 1 {
                    if gone[j] {
                        continue;
                    }
                    let a = (0..j).rev().find(|&k| !gone[k]).unwrap_or(0);
                    let b = (j + 1..n).find(|&k| !gone[k]).unwrap_or(n - 1);
                    let s = side[a];
                    if s == 0.0 || side[a..b].iter().any(|x| *x != s) {
                        continue;
                    }
                    let o = mul(left(unit(sub(pts[j], pts[a]))), -s);
                    let deep = (a + 1..b).map(|k| seg_dist(pts[k], pts[a], pts[b])).fold(0.0, f64::max);
                    if dot(unit(sub(pts[b], pts[j])), o) > 1e-3 && deep <= 0.8 * most {
                        gone[j] = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            };
            bridge(&mut gone);
            // (The line's stretches, from corner to kept corner: ends, chord, outward side, the tangent
            // at each end, and whether it passes outside a corner.)
            struct St {
                a: usize,
                b: usize,
                u: P,
                o: Option<P>,
                ta: P,
                tb: P,
                bridged: bool,
            }
            let stretches = |gone: &[bool]| -> Vec<St> {
                let ks: Vec<usize> = (0..n).filter(|&j| !gone[j]).collect();
                let mut st: Vec<St> = ks
                    .windows(2)
                    .map(|w| {
                        let uu = unit(sub(q[w[1]], q[w[0]]));
                        let s = side[w[0]];
                        let o = (s != 0.0 && side[w[0]..w[1]].iter().all(|x| *x == s)).then(|| mul(left(uu), -s));
                        St { a: w[0], b: w[1], u: uu, o, ta: uu, tb: uu, bridged: w[1] > w[0] + 1 }
                    })
                    .collect();
                // (The stretch from `x` to `y` past the run's end: its way, if the town is on side `s` of it.)
                let past = |x: P, y: P, s: f64| {
                    let v = unit(sub(y, x));
                    let (mid, o) = (lerp(x, y, 0.5), mul(left(v), -s));
                    (in_town(add(mid, mul(o, -4.0))) && !in_town(add(mid, mul(o, 4.0)))).then_some((v, o))
                };
                if let (Some(x), Some(o)) = (before, st[0].o) {
                    if let Some((v, ov)) = past(x, pts[0], side[0]) {
                        if dot(st[0].u, ov) < -1e-3 && dot(o, ov) > 0.0 {
                            st[0].ta = unit(add(v, st[0].u));
                        }
                    }
                }
                let k = st.len() - 1;
                if let (Some(y), Some(o)) = (after, st[k].o) {
                    if let Some((v, _)) = past(pts[n - 1], y, side[n - 2]) {
                        if dot(v, o) < -1e-3 {
                            st[k].tb = unit(add(st[k].u, v));
                        }
                    }
                }
                // (A corner turning in, the town on the same side: the way along its bisector.)
                for k in 1..st.len() {
                    if let (Some(o1), Some(o2)) = (st[k - 1].o, st[k].o) {
                        if dot(o1, o2) > 0.0 && dot(st[k].u, o1) < -1e-3 {
                            let t = unit(add(st[k - 1].u, st[k].u));
                            st[k - 1].tb = t;
                            st[k].ta = t;
                        }
                    }
                }
                st
            };
            let curve = |s: &St, k: usize| -> Vec<P> {
                let Some(o) = s.o.filter(|_| k < last && !castle_side(pts[s.a], pts[s.b])) else { return Vec::new() };
                if sags[k] == 0.0 && same(s.ta, s.u) && same(s.tb, s.u) {
                    return Vec::new();
                }
                let l = dist(q[s.a], q[s.b]);
                let m = crate::core::ceil(l * pulls[k].max(1.0) / BODY_STEP).max(2.0) as usize;
                let pi = std::f64::consts::PI;
                (1..m)
                    .map(|t| {
                        let t = t as f64 / m as f64;
                        let w = libm::sin(pi * t);
                        add(hermite(q[s.a], q[s.b], s.ta, s.tb, pulls[k], t), mul(o, sags[k] * l * w * w))
                    })
                    .collect()
            };
            let bow_of = |s: &St, c: &[P]| s.o.map_or(0.0, |o| c.iter().map(|p| dot(sub(*p, q[s.a]), o)).fold(0.0, f64::max));
            // (The first tangent whose bow isn't past the most, by the stretch's first corner.)
            let first = |s: &St| {
                (0..last)
                    .find(|&k| {
                        let c = curve(s, k);
                        !c.is_empty() && bow_of(s, &c) <= most
                    })
                    .unwrap_or(last)
            };
            let mut lv = vec![last; n];
            for s in stretches(&gone) {
                lv[s.a] = first(&s);
            }
            // (A corner between two straight stretches keeps the arc `outline::round` laid there.)
            let arc_at = |j: usize| -> Vec<P> {
                let short = dist(pts[j - 1], pts[j]).min(dist(pts[j], pts[j + 1]));
                let arc: Vec<P> = old.iter().copied().filter(|x| !same(*x, pts[j - 1]) && !same(*x, pts[j + 1]) && dist(*x, pts[j]) <= 0.5 * short + 1e-6).collect();
                if arc.is_empty() { vec![pts[j]] } else { arc }
            };
            let mut done: Option<(Vec<P>, Vec<(usize, P)>)> = None;
            for _ in 0..pulls.len() + 2 * n + 4 {
                let st = stretches(&gone);
                let ns = st.len();
                // (The line: each stretch's section, its corners at their arcs' middles or as the arcs.)
                let straight = |k: usize| lv[st[k].a] == last && !st[k].bridged;
                let arced: Vec<bool> = (0..=ns).map(|k| k > 0 && k < ns && straight(k - 1) && straight(k)).collect();
                let mut line = vec![q[0]];
                let mut secs: Vec<(usize, usize)> = Vec::new();
                for (k, s) in st.iter().enumerate() {
                    let start = line.len() - 1;
                    line.extend(curve(s, lv[s.a]));
                    if arced[k + 1] {
                        line.extend(arc_at(s.b));
                    } else {
                        line.push(q[s.b]);
                    }
                    line.dedup_by(|x, y| same(*x, *y));
                    secs.push((start, line.len() - 1));
                }
                let mut bad = vec![false; ns];
                for (k, &(sa, sb)) in secs.iter().enumerate() {
                    for w in line[sa..=sb].windows(2) {
                        let m = crate::core::ceil(dist(w[0], w[1]) / TEST_STEP).max(1.0) as usize;
                        if (0..=m).map(|j| lerp(w[0], w[1], j as f64 / m as f64)).any(|p| !fits(p)) {
                            bad[k] = true;
                            break;
                        }
                    }
                }
                // (The towers along it: on a kept corner, at the corner (or where `outline::round`
                // put it, on its arc); along a stretch, square onto its section.)
                let mut tw: Vec<(usize, P)> = Vec::new();
                for (ti, t) in raw_towers.iter().enumerate() {
                    let Some(i) = (0..n - 1).find(|&i| seg_dist(*t, pts[i], pts[i + 1]) < 1e-6) else { continue };
                    let f = dot(sub(*t, pts[i]), u[i]) / dist(pts[i], pts[i + 1]).max(1e-9);
                    let (i, f) = if f > 1.0 - 1e-6 && i + 2 < n { (i + 1, 0.0) } else { (i, f) };
                    if (i == 0 && f < 1e-6) || (i + 2 == n && f > 1.0 - 1e-6) {
                        continue;
                    }
                    let old_at = laid_towers[ti];
                    let corner = f < 1e-6 && !gone[i];
                    // (The stretch it is on: at a kept corner, the one leaving it.)
                    let k = st.iter().position(|s| s.a <= i && i < s.b).unwrap_or(0);
                    let (sa, sb) = secs[k];
                    let p = if corner {
                        if arced[k] { old_at } else { q[i] }
                    } else if straight(k) {
                        onto(&line[sa..=sb], old_at).unwrap_or(old_at)
                    } else {
                        onto(&line[sa..=sb], *t).unwrap_or(old_at)
                    };
                    let ok = dist(p, old_at) < 0.5
                        || (obst.no_nearer(p, old_at, reach, 1.0)
                            && !super::Ground::near_water(site, p, 3.0)
                            && gate_towers.iter().chain(laid_towers.iter().filter(|o| !same(**o, old_at))).all(|o| dist(*o, p) >= dist(*o, old_at).min(40.0)));
                    if !ok {
                        bad[k] = true;
                        if corner && k > 0 {
                            bad[k - 1] = true;
                        }
                    }
                    tw.push((ti, p));
                }
                if !bad.iter().any(|b| *b) {
                    done = Some((line, tw));
                    break;
                }
                // (Each that doesn't fit a step less bowed; straight, its corners back; else its
                // neighbours' bows, which left its corner where it is.)
                let mut stepped = false;
                for k in (0..ns).filter(|&k| bad[k]) {
                    let s = &st[k];
                    if lv[s.a] < last {
                        lv[s.a] += 1;
                        stepped = true;
                    } else if s.bridged {
                        for j in s.a..s.b {
                            gone[j] = false;
                            kept_in[j] = true;
                        }
                        stepped = true;
                    } else {
                        for j in [k.wrapping_sub(1), k + 1] {
                            if j < ns && lv[st[j].a] < last {
                                lv[st[j].a] += 1;
                                stepped = true;
                            }
                        }
                    }
                }
                if !stepped {
                    break;
                }
                // (Corners back: their new stretches start at their first tangent.)
                for s in stretches(&gone) {
                    if kept_in[s.a] {
                        lv[s.a] = first(&s);
                        kept_in[s.a] = false;
                    }
                }
            }
            match done.filter(|_| lv.iter().any(|&k| k < last) || gone.iter().any(|g| *g)) {
                Some((line, tw)) => {
                    any = true;
                    // (How far each side bows out over its own stretch.)
                    for i in 0..n - 1 {
                        if side[i] == 0.0 {
                            continue;
                        }
                        let (o, l) = (mul(left(u[i]), -side[i]), dist(pts[i], pts[i + 1]));
                        let b = line.iter().filter(|p| (-30.0..=l + 30.0).contains(&dot(sub(**p, pts[i]), u[i]))).map(|p| dot(sub(*p, pts[i]), o)).fold(0.0, f64::max);
                        let mid = lerp(pts[i], pts[i + 1], 0.5);
                        if b > 0.5 {
                            if let Some(&(va, vb)) = sides.iter().find(|&&(va, vb)| seg_dist(mid, mesh.pos[va], mesh.pos[vb]) < 1e-3) {
                                piece_bows.push(((va.min(vb), va.max(vb)), b));
                            }
                        }
                    }
                    out_pts.extend(line.iter().skip(if out_pts.is_empty() { 0 } else { 1 }));
                    moved.extend(tw);
                }
                None => keep_old(&mut out_pts),
            }
        }
        if any {
            pieces[pi] = out_pts;
            for (ti, p) in moved {
                towers[ti] = p;
            }
            for (k, b) in piece_bows {
                let e = bows.entry(k).or_insert(0.0);
                *e = e.max(b);
            }
        }
    }
    bows
}
