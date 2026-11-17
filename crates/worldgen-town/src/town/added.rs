//! (Roadmap 6 R4) Streets added to a generated town or village by hand (`TownEdit::streets`: a
//! row's road). The plan never sees them: they are laid over the town as built (after its
//! buildings' edits), each end joined to the nearest of its streets within `JOIN_END_FT` that it
//! can reach without crossing a wall (else left a dead end), pushed into `Layout::roads` as
//! streets (class 3) and taking the town's buildings off their band as roads drawn by hand do
//! (nothing written: the street gone, they stand again; the rest keep their ids).

use super::*;
use crate::core::hash::Fnv64;
use crate::paths::Near;
use crate::world::{AddedStreet, Edits, TownEdit};

/// How far (ft) an added street's end reaches for a street to join.
pub const JOIN_END_FT: f64 = 100.0;

/// The most streets added to one town.
pub const MAX_ADDED: usize = 64;

/// The class an added street has in `Layout::roads` (a paved street: listed as a street).
pub const CLASS: u8 = 3;

/// The streets added to layout `li` that apply: a settlement's (the world's own or one added by
/// hand, any tier) not laid out by hand, still where its edit says it was.
pub fn of<'a>(world: &'a World, t0: &T0, li: usize) -> Option<&'a BTreeMap<u32, AddedStreet>> {
    if world.file.edits.towns.is_empty() {
        return None;
    }
    let e = world.file.edits.towns.get(&wards::key(li)).filter(|e| !e.streets.is_empty())?;
    let s = settlement(t0, li)?;
    if hand::is_hand(world, li) || dist(e.at, [s.x, s.y]) > TownEdit::SAME_AT_FT {
        return None;
    }
    Some(&e.streets)
}

/// A hash of the streets added to layout `li` that apply (ids, lines, kinds, widths), if any.
pub fn fingerprint(world: &World, t0: &T0, li: usize) -> Option<u64> {
    let streets = of(world, t0, li)?;
    let mut h = Fnv64::default();
    for (id, st) in streets {
        h.write(&id.to_le_bytes());
        for v in st.pts.iter().flatten() {
            h.write(&v.to_le_bytes());
        }
        h.write(&[st.kind as u8, 0xad]);
        h.write(&st.width_ft().to_le_bytes());
    }
    Some(h.finish())
}

/// The box (world ft) the streets added to layout `li` may reach (with their joins), if it has
/// any: from the file alone (whether they apply is not asked).
pub fn bounds(edits: &Edits, li: usize) -> Option<[f64; 4]> {
    let e = edits.towns.get(&wards::key(li)).filter(|e| !e.streets.is_empty())?;
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for st in e.streets.values() {
        let m = JOIN_END_FT + 0.5 * st.width_ft();
        for p in &st.pts {
            b = [b[0].min(p[0] - m), b[1].min(p[1] - m), b[2].max(p[0] + m), b[3].max(p[1] + m)];
        }
    }
    (b[0] <= b[2]).then_some(b)
}

/// Each of `streets` (in id order) as laid in layout `l`: (id, line with its ends joined, width
/// ft). An end is joined to the nearest point of `l`'s roads and main streets (and of the added
/// streets before it) within `JOIN_END_FT` whose way to it crosses none of `l.walls`; an end
/// already on such a line, or with none in reach, is left as it is. `l` as built (before
/// `apply`); a street not yet added, given the next id, joins on the layout as `town::layout`
/// gives it (its added streets in) as it will once added.
pub fn lines(l: &Layout, streets: &BTreeMap<u32, AddedStreet>) -> Vec<(u32, Vec<P>, f64)> {
    let mut out: Vec<(u32, Vec<P>, f64)> = Vec::with_capacity(streets.len());
    for (&id, st) in streets {
        if st.pts.len() < 2 {
            continue;
        }
        let w = st.width_ft();
        let mut pts = st.pts.clone();
        let others = || l.roads.iter().map(|r| r.0.as_slice()).chain(l.streets.iter().map(Vec::as_slice)).chain(out.iter().map(|o| o.1.as_slice()));
        let ends = [join_end(l, others(), &st.pts, true, w), join_end(l, others(), &st.pts, false, w)];
        if let Some(q) = ends[0] {
            pts.insert(0, q);
        }
        if let Some(q) = ends[1] {
            pts.push(q);
        }
        out.push((id, pts, w));
    }
    out
}

/// Where an end of line `pts` (its first: `first`) joins a street of `others`: the nearest point
/// within `JOIN_END_FT` reached without crossing a wall, unless the line already meets that
/// street near the end (it ends on it, or runs a little past where it crosses it).
fn join_end<'a>(l: &Layout, others: impl Iterator<Item = &'a [P]>, pts: &[P], first: bool, w: f64) -> Option<P> {
    let end = if first { pts[0] } else { pts[pts.len() - 1] };
    let mut best: Option<(f64, P, &'a [P])> = None;
    for line in others {
        for s in line.windows(2) {
            let (a, b) = (s[0], s[1]);
            if end[0] < a[0].min(b[0]) - JOIN_END_FT || end[0] > a[0].max(b[0]) + JOIN_END_FT || end[1] < a[1].min(b[1]) - JOIN_END_FT || end[1] > a[1].max(b[1]) + JOIN_END_FT {
                continue;
            }
            let q = closest(end, a, b);
            let d = dist(end, q);
            if d > JOIN_END_FT || best.is_some_and(|(bd, bq, _)| (d, q[0], q[1]) >= (bd, bq[0], bq[1])) {
                continue;
            }
            if d > 1e-6 && crosses_wall(l, &[end, q]) {
                continue;
            }
            best = Some((d, q, line));
        }
    }
    let (d, q, street) = best?;
    if d <= 0.5 {
        return None;
    }
    // (The line crossing that street within reach of its end: joined there already.)
    let reach = 2.0 * d + w;
    let crossed = pts.windows(2).any(|s| street.windows(2).any(|t| meet(s[0], s[1], t[0], t[1]).is_some_and(|m| dist(m, end) <= reach)));
    (!crossed).then_some(q)
}

/// Where segments a-b and c-d cross, if they do.
fn meet(a: P, b: P, c: P, d: P) -> Option<P> {
    let (r, s) = (sub(b, a), sub(d, c));
    let den = cross(r, s);
    if crate::core::fabs(den) < 1e-12 {
        return None;
    }
    let t = cross(sub(c, a), s) / den;
    let u = cross(sub(c, a), r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| add(a, mul(r, t)))
}

/// The point of segment a-b nearest `p`.
fn closest(p: P, a: P, b: P) -> P {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    if l2 <= 1e-12 {
        return a;
    }
    let t = (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0);
    add(a, mul(ab, t))
}

/// Whether line `line` (world ft) crosses one of layout `l`'s walls (a town's or a castle's).
pub fn crosses_wall(l: &Layout, line: &[P]) -> bool {
    let o = |p: P, q: P, r: P| cross(sub(q, p), sub(r, p));
    line.windows(2).any(|s| {
        let (p, q) = (s[0], s[1]);
        l.walls.iter().any(|wall| {
            wall.windows(2).any(|t| {
                let (r, u) = (t[0], t[1]);
                if p[0].max(q[0]) < r[0].min(u[0]) || p[0].min(q[0]) > r[0].max(u[0]) || p[1].max(q[1]) < r[1].min(u[1]) || p[1].min(q[1]) > r[1].max(u[1]) {
                    return false;
                }
                o(p, q, r) * o(p, q, u) < 0.0 && o(r, u, p) * o(r, u, q) < 0.0
            })
        })
    })
}

/// Line `pts` thinned (Ramer-Douglas-Peucker): only the points it needs to stay within `tol` ft
/// of itself, its ends kept.
pub fn thin(pts: &[P], tol: f64) -> Vec<P> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((i, j)) = stack.pop() {
        let mut far = (0.0, 0usize);
        for k in i + 1..j {
            let d = seg_dist(pts[k], pts[i], pts[j]);
            if d > far.0 {
                far = (d, k);
            }
        }
        if far.0 > tol {
            keep[far.1] = true;
            stack.push((i, far.1));
            stack.push((far.1, j));
        }
    }
    pts.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p).collect()
}

/// Lay the streets added to layout `l` over it (`town::layout`, after its buildings' edits and
/// look): each joined line into `l.roads` as a street, the buildings meeting its band taken off.
pub fn apply(world: &World, t0: &T0, l: &mut Layout) {
    let Some(streets) = of(world, t0, l.index as usize) else { return };
    let laid = lines(l, streets);
    let near: Vec<Near> = laid.iter().map(|(id, pts, w)| Near { kind: streets[id].kind, half: 0.5 * w, segs: pts.windows(2).map(|s| (s[0], s[1])).collect() }).collect();
    l.buildings.retain(|b| !crate::paths::meets(&near, &b.poly));
    for (_, pts, w) in laid {
        let h = 0.5 * w;
        for p in &pts {
            l.bbox = [l.bbox[0].min(p[0] - h), l.bbox[1].min(p[1] - h), l.bbox[2].max(p[0] + h), l.bbox[3].max(p[1] + h)];
        }
        l.roads.push((pts, CLASS, w));
    }
}

/// Where a street added to layout `li` would be kept from (the `TownEdit::at` its edit needs):
/// the settlement's centre when it is one (the world's own or added by hand, any tier) not laid
/// out by hand; else `None` (a hand town's streets are its records, a place has no streets).
pub fn home(world: &World, t0: &T0, li: usize) -> Option<P> {
    let s = settlement(t0, li)?;
    (!hand::is_hand(world, li)).then_some([s.x, s.y])
}

/// The streets added to `l` (as `town::layout` gives it): (index in `l.roads`, id), in id order.
/// `apply` pushes them last.
pub fn ids(world: &World, t0: &T0, l: &Layout) -> Vec<(usize, u32)> {
    let Some(streets) = of(world, t0, l.index as usize) else { return Vec::new() };
    let ids: Vec<u32> = streets.iter().filter(|s| s.1.pts.len() >= 2).map(|s| *s.0).collect();
    let first = l.roads.len().saturating_sub(ids.len());
    ids.into_iter().enumerate().map(|(k, id)| (first + k, id)).collect()
}

/// The streets added to layout `li` as laid (`town::layout`): (id, joined line, width ft).
pub fn laid(world: &World, t0: &T0, li: usize) -> Vec<(u32, Vec<P>, f64)> {
    if of(world, t0, li).is_none() {
        return Vec::new();
    }
    let l = layout(world, t0, li);
    ids(world, t0, &l).into_iter().filter_map(|(k, id)| l.roads.get(k).map(|r| (id, r.0.clone(), r.2))).collect()
}
