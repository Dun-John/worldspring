//! Distances the world file's checks share with the generators.

use crate::core::tile::TILE_N;

/// A ferry's jetties (ft), from the ends of its line back to its banks.
pub const JETTY_FT: f64 = 24.0;

/// How far a POI layout reaches from its point (ft).
pub const SITE_REACH_FT: f64 = 260.0;

pub const SQ: usize = TILE_N / 2;

pub const SQUARE_FT: f64 = 5.0;

/// Bridge decks sit this far above the water they span.
pub const DECK_CLEARANCE_FT: f32 = 4.0;
