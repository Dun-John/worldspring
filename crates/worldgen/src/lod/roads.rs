//! Roads at every zoom level: the T0 road control points (grade-profiled) with a spatial
//! index, evaluated as Catmull-Rom curves plus a gentle sideways wander; per-tile pieces;
//! and cut/fill carving of the road bed.
//!
//! The curve is the same at every level that carves (the wander's shortest wavelength is
//! resolved at all carving spacings), so fine tiles keep the road where coarse ones put it.

use crate::core::noise::{gradient2, smoothstep};
use crate::core::rng::hash2;
use crate::t0::roads::RoadClass;

#[derive(Clone, Debug)]
pub struct RoadCurve {
    pub class: RoadClass,
    /// Control points (ft) and road surface elevation (ft) at each.
    pub pts: Vec<[f64; 2]>,
    pub z: Vec<f32>,
    /// Wander weight per control point (0 on switchback legs).
    pub wander: Vec<f32>,
    pub seed: u64,
    /// Cumulative chord length (ft) at each control point.
    pub s: Vec<f64>,
}

/// Broad sweeps and gentle bends (in T0 cells) and short kinks (ft).
const SWEEP_AMP_CELLS: f64 = 0.8;
const SWEEP_WAVELENGTH_CELLS: f64 = 14.0;
const BEND_AMP_CELLS: f64 = 0.22;
const BEND_WAVELENGTH_CELLS: f64 = 4.0;
const KINK_AMP_FT: f64 = 70.0;
const KINK_WAVELENGTH_FT: f64 = 1_600.0;

impl RoadCurve {
    pub fn new(class: RoadClass, pts: Vec<[f64; 2]>, z: Vec<f32>, wander: Vec<f32>, seed: u64) -> RoadCurve {
        let mut s = vec![0.0; pts.len()];
        for k in 1..pts.len() {
            let (a, b) = (pts[k - 1], pts[k]);
            s[k] = s[k - 1] + crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]));
        }
        RoadCurve { class, pts, z, wander, seed, s }
    }

    /// Point on segment `k` at parameter `t`: centripetal Catmull-Rom through the control
    /// points (no loops or cusps where a short segment follows a long one), offset along the
    /// normal by the wander (tapered to zero at both ends so junctions meet).
    pub fn eval(&self, k: usize, t: f64, spacing: f64, cell_ft: f64) -> RoadPoint {
        let n = self.pts.len();
        let p = |i: isize| self.pts[i.clamp(0, n as isize - 1) as usize];
        let ki = k as isize;
        let (p0, p1, p2, p3) = (p(ki - 1), p(ki), p(ki + 1), p(ki + 2));
        // Knot intervals: the square roots of the chord lengths (zero past either end).
        let chord = |i: usize| if i >= 1 && i < n { crate::core::sqrt(self.s[i] - self.s[i - 1]) } else { 0.0 };
        let (d0, d1, d2) = (chord(k), chord(k + 1).max(1e-9), chord(k + 2));
        // Hermite tangents (per unit t) at p1 and p2, through a–b–c with knot intervals da, db;
        // at an end (a repeated point) the uniform one.
        let tangent = |a: f64, b: f64, c: f64, da: f64, db: f64| {
            if da < 1e-9 || db < 1e-9 { 0.5 * (c - a) } else { d1 * ((b - a) / da - (c - a) / (da + db) + (c - b) / db) }
        };
        let m1 = [tangent(p0[0], p1[0], p2[0], d0, d1), tangent(p0[1], p1[1], p2[1], d0, d1)];
        let m2 = [tangent(p1[0], p2[0], p3[0], d1, d2), tangent(p1[1], p2[1], p3[1], d1, d2)];
        let (t2, t3) = (t * t, t * t * t);
        let (h00, h10, h01, h11) = (2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, 3.0 * t2 - 2.0 * t3, t3 - t2);
        let (g00, g10, g01, g11) = (6.0 * t2 - 6.0 * t, 3.0 * t2 - 4.0 * t + 1.0, 6.0 * t - 6.0 * t2, 3.0 * t2 - 2.0 * t);
        let base = [h00 * p1[0] + h10 * m1[0] + h01 * p2[0] + h11 * m2[0], h00 * p1[1] + h10 * m1[1] + h01 * p2[1] + h11 * m2[1]];
        let (tx, ty) = (g00 * p1[0] + g10 * m1[0] + g01 * p2[0] + g11 * m2[0], g00 * p1[1] + g10 * m1[1] + g01 * p2[1] + g11 * m2[1]);
        let tl = crate::core::sqrt(tx * tx + ty * ty).max(1e-9);
        let nrm = [-ty / tl, tx / tl];
        let s = self.s[k] + (self.s[k + 1] - self.s[k]) * t;
        let total = self.s[n - 1];
        let wt = (self.wander[k] as f64 + (self.wander[k + 1] - self.wander[k]) as f64 * t)
            * smoothstep(0.0, 0.6 * cell_ft, s)
            * smoothstep(0.0, 0.6 * cell_ft, total - s);
        let mut off = 0.0;
        if wt > 0.0 {
            let sweep_len = SWEEP_WAVELENGTH_CELLS * cell_ft;
            // Sweeps need room: fade them in over the first/last sweep half-wavelength.
            let room = smoothstep(0.0, 0.5 * sweep_len, s) * smoothstep(0.0, 0.5 * sweep_len, total - s);
            off += room * SWEEP_AMP_CELLS * cell_ft * gradient2(self.seed ^ 0x3c, s / sweep_len, 0.19);
            let bend_len = BEND_WAVELENGTH_CELLS * cell_ft;
            off += BEND_AMP_CELLS * cell_ft * gradient2(self.seed, s / bend_len, 0.37);
            let resolve = 1.0 - smoothstep(0.12, 0.35, spacing / KINK_WAVELENGTH_FT);
            off += resolve * KINK_AMP_FT * gradient2(self.seed ^ 0x5a, s / KINK_WAVELENGTH_FT, 0.71);
            off *= wt;
        }
        let z = self.z[k] as f64 + (self.z[k + 1] - self.z[k]) as f64 * t;
        RoadPoint { p: [base[0] + nrm[0] * off, base[1] + nrm[1] * off], z }
    }
}

pub fn road_seed(world_seed: u64, index: usize) -> u64 {
    hash2(world_seed, index as i64, 0x20AD)
}

pub struct RoadNet {
    pub roads: Vec<RoadCurve>,
    bin_ft: f64,
    bins_w: usize,
    bins_h: usize,
    bins: Vec<Vec<(u32, u32)>>,
    cell_ft: f64,
}

/// Embankment / cutting side slope (rise per run): a grassed bank, not a wall.
const SIDE_SLOPE: f64 = 0.5;
/// Road beds only matter where samples are this fine.
pub const CARVE_MAX_SPACING_FT: f64 = 160.0;

impl RoadNet {
    pub fn new(roads: Vec<RoadCurve>, map_w_ft: f64, map_h_ft: f64, cell_ft: f64) -> RoadNet {
        let bin_ft = cell_ft * 8.0;
        let bins_w = (map_w_ft / bin_ft).ceil() as usize + 1;
        let bins_h = (map_h_ft / bin_ft).ceil() as usize + 1;
        let mut bins = vec![Vec::new(); bins_w * bins_h];
        // Curve overshoot + wander stay within this of the chords.
        let pad = (SWEEP_AMP_CELLS + BEND_AMP_CELLS + 0.3) * cell_ft;
        for (ri, r) in roads.iter().enumerate() {
            for k in 0..r.pts.len().saturating_sub(1) {
                let (a, b) = (r.pts[k], r.pts[k + 1]);
                let (x0, x1) = ((a[0].min(b[0]) - pad) / bin_ft, (a[0].max(b[0]) + pad) / bin_ft);
                let (y0, y1) = ((a[1].min(b[1]) - pad) / bin_ft, (a[1].max(b[1]) + pad) / bin_ft);
                for by in (y0.floor().max(0.0) as usize)..=((y1.floor().max(0.0) as usize).min(bins_h - 1)) {
                    for bx in (x0.floor().max(0.0) as usize)..=((x1.floor().max(0.0) as usize).min(bins_w - 1)) {
                        bins[by * bins_w + bx].push((ri as u32, k as u32));
                    }
                }
            }
        }
        RoadNet { roads, bin_ft, bins_w, bins_h, bins, cell_ft }
    }

    pub fn segments_near(&self, rect: [f64; 4], pad: f64) -> Vec<(u32, u32)> {
        let bx0 = ((rect[0] - pad) / self.bin_ft).floor().max(0.0) as usize;
        let by0 = ((rect[1] - pad) / self.bin_ft).floor().max(0.0) as usize;
        let bx1 = (((rect[2] + pad) / self.bin_ft).floor().max(0.0) as usize).min(self.bins_w - 1);
        let by1 = (((rect[3] + pad) / self.bin_ft).floor().max(0.0) as usize).min(self.bins_h - 1);
        let mut out = Vec::new();
        for by in by0.min(by1)..=by1 {
            for bx in bx0.min(bx1)..=bx1 {
                out.extend_from_slice(&self.bins[by * self.bins_w + bx]);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

impl RoadNet {
    /// Nearest point (ft) of any road curve within `reach` of (x, y).
    pub fn nearest_point(&self, x: f64, y: f64, reach: f64) -> Option<(f64, f64)> {
        let mut best: Option<(f64, [f64; 2])> = None;
        for (ri, k) in self.segments_near([x - reach, y - reach, x + reach, y + reach], 0.0) {
            let r = &self.roads[ri as usize];
            let n = ((r.s[k as usize + 1] - r.s[k as usize]) / 50.0).ceil().clamp(1.0, 400.0) as usize;
            for j in 0..=n {
                let p = r.eval(k as usize, j as f64 / n as f64, 5.0, self.cell_ft).p;
                let d = (p[0] - x) * (p[0] - x) + (p[1] - y) * (p[1] - y);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, p));
                }
            }
        }
        best.map(|(_, p)| (p[0], p[1]))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RoadPoint {
    pub p: [f64; 2],
    pub z: f64,
}

pub struct RoadPiece {
    pub class: RoadClass,
    pub pts: Vec<RoadPoint>,
}

/// Road polylines near a rectangle, resampled at about `spacing` (never coarser than the
/// stored points), consecutive segments of one road joined into one piece.
pub fn pieces(net: &RoadNet, rect: [f64; 4], pad: f64, spacing: f64) -> Vec<RoadPiece> {
    let near = net.segments_near(rect, pad);
    let mut out: Vec<RoadPiece> = Vec::new();
    let mut i = 0;
    while i < near.len() {
        let (ri, k0) = near[i];
        let mut k1 = k0;
        while i + 1 < near.len() && near[i + 1].0 == ri && near[i + 1].1 == k1 + 1 {
            i += 1;
            k1 += 1;
        }
        i += 1;
        let r = &net.roads[ri as usize];
        let mut pts: Vec<RoadPoint> = Vec::new();
        for k in k0 as usize..=k1 as usize {
            let len = r.s[k + 1] - r.s[k];
            // Enough points to follow the curve (bends, kinks) without over-sampling; levels
            // too coarse to carve only need half their sample spacing.
            let step = if spacing > CARVE_MAX_SPACING_FT { 0.5 * spacing } else { spacing.max(KINK_WAVELENGTH_FT / 12.0).min(net.cell_ft / 6.0) };
            let n = (len / step).ceil().clamp(1.0, 512.0) as usize;
            for j in 0..=n {
                if j == 0 && !pts.is_empty() {
                    continue;
                }
                pts.push(r.eval(k, j as f64 / n as f64, spacing, net.cell_ft));
            }
        }
        if pts.len() >= 2 {
            out.push(RoadPiece { class: r.class, pts });
        }
    }
    out
}

/// Cut and fill a flat road bed with side slopes into a padded height grid.
///
/// Every road bounds each nearby sample's height to a band around its surface (exact on the
/// bed, widening with the side slope away from it). Bands are combined order-independently:
/// lower bounds by max, upper bounds by min; if roads disagree (junctions) the midpoint wins.
/// Returns a per-sample road mask (class + 1, 0 = none) for surface shading.
pub fn carve(pieces: &[RoadPiece], heights: &mut [f32], water: &[f32], dim: usize, origin: [f64; 2], spacing: f64) -> Vec<u8> {
    let n = dim * dim;
    let mut lo = vec![f32::NEG_INFINITY; n];
    let mut hi = vec![f32::INFINITY; n];
    let mut mask = vec![0u8; n];
    // Beds win over side slopes: where roads at different heights pass close (a crossing on
    // a hillside), each keeps its own bed and the ground between steps instead of both beds
    // being cut to their average. Beds that overlap each other average (a junction).
    let mut bed_sum = vec![0f64; n];
    let mut bed_n = vec![0u16; n];
    for piece in pieces {
        let half = 0.5 * piece.class.width_ft();
        // Side slopes reach until they would meet any plausible terrain offset.
        let reach = half + 60.0 / SIDE_SLOPE;
        for seg in piece.pts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let (sx0, sx1) = ((a.p[0].min(b.p[0]) - reach - origin[0]) / spacing, (a.p[0].max(b.p[0]) + reach - origin[0]) / spacing);
            let (sy0, sy1) = ((a.p[1].min(b.p[1]) - reach - origin[1]) / spacing, (a.p[1].max(b.p[1]) + reach - origin[1]) / spacing);
            if sx1 < 0.0 || sy1 < 0.0 || sx0 > (dim - 1) as f64 || sy0 > (dim - 1) as f64 {
                continue;
            }
            let (ix0, ix1) = (sx0.ceil().max(0.0) as usize, (sx1.floor() as usize).min(dim - 1));
            let (iy0, iy1) = (sy0.ceil().max(0.0) as usize, (sy1.floor() as usize).min(dim - 1));
            let (dx, dy) = (b.p[0] - a.p[0], b.p[1] - a.p[1]);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            for iy in iy0..=iy1 {
                let py = origin[1] + iy as f64 * spacing;
                for ix in ix0..=ix1 {
                    let px = origin[0] + ix as f64 * spacing;
                    let t = (((px - a.p[0]) * dx + (py - a.p[1]) * dy) / len2).clamp(0.0, 1.0);
                    let (cx, cy) = (a.p[0] + dx * t - px, a.p[1] + dy * t - py);
                    let d = crate::core::sqrt(cx * cx + cy * cy);
                    if d > reach {
                        continue;
                    }
                    let z = a.z + (b.z - a.z) * t;
                    let band = (d - half).max(0.0) * SIDE_SLOPE;
                    let k = iy * dim + ix;
                    lo[k] = lo[k].max((z - band) as f32);
                    hi[k] = hi[k].min((z + band) as f32);
                    if d <= half {
                        bed_sum[k] += z;
                        bed_n[k] += 1;
                    }
                    if d <= half + 0.5 * spacing {
                        let c = piece.class as u8 + 1;
                        mask[k] = if mask[k] == 0 { c } else { mask[k].min(c) };
                    }
                }
            }
        }
    }
    for k in 0..n {
        // River water (its channel) stays as the river carved it: the road crosses on a bridge.
        if lo[k] == f32::NEG_INFINITY || water[k] > crate::t0::hydro::DRY {
            continue;
        }
        heights[k] = if bed_n[k] > 0 {
            (bed_sum[k] / bed_n[k] as f64) as f32
        } else if lo[k] <= hi[k] {
            heights[k].clamp(lo[k], hi[k])
        } else {
            0.5 * (lo[k] + hi[k])
        };
    }
    mask
}

/// Roads are routed on the T0 grid, where a river is a line; at fine levels it meanders
/// across a belt. A road running along or slantwise through that belt would cross the
/// meanders again and again. Where a road runs inside a belt it is pushed out to the side it
/// came from; if it leaves on the other side it crosses once, square to the belt, midway.
/// Returns the road resampled every ~200 ft (wander baked in, so 0) if anything moved.
/// `blocked(x, y)`: standing water, where a road must not be pushed.
/// Belt samples per river segment (33 along it: centre, normal, half width), shared by every
/// road's `unweave` (roads beside a river ask for the same segments over and over).
#[derive(Default)]
pub struct BeltCache(std::cell::RefCell<std::collections::HashMap<(u32, u32), std::rc::Rc<[([f64; 2], [f64; 2], f64); 33]>>>);

impl BeltCache {
    fn get(&self, rivers: &crate::lod::rivers::RiverNet, ri: u32, k: u32, cell_ft: f64) -> std::rc::Rc<[([f64; 2], [f64; 2], f64); 33]> {
        self.0
            .borrow_mut()
            .entry((ri, k))
            .or_insert_with(|| std::rc::Rc::new(std::array::from_fn(|j| rivers.rivers[ri as usize].belt(k as usize, j as f64 / 32.0, cell_ft))))
            .clone()
    }
}

pub fn unweave(curve: &RoadCurve, rivers: &crate::lod::rivers::RiverNet, cell_ft: f64, blocked: &dyn Fn(f64, f64) -> bool, cache: &BeltCache) -> Option<(Vec<[f64; 2]>, Vec<f32>, Vec<f32>)> {
    const STEP: f64 = 200.0;
    let margin = 0.5 * curve.class.width_ft() + 40.0;
    let (mut pts, mut zs, mut sv): (Vec<[f64; 2]>, Vec<f32>, Vec<f64>) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..curve.pts.len().saturating_sub(1) {
        let len = curve.s[k + 1] - curve.s[k];
        let n = (len / STEP).ceil().max(1.0) as usize;
        // The segment's rise is spread by distance along the samples (never shorter than
        // the chord), so the resampled profile is no steeper than the original.
        let seg: Vec<[f64; 2]> = (0..=n).map(|j| curve.eval(k, j as f64 / n as f64, 2.5, cell_ft).p).collect();
        let mut cum = vec![0.0; n + 1];
        for j in 1..=n {
            cum[j] = cum[j - 1] + dist(seg[j - 1], seg[j]);
        }
        let (z0, z1) = (curve.z[k] as f64, curve.z[k + 1] as f64);
        for j in 0..=n {
            if j == 0 && k > 0 {
                continue;
            }
            pts.push(seg[j]);
            zs.push((z0 + (z1 - z0) * cum[j] / cum[n].max(1e-9)) as f32);
            sv.push(curve.s[k] + len * j as f64 / n as f64);
        }
    }
    let n = pts.len();
    if n < 3 {
        return None;
    }
    // Nearest belt: (river, signed offset from its centre line, half width, unit normal).
    let belt_near = |p: [f64; 2]| -> Option<(u32, f64, f64, [f64; 2])> {
        let reach = 0.6 * cell_ft;
        let mut best: Option<(f64, (u32, f64, f64, [f64; 2]))> = None;
        for (ri, k) in rivers.segments_near(p[0] - reach, p[1] - reach, p[0] + reach, p[1] + reach, 0.0) {
            let r = &rivers.rivers[ri as usize];
            let k = k as usize;
            let (a, b) = (r.pts[k], r.pts[k + 1]);
            let pad = 0.6 * cell_ft + 2.5 * crate::lod::rivers::width_ft(r.q[k].max(r.q[k + 1]) as f64) + margin;
            if seg_dist(p, a, b) > pad {
                continue;
            }
            for &(c, nrm, half) in cache.get(rivers, ri, k as u32, cell_ft).iter() {
                let dd = dist(c, p);
                if best.is_none_or(|x| dd < x.0) {
                    let d = (p[0] - c[0]) * nrm[0] + (p[1] - c[1]) * nrm[1];
                    best = Some((dd, (ri, d, half, nrm)));
                }
            }
        }
        best.map(|b| b.1).filter(|&(_, d, half, _)| d.abs() < half + margin)
    };
    let mut changed = false;
    for _pass in 0..2 {
        let inside: Vec<Option<(u32, f64, f64, [f64; 2])>> = pts.iter().map(|&p| belt_near(p)).collect();
        let mut i = 0;
        while i < n {
            let Some((river, ..)) = inside[i] else {
                i += 1;
                continue;
            };
            let i0 = i;
            while i + 1 < n && inside[i + 1].is_some_and(|x| x.0 == river) {
                i += 1;
            }
            let i1 = i;
            i += 1;
            let side = |j: usize| if inside[j].unwrap().1 >= 0.0 { 1.0 } else { -1.0 };
            let (entry, exit) = (side(i0), side(i1));
            let half = (i0..=i1).map(|j| inside[j].unwrap().2).fold(0.0, f64::max);
            // A short run from one side to the other is already a clean crossing.
            if entry != exit && sv[i1] - sv[i0] < 2.5 * (half + margin) {
                continue;
            }
            let mid = (i0 + i1) / 2;
            let mut moved: Vec<(usize, [f64; 2])> = Vec::new();
            for j in i0.max(1)..=i1.min(n - 2) {
                let (_, d, half, nrm) = inside[j].unwrap();
                let s = if entry == exit || j <= mid { entry } else { exit };
                let off = s * (half + margin) - d;
                moved.push((j, [pts[j][0] + nrm[0] * off, pts[j][1] + nrm[1] * off]));
            }
            if moved.iter().any(|(_, q)| blocked(q[0], q[1])) {
                continue;
            }
            // The profile keeps its grade limit over the moved chords.
            let mut trial = pts.clone();
            for &(j, q) in &moved {
                trial[j] = q;
            }
            let gmax = curve.class.max_grade();
            let lo = i0.saturating_sub(1).max(1);
            if (lo..=(i1 + 1).min(n - 1)).any(|j| (zs[j] - zs[j - 1]).abs() as f64 > gmax * dist(trial[j], trial[j - 1])) {
                continue;
            }
            for (j, q) in moved {
                pts[j] = q;
                changed = true;
            }
        }
    }
    changed.then(|| {
        let wander = vec![0.0; n];
        (pts, zs, wander)
    })
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = (dx * dx + dy * dy).max(1e-12);
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
    dist(p, [a[0] + dx * t, a[1] + dy * t])
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    crate::core::sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]))
}
