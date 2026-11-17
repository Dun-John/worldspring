//! Worldgen's terrain layer: the continent (`t0`), its tiles refined level by level (`lod`) and
//! ground shaped by hand (`ground`). Depends on worldgen-core and worldgen-model.

pub use worldgen_core as core;
pub use worldgen_model::{model, waters, world};
pub mod ground;
pub mod lod;
pub mod t0;

pub use world::{World, WorldFile, WorldParams};
