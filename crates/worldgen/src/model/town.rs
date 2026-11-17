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

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

pub(crate) fn is_zero(v: &u32) -> bool {
    *v == 0
}
