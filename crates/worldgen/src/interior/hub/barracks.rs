//! The barracks programme: a barracks of 150 squares or more laid out as a hall house round its mess
//! hall (`keep`'s zones, no screens passage): its front on the long side with more open ground before
//! it, else entered from an end through a bay of the mess hall between the kitchen and the pantry
//! (`ends`, `bay`); the officers' quarters and the armory at the end away from the kitchen
//! (`Plan::officers`); barrack rooms, stores and an armory on every floor.

use super::kit::Lay;
use super::{Frame, Kinds, Piece, Plan};
use crate::interior::Shell;

/// The rooms of the mess hall's floor beside the hall (by weight), and what fills the rest.
pub(super) const HALL_KINDS: &[(&str, f64)] = &[("barrack room", 1.2), ("storeroom", 1.0)];
pub(super) const HALL_FILL: &[&str] = &["barrack room", "storeroom"];

/// The rooms of a floor above the mess hall, and what fills the rest.
pub(super) const UPPER_KINDS: &[(&str, f64)] = &[("barrack room", 2.0), ("barrack room", 1.7), ("armory", 1.0), ("officers' quarters", 1.2), ("barrack room", 1.4), ("storeroom", 0.8)];
pub(super) const UPPER_FILL: &[&str] = &["barrack room", "storeroom"];

/// The service end's rooms across it, by weight.
pub(super) const SERVICE: &[(&str, f64)] = &[("kitchen", 1.6), ("pantry", 1.0)];

/// The tower rooms above the ground floor, before they are shuffled.
pub(super) const TOWER_ROOMS: [&str; 4] = ["barrack room", "storeroom", "armory", "barrack room"];

/// The kinds `kinds` as the programme's (owned) list.
pub(super) fn kinds(kinds: &[(&'static str, f64)]) -> Kinds {
    kinds.to_vec()
}

/// How many squares along a side of the body's box `bx` (x0, x1, y0, y1; a long side if `long`, at
/// its low edge if `lo`), 3 in from its corners, have open ground before them.
fn open_along(sh: &Shell, bx: [usize; 4], long_x: bool, ground: &dyn Fn(isize, isize) -> bool, long: bool, lo: bool) -> usize {
    let (nx, ny) = (sh.nx, sh.ny);
    let [x0, x1, y0, y1] = bx;
    let in_at = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && sh.inside[j as usize * nx + i as usize];
    let along_x = long == long_x;
    let (c, d) = match (along_x, lo) {
        (true, true) => (y0 as isize, -1),
        (true, false) => (y1 as isize, 1),
        (false, true) => (x0 as isize, -1),
        (false, false) => (x1 as isize, 1),
    };
    let (a0, a1) = if along_x { (x0 + 3, x1.saturating_sub(3)) } else { (y0 + 3, y1.saturating_sub(3)) };
    (a0 as isize..=a1 as isize)
        .filter(|&a| {
            let (i, j, oi, oj) = if along_x { (a, c, a, c + d) } else { (c, a, c + d, a) };
            in_at(i, j) && !in_at(oi, oj) && ground(oi, oj)
        })
        .count()
}

/// The low end and the front side (at the box's lower edges?), and whether it is entered from its
/// low end (`Plan::bay`): its mess hall opens on its front, the long side with more open ground
/// before it (a range built against a curtain turned from it); with both shut in, the end with more,
/// the kitchen's. Its corners aside, a corner tower's.
pub(super) fn ends(sh: &Shell, bx: [usize; 4], long_x: bool, ground: &dyn Fn(isize, isize) -> bool, street_low: bool, street_front: bool) -> (bool, bool, bool) {
    let open = |long: bool, lo: bool| open_along(sh, bx, long_x, ground, long, lo);
    let (lo, hi) = (open(true, true), open(true, false));
    let (el, eh) = (open(false, true), open(false, false));
    if lo + hi == 0 && el + eh > 0 { (el >= eh, street_front, true) } else { (street_low, if lo == hi { street_front } else { lo > hi }, false) }
}

/// The column (t) of the mess hall's bay through a barracks entered from its low end: through every
/// row of the service end, open ground before it; the kitchen the bigger side, as big as can be,
/// then nearer the middle.
pub(super) fn bay(sh: &Shell, piece: &[Piece], f: &Frame, ground: &dyn Fn(isize, isize) -> bool) -> Option<usize> {
    let (nx, ny) = (sh.nx, sh.ny);
    let (len, wid) = (f.len, f.wid);
    let in_at = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && sh.inside[j as usize * nx + i as usize];
    let service = if len >= 14 { 3 } else { 2 };
    let body = |s: isize, t: isize| {
        let (i, j) = f.grid(s, t);
        in_at(i, j) && piece[j as usize * nx + i as usize] == Piece::Body
    };
    let side = |t0: isize, t1: isize| (0..service).map(|s| (t0..t1).filter(|&t| body(s, t)).count()).sum::<usize>();
    let w = wid as isize;
    (0..w)
        .filter(|&t| {
            let (i, j) = f.grid(-1, t);
            (0..service).all(|s| body(s, t)) && !in_at(i, j) && ground(i, j)
        })
        .max_by_key(|&t| (side(0, t).max(side(t + 1, w)), std::cmp::Reverse((2 * t - w + 1).abs()), std::cmp::Reverse(t)))
        .map(|t| t as usize)
}

impl Plan {
    /// Entered from the end (`bay`, at column `m`): the kitchen and the pantry each side of the mess
    /// hall's (`hall`) bay through the service end, each with its door into the hall.
    pub(super) fn bay_service(&self, lay: &mut Lay, hall: i16, m: usize) {
        let w = self.f.wid;
        self.fill(lay, 0, self.service, m, m + 1, hall);
        let big = self.squares(0, self.service as isize, 0, m as isize) >= self.squares(0, self.service as isize, m as isize + 1, w as isize);
        let (k, p) = if big { ((0, m), (m + 1, w)) } else { ((m + 1, w), (0, m)) };
        for (kind, (t0, t1)) in [("kitchen", k), ("pantry", p)] {
            let r = lay.room(kind);
            self.fill(lay, 0, self.service, t0, t1, r);
            let c = ((t0 + t1) / 2) as isize;
            self.door(lay, (self.service as isize - 1, c), (self.service as isize, c));
        }
    }

    /// The dais end of a barracks' mess hall (`hall`, from row `h0`): the officers' quarters on the
    /// front side of the landing if there is room, else the far one (6 squares at least, a 2 x 2 block
    /// among them: run down the hall's side where a corner tower takes most of the end), the armory
    /// on the other.
    pub(super) fn officers(&self, lay: &mut Lay, hall: i16, h0: usize) {
        let w = self.f.wid;
        let (a, b, _) = self.dais(lay, "officers' quarters", "armory");
        let (r, t0, t1) = match (a, b) {
            (Some(r), _) => (r, 0, self.land_t.0),
            (None, Some(r)) => (r, self.land_t.1, w),
            _ => (-1, 0, 0),
        };
        if r >= 0 {
            lay.rooms[r as usize].kind = "officers' quarters";
            if let Some(b) = b.filter(|&b| b != r) {
                lay.rooms[b as usize].kind = "armory";
            }
            let mut s0 = self.dz;
            let wide = |lay: &Lay| {
                let mine = |s: isize, t: isize| self.body(s, t).is_some_and(|k| lay.cells[k] == r);
                (0..self.f.len as isize).any(|s| (0..w as isize).any(|t| mine(s, t) && mine(s + 1, t) && mine(s, t + 1) && mine(s + 1, t + 1)))
            };
            while (lay.count(r) < 6 || !wide(lay)) && s0 > h0 + 3 && self.dz - s0 < 3 {
                s0 -= 1;
                for t in t0..t1 {
                    if let Some(k) = self.body(s0 as isize, t as isize).filter(|&k| lay.cells[k] == hall) {
                        lay.cells[k] = r;
                    }
                }
            }
        }
    }
}
