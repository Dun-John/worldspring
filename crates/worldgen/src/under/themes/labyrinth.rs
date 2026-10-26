//! Labyrinths (`labyrinth`): every level a maze of passages a square wide between walls a square thick. The maze runs on
//! a lattice of every other square (lined up with the way in): rooms take boxes of lattice points (the maze gate or a
//! landing round the way in, the heart, dead-end side rooms: bone pits, treasure nooks, trapped ends), the rest is dug
//! by a growing tree (mostly from its newest end: long winding runs), then a few walls knocked through for loops (more
//! than one route). Every room but the gate is entered by one way only; the stairs down stand in the heart, and on the
//! deepest level the heart is the boss's lair. The heart lies mid-grid on the top level, then a quarter of the way in
//! from the side away from the way down from above. No cave-ins.

use crate::core::rng::Pcg32;
use crate::under::grow::fallback;
use crate::under::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn labyrinth(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    p.secrets = 2 + (area >= 1.3) as usize;
    p.loops = 0.0;
    let (ax, ay) = (arrive % nx, arrive / nx);
    // The lattice: squares of the arrival's parity, inside the grid's rock rim.
    let xs: Vec<usize> = (1..nx.saturating_sub(1)).filter(|x| x % 2 == ax % 2).collect();
    let ys: Vec<usize> = (1..ny.saturating_sub(1)).filter(|y| y % 2 == ay % 2).collect();
    let (cw, ch) = (xs.len(), ys.len());
    let (Some(ai), Some(aj)) = (xs.iter().position(|&x| x == ax), ys.iter().position(|&y| y == ay)) else { return fallback(p, last) };
    if cw < 8 || ch < 6 {
        return fallback(p, last);
    }
    // Rooms by lattice box (i0, j0, w, h in points); -1 maze.
    let mut owner = vec![-1i16; cw * ch];
    let mut boxes: Vec<(usize, usize, usize, usize, i16)> = Vec::new();
    let fits = |owner: &[i16], (i0, j0, w, h): (usize, usize, usize, usize), ring: usize| {
        i0 + w <= cw && j0 + h <= ch && {
            let (x0, y0, x1, y1) = (i0.saturating_sub(ring), j0.saturating_sub(ring), (i0 + w + ring).min(cw), (j0 + h + ring).min(ch));
            (y0..y1).all(|j| (x0..x1).all(|i| owner[j * cw + i] < 0))
        }
    };
    let claim = |owner: &mut [i16], boxes: &mut Vec<(usize, usize, usize, usize, i16)>, (i0, j0, w, h): (usize, usize, usize, usize), r: i16| {
        for j in j0..j0 + h {
            for i in i0..i0 + w {
                owner[j * cw + i] = r;
            }
        }
        boxes.push((i0, j0, w, h, r));
    };
    // The gate (or a landing) round the way in: 2 × 2 points, the arrival one of them.
    let gate = p.room(if depth == 0 { theme.first } else { "landing" }, 0.0);
    let gi = if ai + 1 < cw { ai } else { ai - 1 };
    let gj = if aj + 1 < ch && rng.next_f64() < 0.5 || aj == 0 { aj } else { aj - 1 };
    claim(&mut owner, &mut boxes, (gi, gj, 2, 2), gate);
    // The heart: mid-grid on the top level, else a quarter in from the side away from the arrival.
    let (tx, ty) = (if depth == 0 { nx / 2 } else if ax < nx / 2 { 3 * nx / 4 } else { nx / 4 }, ny / 2);
    let big = crate::core::sqrt(area) >= 1.2;
    let (hw, hh) = if last { if big { (4, 4) } else { (3, 3) } } else if big { (3, 3) } else { (3, 2) };
    let heart = p.room(if last { BOSS } else { "heart of the maze" }, 0.0);
    let mut spots: Vec<(usize, usize)> = (0..=ch.saturating_sub(hh)).flat_map(|j| (0..=cw.saturating_sub(hw)).map(move |i| (i, j))).collect();
    let mid = |i: usize, j: usize| (xs[i] + xs[(i + hw - 1).min(cw - 1)], ys[j] + ys[(j + hh - 1).min(ch - 1)]);
    spots.sort_by_key(|&(i, j)| {
        let (mx, my) = mid(i, j);
        (mx.abs_diff(2 * tx).pow(2) + my.abs_diff(2 * ty).pow(2), i, j)
    });
    let Some(&hb) = spots.iter().find(|&&(i, j)| fits(&owner, (i, j, hw, hh), 2)) else { return fallback(p, last) };
    claim(&mut owner, &mut boxes, (hb.0, hb.1, hw, hh), heart);
    // Dead-end side rooms.
    let want = ((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * crate::under::grow::fill(area)).round() as usize;
    let kinds = &theme.kinds[..3];
    let mut made = 0;
    for _ in 0..want * 30 {
        if made >= want {
            break;
        }
        let (w, h) = [(2, 2), (3, 2), (2, 3), (2, 2)][rng.below(4) as usize];
        if cw < w + 2 || ch < h + 2 {
            break;
        }
        let (i, j) = (rng.below((cw - w + 1) as u32) as usize, rng.below((ch - h + 1) as u32) as usize);
        if fits(&owner, (i, j, w, h), 1) {
            let r = p.room(kinds[made % kinds.len()], 0.0);
            claim(&mut owner, &mut boxes, (i, j, w, h), r);
            made += 1;
        }
    }
    // The maze: a growing tree from the gate over the lattice points and rooms (a room but the gate is entered once).
    let nr = p.rooms.len();
    let mut seen_room = vec![false; nr];
    let mut seen = vec![false; cw * ch];
    let mut links: Vec<(usize, usize)> = Vec::new();
    let near = |q: usize| {
        let (i, j) = (q % cw, q / cw);
        [(i + 1 < cw).then(|| q + 1), (j + 1 < ch).then(|| q + cw), (i > 0).then(|| q - 1), (j > 0).then(|| q - cw)]
    };
    seen_room[gate as usize] = true;
    let mut active: Vec<usize> = (0..cw * ch).filter(|&q| owner[q] == gate).collect();
    for &q in &active {
        seen[q] = true;
    }
    loop {
        while !active.is_empty() {
            let at = if rng.next_f64() < 0.75 { active.len() - 1 } else { rng.below(active.len() as u32) as usize };
            let q = active[at];
            let mut open: Vec<usize> = near(q)
                .into_iter()
                .flatten()
                .filter(|&m| !seen[m] && (owner[m] < 0 || (!seen_room[owner[m] as usize] && owner[q] != owner[m])))
                .filter(|&m| owner[q] < 0 || owner[m] < 0)
                .collect();
            // (Out of the gate: only onto the maze.)
            if owner[q] == gate {
                open.retain(|&m| owner[m] < 0);
            }
            if open.is_empty() {
                active.remove(at);
                continue;
            }
            let m = open[rng.below(open.len() as u32) as usize];
            links.push((q, m));
            if owner[m] < 0 {
                seen[m] = true;
                active.push(m);
            } else {
                // A room entered: all its points seen, never left.
                let r = owner[m] as usize;
                seen_room[r] = true;
                for k in 0..cw * ch {
                    if owner[k] == r as i16 {
                        seen[k] = true;
                    }
                }
            }
        }
        // (A pocket the rooms closed off: opened from a maze point beside it.)
        let Some((q, m)) = (0..cw * ch).filter(|&q| seen[q] && owner[q] < 0).find_map(|q| near(q).into_iter().flatten().find(|&m| !seen[m] && owner[m] < 0).map(|m| (q, m))) else { break };
        links.push((q, m));
        seen[m] = true;
        active.push(m);
    }
    if !seen_room[heart as usize] {
        return fallback(p, last);
    }
    // A few loops: walls knocked through between maze points.
    let cells = (0..cw * ch).filter(|&q| seen[q] && owner[q] < 0).count();
    let loops = 1 + cells / 70 + rng.below(2) as usize;
    let mut tries = 0;
    let mut added = 0;
    while added < loops && tries < 400 {
        tries += 1;
        let q = rng.below((cw * ch) as u32) as usize;
        if !seen[q] || owner[q] >= 0 {
            continue;
        }
        let Some(m) = near(q)[rng.below(4) as usize] else { continue };
        if !seen[m] || owner[m] >= 0 || links.contains(&(q, m)) || links.contains(&(m, q)) {
            continue;
        }
        links.push((q, m));
        added += 1;
    }
    // Dig it: rooms over their boxes, maze points and the walls between linked ones as the passage, a door where the
    // passage meets a room.
    let sq = |q: usize| ys[q / cw] * nx + xs[q % cw];
    for &(i0, j0, w, h, r) in &boxes {
        if !seen_room[r as usize] {
            continue;
        }
        for y in ys[j0]..=ys[j0 + h - 1] {
            for x in xs[i0]..=xs[i0 + w - 1] {
                p.cells[y * nx + x] = r;
            }
        }
    }
    for q in (0..cw * ch).filter(|&q| seen[q] && owner[q] < 0) {
        p.cells[sq(q)] = passage;
    }
    for &(q, m) in &links {
        let (a, b) = (sq(q), sq(m));
        let wall = (a + b) / 2;
        p.cells[wall] = passage;
        for (o, s) in [(q, a), (m, b)] {
            if owner[o] >= 0 {
                p.door(wall, s);
            }
        }
    }
    // The way down in the heart (its middle), or the boss there.
    p.passage = Some((passage, false));
    p.taken[p.up] = true;
    if !last {
        let (i0, j0, w, h, _) = boxes[1];
        let d = ys[j0 + h / 2] * nx + xs[i0 + w / 2];
        let d = if p.cells[d] == heart { d } else { (0..nx * ny).find(|&k| p.cells[k] == heart).unwrap_or(d) };
        p.down = Some(d);
        p.taken[d] = true;
    }
    p
}
