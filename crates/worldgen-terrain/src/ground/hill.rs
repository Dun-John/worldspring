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
        Some(Near {
            profile: h.profile,
            at: h.at,
            rx: h.rx,
            ry: h.ry,
            crown: ((h.crown.max(0.0) as f64) / narrow).min(0.8),
            peak: (p.level as f64).clamp(-(MAX_HILL_FT as f64), MAX_HILL_FT as f64),
            cos: libm::cos(a),
            sin: libm::sin(a),
            seed: crate::core::rng::hash2(0x4811_1ce5 ^ h.seed as u64, h.seed as i64, 0x172),
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
        let r0 = match self.profile {
            HillProfile::Ridge => {
                let du = (crate::core::fabs(u) - (self.rx - self.ry).max(0.0)).max(0.0);
                crate::core::sqrt(du * du + v * v) / self.ry
            }
            _ => crate::core::sqrt((u / self.rx) * (u / self.rx) + (v / self.ry) * (v / self.ry)),
        };
        // The footprint's edge wobbles inward only (never past the outline).
        let r = r0 * (1.0 + 0.09 * (0.5 + 0.5 * gradient2(self.seed, u * k, v * k)));
        if r >= 1.0 {
            return 0.0;
        }
        let t = ((r - self.crown) / (1.0 - self.crown)).clamp(0.0, 1.0);
        let dome = 1.0 - smoothstep(0.0, 1.0, t);
        let mut f = match self.profile {
            HillProfile::Round | HillProfile::Ridge => dome,
            HillProfile::Knoll => self.outcrops(dome, 3.0, 0.2, u, v, k),
            HillProfile::Mountain => {
                let concave = (1.0 - t) * (1.0 - t);
                self.outcrops(0.6 * concave + 0.4 * dome, 4.0, 0.14, u, v, k)
            }
        };
        // Low swells on the slopes (none at the crown or the foot).
        f += 0.07 * crate::core::noise::fbm(self.seed ^ 0x51, u * 2.0 * k, v * 2.0 * k, 3, 2.0, 0.5) * 4.0 * t * (1.0 - t);
        if self.profile == HillProfile::Ridge {
            // The crest rises and dips along its length.
            f *= 1.0 - 0.12 * (0.5 + 0.5 * gradient2(self.seed ^ 0x2d, u * 1.5 / self.rx, 0.37));
        }
        self.peak * f.max(0.0)
    }

    /// `base` (1 at the crown, 0 at the foot) gathered into `bands` benches with steep risers
    /// between (each riser `w` of a band wide), the bands warped by noise so they break into crags
    /// rather than rings; eased off at the foot and the crown.
    fn outcrops(&self, base: f64, bands: f64, w: f64, u: f64, v: f64, k: f64) -> f64 {
        let warp = 0.45 * gradient2(self.seed ^ 0x0c, u * 1.7 * k, v * 1.7 * k);
        let g = base * bands + warp;
        let i = crate::core::floor(g);
        let stepped = (i + smoothstep(0.5 - w, 0.5 + w, g - i) - warp) / bands;
        let keep = smoothstep(0.0, 0.18, base) * (1.0 - smoothstep(0.65, 0.9, base));
        base + (stepped - base) * keep
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
