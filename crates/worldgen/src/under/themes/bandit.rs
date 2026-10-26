//! Bandit hideouts (`bandit_hideout`): a cave front, cellars behind. The top level is entered by a natural cave (the
//! hidden entry), a crawl on to the common room in a cavern and the lookout up a side crawl (all drawn as caverns);
//! behind them a tunnel into dug and shored rooms grown door to door (`grow`): bunk rooms, kitchen, stores, armory, and
//! farthest in the captain's quarters, behind a door locked first (`Plan::lock_first`), the loot store beyond. A secret
//! tunnel loops from the back rooms to the cave. Deeper levels are mostly built, now and then a cave pocket off a
//! room; the deepest ends in the boss chamber, a dead end (on a one-level hideout the captain's quarters are it).

use crate::core::rng::Pcg32;
use crate::under::grow::{Grower, Piece, ROOMS_GROWING, Side, Spec, Template, boss_room, fallback, fill, fill_out, fill_share, first_room, rooms};
use crate::under::themes::keep::finish;
use crate::under::themes::warren::{blob, burrow, cavern, den, edge, lay, step};
use crate::under::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn bandit(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    p.secrets = 2 + (area >= 1.3) as usize;
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(4);
    let specs = specs(area);
    let mut boss = None;
    let mut g = if depth == 0 {
        // The hidden entry: a small cave round the arrival.
        let c = ((arrive % nx) as isize, (arrive / nx) as isize);
        let r0 = 1.3 + 0.6 * rng.next_f64();
        let Some(sq) = blob(&p, ny, c, (r0, r0 + 0.3), &[arrive], &|_| false, &[], rng) else { return fallback(p, last) };
        let entry = cavern(&mut p, theme.first);
        lay(&mut p, &sq, entry);
        let mut front = vec![entry];
        // A crawl on into the hill (the grid's wider side) to the common room's cavern; the crawl is the entry's.
        let dir = if arrive % nx < nx / 2 { 0 } else { 2 };
        let mut common = (entry, sq);
        for t in 0..10 {
            let d = if t < 6 { dir } else { [1, 3][t % 2] };
            let s = edge(&p, ny, arrive, d);
            let (path, h) = burrow(&p, ny, s, d, 2 + rng.below(3) as usize, 0.3, rng);
            if path.len() < 2 {
                continue;
            }
            let shrink = 1.0 - 0.06 * t as f64;
            let r = ((3.0 + 1.5 * rng.next_f64()) * shrink, (2.5 + 1.2 * rng.next_f64()) * shrink);
            let Some(sq) = den(&p, ny, &path, h, r, rng) else { continue };
            lay(&mut p, &path, entry);
            let cr = cavern(&mut p, "common room");
            lay(&mut p, &sq, cr);
            p.door(path[path.len() - 1], sq[0]);
            front.push(cr);
            common = (cr, sq);
            break;
        }
        // The lookout up a side crawl off the common room (the crawl is the lookout's).
        for _ in 0..6 {
            let s = common.1[rng.below(common.1.len() as u32) as usize];
            let d = [1, 3][rng.below(2) as usize];
            let s = edge(&p, ny, s, d);
            let (path, h) = burrow(&p, ny, s, d, 2 + rng.below(4) as usize, 0.4, rng);
            if path.len() < 2 {
                continue;
            }
            let r = 1.2 + 0.6 * rng.next_f64();
            let Some(sq) = den(&p, ny, &path, h, (r, r), rng) else { continue };
            let lk = cavern(&mut p, "lookout");
            lay(&mut p, &path, lk);
            lay(&mut p, &sq, lk);
            p.door(s, path[0]);
            front.push(lk);
            break;
        }
        // Behind the common room: a tunnel into the cellars.
        let (cr, sq) = common;
        let (x0, x1) = (sq.iter().map(|k| k % nx).min().unwrap_or(0), sq.iter().map(|k| k % nx).max().unwrap_or(0));
        let (y0, y1) = (sq.iter().map(|k| k / nx).min().unwrap_or(0), sq.iter().map(|k| k / nx).max().unwrap_or(0));
        let mut g = Grower::new(nx, ny, passage, None);
        g.cap = ROOMS_GROWING;
        g.spread = true;
        g.pieces.push(Piece { room: cr, x: x0, y: y0, w: x1 + 1 - x0, h: y1 + 1 - y0, door: None, open: true });
        let back = if dir == 0 { Side::E } else { Side::W };
        'tunnel: for side in [back, Side::N, Side::S] {
            let wall = (if matches!(side, Side::N | Side::S) { x1 + 1 - x0 } else { y1 + 1 - y0 }) as isize;
            for t in 0..wall {
                let along = if t % 2 == 0 { t / 2 } else { -(t + 1) / 2 };
                let Some(&c) = g.deck(&mut p, 0, side, "", Template::Corridor, 1, 2 + rng.below(3) as usize, along, false).first() else { continue };
                let (a, d) = (4 + rng.below(2) as usize, 4 + rng.below(2) as usize);
                if g.deck(&mut p, c, side, "bunk room", Template::Rect, a, d, 0, false).first().is_some() {
                    g.pieces[c].open = false;
                    g.pieces[0].open = false;
                    break 'tunnel;
                }
                g.undo_last(&mut p);
            }
        }
        let left = target.saturating_sub(rooms(&p, passage) + 1);
        g.grow(&mut p, &specs, left, false, rng);
        fill_out(&mut g, &mut p, ny, fill_share(area), FILL, 1, rng);
        // Farthest in, the captain's quarters (the boss chamber on a one-level hideout) behind a door locked first, the
        // loot store beyond; then the secret tunnel back to the cave.
        let kind = if last { BOSS } else { "captain's quarters" };
        if let Some(c) = farthest(&mut p, &mut g, ny, kind, &[(5, 4), (4, 4), (3, 3)], rng) {
            if let Some((a, b)) = g.pieces[c].door {
                p.lock_first.push((a.min(b), a.max(b)));
            }
            if last {
                boss = Some(g.pieces[c].room);
            } else {
                for side in Side::ALL {
                    if let Some(&l) = g.deck(&mut p, c, side, "loot store", Template::Rect, 3 + rng.below(2) as usize, 3, 0, false).first() {
                        g.pieces[l].open = false;
                        break;
                    }
                }
            }
        }
        secret_tunnel(&mut p, &g, ny, &front, passage);
        g
    } else {
        // Mostly built: a landing at the way down from above, rooms grown off it; now and then a cave pocket.
        let Some((mut g, _)) = first_room(&mut p, passage, "landing", Template::Rect, (4, 5), None, 0) else { return fallback(p, last) };
        g.cap = ROOMS_GROWING;
        let left = target.saturating_sub(1);
        g.grow(&mut p, &specs, left, false, rng);
        fill_out(&mut g, &mut p, ny, fill_share(area), FILL, 1, rng);
        if rng.next_f64() < 0.5 {
            pocket(&mut p, &g, ny, rng);
        }
        g
    };
    if last && boss.is_none() {
        g.cap = crate::under::grow::ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng);
    }
    finish(&mut p, ny, passage, 1, last, boss, &[], &[]);
    p
}

/// What spreads a big hideout over its grid (`fill_out`).
const FILL: &[&str] = &["bunk room", "storeroom", "kitchen", "armory"];

/// What grows behind the cave: the theme's dug rooms (rect, shored), small or bigger; short tunnels.
fn specs(area: f64) -> Vec<Spec> {
    let up = |n: usize| ((n as f64) * crate::core::sqrt(area).max(1.0)).round() as usize;
    let mut out = Vec::new();
    for kind in ["bunk room", "bunk room", "kitchen", "storeroom", "armory", "storeroom", "common room", "loot store"] {
        out.push(Spec { kind, t: Template::Rect, across: (3, 5), deep: (3, 4) });
        out.push(Spec { kind, t: Template::Rect, across: (4, up(6)), deep: (4, up(6)) });
    }
    out.push(Spec { kind: "", t: Template::Corridor, across: (1, 1), deep: (2, 5) });
    out.push(Spec { kind: "", t: Template::Corridor, across: (1, 1), deep: (2, 3) });
    out
}

/// Room `kind` grown off the open piece farthest on foot from the arrival that has room for it (the first of `sizes`
/// that fits), closed to more growth. Its piece.
fn farthest(p: &mut Plan, g: &mut Grower, ny: usize, kind: &'static str, sizes: &[(usize, usize)], rng: &mut Pcg32) -> Option<usize> {
    let nx = p.nx;
    let dist = p.distances(p.up, ny, None);
    let mut order: Vec<(i32, usize)> = (0..g.pieces.len())
        .filter(|&i| g.pieces[i].open)
        .map(|i| {
            let f = &g.pieces[i];
            let d = (f.y..f.y + f.h).flat_map(|j| (f.x..f.x + f.w).map(move |x| j * nx + x)).filter(|&k| p.cells[k] == f.room).map(|k| dist[k]).max().unwrap_or(-1);
            (d, i)
        })
        .collect();
    order.sort_by_key(|&(d, i)| (std::cmp::Reverse(d), i));
    let s0 = rng.below(4) as usize;
    for (_, i) in order {
        for &(a, d) in sizes {
            for k in 0..4 {
                if let Some(&c) = g.deck(p, i, Side::ALL[(s0 + k) % 4], kind, Template::Rect, a, d, 0, false).first() {
                    g.pieces[c].open = false;
                    return Some(c);
                }
            }
        }
    }
    None
}

/// The secret tunnel: from a back room (the farthest in on foot that is no captain's room nor beyond one) through a
/// little rock to the nearest room of the cave `front`, a secret door at the back room's end; only where it saves a
/// long walk.
fn secret_tunnel(p: &mut Plan, g: &Grower, ny: usize, front: &[i16], passage: i16) {
    let nx = p.nx;
    let dist = p.distances(p.up, ny, None);
    let skip = |r: i16| r == passage || front.contains(&r) || matches!(p.rooms[r as usize].kind, "captain's quarters" | "loot store" | BOSS);
    let mut far: Vec<(i32, i16)> = Vec::new();
    for f in g.pieces.iter().filter(|f| !skip(f.room)) {
        let d = (f.y..f.y + f.h).flat_map(|j| (f.x..f.x + f.w).map(move |x| j * nx + x)).filter(|&k| p.cells[k] == f.room).map(|k| dist[k]).max().unwrap_or(-1);
        far.push((d, f.room));
    }
    far.sort_by_key(|&(d, r)| (std::cmp::Reverse(d), r));
    let squares = |p: &Plan, r: i16| (0..nx * ny).filter(|&k| p.cells[k] == r).collect::<Vec<usize>>();
    for &(d, b) in far.iter().take(5) {
        let bs = squares(p, b);
        // (The front room nearest it.)
        let Some((gap, f)) = front.iter().map(|&f| (squares(p, f).iter().flat_map(|&a| bs.iter().map(move |&c| (a % nx).abs_diff(c % nx) + (a / nx).abs_diff(c / nx))).min().unwrap_or(usize::MAX), f)).min() else { return };
        if gap > 14 || d < gap as i32 + 12 {
            continue;
        }
        let (cells, doors) = (p.cells.clone(), p.doors.clone());
        let made = dig(p, nx, ny, b, f, passage, false);
        let fresh = (0..nx * ny).filter(|&k| cells[k] < 0 && p.cells[k] >= 0).count();
        let at_b = made.iter().copied().find(|e| !doors.contains(e) && (p.cells[e.0] == b || p.cells[e.1] == b));
        if made.is_empty() || fresh > 16 || at_b.is_none() || p.secret.len() >= p.secrets {
            (p.cells, p.doors) = (cells, doors);
            continue;
        }
        p.secret.extend(at_b);
        return;
    }
}

/// A cave pocket off a room: a short crawl out of its wall to a small cavern (the crawl is the cavern's).
fn pocket(p: &mut Plan, g: &Grower, ny: usize, rng: &mut Pcg32) {
    let nx = p.nx;
    let rooms: Vec<&Piece> = g.pieces.iter().filter(|f| f.room != g.passage).collect();
    for _ in 0..8 {
        let Some(f) = rooms.get(rng.below(rooms.len().max(1) as u32) as usize) else { return };
        let d = rng.below(4) as usize;
        let s = edge(p, ny, (f.y + f.h / 2) * nx + f.x + f.w / 2, d);
        if p.cells[s] != f.room || step(s, nx, ny, d).is_none_or(|m| p.cells[m] >= 0) {
            continue;
        }
        let (path, h) = burrow(p, ny, s, d, 2 + rng.below(3) as usize, 0.4, rng);
        if path.len() < 2 {
            continue;
        }
        let r = (1.8 + 1.4 * rng.next_f64(), 1.6 + 1.2 * rng.next_f64());
        let Some(sq) = den(p, ny, &path, h, r, rng) else { continue };
        let c = cavern(p, "cavern");
        lay(p, &path, c);
        lay(p, &sq, c);
        p.door(s, path[0]);
        return;
    }
}
