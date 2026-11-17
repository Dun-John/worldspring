//! Worldgen's settlements layer: towns and villages (`town`), interiors (`interior`), underground
//! sites (`under`), parks, wharves, hand paths, the checks on hand records, the links from
//! buildings to sites below (`sitelinks`) and the battlemap's kinds and chunk data (`battle`).
//! Depends on worldgen-core, worldgen-model and worldgen-terrain.

pub use worldgen_core as core;
pub use worldgen_model::{model, waters};
pub use worldgen_terrain::{ground, lod, t0};
/// The world file and `World` (`worldgen-model`), with the extension traits that check and
/// follow its records against the settlements.
pub mod world {
    pub use worldgen_model::world::*;
    pub use crate::checks::{BuildingEditChecks, CreatedChecks};
    pub use crate::wharf::FollowCrossing;
}
pub mod battle;
mod checks;
pub mod interior;
pub mod park;
pub mod paths;
pub mod sitelinks;
pub mod town;
pub mod under;
pub mod wharf;

pub use world::{World, WorldFile, WorldParams};
