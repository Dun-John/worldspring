//! Water drawn by hand (`Edits::waters`, `e:<id>`): an inlet, basin, lake, pond or moat, each a
//! closed outline and a level. Inside the outline the water stands at its level and the ground
//! falls from the shore to the kind's depth below it over `BANK_FT`; overlapping outlines are one
//! water (the higher level, the deeper bed), and an outline that reaches the sea or a lake at its
//! level joins it. Applied wherever the ground is shaped by hand (`ground::Shaper`, before the
//! plateaus, so a plateau or islet over it still stands dry), so battlemaps, the finest terrain,
//! towns' and crossings' shore checks (`town::Shore`) all read the same water. A pure function of
//! position and the edits.

use serde::{Deserialize, Serialize};

use crate::core::noise::smoothstep;
use crate::town::geom;

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
        None
    }

    fn usable(&self) -> bool {
        (3..=MAX_WATER_PTS).contains(&self.pts.len()) && self.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) && self.level.is_finite() && {
            let b = crate::ground::bbox(&self.pts);
            b[2] - b[0] <= MAX_WATER_SPAN_FT && b[3] - b[1] <= MAX_WATER_SPAN_FT
        }
    }
}

/// `pts` rounded to 0.5 ft, as a `Water` keeps them.
pub fn rounded(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    pts.iter().map(|p| [crate::core::round(p[0] * 2.0) * 0.5, crate::core::round(p[1] * 2.0) * 0.5]).collect()
}

/// A water outline that meets an area, looked up once (`ground::Shaper`): its shore is its
/// edges' pieces outside every other outline (overlapping outlines are one water).
pub(crate) struct Near<'a> {
    pts: &'a [[f64; 2]],
    b: [f64; 4],
    level: f64,
    depth: f64,
    shore: Vec<([f64; 2], [f64; 2])>,
}

/// The usable outlines meeting `rect` (world ft, and a bank's width round it), each with its
/// shore cut where it runs inside another (any in the world: the same wherever it is read).
pub(crate) fn near(edits: &crate::world::Edits, rect: [f64; 4]) -> Vec<Near<'_>> {
    let all: Vec<(&Water, [f64; 4])> = edits.waters.values().filter(|w| w.usable()).map(|w| (w, crate::ground::bbox(&w.pts))).collect();
    let meets = |a: [f64; 4], b: [f64; 4], m: f64| !(a[2] + m < b[0] || a[0] - m > b[2] || a[3] + m < b[1] || a[1] - m > b[3]);
    let mut out = Vec::new();
    for (i, &(w, b)) in all.iter().enumerate() {
        if !meets(b, rect, BANK_FT) {
            continue;
        }
        let others: Vec<&[[f64; 2]]> = all.iter().enumerate().filter(|&(j, o)| j != i && meets(b, o.1, 0.0)).map(|(_, o)| o.0.pts.as_slice()).collect();
        let n = w.pts.len();
        let mut shore = Vec::with_capacity(n);
        for k in 0..n {
            let (a, c) = (w.pts[k], w.pts[(k + 1) % n]);
            if others.is_empty() {
                shore.push((a, c));
                continue;
            }
            // Where the edge crosses the others' edges, and the pieces between outside them all.
            let mut ts = vec![0.0, 1.0];
            for o in &others {
                for q in 0..o.len() {
                    if let Some(t) = cross_at(a, c, o[q], o[(q + 1) % o.len()]) {
                        ts.push(t);
                    }
                }
            }
            ts.sort_by(f64::total_cmp);
            for t in ts.windows(2).filter(|t| t[1] - t[0] > 1e-9) {
                let at = |t: f64| [a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t];
                if !others.iter().any(|o| geom::contains(o, at(0.5 * (t[0] + t[1])))) {
                    shore.push((at(t[0]), at(t[1])));
                }
            }
        }
        out.push(Near { pts: &w.pts, b, level: w.level as f64, depth: w.kind.depth(), shore });
    }
    out
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
    let inside = |n: &Near| x >= n.b[0] && x <= n.b[2] && y >= n.b[1] && y <= n.b[3] && geom::contains(n.pts, [x, y]);
    let mut held = near.iter().filter(|n| inside(n)).peekable();
    held.peek()?;
    let mut d = band;
    for n in near.iter().filter(|n| x >= n.b[0] - band && x <= n.b[2] + band && y >= n.b[1] - band && y <= n.b[3] + band) {
        for &(a, b) in &n.shore {
            d = d.min(geom::seg_dist([x, y], a, b));
        }
    }
    let s = smoothstep(0.0, band, d);
    held.map(|n| (n.level, n.level - n.depth * s)).reduce(|a, b| (a.0.max(b.0), a.1.min(b.1)))
}

/// The level of the water drawn by hand at `p` (world ft), if an outline holds it.
pub fn level_at(edits: &crate::world::Edits, p: [f64; 2]) -> Option<f32> {
    edits.waters.values().filter(|w| w.usable()).filter(|w| geom::contains(&w.pts, p)).map(|w| w.level).reduce(f32::max)
}

/// The outline drawn by hand holding `p`, if any (its id; the first by id).
pub fn holding<'a>(edits: &'a crate::world::Edits, p: [f64; 2]) -> Option<&'a str> {
    edits.waters.iter().find(|(_, w)| w.usable() && geom::contains(&w.pts, p)).map(|(k, _)| k.as_str())
}
