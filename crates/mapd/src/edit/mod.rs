//! The write families: one module per kind of thing a tool makes or changes, each with its handlers (run under
//! their own names, the change labels the app keeps) and their schemas (the `set_` tools are built from them).

pub(crate) mod building;
pub(crate) mod crossing;
pub(crate) mod design;
pub(crate) mod fence;
pub(crate) mod fortify;
pub(crate) mod ground;
pub(crate) mod handwalls;
pub(crate) mod hex;
pub(crate) mod link;
pub(crate) mod lots;
pub(crate) mod notebook;
pub(crate) mod objects;
pub(crate) mod road;
pub(crate) mod settlement;
pub(crate) mod site;
pub(crate) mod sketch;
pub(crate) mod streets;
pub(crate) mod town;
pub(crate) mod water;
pub(crate) mod wharf;
