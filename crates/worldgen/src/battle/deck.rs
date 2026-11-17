//! Hand bridges' decks: their levels over the water and the ground, end to end.

use super::*;

/// A bridge's deck level (ft) between `a` and `b` (a hand bridge's, R5 M20 a lined bridge's):
/// `DECK_CLEARANCE_FT` over the highest water under it (rivers, lakes, the sea), else level with
/// the lower end. A pure function of the world, so every chunk and the plan find the same.
pub fn deck_top(a: [f64; 2], b: [f64; 2], t0: &T0) -> f32 {
    deck_water(a, b, t0).unwrap_or_else(|| t0.ground_at(a[0], a[1], 5.0).min(t0.ground_at(b[0], b[1], 5.0)) as f32)
}

/// The least level (ft) a deck from `a` to `b` may have over the water under it: `DECK_CLEARANCE_FT` over the highest
/// water (rivers, lakes, the sea); `None` where it crosses no water.
fn deck_water(a: [f64; 2], b: [f64; 2], t0: &T0) -> Option<f32> {
    let river = t0.rivers.crossing_level(a, b, 0.0, t0.cell_ft, &crate::lod::rivers::CrossingCache::default());
    let len = crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]));
    let n = (len / 10.0).ceil().max(1.0) as usize;
    let at = |k: usize| [a[0] + (b[0] - a[0]) * k as f64 / n as f64, a[1] + (b[1] - a[1]) * k as f64 / n as f64];
    let still = (0..=n).map(|k| at(k)).map(|p| t0.sample_water(p[0], p[1])).fold(crate::t0::hydro::DRY, f32::max);
    let water = river.map_or(still, |(z, _)| still.max(z as f32));
    (water > crate::t0::hydro::DRY).then_some(water + DECK_CLEARANCE_FT)
}

/// The steepest a hand bridge's deck climbs (ft per ft of its length): 1 in 8.
pub const DECK_GRADE_MAX: f64 = 0.125;

/// A hand bridge's deck (`hand_deck_ends`): its level (ft) at its two ends, `a` then `b`, reached at `span` (0 to 1 along
/// it from `a`): an end on a plateau is at the plateau's level from where the deck meets its edge on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deck {
    pub ends: (f32, f32),
    pub span: (f64, f64),
}

impl Deck {
    /// The deck level (ft) `t` (0 to 1) of the way along it from `a`.
    pub fn level(&self, t: f64) -> f32 {
        let (s0, s1) = self.span;
        deck_level(self.ends, if s1 - s0 > 1e-9 { (t - s0) / (s1 - s0) } else { 0.5 })
    }
}

/// A hand bridge's deck (P6 6b): it climbs straight from one end to the other, each end at the ground there (the world's
/// ground as edited: a plateau's top, an islet's) or clear of the water under the deck, whichever is higher; an end on a
/// plateau (or islet) drawn by hand keeps that level from the plateau's edge in, so the deck meets the top at the edge.
/// The climb is capped at `DECK_GRADE_MAX` from the lower end, so a long bridge that meets a cliff runs into its face
/// rather than rising steeply. A pure function of the world.
pub fn hand_deck_ends(c: &crate::world::Crossing, world: &World, t0: &T0) -> Deck {
    hand_deck(c, world, t0).0
}

/// How far (ft) a hand bridge's deck ends short of the higher end's level, where the 1 in 8 cap holds it down (0 if not).
pub fn hand_deck_short(c: &crate::world::Crossing, world: &World, t0: &T0) -> f32 {
    hand_deck(c, world, t0).1
}

/// How far (0 to 1 of the way from `end` to `other`) the deck first leaves a plateau drawn by hand that `end` stands on
/// at `level` (ft): the nearest crossing of the outline of each such plateau (one as high as the end, within 1 ft).
fn plateau_edge(end: [f64; 2], other: [f64; 2], level: f32, world: &World) -> Option<f64> {
    let d = [other[0] - end[0], other[1] - end[1]];
    let mut best: Option<f64> = None;
    for p in world.file.edits.plateaus.values().filter(|p| p.level >= level - 1.0 && p.level <= level + 1.0 && crate::town::geom::contains(&p.pts, end)) {
        for (i, &q0) in p.pts.iter().enumerate() {
            let q1 = p.pts[(i + 1) % p.pts.len()];
            let e = [q1[0] - q0[0], q1[1] - q0[1]];
            let den = d[0] * e[1] - d[1] * e[0];
            if den.abs() < 1e-12 {
                continue;
            }
            let w = [q0[0] - end[0], q0[1] - end[1]];
            let (s, u) = ((w[0] * e[1] - w[1] * e[0]) / den, (w[0] * d[1] - w[1] * d[0]) / den);
            if s > 0.0 && s < 1.0 && (0.0..=1.0).contains(&u) && best.is_none_or(|b| s < b) {
                best = Some(s);
            }
        }
    }
    best
}

fn hand_deck(c: &crate::world::Crossing, world: &World, t0: &T0) -> (Deck, f32) {
    let rect = [c.a[0].min(c.b[0]) - 20.0, c.a[1].min(c.b[1]) - 20.0, c.a[0].max(c.b[0]) + 20.0, c.a[1].max(c.b[1]) + 20.0];
    let shaper = crate::ground::Shaper::build(&world.file.edits, Some(t0), rect, SQUARE_FT);
    let ground = |p: [f64; 2]| {
        let g = t0.ground_at(p[0], p[1], 5.0) as f32;
        shaper.as_ref().map_or(g, |s| s.at_point(p[0], p[1], g, t0.sample_water(p[0], p[1])))
    };
    let base = deck_water(c.a, c.b, t0).unwrap_or(f32::MIN);
    let (ga, gb) = (ground(c.a), ground(c.b));
    let (la, lb) = (ga.max(base), gb.max(base));
    // (An end on a plateau above the water's clearance: the deck reaches its level where it meets the plateau's edge.)
    let sa = if ga >= base { plateau_edge(c.a, c.b, ga, world).unwrap_or(0.0) } else { 0.0 };
    let sb = if gb >= base { 1.0 - plateau_edge(c.b, c.a, gb, world).unwrap_or(0.0) } else { 1.0 };
    let span = if sb > sa { (sa, sb) } else { (0.0, 1.0) };
    let cap = (DECK_GRADE_MAX * c.length() * (span.1 - span.0)) as f32;
    // (With no water under it and ends level, the old flat deck at the lower end: both ends the same.)
    let (ends, short) = if la > lb + cap {
        ((lb + cap, lb), la - lb - cap)
    } else if lb > la + cap {
        ((la, la + cap), lb - la - cap)
    } else {
        ((la, lb), 0.0)
    };
    (Deck { ends, span }, short)
}

/// The deck level (ft) `t` (0 to 1) of the way along a deck with ends `(a, b)`.
pub fn deck_level(ends: (f32, f32), t: f64) -> f32 {
    ends.0 + (ends.1 - ends.0) * t.clamp(0.0, 1.0) as f32
}

/// A hand bridge's deck level at its middle (`hand_deck_ends`).
pub fn hand_deck_top(c: &crate::world::Crossing, world: &World, t0: &T0) -> f32 {
    hand_deck_ends(c, world, t0).level(0.5)
}
