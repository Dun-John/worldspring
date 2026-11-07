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

/// Tallest a crag stands over flat ground (ft): three tiers, its face a cliff.
const CRAG_FT: f64 = 15.0;
/// How far a crag runs in behind its gate, and its half-width (ft).
const CRAG_LONG: f64 = 75.0;
const CRAG_HALF: f64 = 36.0;

/// A rock crag raised behind a dwarven hall's gate: its face half a square behind the opening,
/// square to the passage, running back and down into the ground round it.
pub struct Crag {
    at: P,
    dir: P,
    amp: f64,
    phase: f64,
}

impl Crag {
    /// The crag behind entrance `e` when the ground there doesn't already rise into a face (None
    /// on a hillside). A pure function of the entrance and the T0 ground: every chunk agrees.
    pub fn new(t0: &T0, e: &Entrance) -> Option<Crag> {
        let behind = add(e.at, mul(e.dir, 40.0));
        let rise = t0.sample(behind[0], behind[1], 5.0) - t0.sample(e.at[0], e.at[1], 5.0);
        let amp = (CRAG_FT - rise.max(0.0)).min(CRAG_FT);
        let phase = unit(hash2(0x6372_6167, crate::core::round(e.at[0]) as i64, crate::core::round(e.at[1]) as i64)) * std::f64::consts::TAU;
        (amp >= 6.0).then_some(Crag { at: e.at, dir: e.dir, amp, phase })
    }

    /// World bounding box (x0, y0, x1, y1).
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        let r = CRAG_LONG.hypot(CRAG_HALF * 1.25);
        (self.at[0] - r, self.at[1] - r, self.at[0] + r, self.at[1] + r)
    }

    /// Height (ft) the crag adds at `p`: nothing in front of the face; behind it a sheer rise of
    /// about one square, a lumpy top, the sides and the back sloping away.
    pub fn lift(&self, p: P) -> f64 {
        let d = [p[0] - self.at[0], p[1] - self.at[1]];
        let s = d[0] * self.dir[0] + d[1] * self.dir[1];
        let t = -d[0] * self.dir[1] + d[1] * self.dir[0];
        if s <= 2.5 {
            return 0.0;
        }
        let face = smoothstep(2.5, 6.5, s);
        // The width swells and pinches along its length; the back is rounded off.
        let half = CRAG_HALF * (1.0 + 0.18 * libm::sin(s / 17.0 + self.phase));
        let back = (s - 0.45 * CRAG_LONG).max(0.0) / (0.55 * CRAG_LONG);
        let q = crate::core::sqrt(back * back + (t / half) * (t / half));
        let body = 1.0 - smoothstep(0.4, 1.0, q);
        if body <= 0.0 {
            return 0.0;
        }
        let lumps = 1.0 + 0.12 * libm::sin(s / 9.0 + 2.0 * self.phase) * libm::cos(t / 11.0 - self.phase);
        self.amp * face * body * lumps
    }
}
