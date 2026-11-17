//! What a hand-laid town block is filled with, as the world file holds it.

use serde::{Deserialize, Serialize};

use crate::core::geom::P;

/// What a block holds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BlockFill {
    /// Lots of a ward (`TOWN_WARDS`), their size (`LOT_SIZES`; none: the ward's usual), their
    /// storeys (`HEIGHTS`; none: the ward's usual), rolled again `reroll` times, a temple's close
    /// round its temple or not (none: where it fits).
    Generated {
        ward: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lots: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<String>,
        #[serde(default, skip_serializing_if = "is_zero")]
        reroll: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        close: Option<bool>,
    },
    /// Left empty, for buildings drawn by hand.
    Hand,
    /// Open ground; `style` the square's kind (`PlazaKind::name`) or the park's (`ParkKind::name`).
    Open {
        open: OpenKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// A castle: as converted, its buildings in the block's `kept` (`hand`: one set on the patch
    /// by hand, not planned).
    Castle {
        #[serde(default, skip_serializing_if = "is_false")]
        hand: bool,
        /// (Roadmap 6 P7) Built from the block's outline (none kept): its gates (world ft, each
        /// on the side nearest it, the main one first; none: the side facing the town's middle),
        /// its keep's shape ("square", "round"; none: rolled) and moat ("none", "dry", "wet";
        /// none: rolled).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        gates: Vec<P>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keep_shape: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moat: Option<String>,
    },
}

impl Default for BlockFill {
    fn default() -> Self {
        BlockFill::Generated { ward: "common".into(), lots: None, height: None, reroll: 0, close: None }
    }
}

/// Open ground a block (or a painted area) is laid out as.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpenKind {
    Plaza,
    Lawn,
    Park,
    Garden,
    Market,
    Cemetery,
    Farmland,
}

pub fn is_false(b: &bool) -> bool {
    !*b
}

pub fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// A junction (`/n<id>`), and the square opening there.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Junction {
    pub p: P,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub square: Option<String>,
}

/// A street (`/e<id>`) from junction `a` to `b` by its bends; its paved width, and how far the
/// blocks on its left and right (looking from `a` to `b`) stand back from its line.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Street {
    pub a: u32,
    pub b: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bends: Vec<P>,
    pub width: f64,
    pub set_l: f64,
    pub set_r: f64,
    #[serde(default, skip_serializing_if = "StreetClass::is_street")]
    pub class: StreetClass,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bridge: bool,
}

/// What a street is (`Street::class`).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreetClass {
    /// A main street (from a gate to the market, over a bridge).
    Main,
    #[default]
    Street,
    /// A quay along the water.
    Quay,
    /// No street: two blocks of one district meet (both setbacks 0).
    Joint,
}

impl StreetClass {
    fn is_street(&self) -> bool {
        *self == StreetClass::Street
    }
}
