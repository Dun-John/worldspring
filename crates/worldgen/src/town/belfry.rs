//! A city's or a metropolis's town hall's belfry (`Building::parts`: its footprint the body, a square
//! tower on it): `SIDE` squares across on a corner of the body's grid (`interior::Shell::of_parts`:
//! whole squares from its first corner along its long axis), over the stair hall the hall's plan puts
//! there (`interior::hub::civic`: so only where a plan by zones has it so). It stands proud of the
//! hall's walls (`PROUD` ft) where no other building or wall is in the way, on the street front's
//! corner nearer the town's middle first, then the others; it rises `STOREYS` above the hall's top
//! floor. A town laid out anew keeps a hall's belfry where the hall stands as generated, else raises
//! it again from the hall's footprint (the same rules).

use super::geom::{P, add, area, cross, dist, dot, mul, obb, seg_dist, sub, union_outline};
use super::{Building, Layout, Part, PartKind, Structure, Ward, catalog};
use crate::t0::settle::Tier;

/// The tower's side (squares: the stair hall's).
pub const SIDE: isize = 3;
/// Its storeys above the hall's top floor.
pub const STOREYS: i8 = 2;
/// How far its walls stand out from the hall's (ft).
pub const PROUD: f64 = 1.5;

const SQ: f64 = 5.0;

/// Whether a building is a town hall.
pub fn is_town_hall(b: &Building) -> bool {
    b.func.is_some_and(|f| catalog::CATALOG[f as usize].key == "town_hall")
}

/// A town hall's belfry (its tower piece), if it has one.
pub fn of(b: &Building) -> Option<&Part> {
    (is_town_hall(b) && b.parts.len() == 2 && b.parts[0].kind == PartKind::Body && b.parts[1].kind == (PartKind::Tower { round: false })).then(|| &b.parts[1])
}

/// Why a city's town hall has no belfry (`raise`), for the checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No corner of it has whole squares for the tower.
    Corners,
    /// No plan by zones puts its stair hall on a corner (too small, a slanted or odd lot).
    Plan,
}

/// Each town hall of layout `l` (a city's or a metropolis's; `eligible`) with its belfry where one
/// goes (`raise`).
/// In a town laid out anew, a town hall standing as `generated` (its id, function and footprint the
/// same) keeps the belfry it had.
pub(super) fn raise_all(l: &mut Layout, generated: Option<&Layout>) {
    if l.tier < Tier::City {
        return;
    }
    // (Kept where its tower, proud of the hall's walls, still stands free, `free`; else raised again.)
    let was = |b: &Building| {
        let g = generated?.building(b.id as usize).filter(|g| g.func == b.func)?;
        let body = &g.parts.first()?.poly;
        let proud = area(&g.poly).abs() > area(body).abs() + 0.5;
        (of(g).is_some_and(|t| !proud || free(l, b, &t.poly)) && body.len() == b.poly.len() && body.iter().zip(&b.poly).all(|(p, q)| dist(*p, *q) < 0.01)).then(|| (g.poly.clone(), g.parts.clone()))
    };
    let done: Vec<(usize, Vec<P>, Vec<Part>)> = l.buildings.iter().enumerate().filter(|(_, b)| eligible(b)).filter_map(|(k, b)| was(b).or_else(|| raise(l, b).ok()).map(|(p, q)| (k, p, q))).collect();
    for (k, poly, parts) in done {
        let b = &mut l.buildings[k];
        b.poly = poly;
        b.parts = parts;
    }
}

/// A town hall a belfry is tried for (in a city: `raise_all`): roofed, not a castle's, not in pieces.
pub fn eligible(b: &Building) -> bool {
    is_town_hall(b) && b.structure == Structure::Roofed && b.ward != Ward::Castle && b.keep.is_none() && b.parts.is_empty()
}

/// Town hall `b` of layout `l` with its belfry: its outline and its pieces (world ft), or why none.
pub fn raise(l: &Layout, b: &Building) -> Result<(Vec<P>, Vec<Part>), Refused> {
    let body = &b.poly;
    let u = obb(body).axis;
    let v = [-u[1], u[0]];
    // (The body's grid, as `interior::Shell::of_parts` lays it: lines through its first corner.)
    let o = add(mul(u, dot(body[0], u)), mul(v, dot(body[0], v)));
    let at = |x: f64, y: f64| add(o, add(mul(u, x * SQ), mul(v, y * SQ)));
    let ccw: Vec<P> = if area(body) < 0.0 { body.iter().rev().copied().collect() } else { body.clone() };
    let inside = |i: isize, j: isize| {
        let p = at(i as f64 + 0.5, j as f64 + 0.5);
        (0..ccw.len()).all(|k| cross(sub(ccw[(k + 1) % ccw.len()], ccw[k]), sub(p, ccw[k])) > 0.0)
    };
    let (x0, x1) = body.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(dot(sub(*p, o), u) / SQ), a.1.max(dot(sub(*p, o), u) / SQ)));
    let (y0, y1) = body.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(dot(sub(*p, o), v) / SQ), a.1.max(dot(sub(*p, o), v) / SQ)));
    let (mut i0, mut i1, mut j0, mut j1) = (isize::MAX, isize::MIN, isize::MAX, isize::MIN);
    for j in crate::core::floor(y0) as isize..crate::core::ceil(y1) as isize {
        for i in crate::core::floor(x0) as isize..crate::core::ceil(x1) as isize {
            if inside(i, j) {
                (i0, i1, j0, j1) = (i0.min(i), i1.max(i), j0.min(j), j1.max(j));
            }
        }
    }
    if i0 > i1 || i1 - i0 + 1 < 2 * SIDE || j1 - j0 + 1 < 2 * SIDE {
        return Err(Refused::Corners);
    }
    let street_d = |p: P| l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |s| seg_dist(p, s[0], s[1]) - 0.5 * w)).fold(f64::MAX, f64::min);
    // The corners' boxes (grid x, y of the first square; the ways out of it, x and y), on the street
    // front first (the side nearest a street), the nearer the town's middle first.
    let d = SIDE;
    let boxes = [(i0, j0, -1, -1), (i1 - d + 1, j0, 1, -1), (i1 - d + 1, j1 - d + 1, 1, 1), (i0, j1 - d + 1, -1, 1)];
    let mid = |bx: isize, by: isize| at(bx as f64 + 0.5 * d as f64, by as f64 + 0.5 * d as f64);
    let sides = [at(0.5 * (i0 + i1 + 1) as f64, j0 as f64), at((i1 + 1) as f64, 0.5 * (j0 + j1 + 1) as f64), at(0.5 * (i0 + i1 + 1) as f64, (j1 + 1) as f64), at(i0 as f64, 0.5 * (j0 + j1 + 1) as f64)];
    let front = (0..4).min_by(|&x, &y| street_d(sides[x]).total_cmp(&street_d(sides[y])).then(x.cmp(&y))).unwrap_or(0);
    // (Corner k is on sides k and k - 1.)
    let on_front = |k: usize| k == front || (k + 1) % 4 == front;
    let mut order: Vec<usize> = (0..4).collect();
    order.sort_by(|&x, &y| on_front(y).cmp(&on_front(x)).then(dist(mid(boxes[x].0, boxes[x].1), l.center).total_cmp(&dist(mid(boxes[y].0, boxes[y].1), l.center))).then(x.cmp(&y)));
    let mut whole = false;
    for k in order {
        let (bx, by, ox, oy) = boxes[k];
        if !(0..d).all(|p| (0..d).all(|q| inside(bx + p, by + q))) {
            continue;
        }
        whole = true;
        // (Proud of the walls on its outer sides where nothing else stands there.)
        let tower = |px: f64, py: f64| {
            let e = PROUD / SQ;
            let (xa, xb) = (bx as f64 - if ox < 0 { e * px } else { 0.0 }, (bx + d) as f64 + if ox > 0 { e * px } else { 0.0 });
            let (ya, yb) = (by as f64 - if oy < 0 { e * py } else { 0.0 }, (by + d) as f64 + if oy > 0 { e * py } else { 0.0 });
            vec![at(xa, ya), at(xb, ya), at(xb, yb), at(xa, yb)]
        };
        let (px, py) = (free(l, b, &tower(1.0, 0.0)) as u8 as f64, free(l, b, &tower(0.0, 1.0)) as u8 as f64);
        let tower = tower(px, py);
        let parts = vec![Part { poly: body.clone(), kind: PartKind::Body }, Part { poly: tower.clone(), kind: PartKind::Tower { round: false } }];
        let Some(outline) = union_outline(&[body.clone(), tower]) else { continue };
        // (Its plan by zones has its stair hall under it: `interior::hub::civic`.)
        let mut t = b.clone();
        t.poly = outline.clone();
        t.parts = parts.clone();
        let sh = crate::interior::Shell::of(l, &t);
        if crate::interior::hub::kind_of(&t, Some("town_hall"), &sh, false, true).is_some() {
            return Ok((outline, parts));
        }
    }
    Err(if whole { Refused::Plan } else { Refused::Corners })
}

/// Whether tower outline `p` of town hall `b` stands clear of layout `l`'s other buildings, of a
/// coaching inn's ground (`Layout::grounds`: its own) and 5 ft off its walls.
fn free(l: &Layout, b: &Building, p: &[P]) -> bool {
    let inn_ground = |g: &(u32, Vec<P>)| g.0 != b.id && l.buildings.iter().any(|o| o.id == g.0 && super::palace::is_coaching(l, o));
    l.buildings.iter().filter(|o| o.id != b.id).all(|o| !overlaps(p, &o.poly)) && l.grounds.iter().filter(|g| inn_ground(g)).all(|g| !overlaps(p, &g.1)) && l.walls.iter().all(|w| w.windows(2).all(|s| p.iter().all(|&q| seg_dist(q, s[0], s[1]) > 5.0)))
}

fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (d1, d2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (d3, d4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Whether two polygons overlap (an edge of one crossing the other's, or one inside the other).
fn overlaps(p: &[P], q: &[P]) -> bool {
    if q.len() < 3 {
        return false;
    }
    p.iter().any(|&x| super::geom::contains(q, x)) || q.iter().any(|&x| super::geom::contains(p, x)) || (0..p.len()).any(|k| (0..q.len()).any(|m| segs_cross(p[k], p[(k + 1) % p.len()], q[m], q[(m + 1) % q.len()])))
}
