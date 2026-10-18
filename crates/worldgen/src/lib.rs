//! Deterministic procedural world generator.
//!
//! Every output is a pure function of (`WorldFile`, job key): no I/O, no threads inside
//! generators, no platform math (transcendentals go through `libm`). The same code runs
//! natively (`mapd`, tests) and as WASM in browser workers, and must produce identical bytes.

pub mod agent;
pub mod battlemap;
pub mod core;
pub mod gazetteer;
pub mod ground;
pub mod interior;
pub mod lod;
pub mod park;
pub mod payload;
pub mod pipeline;
pub mod town;
pub mod t0;
pub mod under;
pub mod world;

pub use world::{World, WorldFile, WorldParams};
