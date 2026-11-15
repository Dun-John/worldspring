//! Water drawn by hand (`Edits::waters`, `e:<id>`): an inlet, basin, lake, pond or moat, each a
//! closed outline and a level. Inside the outline the water stands at its level and the ground
//! falls from the shore to the kind's depth below it over `BANK_FT`; overlapping outlines are one
//! water (one level, the lowest of theirs, and the deeper bed), and an outline that reaches the sea or a lake at its
//! level joins it. Applied wherever the ground is shaped by hand (`ground::Shaper`, before the
//! plateaus, so a plateau or islet over it still stands dry), so battlemaps, the finest terrain,
//! towns' and crossings' shore checks (`town::Shore`) all read the same water. A pure function of
//! position and the edits.
//!
//! The record keeps the points as drawn; every reader takes the shore shaped from them
//! (`Water::shore`): a lake's, pond's or inlet's a smooth curve near the points with a gentle
//! wobble along it, a basin's or moat's the straight sides with their corners rounded.

use std::cell::RefCell;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::noise::smoothstep;
use crate::core::{fabs, floor, sqrt};
use crate::town::geom;

type P = [f64; 2];

pub const MAX_WATERS: usize = 256;
pub const MAX_WATER_PTS: usize = 128;
/// An outline spans at most this (5 miles).
pub const MAX_WATER_SPAN_FT: f64 = 26_400.0;
/// The bed falls from the shore to its depth over this (ft).
pub const BANK_FT: f64 = 15.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaterKind {
    /// The sea's, reaching in (its level the sea's unless given).
    Inlet,
    /// A harbour basin dug from the sea.
    Basin,
    #[default]
    Lake,
    Pond,
    /// A castle's or town's moat.
    Moat,
}

impl WaterKind {
    pub const ALL: [WaterKind; 5] = [WaterKind::Inlet, WaterKind::Basin, WaterKind::Lake, WaterKind::Pond, WaterKind::Moat];

    pub fn parse(s: &str) -> Option<WaterKind> {
        Self::ALL.into_iter().find(|k| k.name() == s)
    }

    pub fn name(self) -> &'static str {
        match self {
            WaterKind::Inlet => "inlet",
            WaterKind::Basin => "basin",
            WaterKind::Lake => "lake",
            WaterKind::Pond => "pond",
            WaterKind::Moat => "moat",
        }
    }

    /// How far its bed lies below its level, away from the shore (ft).
    pub fn depth(self) -> f64 {
        match self {
            WaterKind::Inlet => 12.0,
            WaterKind::Basin => 10.0,
            WaterKind::Lake => 8.0,
            WaterKind::Pond => 3.0,
            WaterKind::Moat => 6.0,
        }
    }

    /// Whether its level is the sea's by default (else the lowest ground on its shore).
    pub fn of_sea(self) -> bool {
        matches!(self, WaterKind::Inlet | WaterKind::Basin)
    }

    /// Whether its shore is natural (a curve round the points) rather than dug (straight sides).
    pub fn natural(self) -> bool {
        !matches!(self, WaterKind::Basin | WaterKind::Moat)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Water {
    /// World ft, 3 to `MAX_WATER_PTS` points rounded to 0.5 ft, a simple polygon.
    pub pts: Vec<[f64; 2]>,
    #[serde(default)]
    pub kind: WaterKind,
    /// Its surface, ft (the generator's datum, as `height` everywhere).
    pub level: f32,
}

impl Water {
    /// What is wrong with it, if anything (checked when one is made; the generator only skips
    /// one it can't evaluate).
    pub fn problem(&self) -> Option<String> {
        let n = self.pts.len();
        if !(3..=MAX_WATER_PTS).contains(&n) {
            return Some(format!("an outline has 3 to {MAX_WATER_PTS} points, not {n}"));
        }
        if !self.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) || !self.level.is_finite() || self.level.abs() > 30_000.0 {
            return Some("points and level must be finite (level ±30,000 ft at most)".into());
        }
        let b = crate::ground::bbox(&self.pts);
        if b[2] - b[0] > MAX_WATER_SPAN_FT || b[3] - b[1] > MAX_WATER_SPAN_FT {
            return Some(format!("an outline spans at most {MAX_WATER_SPAN_FT} ft"));
        }
        if geom::area(&self.pts).abs() < 25.0 {
            return Some("the outline holds no water".into());
        }
        if crate::ground::crosses_itself(&self.pts) {
            return Some("the outline crosses itself".into());
        }
        // (The shaped shore never crosses itself: `shape` shapes less until it doesn't.)
        None
    }

    /// The shore as every reader takes it (`shape` of the points; the same `Arc` for the same
    /// water while it is cached).
    pub fn shore(&self) -> Arc<[P]> {
        SHORES.with(|c| {
            let mut c = c.borrow_mut();
            if let Some((_, _, s)) = c.iter().find(|(k, p, _)| *k == self.kind && p.as_slice() == self.pts.as_slice()) {
                return s.clone();
            }
            if c.len() >= SHORE_CACHE {
                c.clear();
            }
            let s: Arc<[P]> = shape(&self.pts, self.kind).into();
            c.push((self.kind, self.pts.clone(), s.clone()));
            s
        })
    }

    fn usable(&self) -> bool {
        (3..=MAX_WATER_PTS).contains(&self.pts.len()) && self.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) && self.level.is_finite() && {
            let b = crate::ground::bbox(&self.pts);
            b[2] - b[0] <= MAX_WATER_SPAN_FT && b[3] - b[1] <= MAX_WATER_SPAN_FT
        }
    }
}

/// Shaped shore points lie at most this far apart (ft) until there would be `MAX_SHORE_PTS`.
const SHORE_STEP_FT: f64 = 12.0;
pub const MAX_SHORE_PTS: usize = 384;
/// How far a natural shore's control points are pushed out (`spline`).
const PULL: f64 = 0.6;
/// A basin's or moat's corners are rounded over this (ft) at most.
const FILLET_FT: f64 = 6.0;
/// The wobble's noise (keyed by world position).
const SHORE_SEED: u64 = 0x5a3e_91c7_0d2b_6f48;
/// Shores shaped lately (a pure function of the points and the kind; kept per thread).
const SHORE_CACHE: usize = 512;

thread_local! {
    static SHORES: RefCell<Vec<(WaterKind, Vec<P>, Arc<[P]>)>> = const { RefCell::new(Vec::new()) };
}

/// The shore shaped from an outline's points (a simple polygon when they are one): for a natural
/// kind a smooth curve near them (`spline`), sampled every `SHORE_STEP_FT` or so, and
/// moved in and out along it by a low noise (a few ft, at most ~10% of the water's size and 20% of
/// its mean half-width); for a dug one the sides as drawn with each corner rounded over
/// `FILLET_FT`. Where that would cross itself it shapes less (half the wobble, none, the points).
/// The app mirrors it (`app/src/editor/water.ts` `shapeShore`).
pub fn shape(pts: &[P], kind: WaterKind) -> Vec<P> {
    let n = pts.len();
    let ok = |s: &[P]| s.len() >= 3 && s.iter().all(|p| p[0].is_finite() && p[1].is_finite()) && fabs(geom::area(s)) >= 25.0 && !crate::ground::crosses_itself(s);
    if n < 3 || !pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) {
        return pts.to_vec();
    }
    if !kind.natural() {
        let f = fillet(pts);
        return if ok(&f) { f } else { pts.to_vec() };
    }
    let curve = spline(pts);
    let area = fabs(geom::area(pts));
    let per = perimeter(pts);
    if per > 0.0 {
        let hw = 2.0 * area / per;
        let amp = (0.1 * sqrt(area)).min(0.2 * hw).min(40.0);
        let wave = (1.2 * hw).clamp(25.0, 400.0);
        for f in [1.0, 0.5] {
            if amp * f < 0.5 {
                break;
            }
            let s = wobble(&curve, amp * f, wave);
            if ok(&s) {
                return s;
            }
        }
    }
    if ok(&curve) { curve } else { pts.to_vec() }
}

fn perimeter(pts: &[P]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| geom::dist(pts[i], pts[(i + 1) % n])).sum()
}

/// A closed cubic B-spline near `pts`: its control points the points pushed out by `PULL` of
/// how far the plain spline would fall short of them (so it runs about as far outside the sides
/// as inside the corners: four clicks make a round pond about the square's size, a freehand line
/// is kept), each side cut into pieces of about `SHORE_STEP_FT` (more when the whole would pass
/// `MAX_SHORE_PTS`).
fn spline(pts: &[P]) -> Vec<P> {
    let n = pts.len();
    let step = SHORE_STEP_FT.max(perimeter(pts) / (MAX_SHORE_PTS - n.min(MAX_SHORE_PTS - 1)) as f64);
    let q: Vec<P> = (0..n)
        .map(|i| {
            let (a, p, b) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let on = |k: usize| (a[k] + 4.0 * p[k] + b[k]) / 6.0;
            [p[0] + (p[0] - on(0)) * PULL, p[1] + (p[1] - on(1)) * PULL]
        })
        .collect();
    let mut out = Vec::new();
    for i in 0..n {
        let (q0, q1, q2, q3) = (q[(i + n - 1) % n], q[i], q[(i + 1) % n], q[(i + 2) % n]);
        let k = crate::core::ceil(geom::dist(pts[i], pts[(i + 1) % n]) / step).max(1.0) as usize;
        for j in 0..k {
            let u = j as f64 / k as f64;
            let (u2, u3, v) = (u * u, u * u * u, 1.0 - u);
            let (b0, b1, b2, b3) = (v * v * v / 6.0, (3.0 * u3 - 6.0 * u2 + 4.0) / 6.0, (-3.0 * u3 + 3.0 * u2 + 3.0 * u + 1.0) / 6.0, u3 / 6.0);
            out.push([q0[0] * b0 + q1[0] * b1 + q2[0] * b2 + q3[0] * b3, q0[1] * b0 + q1[1] * b1 + q2[1] * b2 + q3[1] * b3]);
        }
    }
    out
}

/// `curve` moved along its normal by `amp` ft times a low noise at each point (wavelength `wave` ft).
fn wobble(curve: &[P], amp: f64, wave: f64) -> Vec<P> {
    let n = curve.len();
    (0..n)
        .map(|i| {
            let (a, p, b) = (curve[(i + n - 1) % n], curve[i], curve[(i + 1) % n]);
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let l = sqrt(tx * tx + ty * ty);
            if l < 1e-9 {
                return p;
            }
            let d = amp * shore_noise(p[0] / wave, p[1] / wave);
            [p[0] + ty / l * d, p[1] - tx / l * d]
        })
        .collect()
}

/// Two octaves of value noise, about -1 to 1.
fn shore_noise(x: f64, y: f64) -> f64 {
    (value(x, y) + 0.5 * value(x * 2.5 + 17.25, y * 2.5 - 9.5)) / 1.5
}

fn value(x: f64, y: f64) -> f64 {
    let (fx, fy) = (floor(x), floor(y));
    let (ix, iy) = (fx as i64, fy as i64);
    let (u, v) = (x - fx, y - fy);
    let (u, v) = (u * u * (3.0 - 2.0 * u), v * v * (3.0 - 2.0 * v));
    let h = |a: i64, b: i64| crate::core::rng::signed(crate::core::rng::hash2(SHORE_SEED, a, b));
    let (top, bottom) = (h(ix, iy) + (h(ix + 1, iy) - h(ix, iy)) * u, h(ix, iy + 1) + (h(ix + 1, iy + 1) - h(ix, iy + 1)) * u);
    top + (bottom - top) * v
}

/// `pts` with each corner rounded: a quadratic curve from `FILLET_FT` (at most a quarter of either
/// side) before it to as far after; corners nearly straight kept.
fn fillet(pts: &[P]) -> Vec<P> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 5);
    for i in 0..n {
        let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let (la, lc) = (geom::dist(a, b), geom::dist(b, c));
        let r = FILLET_FT.min(0.25 * la).min(0.25 * lc);
        if r < 0.5 {
            out.push(b);
            continue;
        }
        let (ua, uc) = ([(a[0] - b[0]) / la, (a[1] - b[1]) / la], [(c[0] - b[0]) / lc, (c[1] - b[1]) / lc]);
        if ua[0] * uc[0] + ua[1] * uc[1] < -0.985 {
            out.push(b);
            continue;
        }
        let (p0, p2) = ([b[0] + ua[0] * r, b[1] + ua[1] * r], [b[0] + uc[0] * r, b[1] + uc[1] * r]);
        for j in 0..=4 {
            let t = j as f64 / 4.0;
            let (w0, w1, w2) = ((1.0 - t) * (1.0 - t), 2.0 * (1.0 - t) * t, t * t);
            out.push([p0[0] * w0 + b[0] * w1 + p2[0] * w2, p0[1] * w0 + b[1] * w1 + p2[1] * w2]);
        }
    }
    out
}

/// `pts` rounded to 0.5 ft, as a `Water` keeps them.
pub fn rounded(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    pts.iter().map(|p| [crate::core::round(p[0] * 2.0) * 0.5, crate::core::round(p[1] * 2.0) * 0.5]).collect()
}

/// A water outline that meets an area, looked up once (`ground::Shaper`): its shore is its
/// edges' pieces outside every other outline (overlapping outlines are one water), in runs of
/// `RUN` with the box round each run.
pub(crate) struct Near {
    pts: Arc<[P]>,
    b: [f64; 4],
    level: f64,
    depth: f64,
    shore: Vec<(P, P)>,
    runs: Vec<[f64; 4]>,
}

const RUN: usize = 16;

/// The usable outlines with their shores (`Water::shore`) and the box round each.
fn shores(edits: &crate::world::Edits) -> Vec<(&Water, Arc<[P]>, [f64; 4])> {
    edits.waters.values().filter(|w| w.usable()).map(|w| {
        let s = w.shore();
        let b = crate::ground::bbox(&s);
        (w, s, b)
    }).collect()
}

/// The usable outlines meeting `rect` (world ft, and a bank's width round it), each with its
/// shore cut where it runs inside another (any in the world: the same wherever it is read).
pub(crate) fn near(edits: &crate::world::Edits, rect: [f64; 4]) -> Vec<Near> {
    let all = shores(edits);
    let levels = group_levels(&all);
    let mut out = Vec::new();
    for (i, (w, pts, b)) in all.iter().enumerate() {
        let b = *b;
        if !meets(b, rect, BANK_FT) {
            continue;
        }
        let others: Vec<(&[P], [f64; 4])> = all.iter().enumerate().filter(|&(j, o)| j != i && meets(b, o.2, 0.0)).map(|(_, o)| (&*o.1, o.2)).collect();
        let n = pts.len();
        let mut shore = Vec::with_capacity(n);
        for k in 0..n {
            let (a, c) = (pts[k], pts[(k + 1) % n]);
            let eb = [a[0].min(c[0]), a[1].min(c[1]), a[0].max(c[0]), a[1].max(c[1])];
            let mut here = others.iter().filter(|o| meets(eb, o.1, 0.0)).peekable();
            if here.peek().is_none() {
                shore.push((a, c));
                continue;
            }
            let here: Vec<&[P]> = here.map(|o| o.0).collect();
            // Where the edge crosses the others' edges, and the pieces between outside them all.
            let mut ts = vec![0.0, 1.0];
            for o in &here {
                for q in 0..o.len() {
                    if let Some(t) = cross_at(a, c, o[q], o[(q + 1) % o.len()]) {
                        ts.push(t);
                    }
                }
            }
            ts.sort_by(f64::total_cmp);
            for t in ts.windows(2).filter(|t| t[1] - t[0] > 1e-9) {
                let at = |t: f64| [a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t];
                if !here.iter().any(|o| geom::contains(o, at(0.5 * (t[0] + t[1])))) {
                    shore.push((at(t[0]), at(t[1])));
                }
            }
        }
        let runs = shore.chunks(RUN).map(|r| r.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, &(p, q)| [b[0].min(p[0]).min(q[0]), b[1].min(p[1]).min(q[1]), b[2].max(p[0]).max(q[0]), b[3].max(p[1]).max(q[1])])).collect();
        out.push(Near { pts: pts.clone(), b, level: levels[i] as f64, depth: w.kind.depth(), shore, runs });
    }
    out
}

fn meets(a: [f64; 4], b: [f64; 4], m: f64) -> bool {
    !(a[2] + m < b[0] || a[0] - m > b[2] || a[3] + m < b[1] || a[1] - m > b[3])
}

/// Whether two outlines overlap (their edges cross, or one holds a corner of the other).
fn overlap(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    let edges = |p: &[[f64; 2]]| (0..p.len()).map(move |k| (p[k], p[(k + 1) % p.len()])).collect::<Vec<_>>();
    let eb = edges(b);
    edges(a).iter().any(|&(p, q)| eb.iter().any(|&(r, t)| cross_at(p, q, r, t).is_some())) || geom::contains(a, b[0]) || geom::contains(b, a[0])
}

/// Each outline's level as one water with every outline it overlaps, through others too (the
/// lowest of them: the merged water's lowest shore, so none stands above its bank).
fn group_levels(all: &[(&Water, Arc<[P]>, [f64; 4])]) -> Vec<f32> {
    let n = all.len();
    let mut root: Vec<usize> = (0..n).collect();
    fn find(root: &mut [usize], mut i: usize) -> usize {
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            if meets(all[i].2, all[j].2, 0.0) && overlap(&all[i].1, &all[j].1) {
                let (a, b) = (find(&mut root, i), find(&mut root, j));
                root[a.max(b)] = a.min(b);
            }
        }
    }
    let mut low = vec![f32::MAX; n];
    for i in 0..n {
        let r = find(&mut root, i);
        low[r] = low[r].min(all[i].0.level);
    }
    (0..n).map(|i| low[find(&mut root, i)]).collect()
}

/// The usable outlines' shores with their levels as one water (`group_levels`), for `level_in`.
pub fn grouped(edits: &crate::world::Edits) -> Vec<(Arc<[P]>, [f64; 4], f32)> {
    let all = shores(edits);
    let levels = group_levels(&all);
    all.into_iter().zip(levels).map(|((_, s, b), l)| (s, b, l)).collect()
}

/// The level of the water drawn by hand at `p` among `grouped`, if an outline holds it.
pub fn level_in(grouped: &[(Arc<[P]>, [f64; 4], f32)], p: [f64; 2]) -> Option<f32> {
    grouped.iter().filter(|(pts, b, _)| p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3] && geom::contains(pts, p)).map(|g| g.2).reduce(f32::max)
}

/// Where segment a-b crosses segment c-d, as a fraction along a-b (None when parallel or apart).
fn cross_at(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let (qa0, qa1) = (c[0] - a[0], c[1] - a[1]);
    let t = (qa0 * s[1] - qa1 * s[0]) / den;
    let u = (qa0 * r[1] - qa1 * r[0]) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

pub(crate) fn bounds(near: &[Near]) -> Option<[f64; 4]> {
    near.iter().map(|n| n.b).reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
}

/// At (x, y): the water's level and the bed under it, of the outlines holding the point (the
/// highest level, the deepest bed), the bed falling from the shore of them all (`band` ft at
/// least: `BANK_FT`), or None.
pub(crate) fn at(near: &[Near], x: f64, y: f64, band: f64) -> Option<(f64, f64)> {
    let band = band.max(BANK_FT);
    let inside = |n: &Near| x >= n.b[0] && x <= n.b[2] && y >= n.b[1] && y <= n.b[3] && geom::contains(&n.pts, [x, y]);
    let mut held = near.iter().filter(|n| inside(n)).peekable();
    held.peek()?;
    let mut d = band;
    for n in near.iter().filter(|n| x >= n.b[0] - band && x <= n.b[2] + band && y >= n.b[1] - band && y <= n.b[3] + band) {
        for (r, rb) in n.runs.iter().enumerate() {
            let (dx, dy) = ((rb[0] - x).max(x - rb[2]).max(0.0), (rb[1] - y).max(y - rb[3]).max(0.0));
            if dx >= d || dy >= d {
                continue;
            }
            for &(a, b) in &n.shore[r * RUN..((r + 1) * RUN).min(n.shore.len())] {
                d = d.min(geom::seg_dist([x, y], a, b));
            }
        }
    }
    let s = smoothstep(0.0, band, d);
    held.map(|n| (n.level, n.level - n.depth * s)).reduce(|a, b| (a.0.max(b.0), a.1.min(b.1)))
}

/// The level of the water drawn by hand at `p` (world ft), if an outline holds it.
pub fn level_at(edits: &crate::world::Edits, p: [f64; 2]) -> Option<f32> {
    level_in(&grouped(edits), p)
}

/// The outline drawn by hand holding `p`, if any (its id; the first by id).
pub fn holding<'a>(edits: &'a crate::world::Edits, p: [f64; 2]) -> Option<&'a str> {
    edits.waters.iter().find(|(_, w)| w.usable() && geom::contains(&w.shore(), p)).map(|(k, _)| k.as_str())
}
