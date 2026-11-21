//! Halls and their frames, main doors, ways in, public stairs, tower rooms and stair placement.

use super::*;

/// What a building's halls are (as generated, and furnished in the designer): whether it is one of
/// a castle's buildings, whether thrones go in it (a castle's or a palace's only) and how its great
/// hall is furnished as a whole (`hall_style`).
pub(crate) fn hall_kind(world: &World, t0: &T0, settlement: usize, b: &town::Building) -> (bool, bool, Vec<(&'static str, hall::Style)>) {
    let key = func_key(b);
    let castle = key == Some("castle") || b.keep.is_some() || b.ward == town::catalog::Ward::Castle;
    (castle, castle || key == Some("palace"), hall_style(world, t0, settlement, b, arch_of(b), key, castle))
}

/// How a building's halls are furnished as wholes (`hall`), with the kinds of room they are: a
/// barracks' mess hall (a castle's flying its house); a castle keep's great hall, its throne dais of
/// stone and gilt in a capital (a pair of thrones) and rich cities, of timber elsewhere, now and then
/// with a lesser chair; a town's or a guild's hall, a harbor master's, an auction house's (a stone
/// dais in a city); any other hall's tables only, as a mess hall's (a guard post's, an orphanage's);
/// a noble estate's; the capital's palace's throne room (a pair of thrones, stone and gilt),
/// banqueting hall and ballroom (and a great hall, where it is laid out as an older palace); another
/// palace's great hall, its throne dais as a castle's. A palace's banners fly the town's house, as
/// its castle's. `castle`: one of a castle's buildings (its banners fly the castle's house).
fn hall_style(world: &World, t0: &T0, settlement: usize, b: &town::Building, arch: Arch, key: Option<&str>, castle: bool) -> Vec<(&'static str, hall::Style)> {
    use crate::t0::settle::Tier;
    let roll = |salt: i64| hash3(world.stream("interior.hall"), town::rng_key(t0, settlement), salt, 0);
    let house = |own: bool| (if own { hash3(world.stream("interior.house"), town::rng_key(t0, settlement), b.id as i64, 1) } else { roll(0x40) } % HOUSES as u64) as u8;
    let set = crate::town::settlement(t0, settlement);
    if key == Some("barracks") {
        return vec![("mess hall", hall::Style::Mess { house: castle.then(|| house(false)) })];
    }
    let capital = set.is_some_and(|s| s.capital);
    if b.keep.is_some() || key == Some("castle") {
        let rich = capital || set.is_some_and(|s| s.tier == Tier::Metropolis || (s.tier == Tier::City && roll(0x41) % 2 == 0));
        let seating = if capital { 1 } else if roll(0x42) % 3 == 0 { 2 } else { 0 };
        return vec![("great hall", hall::Style::Castle { dais: seating + if rich { 0 } else { 3 }, house: house(false), banquet: false })];
    }
    match arch {
        // (A low dais and a chair of office in a town hall, a guild's, a harbor master's and an
        // auction house; the tables only in every other hall: a guard post's, an orphanage's...)
        Arch::Hall if matches!(key, Some("town_hall" | "craft_guild" | "merchant_guild" | "adventurers_guild" | "thieves_guild" | "mages_guild" | "harbor_master" | "auction_house")) => {
            vec![("great hall", hall::Style::Civic { stone: set.is_some_and(|s| s.tier >= Tier::City), arms: arms_look(key) })]
        }
        Arch::Hall => vec![("great hall", hall::Style::Mess { house: None })],
        Arch::Mansion if key == Some("palace") && capital => {
            let house = house(false);
            vec![
                ("throne room", hall::Style::Throne { dais: 1, house }),
                ("banqueting hall", hall::Style::Banquet { house }),
                ("ballroom", hall::Style::Ballroom),
                ("great hall", hall::Style::Castle { dais: 1, house, banquet: true }),
                ("audience chamber", hall::Style::Presence { house }),
                ("long gallery", hall::Style::Gallery),
                ("royal chapel", hall::Style::Chapel),
                ("chapel", hall::Style::Chapel),
            ]
        }
        Arch::Mansion if key == Some("palace") => {
            let rich = set.is_some_and(|s| s.tier == Tier::Metropolis || (s.tier == Tier::City && roll(0x43) % 3 != 0));
            let seating = if roll(0x44) % 3 == 0 { 2 } else { 0 };
            let house = house(false);
            vec![
                ("great hall", hall::Style::Castle { dais: seating + if rich { 0 } else { 3 }, house, banquet: true }),
                ("audience chamber", hall::Style::Presence { house }),
                ("long gallery", hall::Style::Gallery),
                ("chapel", hall::Style::Chapel),
            ]
        }
        Arch::Mansion => vec![("great hall", hall::Style::Estate { house: house(!castle) })],
        // (A temple's god's pieces by the god in its name; a big one's screen at its crossing.)
        Arch::Temple if key == Some("temple") && town::temple::shaped(b) => {
            let god = town::temple::god_look(b);
            match town::temple::cross_of(b) {
                Some(x) => vec![
                    ("nave", hall::Style::Nave { god, cross: Some((x.nave as u8, (x.nave + x.transept - 1) as u8)) }),
                    ("sanctuary", hall::Style::Sanctuary { god }),
                    ("side chapel", hall::Style::SideChapel { god }),
                ],
                None => vec![("nave", hall::Style::Nave { god, cross: None })],
            }
        }
        Arch::Temple if key == Some("shrine") => vec![("shrine", hall::Style::Shrine { god: town::temple::god_look(b) })],
        Arch::Temple if key == Some("monastery") => vec![("nave", hall::Style::Nave { god: town::temple::god_look(b), cross: None }), ("refectory", hall::Style::Refectory)],
        _ => Vec::new(),
    }
}

/// A hall's frame (`hall::Frame`) where its zones gave it none: the largest box of its squares, along
/// the axis from the wall its main door (the one nearest the way in: the front door, else the stairs)
/// is on to the opposite one, or, where that wall is the long one, along its length away from the
/// door's end (then the other way too, where the far end holds no dais: whether it may be turned).
/// None for a room with no box 3 by 2.
pub(super) fn hall_frame(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4]) -> Option<(hall::Frame, bool)> {
    let mine = |i: usize, j: usize| lvl.cells[j * nx + i] == ri;
    let sq: Vec<usize> = (0..nx * ny).filter(|&k| lvl.cells[k] == ri).collect();
    if sq.is_empty() {
        return None;
    }
    let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
    let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
    let main = main_door(lvl, nx, ny, ri, st);
    // The largest box (a summed table over the room's bounding box), then the one holding the door.
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut sum = vec![0usize; (bw + 1) * (bh + 1)];
    for j in 0..bh {
        for i in 0..bw {
            sum[(j + 1) * (bw + 1) + i + 1] = sum[j * (bw + 1) + i + 1] + sum[(j + 1) * (bw + 1) + i] - sum[j * (bw + 1) + i] + mine(x0 + i, y0 + j) as usize;
        }
    }
    let full = |a0: usize, a1: usize, b0: usize, b1: usize| sum[b1 * (bw + 1) + a1] + sum[b0 * (bw + 1) + a0] - sum[b0 * (bw + 1) + a1] - sum[b1 * (bw + 1) + a0] == (a1 - a0) * (b1 - b0);
    let holds = |b: [usize; 4], p: (isize, isize)| p.0 >= (x0 + b[0]) as isize && p.0 < (x0 + b[1]) as isize && p.1 >= (y0 + b[2]) as isize && p.1 < (y0 + b[3]) as isize;
    let mut best: Option<((usize, bool), [usize; 4])> = None;
    for b0 in 0..bh {
        for b1 in b0 + 2..=bh {
            for a0 in 0..bw {
                for a1 in a0 + 2..=bw {
                    if (a1 - a0).max(b1 - b0) < 3 || !full(a0, a1, b0, b1) {
                        continue;
                    }
                    let score = ((a1 - a0) * (b1 - b0), main.is_some_and(|m| holds([a0, a1, b0, b1], m.1)));
                    if best.as_ref().is_none_or(|b| score > b.0) {
                        best = Some((score, [a0, a1, b0, b1]));
                    }
                }
            }
        }
    }
    let (_, [a0, a1, b0, b1]) = best?;
    let (bx0, bx1, by0, by1) = ((x0 + a0) as isize, (x0 + a1) as isize - 1, (y0 + b0) as isize, (y0 + b1) as isize - 1);
    let (w, h) = ((bx1 - bx0 + 1) as usize, (by1 - by0 + 1) as usize);
    // The door's side of the box: the one it lies nearest (0 top, 1 right, 2 bottom, 3 left).
    let c = main.map_or([(bx0 + bx1) as f64 * 0.5, (by0 + by1) as f64 * 0.5], |m| [m.1.0 as f64, m.1.1 as f64]);
    let gaps = [c[1] - by0 as f64, bx1 as f64 - c[0], by1 as f64 - c[1], c[0] - bx0 as f64];
    let door_side = (0..4).min_by(|&a, &b| gaps[a].total_cmp(&gaps[b]).then(a.cmp(&b))).unwrap_or(3);
    let (across, deep) = if door_side % 2 == 0 { (w, h) } else { (h, w) };
    // (Entered on a long wall: along the length, from the end nearer the door.)
    let from = if main.is_none() || across <= deep {
        door_side
    } else if door_side % 2 == 0 {
        if c[0] - bx0 as f64 <= bx1 as f64 - c[0] { 3 } else { 1 }
    } else if c[1] - by0 as f64 <= by1 as f64 - c[1] {
        0
    } else {
        2
    };
    let f = match from {
        0 => hall::Frame { o: (bx0, by0), ds: (0, 1), dt: (1, 0), len: h, wid: w },
        2 => hall::Frame { o: (bx0, by1), ds: (0, -1), dt: (1, 0), len: h, wid: w },
        3 => hall::Frame { o: (bx0, by0), ds: (1, 0), dt: (0, 1), len: w, wid: h },
        _ => hall::Frame { o: (bx1, by0), ds: (-1, 0), dt: (0, 1), len: w, wid: h },
    };
    Some((f, from != door_side))
}

/// A room's main door (`hall_frame`): rooms by doors from the front door's room (or the stairs'),
/// room `ri`'s door to the nearest (the front door itself first); its square in the room (grid) and
/// the door (its index in `lvl.doors`).
pub fn main_door(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4]) -> Option<(usize, (isize, isize), usize)> {
    let mine = |i: usize, j: usize| lvl.cells[j * nx + i] == ri;
    let n = lvl.rooms.len();
    let start = lvl.doors.iter().find(|d| d.kind == "front").map(|d| d.rooms[0]).or_else(|| lvl.has_stairs.then(|| lvl.cells[st[1] * nx + st[0]])).unwrap_or(ri);
    let mut hops = vec![usize::MAX; n];
    if start >= 0 {
        hops[start as usize] = 0;
        let mut q = std::collections::VecDeque::from([start]);
        while let Some(a) = q.pop_front() {
            for d in lvl.doors.iter().filter(|d| d.rooms.contains(&a)) {
                let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
                if o >= 0 && hops[o as usize] == usize::MAX {
                    hops[o as usize] = hops[a as usize] + 1;
                    q.push_back(o);
                }
            }
        }
    }
    let door_sq = |d: &Door| {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let pair = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        pair.into_iter().find(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && mine(i as usize, j as usize))
    };
    lvl.doors
        .iter()
        .enumerate()
        .filter(|(_, d)| d.rooms.contains(&ri))
        .filter_map(|(i, d)| {
            let o = if d.rooms[0] == ri { d.rooms[1] } else { d.rooms[0] };
            let rank = if d.kind == "front" { 0 } else if o >= 0 { hops[o as usize].saturating_add(1) } else { usize::MAX };
            door_sq(d).map(|p| (rank, p, i))
        })
        .min_by_key(|x| (x.0, x.1.1, x.1.0))
}

/// A chapel's frame (`hall::Frame`): the largest box of its squares (as `hall_frame` finds it), along
/// its length, its far end (the altar's) the one away from its way in (`way_in`), else the one more
/// to the east (`u`, `v`: the grid's axes in the world), either where it runs north and south, run on
/// to the far wall where the room does past the box on the axis (on `whole` squares), else short of
/// a far row the outer wall cuts into there; it may be turned (the altar at the other end).
#[allow(clippy::too_many_arguments)]
pub(super) fn chapel_frame(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4], u: P, v: P, whole: Option<&[bool]>) -> Option<(hall::Frame, bool)> {
    let (f, _) = hall_frame(lvl, nx, ny, ri, st)?;
    let (a, c) = (f.grid(0, 0), f.grid(f.len as isize - 1, f.wid as isize - 1));
    let (bx0, bx1, by0, by1) = (a.0.min(c.0), a.0.max(c.0), a.1.min(c.1), a.1.max(c.1));
    let (w, h) = ((bx1 - bx0 + 1) as usize, (by1 - by0 + 1) as usize);
    let east = |d: (isize, isize)| d.0 as f64 * u[0] + d.1 as f64 * v[0];
    let f = match (w >= h, w >= h && east((1, 0)) >= 0.0 || w < h && east((0, 1)) >= 0.0) {
        (true, true) => hall::Frame { o: (bx0, by0), ds: (1, 0), dt: (0, 1), len: w, wid: h },
        (true, false) => hall::Frame { o: (bx1, by0), ds: (-1, 0), dt: (0, 1), len: w, wid: h },
        (false, true) => hall::Frame { o: (bx0, by0), ds: (0, 1), dt: (1, 0), len: h, wid: w },
        (false, false) => hall::Frame { o: (bx0, by1), ds: (0, -1), dt: (1, 0), len: h, wid: w },
    };
    // (Its way in nearer the far end: the other way round.)
    let mut f = f;
    if way_in(lvl, nx, ny, ri, st, f).is_some_and(|m| m > 0.5 * (f.len as f64 - 1.0) + 0.01) {
        f = hall::Frame { o: f.grid(f.len as isize - 1, 0), ds: (-f.ds.0, -f.ds.1), ..f };
    }
    // (On to its far wall where the room runs on past the box on the axis, a row or two, on squares
    // the outer wall leaves whole (`whole`), so the altar stands against it.)
    let w = f.wid as isize;
    let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
    let ours = |p: (isize, isize)| p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < nx && (p.1 as usize) < ny && {
        let k = p.1 as usize * nx + p.0 as usize;
        lvl.cells[k] == ri && whole.is_none_or(|w| w[k])
    };
    let mut ran = false;
    for _ in 0..2 {
        if !(cl..=cr).all(|t| ours(f.grid(f.len as isize, t))) {
            break;
        }
        (f.len, ran) = (f.len + 1, true);
    }
    // (Else short of its far row where the outer wall cuts into it on the axis: against that.)
    let mine = |p: (isize, isize)| p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < nx && (p.1 as usize) < ny && lvl.cells[p.1 as usize * nx + p.0 as usize] == ri;
    while !ran && f.len > 3 && (cl..=cr).all(|t| mine(f.grid(f.len as isize - 1, t)) && !ours(f.grid(f.len as isize - 1, t))) {
        f.len -= 1;
    }
    Some((f, true))
}

/// Where along frame `f` room `ri` is entered (its `s`: a door in a side wall at its square's, one
/// in an end wall half a square beyond it): at the end whose wall a door opens in, where only one
/// end's does; else at its main door (`main_door`); else (none, or that midway along it) at its
/// doors' on the mean. None for a room with no door.
pub fn way_in(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4], f: hall::Frame) -> Option<f64> {
    let at = |d: &Door| {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let [p, q] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        0.5 * (f.local(p.0, p.1).0 + f.local(q.0, q.1).0) as f64
    };
    let (l, mid) = (f.len as f64, 0.5 * (f.len as f64 - 1.0));
    let ends = |e: f64| lvl.doors.iter().any(|d| d.rooms.contains(&ri) && (at(d) - e).abs() < 0.01);
    match (ends(-0.5), ends(l - 0.5)) {
        (true, false) => return Some(-0.5),
        (false, true) => return Some(l - 0.5),
        _ => {}
    }
    if let Some(s) = main_door(lvl, nx, ny, ri, st).map(|m| at(&lvl.doors[m.2])).filter(|s| (s - mid).abs() > 0.01) {
        return Some(s);
    }
    let s: Vec<f64> = lvl.doors.iter().filter(|d| d.rooms.contains(&ri)).map(at).collect();
    (!s.is_empty()).then(|| s.iter().sum::<f64>() / s.len() as f64)
}

/// The frame room `room` of level `z` of building `b` was furnished in as a hall when generated
/// (where it is laid out by zones and its frame's middle square is still that room's in `lvl`: its
/// zones' frame, not one found in its squares), with the squares its furniture keeps to as then
/// (`Shell::whole_by`): the designer's Furnish gives what generation gives.
#[allow(clippy::too_many_arguments)]
pub(crate) fn zone_hall_frame(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, z: i8, lvl: &Level, room: i16, kind: &str) -> Option<(hall::Frame, Vec<bool>)> {
    hub_kind(t0, l, b)?;
    let sh = Shell::of(l, b);
    let (nx, ny) = (sh.nx, sh.ny);
    if lvl.cells.len() != nx * ny {
        return None;
    }
    let mut plans = Vec::new();
    let it = build_noting(world, t0, l, settlement, b, &mut Vec::new(), &mut plans);
    let ours = |f: &hall::Frame| {
        let (i, j) = f.grid(f.len as isize / 2, f.wid as isize / 2);
        i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && lvl.cells[j as usize * nx + i as usize] == room
    };
    let p = plans.iter().find(|p| it.levels[p.level].z == z && it.levels[p.level].rooms.get(p.room as usize).is_some_and(|r| r.kind == kind) && ours(&p.frame))?;
    Some((p.frame, sh.whole_by(-0.1)))
}

/// Stairs for an inn or tavern: a 2 × 1 block (else 1 × 1) in the public room (against one of its
/// walls, if `against`), on `whole` squares, off every doorway, as far from the front door as the
/// room allows.
pub(super) fn pub_stairs(lvl: &Level, nx: usize, ny: usize, whole: &[bool], against: bool, by_doors: bool) -> Option<[usize; 4]> {
    let front = lvl.doors.iter().find(|d| d.kind == "front")?;
    let public = front.rooms[0];
    let fd = [(front.a[0] + front.b[0]) * 0.5, (front.a[1] + front.b[1]) * 0.5];
    // Squares next to any door stay clear.
    let mut near_door = vec![false; nx * ny];
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        for (i, j) in sq {
            for (di, dj) in [(0isize, 0isize), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i + di, j + dj);
                // (`by_doors`: only the door's own squares.)
                if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny && (!by_doors || (di, dj) == (0, 0)) {
                    near_door[b as usize * nx + a as usize] = true;
                }
            }
        }
    }
    let ok = |i: usize, j: usize| i < nx && j < ny && lvl.cells[j * nx + i] == public && !near_door[j * nx + i] && whole[j * nx + i];
    let wall = |i: usize, j: usize| [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| {
        let (a, b) = (i as isize + di, j as isize + dj);
        a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || lvl.cells[b as usize * nx + a as usize] != public
    });
    let mut best: Option<(f32, [usize; 4])> = None;
    let sizes: &[(usize, usize)] = if by_doors { &[(1, 1)] } else { &[(2, 1), (1, 2), (1, 1)] };
    for &(w, h) in sizes {
        for j in 0..ny {
            for i in 0..nx {
                let sq: Vec<(usize, usize)> = (j..j + h).flat_map(|b| (i..i + w).map(move |a| (a, b))).collect();
                if !sq.iter().all(|&(a, b)| ok(a, b)) || (against && !sq.iter().all(|&(a, b)| wall(a, b))) {
                    continue;
                }
                let c = [i as f32 + w as f32 * 0.5, j as f32 + h as f32 * 0.5];
                let d = (c[0] - fd[0]).abs() + (c[1] - fd[1]).abs();
                if best.is_none_or(|b| d > b.0) {
                    best = Some((d, [i, j, w, h]));
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|b| b.1)
}

/// The towers' rooms (`cells` ids from `first`) more than one room from a way about the floor (a
/// passage, a hall, a landing): each given to the room beside it nearest one (v63 castles: a keep's
/// corner tower off a chamber that is itself a room from the ways). A keep's own towers (the
/// first `kept` of them) keep their rooms whole: the room beside one, nearest a way, is given to
/// the room beside it nearest one instead, where it has such a room that is no tower's.
fn tower_rooms_near_ways(cells: &mut [i16], rooms: &[Room], nx: usize, ny: usize, first: usize, kept: usize) {
    let n = rooms.len();
    if first >= n || !rooms.iter().any(|r| is_way_through(r.kind)) {
        return;
    }
    loop {
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for k in 0..nx * ny {
            let a = cells[k];
            if a < 0 || !walkable(rooms[a as usize].kind) {
                continue;
            }
            for q in [(k % nx + 1 < nx).then_some(k + 1), (k / nx + 1 < ny).then_some(k + nx)].into_iter().flatten() {
                let b = cells[q];
                if b >= 0 && b != a && walkable(rooms[b as usize].kind) && !adj[a as usize].contains(&(b as usize)) {
                    adj[a as usize].push(b as usize);
                    adj[b as usize].push(a as usize);
                }
            }
        }
        let mut hops = vec![usize::MAX; n];
        let mut q = std::collections::VecDeque::new();
        for r in (0..n).filter(|&r| is_way_through(rooms[r].kind)) {
            hops[r] = 0;
            q.push_back(r);
        }
        while let Some(a) = q.pop_front() {
            for &b in &adj[a] {
                if hops[b] == usize::MAX {
                    hops[b] = hops[a] + 1;
                    q.push_back(b);
                }
            }
        }
        let nearest = |r: usize| adj[r].iter().copied().filter(|&o| !is_way_through(rooms[o].kind)).min_by_key(|&o| (hops[o], o));
        let merge = (first..n).filter(|&r| hops[r] != usize::MAX && hops[r] >= 3).find_map(|r| {
            let o = nearest(r)?;
            if r < first + kept && o < first {
                // (The room between: to its own neighbour nearer a way, no tower's.)
                let by = adj[o].iter().copied().filter(|&p| p < first && hops[p] < hops[o] && !is_way_through(rooms[p].kind)).min_by_key(|&p| (hops[p], p));
                if let Some(p) = by {
                    return Some((o, p));
                }
            }
            Some((r, o))
        });
        let Some((r, to)) = merge else { return };
        for c in cells.iter_mut() {
            if *c == r as i16 {
                *c = to as i16;
            }
        }
    }
}

/// A floor laid out by zones (`hub::Plan::level`), finished: a room in each of the keep's towers
/// standing on it and in the grid's corner towers (`corner`: their side), of the kinds the plan
/// gave them, doors where its zones want them (`Hints`), windows (none onto open battlements,
/// walks or roofs).
#[allow(clippy::too_many_arguments)]
pub(super) fn hub_level(laid: hub::Laid, sh: &Shell, nx: usize, ny: usize, st: [usize; 4], corner: usize, front: usize, ground: bool, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
    let hub::Laid { mut cells, mut rooms, hints, public, towers, corners, .. } = laid;
    let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
    let first_tower = rooms.len();
    for &(ti, kind) in &towers {
        let id = rooms.len() as i16;
        rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for &k in &sh.towers[ti].0 {
            if sh.inside[k] && !on_stairs(k) {
                cells[k] = id;
            }
        }
    }
    if corner > 0 && nx > 2 * corner && ny > 2 * corner {
        for (n, (cx, cy)) in [(0, 0), (nx - corner, 0), (0, ny - corner), (nx - corner, ny - corner)].into_iter().enumerate() {
            let id = rooms.len() as i16;
            let kind = corners.get(n).copied().unwrap_or(if ground { "guard post" } else { "tower room" });
            rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
            for j in cy..cy + corner {
                for i in cx..cx + corner {
                    if sh.inside[j * nx + i] && !on_stairs(j * nx + i) {
                        cells[j * nx + i] = id;
                    }
                }
            }
        }
    }
    // (A tower's room more than a room from a way about the floor, where the rooms beside it
    // stand: made part of the room beside it nearest one, as no door could bring it nearer.)
    tower_rooms_near_ways(&mut cells, &rooms, nx, ny, first_tower, towers.len());
    let cells = merge_fragments(cells, nx, ny);
    let (cells, mut rooms, map) = compact_map(cells, rooms);
    finish_rooms(&mut rooms, &cells, nx);
    let hints = hints.renumbered(&map);
    let mut lvl = connect_with(cells, rooms, nx, ny, st, front, ground, public, ext_free, Some(&hints), rng);
    windows(&mut lvl, nx, ny, ground);
    lvl
}

/// Every room one can stand in reached from the stairs, crossing from room to room only through
/// doors.
pub(super) fn reaches_all(lvl: &Level, nx: usize, ny: usize, st: [usize; 4]) -> bool {
    let k0 = st[1] * nx + st[0];
    if lvl.cells.get(k0).is_none_or(|&c| c < 0) {
        return false;
    }
    let door_between = |a: usize, b: usize| {
        lvl.doors.iter().any(|d| {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize, d.a[0].max(d.b[0]) as usize);
            let sq = if x0 == x1 { [(x0.wrapping_sub(1), y0), (x0, y0)] } else { [(x0, y0.wrapping_sub(1)), (x0, y0)] };
            let (pa, pb) = ((a % nx, a / nx), (b % nx, b / nx));
            (sq[0] == pa && sq[1] == pb) || (sq[0] == pb && sq[1] == pa)
        })
    };
    let mut seen = vec![false; nx * ny];
    let mut stack = vec![k0];
    seen[k0] = true;
    while let Some(k) = stack.pop() {
        let (i, j) = (k % nx, k / nx);
        for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
            if a >= nx || b >= ny {
                continue;
            }
            let q = b * nx + a;
            if seen[q] || lvl.cells[q] < 0 || (lvl.cells[q] != lvl.cells[k] && !door_between(k, q)) {
                continue;
            }
            seen[q] = true;
            stack.push(q);
        }
    }
    (0..lvl.rooms.len()).all(|r| !walkable(lvl.rooms[r].kind) || lvl.rooms[r].squares == 0 || (0..nx * ny).any(|k| lvl.cells[k] == r as i16 && seen[k]))
}

/// The tops of a keep's four corner towers: a battlemented square each (no main stairs), on
/// the footprint (a corner the footprint cuts off has no tower, nor its spiral stair); a keep
/// built of pieces, its own towers' (`towers`: their squares).
pub(super) fn tower_tops(inside: &[bool], nx: usize, ny: usize, t: usize, towers: &[Vec<usize>], st: [usize; 4], rng: &mut Pcg32) -> Level {
    let mut cells = vec![-1i16; nx * ny];
    let mut rooms = Vec::new();
    for sq in towers {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: "tower top", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for &k in sq {
            cells[k] = id;
        }
    }
    if !towers.is_empty() {
        finish_rooms(&mut rooms, &cells, nx);
        let mut lvl = connect(cells, rooms, nx, ny, st, 0, false, None, &|_, _| true, rng);
        lvl.has_stairs = false;
        return lvl;
    }
    for ((cx, cy), (ki, kj)) in [(0, 0), (nx - t, 0), (0, ny - t), (nx - t, ny - t)].into_iter().zip([(0, 0), (nx - 1, 0), (0, ny - 1), (nx - 1, ny - 1)]) {
        if !inside[kj * nx + ki] {
            continue;
        }
        let id = rooms.len() as i16;
        rooms.push(Room { kind: "tower top", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for j in cy..cy + t {
            for i in cx..cx + t {
                if inside[j * nx + i] {
                    cells[j * nx + i] = id;
                }
            }
        }
    }
    finish_rooms(&mut rooms, &cells, nx);
    let mut lvl = connect(cells, rooms, nx, ny, st, 0, false, None, &|_, _| true, rng);
    lvl.has_stairs = false;
    lvl
}

/// A lighthouse's middle (Roadmap 7 P8): the squares of `inside` round the grid's middle (its lamp
/// room on the roof, at least two) and its spiral stair, a two-by-two block against the wall
/// (three of its squares whole, the fourth inside the round, off the middle squares), the one
/// farthest from the door (`front`, as `mill_stairs`); else one square of the middle, as far; the
/// lamp's the middle square farthest from the stair.
pub(super) fn lighthouse_squares(inside: &[bool], whole: &[bool], nx: usize, ny: usize, front: usize) -> Option<([usize; 4], usize, Vec<usize>)> {
    let mid = [0.5 * nx as f64, 0.5 * ny as f64];
    let at = |k: usize| [(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5];
    let mut by: Vec<usize> = (0..nx * ny).filter(|&k| whole[k]).collect();
    by.sort_by(|&a, &b| geom::dist(at(a), mid).total_cmp(&geom::dist(at(b), mid)).then(a.cmp(&b)));
    let mut inner: Vec<usize> = by.iter().copied().filter(|&k| geom::dist(at(k), mid) <= 1.0).collect();
    if inner.len() < 2 {
        inner = by.iter().copied().take(2).collect();
    }
    if inner.len() < 2 {
        return None;
    }
    let door = match front {
        0 => [mid[0], 0.0],
        1 => [nx as f64, mid[1]],
        2 => [mid[0], ny as f64],
        _ => [0.0, mid[1]],
    };
    let mut best: Option<(f64, [usize; 4])> = None;
    for j in 0..ny.saturating_sub(1) {
        for i in 0..nx.saturating_sub(1) {
            let sq = [j * nx + i, j * nx + i + 1, (j + 1) * nx + i, (j + 1) * nx + i + 1];
            if sq.iter().any(|k| !inside[*k] || inner.contains(k)) || sq.iter().filter(|&&k| whole[k]).count() < 3 {
                continue;
            }
            let d = geom::dist([i as f64 + 1.0, j as f64 + 1.0], door);
            if best.is_none_or(|b| d > b.0 + 1e-9) {
                best = Some((d, [i, j, 2, 2]));
            }
        }
    }
    let st = match best {
        Some((_, st)) => st,
        None => {
            let k = inner.iter().copied().max_by(|&a, &b| geom::dist(at(a), door).total_cmp(&geom::dist(at(b), door)).then(b.cmp(&a)))?;
            [k % nx, k / nx, 1, 1]
        }
    };
    let c = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
    let on_st = |k: usize| (st[0]..st[0] + st[2]).contains(&(k % nx)) && (st[1]..st[1] + st[3]).contains(&(k / nx));
    let lamp = inner.iter().copied().filter(|&k| !on_st(k)).max_by(|&a, &b| geom::dist(at(a), c).total_cmp(&geom::dist(at(b), c)).then(b.cmp(&a)))?;
    inner.sort_unstable();
    Some((st, lamp, inner))
}

/// A lighthouse's roof (Roadmap 7 P8): its lamp room on the `inner` squares under the glazed
/// lantern, the gallery round it outside the glass (on the tower's floor squares, inside its
/// parapet, as its design rebuilds it), a door between them; reached by its stair (`st`).
pub(super) fn lamp_top(nx: usize, ny: usize, inside: &[bool], inner: &[usize], st: [usize; 4], rng: &mut Pcg32) -> Level {
    // (The gallery on the floor's squares only, inside the parapet.)
    let mut cells: Vec<i16> = inside.iter().map(|&q| if q { 1 } else { -1 }).collect();
    for &k in inner {
        cells[k] = 0;
    }
    let mut rooms = vec![
        Room { kind: "lamp room", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 },
        Room { kind: "lamp gallery", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 },
    ];
    finish_rooms(&mut rooms, &cells, nx);
    let mut lvl = connect(cells, rooms, nx, ny, st, 0, false, None, &|_, _| true, rng);
    lvl.has_stairs = false;
    lvl
}

/// A convex polygon pushed outward by `d` ft (about its centroid).
pub(super) fn grow(poly: &[P], d: f64) -> Vec<P> {
    let c = geom::centroid(poly);
    poly.iter()
        .map(|p| {
            let r = sub(*p, c);
            let l = geom::len(r).max(1e-9);
            add(*p, mul(r, d / l))
        })
        .collect()
}

/// A tower mill's stair: two squares up its wall (both on the floor's rim), the side away from
/// the door (`front`: 0 the grid's top, 1 its right, 2 its bottom, 3 its left), the middle left
/// for the stones and the gearing.
pub(super) fn mill_stairs(inside: &[bool], nx: usize, ny: usize, front: usize) -> Option<[usize; 4]> {
    let at = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && inside[j as usize * nx + i as usize];
    let rim = |i: usize, j: usize| at(i as isize, j as isize) && [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| !at(i as isize + di, j as isize + dj));
    let door = match front {
        0 => [0.5 * nx as f64, 0.0],
        1 => [nx as f64, 0.5 * ny as f64],
        2 => [0.5 * nx as f64, ny as f64],
        _ => [0.0, 0.5 * ny as f64],
    };
    let mut best: Option<(f64, [usize; 4])> = None;
    for j in 0..ny {
        for i in 0..nx {
            for (w, h) in [(2, 1), (1, 2)] {
                if i + w > nx || j + h > ny || !(0..h).all(|b| (0..w).all(|a| rim(i + a, j + b))) {
                    continue;
                }
                let m = [i as f64 + 0.5 * w as f64, j as f64 + 0.5 * h as f64];
                let d = geom::dist(m, door);
                if best.is_none_or(|b| d > b.0 + 1e-9) {
                    best = Some((d, [i, j, w, h]));
                }
            }
        }
    }
    best.map(|b| b.1)
}

pub(super) fn place_stairs(inside: &[bool], nx: usize, ny: usize, front: usize, arch: Arch) -> [usize; 4] {
    let fits = |x: usize, y: usize, w: usize, h: usize| x + w <= nx && y + h <= ny && (y..y + h).all(|j| (x..x + w).all(|i| inside[j * nx + i]));
    // Towers: a 2×2 spiral near the centre.
    if arch == Arch::Tower {
        let (cx, cy) = (nx / 2, ny / 2);
        for r in 0..nx.max(ny) {
            for (x, y) in [(cx + r, cy), (cx.saturating_sub(r), cy), (cx, cy + r), (cx, cy.saturating_sub(r))] {
                if fits(x, y, 2, 2) {
                    return [x, y, 2, 2];
                }
            }
        }
    }
    // Two squares along the long axis, one wide, at the back end, mid short side.
    let (w, h) = if nx >= 3 { (2, 1) } else { (1, 2.min(ny)) };
    let back_x = if front == 3 { nx.saturating_sub(w) } else { 0 };
    let back_x = if front == 1 { 0 } else { back_x.max(if front == 3 { 0 } else { nx.saturating_sub(w) }) };
    let y_mid = ny / 2;
    let ys: Vec<usize> = (0..ny).map(|d| if d % 2 == 0 { y_mid + d / 2 } else { y_mid.saturating_sub(d / 2 + 1) }).filter(|&y| y + h <= ny).collect();
    let xs: Vec<usize> = if back_x == 0 { (0..nx).collect() } else { (0..=nx - w).rev().collect() };
    for &x in &xs {
        for &y in &ys {
            if fits(x, y, w, h) {
                return [x, y, w, h];
            }
        }
    }
    // Any inside square.
    let k = inside.iter().position(|&i| i).unwrap_or(0);
    [k % nx, k / nx, 1, 1]
}

/// A shrine's stairs down to its crypt: two squares along a side wall from the front wall (a corner
/// first, then a square or two in), so its far end holds the god's statue and the altar; None where
/// they fit nowhere there.
pub(super) fn shrine_stairs(inside: &[bool], nx: usize, ny: usize, front: usize) -> Option<[usize; 4]> {
    let fits = |st: [usize; 4]| st[0] + st[2] <= nx && st[1] + st[3] <= ny && (st[1]..st[1] + st[3]).all(|j| (st[0]..st[0] + st[2]).all(|i| inside[j * nx + i]));
    // (Along the side walls: across the front's way in from it by `d`, at each end of it.)
    (0..3usize).flat_map(|d| [false, true].map(move |far| (d, far))).find_map(|(d, far)| {
        let st = match front {
            0 => [if far { nx.checked_sub(1)? } else { 0 }, d, 1, 2],
            1 => [nx.checked_sub(2 + d)?, if far { ny.checked_sub(1)? } else { 0 }, 2, 1],
            2 => [if far { nx.checked_sub(1)? } else { 0 }, ny.checked_sub(2 + d)?, 1, 2],
            _ => [d, if far { ny.checked_sub(1)? } else { 0 }, 2, 1],
        };
        fits(st).then_some(st)
    })
}
