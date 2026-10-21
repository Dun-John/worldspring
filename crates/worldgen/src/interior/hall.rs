//! Great halls furnished as one composition about their axis (`furnish_hall`), before the shared
//! rules furnish the rest of a floor: the axis runs from the end the hall is entered at to the far
//! end; the dais at the far end on the axis (a castle's throne dais, or a low dais with a chair of
//! office, a noble estate's carrying the high table), standing banners and braziers flanking a
//! throne, a castle's high table before it; a carpet runner up the axis to it; long tables with
//! benches down both sides, mirrored about the axis; pillar pairs where the hall keeps a clear way
//! by its walls; the hearth centred on a long wall; banners in rows and tapestries on the long
//! walls, mirrored. A mess hall has its tables and hearth only. Every piece that blocks the way is
//! put down only where the hall stays whole: its doors and stairs joined, no square of its floor
//! shut off (`Hall::open`). Kept general (a frame, a style): a palace's throne room (its throne dais,
//! no tables), banqueting hall (the high table on a low dais) and ballroom (the dance floor, the
//! musicians and the harpsichords at its far end, refreshment tables) too; naves and refectories are
//! to be furnished with it.

use super::{Item, Level, building_item, entry_squares};
use crate::core::rng::{Pcg32, hash3};

/// How a hall is furnished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// A castle's great hall: its throne dais (`dais`: its look, `super::item_looks`), standing
    /// banners and braziers flanking it, the high table before it, banners of `house`; `banquet`: it
    /// serves as the banqueting hall (an older palace's), its high table had whatever the long
    /// tables then lose.
    Castle { dais: u8, house: u8, banquet: bool },
    /// A town hall's, a guild's, a harbor master's or an auction house's: a low dais (of stone, else
    /// timber) with a chair of office at its middle.
    Civic { stone: bool },
    /// A noble estate's hall: a low timber dais carrying the high table, a chair of office at its
    /// middle behind it; banners of the family's `house`.
    Estate { house: u8 },
    /// A mess hall (and any other hall: a guard post's, an orphanage's): long tables and benches and
    /// the hearth only; banners of a castle's `house` in a castle's.
    Mess { house: Option<u8> },
    /// A palace's throne room: its throne dais (`dais`: its look) against the far wall, standing
    /// banners and braziers flanking it, the runner up to it, pillars, banners of `house` and
    /// tapestries; no tables.
    Throne { dais: u8, house: u8 },
    /// A palace's banqueting hall: a low stone dais across its far end carrying the high table, long
    /// tables and benches down both sides, the hearth, banners of `house`.
    Banquet { house: u8 },
    /// A palace's ballroom: the inlaid dance floor in its middle, the musicians' corner on the axis
    /// at its far end with a harpsichord each side, refreshment tables by its long walls at the door
    /// end, pillars and tapestries.
    Ballroom,
    /// A palace's audience chamber (the presence chamber): the chair of state on a low stone dais
    /// centred on its far wall, standing banners of `house` either side, the runner up to it from the
    /// way in, banners and tapestries on the long walls; no tables.
    Presence { house: u8 },
    /// A long gallery: pieces along its long walls only, a square deep (statues, benches, tapestries
    /// between them, mirrored), the walk down its middle clear from end to end.
    Gallery,
    /// A chapel along its axis: the altar on a stone step against the far wall (east where it can
    /// be), two blocks of pews facing it either side of the aisle up the middle, the runner up the
    /// aisle, the font by the way in.
    Chapel,
}

impl Style {
    fn house(self) -> Option<u8> {
        match self {
            Style::Castle { house, .. } | Style::Estate { house } | Style::Throne { house, .. } | Style::Banquet { house } | Style::Presence { house } => Some(house),
            Style::Mess { house } => house,
            Style::Civic { .. } | Style::Ballroom | Style::Gallery | Style::Chapel => None,
        }
    }
}

/// A hall's frame on the grid: its square (s, t) is grid square `o + s * ds + t * dt`; s runs from
/// the end it is entered at (0) to the far end (`len - 1`, the dais's), t across it (0 to `wid` - 1).
/// The hall's squares are those of its room within the frame's box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub o: (isize, isize),
    pub ds: (isize, isize),
    pub dt: (isize, isize),
    pub len: usize,
    pub wid: usize,
}

impl Frame {
    pub fn grid(&self, s: isize, t: isize) -> (isize, isize) {
        (self.o.0 + self.ds.0 * s + self.dt.0 * t, self.o.1 + self.ds.1 * s + self.dt.1 * t)
    }

    /// Grid square (i, j) in the frame (s, t).
    pub fn local(&self, i: isize, j: isize) -> (isize, isize) {
        let (di, dj) = (i - self.o.0, j - self.o.1);
        (di * self.ds.0 + dj * self.ds.1, di * self.dt.0 + dj * self.dt.1)
    }
}

/// A platform others stand on (a low dais; a court's gravel walk, laid on its lawn): the items another
/// may overlap; walked over, they may lie in a doorway.
pub fn is_platform(kind: &str) -> bool {
    matches!(kind, "low_dais" | "garden_path")
}

/// The side code of `Item::back` (0 north, 1 east, 2 south, 3 west) for a step in grid axes.
fn back_of(d: (isize, isize)) -> u8 {
    match d {
        (0, -1) => 0,
        (1, 0) => 1,
        (0, 1) => 2,
        _ => 3,
    }
}

/// A hall being furnished.
struct Hall {
    nx: usize,
    ny: usize,
    f: Frame,
    /// The hall's squares (its room's, whether in the frame or not).
    room: Vec<bool>,
    /// Squares a piece may go on (the room's, clear of its doorways, the stairs and their foot, what
    /// stood there already, and squares the outer wall as drawn cuts).
    ok: Vec<bool>,
    /// What stands on each square: 0 nothing, 1 a platform, 2 a piece.
    held: Vec<u8>,
    /// The foot of the stairs (squares only what lies flat may take).
    foot: Vec<bool>,
    /// The squares either side of the hall's doors.
    door: Vec<bool>,
    /// Squares a platform may go on: walked over, it may lie in a doorway and on the foot of the
    /// stairs too.
    deck: Vec<bool>,
    blocked: Vec<bool>,
    entries: Vec<usize>,
    items: Vec<Item>,
}

impl Hall {
    /// The grid index of frame square (s, t), if it is in the frame (or up to two rows past its far
    /// end, where the hall goes on: a dais backing onto its wall there) and the hall.
    fn at(&self, s: isize, t: isize) -> Option<usize> {
        if s < 0 || t < 0 || s >= self.f.len as isize + 2 || t >= self.f.wid as isize {
            return None;
        }
        let (i, j) = self.f.grid(s, t);
        (i >= 0 && j >= 0 && (i as usize) < self.nx && (j as usize) < self.ny).then(|| j as usize * self.nx + i as usize).filter(|&k| self.room[k])
    }

    /// Whether the square past frame square (s, t) by (ds, dt) frame steps is not the hall's: a wall
    /// stands between them.
    fn walled(&self, s: isize, t: isize, d: (isize, isize)) -> bool {
        let (i, j) = self.f.grid(s + d.0, t + d.1);
        i < 0 || j < 0 || i as usize >= self.nx || j as usize >= self.ny || !self.room[j as usize * self.nx + i as usize]
    }

    /// A frame step as a step in grid axes.
    fn step(&self, d: (isize, isize)) -> (isize, isize) {
        (self.f.ds.0 * d.0 + self.f.dt.0 * d.1, self.f.ds.1 * d.0 + self.f.dt.1 * d.1)
    }

    /// The hall's floor holds together: from its first way in, every other way in and every square no
    /// blocking piece stands on.
    fn open(&self) -> bool {
        let Some(&k0) = self.entries.first() else { return true };
        let (nx, ny) = (self.nx, self.ny);
        let mut seen = vec![false; nx * ny];
        let mut stack = vec![k0];
        seen[k0] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < nx && b < ny && !seen[b * nx + a] && self.room[b * nx + a] && !self.blocked[b * nx + a] {
                    seen[b * nx + a] = true;
                    stack.push(b * nx + a);
                }
            }
        }
        self.entries.iter().all(|&e| seen[e]) && (0..nx * ny).all(|k| !self.room[k] || self.blocked[k] || seen[k])
    }

    /// Piece `kind` on frame squares s0..s0+ls, t0..t0+lt (on a platform's squares if `onto`), its back
    /// toward frame step `back`: put down if it fits and leaves the hall whole.
    #[allow(clippy::too_many_arguments)]
    fn put(&mut self, kind: &str, s0: isize, t0: isize, ls: usize, lt: usize, back: Option<(isize, isize)>, variant: Option<u8>, onto: bool) -> bool {
        let platform = is_platform(kind);
        let (a, b) = (self.f.grid(s0, t0), self.f.grid(s0 + ls as isize - 1, t0 + lt as isize - 1));
        let (x, y) = (a.0.min(b.0).max(0) as usize, a.1.min(b.1).max(0) as usize);
        let (w, h) = ((a.0 - b.0).unsigned_abs() + 1, (a.1 - b.1).unsigned_abs() + 1);
        let Some(mut it) = building_item(kind, x, y, w, h) else { return false };
        // (Laid flat on the floor, walked over: on the foot of the stairs too.)
        let flat = !it.blocks_move && it.height_ft <= 0.0;
        let mut sq = Vec::new();
        for s in s0..s0 + ls as isize {
            for t in t0..t0 + lt as isize {
                let Some(k) = self.at(s, t) else { return false };
                let room = if platform { self.deck[k] } else { self.ok[k] || (flat && self.foot[k]) };
                let free = room && (self.held[k] == 0 || (onto && !platform && self.held[k] == 1));
                if !free {
                    return false;
                }
                sq.push(k);
            }
        }
        if it.blocks_move {
            for &k in &sq {
                self.blocked[k] = true;
            }
            if !self.open() {
                for &k in &sq {
                    self.blocked[k] = false;
                }
                return false;
            }
        }
        for &k in &sq {
            self.held[k] = if platform { 1 } else { 2 };
        }
        it.back = back.map(|d| back_of(self.step(d)));
        it.variant = variant;
        self.items.push(it);
        true
    }

    /// The state to go back to (`undo`).
    fn mark(&self) -> (Vec<u8>, Vec<bool>, usize) {
        (self.held.clone(), self.blocked.clone(), self.items.len())
    }

    fn undo(&mut self, m: (Vec<u8>, Vec<bool>, usize)) {
        (self.held, self.blocked) = (m.0, m.1);
        self.items.truncate(m.2);
    }

    /// `put` and its mirror image across the axis (one piece where they are the same), both or
    /// neither; `back` given for the piece on the low-t side (mirrored for the other).
    #[allow(clippy::too_many_arguments)]
    fn pair(&mut self, kind: &str, s0: isize, t0: isize, ls: usize, lt: usize, back: Option<(isize, isize)>, variant: Option<u8>, onto: bool) -> bool {
        let m = self.mark();
        let t1 = self.f.wid as isize - t0 - lt as isize;
        if !self.put(kind, s0, t0, ls, lt, back, variant, onto) {
            return false;
        }
        if t1 != t0 && !self.put(kind, s0, t1, ls, lt, back.map(|d| (d.0, -d.1)), variant, onto) {
            self.undo(m);
            return false;
        }
        true
    }

    /// The longest run of rows in `rows` on which columns `cols` and their mirrors are free.
    fn run(&self, cols: &[isize], rows: (isize, isize)) -> (isize, isize) {
        let w = self.f.wid as isize;
        let free_row = |s: isize| cols.iter().all(|&c| [c, w - 1 - c].iter().all(|&t| self.at(s, t).is_some_and(|k| self.ok[k] && self.held[k] == 0)));
        let (mut best, mut run) = ((0, -1), None::<isize>);
        for s in rows.0..=rows.1 + 1 {
            if s <= rows.1 && free_row(s) {
                run.get_or_insert(s);
            } else if let Some(a) = run.take()
                && s - 1 - a > best.1 - best.0
            {
                best = (a, s - 1);
            }
        }
        best
    }

    /// Lanes of long tables with benches down both sides, mirrored: each lane's columns (bench,
    /// table, bench; -1 none), over rows `rows` (or, for a lane they hold none on, `more`): a table on
    /// each run of rows its column is free on (2 rows or more; one over 8 rows split in two),
    /// shortened until the hall stays whole; its benches along it wherever they are free.
    fn tables(&mut self, lanes: &[[isize; 3]], rows: (isize, isize), more: (isize, isize)) {
        for lane in lanes {
            // (Over the tables' rows, else all before the dais; else the table on the inner column,
            // its bench on the outer, where doors break the outer one.)
            let n = self.items.len();
            for (l, r) in [(*lane, rows), (*lane, more), ([-1, lane[0], lane[1]], rows), ([-1, lane[0], lane[1]], more)] {
                if self.items.len() == n && l[1] >= 0 {
                    self.lane(&l, r);
                }
            }
        }
    }

    /// A long gallery's pieces (`Style::Gallery`), along its long walls from the end it is entered
    /// at, mirrored: a statue, a gap, a bench two squares long, a gap, a tapestry two squares long, a
    /// gap, and so on, each where both walls are whole beside it, a square deep: nothing across the
    /// walk down its middle, the ends left clear.
    fn gallery(&mut self, rng: &mut Pcg32) {
        let (l, w) = (self.f.len as isize, self.f.wid as isize);
        if w < 3 || l < 3 {
            return;
        }
        let side = (0, -1);
        let scene = rng.below(4) as u8;
        let pattern: [(&str, isize); 3] = [("statue", 1), ("bench", 2), ("tapestry", 2)];
        let (mut s, mut n) = (1, 0);
        while s < l - 1 {
            let (kind, ls) = pattern[n % pattern.len()];
            let fits = (s..s + ls).all(|q| q < l - 1 && self.walled(q, 0, side) && self.walled(q, w - 1, (0, 1)));
            let v = (kind == "tapestry").then(|| (scene + (n / pattern.len()) as u8) % 4);
            if fits && self.pair(kind, s, 0, ls as usize, 1, Some(side), v, false) {
                s += ls + 1;
                n += 1;
                continue;
            }
            s += 1;
        }
    }

    /// A chapel's pieces (`Style::Chapel`): the step (a low stone dais) across its far end, two rows
    /// deep in one 8 long or more, as wide as the chapel (else narrower, else as the altar), the altar
    /// on it on the axis against the far wall, a statue each side of it in one 6 wide or more; pews a
    /// square deep in two blocks either side of the aisle up the middle, on every row from the second
    /// to the one before the step's (a square clear by each long wall in one 7 wide or more), all
    /// facing the altar; the runner up the aisle to the step; the font in a corner at the end it is
    /// entered at. False where the altar has no room at the far end.
    fn chapel(&mut self, rng: &mut Pcg32) -> bool {
        let (l, w) = (self.f.len as isize, self.f.wid as isize);
        if l < 3 || w < 2 {
            return false;
        }
        let far = (1, 0);
        let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
        let aw = cr - cl + 1;
        // (The step and the altar against the far wall where it is whole behind the altar (the room
        // past it, if any, on squares the outer wall as drawn cuts into): else the pews and the rest
        // only.)
        let deep = if l >= 8 { 2 } else { 1 };
        let mut step = None;
        if (cl..=cr).all(|t| self.at(l, t).is_none_or(|k| !self.deck[k])) {
            for (dd, dw) in [(deep, w), (deep, w - 2), (1, w), (1, w - 2), (1, aw)] {
                let s0 = l - dd;
                if dw < aw || s0 < 2 {
                    continue;
                }
                let m = self.mark();
                if self.put("low_dais", s0, (w - dw) / 2, dd as usize, dw as usize, Some(far), Some(0), false) {
                    if self.put("altar", l - 1, cl, 1, aw as usize, Some(far), None, true) {
                        step = Some(s0);
                        break;
                    }
                    self.undo(m);
                }
            }
        }
        let altar = step.is_some();
        let s0 = step.unwrap_or(l);
        if altar && w >= 6 {
            self.pair("statue", l - 1, cl - 2, 1, 1, Some(far), None, true);
        }
        // The pews, their backs to the way in.
        let wall = (w >= 7) as isize;
        let last = if s0 - 2 >= 2 { s0 - 2 } else { s0 - 1 };
        for s in 1..=last {
            for t0 in [wall, wall + 1] {
                if cl - t0 >= 1 && self.pair("pew", s, t0, 1, (cl - t0) as usize, Some((-1, 0)), None, false) {
                    break;
                }
            }
        }
        // The runner up the aisle: the rows free on it that reach the step.
        let free = |h: &Hall, s: isize| (cl..=cr).all(|t| h.at(s, t).is_some_and(|k| (h.ok[k] || h.foot[k]) && h.held[k] == 0));
        let mut a = s0 - 1;
        while a > 0 && free(self, a - 1) {
            a -= 1;
        }
        if free(self, s0 - 1) && s0 - a >= 2 {
            self.put("carpet_runner", a, cl, (s0 - a) as usize, aw as usize, None, Some(rng.below(4) as u8), false);
        }
        for (s, t) in [(0, 0), (0, w - 1), (1, 0), (1, w - 1)] {
            if self.put("font", s, t, 1, 1, None, None, false) {
                break;
            }
        }
        altar
    }

    /// `tables` for one lane over `rows`.
    fn lane(&mut self, lane: &[isize; 3], rows: (isize, isize)) {
        let w = self.f.wid as isize;
        {
            let mut from = rows.0;
            while from <= rows.1 {
                // (The next run of free rows on the table's column.)
                let Some(a0) = (from..=rows.1).find(|&q| self.run(&[lane[1]], (q, q)) == (q, q)) else { break };
                let b0 = (a0..=rows.1).take_while(|&q| self.run(&[lane[1]], (q, q)) == (q, q)).last().unwrap_or(a0);
                from = b0 + 1;
                let (mut a, mut b) = (a0, b0);
                while b - a + 1 >= 2 {
                    let m = self.mark();
                    let n = b - a + 1;
                    let parts: Vec<(isize, isize)> = if n > 8 { vec![(a, a + (n - 1) / 2 - 1), (a + (n - 1) / 2 + 1, b)] } else { vec![(a, b)] };
                    let mut ok = true;
                    for &(p0, p1) in &parts {
                        ok = ok && self.pair("long_table", p0, lane[1], (p1 - p0 + 1) as usize, 1, None, None, false);
                    }
                    if ok {
                        // (Its benches, each run of free rows beside it; a lane down the axis' benches
                        // each other's mirror.)
                        for &(p0, p1) in &parts {
                            for &c in [lane[0], lane[2]].iter().filter(|&&c| c >= 0 && !(c == lane[2] && c == w - 1 - lane[0])) {
                                let mut s = p0;
                                while s <= p1 {
                                    if let Some(r1) = (s..=p1).take_while(|&q| self.run(&[c], (q, q)) == (q, q)).last() {
                                        self.pair("bench", s, c, (r1 - s + 1) as usize, 1, None, None, false);
                                        s = r1;
                                    }
                                    s += 1;
                                }
                            }
                        }
                        break;
                    }
                    self.undo(m);
                    if (b - a) % 2 == 0 { b -= 1 } else { a += 1 }
                }
            }
        }
    }
}

/// Furnish room `ri` of `lvl` as a hall in frame `f` (`whole`: squares the outer wall as drawn leaves
/// whole; `seed` picks scenes and colours). The pieces are added to `lvl.furniture`. The rest of the
/// floor is furnished by the shared rules (`super::furnish_rooms`, this room left out). False where a
/// hall that has one had no room for its dais.
#[allow(clippy::too_many_arguments)]
pub fn furnish_hall(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], ri: i16, f: Frame, style: Style, whole: Option<&[bool]>, seed: u64) -> bool {
    let st = if lvl.has_stairs { st } else { [0, 0, 0, 0] };
    let mut rng = Pcg32::new(hash3(seed, ri as i64, 0x4a11, 0), 91);
    let room: Vec<bool> = lvl.cells.iter().map(|&c| c == ri).collect();
    let mut ok: Vec<bool> = (0..nx * ny).map(|k| room[k] && whole.is_none_or(|w| w[k])).collect();
    let mut clear = |i: isize, j: isize| {
        if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
            ok[j as usize * nx + i as usize] = false;
        }
    };
    // (The stairs and the square at each end of them, the squares either side of each door, what
    // stands in the hall already and a square round it.)
    for j in st[1]..st[1] + st[3] {
        for i in st[0]..st[0] + st[2] {
            clear(i as isize, j as isize);
        }
    }
    // (Their foot only where they stand in the hall: beyond a wall it is no way to them.)
    let ends: Vec<(isize, isize)> = if st[2] > 0 && lvl.cells[st[1] * nx + st[0]] == ri {
        vec![(st[0] as isize - 1, st[1] as isize), ((st[0] + st[2]) as isize, st[1] as isize), (st[0] as isize, st[1] as isize - 1), (st[0] as isize, (st[1] + st[3]) as isize)]
    } else {
        Vec::new()
    };
    for it in lvl.furniture.iter().filter(|it| room[it.y as usize * nx + it.x as usize]) {
        for j in it.y as isize - 1..(it.y + it.h) as isize + 1 {
            for i in it.x as isize - 1..(it.x + it.w) as isize + 1 {
                clear(i, j);
            }
        }
    }
    let mut door = vec![false; nx * ny];
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        for (i, j) in sq {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && room[j as usize * nx + i as usize] {
                door[j as usize * nx + i as usize] = true;
            }
        }
    }
    // (A platform is walked over: in a doorway too.)
    let deck = ok.clone();
    for k in 0..nx * ny {
        ok[k] &= !door[k];
    }
    // (The foot of the stairs: what lies flat may lie there, a platform too.)
    let mut foot = vec![false; nx * ny];
    for (i, j) in ends {
        if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
            let k = j as usize * nx + i as usize;
            (foot[k], ok[k]) = (ok[k], false);
        }
    }
    let entries = entry_squares(lvl, nx, ny, st, ri);
    let mut h = Hall { nx, ny, f, room, ok, held: vec![0; nx * ny], foot, door, deck, blocked: vec![false; nx * ny], entries, items: Vec::new() };
    let (l, w) = (f.len as isize, f.wid as isize);
    // (A long gallery and a chapel: their own compositions.)
    if style == Style::Gallery {
        h.gallery(&mut rng);
        lvl.furniture.extend(h.items);
        return true;
    }
    if style == Style::Chapel {
        let ok = h.chapel(&mut rng);
        lvl.furniture.extend(h.items);
        return ok;
    }
    let mess = matches!(style, Style::Mess { .. });
    // (A throne room's, an audience chamber's and a ballroom's floor clear of tables; a throne in a
    // castle's hall or a palace's throne room.)
    let bare = matches!(style, Style::Throne { .. } | Style::Ballroom | Style::Presence { .. });
    let throne = matches!(style, Style::Castle { .. } | Style::Throne { .. });
    if l < 3 || w < 2 {
        return mess || style == Style::Ballroom;
    }
    // The axis: the middle column, or the two middle ones.
    let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
    // (Toward the far wall, toward the low-t wall.)
    let (far, side) = ((1, 0), (0, -1));
    // The tables' lanes from the axis out (a mess hall's first down the axis itself), a column clear
    // by each long wall where the hall is wide enough.
    let mut lanes: Vec<[isize; 3]> = Vec::new();
    // (The next lane's first column toward t = 0: past the runner, or past a mess hall's lane down
    // the axis and a way beside it.)
    let mut c = if !mess {
        cl - 1
    } else if w >= 3 {
        // (Down the axis: a table on the middle column, or two side by side on the middle two.)
        lanes.push(if w % 2 == 1 { [cl - 1, cl, cl + 1] } else { [cl - 1, cl, -1] });
        cl - 3
    } else {
        cl - 2
    };
    // (A column clear by each long wall in a hall 7 wide or more; a mess hall's tables along its
    // walls too.)
    let wall = (cl >= 3 && !mess) as isize;
    while c - wall >= 1 && !bare {
        if c - wall >= 2 {
            lanes.push([c, c - 1, if c - 2 >= wall { c - 2 } else { -1 }]);
            c -= 4;
        } else {
            lanes.push([c, c - 1, -1]);
            c -= 3;
        }
    }
    // (The outermost lane's column: the lanes span the hall from it to its mirror.)
    let outer_col = lanes.iter().flat_map(|l| l.iter().copied()).filter(|&c| c >= 0).min().unwrap_or(cl);
    // The far end: the dais on the axis against the far wall (past the frame's end where the hall
    // goes on to it there), a castle's throne dais a row in from it where a door opens onto the wall
    // behind it (the way from it round the dais; a low dais is walked over: the door opens onto it);
    // the widest that fits (one square wide only where no wider one does): a throne dais up to 5
    // squares, a low dais as wide as the tables' lanes (the far end the hall's focus, the ways by the
    // walls left clear). A castle's hall with no wall at its far end (a shell keep's curved yard) has
    // its throne dais as near the end as it fits.
    // (Its first row, first column, width; its back row.)
    let mut dais: Option<(isize, isize, usize)> = None;
    let mut back = 0;
    if !mess && style != Style::Ballroom {
        let (kind, deep) = if throne { ("throne_dais", 2) } else { ("low_dais", 2) };
        let most = match style {
            Style::Castle { .. } | Style::Throne { .. } => 5,
            Style::Estate { .. } | Style::Banquet { .. } => (w - 2 * outer_col).max(5) as usize,
            Style::Presence { .. } => if w % 2 == 0 { 4 } else { 3 },
            _ => (w - 2 * outer_col).max(if w % 2 == 0 { 4 } else { 3 }) as usize,
        };
        let look = match style {
            Style::Castle { dais, .. } | Style::Throne { dais, .. } => dais,
            Style::Civic { stone } => (!stone) as u8,
            Style::Banquet { .. } | Style::Presence { .. } => 0,
            _ => 1,
        };
        let mut widths: Vec<usize> = (1..=most.min(w as usize - 2).max(1)).rev().filter(|d| d % 2 == w as usize % 2).collect();
        if widths.is_empty() {
            widths.push(w as usize);
        }
        // (One square deep only where nothing deeper fits. Its depth, width and the rows g between its
        // back row and the frame's end: against the wall at it, a row past it or two where the hall
        // goes on there (g -1, -2), a row in where a door needs the way (1); then a castle's anywhere
        // a row or two in.)
        let sizes: Vec<(usize, usize)> = [deep, 1].into_iter().flat_map(|d| widths.iter().map(move |&w| (d, w))).collect();
        let mut tries: Vec<(usize, usize, isize)> = sizes.iter().flat_map(|&(d, w)| [0, -1, -2, 1].map(|g| (d, w, g))).collect();
        let near = tries.len();
        if throne {
            tries.extend(sizes.iter().flat_map(|&(d, w)| [1, 2].map(|g| (d, w, g))));
        }
        for (i, &(deep, dw, g)) in tries.iter().enumerate() {
            let t0 = (w - dw as isize) / 2;
            let s0 = l - deep as isize - g;
            if s0 < 1 {
                continue;
            }
            if i < near {
                // (Against the wall; a row clear behind a throne dais only where a door on the far wall
                // opens behind it.)
                let walled = g <= 0 && (t0..t0 + dw as isize).all(|t| h.walled(l - 1 - g, t, far));
                let door = g == 1 && !is_platform(kind) && (t0..t0 + dw as isize).any(|t| h.at(l - 1, t).is_some_and(|k| h.door[k]));
                if !walled && !door {
                    continue;
                }
            }
            if h.put(kind, s0, t0, deep, dw, Some(far), Some(look), false) {
                dais = Some((s0, t0, dw));
                back = s0 + deep as isize - 1;
                break;
            }
        }
    }
    // On the dais and beside it. (A chair of office on its middle column; where two share the
    // middle, on the one the door at the low end opens by, else the low-t one.)
    let door_at = |h: &Hall, t: isize| h.at(0, t).is_some_and(|k| h.entries.contains(&k));
    let cm = if cl != cr && door_at(&h, cr) && !door_at(&h, cl) { cr } else { cl };
    // (Its spots, first to last: its back row's middle, the other middle column, the front row's
    // middle; a door on the far wall may open onto a low dais.)
    let chair = |h: &mut Hall, rows: &[isize], v: u8| {
        let spots = rows.iter().flat_map(|&s| [(s, cm), (s, cl + cr - cm)]);
        for (s, t) in spots {
            if h.put("chair_of_office", s, t, 1, 1, Some(far), Some(v), true) {
                break;
            }
        }
    };
    if let Some((s0, t0, dw)) = dais {
        let dw = dw as isize;
        match style {
            Style::Castle { house, .. } | Style::Throne { house, .. } => {
                // Standing banners either side of it, braziers beyond them (by its back row, else
                // its front one).
                for (kind, off, v) in [("standing_banner", 1, Some(house)), ("brazier", 2, None)] {
                    for s in [back, s0] {
                        if h.pair(kind, s, t0 - off, 1, 1, None, v, false) {
                            break;
                        }
                    }
                }
            }
            Style::Civic { stone } => chair(&mut h, &[back, s0], if stone { 0 } else { 1 }),
            Style::Presence { house } => {
                // The chair of state on its middle, standing banners either side of it.
                chair(&mut h, &[back, s0], 0);
                for s in [back, s0] {
                    if h.pair("standing_banner", s, t0 - 1, 1, 1, None, Some(house), false) {
                        break;
                    }
                }
            }
            Style::Estate { .. } | Style::Banquet { .. } => {
                // The high table along its front, an estate's chair of office behind its middle (the
                // dais's ends left clear, the way up to it).
                let tl = if dw >= 4 { dw - 2 } else { dw };
                if let Style::Estate { .. } = style {
                    chair(&mut h, &[back], 1);
                }
                if back > s0 {
                    h.put("high_table", s0, (w - tl) / 2, 1, tl as usize, Some(far), Some(rng.below(4) as u8), true);
                }
            }
            Style::Mess { .. } | Style::Ballroom | Style::Gallery | Style::Chapel => {}
        }
    }
    // The rows before the far end: the tables', then a row clear before the dais (and before a
    // castle's high table there, where the hall is long enough for both).
    let s_end = dais.map_or(l, |d| d.0);
    let n_rows = s_end - 1;
    let high_front = matches!(style, Style::Castle { .. }) && dais.is_some() && n_rows >= 5 && cl >= 3;
    let rows = if mess {
        // (A mess hall's tables from end to end, a row clear at each in a long one.)
        if l >= 7 { (1, l - 2) } else { (0, l - 1) }
    } else {
        (1, if high_front { s_end - 3 } else if n_rows >= 4 { s_end - 2 } else { s_end - 1 })
    };
    // The runner up the axis from the door's end to the dais: the longest run of free rows on the
    // middle columns that reaches it.
    if !mess && dais.is_some() {
        let mid = (cr - cl + 1) as usize;
        let free = |h: &Hall, s: isize| (cl..=cr).all(|t| h.at(s, t).is_some_and(|k| (h.ok[k] || h.foot[k]) && h.held[k] == 0));
        let end = s_end - 1;
        let mut a = end;
        while a > 1 && free(&h, a - 1) {
            a -= 1;
        }
        if free(&h, end) && end - a + 1 >= 2 {
            h.put("carpet_runner", a, cl, (end - a + 1) as usize, mid, None, Some(rng.below(4) as u8), false);
        }
    }
    // A ballroom: the musicians' corner on the axis against the far wall with a harpsichord each side
    // of it, the dance floor in the middle (a way two squares wide left round it), refreshment tables
    // by the long walls at the door end.
    if style == Style::Ballroom {
        let v = Some(rng.below(4) as u8);
        let mt = if w % 2 == 0 { 2 } else { 1 };
        let t0 = (w - mt) / 2;
        if (t0..t0 + mt).all(|t| h.walled(l - 1, t, far)) && h.put("musicians_corner", l - 2, t0, 2, mt as usize, Some(far), v, false) && t0 >= 2 {
            h.pair("harpsichord", l - 2, t0 - 2, 2, 1, Some(far), None, false);
        }
        // (A square's way each side in a narrow one.)
        let dw = if w >= 8 { (w - 4).min(6) } else { w - 2 };
        let dw = dw - (dw - w).rem_euclid(2);
        let dl = (l - 4).min(30 / dw.max(1)).min(6);
        if dw >= 2 && dl >= 2 {
            h.put("dance_floor", 1 + (l - 3 - dl) / 2, (w - dw) / 2, dl as usize, dw as usize, None, v, false);
        }
        for len in [3usize, 2] {
            if h.pair("refreshment_table", 1, 0, len, 1, Some(side), None, false) {
                break;
            }
        }
    }
    // A castle's high table: before the dais in two halves either side of the way up the axis, or
    // beside it, its back to the far wall (`again`: a banqueting hall's laid out again, its halves
    // before the dais in a short hall too, shorter where its walls leave no room).
    let castle = matches!(style, Style::Castle { .. });
    let banquet = matches!(style, Style::Castle { banquet: true, .. });
    let high = |h: &mut Hall, rng: &mut Pcg32, again: bool| -> bool {
        let Some((s0, t0, _)) = dais else { return false };
        let v = Some(rng.below(4) as u8);
        let hl = (cl - 1).min(3);
        let least = if again { 2 } else { hl };
        if (high_front || again && n_rows >= 3) && hl >= 2 && (least..=hl).rev().any(|n| h.pair("high_table", s0 - 1, cl - n, 1, n as usize, Some(far), v, false)) {
            return true;
        }
        for len in [3isize, 2] {
            for s in [back, s0] {
                for t in (0..=t0 - 1 - len).rev() {
                    if h.pair("high_table", s, t, 1, len as usize, Some(far), v, false) {
                        return true;
                    }
                }
            }
        }
        // (Laid out again: where the walls give no room for both halves, one across the axis before
        // the dais, else one before it by the way up to it, else beside it.)
        if again && n_rows >= 3 {
            let w = h.f.wid as isize;
            // (Across the axis: the runner up to the dais a row shorter, ending at it.)
            let m = h.mark();
            let runner = h.items.iter().position(|it| it.kind == "carpet_runner");
            let mut run: Option<(isize, Option<u8>)> = None;
            let mut taken: Option<Item> = None;
            if let Some(ri) = runner {
                let it = h.items.remove(ri);
                let inside = |s: isize| {
                    let (i, j) = h.f.grid(s, cl);
                    i >= it.x as isize && j >= it.y as isize && i < (it.x + it.w) as isize && j < (it.y + it.h) as isize
                };
                let a = (0..s0).find(|&s| inside(s)).unwrap_or(s0);
                for y in it.y as usize..(it.y + it.h) as usize {
                    for x in it.x as usize..(it.x + it.w) as usize {
                        h.held[y * h.nx + x] = 0;
                    }
                }
                run = Some((a, it.variant));
                taken = Some(it);
            }
            for len in [3isize, 2, 1] {
                if (w - len) % 2 == 0 && h.put("high_table", s0 - 1, (w - len) / 2, 1, len as usize, Some(far), v, false) {
                    if let Some((a, rv)) = run.filter(|r| s0 - 1 - r.0 >= 2) {
                        h.put("carpet_runner", a, cl, (s0 - 1 - a) as usize, (cr - cl + 1) as usize, None, rv, false);
                    }
                    return true;
                }
            }
            // (Else the runner as it was.)
            h.undo(m);
            if let (Some(ri), Some(it)) = (runner, taken) {
                h.items.insert(ri, it);
            }
            for len in [3isize, 2] {
                let spots = [(s0 - 1, cl - len), (s0 - 1, cr + 1)].into_iter().chain([back, s0].into_iter().flat_map(|s| (0..w).map(move |t| (s, t))));
                for (s, t) in spots {
                    if t >= 0 && (t + len <= cl || t > cr) && h.put("high_table", s, t, 1, len as usize, Some(far), v, false) {
                        return true;
                    }
                }
            }
        }
        false
    };
    // (A great hall that serves as the banqueting hall has its high table: where the tables leave
    // it no room, laid out again with the high table first.)
    let start = h.mark();
    for first in [false, true] {
        if first {
            h.undo(start.clone());
            high(&mut h, &mut rng, true);
        }
        // Tables: the lanes (above), a bench each side of a table.
        if !bare && (rows.1 - rows.0 + 1 >= 2 || s_end >= 3) {
            h.tables(&lanes, rows, if mess { rows } else { (0, s_end - 1) });
        }
        // The hearth as near the middle of a long wall before the dais as the tables leave room for
        // (an outer wall first), with a tapestry across from it.
        let mut sides = [(0isize, (0isize, -1isize)), (w - 1, (0, 1))];
        let outer = |h: &Hall, t: isize, d: (isize, isize)| {
            let (i, j) = h.f.grid(s_end / 2, t + d.1);
            i < 0 || j < 0 || i as usize >= nx || j as usize >= ny || lvl.cells[j as usize * nx + i as usize] < 0
        };
        let (o_hi, o_lo) = (outer(&h, w - 1, (0, 1)), outer(&h, 0, (0, -1)));
        if (o_hi && !o_lo) || (o_hi == o_lo && rng.below(2) == 1) {
            sides.swap(0, 1);
        }
        let mut hearth: Option<(isize, isize)> = None;
        'hearth: for d in [0isize, -1, 1, -2, 2] {
            let s = (s_end - 1) / 2 + d;
            for &(t, dir) in &sides {
                if s >= 1 && s + 1 < s_end && (s..s + 2).all(|q| h.walled(q, t, dir)) && h.put("hearth", s, t, 2, 1, Some(dir), None, false) {
                    hearth = Some((s, t));
                    break 'hearth;
                }
            }
        }
        if let Some((s, t)) = hearth.filter(|_| !mess) {
            let (ot, od) = if t == 0 { (w - 1, (0, 1)) } else { (0, (0, -1)) };
            if (s..s + 2).all(|q| h.walled(q, ot, od)) {
                h.put("tapestry", s, ot, 2, 1, Some(od), Some(rng.below(4) as u8), false);
            }
        }
        if first || !castle || high(&mut h, &mut rng, false) || !banquet {
            break;
        }
    }
    // Banners in rows and tapestries on the long walls, mirrored, to the dais's end: a banner, a gap,
    // a tapestry two squares long, a gap.
    let house = style.house();
    let scene = rng.below(4) as u8;
    let pattern: &[(&str, isize)] = match (style, house) {
        (Style::Mess { .. }, Some(_)) => &[("wall_banner", 1)],
        (Style::Mess { .. }, None) => &[],
        (_, None) => &[("tapestry", 2)],
        _ => &[("wall_banner", 1), ("tapestry", 2)],
    };
    let gap = if mess { 2 } else { 1 };
    let reach = if dais.is_some() { back } else { l - 1 };
    let (mut s, mut n) = (1, 0);
    while s <= reach && !pattern.is_empty() {
        let fits = |h: &Hall, s: isize, ls: isize| (s..s + ls).all(|q| q <= reach && h.walled(q, 0, side) && h.walled(q, w - 1, (0, 1)));
        let (kind, ls) = pattern[n % pattern.len()];
        if fits(&h, s, ls) {
            let v = if kind == "wall_banner" { house } else { Some((scene + (n / 2) as u8) % 4) };
            if h.pair(kind, s, 0, ls as usize, 1, Some(side), v, false) {
                s += ls + gap;
                n += 1;
                continue;
            }
        }
        s += 1;
    }
    // Pillar pairs where a column is free between the outer lane and the long wall's way, every
    // third row they fit on: never before the hearth or what hangs on the walls, nor across the way
    // by a long wall (the row between them and the wall clear, both sides), nor in a door's way (no
    // door's square on that row by the wall, none next to a pillar: the square straight in from it).
    if !mess && w >= 7 {
        // (A hall bare of tables: a column in from the walls, two in a wide one, the column between it
        // and the wall the way.)
        // (Whatever stands or hangs between a pillar and its wall, the wall's column too, keeps it off
        // that row.)
        let p = if bare { if w >= 9 { 2 } else { 1 } } else { outer_col - 1 };
        let lo = 0;
        if p >= 1 && (bare || wall == 1) && !matches!(style, Style::Presence { .. }) {
            let by_door = |h: &Hall, s: isize, t: isize| {
                let (i, j) = h.f.grid(s, t);
                [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].iter().any(|&(a, b)| a >= 0 && b >= 0 && (a as usize) < h.nx && (b as usize) < h.ny && h.door[b as usize * h.nx + a as usize])
            };
            let way = |h: &Hall, s: isize| {
                (lo..p).chain(w - p..w - lo).all(|t| h.at(s, t).is_some_and(|k| h.held[k] == 0 && !h.door[k])) && !by_door(h, s, p) && !by_door(h, s, w - 1 - p)
            };
            let mut s = rows.0 + 1;
            while s < rows.1 {
                s += if way(&h, s) && h.pair("pillar", s, p, 1, 1, None, None, false) { 3 } else { 1 };
            }
        }
    }
    lvl.furniture.extend(h.items);
    mess || style == Style::Ballroom || dais.is_some()
}

/// The musicians' corner on a minstrels' gallery (room `ri`): two squares deep where the gallery is,
/// along the wall away from the hall's open height, as near its middle as it fits.
pub fn furnish_gallery(lvl: &mut Level, nx: usize, ny: usize, ri: i16) {
    let cells = &lvl.cells;
    let mine = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && cells[j as usize * nx + i as usize] == ri;
    let void = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && cells[j as usize * nx + i as usize] >= 0 && lvl.rooms[cells[j as usize * nx + i as usize] as usize].kind == super::OPEN_BELOW;
    let mut door = vec![false; nx * ny];
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        for (i, j) in if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] } {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
                door[j as usize * nx + i as usize] = true;
            }
        }
    }
    let taken: Vec<bool> = {
        let mut t = door;
        for f in &lvl.furniture {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    t[j * nx + i] = true;
                }
            }
        }
        t
    };
    let sq: Vec<usize> = (0..nx * ny).filter(|&k| cells[k] == ri).collect();
    if sq.is_empty() {
        return;
    }
    let mid = [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len() as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len() as f64];
    // (Each spot: its box, and the side its back is to: the one away from the void.)
    let mut best: Option<(f64, Item)> = None;
    for &(w, h) in &[(2usize, 2usize), (2, 1), (1, 2)] {
        for &k in &sq {
            let (x, y) = ((k % nx) as isize, (k / nx) as isize);
            let all: Vec<(isize, isize)> = (0..h as isize).flat_map(|b| (0..w as isize).map(move |a| (x + a, y + b))).collect();
            if !all.iter().all(|&(i, j)| mine(i, j) && !taken[j as usize * nx + i as usize]) {
                continue;
            }
            // (The side facing the void, the back the other way, against a wall.)
            for (dir, code) in [((0isize, -1isize), 0u8), ((1, 0), 1), ((0, 1), 2), ((-1, 0), 3)] {
                let back_edge: Vec<(isize, isize)> = all.iter().map(|&(i, j)| (i + dir.0, j + dir.1)).filter(|p| !all.contains(p)).collect();
                let front_edge: Vec<(isize, isize)> = all.iter().map(|&(i, j)| (i - dir.0, j - dir.1)).filter(|p| !all.contains(p)).collect();
                // (Along a long side of it: the back on a side as long as the piece.)
                if (dir.0 != 0 && h < w) || (dir.1 != 0 && w < h) {
                    continue;
                }
                if !back_edge.iter().all(|&(i, j)| !mine(i, j)) || !front_edge.iter().any(|&(i, j)| void(i, j) || mine(i, j)) {
                    continue;
                }
                let faces_void = front_edge.iter().filter(|&&(i, j)| void(i, j)).count();
                let c = [x as f64 + 0.5 * w as f64, y as f64 + 0.5 * h as f64];
                let score = (w * h) as f64 * 10.0 + faces_void as f64 * 2.0 - (c[0] - mid[0]).abs() - (c[1] - mid[1]).abs();
                if best.as_ref().is_none_or(|b| score > b.0)
                    && let Some(mut it) = building_item("musicians_corner", x as usize, y as usize, w, h)
                {
                    it.back = Some(code);
                    it.variant = Some((k % 4) as u8);
                    best = Some((score, it));
                }
            }
        }
    }
    if let Some((_, it)) = best {
        lvl.furniture.push(it);
    }
}
