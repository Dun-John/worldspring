//! A built site's boss chamber (item 117): a tactical layout picked per site by the rng and by fit (cover: a pillar
//! grid, broken walls, a statue gallery, a rubble ring; obstacles: pits, a chasm with its bridge, a moat round an
//! island; height: a raised platform, balconies, a sunken arena), then the theme's centrepiece (a throne only where a
//! ruler fits, a ritual circle only in temples and cult sanctums) and dressing. Heights go through the floor model
//! (`interior::height`: `platform`/`balcony` raise, `steps`/`climb_ladder` are ramps). Caves, lava tubes and mines
//! keep `kit::boss_chamber`'s own set.

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
}

impl Hall {
    fn new(p: &Plan, nx: usize, ny: usize, ri: i16, squares: &[usize]) -> Hall {
        let dist = p.distances(p.up, ny, None);
        let entry = squares.iter().copied().filter(|&k| dist[k] >= 0).min_by_key(|&k| (dist[k], k)).unwrap_or(squares[0]);
        let (x0, y0) = (squares.iter().map(|k| k % nx).min().unwrap_or(0), squares.iter().map(|k| k / nx).min().unwrap_or(0));
        let (x1, y1) = (squares.iter().map(|k| k % nx).max().unwrap_or(0), squares.iter().map(|k| k / nx).max().unwrap_or(0));
        let (ex, ey) = (entry % nx, entry / nx);
        let gaps = [ey - y0, x1 - ex, y1 - ey, ex - x0];
        let side = (0..4).min_by_key(|&s| (gaps[s], s)).unwrap_or(0) as u8;
        let (l, w) = if side.is_multiple_of(2) { (y1 - y0 + 1, x1 - x0 + 1) } else { (x1 - x0 + 1, y1 - y0 + 1) };
        let mut h = Hall { nx, ny, ri, x0, y0, x1, y1, side, l, w, entry, ed: 0, ea: 0, clear: Default::default() };
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
        (0..dl as isize).all(|i| (0..aw as isize).all(|j| self.sq(p, d + i, a + j).is_some_and(|k| !p.taken[k])))
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
        if p.cells[k] != h.ri || h.clear.borrow().contains(&k) {
            return false;
        }
        if p.taken[k] {
            let mut on = p.items.iter().filter(|f| covers(f, k));
            let raised = on.clone().count() > 0 && on.all(|f| f.kind == "dais" || top && matches!(f.kind, "platform" | "balcony"));
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

/// `set` at (d, a) or the nearest spot round it (up to 3 squares off): where it went.
#[allow(clippy::too_many_arguments)]
fn near(p: &mut Plan, h: &Hall, top: bool, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>) -> Option<(isize, isize)> {
    let mut offs: Vec<(isize, isize)> = (-3..=3isize).flat_map(|i| (-3..=3isize).map(move |j| (i, j))).collect();
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

/// The room's squares reached from its way in without a climb (`interior::height`), round what blocks: all of them.
fn reached(p: &Plan, h: &Hall) -> bool {
    let (nx, ny) = (h.nx, h.ny);
    let f = p.floor(ny);
    let blocked = blocked_squares(&p.items, nx, ny);
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

/// The boss chamber of a built site (not a mine): a layout, the theme's centrepiece and its dressing.
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
    for li in order.into_iter().map(Some).chain([None]) {
        let s = snap(p);
        h.clear.borrow_mut().clear();
        let stage = match li {
            Some(i) => layout(p, &h, LAYOUTS[i], theme, rng),
            None => Some(far_stage(&h)),
        };
        if let Some(stage) = stage {
            centrepiece(p, &h, stage, theme, rng);
            if li.is_none() || reached(p, &h) {
                return;
            }
        }
        undo(p, s);
    }
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
            let band = (front - ps as isize).max(1);
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
                        items.push(h.rect(d, a, 1, 1).map(|(x, y, _, _)| lava(x, y)));
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
            }
            lay(p, h, items).then_some(stage)
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
            let mut raise = vec![false; h.l * h.w];
            for d in 0..l {
                for a in 0..w {
                    let Some(k) = h.sq(p, d, a) else { continue };
                    if arena(d, a) || k == h.entry || [up, down, back].contains(&(d, a)) {
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
            lay(p, h, items).then_some(Stage { raised: true, ..stage })
        }
        _ => None,
    }
}

/// The theme's centrepiece on the stage, the hoard and chest by it, braziers and banners where they fit.
fn centrepiece(p: &mut Plan, h: &Hall, s: Stage, theme: &str, rng: &mut Pcg32) {
    let (f, m, back, top) = (s.far(), s.mid(), Some(h.far()), s.raised);
    let (wa, wb) = h.walls();
    let wl = h.w as isize - 1;
    let put = |p: &mut Plan, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>| near(p, h, top, kind, d, a, dl, aw, back);
    // (On a level stage: a dais (a bier) first, what it bears on it.)
    let dais = |p: &mut Plan| if top { Some((f - 1, m - 1)) } else { near(p, h, false, "dais", f - 1, m - 1, 2, 3, back) };
    let mut loot = true;
    match theme {
        // A throne on its dais; a dwarven hall's forge-throne with its forge and anvil beside.
        "keep" | "dungeon" | "dwarven_hall" => {
            let (dd, da) = dais(p).unwrap_or((f - 1, m - 1));
            near(p, h, true, "throne", dd + 1, da + 1, 1, 1, back);
            if theme == "dwarven_hall" {
                let side = if rng.below(2) == 0 { -2 } else { 4 };
                put(p, "forge", dd + 1, da + side, 1, 1, back);
                put(p, "anvil", dd, da + side, 1, 1, None);
            }
        }
        // An altar, and the ritual circle before it.
        "temple" | "sunken_temple" | "cult_sanctum" => {
            put(p, "altar", f, m - 1, 1, 2, back);
            put(p, "glyph", f - 3, m - 1, 2, 2, None);
            for a in [m - 2, m + 2] {
                put(p, "candles", f, a, 1, 1, None);
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
            put(p, "candles", f, m + 2, 1, 1, None);
        }
        // A cell block: cages along the far wall, shackles on the walls, the warden's desk.
        "prison" => {
            put(p, "cage", f - 1, 0, 2, 2, None);
            put(p, "cage", f - 1, wl - 1, 2, 2, None);
            if h.l >= 9 {
                put(p, "cage", h.l as isize / 2, 0, 2, 2, None);
            }
            edge(p, h, "chains", 3, rng);
            put(p, "desk", f - 3, m - 1, 1, 2, None);
        }
        // A great sarcophagus on its bier, candles at its head; an ossuary's skull piles round about.
        "crypt" | "tomb" | "ossuary" | "catacombs" => {
            let (dd, da) = dais(p).unwrap_or((f - 1, m - 1));
            near(p, h, true, "sarcophagus", dd, da + 1, 2, 1, back);
            for a in [da - 1, da + 3] {
                put(p, "candles", dd, a, 1, 1, None);
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
                put(p, "weapon_rack", f, m - 3, 1, 1, back);
            }
            if !set(p, h, h.item("weapon_rack", f - 1, wl, 1, 1, Some(wb)), top) {
                put(p, "weapon_rack", f, m + 3, 1, 1, back);
            }
        }
        _ => {}
    }
    if loot {
        put(p, "hoard", f, m + 2, 1, 1, None);
        put(p, "chest", f, m - 2, 1, 1, back);
    }
    if matches!(theme, "keep" | "dungeon" | "dwarven_hall" | "temple" | "sunken_temple" | "cult_sanctum" | FORTRESS) {
        for a in [m - 3, m + 3] {
            put(p, "brazier", f - 1, a, 1, 1, None);
        }
    }
    match theme {
        "keep" | "dungeon" | "dwarven_hall" | "cult_sanctum" | "temple" | FORTRESS => edge(p, h, "banner", 2 + rng.below(2) as usize, rng),
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
