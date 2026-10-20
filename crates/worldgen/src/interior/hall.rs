//! Great halls furnished as one composition about their axis (`furnish_hall`), before the shared
//! rules furnish the rest of a floor: the axis runs from the end the hall is entered at to the far
//! end; the dais at the far end on the axis (a castle's throne dais, or a low dais with a chair of
//! office, a noble estate's carrying the high table), standing banners and braziers flanking a
//! throne, a castle's high table before it; a carpet runner up the axis to it; long tables with
//! benches down both sides, mirrored about the axis; pillar pairs where the hall keeps a clear way
//! by its walls; the hearth centred on a long wall; banners in rows and tapestries on the long
//! walls, mirrored. A mess hall has its tables and hearth only. Every piece that blocks the way is
//! put down only where the hall stays whole: its doors and stairs joined, no square of its floor
//! shut off (`Hall::open`). Kept general (a frame, a style): throne rooms, ballrooms, naves and
//! refectories are to be furnished with it too.

use super::{Item, Level, building_item, entry_squares};
use crate::core::rng::{Pcg32, hash3};

/// How a hall is furnished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// A castle's great hall: its throne dais (`dais`: its look, `super::item_looks`), standing
    /// banners and braziers flanking it, the high table before it, banners of `house`.
    Castle { dais: u8, house: u8 },
    /// A town hall's, a guild's, a harbor master's or an auction house's: a low dais (of stone, else
    /// timber) with a chair of office at its middle.
    Civic { stone: bool },
    /// A noble estate's hall: a low timber dais carrying the high table, a chair of office at its
    /// middle behind it; banners of the family's `house`.
    Estate { house: u8 },
    /// A mess hall (and any other hall: a guard post's, an orphanage's): long tables and benches and
    /// the hearth only; banners of a castle's `house` in a castle's.
    Mess { house: Option<u8> },
}

impl Style {
    fn house(self) -> Option<u8> {
        match self {
            Style::Castle { house, .. } | Style::Estate { house } => Some(house),
            Style::Mess { house } => house,
            Style::Civic { .. } => None,
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

/// A platform others stand on (`low_dais`): the one item another may overlap.
pub fn is_platform(kind: &str) -> bool {
    kind == "low_dais"
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
    let mess = matches!(style, Style::Mess { .. });
    if l < 3 || w < 2 {
        return mess;
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
    while c - wall >= 1 {
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
    if !mess {
        let (kind, deep) = if matches!(style, Style::Castle { .. }) { ("throne_dais", 2) } else { ("low_dais", 2) };
        let most = match style {
            Style::Castle { .. } => 5,
            Style::Estate { .. } => (w - 2 * outer_col).max(5) as usize,
            _ => (w - 2 * outer_col).max(if w % 2 == 0 { 4 } else { 3 }) as usize,
        };
        let look = match style {
            Style::Castle { dais, .. } => dais,
            Style::Civic { stone } => (!stone) as u8,
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
        if matches!(style, Style::Castle { .. }) {
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
            Style::Castle { house, .. } => {
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
            Style::Estate { .. } => {
                // The high table along its front, the chair of office behind its middle (the dais's
                // ends left clear, the way up to it).
                let tl = if dw >= 4 { dw - 2 } else { dw };
                chair(&mut h, &[back], 1);
                if back > s0 {
                    h.put("high_table", s0, (w - tl) / 2, 1, tl as usize, Some(far), Some(rng.below(4) as u8), true);
                }
            }
            Style::Mess { .. } => {}
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
    // Tables: the lanes (above), a bench each side of a table.
    if rows.1 - rows.0 + 1 >= 2 || s_end >= 3 {
        h.tables(&lanes, rows, if mess { rows } else { (0, s_end - 1) });
    }
    // The hearth as near the middle of a long wall before the dais as the tables leave room for (an
    // outer wall first), with a tapestry across from it.
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
    // A castle's high table: before the dais in two halves either side of the way up the axis, or
    // beside it, its back to the far wall.
    if let (Style::Castle { .. }, Some((s0, t0, _))) = (style, dais) {
        let v = Some(rng.below(4) as u8);
        let hl = (cl - 1).min(3);
        if !(high_front && hl >= 2 && h.pair("high_table", s0 - 1, cl - hl, 1, hl as usize, Some(far), v, false)) {
            'beside: for len in [3isize, 2] {
                for s in [back, s0] {
                    for t in (0..=t0 - 1 - len).rev() {
                        if h.pair("high_table", s, t, 1, len as usize, Some(far), v, false) {
                            break 'beside;
                        }
                    }
                }
            }
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
        let p = outer_col - 1;
        if p >= 1 && wall == 1 {
            let by_door = |h: &Hall, s: isize, t: isize| {
                let (i, j) = h.f.grid(s, t);
                [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].iter().any(|&(a, b)| a >= 0 && b >= 0 && (a as usize) < h.nx && (b as usize) < h.ny && h.door[b as usize * h.nx + a as usize])
            };
            let way = |h: &Hall, s: isize| {
                (0..p).chain(w - p..w).all(|t| h.at(s, t).is_some_and(|k| h.held[k] == 0 && !h.door[k])) && !by_door(h, s, p) && !by_door(h, s, w - 1 - p)
            };
            let mut s = rows.0 + 1;
            while s < rows.1 {
                s += if way(&h, s) && h.pair("pillar", s, p, 1, 1, None, None, false) { 3 } else { 1 };
            }
        }
    }
    lvl.furniture.extend(h.items);
    mess || dais.is_some()
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
