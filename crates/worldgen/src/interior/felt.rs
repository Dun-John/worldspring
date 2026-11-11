//! R5 towns: a steppe town's round felt hall (`town::roof::hall_ring`: its hall, a temple, an inn)
//! inside: laid out as a yurt is (one round room on one storey, no stairs, its door on the ring's
//! first side: `program`, `furnish_yurt`), furnished for its use (`furnish`).

use super::{Level, building_item};
use crate::town::geom::{self, P};
use crate::town::roof::FeltUse;

/// The felt halls' rooms, by use (`FeltUse` order).
pub const ROOMS: [&str; 3] = ["felt hall", "felt sanctuary", "felt common room"];

/// A felt hall's room (`program`).
pub fn room(u: FeltUse) -> &'static str {
    ROOMS[u as usize]
}

const STEPS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// A felt hall's one round room furnished: its front door through the rim on the side `south` (grid
/// axes) points to, as a yurt's; the hearth in the middle; then by its use:
/// - a hall: a raised seat (a chair of office on a low timber dais) across from the door, benches
///   round the wall;
/// - a temple: the altar across from the door with a brazier each side, mats (floor cushions) in
///   rows facing it off the way up the middle, painted chests by the wall;
/// - an inn: a serving corner (a counter and barrels by the wall to one side at the back), low
///   tables with cushions round the hearth, sleeping places (pallets and bedrolls) round the wall.
///
/// The squares by the door left clear. Each piece's `back` the wall it stands against.
pub(super) fn furnish(lvl: &mut Level, nx: usize, ny: usize, south: P, ext_free: &dyn Fn(isize, isize) -> bool, u: FeltUse) {
    let Some(ri) = lvl.rooms.iter().position(|r| r.kind == room(u)) else { return };
    let ri = ri as i16;
    let cells = lvl.cells.clone();
    let mine = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && cells[j as usize * nx + i as usize] == ri;
    let sq: Vec<(isize, isize)> = (0..nx * ny).filter(|&k| cells[k] == ri).map(|k| ((k % nx) as isize, (k / nx) as isize)).collect();
    if sq.is_empty() {
        return;
    }
    let (x0, x1) = (sq.iter().map(|s| s.0).min().unwrap(), sq.iter().map(|s| s.0).max().unwrap() + 1);
    let (y0, y1) = (sq.iter().map(|s| s.1).min().unwrap(), sq.iter().map(|s| s.1).max().unwrap() + 1);
    let c = [0.5 * (x0 + x1) as f64, 0.5 * (y0 + y1) as f64];
    let big_r = 0.5 * (x1 - x0).max(y1 - y0) as f64;
    // The door: the rim's edge facing most nearly `south` (the ring's door side), onto open ground.
    let mut best: Option<(f64, isize, isize, usize)> = None;
    for &(i, j) in &sq {
        for (side, (di, dj)) in STEPS.into_iter().enumerate() {
            if mine(i + di, j + dj) || !ext_free(i + di, j + dj) {
                continue;
            }
            let m = [i as f64 + 0.5 + 0.5 * di as f64 - c[0], j as f64 + 0.5 + 0.5 * dj as f64 - c[1]];
            let score = (di as f64 * south[0] + dj as f64 * south[1]) * 4.0 + (m[0] * south[0] + m[1] * south[1]) / geom::len(m).max(1e-9);
            if best.is_none_or(|b| score > b.0 + 1e-9) {
                best = Some((score, i, j, side));
            }
        }
    }
    // (Kept clear: the door's square and the one just inside it.)
    let mut inside: Vec<(isize, isize)> = Vec::new();
    if let Some((_, i, j, side)) = best {
        let (di, dj) = STEPS[side];
        inside = vec![(i, j), (i - di, j - dj)];
        lvl.furniture.retain(|f| !inside.iter().any(|&(a, b)| a >= f.x as isize && a < (f.x + f.w) as isize && b >= f.y as isize && b < (f.y + f.h) as isize));
    }
    if let (Some((_, i, j, side)), Some(d)) = (best, lvl.doors.iter_mut().find(|d| d.kind == "front" && d.rooms[0] == ri)) {
        let (i, j) = (i as f32, j as f32);
        (d.a, d.b) = match side {
            0 => ([i, j], [i + 1.0, j]),
            1 => ([i + 1.0, j], [i + 1.0, j + 1.0]),
            2 => ([i, j + 1.0], [i + 1.0, j + 1.0]),
            _ => ([i, j], [i, j + 1.0]),
        };
    }
    // (The walls open where the door now is.)
    lvl.walls = super::wall_runs(&lvl.cells, &lvl.rooms, nx, ny, &lvl.doors);
    let mut held = vec![false; nx * ny];
    for f in &lvl.furniture {
        for j in f.y as usize..(f.y + f.h) as usize {
            for i in f.x as usize..(f.x + f.w) as usize {
                held[j * nx + i] = true;
            }
        }
    }
    for &(i, j) in &inside {
        if mine(i, j) {
            held[j as usize * nx + i as usize] = true;
        }
    }
    let mut door_at = None;
    for d in &lvl.doors {
        let (dx0, dy0) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize);
        let pair = if d.a[0] == d.b[0] { [(dx0 - 1, dy0), (dx0, dy0)] } else { [(dx0, dy0 - 1), (dx0, dy0)] };
        for (i, j) in pair.into_iter().filter(|&(i, j)| mine(i, j)) {
            held[j as usize * nx + i as usize] = true;
            if d.kind == "front" {
                door_at = Some([0.5 * (d.a[0] + d.b[0]) as f64 - c[0], 0.5 * (d.a[1] + d.b[1]) as f64 - c[1]]);
            }
        }
    }
    let to_door = door_at.unwrap_or(south);
    let dn = geom::mul(to_door, 1.0 / geom::len(to_door).max(1e-9));
    let door_angle = libm::atan2(dn[1], dn[0]);
    let side_of = |d: P| if d[0].abs() > d[1].abs() { if d[0] > 0.0 { 1u8 } else { 3 } } else if d[1] > 0.0 { 2 } else { 0 };
    let door_side = side_of(dn);
    let back_side = side_of([-dn[0], -dn[1]]);
    // (The door on a north or south side: pieces across the way in lie along x.)
    let across_x = door_side % 2 == 0;
    let put = |lvl: &mut Level, held: &mut Vec<bool>, kind: &'static str, x: isize, y: isize, w: isize, h: isize, back: u8, variant: Option<u8>| {
        if let Some(mut it) = building_item(kind, x as usize, y as usize, w as usize, h as usize) {
            it.back = Some(back);
            it.variant = variant;
            lvl.furniture.push(it);
            for j in y..y + h {
                for i in x..x + w {
                    held[j as usize * nx + i as usize] = true;
                }
            }
        }
    };
    let free = |held: &Vec<bool>, i: isize, j: isize| mine(i, j) && !held[j as usize * nx + i as usize];
    let fits = |held: &Vec<bool>, x: isize, y: isize, w: isize, h: isize| (y..y + h).all(|j| (x..x + w).all(|i| free(held, i, j)));
    let rim_sq = |i: isize, j: isize| STEPS.iter().any(|&(a, b)| !mine(i + a, j + b));
    // (Each square's bearing from the door: 0 at it, pi across from it; and which hand it is on.)
    let from_door = |i: isize, j: isize| {
        let a = libm::atan2(j as f64 + 0.5 - c[1], i as f64 + 0.5 - c[0]) - door_angle;
        let a = libm::fmod(a + 3.0 * core::f64::consts::PI, 2.0 * core::f64::consts::PI) - core::f64::consts::PI;
        a.abs()
    };
    let left = |i: isize, j: isize| geom::cross(dn, [i as f64 + 0.5 - c[0], j as f64 + 0.5 - c[1]]) > 0.0;
    // (The free box of `w` by `h` nearest a point `d` squares in from the wall across from the door.)
    let across = |held: &Vec<bool>, w: isize, h: isize, d: f64| {
        let t = [c[0] - dn[0] * (big_r - d), c[1] - dn[1] * (big_r - d)];
        let mut out: Option<(f64, isize, isize)> = None;
        for y in y0..y1 - h + 1 {
            for x in x0..x1 - w + 1 {
                if !fits(held, x, y, w, h) {
                    continue;
                }
                let e = geom::dist([x as f64 + 0.5 * w as f64, y as f64 + 0.5 * h as f64], t);
                if out.is_none_or(|o| e < o.0 - 1e-9) {
                    out = Some((e, x, y));
                }
            }
        }
        out.map(|o| (o.1, o.2))
    };
    let out_of = |i: isize, j: isize, w: isize, h: isize| side_of([i as f64 + 0.5 * w as f64 - c[0], j as f64 + 0.5 * h as f64 - c[1]]);
    // The hearth in the middle.
    let (hw, hh) = (if (x1 - x0) % 2 == 0 { 2 } else { 1 }, if (y1 - y0) % 2 == 0 { 2 } else { 1 });
    let (hx, hy) = ((c[0] - 0.5 * hw as f64) as isize, (c[1] - 0.5 * hh as f64) as isize);
    let hearth = if fits(&held, hx, hy, hw, hh) {
        Some((hx, hy, hw, hh))
    } else {
        sq.iter().copied().filter(|&(i, j)| free(&held, i, j)).min_by(|a, b| {
            let d = |s: &(isize, isize)| (s.0 as f64 + 0.5 - c[0]).abs() + (s.1 as f64 + 0.5 - c[1]).abs();
            d(a).total_cmp(&d(b))
        }).map(|(i, j)| (i, j, 1, 1))
    };
    if let Some((x, y, w, h)) = hearth {
        put(lvl, &mut held, "yurt_hearth", x, y, w, h, door_side, None);
    }
    // (A piece that would shut squares off from the door taken up again: `sealed`.)
    let start = inside.iter().copied().rev().find(|&(i, j)| mine(i, j)).unwrap_or(sq[0]);
    let undo = |lvl: &mut Level, held: &mut Vec<bool>| {
        if sealed(lvl, nx, ny, ri, start) && let Some(f) = lvl.furniture.pop() {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    held[j * nx + i] = false;
                }
            }
            return true;
        }
        false
    };
    // (A way kept clear from the door to the hearth: the squares on the line between.)
    for &(i, j) in &sq {
        let p = [i as f64 + 0.5 - c[0], j as f64 + 0.5 - c[1]];
        let along = geom::dot(p, dn);
        if along > 0.0 && geom::cross(dn, p).abs() < 0.75 {
            held[j as usize * nx + i as usize] = true;
        }
    }
    let mut rim: Vec<(isize, isize)> = sq.iter().copied().filter(|&(i, j)| rim_sq(i, j) && from_door(i, j) > 0.9).collect();
    rim.sort_by(|a, b| from_door(b.0, b.1).total_cmp(&from_door(a.0, a.1)).then(a.cmp(b)));
    // (Two-square pieces along the wall from each free rim square and the next round it.)
    let along_wall = |lvl: &mut Level, held: &mut Vec<bool>, kind: &'static str, single: Option<&'static str>, keep: &dyn Fn(isize, isize) -> bool| {
        for &(i, j) in &rim {
            if !free(held, i, j) || !keep(i, j) {
                continue;
            }
            let next = [(1, 0), (0, 1), (-1, 0), (0, -1)].into_iter().map(|(a, b)| (i + a, j + b)).find(|&(a, b)| free(held, a, b) && rim.contains(&(a, b)) && keep(a, b));
            match (next, single) {
                (Some((a, b)), _) => {
                    let (x, y, w, h) = (i.min(a), j.min(b), (i - a).abs() + 1, (j - b).abs() + 1);
                    put(lvl, held, kind, x, y, w, h, out_of(x, y, w, h), None);
                }
                (None, Some(k)) => put(lvl, held, k, i, j, 1, 1, out_of(i, j, 1, 1), None),
                (None, None) => {}
            }
        }
    };
    match u {
        FeltUse::Hall => {
            // The raised seat: a chair of office on a low timber dais, across from the door.
            for (w, d) in [(3, 2), (3, 1), (1, 1)] {
                let (dw, dh) = if across_x { (w, d) } else { (d, w) };
                let Some((x, y)) = across(&held, dw, dh, 0.5 * d as f64 + 0.6) else { continue };
                put(lvl, &mut held, "low_dais", x, y, dw, dh, back_side, Some(1));
                // (The chair on the dais's back row, at its middle.)
                let (ci, cj) = match back_side {
                    0 => (x + dw / 2, y),
                    1 => (x + dw - 1, y + dh / 2),
                    2 => (x + dw / 2, y + dh - 1),
                    _ => (x, y + dh / 2),
                };
                if let Some(mut it) = building_item("chair_of_office", ci as usize, cj as usize, 1, 1) {
                    it.back = Some(back_side);
                    lvl.furniture.push(it);
                }
                break;
            }
            // Benches round the wall.
            along_wall(lvl, &mut held, "bench", None, &|_, _| true);
        }
        FeltUse::Temple => {
            // The altar against the wall across from the door, a brazier each side.
            let (aw, ah) = if across_x { (2, 1) } else { (1, 2) };
            for (w, h) in [(aw, ah), (1, 1)] {
                let Some((x, y)) = across(&held, w, h, 0.9) else { continue };
                put(lvl, &mut held, "altar", x, y, w, h, back_side, None);
                if undo(lvl, &mut held) {
                    continue;
                }
                let sides = if across_x { [(x - 1, y), (x + w, y)] } else { [(x, y - 1), (x, y + h)] };
                for (i, j) in sides {
                    if free(&held, i, j) {
                        put(lvl, &mut held, "brazier", i, j, 1, 1, back_side, None);
                        undo(lvl, &mut held);
                    }
                }
                break;
            }
            // Painted chests by the wall at the sides.
            let mut chests = 0;
            for &(i, j) in &rim {
                let a = from_door(i, j);
                if chests < 2 && a > 1.3 && a < 2.0 && free(&held, i, j) && left(i, j) == (chests == 0) {
                    put(lvl, &mut held, "painted_chest", i, j, 1, 1, out_of(i, j, 1, 1), None);
                    if !undo(lvl, &mut held) {
                        chests += 1;
                    }
                }
            }
            // Mats in rows facing the altar, off the rim, every other row, off the way up the middle.
            for &(i, j) in &sq {
                let p = [i as f64 + 0.5 - c[0], j as f64 + 0.5 - c[1]];
                let row = if across_x { j } else { i };
                if free(&held, i, j) && !rim_sq(i, j) && geom::cross(dn, p).abs() >= 0.75 && row % 2 == 0 && geom::len(p) > 1.6 {
                    put(lvl, &mut held, "floor_cushion", i, j, 1, 1, back_side, None);
                }
            }
        }
        FeltUse::Inn => {
            // The serving corner: a counter by the wall on one hand at the back (else the other, else
            // a short one), barrels beside it.
            let band = |i: isize, j: isize, hand: bool| {
                let a = from_door(i, j);
                a > 1.4 && a < 2.7 && left(i, j) == hand
            };
            let mut hand = true;
            let mut counter = false;
            'serve: for (h, two) in [(true, true), (false, true), (true, false), (false, false)] {
                for &(i, j) in &rim {
                    if !free(&held, i, j) || !band(i, j, h) {
                        continue;
                    }
                    let next = [(1, 0), (0, 1), (-1, 0), (0, -1)].into_iter().map(|(a, b)| (i + a, j + b)).find(|&(a, b)| free(&held, a, b) && rim.contains(&(a, b)) && band(a, b, h));
                    let (x, y, w, hh) = match next {
                        Some((a, b)) if two => (i.min(a), j.min(b), (i - a).abs() + 1, (j - b).abs() + 1),
                        _ if two => continue,
                        _ => (i, j, 1, 1),
                    };
                    put(lvl, &mut held, "counter", x, y, w, hh, out_of(x, y, w, hh), None);
                    if !undo(lvl, &mut held) {
                        (hand, counter) = (h, true);
                        break 'serve;
                    }
                }
            }
            let mut barrels = 0;
            for &(i, j) in &rim {
                if counter && barrels < 3 && band(i, j, hand) && free(&held, i, j) {
                    put(lvl, &mut held, "barrel", i, j, 1, 1, out_of(i, j, 1, 1), None);
                    if !undo(lvl, &mut held) {
                        barrels += 1;
                    }
                }
            }
            // Low tables round the hearth, a cushion each side of each where it goes, a square
            // clear round them.
            let mut inner: Vec<(isize, isize)> = sq.iter().copied().filter(|&(i, j)| !rim_sq(i, j)).collect();
            inner.sort_by(|a, b| from_door(b.0, b.1).total_cmp(&from_door(a.0, a.1)).then(a.cmp(b)));
            let mut tables = 0;
            for &(i, j) in &inner {
                let r = geom::len([i as f64 + 0.5 - c[0], j as f64 + 0.5 - c[1]]);
                let open = STEPS.iter().filter(|&&(di, dj)| free(&held, i + di, j + dj)).count();
                if tables >= 4 || !free(&held, i, j) || r < 2.0 || open < 2 {
                    continue;
                }
                put(lvl, &mut held, "table", i, j, 1, 1, out_of(i, j, 1, 1), None);
                if undo(lvl, &mut held) {
                    continue;
                }
                tables += 1;
                for (k, (di, dj)) in STEPS.into_iter().enumerate() {
                    if free(&held, i + di, j + dj) {
                        put(lvl, &mut held, "floor_cushion", i + di, j + dj, 1, 1, [2u8, 3, 0, 1][k], None);
                    }
                }
                for dj in -2..=2isize {
                    for di in -2..=2isize {
                        if mine(i + di, j + dj) && !rim_sq(i + di, j + dj) {
                            held[(j + dj) as usize * nx + (i + di) as usize] = true;
                        }
                    }
                }
            }
            // Sleeping places round the rest of the wall.
            along_wall(lvl, &mut held, "yurt_pallet", Some("floor_cushion"), &|_, _| true);
        }
    }
}

/// Whether a square of room `ri` not under a piece that blocks the way is cut off from `from`.
fn sealed(lvl: &Level, nx: usize, ny: usize, ri: i16, from: (isize, isize)) -> bool {
    let mut blocked = vec![false; nx * ny];
    for f in lvl.furniture.iter().filter(|f| f.blocks_move) {
        for j in f.y as usize..((f.y + f.h) as usize).min(ny) {
            for i in f.x as usize..((f.x + f.w) as usize).min(nx) {
                blocked[j * nx + i] = true;
            }
        }
    }
    let walk = |k: usize| lvl.cells[k] == ri && !blocked[k];
    let mut seen = vec![false; nx * ny];
    let mut stack: Vec<usize> = Vec::new();
    if from.0 >= 0 && from.1 >= 0 && (from.0 as usize) < nx && (from.1 as usize) < ny && walk(from.1 as usize * nx + from.0 as usize) {
        stack.push(from.1 as usize * nx + from.0 as usize);
    }
    while let Some(k) = stack.pop() {
        if seen[k] {
            continue;
        }
        seen[k] = true;
        let (i, j) = (k % nx, k / nx);
        for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
            if a < nx && b < ny && walk(b * nx + a) && !seen[b * nx + a] {
                stack.push(b * nx + a);
            }
        }
    }
    (0..nx * ny).any(|k| walk(k) && !seen[k])
}
