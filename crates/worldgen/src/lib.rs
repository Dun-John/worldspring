//! Deterministic procedural world generator.
//!
//! Every output is a pure function of (`WorldFile`, job key): no I/O, no threads inside
//! generators, no platform math (transcendentals go through `libm`). The same code runs
//! natively (`mapd`, tests) and as WASM in browser workers, and must produce identical bytes.

pub mod agent;
pub mod placing;
pub mod remap;
pub use worldgen_battlemap::battlemap;
pub use worldgen_core as core;
pub use worldgen_model::{model, waters};
pub use worldgen_town::world;
pub use worldgen_terrain::{ground, lod};
/// T0 (`worldgen-terrain`), with the parts that need the settlements above it: `remap` (edits
/// following a remade world's places) and the watchtower on an islet.
pub mod t0 {
    pub use worldgen_terrain::t0::*;
    pub use crate::remap;
    /// Islets (`worldgen-terrain`), and the watchtower standing on one.
    pub mod islets {
        pub use worldgen_terrain::t0::islets::*;
        pub use worldgen_town::town::sites::tower_building;
    }
}
pub mod fencecut;
pub mod gazetteer;
pub mod geoquery;
/// Interiors (`worldgen-town`), and an interior by id with the links put down by hand on it.
pub mod interior {
    pub use worldgen_town::interior::*;
    pub use crate::links::{generate_id, interior_json};
}
pub mod links;
pub mod payload;
pub mod pick;
pub mod pipeline;
pub mod roadcut;
pub mod roadids;
pub mod undersearch;
/// Wharves (`worldgen-town`), and the water and squares where an islet or a wharf's root goes.
pub mod wharf {
    pub use worldgen_town::wharf::*;
    pub use crate::wharf_site::{water_at, water_over, islet_at, islet_at_fine, root_problem};
}
mod wharf_site;
pub use worldgen_town::{park, paths, sitelinks, town, under};

pub use world::{World, WorldFile, WorldParams};
