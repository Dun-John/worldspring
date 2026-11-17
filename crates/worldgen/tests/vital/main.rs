//! The only test suite: vital requirements, not per-module coverage.
//! Run with `cargo test --release -p worldgen`.

use worldgen::core::tile::{HALO, TILE_N, TileKey};
use worldgen::lod::terrain_refine::padded_index;
use worldgen::pipeline::{Executor, det_report};
use worldgen::under::design::SiteDesignOps;
use worldgen::world::{CreatedChecks, FollowCrossing};
use worldgen::{World, WorldFile};

mod common;
mod terrain;
mod towns;
mod battlemap;
mod interiors;
mod castles;
mod underground;
mod edits;
mod town_edits;
mod sketch;
mod remade;

use common::*;
use castles::*;
use town_edits::*;

