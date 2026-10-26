//! Prisons underground (`prison`), grown room by room (`grow`). The top level: the gatehouse at the way in behind an
//! iron grate, a short way on to a ring corridor round the guard post at the heart; cell blocks branch off the ring
//! (straight rows with small cells on both sides, mirrored on the level's axis, a grate at each mouth); the warden's
//! office, mess hall, torture chamber and interrogation room off the ring where they fit, the rest grown freely.
//! Deeper levels: a guard room at the way down from above, a row of solitary cells, oubliettes (sometimes a wet one,
//! deep water in the middle), torture and guard rooms; the deepest ends in the boss chamber, a dead end. Mostly dry.

use crate::core::rng::Pcg32;
use crate::under::grow::{Grower, ROOMS_GROWING, ROOMS_MAX, Side, Template, boss_room, grate, rooms, specs, dead_end, fallback, flood_room, way_down};
use crate::under::*;

/// A cell (and a solitary one), the row it opens off.
const CELL: &str = "cell";
const SOLITARY: &str = "solitary cell";
const ROW: &str = "cell row";
const GUARD_POST: &str = "guard post";

#[allow(clippy::too_many_arguments)]
pub(crate) fn prison(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let (ax, ay) = (arrive % nx, arrive / nx);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let odd = |n: usize| n | 1;
    let sc = |n: usize| ((n as f64) * area.sqrt()).round() as usize;
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * area).round() as usize).max(4);
    let mut g;
    let mut boss = None;
    let mut wet = None;
    if depth == 0 {
        g = Grower::new(nx, ny, passage, Some(ay));
        g.cap = ROOMS_GROWING;
        // The gatehouse at the way in.
        let (gw, gh) = (pick(3, 4, rng), odd(pick(3, 5, rng)));
        let x0 = ax.saturating_sub(1).max(1);
        let y0 = ay.saturating_sub(gh / 2).max(1);
        let gate = g.seed(&mut p, theme.first, Template::Rect, x0, y0, gw, gh).or_else(|| g.seed(&mut p, theme.first, Template::Rect, ax, ay, 1, 1));
        let Some(gate) = gate else { return fallback(p, last) };
        // A short way on, behind a grate, to the ring round the guard post.
        let lead = g.deck(&mut p, gate, Side::E, theme.passage, Template::Corridor, 1, pick(2, 4, rng), 0, false);
        let from = match lead.first() {
            Some(&c) => {
                grate(&mut p, &g.pieces[c]);
                c
            }
            None => gate,
        };
        // (About 4 by 4: 5 square, else 3.)
        let mut s = 5;
        let mut ring = None;
        while s >= 3 && ring.is_none() {
            ring = guard_ring(&mut p, &mut g, from, s);
            s -= 2;
        }
        g.pieces[gate].open = false;
        if let Some(&c) = lead.first() {
            g.pieces[c].open = false;
        }
        let mut near = Vec::new();
        if let Some(([n, e, s, w], way_in)) = ring {
            if lead.is_empty() {
                p.portcullis.insert(way_in);
            }
            // Cell blocks: a mirrored pair off the north and south of the ring, another east on bigger sites.
            let long = pick(5, sc(9).max(5), rng);
            let rw = if area >= 1.0 && rng.next_f64() < 0.5 { 2 } else { 1 };
            let cell = if rng.next_f64() < 0.5 { (1, 2) } else { (2, 2) };
            let mut blocks = g.cell_block(&mut p, n, Side::N, 0, (ROW, rw, long), (CELL, cell.0, cell.1), true, true);
            if area >= 1.0 || blocks.is_empty() {
                // (Its grate only when it is the only block: a few grates a level.)
                let gate = blocks.is_empty();
                blocks.extend(g.cell_block(&mut p, e, Side::E, 0, (ROW, 1, pick(4, sc(8).max(4), rng)), (CELL, cell.0, cell.1), gate, true));
            }
            near = vec![e, n, s, w];
            near.extend(blocks);
        }
        // The prison's offices where they fit (off the ring first), then more rooms to the theme's count.
        let cw = theme.corridor;
        for (kind, size) in [("warden's office", (3, 4)), ("mess hall", (5, 4)), ("torture chamber", (4, 4)), ("interrogation room", (3, 3))] {
            g.attach(&mut p, &near, kind, Template::Rect, size, cw, rng);
        }
        let specs = specs(&["guard room", "storeroom", "barracks", "torture chamber", "interrogation room"], cw, ((4, 6), (4, 5)));
        let have = rooms(&p, passage);
        g.grow(&mut p, &specs, target.saturating_sub(have), false, rng);
        if last {
            g.cap = ROOMS_MAX;
            boss = boss_room(&mut p, &mut g, ny, rng);
        }
    } else {
        // A guard room round the way down from above, then solitary cells and oubliettes off it, the rest grown.
        let s = odd(pick(3, 5, rng));
        let x = ax.saturating_sub(s / 2).clamp(1, nx.saturating_sub(s + 1).max(1));
        let y = ay.saturating_sub(s / 2).clamp(1, ny.saturating_sub(s + 1).max(1));
        g = Grower::new(nx, ny, passage, None);
        g.cap = ROOMS_GROWING;
        let Some(core) = g.seed(&mut p, "guard room", Template::Rect, x, y, s, s).or_else(|| g.seed(&mut p, "guard room", Template::Rect, ax, ay, 1, 1)) else {
            return fallback(p, last);
        };
        let cw = theme.corridor;
        let (s0, long) = (rng.below(4) as usize, pick(4, 7, rng));
        let mut put = Vec::new();
        for k in 0..4 {
            put = g.cell_block(&mut p, core, Side::ALL[(s0 + k) % 4], 0, (ROW, 1, long), (SOLITARY, 1, 2), true, false);
            if !put.is_empty() {
                break;
            }
        }
        let mut near = vec![core];
        near.extend(put);
        let oubliettes = pick(1, 2 + (area >= 1.3) as usize, rng);
        for i in 0..oubliettes {
            let (t, n) = if rng.next_f64() < 0.5 { (Template::Round, 5) } else { (Template::Rect, pick(3, 4, rng)) };
            if let Some(o) = g.attach(&mut p, &near, "oubliette", t, (n, n), cw, rng) {
                g.pieces[o].open = false;
                if i == 0 && n >= 4 && rng.next_f64() < 0.4 {
                    wet = Some(g.pieces[o].room);
                }
            }
        }
        let specs = specs(&["torture chamber", "guard room", "interrogation room", "oubliette", "storeroom"], cw, ((4, 6), (4, 5)));
        let have = rooms(&p, passage);
        g.grow(&mut p, &specs, target.saturating_sub(have).max(2), false, rng);
        if last {
            g.cap = ROOMS_MAX;
            boss = boss_room(&mut p, &mut g, ny, rng);
        }
    }
    if last && boss.is_none() {
        boss = dead_end(&p, ny);
        if let Some(b) = boss {
            p.rooms[b as usize].kind = BOSS;
        }
    }
    p.passage = Some((passage, theme.corridor > 1));
    p.taken[arrive] = true;
    if !last {
        let cells: Vec<i16> = (0..p.rooms.len() as i16).filter(|&r| matches!(p.rooms[r as usize].kind, CELL | SOLITARY | ROW)).collect();
        let d = way_down(&p, ny, &cells);
        p.down = Some(d);
        p.taken[d] = true;
    }
    // A wet oubliette: deep water in the middle, a wading ledge round it.
    if let Some(r) = wet
        && Some(r) != boss
    {
        flood_room(&mut p, ny, r, (-1.0, -1.0), true);
    }
    p
}

/// A guard post `s` squares square with a ring corridor round it, grown east from piece `from` (centred on its
/// row; the level's axis put through it); doors from the post to the ring on its sides. The ring's pieces (north,
/// east, south, west) and the doorway into it.
fn guard_ring(p: &mut Plan, g: &mut Grower, from: usize, s: usize) -> Option<([usize; 4], (usize, usize))> {
    let nx = p.nx;
    let f = g.pieces[from];
    let o = s + 2;
    let x = f.x + f.w;
    let cy = f.y + f.h / 2;
    let y = cy.checked_sub(o / 2)?;
    let ny = p.cells.len() / nx;
    if y < 1 || x + o >= nx || y + o >= ny || (y..y + o).any(|j| (x..x + o).any(|i| p.cells[j * nx + i] >= 0)) {
        return None;
    }
    let post = g.seed(p, GUARD_POST, Template::Rect, x + 1, y + 1, s, s)?;
    g.pieces[post].open = false;
    // (The ring: four corridor strips, laid as pieces of the passage.)
    let strips = [(x, y, o, 1), (x + o - 1, y + 1, 1, o - 2), (x, y + o - 1, o, 1), (x, y + 1, 1, o - 2)];
    let mut out = [0; 4];
    for (k, &(i, j, w, h)) in strips.iter().enumerate() {
        out[k] = g.seed(p, "", Template::Corridor, i, j, w, h)?;
    }
    g.axis = Some(cy);
    // The way on into the ring, and the post's doors (west and east always, north and south too).
    let (a, b) = (cy * nx + x - 1, cy * nx + x);
    if p.cells[a] != g.passage {
        p.door(a, b);
    }
    let m = (y + o / 2, x + o / 2);
    for (q, r) in [(m.0 * nx + x, m.0 * nx + x + 1), (m.0 * nx + x + o - 1, m.0 * nx + x + o - 2), (y * nx + m.1, (y + 1) * nx + m.1), ((y + o - 1) * nx + m.1, (y + o - 2) * nx + m.1)] {
        p.door(q, r);
    }
    Some((out, (a.min(b), a.max(b))))
}
