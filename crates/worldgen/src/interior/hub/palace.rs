//! The capital's palace: a grand palace symmetric about its axis, square to its front (the side of
//! its footprint nearest a street, an odd number of squares long where it can be: `town::palace`).
//! Round a court where it is 17 squares across and 20 deep or more (its court 7 squares across or
//! more each way): a front range, a back range and a range down each side; else a front range and a
//! back one. Every room has its mirror image across the axis (their kinds may differ), and the state
//! rooms open one into the next:
//! - the ground floor: the entrance hall on the axis from the front door, the grand stair rising in
//!   it on the axis, a guard room each side of it and a room in each corner; the court beyond (a
//!   fountain in its middle); the service wing down one side (servants' hall, pantry, kitchen), the
//!   royal chapel down the other; the banqueting hall across the back on the axis, entered from the
//!   court, the buttery between it and the kitchen, a vestry by the chapel;
//! - the state floor: the stair's landing, a salon over the entrance, a guard room each side, an
//!   antechamber in each corner, then the audience chamber (the presence chamber) down one side and
//!   the ballroom down the other, a guard room in each back corner and the throne room across the back
//!   on the axis, its throne dais against its back wall: an enfilade from the landing round to it both
//!   ways, its doors on lines a square in from the ranges' inner walls;
//! - the floor above: the long gallery across the front, the royal apartments round a corridor that
//!   rings the court (the great chamber and the bedchamber at the side ranges' front ends, the
//!   floor's biggest rooms but the gallery, small wardrobes beside them in the front range's corners,
//!   studies by the stair, then smaller guest chambers, a library, studies);
//! - the attic: servants' quarters and stores round the same corridor.

use super::kit::{Lay, side};
use super::{Frame, Kind, Laid, Piece, Plan};
use crate::interior::{Shell, building_item, hall};
use crate::town::{self, geom::P};

/// A palace's zones in its frame (s from its front, t across it from the left).
#[derive(Clone, Debug)]
pub(super) struct Zones {
    /// The front range's rows (0..front), the back range's (back.. the body's depth), the side
    /// ranges' width; with no court the back range follows the front (`back == front`).
    pub(super) front: usize,
    pub(super) back: usize,
    pub(super) side: usize,
    pub(super) court: bool,
    /// The middle bay's columns about the axis (the entrance hall, the landing).
    pub(super) mid: (usize, usize),
    /// The front range's corner rooms' width (0: none; the guard rooms reach the corners).
    pub(super) corner: usize,
    /// The main stair's rows (s0..s1).
    pub(super) stair: (usize, usize),
}

/// The capital's palace's plan: its frame, its zones, the grand stair on its axis. None for a
/// footprint that isn't a rectangle 13 squares by 11 or more (laid out as an older palace).
/// `front`: its front's middle as the town laid it out (grid units), else picked here.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan(b: &town::Building, sh: &Shell, piece: Vec<Piece>, seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool, front: Option<P>) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let body: Vec<usize> = (0..nx * ny).filter(|&k| piece[k] == Piece::Body && sh.whole[k]).collect();
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
    // The front: a side it stands round a court from, then one whose door on the axis opens onto
    // open ground, then one with room before it for its forecourt (2 squares from the street or
    // more: `town::palace` draws the capital's back from the street for it), then a long side,
    // nearest a street (on one: within 80 ft), an odd number of squares long first; 13 squares long
    // or more, 11 deep.
    let fronts = [
        (bw, bh, [0.5 * (x0 + x1 + 1) as f64, y0 as f64]),
        (bh, bw, [(x1 + 1) as f64, 0.5 * (y0 + y1 + 1) as f64]),
        (bw, bh, [0.5 * (x0 + x1 + 1) as f64, (y1 + 1) as f64]),
        (bh, bw, [x0 as f64, 0.5 * (y0 + y1 + 1) as f64]),
    ];
    let (xi, yi) = ((x0 as isize, x1 as isize), (y0 as isize, y1 as isize));
    let frame = |k: usize| {
        let (w, d) = (fronts[k].0, fronts[k].1);
        match k {
            0 => Frame { o: (xi.0, yi.0), ds: (0, 1), dt: (1, 0), len: d, wid: w },
            1 => Frame { o: (xi.1, yi.0), ds: (-1, 0), dt: (0, 1), len: d, wid: w },
            2 => Frame { o: (xi.1, yi.1), ds: (0, -1), dt: (-1, 0), len: d, wid: w },
            _ => Frame { o: (xi.0, yi.1), ds: (1, 0), dt: (0, -1), len: d, wid: w },
        }
    };
    // (Open ground before the axis's square, or the two middle ones' on an even front.)
    let open = |k: usize| {
        let (f, w) = (frame(k), fronts[k].0 as isize);
        [(w - 1) / 2, w / 2].iter().all(|&t| {
            let (i, j) = f.grid(-1, t);
            ground(i, j)
        })
    };
    let fits = |k: &usize| fronts[*k].0 >= 13 && fronts[*k].1 >= 11;
    let k = match front {
        Some(h) => (0..4).filter(fits).min_by(|&a, &c| town::geom::dist(fronts[a].2, h).total_cmp(&town::geom::dist(fronts[c].2, h)).then(a.cmp(&c)))?,
        None => (0..4).filter(fits).min_by_key(|&k| {
            let d = near(fronts[k].2);
            (!zones(fronts[k].0, fronts[k].1).court, !open(k), d < 9.5, fronts[k].1 > fronts[k].0, d >= 80.0, fronts[k].0 % 2 == 0, crate::core::round(d) as i64, k)
        })?,
    };
    let (w, d) = (fronts[k].0, fronts[k].1);
    let f = frame(k);
    let z = zones(w, d);
    let (s0, s1) = z.stair;
    let sw = if w % 2 == 1 { 3 } else { 2 };
    let (a, c) = (f.grid(s0 as isize, ((w - sw) / 2) as isize), f.grid(s1 as isize - 1, ((w + sw) / 2) as isize - 1));
    let stairs = [a.0.min(c.0) as usize, a.1.min(c.1) as usize, (a.0 - c.0).unsigned_abs() + 1, (a.1 - c.1).unsigned_abs() + 1];
    Some(Plan {
        kind: Kind::Palace,
        nx,
        ny,
        f,
        piece,
        service: 0,
        screens: false,
        dz: 0,
        hall_t: (0, w),
        land_t: z.mid,
        stairs,
        hall_z: 0,
        open: false,
        floors: b.floors.max(1) as i8,
        wing_floors: 1,
        fore_floors: 1,
        pieces: false,
        top: Vec::new(),
        wing_tops: Vec::new(),
        wings: Vec::new(),
        fore: Vec::new(),
        tower_top: Vec::new(),
        tower_sq: Vec::new(),
        overlap: Vec::new(),
        ring: None,
        seed,
        pending: Vec::new(),
        chapel: true,
        ends: [true, true],
        side: -1,
        oratory: false,
        last: Vec::new(),
        bay: None,
        small: false,
        pal: Some(z),
        tem: None,
        mon: None,
        civ: None,
        inn: None,
        spiral: None,
        rich: false,
    })
}

/// A palace's zones for a body `w` squares across its front and `d` deep: the front range about 0.3 of
/// the depth (7 to 9 rows, deep enough for the stair's landing and the long gallery 3 squares wide
/// before it), the back range about a third (6 to 9), the side ranges a quarter of the width (5 to 8
/// squares; 4 on a front under 19 squares; 4 to 7 with no court), the middle bay 5 or 7 squares (4
/// or 6 on an even front), the stair 3 rows long where the front range is 8 deep or more, else 2,
/// ending a row short of its back wall. Round a court only where it is 7 squares across or more
/// each way.
fn zones(w: usize, d: usize) -> Zones {
    let r = |x: f64| crate::core::round(x) as usize;
    let mut court = w >= 17 && d >= 17;
    let (mut front, mut back, mut side) = (r(0.3 * d as f64).clamp(7, 9), r(0.32 * d as f64).clamp(6, 9), r(0.24 * w as f64).clamp(5, 8));
    if w < 19 {
        side = 4;
    }
    while court && d < front + back + COURT_LEAST && (back > 6 || front > 7) {
        if back > 6 { back -= 1 } else { front -= 1 }
    }
    let e = if w % 2 == 1 { if w >= 23 { 7 } else { 5 } } else if w >= 22 { 6 } else { 4 };
    let mid = ((w - e) / 2, (w + e) / 2);
    // (A court needs its depth, and corner rooms beside the guard rooms, and its width.)
    if d < front + back + COURT_LEAST || mid.0 < side + 2 || w < 2 * side + COURT_LEAST {
        court = false;
    }
    if !court {
        front = (d / 2).clamp(7, 9);
        side = r(0.24 * w as f64).clamp(4, 7);
    }
    // (Corner rooms where the guard rooms beside them keep 3 squares across.)
    let corner = if mid.0 >= side + 3 { side } else { 0 };
    let ls = if front >= 8 { 3 } else { 2 };
    Zones { front, back: if court { d - back } else { front }, side, court, mid, corner, stair: (front - 1 - ls, front - 1) }
}

/// The least a palace's court is across and deep (squares): a court, not a light well.
const COURT_LEAST: usize = 7;

/// Whether a capital's palace `w` squares across its front and `d` deep stands round a court
/// (`zones`).
pub(crate) fn courted(w: usize, d: usize) -> bool {
    zones(w, d).court
}

/// The side code of `Item::back` (0 north, 1 east, 2 south, 3 west) for a step in grid axes.
fn side_code(d: (isize, isize)) -> u8 {
    match d {
        (0, -1) => 0,
        (1, 0) => 1,
        (0, 1) => 2,
        _ => 3,
    }
}

/// A piece in the court (`Plan::court_pieces`): its kind, its box in the palace's frame (s0..s1,
/// t0..t1), the frame step its back is toward.
pub type CourtPiece = (&'static str, (usize, usize, usize, usize), Option<(isize, isize)>);

impl Plan {
    /// The court's pieces, as the ground floor has them and the battlemap draws them
    /// (`crate::interior::court_of`): the fountain in its middle (3 squares across in a wide court, 2
    /// on an even axis, else 1), gravel walks as wide as it along the axis from the entrance hall's
    /// door to the banqueting hall's and across the court through it, and a bench each side of the
    /// axis's walk before the fountain and after it, its back to the lawn. None with no court.
    pub fn court_pieces(&self) -> Vec<CourtPiece> {
        let Some(zn) = self.pal.as_ref().filter(|z| z.court) else { return Vec::new() };
        let (w, fr, bk, sd) = (self.f.wid, zn.front, zn.back, zn.side);
        let (cw, cd) = (w - 2 * sd, bk - fr);
        let fw = if cw % 2 == 0 { 2 } else if cw >= 9 && cd >= 7 { 3 } else { 1 };
        let fd = fw.min(cd.saturating_sub(4)).max(1);
        let (s0, t0) = (fr + (cd - fd) / 2, (w - fw) / 2);
        let (s1, t1) = (s0 + fd, t0 + fw);
        let mut out: Vec<CourtPiece> = vec![("fountain", (s0, s1, t0, t1), None)];
        for (a, b) in [(fr, s0), (s1, bk)] {
            if b > a {
                out.push(("garden_path", (a, b, t0, t1), None));
            }
        }
        for (a, b) in [(sd, t0), (t1, w - sd)] {
            if b > a {
                out.push(("garden_path", (s0, s1, a, b), None));
            }
        }
        // (Beside the axis's walk on the lawn, clear of the court's side walls by a square.)
        if t0 >= sd + 2 {
            for (a, b) in [(s0.saturating_sub(2).max(fr), s0), (s1, (s1 + 2).min(bk))] {
                if b > a {
                    out.push(("bench", (a, b, t0 - 1, t0), Some((0, -1))));
                    out.push(("bench", (a, b, t1, t1 + 1), Some((0, 1))));
                }
            }
        }
        out
    }

    /// A room of kind `kind` over the body's squares s0..s1, t0..t1.
    pub(super) fn put_box(&self, lay: &mut Lay, kind: &'static str, s0: usize, s1: usize, t0: usize, t1: usize) -> i16 {
        let r = lay.room(kind);
        self.fill(lay, s0, s1, t0, t1, r);
        r
    }

    /// A room over s0..s1, t0..t1 (kind `a`) and its mirror image across the axis (kind `b`).
    fn put_pair(&self, lay: &mut Lay, (a, b): (&'static str, &'static str), s0: usize, s1: usize, t0: usize, t1: usize) -> (i16, i16) {
        let w = self.f.wid;
        (self.put_box(lay, a, s0, s1, t0, t1), self.put_box(lay, b, s0, s1, w - t1, w - t0))
    }

    /// Box s0..s1, t0..t1 (on the low-t side of the axis) and its mirror image, cut along its longer
    /// side into as few rooms as leaves each no longer than 2.5 times its width nor over `most` squares;
    /// the rooms' kinds in turn from `kinds` (the last repeated), each with its mirror's; `first`: the
    /// room holding this square (s, t) takes the first kind, the others the rest in turn (where the
    /// box is cut or not: a door the enfilade goes through there).
    #[allow(clippy::too_many_arguments)]
    fn put_rooms(&self, lay: &mut Lay, kinds: &[(&'static str, &'static str)], first: Option<(usize, usize)>, s0: usize, s1: usize, t0: usize, t1: usize, most: usize) {
        let (ls, lt) = (s1 - s0, t1 - t0);
        if ls == 0 || lt == 0 {
            return;
        }
        let (long, short) = (ls.max(lt), ls.min(lt));
        let n = (1..=long).find(|&n| {
            let part = long.div_ceil(n);
            2 * part <= 5 * short && part * short <= most
        });
        let n = n.unwrap_or(long / 2).max(1);
        let along = ls >= lt;
        let held = first.and_then(|(s, t)| (0..n).find(|&k| (long * k / n..long * (k + 1) / n).contains(&if along { s.wrapping_sub(s0) } else { t.wrapping_sub(t0) })));
        for k in 0..n {
            let (a, b) = (long * k / n, long * (k + 1) / n);
            let i = match held {
                Some(h) if k == h => 0,
                Some(h) => k + (k < h) as usize,
                None => k,
            };
            let kind = kinds[i.min(kinds.len() - 1)];
            if along {
                self.put_pair(lay, kind, s0 + a, s0 + b, t0, t1);
            } else {
                self.put_pair(lay, kind, s0, s1, t0 + a, t0 + b);
            }
        }
    }

    /// Box s0..s1, t0..t1 about the axis cut across it into an odd number of rooms, mirrored (the
    /// middle one `mid`, the others `kinds` from the axis out), each no longer than 2.5:1 nor over
    /// `most` squares where the box allows.
    #[allow(clippy::too_many_arguments)]
    fn put_middle(&self, lay: &mut Lay, mid: &'static str, kinds: &[(&'static str, &'static str)], s0: usize, s1: usize, t0: usize, t1: usize, most: usize) {
        let (ls, lt) = (s1 - s0, t1 - t0);
        let ok = |x: usize| x > 0 && 2 * x <= 5 * ls && 2 * ls <= 5 * x && x * ls <= most;
        // (The middle room's width, then the pairs' each side: 1, 3 or 5 rooms across.)
        // (Failing that, as many as leaves the biggest no bigger than `most` squares, the rooms 2
        // wide.)
        let mut best = (lt, 1);
        let mut fit = None;
        for n in [1usize, 3, 5, 7, 9] {
            let part = lt / n;
            let m = lt - part * (n - 1);
            if part < 2 || m < 2 {
                break;
            }
            if ok(part) && ok(m) {
                best = (m, n);
                fit = None;
                break;
            }
            if fit.is_none() && part.max(m) * ls <= most {
                fit = Some((m, n));
            }
        }
        if let Some(f) = fit {
            best = f;
        }
        let (m, n) = best;
        let part = if n > 1 { (lt - m) / (n - 1) } else { 0 };
        let c0 = t0 + (lt - m) / 2;
        self.put_box(lay, mid, s0, s1, c0, c0 + m);
        for k in 0..(n - 1) / 2 {
            let b = c0 - k * part;
            self.put_pair(lay, kinds[k.min(kinds.len().saturating_sub(1))], s0, s1, b - part, b);
        }
    }

    /// A door between the rooms at frame squares `a` and `b` (side by side) whatever else does.
    pub(super) fn join(&self, lay: &mut Lay, a: (isize, isize), b: (isize, isize)) {
        let (Some(ka), Some(kb)) = (self.at(a.0, a.1), self.at(b.0, b.1)) else { return };
        let (ra, rb) = (lay.cells[ka], lay.cells[kb]);
        if ra < 0 || rb < 0 || ra == rb {
            return;
        }
        self.door(lay, a, b);
        let p = (ra.min(rb), ra.max(rb));
        if !lay.hints.link.contains(&p) {
            lay.hints.link.push(p);
        }
    }

    /// `join` and its mirror image across the axis.
    fn join_pair(&self, lay: &mut Lay, a: (isize, isize), b: (isize, isize)) {
        let w = self.f.wid as isize;
        self.join(lay, a, b);
        self.join(lay, (a.0, w - 1 - a.1), (b.0, w - 1 - b.1));
    }

    /// A box of the frame (s0..s1, t0..t1) as a hall's frame (`hall::Frame`), entered at its s0 end.
    fn hall_box(&self, s0: usize, s1: usize, t0: usize, t1: usize) -> hall::Frame {
        hall::Frame { o: self.f.grid(s0 as isize, t0 as isize), ds: self.f.ds, dt: self.f.dt, len: s1 - s0, wid: t1 - t0 }
    }

    /// The capital's palace's floor `z` (`palace` above): the ground floor, the state floor, the
    /// royal floor and the attic round the court (or the back range).
    pub(super) fn palace_level(&mut self, z: i8, mut lay: Lay) -> Laid {
        let zn = self.pal.clone().expect("a palace");
        let (d, w) = (self.f.len, self.f.wid);
        let (fr, bk, sd, cn) = (zn.front, zn.back, zn.side, zn.corner);
        let (m0, m1) = zn.mid;
        // (The axis's column, the left of the two middle ones on an even front.)
        let ax = ((w - 1) / 2) as isize;
        let (fri, bki, sdi) = (fr as isize, bk as isize, sd as isize);
        // (Doors on the enfilade a square in from a range's inner wall.)
        let ce = sdi - 2;
        // (The rows before the stair's landing: the salon, the long gallery.)
        let gd = zn.stair.0 - 1;
        let top0 = if gd >= 2 { gd } else { 0 };
        // (Over the court nothing but the sky: no room takes its squares above the ground floor.)
        if zn.court {
            for s in fr..bk {
                for t in sd..w - sd {
                    if let Some(k) = self.at(s as isize, t as isize) {
                        self.piece[k] = if z == 0 { Piece::Body } else { Piece::Out };
                    }
                }
            }
        }
        let mut halls = Vec::new();
        let mut name = "Ground floor";
        match z {
            0 => {
                let hall = self.put_box(&mut lay, "entrance hall", 0, fr, m0, m1);
                // (In a deep front range, an armory before each guard room.)
                self.put_rooms(&mut lay, &[("guardroom", "guardroom"), ("armory", "armory")], Some((fr / 2, m0 - 1)), 0, fr, cn, m0, 40);
                if cn > 0 {
                    self.put_rooms(&mut lay, &[("office", "chamber"), ("archive", "study")], None, 0, fr, 0, cn, 40);
                }
                if zn.court {
                    self.put_box(&mut lay, "courtyard", fr, bk, sd, w - sd);
                    // The service wing down the left side: the servants' hall by the front, the kitchen
                    // by the back range (a pantry between them in a long one); the royal chapel down
                    // the right.
                    let deep = bk - fr;
                    if deep >= 8 {
                        let (a, b) = (fr + deep / 3, fr + 2 * deep / 3);
                        self.put_box(&mut lay, "servants' hall", fr, a, 0, sd);
                        self.put_box(&mut lay, "pantry", a, b, 0, sd);
                        self.put_box(&mut lay, "kitchen", b, bk, 0, sd);
                    } else {
                        self.put_box(&mut lay, "servants' hall", fr, fr + deep / 2, 0, sd);
                        self.put_box(&mut lay, "kitchen", fr + deep / 2, bk, 0, sd);
                    }
                    self.put_box(&mut lay, "royal chapel", fr, bk, w - sd, w);
                    self.put_box(&mut lay, "banqueting hall", bk, d, sd, w - sd);
                    self.put_rooms(&mut lay, &[("buttery", "vestry"), ("storeroom", "storeroom")], Some((bk + 1, sd - 1)), bk, d, 0, sd, 40);
                    self.join(&mut lay, (fri - 1, ax), (fri, ax));
                    self.join(&mut lay, (bki - 1, ax), (bki, ax));
                    // (The kitchen to the buttery to the hall; the chapel to its vestry.)
                    self.join(&mut lay, (bki - 1, sdi / 2), (bki, sdi / 2));
                    self.join(&mut lay, (bki + 1, sdi - 1), (bki + 1, sdi));
                    self.join(&mut lay, (bki - 1, w as isize - 1 - sdi / 2), (bki, w as isize - 1 - sdi / 2));
                    // The court's fountain, walks and benches (`court_pieces`).
                    for (kind, (s0, s1, t0, t1), back) in self.court_pieces() {
                        let (a, c) = (self.f.grid(s0 as isize, t0 as isize), self.f.grid(s1 as isize - 1, t1 as isize - 1));
                        if let Some(mut it) = building_item(kind, a.0.min(c.0) as usize, a.1.min(c.1) as usize, (a.0 - c.0).unsigned_abs() + 1, (a.1 - c.1).unsigned_abs() + 1) {
                            it.back = back.map(|d| side_code((self.f.ds.0 * d.0 + self.f.dt.0 * d.1, self.f.ds.1 * d.0 + self.f.dt.1 * d.1)));
                            lay.items.push(it);
                        }
                    }
                } else {
                    self.put_box(&mut lay, "banqueting hall", fr, d, sd, w - sd);
                    self.put_box(&mut lay, "kitchen", fr, d, 0, sd);
                    self.put_box(&mut lay, "royal chapel", fr, d, w - sd, w);
                    self.join(&mut lay, (fri - 1, ax), (fri, ax));
                    self.join(&mut lay, ((fri + d as isize) / 2, sdi - 1), ((fri + d as isize) / 2, sdi));
                }
                halls.push(self.hall_box(bk, d, sd, w - sd));
                // (The guard rooms onto the entrance hall, the corner rooms through them.)
                self.join_pair(&mut lay, (fri / 2, m0 as isize - 1), (fri / 2, m0 as isize));
                if cn > 0 {
                    self.join_pair(&mut lay, (fri / 2, cn as isize - 1), (fri / 2, cn as isize));
                }
                // The front door on the axis.
                let mut e = Vec::new();
                for t in [ax, w as isize - 1 - ax, ax - 1, w as isize - ax] {
                    let (p, q) = (self.f.grid(0, t), self.f.grid(-1, t));
                    e.push((side(p, q), p.0 as usize, p.1 as usize));
                }
                lay.hints.front = Some((hall, e));
            }
            1 => {
                name = "State floor";
                self.put_box(&mut lay, "landing", top0, fr, m0, m1);
                if gd >= 2 {
                    self.put_box(&mut lay, "salon", 0, gd, m0, m1);
                    self.join(&mut lay, (gd as isize - 1, ax), (gd as isize, ax));
                }
                // (In a deep front range, a wardrobe before each guard room and a closet before each
                // antechamber, off the enfilade along its back; a guard room wider than deep whole, the
                // enfilade through it from side to side. With no corner rooms, the antechamber across to
                // a square past the enfilade's door into the side range.)
                let ca = if cn > 0 { cn } else { sd - 1 };
                if fr >= m0 - ca {
                    self.put_rooms(&mut lay, &[("guardroom", "guardroom"), ("wardrobe", "wardrobe")], Some((fr - 2, m0 - 1)), 0, fr, ca, m0, 40);
                } else {
                    // (No bigger than 40 squares: the rows before it wardrobes and studies.)
                    let gs = (40 / (m0 - ca)).clamp(3, fr);
                    self.put_pair(&mut lay, ("guardroom", "guardroom"), fr - gs, fr, ca, m0);
                    if gs < fr {
                        self.put_rooms(&mut lay, &[("wardrobe", "wardrobe"), ("study", "study")], None, 0, fr - gs, ca, m0, 40);
                    }
                }
                self.put_rooms(&mut lay, &[("antechamber", "antechamber"), ("study", "study")], Some((fr - 1, sd - 2)), 0, fr, 0, ca, 40);
                let e = fri - 2;
                self.join_pair(&mut lay, (e, m0 as isize - 1), (e, m0 as isize));
                self.join_pair(&mut lay, (e, ca as isize - 1), (e, ca as isize));
                if zn.court {
                    self.put_pair(&mut lay, ("audience chamber", "ballroom"), fr, bk, 0, sd);
                    self.put_box(&mut lay, "throne room", bk, d, sd, w - sd);
                    self.put_rooms(&mut lay, &[("guardroom", "guardroom"), ("wardrobe", "wardrobe")], Some((bk + 1, sd - 1)), bk, d, 0, sd, 40);
                    self.join_pair(&mut lay, (fri - 1, ce), (fri, ce));
                    self.join_pair(&mut lay, (bki - 1, ce), (bki, ce));
                    self.join_pair(&mut lay, (bki + 1, sdi - 1), (bki + 1, sdi));
                    halls.push(self.hall_box(fr, bk, 0, sd));
                } else {
                    self.put_pair(&mut lay, ("audience chamber", "ballroom"), fr, d, 0, sd);
                    self.put_box(&mut lay, "throne room", fr, d, sd, w - sd);
                    self.join_pair(&mut lay, (fri - 1, ce), (fri, ce));
                    self.join_pair(&mut lay, (fri + 1, sdi - 1), (fri + 1, sdi));
                    halls.push(self.hall_box(fr, d, 0, sd));
                }
                halls.push(self.hall_box(bk, d, sd, w - sd));
            }
            _ => {
                let royal = z == 2;
                name = if royal { "Royal floor" } else { "Upper floor" };
                self.put_box(&mut lay, "landing", top0, fr, m0, m1);
                // Across the front: the long gallery (above it, a store over the salon, the rooms beside
                // it running through to the front wall).
                let r0 = if royal { top0 } else { 0 };
                if gd >= 2 {
                    if royal {
                        self.put_box(&mut lay, "long gallery", 0, gd, 0, w);
                    } else {
                        self.put_box(&mut lay, "storeroom", 0, gd, m0, m1);
                    }
                }
                let c = lay.room("corridor");
                let (front_kind, front_more, side_kinds, back_mid, back_kinds, corner_kinds): (_, _, &[(&str, &str)], _, &[(&str, &str)], _) = if royal {
                    (("great chamber", "bedchamber"), ("guest chamber", "wardrobe"), &[("guest chamber", "study")], "library", &[("guest chamber", "guest chamber")], ("study", "study"))
                } else {
                    (("chamber", "chamber"), ("chamber", "chamber"), &[("servants' quarters", "servants' quarters"), ("storeroom", "chamber")], "storeroom", &[("servants' quarters", "servants' quarters")], ("chamber", "storeroom"))
                };
                if zn.court {
                    // A corridor round the court: along the front range's back row from the landing,
                    // down each side range's inner column and along the back range's inner row.
                    self.fill(&mut lay, fr - 1, fr, sd - 1, m0, c);
                    self.fill(&mut lay, fr - 1, fr, m1, w - sd + 1, c);
                    self.fill(&mut lay, fr, bk + 1, sd - 1, sd, c);
                    self.fill(&mut lay, fr, bk + 1, w - sd, w - sd + 1, c);
                    self.fill(&mut lay, bk, bk + 1, sd, w - sd, c);
                    if royal && sd > 4 {
                        // The great chamber and the bedchamber at the side ranges' front ends, the
                        // floor's biggest rooms but the gallery (up to 50 squares, 2.5:1 at most);
                        // the wardrobes beside them in the front range's corners, small, studies by
                        // the stair; guest chambers and the rest smaller than they are.
                        let (sw, sl) = (sd - 1, bk + 1 - fr);
                        let lr = sl.min(5 * sw / 2).min((50 / sw).max(sw)).max(1);
                        // (Rows past them too few for rooms of their own: theirs.)
                        let lr = if sl - lr < 3 { sl } else { lr };
                        let less = (lr * sw).saturating_sub(1).max(9);
                        self.put_pair(&mut lay, front_kind, fr - 1, fr - 1 + lr, 0, sw);
                        self.put_rooms(&mut lay, &[("wardrobe", "wardrobe"), ("study", "study")], Some((fr - 2, 0)), r0, fr - 1, 0, m0, 15.min(less));
                        if lr < sl {
                            self.put_rooms(&mut lay, side_kinds, None, fr - 1 + lr, bk, 0, sw, less);
                        }
                        self.put_rooms(&mut lay, &[corner_kinds, ("guest chamber", "study")], None, bk, d, 0, sw, less);
                        if bk + 1 < d {
                            self.put_middle(&mut lay, back_mid, back_kinds, bk + 1, d, sd - 1, w - sd + 1, less);
                        }
                    } else if royal {
                        // (Side ranges too narrow for them: the great chamber and the bedchamber
                        // across the back range either side of the axis, a study 3 wide between
                        // them on an odd front; the rest smaller.)
                        let sw = sd - 1;
                        let mw = if w % 2 == 1 { 3 } else { 0 };
                        let (bw, bd) = (w / 2 - sw - mw / 2, d - bk - 1);
                        let less = (bw * bd).saturating_sub(1).max(9);
                        if bk + 1 < d {
                            self.put_pair(&mut lay, front_kind, bk + 1, d, sw, sw + bw);
                            if mw > 0 {
                                self.put_box(&mut lay, "study", bk + 1, d, sw + bw, sw + bw + mw);
                            }
                        }
                        self.put_rooms(&mut lay, &[("wardrobe", "wardrobe"), ("study", "study")], Some((fr - 2, 0)), r0, fr - 1, 0, m0, 15.min(less));
                        self.put_rooms(&mut lay, side_kinds, None, fr - 1, bk, 0, sw, less);
                        self.put_rooms(&mut lay, &[corner_kinds, ("guest chamber", "study")], None, bk, d, 0, sw, less);
                    } else {
                        self.put_rooms(&mut lay, &[front_kind, front_more], Some((fr - 2, m0 - 1)), r0, fr - 1, 0, m0, 40);
                        self.put_rooms(&mut lay, side_kinds, None, fr - 1, bk, 0, sd - 1, 40);
                        self.put_rooms(&mut lay, &[corner_kinds, ("wardrobe", "storeroom")], None, bk, d, 0, sd - 1, 40);
                        if bk + 1 < d {
                            self.put_middle(&mut lay, back_mid, back_kinds, bk + 1, d, sd - 1, w - sd + 1, 40);
                        }
                    }
                } else {
                    // A corridor across the back range's first row from the landing.
                    self.fill(&mut lay, fr, fr + 1, 0, w, c);
                    self.put_rooms(&mut lay, &[front_kind, front_more], Some((fr - 1, m0 - 1)), r0, fr, 0, m0, 40);
                    if fr + 1 < d {
                        self.put_pair(&mut lay, corner_kinds, fr + 1, d, 0, sd);
                        self.put_middle(&mut lay, back_mid, back_kinds, fr + 1, d, sd, w - sd, 40);
                    }
                }
            }
        }
        // (Squares no box took: the room beside them.)
        let mut free = Vec::new();
        self.leftovers(&mut lay, &mut free);
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name, roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls }
    }
}
