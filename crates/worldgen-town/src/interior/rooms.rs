//! Levels split into rooms: `layout_level`, strips, grids, BSP, merged fragments.

use super::*;

#[derive(Clone, Copy, Debug)]
struct R {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_level(inside: &[bool], nx: usize, ny: usize, prog: &LevelProgram, st: [usize; 4], front: usize, ground: bool, arch: Arch, towers: Option<&[(Vec<usize>, Vec<usize>)]>, yard: Option<&[bool]>, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
    // A large keep's dungeon floor: guardrooms, a prison block, stores and the stair down to
    // the deep dungeons, on corridors (a small keep keeps its cell block).
    let keep_dungeon;
    let prog = if prog.cells && arch == Arch::Keep && inside.iter().filter(|&&i| i).count() >= 150 {
        keep_dungeon = LevelProgram {
            name: "Dungeon",
            rooms: vec![("guardroom", 1.5), ("prison block", 1.5), ("torture chamber", 1.0), ("armory", 1.0), ("wine cellar", 1.0), ("well room", 0.8), ("vault", 1.0), ("storeroom", 1.0), ("prison block", 1.0)],
            filler: Some("storeroom"),
            corridor: true,
            cells: false,
            by_size: false,
        };
        &keep_dungeon
    } else {
        prog
    };
    let mut kinds: Vec<(&'static str, f64)> = prog.rooms.clone();
    let area = inside.iter().filter(|&&i| i).count();
    // Rooms per floor: at least 6 squares each (small houses keep fewer rooms), fillers to
    // keep ordinary rooms under ~30 squares.
    while kinds.len() > 1 && area / kinds.len() < 6 {
        kinds.pop();
    }
    // A small inn or tavern is all taproom (the bar and tables need the space).
    if ground && matches!(arch, Arch::Tavern | Arch::Inn) && area < 40 {
        kinds.truncate(1);
    }
    // One-storey homes by size (most are 20 to 80 squares): under 24 squares one room, its bed by
    // the hearth; under 72 a main room (its hearth the kitchen) and a bedroom; bigger, a kitchen
    // too. Shops and workshops keep no storeroom under 48 squares, a shop no workshop under 32
    // (the shop its work room, the bedroom behind it).
    if prog.by_size && ground {
        if arch == Arch::House {
            kinds = if area < 24 {
                vec![("hearth room", 1.0)]
            } else if area < 72 {
                vec![("main room", 2.0), ("bedroom", 1.0)]
            } else {
                prog.rooms.clone()
            };
        } else {
            if area < 48 && matches!(kinds.last(), Some(&("storeroom" | "office", _))) {
                kinds.pop();
            }
            if area < 32 {
                kinds.retain(|k| k.0 != "workshop");
            }
        }
    }
    if let Some(f) = prog.filler {
        let fixed_big: f64 = kinds.iter().filter(|k| big_room(k.0)).map(|k| k.1).sum();
        let total: f64 = kinds.iter().map(|k| k.1).sum();
        let ordinary_area = area as f64 * (1.0 - fixed_big / total.max(1e-9));
        let ordinary = kinds.iter().filter(|k| !big_room(k.0)).count().max(1) as f64;
        // Guest rooms are about 15 ft square (a bed, a chest, a wardrobe); other fillers up to
        // ~30 squares.
        let size = if f == "guest room" { 13.0 } else { 30.0 };
        let mut extra = ((ordinary_area / size) - ordinary).max(0.0) as usize;
        extra = extra.min(if f == "guest room" { 20 } else { 10 });
        for _ in 0..extra {
            kinds.push((f, 1.0));
        }
    }

    let mut cells = vec![-1i16; nx * ny];
    let full = R { x: 0, y: 0, w: nx, h: ny };
    let mut leaves: Vec<(R, usize)> = Vec::new();
    let mut hall: Option<R> = None;

    if arch == Arch::Arena && ground {
        // A pit in the middle, stands around it.
        let inset = (nx.min(ny) / 4).clamp(1, 4);
        let pit = R { x: inset, y: inset, w: nx.saturating_sub(2 * inset).max(1), h: ny.saturating_sub(2 * inset).max(1) };
        let mut rooms = vec![Room { kind: "stands", squares: 0, raise_ft: 5.0, center: [0.0; 2], shape: 0 }, Room { kind: "arena floor", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }];
        for j in 0..ny {
            for i in 0..nx {
                if inside[j * nx + i] {
                    let in_pit = i >= pit.x && i < pit.x + pit.w && j >= pit.y && j < pit.y + pit.h;
                    let on_stairs = i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
                    cells[j * nx + i] = if in_pit && !on_stairs { 1 } else { 0 };
                }
            }
        }
        // On an odd footprint the pit can cut the stands in two (or the stands the pit): each
        // smaller piece joins the other room, so both stay one room each.
        for _ in 0..2 {
            let mut comp = vec![usize::MAX; nx * ny];
            let mut sizes: Vec<(i16, usize)> = Vec::new();
            for k0 in 0..nx * ny {
                if cells[k0] < 0 || comp[k0] != usize::MAX {
                    continue;
                }
                let (id, room) = (sizes.len(), cells[k0]);
                let (mut stack, mut n) = (vec![k0], 0);
                comp[k0] = id;
                while let Some(k) = stack.pop() {
                    n += 1;
                    let (i, j) = (k % nx, k / nx);
                    for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                        if a < nx && b < ny && cells[b * nx + a] == room && comp[b * nx + a] == usize::MAX {
                            comp[b * nx + a] = id;
                            stack.push(b * nx + a);
                        }
                    }
                }
                sizes.push((room, n));
            }
            let main = |room: i16| (0..sizes.len()).filter(|&c| sizes[c].0 == room).max_by_key(|&c| (sizes[c].1, std::cmp::Reverse(c)));
            let keep = [main(0), main(1)];
            for k in 0..nx * ny {
                if cells[k] >= 0 && Some(comp[k]) != keep[cells[k] as usize] {
                    cells[k] = 1 - cells[k];
                }
            }
        }
        finish_rooms(&mut rooms, &cells, nx);
        let mut lvl = connect(cells, rooms, nx, ny, st, front, ground, None, ext_free, rng);
        windows(&mut lvl, nx, ny, ground);
        return lvl;
    }

    if prog.cells
        && nx >= 7
        && ny >= 5
        && let Some((cells, mut rooms)) = cell_block(inside, nx, ny, st, prog.rooms[0].0).filter(|(c, _)| rooms_whole(c, nx, ny))
    {
        let (cells, mut rooms2) = compact(cells, std::mem::take(&mut rooms));
        finish_rooms(&mut rooms2, &cells, nx);
        let public = if ground { Some(prog.rooms[0].0) } else { None };
        let mut lvl = connect(cells, rooms2, nx, ny, st, front, ground, public, ext_free, rng);
        windows(&mut lvl, nx, ny, ground);
        return lvl;
    }
    // (A floor too small for a cell block is a guardroom with a holding cell off it.)
    if prog.cells {
        kinds = if area >= 40 {
            vec![(prog.rooms[0].0, 2.0), ("holding cell", 1.0), ("storeroom", 1.0)]
        } else if area >= 12 {
            vec![(prog.rooms[0].0, 2.0), ("holding cell", 1.0)]
        } else {
            vec![(prog.rooms[0].0, 1.0)]
        };
    }
    // Large floors: a grid of corridors, every room opening onto one.
    let large = !prog.cells && area >= 150 && nx.min(ny) >= 12 && prog.rooms.len() >= 2 && !(ground && matches!(arch, Arch::Warehouse | Arch::Temple)) && !matches!(arch, Arch::Tower | Arch::Arena);
    let mut rooms: Vec<Room> = if large {
        let (gc, gr) = grid_rooms(inside, nx, ny, prog, st, front, ground, rng);
        cells = gc;
        gr
    } else {
    let corridor_ok = prog.corridor && kinds.len() >= 3 && ny >= 7 && nx >= 6;
    if corridor_ok {
        // A 1-square corridor down the long axis through the stair row.
        let cy = st[1].clamp(1, ny - 2);
        let h = R { x: 0, y: cy, w: nx, h: 1 };
        hall = Some(h);
        let top = R { x: 0, y: 0, w: nx, h: cy };
        let bot = R { x: 0, y: cy + 1, w: nx, h: ny - cy - 1 };
        let (mut a, mut bb): (Vec<(&str, f64)>, Vec<(&str, f64)>) = (Vec::new(), Vec::new());
        let (mut wa, mut wb) = (0.0, 0.0);
        for k in kinds.iter() {
            if wa * (bot.h as f64) <= wb * (top.h as f64) {
                a.push(*k);
                wa += k.1;
            } else {
                bb.push(*k);
                wb += k.1;
            }
        }
        // One row of rooms each side, every one on the corridor.
        strip(top, &a, &mut leaves, 0);
        let off = a.len();
        let mut lb = Vec::new();
        strip(bot, &bb, &mut lb, 0);
        leaves.extend(lb.into_iter().map(|(r, i)| (r, i + off)));
        kinds = a.into_iter().chain(bb).collect();
    } else {
        bsp(full, &kinds, st, rng, &mut leaves, 0);
        // A small home's or shop's bedroom (rooms go to leaves by size) off the stairs down: if
        // they fall in its leaf, the rooms laid the other way round (its leaf at the other end), if
        // the biggest room still has a way out (the main room's door).
        if prog.by_size && ground && kinds.len() >= 2 {
            let area_of = |r: &R| (r.y..r.y + r.h).map(|j| (r.x..r.x + r.w).filter(|&i| inside[j * nx + i]).count()).sum::<usize>();
            let on_st = |r: &R| r.x < st[0] + st[2] && st[0] < r.x + r.w && r.y < st[1] + st[3] && st[1] < r.y + r.h;
            let mut by_w: Vec<usize> = (0..kinds.len()).collect();
            by_w.sort_by(|&p, &q| kinds[q].1.total_cmp(&kinds[p].1).then(p.cmp(&q)));
            let mut by_a: Vec<usize> = (0..leaves.len()).collect();
            by_a.sort_by(|&p, &q| area_of(&leaves[q].0).cmp(&area_of(&leaves[p].0)).then(p.cmp(&q)));
            let rank = by_w.iter().position(|&k| kinds[k].0 == "bedroom");
            if rank.and_then(|r| by_a.get(r)).is_some_and(|&li| on_st(&leaves[li].0)) {
                let back: Vec<(&'static str, f64)> = kinds.iter().rev().copied().collect();
                let mut turned = Vec::new();
                bsp(full, &back, st, rng, &mut turned, 0);
                let way_out = |r: &R| {
                    (r.y..r.y + r.h).any(|j| {
                        (r.x..r.x + r.w).any(|i| {
                            inside[j * nx + i]
                                && [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| {
                                    let (a, b) = (i as isize + di, j as isize + dj);
                                    (a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || !inside[b as usize * nx + a as usize]) && ext_free(a, b)
                                })
                        })
                    })
                };
                if turned.iter().max_by_key(|l| area_of(&l.0)).is_some_and(|l| way_out(&l.0)) {
                    leaves = turned;
                }
            }
        }
    }

    // Assign program kinds to leaves: the first (public) room to the biggest leaf on the
    // front side (on the ground floor), the rest by size.
    let leaf_area = |r: &R| (r.y..r.y + r.h).map(|j| (r.x..r.x + r.w).filter(|&i| inside[j * nx + i]).count()).sum::<usize>();
    let touches_front = |r: &R| match front {
        0 => r.y == 0,
        1 => r.x + r.w == nx,
        2 => r.y + r.h == ny,
        _ => r.x == 0,
    };
    let mut order: Vec<usize> = (0..leaves.len()).collect();
    order.sort_by(|&p, &q| leaf_area(&leaves[q].0).cmp(&leaf_area(&leaves[p].0)).then(p.cmp(&q)));
    let mut by_weight: Vec<usize> = (0..kinds.len()).collect();
    by_weight.sort_by(|&p, &q| kinds[q].1.total_cmp(&kinds[p].1).then(p.cmp(&q)));
    let mut assign = vec![usize::MAX; leaves.len()];
    // The public room (the first of the program) on the ground floor: the biggest leaf with
    // open ground outside it (a door can be put there), on the street side if possible.
    let open_out = |r: &R| {
        (r.y..r.y + r.h).any(|j| {
            (r.x..r.x + r.w).any(|i| {
                inside[j * nx + i]
                    && [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| {
                        let (a, b) = (i as isize + di, j as isize + dj);
                        let out = a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || !inside[b as usize * nx + a as usize];
                        out && ext_free(a, b)
                    })
            })
        })
    };
    if ground && hall.is_none() && !order.is_empty() {
        // The biggest leaf with a way outside; the street side wins only a near tie.
        let open: Vec<usize> = order.iter().copied().filter(|&li| open_out(&leaves[li].0)).collect();
        let first = match open.first() {
            Some(&big) => open.iter().copied().find(|&li| touches_front(&leaves[li].0) && 4 * leaf_area(&leaves[li].0) >= 3 * leaf_area(&leaves[big].0)).unwrap_or(big),
            None => order[0],
        };
        assign[first] = 0;
    }
    let pre: Vec<usize> = assign.iter().copied().filter(|&k| k != usize::MAX).collect();
    let mut next_kind = by_weight.iter().copied().filter(|k| !pre.contains(k));
    for &li in &order {
        if assign[li] == usize::MAX {
            assign[li] = next_kind.next().unwrap_or(by_weight[0]);
        }
    }
    // (And where they still fall in it, a service room's leaf for it if one is clear of them: its
    // bed and the stairs' way clear.)
    if prog.by_size && ground {
        let on_st = |r: &R| r.x < st[0] + st[2] && st[0] < r.x + r.w && r.y < st[1] + st[3] && st[1] < r.y + r.h;
        let kind = |li: usize| kinds[assign[li]].0;
        if let Some(bl) = (0..leaves.len()).find(|&li| kind(li) == "bedroom" && on_st(&leaves[li].0)) {
            let others: Vec<usize> = (0..leaves.len()).filter(|&li| kind(li) != "bedroom" && leaf_area(&leaves[li].0) > 0 && !on_st(&leaves[li].0)).collect();
            if let Some(o) = others.iter().copied().find(|&li| assign[li] != 0 || hall.is_some()) {
                assign.swap(bl, o);
            }
        }
    }
    let mut rooms: Vec<Room> = Vec::new();
    for (li, (r, _)) in leaves.iter().enumerate() {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: kinds[assign[li]].0, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for j in r.y..r.y + r.h {
            for i in r.x..r.x + r.w {
                if inside[j * nx + i] {
                    cells[j * nx + i] = id;
                }
            }
        }
    }
    if let Some(h) = hall {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: "hall", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for i in h.x..h.x + h.w {
            if inside[h.y * nx + i] {
                cells[h.y * nx + i] = id;
            }
        }
    }
    rooms
    };
    // Keeps: a tower in each corner, a room of its own on every floor above the cellar (a keep
    // built of pieces: in each of its towers).
    let t = if arch == Arch::Keep && towers.is_none() { keep_tower_squares(nx, ny) } else { 0 };
    let ways: Vec<usize> = towers.unwrap_or_default().iter().flat_map(|t| t.1.iter().copied()).collect();
    let first_tower = rooms.len();
    let mut tower_ids: Vec<(i16, &Vec<usize>, &Vec<usize>)> = Vec::new();
    for (sq, doors) in towers.unwrap_or_default() {
        let kind: &'static str = if ground { "guard post" } else { "tower room" };
        let id = rooms.len() as i16;
        rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for &k in sq {
            let (i, j) = (k % nx, k / nx);
            if inside[k] && !(i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3]) {
                cells[k] = id;
            }
        }
        // Its ways in are wall until the rooms round it are settled (below).
        for &k in doors {
            cells[k] = -1;
        }
        tower_ids.push((id, sq, doors));
    }
    if t > 0 {
        let kind: &'static str = if ground { "guard post" } else { "tower room" };
        for (cx, cy) in [(0, 0), (nx - t, 0), (0, ny - t), (nx - t, ny - t)] {
            let id = rooms.len() as i16;
            rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
            for j in cy..cy + t {
                for i in cx..cx + t {
                    if inside[j * nx + i] {
                        cells[j * nx + i] = id;
                    }
                }
            }
        }
    }
    // Rooms split into pieces by the footprint shape keep their largest piece; the others
    // join the neighbour they share the longest wall with. Empty rooms are dropped.
    let mut cells = merge_fragments(cells, nx, ny);
    // A keep built of pieces: a sliver a square wide left between its pieces' walls (and its
    // towers') is part of the room it opens off, not a room of its own.
    if towers.is_some() {
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        for r in 0..first_tower.min(rooms.len()) as i16 {
            let mine: Vec<usize> = (0..nx * ny).filter(|&k| cells[k] == r).collect();
            if mine.is_empty() || mine.len() > 8 || mine.iter().any(|&k| on_stairs(k)) {
                continue;
            }
            let (i0, i1) = mine.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (j0, j1) = mine.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            if i0 != i1 && j0 != j1 {
                continue;
            }
            let mut shared: Vec<(i16, usize)> = Vec::new();
            for &k in &mine {
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny {
                        let o = cells[b * nx + a];
                        if o >= 0 && o != r && (o as usize) < first_tower {
                            match shared.iter_mut().find(|x| x.0 == o) {
                                Some(x) => x.1 += 1,
                                None => shared.push((o, 1)),
                            }
                        }
                    }
                }
            }
            if let Some(&(o, _)) = shared.iter().max_by(|x, y| x.1.cmp(&y.1).then(y.0.cmp(&x.0))) {
                for &k in &mine {
                    cells[k] = o;
                }
            }
        }
    }
    // A room that is nothing but the stair block is the stairwell of its neighbour.
    for r in 0..rooms.len() as i16 {
        let mine: Vec<usize> = (0..nx * ny).filter(|&k| cells[k] == r).collect();
        let all_stairs = !mine.is_empty() && mine.iter().all(|&k| { let (i, j) = (k % nx, k / nx); i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3] });
        if all_stairs {
            let nb = mine.iter().flat_map(|&k| {
                let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)]
            }).filter(|&(a, b)| a >= 0 && b >= 0 && a < nx as isize && b < ny as isize).map(|(a, b)| cells[b as usize * nx + a as usize]).find(|&c| c >= 0 && c != r);
            if let Some(nb) = nb {
                for &k in &mine {
                    cells[k] = nb;
                }
            }
        }
    }
    // A keep's towers: each one's way in on this floor, through its wall from the room beside it
    // nearest the circulation (a hall or corridor, else a room beside one); where that room is no
    // nearer than that, a passage on from it to the nearest hall or corridor through the rooms
    // between (the way in theirs). Its other ways are wall here.
    if !tower_ids.is_empty() {
        let side = |k: usize| {
            let (i, j) = (k % nx, k / nx);
            [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().filter(|&(a, b)| a < nx && b < ny).map(move |(a, b)| b * nx + a)
        };
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        let mut carved = false;
        for &(id, sq, doors) in &tower_ids {
            let circ = |r: i16| r >= 0 && is_way_through(rooms[r as usize].kind);
            // How many rooms from the circulation each room is, by the walls they share.
            let mut hops: Vec<usize> = vec![usize::MAX; rooms.len()];
            let mut q: std::collections::VecDeque<usize> = (0..rooms.len()).filter(|&r| circ(r as i16) && cells.contains(&(r as i16))).collect();
            for &r in &q {
                hops[r] = 0;
            }
            while let Some(r) = q.pop_front() {
                for k in (0..nx * ny).filter(|&k| cells[k] == r as i16) {
                    for n in side(k) {
                        let o = cells[n];
                        if o >= 0 && (o as usize) < first_tower && hops[o as usize] == usize::MAX {
                            hops[o as usize] = hops[r] + 1;
                            q.push_back(o as usize);
                        }
                    }
                }
            }
            let opens = |k: usize| side(k).filter(|q| !ways.contains(q) && !sq.contains(q)).map(|q| cells[q]).find(|&r| r >= 0 && (r as usize) < first_tower);
            let rank = |k: usize| opens(k).map_or(usize::MAX, |r| hops[r as usize]);
            // (A tower with no ways set on this floor opens onto any room beside it: where none of
            // them is within a room of the circulation, a passage from beside it to the nearest
            // hall or corridor, 16 squares at most, as below.)
            if doors.is_empty() {
                let beside: Vec<usize> = sq.iter().flat_map(|&k| side(k)).filter(|&q| !sq.contains(&q) && cells[q] >= 0 && (cells[q] as usize) < first_tower && !on_stairs(q)).collect();
                if beside.is_empty() || beside.iter().any(|&q| hops[cells[q] as usize] < 2) {
                    continue;
                }
                let mut from = vec![usize::MAX; nx * ny];
                let mut q: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
                for &b in &beside {
                    if from[b] == usize::MAX {
                        from[b] = b;
                        q.push_back(b);
                    }
                }
                let mut found = None;
                while let Some(c) = q.pop_front() {
                    if circ(cells[c]) {
                        let mut path = Vec::new();
                        let mut x = from[c];
                        while from[x] != x {
                            path.push(x);
                            x = from[x];
                        }
                        path.push(x);
                        found = (path.len() <= 16).then_some((cells[c], path));
                        break;
                    }
                    for n in side(c) {
                        if from[n] == usize::MAX && cells[n] >= 0 && (cells[n] as usize) < first_tower && !ways.contains(&n) && !on_stairs(n) && !sq.contains(&n) {
                            from[n] = c;
                            q.push_back(n);
                        }
                    }
                }
                if let Some((hall, path)) = found {
                    for c in path {
                        cells[c] = hall;
                    }
                    carved = true;
                }
                continue;
            }
            let Some(k) = doors.iter().copied().filter(|&k| side(k).any(|q| cells[q] == id) && opens(k).is_some()).min_by_key(|&k| (rank(k), k)) else { continue };
            let r = opens(k).unwrap_or(-1);
            let passage = (rank(k) >= 2).then(|| {
                let mut from = vec![usize::MAX; nx * ny];
                let mut q = std::collections::VecDeque::from([k]);
                from[k] = k;
                while let Some(c) = q.pop_front() {
                    if c != k && circ(cells[c]) {
                        let mut path = Vec::new();
                        let mut x = from[c];
                        while x != k {
                            path.push(x);
                            x = from[x];
                        }
                        return (path.len() <= 8).then_some((cells[c], path));
                    }
                    for n in side(c) {
                        if from[n] == usize::MAX && cells[n] >= 0 && (cells[n] as usize) < first_tower && !ways.contains(&n) && !on_stairs(n) {
                            from[n] = c;
                            q.push_back(n);
                        }
                    }
                }
                None
            });
            match passage.flatten() {
                Some((hall, path)) => {
                    for c in path.into_iter().chain([k]) {
                        cells[c] = hall;
                    }
                    carved = true;
                }
                None => cells[k] = r,
            }
        }
        if carved {
            cells = merge_fragments(cells, nx, ny);
        }
    }
    // (Roadmap 7, 164) An inn round its yard (`yard`: a U's or a court's, `town::wings`), above its
    // ground floor: a gallery along the yard side of its ranges, railed onto the yard (`doors`: a yard
    // gallery's side onto open ground), every room off it; not on the outer wall's squares.
    if let Some(yard) = yard.filter(|_| !ground) {
        let g = rooms.len() as i16;
        let on_st = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
        let near = |i: usize, j: usize| (-1..=1isize).any(|dj| (-1..=1isize).any(|di| {
            let (a, b) = (i as isize + di, j as isize + dj);
            a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny && yard[b as usize * nx + a as usize]
        }));
        let mut any = false;
        for k in 0..nx * ny {
            let (i, j) = (k % nx, k / nx);
            if cells[k] >= 0 && i > 0 && j > 0 && i + 1 < nx && j + 1 < ny && !on_st(i, j) && near(i, j) {
                cells[k] = g;
                any = true;
            }
        }
        if any {
            rooms.push(Room { kind: "yard gallery", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
            cells = merge_fragments(cells, nx, ny);
        }
    }
    // On a floor of eight rooms or more with a hall or corridor (not a keep's), no room more than two
    // rooms from it by the walls they share: a passage carved to such a room from the nearest hall or
    // corridor square through the rooms between (theirs the hall's, 16 squares at most), the
    // farthest room first; a room the passage cuts in two keeps its largest piece, and any other
    // piece of 12 squares or more is a room of its own of the same kind (the rest made whole again).
    if arch != Arch::Keep && first_tower == rooms.len() {
        let circ = |rooms: &[Room], r: i16| r >= 0 && is_way_through(rooms[r as usize].kind);
        let side = |k: usize| {
            let (i, j) = (k % nx, k / nx);
            [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().filter(|&(a, b)| a < nx && b < ny).map(move |(a, b)| b * nx + a)
        };
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        let mut carved = false;
        for _ in 0..8 {
            let live: Vec<bool> = (0..rooms.len()).map(|r| cells.contains(&(r as i16))).collect();
            if live.iter().filter(|&&l| l).count() < 8 || !(0..rooms.len()).any(|r| live[r] && circ(&rooms, r as i16)) {
                break;
            }
            let mut hops: Vec<usize> = vec![usize::MAX; rooms.len()];
            let mut q: std::collections::VecDeque<usize> = (0..rooms.len()).filter(|&r| live[r] && circ(&rooms, r as i16)).collect();
            for &r in &q {
                hops[r] = 0;
            }
            while let Some(r) = q.pop_front() {
                for k in (0..nx * ny).filter(|&k| cells[k] == r as i16) {
                    for n in side(k) {
                        let o = cells[n];
                        if o >= 0 && hops[o as usize] == usize::MAX {
                            hops[o as usize] = hops[r] + 1;
                            q.push_back(o as usize);
                        }
                    }
                }
            }
            let Some(far) = (0..rooms.len()).filter(|&r| hops[r] != usize::MAX && hops[r] >= 3).max_by_key(|&r| (hops[r], std::cmp::Reverse(r))) else { break };
            // From the far room's squares to the nearest square of a hall or corridor, turning as
            // little as it can (a step 10, a turn 3 more: straight runs, not a staircase).
            let state = |k: usize, d: usize| k * 5 + d;
            let mut cost = vec![u32::MAX; nx * ny * 5];
            let mut back = vec![usize::MAX; nx * ny * 5];
            let mut heap = std::collections::BinaryHeap::new();
            for k in (0..nx * ny).filter(|&k| cells[k] == far as i16) {
                cost[state(k, 4)] = 0;
                heap.push(std::cmp::Reverse((0u32, state(k, 4))));
            }
            let mut found = None;
            while let Some(std::cmp::Reverse((c, st_))) = heap.pop() {
                if c > cost[st_] {
                    continue;
                }
                let (k, d) = (st_ / 5, st_ % 5);
                if circ(&rooms, cells[k]) {
                    found = Some(st_);
                    break;
                }
                let (i, j) = (k % nx, k / nx);
                for (nd, (a, b)) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().enumerate() {
                    if a >= nx || b >= ny {
                        continue;
                    }
                    let n = b * nx + a;
                    if cells[n] < 0 || on_stairs(n) || cells[n] == far as i16 {
                        continue;
                    }
                    let nc = c + 10 + if d != 4 && d != nd { 3 } else { 0 };
                    if nc < cost[state(n, nd)] {
                        cost[state(n, nd)] = nc;
                        back[state(n, nd)] = st_;
                        heap.push(std::cmp::Reverse((nc, state(n, nd))));
                    }
                }
            }
            let Some(end_state) = found else { break };
            let end = end_state / 5;
            let mut path = Vec::new();
            let mut x = back[end_state];
            while x != usize::MAX && cells[x / 5] != far as i16 {
                path.push(x / 5);
                x = back[x];
            }
            if path.len() > 16 {
                break;
            }
            let hall = cells[end];
            let mut cut: Vec<i16> = Vec::new();
            for c in path {
                if !cut.contains(&cells[c]) {
                    cut.push(cells[c]);
                }
                cells[c] = hall;
            }
            // (Each room cut: its pieces, the largest kept; the others, with those of other rooms
            // they touch, a room of their own of the biggest one's kind where 12 squares or more.)
            let mut off: Vec<(i16, Vec<usize>)> = Vec::new();
            for r in cut {
                let mut pieces: Vec<Vec<usize>> = Vec::new();
                let mut seen = vec![false; nx * ny];
                for k0 in (0..nx * ny).filter(|&k| cells[k] == r) {
                    if seen[k0] {
                        continue;
                    }
                    seen[k0] = true;
                    let mut piece = vec![k0];
                    let mut i = 0;
                    while i < piece.len() {
                        for n in side(piece[i]) {
                            if cells[n] == r && !seen[n] {
                                seen[n] = true;
                                piece.push(n);
                            }
                        }
                        i += 1;
                    }
                    pieces.push(piece);
                }
                pieces.sort_by_key(|p| (std::cmp::Reverse(p.len()), p[0]));
                off.extend(pieces.into_iter().skip(1).map(|p| (r, p)));
            }
            let mut group: Vec<usize> = (0..off.len()).collect();
            let touch = |a: &[usize], b: &[usize]| a.iter().any(|&k| side(k).any(|n| b.contains(&n)));
            for i in 0..off.len() {
                for j in i + 1..off.len() {
                    if touch(&off[i].1, &off[j].1) {
                        let (gi, gj) = (group[i], group[j]);
                        for g in group.iter_mut() {
                            if *g == gj {
                                *g = gi;
                            }
                        }
                    }
                }
            }
            for g in 0..off.len() {
                let members: Vec<usize> = (0..off.len()).filter(|&i| group[i] == g).collect();
                if members.iter().map(|&i| off[i].1.len()).sum::<usize>() < 12 {
                    continue;
                }
                let big = members.iter().copied().max_by_key(|&i| (off[i].1.len(), std::cmp::Reverse(i))).unwrap_or(g);
                let id = rooms.len() as i16;
                let from = off[big].0 as usize;
                rooms.push(Room { kind: rooms[from].kind, squares: 0, raise_ft: rooms[from].raise_ft, center: [0.0; 2], shape: 0 });
                for &i in &members {
                    for &k in &off[i].1 {
                        cells[k] = id;
                    }
                }
            }
            carved = true;
        }
        if carved {
            cells = merge_fragments(cells, nx, ny);
        }
    }
    let (cells, mut rooms) = compact(cells, rooms);
    finish_rooms(&mut rooms, &cells, nx);
    let public = if ground { prog.rooms.first().map(|r| r.0) } else { None };
    let mut lvl = connect(cells, rooms, nx, ny, st, front, ground, public, ext_free, rng);
    windows(&mut lvl, nx, ny, ground);
    lvl
}

/// Cut `r` along x into one room per kind, widths by weight (at least 2 squares each).
fn strip(r: R, kinds: &[(&'static str, f64)], out: &mut Vec<(R, usize)>, base: usize) {
    if kinds.is_empty() || r.w == 0 || r.h == 0 {
        return;
    }
    let n = kinds.len().min(r.w / 2).max(1);
    let total: f64 = kinds[..n].iter().map(|k| k.1).sum();
    let mut x = r.x;
    let mut acc = 0.0;
    for (k, kind) in kinds[..n].iter().enumerate() {
        acc += kind.1;
        let x1 = if k + 1 == n { r.x + r.w } else { (r.x + ((r.w as f64) * acc / total).round() as usize).clamp(x + 2, r.x + r.w - 2 * (n - k - 1)) };
        out.push((R { x, y: r.y, w: x1 - x, h: r.h }, base + k));
        x = x1;
    }
}

/// Large floors: corridor rows about every 8 squares along the long axis (one through the
/// stairs where it can), cross corridors near both ends and every ~22 squares joining them,
/// and a single row of rooms in each band between, so every room opens onto a corridor. On
/// the ground floor the public room (the great hall) spans two bands by the front.
#[allow(clippy::too_many_arguments)]
fn grid_rooms(inside: &[bool], nx: usize, ny: usize, prog: &LevelProgram, st: [usize; 4], front: usize, ground: bool, rng: &mut Pcg32) -> (Vec<i16>, Vec<Room>) {
    // (u, v): u along the long axis, v across it.
    let long_x = nx >= ny;
    let (w, h) = if long_x { (nx, ny) } else { (ny, nx) };
    let idx = |u: usize, v: usize| if long_x { v * nx + u } else { u * nx + v };
    let (sv0, sv1) = if long_x { (st[1], st[1] + st[3]) } else { (st[0], st[0] + st[2]) };
    let m = ((h as f64 - 1.0) / 8.5).round().max(1.0) as usize;
    let mut rows: Vec<usize> = (0..m).map(|k| ((k + 1) * (h + 1) / (m + 1)).saturating_sub(1).clamp(3, h - 4)).collect();
    let mid = (sv0 + sv1) / 2;
    if let Some(r) = rows.iter_mut().min_by_key(|r| r.abs_diff(mid))
        && r.abs_diff(mid) <= 3
    {
        *r = mid.clamp(3, h - 4);
    }
    rows.sort();
    rows.dedup();
    let end = 3usize.min(w / 4);
    let mut cols = vec![end, w - 1 - end];
    let span = w - 1 - 2 * end;
    for k in 1..=span / 22 {
        cols.push(end + k * span / (span / 22 + 1));
    }
    cols.sort();
    cols.dedup();
    let (r0, r1) = (rows[0], *rows.last().unwrap());
    let mut cells = vec![-1i16; nx * ny];
    let mut rooms = vec![Room { kind: "hall", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }];
    let set = |cells: &mut Vec<i16>, u: usize, v: usize, id: i16| {
        if inside[idx(u, v)] {
            cells[idx(u, v)] = id;
        }
    };
    for &v in &rows {
        for u in 0..w {
            set(&mut cells, u, v, 0);
        }
    }
    // Cross corridors join the rows; the two at the ends run wall to wall (past the corner
    // towers, which open onto them).
    for &u in &cols {
        let (a, b) = if u == cols[0] || u == *cols.last().unwrap() { (0, h - 1) } else { (r0, r1) };
        for v in a..=b {
            set(&mut cells, u, v, 0);
        }
    }
    // On an irregular footprint some corridor runs end up apart: join each to the rest by the
    // shortest way across the floor.
    loop {
        let mut comp = vec![usize::MAX; nx * ny];
        let mut sizes: Vec<usize> = Vec::new();
        for s0 in 0..nx * ny {
            if cells[s0] != 0 || comp[s0] != usize::MAX {
                continue;
            }
            let c = sizes.len();
            let mut stack = vec![s0];
            comp[s0] = c;
            let mut size = 0;
            while let Some(k) = stack.pop() {
                size += 1;
                let (i, j) = (k % nx, k / nx);
                for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                    let (a, b) = (i as isize + di, j as isize + dj);
                    if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                        let m = b as usize * nx + a as usize;
                        if cells[m] == 0 && comp[m] == usize::MAX {
                            comp[m] = c;
                            stack.push(m);
                        }
                    }
                }
            }
            sizes.push(size);
        }
        if sizes.len() <= 1 {
            break;
        }
        let main = (0..sizes.len()).max_by_key(|&c| sizes[c]).unwrap();
        let other = (0..sizes.len()).find(|&c| c != main).unwrap();
        let mut prev = vec![usize::MAX; nx * ny];
        let mut q: std::collections::VecDeque<usize> = (0..nx * ny).filter(|&k| comp[k] == other).collect();
        for &k in &q {
            prev[k] = k;
        }
        let mut hit = None;
        while let Some(k) = q.pop_front() {
            if comp[k] == main {
                hit = Some(k);
                break;
            }
            let (i, j) = (k % nx, k / nx);
            for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i as isize + di, j as isize + dj);
                if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                    let m = b as usize * nx + a as usize;
                    if inside[m] && prev[m] == usize::MAX {
                        prev[m] = k;
                        q.push_back(m);
                    }
                }
            }
        }
        match hit {
            Some(mut k) => {
                while prev[k] != k {
                    cells[k] = 0;
                    k = prev[k];
                }
            }
            // Unreachable across the floor: let it go to the rooms.
            None => {
                for k in 0..nx * ny {
                    if comp[k] == other {
                        cells[k] = -1;
                    }
                }
            }
        }
    }
    // Bands between the rows (and the walls): (v0, v1) half-open.
    let mut edges = vec![0usize];
    for &r in &rows {
        edges.push(r);
        edges.push(r + 1);
    }
    edges.push(h);
    let bands: Vec<(usize, usize)> = edges.chunks(2).map(|c| (c[0], c[1])).filter(|c| c.1 > c.0).collect();
    // The public room on the ground floor: two bands and the corridor between, on the front
    // side, over the middle half (or from the front end when the front is a short side).
    let mut public: Option<(usize, usize, usize, usize)> = None;
    if ground && big_room(prog.rooms[0].0) && bands.len() >= 2 {
        let front_v_low = if long_x { front == 0 } else { front == 3 };
        let front_v = if long_x { matches!(front, 0 | 2) } else { matches!(front, 1 | 3) };
        let pair = if !front_v || front_v_low { 0 } else { bands.len() - 2 };
        let (v0, v1) = (bands[pair].0, bands[pair + 1].1);
        let (u0, u1) = if front_v {
            (w / 4, w - w / 4)
        } else if (long_x && front == 3) || (!long_x && front == 0) {
            (0, w / 2)
        } else {
            (w - w / 2, w)
        };
        public = Some((u0, u1, v0, v1));
        let id = rooms.len() as i16;
        rooms.push(Room { kind: prog.rooms[0].0, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for v in v0..v1 {
            for u in u0..u1 {
                set(&mut cells, u, v, id);
            }
        }
    }
    let in_public = |u: usize, v: usize| public.is_some_and(|(u0, u1, v0, v1)| u >= u0 && u < u1 && v >= v0 && v < v1);
    // Rooms: each band's runs between cross corridors (inner bands) cut along u.
    let mut pieces: Vec<(usize, usize, usize, usize)> = Vec::new();
    for &(v0, v1) in &bands {
        let inner = v0 > r0 && v1 <= r1 + 1;
        let mut cuts: Vec<usize> = vec![0];
        for &c in &cols {
            if inner || c == cols[0] || c == *cols.last().unwrap() {
                cuts.push(c);
                cuts.push(c + 1);
            }
        }
        cuts.push(w);
        for seg in cuts.chunks(2) {
            let (mut a, b) = (seg[0], seg[1]);
            // Skip the public room's columns.
            while a < b {
                if (v0..v1).all(|v| in_public(a, v)) {
                    a += 1;
                    continue;
                }
                let mut e = a;
                while e < b && !(v0..v1).all(|v| in_public(e, v)) {
                    e += 1;
                }
                // Cut [a, e) into rooms about 40 squares each.
                let depth = v1 - v0;
                let target = ((40.0 / depth as f64).round() as usize).clamp(3, 9);
                let mut x = a;
                while x < e {
                    let wdt = (target + rng.below(3) as usize).saturating_sub(1).max(3);
                    let x1 = if e - x < wdt + 3 { e } else { x + wdt };
                    pieces.push((x, x1, v0, v1));
                    x = x1;
                }
                a = e;
            }
        }
    }
    // Program rooms (after the public room) to the biggest pieces, fillers to the rest.
    let mut kinds: Vec<(&'static str, f64)> = prog.rooms.iter().copied().skip(if public.is_some() { 1 } else { 0 }).collect();
    kinds.sort_by(|a, b| b.1.total_cmp(&a.1));
    let filler = prog.filler.unwrap_or(prog.rooms[0].0);
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    let size = |p: &(usize, usize, usize, usize)| (p.1 - p.0) * (p.3 - p.2);
    order.sort_by(|&a, &b| size(&pieces[b]).cmp(&size(&pieces[a])).then(a.cmp(&b)));
    let mut kind_of = vec![filler; pieces.len()];
    for (k, &pi) in order.iter().enumerate() {
        if let Some(&(kk, _)) = kinds.get(k) {
            kind_of[pi] = kk;
        }
    }
    for (pi, &(u0, u1, v0, v1)) in pieces.iter().enumerate() {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: kind_of[pi], squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for v in v0..v1 {
            for u in u0..u1 {
                if !in_public(u, v) && cells[idx(u, v)] != 0 {
                    set(&mut cells, u, v, id);
                }
            }
        }
    }
    (cells, rooms)
}

/// A cell block: a strip beside the stairs (full height, the stairs' end of the building) for
/// the guards, a spine corridor along it, and corridors off the spine every five rows with a
/// row of 10 × 10 ft cells on each side. Every cell touches a corridor; rows left over at the
/// far end become corridor.
///
/// Irregular footprints: cells only in full-height columns (their corridor rows are whole);
/// partial columns on the stairs' side join the strip, those at the far end are corridor.
/// `None` when fewer than two full columns are left for cells.
fn cell_block(inside: &[bool], nx: usize, ny: usize, st: [usize; 4], strip_kind: &'static str) -> Option<(Vec<i16>, Vec<Room>)> {
    let full: Vec<bool> = (0..nx).map(|i| (0..ny).all(|j| inside[j * nx + i])).collect();
    let west = st[0] + st[2] / 2 < nx / 2;
    let sw = if west { (st[0] + st[2]).max(3) } else { (nx - st[0]).max(3) };
    let sw = sw.min(nx - 3);
    // The spine: the first full column past the strip.
    let spine = if west { (sw..nx).find(|&i| full[i])? } else { (0..nx - sw).rev().find(|&i| full[i])? };
    let beyond = |i: usize| if west { i > spine } else { i < spine };
    if (0..nx).filter(|&i| beyond(i) && full[i]).count() < 2 {
        return None;
    }
    let in_strip = |i: usize| if west { i < spine } else { i > spine };
    let mut cells = vec![-1i16; nx * ny];
    let mut rooms = vec![Room { kind: strip_kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }, Room { kind: "corridor", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }];
    // Row roles: 2 = corridor (every fifth row from 2, and the rows the pattern leaves without
    // one at the far end); 0/1 = cell rows.
    let role = |j: usize| -> u8 {
        let m = j % 5;
        let block_end = (j / 5) * 5 + 5;
        if m == 2 {
            2
        } else if m < 2 && block_end > ny && ny - (j / 5) * 5 <= 2 {
            // The last block has no corridor row: its last row is one.
            if j + 1 == ny { 2 } else { 0 }
        } else {
            0
        }
    };
    let cell_cols: Vec<usize> = (0..nx).filter(|&i| beyond(i) && full[i]).collect();
    for j in 0..ny {
        for i in 0..nx {
            if !inside[j * nx + i] {
                continue;
            }
            cells[j * nx + i] = if in_strip(i) {
                0
            } else if i == spine || role(j) == 2 || !full[i] {
                1
            } else {
                -2
            };
        }
    }
    // Cells: 2 columns wide (the last may be 1) and the cell rows between corridors (at most 2).
    let mut j = 0;
    while j < ny {
        if role(j) == 2 {
            j += 1;
            continue;
        }
        let mut h = 1;
        while h < 2 && j + h < ny && role(j + h) != 2 {
            h += 1;
        }
        let mut k = 0;
        while k < cell_cols.len() {
            let w = if cell_cols.len() - k == 3 { 1 } else { 2.min(cell_cols.len() - k) };
            let id = rooms.len() as i16;
            let mut any = false;
            for b in j..j + h {
                for &i in &cell_cols[k..k + w] {
                    if cells[b * nx + i] == -2 {
                        cells[b * nx + i] = id;
                        any = true;
                    }
                }
            }
            if any {
                rooms.push(Room { kind: "cell", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
            }
            k += w;
        }
        j += h;
    }
    for c in cells.iter_mut() {
        if *c == -2 {
            *c = 1;
        }
    }
    Some((cells, rooms))
}

/// Recursive split of a rectangle among rooms (weights), never cutting through the stairs.
fn bsp(r: R, rooms: &[(&'static str, f64)], st: [usize; 4], rng: &mut Pcg32, out: &mut Vec<(R, usize)>, base: usize) {
    if rooms.len() <= 1 || r.w * r.h < 8 {
        // Leftover rooms (no space) merge into this leaf.
        out.push((r, base));
        if rooms.len() > 1 {
            // Mark the rest as leaves of zero size so indices stay aligned.
            for k in 1..rooms.len() {
                out.push((R { x: r.x, y: r.y, w: 0, h: 0 }, base + k));
            }
        }
        return;
    }
    let total: f64 = rooms.iter().map(|k| k.1).sum();
    // Split the list where the prefix weight is closest to half.
    let mut acc = 0.0;
    let mut k = 1;
    let mut best = f64::MAX;
    for i in 1..rooms.len() {
        acc += rooms[i - 1].1;
        let d = (acc / total - 0.5).abs();
        if d < best {
            best = d;
            k = i;
        }
    }
    let frac = rooms[..k].iter().map(|x| x.1).sum::<f64>() / total;
    let horizontal = r.w >= r.h; // cut across the long side
    let span = if horizontal { r.w } else { r.h };
    let jitter = (rng.next_f64() - 0.5) * 0.12;
    let mut cut = crate::core::round(span as f64 * (frac + jitter)) as usize;
    // Sides at least 2 squares where the span allows it.
    let edge = if span >= 4 { 2 } else { 1 };
    cut = cut.clamp(edge, span - edge);
    // Keep the stair block whole.
    let (s0, s1, o0, o1, r0, r1) = if horizontal { (st[0], st[0] + st[2], st[1], st[1] + st[3], r.y, r.y + r.h) } else { (st[1], st[1] + st[3], st[0], st[0] + st[2], r.x, r.x + r.w) };
    let base_pos = if horizontal { r.x } else { r.y };
    let pos = base_pos + cut;
    if o0 < r1 && o1 > r0 && pos > s0 && pos < s1 {
        let (lo, hi) = (s0 - base_pos, s1 - base_pos);
        let cand = if cut - lo <= hi - cut { lo } else { hi };
        if cand >= 2 && span - cand >= 2 {
            cut = cand;
        } else if hi >= 2 && span - hi >= 2 {
            cut = hi;
        } else if lo >= 2 && span - lo >= 2 {
            cut = lo;
        }
    }
    let (a, b) = if horizontal {
        (R { x: r.x, y: r.y, w: cut, h: r.h }, R { x: r.x + cut, y: r.y, w: r.w - cut, h: r.h })
    } else {
        (R { x: r.x, y: r.y, w: r.w, h: cut }, R { x: r.x, y: r.y + cut, w: r.w, h: r.h - cut })
    };
    bsp(a, &rooms[..k], st, rng, out, base);
    bsp(b, &rooms[k..], st, rng, out, base + k);
}

/// Every room is one piece (its squares joined side to side): a cell block laid on a footprint it
/// doesn't fit (a keep drawn round its towers by hand) can leave a corridor in two.
pub(super) fn rooms_whole(cells: &[i16], nx: usize, ny: usize) -> bool {
    let mut seen = vec![false; nx * ny];
    let mut rooms_seen: Vec<i16> = Vec::new();
    for k0 in 0..nx * ny {
        if cells[k0] < 0 || seen[k0] {
            continue;
        }
        if rooms_seen.contains(&cells[k0]) {
            return false;
        }
        rooms_seen.push(cells[k0]);
        let mut stack = vec![k0];
        seen[k0] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < nx && b < ny && !seen[b * nx + a] && cells[b * nx + a] == cells[k0] {
                    seen[b * nx + a] = true;
                    stack.push(b * nx + a);
                }
            }
        }
    }
    true
}

pub(super) fn merge_fragments(mut cells: Vec<i16>, nx: usize, ny: usize) -> Vec<i16> {
    loop {
        // Connected pieces of every room.
        let mut piece = vec![usize::MAX; nx * ny];
        let mut pieces: Vec<(i16, Vec<usize>)> = Vec::new();
        for k in 0..nx * ny {
            if cells[k] < 0 || piece[k] != usize::MAX {
                continue;
            }
            let id = pieces.len();
            let mut members = vec![k];
            piece[k] = id;
            let mut i = 0;
            while i < members.len() {
                let q = members[i];
                i += 1;
                let (x, y) = ((q % nx) as isize, (q / nx) as isize);
                for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                    let (a, b) = (x + dx, y + dy);
                    if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize {
                        continue;
                    }
                    let r = b as usize * nx + a as usize;
                    if cells[r] == cells[k] && piece[r] == usize::MAX {
                        piece[r] = id;
                        members.push(r);
                    }
                }
            }
            pieces.push((cells[k], members));
        }
        // A piece that is not its room's largest.
        let largest = |room: i16| pieces.iter().filter(|p| p.0 == room).map(|p| p.1.len()).max().unwrap_or(0);
        let Some(p) = pieces.iter().find(|p| p.1.len() < largest(p.0) || (p.1.len() == largest(p.0) && pieces.iter().any(|q| q.0 == p.0 && q.1.len() == p.1.len() && q.1[0] < p.1[0]))) else {
            return cells;
        };
        // Its longest shared wall with another room.
        let mut shared: std::collections::BTreeMap<i16, usize> = Default::default();
        for &q in &p.1 {
            let (x, y) = ((q % nx) as isize, (q / nx) as isize);
            for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (x + dx, y + dy);
                if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize {
                    continue;
                }
                let r = cells[b as usize * nx + a as usize];
                if r >= 0 && r != p.0 {
                    *shared.entry(r).or_insert(0) += 1;
                }
            }
        }
        let Some((&to, _)) = shared.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))) else {
            // An isolated piece (the footprint itself is in pieces): leave it.
            return cells;
        };
        for &q in &p.1 {
            cells[q] = to;
        }
    }
}

fn compact(cells: Vec<i16>, rooms: Vec<Room>) -> (Vec<i16>, Vec<Room>) {
    let (cells, rooms, _) = compact_map(cells, rooms);
    (cells, rooms)
}

/// `compact`, with each old room's new index (-1 gone).
pub(super) fn compact_map(cells: Vec<i16>, rooms: Vec<Room>) -> (Vec<i16>, Vec<Room>, Vec<i16>) {
    let mut used = vec![false; rooms.len()];
    for &c in &cells {
        if c >= 0 {
            used[c as usize] = true;
        }
    }
    let mut map = vec![-1i16; rooms.len()];
    let mut out = Vec::new();
    for (i, r) in rooms.into_iter().enumerate() {
        if used[i] {
            map[i] = out.len() as i16;
            out.push(r);
        }
    }
    (cells.into_iter().map(|c| if c >= 0 { map[c as usize] } else { -1 }).collect(), out, map)
}

pub(super) fn finish_rooms(rooms: &mut [Room], cells: &[i16], nx: usize) {
    let mut sum = vec![(0.0f64, 0.0f64, 0usize); rooms.len()];
    for (k, &c) in cells.iter().enumerate() {
        if c >= 0 {
            let s = &mut sum[c as usize];
            s.0 += (k % nx) as f64 + 0.5;
            s.1 += (k / nx) as f64 + 0.5;
            s.2 += 1;
        }
    }
    for (r, s) in rooms.iter_mut().zip(sum) {
        r.squares = s.2;
        if s.2 > 0 {
            r.center = [(s.0 / s.2 as f64) as f32, (s.1 / s.2 as f64) as f32];
        }
    }
}
