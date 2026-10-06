//! Rivers as continuous curves, shared by every zoom level.
//!
//! Each T0 river chain becomes a parametric curve: a Catmull-Rom spline through its cell
//! centers plus a lateral meander offset (wavelength ≈ 11 channel widths, amplitude growing
//! as the slope flattens, tapered to zero at sources, mouths and confluences) and a small
//! irregular wiggle. The curve is a pure function of (river, segment, t), so every tile at
//! every level samples the same line: the vector ink line, the carved channel and the water
//! surface always agree, and neighbouring tiles agree exactly (seams).
//!
//! Per tile, `carve` lowers terrain into a parabolic channel and valley banks (an
//! order-independent `min`) and raises the water surface to the river level (`max`), and
//! `strips` returns the curve as polylines for the renderer.

use crate::core::noise::{gradient2, smoothstep};
use crate::core::rng::hash2;

/// Channel width (ft) from discharge (mm·cells); matches the renderer's expectations.
pub fn width_ft(q: f64) -> f64 {
    (14.0 * libm::pow((q / 90_000.0).max(0.05), 0.6)).max(8.0)
}

fn depth_ft(width: f64) -> f64 {
    2.0 + 0.02 * width
}

fn wavelength_ft(width: f64) -> f64 {
    (11.0 * width).clamp(200.0, 25_000.0)
}

/// Valley-side slope beyond the banks (rise/run).
const BANK_SLOPE: f64 = 0.1;
/// Levee crest slope away from the bank, and where across the levee band (in band widths)
/// it starts and finishes easing back down to the ground.
const LEVEE_SLOPE: f64 = 0.04;
const LEVEE_FADE: (f64, f64) = (0.3, 1.5);
/// Where across its reach (a fraction) the valley starts easing back up to the ground, so
/// where the ground stands high above an incised river its sides steepen gradually.
const VALLEY_EASE: f64 = 0.4;

#[derive(Clone, Debug, Default)]
pub struct RiverCurve {
    /// Control points (ft): T0 cell centers, source to mouth.
    pub pts: Vec<[f64; 2]>,
    /// Water surface elevation at each control point (ft), non-increasing.
    pub z: Vec<f32>,
    /// Discharge at each control point (mm·cells).
    pub q: Vec<f32>,
    /// Meander amplitude factor 0..1 (0 at ends and confluences).
    pub taper: Vec<f32>,
    /// Cumulative chord length (ft) and meander phase (rad) at each control point.
    pub s: Vec<f64>,
    pub phase: Vec<f64>,
    /// Cumulative wiggle-noise coordinate (∫ ds / 4w): noise is sampled along this, not at
    /// s / 4w, which would race through the noise wherever the width changes.
    pub wphase: Vec<f64>,
    pub seed: u64,
}

/// All river curves plus a coarse spatial index of their segments.
#[derive(Clone, Debug, Default)]
pub struct RiverNet {
    pub rivers: Vec<RiverCurve>,
    bin_ft: f64,
    bins_w: usize,
    bins_h: usize,
    /// Per bin: (river, segment) pairs whose padded bounds touch the bin.
    bins: Vec<Vec<(u32, u32)>>,
    /// Largest lateral excursion of any curve from its control polygon (ft).
    max_offset_ft: f64,
}

#[derive(Clone, Copy)]
pub struct CurvePoint {
    pub p: [f64; 2],
    pub z: f64,
    pub w: f64,
    pub q: f64,
}

impl RiverCurve {
    /// Build from a chain of T0 cells. `z` must be non-increasing; `max_amp_ft` caps meanders.
    pub fn new(pts: Vec<[f64; 2]>, mut z: Vec<f32>, q: Vec<f32>, taper: Vec<f32>, seed: u64) -> RiverCurve {
        for k in 1..z.len() {
            z[k] = z[k].min(z[k - 1]);
        }
        let mut s = vec![0.0; pts.len()];
        let mut phase = vec![0.0; pts.len()];
        let mut wphase = vec![0.0; pts.len()];
        phase[0] = crate::core::rng::unit(seed) * std::f64::consts::TAU;
        for k in 1..pts.len() {
            let d = dist(pts[k - 1], pts[k]);
            s[k] = s[k - 1] + d;
            let w = width_ft(0.5 * (q[k - 1] + q[k]) as f64);
            phase[k] = phase[k - 1] + std::f64::consts::TAU * d / wavelength_ft(w);
            wphase[k] = wphase[k - 1] + d / (4.0 * w);
        }
        RiverCurve { pts, z, q, taper, s, phase, wphase, seed }
    }

    pub fn segments(&self) -> usize {
        self.pts.len().saturating_sub(1)
    }

    /// Point on segment `k` at `t` in [0, 1]. `spacing` is the sample spacing of the level
    /// asking: meanders it cannot resolve fade out, so coarse levels see a smooth line.
    pub fn eval(&self, k: usize, t: f64, spacing: f64, cell_ft: f64) -> CurvePoint {
        // Joints are always evaluated as the start of the next segment, so a joint is the
        // same bits no matter which segment asked for it.
        if t >= 1.0 && k + 1 < self.segments() {
            return self.eval(k + 1, 0.0, spacing, cell_ft);
        }
        let f = self.frame(k, t, cell_ft);
        // Unresolvable meanders fade out at coarse levels instead of aliasing into zigzags.
        let resolve = 1.0 - smoothstep(0.12, 0.35, spacing / f.lambda);
        let resolve_drift = 1.0 - smoothstep(0.12, 0.35, spacing / f.drift_len);
        let off = f.taper * (resolve * (f.amp * libm::sin(f.phase) + f.wiggle) + resolve_drift * f.drift);
        CurvePoint { p: [f.base[0] + f.nrm[0] * off, f.base[1] + f.nrm[1] * off], z: f.z, w: f.w, q: f.q }
    }

    /// The band the finest curve can wander in around segment `k` at `t`: its centre line
    /// (spline plus the cell-scale drift), unit normal, and half width (meander and wiggle
    /// reach plus half the channel).
    pub fn belt(&self, k: usize, t: f64, cell_ft: f64) -> ([f64; 2], [f64; 2], f64) {
        let f = self.frame(k, t, cell_ft);
        let off = f.taper * f.drift;
        let half = f.taper * (f.amp + 1.2 * f.w) + 0.5 * f.w;
        ([f.base[0] + f.nrm[0] * off, f.base[1] + f.nrm[1] * off], f.nrm, half)
    }

    fn frame(&self, k: usize, t: f64, cell_ft: f64) -> Frame {
        let n = self.pts.len();
        let p = |i: isize| self.pts[i.clamp(0, n as isize - 1) as usize];
        let (p0, p1, p2, p3) = (p(k as isize - 1), p(k as isize), p(k as isize + 1), p(k as isize + 2));
        let (t2, t3) = (t * t, t * t * t);
        let cr = |a: f64, b: f64, c: f64, d: f64| {
            0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3)
        };
        let dcr = |a: f64, b: f64, c: f64, d: f64| {
            0.5 * ((-a + c) + 2.0 * (2.0 * a - 5.0 * b + 4.0 * c - d) * t + 3.0 * (-a + 3.0 * b - 3.0 * c + d) * t2)
        };
        let base = [cr(p0[0], p1[0], p2[0], p3[0]), cr(p0[1], p1[1], p2[1], p3[1])];
        let tan = [dcr(p0[0], p1[0], p2[0], p3[0]), dcr(p0[1], p1[1], p2[1], p3[1])];
        let tl = crate::core::sqrt(tan[0] * tan[0] + tan[1] * tan[1]).max(1e-9);
        let nrm = [-tan[1] / tl, tan[0] / tl];

        let k1 = (k + 1).min(n - 1);
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        let q = lerp(self.q[k] as f64, self.q[k1] as f64);
        let z = lerp(self.z[k] as f64, self.z[k1] as f64);
        let s = lerp(self.s[k], self.s[k1]);
        let phase = lerp(self.phase[k], self.phase[k1]);
        let taper = lerp(self.taper[k] as f64, self.taper[k1] as f64);
        let w = width_ft(q);
        let lambda = wavelength_ft(w);

        // Sinuosity from the slope at each control point (centred difference), interpolated,
        // so meander amplitude is continuous across joints.
        let sinu_at = |i: usize| {
            let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
            let slope = (self.z[a] - self.z[b]) as f64 / (self.s[b] - self.s[a]).max(1.0);
            1.0 - smoothstep(0.0015, 0.015, slope)
        };
        let sinuosity = lerp(sinu_at(k), sinu_at(k1));
        // Noise coordinates accumulate along the river (like the meander phase), so they
        // advance smoothly even where the width changes.
        let vary = 0.6 + 0.4 * gradient2(self.seed, phase / (3.0 * std::f64::consts::TAU), 0.37);
        let amp = (0.32 * lambda * sinuosity * vary).min(0.3 * cell_ft);
        let wphase = lerp(self.wphase[k], self.wphase[k1]);
        let wiggle = 1.2 * w * gradient2(self.seed ^ 0x55, wphase, 0.71);
        // Cell-scale drift: even small streams never run straight between T0 cells.
        let drift_len = 2.5 * cell_ft;
        let drift = 0.22 * cell_ft * gradient2(self.seed ^ 0x99, s / drift_len, 0.13);
        Frame { base, nrm, z, w, q, taper, lambda, amp, phase, wiggle, drift_len, drift }
    }
}

struct Frame {
    base: [f64; 2],
    nrm: [f64; 2],
    z: f64,
    w: f64,
    q: f64,
    taper: f64,
    lambda: f64,
    amp: f64,
    phase: f64,
    wiggle: f64,
    drift_len: f64,
    drift: f64,
}

impl RiverNet {
    pub fn new(rivers: Vec<RiverCurve>, map_w_ft: f64, map_h_ft: f64, cell_ft: f64) -> RiverNet {
        let bin_ft = cell_ft * 8.0;
        let bins_w = (map_w_ft / bin_ft).ceil() as usize + 1;
        let bins_h = (map_h_ft / bin_ft).ceil() as usize + 1;
        let mut bins = vec![Vec::new(); bins_w * bins_h];
        let max_offset_ft = 0.52 * cell_ft + 1.2 * width_ft(1e9) + cell_ft;
        for (ri, r) in rivers.iter().enumerate() {
            for k in 0..r.segments() {
                let (a, b) = (r.pts[k], r.pts[k + 1]);
                let pad = max_offset_ft;
                let (x0, x1) = ((a[0].min(b[0]) - pad) / bin_ft, (a[0].max(b[0]) + pad) / bin_ft);
                let (y0, y1) = ((a[1].min(b[1]) - pad) / bin_ft, (a[1].max(b[1]) + pad) / bin_ft);
                for by in (y0.floor().max(0.0) as usize)..=((y1.floor().max(0.0) as usize).min(bins_h - 1)) {
                    for bx in (x0.floor().max(0.0) as usize)..=((x1.floor().max(0.0) as usize).min(bins_w - 1)) {
                        bins[by * bins_w + bx].push((ri as u32, k as u32));
                    }
                }
            }
        }
        RiverNet { rivers, bin_ft, bins_w, bins_h, bins, max_offset_ft }
    }

    /// (river, segment) pairs whose curves may pass within `pad` ft of the rectangle.
    pub fn segments_near(&self, x0: f64, y0: f64, x1: f64, y1: f64, pad: f64) -> Vec<(u32, u32)> {
        let reach = pad + self.max_offset_ft;
        let bx0 = ((x0 - reach) / self.bin_ft).floor().max(0.0) as usize;
        let by0 = ((y0 - reach) / self.bin_ft).floor().max(0.0) as usize;
        let bx1 = (((x1 + reach) / self.bin_ft).floor().max(0.0) as usize).min(self.bins_w - 1);
        let by1 = (((y1 + reach) / self.bin_ft).floor().max(0.0) as usize).min(self.bins_h - 1);
        let mut out = Vec::new();
        for by in by0..=by1.max(by0).min(self.bins_h - 1) {
            for bx in bx0..=bx1.max(bx0).min(self.bins_w - 1) {
                out.extend_from_slice(&self.bins[by * self.bins_w + bx]);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// A sampled polyline piece of one river near a tile.
pub struct Piece {
    pub pts: Vec<CurvePoint>,
}

/// Sample every river curve near the rectangle at a resolution suited to `spacing`.
/// `pad(w)`: how far beyond the rectangle (ft) a river `w` ft wide still matters (its
/// carving reach), so wide rivers are sampled from farther out than creeks.
pub fn pieces(net: &RiverNet, rect: [f64; 4], pad: &dyn Fn(f64) -> f64, spacing: f64, cell_ft: f64) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    let seg_w = |r: &RiverCurve, k: usize| width_ft(r.q[k].max(r.q[k + 1]) as f64);
    // Bins with the widest river's pad, then each segment by its own.
    let near: Vec<(u32, u32)> = net
        .segments_near(rect[0], rect[1], rect[2], rect[3], pad(width_ft(1e9)))
        .into_iter()
        .filter(|&(ri, k)| {
            let r = &net.rivers[ri as usize];
            let (a, b) = (r.pts[k as usize], r.pts[k as usize + 1]);
            // How far this segment's curve can stray from its chord (see `max_offset_ft`).
            let w = seg_w(r, k as usize);
            let m = pad(w) + 1.52 * cell_ft + 1.2 * w;
            a[0].max(b[0]) + m >= rect[0] && a[0].min(b[0]) - m <= rect[2] && a[1].max(b[1]) + m >= rect[1] && a[1].min(b[1]) - m <= rect[3]
        })
        .collect();
    let mut i = 0;
    while i < near.len() {
        // Consecutive segments of one river are sampled as one piece (shared endpoints).
        let (ri, k0) = near[i];
        let mut k1 = k0;
        while i + 1 < near.len() && near[i + 1].0 == ri && near[i + 1].1 == k1 + 1 {
            i += 1;
            k1 += 1;
        }
        i += 1;
        let r = &net.rivers[ri as usize];
        let mut cur: Vec<CurvePoint> = Vec::new();
        let mut last_out: Option<CurvePoint> = None;
        for k in k0..=k1 {
            let k = k as usize;
            let seg_len = (r.s[k + 1] - r.s[k]).max(1.0);
            let w = seg_w(r, k);
            let step = (wavelength_ft(w) / 16.0).max(spacing).min(seg_len);
            // Within a step of the padded rectangle: a chord that cuts its corner keeps both ends.
            let pad = pad(w) + step;
            let inside = |p: [f64; 2]| p[0] >= rect[0] - pad && p[0] <= rect[2] + pad && p[1] >= rect[1] - pad && p[1] <= rect[3] + pad;
            let n = (seg_len / step).ceil().max(1.0) as usize;
            for j in 0..=n {
                // A joint is the previous segment's last point.
                if j == 0 && k > k0 as usize {
                    continue;
                }
                let cp = r.eval(k, j as f64 / n as f64, spacing, cell_ft);
                if inside(cp.p) {
                    // A piece starts and ends one point outside, so the line reaches the edge.
                    if cur.is_empty()
                        && let Some(o) = last_out
                    {
                        cur.push(o);
                    }
                    cur.push(cp);
                } else if !cur.is_empty() {
                    cur.push(cp);
                    out.push(Piece { pts: std::mem::take(&mut cur) });
                }
                if !inside(cp.p) {
                    last_out = Some(cp);
                }
            }
        }
        if cur.len() >= 2 {
            out.push(Piece { pts: cur });
        }
    }
    out.retain(|p| p.pts.len() >= 2);
    out
}

/// Carve channels and valleys into a padded height grid and write river water levels.
/// `origin` is the world position of padded sample (0, 0); `water` has the same layout.
///
/// Every accumulator is a `min` over the segments near a sample, so the result is the same
/// whatever order rivers are visited in, and each follows the distance to the nearest point
/// of the curve (a `max` of a value growing with distance would pick the farthest segment
/// in reach, leaving a ring round every joint):
/// - `cut`: the ground carved to the channel and valley floor, fading back to the ground as
///   it was over the outer part of the carving reach (`VALLEY_EASE`; no cliff where the
///   ground stands above the valley side);
/// - `lev`, `ulev`: the levee floor just above the water and the position across the levee
///   band in band widths (0 at the bank), by which the levee eases out (`LEVEE_FADE`);
/// - `channel`: any channel covers the sample (channels beat banks).
///
/// Inside settlements (`town`: each padded sample's settlement-pad weight, 0 outside) the
/// gentler, wider valley of `town_valley` is mixed in by that weight. Settlement pads reset
/// the ground to what their layouts were planned on, which drops the broad valley coarser
/// levels carved; this puts one back (and `town::Site::height` plans on it). Its floor and
/// the distance to the nearest centre line are found on every `TOWN_STEP`th sample (a
/// lattice neighbouring tiles share) and interpolated: they are smooth, and the valley
/// reaches far.
///
/// `standing(x, y)` is the lake or sea surface at a world position (`DRY` on land). Banks
/// are levees: they exist only on land, never on the lake or sea floor a river runs out
/// into (there they would build a walled canal across the water).
#[allow(clippy::too_many_arguments)]
pub fn carve(pieces: &[Piece], heights: &mut [f32], water: &mut [f32], dim: usize, origin: [f64; 2], spacing: f64, reach_samples: f64, standing: &dyn Fn(f64, f64) -> f32, town: Option<&[f32]>) {
    let n = dim * dim;
    let mut cut = vec![f32::INFINITY; n];
    let mut lev = vec![f32::INFINITY; n];
    let mut ulev = vec![f32::INFINITY; n];
    let mut channel = vec![false; n];
    for piece in pieces {
        for seg in piece.pts.windows(2) {
            let (a, b) = (&seg[0], &seg[1]);
            let wmax = a.w.max(b.w);
            let reach = carve_reach(wmax, spacing, reach_samples);
            let (sx0, sx1) = ((a.p[0].min(b.p[0]) - reach - origin[0]) / spacing, (a.p[0].max(b.p[0]) + reach - origin[0]) / spacing);
            let (sy0, sy1) = ((a.p[1].min(b.p[1]) - reach - origin[1]) / spacing, (a.p[1].max(b.p[1]) + reach - origin[1]) / spacing);
            if sx1 < 0.0 || sy1 < 0.0 || sx0 > (dim - 1) as f64 || sy0 > (dim - 1) as f64 {
                continue;
            }
            let (iy0, iy1) = (sy0.ceil().max(0.0) as usize, (sy1.floor() as usize).min(dim - 1));
            let (dx, dy) = (b.p[0] - a.p[0], b.p[1] - a.p[1]);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            for iy in iy0..=iy1 {
                let py = origin[1] + iy as f64 * spacing;
                // Only the samples of this row inside the segment's capsule (the box round
                // it is mostly corners).
                let Some((x0, x1)) = capsule_row(a.p, b.p, reach, py) else { continue };
                let (sx0, sx1) = ((x0 - origin[0]) / spacing, (x1 - origin[0]) / spacing);
                if sx1 < 0.0 || sx0 > (dim - 1) as f64 {
                    continue;
                }
                let (ix0, ix1) = (sx0.ceil().max(0.0) as usize, (sx1.floor() as usize).min(dim - 1));
                for ix in ix0..=ix1 {
                    let px = origin[0] + ix as f64 * spacing;
                    let t = (((px - a.p[0]) * dx + (py - a.p[1]) * dy) / len2).clamp(0.0, 1.0);
                    let (cx, cy) = (a.p[0] + dx * t - px, a.p[1] + dy * t - py);
                    let d = crate::core::sqrt(cx * cx + cy * cy);
                    if d > reach {
                        continue;
                    }
                    let w = a.w + (b.w - a.w) * t;
                    let z = a.z + (b.z - a.z) * t;
                    let half = 0.5 * w;
                    let target = if d < half {
                        let u = d / half;
                        z - depth_ft(w) * (1.0 - u * u)
                    } else {
                        z + (d - half) * BANK_SLOPE
                    };
                    let k = iy * dim + ix;
                    let h = heights[k] as f64;
                    let mut v = h.min(target);
                    if d > VALLEY_EASE * reach {
                        v += (h - v) * smoothstep(VALLEY_EASE * reach, reach, d);
                    }
                    cut[k] = cut[k].min(v as f32);
                    if d < half {
                        channel[k] = true;
                    } else {
                        // Natural levee a foot or two high, sized to the river (not the level),
                        // its back slope eased out over half its width again.
                        let band = (0.35 * w).max(12.0);
                        if d < half + LEVEE_FADE.1 * band {
                            lev[k] = lev[k].min((z + 0.5 + (d - half) * LEVEE_SLOPE) as f32);
                            ulev[k] = ulev[k].min(((d - half) / band) as f32);
                        }
                    }
                    if d < half + spacing && (z as f32) > water[k] {
                        water[k] = z as f32;
                    }
                }
            }
        }
    }
    let town_field = town.map(|_| town_lattice(pieces, dim, origin, spacing));
    let m = (dim - 1).div_ceil(TOWN_STEP) + 1;
    for k in 0..n {
        let pad = town.map_or(0.0, |tw| tw[k]) as f64;
        if cut[k] == f32::INFINITY && pad <= 0.0 {
            continue;
        }
        let mut v = if cut[k] < f32::INFINITY { cut[k] as f64 } else { heights[k] as f64 };
        if pad > 0.0
            && let Some(f) = &town_field
        {
            // Bilinear between the lattice nodes round the sample.
            let (i, j) = (k % dim, k / dim);
            let (i0, j0) = ((i / TOWN_STEP).min(m - 2), (j / TOWN_STEP).min(m - 2));
            let (fx, fy) = ((i - i0 * TOWN_STEP) as f64 / TOWN_STEP as f64, (j - j0 * TOWN_STEP) as f64 / TOWN_STEP as f64);
            let at = |di: usize, dj: usize| f[(j0 + dj) * m + i0 + di];
            let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
            let mix = |c: usize| {
                let g = |di, dj| if c == 0 { at(di, dj).0 } else { at(di, dj).1 };
                lerp(lerp(g(0, 0), g(1, 0), fx), lerp(g(0, 1), g(1, 1), fx), fy)
            };
            let (floor, d) = (mix(0), mix(1));
            let h = heights[k] as f64;
            let t = h.min(floor);
            let valley = t + (h - t) * smoothstep(0.75 * TOWN_VALLEY_FT, TOWN_VALLEY_FT, d);
            v += pad * (v.min(valley) - v);
        }
        if !channel[k] && lev[k] < f32::INFINITY && (lev[k] as f64) > v {
            let (x, y) = (origin[0] + (k % dim) as f64 * spacing, origin[1] + (k / dim) as f64 * spacing);
            if standing(x, y) <= crate::t0::hydro::DRY {
                v += (1.0 - smoothstep(LEVEE_FADE.0, LEVEE_FADE.1, ulev[k] as f64)) * (lev[k] as f64 - v);
            }
        }
        heights[k] = v as f32;
    }
}

/// The x range (slightly widened) of the points on the row `y` within `r` of the segment
/// a–b: the union of its two end discs and the strip between them.
fn capsule_row(a: [f64; 2], b: [f64; 2], r: f64, y: f64) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in [a, b] {
        let dy = y - c[1];
        if dy.abs() <= r {
            let h = crate::core::sqrt(r * r - dy * dy);
            (lo, hi) = (lo.min(c[0] - h), hi.max(c[0] + h));
        }
    }
    // The strip: 0 ≤ projection ≤ len and |offset| ≤ r, each a slab in x on this row.
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = crate::core::sqrt(dx * dx + dy * dy);
    if len > 1e-9 {
        let (ux, uy) = (dx / len, dy / len);
        let ry = y - a[1];
        let (mut s0, mut s1) = (f64::NEG_INFINITY, f64::INFINITY);
        // Projection (x − ax)·ux + ry·uy in [0, len].
        if ux.abs() > 1e-12 {
            let (p, q) = ((0.0 - ry * uy) / ux, (len - ry * uy) / ux);
            (s0, s1) = (s0.max(p.min(q)), s1.min(p.max(q)));
        } else if !(0.0..=len).contains(&(ry * uy)) {
            s1 = s0;
        }
        // Offset −(x − ax)·uy + ry·ux in [−r, r].
        if uy.abs() > 1e-12 {
            let (p, q) = ((ry * ux - r) / uy, (ry * ux + r) / uy);
            (s0, s1) = (s0.max(p.min(q)), s1.min(p.max(q)));
        } else if (ry * ux).abs() > r {
            s1 = s0;
        }
        if s0 < s1 {
            (lo, hi) = (lo.min(a[0] + s0), hi.max(a[0] + s1));
        }
    }
    let pad = 1e-6 * (1.0 + r);
    (lo <= hi).then_some((lo - pad, hi + pad))
}

/// Inside settlements a river runs in a gentle valley (sides `TOWN_SLOPE`, gentle enough to
/// build on) out to `TOWN_VALLEY_FT` from its centre line, easing back into the ground over
/// the last quarter.
pub const TOWN_VALLEY_FT: f64 = 1_200.0;
pub const TOWN_SLOPE: f64 = 0.05;
const TOWN_STEP: usize = 8;

/// The town valley's floor (`town_valley` before the ground) and the distance to the nearest
/// centre line at every `TOWN_STEP`th padded sample (row-major, `(dim - 1) / TOWN_STEP + 1` a
/// side); far from any river, a floor high above any ground and twice the valley's reach.
fn town_lattice(pieces: &[Piece], dim: usize, origin: [f64; 2], spacing: f64) -> Vec<(f64, f64)> {
    let m = (dim - 1).div_ceil(TOWN_STEP) + 1;
    let mut out: Vec<(f64, f64)> = vec![(1e6, 2.0 * TOWN_VALLEY_FT); m * m];
    for (k, f) in out.iter_mut().enumerate() {
        let p = [origin[0] + ((k % m) * TOWN_STEP) as f64 * spacing, origin[1] + ((k / m) * TOWN_STEP) as f64 * spacing];
        for piece in pieces {
            for seg in piece.pts.windows(2) {
                let (a, b) = (&seg[0], &seg[1]);
                if (p[0] - a.p[0]).abs().min((p[0] - b.p[0]).abs()) > TOWN_VALLEY_FT + 400.0 || (p[1] - a.p[1]).abs().min((p[1] - b.p[1]).abs()) > TOWN_VALLEY_FT + 400.0 {
                    continue;
                }
                let (dx, dy) = (b.p[0] - a.p[0], b.p[1] - a.p[1]);
                let t = (((p[0] - a.p[0]) * dx + (p[1] - a.p[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
                let (cx, cy) = (a.p[0] + dx * t - p[0], a.p[1] + dy * t - p[1]);
                let d = crate::core::sqrt(cx * cx + cy * cy);
                if d < TOWN_VALLEY_FT {
                    let (w, z) = (a.w + (b.w - a.w) * t, a.z + (b.z - a.z) * t);
                    f.0 = f.0.min(z + (d - 0.5 * w).max(0.0) * TOWN_SLOPE);
                }
                f.1 = f.1.min(d);
            }
        }
    }
    out
}

/// The ground `h` at `d` ft from a river's centre line (half width `half`, surface `z`) in a
/// settlement's valley. Also what the settlement's layout is planned on (`town::Site`).
pub fn town_valley(h: f64, d: f64, half: f64, z: f64) -> f64 {
    let v = h.min(z + (d - half).max(0.0) * TOWN_SLOPE);
    v + (h - v) * smoothstep(0.75 * TOWN_VALLEY_FT, TOWN_VALLEY_FT, d)
}

/// How far (ft) from its centre line a river `w` ft wide carves at a level of `spacing`.
pub fn carve_reach(w: f64, spacing: f64, reach_samples: f64) -> f64 {
    w * 0.5 + (reach_samples * spacing).max(3.0 * w)
}

#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    crate::core::sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]))
}

/// Move a point (ft) out of any river channel and off its banks: pushed away from the
/// nearest point of the finest river curve until it is `margin` ft beyond the bank.
pub fn clear_of_rivers(net: &RiverNet, x: f64, y: f64, margin: f64, cell_ft: f64) -> (f64, f64) {
    let (mut px, mut py) = (x, y);
    for _ in 0..3 {
        let reach = 2_000.0 + margin;
        let mut best: Option<(f64, [f64; 2], f64)> = None;
        for (ri, k) in net.segments_near(px - reach, py - reach, px + reach, py + reach, 0.0) {
            let r = &net.rivers[ri as usize];
            // The curve stays within max_offset_ft of its control segment: skip segments that
            // cannot beat the best point so far.
            let (a, b) = (r.pts[k as usize], r.pts[k as usize + 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((px - a[0]) * dx + (py - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
            let chord = dist([a[0] + dx * t, a[1] + dy * t], [px, py]);
            if best.is_some_and(|b| chord - net.max_offset_ft > b.0) || chord - net.max_offset_ft > 0.5 * width_ft(1e9) + margin {
                continue;
            }
            // Every ~20 ft: the finest curve meanders well away from sparse samples.
            let n = ((r.s[k as usize + 1] - r.s[k as usize]) / 20.0).ceil().clamp(24.0, 2000.0) as usize;
            for s in 0..=n {
                let cp = r.eval(k as usize, s as f64 / n as f64, 2.5, cell_ft);
                let d = dist(cp.p, [px, py]);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, cp.p, cp.w));
                }
            }
        }
        let Some((d, p, w)) = best else { break };
        let need = 0.5 * w + margin;
        if d >= need {
            break;
        }
        let (dx, dy) = if d > 1e-6 { ((px - p[0]) / d, (py - p[1]) / d) } else { (1.0, 0.0) };
        px = p[0] + dx * need * 1.05;
        py = p[1] + dy * need * 1.05;
    }
    (px, py)
}

/// Per-river seed.
pub fn river_seed(world_seed: u64, index: usize) -> u64 {
    hash2(world_seed, index as i64, 0x5157)
}
