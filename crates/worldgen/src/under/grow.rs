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
    nx: usize,
    ny: usize,
}

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
        Grower { pieces: Vec::new(), axis, passage, nx, ny }
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
        if let Some((a, c)) = door {
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
            let from = open[rng.below(open.len() as u32) as usize];
            let side = Side::ALL[rng.below(4) as usize];
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
