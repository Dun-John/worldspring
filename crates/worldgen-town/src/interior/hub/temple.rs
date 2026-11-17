//! Temples by zones, on their pieces (`town::temple`), along their axis from the door (s) to the
//! apse, across it (t, the body's width):
//! - a big temple, a cross: the nave (a way about the floor) from the door through the crossing to
//!   the sanctuary, the aisles either side of it and the transept's arms off the crossing; in each
//!   arm's end toward the apse a side chapel, in the low-t arm's other end the vestry, in the other
//!   arm's the stairs down to the crypt by the crossing; the sanctuary across the body's end,
//!   rounded by the apse. An altar rail between the sanctuary and the nave and between a side chapel
//!   and the nave (`super::super::railed`), its gate on the axis;
//! - a small temple: a single hall, its far end rounded by the apse, the stairs down to the crypt
//!   along its wall by the door, a square in from it (clear of the outer wall as drawn); a town's
//!   shrine on a lot big enough the same, smaller, its one room the shrine.
//!
//! The front door is on the axis at the door's end, else in a long wall by it (a porch). Each room's
//! frame for its furnishing (`hall`): the nave from the door to the sanctuary, the sanctuary, each
//! side chapel toward the apse, the hall from its door to its apse.

use super::kit::{Lay, side};
use super::{Frame, Kind, Laid, Piece, Plan};
use crate::interior::{Shell, hall, shape};
use crate::town::{self, PartKind, geom::{self, P}};

/// A temple's zones in its frame (squares from its door; t across the body, the arms beyond it).
#[derive(Clone, Copy, Debug)]
pub(super) struct Zones {
    /// The cross's rows and arms (`town::temple::Cross`), none for a small temple's hall.
    pub(super) cross: Option<town::temple::Cross>,
    /// The apse's shape code (`shape::APSE_*`) on the grid.
    pub(super) apse: u8,
    /// A shrine's hall (its one room the shrine).
    pub(super) shrine: bool,
}

/// The rows a side chapel takes at its arm's end toward the apse.
pub const CHAPEL_ROWS: usize = 2;

/// A temple's plan on its pieces: its frame from its body (its door's end, the apse's way), the cross
/// measured from them, every square of the frame and the arms inside the building (else None, laid
/// out as other buildings are), the stairs down to the crypt.
pub(super) fn plan(b: &town::Building, sh: &Shell, kind: Kind, seed: u64, grid_p: &dyn Fn(P) -> P) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let body = &b.parts.first().filter(|p| p.kind == PartKind::Body && p.poly.len() == 4)?.poly;
    let g: Vec<P> = body.iter().map(|q| grid_p(*q)).collect();
    let dir = |d: P| -> (isize, isize) {
        if d[0].abs() >= d[1].abs() { (if d[0] > 0.0 { 1 } else { -1 }, 0) } else { (0, if d[1] > 0.0 { 1 } else { -1 }) }
    };
    let (ds, dt) = (dir(geom::sub(g[1], g[0])), dir(geom::sub(g[3], g[0])));
    let c = geom::add(g[0], [0.5 * (ds.0 + dt.0) as f64, 0.5 * (ds.1 + dt.1) as f64]);
    let o = (crate::core::floor(c[0]) as isize, crate::core::floor(c[1]) as isize);
    let wid = crate::core::round(geom::dist(g[0], g[3])) as usize;
    let len = crate::core::round(geom::dist(g[0], g[1]) + 0.5 * wid as f64) as usize;
    let cross = town::temple::cross_of(b);
    if (kind == Kind::Church) != cross.is_some() || cross.is_some_and(|x| x.len() != len || x.width != wid) || wid < 4 {
        return None;
    }
    let f = Frame { o, ds, dt, len, wid };
    let apse = match ds {
        (1, 0) => shape::APSE_E,
        (-1, 0) => shape::APSE_W,
        (0, 1) => shape::APSE_S,
        _ => shape::APSE_N,
    };
    // (Every square of the body and the arms inside, but the apse's cut corners; nothing else.)
    let mut piece = vec![Piece::Out; nx * ny];
    let at = |s: isize, t: isize| {
        let (i, j) = f.grid(s, t);
        (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    let mut want = vec![false; nx * ny];
    let lo = cross.map_or(0, |x| x.arm as isize);
    for s in 0..len as isize {
        for t in -lo..(wid as isize + lo) {
            let arm = t < 0 || t >= wid as isize;
            if arm && !cross.is_some_and(|x| s >= x.nave as isize && s < (x.nave + x.transept) as isize) {
                continue;
            }
            let Some(k) = at(s, t) else { return None };
            want[k] = true;
        }
    }
    let sanct = cross.map_or(len, |x| x.sanctuary);
    let bx = {
        let (a, e) = (f.grid((len - sanct) as isize, 0), f.grid(len as isize - 1, wid as isize - 1));
        [a.0.min(e.0) as usize, a.1.min(e.1) as usize, (a.0 - e.0).unsigned_abs() + 1, (a.1 - e.1).unsigned_abs() + 1]
    };
    let round = shape::shape_cells(bx, apse);
    for (q, &inn) in round.iter().enumerate() {
        want[(bx[1] + q / bx[2]) * nx + bx[0] + q % bx[2]] = inn;
    }
    if (0..nx * ny).any(|k| want[k] != sh.inside[k]) {
        return None;
    }
    for k in 0..nx * ny {
        if want[k] {
            piece[k] = Piece::Body;
        }
    }
    // The stairs down to the crypt, on whole squares (clear of the outer wall as drawn): two squares
    // across the high-t arm's opening off the crossing (an arm 3 squares out or more), else a square
    // there, clear of the crossing's first row (the screen's); a hall's along a long wall a square in from
    // it, by the door (else a square there).
    let w = wid as isize;
    let spots: Vec<[(isize, isize); 2]> = match cross {
        Some(x) => {
            let n0 = x.nave as isize;
            vec![[(n0 + 1, w), (n0 + 1, w + 1)], [(n0 + 1, w), (n0 + 1, w)], [(n0 + 1, w - 1), (n0 + 1, w)]]
        }
        None => vec![[(1, 1), (2, 1)], [(1, w - 2), (2, w - 2)], [(1, 1), (1, 1)]],
    };
    let stairs = spots.into_iter().find_map(|[a, e]| {
        let (a, e) = (at(a.0, a.1)?, at(e.0, e.1)?);
        (sh.whole[a] && sh.whole[e]).then(|| {
            let (i0, j0, i1, j1) = (a % nx, a / nx, e % nx, e / nx);
            [i0.min(i1), j0.min(j1), i0.abs_diff(i1) + 1, j0.abs_diff(j1) + 1]
        })
    });
    let stairs = stairs?;
    let mut p = Plan::bare(kind, nx, ny, f, piece, stairs, b.floors.max(1) as i8, seed);
    p.tem = Some(Zones { cross, apse, shrine: town::temple::is_shrine(b) });
    Some(p)
}

impl Plan {
    /// A temple's zones: its cross (none for a hall) and its apse's shape.
    pub fn temple(&self) -> Option<(Option<town::temple::Cross>, u8)> {
        self.tem.map(|z| (z.cross, z.apse))
    }

    /// A room over the frame's squares s0..s1, t0..t1 (t may reach into the arms).
    fn temple_box(&self, lay: &mut Lay, r: i16, s0: isize, s1: isize, t0: isize, t1: isize) {
        for s in s0..s1 {
            for t in t0..t1 {
                if let Some(k) = self.at(s, t).filter(|&k| self.piece[k] == Piece::Body) {
                    lay.cells[k] = r;
                }
            }
        }
    }

    /// A temple's ground floor (`temple` above); its other floors as other buildings' are.
    pub(super) fn temple_level(&mut self, z: i8, mut lay: Lay) -> Option<Laid> {
        if z != 0 {
            return None;
        }
        let zn = self.tem?;
        let (len, w) = (self.f.len as isize, self.f.wid as isize);
        let ax = (w - 1) / 2;
        let mut halls = Vec::new();
        let nave = lay.room(if zn.shrine { "shrine" } else { "nave" });
        let frame = |s0: isize, t0: isize, l: usize, wd: usize| hall::Frame { o: self.f.grid(s0, t0), ds: self.f.ds, dt: self.f.dt, len: l, wid: wd };
        match zn.cross {
            Some(x) => {
                let (n0, n1, arm) = (x.nave as isize, (x.nave + x.transept) as isize, x.arm as isize);
                let c0 = n1 - CHAPEL_ROWS as isize;
                let san = len - x.sanctuary as isize;
                self.temple_box(&mut lay, nave, 0, san, 0, w);
                self.temple_box(&mut lay, nave, n0, c0, w, w + arm);
                let vestry = lay.room("vestry");
                self.temple_box(&mut lay, vestry, n0, c0, -arm, 0);
                for t0 in [-arm, w] {
                    let r = lay.room("side chapel");
                    self.temple_box(&mut lay, r, c0, n1, t0, t0 + arm);
                    halls.push(frame(c0, t0, CHAPEL_ROWS, x.arm));
                }
                let r = lay.room("sanctuary");
                lay.rooms[r as usize].shape = zn.apse;
                self.temple_box(&mut lay, r, san, len, 0, w);
                // (The vestry off the crossing (clear of the screen's row), each chapel's gate in its
                // rail by the crossing, the sanctuary's on the axis.)
                self.join(&mut lay, (n0 + 1, -1), (n0 + 1, 0));
                self.join(&mut lay, (c0, -1), (c0, 0));
                self.join(&mut lay, (c0, w), (c0, w - 1));
                self.join(&mut lay, (san - 1, ax), (san, ax));
                halls.insert(0, frame(0, 0, san as usize, w as usize));
                halls.insert(1, frame(san, 0, x.sanctuary, w as usize));
            }
            None => {
                lay.rooms[nave as usize].shape = zn.apse;
                self.temple_box(&mut lay, nave, 0, len, 0, w);
                halls.push(frame(0, 0, len as usize, w as usize));
            }
        }
        // The front door: on the axis at the door's end, else in a long wall by it.
        let mut e = Vec::new();
        for t in [ax, w - 1 - ax] {
            let (p, q) = (self.f.grid(0, t), self.f.grid(-1, t));
            e.push((side(p, q), p.0 as usize, p.1 as usize));
        }
        for s in 1..3 {
            for (t, out) in [(0, -1), (w - 1, w)] {
                let (p, q) = (self.f.grid(s, t), self.f.grid(s, out));
                e.push((side(p, q), p.0 as usize, p.1 as usize));
            }
        }
        lay.hints.front = Some((nave, e));
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: Some("nave"), name: "Ground floor", roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls })
    }
}
