//! Dwarven halls (`dwarven_hall`), axial, grown room by room (`grow`): the gate hall at the way in (a landing at the way
//! down from above), a long pillared gallery on the level's axis with side wings off it in pairs (forge, brewery,
//! barracks, stores, armory, mushroom farm, treasury), the great hall at its far end lined with ancestor statues (an
//! ancestor hall on some levels below; the boss chamber on the deepest). One level in two the gallery breaks at a
//! chasm hall: a chasm across it, crossed by a stone bridge on the axis or by rope bridges. Mostly square halls, some
//! octagons; big sites grow to fill their grid. Wide passages; dry.

use crate::core::rng::Pcg32;
use crate::under::grow::{Grower, Piece, ROOMS_GROWING, ROOMS_MAX, Side, Spec, Template, boss_room, fallback, fill, first_room, rooms};
use crate::under::themes::keep::finish;
use crate::under::*;

const GALLERY: &str = "pillared gallery";
const CHASM_HALL: &str = "chasm hall";
const WINGS: [&str; 7] = ["forge", "brewery", "barracks", "storeroom", "armory", "mushroom farm", "treasury"];

#[allow(clippy::too_many_arguments)]
pub(crate) fn dwarven(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let odd = |n: usize| n | 1;
    let sc = |n: usize| ((n as f64) * crate::core::sqrt(area)).round() as usize;
    let cw = theme.corridor;
    let target = ((((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(nx * ny / 140)).clamp(4, ROOMS_GROWING - 6);
    p.secrets = 2 + (area >= 1.3) as usize;
    let chasm = depth % 2 == 1;
    let dir = if arrive % nx < nx / 2 { Side::E } else { Side::W };
    // The gate hall with its back to the way in; below, a landing round the way down from above.
    let first = if depth == 0 {
        let (w, h) = (pick(4, 6, rng), odd(pick(5, 7, rng)));
        first_room(&mut p, passage, theme.first, Template::Rect, (w, h), Some(dir), 1)
    } else {
        let s = odd(pick(5, 7, rng));
        let t = if rng.next_f64() < 0.35 { Template::Octagon } else { Template::Rect };
        first_room(&mut p, passage, "landing", t, (s, s), None, 0)
    };
    let Some((mut g, gate)) = first else { return fallback(p, last) };
    // (Entered near the grid's top or bottom: a passage to a guard room nearer its middle row, the axis through it.)
    let gate = g.to_middle(&mut p, gate, cw, ("guard room", Template::Rect, 5), ny / 6);
    let f = g.pieces[gate];
    let dir = if f.x + f.w / 2 < nx / 2 { Side::E } else { Side::W };
    // How far the grid runs on past the first hall.
    let span = if dir == Side::E { nx.saturating_sub(f.x + f.w + 1) } else { f.x.saturating_sub(1) };
    let across = odd(pick(5, sc(7).max(5), rng));
    let (end_a, end_d) = (odd(pick(9, sc(13).max(9), rng)), pick(9, sc(11).max(9), rng));
    let chasm_d = if chasm { across + 2 + pick(0, 2, rng) } else { 0 };
    let room_left = span.saturating_sub(end_d + chasm_d + 2);
    let least = if area > 1.0 { room_left / 2 } else { 0 };
    let len = pick(sc(10), sc(18), rng).max(least).min(room_left).max(6);
    let mut pillared: Vec<(i16, usize)> = Vec::new();
    let mut halls: Vec<i16> = Vec::new();
    // The gallery down the axis (in two, the chasm hall between, one level in two).
    let mut at = gate;
    let mut galleries = Vec::new();
    let parts = if chasm { [len / 2, len - len / 2] } else { [len, 0] };
    for (i, part) in parts.into_iter().enumerate().filter(|&(_, n)| n > 0) {
        if i == 1 {
            let wide = odd(across + 2 + 2 * rng.below(2) as usize);
            let Some(&c) = g.deck(&mut p, at, dir, CHASM_HALL, Template::Rect, wide, chasm_d.max(wide), 0, true).first() else { break };
            g.pieces[c].open = false;
            let room = g.pieces[c].room;
            // A stone bridge on the axis (its squares kept clear of the chasm), else rope bridges.
            let stone = rng.next_f64() < 0.5;
            if stone {
                let c = g.pieces[c];
                let row = c.y + c.h / 2;
                for i in c.x..c.x + c.w {
                    p.taken[row * nx + i] = true;
                    p.bridge.push(row * nx + i);
                }
            }
            p.chasm = Some((room, !stone));
            at = c;
        }
        for l in [part, part * 2 / 3, 5] {
            if let Some(&gal) = g.deck(&mut p, at, dir, GALLERY, Template::Rect, across, l.max(4), 0, true).first() {
                pillared.push((g.pieces[gal].room, 1));
                galleries.push(gal);
                at = gal;
                break;
            }
        }
    }
    // Side wings in pairs off the galleries' long walls, each pair two kinds.
    let kinds: Vec<&'static str> = if depth == 0 { WINGS.to_vec() } else { WINGS.iter().copied().filter(|&k| k != "brewery").collect() };
    for &gal in &galleries {
        let w = g.pieces[gal].w.max(g.pieces[gal].h);
        let pairs = (w / 6).max(1);
        for c in 0..pairs {
            let along = ((c + 1) * w / (pairs + 1)) as isize - (w / 2) as isize;
            let (a, d) = (odd(pick(5, sc(7).max(5), rng)), pick(4, sc(6).max(4), rng));
            let t = if rng.next_f64() < 0.2 { Template::Octagon } else { Template::Rect };
            let k0 = rng.below(kinds.len() as u32) as usize;
            let k1 = (k0 + 1 + rng.below(kinds.len() as u32 - 1) as usize) % kinds.len();
            let via = if rng.next_f64() < 0.5 { g.deck(&mut p, gal, Side::N, "", Template::Corridor, cw, 2, along, true) } else { Vec::new() };
            let new = match via.first() {
                Some(&c) => {
                    let new = g.deck(&mut p, c, Side::N, kinds[k0], t, a, d, 0, true);
                    if new.is_empty() {
                        for _ in 0..via.len() {
                            g.undo_last(&mut p);
                        }
                    } else {
                        g.pieces[c].open = false;
                    }
                    new
                }
                None => g.deck(&mut p, gal, Side::N, kinds[k0], t, a, d, along, true),
            };
            if let Some(&twin) = new.get(1) {
                p.rooms[g.pieces[twin].room as usize].kind = kinds[k1];
            }
        }
    }
    // The hall at the far end: the great hall (an ancestor hall on some levels below; the boss chamber deepest).
    let end_kind = if last { BOSS } else if depth == 0 || depth % 2 == 0 { "great hall" } else { "ancestor hall" };
    let t = if rng.next_f64() < 0.3 { Template::Octagon } else { Template::Rect };
    let mut boss = None;
    for (t, a, d) in [(t, end_a, end_d), (Template::Rect, 9, 9), (Template::Rect, 7, 7), (Template::Rect, 5, 5)] {
        if let Some(&e) = g.deck(&mut p, at, dir, end_kind, t, a, d, 0, true).first() {
            let room = g.pieces[e].room;
            if last {
                g.pieces[e].open = false;
                boss = Some(room);
            } else {
                halls.push(room);
                if a >= 9 && t == Template::Rect {
                    pillared.push((room, 2));
                }
            }
            break;
        }
    }
    // Then more rooms: some in mirrored pairs, the rest spreading over the grid.
    let specs = specs(&kinds, cw, area);
    let left = target.saturating_sub(rooms(&p, passage));
    let made = g.grow(&mut p, &specs, left / 3, true, rng);
    g.grow(&mut p, &specs, left.saturating_sub(made), false, rng);
    if last && boss.is_none() {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng);
    }
    finish(&mut p, ny, passage, cw, last, boss, &[], &pillared);
    // Ancestor statues along the great halls' walls.
    for r in halls {
        statues(&mut p, ny, &g, r);
    }
    p
}

/// What grows among the halls: the wings' rooms, each small or big, some octagons; short wide passages.
fn specs(kinds: &[&'static str], cw: usize, area: f64) -> Vec<Spec> {
    let up = |n: usize| ((n as f64) * crate::core::sqrt(area).max(1.0)).round() as usize;
    let mut out = Vec::new();
    for &kind in kinds {
        out.push(Spec { kind, t: Template::Rect, across: (4, up(6)), deep: (4, up(6)) });
        let t = if matches!(kind, "barracks" | "treasury") { Template::Octagon } else { Template::Rect };
        out.push(Spec { kind, t, across: (5, up(8)), deep: (5, up(8)) });
    }
    out.push(Spec { kind: "", t: Template::Corridor, across: (cw, cw), deep: (2, 4) });
    out.push(Spec { kind: "", t: Template::Corridor, across: (cw, cw), deep: (3, up(6)) });
    out
}

/// Statues every third square along a hall's two long walls (off doorways and what is there), kept only where every
/// square stays reachable.
fn statues(p: &mut Plan, ny: usize, g: &Grower, room: i16) {
    let nx = p.nx;
    let Some(&Piece { x, y, w, h, .. }) = g.pieces.iter().find(|f| f.room == room) else { return };
    let mut near_door = vec![false; p.cells.len()];
    for &(a, b) in &p.doors {
        for k in [a, b] {
            near_door[k] = true;
            for m in neighbours(k, nx, ny) {
                near_door[m] = true;
            }
        }
    }
    let along_x = w >= h;
    let (len, wide) = if along_x { (w, h) } else { (h, w) };
    for t in (2..len.saturating_sub(2)).step_by(3) {
        for r in [0, wide - 1] {
            // (An octagon's cut corners: the first square in from the wall that is the room's.)
            for r in [r, if r == 0 { 1 } else { r - 1 }] {
                let (i, j) = if along_x { (x + t, y + r) } else { (x + r, y + t) };
                let k = j * nx + i;
                if p.cells[k] != room {
                    continue;
                }
                if !near_door[k] && !p.taken[k] && p.down != Some(k) && k != p.up {
                    p.place(item("statue", i, j, 1, 1), Some(room), ny);
                }
                break;
            }
        }
    }
}
