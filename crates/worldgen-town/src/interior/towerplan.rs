//! D and octagon towers' floors (item 104): laid inside the tower's real outline (`wallstyle::tower_outline`,
//! as its ring wall on the battlemap is), not a disc. The grid is set square to the way the tower faces (a D's
//! round front, an octagon's sides), so the shape lies even on it; a square is the tower's where its middle is
//! within 1.5 ft of the outline, as on the battlemap (`wallstyle::raster`). The rest (walls, stairs, the door
//! toward the town or the castle yard, the wall walk each way, the fighting top) is `tower`'s, on these squares.

use crate::town::Layout;
use crate::town::geom::{P, contains, seg_dist};
use crate::town::wallstyle::{TowerShape, tower_outline};

/// A D or octagon tower's grid axis (unit, the way it faces) and outline; `None` for any other shape.
pub(super) fn outline(l: &Layout, at: P, r: f64, gate: bool) -> Option<(P, Vec<P>)> {
    let (shape, dir, poly, _) = tower_outline(l, at, r, gate);
    matches!(shape, TowerShape::D | TowerShape::Octagon).then_some((dir, poly))
}

/// Whether the square with its middle at `p` is the tower's (outline `poly`).
pub(super) fn covers(poly: &[P], p: P) -> bool {
    let n = poly.len();
    contains(poly, p) || (0..n).any(|k| seg_dist(p, poly[k], poly[(k + 1) % n]) <= 1.5)
}
