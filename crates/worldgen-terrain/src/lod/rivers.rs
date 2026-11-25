//! Rivers as continuous curves, shared by every zoom level.
//!
//! Each T0 river chain becomes a parametric curve: a Catmull-Rom spline through its cell
//! centers plus a lateral meander offset (wavelength ≈ 11 channel widths, amplitude growing
//! as the slope flattens, tapered to zero at sources, mouths and confluences; its phase warped
//! and its loops skewed by noise along the river, so no two reaches repeat) and a small
//! irregular wiggle. The curve is a pure function of (river, segment, t), so every tile at
//! every level samples the same line: the vector ink line, the carved channel and the water
//! surface always agree, and neighbouring tiles agree exactly (seams).
//!
//! Per tile, `carve` lowers terrain into a parabolic channel and valley banks (an
//! order-independent `min`) and raises the water surface to the river level (`max`), and
//! `strips` returns the curve as polylines for the renderer.

use crate::core::geom::dist;
use crate::core::noise::{gradient2, smoothstep};
use crate::core::rng::hash2;

/// Channel width (ft) from discharge (`t0::hydro::RIVER_Q`'s units); matches the renderer's expectations.
pub fn width_ft(q: f64) -> f64 {
    (14.0 * libm::pow((q / crate::t0::hydro::RIVER_Q).max(0.05), 0.6)).max(8.0)
}

fn depth_ft(width: f64) -> f64 {
    2.0 + 0.02 * width
}

fn wavelength_ft(width: f64) -> f64 {
    (11.0 * width).clamp(200.0, 25_000.0)
}

/// Valley-side slope beyond the banks (rise/run).
const BANK_SLOPE: f64 = 0.1;
/// Levee crest slope away from the bank (across the levee band), and its back slope beyond
/// (down to the ground: a broad embankment where the river crosses lower ground).
const LEVEE_SLOPE: f64 = 0.04;
const LEVEE_BACK: f64 = 0.15;
/// Where across its reach (a fraction) the valley starts easing back up to the ground, so
/// where the ground stands high above an incised river its sides steepen gradually.
const VALLEY_EASE: f64 = 0.4;
/// How much wider than the river its mouth opens where it meets standing water (over the
/// first three quarters of its last segment, from the last land cell to the first wet one):
/// a funnel, not a constant-width notch in the shore.
const MOUTH_FLARE: f64 = 1.6;

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
    /// Water surface at eighths of each segment (`RiverNet::settle`; empty: linear between
    /// the control points).
    pub prof: Vec<[f32; 9]>,
    /// Ends in a lake or the sea (not in another river): its last segment widens into an
    /// estuary (`MOUTH_FLARE`). Derived from the net (`RiverNet::new`), not saved.
    pub mouth: bool,
    /// Its waterfalls, by segment (`RiverNet::settle`, derived from the profiles: not saved).
    pub falls: Vec<FallAt>,
}

/// Where a river's water drops over a lip, as its curve sees it: on segment `seg` the surface
/// follows the profile to `t` (the lip, `top`), falls to `bottom` by `t1` (the face's foot) and
/// then keeps at or under the pool's level (`RiverCurve::frame`).
#[derive(Clone, Copy, Debug)]
pub struct FallAt {
    pub seg: u32,
    pub t: f64,
    pub t1: f64,
    pub top: f32,
    pub bottom: f32,
    /// Index in `RiverNet::falls`.
    pub id: u32,
}

impl FallAt {
    #[inline]
    fn level(&self, t: f64, profile: f64) -> f64 {
        if t <= self.t {
            profile
        } else if t < self.t1 {
            self.top as f64 + (self.bottom - self.top) as f64 * (t - self.t) / (self.t1 - self.t)
        } else {
            profile.min(self.bottom as f64)
        }
    }
}

/// A waterfall (Roadmap 7): a river above `FALL_MIN_WIDTH_FT` where its profile drops steeply
/// over a short reach (a knickpoint: an escarpment, a plateau's rim, a hill's edge, a sea cliff)
/// falls over a lip instead, into a plunge pool below a face `face_ft` across (ft, world).
#[derive(Clone, Copy, Debug)]
pub struct Fall {
    pub id: u32,
    pub river: u32,
    pub seg: u32,
    /// The lip (the finest curve's point there) and the way the water runs over it (unit).
    pub at: [f64; 2],
    pub dir: [f64; 2],
    /// Water at the lip and in the pool (ft, the world's datum).
    pub top: f32,
    pub bottom: f32,
    /// The channel's width at the lip (ft).
    pub width: f32,
    /// How far (ft) the face runs out from the lip to its foot.
    pub face_ft: f32,
    /// About one in three falls hides a cave behind the curtain: its mouth (ft), in the cliff at
    /// the back of the shelf behind the falling water, within the sheet's width (a
    /// `PoiKind::Cave` site stands there, hidden from players).
    pub cave: Option<[f64; 2]>,
    /// Where the shelf behind the water (`ledge`) comes out on its bank, by the water's edge just
    /// below the face's foot (ft). Every fall has one, cave or not (so a shelf tells nothing),
    /// unless the bank there is drowned or shut in (`RiverNet::drop_caves`).
    pub bank: Option<[f64; 2]>,
}

impl Fall {
    pub fn drop_ft(&self) -> f32 {
        self.top - self.bottom
    }
}

/// Narrowest river that falls (ft): creeks run down their steps as rapids.
pub const FALL_MIN_WIDTH_FT: f64 = 12.0;
/// Steepest eighth of a segment's profile (rise over run) that makes a knickpoint.
const FALL_GRADIENT: f64 = 0.2;
/// Least drop (ft) over the lip.
const FALL_MIN_DROP_FT: f64 = 40.0;
/// Greatest drop (ft) over one lip: a higher step runs on below the pool as rapids.
const FALL_MAX_DROP_FT: f64 = 800.0;
/// The walls of the gorge below a fall (rise over run; elsewhere `BANK_SLOPE`).
const GORGE_SLOPE: f64 = 0.5;
/// How far (ft, plus two widths) above a lip the channel's bed is kept to the river's depth.
const LIP_KEEP_FT: f64 = 120.0;
/// Falls on one river are at least this many segments apart (the bigger kept).
const FALL_SPACING_SEGS: u32 = 4;

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
    /// Each segment's curve as the coarsest carving level sees it (`VALLEY_CHORDS` chords:
    /// points with water level and width), for `valley`; filled by `settle`.
    coarse: Vec<Vec<[[f32; 4]; VALLEY_CHORDS + 1]>>,
    /// Those chords by `VALLEY_INDEX_FT` cell, each listed in every cell its box grown by its
    /// reach touches (cells across, down, lists), for `valley`.
    valley_index: (usize, usize, Vec<Vec<[[f32; 4]; 2]>>),
    /// The lattice (ft) of the T0 ground the profiles were last settled on (`T0` settles
    /// once per ground: settling again on the same ground changes nothing).
    pub settled_on: Option<f64>,
    /// The waterfalls, by id (`settle`: a pure function of the curves and the ground, less
    /// `dropped_falls`).
    pub falls: Vec<Fall>,
    /// Falls kept from standing, by (river, segment): those in a town's pad or under a road
    /// (`T0::generate` finds them once the towns and roads stand; saved with the T0).
    pub dropped_falls: Vec<(u32, u32)>,
    /// Falls that hide no cave after all, by (river, segment): their caves' mouths stood in a
    /// lake or the sea (`T0::generate`; saved with the T0).
    pub caveless_falls: Vec<(u32, u32)>,
}

/// Spacing (ft) of the coarsest level that carves valleys (2.5 · 2^n ≤ 2,500), whose wide,
/// eased valley sides `valley` reproduces.
const VALLEY_SPACING_FT: f64 = 1_280.0;
/// Cell (ft) of `RiverNet`'s chord index for `valley`.
const VALLEY_INDEX_FT: f64 = 8_000.0;
const VALLEY_CHORDS: usize = 5;

#[derive(Clone, Copy)]
pub struct CurvePoint {
    pub p: [f64; 2],
    pub z: f64,
    pub w: f64,
    pub q: f64,
    /// Below a fall's face on its segment: the fall's id (`RiverNet::falls`; `carve` keeps the
    /// pool's cut from eating back past the lip).
    pub fall: Option<u32>,
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
        RiverCurve { pts, z, q, taper, s, phase, wphase, seed, prof: Vec::new(), mouth: false, falls: Vec::new() }
    }

    pub fn segments(&self) -> usize {
        self.pts.len().saturating_sub(1)
    }

    /// The water surface on segment `k` at `t` as settled (no falls).
    fn profile(&self, k: usize, t: f64) -> f64 {
        match self.prof.get(k) {
            Some(pr) => {
                let u = (t * 8.0).clamp(0.0, 8.0);
                let j = (crate::core::floor(u) as usize).min(7);
                pr[j] as f64 + (pr[j + 1] - pr[j]) as f64 * (u - j as f64)
            }
            None => {
                let k1 = (k + 1).min(self.pts.len() - 1);
                self.z[k] as f64 + (self.z[k1] as f64 - self.z[k] as f64) * t
            }
        }
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
        // Unresolvable meanders fade out at coarse levels instead of aliasing into zigzags: a
        // loop under ~16 samples long draws as a regular sawtooth a few pixels across, so they
        // go by then. (Measured past 20 ft of spacing: the finest levels, and the generators
        // asking at 2.5 to 20 ft, see every loop as before.)
        let resolve = 1.0 - smoothstep(0.04, 0.12, (spacing - 20.0).max(0.0) / f.lambda_local);
        let resolve_drift = 1.0 - smoothstep(0.12, 0.35, spacing / f.drift_len);
        let off = f.taper * (resolve * (f.amp * libm::sin(f.phase) + f.wiggle) + resolve_drift * f.drift);
        let fall = self.falls.iter().find(|fa| fa.seg as usize == k && t >= fa.t1).map(|fa| fa.id);
        CurvePoint { p: [f.base[0] + f.nrm[0] * off, f.base[1] + f.nrm[1] * off], z: f.z, w: f.w, q: f.q, fall }
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

    /// The spline through the control points on segment `k` at `t`: its point and unit normal.
    #[inline]
    fn spline(&self, k: usize, t: f64) -> ([f64; 2], [f64; 2]) {
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
        (base, [-tan[1] / tl, tan[0] / tl])
    }

    /// Whether `eval(k, t, spacing, ..)` surely lies outside the box `[x0, y0, x1, y1]`, for
    /// `t` inside (0, 1), at a fraction of `eval`'s cost (`pieces` skips those points). The point
    /// stands off the spline by the drift, the meander and the wiggle (`eval`): the meander's
    /// amplitude is under 0.32 wavelengths and 0.3 cells, the wiggle under 1.2 widths and the
    /// drift under 0.22 cells (the noise keeps within ±1), each times a taper no greater than
    /// its ends'. So a spline point farther off than all three is out; one nearer is tried again
    /// from where the drift (one noise sample) puts it, with the other two's reach (`swing`:
    /// `swing_ft`, the segment's).
    fn misses(&self, k: usize, t: f64, spacing: f64, cell_ft: f64, bx: [f64; 4], swing: f64) -> bool {
        let out = |p: [f64; 2], m: f64| p[0] < bx[0] - m || p[0] > bx[2] + m || p[1] < bx[1] - m || p[1] > bx[3] + m;
        let (base, nrm) = self.spline(k, t);
        // (In the box: most likely in, and no bound will tell.)
        if !out(base, 0.0) {
            return false;
        }
        let k1 = (k + 1).min(self.pts.len() - 1);
        if out(base, swing + (self.taper[k].abs().max(self.taper[k1].abs())) as f64 * 0.22 * cell_ft) {
            return true;
        }
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        let at_mouth = self.mouth && k + 1 == self.segments();
        let taper = if at_mouth { self.taper[k] as f64 * (1.0 - smoothstep(0.45, 1.0, t)) } else { lerp(self.taper[k] as f64, self.taper[k1] as f64) };
        let drift_len = 2.5 * cell_ft;
        let drift = 0.22 * cell_ft * gradient2(self.seed ^ 0x99, lerp(self.s[k], self.s[k1]) / drift_len, 0.13);
        let off = taper * (1.0 - smoothstep(0.12, 0.35, spacing / drift_len)) * drift;
        out([base[0] + nrm[0] * off, base[1] + nrm[1] * off], swing)
    }

    /// How far the meander and the wiggle can carry segment `k`'s points off the drifted spline
    /// (`misses`), and a foot over (rounding in the interpolated width and taper).
    fn swing_ft(&self, k: usize, cell_ft: f64) -> f64 {
        let k1 = (k + 1).min(self.pts.len() - 1);
        let w = width_ft(self.q[k].max(self.q[k1]) as f64);
        (self.taper[k].abs().max(self.taper[k1].abs())) as f64 * ((0.32 * wavelength_ft(w)).min(0.3 * cell_ft) + 1.2 * w) + 1.0
    }

    fn frame(&self, k: usize, t: f64, cell_ft: f64) -> Frame {
        let n = self.pts.len();
        let (base, nrm) = self.spline(k, t);

        let k1 = (k + 1).min(n - 1);
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        let q = lerp(self.q[k] as f64, self.q[k1] as f64);
        let z = self.profile(k, t);
        let z = match self.falls.iter().find(|fa| fa.seg as usize == k) {
            Some(fa) => fa.level(t, z),
            None => z,
        };
        let s = lerp(self.s[k], self.s[k1]);
        let phase = lerp(self.phase[k], self.phase[k1]);
        // A mouth's last segment runs from its last land cell to the first wet one, the shore
        // about halfway: the river keeps its bends to the shore, not straightening a cell early.
        let at_mouth = self.mouth && k + 1 == self.segments();
        let taper = if at_mouth { self.taper[k] as f64 * (1.0 - smoothstep(0.45, 1.0, t)) } else { lerp(self.taper[k] as f64, self.taper[k1] as f64) };
        let w = width_ft(q);
        let lambda = wavelength_ft(w);
        // (The estuary's widening is the channel's only: the meanders keep the river's scale.)
        let w_out = if at_mouth { (w * (1.0 + MOUTH_FLARE * smoothstep(0.0, 0.75, t))).min(width_ft(1e9)) } else { w };

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
        // Loops are not all alike: the phase is warped by noise along the river (bends come
        // tighter here, longer there: local wavelength about 0.65 to 1.8 times the mean, never
        // running backwards) and each loop is skewed up or down stream by a slowly turning
        // amount; the amplitude swells and calms over a few loops (some reaches run nearly
        // straight) and differs loop to loop, never past the old greatest.
        use std::f64::consts::TAU;
        let (warp, dwarp, _) = crate::core::noise::gradient2_d(self.seed ^ 0x2B, phase / (3.0 * TAU), 0.29);
        let rate = (1.0 + 4.5 * dwarp / (3.0 * TAU)).max(0.35);
        let turn = phase + 4.5 * warp;
        let skew = 0.3 * gradient2(self.seed ^ 0x3C, phase / (4.0 * TAU), 0.61);
        let angle = turn + 0.5 * skew * libm::sin(2.0 * turn);
        let vary = (0.55 + 0.7 * gradient2(self.seed, phase / (3.0 * TAU), 0.37) + 0.3 * gradient2(self.seed ^ 0x4D, phase / (1.3 * TAU), 0.83)).clamp(0.12, 1.0);
        // (Where the warp tightens the bends, the loops come smaller too, so the curve never
        // swings sideways faster than unwarped loops would.)
        let amp = (0.32 * lambda * sinuosity * vary).min(0.3 * cell_ft) / (rate * (1.0 + skew.abs())).max(1.0);
        // Round bends, no hooked necks: a loop's tightest radius stays over ~0.8 channel widths
        // (about the old unwarped loops' least, less what the spline's own bend already takes), so
        // the inner bank never kinks, and no loop swings out past half the spline's radius, where
        // it would fold back on itself.
        // (Its curvature peaks near amp * angle'^2 * g, angle' = TAU / local wavelength, g the
        // skew's steepening.) The spline's curvature is taken at its points and interpolated,
        // so the cap is continuous across joints.
        let kappa_at = |i: usize| {
            if i == 0 || i + 1 >= n {
                return 0.0;
            }
            let (a, b, c) = (self.pts[i - 1], self.pts[i], self.pts[i + 1]);
            let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            let cross = (u[0] * v[1] - u[1] * v[0]).abs();
            2.0 * cross / (dist(a, b) * dist(b, c) * dist(a, c)).max(1e-9)
        };
        let kb = lerp(kappa_at(k), kappa_at(k1));
        let r_min = 0.8 * w;
        let g = (1.0 + skew.abs()) * (1.0 + skew.abs()) + 2.0 * skew.abs();
        let turn_rate = TAU * rate / lambda;
        let amp = amp.min((1.0 / r_min - kb).max(0.3 / r_min) / (turn_rate * turn_rate * g)).min(0.5 / kb.max(1e-12));
        let wphase = lerp(self.wphase[k], self.wphase[k1]);
        let wiggle = 1.2 * w * gradient2(self.seed ^ 0x55, wphase, 0.71);
        // Cell-scale drift: even small streams never run straight between T0 cells.
        let drift_len = 2.5 * cell_ft;
        let drift = 0.22 * cell_ft * gradient2(self.seed ^ 0x99, s / drift_len, 0.13);
        Frame { base, nrm, z, w: w_out, q, taper, lambda_local: lambda / rate, amp, phase: angle, wiggle, drift_len, drift }
    }
}

struct Frame {
    base: [f64; 2],
    nrm: [f64; 2],
    z: f64,
    w: f64,
    q: f64,
    taper: f64,
    /// Meander wavelength where the phase warp has it (ft).
    lambda_local: f64,
    amp: f64,
    phase: f64,
    wiggle: f64,
    drift_len: f64,
    drift: f64,
}

impl RiverNet {
    pub fn new(mut rivers: Vec<RiverCurve>, map_w_ft: f64, map_h_ft: f64, cell_ft: f64) -> RiverNet {
        // A river whose last point is no point of another river's ends in standing water.
        let mut joints: crate::core::hash::FastSet<(u64, u64)> = Default::default();
        for r in &rivers {
            for p in r.pts.iter().take(r.pts.len().saturating_sub(1)) {
                joints.insert((p[0].to_bits(), p[1].to_bits()));
            }
        }
        for r in &mut rivers {
            r.mouth = r.pts.last().is_some_and(|p| !joints.contains(&(p[0].to_bits(), p[1].to_bits())));
        }
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
        RiverNet { rivers, bin_ft, bins_w, bins_h, bins, max_offset_ft, coarse: Vec::new(), valley_index: (0, 0, Vec::new()), settled_on: None, falls: Vec::new(), dropped_falls: Vec::new(), caveless_falls: Vec::new() }
    }

    /// Water surfaces that follow the ground: the curve wanders off its cells, and the
    /// ground between two cells is not a straight line, so a surface linear between the cells'
    /// levels would stand above the land in places (held by a dike). Along each segment the
    /// water keeps its level until the ground (`ground(x, y)`, the T0 surface) falls away,
    /// then follows it a foot under, never below the level downstream: a pure function of
    /// the curve and the T0 grid, so generated and loaded worlds agree.
    pub fn settle(&mut self, ground: &dyn Fn(f64, f64) -> f64, cell_ft: f64) {
        self.settle_profiles(ground, cell_ft);
    }

    /// Control levels that agree with the ground the terrain is built on (`ground`, the T0
    /// surface as the coarsest terrain level samples it; a river's cells' heights can stand
    /// tens of feet above it in a valley narrower than that sampling): each land point at most
    /// a foot under the ground there, falling downstream, never below the lake or sea it runs
    /// into (`wet`: standing water); a tributary and its river meet at one level. Run once
    /// when the world is generated (curves' shapes follow the levels; the levels are saved).
    pub fn settle_levels(&mut self, ground: &dyn Fn(f64, f64) -> f64, wet: &dyn Fn(f64, f64) -> bool, cell_ft: f64) {
        let fixed: Vec<Vec<bool>> = self.rivers.iter().map(|r| r.pts.iter().map(|p| wet(p[0], p[1])).collect()).collect();
        // The level of the first standing water downstream of each point.
        let floors: Vec<Vec<f32>> = self
            .rivers
            .iter()
            .zip(&fixed)
            .map(|(r, fx)| {
                let n = r.pts.len();
                let mut f = vec![f32::MIN; n];
                for k in (0..n).rev() {
                    f[k] = if fx[k] { r.z[k] } else if k + 1 < n { f[k + 1] } else { f32::MIN };
                }
                f
            })
            .collect();
        let fall = |r: &mut RiverCurve, fx: &[bool], fl: &[f32], from: usize| {
            for k in from..r.z.len() {
                if !fx[k] && k > 0 {
                    r.z[k] = r.z[k].min(r.z[k - 1]).max(fl[k]);
                }
            }
        };
        for ((r, fx), fl) in self.rivers.iter_mut().zip(&fixed).zip(&floors) {
            for k in 0..r.z.len() {
                if !fx[k] {
                    r.z[k] = r.z[k].min((ground(r.pts[k][0], r.pts[k][1]) - 1.0) as f32).max(fl[k]);
                }
            }
            fall(r, fx, fl, 1);
        }
        // Junctions: a tributary's last point is a point of its river.
        let mut at: std::collections::BTreeMap<(u64, u64), (usize, usize)> = Default::default();
        for (ri, r) in self.rivers.iter().enumerate() {
            for (k, p) in r.pts.iter().enumerate().take(r.pts.len().saturating_sub(1)) {
                at.entry((p[0].to_bits(), p[1].to_bits())).or_insert((ri, k));
            }
        }
        for _ in 0..3 {
            for ri in 0..self.rivers.len() {
                let Some(&last) = self.rivers[ri].pts.last() else { continue };
                let Some(&(p, m)) = at.get(&(last[0].to_bits(), last[1].to_bits())) else { continue };
                if p == ri {
                    continue;
                }
                let end = self.rivers[ri].z.len() - 1;
                let level = self.rivers[ri].z[end].min(self.rivers[p].z[m]).max(floors[p][m]);
                self.rivers[ri].z[end] = level;
                if level < self.rivers[p].z[m] && !fixed[p][m] {
                    self.rivers[p].z[m] = level;
                    let (r, fx, fl) = (&mut self.rivers[p], &fixed[p], &floors[p]);
                    fall(r, fx, fl, m + 1);
                }
            }
        }
        // (A junction can lift a tributary's last level over the one before it, to the lake its
        // river runs into; levels never rise downstream, as a curve loaded from bytes has them.)
        for r in &mut self.rivers {
            for k in 1..r.z.len() {
                r.z[k] = r.z[k].min(r.z[k - 1]);
            }
        }
        // Meanders and drift carry the curve up to a cell off its cells' line: where that puts
        // it over ground lower than its water (a neighbouring valley), it would stand on a dike.
        // There the wander is halved (up to three times), drawing it back toward its cells. Only
        // where the wander is to blame: where the cells' own line stands as high over the ground
        // (a fall the levels step down over, a low shore by the mouth), calming would only
        // straighten the river into a canal and lift it no less.
        for r in &mut self.rivers {
            // (Looked up when a segment first asks, on the spline alone: `frame`'s base.)
            let mut base_perched: Vec<Option<bool>> = vec![None; r.segments()];
            let mut last: Option<Vec<bool>> = None;
            for _ in 0..3 {
                let mut calm = vec![false; r.pts.len()];
                for k in 0..r.segments() {
                    if r.taper[k] <= 0.0 && r.taper[k + 1] <= 0.0 {
                        continue;
                    }
                    // (A segment's points move only with its two ends' tapers: sound last round
                    // and neither halved since, it is sound still.)
                    if last.as_ref().is_some_and(|c| !c[k] && !c[k + 1]) {
                        continue;
                    }
                    let on_base = *base_perched[k].get_or_insert_with(|| {
                        (1..8).any(|j| {
                            let t = j as f64 / 8.0;
                            let p = r.spline(k, t).0;
                            let level = r.z[k] as f64 + (r.z[k + 1] - r.z[k]) as f64 * t;
                            !wet(p[0], p[1]) && level > ground(p[0], p[1]) + 3.0
                        })
                    });
                    if on_base {
                        continue;
                    }
                    let perched = (1..8).any(|j| {
                        let t = j as f64 / 8.0;
                        let c = r.eval(k, t, 2.5, cell_ft);
                        let level = r.z[k] as f64 + (r.z[k + 1] - r.z[k]) as f64 * t;
                        !wet(c.p[0], c.p[1]) && level > ground(c.p[0], c.p[1]) + 3.0
                    });
                    if perched {
                        calm[k] = true;
                        calm[k + 1] = true;
                    }
                }
                if !calm.iter().any(|c| *c) {
                    break;
                }
                for (t, c) in r.taper.iter_mut().zip(&calm) {
                    if *c {
                        *t *= 0.5;
                    }
                }
                last = Some(calm);
            }
        }
    }

    /// Waterfalls where a river at least `FALL_MIN_WIDTH_FT` wide meets a knickpoint: an eighth
    /// of a segment's settled profile steeper than `FALL_GRADIENT`, the steep run from there
    /// dropping at least `FALL_MIN_DROP_FT`. The lip stands a hashed way into that eighth; the
    /// water then falls over a face about a sixth as wide as it is high (2.5 to 25 ft) to the
    /// level at the steep run's end, and keeps at or under it (a short gorge below the face) until
    /// the profile is lower. The biggest falls are kept, `FALL_SPACING_SEGS` apart on a river;
    /// about a third hide a cave. O(segments).
    fn find_falls(&mut self, cell_ft: f64) {
        struct Cand {
            drop: f64,
            ri: u32,
            k: u32,
            t: f64,
            t1: f64,
            top: f64,
            bottom: f64,
            face: f64,
        }
        let mut cands: Vec<Cand> = Vec::new();
        for (ri, r) in self.rivers.iter_mut().enumerate() {
            r.falls.clear();
            for (k, pr) in r.prof.iter().enumerate() {
                let w = width_ft(0.5 * (r.q[k] + r.q[k + 1]) as f64);
                let seg_len = r.s[k + 1] - r.s[k];
                if w < FALL_MIN_WIDTH_FT || seg_len < 40.0 {
                    continue;
                }
                let eighth = seg_len / 8.0;
                let grad = |j: usize| (pr[j] - pr[j + 1]) as f64 / eighth;
                let Some(j) = (0..8).filter(|&j| grad(j) >= FALL_GRADIENT).max_by(|&a, &b| grad(a).total_cmp(&grad(b)).then(b.cmp(&a))) else { continue };
                let mut end = j + 1;
                while end < 8 && grad(end) >= 0.5 * FALL_GRADIENT {
                    end += 1;
                }
                let u = crate::core::rng::unit(hash2(r.seed ^ 0xFA11, k as i64, 1));
                let t = (j as f64 + 0.3 + 0.4 * u) / 8.0;
                let top = r.profile(k, t);
                let drop = (top - r.profile(k, end as f64 / 8.0)).min(FALL_MAX_DROP_FT);
                if drop < FALL_MIN_DROP_FT {
                    continue;
                }
                let face = (drop / 6.0).clamp(2.5, 25.0);
                let t1 = (t + face / seg_len).min(1.0 - 1e-3);
                if t1 <= t {
                    continue;
                }
                let bottom = (top - drop).min(r.profile(k, t1));
                cands.push(Cand { drop: top - bottom, ri: ri as u32, k: k as u32, t, t1, top, bottom, face });
            }
        }
        cands.sort_by(|a, b| b.drop.total_cmp(&a.drop).then(a.ri.cmp(&b.ri)).then(a.k.cmp(&b.k)));
        let mut kept: Vec<Cand> = Vec::new();
        for c in cands {
            if kept.iter().all(|o| o.ri != c.ri || o.k.abs_diff(c.k) >= FALL_SPACING_SEGS) {
                kept.push(c);
            }
        }
        // (After the spacing: a dropped fall leaves no smaller one in its place.)
        kept.retain(|c| !self.dropped_falls.contains(&(c.ri, c.k)));
        kept.sort_by(|a, b| a.ri.cmp(&b.ri).then(a.k.cmp(&b.k)));
        self.falls = kept
            .iter()
            .enumerate()
            .map(|(id, c)| {
                let r = &mut self.rivers[c.ri as usize];
                let (top, bottom) = (c.top as f32, c.bottom as f32);
                r.falls.push(FallAt { seg: c.k, t: c.t, t1: c.t1, top, bottom, id: id as u32 });
                let at = r.eval(c.k as usize, c.t, 2.5, cell_ft);
                let foot = r.eval(c.k as usize, c.t1, 2.5, cell_ft).p;
                let (dx, dy) = (foot[0] - at.p[0], foot[1] - at.p[1]);
                let l = crate::core::sqrt(dx * dx + dy * dy).max(1e-9);
                let dir = [dx / l, dy / l];
                let h = hash2(r.seed ^ 0xCA7E, c.k as i64, 2);
                // (By the finest curve just below the foot, its water's edge there: the shelf
                // `ledge` carves runs along it.)
                let side = if h >> 40 & 1 == 0 { 1.0 } else { -1.0 };
                let bank = (!self.caveless_falls.contains(&(c.ri, c.k))).then(|| {
                    let seg_len = (r.s[c.k as usize + 1] - r.s[c.k as usize]).max(1.0);
                    let (ta, tb) = ((c.t1 + 3.0 / seg_len).min(1.0), (c.t1 + 9.0 / seg_len).min(1.0));
                    let (a, b) = (r.eval(c.k as usize, ta, 2.5, cell_ft), r.eval(c.k as usize, tb, 2.5, cell_ft));
                    let (tx, ty) = (b.p[0] - a.p[0], b.p[1] - a.p[1]);
                    let tl = crate::core::sqrt(tx * tx + ty * ty);
                    let t = if tl > 1e-6 { [tx / tl, ty / tl] } else { dir };
                    let across = 0.5 * a.w + 5.0;
                    [a.p[0] - side * t[1] * across, a.p[1] + side * t[0] * across]
                });
                // (The mouth: on the shelf behind the sheet, a quarter of the way across from the
                // middle toward the shelf's bank.)
                let cave = (crate::core::rng::unit(h) < 1.0 / 3.0 && bank.is_some()).then(|| {
                    let (u, v) = behind_mouth(c.face, at.w, side);
                    [at.p[0] + dir[0] * u - dir[1] * v, at.p[1] + dir[1] * u + dir[0] * v]
                });
                Fall { id: id as u32, river: c.ri, seg: c.k, at: at.p, dir, top, bottom, width: at.w as f32, face_ft: c.face as f32, cave, bank }
            })
            .collect();
    }

    /// Keep the falls at `drop` (river, segment) from standing: their rivers run on down their
    /// steps as rapids, the falls after them renumbered (as `find_falls` would have them).
    pub fn drop_falls(&mut self, drop: &[(u32, u32)]) {
        self.dropped_falls.extend_from_slice(drop);
        self.dropped_falls.sort_unstable();
        self.dropped_falls.dedup();
        let dropped = &self.dropped_falls;
        let mut new_id = vec![u32::MAX; self.falls.len()];
        self.falls.retain(|f| !dropped.contains(&(f.river, f.seg)));
        for (id, f) in self.falls.iter_mut().enumerate() {
            new_id[f.id as usize] = id as u32;
            f.id = id as u32;
        }
        for r in &mut self.rivers {
            r.falls.retain(|fa| new_id[fa.id as usize] != u32::MAX);
            for fa in &mut r.falls {
                fa.id = new_id[fa.id as usize];
            }
        }
    }

    /// Take the caves from the falls at `at` (river, segment).
    pub fn drop_caves(&mut self, at: &[(u32, u32)]) {
        self.caveless_falls.extend_from_slice(at);
        self.caveless_falls.sort_unstable();
        self.caveless_falls.dedup();
        for f in &mut self.falls {
            if self.caveless_falls.contains(&(f.river, f.seg)) {
                f.cave = None;
                f.bank = None;
            }
        }
    }

    /// The falls whose lip lies in `rect` grown by `pad` (ft).
    pub fn falls_in(&self, rect: [f64; 4], pad: f64) -> impl Iterator<Item = &Fall> {
        self.falls.iter().filter(move |f| f.at[0] >= rect[0] - pad && f.at[0] <= rect[2] + pad && f.at[1] >= rect[1] - pad && f.at[1] <= rect[3] + pad)
    }

    /// The falls whose sheet or shelf (`sheet_zone`, `ledge_zone`) may reach into `rect` (ft):
    /// each padded by its own width and face.
    pub fn falls_touching(&self, rect: [f64; 4]) -> impl Iterator<Item = &Fall> {
        self.falls.iter().filter(move |f| {
            let pad = zone_reach_ft(f);
            f.at[0] >= rect[0] - pad && f.at[0] <= rect[2] + pad && f.at[1] >= rect[1] - pad && f.at[1] <= rect[3] + pad
        })
    }

    fn settle_profiles(&mut self, ground: &dyn Fn(f64, f64) -> f64, cell_ft: f64) {
        for r in &mut self.rivers {
            let prof: Vec<[f32; 9]> = (0..r.segments())
                .map(|k| {
                    let (top, bottom) = (r.z[k], r.z[k + 1]);
                    let mut pr = [top; 9];
                    let mut run = top as f64;
                    for (j, v) in pr.iter_mut().enumerate().take(8).skip(1) {
                        let p = r.eval(k, j as f64 / 8.0, 2.5, cell_ft).p;
                        run = run.min(ground(p[0], p[1]) - 1.0).max(bottom as f64);
                        *v = run as f32;
                    }
                    pr[8] = bottom;
                    pr
                })
                .collect();
            r.prof = prof;
        }
        self.find_falls(cell_ft);
        self.coarse = self
            .rivers
            .iter()
            .map(|r| {
                (0..r.segments())
                    .map(|k| {
                        std::array::from_fn(|j| {
                            let c = r.eval(k, j as f64 / VALLEY_CHORDS as f64, VALLEY_SPACING_FT, cell_ft);
                            [c.p[0] as f32, c.p[1] as f32, c.z as f32, c.w as f32]
                        })
                    })
                    .collect()
            })
            .collect();
        // The chords by cell, for `valley` (a point asks only its own cell's).
        let (mut x1, mut y1) = (0.0f64, 0.0f64);
        let grown = |a: [f32; 4], b: [f32; 4]| {
            let r = carve_reach(a[3].max(b[3]) as f64, VALLEY_SPACING_FT, 8.0) + 1.0;
            [a[0].min(b[0]) as f64 - r, a[1].min(b[1]) as f64 - r, a[0].max(b[0]) as f64 + r, a[1].max(b[1]) as f64 + r]
        };
        for c in self.coarse.iter().flatten().flat_map(|s| s.windows(2)) {
            let g = grown(c[0], c[1]);
            (x1, y1) = (x1.max(g[2]), y1.max(g[3]));
        }
        let (w, h) = ((x1 / VALLEY_INDEX_FT) as usize + 1, (y1 / VALLEY_INDEX_FT) as usize + 1);
        let mut cells: Vec<Vec<[[f32; 4]; 2]>> = vec![Vec::new(); w * h];
        for c in self.coarse.iter().flatten().flat_map(|s| s.windows(2)) {
            let g = grown(c[0], c[1]);
            let span = |lo: f64, hi: f64, n: usize| ((lo / VALLEY_INDEX_FT).floor().max(0.0) as usize, ((hi / VALLEY_INDEX_FT).floor().max(0.0) as usize).min(n - 1));
            let ((cx0, cx1), (cy0, cy1)) = (span(g[0], g[2], w), span(g[1], g[3], h));
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    cells[cy * w + cx].push([c[0], c[1]]);
                }
            }
        }
        self.valley_index = (w, h, cells);
    }

    /// The highest water surface (ft) of any river channel the straight line a–b crosses or
    /// comes within `margin` ft of (its finest curve in ~150-ft chords, sampled once per river
    /// segment into `cache`), and that river's width there.
    pub fn crossing_level(&self, a: [f64; 2], b: [f64; 2], margin: f64, cell_ft: f64, cache: &CrossingCache) -> Option<(f64, f64)> {
        let seg_dist = |p: [f64; 2], u: [f64; 2], v: [f64; 2]| {
            let (dx, dy) = (v[0] - u[0], v[1] - u[1]);
            let t = (((p[0] - u[0]) * dx + (p[1] - u[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
            dist(p, [u[0] + dx * t, u[1] + dy * t])
        };
        let (rx, ry) = (b[0] - a[0], b[1] - a[1]);
        let pad = margin + 0.5 * width_ft(1e9);
        let (x0, y0, x1, y1) = (a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1]));
        let mut level: Option<(f64, f64)> = None;
        for (ri, k) in self.segments_near(x0, y0, x1, y1, pad) {
            let seg = cache.get(self, ri, k, cell_ft);
            let (bb, pts) = (&seg.0, &seg.1);
            if bb[0] > x1 + pad || bb[2] < x0 - pad || bb[1] > y1 + pad || bb[3] < y0 - pad {
                continue;
            }
            for c in pts.windows(2) {
                let (p, q) = (c[0], c[1]);
                let reach = 0.5 * p[3].max(q[3]) + margin;
                if p[0].min(q[0]) > x1 + reach || p[0].max(q[0]) < x0 - reach || p[1].min(q[1]) > y1 + reach || p[1].max(q[1]) < y0 - reach {
                    continue;
                }
                let (pp, qq) = ([p[0], p[1]], [q[0], q[1]]);
                let (sx, sy) = (q[0] - p[0], q[1] - p[1]);
                let den = rx * sy - ry * sx;
                // Where along the chord (0..1) the road meets or passes nearest it.
                let along = |x: [f64; 2]| ((x[0] - p[0]) * sx + (x[1] - p[1]) * sy) / (sx * sx + sy * sy).max(1e-9);
                let mut at: Option<f64> = None;
                if den.abs() > 1e-9 {
                    let t = ((p[0] - a[0]) * sy - (p[1] - a[1]) * sx) / den;
                    let v = ((p[0] - a[0]) * ry - (p[1] - a[1]) * rx) / den;
                    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&v) {
                        at = Some(v);
                    }
                }
                if at.is_none() {
                    let (da, db) = (seg_dist(a, pp, qq), seg_dist(b, pp, qq));
                    if seg_dist(pp, a, b) < reach || seg_dist(qq, a, b) < reach || da < reach || db < reach {
                        at = Some(along(if da < db { a } else { b }).clamp(0.0, 1.0));
                    }
                }
                if let Some(v) = at {
                    let (z, w) = (p[2] + (q[2] - p[2]) * v, p[3] + (q[3] - p[3]) * v);
                    if level.is_none_or(|l| z > l.0) {
                        level = Some((z, w));
                    }
                }
            }
        }
        level
    }

    /// The ground `h` at (x, y) cut by the river valleys as the coarsest carving level cuts
    /// them (sides `BANK_SLOPE` from the water, easing back to the ground over the outer part
    /// of their reach; never below the water): what the terrain looks like there before its
    /// fine detail. Road profiles and settlement layouts are planned on it (with
    /// `T0::ground_at` as `h`), so they sit in the valleys the terrain carves.
    pub fn valley(&self, h: f64, x: f64, y: f64) -> f64 {
        if self.coarse.is_empty() {
            return h;
        }
        let mut v = h;
        // The chords listed in the point's cell (the only ones whose reach can touch it; the
        // least cut wins whatever the order); a chord whose box, grown by its reach (and a
        // foot), misses the point leaves it as it is.
        let (w, hh, cells) = &self.valley_index;
        if x < 0.0 || y < 0.0 {
            return v;
        }
        let (cx, cy) = ((x / VALLEY_INDEX_FT) as usize, (y / VALLEY_INDEX_FT) as usize);
        if cx >= *w || cy >= *hh {
            return v;
        }
        for &[a, b] in &cells[cy * w + cx] {
            let r = carve_reach(a[3].max(b[3]) as f64, VALLEY_SPACING_FT, 8.0) + 1.0;
            if x < a[0].min(b[0]) as f64 - r || x > a[0].max(b[0]) as f64 + r || y < a[1].min(b[1]) as f64 - r || y > a[1].max(b[1]) as f64 + r {
                continue;
            }
            v = v.min(valley_cut(a, b, h, x, y));
        }
        v
    }

    /// The coarse chords that can cut the ground anywhere in `rect` (x0, y0, x1, y1 ft), for
    /// `valley_with`: many points in one area ask the same few.
    pub fn valley_chords(&self, rect: [f64; 4]) -> Vec<([f32; 4], [f32; 4])> {
        let mut out = Vec::new();
        if self.coarse.is_empty() {
            return out;
        }
        let far = carve_reach(width_ft(1e9), VALLEY_SPACING_FT, 8.0);
        let mut seen = crate::core::hash::FastSet::default();
        for (ri, k) in self.segments_near(rect[0], rect[1], rect[2], rect[3], far) {
            if !seen.insert((ri, k)) {
                continue;
            }
            for c in self.coarse[ri as usize][k as usize].windows(2) {
                let (a, b) = (c[0], c[1]);
                let r = carve_reach(a[3].max(b[3]) as f64, VALLEY_SPACING_FT, 8.0) + 1.0;
                if a[0].max(b[0]) as f64 + r >= rect[0] && (a[0].min(b[0]) as f64) - r <= rect[2] && a[1].max(b[1]) as f64 + r >= rect[1] && (a[1].min(b[1]) as f64) - r <= rect[3] {
                    out.push((a, b));
                }
            }
        }
        out
    }

    /// `valley` at a point inside the rectangle `chords` were gathered for (`valley_chords`):
    /// the same cut (the least over the chords that reach the point).
    pub fn valley_with(chords: &[([f32; 4], [f32; 4])], h: f64, x: f64, y: f64) -> f64 {
        let mut v = h;
        for &(a, b) in chords {
            let r = carve_reach(a[3].max(b[3]) as f64, VALLEY_SPACING_FT, 8.0) + 1.0;
            if x < a[0].min(b[0]) as f64 - r || x > a[0].max(b[0]) as f64 + r || y < a[1].min(b[1]) as f64 - r || y > a[1].max(b[1]) as f64 + r {
                continue;
            }
            v = v.min(valley_cut(a, b, h, x, y));
        }
        v
    }

    /// `valley` over a grid of heights (row-major, `w` wide, point (i, j) at `origin` +
    /// (i, j) · `spacing` ft), in place: each chord cuts the points within its reach.
    pub fn valley_grid(&self, heights: &mut [f32], w: usize, origin: [f64; 2], spacing: f64) {
        let h = heights.len() / w.max(1);
        let ground = heights.to_vec();
        for chords in self.coarse.iter().flatten() {
            for c in chords.windows(2) {
                let (a, b) = (c[0], c[1]);
                let reach = carve_reach(a[3].max(b[3]) as f64, VALLEY_SPACING_FT, 8.0);
                let (x0, x1) = (a[0].min(b[0]) as f64 - reach, a[0].max(b[0]) as f64 + reach);
                let (y0, y1) = (a[1].min(b[1]) as f64 - reach, a[1].max(b[1]) as f64 + reach);
                let (i0, i1) = (((x0 - origin[0]) / spacing).ceil().max(0.0) as usize, (((x1 - origin[0]) / spacing).floor().max(-1.0) as i64).min(w as i64 - 1));
                let (j0, j1) = (((y0 - origin[1]) / spacing).ceil().max(0.0) as usize, (((y1 - origin[1]) / spacing).floor().max(-1.0) as i64).min(h as i64 - 1));
                if i1 < 0 || j1 < 0 {
                    continue;
                }
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let k = j * w + i;
                        let (x, y) = (origin[0] + i as f64 * spacing, origin[1] + j as f64 * spacing);
                        heights[k] = heights[k].min(valley_cut(a, b, ground[k] as f64, x, y) as f32);
                    }
                }
            }
        }
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

/// The ground `h` at (x, y) as one coarse chord a–b (x, y, water level, width) of a river
/// cuts it (`RiverNet::valley`).
fn valley_cut(a: [f32; 4], b: [f32; 4], h: f64, x: f64, y: f64) -> f64 {
    let (ax, ay, bx, by) = (a[0] as f64, a[1] as f64, b[0] as f64, b[1] as f64);
    let (dx, dy) = (bx - ax, by - ay);
    let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
    let (ex, ey) = (ax + dx * t - x, ay + dy * t - y);
    let d = crate::core::sqrt(ex * ex + ey * ey);
    let w = a[3] as f64 + (b[3] - a[3]) as f64 * t;
    let reach = carve_reach(w, VALLEY_SPACING_FT, 8.0);
    if d >= reach {
        return h;
    }
    let z = a[2] as f64 + (b[2] - a[2]) as f64 * t;
    let mut c = h.min(z + (d - 0.5 * w).max(0.0) * BANK_SLOPE);
    if d > VALLEY_EASE * reach {
        c += (h - c) * smoothstep(VALLEY_EASE * reach, reach, d);
    }
    c
}

/// River segments' finest curves sampled for `RiverNet::crossing_level` (bounding box and
/// points: x, y, water level, width), each once, as asked for.
#[derive(Default)]
pub struct CrossingCache(std::cell::RefCell<crate::core::hash::FastMap<(u32, u32), std::rc::Rc<([f64; 4], Vec<[f64; 4]>)>>>);

impl CrossingCache {
    fn get(&self, net: &RiverNet, ri: u32, k: u32, cell_ft: f64) -> std::rc::Rc<([f64; 4], Vec<[f64; 4]>)> {
        self.0
            .borrow_mut()
            .entry((ri, k))
            .or_insert_with(|| {
                let r = &net.rivers[ri as usize];
                let n = ((r.s[k as usize + 1] - r.s[k as usize]) / 150.0).ceil().clamp(4.0, 200.0) as usize;
                let pts: Vec<[f64; 4]> = (0..=n)
                    .map(|j| {
                        let c = r.eval(k as usize, j as f64 / n as f64, 2.5, cell_ft);
                        [c.p[0], c.p[1], c.z, c.w]
                    })
                    .collect();
                let bb = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - p[3]), b[1].min(p[1] - p[3]), b[2].max(p[0] + p[3]), b[3].max(p[1] + p[3])]);
                std::rc::Rc::new((bb, pts))
            })
            .clone()
    }
}

/// A sampled polyline piece of one river near a tile.
pub struct Piece {
    pub pts: Vec<CurvePoint>,
    /// Its river (`RiverNet::rivers`).
    pub river: u32,
    /// The falls its points' `fall` name (and, on a tile's first piece, every fall whose carving
    /// reaches the tile).
    pub falls: Vec<Fall>,
}

/// Sample every river curve near the rectangle at a resolution suited to `spacing`.
/// `pad(w)`: how far beyond the rectangle (ft) a river `w` ft wide still matters (its
/// carving reach), so wide rivers are sampled from farther out than creeks.
pub fn pieces(net: &RiverNet, rect: [f64; 4], pad: &dyn Fn(f64) -> f64, spacing: f64, cell_ft: f64) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    let seg_w = |r: &RiverCurve, k: usize| {
        let w = width_ft(r.q[k].max(r.q[k + 1]) as f64);
        if r.mouth && k + 1 == r.segments() { (w * (1.0 + MOUTH_FLARE)).min(width_ft(1e9)) } else { w }
    };
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
            // ~32 chords a mean loop (20 or more on the tightest), so banks draw round, not faceted.
            let step = (wavelength_ft(w) / 32.0).max(spacing).min(seg_len);
            // Within a step of the padded rectangle: a chord that cuts its corner keeps both ends.
            let pad = pad(w) + step;
            let inside = |p: [f64; 2]| p[0] >= rect[0] - pad && p[0] <= rect[2] + pad && p[1] >= rect[1] - pad && p[1] <= rect[3] + pad;
            let n = (seg_len / step).ceil().max(1.0) as usize;
            // A fall's lip and its face's foot are points of every level's line, so the water
            // drops between them, not over a chord.
            let mut ts: Vec<f64> = (0..=n).map(|j| j as f64 / n as f64).collect();
            for fa in r.falls.iter().filter(|fa| fa.seg as usize == k) {
                ts.retain(|&t| (t - fa.t).abs() > 1e-6 && (t - fa.t1).abs() > 1e-6 || t == 0.0 || t == 1.0);
                ts.extend([fa.t, fa.t1]);
                ts.sort_by(f64::total_cmp);
            }
            // Points surely outside (`RiverCurve::misses`) with both neighbours surely outside are
            // not sampled: no piece would keep them (a piece is the points inside and the one
            // before and after), and a long segment is nearly all such points by a small rectangle.
            let bx = [rect[0] - pad, rect[1] - pad, rect[2] + pad, rect[3] + pad];
            let swing = r.swing_ft(k, cell_ft);
            let far: Vec<bool> = ts.iter().map(|&t| t > 0.0 && t < 1.0 && r.misses(k, t, spacing, cell_ft, bx, swing)).collect();
            for (j, &t) in ts.iter().enumerate() {
                // A joint is the previous segment's last point.
                if j == 0 && k > k0 as usize {
                    continue;
                }
                if far[j] && far[j - 1] && far[j + 1] {
                    continue;
                }
                let cp = r.eval(k, t, spacing, cell_ft);
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
                    out.push(Piece { pts: std::mem::take(&mut cur), river: ri, falls: Vec::new() });
                }
                if !inside(cp.p) {
                    last_out = Some(cp);
                }
            }
        }
        if cur.len() >= 2 {
            out.push(Piece { pts: cur, river: ri, falls: Vec::new() });
        }
    }
    out.retain(|p| p.pts.len() >= 2);
    for p in &mut out {
        for c in &p.pts {
            if let Some(id) = c.fall
                && !p.falls.iter().any(|f| f.id == id)
            {
                p.falls.push(net.falls[id as usize]);
            }
        }
    }
    // What a fall carves (the bed kept above its lip, the cap on the water below, a cave's shelf
    // running on past the fall's own points: `carve`, `ledge`) reaches tiles its points may
    // not: every tile it reaches carves it alike, so its fall goes with the first piece of each.
    if let Some(first) = out.first_mut() {
        for f in &net.falls {
            // (A bound round the lip, never short of it: an extra fall carves nothing.)
            let p = pad(2.0 * f.width as f64) + fall_reach_ft(f) + 2.0 * spacing;
            if f.at[0] + p >= rect[0] && f.at[0] - p <= rect[2] && f.at[1] + p >= rect[1] && f.at[1] - p <= rect[3] && !first.falls.iter().any(|o| o.id == f.id) {
                first.falls.push(*f);
            }
        }
    }
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
/// - `near`: the nearest point of any centre line (distance, then level, width), from which
///   the levee is built: a crest just above the water across the levee band, then a back
///   slope (`LEVEE_BACK`) down to the ground, so the river is held where the land beside it
///   lies lower than its surface;
/// - `channel`: any channel covers the sample (channels beat banks).
///
/// `standing(x, y)` is the lake or sea surface at a world position (`DRY` on land). Banks
/// are levees: they exist only on land, never on the lake or sea floor a river runs out
/// into (there they would build a walled canal across the water).
#[allow(clippy::too_many_arguments)]
pub fn carve(pieces: &[Piece], heights: &mut [f32], water: &mut [f32], dim: usize, origin: [f64; 2], spacing: f64, reach_samples: f64, standing: &dyn Fn(f64, f64) -> f32) {
    let n = dim * dim;
    let mut cut = vec![f32::INFINITY; n];
    let mut near = vec![(f32::INFINITY, 0f32, 0f32); n];
    let mut channel = vec![false; n];
    // Above a lip the channel keeps its bed (coarser levels' pools, interpolated, sag into it),
    // and below it the water stands no higher than the fall has it (the channel above would
    // spread its level round its end, over the face and the pool).
    let mut lips: Vec<&Fall> = pieces.iter().flat_map(|p| &p.falls).collect();
    lips.sort_by_key(|f| f.id);
    lips.dedup_by_key(|f| f.id);
    let fall_cap = |x: f64, y: f64, z: f64| {
        let mut z = z;
        for f in &lips {
            let (rx, ry) = (x - f.at[0], y - f.at[1]);
            let (u, across) = (rx * f.dir[0] + ry * f.dir[1], (ry * f.dir[0] - rx * f.dir[1]).abs());
            let face = f.face_ft as f64;
            if u > 0.0 && u < face + f.width as f64 + 2.0 * spacing && across < 0.5 * f.width as f64 + 2.0 * spacing {
                let (top, bottom) = (f.top as f64, f.bottom as f64);
                z = z.min(if u < face { top + (bottom - top) * u / face } else { bottom });
            }
        }
        z
    };
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
            // Below a fall's face the pool's cut climbs as steeply as the face behind its foot, so
            // the lip stands (and the ground beside it: the headwall of the gorge below).
            let lip = a.fall.and_then(|id| piece.falls.iter().find(|f| f.id == id)).map(|f| (f.at, f.dir, f.face_ft as f64, f.drop_ft() as f64 / f.face_ft as f64));
            // Below a fall of this river the sides are carved from `fall_land`'s level, past a
            // strip by the water (none at the face's foot, widening below it, so the gorge's
            // rim rounds the pool) and walls up from it: the fall whose level stands highest.
            let held = {
                let mid = [0.5 * (a.p[0] + b.p[0]), 0.5 * (a.p[1] + b.p[1])];
                let zm = 0.5 * (a.z + b.z);
                let mut best: Option<(&Fall, LiftExtent, f64)> = None;
                for f in lips.iter().filter(|f| f.river == piece.river) {
                    let ext = lift_extent(f);
                    let rise = fall_land(f, &ext, mid, zm, fall_hold(f, &ext, mid)) - zm;
                    if rise > 0.01 && best.as_ref().is_none_or(|bst| rise > bst.2 || (rise == bst.2 && f.id < bst.0.id)) {
                        best = Some((f, ext, rise));
                    }
                }
                best.map(|(f, ext, _)| (fall_hold(f, &ext, a.p), fall_hold(f, &ext, b.p), f, ext))
            };
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
                    let mut target = if d < half {
                        let u = d / half;
                        z - depth_ft(w) * (1.0 - u * u)
                    } else {
                        let slope = if lip.is_some() { GORGE_SLOPE } else { BANK_SLOPE };
                        let mut side = z + (d - half) * slope;
                        if let Some((ha, hb, f, ext)) = &held {
                            let land = fall_land(f, ext, [px, py], z, ha + (hb - ha) * t);
                            if land > z {
                                let u = (a.p[0] + dx * t - f.at[0]) * f.dir[0] + (a.p[1] + dy * t - f.at[1]) * f.dir[1];
                                let face = f.face_ft as f64;
                                let strip = ext.strip * smoothstep(face, face + 2.0 * ext.strip, u);
                                let wall = z + (d - half - strip).max(0.0) * ext.wall;
                                side = side.max((land + (d - half) * slope).min(wall));
                            }
                        }
                        side
                    };
                    if let Some((at, dir, face, slope)) = lip
                        && d < half
                    {
                        let u = (px - at[0]) * dir[0] + (py - at[1]) * dir[1];
                        if u < face {
                            target += slope * (face - u);
                        }
                    }
                    let k = iy * dim + ix;
                    let h = heights[k] as f64;
                    let mut v = h.min(target);
                    if d > VALLEY_EASE * reach {
                        v += (h - v) * smoothstep(VALLEY_EASE * reach, reach, d);
                    }
                    cut[k] = cut[k].min(v as f32);
                    if d < half {
                        channel[k] = true;
                    }
                    let here = (d as f32, z as f32, w as f32);
                    if here.0 < near[k].0 || (here.0 == near[k].0 && (here.1, here.2) < (near[k].1, near[k].2)) {
                        near[k] = here;
                    }
                    if d < half + spacing && (z as f32) > water[k] {
                        let z = if lips.is_empty() { z } else { fall_cap(px, py, z) };
                        water[k] = water[k].max(z as f32);
                    }
                }
            }
        }
    }
    for k in 0..n {
        if cut[k] == f32::INFINITY {
            continue;
        }
        let mut v = cut[k] as f64;
        if channel[k] && !lips.is_empty() {
            let (x, y) = (origin[0] + (k % dim) as f64 * spacing, origin[1] + (k / dim) as f64 * spacing);
            // (The channel above the lip, told by its level, not a box along the lip's way: the
            // river bends there. Faded out by the distance from the lip, so no edge shows.)
            let (z, w) = (near[k].1 as f64, near[k].2 as f64);
            for f in &lips {
                let (rx, ry) = (x - f.at[0], y - f.at[1]);
                let keep = LIP_KEEP_FT + 2.0 * f.width as f64;
                let r2 = rx * rx + ry * ry;
                if z >= f.top as f64 - 0.5 && r2 < keep * keep && rx * f.dir[0] + ry * f.dir[1] <= 0.0 {
                    let fade = 1.0 - smoothstep(0.6 * keep, keep, crate::core::sqrt(r2));
                    v = v.max(v + (z - depth_ft(w) - v) * fade);
                }
            }
        }
        if !channel[k] && near[k].0 < f32::INFINITY {
            // Natural levee a foot or two high, sized to the river (not the level).
            let (d, z, w) = (near[k].0 as f64, near[k].1 as f64, near[k].2 as f64);
            let (half, band) = (0.5 * w, (0.35 * w).max(12.0));
            let floor = z + 0.5 + (d - half).clamp(0.0, band) * LEVEE_SLOPE - (d - half - band).max(0.0) * LEVEE_BACK;
            if floor > v {
                let (x, y) = (origin[0] + (k % dim) as f64 * spacing, origin[1] + (k / dim) as f64 * spacing);
                if standing(x, y) <= crate::t0::hydro::DRY {
                    let reach = carve_reach(w, spacing, reach_samples);
                    v += (1.0 - smoothstep(VALLEY_EASE * reach, reach, d)) * (floor - v);
                }
            }
        }
        heights[k] = v as f32;
    }
    if spacing <= LEDGE_MAX_SPACING_FT {
        for f in lips.iter().filter(|f| f.bank.is_some()) {
            ledge(pieces, f, heights, water, dim, origin, spacing);
        }
    }
}

/// Round a fall the valley sides below it (`carve`): the strip by the water left at the river's
/// levels (ft, plus widths, plus the shelf's reach), the gorge's walls past it (rise over run: the
/// face's own, within these bounds), how far below the lip the sides take to come down to the
/// pool's level (drops, at least `LIFT_RUN_MIN_FT`), and how far from the lip's line across the
/// river may stray and still be held (ft plus drops).
const LIFT_STRIP_FT: f64 = 15.0;
const LIFT_STRIP_WIDTHS: f64 = 0.25;
const LIFT_WALL_MIN: f64 = 6.0;
const LIFT_WALL_MAX: f64 = 40.0;
const LIFT_RUN_DROPS: f64 = 2.5;
const LIFT_RUN_MIN_FT: f64 = 150.0;
const LIFT_ACROSS_FT: f64 = 300.0;
const LIFT_ACROSS_DROPS: f64 = 1.0;
/// The land's drop below a fall starts as far below the lip as the point lies from the river,
/// rounded over this (ft, plus half the width) by the river.
const LIFT_ROUND_FT: f64 = 40.0;

/// `fall_land`'s extents (ft): the strip by the water, the walls' slope, the run below the lip
/// and how far across it holds.
struct LiftExtent {
    strip: f64,
    wall: f64,
    run: f64,
    across: f64,
}

fn lift_extent(f: &Fall) -> LiftExtent {
    let (face, w, drop) = (f.face_ft as f64, f.width as f64, f.drop_ft() as f64);
    let strip = LIFT_STRIP_FT + LIFT_STRIP_WIDTHS * w + LEDGE_REACH_FT;
    let wall = (drop / face.max(1.0)).clamp(LIFT_WALL_MIN, LIFT_WALL_MAX);
    LiftExtent { strip, wall, run: face + LIFT_RUN_MIN_FT.max(LIFT_RUN_DROPS * drop), across: 0.5 * w + LIFT_ACROSS_FT + LIFT_ACROSS_DROPS * drop }
}

/// The level (ft) the valley sides at `p` beside a fall's river (water at `z`, `hold` from
/// `fall_hold` at the river's nearest point) are carved from: below the lip not the river's own,
/// which drops at the face, but one coming down from the lip's to the pool's over the run (never
/// under the river's), starting farther down the farther from the river (the land's drop rounds
/// the gorge's head, no line across the valley), so the land beside the gorge runs on down past
/// the fall instead of stepping across the valley at its lip.
fn fall_land(f: &Fall, ext: &LiftExtent, p: [f64; 2], z: f64, hold: f64) -> f64 {
    let r = [p[0] - f.at[0], p[1] - f.at[1]];
    let u = r[0] * f.dir[0] + r[1] * f.dir[1];
    let a = f.dir[0] * r[1] - f.dir[1] * r[0];
    let round = LIFT_ROUND_FT + 0.5 * f.width as f64;
    let u = u - (crate::core::sqrt(a * a + round * round) - round);
    if hold <= 0.0 {
        return z;
    }
    let level = f.top as f64 - f.drop_ft() as f64 * smoothstep(0.0, ext.run, u.max(0.0));
    z + (level - z).max(0.0) * hold
}

/// How fully `fall_land` holds the sides beside the point `p` of a fall's river: all of it where
/// the river runs near the lip's line, none where it has strayed `across` from it.
fn fall_hold(f: &Fall, ext: &LiftExtent, p: [f64; 2]) -> f64 {
    let r = [p[0] - f.at[0], p[1] - f.at[1]];
    let a = (f.dir[0] * r[1] - f.dir[1] * r[0]).abs();
    1.0 - smoothstep(0.7 * ext.across, ext.across, a)
}

/// How far within the ground a fall reshapes (`fall_land`'s run below the lip, half the hold
/// across above it, the hold across) a point lies: 1 well inside, fading to 0 at its edge.
/// Refinement takes its roughness there from the smooth ground's slope, not the gorge's.
pub fn fall_calm(f: &Fall, p: [f64; 2]) -> f64 {
    let ext = lift_extent(f);
    let r = [p[0] - f.at[0], p[1] - f.at[1]];
    let u = (r[0] * f.dir[0] + r[1] * f.dir[1]) / if r[0] * f.dir[0] + r[1] * f.dir[1] >= 0.0 { ext.run } else { 0.5 * ext.across };
    let a = (f.dir[0] * r[1] - f.dir[1] * r[0]) / ext.across;
    1.0 - smoothstep(0.6, 1.0, crate::core::sqrt(u * u + a * a))
}

/// The falls whose `fall_calm` reaches into `rect` (x0, y0, x1, y1 ft).
pub fn falls_calm_in(net: &RiverNet, rect: [f64; 4]) -> Vec<Fall> {
    net.falls
        .iter()
        .filter(|f| {
            let ext = lift_extent(f);
            let r = ext.run.max(ext.across);
            f.at[0] + r >= rect[0] && f.at[0] - r <= rect[2] && f.at[1] + r >= rect[1] && f.at[1] - r <= rect[3]
        })
        .copied()
        .collect()
}

/// The rock shelf to a fall's cave: how far out from the water's edge it starts and how wide it
/// runs (ft), how far past the face's foot (plus the river's width), how high over the water,
/// and the finest spacing that carves it (coarser levels cannot hold it; finer ones carve
/// their own).
const LEDGE_FROM_FT: f64 = 1.0;
const LEDGE_WIDTH_FT: f64 = 11.0;
const LEDGE_RUN_FT: f64 = 60.0;
const LEDGE_RISE_FT: f64 = 2.0;
/// Steepest the shelf descends (rise over run) where the river runs on down steeply below the pool.
const LEDGE_SLOPE: f64 = 0.6;
const LEDGE_MAX_SPACING_FT: f64 = 5.0;
/// Beyond the shelf below the face, the ground keeps within this slope of it for this far (ft),
/// so a pit or a wall beside it is a ramp off it.
const LEDGE_SKIRT_FT: f64 = 15.0;
const LEDGE_SKIRT_SLOPE: f64 = 0.8;
/// Behind the falling water the shelf runs across the channel at the face's foot, from
/// `behind_back` (ft from the lip) to this far past the foot, and across from the bank on its side
/// to this far (ft) past the cave's mouth.
const BEHIND_PAST_FOOT_FT: f64 = 1.0;
const BEHIND_BEYOND_MOUTH_FT: f64 = 4.0;
/// The mouth: this fraction of the half width out from the middle of the channel toward the
/// shelf's bank (along the stream, `behind_mouth`).
const BEHIND_MOUTH_ACROSS: f64 = 0.25;

/// How far (ft from the lip) the shelf behind a fall's water reaches back into its face: about
/// 9 ft deep, cut into the lower part of the face (never its top fifth, so the lip stands).
#[inline]
fn behind_back(face: f64) -> f64 {
    (face - 9.0).max(0.2 * face)
}

/// Where a cave's mouth stands behind a fall of `face` and `width` (ft), the shelf's bank on
/// `side` (+1 to the stream's left): (along the stream from the lip, across it). Midway across the
/// shelf (its 5-ft square stays on it), in the cliff at its back as the map draws it.
pub fn behind_mouth(face: f64, width: f64, side: f64) -> (f64, f64) {
    (0.5 * (behind_back(face) + face + BEHIND_PAST_FOOT_FT), side * BEHIND_MOUTH_ACROSS * 0.5 * width)
}

/// How far (ft) from its lip anything a fall carves can reach (`carve`, `ledge`: the bed kept
/// above it, the cap on the water below, the drop eased over the land, the shelf and its skirt),
/// past the carving reach of the river beside it.
pub fn fall_reach_ft(f: &Fall) -> f64 {
    let (face, w) = (f.face_ft as f64, f.width as f64);
    let ledge = ledge_reach_ft(f);
    let lift = lift_extent(f);
    (LIP_KEEP_FT + 3.0 * w + face).max(lift.run.hypot(lift.across)).max(ledge)
}

/// How far out (ft) past the water's edge the shelf and its skirt run.
const LEDGE_REACH_FT: f64 = LEDGE_FROM_FT + LEDGE_WIDTH_FT + LEDGE_SKIRT_FT;

/// Where a fall's shelf and its skirt may run (`ledge`, `ledge_box`, `ledge_zone`): from `u0` to
/// `u1` ft along the stream from the lip, out to `reach` ft either side of its line.
fn ledge_span(f: &Fall) -> (f64, f64, f64) {
    let (face, half_w) = (f.face_ft as f64, 0.5 * f.width as f64);
    (behind_back(face).min(face - 3.0), face + 3.0 + LEDGE_RUN_FT + 2.0 * half_w, half_w * 1.5 + LEDGE_REACH_FT)
}

/// How far (ft) from its lip anything of a fall's shelf can lie (a bound on `ledge_span`): the
/// reaches that gather falls for tiles and battlemaps (`fall_reach_ft`, `zone_reach_ft`) are
/// built on it, so they cover the shelf whatever its constants.
fn ledge_reach_ft(f: &Fall) -> f64 {
    let (u0, u1, reach) = ledge_span(f);
    u1.max(-u0) + reach
}

/// The box (ft) round a fall's shelf and its skirt (`ledge`; none without one).
fn ledge_box(f: &Fall) -> Option<[f64; 4]> {
    f.bank?;
    let (u0, u1, reach) = ledge_span(f);
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for u in [u0, u1] {
        for v in [-reach, reach] {
            let p = [f.at[0] + f.dir[0] * u - f.dir[1] * v, f.at[1] + f.dir[1] * u + f.dir[0] * v];
            b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
        }
    }
    Some(b)
}

/// How far (ft) from its lip `sheet_zone` and `ledge_zone` reach (a bound).
fn zone_reach_ft(f: &Fall) -> f64 {
    // (`ledge_zone` widens the span by 2 ft; `sheet_zone` reaches face + 8 along, half + 6 across.)
    let (face, half_w) = (f.face_ft as f64, 0.5 * f.width as f64);
    (ledge_reach_ft(f) + 2.0).max((face + 8.0).hypot(half_w + 6.0))
}

/// Whether `p` (ft) lies on or by a fall's falling sheet as the map draws it (from just above the
/// lip to past the face's foot, out past its sides), over the shelf behind it: the battlemap's
/// scatter keeps off it.
pub fn sheet_zone(f: &Fall, p: [f64; 2]) -> bool {
    let (face, half_w) = (f.face_ft as f64, 0.5 * f.width as f64);
    let r = [p[0] - f.at[0], p[1] - f.at[1]];
    let u = r[0] * f.dir[0] + r[1] * f.dir[1];
    let v = f.dir[0] * r[1] - f.dir[1] * r[0];
    u >= -3.0 && u <= face + 8.0 && v.abs() <= half_w + 6.0
}

/// Whether `p` (ft) lies where a fall's shelf may run (`ledge`): from the back of the shelf behind
/// the water to the shelf's end, out to its skirt on either side. The battlemap keeps its
/// micro-relief out of there.
pub fn ledge_zone(f: &Fall, p: [f64; 2]) -> bool {
    let r = [p[0] - f.at[0], p[1] - f.at[1]];
    let u = r[0] * f.dir[0] + r[1] * f.dir[1];
    let v = f.dir[0] * r[1] - f.dir[1] * r[0];
    let (u0, u1, reach) = ledge_span(f);
    f.bank.is_some() && u >= u0 - 2.0 && u <= u1 && v.abs() <= reach
}

/// A walkable shelf of rock behind the falling water (across the channel at the face's foot,
/// cut back into the face, from the bank on its side to past the cave's mouth or the middle),
/// then on along that bank below the pool (a strip `LEDGE_WIDTH_FT` wide by the water's edge,
/// `LEDGE_RISE_FT` over the pool, then over the river below or easing down at `LEDGE_SLOPE`), so
/// the space behind the curtain, and a cave there, is always reached from the land downstream.
/// Every fall with a `bank` has one, so it tells nothing of a cave. Dry: the water beside it
/// stays below it. A pure function of position (the nearest point of the rivers' pieces near the
/// tile).
fn ledge(pieces: &[Piece], f: &Fall, heights: &mut [f32], water: &mut [f32], dim: usize, origin: [f64; 2], spacing: f64) {
    let Some(bank) = f.bank else { return };
    let (face, half_w) = (f.face_ft as f64, 0.5 * f.width as f64);
    let back = behind_back(face);
    let (u0, u1, reach) = ledge_span(f);
    let reach = reach + spacing;
    let Some(b) = ledge_box(f) else { return };
    let (lo, hi) = ([b[0] - spacing, b[1] - spacing], [b[2] + spacing, b[3] + spacing]);
    let i0 = (((lo[0] - origin[0]) / spacing).ceil().max(0.0) as usize).min(dim);
    let i1 = (((hi[0] - origin[0]) / spacing).floor() + 1.0).clamp(0.0, dim as f64) as usize;
    let j0 = (((lo[1] - origin[1]) / spacing).ceil().max(0.0) as usize).min(dim);
    let j1 = (((hi[1] - origin[1]) / spacing).floor() + 1.0).clamp(0.0, dim as f64) as usize;
    if i0 >= i1 || j0 >= j1 {
        return;
    }
    // The rivers' segments near it (the nearest point decides the side and the water's edge).
    let segs: Vec<(&CurvePoint, &CurvePoint)> = pieces
        .iter()
        .flat_map(|p| p.pts.windows(2).map(|s| (&s[0], &s[1])))
        .filter(|(a, b)| {
            let r = 0.5 * a.w.max(b.w) + reach;
            a.p[0].min(b.p[0]) - r <= hi[0] && a.p[0].max(b.p[0]) + r >= lo[0] && a.p[1].min(b.p[1]) - r <= hi[1] && a.p[1].max(b.p[1]) + r >= lo[1]
        })
        .collect();
    if segs.is_empty() {
        return;
    }
    // The nearest point of the rivers to (x, y), of those whose water's edge lies within the
    // shelf's reach (`LEDGE_REACH_FT`): (squared distance, side, level, width). Only those: every
    // tile round the point samples them (`pieces`' pad runs past that reach), so tiles agree.
    let nearest = |x: f64, y: f64| {
        let mut best = (f64::INFINITY, 0.0, 0.0, 0.0);
        for &(a, b) in &segs {
            let (dx, dy) = (b.p[0] - a.p[0], b.p[1] - a.p[1]);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            let t = (((x - a.p[0]) * dx + (y - a.p[1]) * dy) / len2).clamp(0.0, 1.0);
            let (qx, qy) = (a.p[0] + dx * t - x, a.p[1] + dy * t - y);
            let d2 = qx * qx + qy * qy;
            let edge = 0.5 * (a.w + (b.w - a.w) * t) + LEDGE_REACH_FT;
            if d2 < best.0 && d2 <= edge * edge {
                best = (d2, dx * (y - a.p[1]) - dy * (x - a.p[0]), a.z + (b.z - a.z) * t, a.w + (b.w - a.w) * t);
            }
        }
        best
    };
    // (The shelf's side of the river, as the river runs past its bank; and across the lip's
    // line, the same side.)
    let side = nearest(bank[0], bank[1]).1.signum();
    let vside = {
        let r = [bank[0] - f.at[0], bank[1] - f.at[1]];
        (f.dir[0] * r[1] - f.dir[1] * r[0]).signum()
    };
    // (Behind the water: from past the mouth, the far side of the middle, out to the bank's strip.)
    let (bv0, bv1) = (-BEHIND_MOUTH_ACROSS * half_w - BEHIND_BEYOND_MOUTH_FT, half_w + LEDGE_FROM_FT + LEDGE_WIDTH_FT);
    let bottom = f.bottom as f64;
    for j in j0..j1 {
        let y = origin[1] + j as f64 * spacing;
        for i in i0..i1 {
            let x = origin[0] + i as f64 * spacing;
            let r = [x - f.at[0], y - f.at[1]];
            let u = r[0] * f.dir[0] + r[1] * f.dir[1];
            if u < u0 || u > u1 {
                continue;
            }
            let k = j * dim + i;
            let v = (f.dir[0] * r[1] - f.dir[1] * r[0]) * vside;
            if u >= back && u <= face + BEHIND_PAST_FOOT_FT && v >= bv0 && v <= bv1 {
                let shelf = bottom + LEDGE_RISE_FT;
                heights[k] = shelf as f32;
                water[k] = water[k].min((shelf - 0.5) as f32);
                continue;
            }
            let best = nearest(x, y);
            if best.0 == f64::INFINITY {
                continue;
            }
            let (d, on, z, w) = (crate::core::sqrt(best.0), best.1, best.2, best.3);
            let out = d - 0.5 * w;
            let skirt = out - LEDGE_FROM_FT - LEDGE_WIDTH_FT;
            if on * side <= 0.0 || out < LEDGE_FROM_FT || skirt > LEDGE_SKIRT_FT || (skirt > 0.0 && u <= face + 3.0) {
                continue;
            }
            let shelf = (z.min(bottom) + LEDGE_RISE_FT).max(bottom + LEDGE_RISE_FT - LEDGE_SLOPE * (u - face - 3.0).max(0.0));
            if skirt > 0.0 {
                let give = LEDGE_SKIRT_SLOPE * skirt;
                heights[k] = (heights[k] as f64).clamp(shelf - give, shelf + give) as f32;
            } else {
                heights[k] = shelf as f32;
                water[k] = water[k].min((shelf - 0.5) as f32);
            }
        }
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

/// How far (ft) from its centre line a river `w` ft wide carves at a level of `spacing`.
pub fn carve_reach(w: f64, spacing: f64, reach_samples: f64) -> f64 {
    w * 0.5 + (reach_samples * spacing).max(3.0 * w)
}

/// Move a point (ft) out of any river channel and off its banks: pushed away from the
/// nearest point of the finest river curve until it is `margin` ft beyond the bank.
/// Where (x, y) is within `range` ft of a river's water: its surface there (the nearest point of
/// the finest curve, sampled every ~20 ft) and the channel's depth in its middle.
pub fn surface_near(net: &RiverNet, x: f64, y: f64, range: f64, cell_ft: f64) -> Option<(f64, f64)> {
    // (Out to the widest river's half width.)
    let pad = range + 600.0;
    let mut best: Option<(f64, f64, f64)> = None;
    for (ri, k) in net.segments_near(x - pad, y - pad, x + pad, y + pad, 0.0) {
        let (r, k) = (&net.rivers[ri as usize], k as usize);
        let n = ((r.s[k + 1] - r.s[k]) / 20.0).ceil().clamp(8.0, 2000.0) as usize;
        for j in 0..=n {
            let c = r.eval(k, j as f64 / n as f64, 2.5, cell_ft);
            let d = crate::core::sqrt((c.p[0] - x) * (c.p[0] - x) + (c.p[1] - y) * (c.p[1] - y)) - 0.5 * c.w;
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, c.z, depth_ft(c.w)));
            }
        }
    }
    best.filter(|b| b.0 <= range).map(|b| (b.1, b.2))
}

pub fn clear_of_rivers(net: &RiverNet, x: f64, y: f64, margin: f64, cell_ft: f64) -> (f64, f64) {
    clear_of_rivers_with(net, x, y, margin, cell_ft, &ClearCache::default())
}

/// Each river segment's finest curve every ~20 ft (point, width) for `clear_of_rivers_with`,
/// and how far those points stray from the segment's chord at most.
#[derive(Default)]
pub struct ClearCache(std::cell::RefCell<crate::core::hash::FastMap<(u32, u32), std::rc::Rc<(Vec<([f64; 2], f64)>, f64)>>>);

/// `clear_of_rivers` sharing the curve samples in `cache` (many points near the same rivers).
pub fn clear_of_rivers_with(net: &RiverNet, x: f64, y: f64, margin: f64, cell_ft: f64, cache: &ClearCache) -> (f64, f64) {
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
            let samples = cache
                .0
                .borrow_mut()
                .entry((ri, k))
                .or_insert_with(|| {
                    let n = ((r.s[k as usize + 1] - r.s[k as usize]) / 20.0).ceil().clamp(24.0, 2000.0) as usize;
                    let pts: Vec<([f64; 2], f64)> = (0..=n).map(|s| r.eval(k as usize, s as f64 / n as f64, 2.5, cell_ft)).map(|c| (c.p, c.w)).collect();
                    let stray = pts.iter().map(|&(q, _)| {
                        let t = (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
                        dist([a[0] + dx * t, a[1] + dy * t], q)
                    });
                    let stray = stray.fold(0.0, f64::max);
                    std::rc::Rc::new((pts, stray))
                })
                .clone();
            // None of this curve's points can beat the best so far (a foot to spare).
            if best.is_some_and(|b| chord - samples.1 - 1.0 > b.0) {
                continue;
            }
            for &(p, w) in &samples.0 {
                let d = dist(p, [px, py]);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, p, w));
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

