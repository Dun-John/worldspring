//! The boss chamber's frame (`Hall`), its stage and the placing helpers every layout uses.

use super::*;

/// The chamber seen from its way in: `d` the depth from the entry wall (0) to the far wall (`l` - 1), `a` across it
/// (0..`w`), over the room's bounding box.
pub(super) struct Hall {
    pub(super) nx: usize,
    pub(super) ny: usize,
    pub(super) ri: i16,
    pub(super) x0: usize,
    pub(super) y0: usize,
    pub(super) x1: usize,
    pub(super) y1: usize,
    /// The wall the way in is on: 0 north, 1 east, 2 south, 3 west (an item's `back`).
    pub(super) side: u8,
    pub(super) l: usize,
    pub(super) w: usize,
    pub(super) entry: usize,
    pub(super) ed: usize,
    pub(super) ea: usize,
    /// Squares kept clear of the centrepiece: round the steps, ladders and bridges, so their landings stay open.
    pub(super) clear: std::cell::RefCell<Vec<usize>>,
    /// The second way in (`side_way`), if any: its doorway square and the square inside it, kept clear of everything.
    pub(super) door: Option<(usize, usize)>,
}

impl Hall {
    pub(super) fn new(p: &Plan, nx: usize, ny: usize, ri: i16, squares: &[usize]) -> Hall {
        let dist = p.distances(p.up, ny, None);
        // (Its main way in, not the second one: `side_door`.)
        let entry = p.side_door.filter(|s| p.cells[s.2] == ri).map(|s| s.2).unwrap_or_else(|| squares.iter().copied().filter(|&k| dist[k] >= 0).min_by_key(|&k| (dist[k], k)).unwrap_or(squares[0]));
        let (x0, y0) = (squares.iter().map(|k| k % nx).min().unwrap_or(0), squares.iter().map(|k| k / nx).min().unwrap_or(0));
        let (x1, y1) = (squares.iter().map(|k| k % nx).max().unwrap_or(0), squares.iter().map(|k| k / nx).max().unwrap_or(0));
        let (ex, ey) = (entry % nx, entry / nx);
        let gaps = [ey - y0, x1 - ex, y1 - ey, ex - x0];
        let side = (0..4).min_by_key(|&s| (gaps[s], s)).unwrap_or(0) as u8;
        let (l, w) = if side.is_multiple_of(2) { (y1 - y0 + 1, x1 - x0 + 1) } else { (x1 - x0 + 1, y1 - y0 + 1) };
        let mut h = Hall { nx, ny, ri, x0, y0, x1, y1, side, l, w, entry, ed: 0, ea: 0, clear: Default::default(), door: p.side_door.filter(|s| p.cells[s.0] == ri).map(|s| (s.0, s.1)) };
        (h.ed, h.ea) = h.frame(entry);
        h
    }

    /// The grid square (x, y) at (d, a).
    pub(super) fn at(&self, d: usize, a: usize) -> (usize, usize) {
        match self.side {
            0 => (self.x0 + a, self.y0 + d),
            2 => (self.x0 + a, self.y1 - d),
            3 => (self.x0 + d, self.y0 + a),
            _ => (self.x1 - d, self.y0 + a),
        }
    }

    /// (d, a) of grid square `k`.
    pub(super) fn frame(&self, k: usize) -> (usize, usize) {
        let (x, y) = (k % self.nx, k / self.nx);
        match self.side {
            0 => (y - self.y0, x - self.x0),
            2 => (self.y1 - y, x - self.x0),
            3 => (x - self.x0, y - self.y0),
            _ => (self.x1 - x, y - self.y0),
        }
    }

    /// The room's square at (d, a), if (d, a) is in the box and on the room's floor.
    pub(super) fn sq(&self, p: &Plan, d: isize, a: isize) -> Option<usize> {
        if d < 0 || a < 0 || d as usize >= self.l || a as usize >= self.w {
            return None;
        }
        let (x, y) = self.at(d as usize, a as usize);
        let k = y * self.nx + x;
        (p.cells[k] == self.ri).then_some(k)
    }

    /// The grid rect (x, y, w, h) of `dl` x `aw` squares from (d, a), if in the box.
    pub(super) fn rect(&self, d: isize, a: isize, dl: usize, aw: usize) -> Option<(usize, usize, usize, usize)> {
        if d < 0 || a < 0 || d as usize + dl > self.l || a as usize + aw > self.w || dl == 0 || aw == 0 {
            return None;
        }
        let (xa, ya) = self.at(d as usize, a as usize);
        let (xb, yb) = self.at(d as usize + dl - 1, a as usize + aw - 1);
        Some((xa.min(xb), ya.min(yb), xa.abs_diff(xb) + 1, ya.abs_diff(yb) + 1))
    }

    /// An item of `kind` over `dl` x `aw` squares from (d, a), its back to `back`.
    pub(super) fn item(&self, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>) -> Option<Item> {
        let (x, y, w, h) = self.rect(d, a, dl, aw)?;
        let mut it = item(kind, x, y, w, h);
        it.back = back;
        Some(it)
    }

    /// Every square of the rect on the room's floor and free.
    pub(super) fn free(&self, p: &Plan, d: isize, a: isize, dl: usize, aw: usize) -> bool {
        (0..dl as isize).all(|i| (0..aw as isize).all(|j| self.sq(p, d + i, a + j).is_some_and(|k| !p.taken[k] && !self.kept(k))))
    }

    /// One of the second way in's squares (`door`).
    pub(super) fn kept(&self, k: usize) -> bool {
        self.door.is_some_and(|(a, b)| k == a || k == b)
    }

    /// The grid square at (d, a), in the box or not (None off the grid).
    pub(super) fn cell(&self, d: isize, a: isize) -> Option<usize> {
        let (x0, y0, x1, y1) = (self.x0 as isize, self.y0 as isize, self.x1 as isize, self.y1 as isize);
        let (x, y) = match self.side {
            0 => (x0 + a, y0 + d),
            2 => (x0 + a, y1 - d),
            3 => (x0 + d, y0 + a),
            _ => (x1 - d, y0 + a),
        };
        (x >= 0 && y >= 0 && (x as usize) < self.nx && (y as usize) < self.ny).then(|| y as usize * self.nx + x as usize)
    }

    pub(super) fn far(&self) -> u8 {
        (self.side + 2) % 4
    }

    /// The side wall at a = 0 and at a = w - 1.
    pub(super) fn walls(&self) -> (u8, u8) {
        if self.side.is_multiple_of(2) { (3, 1) } else { (0, 2) }
    }
}

/// Where the centrepiece goes: a frame rect (its far row the far wall's side) and whether it stands raised (no dais).
#[derive(Clone, Copy)]
pub(super) struct Stage {
    pub(super) d0: isize,
    pub(super) a0: isize,
    pub(super) dl: usize,
    pub(super) aw: usize,
    pub(super) raised: bool,
}

impl Stage {
    pub(super) fn far(&self) -> isize {
        self.d0 + self.dl as isize - 1
    }
    pub(super) fn mid(&self) -> isize {
        self.a0 + self.aw as isize / 2
    }
}

pub(super) struct Snap {
    pub(super) items: usize,
    pub(super) taken: Vec<bool>,
    pub(super) blocked: Vec<bool>,
}

pub(super) fn snap(p: &Plan) -> Snap {
    Snap { items: p.items.len(), taken: p.taken.clone(), blocked: p.blocked.clone() }
}

pub(super) fn undo(p: &mut Plan, s: Snap) {
    p.items.truncate(s.items);
    p.taken = s.taken;
    p.blocked = s.blocked;
}

/// Put `it` where its squares are free floor of the room, or all stand on a dais (on a platform or the arena's
/// gallery too with `top`: the stage is raised) and nothing else, keeping every square reached (`Plan::place`).
pub(super) fn set(p: &mut Plan, h: &Hall, it: Option<Item>, top: bool) -> bool {
    let Some(it) = it else { return false };
    let nx = h.nx;
    let squares: Vec<usize> = (it.y as usize..(it.y + it.h) as usize).flat_map(|j| (it.x as usize..(it.x + it.w) as usize).map(move |i| j * nx + i)).collect();
    let covers = |f: &Item, k: usize| {
        let (i, j) = ((k % nx) as u16, (k / nx) as u16);
        i >= f.x && i < f.x + f.w && j >= f.y && j < f.y + f.h
    };
    let mut lifted = Vec::new();
    for &k in &squares {
        if p.cells[k] != h.ri || h.clear.borrow().contains(&k) || h.kept(k) {
            return false;
        }
        if p.taken[k] {
            let mut on = p.items.iter().filter(|f| covers(f, k));
            let raised = on.clone().count() > 0 && on.all(|f| f.kind == "dais" && ON_DAIS.contains(&it.kind) || top && matches!(f.kind, "platform" | "balcony"));
            if !raised || RAISING.contains(&it.kind) {
                return false;
            }
            lifted.push(k);
        }
    }
    // (All on the floor or all on top.)
    if !lifted.is_empty() && lifted.len() < squares.len() {
        return false;
    }
    for &k in &lifted {
        p.taken[k] = false;
    }
    let ny = h.ny;
    if p.place(it, Some(h.ri), ny) {
        return true;
    }
    for &k in &lifted {
        p.taken[k] = true;
    }
    false
}

/// What stands on a dais (a bier): the throne or the sarcophagus, nothing else.
pub(super) const ON_DAIS: [&str; 2] = ["throne", "sarcophagus"];

/// `set` at (d, a) or the nearest spot round it (up to 3 squares off): where it went.
#[allow(clippy::too_many_arguments)]
pub(super) fn near(p: &mut Plan, h: &Hall, top: bool, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>) -> Option<(isize, isize)> {
    near_in(p, h, top, kind, d, a, dl, aw, back, 3)
}

/// `near` up to `r` squares off.
#[allow(clippy::too_many_arguments)]
pub(super) fn near_in(p: &mut Plan, h: &Hall, top: bool, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>, r: isize) -> Option<(isize, isize)> {
    let mut offs: Vec<(isize, isize)> = (-r..=r).flat_map(|i| (-r..=r).map(move |j| (i, j))).collect();
    offs.sort_by_key(|&(i, j)| (i.abs() + j.abs(), i.abs(), -i, j.abs(), j));
    offs.into_iter().map(|(i, j)| (d + i, a + j)).find(|&(d, a)| set(p, h, h.item(kind, d, a, dl, aw, back), top))
}

/// Terrain and raised floors, without checks but that their squares are free floor of the room.
pub(super) fn lay(p: &mut Plan, h: &Hall, items: Vec<Option<Item>>) -> bool {
    let Some(items) = items.into_iter().collect::<Option<Vec<Item>>>() else { return false };
    let nx = h.nx;
    let mut seen = Vec::new();
    for it in &items {
        for j in it.y as usize..(it.y + it.h) as usize {
            for i in it.x as usize..(it.x + it.w) as usize {
                let k = j * nx + i;
                let span = SPANS.contains(&it.kind);
                if p.cells[k] != h.ri || p.taken[k] || (!span && seen.contains(&k)) {
                    return false;
                }
                seen.push(k);
            }
        }
    }
    for it in items.iter().filter(|it| RAMPS.contains(&it.kind) || SPANS.contains(&it.kind)) {
        for j in it.y as usize..(it.y + it.h) as usize {
            for i in it.x as usize..(it.x + it.w) as usize {
                h.clear.borrow_mut().extend(neighbours(j * nx + i, nx, h.ny));
            }
        }
    }
    p.put_all(items);
    true
}

/// The room's squares reached from its way in without a climb (`interior::height`), round what blocks and the hazards
/// (`PERILS`, lava: never the only way across): all of them.
pub(super) fn reached(p: &Plan, h: &Hall) -> bool {
    let (nx, ny) = (h.nx, h.ny);
    let f = p.floor(ny);
    let mut blocked = blocked_squares(&p.items, nx, ny);
    for it in p.items.iter().filter(|it| it.kind == "lava" || PERILS.contains(&it.name)) {
        for j in it.y as usize..(it.y + it.h) as usize {
            for i in it.x as usize..(it.x + it.w) as usize {
                blocked[j * nx + i] = true;
            }
        }
    }
    if blocked[h.entry] {
        return false;
    }
    let mut seen = vec![false; nx * ny];
    seen[h.entry] = true;
    let mut stack = vec![h.entry];
    while let Some(k) = stack.pop() {
        for m in neighbours(k, nx, ny) {
            if !seen[m] && p.cells[m] == h.ri && !blocked[m] && p.passable(k, m) && f.sheer(k, m) <= 0.0 {
                seen[m] = true;
                stack.push(m);
            }
        }
    }
    (0..nx * ny).all(|k| p.cells[k] != h.ri || blocked[k] || seen[k])
}

/// The far end's stage: up to 3 deep and 5 across, round the middle.
pub(super) fn far_stage(h: &Hall) -> Stage {
    let dl = if h.l >= 10 { 3 } else { 2 };
    let aw = h.w.min(5);
    Stage { d0: (h.l - dl) as isize, a0: ((h.w - aw) / 2) as isize, dl, aw, raised: false }
}
