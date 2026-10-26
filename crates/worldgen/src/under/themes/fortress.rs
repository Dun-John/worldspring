//! Collapsed fortresses (`collapsed_fortress`): a fort's halls under a ruin, caves eating in. The top level: a broken
//! gatehouse at the way in (rubble across it), guard rooms in a pair beside it, a pillared hall on the axis with
//! barracks off its sides, then rooms grown door to door (barracks, armories, stores, a mess hall, a shrine); round
//! tower cellars off the rooms nearest the grid's corners. Deeper levels: a guard room at the way down from above and
//! fewer rooms grown round it. Then caverns break through (`breach`): one on the top level, two below, three from the
//! third level down, bigger the deeper, joined by rubble gaps. The deepest ends in the boss chamber, a dead end.

use crate::core::rng::{Pcg32, hash3};
use crate::under::grow::{ROOMS_MAX, Side, Spec, Template, boss_room, fallback, fill, fill_out, fill_share, first_room, rooms};
use crate::under::themes::breach::breach;
use crate::under::themes::keep::finish;
use crate::under::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn fortress(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let odd = |n: usize| n | 1;
    let sc = |n: usize| ((n as f64) * crate::core::sqrt(area)).round() as usize;
    let cw = theme.corridor;
    p.secrets = 2 + (area >= 1.3) as usize;
    // (Less fort the deeper: the caves have the rest.)
    let fort = [1.0, 0.7, 0.55, 0.45][depth.min(3)];
    let target = ((((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)) * fort).round() as usize).max(3);
    let up = |n: usize| ((n as f64) * crate::core::sqrt(area).max(1.0)).round() as usize;
    let mut specs: Vec<Spec> = Vec::new();
    for kind in ["barracks", "armory", "storeroom", "guard room", "mess hall", "shrine", "barracks"] {
        specs.push(Spec { kind, t: Template::Rect, across: (3, up(5)), deep: (3, up(5)) });
        let t = if kind == "guard room" || kind == "shrine" { Template::Octagon } else { Template::Rect };
        specs.push(Spec { kind, t, across: (5, up(7)), deep: (4, up(6)) });
    }
    specs.push(Spec { kind: "", t: Template::Corridor, across: (cw, cw), deep: (2, 4) });
    let mut pillared: Vec<(i16, usize)> = Vec::new();
    let mut gatehouse = None;
    let mut g = if depth == 0 {
        // The broken gatehouse, its back to the way in, facing the wider side of the grid; guard rooms beside it.
        let dir = if arrive % nx < nx / 2 { Side::E } else { Side::W };
        let (ew, eh) = (pick(3, 4, rng), odd(pick(5, 7, rng)));
        let Some((mut g, gate)) = first_room(&mut p, passage, theme.first, Template::Rect, (ew, eh), Some(dir), 1) else { return fallback(p, last) };
        gatehouse = Some(g.pieces[gate]);
        g.deck(&mut p, gate, Side::N, "guard room", Template::Rect, 3, 3, 0, true);
        let from = g.to_middle(&mut p, gate, cw, ("guard room", Template::Rect, 5), ny / 5);
        let dir = if from == gate { dir } else if g.pieces[from].x < nx / 2 { Side::E } else { Side::W };
        // The pillared hall on the axis, barracks off its sides.
        for (a, d) in [(odd(pick(7, sc(9).max(7), rng)), pick(9, sc(12).max(9), rng)), (7, 9), (5, 7)] {
            if let Some(&h) = g.deck(&mut p, from, dir, "pillared hall", Template::Rect, a, d, 0, true).first() {
                pillared.push((g.pieces[h].room, 2));
                let (ba, bd) = (pick(4, sc(6).max(4), rng), pick(4, 5, rng));
                g.deck(&mut p, h, Side::N, "barracks", Template::Rect, ba, bd, 0, true);
                break;
            }
        }
        let left = target.saturating_sub(rooms(&p, passage));
        let made = g.grow(&mut p, &specs, left / 3, true, rng);
        g.grow(&mut p, &specs, left.saturating_sub(made), false, rng);
        g
    } else {
        let Some((mut g, _)) = first_room(&mut p, passage, "guard room", Template::Rect, (5, 5), None, 0) else { return fallback(p, last) };
        let left = target.saturating_sub(rooms(&p, passage)).max(2);
        g.grow(&mut p, &specs, left, false, rng);
        g
    };
    // (Big sites: spread over the grid.)
    fill_out(&mut g, &mut p, ny, fill_share(area), &["barracks", "storeroom", "guard room", "armory"], cw, rng);
    // Round tower cellars off the rooms nearest the corners (kept only near their corner).
    let tw = if area >= 1.6 { 7 } else { 5 };
    for (cx, cy) in [(0, 0), (nx, 0), (0, ny), (nx, ny)] {
        let d = |i: usize, g: &crate::under::grow::Grower| {
            let f = &g.pieces[i];
            (f.x + f.w / 2).abs_diff(cx) + (f.y + f.h / 2).abs_diff(cy)
        };
        let mut near: Vec<usize> = (0..g.pieces.len()).filter(|&i| g.pieces[i].open && g.pieces[i].room != passage).collect();
        near.sort_by_key(|&i| (d(i, &g), i));
        near.truncate(3);
        if near.is_empty() {
            continue;
        }
        let before = g.pieces.len();
        if let Some(t) = g.attach(&mut p, &near, "tower cellar", Template::Round, (tw, tw), cw, rng) {
            g.pieces[t].open = false;
            if d(t, &g) > (nx + ny) / 4 {
                while g.pieces.len() > before {
                    g.undo_last(&mut p);
                }
            }
        }
    }
    let mut boss = None;
    if last {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng);
    }
    finish(&mut p, ny, passage, cw, last, boss, &[], &pillared);
    // Rubble across the broken gatehouse.
    if let Some(f) = gatehouse {
        let items = rubble_slope(&Ground::of_plan(&p), f.x, f.y, f.w, f.h);
        p.put_all(items);
    }
    // Caves breaking in: more and bigger the deeper.
    let r = (4.0 + 1.5 * depth.min(3) as f64) * crate::core::sqrt(area).clamp(0.8, 1.4);
    for i in 0..1 + depth.min(2) {
        breach(&mut p, ny, hash3(rng.next_u32() as u64, depth as i64, i as i64, 0xf027), r);
    }
    p
}
