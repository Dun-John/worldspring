//! How far a settlement reaches: its built-up radius, where roads stop short of it, its
//! layout's reach and the terrain pad round it.

use crate::t0::settle::{Settlement, Tier};

/// Built-up radius (ft) by tier and population.
pub fn urban_radius(tier: Tier, population: u32) -> f64 {
    let p = crate::core::sqrt(population as f64);
    match tier {
        Tier::Village => 150.0 + 12.0 * p,
        Tier::Town => 11.0 * p,
        Tier::City | Tier::Metropolis => 12.0 * p,
    }
}

/// Where network roads stop short of a town (ft from its centre): outside the walls and
/// most of the sprawl; the town draws the last stretch to its gate.
pub fn road_trim_radius(tier: Tier, population: u32) -> f64 {
    1.5 * urban_radius(tier, population)
}

/// How far a settlement's layout (fields included) can reach from its center.
pub fn reach(s: &Settlement) -> f64 {
    let r = urban_radius(s.tier, s.population);
    if s.tier == Tier::Village { r * 2.2 } else { r * (OUTSKIRTS + 0.15) }
}

/// How far (ft) from its centre a settlement's pad reaches: the terrain eases from the
/// ground its layout was planned on (all of the layout's reach) back to the land round it
/// over a broad band, so whatever stands between them (fine detail, a river valley the
/// land outside lacks) is a gentle slope rather than a rim.
pub fn pad_extent(reach: f64) -> f64 {
    reach + (0.6 * reach).max(900.0)
}

/// The settlement pad's weight at `d` ft from its centre: 1 over the layout, 0 past
/// `pad_extent`.
pub fn pad_weight(d: f64, reach: f64) -> f64 {
    1.0 - crate::core::noise::smoothstep(reach, pad_extent(reach), d)
}

/// Outskirts (farmland) reach this many urban radii from the centre.
pub const OUTSKIRTS: f64 = 2.15;

/// How far (ft) from its centre a place stands in a settlement: its buildings and districts, the
/// sprawl outside the walls included. Measured over seeds 1–3 in built-up radii: villages'
/// buildings reach 1.36; cities' and metropolises' districts 2.12 and 1.90, towns' 2.05 for 95
/// in 100 (a few river towns' run on past their reach).
pub fn holds_radius(s: &Settlement) -> f64 {
    urban_radius(s.tier, s.population) * if s.tier == Tier::Village { 1.4 } else { OUTSKIRTS }
}
