//! Mines: a haulage way with side drifts and stopes, a shaft to the next level.

use crate::core::rng::Pcg32;
use crate::under::*;

/// A haulage way with side drifts and stopes; a shaft at the end, or the boss chamber.
pub(crate) fn mine(nx: usize, ny: usize, arrive: usize, last: bool, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, true);
    let (ax, ay) = ((arrive % nx) as isize, (arrive / nx) as isize);
    let toward: isize = if (ax as usize) < nx / 2 { 1 } else { -1 };
    let end = if toward > 0 {
        nx as isize - (if last { 13 } else { 4 })
    } else if last {
        12
    } else {
        3
    };
    let haul = p.room("haulage way", 0.0);
    let ok = |i: isize, j: isize| i >= 1 && j >= 2 && i < nx as isize - 1 && j < ny as isize - 2;
    let mut y = ay;
    let mut x = ax;
    let mut main: Vec<(isize, isize)> = Vec::new();
    // Two squares wide (rails down the middle); jogs a square now and then.
    while x != end + toward {
        for dj in [0, -1] {
            if ok(x, y + dj) {
                p.cells[(y + dj) as usize * nx + x as usize] = haul;
            }
        }
        main.push((x, y));
        if rng.next_f64() < 0.12 {
            let ny_ = (y + if rng.next_f64() < 0.5 { 1 } else { -1 }).clamp(3, ny as isize - 4);
            for dj in [0, -1] {
                if ok(x, ny_ + dj) {
                    p.cells[(ny_ + dj) as usize * nx + x as usize] = haul;
                }
            }
            y = ny_;
        }
        x += toward;
    }
    // From the arrival square onto the haulage way.
    for j in ay.min(y - 1)..=ay.max(y) {
        if ok(ax, j) {
            p.cells[j as usize * nx + ax as usize] = haul;
        }
    }
    let free = |p: &Plan, i: isize, j: isize, own: i16| {
        ok(i, j) && neighbours(j as usize * nx + i as usize, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == own) && p.cells[j as usize * nx + i as usize] < 0
    };
    // Side drifts, kept a square of rock from everything else; a stope at some ends.
    let mut k = 4 + rng.below(4) as usize;
    while k + 6 < main.len() {
        let (mx, my) = main[k];
        let side: isize = if rng.next_f64() < 0.5 { 1 } else { -1 };
        let id = p.room("drift", 0.0);
        let (mut i, mut j) = (mx, if side > 0 { my + 1 } else { my - 2 });
        let len = 5 + rng.below(10) as isize;
        let mut dug = 0;
        let mut last_sq = None;
        for s in 0..len {
            // The first square opens off the haulage way.
            let fits = if s == 0 {
                ok(i, j) && p.cells[j as usize * nx + i as usize] < 0
            } else {
                free(&p, i, j, id)
                    || (ok(i, j)
                        && p.cells[j as usize * nx + i as usize] < 0
                        && neighbours(j as usize * nx + i as usize, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == id || (s == 1 && p.cells[m] == haul)))
            };
            if !fits {
                break;
            }
            p.cells[j as usize * nx + i as usize] = id;
            dug += 1;
            last_sq = Some((i, j));
            if rng.next_f64() < 0.2 {
                i += if rng.next_f64() < 0.5 { 1 } else { -1 };
                if !(ok(i, j) && p.cells[j as usize * nx + i as usize] < 0) {
                    break;
                }
                p.cells[j as usize * nx + i as usize] = id;
            }
            j += side;
        }
        if dug >= 4
            && rng.next_f64() < 0.6
            && let Some((ei, ej)) = last_sq
        {
            let kind = ["stope", "ore chamber", "powder store", "pump room", "collapsed stope"][rng.below(5) as usize];
            let st = p.room(kind, 0.0);
            let (w, h) = (3 + rng.below(3) as isize, 3 + rng.below(2) as isize);
            let (x0, y0) = (ei - w / 2, if side > 0 { ej + 1 } else { ej - h });
            let fits = (y0..y0 + h).all(|j| {
                (x0..x0 + w).all(|i| {
                    free(&p, i, j, st)
                        || (ok(i, j)
                            && p.cells[j as usize * nx + i as usize] < 0
                            && neighbours(j as usize * nx + i as usize, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == st || p.cells[m] == id))
                })
            });
            if fits {
                for j in y0..y0 + h {
                    for i in x0..x0 + w {
                        p.cells[j as usize * nx + i as usize] = st;
                    }
                }
            }
        }
        k += 5 + rng.below(5) as usize;
    }
    let (ex, ey) = *main.last().unwrap_or(&(ax, ay));
    if last {
        // The boss chamber at the end of the haulage way: the deep vein.
        let boss = p.room(BOSS, 0.0);
        let (w, h) = (9isize, 8isize);
        let x0 = if toward > 0 { ex + 1 } else { ex - w };
        let y0 = (ey - h / 2).clamp(1, ny as isize - 1 - h);
        for j in y0..y0 + h {
            for i in x0..x0 + w {
                if i >= 1 && i < nx as isize - 1 && p.cells[j as usize * nx + i as usize] < 0 {
                    p.cells[j as usize * nx + i as usize] = boss;
                }
            }
        }
        // Joined to the haulage way at its end.
        for dj in [0, -1] {
            let k = (ey + dj) as usize * nx + (ex + toward) as usize;
            if p.cells[k] < 0 {
                p.cells[k] = boss;
            }
        }
    } else {
        let shaft = p.room("shaft head", 0.0);
        for dj in [0, -1] {
            p.cells[(ey + dj) as usize * nx + ex as usize] = shaft;
        }
        p.down = Some(ey as usize * nx + ex as usize);
    }
    // Whatever the arrival can't reach goes back to rock.
    let dist = p.distances(arrive, ny, None);
    for k in 0..nx * ny {
        if dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    p
}
