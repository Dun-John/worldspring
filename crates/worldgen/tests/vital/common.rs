//! Helpers shared by the test modules: the test world, distances, interior checks.

use super::*;

pub(super) fn world_json(seed: u32) -> String {
    serde_json::to_string(&WorldFile { seed, ..Default::default() }).unwrap()
}

/// A layout's roofed buildings by 100-ft bucket, for "does this door open into a neighbour's wall".
pub(super) fn building_buckets(l: &worldgen::town::Layout) -> std::collections::HashMap<(i64, i64), Vec<usize>> {
    use worldgen::town::Structure;
    let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (k, b) in l.buildings.iter().enumerate() {
        if b.structure != Structure::Roofed {
            continue;
        }
        let (x0, y0, x1, y1) = b.poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        for by in (y0 / 100.0).floor() as i64..=(y1 / 100.0).floor() as i64 {
            for bx in (x0 / 100.0).floor() as i64..=(x1 / 100.0).floor() as i64 {
                buckets.entry((bx, by)).or_default().push(k);
            }
        }
    }
    buckets
}

/// The rules every interior keeps (`interiors_guarantee`): building or tower `id` of layout `si`
/// (`l`). Returns how long it took (ms), whether it is a drinking place, and its prison cells.
pub(super) fn check_interior(world: &World, t0: &worldgen::t0::T0, l: &worldgen::town::Layout, si: usize, id: &str, buckets: &std::collections::HashMap<(i64, i64), Vec<usize>>) -> (f64, bool, usize) {
    use worldgen::interior;
    use worldgen::town::geom;
    let (mut drinking, mut cells) = (false, 0);
    let bi = &id;
    let t = std::time::Instant::now();
    let it = interior::generate_id(&world, &t0, &id).expect("enterable");
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let (nx, ny) = (it.nx, it.ny);
    let tag = format!("{bi} ({})", it.function);
    // Shapes are only drawn: a room keeps one while its squares are the shape's. The outer wall
    // drawn round a footprint off the grid (`outlines`: on the footprint, pushed out where squares
    // reach past it) leaves every square's middle a tenth of a square inside its inner face, keeps
    // within a square of them, and never cuts a generated stair block.
    for lv in &it.levels {
        for (ri, r) in lv.rooms.iter().enumerate() {
            assert!(r.shape == 0 || interior::shape::fits(&lv.cells, nx, ri as i16, r.shape), "{tag} {}: the {} is drawn shaped ({}) off its squares", lv.name, r.kind, r.shape);
        }
    }
    if !it.outlines.is_empty() {
        // How far a point lies inside the outline (its parts' best), negative outside.
        let inset = |x: f32, y: f32| {
            it.outlines
                .iter()
                .map(|o| {
                    (0..o.len())
                        .map(|k| {
                            let (a, b) = (o[k], o[(k + 1) % o.len()]);
                            ((b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])) / (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9)
                        })
                        .fold(f32::MAX, f32::min)
                })
                .fold(f32::MIN, f32::max)
        };
        for lv in &it.levels {
            for k in (0..nx * ny).filter(|&k| lv.cells[k] >= 0) {
                let d = inset((k % nx) as f32 + 0.5, (k / nx) as f32 + 0.5);
                assert!(d >= interior::WALL_HALF as f32 + 0.1 - 1e-3, "{tag} {}: square {},{} only {d:.2} inside its outer wall", lv.name, k % nx, k / nx);
            }
            // (Unless no square of the building is clear of the wall.)
            let st = it.stairs;
            let room = (0..nx * ny).any(|k| {
                let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                lv.cells[k] >= 0 && [(i, j), (i + 1.0, j), (i, j + 1.0), (i + 1.0, j + 1.0)].iter().all(|c| inset(c.0, c.1) >= interior::WALL_HALF as f32 - 1e-3)
            });
            if lv.has_stairs && room && !world.file.edits.designs.contains_key(id) {
                for (x, y) in [(st[0], st[1]), (st[0] + st[2], st[1]), (st[0], st[1] + st[3]), (st[0] + st[2], st[1] + st[3])] {
                    let d = inset(x as f32, y as f32);
                    assert!(d >= interior::WALL_HALF as f32 - 1e-3, "{tag} {}: the outer wall cuts the stairs {st:?} ({d:.2} in)", lv.name);
                }
            }
        }
        let g = &it.levels[it.entry_level];
        // (A keep's towers in grid units: a square of wall round each one's room.)
        let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok()).and_then(|k| l.building(k));
        let towers: Vec<Vec<[f64; 2]>> = own
            .map(|b| {
                b.parts
                    .iter()
                    .filter(|p| matches!(p.kind, worldgen::town::PartKind::Tower { .. }))
                    .map(|p| p.poly.iter().map(|q| {
                        let d = geom::sub(*q, it.origin);
                        [geom::dot(d, it.axis) / 5.0, geom::dot(d, it.across) / 5.0]
                    }).collect())
                    .collect()
            })
            .unwrap_or_default();
        let by_tower = |v: &[f32; 2]| {
            let p = [v[0] as f64, v[1] as f64];
            towers.iter().any(|t| geom::contains(t, p) || (0..t.len()).any(|k| geom::seg_dist(p, t[k], t[(k + 1) % t.len()]) <= 1.0))
        };
        for v in it.outlines.iter().flatten() {
            let near = (0..nx * ny).filter(|&k| g.cells[k] >= 0).map(|k| {
                let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                let (dx, dy) = ((i - v[0]).max(v[0] - i - 1.0).max(0.0), (j - v[1]).max(v[1] - j - 1.0).max(0.0));
                (dx * dx + dy * dy).sqrt()
            });
            let d = near.fold(f32::MAX, f32::min);
            // (A keep's tower: a square of wall round its room.)
            let most = if by_tower(v) { 1.5 } else { 1.0 };
            assert!(d <= most, "{tag}: its outer wall stands {d:.2} squares off its floor at {v:?}");
        }
    }
    // (Generated ones; a design may have no cellar, a yurt has one storey.)
    // (R5 towns: a steppe town's round felt hall, as a yurt.)
    let yurt = it.levels.len() == 1 && it.levels[0].rooms.iter().any(|r| matches!(r.kind, "yurt" | "felt hall" | "felt sanctuary" | "felt common room")) && !it.levels[0].has_stairs && it.levels[0].windows.is_empty();
    // (R5 towns: a cemetery's tomb, one chamber on one storey.)
    let tomb = it.levels.len() == 1 && it.levels[0].rooms.iter().any(|r| r.kind == "tomb chamber") && !it.levels[0].has_stairs;
    assert!(it.levels.len() >= 2 || yurt || tomb || world.file.edits.designs.contains_key(id), "{tag}: only {} level", it.levels.len());
    // A building's interior as a design builds it again exactly, and keeps the designer's rules.
    if id.starts_with("b:") {
        let (d, _) = interior::design::design_of(world, t0, id, false).unwrap();
        let (built, problems) = interior::design::problems(world, t0, id, &d).unwrap();
        assert!(problems.is_empty(), "{tag}: as a design: {problems:?}");
        assert_eq!(serde_json::to_string(&built).unwrap(), serde_json::to_string(&it).unwrap(), "{tag}: built again from its design, it differs");
    }
    // One chapel per building; each spiral stair on the same square on a run of two levels or more
    // (a keep's tower rises as high as the piece it stands on, and a storey more).
    let chapels = it.levels.iter().flat_map(|lv| lv.rooms.iter()).filter(|r| r.kind == "chapel").count();
    assert!(chapels <= 1, "{tag}: {chapels} chapels");
    let spirals: Vec<Vec<(u16, u16)>> = it.levels.iter().map(|lv| lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| (f.x, f.y)).collect()).collect();
    let mut at: Vec<(u16, u16)> = spirals.iter().flatten().copied().collect();
    at.sort();
    at.dedup();
    for s in at {
        let on: Vec<usize> = (0..spirals.len()).filter(|&li| spirals[li].contains(&s)).collect();
        assert!(on.len() >= 2, "{tag}: a spiral stair at {s:?} on one level only");
        assert!(on.windows(2).all(|w| w[1] == w[0] + 1), "{tag}: the spiral stair at {s:?} skips a level");
    }
    // Cells: under 15 ft a side, each with a door onto a corridor or guardroom (never
    // through another cell).
    for lv in &it.levels {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, r)| r.kind == "cell") {
            let sq: Vec<usize> = (0..it.nx * it.ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % it.nx), a.1.max(k % it.nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / it.nx), a.1.max(k / it.nx)));
            assert!(x1 - x0 < 3 && y1 - y0 < 3, "{tag} {}: a {}x{} ft cell", lv.name, 5 * (x1 - x0 + 1), 5 * (y1 - y0 + 1));
            let doors: Vec<i16> = lv.doors.iter().filter(|d| d.rooms.contains(&(ri as i16))).map(|d| if d.rooms[0] == ri as i16 { d.rooms[1] } else { d.rooms[0] }).collect();
            assert!(!doors.is_empty(), "{tag} {}: a cell with no door", lv.name);
            assert!(doors.iter().all(|&o| o < 0 || lv.rooms[o as usize].kind != "cell"), "{tag} {}: a cell entered through another cell", lv.name);
            cells += 1;
        }
    }
    // Drinking places serve from a bar to tables on the ground floor.
    const DRINKING: [&str; 10] = ["Inn", "Tavern", "Alehouse", "Fine dining", "Brewery", "Winery", "Distillery", "Gambling den", "Smugglers' den", "Caravanserai"];
    if DRINKING.contains(&it.function) {
        let g = &it.levels[it.entry_level];
        let bars = g.furniture.iter().filter(|f| f.kind == "bar").count();
        let tables = g.furniture.iter().filter(|f| f.kind == "table" || f.kind == "booth_table").count();
        assert!(bars >= 1 && tables >= 2, "{tag}: {bars} bars and {tables} tables on the ground floor");
        // Guests come in through the taproom and go upstairs from it.
        let public = |r: i16| r >= 0 && matches!(g.rooms[r as usize].kind, "common room" | "taproom");
        let front = g.doors.iter().find(|d| d.kind == "front").expect("front door");
        assert!(public(front.rooms[0]), "{tag}: the front door opens into the {}", g.rooms[front.rooms[0] as usize].kind);
        let stair_room = g.cells[it.stairs[1] * it.nx + it.stairs[0]];
        assert!(public(stair_room), "{tag}: the stairs are in the {}", g.rooms[stair_room.max(0) as usize].kind);
        drinking = true;
    }
    // (Roadmap 7 P8) A round tower's winding stairs (`winder` squares, a level's runs in climbing
    // order, each run's squares side by side): each run of two squares or more comes out on the
    // level above, its top square clear floor there where a run of that level starts (a lone square
    // on the stair's top level); each lone square the top of a run below; a run starts from nothing
    // below only on the lowest level with one or the entry level; without a stair block, a run on
    // every level from the lowest up. The flood on a level starts from its runs.
    let runs: Vec<Vec<Vec<usize>>> = it.levels.iter().map(|lv| winder_runs(lv, nx)).collect();
    let lowest = runs.iter().position(|r| !r.is_empty());
    if let Some(lo) = lowest {
        for (li, lv) in it.levels.iter().enumerate() {
            for r in &runs[li] {
                let (foot, top) = (r[0], *r.last().unwrap());
                if r.len() >= 2 {
                    let up = it.levels.get(li + 1);
                    assert!(up.is_some_and(|u| u.cells[top] >= 0 && !u.furniture.iter().any(|f| f.blocks_move && f.x as usize == top % nx && f.y as usize == top / nx)), "{tag} {}: the winding stair comes out off the floor", lv.name);
                    assert!(runs[li + 1].iter().any(|q| q[0] == top), "{tag} {}: the winding stair doesn't go on where it comes out", lv.name);
                }
                let below = li > 0 && runs[li - 1].iter().any(|q| q.len() >= 2 && *q.last().unwrap() == foot);
                assert!(below || (r.len() >= 2 && (li == lo || li == it.entry_level)), "{tag} {}: a winding stair from nowhere at {foot} (li {li} lo {lo} entry {} runs {:?} below {:?})", lv.name, it.entry_level, runs[li], if li > 0 { runs[li - 1].clone() } else { vec![] });
            }
            assert!(li <= lo || !runs[li].is_empty() || it.levels.iter().any(|l| l.has_stairs), "{tag}: no winding stair up to {}", lv.name);
        }
    }
    let st = it.stairs;
    let on_stairs = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
    for lv in &it.levels {
        if lv.has_stairs {
            for j in st[1]..st[1] + st[3] {
                for i in st[0]..st[0] + st[2] {
                    assert!(lv.cells[j * nx + i] >= 0, "{tag} {}: stairs outside the building", lv.name);
                }
            }
        }
        let fronts = lv.doors.iter().filter(|d| d.kind == "front").count();
        assert_eq!(fronts, if lv.z == 0 { 1 } else { 0 }, "{tag} {}: {fronts} front doors", lv.name);
        // The front door opens onto open ground, not into a neighbouring building.
        if let Some(d) = lv.doors.iter().find(|d| d.kind == "front") {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
            let outside = sq.into_iter().find(|&(i, j)| i < 0 || j < 0 || i >= nx as isize || j >= ny as isize || lv.cells[j as usize * nx + i as usize] < 0).expect("door on an outside wall");
            let p = interior::to_world(&it, outside.0 as f64 + 0.5, outside.1 as f64 + 0.5);
            let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok());
            let hit = buckets.get(&((p[0] / 100.0).floor() as i64, (p[1] / 100.0).floor() as i64)).into_iter().flatten().find(|&&k| Some(l.buildings[k].id as usize) != own && geom::contains(&l.buildings[k].poly, p));
            assert!(hit.is_none(), "{tag}: the front door opens into building {}", hit.unwrap());
        }
        // Squares either side of each door.
        let door_sq = |d: &interior::Door| -> Vec<(isize, isize)> {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            if x0 == x1 { vec![(x0 - 1, y0), (x0, y0)] } else { vec![(x0, y0 - 1), (x0, y0)] }
        };
        let inb = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny;
        let mut blocked = vec![false; nx * ny];
        for f in &lv.furniture {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    assert!(!lv.has_stairs || !on_stairs(i, j), "{tag} {}: {} on the stairs", lv.name, f.name);
                    // (A platform, walked over, may lie in a doorway.)
                    assert!(interior::hall::is_platform(f.kind) || !lv.doors.iter().any(|d| door_sq(d).contains(&(i as isize, j as isize))), "{tag} {}: {} in a doorway", lv.name, f.name);
                    if f.blocks_move {
                        blocked[j * nx + i] = true;
                    }
                }
            }
        }
        // Flood over open floor from the stairs, crossing between rooms only at doors.
        let door_between = |a: (usize, usize), b: (usize, usize)| {
            lv.doors.iter().any(|d| {
                let s = door_sq(d);
                s.contains(&(a.0 as isize, a.1 as isize)) && s.contains(&(b.0 as isize, b.1 as isize))
            })
        };
        // From the stairs (or, above the main stairs, every spiral stair; a ground floor with
        // neither, a designed one-storey building without a cellar, from its doors).
        let mut starts: Vec<usize> = if lv.has_stairs {
            vec![st[1] * nx + st[0]]
        } else {
            lv.furniture.iter().filter(|f| matches!(f.kind, "spiral_stair" | "winder")).map(|f| f.y as usize * nx + f.x as usize).collect()
        };
        if starts.is_empty() && lv.z == 0 {
            starts = lv.doors.iter().filter(|d| d.rooms[1] < 0).flat_map(door_sq).filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] >= 0).collect();
        }
        let mut seen = vec![false; nx * ny];
        let mut stack = starts.clone();
        for &k in &starts {
            seen[k] = true;
        }
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i as isize + di, j as isize + dj);
                if !inb((a, b)) {
                    continue;
                }
                let q = b as usize * nx + a as usize;
                if seen[q] || lv.cells[q] < 0 || blocked[q] {
                    continue;
                }
                if lv.cells[q] != lv.cells[k] && !door_between((i, j), (a as usize, b as usize)) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        for d in &lv.doors {
            let reached = door_sq(d).into_iter().filter(|&p| inb(p)).any(|(i, j)| seen[j as usize * nx + i as usize]);
            assert!(reached, "{tag} {}: a {} door is cut off", lv.name, d.kind);
        }
        // Every spiral stair reached from the stairs, and from its room's doors with nothing but a
        // rug in the way (not a bed one could step over): the way on up to the rooms above it.
        let mut occupied = vec![false; nx * ny];
        for f in lv.furniture.iter().filter(|f| f.kind != "rug" && f.kind != "spiral_stair") {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    occupied[j * nx + i] = true;
                }
            }
        }
        for f in lv.furniture.iter().filter(|f| f.kind == "spiral_stair") {
            let k0 = f.y as usize * nx + f.x as usize;
            assert!(seen[k0], "{tag} {}: the spiral stair at ({}, {}) is cut off", lv.name, f.x, f.y);
            let r = lv.cells[k0];
            let mut near: Vec<usize> = lv.doors.iter().flat_map(door_sq).filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] == r && seen[k]).collect();
            if near.is_empty() {
                continue;
            }
            let mut got = vec![false; nx * ny];
            for &k in &near {
                got[k] = true;
            }
            while let Some(k) = near.pop() {
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    let q = b.wrapping_mul(nx).wrapping_add(a);
                    if a < nx && b < ny && !got[q] && lv.cells[q] == r && !occupied[q] {
                        got[q] = true;
                        near.push(q);
                    }
                }
            }
            assert!(got[k0], "{tag} {}: furniture shuts the spiral stair at ({}, {}) off from its room's doors", lv.name, f.x, f.y);
        }
        for (ri, room) in lv.rooms.iter().enumerate() {
            let open = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && !blocked[k]);
            let reached = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && seen[k]);
            if !interior::walkable(room.kind) {
                // A hall's height left open, a roof's slopes: no door, no floor, nothing on them.
                assert!(!reached && !lv.doors.iter().any(|d| d.rooms.contains(&(ri as i16))), "{tag} {}: the {} has a way in", lv.name, room.kind);
                assert!(!lv.furniture.iter().any(|f| (f.y..f.y + f.h).any(|j| (f.x..f.x + f.w).any(|i| lv.cells[j as usize * nx + i as usize] == ri as i16))), "{tag} {}: something stands on the {}", lv.name, room.kind);
                continue;
            }
            assert!(!open || reached, "{tag} {}: the {} is unreachable", lv.name, room.kind);
        }
        // Floors with corridors: no room is reached through more than one other room (a
        // guardroom is a way through, as for cells; so is the public room the front door opens
        // into, as `connect` lays it out: a taproom, a reading room, a great hall, counted from
        // as well on a floor with a corridor).
        let entry = lv.doors.iter().find(|d| d.kind == "front").and_then(|d| d.rooms.iter().copied().find(|&r| r >= 0));
        let circ = |r: usize| interior::is_way_through(lv.rooms[r].kind);
        if lv.rooms.len() >= 8 && (0..lv.rooms.len()).any(circ) {
            let mut hops = vec![usize::MAX; lv.rooms.len()];
            let mut q = std::collections::VecDeque::new();
            for r in (0..lv.rooms.len()).filter(|&r| circ(r) || entry == Some(r as i16)) {
                hops[r] = 0;
                q.push_back(r);
            }
            while let Some(r) = q.pop_front() {
                for d in &lv.doors {
                    let [a, b] = d.rooms;
                    let other = if a == r as i16 { b } else if b == r as i16 { a } else { continue };
                    if other >= 0 && hops[other as usize] == usize::MAX {
                        hops[other as usize] = hops[r] + 1;
                        q.push_back(other as usize);
                    }
                }
            }
            for (ri, room) in lv.rooms.iter().enumerate() {
                if room.squares > 0 && hops[ri] != usize::MAX && interior::walkable(room.kind) {
                    assert!(hops[ri] <= 2, "{tag} {}: the {} is {} rooms from a corridor", lv.name, room.kind, hops[ri] - 1);
                }
            }
        }
    }
    (ms, drinking, cells)
}

pub(super) fn geom_dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// A level's winding stair runs (`winder` squares in climbing order, a new run wherever the next
/// square isn't beside the last).
pub(super) fn winder_runs(lv: &worldgen::interior::Level, nx: usize) -> Vec<Vec<usize>> {
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for f in lv.furniture.iter().filter(|f| f.kind == "winder") {
        let k = f.y as usize * nx + f.x as usize;
        match runs.last_mut() {
            Some(r) if r.last().is_some_and(|&q| (q % nx).abs_diff(k % nx) <= 1 && (q / nx).abs_diff(k / nx) <= 1) => r.push(k),
            _ => runs.push(vec![k]),
        }
    }
    runs
}
