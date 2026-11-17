//! A built site's boss chamber (item 117): a tactical layout picked per site by the rng and by fit (cover: a pillar
//! grid, broken walls, a statue gallery, a rubble ring; obstacles: pits, a chasm with its bridge, a moat round an
//! island; height: a raised platform, balconies, a sunken arena), then the theme's centrepiece (a throne only where a
//! ruler fits, a ritual circle only in temples and cult sanctums) and dressing. Heights go through the floor model
//! (`interior::height`: `platform`/`balcony` raise, `steps`/`climb_ladder` are ramps). Caves and lava tubes get the
//! natural set instead, mines a half-dug face with scaffolding (`wild`). Any of them may get hazards (`hazards`) and a
//! second way in (`side_door`, picked before the level is furnished).

use crate::core::rng::Pcg32;
use crate::interior::Item;
use crate::interior::height::{RAISING, RAMPS};
use crate::under::*;

/// The built layouts, as the `under` example's metrics name them.
pub const LAYOUTS: [&str; 10] = ["pillar grid", "broken walls", "statue gallery", "rubble ring", "pits", "chasm and bridge", "moat", "raised platform", "balconies", "sunken arena"];

/// The least room each layout needs: (depth from the way in, width across).
const FIT: [(usize, usize); 10] = [(5, 5), (5, 5), (6, 5), (8, 7), (6, 5), (7, 4), (7, 5), (6, 5), (6, 5), (8, 7)];

/// The chamber seen from its way in: `d` the depth from the entry wall (0) to the far wall (`l` - 1), `a` across it
/// (0..`w`), over the room's bounding box.
struct Hall {
    nx: usize,
    ny: usize,
    ri: i16,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    /// The wall the way in is on: 0 north, 1 east, 2 south, 3 west (an item's `back`).
    side: u8,
    l: usize,
    w: usize,
    entry: usize,
    ed: usize,
    ea: usize,
    /// Squares kept clear of the centrepiece: round the steps, ladders and bridges, so their landings stay open.
    clear: std::cell::RefCell<Vec<usize>>,
    /// The second way in (`side_way`), if any: its doorway square and the square inside it, kept clear of everything.
    door: Option<(usize, usize)>,
}

impl Hall {
    fn new(p: &Plan, nx: usize, ny: usize, ri: i16, squares: &[usize]) -> Hall {
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
    fn at(&self, d: usize, a: usize) -> (usize, usize) {
        match self.side {
            0 => (self.x0 + a, self.y0 + d),
            2 => (self.x0 + a, self.y1 - d),
            3 => (self.x0 + d, self.y0 + a),
            _ => (self.x1 - d, self.y0 + a),
        }
    }

    /// (d, a) of grid square `k`.
    fn frame(&self, k: usize) -> (usize, usize) {
        let (x, y) = (k % self.nx, k / self.nx);
        match self.side {
            0 => (y - self.y0, x - self.x0),
            2 => (self.y1 - y, x - self.x0),
            3 => (x - self.x0, y - self.y0),
            _ => (self.x1 - x, y - self.y0),
        }
    }

    /// The room's square at (d, a), if (d, a) is in the box and on the room's floor.
    fn sq(&self, p: &Plan, d: isize, a: isize) -> Option<usize> {
        if d < 0 || a < 0 || d as usize >= self.l || a as usize >= self.w {
            return None;
        }
        let (x, y) = self.at(d as usize, a as usize);
        let k = y * self.nx + x;
        (p.cells[k] == self.ri).then_some(k)
    }

    /// The grid rect (x, y, w, h) of `dl` x `aw` squares from (d, a), if in the box.
    fn rect(&self, d: isize, a: isize, dl: usize, aw: usize) -> Option<(usize, usize, usize, usize)> {
        if d < 0 || a < 0 || d as usize + dl > self.l || a as usize + aw > self.w || dl == 0 || aw == 0 {
            return None;
        }
        let (xa, ya) = self.at(d as usize, a as usize);
        let (xb, yb) = self.at(d as usize + dl - 1, a as usize + aw - 1);
        Some((xa.min(xb), ya.min(yb), xa.abs_diff(xb) + 1, ya.abs_diff(yb) + 1))
    }

    /// An item of `kind` over `dl` x `aw` squares from (d, a), its back to `back`.
    fn item(&self, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>) -> Option<Item> {
        let (x, y, w, h) = self.rect(d, a, dl, aw)?;
        let mut it = item(kind, x, y, w, h);
        it.back = back;
        Some(it)
    }

    /// Every square of the rect on the room's floor and free.
    fn free(&self, p: &Plan, d: isize, a: isize, dl: usize, aw: usize) -> bool {
        (0..dl as isize).all(|i| (0..aw as isize).all(|j| self.sq(p, d + i, a + j).is_some_and(|k| !p.taken[k] && !self.kept(k))))
    }

    /// One of the second way in's squares (`door`).
    fn kept(&self, k: usize) -> bool {
        self.door.is_some_and(|(a, b)| k == a || k == b)
    }

    /// The grid square at (d, a), in the box or not (None off the grid).
    fn cell(&self, d: isize, a: isize) -> Option<usize> {
        let (x0, y0, x1, y1) = (self.x0 as isize, self.y0 as isize, self.x1 as isize, self.y1 as isize);
        let (x, y) = match self.side {
            0 => (x0 + a, y0 + d),
            2 => (x0 + a, y1 - d),
            3 => (x0 + d, y0 + a),
            _ => (x1 - d, y0 + a),
        };
        (x >= 0 && y >= 0 && (x as usize) < self.nx && (y as usize) < self.ny).then(|| y as usize * self.nx + x as usize)
    }

    fn far(&self) -> u8 {
        (self.side + 2) % 4
    }

    /// The side wall at a = 0 and at a = w - 1.
    fn walls(&self) -> (u8, u8) {
        if self.side.is_multiple_of(2) { (3, 1) } else { (0, 2) }
    }
}

/// Where the centrepiece goes: a frame rect (its far row the far wall's side) and whether it stands raised (no dais).
#[derive(Clone, Copy)]
struct Stage {
    d0: isize,
    a0: isize,
    dl: usize,
    aw: usize,
    raised: bool,
}

impl Stage {
    fn far(&self) -> isize {
        self.d0 + self.dl as isize - 1
    }
    fn mid(&self) -> isize {
        self.a0 + self.aw as isize / 2
    }
}

struct Snap {
    items: usize,
    taken: Vec<bool>,
    blocked: Vec<bool>,
}

fn snap(p: &Plan) -> Snap {
    Snap { items: p.items.len(), taken: p.taken.clone(), blocked: p.blocked.clone() }
}

fn undo(p: &mut Plan, s: Snap) {
    p.items.truncate(s.items);
    p.taken = s.taken;
    p.blocked = s.blocked;
}

/// Put `it` where its squares are free floor of the room, or all stand on a dais (on a platform or the arena's
/// gallery too with `top`: the stage is raised) and nothing else, keeping every square reached (`Plan::place`).
fn set(p: &mut Plan, h: &Hall, it: Option<Item>, top: bool) -> bool {
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
const ON_DAIS: [&str; 2] = ["throne", "sarcophagus"];

/// `set` at (d, a) or the nearest spot round it (up to 3 squares off): where it went.
#[allow(clippy::too_many_arguments)]
fn near(p: &mut Plan, h: &Hall, top: bool, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>) -> Option<(isize, isize)> {
    near_in(p, h, top, kind, d, a, dl, aw, back, 3)
}

/// `near` up to `r` squares off.
#[allow(clippy::too_many_arguments)]
fn near_in(p: &mut Plan, h: &Hall, top: bool, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>, r: isize) -> Option<(isize, isize)> {
    let mut offs: Vec<(isize, isize)> = (-r..=r).flat_map(|i| (-r..=r).map(move |j| (i, j))).collect();
    offs.sort_by_key(|&(i, j)| (i.abs() + j.abs(), i.abs(), -i, j.abs(), j));
    offs.into_iter().map(|(i, j)| (d + i, a + j)).find(|&(d, a)| set(p, h, h.item(kind, d, a, dl, aw, back), top))
}

/// Terrain and raised floors, without checks but that their squares are free floor of the room.
fn lay(p: &mut Plan, h: &Hall, items: Vec<Option<Item>>) -> bool {
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
fn reached(p: &Plan, h: &Hall) -> bool {
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
fn far_stage(h: &Hall) -> Stage {
    let dl = if h.l >= 10 { 3 } else { 2 };
    let aw = h.w.min(5);
    Stage { d0: (h.l - dl) as isize, a0: ((h.w - aw) / 2) as isize, dl, aw, raised: false }
}

/// The boss chamber of a built site (not a mine): maybe a second way in, a layout, the theme's centrepiece and its
/// dressing, maybe hazards.
pub(crate) fn built(p: &mut Plan, nx: usize, ny: usize, ri: i16, squares: &[usize], rng: &mut Pcg32) {
    let h = Hall::new(p, nx, ny, ri, squares);
    let theme = p.theme;
    let mut order: Vec<usize> = (0..LAYOUTS.len()).filter(|&i| h.l >= FIT[i].0 && h.w >= FIT[i].1).collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i as u32 + 1) as usize);
    }
    // (A flooded vault's strongroom stands on an island.)
    if theme == "flooded_vault" && let Some(m) = order.iter().position(|&i| LAYOUTS[i] == "moat") {
        order[..=m].rotate_right(1);
    }
    let fallback = far_stage(&h);
    let (stage, dressed) = choose(p, &h, order, rng, &|p, h, i, rng| layout(p, h, LAYOUTS[i], theme, rng), &|p, h, s, rng| centrepiece(p, h, s, theme, rng), fallback);
    spread(p, &h, stage, cover_kinds(theme, false), rng);
    let mut kinds = vec!["cracked floor", "pressure plate"];
    match theme {
        "dwarven_hall" => kinds.push("steam vent"),
        "flooded_vault" | "sunken_temple" => kinds.push("flooded pit"),
        _ => {}
    }
    hazards(p, &h, &kinds, stage, dressed, rng);
}

/// The layouts in `order` tried in turn (`lay_out`, then `dress`), each kept if the chamber is all reached (`reached`),
/// else undone; last the `fallback` stage, dressed. The stage used and the index of the dressing's first item.
#[allow(clippy::type_complexity)]
fn choose(
    p: &mut Plan,
    h: &Hall,
    order: Vec<usize>,
    rng: &mut Pcg32,
    lay_out: &dyn Fn(&mut Plan, &Hall, usize, &mut Pcg32) -> Option<Stage>,
    dress: &dyn Fn(&mut Plan, &Hall, Stage, &mut Pcg32),
    fallback: Stage,
) -> (Stage, usize) {
    for li in order.into_iter().map(Some).chain([None]) {
        let s = snap(p);
        h.clear.borrow_mut().clear();
        let stage = match li {
            Some(i) => lay_out(p, h, i, rng),
            None => Some(fallback),
        };
        if let Some(stage) = stage {
            let first = p.items.len();
            dress(p, h, stage, rng);
            if li.is_none() || reached(p, h) {
                return (stage, first);
            }
        }
        undo(p, s);
    }
    (fallback, p.items.len())
}

fn layout(p: &mut Plan, h: &Hall, name: &str, theme: &str, rng: &mut Pcg32) -> Option<Stage> {
    let (l, w) = (h.l as isize, h.w as isize);
    let mid = w / 2;
    let stage = far_stage(h);
    // (The rows before the stage: what the cover and obstacles fill.)
    let front = stage.d0 - 1;
    match name {
        "pillar grid" => {
            // (Rows and columns two apart in a small hall, three in a big one, an aisle down the middle.)
            let (sd, sa) = (if h.l >= 10 { 3 } else { 2 }, if h.w >= 9 { 3 } else { 2 });
            let mut n = 0;
            let mut d = 1 + (h.l >= 10) as isize;
            while d <= front {
                let mut a = 1;
                while mid - a >= 0 {
                    for x in [mid - a, mid + a] {
                        n += set(p, h, h.item("pillar", d, x, 1, 1, None), false) as usize;
                    }
                    a += sa;
                }
                d += sd;
            }
            (n >= 4).then_some(stage)
        }
        "broken walls" => {
            // Stubs of wall across the way in, a row apart, from each side by turns, a gap in the middle.
            let len = ((w - 1) / 2).clamp(1, 3) as usize;
            let first = rng.below(2) as isize;
            let (mut n, mut i) = (0, 0);
            let gap = if front < 4 { 1 } else { 2 };
            let mut d = if front < 4 { 1 } else { 1 + rng.below(2) as isize };
            while d <= front {
                let left = (i + first) % 2 == 0;
                i += 1;
                let a = if left { mid - len as isize } else { mid + 1 };
                if set(p, h, h.item("wall_stub", d, a, 1, len, None), false) {
                    n += 1;
                    // (Its tumbled blocks on the far side, at its inner end.)
                    let end = if left { a + len as isize - 1 } else { a };
                    set(p, h, h.item("collapsed_wall", d + 1, end, 1, 1, Some(h.side)), false);
                }
                d += gap + (h.l >= 10 && rng.below(2) == 0) as isize;
            }
            // A fallen column, lying along the way in or across it.
            let along = h.l >= 8 && rng.below(2) == 0;
            let (dl, aw) = if along { (3, 1) } else { (1, 3.min(h.w - 2)) };
            let d = 1 + rng.below(front.max(1) as u32) as isize;
            near(p, h, false, "fallen_column", d, mid - aw as isize / 2, dl, aw, None);
            (n >= 2).then_some(stage)
        }
        "statue gallery" => {
            let mut n = 0;
            if h.l >= 9 && h.w >= 9 && rng.below(5) < 2 {
                // Ringing the stage.
                let (f, m) = (stage.far(), stage.mid());
                for (d, a) in [(f, m - 3), (f, m + 3), (f - 2, m - 3), (f - 2, m + 3), (f - 4, m - 2), (f - 4, m + 2)] {
                    n += set(p, h, h.item("statue", d, a, 1, 1, None), false) as usize;
                }
            } else {
                // Lining the nave, against the walls in a narrow hall.
                let off = if h.w >= 9 { 3 } else if h.w >= 7 { 2 } else { mid };
                let mut d = 1;
                while d <= front {
                    for a in [mid - off, mid + off] {
                        n += set(p, h, h.item("statue", d, a.clamp(0, w - 1), 1, 1, None), false) as usize;
                    }
                    d += 2;
                }
            }
            (n >= 4).then_some(stage)
        }
        "rubble ring" => {
            let r = ((front.min(w - 1) - 1) / 2).clamp(1, 3);
            let dc = (front + 1) / 2;
            let mut n = 0;
            let mut boulders = 0;
            for dd in -r..=r {
                for da in -r..=r {
                    if dd.abs().max(da.abs()) != r || rng.below(10) < 3 {
                        continue;
                    }
                    if boulders < 2 && rng.below(8) == 0 && set(p, h, h.item("boulder", dc + dd, mid + da, 1, 1, None), false) {
                        boulders += 1;
                    } else {
                        n += set(p, h, h.item("rubble", dc + dd, mid + da, 1, 1, None), false) as usize;
                    }
                }
            }
            (n >= 6).then_some(stage)
        }
        "pits" => {
            // Two or three across the approach, staggered, a way between them.
            let ps = if h.l >= 9 && h.w >= 7 { 2 } else { 1 };
            let at: &[isize] = if h.w >= 9 { &[-4, -1, 2] } else if ps == 2 { &[-3, 1] } else { &[-2, 0, 2] };
            // (Two rows short of the stage at least, so none opens beside the centrepiece.)
            let band = (front - ps as isize - 1).max(1);
            let mut n = 0;
            for (i, &a) in at.iter().enumerate() {
                let d = 1 + (band / 2 + if i % 2 == 1 { 1 } else { 0 } + rng.below(2) as isize - 1).clamp(0, band - 1);
                n += set(p, h, h.item("pit", d, (mid + a).clamp(0, w - ps as isize), ps, ps, None), false) as usize;
            }
            (n >= 2).then_some(stage)
        }
        "chasm and bridge" => {
            let dm = ((l - 2) / 2).clamp(2, l - 4);
            // (Across the room, row by row, crossed by one stone bridge in line with the way in, or two wide.)
            let bw = if h.w >= 10 { 2 } else { 1 };
            let ba = (h.ea as isize).clamp(1, w - 1 - bw as isize).max(0);
            let mut items = Vec::new();
            for d in [dm, dm + 1] {
                let mut a = 0;
                while a < w {
                    if h.sq(p, d, a).is_none() {
                        a += 1;
                        continue;
                    }
                    let mut b = a;
                    while b + 1 < w && h.sq(p, d, b + 1).is_some() {
                        b += 1;
                    }
                    items.push(h.item("chasm", d, a, 1, (b - a + 1) as usize, None));
                    a = b + 1;
                }
            }
            items.push(h.item("stone_bridge", dm, ba, 2, bw, None));
            // (The bridge's ends on floor.)
            if h.sq(p, dm - 1, ba).is_none() || h.sq(p, dm + 2, ba).is_none() {
                return None;
            }
            lay(p, h, items).then_some(stage)
        }
        "moat" => {
            // An island of 3 (4 in a big hall) in a ring of water 2 wide (1 in a small hall), a floor row before it.
            let il = if h.l >= 13 && h.w >= 11 { 4 } else { 3 };
            // (Against the far wall, so no pocket of floor behind it is cut off; the ring 1 wide where 2 won't fit.
            // Else a row or two of floor behind it, joined round its sides: `reached` says.)
            let fits = |(mw, gap): (isize, isize)| {
                let (il, di, ai) = (il as isize, l - il as isize - mw - gap, (w - il as isize) / 2);
                let bridge = (di - mw, ai + il / 2);
                let ok = di - mw >= 2 && ai >= mw && h.free(p, di - mw, ai - mw, (il + 2 * mw) as usize, (il + 2 * mw) as usize) && h.sq(p, bridge.0 - 1, bridge.1).is_some();
                ok.then_some((mw, di, ai, bridge))
            };
            let (mw, di, ai, bridge) = [(2, 0), (1, 0), (2, 1), (1, 1), (1, 2)].into_iter().find_map(fits)?;
            let il = il as isize;
            let molten = theme == "dwarven_hall";
            let mut items = Vec::new();
            for d in di - mw..di + il + mw {
                for a in ai - mw..ai + il + mw {
                    if d >= di && d < di + il && a >= ai && a < ai + il {
                        continue;
                    }
                    if molten && (d, a) != bridge && (mw < 2 || (d, a) != (bridge.0 + 1, bridge.1)) {
                        items.push(h.rect(d, a, 1, 1).map(|(x, y, _, _)| super::kit::lava(x, y)));
                    } else if !molten {
                        items.push(h.item("water_deep", d, a, 1, 1, None));
                    }
                }
            }
            let span = if molten || rng.below(2) == 0 { "stone_bridge" } else { "stepping_stone" };
            items.push(h.item(span, bridge.0, bridge.1, mw as usize, 1, None));
            lay(p, h, items).then_some(Stage { d0: di, a0: ai, dl: il as usize, aw: il as usize, raised: false })
        }
        "raised platform" => {
            let pd = if h.l >= 10 { 3 } else { 2 };
            let (mut a0, mut a1) = (0, w - 1);
            while a1 - a0 + 1 >= 3 && !h.free(p, l - pd as isize, a0, pd, (a1 - a0 + 1) as usize) {
                a0 += 1;
                a1 -= 1;
            }
            let aw = (a1 - a0 + 1) as usize;
            if aw < 3 {
                return None;
            }
            let sd = l - pd as isize - 1;
            let sa = a0 + aw as isize / 2;
            let mut items = vec![h.item("platform", l - pd as isize, a0, pd, aw, None), h.item("steps", sd, sa, 1, 1, Some(h.far()))];
            // (A second flight at one end, now and then.)
            if aw >= 7 && rng.below(2) == 0 {
                let end = if rng.below(2) == 0 { a0 } else { a1 };
                items.push(h.item("steps", sd, end, 1, 1, Some(h.far())));
            }
            lay(p, h, items).then_some(Stage { d0: l - pd as isize, a0, dl: pd, aw, raised: true })
        }
        "balconies" => {
            let two = h.w >= 8 && rng.below(5) < 3;
            let bw = if h.w >= 10 { 2 } else { 1 };
            let first = rng.below(2) as usize;
            let mut items = Vec::new();
            let mut sides = Vec::new();
            for s in 0..if two { 2 } else { 1 } {
                let right = (first + s) % 2 == 1;
                let a = if right { w - bw as isize } else { 0 };
                let (mut d0, mut d1) = (2, l - 1);
                while d1 - d0 + 1 >= 4 && !h.free(p, d0, a, (d1 - d0 + 1) as usize, bw) {
                    if h.free(p, d0, a, 1, bw) { d1 -= 1 } else { d0 += 1 }
                }
                if d1 - d0 + 1 < 4 {
                    return None;
                }
                let up = if rng.below(5) < 3 { "climb_ladder" } else { "steps" };
                let back = Some(if right { h.walls().1 } else { h.walls().0 });
                items.push(h.item("balcony", d0, a, (d1 - d0 + 1) as usize, bw, back));
                items.push(h.item(up, d0 - 1, if right { w - 1 } else { 0 }, 1, 1, Some(h.far())));
                sides.push((right, d0, d1));
            }
            if !lay(p, h, items) {
                return None;
            }
            // Columns under the gallery's edge, holding it up, every second square (third in a long hall), before the stage.
            let gap = if h.l >= 12 { 3 } else { 2 };
            for &(right, d0, d1) in &sides {
                let a = if right { w - 1 - bw as isize } else { bw as isize };
                let mut d = d0 + 1;
                while d <= d1.min(front) {
                    set(p, h, h.item("pillar", d, a, 1, 1, None), false);
                    d += gap;
                }
            }
            // Cover in the hall between them: a broken wall or a statue a third and two thirds of the way in, on
            // alternate sides of the middle (where the hall is wide enough to walk round them).
            let (lo, hi) = (bw as isize + 1, w - 2 - bw as isize);
            if hi - lo >= 3 && front >= 4 {
                let left = rng.below(2) == 0;
                for (i, d) in [(front + 1) / 3, (2 * front + 1) / 3].into_iter().enumerate() {
                    let west = left == (i == 0);
                    let stub = rng.below(2) == 0;
                    let aw = if stub { 2 } else { 1 };
                    let a = if west { lo + rng.below(2) as isize } else { hi - aw as isize + 1 - rng.below(2) as isize };
                    let kind = if stub { "wall_stub" } else { "statue" };
                    let _ = [d, d + 1, d - 1].into_iter().filter(|&d| d >= 1 && d <= front).any(|d| set(p, h, h.item(kind, d, a, 1, aw, None), false));
                }
            }
            Some(stage)
        }
        "sunken arena" => {
            // The chamber's floor raised by `rise` round a fighting floor at the level's own height (heights stand
            // above the level's floor): the doorway's square stays down, steps up inside it, steps down into the
            // arena off to one side of the way in, and up again at its far side toward the stage.
            let rise = if rng.below(2) == 0 { 5.0 } else { 10.0 };
            let (ad0, ad1, aa0, aa1) = (3, l - 4, 2, w - 3);
            let (ed, ea) = (h.ed as isize, h.ea as isize);
            let up = (ed + 1, ea);
            if ed + 1 >= ad0 || h.sq(p, up.0, up.1).is_none_or(|k| p.taken[k]) {
                return None;
            }
            let side = if ea + 2 <= aa1 && (ea - 2 < aa0 || rng.below(2) == 0) { ea + 2 } else { ea - 2 };
            let down = (ad0 - 1, side.clamp(aa0, aa1));
            let back = (ad1 + 1, mid.clamp(aa0, aa1));
            let arena = |d: isize, a: isize| d >= ad0 && d <= ad1 && a >= aa0 && a <= aa1;
            // (A second way in stays down too, its own steps up inside it unless it opens on the arena.)
            let side_steps = h.door.and_then(|(k, iw)| {
                let (d, a) = h.frame(iw);
                (!arena(d as isize, a as isize)).then_some((k, iw, (d as isize, a as isize)))
            });
            if side_steps.is_some_and(|(_, _, at)| [up, down, back].contains(&at)) {
                return None;
            }
            let mut raise = vec![false; h.l * h.w];
            for d in 0..l {
                for a in 0..w {
                    let Some(k) = h.sq(p, d, a) else { continue };
                    if arena(d, a) || k == h.entry || h.door.is_some_and(|(door, _)| door == k) || [up, down, back].contains(&(d, a)) || side_steps.is_some_and(|s| s.2 == (d, a)) {
                        continue;
                    }
                    if p.taken[k] {
                        return None;
                    }
                    raise[d as usize * h.w + a as usize] = true;
                }
            }
            let mut items = Vec::new();
            for (d, a) in [up, down, back] {
                items.push(h.item("steps", d, a, 1, 1, Some(if (d, a) == up { h.far() } else { h.side })));
            }
            if let Some((k, iw, (d, a))) = side_steps {
                items.push(h.item("steps", d, a, 1, 1, Some(toward(k, iw, h.nx))));
            }
            // (The raised floor as rects: a run along a row, as deep as it goes.)
            for d in 0..h.l {
                let mut a = 0;
                while a < h.w {
                    if !raise[d * h.w + a] {
                        a += 1;
                        continue;
                    }
                    let mut b = a;
                    while b + 1 < h.w && raise[d * h.w + b + 1] {
                        b += 1;
                    }
                    let mut e = d;
                    while e + 1 < h.l && (a..=b).all(|x| raise[(e + 1) * h.w + x]) {
                        e += 1;
                    }
                    for dd in d..=e {
                        for x in a..=b {
                            raise[dd * h.w + x] = false;
                        }
                    }
                    // (A 10-ft surround is a gallery round the pit: the kinds carry their heights, as designs do.)
                    items.push(h.item(if rise > 5.0 { "balcony" } else { "platform" }, d as isize, a as isize, e - d + 1, b - a + 1, None));
                    a = b + 1;
                }
            }
            if !lay(p, h, items) {
                return None;
            }
            // Cover down in the pit, one to three pieces (by its size) standing apart, off its edge and its steps.
            let size = (ad1 - ad0 + 1) * (aa1 - aa0 + 1);
            let want = 1 + (size >= 20) as usize + (size >= 40) as usize;
            let mut spots: Vec<(isize, isize)> = (ad0 + 1..ad1).flat_map(|d| (aa0 + 1..aa1).map(move |a| (d, a))).collect();
            let mut put = 0;
            for _ in 0..want * 6 {
                if put >= want || spots.is_empty() {
                    break;
                }
                let i = rng.below(spots.len() as u32) as usize;
                let (d, a) = spots[i];
                let kind = ["pillar", "rubble", "weapon_rack", "boulder"][rng.below(4) as usize];
                let s = snap(p);
                if set(p, h, h.item(kind, d, a, 1, 1, None), false) && reached(p, h) {
                    put += 1;
                    spots.retain(|&(d2, a2)| (d2 - d).abs().max((a2 - a).abs()) >= 3);
                } else {
                    undo(p, s);
                    spots.swap_remove(i);
                }
            }
            Some(Stage { raised: true, ..stage })
        }
        _ => None,
    }
}

/// The theme's centrepiece on the stage, the hoard and chest by it, braziers and banners where they fit.
fn centrepiece(p: &mut Plan, h: &Hall, s: Stage, theme: &str, rng: &mut Pcg32) {
    let (f, back, top) = (s.far(), Some(h.far()), s.raised);
    // (The middle of the far row's floor where the room narrows there (an apse, a round room), else the stage's.)
    let row: Vec<isize> = (s.a0..s.a0 + s.aw as isize).filter(|&a| h.sq(p, f, a).is_some()).collect();
    let m = match (row.first(), row.last()) {
        (Some(&a), Some(&b)) if b - a + 1 < s.aw as isize => (a + b) / 2,
        _ => s.mid(),
    };
    let (wa, wb) = h.walls();
    let wl = h.w as isize - 1;
    let put = |p: &mut Plan, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>| near(p, h, top, kind, d, a, dl, aw, back);
    // (What goes round it, braziers, candles, the loot: by its spot or a square off, else left out, so a small stage
    // or an island is not heaped.)
    let extra = |p: &mut Plan, kind: &'static str, d: isize, a: isize, back: Option<u8>| near_in(p, h, top, kind, d, a, 1, 1, back, 1);
    // (On a level stage: a dais (a bier) first, what it bears on it.)
    let dais = |p: &mut Plan| if top { Some((f - 1, m - 1)) } else { near(p, h, false, "dais", f - 1, m - 1, 2, 3, back) };
    let mut loot = true;
    match theme {
        // A throne on its dais; a dwarven hall's forge-throne with its forge and anvil off to one side, two squares
        // clear of the dais, the hoard on the other (no chest or braziers: the forge is the fire).
        "keep" | "dungeon" | "dwarven_hall" => {
            let (dd, da) = dais(p).unwrap_or((f - 1, m - 1));
            near(p, h, true, "throne", dd + 1, da + 1, 1, 1, back);
            if theme == "dwarven_hall" {
                let west = rng.below(2) == 0;
                let side = if west { (da - 3).max(0) } else { (da + 5).min(wl) };
                put(p, "forge", dd + 1, side, 1, 1, back);
                put(p, "anvil", dd, side, 1, 1, None);
                put(p, "hoard", dd + 1, if west { da + 4 } else { da - 2 }, 1, 1, None);
                loot = false;
            }
        }
        // An altar, and the ritual circle before it.
        "temple" | "sunken_temple" | "cult_sanctum" => {
            put(p, "altar", f, m - 1, 1, 2, back);
            // (Before the altar: on a raised stage 3 deep, on it.)
            put(p, "glyph", if top && s.dl >= 3 { f - 2 } else { f - 3 }, m - 1, 2, 2, None);
            for a in [m - 2, m + 2] {
                extra(p, "candles", f, a, None);
            }
        }
        // An arcane workshop: the workbench, bookshelves on the side walls, a focus before them (no circle).
        "wizard_lair" => {
            put(p, "arcane_workbench", f, m - 1, 1, 2, back);
            if !set(p, h, h.item("bookshelf", f - 2, 0, 2, 1, Some(wa)), top) {
                put(p, "bookshelf", f, m - 3, 1, 1, back);
            }
            if !set(p, h, h.item("bookshelf", f - 2, wl, 2, 1, Some(wb)), top) {
                put(p, "bookshelf", f, m + 3, 1, 1, back);
            }
            put(p, "arcane_focus", f - 2, m, 1, 1, None);
            extra(p, "candles", f, m + 2, None);
        }
        // A cell block: cages along the far wall, shackles on the walls, the warden's desk.
        "prison" => {
            let first = p.items.len();
            put(p, "cage", f - 1, 0, 2, 2, None);
            put(p, "cage", f - 1, wl - 1, 2, 2, None);
            if h.l >= 9 {
                put(p, "cage", h.l as isize / 2, 0, 2, 2, None);
            }
            // (A cage's door is on its right as drawn, its top its back: turned a quarter past the wall it stands
            // against, the far wall if none, the door faces the room.)
            let cages: Vec<usize> = (first..p.items.len()).filter(|&i| p.items[i].kind == "cage").collect();
            for i in cages {
                let it = p.items[i];
                let (x0, y0, x1, y1) = (it.x as isize, it.y as isize, (it.x + it.w) as isize, (it.y + it.h) as isize);
                let out = |x: isize, y: isize| x < 0 || y < 0 || x >= h.nx as isize || y >= h.ny as isize || p.cells[y as usize * h.nx + x as usize] != h.ri;
                let sides = [(x0..x1).filter(|&x| out(x, y0 - 1)).count(), (y0..y1).filter(|&y| out(x1, y)).count(), (x0..x1).filter(|&x| out(x, y1)).count(), (y0..y1).filter(|&y| out(x0 - 1, y)).count()];
                let wall = (0..4u8).filter(|&d| sides[d as usize] > 0).max_by_key(|&d| (sides[d as usize], d == h.far())).unwrap_or(h.far());
                p.items[i].back = Some((wall + 1) % 4);
            }
            edge(p, h, "chains", 3, rng);
            put(p, "desk", f - 3, m - 1, 1, 2, None);
        }
        // A great sarcophagus on its bier, candles at its head; an ossuary's skull piles round about.
        "crypt" | "tomb" | "ossuary" | "catacombs" => {
            let (dd, da) = dais(p).unwrap_or((f - 1, m - 1));
            near(p, h, true, "sarcophagus", dd, da + 1, 2, 1, back);
            for a in [da - 1, da + 3] {
                extra(p, "candles", dd, a, None);
            }
            if theme == "ossuary" {
                edge(p, h, "skulls", 4, rng);
            }
        }
        // The chief's seat and the loot heaped round it.
        "bandit_hideout" => {
            put(p, "chair_of_office", f, m, 1, 1, back);
            put(p, "hoard", f, m + 1, 1, 1, None);
            for (k, a) in [("chest", m - 2), ("chest", m + 2), ("crate", m - 3), ("sacks", m + 3)] {
                put(p, k, f, a, 1, 1, None);
            }
            put(p, "table", f - 2, m - 1, 1, 2, None);
            loot = false;
        }
        // Bone mounds round a totem.
        "goblin_warren" => {
            put(p, "totem", f - 1, m, 1, 1, None);
            put(p, "bone_mound", f - 1, m - 3, 2, 2, None);
            put(p, "bone_mound", f - 1, m + 2, 2, 2, None);
            put(p, "skulls", f, m - 1, 1, 1, None);
        }
        // The strongroom's hoard (on its island when the moat fits).
        "flooded_vault" => {
            put(p, "hoard", f - s.dl as isize / 2, m, 1, 1, None);
            put(p, "chest", f, m - 1, 1, 1, back);
            put(p, "chest", f, m + 1, 1, 1, back);
            loot = false;
        }
        // A beast's lair: its nest, gnawed bones and an adventurer who came before.
        "labyrinth" => {
            put(p, "nest", f - 1, m - 1, 2, 2, None);
            for _ in 0..5 {
                let (d, a) = (f - 1 - rng.below(4) as isize, m - 3 + rng.below(7) as isize);
                put(p, "bones", d, a, 1, 1, None);
            }
            put(p, "skeleton", f - 3, m + 2, 2, 1, None);
        }
        // A fallen commander's war table, racks on the walls (no throne: not a ruler's seat).
        FORTRESS => {
            put(p, "long_table", f - 2, m - 1, 1, 3, None);
            if !set(p, h, h.item("weapon_rack", f - 1, 0, 1, 1, Some(wa)), top) {
                extra(p, "weapon_rack", f, m - 3, back);
            }
            if !set(p, h, h.item("weapon_rack", f - 1, wl, 1, 1, Some(wb)), top) {
                extra(p, "weapon_rack", f, m + 3, back);
            }
        }
        _ => {}
    }
    if loot {
        // (The hoard somewhere: beside the centrepiece, else wherever near.)
        if extra(p, "hoard", f, m + 2, None).is_none() {
            put(p, "hoard", f, m + 2, 1, 1, None);
        }
        extra(p, "chest", f, m - 2, back);
    }
    if matches!(theme, "keep" | "dungeon" | "temple" | "sunken_temple" | "cult_sanctum" | FORTRESS) {
        for a in [m - 3, m + 3] {
            extra(p, "brazier", f - 1, a, None);
        }
    }
    match theme {
        "dwarven_hall" => edge(p, h, "banner", 2, rng),
        "keep" | "dungeon" | "cult_sanctum" | "temple" | FORTRESS => edge(p, h, "banner", 2 + rng.below(2) as usize, rng),
        "prison" => edge(p, h, "sconce", 2, rng),
        "bandit_hideout" => edge(p, h, "lantern", 1, rng),
        _ => {}
    }
}

/// `n` of `kind` on the room's wall squares in its far half.
fn edge(p: &mut Plan, h: &Hall, kind: &'static str, n: usize, rng: &mut Pcg32) {
    let (nx, ny) = (h.nx, h.ny);
    let ws: Vec<usize> = (0..nx * ny)
        .filter(|&k| p.cells[k] == h.ri && !p.taken[k] && h.frame(k).0 * 2 >= h.l && neighbours(k, nx, ny).any(|m| p.cells[m] != h.ri))
        .collect();
    let mut put = 0;
    for _ in 0..n * 6 {
        if put >= n || ws.is_empty() {
            break;
        }
        let k = ws[rng.below(ws.len() as u32) as usize];
        if set(p, h, Some(item(kind, k % nx, k / nx, 1, 1)), false) {
            put += 1;
        }
    }
}

// Item 117 slice 2: caves', lava tubes' and mines' boss chambers, hazards, a second way in.

/// The natural sets' layouts (caves: a pool round an island; lava tubes: a lava channel), as the `under` example names
/// them.
pub const NATURAL: [&str; 6] = ["rock columns", "ledges", "rope-bridge chasm", "rubble field", "pool island", "lava channel"];
const NFIT: [(usize, usize); 6] = [(5, 5), (6, 5), (7, 4), (5, 5), (7, 5), (6, 4)];

/// A mine's: the half-dug face with scaffolding up ladders (along one side wall, both, or across the face).
pub const MINE: [&str; 3] = ["side walkway", "twin walkways", "face gallery"];
const MFIT: [(usize, usize); 3] = [(6, 4), (6, 6), (5, 6)];

/// The hazards a boss chamber may get (`hazards`), by item name; the hidden ones' rules start "hidden" (DM-only until
/// found, as traps).
pub const PERILS: [&str; 4] = ["steam vent", "cracked floor", "flooded pit", "pressure plate"];

/// Grid steps by an item's `back` code: 0 north, 1 east, 2 south, 3 west.
const DIRS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

fn step(k: usize, dir: usize, nx: usize, ny: usize) -> Option<usize> {
    let (x, y) = ((k % nx) as isize + DIRS[dir].0, (k / nx) as isize + DIRS[dir].1);
    (x >= 0 && y >= 0 && (x as usize) < nx && (y as usize) < ny).then(|| y as usize * nx + x as usize)
}

/// The side square `b` lies on from its neighbour `a` (an item's `back`: steps' way up).
fn toward(a: usize, b: usize, nx: usize) -> u8 {
    if b + nx == a {
        0
    } else if b == a + 1 {
        1
    } else if b == a + nx {
        2
    } else {
        3
    }
}

fn peril(name: &str, x: usize, y: usize) -> Item {
    let kind = match name {
        "steam vent" => "vent",
        "flooded pit" => "flooded_pit",
        "pressure plate" => "trap",
        _ => "cracked_floor",
    };
    item(kind, x, y, 1, 1)
}

/// About half the chambers: one or two of `kinds` (`PERILS`) on free floor in the approach: off the way in, the second
/// way in, every landing (`Hall::clear`), the stage and the centrepiece's squares and round them (the items from
/// `dressed` on), each kept only if the chamber is still all reached round them.
fn hazards(p: &mut Plan, h: &Hall, kinds: &[&'static str], stage: Stage, dressed: usize, rng: &mut Pcg32) {
    if kinds.is_empty() || rng.below(2) != 0 {
        return;
    }
    let want = 1 + rng.below(2) as usize;
    let (nx, ny) = (h.nx, h.ny);
    let (ex, ey) = (h.entry % nx, h.entry / nx);
    let held = held(p, h, stage, dressed);
    let spots: Vec<usize> = (0..nx * ny)
        .filter(|&k| p.cells[k] == h.ri && !p.taken[k] && !h.kept(k) && !held[k] && !h.clear.borrow().contains(&k) && (k % nx).abs_diff(ex).max((k / nx).abs_diff(ey)) >= 2)
        .collect();
    let mut put = 0;
    for _ in 0..want * 8 {
        if put >= want || spots.is_empty() {
            break;
        }
        let k = spots[rng.below(spots.len() as u32) as usize];
        let kind = kinds[rng.below(kinds.len() as u32) as usize];
        if p.taken[k] {
            continue;
        }
        let s = snap(p);
        if p.place(peril(kind, k % nx, k / nx), Some(h.ri), ny) && reached(p, h) {
            put += 1;
        } else {
            undo(p, s);
        }
    }
}

/// About half the chambers: a second way in from a nearby room on the level, two times in three a secret door, else a
/// plain door (an open arch on a natural level). Through a wall the two rooms share, or a passage dug through a little
/// rock (that room's floor, rock either side), from a square of that room as far from the main way in as the walk goes
/// round, and (on a built level) one the same locks shut off as the square before the main door, so no key is skipped;
/// a locked main door's lock goes on it too. Never into a raised room. Its doorway square and the square inside it:
/// `Hall::door`.
fn side_way(p: &mut Plan, h: &Hall, rng: &mut Pcg32) -> Option<(usize, usize)> {
    let secret = rng.below(3) < 2;
    let (nx, ny, ri) = (h.nx, h.ny, h.ri);
    let n = nx * ny;
    let built = !p.natural;
    let boss: Vec<bool> = p.cells.iter().map(|&c| c == ri).collect();
    let walk = p.distances(p.up, ny, Some(&boss));
    let out = neighbours(h.entry, nx, ny).filter(|&m| p.cells[m] >= 0 && p.cells[m] != ri && p.passable(h.entry, m) && walk[m] >= 0).min_by_key(|&m| (walk[m], m))?;
    let zone = built.then(|| super::kit::parts(p, ny, &|q| p.locks.contains_key(&q)).0);
    let lock = p.locks.get(&(out.min(h.entry), out.max(h.entry))).copied();
    let raise = p.rooms[ri as usize].raise;
    let dig = if built { 5 } else { 8 };
    let apart = |a: usize, b: usize| (a % nx).abs_diff(b % nx) + (a / nx).abs_diff(b / nx);
    // (Rock dug, the boss's square, the way out, the far room's square.)
    let mut cands: Vec<(usize, usize, usize, usize)> = Vec::new();
    for k in 0..n {
        if p.cells[k] != ri || p.taken[k] || apart(k, h.entry) < 4 {
            continue;
        }
        for dir in 0..4 {
            if step(k, (dir + 2) % 4, nx, ny).is_none_or(|iw| p.cells[iw] != ri || p.taken[iw]) {
                continue;
            }
            let mut q = k;
            for t in 0..=dig {
                let Some(s) = step(q, dir, nx, ny) else { break };
                q = s;
                let c = p.cells[q];
                if c < 0 {
                    // (Rock either side of the passage.)
                    if t == dig || [(dir + 1) % 4, (dir + 3) % 4].iter().any(|&sd| step(q, sd, nx, ny).is_none_or(|m| p.cells[m] >= 0)) {
                        break;
                    }
                    continue;
                }
                let room = &p.rooms[c as usize];
                let ok = c != ri
                    && room.kind != BOSS
                    && (built || t > 0)
                    && !p.taken[q]
                    && walk[q] >= 0
                    && apart(q, out) >= 4
                    && (room.raise - raise).abs() <= crate::interior::height::STEP_FT
                    && (t == 0 || room.shape == 0)
                    && !p.doors.contains(&(k.min(q), k.max(q)))
                    && zone.as_ref().is_none_or(|z| z[q] == z[out]);
                if ok {
                    cands.push((t, k, dir, q));
                }
                break;
            }
        }
    }
    // (Into the chamber's far half where one can, through the least rock.)
    if cands.iter().any(|c| h.frame(c.1).0 * 2 >= h.l) {
        cands.retain(|c| h.frame(c.1).0 * 2 >= h.l);
    }
    let least = cands.iter().map(|c| c.0).min()?;
    cands.retain(|c| c.0 <= least + 1);
    let (t, k, dir, q) = cands[rng.below(cands.len() as u32) as usize];
    let r = p.cells[q];
    let mut s = k;
    let mut first = q;
    for i in 0..t {
        s = step(s, dir, nx, ny)?;
        if i == 0 {
            first = s;
        }
        p.cells[s] = r;
        p.taken[s] = true;
    }
    let e = (k.min(first), k.max(first));
    p.door(k, first);
    if secret {
        p.secret.insert(e);
    } else if !built {
        p.gaps.insert(e);
    }
    if let Some(c) = lock.filter(|_| built) {
        p.locks.insert(e, c);
    }
    p.taken[k] = true;
    p.taken[q] = true;
    Some((k, step(k, (dir + 2) % 4, nx, ny)?))
}

/// A boss chamber on an island in its own lake (`themes::cave`): deep water round it, in another room.
fn islet(p: &Plan, nx: usize, ny: usize, ri: i16, squares: &[usize]) -> bool {
    let deep = |m: usize| p.items.iter().any(|f| f.kind == "water_deep" && (f.x as usize..(f.x + f.w) as usize).contains(&(m % nx)) && (f.y as usize..(f.y + f.h) as usize).contains(&(m / nx)));
    squares.iter().any(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] >= 0 && p.cells[m] != ri && deep(m)))
}

/// The level's second way into its boss chamber (`side_way`; about half the chambers get one, none to an island), picked
/// before the level's rooms are furnished so its doorway squares are still free: `Plan::side_door`.
pub(crate) fn side_door(p: &mut Plan, nx: usize, ny: usize, rng: &mut Pcg32) {
    let Some(ri) = p.rooms.iter().position(|r| r.kind == BOSS).map(|r| r as i16) else { return };
    let squares: Vec<usize> = (0..nx * ny).filter(|&k| p.cells[k] == ri).collect();
    // (Tried in four chambers of five: about three in five have room for one.)
    if squares.is_empty() || rng.below(5) == 0 || islet(p, nx, ny, ri, &squares) {
        return;
    }
    let h = Hall::new(p, nx, ny, ri, &squares);
    let entry = h.entry;
    p.side_door = side_way(p, &h, rng).map(|(k, iw)| (k, iw, entry));
}

/// The stage round the chamber's farthest square from its way in (walking its floor): its far row that square's.
fn far_point(p: &Plan, h: &Hall) -> Stage {
    let (nx, ny) = (h.nx, h.ny);
    let mut dist = vec![-1i32; nx * ny];
    dist[h.entry] = 0;
    let mut q = std::collections::VecDeque::from([h.entry]);
    let mut far = h.entry;
    while let Some(k) = q.pop_front() {
        if dist[k] > dist[far] || dist[k] == dist[far] && k < far {
            far = k;
        }
        for m in neighbours(k, nx, ny) {
            if dist[m] < 0 && p.cells[m] == h.ri {
                dist[m] = dist[k] + 1;
                q.push_back(m);
            }
        }
    }
    let (d, a) = h.frame(far);
    Stage { d0: d as isize - 1, a0: a as isize - 2, dl: 2, aw: 5, raised: false }
}

/// A cave's, lava tube's or mine's boss chamber: maybe a second way in, a layout of its set (`NATURAL`, `MINE`) by fit,
/// the theme's dressing and the hoard by the far end, maybe hazards. A cave boss on its own lake's island
/// (`themes::cave`) is that layout already: dressed only, no second way.
pub(crate) fn wild(p: &mut Plan, nx: usize, ny: usize, ri: i16, squares: &[usize], kind: UnderKind, rng: &mut Pcg32) {
    let h = Hall::new(p, nx, ny, ri, squares);
    let theme = p.theme;
    let isle = islet(p, nx, ny, ri, squares);
    let mine = kind == UnderKind::Mine;
    let lava = kind == UnderKind::LavaTube;
    let (names, fit): (&[&str], &[(usize, usize)]) = if mine { (&MINE, &MFIT) } else { (&NATURAL, &NFIT) };
    let mut order: Vec<usize> = (0..names.len())
        .filter(|&i| !isle && h.l >= fit[i].0 && h.w >= fit[i].1 && (mine || names[i] != if lava { "pool island" } else { "lava channel" }))
        .collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i as u32 + 1) as usize);
    }
    // (A flooded cave's boss on an island in a pool.)
    if theme == "flooded" && let Some(m) = order.iter().position(|&i| names[i] == "pool island") {
        order[..=m].rotate_right(1);
    }
    let fallback = far_point(p, &h);
    let (stage, dressed) = if mine {
        choose(p, &h, order, rng, &|p, h, i, rng| mine_layout(p, h, MINE[i], rng), &|p, h, s, rng| mine_dress(p, h, s, rng), fallback)
    } else {
        choose(p, &h, order, rng, &|p, h, i, rng| wild_layout(p, h, NATURAL[i], lava, rng), &|p, h, s, rng| wild_dress(p, h, s, theme, rng), fallback)
    };
    spread(p, &h, stage, cover_kinds(if mine { "mine" } else if lava { "lava_tube" } else { theme }, true), rng);
    let kinds: &[&'static str] = match theme {
        _ if lava => &["steam vent", "steam vent", "cracked floor"],
        _ if mine => &["cracked floor", "flooded pit"],
        "flooded" | "cave" | "fungal" => &["cracked floor", "flooded pit"],
        _ => &["cracked floor"],
    };
    hazards(p, &h, kinds, stage, dressed, rng);
}

/// The room's squares in frame coordinates.
fn frame_squares(p: &Plan, h: &Hall) -> Vec<(isize, isize)> {
    (0..h.l as isize).flat_map(|d| (0..h.w as isize).map(move |a| (d, a))).filter(|&(d, a)| h.sq(p, d, a).is_some()).collect()
}

fn wild_layout(p: &mut Plan, h: &Hall, name: &str, lava: bool, rng: &mut Pcg32) -> Option<Stage> {
    let (l, w) = (h.l as isize, h.w as isize);
    let stage = far_point(p, h);
    let all = frame_squares(p, h);
    let floor = |p: &Plan, d: isize, a: isize| h.sq(p, d, a).is_some_and(|k| !p.taken[k] && !h.kept(k));
    let (dc, ac) = (all.iter().map(|s| s.0).sum::<isize>() / all.len().max(1) as isize, all.iter().map(|s| s.1).sum::<isize>() / all.len().max(1) as isize);
    match name {
        "rock columns" => {
            // Columns standing clear of the walls, two or more apart, a stalagmite or two among them.
            let kind = if lava { "basalt" } else { "rock_column" };
            let inner: Vec<(isize, isize)> = all.iter().copied().filter(|&(d, a)| d >= 2 && (-1..=1).all(|i| (-1..=1).all(|j| h.sq(p, d + i, a + j).is_some()))).collect();
            if inner.is_empty() {
                return None;
            }
            let want = (all.len() / 14).clamp(3, 6);
            let mut at: Vec<(isize, isize)> = Vec::new();
            for _ in 0..want * 6 {
                if at.len() >= want {
                    break;
                }
                let (d, a) = inner[rng.below(inner.len() as u32) as usize];
                if at.iter().all(|&(e, b)| (e - d).abs().max((b - a).abs()) >= 2) && set(p, h, h.item(kind, d, a, 1, 1, None), false) {
                    at.push((d, a));
                }
            }
            for _ in 0..1 + rng.below(2) {
                let (d, a) = inner[rng.below(inner.len() as u32) as usize];
                set(p, h, h.item(if lava { "obsidian" } else { "stalagmite" }, d, a, 1, 1, None), false);
            }
            (at.len() >= 3).then_some(stage)
        }
        "ledges" => {
            // Raised shelves along the side walls (5 or 10 ft), up a rubble slope at one end or from the floor beside.
            let kind = if rng.below(2) == 0 { "platform" } else { "balcony" };
            let first = rng.below(2) as u8;
            let two = h.w >= 9 && rng.below(5) < 2;
            let mut items = Vec::new();
            let mut tops = Vec::new();
            for s in 0..1 + two as u8 {
                let Some((rects, mut ups)) = shelf(p, h, (first + s) % 2, 3, (1, l - 1)) else {
                    if s == 0 {
                        return None;
                    }
                    break;
                };
                items.extend(rects.iter().map(|&(d, a, dl, aw)| h.item(kind, d, a, dl, aw, None)));
                let turn = rng.below(ups.len().max(1) as u32) as usize;
                ups.rotate_left(turn);
                let &((ud, ua), _) = ups.first()?;
                items.push(h.item("rubble_slope", ud, ua, 1, 1, None));
                tops.push(rects[rects.len() - 1]);
            }
            if !lay(p, h, items) {
                return None;
            }
            // (A cache up on the shelf's far end.)
            for (d, a, _, _) in tops {
                set(p, h, h.item("chest", d, a, 1, 1, None), true);
            }
            Some(stage)
        }
        "rope-bridge chasm" => {
            // A chasm two rows wide across the chamber, a rope bridge over it in line with the middle.
            let dm = (l / 2 - 1).clamp(2, l - 4);
            if all.iter().filter(|s| s.0 > dm + 1).count() < 6 {
                return None;
            }
            let mut cols: Vec<isize> = (0..w).collect();
            cols.sort_by_key(|&a| ((a - ac).abs(), a));
            let ba = cols.into_iter().find(|&a| (dm - 1..=dm + 2).all(|d| floor(p, d, a)))?;
            let mut items = Vec::new();
            for d in [dm, dm + 1] {
                let mut a = 0;
                while a < w {
                    if h.sq(p, d, a).is_none() {
                        a += 1;
                        continue;
                    }
                    let mut b = a;
                    while b + 1 < w && h.sq(p, d, b + 1).is_some() {
                        b += 1;
                    }
                    items.push(h.item("chasm", d, a, 1, (b - a + 1) as usize, None));
                    a = b + 1;
                }
            }
            items.push(h.item("rope_bridge", dm, ba, 2, 1, None));
            lay(p, h, items).then_some(stage)
        }
        "rubble field" => {
            // A collapsed ceiling: rubble heaped round the middle, a boulder or two, the unstable roof over it.
            let r = if all.len() >= 60 { 3 } else { 2 };
            set(p, h, h.item("cave_in", dc, ac, 1, 1, None), false);
            let mut n = 0;
            for &(d, a) in &all {
                let (dd, da) = (d - dc, a - ac);
                if dd * dd + da * da > r * r + 1 || d < 2 || rng.below(20) >= 11 {
                    continue;
                }
                n += set(p, h, h.item("rubble", d, a, 1, 1, None), false) as usize;
            }
            for _ in 0..1 + rng.below(2) {
                let (d, a) = (dc - r + rng.below(2 * r as u32 + 1) as isize, ac - r + rng.below(2 * r as u32 + 1) as isize);
                near(p, h, false, "boulder", d, a, 2, 2, None);
            }
            (n >= 5).then_some(stage)
        }
        "pool island" => {
            // Deep water round an island (its far side may be the chamber's wall), stepping stones out to it from the
            // near side, shallows here and there round the pool.
            let il: isize = if all.len() >= 70 { 3 } else { 2 };
            let (fd, _) = (stage.far(), stage.mid());
            let (td, ta) = ((dc + fd) / 2, ac);
            let mut spots: Vec<(isize, isize)> = (2..l - il + 1).flat_map(|d| (0..w - il + 1).map(move |a| (d, a))).collect();
            spots.sort_by_key(|&(d, a)| ((d + il / 2 - td).abs() + (a + il / 2 - ta).abs(), d, a));
            for (di, ai) in spots.into_iter().take(40) {
                let isle = |d: isize, a: isize| d >= di && d < di + il && a >= ai && a < ai + il;
                if !(di..di + il).all(|d| (ai..ai + il).all(|a| floor(p, d, a))) {
                    continue;
                }
                let (cd, ca) = (di - 1, ai + il / 2);
                if !floor(p, cd, ca) || !floor(p, cd - 1, ca) {
                    continue;
                }
                let mut items = Vec::new();
                let mut ok = true;
                for d in di - 1..=di + il {
                    for a in ai - 1..=ai + il {
                        if isle(d, a) {
                            continue;
                        }
                        match h.sq(p, d, a) {
                            Some(_) if floor(p, d, a) => items.push(h.item("water_deep", d, a, 1, 1, None)),
                            Some(_) => ok = false,
                            // (Off the room: rock only, or the island is reached from there.)
                            None => ok &= h.cell(d, a).is_none_or(|k| p.cells[k] < 0),
                        }
                    }
                }
                if !ok {
                    continue;
                }
                items.push(h.item("stepping_stone", cd, ca, 1, 1, None));
                for d in di - 2..=di + il + 1 {
                    for a in ai - 2..=ai + il + 1 {
                        let ring = d == di - 2 || d == di + il + 1 || a == ai - 2 || a == ai + il + 1;
                        if ring && (d, a) != (cd - 1, ca) && floor(p, d, a) && h.sq(p, d, a) != Some(h.entry) && rng.below(2) == 0 {
                            items.push(h.item("water_shallow", d, a, 1, 1, None));
                        }
                    }
                }
                let s = snap(p);
                if lay(p, h, items) {
                    return Some(Stage { d0: di, a0: ai, dl: il as usize, aw: il as usize, raised: false });
                }
                undo(p, s);
                h.clear.borrow_mut().clear();
            }
            None
        }
        "lava channel" => {
            // Lava across the chamber, a row or two wide, crossed by a rock bridge or basalt stepping stones.
            let rows = if h.l >= 10 { 2 } else { 1 };
            let dm = (l / 2 - 1).clamp(2, l - 2 - rows);
            if all.iter().filter(|s| s.0 >= dm + rows).count() < 5 {
                return None;
            }
            let mut cols: Vec<isize> = (0..w).collect();
            cols.sort_by_key(|&a| ((a - ac).abs(), a));
            let ba = cols.into_iter().find(|&a| (dm - 1..=dm + rows).all(|d| floor(p, d, a)))?;
            let mut items = Vec::new();
            for d in dm..dm + rows {
                for a in 0..w {
                    if a != ba && h.sq(p, d, a).is_some() {
                        items.push(h.rect(d, a, 1, 1).map(|(x, y, _, _)| super::kit::lava(x, y)));
                    }
                }
            }
            let span = if rng.below(2) == 0 { "stone_bridge" } else { "stepping_stone" };
            items.push(h.item(span, dm, ba, rows as usize, 1, None));
            lay(p, h, items).then_some(stage)
        }
        _ => None,
    }
}

/// The natural set's dressing by theme round the stage (a beast's nest and bone mounds, a fungus grove, a crystal
/// cluster, ice columns, obsidian and sulfur in a lava tube), the hoard and a chest by it.
fn wild_dress(p: &mut Plan, h: &Hall, s: Stage, theme: &str, rng: &mut Pcg32) {
    let (f, m, top) = (s.far(), s.mid(), s.raised);
    let put = |p: &mut Plan, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize| near(p, h, top, kind, d, a, dl, aw, None);
    let scatter = |p: &mut Plan, kind: &'static str, n: u32, rng: &mut Pcg32| {
        for _ in 0..n {
            let (d, a) = (f - 1 - rng.below(3) as isize, m - 3 + rng.below(7) as isize);
            put(p, kind, d, a, 1, 1);
        }
    };
    match theme {
        "beast_den" => {
            put(p, "nest", f - 1, m - 1, 2, 2);
            put(p, "bone_mound", f - 2, m - 4, 2, 2);
            if rng.below(2) == 0 {
                put(p, "bone_mound", f - 2, m + 2, 2, 2);
            }
            scatter(p, "bones", 3, rng);
        }
        "fungal" => {
            scatter(p, "mushroom", 3, rng);
            scatter(p, "fungus", 3, rng);
        }
        "crystal" => scatter(p, "crystal", 5, rng),
        "ice" => scatter(p, "ice", 4, rng),
        "lava_tube" => {
            scatter(p, "obsidian", 2, rng);
            scatter(p, "sulfur", 2, rng);
        }
        _ => {
            scatter(p, if theme == "flooded" { "bones" } else { "stalagmite" }, 3, rng);
            put(p, "skeleton", f - 2, m + 2, 2, 1);
        }
    }
    put(p, "hoard", f, m, 1, 1);
    put(p, "chest", f, m + 1, 1, 1);
}

/// A raised strip along a wall (`wall`: 0 the side at a = 0, 1 the side at a = w - 1, 2 the far wall) over `span` of
/// the rows along it (d for a side wall, a for the far wall): on each row, from the wall's square out to one straight
/// inner line (at most `most` wide), all free floor clear of the way in, with floor beside it; the longest such run, of
/// 4 rows or more. Its rects (d, a, dl, aw), and the free squares a way up may take with the shelf's square each climbs
/// to (at the near end, the far end, beside its middle).
#[allow(clippy::type_complexity)]
fn shelf(p: &Plan, h: &Hall, wall: u8, most: isize, span: (isize, isize)) -> Option<(Vec<(isize, isize, usize, usize)>, Vec<((isize, isize), (isize, isize))>)> {
    let (l, w) = (h.l as isize, h.w as isize);
    let deep = if wall == 2 { l } else { w };
    let to = |u: isize, v: isize| match wall {
        0 => (u, v),
        1 => (u, w - 1 - v),
        _ => (l - 1 - v, u),
    };
    let (ex, ey) = (h.entry % h.nx, h.entry / h.nx);
    let free = |(d, a): (isize, isize)| h.sq(p, d, a).is_some_and(|k| !p.taken[k] && !h.kept(k) && (k % h.nx).abs_diff(ex).max((k / h.nx).abs_diff(ey)) >= 2);
    let outer = |u: isize| (0..deep).find(|&v| {
        let (d, a) = to(u, v);
        h.sq(p, d, a).is_some()
    });
    let row_ok = |u: isize, line: isize| {
        outer(u).is_some_and(|o| o <= line && line - o < most && (o..=line).all(|v| free(to(u, v))) && {
            let (d, a) = to(u, line + 1);
            h.sq(p, d, a).is_some()
        })
    };
    let mut best: Option<(isize, isize, isize)> = None;
    for line in 0..=(deep / 3).min(most + 1) {
        let mut u = span.0;
        while u <= span.1 {
            if !row_ok(u, line) {
                u += 1;
                continue;
            }
            let mut e = u;
            while e < span.1 && row_ok(e + 1, line) {
                e += 1;
            }
            if best.is_none_or(|b| e - u > b.2 - b.1) {
                best = Some((line, u, e));
            }
            u = e + 1;
        }
    }
    let (line, u0, u1) = best.filter(|b| b.2 - b.1 >= 3)?;
    let rects = (u0..=u1)
        .map(|u| {
            let (a, b) = (to(u, outer(u).unwrap_or(line)), to(u, line));
            (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).unsigned_abs() + 1, (a.1 - b.1).unsigned_abs() + 1)
        })
        .collect();
    let mid = (u0 + u1) / 2;
    let ups = [((u0 - 1, line), (u0, line)), ((u1 + 1, line), (u1, line)), ((mid, line + 1), (mid, line))].into_iter().map(|(a, b)| (to(a.0, a.1), to(b.0, b.1))).filter(|&(a, _)| free(a)).collect();
    Some((rects, ups))
}

/// A mine's: scaffolding (a raised walkway, 10 ft, up a ladder) along a side wall, both, or across the dug half of the
/// face; the face's other half an ore vein; rails from the way in to the face, an ore cart on them.
fn mine_layout(p: &mut Plan, h: &Hall, name: &str, rng: &mut Pcg32) -> Option<Stage> {
    let (l, w) = (h.l as isize, h.w as isize);
    let vein_left = rng.below(2) == 0;
    let mut items = Vec::new();
    let walkway = |p: &Plan, wall: u8, span: (isize, isize), items: &mut Vec<Option<Item>>| {
        let Some((rects, ups)) = shelf(p, h, wall, 2, span) else { return false };
        let Some(&(foot, top)) = ups.first() else { return false };
        items.extend(rects.iter().map(|&(d, a, dl, aw)| h.item("scaffold", d, a, dl, aw, None)));
        let (Some(k), Some(m)) = (h.sq(p, foot.0, foot.1), h.sq(p, top.0, top.1)) else { return false };
        items.push(h.item("climb_ladder", foot.0, foot.1, 1, 1, Some(toward(k, m, h.nx))));
        true
    };
    // (Along the side walls toward the face only, about half their length: the rest of the walls stay bare.)
    let run = (l / 2).max(4);
    let near_face = ((l - 1 - run).max(1), l - 2);
    let ok = match name {
        "side walkway" => walkway(p, rng.below(2) as u8, near_face, &mut items),
        "twin walkways" => walkway(p, 0, near_face, &mut items) && walkway(p, 1, near_face, &mut items),
        // (Across the dug half of the face.)
        "face gallery" => walkway(p, 2, if vein_left { (w / 2, w - 1) } else { (0, (w - 1) / 2) }, &mut items),
        _ => false,
    };
    if !ok || !lay(p, h, items) {
        return None;
    }
    // The face: the vein in its half of the far wall, rubble and tools where the other half is dug back.
    let half = |a: isize| if vein_left { a < w / 2 } else { a >= w / 2 };
    let mut face: Vec<(isize, isize)> = frame_squares(p, h)
        .into_iter()
        .filter(|&(d, a)| d >= l - 3 && half(a) && h.sq(p, d, a).is_some_and(|k| neighbours(k, h.nx, h.ny).count() < 4 || neighbours(k, h.nx, h.ny).any(|m| p.cells[m] < 0)))
        .collect();
    face.sort_by_key(|&(d, a)| (-d, a));
    let mut veins = 0;
    for (d, a) in face {
        if veins < 4 {
            veins += set(p, h, h.item("ore_vein", d, a, 1, 1, None), false) as usize;
        }
    }
    let dug = if vein_left { w * 3 / 4 } else { w / 4 };
    near(p, h, false, "rubble", l - 2, dug, 1, 1, None);
    near(p, h, false, "tools", l - 2, if vein_left { w / 4 } else { w * 3 / 4 }, 1, 1, None);
    let ar = (h.ea as isize).clamp(1, (w - 2).max(1));
    near(p, h, false, "ore_cart", l - 3, ar, 1, 1, None);
    let rails: Vec<Option<Item>> = (1..l - 1).filter(|&d| h.free(p, d, ar, 1, 1)).map(|d| h.item("rail", d, ar, 1, 1, None)).collect();
    if rails.len() >= 3 {
        lay(p, h, rails);
    }
    (veins >= 2).then(|| far_point(p, h))
}

/// A mine's dressing: tools and lanterns, the hoard and a chest by the face.
fn mine_dress(p: &mut Plan, h: &Hall, s: Stage, rng: &mut Pcg32) {
    let (f, m) = (s.far(), s.mid());
    near(p, h, false, "hoard", f - 1, m, 1, 1, None);
    near(p, h, false, "chest", f - 1, m + 1, 1, 1, None);
    near(p, h, false, "tools", f - 2, m - 2, 1, 1, None);
    edge(p, h, "lantern", 2, rng);
    // (One prop against a wall; the rest of the dressing, crates, barrels, a cart, a short stack, goes round the
    // chamber by `spread`.)
    edge(p, h, "timber", 1, rng);
}

// Item 117 fix round 2: dressing through the whole chamber, hazards kept off the centrepiece, the room's label spot.

/// The theme's freestanding cover, 1x1 (the kinds `spread` puts down): none that tells a layout apart in the `under`
/// example's metrics (statues, pillars, broken walls, rubble, rock columns).
fn cover_kinds(theme: &str, natural: bool) -> &'static [&'static str] {
    match theme {
        "keep" | "dungeon" | FORTRESS => &["weapon_rack", "barrel", "crate"],
        "dwarven_hall" => &["barrel", "anvil", "keg_rack"],
        "temple" | "sunken_temple" | "cult_sanctum" => &["urn", "effigy"],
        "wizard_lair" => &["bookshelf", "crate", "table"],
        "prison" => &["rack", "stocks", "iron_maiden"],
        "crypt" | "tomb" | "catacombs" => &["coffin", "urn", "effigy"],
        "ossuary" => &["skulls", "urn", "coffin"],
        "bandit_hideout" => &["crate", "barrel", "sacks"],
        "goblin_warren" => &["sacks", "crate", "skulls"],
        "flooded_vault" => &["crate", "barrel", "urn"],
        "labyrinth" => &["skulls", "boulder", "urn"],
        "mine" => &["crate", "barrel", "timber", "ore_cart"],
        "lava_tube" => &["boulder"],
        "fungal" => &["mushroom", "boulder"],
        "crystal" => &["crystal", "boulder"],
        "ice" => &["ice", "boulder"],
        _ if natural => &["stalagmite", "boulder"],
        _ => &["crate", "barrel"],
    }
}

/// Kinds that stand against a wall, their back to it.
const WALLED: [&str; 4] = ["bookshelf", "weapon_rack", "keg_rack", "rack"];

/// The squares kept for the centrepiece: the stage and a square round it, and round what the dressing put down (the
/// items from `dressed` on, but what hangs on the walls).
fn held(p: &Plan, h: &Hall, s: Stage, dressed: usize) -> Vec<bool> {
    let (nx, ny) = (h.nx, h.ny);
    let mut out = vec![false; nx * ny];
    for d in s.d0 - 1..=s.far() + 1 {
        for a in s.a0 - 1..s.a0 + s.aw as isize + 1 {
            if let Some(k) = h.cell(d, a) {
                out[k] = true;
            }
        }
    }
    for it in p.items.iter().skip(dressed).filter(|it| !matches!(it.kind, "banner" | "chains" | "sconce" | "lantern")) {
        for j in (it.y as usize).saturating_sub(1)..((it.y + it.h) as usize + 1).min(ny) {
            for i in (it.x as usize).saturating_sub(1)..((it.x + it.w) as usize + 1).min(nx) {
                out[j * nx + i] = true;
            }
        }
    }
    out
}

/// Cover and dressing in every quarter of the chamber (near and far, either side of its middle) that has room and none:
/// one or two of `kinds` each, standing apart, off the main lane (a square either side of the line from the way in to
/// the stage), the landings, the second way in and the stage; each kept only if the chamber is still all reached.
fn spread(p: &mut Plan, h: &Hall, s: Stage, kinds: &[&'static str], rng: &mut Pcg32) {
    let (l, w) = (h.l as isize, h.w as isize);
    let (nx, ny) = (h.nx, h.ny);
    let (ed, ea) = (h.ed as isize, h.ea as isize);
    let (td, ta) = (s.d0.max(ed + 1), s.mid());
    // (|a - the lane's a at d| <= 1, in whole numbers.)
    let lane = |d: isize, a: isize| d <= td && (a * (td - ed) - ea * (td - ed) - (ta - ea) * (d - ed).max(0)).abs() <= td - ed;
    let (ex, ey) = (h.entry % nx, h.entry / nx);
    let held = held(p, h, s, p.items.len());
    let quarter = |d: isize, a: isize| (d * 2 >= l) as usize * 2 + (a * 2 >= w) as usize;
    let cover = |it: &Item| PROPS.iter().any(|q| q.0 == it.kind && q.2 >= 1) && !TERRAIN.contains(&it.kind) && !SPANS.contains(&it.kind);
    let mut has = [false; 4];
    for it in p.items.iter().filter(|it| cover(it)) {
        let k = it.y as usize * nx + it.x as usize;
        if p.cells[k] == h.ri {
            let (d, a) = h.frame(k);
            has[quarter(d as isize, a as isize)] = true;
        }
    }
    for q in 0..4 {
        let squares = frame_squares(p, h).into_iter().filter(|&(d, a)| quarter(d, a) == q).count();
        if has[q] || squares < 4 {
            continue;
        }
        let cells = &p.cells;
        let wall = |k: usize| (0..4u8).find(|&dir| step(k, dir as usize, nx, ny).is_none_or(|m| cells[m] != h.ri));
        // (Standing alone where it can, else beside something.)
        let find = |alone: bool| -> Vec<(isize, isize, usize, Option<u8>)> {
            frame_squares(p, h)
                .into_iter()
                .filter(|&(d, a)| quarter(d, a) == q && !lane(d, a))
                .filter_map(|(d, a)| h.sq(p, d, a).map(|k| (d, a, k)))
                .filter(|&(_, _, k)| {
                    !p.taken[k] && !held[k] && !h.kept(k) && !h.clear.borrow().contains(&k) && (k % nx).abs_diff(ex).max((k / nx).abs_diff(ey)) >= 2 && (!alone || neighbours(k, nx, ny).all(|m| !p.taken[m] || cells[m] != h.ri))
                })
                .map(|(d, a, k)| (d, a, k, wall(k)))
                .collect()
        };
        let mut spots = find(true);
        if spots.is_empty() {
            spots = find(false);
        }
        let want = 1 + (spots.len() >= 16) as usize;
        let mut put = 0;
        for _ in 0..want * 6 {
            if put >= want || spots.is_empty() {
                break;
            }
            let kind = kinds[rng.below(kinds.len() as u32) as usize];
            let walled = WALLED.contains(&kind);
            let fit: Vec<usize> = (0..spots.len()).filter(|&i| spots[i].3.is_some() == walled).collect();
            if fit.is_empty() {
                continue;
            }
            let (d, a, k, back) = spots[fit[rng.below(fit.len() as u32) as usize]];
            let snap = snap(p);
            if set(p, h, h.item(kind, d, a, 1, 1, back), false) && reached(p, h) {
                put += 1;
                spots.retain(|&(d2, a2, _, _)| (d2 - d).abs().max((a2 - a).abs()) >= 3);
            } else {
                undo(p, snap);
                spots.retain(|&(_, _, m, _)| m != k);
            }
        }
    }
}

/// Where a boss chamber's name goes (`Room::center`): on open floor, away from its props, bridges, steps and doors: a
/// square of the room with no item on or round it (else the most open squares round it, five at least), the most open
/// floor along its row (the name runs across), nearest the room's middle; else the middle as before (None).
pub(crate) fn label(cells: &[i16], items: &[Item], doors: &crate::core::hash::FastSet<(usize, usize)>, nx: usize, ny: usize, ri: i16, middle: [f32; 2]) -> Option<usize> {
    let n = nx * ny;
    let mut busy = vec![false; n];
    for it in items {
        // (A raised floor is floor: its edge is what the name should keep off.)
        let edge_only = RAISING.contains(&it.kind);
        for j in it.y as usize..(it.y + it.h) as usize {
            for i in it.x as usize..(it.x + it.w) as usize {
                let inner = i > it.x as usize && j > it.y as usize && i + 1 < (it.x + it.w) as usize && j + 1 < (it.y + it.h) as usize;
                if !edge_only || !inner {
                    busy[j * nx + i] = true;
                }
            }
        }
    }
    for &(a, b) in doors {
        busy[a] = true;
        busy[b] = true;
    }
    let open = |k: usize| cells[k] == ri && !busy[k];
    // (Open squares round it, of the eight.)
    let clear = |k: usize| {
        let (x, y) = (k % nx, k / nx);
        if x == 0 || y == 0 || x + 1 >= nx || y + 1 >= ny {
            return 0;
        }
        (y - 1..=y + 1).flat_map(|j| (x - 1..=x + 1).map(move |i| j * nx + i)).filter(|&m| m != k && open(m)).count()
    };
    let run = |k: usize| {
        let (x, y) = (k % nx, k / nx);
        (x.saturating_sub(2)..=(x + 2).min(nx - 1)).filter(|&i| open(y * nx + i)).count()
    };
    let dist = |k: usize| {
        let (dx, dy) = ((k % nx) as f32 + 0.5 - middle[0], (k / nx) as f32 + 0.5 - middle[1]);
        dx * dx + dy * dy
    };
    (0..n).filter(|&k| open(k) && clear(k) >= 5).max_by(|&a, &b| (clear(a) == 8).cmp(&(clear(b) == 8)).then(run(a).cmp(&run(b))).then(clear(a).cmp(&clear(b))).then(dist(b).total_cmp(&dist(a))).then(b.cmp(&a)))
}
