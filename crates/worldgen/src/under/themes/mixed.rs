//! The plain dungeon (`dungeon`), grown room by room (`grow`) with mixed kits. The top level: an entry hall at the way
//! in, a mirrored core on its axis (guard rooms in a pair, a hall of some shape, a pair of cell blocks or stores off
//! it, a room beyond), then rooms grown door to door, a few more in mirrored pairs, the rest freely. Deeper levels: a
//! guard room at the way down from above, a cell block behind its grate, stores, a torture chamber, the rest grown.
//! Rooms of varied shapes (rect, octagon, cross, round, apse), passages short; loops where rooms touch and shortcuts
//! (`kit::guard`); the deepest ends in the boss chamber, a dead end. A well room's shallow pool; mostly dry.

use crate::core::rng::Pcg32;
use crate::under::grow::{ROOMS_MAX, Side, Spec, Template, boss_room, fallback, fill, first_room, rooms};
use crate::under::themes::keep::{CELL, ROW, cell_block, finish};
use crate::under::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn mixed(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let odd = |n: usize| n | 1;
    let sc = |n: usize| ((n as f64) * crate::core::sqrt(area)).round() as usize;
    let cw = theme.corridor;
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(4);
    p.secrets = 2 + (area >= 1.3) as usize;
    let specs = specs(theme, cw, area);
    let mut pools: Vec<(i16, (f64, f64), bool)> = Vec::new();
    let shapes = [Template::Octagon, Template::Cross, Template::Rect, Template::Round];
    let g = if depth == 0 {
        // The entry hall, its back to the way in, facing the wider side of the grid.
        let dir = if arrive % nx < nx / 2 { Side::E } else { Side::W };
        let (ew, eh) = (pick(3, 5, rng), odd(pick(5, 7, rng)));
        let Some((mut g, entry)) = first_room(&mut p, passage, theme.first, Template::Rect, (ew, eh), Some(dir), 1) else { return fallback(p, last) };
        // Guard rooms in a pair beside it.
        let (ga, gd) = (pick(3, 4, rng), pick(3, 4, rng));
        g.deck(&mut p, entry, Side::N, "guard room", Template::Rect, ga, gd, 0, true);
        // (Entered near the grid's top or bottom: a passage to a guard room nearer its middle row, the axis through it.)
        let from = g.to_middle(&mut p, entry, cw, ("guard room", Template::Rect, 5), ny / 5);
        let dir = if from == entry { dir } else if g.pieces[from].x < nx / 2 { Side::E } else { Side::W };
        // The core: a short passage (sometimes) on to a hall of some shape on the axis.
        let mut from = from;
        let entry = from;
        if rng.next_f64() < 0.5
            && let Some(&c) = g.deck(&mut p, entry, dir, "", Template::Corridor, cw, pick(2, 3, rng), 0, true).first()
        {
            from = c;
        }
        let core_kind = ["shrine", "well room", "pit room", "guard room"][rng.below(4) as usize];
        let t = shapes[rng.below(4) as usize];
        let mut core = None;
        for s in [odd(pick(5, sc(7).max(5), rng)), 5] {
            if let Some(&c) = g.deck(&mut p, from, dir, core_kind, t, s, s, 0, true).first() {
                core = Some(c);
                break;
            }
        }
        if let Some(core) = core {
            if core_kind == "well room" {
                pools.push((g.pieces[core].room, (0.45, 2.0), false));
            }
            // Off its sides, mirrored: a pair of cell blocks behind grates, or stores.
            let made = rng.next_f64() < 0.55 && {
                let cell = if rng.next_f64() < 0.5 { (1, 2) } else { (2, 2) };
                !g.cell_block(&mut p, core, Side::N, 0, (ROW, 1, pick(4, sc(7).max(4), rng)), (CELL, cell.0, cell.1), true, true).is_empty()
            };
            if !made {
                let kind = ["storeroom", "armory", "barracks"][rng.below(3) as usize];
                let t = [Template::Rect, Template::Octagon, Template::Apse][rng.below(3) as usize];
                let (a, d) = (odd(pick(3, sc(5), rng)), pick(3, sc(5), rng));
                g.deck(&mut p, core, Side::N, kind, t, a, d, 0, true);
            }
            // A room beyond it on the axis.
            let kind = ["barracks", "torture chamber", "armory", "prison"][rng.below(4) as usize];
            let t = [Template::Rect, Template::Apse, Template::Octagon, Template::Cross][rng.below(4) as usize];
            let (a, d) = (odd(pick(5, sc(7), rng)), pick(4, sc(6), rng));
            g.deck(&mut p, core, dir, kind, t, a, d, 0, true);
        }
        // Then more rooms: a few in mirrored pairs, the rest freely.
        let left = target.saturating_sub(rooms(&p, passage));
        let made = g.grow(&mut p, &specs, left / 3, true, rng);
        g.grow(&mut p, &specs, left.saturating_sub(made), false, rng);
        g
    } else {
        // A guard room round the way down from above, a cell block off it, stores and a torture chamber near.
        let t = shapes[rng.below(4) as usize];
        let s = odd(pick(3, 5, rng)).max(if t == Template::Rect { 3 } else { 5 });
        let Some((mut g, core)) = first_room(&mut p, passage, "guard room", t, (s, s), None, 0) else { return fallback(p, last) };
        let mut near = vec![core];
        if rng.next_f64() < 0.6 {
            let cell = if rng.next_f64() < 0.5 { (1, 2) } else { (2, 2) };
            near.extend(cell_block(&mut p, &mut g, &[core], (ROW, 1, pick(4, 7, rng)), (CELL, cell.0, cell.1), rng));
        }
        for (kind, t, size) in [("storeroom", Template::Rect, (4, 4)), ("torture chamber", Template::Octagon, (5, 5)), ("well room", Template::Round, (5, 5))] {
            if kind == "well room" && rng.next_f64() < 0.5 {
                continue;
            }
            let Some(r) = g.attach(&mut p, &near, kind, t, size, cw, rng) else { continue };
            if kind == "well room" {
                pools.push((g.pieces[r].room, (0.45, 2.0), false));
            }
        }
        let left = target.saturating_sub(rooms(&p, passage)).max(2);
        g.grow(&mut p, &specs, left, false, rng);
        g
    };
    let mut g = g;
    let mut boss = None;
    if last {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng);
    }
    finish(&mut p, ny, passage, cw, last, boss, &pools, &[]);
    p
}

/// What grows on a dungeon's levels: the theme's rooms, each small or bigger in a shape that suits it; short passages.
fn specs(theme: &Theme, cw: usize, area: f64) -> Vec<Spec> {
    let up = |n: usize| ((n as f64) * crate::core::sqrt(area).max(1.0)).round() as usize;
    let mut out = Vec::new();
    for &kind in theme.kinds.iter().filter(|&&k| k != "cell block") {
        out.push(Spec { kind, t: Template::Rect, across: (3, up(5)), deep: (3, up(5)) });
        let t = match kind {
            "shrine" => Template::Apse,
            "well room" | "pit room" => Template::Round,
            "torture chamber" | "guard room" => Template::Octagon,
            "prison" | "barracks" => Template::Cross,
            _ => Template::Rect,
        };
        out.push(Spec { kind, t, across: (5, up(7)), deep: (5, up(7)) });
    }
    out.push(Spec { kind: "", t: Template::Corridor, across: (cw, cw), deep: (2, 3) });
    out.push(Spec { kind: "", t: Template::Corridor, across: (cw, cw), deep: (2, 4) });
    out
}
