//! Caves: a cellular automaton grown around a winding spine, and `natural_level`, which turns open squares into
//! chambers (shared with lava tubes).

use crate::core::rng::Pcg32;
use crate::under::*;

/// Cellular automaton cave grown around a winding spine from the arrival toward the far side.
pub(crate) fn cave(nx: usize, ny: usize, arrive: usize, last: bool, theme: &Theme, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut open = vec![false; n];
    let interior = |k: usize| {
        let (i, j) = (k % nx, k / nx);
        i >= 1 && j >= 1 && i + 1 < nx && j + 1 < ny
    };
    for k in 0..n {
        open[k] = interior(k) && rng.next_f64() < 0.45;
    }
    // The spine: a biased walk away from the arrival, two squares wide.
    let mut keep = vec![false; n];
    let (mut x, mut y) = ((arrive % nx) as f64, (arrive / nx) as f64);
    let toward = if ((arrive % nx) as f64) < nx as f64 / 2.0 { 1.0 } else { -1.0 };
    let mut heading: f64 = if toward > 0.0 { 0.0 } else { std::f64::consts::PI };
    for _ in 0..(nx * 2) {
        for dj in -1..=1 {
            for di in -1..=1 {
                let (i, j) = ((x as isize + di).clamp(1, nx as isize - 2), (y as isize + dj).clamp(1, ny as isize - 2));
                keep[j as usize * nx + i as usize] = true;
            }
        }
        heading += rng.range(-0.6, 0.6);
        // Pull back toward the far side and away from the edges.
        let back = if toward > 0.0 { 0.0 } else { std::f64::consts::PI };
        heading += 0.15 * libm::sin(back - heading);
        x = (x + libm::cos(heading)).clamp(2.0, nx as f64 - 3.0);
        y = (y + libm::sin(heading) + 0.04 * (ny as f64 / 2.0 - y)).clamp(2.0, ny as f64 - 3.0);
        if (toward > 0.0 && x >= nx as f64 - 4.0) || (toward < 0.0 && x <= 3.0) {
            break;
        }
    }
    for it in 0..5 {
        let mut next = open.clone();
        for k in 0..n {
            if !interior(k) {
                next[k] = false;
                continue;
            }
            let (i, j) = (k % nx, k / nx);
            let mut rock = 0;
            for dj in -1..=1isize {
                for di in -1..=1isize {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    let (a, b) = (i as isize + di, j as isize + dj);
                    if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || !open[b as usize * nx + a as usize] {
                        rock += 1;
                    }
                }
            }
            next[k] = if rock >= 5 {
                false
            } else if rock <= 3 {
                true
            } else {
                open[k]
            };
            if it < 3 && keep[k] {
                next[k] = true;
            }
        }
        open = next;
    }
    open[arrive] = true;
    natural_level(nx, ny, arrive, last, open, theme.kinds, theme.ledge, rng)
}

/// Finish a natural level from its open squares: keep what the arrival reaches, the boss
/// chamber (last level) or the way down at the far end, chambers by nearest seed (some
/// raised as ledges).
pub(super) fn natural_level(nx: usize, ny: usize, arrive: usize, last: bool, open: Vec<bool>, kinds: &[&'static str], ledge: &'static str, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut p = Plan::new(nx, ny, arrive, true);
    let floor = p.room("floor", 0.0);
    for k in 0..n {
        if open[k] {
            p.cells[k] = floor;
        }
    }
    let dist = p.distances(arrive, ny, None);
    for k in 0..n {
        if dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    let far = (0..n).filter(|&k| dist[k] >= 0).max_by_key(|&k| dist[k]).unwrap_or(arrive);
    let max_d = dist[far].max(0);
    // Chambers: nearest of a few seeds by walking distance.
    let cells: Vec<usize> = (0..n).filter(|&k| p.cells[k] >= 0).collect();
    let mut seeds = vec![arrive];
    for _ in 0..(cells.len() / 110 + 3) {
        seeds.push(cells[rng.below(cells.len() as u32) as usize]);
    }
    p.rooms.clear();
    let mut owner = vec![-1i16; n];
    let mut q = std::collections::VecDeque::new();
    for (si, &s) in seeds.iter().enumerate() {
        if owner[s] < 0 {
            let raise = if si == 0 { 0.0 } else { [0.0, 0.0, 0.0, 5.0, 5.0, 10.0][rng.below(6) as usize] };
            let kind = if raise > 0.0 { ledge } else { kinds[rng.below(kinds.len() as u32) as usize] };
            let id = p.room(kind, raise);
            owner[s] = id;
            q.push_back(s);
        }
    }
    while let Some(k) = q.pop_front() {
        for m in neighbours(k, nx, ny) {
            if p.cells[m] >= 0 && owner[m] < 0 {
                owner[m] = owner[k];
                q.push_back(m);
            }
        }
    }
    for k in 0..n {
        if p.cells[k] >= 0 {
            p.cells[k] = owner[k];
        }
    }
    // The arrival stands on the floor (not a ledge).
    let a = p.cells[arrive] as usize;
    p.rooms[a].raise = 0.0;
    if last {
        // The boss chamber: the far end. Squares at least `max_d - reach` steps in, joined to
        // the farthest square: every other square's shortest way in avoids them (a dead end).
        let reach = 12.min(max_d / 3);
        let t = max_d - reach;
        let boss = p.room(BOSS, 0.0);
        let mut q = std::collections::VecDeque::from([far]);
        let mut seen = vec![false; n];
        seen[far] = true;
        let mut chamber = Vec::new();
        while let Some(k) = q.pop_front() {
            chamber.push(k);
            for m in neighbours(k, nx, ny) {
                if !seen[m] && p.cells[m] >= 0 && dist[m] >= t {
                    seen[m] = true;
                    q.push_back(m);
                }
            }
        }
        for &k in &chamber {
            p.cells[k] = boss;
        }
        // Widen it into the rock where that touches nothing else.
        let (fx, fy) = ((far % nx) as isize, (far / nx) as isize);
        for _ in 0..3 {
            for j in fy - 5..=fy + 5 {
                for i in fx - 5..=fx + 5 {
                    if i < 1 || j < 1 || i >= nx as isize - 1 || j >= ny as isize - 1 || (i - fx) * (i - fx) + (j - fy) * (j - fy) > 25 {
                        continue;
                    }
                    let k = j as usize * nx + i as usize;
                    if p.cells[k] < 0 && neighbours(k, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == boss) && neighbours(k, nx, ny).any(|m| p.cells[m] == boss) {
                        p.cells[k] = boss;
                    }
                }
            }
        }
    } else {
        p.down = Some(far);
        let r = p.cells[far] as usize;
        p.rooms[r].raise = 0.0;
    }
    p
}
