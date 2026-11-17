//! Paired exits between places, as the world file holds them: their two ends.

use serde::{Deserialize, Serialize};

/// One end: a level (`l:<site>:<level>`, levels counted from the bottom) and a square on it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct End {
    pub level: String,
    pub x: u16,
    pub y: u16,
}

/// A paired exit: two ends, how it goes (`WAYS`) and a name of its own, if it has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub a: End,
    pub b: End,
    #[serde(default = "stairs")]
    pub way: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn stairs() -> String {
    WAYS[0].into()
}

/// How a link goes (its item's name; `variant` is its place here): the first is the default.
pub const WAYS: [&str; 6] = ["stairs", "ladder", "passage", "trapdoor", "rope", "portal"];
