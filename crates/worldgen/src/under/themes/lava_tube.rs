//! Lava tubes: a meandering tube with side pockets; the lower tube carries a lava channel.

use crate::core::rng::Pcg32;
use crate::under::*;
use super::cave::natural_level;

/// A meandering lava tube with side pockets; the lower tube carries a lava channel.
pub(crate) fn lava_tube(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut open = vec![false; n];
    let (ax, ay) = ((arrive % nx) as isize, (arrive / nx) as isize);
    let toward: isize = if (ax as usize) < nx / 2 { 1 } else { -1 };
    let (p1, p2, p3) = (rng.range(0.0, 6.28), rng.range(0.0, 6.28), rng.range(0.0, 6.28));
    let amp = (ny as f64 / 2.0 - 6.0).max(1.0);
    let centre = |x: f64| ny as f64 / 2.0 + amp * (0.6 * libm::sin(x * 0.11 + p1) + 0.4 * libm::sin(x * 0.27 + p2));
    let half = |x: f64| if depth > 0 { 2.5 } else { 1.5 } + 1.4 * (0.5 + 0.5 * libm::sin(x * 0.19 + p3));
    let carve = |open: &mut Vec<bool>, i: isize, j: isize| {
        if i >= 1 && j >= 1 && i < nx as isize - 1 && j < ny as isize - 1 {
            open[j as usize * nx + i as usize] = true;
        }
    };
    let (x0, x1) = if toward > 0 { (ax, nx as isize - 3) } else { (2, ax) };
    for i in x0..=x1 {
        let (c, w) = (centre(i as f64), half(i as f64));
        for j in (c - w).floor() as isize..=(c + w).ceil() as isize {
            if (j as f64 - c).abs() <= w {
                carve(&mut open, i, j);
            }
        }
    }
    // From the arrival to the tube.
    let c0 = centre(ax as f64).round() as isize;
    for j in ay.min(c0)..=ay.max(c0) {
        carve(&mut open, ax, j);
        carve(&mut open, ax + toward, j);
    }
    // Side pockets.
    for _ in 0..3 + rng.below(3) {
        let i = x0 + rng.below((x1 - x0).max(1) as u32) as isize;
        let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
        let (c, w) = (centre(i as f64), half(i as f64));
        let r = rng.range(1.8, 3.2);
        let pc = c + side * (w + r + 0.5);
        for j in (pc - r).floor() as isize..=(pc + r).ceil() as isize {
            for di in -(r.ceil() as isize)..=r.ceil() as isize {
                if ((di * di) as f64 + (j as f64 - pc) * (j as f64 - pc)) <= r * r {
                    carve(&mut open, i + di, j);
                }
            }
        }
        let (lo, hi) = (c.min(pc).round() as isize, c.max(pc).round() as isize);
        for j in lo..=hi {
            carve(&mut open, i, j);
        }
    }
    open[arrive] = true;
    let mut p = natural_level(nx, ny, arrive, last, open, theme.kinds, theme.ledge, rng);
    // The lower tube: a lava channel down its middle (never on the ways in, out or the
    // boss chamber's floor).
    if depth > 0 {
        for i in 1..nx as isize - 1 {
            let j = centre(i as f64).round() as isize;
            if j < 1 || j >= ny as isize - 1 {
                continue;
            }
            let k = j as usize * nx + i as usize;
            let r = p.cells[k];
            if r < 0 || p.taken[k] || Some(k) == p.down || k == p.up || p.rooms[r as usize].kind == BOSS {
                continue;
            }
            p.put(lava(i as usize, j as usize));
        }
    }
    p
}
