//! A monastery round its cloister (on the rectangle its lots merged into: `town::palace`), along its
//! long side (s, from the end its way in is at) and across it (t, from its church's side):
//! - the church along one long side, a single hall from its west end to its altar and the god's
//!   statue at its east end, entered from the cloister walk;
//! - the cloister beyond it: an open garth (gravel walks across it to the cloister well in its
//!   middle) inside the walk round it, an arcade between them (`super::super::railed`);
//! - round the walk: the west range at the way in (the entrance hall from the front door to the walk,
//!   the scriptorium, the kitchen in the corner by the refectory), the refectory along the walk across
//!   from the church, the east range (the chapter house beside the church, its corners cut as an
//!   octagon's, then the dorter);
//! - above: the church's height open, nothing over the cloister; the ranges' rooms one into the next
//!   from the landing over the entrance hall (the library, a chamber, the infirmary, the novices'
//!   dorter, the abbot's chamber);
//! - below: the crypt under the church only, its stairs down from the entrance hall (`Plan::crypt`).

use super::kit::{Lay, side};
use super::{Frame, Kind, Laid, Piece, Plan};
use crate::interior::{OPEN_BELOW, Shell, building_item, hall, shape};
use crate::town::{self, geom::{self, P}};

/// A monastery's zones in its frame: the church's width, the west range's depth (s), the east
/// range's, the refectory's range's width (t), the chapter house's width.
#[derive(Clone, Copy, Debug)]
pub(super) struct Zones {
    pub(super) church: usize,
    pub(super) west: usize,
    pub(super) east: usize,
    pub(super) south: usize,
    pub(super) chapter: usize,
}

/// The least a monastery's footprint is (squares: long, short): its ranges (3 squares deep where it is
/// small) round a cloister whose garth is 3 squares or more each way.
pub const LEAST: (usize, usize) = (12, 12);

/// Whether a monastery stands round its cloister: a rectangle (its lots merged, `town::palace`) of
/// `LEAST` or more.
pub fn fits(b: &town::Building) -> bool {
    if b.poly.len() != 4 || !b.parts.is_empty() || !town::palace::is_monastery(b) {
        return false;
    }
    let o = geom::obb(&b.poly);
    o.long >= 5.0 * LEAST.0 as f64 - 1.0 && o.short >= 5.0 * LEAST.1 as f64 - 1.0 && geom::area(&b.poly).abs() >= 0.97 * o.long * o.short
}

/// A monastery's zones for a footprint `l` squares along its long side and `s` across: the church 4
/// squares wide (5 from 16 across), the west range 4 deep, the east 5 (4 under 16 long), the
/// refectory's range 4 wide (the west range and the refectory's 3 where that leaves no garth 3 squares
/// across); the chapter house as wide as the east range is deep, leaving the dorter 2 squares of the
/// walk. None where the garth would still be under 3 squares either way.
fn zones(l: usize, s: usize) -> Option<Zones> {
    let church = if s >= 16 { 5 } else { 4 };
    let east = if l >= 16 { 5 } else { 4 };
    let west = if l >= east + 4 + 5 { 4 } else { 3 };
    let south = if s >= church + 4 + 5 { 4 } else { 3 };
    let span = s.checked_sub(church + south)?;
    if l < west + east + 5 || span < 5 {
        return None;
    }
    let chapter = east.min(span - 2);
    Some(Zones { church, west, east, south, chapter })
}

/// A monastery's plan: its frame, its front (the end its way in is at) nearest a street, its door there
/// onto open ground first, its church along the long side nearer a street; its stairs in the entrance
/// hall. None for a footprint that doesn't fit (`fits`, `zones`).
pub(super) fn plan(b: &town::Building, sh: &Shell, seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let body: Vec<usize> = (0..nx * ny).filter(|&k| sh.inside[k]).collect();
    let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
    for &k in &body {
        (x0, x1, y0, y1) = (x0.min(k % nx), x1.max(k % nx), y0.min(k / nx), y1.max(k / nx));
    }
    if body.is_empty() {
        return None;
    }
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    if body.len() < bw * bh {
        return None;
    }
    let (xi, yi) = ((x0 as isize, x1 as isize), (y0 as isize, y1 as isize));
    let (l, s) = (bw.max(bh), bw.min(bh));
    let z = zones(l, s)?;
    // (Each frame along the long side: its corner, its way along and across.)
    let frames: Vec<Frame> = if bw >= bh {
        vec![
            Frame { o: (xi.0, yi.0), ds: (1, 0), dt: (0, 1), len: l, wid: s },
            Frame { o: (xi.0, yi.1), ds: (1, 0), dt: (0, -1), len: l, wid: s },
            Frame { o: (xi.1, yi.0), ds: (-1, 0), dt: (0, 1), len: l, wid: s },
            Frame { o: (xi.1, yi.1), ds: (-1, 0), dt: (0, -1), len: l, wid: s },
        ]
    } else {
        vec![
            Frame { o: (xi.0, yi.0), ds: (0, 1), dt: (1, 0), len: l, wid: s },
            Frame { o: (xi.1, yi.0), ds: (0, 1), dt: (-1, 0), len: l, wid: s },
            Frame { o: (xi.0, yi.1), ds: (0, -1), dt: (1, 0), len: l, wid: s },
            Frame { o: (xi.1, yi.1), ds: (0, -1), dt: (-1, 0), len: l, wid: s },
        ]
    };
    // (A frame point (s, t) in grid units.)
    let pt = |f: &Frame, s: f64, t: f64| {
        let (i, j) = f.grid(0, 0);
        let c = [i as f64 + 0.5 - 0.5 * (f.ds.0 + f.dt.0) as f64, j as f64 + 0.5 - 0.5 * (f.ds.1 + f.dt.1) as f64];
        [c[0] + f.ds.0 as f64 * s + f.dt.0 as f64 * t, c[1] + f.ds.1 as f64 * s + f.dt.1 as f64 * t]
    };
    let f = *frames
        .iter()
        .enumerate()
        .min_by_key(|(k, f)| {
            let door = (z.church as isize..(z.church + 2) as isize).any(|t| {
                let (i, j) = f.grid(-1, t);
                ground(i, j)
            });
            let (d, c) = (near(pt(f, 0.0, 0.5 * s as f64)), near(pt(f, 0.5 * l as f64, 0.0)));
            (!door, d >= 80.0, c >= 80.0, crate::core::round(d) as i64, crate::core::round(c) as i64, *k)
        })
        .map(|(_, f)| f)?;
    // The stairs in the entrance hall, along it on its side away from the church, the way through
    // beside them.
    let (a, e) = (f.grid(1, z.church as isize + 1), f.grid(2, z.church as isize + 1));
    let stairs = [a.0.min(e.0) as usize, a.1.min(e.1) as usize, a.0.abs_diff(e.0) + 1, a.1.abs_diff(e.1) + 1];
    let mut piece = vec![Piece::Out; nx * ny];
    for &k in &body {
        piece[k] = Piece::Body;
    }
    let mut p = Plan::bare(Kind::Monastery, nx, ny, f, piece, stairs, b.floors.max(1) as i8, seed);
    p.mon = Some(z);
    Some(p)
}

/// A piece in the garth (`Plan::garth_pieces`): its kind, its box in the frame (s0..s1, t0..t1).
pub type GarthPiece = (&'static str, (usize, usize, usize, usize));

impl Plan {
    /// A monastery's zones: the church's width, the ranges' depths (west, east, the refectory's), the
    /// chapter house's width; with its garth's box in its frame (s0..s1, t0..t1).
    pub fn monastery(&self) -> Option<((usize, usize, usize, usize, usize), (usize, usize, usize, usize))> {
        self.mon.map(|z| ((z.church, z.west, z.east, z.south, z.chapter), self.garth().expect("zones")))
    }

    /// A monastery's crypt's squares: under the church, and under the entrance hall the foot of its
    /// stairs and the way beside them into the church's; None for any other building.
    pub fn crypt(&self) -> Option<Vec<bool>> {
        let z = self.mon?;
        let mut out = vec![false; self.nx * self.ny];
        let cw = z.church as isize;
        let under = (0..self.f.len as isize).flat_map(|s| (0..cw).map(move |t| (s, t))).chain((1..3).flat_map(|s| [(s, cw), (s, cw + 1)]));
        for (s, t) in under {
            if let Some(k) = self.at(s, t).filter(|&k| self.piece[k] == Piece::Body) {
                out[k] = true;
            }
        }
        Some(out)
    }

    /// The garth's box (s0..s1, t0..t1): the cloister but its walk.
    fn garth(&self) -> Option<(usize, usize, usize, usize)> {
        let z = self.mon?;
        Some((z.west + 1, self.f.len - z.east - 1, z.church + 1, self.f.wid - z.south - 1))
    }

    /// The garth's pieces, as its ground floor has them and the battlemap draws them
    /// (`crate::interior::court_plan`): the cloister well in its middle (2 squares across in a garth 6
    /// or more each way and even, else 1; 2 by 1 on an even side), gravel walks a square wide (2 on an
    /// even side) from it to the walk on each side, along its axes. None for any other building.
    pub fn garth_pieces(&self) -> Vec<GarthPiece> {
        let Some((s0, s1, t0, t1)) = self.garth() else { return Vec::new() };
        let (gl, gw) = (s1 - s0, t1 - t0);
        let ws = if gl % 2 == 0 { 2 } else { 1 };
        let wt = if gw % 2 == 0 { 2 } else { 1 };
        let (a0, b0) = (s0 + (gl - ws) / 2, t0 + (gw - wt) / 2);
        let (a1, b1) = (a0 + ws, b0 + wt);
        let mut out: Vec<GarthPiece> = vec![("cloister_well", (a0, a1, b0, b1))];
        for (a, b) in [(s0, a0), (a1, s1)] {
            if b > a {
                out.push(("garden_path", (a, b, b0, b1)));
            }
        }
        for (a, b) in [(t0, b0), (b1, t1)] {
            if b > a {
                out.push(("garden_path", (a0, a1, a, b)));
            }
        }
        out
    }

    /// A monastery's floor `z` (`monastery` above); its cellar as other buildings' are.
    pub(super) fn monastery_level(&mut self, z: i8, mut lay: Lay) -> Option<Laid> {
        if z < 0 {
            return None;
        }
        let zn = self.mon?;
        let (l, w) = (self.f.len, self.f.wid);
        let (cw, wr, er, sr, ch) = (zn.church, zn.west, zn.east, zn.south, zn.chapter);
        let (li, wi, cwi, wri, eri, sri, chi) = (l as isize, w as isize, cw as isize, wr as isize, er as isize, sr as isize, ch as isize);
        let (gs0, gs1, gt0, gt1) = self.garth()?;
        // (Over the cloister nothing but the sky above the ground floor.)
        for s in wr..l - er {
            for t in cw..w - sr {
                if let Some(k) = self.at(s as isize, t as isize) {
                    self.piece[k] = if z == 0 { Piece::Body } else { Piece::Out };
                }
            }
        }
        let frame = |s0: usize, t0: usize, len: usize, wid: usize| hall::Frame { o: self.f.grid(s0 as isize, t0 as isize), ds: self.f.ds, dt: self.f.dt, len, wid };
        let mut halls = Vec::new();
        if z == 0 {
            let church = self.put_box(&mut lay, "nave", 0, l, 0, cw);
            let hall = self.put_box(&mut lay, "entrance hall", 0, wr, cw, cw + 2);
            let scr = self.put_box(&mut lay, "scriptorium", 0, wr, cw + 2, w - sr);
            let kitchen = self.put_box(&mut lay, "kitchen", 0, wr, w - sr, w);
            let walk = self.put_box(&mut lay, "cloister walk", wr, l - er, cw, w - sr);
            self.put_box(&mut lay, "garth", gs0, gs1, gt0, gt1);
            let refectory = self.put_box(&mut lay, "refectory", wr, l - er, w - sr, w);
            let chapter = self.put_box(&mut lay, "chapter house", l - er, l, cw, cw + ch);
            lay.rooms[chapter as usize].shape = shape::CHAMFERED;
            let dorter = self.put_box(&mut lay, "dorter", l - er, l, cw + ch, w);
            let _ = (church, scr, kitchen, walk, refectory, dorter);
            // (The entrance hall through to the walk, the church off the walk's west end, the garth
            // off the walk across from it, the scriptorium, the refectory (and the kitchen through it),
            // the chapter house on its axis and the dorter off the walk.)
            let (gm_t, gm_s) = ((gt0 + gt1 - 1) as isize / 2, (gs0 + gs1 - 1) as isize / 2);
            self.join(&mut lay, (wri - 1, cwi), (wri, cwi));
            self.join(&mut lay, (wri + 1, cwi), (wri + 1, cwi - 1));
            self.join(&mut lay, (wri, gm_t), (wri + 1, gm_t));
            self.join(&mut lay, (gm_s, (w - sr) as isize - 1), (gm_s, (w - sr) as isize - 2));
            self.join(&mut lay, (wri - 1, cwi + 3), (wri, cwi + 3));
            self.join(&mut lay, (wri + 1, wi - sri - 1), (wri + 1, wi - sri));
            self.join(&mut lay, (wri - 1, wi - sri + 1), (wri, wi - sri + 1));
            self.join(&mut lay, (li - eri - 1, cwi + chi / 2), (li - eri, cwi + chi / 2));
            if cw + ch < w - sr {
                self.join(&mut lay, (li - eri - 1, cwi + chi), (li - eri, cwi + chi));
            } else {
                self.join(&mut lay, (li - eri - 1, wi - sri + 1), (li - eri, wi - sri + 1));
            }
            // The garth's well and walks.
            for (kind, (s0, s1, t0, t1)) in self.garth_pieces() {
                let (a, c) = (self.f.grid(s0 as isize, t0 as isize), self.f.grid(s1 as isize - 1, t1 as isize - 1));
                if let Some(it) = building_item(kind, a.0.min(c.0) as usize, a.1.min(c.1) as usize, (a.0 - c.0).unsigned_abs() + 1, (a.1 - c.1).unsigned_abs() + 1) {
                    lay.items.push(it);
                }
            }
            // The front door from the street into the entrance hall.
            let mut e = Vec::new();
            for t in [cwi, cwi + 1] {
                let (p, q) = (self.f.grid(0, t), self.f.grid(-1, t));
                e.push((side(p, q), p.0 as usize, p.1 as usize));
            }
            lay.hints.front = Some((hall, e));
            halls.push(frame(0, 0, l, cw));
            halls.push(frame(wr, w - sr, l - er - wr, sr));
            return Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name: "Ground floor", roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls });
        }
        // Above: the church's height open; the ranges' rooms round the landing over the entrance hall.
        self.put_box(&mut lay, OPEN_BELOW, 0, l, 0, cw);
        self.put_box(&mut lay, "landing", 0, wr, cw, cw + 2);
        self.put_box(&mut lay, "library", 0, wr, cw + 2, w - sr);
        self.put_box(&mut lay, "chamber", 0, wr, w - sr, w);
        self.put_box(&mut lay, "infirmary", wr, l - er, w - sr, w);
        self.put_box(&mut lay, "dorter", l - er, l, cw + ch, w);
        self.put_box(&mut lay, "chamber", l - er, l, cw, cw + ch);
        self.join(&mut lay, (1, cwi + 1), (1, cwi + 2));
        self.join(&mut lay, (1, wi - sri - 1), (1, wi - sri));
        self.join(&mut lay, (wri - 1, wi - sri + 1), (wri, wi - sri + 1));
        self.join(&mut lay, (li - eri - 1, wi - sri + 1), (li - eri, wi - sri + 1));
        self.join(&mut lay, (li - 2, cwi + chi), (li - 2, cwi + chi - 1));
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name: "Upper floor", roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls })
    }
}
