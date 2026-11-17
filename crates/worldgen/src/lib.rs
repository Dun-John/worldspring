//! Deterministic procedural world generator.
//!
//! Every output is a pure function of (`WorldFile`, job key): no I/O, no threads inside
//! generators, no platform math (transcendentals go through `libm`). The same code runs
//! natively (`mapd`, tests) and as WASM in browser workers, and must produce identical bytes.

pub mod agent;
mod checks;
pub mod placing;
pub mod battlemap;
pub use worldgen_core as core;
pub use worldgen_model::{model, waters};
/// The world file and `World` (`worldgen-model`), with the extension traits that check and
/// follow its records against the generators above it.
pub mod world {
    pub use worldgen_model::world::*;
    pub use crate::checks::{BuildingEditChecks, CreatedChecks};
    pub use crate::wharf::FollowCrossing;
}
pub mod gazetteer;
pub mod geoquery;
pub mod ground;
pub mod paths;
pub mod interior;
pub mod links;
pub mod lod;
pub mod park;
pub mod payload;
pub mod pipeline;
pub mod roadids;
pub mod town;
pub mod t0;
pub mod sitelinks;
pub mod under;
pub mod undersearch;
pub mod wharf;

pub use world::{World, WorldFile, WorldParams};
