//! Hills raised by hand (Roadmap 7 item 172): a `Plateau` of kind `Hill` whose `hill` gives its
//! footprint (an ellipse, or a ridge's capsule along `rx`) and profile, and whose `level` is its
//! peak over the ground. Its height is added to the ground on top of the brushed change (that
//! eased into `MAX_SHIFT_FT`, the hills not): a pure function of world position and the hill (its noise
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
    /// How much finer detail it has than a small hill's (0 up to `DETAIL_FT` across its
    /// narrower half-axis, toward 1 for one miles across): its outline, spurs and outcrops.
    detail: f64,
    /// Fbm octaves for its swells (more on a big one).
    octaves: u32,
    pub(super) b: [f64; 4],
}

/// The narrower half-axis (ft) up to which a hill has a small hill's detail.
const DETAIL_FT: f64 = 640.0;

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
            peak: (p.level as f64).clamp(-(MAX_PEAK_FT as f64), MAX_PEAK_FT as f64),
            detail: 1.0 - DETAIL_FT / narrow.max(DETAIL_FT),
            octaves: 3 + (libm::log2(narrow.max(DETAIL_FT) / DETAIL_FT) as u32).min(3),
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
        let d = self.detail;
        let mut wob = 1.0 + 0.06 * (0.5 + 0.5 * gradient2(self.seed, u * k, v * k));
        // (A big one's outline ragged finer too, still inward only.)
        if d > 0.0 {
            wob += d * (0.05 * (0.5 + 0.5 * gradient2(self.seed ^ 0x71, u * 4.0 * k, v * 4.0 * k)) + 0.025 * (0.5 + 0.5 * gradient2(self.seed ^ 0x72, u * 11.0 * k, v * 11.0 * k)));
        }
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
        let mut spur = lobe(self.spurs[0]).max(lobe(self.spurs[1]));
        // (A big one has lesser spurs all round, and a more broken outline between them.)
        let mut m_fine = 0.0;
        if d > 0.0 && self.crown <= 0.0 {
            // (Joined smoothly, not by max: on a big one a kink in the outline's scale shows as a
            // crease running up to the peak; and the finer terms only out toward the rim, so they
            // don't pinch into a star round the peak.)
            let rim = smoothstep(0.35, 0.95, r0);
            let (l0, l1) = (lobe(self.spurs[0]), lobe(self.spurs[1]));
            let more = 0.8 * d * rim * smoothstep(0.15, 0.6, gradient2(self.seed ^ 0x34, da * 3.4 + 4.1, db * 3.4 - 0.7));
            spur = 1.0 - (1.0 - l0) * (1.0 - l1) * (1.0 - more);
            m_fine = rim * d * (0.05 * (0.5 + 0.5 * gradient2(self.seed ^ 0x33, da * 6.3 + 1.1, db * 6.3)) + 0.03 * (0.5 + 0.5 * gradient2(self.seed ^ 0x35, da * 13.0 - 2.9, db * 13.0)));
        }
        let m = if self.crown > 0.0 {
            1.0 - 0.05 * (0.5 + 0.5 * gradient2(self.seed ^ 0x31, da * 1.6, db * 1.6))
        } else {
            0.92 + 0.08 * spur - 0.05 * (0.5 + 0.5 * gradient2(self.seed ^ 0x31, da * 1.6 + 2.3, db * 1.6)) - 0.03 * (0.5 + 0.5 * gradient2(self.seed ^ 0x32, da * 3.1 + 7.3, db * 3.1)) - m_fine
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
        // (A big one's outcrops broken into more, shorter pieces round it.)
        let fq = 3.6 * (1.0 + 1.5 * d);
        let arc = if d > 0.0 { arc.max(d * smoothstep(-0.25, 0.2, gradient2(seed ^ 0x1d, ea * 2.7 - 0.4, eb * 2.7 + 5.2))) } else { arc };
        let mask = |i: f64| arc * smoothstep(-0.12, 0.18, gradient2(seed ^ 0x0e, ea * fq + i * 5.3, eb * fq - i * 2.9));
        let mut f = match self.profile {
            HillProfile::Knoll => self.outcrops(dome, self.bands(3.0, 45.0), self.riser(0.2), u, v, k, mask),
            HillProfile::Mountain => {
                let concave = (1.0 - t) * (1.0 - t);
                self.outcrops(0.6 * concave + 0.4 * dome, self.bands(4.0, 75.0), self.riser(0.14), u, v, k, mask)
            }
            _ => dome,
        };
        // Low swells on the slopes (none at the crown or the foot), and the spurs standing out of them.
        f += (0.07 * crate::core::noise::fbm(self.seed ^ 0x51, u * 2.0 * k, v * 2.0 * k, self.octaves, 2.0, 0.5) + if self.crown > 0.0 { 0.0 } else { 0.12 * spur }) * 4.0 * t * (1.0 - t);
        self.peak * f.max(0.0)
    }

    /// A ridge at (`a`, `b`) in its half-axes: a rounded crest that wanders across it and leans to
    /// its steeper side, rising and falling along it (a summit, a lower one, saddles between) and
    /// easing down at its ends; a width that changes along it, with a shoulder or two running out
    /// to the side; a ragged outline; outcrops only on the steep side of its high parts.
    fn ridge(&self, a: f64, b: f64, u: f64, v: f64, k: f64, wob: f64) -> f64 {
        // How far across it may reach here (in half-widths): the footprint's capsule.
        let du = (crate::core::fabs(u) - (self.rx - self.ry).max(0.0)).max(0.0) / self.ry;
        if du >= 1.0 || crate::core::fabs(b) >= 1.0 {
            return 0.0;
        }
        let lim = crate::core::sqrt(1.0 - du * du);
        let seed = self.seed;
        let n = |s: u64, x: f64, y: f64| gradient2(seed ^ s, x, y);
        let bump = |c: f64, w: f64| 1.0 - smoothstep(0.0, w, crate::core::fabs(a - c));
        // Shoulders: one or two spots along it where a side runs out further, a spur off the crest.
        let pick = |i: i64| crate::core::rng::unit(crate::core::rng::hash2(seed, i, 0x5b));
        let spur_at = [-0.55 + 0.5 * pick(1), 0.05 + 0.5 * pick(2)];
        let spur_side = [if pick(3) < 0.5 { -1.0 } else { 1.0 }, if pick(4) < 0.5 { -1.0 } else { 1.0 }];
        let spur = [bump(spur_at[0], 0.24), bump(spur_at[1], 0.16) * (0.4 + 0.6 * pick(5))];
        // Its two edges (half-widths out from its axis), each ragged on its own.
        // (A long one wobbles at its own width's scale along it too: `u / ry`.)
        let (d, ua) = (self.detail, u / self.ry);
        let width = 0.8 + 0.14 * n(0x2b, a * 2.1, 0.9) + 0.04 * n(0x2a, a * 6.1, 2.3) + if d > 0.0 { d * (0.08 * n(0x3a, ua * 0.9, 1.3) + 0.04 * n(0x3c, ua * 2.3, 0.2)) } else { 0.0 };
        let edge = |s: f64, salt: u64| {
            let out = (0..2).map(|i| if spur_side[i] == s { 0.5 * spur[i] } else { 0.0 }).sum::<f64>();
            (width + out + 0.1 * n(salt, a * 4.7, 3.1) + 0.03 * n(salt ^ 0x9, a * 11.0, 0.4)).min(lim).max(0.0)
        };
        let (ep, en) = (edge(1.0, 0x27), edge(-1.0, 0x28));
        if b >= ep || b <= -en {
            return 0.0;
        }
        // The crest: leaning to the steep side (more where it is high), wandering across it.
        let high2 = if self.high > 0.0 { self.high - 0.75 } else { self.high + 0.75 };
        let lift = 0.4 + 0.52 * bump(self.high, 0.5) + 0.34 * bump(high2, 0.32) + 0.08 * n(0x2d, a * 3.2, 0.37) + if d > 0.0 { d * 0.12 * n(0x3b, ua * 0.6, 0.4) } else { 0.0 };
        let ends = 1.0 - smoothstep(0.45, 1.0, crate::core::fabs(a));
        let lift = lift.clamp(0.0, 1.0) * ends;
        let lean = self.side * (0.06 + 0.2 * smoothstep(0.5, 1.0, lift / ends.max(1e-9)));
        let wander = 0.28 * n(0x2e, a * 1.3, 0.61) + 0.08 * n(0x2f, a * 3.4, 1.7);
        let gap = 0.3 * (ep + en);
        let bc = (lean + wander).clamp(-en + gap, ep - gap);
        // Across: from the crest out to the edge on that side, the ragged outline drawn inward.
        let run = if b > bc { ep - bc } else { bc + en };
        let near_spur = (0..2).map(|i| if spur_side[i] * (b - bc) > 0.0 { spur[i] } else { 0.0 }).fold(0.0, f64::max);
        let rag = 1.0 + 0.16 * (0.5 + 0.5 * n(0x26, u * 1.6 * k, v * 1.6 * k)) + (wob - 1.0);
        let t = (crate::core::fabs(b - bc) / run.max(1e-9) * rag).min(1.0);
        if t >= 1.0 {
            return 0.0;
        }
        // (A crown, if it has one, flat along the crest.)
        let t = ((t - self.crown) / (1.0 - self.crown)).max(0.0);
        // (Rounded at the crest, spread at the foot, straight between; a shoulder holds up a spur.)
        const C1: f64 = 0.22;
        const C2: f64 = 0.26;
        let m = 1.0 / (1.0 - 0.5 * C1 - 0.5 * C2);
        let fall = if t < C1 { m * t * t / (2.0 * C1) } else if t > 1.0 - C2 { 1.0 - m * (1.0 - t) * (1.0 - t) / (2.0 * C2) } else { m * (t - 0.5 * C1) };
        let dome = (1.0 - fall) * (1.0 + 0.35 * near_spur * t * (1.0 - t));
        let steep = smoothstep(0.02, 0.15, self.side * (b - bc) / run.max(1e-9));
        let mask = |i: f64| steep * smoothstep(0.6, 0.85, lift) * smoothstep(0.0, 0.3, n(0x0e, a * 4.5 + d * ua * 1.2 + i * 3.7, 0.8 + i * 1.3));
        let mut f = self.outcrops(dome * lift, self.bands(2.0, 80.0), self.riser(0.17), u, v, k, mask);
        f += 0.06 * crate::core::noise::fbm(self.seed ^ 0x51, u * 2.0 * k, v * 2.0 * k, self.octaves, 2.0, 0.5) * 4.0 * t * (1.0 - t) * ends;
        self.peak * f.max(0.0)
    }

    /// `base` (1 at the crown, 0 at the foot) gathered into `bands` benches with steep risers
    /// between (each riser `w` of a band wide), the bands warped by noise; `mask(band)` (0..1)
    /// keeps the risers only where it is up, so outcrops stand on parts of the slope, not in rings.
    /// Eased off at the foot and the crown.
    #[allow(clippy::too_many_arguments)]
    fn outcrops(&self, base: f64, bands: f64, w: f64, u: f64, v: f64, k: f64, mask: impl Fn(f64) -> f64) -> f64 {
        let mut warp = 0.45 * gradient2(self.seed ^ 0x0c, u * 1.7 * k, v * 1.7 * k);
        if self.detail > 0.0 {
            warp += self.detail * 0.3 * gradient2(self.seed ^ 0x0f, u * 6.0 * k, v * 6.0 * k);
        }
        let g = base * bands + warp;
        let i = crate::core::floor(g);
        let stepped = (i + smoothstep(0.5 - w, 0.5 + w, g - i) - warp) / bands;
        let keep = smoothstep(0.0, 0.18, base) * (1.0 - smoothstep(0.65, 0.9, base));
        base + (stepped - base) * keep * mask(i)
    }

    /// Its outcrop bands: `least`, or one for each `step` ft of its peak (a big one has more).
    fn bands(&self, least: f64, step: f64) -> f64 {
        least.max(crate::core::floor(self.peak / step))
    }

    /// Its risers' width (of a band, at most `widest`): narrow enough on a big, gentle one to
    /// stay crags (a riser's grade is about peak / (2 w narrow); a big one's a little steeper).
    fn riser(&self, widest: f64) -> f64 {
        let narrow = if self.profile == HillProfile::Ridge { self.ry } else { self.rx.min(self.ry) };
        widest.min(crate::core::fabs(self.peak) / ((3.0 + self.detail) * narrow)).max(0.02)
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
    let r = (crown + 3.0 * peak).min(640.0);
    hill_for(key, HillProfile::Round, at, r, r, 0.0, peak, crown.min(0.8 * r))
}
