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
//!
//! A moat or a stream may instead be a line (`Water::line`): its water the band `width` ft wide
//! along it (`band`; a moat's line may come round to its start, a ring round what it guards). A
//! stream's level isn't flat: it follows the ground down its line (`Water::levels`, `stream_levels`).

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
/// A moat's width when none is given, and the widths it may have (ft).
pub const MOAT_WIDTH_FT: f64 = 20.0;
pub const MOAT_WIDTHS: [f64; 2] = [8.0, 60.0];
/// A stream's.
pub const STREAM_WIDTH_FT: f64 = 5.0;
pub const STREAM_WIDTHS: [f64; 2] = [3.0, 12.0];
/// A stream's level is taken along its line this far apart (ft), at most `MAX_STREAM_LEVELS` times.
pub const STREAM_STEP_FT: f64 = 20.0;
pub const MAX_STREAM_LEVELS: usize = 512;

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
    /// A small stream: always a line, its level falling along it.
    Stream,
}

impl WaterKind {
    pub const ALL: [WaterKind; 6] = [WaterKind::Inlet, WaterKind::Basin, WaterKind::Lake, WaterKind::Pond, WaterKind::Moat, WaterKind::Stream];

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
            WaterKind::Stream => "stream",
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
            WaterKind::Stream => 1.5,
        }
    }

    /// How far (ft) its bed falls from the shore to that depth: a moat's dug steep, a stream's
    /// banks low.
    pub fn bank(self) -> f64 {
        match self {
            WaterKind::Moat => 4.0,
            WaterKind::Stream => 3.0,
            _ => BANK_FT,
        }
    }

    /// Whether it may be drawn as a line (a moat), or only so (a stream).
    pub fn may_be_line(self) -> bool {
        matches!(self, WaterKind::Moat | WaterKind::Stream)
    }

    /// Its line's width when none is given, and the widths allowed (ft).
    pub fn widths(self) -> (f64, [f64; 2]) {
        if self == WaterKind::Stream { (STREAM_WIDTH_FT, STREAM_WIDTHS) } else { (MOAT_WIDTH_FT, MOAT_WIDTHS) }
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
    /// A line, not an outline (a moat or stream): `pts` its line, 2 points or more.
    #[serde(default, skip_serializing_if = "is_false")]
    pub line: bool,
    /// A line's width (ft; `WaterKind::widths` when none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// A moat's line come round to its start (a ring).
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
    /// A stream's level along its line (`Water::centre`), evenly from its first point to its last
    /// (`stream_levels`); none: `level` all along.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<f32>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Water {
    /// What is wrong with it, if anything (checked when one is made; the generator only skips
    /// one it can't evaluate).
    pub fn problem(&self) -> Option<String> {
        if self.line || self.kind == WaterKind::Stream {
            return self.line_problem();
        }
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

    /// A water outline (no line).
    pub fn outline(pts: Vec<P>, kind: WaterKind, level: f32) -> Water {
        Water { pts, kind, level, line: false, width: None, closed: false, levels: Vec::new() }
    }

    /// A moat's or stream's line (a stream's never closed).
    pub fn of_line(pts: Vec<P>, kind: WaterKind, level: f32, width: Option<f64>, closed: bool) -> Water {
        Water { pts, kind, level, line: true, width, closed: closed && kind != WaterKind::Stream, levels: Vec::new() }
    }

    /// A line's width (ft), within what its kind allows.
    pub fn width_ft(&self) -> f64 {
        let (d, [lo, hi]) = self.kind.widths();
        self.width.filter(|w| w.is_finite()).unwrap_or(d).clamp(lo, hi)
    }

    /// Whether it is a line (a stream always is).
    pub fn is_line(&self) -> bool {
        self.line || self.kind == WaterKind::Stream
    }

    /// A line's middle as every reader takes it: a moat's the points as drawn, a stream's
    /// smoothed (`chaikin`, its ends kept).
    pub fn centre(&self) -> Vec<P> {
        if self.kind == WaterKind::Stream { chaikin(&chaikin(&self.pts)) } else { self.pts.clone() }
    }

    fn line_problem(&self) -> Option<String> {
        let n = self.pts.len();
        let closed = self.closed && self.kind != WaterKind::Stream;
        if !self.kind.may_be_line() {
            return Some("only a moat or a stream is drawn as a line".into());
        }
        let least = if closed { 3 } else { 2 };
        if !(least..=MAX_WATER_PTS).contains(&n) {
            return Some(format!("a line has {least} to {MAX_WATER_PTS} points, not {n}"));
        }
        if !self.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) || !self.level.is_finite() || self.level.abs() > 30_000.0 {
            return Some("points and level must be finite (level ±30,000 ft at most)".into());
        }
        let (_, [lo, hi]) = self.kind.widths();
        if let Some(w) = self.width
            && !(w.is_finite() && (lo..=hi).contains(&w))
        {
            return Some(format!("a {}'s width is {lo} to {hi} ft", self.kind.name()));
        }
        if self.levels.len() > MAX_STREAM_LEVELS || self.levels.iter().any(|l| !l.is_finite() || l.abs() > 30_000.0) || !self.levels.is_empty() && self.levels.len() < 2 {
            return Some(format!("a stream's levels are 2 to {MAX_STREAM_LEVELS} finite heights"));
        }
        let b = crate::ground::bbox(&self.pts);
        if b[2] - b[0] > MAX_WATER_SPAN_FT || b[3] - b[1] > MAX_WATER_SPAN_FT {
            return Some(format!("a line spans at most {MAX_WATER_SPAN_FT} ft"));
        }
        let c = self.centre();
        let len: f64 = c.windows(2).map(|s| geom::dist(s[0], s[1])).sum();
        if len < self.width_ft() || c.windows(2).any(|s| geom::dist(s[0], s[1]) < 0.5) {
            return Some("the line is too short, or has points on top of each other".into());
        }
        if closed && (crate::ground::crosses_itself(&c) || fabs(geom::area(&c)) < 25.0) {
            return Some("the ring crosses itself or holds nothing".into());
        }
        if !band_ok(&c, closed, 0.5 * self.width_ft()) {
            return Some("the band folds over itself: make the bends gentler, the sides longer or the water narrower".into());
        }
        None
    }

    /// The shore as every reader takes it (`shape` of the points, or a line's `band`; the same
    /// `Arc` for the same water while it is cached).
    pub fn shore(&self) -> Arc<[P]> {
        self.shore_bridge().0
    }

    /// The shore, and where a ring's band crosses from its outer edge to its inner (`band`).
    fn shore_bridge(&self) -> (Arc<[P]>, Option<usize>) {
        let key = (self.kind, self.is_line(), self.width_ft().to_bits(), self.closed);
        SHORES.with(|c| {
            let mut c = c.borrow_mut();
            if let Some((_, _, s, b)) = c.iter().find(|(k, p, _, _)| *k == key && p.as_slice() == self.pts.as_slice()) {
                return (s.clone(), *b);
            }
            if c.len() >= SHORE_CACHE {
                c.clear();
            }
            let (s, b) = if self.is_line() { band(&self.centre(), self.closed && self.kind != WaterKind::Stream, 0.5 * self.width_ft()) } else { (shape(&self.pts, self.kind), None) };
            let s: Arc<[P]> = s.into();
            c.push((key, self.pts.clone(), s.clone(), b));
            (s, b)
        })
    }

    fn usable(&self) -> bool {
        let least = if self.is_line() { 2 } else { 3 };
        (least..=MAX_WATER_PTS).contains(&self.pts.len()) && self.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) && self.level.is_finite() && self.levels.iter().all(|l| l.is_finite()) && {
            let b = crate::ground::bbox(&self.pts);
            b[2] - b[0] <= MAX_WATER_SPAN_FT && b[3] - b[1] <= MAX_WATER_SPAN_FT
        }
    }

    /// A stream's line with its levels, for reading its level anywhere along it.
    fn course(&self) -> Option<Arc<Course>> {
        (self.kind == WaterKind::Stream && self.levels.len() >= 2).then(|| {
            let c = self.centre();
            let mut s = vec![0.0];
            for w in c.windows(2) {
                s.push(s[s.len() - 1] + geom::dist(w[0], w[1]));
            }
            Arc::new(Course { c, s, levels: self.levels.clone() })
        })
    }
}

/// A stream's line, the length along it to each point, and its levels evenly along it.
pub(crate) struct Course {
    c: Vec<P>,
    s: Vec<f64>,
    levels: Vec<f32>,
}

impl Course {
    /// The level at the point of the line nearest `p`.
    fn level(&self, p: P) -> f64 {
        let (mut best, mut at) = (f64::MAX, 0.0);
        for (i, w) in self.c.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dy * dy;
            let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let (ex, ey) = (a[0] + dx * t - p[0], a[1] + dy * t - p[1]);
            let d = ex * ex + ey * ey;
            if d < best {
                best = d;
                at = self.s[i] + (self.s[i + 1] - self.s[i]) * t;
            }
        }
        let total = self.s[self.s.len() - 1];
        let u = if total > 0.0 { at / total * (self.levels.len() - 1) as f64 } else { 0.0 };
        let i = (floor(u) as usize).min(self.levels.len() - 2);
        let f = (u - i as f64).clamp(0.0, 1.0);
        self.levels[i] as f64 + (self.levels[i + 1] as f64 - self.levels[i] as f64) * f
    }
}

/// Where a stream's level is taken along its line (`Water::centre`): evenly from its first
/// point to its last, `STREAM_STEP_FT` or so apart (at most `MAX_STREAM_LEVELS`).
pub fn stream_samples(centre: &[P]) -> Vec<P> {
    let total: f64 = centre.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
    if centre.len() < 2 || total <= 0.0 {
        return centre.iter().take(1).copied().collect();
    }
    let n = ((crate::core::ceil(total / STREAM_STEP_FT) as usize) + 1).clamp(2, MAX_STREAM_LEVELS);
    let mut out = Vec::with_capacity(n);
    let (mut seg, mut s0) = (0usize, 0.0);
    for k in 0..n {
        let s = total * k as f64 / (n - 1) as f64;
        while seg + 2 < centre.len() && s0 + geom::dist(centre[seg], centre[seg + 1]) < s {
            s0 += geom::dist(centre[seg], centre[seg + 1]);
            seg += 1;
        }
        let l = geom::dist(centre[seg], centre[seg + 1]);
        out.push(geom::lerp(centre[seg], centre[seg + 1], if l > 0.0 { ((s - s0) / l).clamp(0.0, 1.0) } else { 0.0 }));
    }
    out
}

/// A stream's levels from what is at each of its samples (`stream_samples`): the water's level
/// where it runs into water already there, else 1 ft under the ground; never rising downstream
/// (from its higher end: a running least), never under `floor` (the sea's), to 0.1 ft.
pub fn stream_levels(at: &[(f64, Option<f64>)], floor: f64) -> Vec<f32> {
    let h: Vec<f64> = at.iter().map(|&(g, wet)| wet.unwrap_or(g - 1.0)).collect();
    if h.is_empty() {
        return Vec::new();
    }
    let down = h[0] >= h[h.len() - 1];
    let mut out = vec![0.0f32; h.len()];
    let mut low = f64::MAX;
    for k in 0..h.len() {
        let i = if down { k } else { h.len() - 1 - k };
        low = low.min(h[i]);
        out[i] = ((crate::core::round(low.max(floor) * 10.0)) * 0.1) as f32;
    }
    out
}

/// An open line smoothed once: each piece cut at a quarter and three quarters along, its ends kept.
fn chaikin(c: &[P]) -> Vec<P> {
    if c.len() < 3 {
        return c.to_vec();
    }
    let mut out = vec![c[0]];
    for w in c.windows(2) {
        out.push(geom::lerp(w[0], w[1], 0.25));
        out.push(geom::lerp(w[0], w[1], 0.75));
    }
    out.push(c[c.len() - 1]);
    out
}

/// A line's water: the band `h` ft either side of `c`, round at a bend's outside and at the
/// ends. Open: one outline (round the left side, the end, back along the right side). Closed: the
/// outer edge, then its inner edge backwards joined to it at the first point by a bridge there and
/// back (edges `k` and the last, the `Some(k)` given: no shore; a point inside the inner edge is
/// outside the water, as `geom::contains` counts crossings). The app mirrors it (`water.ts` `band`).
pub fn band(c: &[P], closed: bool, h: f64) -> (Vec<P>, Option<usize>) {
    if c.len() < 2 {
        return (c.to_vec(), None);
    }
    if closed {
        let (outer, inner) = (side(c, h, true), side(c, -h, true));
        let k = outer.len();
        let mut out = outer.clone();
        out.push(outer[0]);
        out.push(inner[0]);
        out.extend(inner[1..].iter().rev());
        out.push(inner[0]);
        return (out, Some(k));
    }
    let (l, r) = (side(c, h, false), side(c, -h, false));
    let n = c.len();
    let ang = |a: P, b: P| libm::atan2(b[1] - a[1], b[0] - a[0]) + core::f64::consts::FRAC_PI_2;
    let mut out = l;
    let cap = |out: &mut Vec<P>, p: P, a0: f64| {
        for j in 1..CAP_STEPS {
            let a = a0 - core::f64::consts::PI * j as f64 / CAP_STEPS as f64;
            out.push([p[0] + h * libm::cos(a), p[1] + h * libm::sin(a)]);
        }
    };
    cap(&mut out, c[n - 1], ang(c[n - 2], c[n - 1]));
    out.extend(r.iter().rev());
    cap(&mut out, c[0], ang(c[0], c[1]) + core::f64::consts::PI);
    (out, None)
}

/// Pieces of a line's round end.
const CAP_STEPS: usize = 8;

/// One edge of a line `c` `h` ft to its left (right when negative): round on a bend's outside,
/// the corner of the two offset sides on its inside.
fn side(c: &[P], h: f64, closed: bool) -> Vec<P> {
    let n = c.len();
    let norm = |a: P, b: P| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = sqrt(dx * dx + dy * dy).max(1e-9);
        [-dy / l, dx / l]
    };
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let p = c[i];
        let prev = if i > 0 { Some(c[i - 1]) } else if closed { Some(c[n - 1]) } else { None };
        let next = if i + 1 < n { Some(c[i + 1]) } else if closed { Some(c[0]) } else { None };
        let (m0, m1) = match (prev, next) {
            (None, Some(b)) => (norm(p, b), norm(p, b)),
            (Some(a), None) => (norm(a, p), norm(a, p)),
            (Some(a), Some(b)) => (norm(a, p), norm(p, b)),
            (None, None) => continue,
        };
        let cross = m0[0] * m1[1] - m0[1] * m1[0];
        let dot = m0[0] * m1[0] + m0[1] * m1[1];
        if cross * h < 0.0 && dot < 0.995 {
            // The outside of the bend: round, every 22.5 degrees or so.
            let (a0, a1) = (libm::atan2(m0[1], m0[0]), libm::atan2(m1[1], m1[0]));
            let mut da = a1 - a0;
            if da > core::f64::consts::PI {
                da -= 2.0 * core::f64::consts::PI;
            } else if da < -core::f64::consts::PI {
                da += 2.0 * core::f64::consts::PI;
            }
            let k = (crate::core::ceil(fabs(da) / (core::f64::consts::PI / 8.0)) as usize).max(1);
            for j in 0..=k {
                let a = a0 + da * j as f64 / k as f64;
                out.push([p[0] + h * libm::cos(a), p[1] + h * libm::sin(a)]);
            }
        } else {
            let (bx, by) = (m0[0] + m1[0], m0[1] + m1[1]);
            let bl = sqrt(bx * bx + by * by).max(1e-9);
            let (bx, by) = (bx / bl, by / bl);
            let l = h / (bx * m0[0] + by * m0[1]).max(0.25);
            out.push([p[0] + bx * l, p[1] + by * l]);
        }
    }
    out
}

/// Whether a line's band is sound: its outline (or a ring's two edges) never crosses itself, a
/// ring's inner edge inside its outer.
fn band_ok(c: &[P], closed: bool, h: f64) -> bool {
    if !closed {
        let (b, _) = band(c, false, h);
        return !crate::ground::crosses_itself(&b);
    }
    let (outer, inner) = (side(c, h, true), side(c, -h, true));
    let (ao, ai) = (geom::area(&outer), geom::area(&inner));
    let (big, small) = if fabs(ao) >= fabs(ai) { (&outer, &inner) } else { (&inner, &outer) };
    let (ab, asm) = (geom::area(big), geom::area(small));
    ab * asm > 0.0 && fabs(asm) >= 25.0 && !crate::ground::crosses_itself(big) && !crate::ground::crosses_itself(small) && !edges_cross(big, small) && geom::contains(big, small[0])
}

fn edges_cross(a: &[P], b: &[P]) -> bool {
    let edges = |p: &[P]| (0..p.len()).map(move |k| (p[k], p[(k + 1) % p.len()])).collect::<Vec<_>>();
    let eb = edges(b);
    edges(a).iter().any(|&(p, q)| eb.iter().any(|&(r, t)| cross_at(p, q, r, t).is_some()))
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
/// What a shore is shaped by besides the points: the kind, a line, its width's bits, a ring.
type ShoreKey = (WaterKind, bool, u64, bool);

thread_local! {
    static SHORES: RefCell<Vec<(ShoreKey, Vec<P>, Arc<[P]>, Option<usize>)>> = const { RefCell::new(Vec::new()) };
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
    /// How far its bed falls to its depth (`WaterKind::bank`).
    bank: f64,
    kind: WaterKind,
    /// A stream's line and levels (its level is theirs, not `level`).
    course: Option<Arc<Course>>,
    shore: Vec<(P, P)>,
    runs: Vec<[f64; 4]>,
}

impl Near {
    fn level_at(&self, p: P) -> f64 {
        self.course.as_ref().map_or(self.level, |c| c.level(p))
    }
}

const RUN: usize = 16;

/// The usable outlines with their shores (`Water::shore`) and the box round each.
fn shores(edits: &crate::world::Edits) -> Vec<(&Water, Arc<[P]>, [f64; 4], Option<usize>)> {
    edits.waters.values().filter(|w| w.usable()).map(|w| {
        let (s, k) = w.shore_bridge();
        let b = crate::ground::bbox(&s);
        (w, s, b, k)
    }).collect()
}

/// The usable outlines meeting `rect` (world ft, and a bank's width round it), each with its
/// shore cut where it runs inside another (any in the world: the same wherever it is read).
pub(crate) fn near(edits: &crate::world::Edits, rect: [f64; 4]) -> Vec<Near> {
    let all = shores(edits);
    let levels = group_levels(&all);
    let mut out = Vec::new();
    for (i, (w, pts, b, bridge)) in all.iter().enumerate() {
        let b = *b;
        if !meets(b, rect, BANK_FT) {
            continue;
        }
        let others: Vec<(&[P], [f64; 4])> = all.iter().enumerate().filter(|&(j, o)| j != i && meets(b, o.2, 0.0)).map(|(_, o)| (&*o.1, o.2)).collect();
        let n = pts.len();
        let mut shore = Vec::with_capacity(n);
        for k in 0..n {
            // (A ring's bridge between its edges is no shore.)
            if bridge.is_some_and(|m| k == m || k == n - 1) {
                continue;
            }
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
        out.push(Near { pts: pts.clone(), b, level: levels[i] as f64, depth: w.kind.depth(), bank: w.kind.bank(), kind: w.kind, course: w.course(), shore, runs });
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
/// (A stream is its own water: its level is its course's where it runs into another.)
fn group_levels(all: &[(&Water, Arc<[P]>, [f64; 4], Option<usize>)]) -> Vec<f32> {
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
            if all[i].0.kind != WaterKind::Stream && all[j].0.kind != WaterKind::Stream && meets(all[i].2, all[j].2, 0.0) && overlap(&all[i].1, &all[j].1) {
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

/// A usable outline's shore with its level as one water (`group_levels`), for `level_in`.
pub struct Grouped {
    pts: Arc<[P]>,
    b: [f64; 4],
    level: f32,
    course: Option<Arc<Course>>,
}

impl Grouped {
    /// Its shore.
    pub fn pts(&self) -> &[P] {
        &self.pts
    }
}

/// The usable outlines' shores with their levels as one water (`group_levels`), for `level_in`.
pub fn grouped(edits: &crate::world::Edits) -> Vec<Grouped> {
    let all = shores(edits);
    let levels = group_levels(&all);
    all.into_iter().zip(levels).map(|((w, pts, b, _), level)| Grouped { pts, b, level, course: w.course() }).collect()
}

/// The level of the water drawn by hand at `p` among `grouped`, if an outline holds it (a
/// stream's only where no other water does).
pub fn level_in(grouped: &[Grouped], p: [f64; 2]) -> Option<f32> {
    let holds = |g: &&Grouped| p[0] >= g.b[0] && p[0] <= g.b[2] && p[1] >= g.b[1] && p[1] <= g.b[3] && geom::contains(&g.pts, p);
    grouped.iter().filter(|g| g.course.is_none()).filter(holds).map(|g| g.level).reduce(f32::max)
        .or_else(|| grouped.iter().filter(|g| g.course.is_some()).filter(holds).map(|g| g.course.as_ref().map_or(g.level, |c| c.level(p) as f32)).reduce(f32::max))
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
    let least = band;
    let band = band.max(BANK_FT);
    let inside = |n: &Near| x >= n.b[0] && x <= n.b[2] && y >= n.b[1] && y <= n.b[3] && geom::contains(&n.pts, [x, y]);
    // (A stream where it runs into other water is that water.)
    let still = near.iter().any(|n| n.course.is_some()) && near.iter().any(|n| n.course.is_none() && inside(n));
    let mut held = near.iter().filter(|n| (!still || n.course.is_none()) && inside(n)).peekable();
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
    held.map(|n| {
        let (l, s) = (n.level_at([x, y]), smoothstep(0.0, n.bank.max(least), d));
        (l, l - n.depth * s)
    })
    .reduce(|a, b| (a.0.max(b.0), a.1.min(b.1)))
}

/// Beside the water drawn by hand, within `reach` ft of its shore (at a point no water holds):
/// the kind of the nearest and how far its shore is.
pub(crate) fn edge(near: &[Near], x: f64, y: f64, reach: f64) -> Option<(WaterKind, f64)> {
    let mut best: Option<(WaterKind, f64)> = None;
    for n in near.iter().filter(|n| x >= n.b[0] - reach && x <= n.b[2] + reach && y >= n.b[1] - reach && y <= n.b[3] + reach) {
        for (r, rb) in n.runs.iter().enumerate() {
            let d = best.map_or(reach, |b| b.1);
            let (dx, dy) = ((rb[0] - x).max(x - rb[2]).max(0.0), (rb[1] - y).max(y - rb[3]).max(0.0));
            if dx >= d || dy >= d {
                continue;
            }
            for &(a, b) in &n.shore[r * RUN..((r + 1) * RUN).min(n.shore.len())] {
                let e = geom::seg_dist([x, y], a, b);
                if e < best.map_or(reach, |b| b.1) {
                    best = Some((n.kind, e));
                }
            }
        }
    }
    best
}

/// The level of the water drawn by hand at `p` (world ft), if an outline holds it.
pub fn level_at(edits: &crate::world::Edits, p: [f64; 2]) -> Option<f32> {
    level_in(&grouped(edits), p)
}

/// The outline drawn by hand holding `p`, if any (its id; the first by id).
pub fn holding<'a>(edits: &'a crate::world::Edits, p: [f64; 2]) -> Option<&'a str> {
    edits.waters.iter().find(|(_, w)| w.usable() && geom::contains(&w.shore(), p)).map(|(k, _)| k.as_str())
}
