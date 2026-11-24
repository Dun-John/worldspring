//! Roads drawn by hand (`Edits::paths`, `rw:<id>`): a footpath, track, gravel road, paved road or
//! cobbled street along a line of points, drawn on the ground. The world's road network and its
//! towns' streets stay as they are; the road shows on the map and the battlemap (its own surface
//! along a band of its width), scatter and the world's own buildings keep off it, and where it
//! meets water it crosses on a ford (a footpath or track) or a bridge (the others: timber for
//! gravel, stone for paved and cobbled).
//!
//! The record keeps the points as drawn; every reader takes the line shaped from them (`Path::line`:
//! a centripetal Catmull-Rom curve through the points, sampled about every `STEP_FT`) and the band
//! `Path::width_ft` wide along it. Its crossings are where that line runs over water
//! (`crossings`), a pure function of the world and the record.

use std::cell::RefCell;
use std::sync::Arc;


use crate::core::{ceil, sqrt};
use crate::t0::T0;
use crate::town::geom;
use crate::world::{Crossing, CrossingKind, CrossingMaterial, Edits, World};
pub use crate::model::paths::{PathKind, Path};

type P = [f64; 2];

pub const MAX_PATHS: usize = 256;
pub const MAX_PATH_PTS: usize = 256;
/// A line spans at most this (5 miles).
pub const MAX_PATH_SPAN_FT: f64 = 26_400.0;
/// A road's width (ft), least and most.
pub const MIN_WIDTH_FT: f64 = 3.0;
pub const MAX_WIDTH_FT: f64 = 40.0;
/// The shaped line's points lie about this far apart (ft), more where a long line would pass `MAX_LINE_PTS`.
const STEP_FT: f64 = 10.0;
const MAX_LINE_PTS: usize = 3000;
/// How finely (ft along the line) its water is looked for.
const WET_STEP_FT: f64 = 2.5;
/// A crossing reaches this far (ft) past the water at each end, and two stretches of water closer than
/// twice this are one crossing.
const BANK_REACH_FT: f64 = 6.0;
/// A crossing longer than this (ft) is cut into pieces about as long (each its own deck or ford, end to end).
const MAX_SPAN_FT: f64 = 300.0;
const CACHE: usize = 256;

/// `Path`'s width, checks and line, here above the model crate (`model::paths`).
pub trait PathLine {
    fn width_ft(&self) -> f64;
    fn problem(&self) -> Option<String>;
    fn line(&self) -> Arc<[P]>;
}

impl PathLine for Path {
    /// Its width (ft), within `MIN_WIDTH_FT`..`MAX_WIDTH_FT`.
    fn width_ft(&self) -> f64 {
        self.width.filter(|w| w.is_finite()).unwrap_or(self.kind.width_ft()).clamp(MIN_WIDTH_FT, MAX_WIDTH_FT)
    }

    /// What is wrong with it, if anything (checked when one is made; the generator skips one with a problem).
    fn problem(&self) -> Option<String> {
        let n = self.pts.len();
        if !(2..=MAX_PATH_PTS).contains(&n) {
            return Some(format!("a road has 2 to {MAX_PATH_PTS} points, not {n}"));
        }
        if !self.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) {
            return Some("points must be finite".into());
        }
        if let Some(w) = self.width
            && !(w.is_finite() && (MIN_WIDTH_FT..=MAX_WIDTH_FT).contains(&w))
        {
            return Some(format!("width: {MIN_WIDTH_FT} to {MAX_WIDTH_FT} ft"));
        }
        let b = crate::ground::bbox(&self.pts);
        if b[2] - b[0] > MAX_PATH_SPAN_FT || b[3] - b[1] > MAX_PATH_SPAN_FT {
            return Some(format!("a road spans at most {MAX_PATH_SPAN_FT} ft"));
        }
        if self.pts.windows(2).any(|s| geom::dist(s[0], s[1]) < 0.5) {
            return Some("two points in a row are the same: a road's every stretch has a length".into());
        }
        None
    }

    /// The line as every reader takes it (`curve` of the points; the same `Arc` for the same points while cached).
    fn line(&self) -> Arc<[P]> {
        LINES.with(|c| {
            let mut c = c.borrow_mut();
            if let Some((_, l)) = c.iter().find(|(p, _)| p.as_slice() == self.pts.as_slice()) {
                return l.clone();
            }
            if c.len() >= CACHE {
                c.clear();
            }
            let l: Arc<[P]> = curve(&self.pts).into();
            c.push((self.pts.clone(), l.clone()));
            l
        })
    }
}

thread_local! {
    static LINES: RefCell<Vec<(Vec<P>, Arc<[P]>)>> = const { RefCell::new(Vec::new()) };
    static CROSSINGS: RefCell<Vec<(u64, Arc<[Crossing]>)>> = const { RefCell::new(Vec::new()) };
}

/// A smooth line through `pts` (a centripetal Catmull-Rom spline, its ends' tangents along their
/// stretches), sampled about every `STEP_FT` (fewer where the whole would pass `MAX_LINE_PTS`); it
/// passes through every point.
pub fn curve(pts: &[P]) -> Vec<P> {
    let n = pts.len();
    if n < 2 {
        return pts.to_vec();
    }
    let total: f64 = pts.windows(2).map(|s| geom::dist(s[0], s[1])).sum();
    let step = STEP_FT.max(total / MAX_LINE_PTS as f64);
    let ext = |a: P, b: P| [2.0 * a[0] - b[0], 2.0 * a[1] - b[1]];
    let knot = |a: P, b: P| sqrt(geom::dist(a, b)).max(1e-3);
    let mut out = Vec::with_capacity((total / step) as usize + n + 1);
    for i in 0..n - 1 {
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p0 = if i == 0 { ext(p1, p2) } else { pts[i - 1] };
        let p3 = if i + 2 < n { pts[i + 2] } else { ext(p2, p1) };
        let t1 = knot(p0, p1);
        let t2 = t1 + knot(p1, p2);
        let t3 = t2 + knot(p2, p3);
        let lerp = |a: P, b: P, ta: f64, tb: f64, t: f64| {
            let (u, v) = ((tb - t) / (tb - ta), (t - ta) / (tb - ta));
            [a[0] * u + b[0] * v, a[1] * u + b[1] * v]
        };
        let k = ceil(geom::dist(p1, p2) / step).max(1.0) as usize;
        for j in 0..k {
            let t = t1 + (t2 - t1) * j as f64 / k as f64;
            let a1 = lerp(p0, p1, 0.0, t1, t);
            let a2 = lerp(p1, p2, t1, t2, t);
            let a3 = lerp(p2, p3, t2, t3, t);
            let b1 = lerp(a1, a2, 0.0, t2, t);
            let b2 = lerp(a2, a3, t1, t3, t);
            out.push(lerp(b1, b2, t1, t2, t));
        }
    }
    out.push(pts[n - 1]);
    out
}

/// A road near an area: its kind, half width (ft) and the stretches of its line meeting the area.
pub struct Near {
    pub kind: PathKind,
    pub half: f64,
    pub segs: Vec<(P, P)>,
}

/// The roads drawn by hand whose band comes within `pad` ft of `rect` (x0, y0, x1, y1), in id order.
pub fn near(edits: &Edits, rect: [f64; 4], pad: f64) -> Vec<Near> {
    let mut out = Vec::new();
    for p in edits.paths.values() {
        let b = crate::ground::bbox(&p.pts);
        let half = 0.5 * p.width_ft();
        // (The curve strays a little outside its points' box: a margin for it.)
        let m = half + pad + 0.25 * (b[2] - b[0]).max(b[3] - b[1]).min(400.0);
        if b[0] - m > rect[2] || b[2] + m < rect[0] || b[1] - m > rect[3] || b[3] + m < rect[1] || p.problem().is_some() {
            continue;
        }
        let r = half + pad;
        let segs: Vec<(P, P)> = p
            .line()
            .windows(2)
            .filter(|s| s[0][0].min(s[1][0]) - r <= rect[2] && s[0][0].max(s[1][0]) + r >= rect[0] && s[0][1].min(s[1][1]) - r <= rect[3] && s[0][1].max(s[1][1]) + r >= rect[1])
            .map(|s| (s[0], s[1]))
            .collect();
        if !segs.is_empty() {
            out.push(Near { kind: p.kind, half, segs });
        }
    }
    out
}

/// (Roadmap 7 item 168) A road drawn by hand, graded: its line's stretches near an area with the
/// bed's surface (ft) at each end. The bed follows the smooth ground (`T0::ground_at`, the terrain
/// without refinement detail) within its kind's grade (`max_grade`) as a network road's profile
/// does (`fit_profile`), but never more than `BED_MAX_DEV_FT` from that ground (a line drawn up a
/// slope too steep for it climbs it: a footpath there gets steps); a pure function of the record
/// and the world, so every chunk grades it alike.
pub struct Bed {
    pub kind: PathKind,
    pub half: f64,
    pub segs: Vec<(P, P, f32, f32)>,
}

/// How far a hand road's bed strays from the smooth ground at most (ft): a cutting or bank.
pub const BED_MAX_DEV_FT: f64 = 6.0;

/// The steepest a hand road's bed climbs where the ground lets it (a footpath: as steep as the
/// ground, its steps take the rest).
pub fn max_grade(kind: PathKind) -> f64 {
    [1.0, 0.15, 0.12, 0.10, 0.10][kind as usize]
}

/// The graded roads drawn by hand whose band comes within `pad` ft of `rect`, in id order (as `near`).
pub fn beds(world: &World, t0: &T0, rect: [f64; 4], pad: f64) -> Vec<Bed> {
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
    let mut out = Vec::new();
    for p in world.file.edits.paths.values() {
        let b = crate::ground::bbox(&p.pts);
        let half = 0.5 * p.width_ft();
        let m = half + pad + 0.25 * (b[2] - b[0]).max(b[3] - b[1]).min(400.0);
        if b[0] - m > rect[2] || b[2] + m < rect[0] || b[1] - m > rect[3] || b[3] + m < rect[1] || p.problem().is_some() {
            continue;
        }
        let r = half + pad;
        let line = p.line();
        let hit = |a: P, c: P| a[0].min(c[0]) - r <= rect[2] && a[0].max(c[0]) + r >= rect[0] && a[1].min(c[1]) - r <= rect[3] && a[1].max(c[1]) + r >= rect[1];
        if !line.windows(2).any(|s| hit(s[0], s[1])) {
            continue;
        }
        let z = bed_profile(t0, &line, p.kind, lattice);
        let segs: Vec<(P, P, f32, f32)> = (1..line.len()).filter(|&i| hit(line[i - 1], line[i])).map(|i| (line[i - 1], line[i], z[i - 1], z[i])).collect();
        out.push(Bed { kind: p.kind, half, segs });
    }
    out
}

/// The bed's surface at each point of a hand road's `line`.
fn bed_profile(t0: &T0, line: &[P], kind: PathKind, lattice: f64) -> Vec<f32> {
    let ground: Vec<f64> = line.iter().map(|p| t0.ground_at(p[0], p[1], lattice)).collect();
    let fit = crate::t0::roads::fit_profile(line, &ground, max_grade(kind));
    let mut z: Vec<f64> = fit.iter().zip(&ground).map(|(&f, &g)| (f as f64).clamp(g - BED_MAX_DEV_FT, g + BED_MAX_DEV_FT)).collect();
    // Eased where the clamp bites (a few passes of a three-point mean; the ends stay put).
    for _ in 0..4 {
        let prev = z.clone();
        for i in 1..z.len().saturating_sub(1) {
            z[i] = 0.25 * prev[i - 1] + 0.5 * prev[i] + 0.25 * prev[i + 1];
        }
    }
    z.into_iter().map(|v| v as f32).collect()
}

/// Where a point lies by a graded bed (`bed_at`).
#[derive(Clone, Copy, Debug)]
pub struct OnBed {
    /// Distance (ft) from the line, the bed's surface there (ft), the stretch's start and unit
    /// direction, its grade (rise per ft along it), and the bed's and stretch's indices.
    pub d: f64,
    pub z: f64,
    pub a: P,
    pub u: P,
    pub grade: f64,
    pub kind: PathKind,
    pub half: f64,
    pub bed: u32,
    pub seg: u32,
}

/// The graded bed nearest `p` within `reach(bed)` ft of its line.
pub fn bed_at(beds: &[Bed], p: P, reach: impl Fn(&Bed) -> f64) -> Option<OnBed> {
    let mut best: Option<OnBed> = None;
    for (bi, b) in beds.iter().enumerate() {
        let r = reach(b);
        for (si, &(a, c, za, zc)) in b.segs.iter().enumerate() {
            if p[0] < a[0].min(c[0]) - r || p[0] > a[0].max(c[0]) + r || p[1] < a[1].min(c[1]) - r || p[1] > a[1].max(c[1]) + r {
                continue;
            }
            let (dx, dy) = (c[0] - a[0], c[1] - a[1]);
            let len2 = dx * dx + dy * dy;
            if len2 < 1e-9 {
                continue;
            }
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
            let (ex, ey) = (a[0] + dx * t - p[0], a[1] + dy * t - p[1]);
            let d = sqrt(ex * ex + ey * ey);
            if d <= r && best.is_none_or(|q| d < q.d) {
                let len = sqrt(len2);
                let z = za as f64 + (zc - za) as f64 * t;
                best = Some(OnBed { d, z, a, u: [dx / len, dy / len], grade: (zc - za) as f64 / len, kind: b.kind, half: b.half, bed: bi as u32, seg: si as u32 });
            }
        }
    }
    best
}

/// The road whose band (grown by `grow` ft, and at least `min_half` ft each side of its line) holds `p`: the
/// nearest line's; its kind and how far (ft) `p` is from its line.
pub fn at(near: &[Near], p: P, grow: f64, min_half: f64) -> Option<(PathKind, f64)> {
    let mut best: Option<(PathKind, f64)> = None;
    for n in near {
        let reach = (n.half + grow).max(min_half);
        for &(a, b) in &n.segs {
            if p[0] < a[0].min(b[0]) - reach || p[0] > a[0].max(b[0]) + reach || p[1] < a[1].min(b[1]) - reach || p[1] > a[1].max(b[1]) + reach {
                continue;
            }
            let d = geom::seg_dist(p, a, b);
            if d <= reach && best.is_none_or(|b| d < b.1) {
                best = Some((n.kind, d));
            }
        }
    }
    best
}

/// Whether outline `poly` (world ft) meets a road's band in `near`.
pub fn meets(near: &[Near], poly: &[P]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let b = crate::ground::bbox(poly);
    near.iter().any(|n| {
        n.segs.iter().any(|&(a, c)| {
            if a[0].max(c[0]) + n.half < b[0] || a[0].min(c[0]) - n.half > b[2] || a[1].max(c[1]) + n.half < b[1] || a[1].min(c[1]) - n.half > b[3] {
                return false;
            }
            geom::contains(poly, a) || (0..poly.len()).any(|i| seg_seg(a, c, poly[i], poly[(i + 1) % poly.len()]) <= n.half)
        })
    })
}

/// The least distance (ft) between segments a-b and c-d.
fn seg_seg(a: P, b: P, c: P, d: P) -> f64 {
    let cross = |o: P, p: P, q: P| (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        return 0.0;
    }
    geom::seg_dist(a, c, d).min(geom::seg_dist(b, c, d)).min(geom::seg_dist(c, a, b)).min(geom::seg_dist(d, a, b))
}

/// A fingerprint of the roads meeting `rect` grown by `pad` ft (their points, kinds and widths), or `None` without any.
pub(crate) fn fingerprint(edits: &Edits, rect: [f64; 4], pad: f64) -> Option<u64> {
    let mut h = crate::core::hash::Fnv64::default();
    let mut any = false;
    for (k, p) in edits.paths.iter() {
        let b = crate::ground::bbox(&p.pts);
        let m = 0.5 * p.width_ft() + pad + 0.25 * (b[2] - b[0]).max(b[3] - b[1]).min(400.0);
        if b[0] - m > rect[2] || b[2] + m < rect[0] || b[1] - m > rect[3] || b[3] + m < rect[1] {
            continue;
        }
        any = true;
        h.write(k.as_bytes());
        for v in p.pts.iter().flatten() {
            h.write(&v.to_le_bytes());
        }
        h.write(&[p.kind as u8]);
        h.write(&p.width_ft().to_le_bytes());
    }
    any.then(|| h.finish())
}

/// Where a road's line runs over water (rivers, lakes, the sea, water drawn by hand), as crossings
/// put down by hand: from `BANK_REACH_FT` before each stretch of water to as far past it (a long one
/// in pieces of at most `MAX_SPAN_FT`), a bridge (stone for a paved road or cobbled street, timber for
/// gravel) or a ford its width. A pure function of the world, the record and the water drawn by hand
/// near it (cached per thread).
pub fn crossings(world: &World, t0: &T0, p: &Path) -> Arc<[Crossing]> {
    if p.problem().is_some() {
        return Arc::from(Vec::new());
    }
    let b = crate::ground::bbox(&p.pts);
    let grow = 400.0;
    let mut h = crate::core::hash::Fnv64::default();
    h.write(&world.hash.to_le_bytes());
    for v in p.pts.iter().flatten() {
        h.write(&v.to_le_bytes());
    }
    h.write(&[p.kind as u8]);
    h.write(&p.width_ft().to_le_bytes());
    for (k, w) in world.file.edits.waters.iter() {
        let wb = crate::ground::bbox(&w.pts);
        if wb[0] > b[2] + grow || wb[2] < b[0] - grow || wb[1] > b[3] + grow || wb[3] < b[1] - grow {
            continue;
        }
        h.write(k.as_bytes());
        for v in w.pts.iter().flatten() {
            h.write(&v.to_le_bytes());
        }
        h.write(&w.level.to_le_bytes());
        // (All that shapes its shore and bed: its kind if it has one, depth, shore, line.)
        h.write(w.kind.map_or("-", |k| k.name()).as_bytes());
        h.write(&w.depth_ft().to_le_bytes());
        h.write(&[w.is_dug() as u8, w.is_line() as u8, w.closed as u8]);
        h.write(&w.width_ft().to_le_bytes());
        for l in &w.levels {
            h.write(&l.to_le_bytes());
        }
    }
    let key = h.finish();
    if let Some(c) = CROSSINGS.with(|c| c.borrow().iter().find(|e| e.0 == key).map(|e| e.1.clone())) {
        return c;
    }
    let c: Arc<[Crossing]> = find_crossings(world, t0, p).into();
    CROSSINGS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE {
            cache.clear();
        }
        cache.push((key, c.clone()));
    });
    c
}

fn find_crossings(world: &World, t0: &T0, p: &Path) -> Vec<Crossing> {
    let material = if matches!(p.kind, PathKind::Paved | PathKind::Cobbled) { CrossingMaterial::Stone } else { CrossingMaterial::Timber };
    wet_crossings(world, t0, &p.line(), p.width_ft(), material)
}

/// The bridges (timber) a street's `line` (world ft, as laid, not shaped) needs where it runs over
/// water, `width` ft wide: as a road drawn by hand gets them. A pure function of the world.
pub fn street_bridges(world: &World, t0: &T0, line: &[P], width: f64) -> Vec<Crossing> {
    wet_crossings(world, t0, line, width, CrossingMaterial::Timber)
}

fn wet_crossings(world: &World, t0: &T0, line: &[P], width: f64, material: CrossingMaterial) -> Vec<Crossing> {
    // (Arc length at each of the line's points.)
    let mut at_s = Vec::with_capacity(line.len());
    let mut s = 0.0;
    for (i, q) in line.iter().enumerate() {
        if i > 0 {
            s += geom::dist(line[i - 1], *q);
        }
        at_s.push(s);
    }
    let total = s;
    if total < 1.0 {
        return Vec::new();
    }
    let point = |s: f64| -> P {
        let s = s.clamp(0.0, total);
        let i = at_s.partition_point(|&v| v < s).clamp(1, line.len() - 1);
        let (s0, s1) = (at_s[i - 1], at_s[i]);
        let t = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        [line[i - 1][0] + (line[i][0] - line[i - 1][0]) * t, line[i - 1][1] + (line[i][1] - line[i - 1][1]) * t]
    };
    let grouped = crate::waters::grouped(&world.file.edits);
    let cache = crate::lod::rivers::CrossingCache::default();
    // (Roadmap 7, 199: lava drawn by hand is crossed as water is, on a stone bridge.)
    let lb = crate::ground::bbox(line);
    let lava = crate::waters::lava_near(&world.file.edits, lb, 0.5 * width + 1.0);
    let in_lava = |q: P| !lava.is_empty() && crate::waters::lava_dist(&lava, q[0], q[1], 0.0).is_some();
    let wet = |q: P| -> bool {
        if in_lava(q) || crate::waters::level_in(&grouped, q).is_some() {
            return true;
        }
        let still = t0.sample_water(q[0], q[1]);
        if still > crate::t0::hydro::DRY && still as f64 > t0.ground_at(q[0], q[1], 5.0) {
            return true;
        }
        t0.rivers.crossing_level(q, q, 0.0, t0.cell_ft, &cache).is_some()
    };
    // Stretches of water along the line (ft along it), those closer than twice the reach one.
    let n = (total / WET_STEP_FT) as usize;
    let mut runs: Vec<(f64, f64, bool)> = Vec::new();
    for k in 0..=n {
        let s = k as f64 * WET_STEP_FT;
        let q = point(s);
        if !wet(q) {
            continue;
        }
        let hot = in_lava(q);
        match runs.last_mut() {
            Some(r) if s - r.1 <= 2.0 * BANK_REACH_FT + WET_STEP_FT => (r.1, r.2) = (s, r.2 || hot),
            _ => runs.push((s, s, hot)),
        }
    }
    let half_w = 0.5 * width;
    let kind = CrossingKind::Bridge;
    let mut out = Vec::new();
    for (s0, s1, hot) in runs {
        let material = if hot { CrossingMaterial::Stone } else { material };
        let (a, b) = ((s0 - BANK_REACH_FT).max(0.0), (s1 + BANK_REACH_FT).min(total));
        if b - a < 2.0 * half_w.min(4.0) {
            continue;
        }
        let pieces = ceil((b - a) / MAX_SPAN_FT).max(1.0) as usize;
        for i in 0..pieces {
            let (u, v) = (a + (b - a) * i as f64 / pieces as f64, a + (b - a) * (i + 1) as f64 / pieces as f64);
            out.push(Crossing { kind, a: point(u), b: point(v), width, material });
        }
    }
    out
}

thread_local! {
    static LAVA_BRIDGES: RefCell<Option<(u64, Arc<[Crossing]>)>> = const { RefCell::new(None) };
}

/// (Roadmap 7, 199) Stone bridges carrying the network's roads over lava drawn by hand: where a
/// road (`lod::roads::pieces` round each lava's shore) runs in lava, a bridge from a bank's reach
/// before to one after, as a road drawn by hand gets (at most `MAX_SPAN_FT` a span). A pure function
/// of the world (each lava's own box), the same in every chunk.
pub fn lava_crossings(world: &World, t0: &T0) -> Arc<[Crossing]> {
    let edits = &world.file.edits;
    if !edits.waters.values().any(|w| w.is_lava()) {
        return Arc::from(Vec::new());
    }
    let mut h = crate::core::hash::Fnv64::default();
    h.write(&world.hash.to_le_bytes());
    for (k, w) in edits.waters.iter().filter(|(_, w)| w.is_lava()) {
        h.write(k.as_bytes());
        for v in w.pts.iter().flatten() {
            h.write(&v.to_le_bytes());
        }
        h.write(&w.width_ft().to_le_bytes());
        h.write(&[w.line as u8, w.closed as u8]);
    }
    let key = h.finish();
    if let Some(c) = LAVA_BRIDGES.with(|c| c.borrow().as_ref().filter(|e| e.0 == key).map(|e| e.1.clone())) {
        return c;
    }
    let all = crate::waters::lava_near(edits, [f64::MIN / 4.0, f64::MIN / 4.0, f64::MAX / 4.0, f64::MAX / 4.0], 0.0);
    let mut out: Vec<Crossing> = Vec::new();
    for l in &all {
        for piece in crate::lod::roads::pieces(&t0.roads, l.bbox(), 30.0, 20.0) {
            let line: Vec<P> = piece.pts.iter().map(|p| p.p).collect();
            let width = piece.class.width_ft() + 2.0;
            let lava = |q: P| crate::waters::lava_dist(&all, q[0], q[1], 0.0).is_some();
            for c in bridges_over(&line, width, lava) {
                if !out.iter().any(|o| geom::dist(o.a, c.a) < 1.0 && geom::dist(o.b, c.b) < 1.0) {
                    out.push(c);
                }
            }
        }
    }
    let out: Arc<[Crossing]> = out.into();
    LAVA_BRIDGES.with(|c| *c.borrow_mut() = Some((key, out.clone())));
    out
}

/// The bridges a town's own street `line` (world ft) `width` ft wide needs over lava drawn by
/// hand (`bridges_over`; its water's bridges are the town's own). A pure function of the world.
pub fn street_lava_bridges(world: &World, line: &[P], width: f64) -> Vec<Crossing> {
    let lava = crate::waters::lava_near(&world.file.edits, crate::ground::bbox(line), 0.5 * width + 1.0);
    if lava.is_empty() {
        return Vec::new();
    }
    bridges_over(line, width, |q| crate::waters::lava_dist(&lava, q[0], q[1], 0.0).is_some())
}

/// Stone bridges along `line` over the stretches where `hot` holds (sampled `WET_STEP_FT` apart,
/// stretches closer than twice a bank's reach one), each run a bank's reach past either end.
fn bridges_over(line: &[P], width: f64, hot: impl Fn(P) -> bool) -> Vec<Crossing> {
    let mut at_s = vec![0.0];
    for s in line.windows(2) {
        at_s.push(at_s[at_s.len() - 1] + geom::dist(s[0], s[1]));
    }
    let total = at_s[at_s.len() - 1];
    if line.len() < 2 || total < 1.0 {
        return Vec::new();
    }
    let point = |s: f64| -> P {
        let s = s.clamp(0.0, total);
        let i = at_s.partition_point(|&v| v < s).clamp(1, line.len() - 1);
        let (s0, s1) = (at_s[i - 1], at_s[i]);
        let t = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        [line[i - 1][0] + (line[i][0] - line[i - 1][0]) * t, line[i - 1][1] + (line[i][1] - line[i - 1][1]) * t]
    };
    let mut runs: Vec<(f64, f64)> = Vec::new();
    for k in 0..=(total / WET_STEP_FT) as usize {
        let s = k as f64 * WET_STEP_FT;
        if !hot(point(s)) {
            continue;
        }
        match runs.last_mut() {
            Some(r) if s - r.1 <= 2.0 * BANK_REACH_FT + WET_STEP_FT => r.1 = s,
            _ => runs.push((s, s)),
        }
    }
    let mut out = Vec::new();
    for (s0, s1) in runs {
        let (a, b) = ((s0 - BANK_REACH_FT).max(0.0), (s1 + BANK_REACH_FT).min(total));
        let pieces = ceil((b - a) / MAX_SPAN_FT).max(1.0) as usize;
        for i in 0..pieces {
            let (u, v) = (a + (b - a) * i as f64 / pieces as f64, a + (b - a) * (i + 1) as f64 / pieces as f64);
            out.push(Crossing { kind: CrossingKind::Bridge, a: point(u), b: point(v), width, material: CrossingMaterial::Stone });
        }
    }
    out
}

/// The lava bridges (`lava_crossings`) within `pad` ft of `rect`.
pub fn lava_crossings_near(world: &World, t0: &T0, rect: [f64; 4], pad: f64) -> Vec<Crossing> {
    lava_crossings(world, t0)
        .iter()
        .filter(|c| {
            let r = 0.5 * c.width + pad;
            c.a[0].min(c.b[0]) - r <= rect[2] && c.a[0].max(c.b[0]) + r >= rect[0] && c.a[1].min(c.b[1]) - r <= rect[3] && c.a[1].max(c.b[1]) + r >= rect[1]
        })
        .cloned()
        .collect()
}

/// The crossings of every road drawn by hand that come within `pad` ft of `rect`.
pub fn crossings_near(world: &World, t0: &T0, rect: [f64; 4], pad: f64) -> Vec<Crossing> {
    let mut out = Vec::new();
    for p in world.file.edits.paths.values() {
        let b = crate::ground::bbox(&p.pts);
        let m = 0.5 * p.width_ft() + pad + 0.25 * (b[2] - b[0]).max(b[3] - b[1]).min(400.0) + BANK_REACH_FT;
        if b[0] - m > rect[2] || b[2] + m < rect[0] || b[1] - m > rect[3] || b[3] + m < rect[1] {
            continue;
        }
        for c in crossings(world, t0, p).iter() {
            let r = 0.5 * c.width + pad;
            if c.a[0].min(c.b[0]) - r <= rect[2] && c.a[0].max(c.b[0]) + r >= rect[0] && c.a[1].min(c.b[1]) - r <= rect[3] && c.a[1].max(c.b[1]) + r >= rect[1] {
                out.push(c.clone());
            }
        }
    }
    out
}

/// Points rounded to 0.5 ft (as the record keeps them).
pub fn rounded(pts: &[P]) -> Vec<P> {
    pts.iter().map(|p| [crate::core::round(p[0] * 2.0) * 0.5, crate::core::round(p[1] * 2.0) * 0.5]).collect()
}

/// The line's length (ft) and its middle point (half way along it).
pub fn length_and_mid(p: &Path) -> (f64, P) {
    let line = p.line();
    let total: f64 = line.windows(2).map(|s| geom::dist(s[0], s[1])).sum();
    let mut left = 0.5 * total;
    for s in line.windows(2) {
        let d = geom::dist(s[0], s[1]);
        if d >= left && d > 0.0 {
            let t = left / d;
            return (total, [s[0][0] + (s[1][0] - s[0][0]) * t, s[0][1] + (s[1][1] - s[0][1]) * t]);
        }
        left -= d;
    }
    (total, line.last().copied().unwrap_or([0.0, 0.0]))
}
