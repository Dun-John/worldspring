//! Rooms furnished by rule: tables and seats, booths, yurts, entries kept clear.

use super::*;

/// Furniture by room rules: wall pieces along walls, free pieces clear of them; never on the
/// stairs or a doorway (the squares either side of every door stay open), and never cutting
/// a room's doors and stairs (a spiral stair's too, by anything but a rug) off from each other. With `whole` (`Shell::whole_by`), nothing on a
/// square it leaves out, one the outer wall as drawn cuts (but a drinking room's bar and tables
/// where they fit nowhere else).
pub(super) fn furnish(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], whole: Option<&[bool]>, rng: &mut Pcg32) {
    furnish_rooms(lvl, nx, ny, st, None, whole, &[], true, false, rng);
}

/// `furnish` for every room, or `only` one (round what is already there), but those in `skip` (halls
/// furnished on their own: `hall`); without `thrones` (a building neither a castle nor a palace), no
/// throne where a room's rules have one; with `open` (a temple's, a shrine's, a monastery's), every
/// square of a room no piece blocks reached from its ways in (`room_connected`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn furnish_rooms(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], only: Option<i16>, whole: Option<&[bool]>, skip: &[i16], thrones: bool, open: bool, rng: &mut Pcg32) {
    let cut = |k: usize| whole.is_some_and(|w| !w[k]);
    let st = if lvl.has_stairs { st } else { [0, 0, 0, 0] };
    let mut taken = vec![false; nx * ny];
    for j in st[1]..st[1] + st[3] {
        for i in st[0]..st[0] + st[2] {
            taken[j * nx + i] = true;
        }
    }
    // Stairs need a free approach square at each end.
    for (i, j) in [(st[0] as isize - 1, st[1] as isize), ((st[0] + st[2]) as isize, st[1] as isize), (st[0] as isize, st[1] as isize - 1), (st[0] as isize, (st[1] + st[3]) as isize)] {
        if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
            taken[j as usize * nx + i as usize] = true;
        }
    }
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        for (i, j) in sq {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
                taken[j as usize * nx + i as usize] = true;
            }
        }
    }
    let cells = lvl.cells.clone();
    let room_at = |i: isize, j: isize| if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { cells[j as usize * nx + i as usize] };
    let mut blocked = vec![false; nx * ny];
    // Pieces already in place (spiral stairs) stay, with a clear square around them (and a way to
    // them: `entry_squares`).
    let mut furniture: Vec<Item> = lvl.furniture.clone();
    for f in &furniture {
        for j in (f.y as usize).saturating_sub(1)..((f.y + f.h) as usize + 1).min(ny) {
            for i in (f.x as usize).saturating_sub(1)..((f.x + f.w) as usize + 1).min(nx) {
                taken[j * nx + i] = true;
            }
        }
    }
    for (ri, room) in lvl.rooms.iter().enumerate() {
        let ri = ri as i16;
        if only.is_some_and(|o| o != ri) || skip.contains(&ri) {
            continue;
        }
        let mine: Vec<(usize, usize)> = (0..nx * ny).filter(|&k| cells[k] == ri).map(|k| (k % nx, k / nx)).collect();
        if mine.is_empty() {
            continue;
        }
        let touches = |i: usize, j: usize| [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| room_at(i as isize + di, j as isize + dj) != ri);
        let spiral = lvl.furniture.iter().any(|f| f.kind == "spiral_stair" && cells[f.y as usize * nx + f.x as usize] == ri);
        // A room whose required pieces don't all fit is furnished again from scratch with
        // other random choices (a bar across the only aisle leaves no room for tables).
        let (taken0, blocked0, n0) = (taken.clone(), blocked.clone(), furniture.len());
        for attempt in 0..8 {
        if attempt > 0 {
            taken.clone_from(&taken0);
            blocked.clone_from(&blocked0);
            furniture.truncate(n0);
        }
        let mut missing = false;
        for &(kind, place, fixed, per) in furnishing(room.kind).iter().filter(|r| thrones || r.0 != "throne") {
            let must = required(room.kind, kind);
            // (A council chamber too small for the council table: a long table and the chair of office.)
            if room.kind == "council chamber" && matches!(kind, "long_table" | "chair_of_office") && furniture[n0..].iter().any(|f| f.kind == "council_table") {
                continue;
            }
            let mut count = fixed as usize + if per > 0 { room.squares / per as usize } else { 0 };
            // Guest rooms over 15 ft each way take more beds (one per ~10 squares).
            if kind == "bed" && room.kind == "guest room" {
                let (x0, x1) = mine.iter().fold((usize::MAX, 0), |a, &(x, _)| (a.0.min(x), a.1.max(x)));
                let (y0, y1) = mine.iter().fold((usize::MAX, 0), |a, &(_, y)| (a.0.min(y), a.1.max(y)));
                count = if (x1 - x0 + 1).min(y1 - y0 + 1) > 3 { (room.squares / 10).max(2) } else { 1 };
            }
            let (name, cover, blocks, height) = item_info(kind);
            for n_placed in 0..count.min(12) {
                if matches!(place, Place::Seated | Place::Booth) {
                    // (Off the squares the wall cuts, but a table a room must have.)
                    let none = |_: usize| false;
                    let skip: &dyn Fn(usize) -> bool = if must { &none } else { &cut };
                    let ok = if matches!(place, Place::Seated) {
                        seat_table(&cells, nx, ny, ri, must, &mut taken, &mut blocked, &mut furniture, rng, |b| room_connected(lvl, nx, ny, st, ri, b, open), skip)
                    } else {
                        booth(&cells, nx, ny, ri, &mut taken, &mut blocked, &mut furniture, rng, |b| room_connected(lvl, nx, ny, st, ri, b, open), skip)
                    };
                    if !ok {
                        if must && n_placed < fixed as usize {
                            missing = true;
                        }
                        break;
                    }
                    continue;
                }
                let (a, b) = match place {
                    Place::Wall(l, d) | Place::Center(l, d) => (l as usize, d as usize),
                    Place::Seated | Place::Booth => unreachable!(),
                };
                // Sizes to try: the preferred one, then (for required pieces) smaller.
                let mut sizes = vec![(a, b)];
                // (Pieces drawn whole at their own sizes: a council table with its seats 6 long in a
                // big room, 3 at the least; a desk, a board, a stall not shrunk.)
                let whole = matches!(kind, "council_table" | "counting_desk" | "job_board" | "stable_stall");
                if kind == "council_table" {
                    sizes = if mine.len() >= 40 { vec![(6, 2), (5, 2), (4, 2), (3, 2)] } else { vec![(4, 2), (3, 2)] };
                }
                // (A bedroom's bed a single, 1 by 2, at the least.)
                let bed = kind == "bed" && room.kind != "best chamber";
                if must && bed {
                    if (a, b) != (1, 2) {
                        sizes.push((1, 2));
                    }
                } else if must && !whole {
                    for s in [(2, 1), (1, 1)] {
                        if !sizes.contains(&s) && s.0 * s.1 < a * b {
                            sizes.push(s);
                        }
                    }
                    // Small rooms: compact pieces first (a short bar, stools-and-table).
                    if mine.len() < 16 {
                        let small = if kind == "bar" { 2 } else { 1 };
                        sizes.retain(|s| s.0 * s.1 <= small);
                    }
                }
                let strict = [true, false];
                let mut placed = false;
                'search: for &(pa, pb) in &sizes {
                    for &strict in strict.iter().take(if must { 2 } else { 1 }) {
                        let mut cands: Vec<(usize, usize, usize, usize)> = Vec::new();
                        for &(x, y) in &mine {
                            let orients: &[(usize, usize)] = if pa == pb { &[(pa, pb)] } else { &[(pa, pb), (pb, pa)] };
                            for &(w, h) in orients {
                                if x + w > nx || y + h > ny {
                                    continue;
                                }
                                if !(y..y + h).all(|j| (x..x + w).all(|i| cells[j * nx + i] == ri && !taken[j * nx + i])) {
                                    continue;
                                }
                                let edge_n = (y..y + h).flat_map(|j| (x..x + w).map(move |i| (i, j))).filter(|&(i, j)| touches(i, j)).count();
                                let ok = !strict
                                    || match place {
                                        Place::Wall(..) => edge_n >= w.max(h).min(w * h),
                                        Place::Center(..) => edge_n == 0 || mine.len() < 12,
                                        Place::Seated | Place::Booth => true,
                                    };
                                if ok {
                                    cands.push((x, y, w, h));
                                }
                            }
                        }
                        // Random order; the first that keeps the room passable wins (off the squares
                        // the wall cuts, but a piece a room must have).
                        for k in (1..cands.len()).rev() {
                            cands.swap(k, rng.below(k as u32 + 1) as usize);
                        }
                        for (x, y, w, h) in cands {
                            if !must && (y..y + h).any(|j| (x..x + w).any(|i| cut(j * nx + i))) {
                                continue;
                            }
                            // (In a room with a spiral stair, nothing but a rug in the way to it, not
                            // even a bed one could step over.)
                            if blocks || (spiral && kind != "rug") {
                                for j in y..y + h {
                                    for i in x..x + w {
                                        blocked[j * nx + i] = true;
                                    }
                                }
                                if !room_connected(lvl, nx, ny, st, ri, &blocked, open) {
                                    for j in y..y + h {
                                        for i in x..x + w {
                                            blocked[j * nx + i] = false;
                                        }
                                    }
                                    continue;
                                }
                            }
                            for j in y..y + h {
                                for i in x..x + w {
                                    taken[j * nx + i] = true;
                                }
                            }
                            // Free pieces keep a one-square aisle around them in bigger rooms.
                            if matches!(place, Place::Center(..)) && mine.len() >= 12 && strict {
                                for j in y.saturating_sub(1)..(y + h + 1).min(ny) {
                                    for i in x.saturating_sub(1)..(x + w + 1).min(nx) {
                                        if cells[j * nx + i] == ri {
                                            taken[j * nx + i] = true;
                                        }
                                    }
                                }
                            }
                            furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover, blocks_move: blocks, height_ft: height, hazard: None, sprite: 0, back: None, variant: None });
                            placed = true;
                            break 'search;
                        }
                    }
                }
                if !placed {
                    if must && n_placed < fixed as usize {
                        missing = true;
                    }
                    break;
                }
            }
        }
        if !missing {
            break;
        }
        }
    }
    lvl.furniture = furniture;
}

/// A yurt's one round room furnished (`program` "yurt"): its front door through the rim on the
/// side `south` (grid axes) points to as near as open ground allows, at the middle of that side;
/// the hearth in the middle (on the middle square, or the middle two or four: a ring of stones
/// round the fire, cushions round it but on the door's side); painted chests at the back across
/// from the door; pallets with bedrolls along the wall two squares long, a floor cushion where one
/// won't go; the squares beside the door left clear. Each piece's `back` the wall it stands against
/// (the hearth's: the door's side).
pub(super) fn furnish_yurt(lvl: &mut Level, nx: usize, ny: usize, south: P, ext_free: &dyn Fn(isize, isize) -> bool) {
    let Some(ri) = lvl.rooms.iter().position(|r| r.kind == "yurt") else { return };
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
    const STEPS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    // The door: the rim's edge facing most nearly `south` (the ring's door side), onto open ground.
    let mut best: Option<(f64, isize, isize, usize)> = None;
    for &(i, j) in &sq {
        for (side, (di, dj)) in STEPS.into_iter().enumerate() {
            if mine(i + di, j + dj) || !ext_free(i + di, j + dj) {
                continue;
            }
            // (Along the side's normal, then toward the middle of the ring.)
            let m = [i as f64 + 0.5 + 0.5 * di as f64 - c[0], j as f64 + 0.5 + 0.5 * dj as f64 - c[1]];
            let score = (di as f64 * south[0] + dj as f64 * south[1]) * 4.0 + (m[0] * south[0] + m[1] * south[1]) / geom::len(m).max(1e-9);
            if best.is_none_or(|b| score > b.0 + 1e-9) {
                best = Some((score, i, j, side));
            }
        }
    }
    // (Kept clear of rugs and chests: the door's square and the one just inside it.)
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
    let mut held = vec![false; nx * ny];
    // (Squares already taken, and the doorways.)
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
    let door_angle = libm::atan2(to_door[1], to_door[0]);
    let side_of = |d: P| if d[0].abs() > d[1].abs() { if d[0] > 0.0 { 1u8 } else { 3 } } else if d[1] > 0.0 { 2 } else { 0 };
    let put = |lvl: &mut Level, held: &mut Vec<bool>, kind: &'static str, x: isize, y: isize, w: isize, h: isize, back: u8| {
        if let Some(mut it) = building_item(kind, x as usize, y as usize, w as usize, h as usize) {
            it.back = Some(back);
            lvl.furniture.push(it);
            for j in y..y + h {
                for i in x..x + w {
                    held[j as usize * nx + i as usize] = true;
                }
            }
        }
    };
    let free = |held: &Vec<bool>, i: isize, j: isize| mine(i, j) && !held[j as usize * nx + i as usize];
    // The hearth.
    let (hw, hh) = (if (x1 - x0) % 2 == 0 { 2 } else { 1 }, if (y1 - y0) % 2 == 0 { 2 } else { 1 });
    let (hx, hy) = ((c[0] - 0.5 * hw as f64) as isize, (c[1] - 0.5 * hh as f64) as isize);
    let fits = (hy..hy + hh).all(|j| (hx..hx + hw).all(|i| free(&held, i, j)));
    let hearth = if fits {
        Some((hx, hy, hw, hh))
    } else {
        sq.iter().copied().filter(|&(i, j)| free(&held, i, j)).min_by(|a, b| {
            let d = |s: &(isize, isize)| (s.0 as f64 + 0.5 - c[0]).abs() + (s.1 as f64 + 0.5 - c[1]).abs();
            d(a).total_cmp(&d(b))
        }).map(|(i, j)| (i, j, 1, 1))
    };
    let door_side = side_of(to_door);
    if let Some((x, y, w, h)) = hearth {
        put(lvl, &mut held, "yurt_hearth", x, y, w, h, door_side);
        // (A hearth on one square: cushions on the squares round it, not the door's side.)
        if w == 1 && h == 1 {
            for (k, (di, dj)) in STEPS.into_iter().enumerate() {
                let (i, j) = (x + di, y + dj);
                let rim = STEPS.iter().any(|&(a, b)| !mine(i + a, j + b));
                if k as u8 != door_side && free(&held, i, j) && !rim {
                    put(lvl, &mut held, "floor_cushion", i, j, 1, 1, side_of([-di as f64, -dj as f64]));
                }
            }
        }
    }
    // Round the wall: each square's bearing from the door (0 at it, pi across from it).
    let from_door = |i: isize, j: isize| {
        let a = libm::atan2(j as f64 + 0.5 - c[1], i as f64 + 0.5 - c[0]) - door_angle;
        let a = libm::fmod(a + 3.0 * core::f64::consts::PI, 2.0 * core::f64::consts::PI) - core::f64::consts::PI;
        a.abs()
    };
    let mut rim: Vec<(isize, isize)> = sq.iter().copied().filter(|&(i, j)| STEPS.iter().any(|&(a, b)| !mine(i + a, j + b)) && from_door(i, j) > 0.9).collect();
    rim.sort_by(|a, b| from_door(b.0, b.1).total_cmp(&from_door(a.0, a.1)).then(a.cmp(b)));
    let out = |i: isize, j: isize, w: isize, h: isize| side_of([i as f64 + 0.5 * w as f64 - c[0], j as f64 + 0.5 * h as f64 - c[1]]);
    let mut chests = 0;
    for &(i, j) in &rim {
        if !free(&held, i, j) {
            continue;
        }
        // (At the back: up to two chests.)
        if from_door(i, j) > 2.3 && chests < 2 {
            put(lvl, &mut held, "painted_chest", i, j, 1, 1, out(i, j, 1, 1));
            chests += 1;
            continue;
        }
        // (A pallet along the wall over this square and the next round it.)
        let next = [(1, 0), (0, 1), (-1, 0), (0, -1)].into_iter().map(|(a, b)| (i + a, j + b)).find(|&(a, b)| free(&held, a, b) && rim.contains(&(a, b)));
        match next {
            Some((a, b)) => {
                let (x, y, w, h) = (i.min(a), j.min(b), (i - a).abs() + 1, (j - b).abs() + 1);
                put(lvl, &mut held, "yurt_pallet", x, y, w, h, out(x, y, w, h));
            }
            None => put(lvl, &mut held, "floor_cushion", i, j, 1, 1, out(i, j, 1, 1)),
        }
    }
}

pub(super) fn push_item(furniture: &mut Vec<Item>, kind: &'static str, x: usize, y: usize) {
    let (name, cover, blocks, height) = item_info(kind);
    furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: 1, h: 1, cover, blocks_move: blocks, height_ft: height, hazard: None, sprite: 0, back: None, variant: None });
}

/// A one-square round table with chairs on its open sides: four chairs and a clear ring around
/// it where the room allows, else fewer (a required table takes any free square).
#[allow(clippy::too_many_arguments)]
fn seat_table(
    cells: &[i16],
    nx: usize,
    ny: usize,
    ri: i16,
    must: bool,
    taken: &mut [bool],
    blocked: &mut [bool],
    furniture: &mut Vec<Item>,
    rng: &mut Pcg32,
    passable: impl Fn(&[bool]) -> bool,
    skip: &dyn Fn(usize) -> bool,
) -> bool {
    let free = |taken: &[bool], i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && cells[j as usize * nx + i as usize] == ri && !taken[j as usize * nx + i as usize];
    let needs: &[usize] = if must { &[4, 3, 2, 1, 0] } else { &[4, 3] };
    for &need in needs {
        let mut cands: Vec<(usize, usize)> = (0..nx * ny)
            .filter(|&k| cells[k] == ri && !taken[k])
            .map(|k| (k % nx, k / nx))
            .filter(|&(i, j)| [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)].iter().filter(|&&(di, dj)| free(taken, i as isize + di, j as isize + dj)).count() >= need)
            .collect();
        for k in (1..cands.len()).rev() {
            cands.swap(k, rng.below(k as u32 + 1) as usize);
        }
        for (i, j) in cands {
            if skip(j * nx + i) {
                continue;
            }
            blocked[j * nx + i] = true;
            if !passable(blocked) {
                blocked[j * nx + i] = false;
                continue;
            }
            let chairs: Vec<(usize, usize)> = [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .map(|&(di, dj)| (i as isize + di, j as isize + dj))
                .filter(|&(a, b)| free(taken, a, b) && !skip(b as usize * nx + a as usize))
                .map(|(a, b)| (a as usize, b as usize))
                .collect();
            push_item(furniture, "table", i, j);
            // A full table keeps its 3 × 3 to itself (the next table's chairs don't crowd it).
            let ring = if need == 4 { 1 } else { 0 };
            for b in j.saturating_sub(ring)..(j + ring + 1).min(ny) {
                for a in i.saturating_sub(ring)..(i + ring + 1).min(nx) {
                    if cells[b * nx + a] == ri {
                        taken[b * nx + a] = true;
                    }
                }
            }
            for (a, b) in chairs {
                push_item(furniture, "chair", a, b);
                taken[b * nx + a] = true;
            }
            taken[j * nx + i] = true;
            return true;
        }
    }
    false
}

/// A booth in a room corner: the table on the inner square, bench seats on the three squares
/// along the two walls.
#[allow(clippy::too_many_arguments)]
fn booth(cells: &[i16], nx: usize, ny: usize, ri: i16, taken: &mut [bool], blocked: &mut [bool], furniture: &mut Vec<Item>, rng: &mut Pcg32, passable: impl Fn(&[bool]) -> bool, skip: &dyn Fn(usize) -> bool) -> bool {
    let out = |i: isize, j: isize| i < 0 || j < 0 || i >= nx as isize || j >= ny as isize || cells[j as usize * nx + i as usize] != ri;
    let mut cands: Vec<(usize, usize, usize, usize)> = Vec::new();
    for j in 0..ny.saturating_sub(1) {
        for i in 0..nx.saturating_sub(1) {
            let sq = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)];
            if !sq.iter().all(|&(a, b)| cells[b * nx + a] == ri && !taken[b * nx + a]) {
                continue;
            }
            let (x, y) = (i as isize, j as isize);
            let left = out(x - 1, y) && out(x - 1, y + 1);
            let right = out(x + 2, y) && out(x + 2, y + 1);
            let top = out(x, y - 1) && out(x + 1, y - 1);
            let bottom = out(x, y + 2) && out(x + 1, y + 2);
            // The table sits on the square away from both walls.
            let tx = if left { i + 1 } else if right { i } else { continue };
            let ty = if top { j + 1 } else if bottom { j } else { continue };
            cands.push((i, j, tx, ty));
        }
    }
    for k in (1..cands.len()).rev() {
        cands.swap(k, rng.below(k as u32 + 1) as usize);
    }
    for (i, j, tx, ty) in cands {
        if [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)].iter().any(|&(a, b)| skip(b * nx + a)) {
            continue;
        }
        blocked[ty * nx + tx] = true;
        if !passable(blocked) {
            blocked[ty * nx + tx] = false;
            continue;
        }
        for (a, b) in [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)] {
            push_item(furniture, if (a, b) == (tx, ty) { "booth_table" } else { "booth_seat" }, a, b);
            taken[b * nx + a] = true;
        }
        return true;
    }
    false
}

/// The squares inside a room next to its doorways, its stairs and a spiral stair's (its entry
/// points).
pub(super) fn entry_squares(lvl: &Level, nx: usize, ny: usize, st: [usize; 4], ri: i16) -> Vec<usize> {
    let mut out = Vec::new();
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        for (i, j) in sq {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && lvl.cells[j as usize * nx + i as usize] == ri {
                out.push(j as usize * nx + i as usize);
            }
        }
    }
    for j in st[1]..st[1] + st[3] {
        for i in st[0]..st[0] + st[2] {
            if lvl.cells[j * nx + i] == ri {
                out.push(j * nx + i);
            }
        }
    }
    // (A spiral stair in it: the way on to the floors above and below.)
    for f in lvl.furniture.iter().filter(|f| f.kind == "spiral_stair") {
        let k = f.y as usize * nx + f.x as usize;
        if lvl.cells[k] == ri {
            out.push(k);
        }
    }
    out
}

/// Whether all of a room's entry squares connect over squares not blocked by furniture (with `open`,
/// every square of it not blocked too).
fn room_connected(lvl: &Level, nx: usize, ny: usize, st: [usize; 4], ri: i16, blocked: &[bool], open: bool) -> bool {
    let entries = entry_squares(lvl, nx, ny, st, ri);
    let Some(&start) = entries.first() else { return true };
    let mut seen = vec![false; nx * ny];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(k) = stack.pop() {
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (a, b) = (i + di, j + dj);
            if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize {
                continue;
            }
            let q = b as usize * nx + a as usize;
            if !seen[q] && lvl.cells[q] == ri && !blocked[q] {
                seen[q] = true;
                stack.push(q);
            }
        }
    }
    entries.iter().all(|&e| seen[e]) && (!open || (0..nx * ny).all(|k| seen[k] || lvl.cells[k] != ri || blocked[k]))
}
