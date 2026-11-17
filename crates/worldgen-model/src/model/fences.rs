//! (Roadmap 6 R6) Fences and hedges as the world file holds them: lines drawn by hand
//! (`Edits::fences`, `f:<n>`), gaps cut by the eraser (`Edits::fence_gaps`, `fg:<n>`) and the
//! changes made to a town's generated runs (`TownEdit::fences`, `town::fence`).

use serde::{Deserialize, Serialize};

use crate::core::geom::P;

/// What a fence or hedge is (how the battlemap draws it and what it does in play).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceKind {
    /// A field's or croft's hedge: hard going, half cover, seen over.
    #[default]
    Hedgerow,
    /// A park's clipped hedge: blocks the way, three-quarters cover, seen over.
    Hedge,
    DryStoneWall,
    /// Rails: hard going, no cover.
    PostAndRail,
    Wattle,
    /// A manor's yard wall, a forecourt's (generated only): raised, blocks the way and sight.
    GardenWall,
    LowWall,
    Picket,
    IronRailing,
    Stockade,
    HighWall,
}

impl FenceKind {
    pub const ALL: [FenceKind; 11] = [
        FenceKind::Hedgerow,
        FenceKind::Hedge,
        FenceKind::DryStoneWall,
        FenceKind::PostAndRail,
        FenceKind::Wattle,
        FenceKind::GardenWall,
        FenceKind::LowWall,
        FenceKind::Picket,
        FenceKind::IronRailing,
        FenceKind::Stockade,
        FenceKind::HighWall,
    ];

    /// Its key (as the world file writes it).
    pub fn name(self) -> &'static str {
        ["hedgerow", "hedge", "dry_stone_wall", "post_and_rail", "wattle", "garden_wall", "low_wall", "picket", "iron_railing", "stockade", "high_wall"][self as usize]
    }

    pub fn parse(s: &str) -> Option<FenceKind> {
        Self::ALL.iter().copied().find(|k| k.name() == s)
    }

    /// What it is called (in replies and lists).
    pub fn label(self) -> &'static str {
        ["hedgerow", "clipped hedge", "dry-stone wall", "post-and-rail fence", "wattle fence", "garden wall", "low stone wall", "picket fence", "iron railing", "stockade", "high wall"][self as usize]
    }
}

/// A gate's opening (ft along its line).
pub const GATE_OPEN_FT: f64 = 5.0;
/// Points a drawn fence holds at most.
pub const MAX_FENCE_PTS: usize = 400;
/// An eraser stroke's radius (ft) when none is given.
pub const GAP_R_FT: f32 = 2.5;

/// A fence or hedge drawn by hand (`f:<n>`): its line (world ft; round to its first point when
/// `closed`) and its gates (ft along the line from `pts[0]`, each a `GATE_OPEN_FT` opening).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fence {
    #[serde(default)]
    pub kind: FenceKind,
    pub pts: Vec<P>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<f32>,
}

/// A gap cut by the eraser (`fg:<n>`): discs of radius `r` (ft) along its stroke, cutting every
/// fence and hedge under them, generated or drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FenceGap {
    pub pts: Vec<P>,
    #[serde(default = "gap_r")]
    pub r: f32,
}

fn gap_r() -> f32 {
    GAP_R_FT
}

/// A town's generated run changed by hand (`TownEdit::fences`, by its local key): gone, another
/// kind, gates put in (ft along it from its canonical start, `town::fence`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FenceChange {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<FenceKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<f32>,
}

impl FenceChange {
    /// Whether it changes nothing.
    pub fn is_noop(&self) -> bool {
        !self.gone && self.kind.is_none() && self.gates.is_empty()
    }
}
