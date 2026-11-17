//! Paths drawn by hand, as the world file holds them: their kinds and lines (`paths` shapes them).

use serde::{Deserialize, Serialize};

use crate::core::geom::P;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PathKind {
    Footpath,
    #[default]
    Track,
    Gravel,
    Paved,
    Cobbled,
}

impl PathKind {
    pub const ALL: [PathKind; 5] = [PathKind::Footpath, PathKind::Track, PathKind::Gravel, PathKind::Paved, PathKind::Cobbled];

    pub fn parse(s: &str) -> Option<PathKind> {
        Self::ALL.iter().copied().find(|k| k.name() == s)
    }

    pub fn name(self) -> &'static str {
        ["footpath", "track", "gravel", "paved", "cobbled"][self as usize]
    }

    /// What it is called (in replies and lists).
    pub fn label(self) -> &'static str {
        ["footpath", "track", "gravel road", "paved road", "cobbled street"][self as usize]
    }

    /// Its width (ft) when none is given.
    pub fn width_ft(self) -> f64 {
        [4.0, 10.0, 14.0, 18.0, 20.0][self as usize]
    }

    /// Whether it crosses water on a bridge (else a ford).
    pub fn bridged(self) -> bool {
        matches!(self, PathKind::Gravel | PathKind::Paved | PathKind::Cobbled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Path {
    /// World ft, 2 to `MAX_PATH_PTS` points rounded to 0.5 ft.
    pub pts: Vec<P>,
    #[serde(rename = "type", default)]
    pub kind: PathKind,
    /// Its width (ft); the kind's own when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
