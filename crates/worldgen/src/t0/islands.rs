//! Land masses: the mainland (the largest) and the islands, labelled once by flood fill over the
//! land (lakes count as land: they lie inside it), and the settlements each island is owed
//! (`owed`), placed first in their tier's pass (`settle::place`) as pins are.

use super::biome::Biome;
use super::flood::neighbors;
use super::hydro::NO_LAKE;
use crate::core::rng::{hash3, unit};

/// No land mass (sea).
pub const NONE: u32 = u32::MAX;

/// Under this (sq mi), or at least `BARREN_SHARE` barren, an island has no settlements.
pub const TINY_MI2: f64 = 10.0;
pub const BARREN_SHARE: f64 = 0.6;
/// Up to this a small island has at most one fishing village, on about `SMALL_SETTLED` of them.
pub const SMALL_MI2: f64 = 60.0;
pub const SMALL_SETTLED: f64 = 2.0 / 3.0;
/// Up to this an island has 1 village and one more every `VILLAGE_STEP_MI2` above `SMALL_MI2`
/// (at most `MID_VILLAGES`), and a port town from `TOWN_MI2`.
pub const MID_MI2: f64 = 400.0;
pub const VILLAGE_STEP_MI2: f64 = 150.0;
pub const MID_VILLAGES: usize = 3;
pub const TOWN_MI2: f64 = 150.0;

pub struct Island {
    /// Its lowest cell index: what its own draws are keyed by.
    pub first: usize,
    /// Dry land (not lake), sq mi.
    pub area_mi2: f64,
    /// The share of that land too barren to live on (`barren`).
    pub barren: f64,
}

pub struct Islands {
    /// Per cell: its land mass, or `NONE` at sea.
    pub of: Vec<u32>,
    pub parts: Vec<Island>,
    /// The largest land mass.
    pub mainland: u32,
}

/// Ground no one settles: ice, bare mountain, hot desert, lava, salt flats and ash fields.
pub fn barren(b: Biome) -> bool {
    matches!(b, Biome::Ice | Biome::Alpine | Biome::HotDesert | Biome::Volcanic | Biome::SaltFlat | Biome::Ashland)
}

/// Label the land masses (8 neighbours, as the economy labels the seas), numbered by their
/// lowest cell.
pub fn label(w: usize, h: usize, cell_ft: f64, land: &[bool], lake_of: &[u32], biome: &[u32]) -> Islands {
    let n = w * h;
    let cell_mi2 = (cell_ft / 5280.0) * (cell_ft / 5280.0);
    let mut of = vec![NONE; n];
    let mut parts = Vec::new();
    let mut stack = Vec::new();
    for k0 in 0..n {
        if !land[k0] || of[k0] != NONE {
            continue;
        }
        let id = parts.len() as u32;
        of[k0] = id;
        stack.push(k0);
        let (mut dry, mut bare) = (0usize, 0usize);
        while let Some(k) = stack.pop() {
            if lake_of[k] == NO_LAKE {
                dry += 1;
                bare += barren(Biome::from_u8((biome[k] & 0xff) as u8)) as usize;
            }
            for (nb, _) in neighbors(w, h, k) {
                if land[nb] && of[nb] == NONE {
                    of[nb] = id;
                    stack.push(nb);
                }
            }
        }
        parts.push(Island { first: k0, area_mi2: dry as f64 * cell_mi2, barren: bare as f64 / dry.max(1) as f64 });
    }
    let mainland = (0..parts.len()).max_by(|&a, &b| parts[a].area_mi2.total_cmp(&parts[b].area_mi2).then(b.cmp(&a))).map_or(NONE, |i| i as u32);
    Islands { of, parts, mainland }
}

/// What an island is owed: villages, towns, and whether its first village must stand on the
/// shore and its first town be a port.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Owed {
    pub villages: usize,
    pub towns: usize,
    pub shore_village: bool,
    pub port_town: bool,
}

impl Islands {
    /// The island (not the mainland) cell `k` stands on.
    pub fn island_at(&self, k: usize) -> Option<u32> {
        let c = self.of[k];
        (c != NONE && c != self.mainland).then_some(c)
    }

    /// What island `c` is owed, at the world's villages and towns per sq mi (`density`); `seed`
    /// keys a small island's draw (with its lowest cell).
    pub fn owed(&self, c: u32, seed: u64, density: (f64, f64)) -> Owed {
        let Island { first, area_mi2: a, barren } = self.parts[c as usize];
        if c == self.mainland || a < TINY_MI2 || barren >= BARREN_SHARE {
            return Owed::default();
        }
        if a < SMALL_MI2 {
            let settled = unit(hash3(seed, first as i64, 0, 0)) < SMALL_SETTLED;
            return Owed { villages: settled as usize, towns: 0, shore_village: true, port_town: false };
        }
        let (villages, towns) = if a < MID_MI2 {
            ((1 + ((a - SMALL_MI2) / VILLAGE_STEP_MI2) as usize).min(MID_VILLAGES), (a >= TOWN_MI2) as usize)
        } else {
            ((crate::core::round(a * density.0) as usize).max(3), (crate::core::round(a * density.1) as usize).max(1))
        };
        Owed { villages, towns, shore_village: false, port_town: towns > 0 }
    }
}
