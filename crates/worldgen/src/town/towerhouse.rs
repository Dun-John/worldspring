//! A tower house: what a castle set by hand comes to on a patch too small for a curtain and a
//! yard (a sliver between the town wall and a street). Anything set by hand appears: here a small
//! tower keep (`KeepKind::Plain`, 4 storeys) standing alone, on the block nearest the patch it
//! fits on (its own first), clear of the streets, the town wall and its towers and gates as laid
//! (standing or not, so walls on and off lay it the same), the houses under it cleared.

use super::castle::{KEEP_FITS, KeepKind, plain_body};
use super::catalog::Ward;
use super::geom::*;
use super::{Building, Ground, Layout, Part, PartKind, Structure};
use crate::core::rng::Pcg32;

/// How far a tower house may stand from its patch (ft, from the patch's middle).
const REACH: f64 = 160.0;
/// Its least clearance (ft): from a block's edge (the street before it), from the town wall's
/// line, from a town tower's reach, from a town gate.
const IN_BLOCK: f64 = 2.0;
const OFF_WALL: f64 = 10.0;
const OFF_TOWER: f64 = 12.0;
const OFF_GATE: f64 = 40.0;
/// Houses within this of it are cleared away (ft).
const CLEAR: f64 = 4.0;

/// A tower house for the hand castle on `patch` (its outline): its building, or `None` where no
/// block within `REACH` holds even the smallest one. `walls`, `towers` (with their reach) and
/// `gates`: the town wall as laid.
pub(super) fn tower_house<G: Ground>(site: &G, patch: &[P], l: &Layout, walls: &[(P, P)], towers: &[(P, f64)], gates: &[P], prng: &mut Pcg32) -> Option<Building> {
    let rolls: [f64; 6] = std::array::from_fn(|_| prng.next_f64());
    let at = centroid(patch);
    let near = |b: &Vec<P>| b.len() >= 3 && b.iter().map(|p| dist(*p, at)).fold(f64::MAX, f64::min) < REACH;
    let blocks: Vec<&Vec<P>> = l.blocks.iter().filter(|b| near(b)).collect();
    let ok = |poly: &[P], block: &[P]| {
        poly.iter().all(|p| depth(block, *p) >= IN_BLOCK)
            && walls.iter().all(|&(q, r)| super::poly_seg_dist(poly, q, r) >= OFF_WALL && !contains(poly, q))
            && towers.iter().all(|&(t, r)| !contains(poly, t) && poly_gap(poly, &circle(t, r, 12)) >= OFF_TOWER)
            && gates.iter().all(|g| !contains(poly, *g) && poly_gap(poly, &[*g]) >= OFF_GATE)
            && l.roads.iter().all(|(pts, _, w)| pts.windows(2).all(|s| super::poly_seg_dist(poly, s[0], s[1]) >= 0.5 * w + 1.0))
            && site.buildable(poly)
    };
    // (The biggest that fits, nearest the patch's middle; the blocks' order and a 4-ft lattice
    // keep it the same whatever else changes far off.)
    for f in KEEP_FITS {
        let body = plain_body(f, &rolls);
        let mut best: Option<(f64, Vec<P>)> = None;
        for block in &blocks {
            let axis = main_axis(block);
            let v = [-axis[1], axis[0]];
            let (x0, y0, x1, y1) = block.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
            let (i0, j0, i1, j1) = (crate::core::floor(x0 / 4.0) as i64, crate::core::floor(y0 / 4.0) as i64, crate::core::ceil(x1 / 4.0) as i64, crate::core::ceil(y1 / 4.0) as i64);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let c = [4.0 * i as f64, 4.0 * j as f64];
                    let d = dist(c, at);
                    if !contains(block, c) || best.as_ref().is_some_and(|b| b.0 <= d) {
                        continue;
                    }
                    let mut poly: Vec<P> = body.iter().map(|q| add(c, add(mul(axis, q[0]), mul(v, q[1])))).collect();
                    if area(&poly) < 0.0 {
                        poly.reverse();
                    }
                    if ok(&poly, block) {
                        best = Some((d, poly));
                    }
                }
            }
        }
        if let Some((_, poly)) = best {
            return Some(Building {
                pad_ft: site.pad(&poly),
                parts: vec![Part { poly: poly.clone(), kind: PartKind::Body }],
                poly,
                ward: Ward::Castle,
                func: None,
                residential: 4,
                name: None,
                floors: 4,
                structure: Structure::Roofed,
                roof: None,
                tint: None,
                material: None,
                shape: None,
                shade: None,
                id: 0,
                keep: Some(KeepKind::Plain),
                deck: None,
            });
        }
    }
    None
}

/// Whether a house stands on a tower house's ground (`CLEAR` round it), so is cleared away.
pub(super) fn under(keep: &[P], poly: &[P]) -> bool {
    poly_gap(keep, poly) < CLEAR || contains(keep, poly[0]) || contains(poly, keep[0])
}

/// How deep `p` lies inside `poly` (negative outside).
fn depth(poly: &[P], p: P) -> f64 {
    let m = poly.len();
    let d = (0..m).map(|k| seg_dist(p, poly[k], poly[(k + 1) % m])).fold(f64::MAX, f64::min);
    if contains(poly, p) { d } else { -d }
}
