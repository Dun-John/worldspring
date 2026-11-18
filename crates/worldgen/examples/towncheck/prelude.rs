//! Everything a mode module needs: the common imports, the shared checks and every other mode.

pub use std::collections::{BTreeMap, HashMap, VecDeque};
pub use std::rc::Rc;
pub use std::sync::atomic::{AtomicBool, Ordering};
pub use std::time::Instant;

pub use worldgen::battlemap::{Chunk, Kind, SQ, SQUARE_FT, Surface, URBAN_STREET, URBAN_TOWER, URBAN_WALL};
pub use worldgen::core::tile::TileKey;
pub use worldgen::pipeline::Executor;
pub use worldgen::t0::T0;
pub use worldgen::town::catalog::Ward;
pub use worldgen::town::geom::{add, area, centroid, contains, cross, dist, dot, len, lerp, mean, mul, seg_dist, sub};
pub use worldgen::town::{self, Layout, PlazaKind};
pub use worldgen::under::design::SiteDesignOps;
pub use worldgen::world::CreatedChecks;
pub use worldgen::{World, interior};

pub use crate::{P, QUIET, top};
pub(crate) use crate::shared::*;
pub(crate) use crate::castles::*;
pub(crate) use crate::interiors::*;
pub(crate) use crate::temples::*;
pub(crate) use crate::villages::*;
pub(crate) use crate::walls::*;
pub(crate) use crate::cemeteries::*;
pub(crate) use crate::handwalls::*;
pub(crate) use crate::steppe::*;
