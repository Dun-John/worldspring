//! Hills raised by hand (Roadmap 7 item 172): a `Plateau` of kind `Hill` whose `hill` gives its
//! footprint (an ellipse, or a ridge's capsule along `rx`) and profile, and whose `level` is its
//! peak over the ground. Its height is added to the ground with the brushed change (so the two
//! together ease into `MAX_SHIFT_FT`): a pure function of world position and the hill (its noise
//! keyed by its own seed), smooth down to 0 at the footprint's edge. A knoll's and a mountain's
//! slopes gather into steeper bands (outcrops) between gentler benches, steep enough for the
//! battlemap's cliff faces.

use super::*;
use crate::core::noise::gradient2;

/// One hill as `Shaper` evaluates it.
pub(super) struct Near {
    profile: HillProfile,
    at: [f64; 2],
    rx: f64,
    ry: f64,
    crown: f64,
    peak: f64,
    cos: f64,
    sin: f64,
    seed: u64,
    /// Where the peak stands off the middle (in half-axes, 0.1 to 0.18: the middle stays high).
    off: [f64; 2],
    /// Two directions the slopes run out furthest in (spurs), unit vectors in half-axes.
    spurs: [[f64; 2]; 2],
    /// A ridge's steeper side (+1 or -1 across it) and where along it its crest is highest.
    side: f64,
    high: f64,
    pub(super) b: [f64; 4],
}

/// Outline points of a footprint (`Hill::problem` bounds `pts`).
const OUTLINE_PTS: usize = 32;

/// Its footprint's bounds (world ft): the rotated rectangle round its half-axes.
pub fn footprint_bounds(h: &Hill) -> [f64; 4] {
    let a = h.angle as f64 * (std::f64::consts::PI / 180.0);
    let (c, s) = (crate::core::fabs(libm::cos(a)), crate::core::fabs(libm::sin(a)));
    let (ex, ey) = (h.rx * c + h.ry * s, h.rx * s + h.ry * c);
    [h.at[0] - ex, h.at[1] - ey, h.at[0] + ex, h.at[1] + ey]
}

impl Near {
    pub(super) fn of(p: &Plateau) -> Option<Near> {
        let h = p.hill.as_ref().filter(|_| p.kind == PlateauKind::Hill)?;
        if !(h.rx.is_finite() && h.ry.is_finite() && h.rx >= 1.0 && h.ry >= 1.0 && p.level.is_finite() && h.at[0].is_finite() && h.at[1].is_finite() && h.angle.is_finite()) {
            return None;
        }
        let a = h.angle as f64 * (std::f64::consts::PI / 180.0);
        let narrow = if h.profile == HillProfile::Ridge { h.ry } else { h.rx.min(h.ry) };
        let seed = crate::core::rng::hash2(0x4811_1ce5 ^ h.seed as u64, h.seed as i64, 0x172);
        let pick = |k: i64| crate::core::rng::unit(crate::core::rng::hash2(seed, k, 0x3a));
        let turn = |t: f64| [libm::cos(t * 2.0 * std::f64::consts::PI), libm::sin(t * 2.0 * std::f64::consts::PI)];
        let (d0, d1) = (pick(1), pick(2));
        let lean = turn(pick(3));
        let reach = 0.1 + 0.08 * pick(4);
        Some(Near {
            profile: h.profile,
            at: h.at,
            rx: h.rx,
            ry: h.ry,
            crown: ((h.crown.max(0.0) as f64) / narrow).min(0.8),
            peak: (p.level as f64).clamp(-(MAX_HILL_FT as f64), MAX_HILL_FT as f64),
            cos: libm::cos(a),
            sin: libm::sin(a),
            seed,
            // A crowned hill (a castle's) keeps its crown in the middle.
            off: if h.crown > 0.0 { [0.0, 0.0] } else { [lean[0] * reach, lean[1] * reach] },
            spurs: [turn(d0), turn(d0 + 0.3 + 0.4 * d1)],
            side: if pick(5) < 0.5 { -1.0 } else { 1.0 },
            high: 0.5 * (pick(6) * 2.0 - 1.0),
            b: footprint_bounds(h),
        })
    }

    /// Its height (ft over the ground) at (x, y).
    pub(super) fn height(&self, x: f64, y: f64) -> f64 {
        if x <= self.b[0] || x >= self.b[2] || y <= self.b[1] || y >= self.b[3] {
            return 0.0;
        }
        let (dx, dy) = (x - self.at[0], y - self.at[1]);
        let (u, v) = (dx * self.cos + dy * self.sin, -dx * self.sin + dy * self.cos);
        let narrow = if self.profile == HillProfile::Ridge { self.ry } else { self.rx.min(self.ry) };
        // Noise lengths from the hill's own size: a few wobbles round it.
        let k = 1.0 / (0.55 * narrow).max(12.0);
        let (a, b) = (u / self.rx, v / self.ry);
        let wob = 1.0 + 0.06 * (0.5 + 0.5 * gradient2(self.seed, u * k, v * k));
        if self.profile == HillProfile::Ridge {
            return self.ridge(a, b, u, v, k, wob);
        }
        let r0 = crate::core::sqrt(a * a + b * b);
        if r0 * wob >= 1.0 {
            return 0.0;
        }
        // An irregular outline: full out along the two spurs, drawn in and wobbled between them
        // (inward only, so never past the footprint).
        let (da, db) = if r0 > 1e-9 { (a / r0, b / r0) } else { (1.0, 0.0) };
        let lobe = |d: [f64; 2]| {
            let c = (da * d[0] + db * d[1]).max(0.0);
            let c2 = c * c;
            c2 * c2 * c2
        };
        let spur = lobe(self.spurs[0]).max(lobe(self.spurs[1]));
        let m = if self.crown > 0.0 {
            1.0 - 0.05 * (0.5 + 0.5 * gradient2(self.seed ^ 0x31, da * 1.6, db * 1.6))
        } else {
            0.92 + 0.08 * spur - 0.05 * (0.5 + 0.5 * gradient2(self.seed ^ 0x31, da * 1.6 + 2.3, db * 1.6)) - 0.03 * (0.5 + 0.5 * gradient2(self.seed ^ 0x32, da * 3.1 + 7.3, db * 3.1))
        };
        let (sa, sb) = (a * wob / m.clamp(0.6, 1.0), b * wob / m.clamp(0.6, 1.0));
        let rs2 = sa * sa + sb * sb;
        if rs2 >= 1.0 {
            return 0.0;
        }
        // The peak stands off the middle; the outline stays where it is.
        let pull = 1.0 - rs2;
        let (pa, pb) = (sa - self.off[0] * pull, sb - self.off[1] * pull);
        let r = crate::core::sqrt(pa * pa + pb * pb).min(1.0);
        let (ea, eb) = if r > 1e-9 { (pa / r, pb / r) } else { (1.0, 0.0) };
        let t = ((r - self.crown) / (1.0 - self.crown)).clamp(0.0, 1.0);
        let dome = 1.0 - smoothstep(0.0, 1.0, t);
        // Outcrops on one or two arcs of the slope, broken along their length.
        let seed = self.seed;
        let arc = smoothstep(-0.05, 0.3, gradient2(seed ^ 0x0d, ea * 1.2 + 3.1, eb * 1.2 - 1.7));
        let mask = |i: f64| arc * smoothstep(-0.12, 0.18, gradient2(seed ^ 0x0e, ea * 3.6 + i * 5.3, eb * 3.6 - i * 2.9));
        let mut f = match self.profile {
            HillProfile::Knoll => self.outcrops(dome, 3.0, 0.2, u, v, k, mask),
            HillProfile::Mountain => {
                let concave = (1.0 - t) * (1.0 - t);
                self.outcrops(0.6 * concave + 0.4 * dome, 4.0, 0.14, u, v, k, mask)
            }
            _ => dome,
        };
        // Low swells on the slopes (none at the crown or the foot), and the spurs standing out of them.
        f += (0.07 * crate::core::noise::fbm(self.seed ^ 0x51, u * 2.0 * k, v * 2.0 * k, 3, 2.0, 0.5) + if self.crown > 0.0 { 0.0 } else { 0.12 * spur }) * 4.0 * t * (1.0 - t);
        self.peak * f.max(0.0)
    }

    /// A ridge at (`a`, `b`) in its half-axes: a crest running along it that swings to its steeper
    /// side and bends, rising to a high point off its middle with saddles either side, its ends
    /// tapered and rounded, a few broken outcrops down the steep side.
    fn ridge(&self, a: f64, b: f64, u: f64, v: f64, k: f64, wob: f64) -> f64 {
        // Inside both a tapered superellipse and the footprint's capsule.
        let taper = 1.0 - 0.2 * a * a;
        let bb = b / taper;
        let a4 = a * a * a * a;
        let du = (crate::core::fabs(u) - (self.rx - self.ry).max(0.0)).max(0.0);
        let cap = (du * du + v * v) / (self.ry * self.ry);
        let rs2 = (a4 + bb * bb).max(cap) * wob * wob;
        if rs2 >= 1.0 {
            return 0.0;
        }
        // (How far it swings varies along it, so its steep side is crag in places, a slope between.)
        let lean = 0.12 + 0.22 * (0.5 + 0.5 * gradient2(self.seed ^ 0x2f, a * 2.6, 0.29));
        let shift = (self.side * lean + 0.1 * gradient2(self.seed ^ 0x2e, a * 1.4, 0.61)) * (1.0 - rs2);
        let bc = bb - shift;
        let r = crate::core::sqrt(a4 + bc * bc).min(1.0);
        let t = ((r - self.crown) / (1.0 - self.crown)).clamp(0.0, 1.0);
        // (Eased at the crest and the foot, straight between: a gentler steepest grade than a
        // smoothstep's, so only where it swings hard does its steep side reach a cliff's.)
        const C: f64 = 0.2;
        let dome = 1.0 - if t < C { t * t / (2.0 * C * (1.0 - C)) } else if t > 1.0 - C { 1.0 - (1.0 - t) * (1.0 - t) / (2.0 * C * (1.0 - C)) } else { (t - 0.5 * C) / (1.0 - C) };
        let crest = 1.0 - 0.28 * smoothstep(0.0, 1.2, crate::core::fabs(a - self.high)) - 0.16 * (0.5 + 0.5 * gradient2(self.seed ^ 0x2d, a * 3.2, 0.37)) + 0.08;
        let seed = self.seed;
        let steep = smoothstep(0.02, 0.3, self.side * bc);
        let mask = |i: f64| steep * smoothstep(-0.1, 0.22, gradient2(seed ^ 0x0e, a * 4.5 + i * 3.7, 0.8 + i * 1.3));
        let mut f = self.outcrops(dome * crest, 2.0, 0.17, u, v, k, mask);
        f += 0.06 * crate::core::noise::fbm(self.seed ^ 0x51, u * 2.0 * k, v * 2.0 * k, 3, 2.0, 0.5) * 4.0 * t * (1.0 - t);
        self.peak * f.max(0.0)
    }

    /// `base` (1 at the crown, 0 at the foot) gathered into `bands` benches with steep risers
    /// between (each riser `w` of a band wide), the bands warped by noise; `mask(band)` (0..1)
    /// keeps the risers only where it is up, so outcrops stand on parts of the slope, not in rings.
    /// Eased off at the foot and the crown.
    #[allow(clippy::too_many_arguments)]
    fn outcrops(&self, base: f64, bands: f64, w: f64, u: f64, v: f64, k: f64, mask: impl Fn(f64) -> f64) -> f64 {
        let warp = 0.45 * gradient2(self.seed ^ 0x0c, u * 1.7 * k, v * 1.7 * k);
        let g = base * bands + warp;
        let i = crate::core::floor(g);
        let stepped = (i + smoothstep(0.5 - w, 0.5 + w, g - i) - warp) / bands;
        let keep = smoothstep(0.0, 0.18, base) * (1.0 - smoothstep(0.65, 0.9, base));
        base + (stepped - base) * keep * mask(i)
    }
}

/// A hill (`Plateau` of kind `Hill`) at `at` (world ft) with half-axes `rx` along `angle` (deg)
/// and `ry` across, rising `peak_ft` over the ground, its noise seeded from `key`.
pub fn hill_for(key: &str, profile: HillProfile, at: [f64; 2], rx: f64, ry: f64, angle: f64, peak_ft: f64, crown: f64) -> Result<Plateau, String> {
    let r = |v: f64| crate::core::round(v * 2.0) * 0.5;
    let (rx, ry) = (r(rx), r(ry));
    let angle = if angle.is_finite() { (crate::core::round(angle * 10.0) * 0.1).rem_euclid(360.0) } else { 0.0 };
    let hill = Hill { profile, at: [r(at[0]), r(at[1])], rx, ry, angle: angle as f32, crown: r(crown.max(0.0)) as f32, seed: crate::core::hash::fnv64(key.as_bytes()) as u32 };
    let a = angle * (std::f64::consts::PI / 180.0);
    let (c, s) = (libm::cos(a), libm::sin(a));
    let half = (rx - ry).max(0.0);
    let pts = (0..OUTLINE_PTS)
        .map(|i| {
            let th = i as f64 * (2.0 * std::f64::consts::PI / OUTLINE_PTS as f64);
            let (cu, sv) = (libm::cos(th), libm::sin(th));
            let (u, v) = if profile == HillProfile::Ridge { (if cu >= 0.0 { half } else { -half } + ry * cu, ry * sv) } else { (rx * cu, ry * sv) };
            [r(hill.at[0] + u * c - v * s), r(hill.at[1] + u * s + v * c)]
        })
        .collect();
    let p = Plateau { pts, level: (crate::core::round(peak_ft * 10.0) * 0.1) as f32, edge: 0.0, kind: PlateauKind::Hill, floor: None, hill: Some(hill) };
    match p.problem() {
        Some(e) => Err(e),
        None => Ok(p),
    }
}

/// A round hill to raise under a castle drawn as `poly` (world ft; the app's `castleHill`): at
/// its points' mean, a flat crown a little wider than the castle, a peak of 60 to 120 ft and
/// slopes about three times as long as it is high.
pub fn castle_hill(key: &str, poly: &[[f64; 2]]) -> Result<Plateau, String> {
    if poly.len() < 3 {
        return Err("the castle has no outline".into());
    }
    let n = poly.len() as f64;
    let at = [poly.iter().map(|p| p[0]).sum::<f64>() / n, poly.iter().map(|p| p[1]).sum::<f64>() / n];
    let reach = poly.iter().map(|p| crate::core::sqrt((p[0] - at[0]) * (p[0] - at[0]) + (p[1] - at[1]) * (p[1] - at[1]))).fold(0.0, f64::max);
    let crown = reach * 1.15 + 10.0;
    let peak = (50.0 + 0.4 * crown).clamp(60.0, 120.0);
    let r = (crown + 3.0 * peak).min(*HILL_RADIUS_FT.end());
    hill_for(key, HillProfile::Round, at, r, r, 0.0, peak, crown.min(0.8 * r))
}
