//! Lava tubes: a meandering tube with side pockets (cave chambers grown beside it); the lower tube carries a lava
//! channel.

use crate::core::rng::Pcg32;
use crate::under::*;
use super::cave::{Caverns, around, caverns, natural_level};

/// A meandering lava tube with side pockets grown off it; the lower tube carries a lava channel.
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
    open[arrive] = true;
    // (The tube's chambers of the first two kinds, the pockets of the rest.)
    let (tube, pockets) = theme.kinds.split_at(2.min(theme.kinds.len()));
    let mut p = natural_level(nx, ny, arrive, last, open, tube, theme.ledge, rng);
    // Side pockets: cave chambers grown in the rock beside the tube at a few places along it (`cave::caverns`), joined
    // to it by short tunnels; kept off the boss chamber and the ways in and out.
    let near = |p: &Plan, m: usize| p.cells[m] >= 0 && p.rooms[p.cells[m] as usize].kind == BOSS || m == arrive || Some(m) == p.down;
    let shy: Vec<bool> = (0..n).map(|k| near(&p, k) || around(k, nx, ny).any(|m| near(&p, m) || around(m, nx, ny).any(|t| near(&p, t)))).collect();
    let mut region = vec![false; n];
    for _ in 0..3 + rng.below(3) {
        let i = x0 + rng.below((x1 - x0).max(1) as u32) as isize;
        let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
        let (c, w) = (centre(i as f64), half(i as f64));
        let r = rng.range(4.0, 6.5);
        let pc = c + side * (w + r - 0.5);
        for k in 0..n {
            let (dx, dy) = ((k % nx) as f64 - i as f64, (k / nx) as f64 - pc);
            if dx * dx + dy * dy <= r * r && p.cells[k] < 0 && !shy[k] {
                region[k] = true;
            }
        }
    }
    let kinds = if pockets.is_empty() { tube } else { pockets };
    let cv = Caverns { kinds, ledge: "", spacing: 7.0, seeds: &[], raise: 0.0, squeeze: 0.3, loops: 0.3, reach: 6, dead_end: false };
    let grown = caverns(&mut p, ny, &region, &cv, rng);
    // (Rubble slopes up into a pocket off a raised stretch of the tube.)
    for j in grown.joins.iter().filter(|j| j.a < 0 || j.b < 0) {
        let raised = j.squares.iter().any(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] >= 0 && !grown.rooms.contains(&p.cells[m]) && p.rooms[p.cells[m] as usize].raise > 0.0));
        if raised {
            let slope: Vec<crate::interior::Item> = j.squares.iter().filter(|&&k| !p.taken[k]).map(|&k| item("rubble_slope", k % nx, k / nx, 1, 1)).collect();
            p.put_all(slope);
        }
    }
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
