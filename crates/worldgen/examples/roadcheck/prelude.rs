//! Everything a module needs: the common imports, the shared helpers and every mode.

pub use std::collections::HashMap;
pub use std::sync::atomic::{AtomicBool, Ordering};

pub use serde_json::{json, Map, Value};
pub use worldgen::t0::T0;
pub use worldgen::town::geom::{centroid, contains, dist, lerp, seg_dist, sub};

pub(crate) use crate::JSON;
pub(crate) use crate::shared::*;
pub(crate) use crate::economy::*;
pub(crate) use crate::realms::*;
pub(crate) use crate::islands::*;
pub(crate) use crate::sea::*;
pub(crate) use crate::network::*;
