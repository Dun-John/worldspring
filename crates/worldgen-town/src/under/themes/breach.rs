//! Cave-ins (the toolkit's breach): a cavern grown by a cellular automaton across a built level's rooms, its own room
//! drawn as a cavern (`CAVERN`), joined to every piece of a room it touches by an open gap (`Plan::gaps`) one to three
//! squares wide, a rubble slope spilling over each gap and tumbled blocks (`collapsed_wall`) on the room side. Run before
//! the locks and keys (`kit::guard` then locks round it). It never takes the ways in and out nor a square of the boss
//! chamber nor of a chasm hall (its stone bridge), and is undone unless every square stays reachable and none is reached
//! only through the boss chamber. What stood on the squares it takes goes (statues, pillars), and so do the doors into
//! it: each piece of a room it touches opens on it by its gap alone (the boss chamber keeps its door).
//! Under ruins about half the levels get one, under living castles and cities about one site in ten (`site`); the
//! collapsed fortress (`themes::fortress`) lays its own, more and bigger the deeper.

use crate::core::rng::Pcg32;
use crate::under::*;

/// Carve a cave-in of about radius `r` (squares) round a built square drawn by `seed`. Whether one was carved.
pub(crate) fn breach(p: &mut Plan, ny: usize, seed: u64, r: f64) -> bool {
    let nx = p.nx;
    let n = nx * ny;
    if p.natural || nx < 8 || ny < 8 {
        return false;
    }
    let mut rng = Pcg32::new(seed, 97);
    let boss = |p: &Plan, k: usize| p.cells[k] >= 0 && p.rooms[p.cells[k] as usize].kind == BOSS;
    // (Nor a chasm hall: its stone bridge is kept clear of the chasm laid later.)
    let chasm = p.chasm.map(|c| c.0);
    let off = |p: &Plan, k: usize| boss(p, k) || chasm.is_some_and(|r| p.cells[k] == r);
    // (The ways in and out, and their links, stay as they are.)
    let mut keep = vec![false; n];
    keep[p.up] = true;
    if let Some(d) = p.down {
        keep[d] = true;
    }
    for it in p.items.iter().filter(|it| matches!(it.kind, "up" | "down" | "exit" | "tunnel")) {
        keep[it.y as usize * nx + it.x as usize] = true;
    }
    for l in &p.links {
        keep[l.y as usize * nx + l.x as usize] = true;
    }
    let inner = |k: usize| k % nx >= 1 && k / nx >= 1 && k % nx + 2 <= nx && k / nx + 2 <= ny;
    let floor: Vec<usize> = (0..n).filter(|&k| p.cells[k] >= 0 && !off(p, k) && !keep[k] && inner(k)).collect();
    if floor.is_empty() {
        return false;
    }
    for _ in 0..4 {
        let c = floor[rng.below(floor.len() as u32) as usize];
        let (rx, ry) = (r * (0.8 + 0.4 * rng.next_f64()), r * (0.8 + 0.4 * rng.next_f64()));
        let (cx, cy) = ((c % nx) as isize, (c / nx) as isize);
        let (ex, ey) = (crate::core::ceil(rx) as isize + 1, crate::core::ceil(ry) as isize + 1);
        let (x0, x1) = ((cx - ex).max(1), (cx + ex).min(nx as isize - 2));
        let (y0, y1) = ((cy - ey).max(1), (cy + ey).min(ny as isize - 2));
        let (bw, bh) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
        // The automaton: a random fill inside the ellipse, smoothed (open where 5 of the 9 squares round are).
        let ok = |i: isize, j: isize| !keep[j as usize * nx + i as usize] && !off(p, j as usize * nx + i as usize);
        let mut open = vec![false; bw * bh];
        for j in 0..bh {
            for i in 0..bw {
                let (dx, dy) = ((x0 + i as isize - cx) as f64 / rx, (y0 + j as isize - cy) as f64 / ry);
                open[j * bw + i] = dx * dx + dy * dy <= 1.0 && rng.next_f64() < 0.62;
            }
        }
        for _ in 0..4 {
            let was = open.clone();
            for j in 0..bh {
                for i in 0..bw {
                    let mut m = 0;
                    for (di, dj) in (-1..=1isize).flat_map(|a| (-1..=1isize).map(move |b| (a, b))) {
                        let (a, b) = (i as isize + di, j as isize + dj);
                        if a >= 0 && b >= 0 && (a as usize) < bw && (b as usize) < bh && was[b as usize * bw + a as usize] {
                            m += 1;
                        }
                    }
                    open[j * bw + i] = m >= 5;
                }
            }
        }
        open[(cy - y0) as usize * bw + (cx - x0) as usize] = true;
        // The part joined to the middle, off the squares it may not take.
        let mut cave = vec![false; n];
        let mut stack = vec![c];
        cave[c] = true;
        let mut sq = Vec::new();
        while let Some(k) = stack.pop() {
            sq.push(k);
            for m in neighbours(k, nx, ny) {
                let (i, j) = ((m % nx) as isize, (m / nx) as isize);
                if !cave[m] && i >= x0 && i <= x1 && j >= y0 && j <= y1 && open[(j - y0) as usize * bw + (i - x0) as usize] && ok(i, j) {
                    cave[m] = true;
                    stack.push(m);
                }
            }
        }
        if sq.len() < 10 {
            continue;
        }
        sq.sort_unstable();
        let saved = (p.cells.clone(), p.doors.clone(), p.secret.clone(), p.portcullis.clone(), p.rooms.len(), p.items.clone(), p.taken.clone(), p.blocked.clone(), p.lock_first.clone());
        let at_boss: Vec<bool> = (0..n).map(|k| boss(p, k)).collect();
        let cr = super::warren::cavern(p, "cave-in");
        for &k in &sq {
            p.cells[k] = cr;
        }
        // (Doors into it go, the boss chamber's but: each piece of a room it touches gets its gap below.)
        for set in [&mut p.doors, &mut p.secret, &mut p.portcullis] {
            set.retain(|&(a, b)| !(cave[a] || cave[b]) || !(cave[a] && cave[b]) && (at_boss[a] || at_boss[b]));
        }
        let doors = &p.doors;
        p.lock_first.retain(|e| doors.contains(e));
        // (What stood on its squares goes, the ways but, which it never takes; and their squares are free again.)
        let on = |it: &Item| (it.y as usize..(it.y + it.h) as usize).any(|j| (it.x as usize..(it.x + it.w) as usize).any(|i| cave[j * nx + i]));
        let gone: Vec<Item> = p.items.iter().filter(|it| on(it)).cloned().collect();
        if !gone.is_empty() {
            p.items.retain(|it| !on(it));
            for it in &gone {
                for j in it.y as usize..(it.y + it.h) as usize {
                    for i in it.x as usize..(it.x + it.w) as usize {
                        p.taken[j * nx + i] = false;
                    }
                }
            }
            for it in &p.items {
                for j in it.y as usize..(it.y + it.h) as usize {
                    for i in it.x as usize..(it.x + it.w) as usize {
                        p.taken[j * nx + i] = true;
                    }
                }
            }
            p.blocked = blocked_squares(&p.items, nx, ny);
        }
        for &k in &sq {
            p.taken[k] = false;
        }
        // The pieces of rooms it touches (4 ways within their room), each opened onto it by one gap.
        let mut piece = vec![u32::MAX; n];
        let mut pieces: Vec<Vec<(usize, usize)>> = Vec::new();
        for &a in &sq {
            for b in neighbours(a, nx, ny) {
                let rb = p.cells[b];
                if rb < 0 || rb == cr || boss(p, b) {
                    continue;
                }
                if piece[b] == u32::MAX {
                    let id = pieces.len() as u32;
                    pieces.push(Vec::new());
                    let mut st = vec![b];
                    piece[b] = id;
                    while let Some(k) = st.pop() {
                        for m in neighbours(k, nx, ny) {
                            if piece[m] == u32::MAX && p.cells[m] == rb {
                                piece[m] = id;
                                st.push(m);
                            }
                        }
                    }
                }
                pieces[piece[b] as usize].push((a, b));
            }
        }
        let mut gaps: Vec<(usize, usize)> = Vec::new();
        for edges in &pieces {
            // (Off the ways and their squares where it can.)
            let good: Vec<&(usize, usize)> = edges.iter().filter(|e| !p.taken[e.1]).collect();
            let &(a, b) = if good.is_empty() { &edges[rng.below(edges.len() as u32) as usize] } else { good[rng.below(good.len() as u32) as usize] };
            let along = if a.abs_diff(b) == 1 { nx } else { 1 };
            let wide = 1 + rng.below(3) as usize;
            let mut at = vec![(a, b)];
            for s in 1..wide {
                let side = if s % 2 == 1 { 1isize } else { -1 };
                let reach = s.div_ceil(2) as isize * side * along as isize;
                let (a2, b2) = (a as isize + reach, b as isize + reach);
                if a2 < 0 || b2 < 0 || a2 as usize >= n || b2 as usize >= n {
                    continue;
                }
                let (a2, b2) = (a2 as usize, b2 as usize);
                if neighbours(a2, nx, ny).any(|m| m == b2) && cave[a2] && p.cells[a2] == cr && piece[b2] == piece[b] && edges.contains(&(a2, b2)) {
                    at.push((a2, b2));
                }
            }
            for (a, b) in at {
                // (Never hidden nor barred: the pair was dropped from those with the doors into the cave.)
                p.door(a, b);
                p.gaps.insert((a.min(b), a.max(b)));
                gaps.push((a, b));
            }
        }
        // Every square still reached, and none only through the boss chamber.
        let dist = p.distances(p.up, ny, Some(&p.blocked));
        let mut shut = p.blocked.clone();
        for k in (0..n).filter(|&k| boss(p, k)) {
            shut[k] = true;
        }
        let round = p.distances(p.up, ny, Some(&shut));
        if gaps.is_empty() || (0..n).any(|k| p.cells[k] >= 0 && !p.blocked[k] && (dist[k] < 0 || (!shut[k] && round[k] < 0))) {
            (p.cells, p.doors, p.secret, p.portcullis) = (saved.0, saved.1, saved.2, saved.3);
            p.rooms.truncate(saved.4);
            (p.items, p.taken, p.blocked, p.lock_first) = (saved.5, saved.6, saved.7, saved.8);
            for &(a, b) in &gaps {
                p.gaps.remove(&(a.min(b), a.max(b)));
            }
            continue;
        }
        // Tumbled blocks on the room side of every other gap, rubble over each.
        for &(_, b) in gaps.iter().step_by(2) {
            if !p.taken[b] && p.rooms[p.cells[b] as usize].shape != crate::interior::shape::CAVERN {
                p.place(item("collapsed_wall", b % nx, b / nx, 1, 1), Some(p.cells[b]), ny);
            }
        }
        let mut spill = Vec::new();
        let mut g = Ground::of_plan(p);
        for &(a, b) in &gaps {
            let (xa, ya, xb, yb) = (a % nx, a / nx, b % nx, b / nx);
            let (x, y) = (xa.min(xb).saturating_sub(1), ya.min(yb).saturating_sub(1));
            let (w, h) = (xa.max(xb) + 2 - x, ya.max(yb) + 2 - y);
            for it in rubble_slope(&g, x, y, w, h) {
                for k in (it.y as usize..(it.y + it.h) as usize).flat_map(|j| (it.x as usize..(it.x + it.w) as usize).map(move |i| j * nx + i)) {
                    g.free[k] = false;
                }
                spill.push(it);
            }
        }
        p.put_all(spill);
        return true;
    }
    false
}
