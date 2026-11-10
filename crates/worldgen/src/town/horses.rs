//! (R5 towns, item 114) A steppe town's horse pens and horse market: their battlemap props (ids 106-107,
//! the towns lane's range). The park itself is laid out in a later slice.

use crate::battlemap::{Kind, KindInfo};

const fn k(id: Kind, name: &'static str, radius: f32, blocks_move: bool, blocks_sight: bool, cover: u8, difficult: bool, height_ft: f32) -> KindInfo {
    KindInfo { id: id as u16, name, radius, blocks_move, blocks_sight, cover, difficult, height_ft, hazard: None, feature: true }
}

/// The horse park's props, for the battlemap's catalog.
pub(crate) const KINDS: [(Kind, KindInfo); 2] = [
    // (About 8 ft nose to tail.)
    (Kind::Horse, k(Kind::Horse, "horse", 0.8, true, false, 1, false, 5.5)),
    // (A rail on two posts, 10 ft long along x.)
    (Kind::HitchingRail, k(Kind::HitchingRail, "hitching rail", 1.0, false, false, 0, false, 3.5)),
];
