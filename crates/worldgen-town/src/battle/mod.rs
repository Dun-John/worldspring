//! The battlemap's kinds and chunk data, and its pieces the settlements lay out with (`battlemap`
//! re-exports them): the object catalog, ground codes, decks and gateways.

use serde::Serialize;

use crate::World;
use crate::core::tile::TileKey;
use crate::t0::T0;
use crate::t0::biome::Biome;
pub use crate::model::consts::{SQ, SQUARE_FT, DECK_CLEARANCE_FT};

mod kinds;
mod chunk;
mod deck;
mod kitchen;
pub use kinds::*;
pub use chunk::*;
pub use deck::*;

/// Squares per edge including a one-square halo ring (for seamless rendering gradients).
pub const HS: usize = SQ + 2;

pub const TIER_FT: f64 = 5.0;

pub const MAX_COVER_GAP: i32 = 6;

/// In a castle's yard, whose curtains, towers and buildings are its cover: no dry square of it more
/// than this from cover, and a window of it (most of its squares) holding any of them needs no more
/// (`cover_gap`, `works_cover`).
pub const YARD_COVER_GAP: i32 = 12;

/// On a field (v63 countryside), where its hedgerows and walls are the cover (`town::rural::rows`
/// split a field at most about 110 ft apart): how far a square may be from cover.
pub const FIELD_COVER_GAP: i32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Surface {
    Grass = 0,
    ForestFloor,
    DryGrass,
    Sand,
    Snow,
    Rock,
    Mud,
    Ash,
    Salt,
    Dirt,
    Shallow,
    Deep,
    Lava,
    Ice,
    /// King's road paving.
    Cobble,
    /// Packed-earth road or track.
    Road,
    /// Bridge deck.
    Planks,
    /// A building (its roof; interiors come with M5).
    Roof,
    /// Cropland.
    Field,
    /// A flight of steps on a steep paved street (cities and up).
    Stairs,
    /// A park's gravel paths.
    Gravel,
}

impl Surface {
    #[inline]
    pub fn is_road(self) -> bool {
        matches!(self, Surface::Cobble | Surface::Road | Surface::Planks | Surface::Stairs)
    }
}

/// Settlement ground under a square (`Chunk::urban`).
pub const URBAN_STREET: u8 = 1;

pub const URBAN_FIELD: u8 = 2;

pub const URBAN_PLAZA: u8 = 3;

pub const URBAN_GRAVEYARD: u8 = 4;

pub const URBAN_RUIN: u8 = 5;

/// Bridge deck or pier.
pub const URBAN_DECK: u8 = 6;

/// Town wall (a raised stone wall-walk) and towers.
pub const URBAN_WALL: u8 = 7;

pub const URBAN_TOWER: u8 = 8;

/// Ground at the mouth of a way underground (kept clear of plants and props).
pub const URBAN_ENTRANCE: u8 = 9;

/// Packed earth trodden bare (a camp's clearing).
pub const URBAN_YARD: u8 = 10;

/// A lawn in town (a green, a temple's close, a park): the ground's own, with no wild scatter.
pub const URBAN_PARK: u8 = 11;

/// A park's gravel paths and rounds (kept clear of what the guarantee adds).
pub const URBAN_PATH: u8 = 12;

/// A village's toft and croft, a manor's or a shrine's yard (`town::village`): the ground's own,
/// grass for forest floor, no wild scatter.
pub const URBAN_CROFT: u8 = 13;

/// A tower's open floor inside its ring of wall, and its door (item 104, `town::wallstyle::raster`):
/// stone underfoot, open to the sky on the battlemap.
pub const URBAN_TOWER_FLOOR: u8 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Atmosphere {
    None = 0,
    Mist,
    Fog,
    Fireflies,
    Snowfall,
    BlowingSand,
    Embers,
    HeatHaze,
    FallingLeaves,
    Drizzle,
}

/// The gates in a layout's walls as pairs of gate towers (world ft; `towers` as
/// `interior::towers` gives them): two gate towers 36 to 54 ft apart (a passage at least 6 ft
/// wide between them), each beyond the other's wall (the towers of two gates side by side face
/// each other along one stretch) and the walls running on from them opposite ways (a castle's
/// outer and inner gates are not one gatehouse). Paired in order, each with its nearest; a pure function of the
/// layout, so every chunk finds the same.
pub fn gateways(l: &crate::town::Layout, towers: &[([f64; 2], f64, bool)]) -> Vec<([f64; 2], [f64; 2])> {
    gateways_on(&l.walls, towers, &l.gates)
}

/// `gateways` along `walls` (a town's as laid, standing or not), its gates' corners `town_gates`.
pub fn gateways_on(walls: &[Vec<[f64; 2]>], towers: &[([f64; 2], f64, bool)], town_gates: &[[f64; 2]]) -> Vec<([f64; 2], [f64; 2])> {
    let gates: Vec<[f64; 2]> = towers.iter().filter(|t| t.2).map(|t| t.0).collect();
    // Which way the wall runs from each gate tower, where a stretch of it ends there.
    let away: Vec<Option<[f64; 2]>> = gates
        .iter()
        .map(|&g| {
            let at = |p: [f64; 2]| (p[0] - g[0]) * (p[0] - g[0]) + (p[1] - g[1]) * (p[1] - g[1]) < 0.25;
            walls.iter().filter(|w| w.len() >= 2).find_map(|w| {
                let n = w.len();
                if at(w[0]) {
                    Some([w[1][0] - g[0], w[1][1] - g[1]])
                } else if at(w[n - 1]) {
                    Some([w[n - 2][0] - g[0], w[n - 2][1] - g[1]])
                } else {
                    None
                }
            })
        })
        .collect();
    let mut used = vec![false; gates.len()];
    let mut out = Vec::new();
    for i in 0..gates.len() {
        if used[i] {
            continue;
        }
        let mut best: Option<(f64, usize)> = None;
        for j in i + 1..gates.len() {
            let v = [gates[j][0] - gates[i][0], gates[j][1] - gates[i][1]];
            let d = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]);
            let beyond = |a: Option<[f64; 2]>, s: f64| a.is_none_or(|a| s * (a[0] * v[0] + a[1] * v[1]) < 0.0);
            // (The wall runs on from each the opposite way: not two gates of two curtains, one
            // behind the other.)
            let apart = match (away[i], away[j]) {
                (Some(a), Some(b)) => a[0] * b[0] + a[1] * b[1] < 0.0,
                _ => true,
            };
            // (A town gate midway between them: theirs, whichever way the wall runs on, as across
            // the point of a sharp corner, `wallgates::spread_sharp`.)
            let mid = [0.5 * (gates[i][0] + gates[j][0]), 0.5 * (gates[i][1] + gates[j][1])];
            let gate_between = town_gates.iter().any(|g| (g[0] - mid[0]).abs() + (g[1] - mid[1]).abs() < 1.0);
            if used[j] || !(36.0..=54.0).contains(&d) || (!gate_between && ((away[i].is_none() && away[j].is_none()) || !beyond(away[i], 1.0) || !beyond(away[j], -1.0) || !apart)) {
                continue;
            }
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, j));
            }
        }
        if let Some((_, j)) = best {
            used[i] = true;
            used[j] = true;
            out.push((gates[i], gates[j]));
        }
    }
    out
}

/// Round raised features on town squares (the battlemap draws each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlazaFeature {
    /// A round stone dais with steps (a speaker's platform).
    Dais = 0,
    /// A fountain: a raised stone basin with a central jet.
    Fountain = 1,
    /// A raised round planter with a tree in it.
    Planter = 2,
    /// A market cross on a stepped round plinth.
    Cross = 3,
    /// A low bank of trodden earth, hoof-worn round its edge (a steppe town's dais: R5 towns, 114).
    Mound = 4,
}

/// The biome of the land at `p` (world ft): where the map there is the sea's or a lake's (a
/// town on the shore), the nearest land's round it, out to about a T0 cell, else grassland.
pub fn land_biome(t0: &T0, p: [f64; 2]) -> Biome {
    let at = |q: [f64; 2]| Biome::from_u8(t0.sample_biome(q[0], q[1], t0.biome_warp(q[0], q[1]))[0]);
    let wet = |b: Biome| matches!(b, Biome::Ocean | Biome::Lake);
    std::iter::once(p)
        .chain([300.0, 1000.0, 2500.0, 4500.0, 7000.0].into_iter().flat_map(|r| {
            (0..8).map(move |k| {
                let t = std::f64::consts::TAU * k as f64 / 8.0;
                [p[0] + r * libm::cos(t), p[1] + r * libm::sin(t)]
            })
        }))
        .map(at)
        .find(|&b| !wet(b))
        .unwrap_or(Biome::Grassland)
}

/// What grows on a town's green or close in a biome: its trees, shrubs and small plants (palms
/// in the desert, acacias on the savanna, conifers in the north, none of a kind where the biome
/// has none), and whether its trees are few (one or two broadleaves on the steppe's grass).
pub fn greenery(b: Biome) -> (&'static [Kind], &'static [Kind], &'static [Kind], bool) {
    use Kind::*;
    match b {
        Biome::TemperateForest => (&[TreeDeciduous], &[Bush], &[Flowers, Mushrooms], false),
        Biome::TemperateRainforest => (&[TreeDeciduous, TreeConifer], &[Bush, Thicket], &[Mushrooms], false),
        Biome::Grassland => (&[TreeDeciduous], &[Bush], &[Flowers, TallGrass], false),
        Biome::Steppe => (&[TreeDeciduous], &[Bush], &[TallGrass], true),
        Biome::Savanna => (&[TreeAcacia], &[Bush], &[TallGrass], false),
        Biome::Jungle => (&[TreePalm, TreeJungle], &[Bush, Thicket], &[Flowers], false),
        Biome::HotDesert | Biome::SaltFlat => (&[TreePalm], &[Cactus], &[], false),
        Biome::Taiga | Biome::Alpine | Biome::Tundra | Biome::ColdDesert | Biome::Ice => (&[TreeConifer], &[Bush], &[], false),
        Biome::Swamp => (&[TreeWillow], &[Bush], &[Reeds], false),
        Biome::Blight | Biome::Ashland | Biome::Volcanic => (&[TreeDead], &[], &[], false),
        Biome::Ocean | Biome::Lake => (&[TreeDeciduous], &[Bush], &[], false),
    }
}

/// A dry square's beach (`Surface::Sand`, whatever the biome): within 3 ft of the sea beside it
/// (`depth`: its water's level over it; `level` that water's), on gentle ground.
#[inline]
pub fn beach(depth: f64, level: f64, sea: f64, slope: f64) -> bool {
    depth > -3.0 && level <= sea + 0.5 && slope < 0.15
}
