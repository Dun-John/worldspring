//! The room grower (tools for the themed planners): rooms grown door to door from the open walls of rooms already
//! placed, from templates (rect, round, octagon, cross, apse, corridor), mirrored about the level's axis when asked (a
//! symmetry deck: a room and its mirror image put down together, or one room centred on the axis); colonnades down
//! long rooms; the way down; water by room (`flood_room`, laid once the locks are in: `kit::water_at`). Rooms grown
//! against each other share walls: `kit::guard` opens some of those as loops (4+ hops apart, some secret, never
//! round a lock).

use crate::core::rng::Pcg32;
use crate::interior::shape;
use crate::under::*;

/// A side of a box (grid north is -y).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Side {
    N,
    E,
    S,
    W,
}

impl Side {
    pub(crate) const ALL: [Side; 4] = [Side::N, Side::E, Side::S, Side::W];

    /// Mirrored about a row: north and south swap.
    fn flip(self) -> Side {
        match self {
            Side::N => Side::S,
            Side::S => Side::N,
            s => s,
        }
    }

    fn across_x(self) -> bool {
        matches!(self, Side::N | Side::S)
    }
}

/// What a room is grown from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Template {
    Rect,
    Round,
    Octagon,
    /// A plus: arms a third of each side wide.
    Cross,
    /// Rounded at the end it grows out to.
    Apse,
    /// A run of the level's passage room.
    Corridor,
}

/// A room to grow: its kind, template, and sizes (squares) across the wall it grows from and deep away from it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spec {
    pub(crate) kind: &'static str,
    pub(crate) t: Template,
    pub(crate) across: (usize, usize),
    pub(crate) deep: (usize, usize),
}

/// A piece put down: its room (a corridor's is the passage), its box, the door it was grown through, whether more may
/// grow from it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Piece {
    pub(crate) room: i16,
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) door: Option<(usize, usize)>,
    pub(crate) open: bool,
}

/// The squares of box (x, y, w, h) a template covers, row by row, and the room shape that draws it; `out` the side it
/// grows out to (where an apse rounds).
pub(crate) fn mask(t: Template, x: usize, y: usize, w: usize, h: usize, out: Side) -> (Vec<bool>, u8) {
    let code = match (t, out) {
        (Template::Round, _) => shape::ROUND,
        (Template::Octagon, _) => shape::OCTAGON,
        (Template::Apse, Side::N) => shape::APSE_N,
        (Template::Apse, Side::E) => shape::APSE_E,
        (Template::Apse, Side::S) => shape::APSE_S,
        (Template::Apse, Side::W) => shape::APSE_W,
        _ => shape::SQUARE,
    };
    if t == Template::Cross {
        // (Each arm about a third of its side, centred: as many squares off each side.)
        let arm = |n: usize| {
            let a = n.div_ceil(3).max(1);
            if (n - a) % 2 == 1 { a + 1 } else { a }
        };
        let (aw, ah) = (arm(w), arm(h));
        let (cx, cy) = ((w - aw) / 2, (h - ah) / 2);
        let m = (0..w * h).map(|k| (cx..cx + aw).contains(&(k % w)) || (cy..cy + ah).contains(&(k / w))).collect();
        return (m, code);
    }
    if code == shape::SQUARE {
        return (vec![true; w * h], code);
    }
    (shape::shape_cells([x, y, w, h], code), code)
}

pub(crate) struct Grower {
    pub(crate) pieces: Vec<Piece>,
    /// The mirror axis: a row (row j mirrors to 2·axis − j); None: nothing mirrored.
    pub(crate) axis: Option<usize>,
    /// The level's passage (corridor pieces' room).
    pub(crate) passage: i16,
    /// At most this many rooms on the level (a design's text names 75: `design::SYMBOLS`); cells keep `CELLS_LEAVE`
    /// of them for other rooms.
    pub(crate) cap: usize,
    /// Grow towards the whole level (bigger sites fill their grid): most tries aim at a random square and grow from
    /// the open piece nearest it, out of its side facing it.
    pub(crate) spread: bool,
    nx: usize,
    ny: usize,
}

/// Rooms a level may have (a design's text names them by `design::SYMBOLS`), and while growing (some left for the
/// boss chamber).
pub(crate) const ROOMS_MAX: usize = crate::under::design::SYMBOLS.len();
pub(crate) const ROOMS_GROWING: usize = ROOMS_MAX - 3;

/// Rooms the cells leave for others under the grower's cap.
const CELLS_LEAVE: usize = 30;

/// A box placed: corner, size, template squares, room shape, the side it grew out to.
struct Place {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    m: Vec<bool>,
    code: u8,
}

impl Grower {
    pub(crate) fn new(nx: usize, ny: usize, passage: i16, axis: Option<usize>) -> Grower {
        Grower { pieces: Vec::new(), axis, passage, cap: usize::MAX, spread: false, nx, ny }
    }

    /// Whether a box's template squares all lie on rock inside the ring of rock round the level.
    fn fits(&self, p: &Plan, b: &Place) -> bool {
        b.x >= 1
            && b.y >= 1
            && b.x + b.w < self.nx
            && b.y + b.h < self.ny
            && (0..b.w * b.h).all(|k| !b.m[k] || p.cells[(b.y + k / b.w) * self.nx + b.x + k % b.w] < 0)
    }

    /// Put a box's squares down as a new room (`kind`; a corridor joins the passage).
    fn lay(&mut self, p: &mut Plan, b: &Place, kind: &'static str, t: Template, door: Option<(usize, usize)>) -> usize {
        let room = if t == Template::Corridor { self.passage } else { p.room(kind, 0.0) };
        if t != Template::Corridor {
            p.rooms[room as usize].shape = b.code;
        }
        for k in (0..b.w * b.h).filter(|&k| b.m[k]) {
            p.cells[(b.y + k / b.w) * self.nx + b.x + k % b.w] = room;
        }
        if let Some((a, c)) = door
            && p.cells[a] != p.cells[c]
        {
            p.door(a, c);
        }
        self.pieces.push(Piece { room, x: b.x, y: b.y, w: b.w, h: b.h, door, open: true });
        self.pieces.len() - 1
    }

    /// The first room of a level, at box (x, y, w, h): no door.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn seed(&mut self, p: &mut Plan, kind: &'static str, t: Template, x: usize, y: usize, w: usize, h: usize) -> Option<usize> {
        let (m, code) = mask(t, x, y, w, h, Side::E);
        let b = Place { x, y, w, h, m, code };
        self.fits(p, &b).then(|| self.lay(p, &b, kind, t, None))
    }

    /// Grow a room out of side `side` of piece `from`: `across` squares along that wall (centred on it, `along` squares
    /// off), `deep` away from it, with a door through the shared wall at the middle of where they meet. `mirror` (with
    /// an axis): its mirror image too, both or neither (a room lying centred on the axis is its own). The new pieces.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn deck(&mut self, p: &mut Plan, from: usize, side: Side, kind: &'static str, t: Template, across: usize, deep: usize, along: isize, mirror: bool) -> Vec<usize> {
        let f = self.pieces[from];
        let (nx, ny) = (self.nx as isize, self.ny as isize);
        let (w, h) = if side.across_x() { (across, deep) } else { (deep, across) };
        let (fx, fy, fw, fh) = (f.x as isize, f.y as isize, f.w as isize, f.h as isize);
        let (wi, hi) = (w as isize, h as isize);
        let (x, y) = match side {
            Side::E => (fx + fw, fy + (fh - hi).div_euclid(2) + along),
            Side::W => (fx - wi, fy + (fh - hi).div_euclid(2) + along),
            Side::S => (fx + (fw - wi).div_euclid(2) + along, fy + fh),
            Side::N => (fx + (fw - wi).div_euclid(2) + along, fy - hi),
        };
        if x < 1 || y < 1 || x + wi >= nx || y + hi >= ny {
            return Vec::new();
        }
        let (x, y) = (x as usize, y as usize);
        let (m, code) = mask(t, x, y, w, h, side);
        let b = Place { x, y, w, h, m, code };
        if !self.fits(p, &b) {
            return Vec::new();
        }
        let Some(door) = self.door(p, &f, &b, side) else { return Vec::new() };
        let axis = self.axis.filter(|_| mirror);
        let twin = match axis {
            Some(a) if 2 * a + 1 >= y + h => {
                let my = 2 * a + 1 - y - h;
                if my == y {
                    None
                } else if my < y + h && y < my + h {
                    // (Its mirror would overlap it.)
                    return Vec::new();
                } else {
                    let (m2, code2) = mask(t, x, my, w, h, side.flip());
                    let b2 = Place { x, y: my, w, h, m: m2, code: code2 };
                    let flip = |k: usize| (2 * a - k / self.nx) * self.nx + k % self.nx;
                    let d2 = (flip(door.0), flip(door.1));
                    if !self.fits(p, &b2) || p.cells[d2.0] < 0 {
                        return Vec::new();
                    }
                    Some((b2, d2))
                }
            }
            Some(_) => return Vec::new(),
            None => None,
        };
        if t != Template::Corridor && p.rooms.len() + 1 + twin.is_some() as usize > self.cap {
            return Vec::new();
        }
        let mut out = vec![self.lay(p, &b, kind, t, Some(door))];
        if let Some((b2, d2)) = twin {
            out.push(self.lay(p, &b2, kind, t, Some(d2)));
        }
        out
    }

    /// The door between piece `f` and box `b` grown from its side `side`: the facing squares nearest the middle of where
    /// they meet (from `f`'s square, into `b`'s).
    fn door(&self, p: &Plan, f: &Piece, b: &Place, side: Side) -> Option<(usize, usize)> {
        let nx = self.nx;
        let inside = |k: usize| {
            let (i, j) = (k % nx, k / nx);
            i >= b.x && j >= b.y && i < b.x + b.w && j < b.y + b.h && b.m[(j - b.y) * b.w + i - b.x]
        };
        let (lo, hi) = if side.across_x() { (f.x.max(b.x), (f.x + f.w).min(b.x + b.w)) } else { (f.y.max(b.y), (f.y + f.h).min(b.y + b.h)) };
        let mut best: Option<(usize, (usize, usize))> = None;
        for t in lo..hi {
            let (a, c) = match side {
                Side::E => ((t * nx) + f.x + f.w - 1, t * nx + b.x),
                Side::W => (t * nx + f.x, t * nx + b.x + b.w - 1),
                Side::S => ((f.y + f.h - 1) * nx + t, b.y * nx + t),
                Side::N => (f.y * nx + t, (b.y + b.h - 1) * nx + t),
            };
            if p.cells[a] == f.room && inside(c) {
                let off = (2 * t + 1).abs_diff(lo + hi);
                if best.is_none_or(|(o, _)| off < o) {
                    best = Some((off, (a, c)));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Whether piece `i` lies centred on the axis.
    pub(crate) fn centred(&self, i: usize) -> bool {
        let f = &self.pieces[i];
        self.axis.is_some_and(|a| 2 * a + 1 == 2 * f.y + f.h)
    }

    /// Grow up to `rooms` more rooms from random open pieces' walls (a spec drawn from `specs`: list one more often to
    /// draw it more), sized in its ranges, `mirror`ed about the axis if there is one; a corridor leads on to a room at
    /// its far end (both or neither). How many rooms were grown.
    pub(crate) fn grow(&mut self, p: &mut Plan, specs: &[Spec], rooms: usize, mirror: bool, rng: &mut Pcg32) -> usize {
        let rooms_of = |s: &Spec| s.t != Template::Corridor;
        let mut made = 0;
        for _ in 0..rooms * 30 {
            if made >= rooms {
                break;
            }
            let open: Vec<usize> = (0..self.pieces.len()).filter(|&i| self.pieces[i].open).collect();
            if open.is_empty() {
                break;
            }
            let (from, side) = if self.spread && rng.next_f64() < 0.7 { self.aim(p, &open, rng) } else { (open[rng.below(open.len() as u32) as usize], Side::ALL[rng.below(4) as usize]) };
            // (A kind the level has already drawn again less often.)
            let mut s = specs[rng.below(specs.len() as u32) as usize];
            for _ in 0..3 {
                if s.t == Template::Corridor || !p.rooms.iter().any(|r| r.kind == s.kind) || rng.next_f64() < 0.25 {
                    break;
                }
                s = specs[rng.below(specs.len() as u32) as usize];
            }
            let snapshot = (p.cells.clone(), p.doors.clone(), p.rooms.len(), self.pieces.len());
            let new = self.random_deck(p, from, side, &s, mirror, rng);
            if new.is_empty() {
                continue;
            }
            if rooms_of(&s) {
                made += new.len();
                continue;
            }
            // A corridor: a room at its far end.
            let ends: Vec<Spec> = specs.iter().copied().filter(rooms_of).collect();
            let mut led = Vec::new();
            for _ in 0..4 {
                if ends.is_empty() {
                    break;
                }
                let e = ends[rng.below(ends.len() as u32) as usize];
                led = self.random_deck(p, new[0], side, &e, mirror, rng);
                if !led.is_empty() {
                    break;
                }
            }
            if led.is_empty() {
                (p.cells, p.doors) = (snapshot.0, snapshot.1);
                p.rooms.truncate(snapshot.2);
                self.pieces.truncate(snapshot.3);
                continue;
            }
            for &c in &new {
                // (The corridor led somewhere: nothing more off its sides.)
                self.pieces[c].open = false;
            }
            made += led.len();
        }
        made
    }

    /// Where a spreading try grows from: a random square of the level (in the rock, mostly), the open piece nearest it
    /// (one of the nearest three), and that piece's side facing it.
    fn aim(&self, p: &Plan, open: &[usize], rng: &mut Pcg32) -> (usize, Side) {
        // (Drawn again, a few times, while it lies within 3 squares of a room: out into the rock.)
        let mut t = (0, 0);
        for _ in 0..4 {
            t = (1 + rng.below(self.nx as u32 - 2) as usize, 1 + rng.below(self.ny as u32 - 2) as usize);
            let (x0, y0) = (t.0.saturating_sub(3), t.1.saturating_sub(3));
            if (y0..(t.1 + 4).min(self.ny)).all(|j| (x0..(t.0 + 4).min(self.nx)).all(|i| p.cells[j * self.nx + i] < 0)) {
                break;
            }
        }
        let (tx, ty) = (t.0 as isize, t.1 as isize);
        let centre = |i: usize| {
            let f = &self.pieces[i];
            ((2 * f.x + f.w) as isize / 2, (2 * f.y + f.h) as isize / 2)
        };
        let mut near: Vec<(isize, usize)> = open
            .iter()
            .map(|&i| {
                let (cx, cy) = centre(i);
                ((cx - tx).abs() + (cy - ty).abs(), i)
            })
            .collect();
        near.sort_unstable();
        let from = near[rng.below(near.len().min(3) as u32) as usize].1;
        let (cx, cy) = centre(from);
        let (dx, dy) = (tx - cx, ty - cy);
        let side = if dx.abs() >= dy.abs() { if dx >= 0 { Side::E } else { Side::W } } else if dy >= 0 { Side::S } else { Side::N };
        (from, side)
    }

    /// `deck` with sizes drawn from a spec's ranges and an offset along the wall; a room grown along the axis from a
    /// piece centred on it is centred too (odd across).
    fn random_deck(&mut self, p: &mut Plan, from: usize, side: Side, s: &Spec, mirror: bool, rng: &mut Pcg32) -> Vec<usize> {
        let pick = |r: (usize, usize), rng: &mut Pcg32| r.0 + rng.below((r.1 - r.0 + 1) as u32) as usize;
        let (mut across, deep) = (pick(s.across, rng), pick(s.deep, rng));
        let f = self.pieces[from];
        let wall = if side.across_x() { f.w } else { f.h };
        let on_axis = mirror && !side.across_x() && self.centred(from);
        let along = if on_axis || rng.next_f64() < 0.5 {
            0
        } else {
            let r = (wall / 2) as isize;
            rng.below((2 * r + 1) as u32) as isize - r
        };
        if on_axis && along == 0 && across % 2 == 0 {
            across += 1;
        }
        self.deck(p, from, side, s.kind, s.t, across, deep, along, mirror)
    }
}

/// Kits of rooms for prisons, keeps and undercrofts: rows of cells off a straight block, rooms put where they fit.
impl Grower {
    /// A run of corridors (`cw` wide) and rooms (one of `kinds` in turn, 4-6 squares) from piece `from` out towards
    /// square `to`, a step at a time out of the side facing it (else the other way towards it) till it lies near or
    /// nothing more fits. The rooms laid.
    pub(crate) fn reach(&mut self, p: &mut Plan, from: usize, to: (usize, usize), kinds: &[&'static str], cw: usize, rng: &mut Pcg32) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = from;
        for step in 0..12 {
            let f = self.pieces[at];
            let (cx, cy) = ((2 * f.x + f.w) as isize / 2, (2 * f.y + f.h) as isize / 2);
            let (dx, dy) = (to.0 as isize - cx, to.1 as isize - cy);
            if dx.abs() + dy.abs() < 10 {
                break;
            }
            let h = if dx >= 0 { Side::E } else { Side::W };
            let v = if dy >= 0 { Side::S } else { Side::N };
            let sides = if dx.abs() >= dy.abs() { [h, v] } else { [v, h] };
            let kind = kinds[step % kinds.len()];
            let mut next = None;
            for side in sides {
                let far = if side.across_x() { dy.unsigned_abs() } else { dx.unsigned_abs() };
                let len = (6 + rng.below(9) as usize).min(far.saturating_sub(3)).max(2);
                let Some(&c) = self.deck(p, at, side, "", Template::Corridor, cw, len, 0, false).first() else { continue };
                let (a, d) = (4 + rng.below(3) as usize, 4 + rng.below(3) as usize);
                if let Some(&r) = self.deck(p, c, side, kind, Template::Rect, a, d, 0, false).first() {
                    self.pieces[c].open = false;
                    next = Some(r);
                    break;
                }
                self.undo_last(p);
            }
            let Some(r) = next else { break };
            out.push(r);
            at = r;
        }
        out
    }

    /// Take back the last piece laid (its squares were rock, its door goes; a room's def too).
    pub(crate) fn undo_last(&mut self, p: &mut Plan) {
        let Some(f) = self.pieces.pop() else { return };
        for j in f.y..f.y + f.h {
            for i in f.x..f.x + f.w {
                if p.cells[j * self.nx + i] == f.room {
                    p.cells[j * self.nx + i] = -1;
                }
            }
        }
        if let Some((a, b)) = f.door {
            p.doors.remove(&(a.min(b), a.max(b)));
        }
        if f.room != self.passage {
            p.rooms.pop();
        }
    }

    /// Put room `kind` (template `t`, `across` by `deep` squares) where it fits, not mirrored: off a wall of one of the
    /// `near` pieces still open, else of any open piece (from a random one on), straight through the wall or at the
    /// end of a corridor `cw` wide grown first (`cw` 0: never). Its piece.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn attach(&mut self, p: &mut Plan, near: &[usize], kind: &'static str, t: Template, (across, deep): (usize, usize), cw: usize, rng: &mut Pcg32) -> Option<usize> {
        let n = self.pieces.len();
        let start = if n == 0 { 0 } else { rng.below(n as u32) as usize };
        let mut order: Vec<usize> = near.iter().copied().filter(|&i| self.pieces[i].open).collect();
        order.extend((0..n).map(|k| (start + k) % n).filter(|i| self.pieces[*i].open && !near.contains(i)));
        let s0 = rng.below(4) as usize;
        let clen = 2 + rng.below(3) as usize;
        for via in [false, true] {
            if via && cw == 0 {
                break;
            }
            for &i in &order {
                for k in 0..4 {
                    let side = Side::ALL[(s0 + k) % 4];
                    let f = self.pieces[i];
                    let wall = if side.across_x() { f.w } else { f.h } as isize;
                    for along in [0, -(wall / 3), wall / 3] {
                        if !via {
                            if let Some(&r) = self.deck(p, i, side, kind, t, across, deep, along, false).first() {
                                return Some(r);
                            }
                            continue;
                        }
                        let Some(&c) = self.deck(p, i, side, "", Template::Corridor, cw, clen, along, false).first() else { continue };
                        if let Some(&r) = self.deck(p, c, side, kind, t, across, deep, 0, false).first() {
                            self.pieces[c].open = false;
                            return Some(r);
                        }
                        self.undo_last(p);
                    }
                }
            }
        }
        None
    }

    /// Small rooms (`kind`, `a` squares along the wall, `d` deep) lined along both long walls of piece `row`, as many as
    /// fit, mirrored with it when asked (`deck`); each closed to more growth. How many were put.
    pub(crate) fn line(&mut self, p: &mut Plan, row: usize, kind: &'static str, (a, d): (usize, usize), mirror: bool) -> usize {
        let f = self.pieces[row];
        let (len, sides) = if f.w > f.h { (f.w, [Side::N, Side::S]) } else { (f.h, [Side::W, Side::E]) };
        let mut put = 0;
        for side in sides {
            let mut t = 0;
            while t + a <= len && p.rooms.len() + CELLS_LEAVE < self.cap {
                let along = t as isize - (len as isize - a as isize).div_euclid(2);
                for c in self.deck(p, row, side, kind, Template::Rect, a, d, along, mirror) {
                    self.pieces[c].open = false;
                    put += 1;
                }
                t += a;
            }
        }
        put
    }

    /// A cell block: a straight row (`row` kind, `rw` wide, `len` long) grown out of side `side` of piece `from`
    /// (`along` off the middle of its wall), cells (`cell`, `a` by `d`) along both its long walls, an iron grate at its
    /// mouth (`gate`); mirrored when asked. The rows put (none if it has no cells).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn cell_block(&mut self, p: &mut Plan, from: usize, side: Side, along: isize, (row, rw, len): (&'static str, usize, usize), (cell, a, d): (&'static str, usize, usize), gate: bool, mirror: bool) -> Vec<usize> {
        let rows = self.deck(p, from, side, row, Template::Rect, rw, len, along, mirror);
        let Some(&first) = rows.first() else { return rows };
        let mut cells = self.line(p, first, cell, (a, d), mirror);
        if rows.len() > 1 && !mirror {
            cells += self.line(p, rows[1], cell, (a, d), false);
        }
        if cells == 0 {
            // (A row with no cells: take it back, with its mirror.)
            for _ in 0..rows.len() {
                self.undo_last(p);
            }
            return Vec::new();
        }
        for &r in rows.iter().filter(|_| gate) {
            if let Some((x, y)) = self.pieces[r].door {
                p.portcullis.insert((x.min(y), x.max(y)));
            }
        }
        rows
    }
}

/// The way down on a level: the square nearest the middle of the room farthest on foot from the arrival (not the
/// passage, the arrival's room, nor one of `skip`), off its doorways; the farthest square if there is no such room.
pub(crate) fn way_down(p: &Plan, ny: usize, skip: &[i16]) -> usize {
    let nx = p.nx;
    let dist = p.distances(p.up, ny, None);
    let passage = p.passage.map_or(-1, |q| q.0);
    let mut at_door = vec![false; p.cells.len()];
    for &(a, b) in &p.doors {
        at_door[a] = true;
        at_door[b] = true;
    }
    let ok = |k: usize| {
        let r = p.cells[k];
        r >= 0 && dist[k] >= 0 && r != passage && r != p.cells[p.up] && !skip.contains(&r) && p.rooms[r as usize].kind != BOSS
    };
    let Some(far) = (0..nx * ny).filter(|&k| ok(k)).max_by_key(|&k| (dist[k], std::cmp::Reverse(k))) else {
        return (0..nx * ny).filter(|&k| dist[k] > 0 && !at_door[k]).max_by_key(|&k| (dist[k], std::cmp::Reverse(k))).unwrap_or(p.up);
    };
    let room = p.cells[far];
    let squares: Vec<usize> = (0..nx * ny).filter(|&k| p.cells[k] == room).collect();
    let (sx, sy) = squares.iter().fold((0.0, 0.0), |(a, b), &k| (a + (k % nx) as f64, b + (k / nx) as f64));
    let (cx, cy) = (sx / squares.len() as f64, sy / squares.len() as f64);
    let d = |k: usize| ((k % nx) as f64 - cx).abs() + ((k / nx) as f64 - cy).abs();
    squares.iter().copied().filter(|&k| !at_door[k] && k != p.up).min_by(|&a, &b| d(a).total_cmp(&d(b)).then(a.cmp(&b))).unwrap_or(far)
}

/// Pillars in two rows down room `room`'s longer side, `inset` squares in from its long walls, every `step` squares
/// (the rows centred on the room), each kept off doorways and the squares beside them and only where every square
/// stays reachable (`Plan::place`). How many were put.
pub(crate) fn colonnade(p: &mut Plan, ny: usize, room: i16, inset: usize, step: usize) -> usize {
    let nx = p.nx;
    let Some([x, y, w, h]) = shape::room_box(&p.cells, nx, room) else { return 0 };
    let along_x = w >= h;
    let (len, wide) = if along_x { (w, h) } else { (h, w) };
    if wide < 2 * inset + 3 || len < 5 || step == 0 {
        return 0;
    }
    let mut near_door = vec![false; p.cells.len()];
    for &(a, b) in &p.doors {
        for k in [a, b] {
            near_door[k] = true;
            for m in neighbours(k, nx, ny) {
                near_door[m] = true;
            }
        }
    }
    let count = (len - 3) / step + 1;
    let start = 1 + (len - 2 - ((count - 1) * step + 1)) / 2;
    let mut put = 0;
    for c in 0..count {
        let t = start + c * step;
        for r in [inset, wide - 1 - inset] {
            let (i, j) = if along_x { (x + t, y + r) } else { (x + r, y + t) };
            let k = j * nx + i;
            if p.cells[k] == room && !near_door[k] && !p.taken[k] && p.down != Some(k) && k != p.up && p.place(item("pillar", i, j, 1, 1), Some(room), ny) {
                put += 1;
            }
        }
    }
    put
}

/// Water for room `room` (laid with the locks in: `Plan::water`): the noise heights over which its squares are
/// shallow and deep water (`kit::water_at`; over 1: never); `ledge`: no deep water on squares along its walls, a dry
/// or wading way round.
pub(crate) fn flood_room(p: &mut Plan, ny: usize, room: i16, (shallow, deep): (f64, f64), ledge: bool) {
    let nx = p.nx;
    if p.water.is_empty() {
        p.water = vec![[2.0; 2]; p.cells.len()];
    }
    for k in (0..p.cells.len()).filter(|&k| p.cells[k] == room) {
        let edge = neighbours(k, nx, ny).any(|m| p.cells[m] != room);
        p.water[k] = [shallow as f32, if ledge && edge { 2.0 } else { deep as f32 }];
    }
}

/// The boss chamber grown off the open piece farthest from the arrival that has room for it (an apse at its far end),
/// closed to more growth.
pub(crate) fn boss_room(p: &mut Plan, g: &mut Grower, ny: usize, rng: &mut Pcg32) -> Option<i16> {
    let nx = p.nx;
    let dist = p.distances(p.up, ny, None);
    let mut order: Vec<(i32, usize)> = (0..g.pieces.len())
        .filter(|&i| g.pieces[i].open || g.pieces[i].room == g.passage)
        .map(|i| {
            let f = &g.pieces[i];
            let k = (f.y + f.h / 2) * nx + f.x + f.w / 2;
            (if p.cells[k] == f.room { dist[k] } else { dist[f.y * nx + f.x].max(0) }, i)
        })
        .collect();
    order.sort_by_key(|&(d, i)| (std::cmp::Reverse(d), i));
    let (a, d) = (7 + 2 * rng.below(2) as usize, 7 + rng.below(3) as usize);
    for (_, i) in order {
        for side in Side::ALL {
            for (t, a, d) in [(Template::Apse, a, d), (Template::Rect, 7, 7), (Template::Rect, 5, 6)] {
                let mirror = !matches!(side, Side::N | Side::S) && g.centred(i);
                if let Some(&b) = g.deck(p, i, side, BOSS, t, a, d, 0, mirror).first() {
                    g.pieces[b].open = false;
                    return Some(g.pieces[b].room);
                }
            }
        }
    }
    None
}

/// The room with one door farthest on foot from the arrival (not the passage nor the arrival's).
pub(crate) fn dead_end(p: &Plan, ny: usize) -> Option<i16> {
    let passage = p.passage.map_or(-1, |q| q.0);
    let mut doors = vec![0; p.rooms.len()];
    for &(a, b) in &p.doors {
        for k in [a, b] {
            doors[p.cells[k] as usize] += 1;
        }
    }
    let dist = p.distances(p.up, ny, None);
    (0..p.cells.len())
        .filter(|&k| {
            let r = p.cells[k];
            r >= 0 && r != passage && r != p.cells[p.up] && doors[r as usize] == 1 && dist[k] >= 0 && !matches!(p.rooms[r as usize].kind, "cell" | "solitary cell" | "cell row")
        })
        .max_by_key(|&k| (dist[k], std::cmp::Reverse(k)))
        .map(|k| p.cells[k])
}

/// How many times a medium site's rooms a site gets, its grid `area` times a medium one's: more than its area on big
/// sites, so they fill their grids.
pub(crate) fn fill(area: f64) -> f64 {
    if area > 1.0 { area * crate::core::sqrt(area) } else { area }
}

/// The share of its grid (inside the rock rim) a level's floor spans, by its bounding box; and that box (x0, y0, x1, y1).
pub(crate) fn spanned(p: &Plan, ny: usize) -> (f64, (usize, usize, usize, usize)) {
    let nx = p.nx;
    let (mut x0, mut y0, mut x1, mut y1) = (nx, ny, 0, 0);
    // (Rooms only: a planner's marks past the last room, `HALO`, are rock.)
    for k in (0..nx * ny).filter(|&k| p.cells[k] >= 0 && (p.cells[k] as usize) < p.rooms.len()) {
        let (i, j) = (k % nx, k / nx);
        (x0, y0, x1, y1) = (x0.min(i), y0.min(j), x1.max(i), y1.max(j));
    }
    if x1 < x0 {
        return (0.0, (0, 0, 0, 0));
    }
    (((x1 + 1 - x0) * (y1 + 1 - y0)) as f64 / ((nx - 2) * (ny - 2)) as f64, (x0, y0, x1, y1))
}

/// How much of its grid a big site's level should span (`fill_out`): most of a huge one's, two thirds of a large one's.
pub(crate) fn fill_share(area: f64) -> f64 {
    if area >= 2.2 {
        0.85
    } else if area >= 1.5 {
        0.7
    } else {
        0.0
    }
}

/// Grow to fill: while the level's floor spans less than `want` of its grid, a run of corridors and rooms (`kinds` in
/// turn, `Grower::reach`) out towards the widest band of rock left beside it, from one of the open pieces nearest;
/// done when no band takes a run. The rooms laid.
pub(crate) fn fill_out(g: &mut Grower, p: &mut Plan, ny: usize, want: f64, kinds: &[&'static str], cw: usize, rng: &mut Pcg32) -> usize {
    let nx = p.nx;
    let mut made = 0;
    for _ in 0..16 {
        let (share, (x0, y0, x1, y1)) = spanned(p, ny);
        if share >= want || share == 0.0 {
            break;
        }
        let (bw, bh) = (x1 + 1 - x0, y1 + 1 - y0);
        // (Bands west, east, north, south: how deep, how much rock; a run aims 3 squares in from the grid's edge.)
        let mut bands: Vec<(usize, usize)> = [x0.saturating_sub(1) * bh, (nx - 2).saturating_sub(x1) * bh, y0.saturating_sub(1) * bw, (ny - 2).saturating_sub(y1) * bw].into_iter().enumerate().map(|(i, a)| (a, i)).collect();
        bands.sort_by_key(|&(a, i)| (std::cmp::Reverse(a), i));
        let mut laid = false;
        for (_, side) in bands {
            let deep = [x0.saturating_sub(1), (nx - 2).saturating_sub(x1), y0.saturating_sub(1), (ny - 2).saturating_sub(y1)][side];
            if deep < 3 {
                continue;
            }
            let (along_x, along_y) = (x0 + rng.below(bw as u32) as usize, y0 + rng.below(bh as u32) as usize);
            let to = [(3, along_y), (nx - 4, along_y), (along_x, 3), (along_x, ny - 4)][side];
            let far = |f: &Piece| ((2 * f.x + f.w) / 2).abs_diff(to.0) + ((2 * f.y + f.h) / 2).abs_diff(to.1);
            let mut near: Vec<(usize, usize)> = (0..g.pieces.len()).filter(|&i| g.pieces[i].open).map(|i| (far(&g.pieces[i]), i)).collect();
            near.sort_unstable();
            for &(_, i) in near.iter().take(5) {
                let rooms = g.reach(p, i, to, kinds, cw, rng);
                if !rooms.is_empty() {
                    made += rooms.len();
                    laid = true;
                    break;
                }
            }
            if laid {
                break;
            }
        }
        if !laid {
            break;
        }
    }
    made
}

/// A level the grower couldn't start (no room round the arrival): one square, the way on.
pub(crate) fn fallback(mut p: Plan, last: bool) -> Plan {
    let r = p.room(if last { BOSS } else { "landing" }, 0.0);
    p.cells[p.up] = r;
    p
}

/// An iron grate in the doorway a piece was grown through.
pub(crate) fn grate(p: &mut Plan, f: &Piece) {
    if let Some((a, b)) = f.door {
        p.portcullis.insert((a.min(b), a.max(b)));
    }
}

/// How many rooms a level has (not its passage, cells nor cell rows).
pub(crate) fn rooms(p: &Plan, passage: i16) -> usize {
    (0..p.rooms.len()).filter(|&r| r as i16 != passage && !matches!(p.rooms[r].kind, "cell" | "solitary cell" | "cell row")).count()
}

/// What grows among the kits: these rooms, each small or up to `big` (across, deep), and corridors `w` wide.
pub(crate) fn specs(kinds: &[&'static str], w: usize, big: ((usize, usize), (usize, usize))) -> Vec<Spec> {
    let mut out = Vec::new();
    for &kind in kinds {
        out.push(Spec { kind, t: Template::Rect, across: (3, 5), deep: (3, 5) });
        out.push(Spec { kind, t: Template::Rect, across: big.0, deep: big.1 });
    }
    out.push(Spec { kind: "", t: Template::Corridor, across: (w, w), deep: (3, 6) });
    out.push(Spec { kind: "", t: Template::Corridor, across: (w, w), deep: (2, 4) });
    out
}

/// A level's first room round the arrival (`kind`, template `t`, else a rect; `w` by `h` squares, `h` odd), the arrival
/// `back` squares in from its wall facing away from `dir` (E or W; None: centred on it): the grower to go on with
/// (spreading, its axis the room's middle row) and its piece; None if not even one square fits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn first_room(p: &mut Plan, passage: i16, kind: &'static str, t: Template, (w, h): (usize, usize), dir: Option<Side>, back: usize) -> Option<(Grower, usize)> {
    let nx = p.nx;
    let ny = p.cells.len() / nx;
    let (ax, ay) = (p.up % nx, p.up / nx);
    let x = match dir {
        Some(Side::E) => ax.saturating_sub(back),
        Some(Side::W) => (ax + back + 1).saturating_sub(w),
        _ => ax.saturating_sub(w / 2),
    };
    let x = x.clamp(1, nx.saturating_sub(w + 1).max(1));
    let y = ay.saturating_sub(h / 2).clamp(1, ny.saturating_sub(h + 1).max(1));
    let holds = |t: Template| (x..x + w).contains(&ax) && (y..y + h).contains(&ay) && mask(t, x, y, w, h, Side::E).0[(ay - y) * w + ax - x];
    let mut g = Grower::new(nx, ny, passage, Some(y + h / 2));
    g.cap = ROOMS_GROWING;
    g.spread = true;
    let t = if holds(t) { t } else { Template::Rect };
    let first = match holds(t).then(|| g.seed(p, kind, t, x, y, w, h)).flatten() {
        Some(f) => f,
        None => {
            g.axis = Some(ay);
            g.seed(p, kind, Template::Rect, ax, ay, 1, 1)?
        }
    };
    Some((g, first))
}

impl Grower {
    /// From piece `from`, when its middle lies more than `slack` rows off the grid's middle row, a passage `cw` wide
    /// north or south towards it ending in a room (`kind`, `t`, `s` squares square) centred on its far end, the axis
    /// moved to that room's middle row (so a level entered near its edge still has room for its mirrored halls). The
    /// piece to go on from: that room, else `from`.
    pub(crate) fn to_middle(&mut self, p: &mut Plan, from: usize, cw: usize, (kind, t, s): (&'static str, Template, usize), slack: usize) -> usize {
        let f = self.pieces[from];
        let (mid, at) = (self.ny / 2, f.y + f.h / 2);
        if at.abs_diff(mid) <= slack {
            return from;
        }
        let side = if at < mid { Side::S } else { Side::N };
        let off = at.abs_diff(mid).saturating_sub(f.h / 2 + s / 2);
        for len in [off, off * 2 / 3, off / 2] {
            let Some(&c) = self.deck(p, from, side, "", Template::Corridor, cw, len.max(2), 0, false).first() else { continue };
            if let Some(&r) = self.deck(p, c, side, kind, t, s, s, 0, false).first() {
                self.pieces[c].open = false;
                let r2 = self.pieces[r];
                self.axis = Some(r2.y + r2.h / 2);
                return r;
            }
            self.undo_last(p);
        }
        from
    }
}
