//! A wild site's way down as seen from above (v63 underground): the entrance its theme is drawn
//! with, and the crag a dwarven hall's gate is cut into where the ground is flat.

use super::{Theme, UnderKind, resolve};
use crate::World;
use crate::core::noise::smoothstep;
use crate::core::rng::{Pcg32, hash2, hash3, unit};
use crate::t0::T0;
use crate::town::{
    self, Entrance, Layout,
    geom::{P, add, mul},
};

// v63 underground: entrance styles (`size` = kind + 16 × style; 0-3 were taken before).
/// A barrow mound with a stone doorway (tomb, crypt, ossuary, catacombs).
pub const STYLE_BARROW: u8 = 4;
/// A carved rune gate cut into a rock face (dwarven hall).
pub const STYLE_RUNE_GATE: u8 = 5;
/// A ruined tower's stump over the stair (wizard's lair).
pub const STYLE_TOWER_STUMP: u8 = 6;

/// The theme the site behind entrance `k` of layout `layout` comes to, as `generate` resolves it
/// (the same rng and its first draws; a temple's sinking left out). `ruin`: no roof stands there.
pub fn theme_at(world: &World, t0: &T0, layout: usize, k: usize, kind: UnderKind, ruin: bool) -> &'static Theme {
    let mut rng = Pcg32::new(hash3(world.stream("under"), town::rng_key(t0, layout), k as i64, kind as i64), 91);
    let spec = layout
        .checked_sub(t0.settlements.len())
        .and_then(|p| t0.created_site(p))
        .map(|c| c.spec)
        .unwrap_or_default();
    resolve(kind, spec, ruin, &mut rng).2
}

/// The style a wild site's way down is drawn in, by its theme (None: as before, by kind).
pub fn entrance_style(world: &World, t0: &T0, l: &Layout, e: &Entrance) -> Option<u8> {
    if (l.index as usize) < t0.settlements.len() || !matches!(e.kind, UnderKind::Dungeon | UnderKind::Crypt | UnderKind::Catacombs) {
        return None;
    }
    let ruin = l.buildings.iter().all(|b| b.structure == town::Structure::Ruin);
    let t = theme_at(world, t0, l.index as usize, e.id as usize, e.kind, ruin);
    match t.key {
        "dwarven_hall" => Some(STYLE_RUNE_GATE),
        "wizard_lair" => Some(STYLE_TOWER_STUMP),
        _ if matches!(t.kind, UnderKind::Crypt | UnderKind::Catacombs) => Some(STYLE_BARROW),
        _ => None,
    }
}

/// The ground a barrow's mound covers (kept clear of trees and relief): an oval from just before
/// the doorway to 5 squares in, 4 across.
pub fn barrow_ground(e: &Entrance) -> Vec<P> {
    let sq = crate::battlemap::SQUARE_FT;
    let side = [-e.dir[1], e.dir[0]];
    (0..16)
        .map(|k| {
            let t = k as f64 / 16.0 * std::f64::consts::TAU;
            add(e.at, add(mul(e.dir, (2.4 + 3.4 * libm::cos(t)) * sq), mul(side, 2.4 * libm::sin(t) * sq)))
        })
        .collect()
}

/// Tallest a crag's face stands over flat ground (ft).
const CRAG_FT: f64 = 11.0;
/// The crag is a crescent round a point this far in front of the gate (ft); its face runs round
/// that point this far out (so it stands 3 ft behind the opening, square to the passage).
const CRAG_FRONT: f64 = 9.0;
const CRAG_FACE_R: f64 = 12.0;
/// The face's run from foot to top (ft): sheer, so it is inked as a cliff.
const CRAG_FACE_RUN: f64 = 4.0;
/// Where the level top along the face's rim gives way to the back's slope (ft in from the face).
const CRAG_TOP: f64 = 11.0;
/// How far the rock runs back from its face before it meets the ground (ft), behind the gate
/// and less at its horns.
const CRAG_BACK: f64 = 36.0;
const CRAG_BACK_HORN: f64 = 22.0;
/// Ground past the crag's back (ft) where the micro-relief eases back in.
const CRAG_APRON: f64 = 15.0;
/// Farthest the crag (and its apron) reaches from its centre (ft), outline noise included.
const CRAG_REACH: f64 = CRAG_FACE_R + CRAG_BACK + CRAG_APRON + 6.0;

/// A small rock outcrop raised behind a dwarven hall's gate: a crescent wrapping behind and
/// round the sides of the gate, its inner face a cliff (3 ft behind the opening), its back and
/// horns sloping smoothly down into the ground round it. Its outline is warped by noise of
/// the world position.
pub struct Crag {
    at: P,
    dir: P,
    amp: f64,
    ph: [f64; 2],
    seed: u64,
}

impl Crag {
    /// The crag behind entrance `e` when the ground there doesn't already rise into a face (None
    /// on a hillside). A pure function of the entrance and the T0 ground: every chunk agrees.
    pub fn new(t0: &T0, e: &Entrance) -> Option<Crag> {
        let behind = add(e.at, mul(e.dir, 40.0));
        let rise = t0.sample(behind[0], behind[1], 5.0) - t0.sample(e.at[0], e.at[1], 5.0);
        let amp = (CRAG_FT - 0.6 * rise.max(0.0)).min(CRAG_FT);
        let seed = hash2(0x6372_6167, crate::core::round(e.at[0]) as i64, crate::core::round(e.at[1]) as i64);
        let ph = [unit(seed) * std::f64::consts::TAU, unit(hash2(seed, 1, 2)) * std::f64::consts::TAU];
        (rise < 9.0).then_some(Crag { at: e.at, dir: e.dir, amp, ph, seed })
    }

    /// World bounding box (x0, y0, x1, y1).
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        let c = add(self.at, mul(self.dir, -CRAG_FRONT));
        (c[0] - CRAG_REACH, c[1] - CRAG_REACH, c[0] + CRAG_REACH, c[1] + CRAG_REACH)
    }

    /// Where `p` lies on the crag: its distance (ft) in from the face (negative in front of it),
    /// the rock's depth there (face to back, ft) and the face's height (ft); None off the crag.
    fn place(&self, p: P) -> Option<(f64, f64, f64)> {
        let c = add(self.at, mul(self.dir, -CRAG_FRONT));
        let d = [p[0] - c[0], p[1] - c[1]];
        let s = d[0] * self.dir[0] + d[1] * self.dir[1];
        let t = -d[0] * self.dir[1] + d[1] * self.dir[0];
        let r = crate::core::sqrt(s * s + t * t);
        if r > CRAG_REACH {
            return None;
        }
        // Angle off the passage's line: 0 behind the gate, the horns towards ±2 rad.
        let th = libm::atan2(t, s).abs();
        let hgt = self.amp * (1.0 - smoothstep(0.4, 2.0, th));
        if hgt <= 0.0 {
            return None;
        }
        // The face is true behind the gate and wanders round the horns; the back's outline
        // wanders everywhere (noise of the world position, so it is not a drawn curve).
        let side = smoothstep(0.3, 0.8, th);
        let sth = if t < 0.0 { -th } else { th };
        let wander = 1.4 * libm::sin(2.0 * sth + self.ph[0]) + 0.8 * libm::sin(5.0 * sth + self.ph[1]);
        let warp = crate::core::noise::gradient2(self.seed, p[0] / 14.0, p[1] / 14.0);
        let u = r - CRAG_FACE_R - side * (wander + 2.0 * warp);
        let depth = CRAG_BACK - (CRAG_BACK - CRAG_BACK_HORN) * smoothstep(0.3, 1.8, th) + 2.5 * crate::core::noise::gradient2(self.seed ^ 0x5a5a, p[0] / 18.0, p[1] / 18.0);
        Some((u, depth, hgt))
    }

    /// Height (ft) the crag adds at `p`: nothing in front of the face; a sheer face, a level rim,
    /// then the top sloping away to the ground at the back (under 3 ft a square: a slope).
    pub fn lift(&self, p: P) -> f64 {
        let Some((u, depth, hgt)) = self.place(p) else { return 0.0 };
        if u <= 0.0 || u >= depth {
            return 0.0;
        }
        let face = smoothstep(0.0, CRAG_FACE_RUN, u);
        let back = 1.0 - ramp((u - CRAG_TOP) / (depth - CRAG_TOP));
        hgt * face * back
    }

    /// How far (0-1) the ground's micro-relief is smoothed away at `p`: wholly on the crag
    /// behind its face, easing back in over an apron past its back and towards its horns.
    pub fn apron(&self, p: P) -> f64 {
        self.place(p).map_or(0.0, |(u, depth, hgt)| smoothstep(0.0, 3.0, hgt) * smoothstep(-3.0, 0.0, u) * (1.0 - smoothstep(depth, depth + CRAG_APRON, u)))
    }

    /// Whether `p` is on the crag's face or the thin rim along its top (drawn as rock; the top
    /// behind keeps the ground's own cover).
    pub fn rock(&self, p: P) -> bool {
        self.place(p).is_some_and(|(u, _, hgt)| hgt >= 5.0 && (-1.0..=CRAG_FACE_RUN + 3.5).contains(&u))
    }

    /// Whether `p` is by the crag's face (drawn with its ink and rock shading; the rest of the
    /// crag is shaded soft). The two meet on the level rim and on the horns where the face is low,
    /// so the change of shading never shows.
    pub fn by_face(&self, p: P) -> bool {
        self.place(p).is_some_and(|(u, _, hgt)| hgt >= 1.5 && (-6.0..=CRAG_TOP - 3.0).contains(&u))
    }
}

/// 0 → 1 over `v` in [0, 1]: straight through the middle (slope 4/3), eased at both ends.
fn ramp(v: f64) -> f64 {
    const K: f64 = 0.25;
    let v = v.clamp(0.0, 1.0);
    if v < K {
        v * v / (2.0 * K * (1.0 - K))
    } else if v > 1.0 - K {
        1.0 - (1.0 - v) * (1.0 - v) / (2.0 * K * (1.0 - K))
    } else {
        (v - 0.5 * K) / (1.0 - K)
    }
}
