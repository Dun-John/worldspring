//! Worldgen's model layer: the world file and the resolved `World`, the plain data its edits carry
//! (`model`) and hand-drawn waters. Depends on nothing but worldgen-core, libm and serde.

pub use worldgen_core as core;
pub mod model;
pub mod waters;
pub mod world;

pub use world::{World, WorldFile, WorldParams};
