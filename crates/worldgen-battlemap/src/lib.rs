//! Worldgen's battlemap layer: a finest-level tile's five-foot squares, objects and structures
//! (`battlemap`). Depends on worldgen-town and the layers under it, which it re-exports so the
//! moved code's `crate::` paths stay as they were.

pub use worldgen_town::*;
pub mod battlemap;
