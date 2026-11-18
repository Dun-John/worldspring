//! The rules a design is held to (`check`, `key_order`): see the module doc above.
use super::*;

/// Squares reached from `start` over floor: between rooms only through `doors` (any step on a
/// natural level), never onto `blocked` ones.
pub(super) fn flood(cells: &[i16], nx: usize, ny: usize, start: usize, natural: bool, doors: &[(usize, usize)], blocked: &dyn Fn(usize) -> bool) -> Vec<bool> {
    let mut seen = vec![false; nx * ny];
    if cells[start] < 0 {
        return seen;
    }
    let door: crate::core::hash::FastSet<(usize, usize)> = doors.iter().copied().collect();
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(k) = stack.pop() {
        for q in neighbours(k, nx, ny) {
            if seen[q] || cells[q] < 0 || blocked(q) {
                continue;
            }
            if !natural && cells[q] != cells[k] && !door.contains(&(k.min(q), k.max(q))) {
                continue;
            }
            seen[q] = true;
            stack.push(q);
        }
    }
    seen
}

/// Squares of a level that are reached from `start` (`reached`: by any walk) but not without climbing a sheer edge
/// (`height::Floor::sheer`): the walk of `r5_no_climb` in tests/vital/underground.rs.
pub fn climb_only(lv: &crate::interior::Level, nx: usize, ny: usize, start: usize, blocks: &[bool], doors: &[(usize, usize)], reached: &[bool]) -> Vec<usize> {
    if lv.floor.is_empty() || lv.floor.len() != nx * ny {
        return Vec::new();
    }
    let f = crate::interior::height::of_level(lv, nx);
    let door: crate::core::hash::FastSet<(usize, usize)> = doors.iter().copied().collect();
    let mut seen = vec![false; nx * ny];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(k) = stack.pop() {
        for q in neighbours(k, nx, ny) {
            if seen[q] || lv.cells[q] < 0 || blocks[q] || f.sheer(k, q) > 0.0 {
                continue;
            }
            if !lv.natural && lv.cells[q] != lv.cells[k] && !door.contains(&(k.min(q), k.max(q))) {
                continue;
            }
            seen[q] = true;
            stack.push(q);
        }
    }
    (0..nx * ny).filter(|&q| reached[q] && !seen[q]).collect()
}

/// The rules every site keeps (generated ones are held to them in tests/vital/underground.rs): the top
/// level's one way in (under the entrance, `entry`, if given); one way down from each level
/// but the deepest, on the square of the one way up on the level below; every item on floor,
/// none on another; every square reached from the way onto its level, around whatever blocks
/// movement. A site without one boss chamber on its deepest level is noted, not refused, and so
/// are keys out of order (`key_order`).
pub fn check(it: &Interior, entry: Option<[u16; 2]>) -> Vec<Problem> {
    let (nx, ny) = (it.nx, it.ny);
    let n = it.levels.len();
    let mut out = Vec::new();
    let mut put = |level: usize, at: Option<usize>, text: String, blocking: bool| {
        out.push(Problem {
            level,
            at: at.map(|k| [(k % nx) as u16, (k / nx) as u16]),
            text: format!("{}: {text}", depth_tag(n, level)),
            blocking,
        });
    };
    if n == 0 {
        put(0, None, "no levels".into(), true);
        return out;
    }
    let at = |li: usize, kind: &str| {
        it.levels[li]
            .furniture
            .iter()
            .filter(|f| f.kind == kind)
            .map(|f| f.y as usize * nx + f.x as usize)
            .collect::<Vec<_>>()
    };
    for li in 0..n {
        let lv = &it.levels[li];
        let top = li + 1 == n;
        // The ways.
        let (exit, up, down) = (at(li, "exit"), at(li, "up"), at(li, "down"));
        if top {
            match exit.as_slice() {
                [] => put(li, None, "no way in from the surface".into(), true),
                [k] => {
                    if let Some(e) = entry
                        && [(k % nx) as u16, (k / nx) as u16] != e
                    {
                        put(li, Some(*k), format!("the way in must stay under the entrance, at {},{}", e[0], e[1]), true);
                    }
                }
                _ => put(li, Some(exit[1]), format!("{} ways in from the surface (one only)", exit.len()), true),
            }
            if !up.is_empty() {
                put(li, Some(up[0]), "a way up on the top level".into(), true);
            }
        } else {
            if !exit.is_empty() {
                put(li, Some(exit[0]), "a way in from the surface below the top level".into(), true);
            }
            let above = at(li + 1, "down");
            // (A landing set by hand is where the way up sits, instead of under the way down above.)
            let landing = lv.landing.map(|l| l[1] as usize * nx + l[0] as usize).filter(|&k| k < nx * ny);
            if let Some(k) = landing
                && lv.cells[k] < 0
            {
                put(li, Some(k), "the landing is not on the floor".into(), true);
            }
            match up.as_slice() {
                [] => put(li, above.first().copied(), "no way up".into(), true),
                [k] if landing.is_some_and(|l| l != *k) => put(li, Some(*k), format!("the way up is not at the landing, {},{}", landing.unwrap() % nx, landing.unwrap() / nx), true),
                [k] if landing.is_none() && above.len() == 1 && above[0] != *k => put(li, Some(*k), "the way up is not under the way down from the level above".into(), true),
                [_] => {}
                _ => put(li, Some(up[1]), format!("{} ways up (one only)", up.len()), true),
            }
        }
        if li == 0 {
            if !down.is_empty() {
                put(li, Some(down[0]), "a way down from the deepest level".into(), true);
            }
        } else if down.is_empty() {
            put(li, None, "no way down".into(), true);
        } else if down.len() > 1 {
            put(li, Some(down[1]), format!("{} ways down (one only)", down.len()), true);
        }
        // Items on floor, none on another.
        // (A square holds one item, or terrain with a span over it: `held` bits 1 terrain, 2 span, 4 anything else; a
        // raised floor, 8 (`height::RAISING`: a dais, a platform), may bear one item.)
        let mut held = vec![0u8; nx * ny];
        let blocks = blocked_squares(&lv.furniture, nx, ny);
        let mut clash = None;
        for f in &lv.furniture {
            let bit = if TERRAIN.contains(&f.kind) { 1 } else if SPANS.contains(&f.kind) { 2 } else if crate::interior::height::RAISING.contains(&f.kind) { 8 } else { 4 };
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    if i >= nx || j >= ny {
                        continue;
                    }
                    let k = j * nx + i;
                    if lv.cells[k] < 0 {
                        put(li, Some(k), format!("the {} at {i},{j} is in the rock", f.name), true);
                    }
                    let bars = match bit {
                        1 => 1 | 4 | 8,
                        2 => 2 | 4 | 8,
                        4 => 1 | 2 | 4,
                        _ => 1 | 2 | 8,
                    };
                    if held[k] & bars != 0 {
                        clash.get_or_insert((k, f.name, f.x, f.y));
                    }
                    held[k] |= bit;
                }
            }
        }
        if let Some((k, name, fx, fy)) = clash {
            put(li, Some(k), format!("the {name} at {fx},{fy} stands on something else (they overlap at {},{})", k % nx, k / nx), true);
        }
        // Everything reached from the way onto the level.
        let Some(start) = (if top { exit.first() } else { up.first() }).copied() else {
            continue;
        };
        if lv.cells[start] < 0 || blocks[start] {
            put(li, Some(start), "the way onto the level is not open floor".into(), true);
            continue;
        }
        let doors: Vec<(usize, usize)> = lv
            .doors
            .iter()
            .map(|d| {
                let (x, y) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize);
                if d.a[1] == d.b[1] {
                    (y.saturating_sub(1) * nx + x, y * nx + x)
                } else {
                    (y * nx + x.saturating_sub(1), y * nx + x)
                }
            })
            .collect();
        let seen = flood(&lv.cells, nx, ny, start, lv.natural, &doors, &|q| blocks[q]);
        // Each stretch cut off, once.
        let mut lost = vec![false; nx * ny];
        let mut cut = 0;
        for k in 0..nx * ny {
            if lv.cells[k] < 0 || blocks[k] || seen[k] || lost[k] {
                continue;
            }
            let part = flood(&lv.cells, nx, ny, k, lv.natural, &doors, &|q| blocks[q]);
            let size = part.iter().filter(|&&s| s).count();
            for (q, s) in part.iter().enumerate() {
                lost[q] |= *s;
            }
            cut += 1;
            if cut <= 8 {
                let room = it.levels[li].rooms[lv.cells[k] as usize].kind;
                put(
                    li,
                    Some(k),
                    format!("{size} square{} of the {room} at {},{} can't be reached", if size == 1 { "" } else { "s" }, k % nx, k / nx),
                    true,
                );
            }
        }
        if cut > 8 {
            put(li, None, format!("{} more stretches can't be reached", cut - 8), true);
        }
        // Squares that can be walked to, but only by climbing a sheer edge (item 118's rule).
        let by_climb = climb_only(lv, nx, ny, start, &blocks, &doors, &seen);
        let mut said: Vec<i16> = Vec::new();
        for &k in &by_climb {
            let room = lv.cells[k];
            if said.contains(&room) || said.len() >= 8 {
                continue;
            }
            said.push(room);
            let f = crate::interior::height::of_level(lv, nx);
            let count = by_climb.iter().filter(|&&q| lv.cells[q] == room).count();
            put(
                li,
                Some(k),
                format!(
                    "{count} square{} of the {} at {},{} ({} ft up) can be reached only by climbing a sheer edge (steps, a climb_ladder or a rubble_slope must join them to the floor below, or lower the platform)",
                    if count == 1 { "" } else { "s" },
                    lv.rooms[room as usize].kind,
                    k % nx,
                    k / nx,
                    f.ft[k]
                ),
                true,
            );
        }
        if lv.cells.iter().all(|&c| c < 0) {
            put(li, None, "no floor".into(), true);
        }
    }
    let boss: Vec<usize> = it
        .levels
        .iter()
        .enumerate()
        .flat_map(|(li, lv)| lv.rooms.iter().filter(|r| r.kind == BOSS && r.squares > 0).map(move |_| li))
        .collect();
    match boss.as_slice() {
        [] => put(0, None, "no boss chamber".into(), false),
        [0] => {}
        [li] => put(*li, None, "the boss chamber is not on the deepest level".into(), false),
        _ => put(boss[1], None, format!("{} boss chambers", boss.len()), false),
    }
    out.extend(key_order(it));
    out
}

/// The two squares a level's door lies between (smaller first).
pub(crate) fn door_pair(d: &crate::interior::Door, nx: usize) -> (usize, usize) {
    let (x, y) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize);
    if d.a[1] == d.b[1] { (y.saturating_sub(1) * nx + x, y * nx + x) } else { (y * nx + x.saturating_sub(1), y * nx + x) }
}

/// Every key opens a lock, every lock has one key (`keys` and `locks` as `key_order` finds them: level, square, colour;
/// level, the squares the door lies between, colour). A building's design is checked this far too.
pub(crate) fn key_counts(keys: &[(usize, usize, u8)], locks: &[(usize, (usize, usize), u8)], nx: usize, put: &mut dyn FnMut(usize, Option<usize>, String)) {
    for &(li, k, c) in keys {
        if !(1..=MAX_LOCK).contains(&c) {
            put(li, Some(k), format!("the key at {},{} has no colour (variant 1-{MAX_LOCK})", k % nx, k / nx));
        } else if !locks.iter().any(|l| l.2 == c) {
            put(li, Some(k), format!("the key at {},{} opens no lock ({c})", k % nx, k / nx));
        }
    }
    for c in 1..=MAX_LOCK {
        let (Some(&(li, (a, _), _)), held) = (locks.iter().find(|l| l.2 == c), keys.iter().filter(|k| k.2 == c).count()) else { continue };
        match held {
            0 => put(li, Some(a), format!("lock {c} at {},{} has no key", a % nx, a / nx)),
            1 => {}
            m => put(li, Some(a), format!("lock {c} at {},{} has {m} keys (one only)", a % nx, a / nx)),
        }
    }
}

/// The keys' order: from the way in, a locked door is a wall until its key (a `key` item, its
/// `variant` the lock's colour) has been reached, across the levels by their ways down and up;
/// in the end every square reached without the locks must be reached with them, and every lock
/// has one key. Notes, not refusals (the DM can always open a door); generated sites keep it
/// (tests/vital/underground.rs).
pub fn key_order(it: &Interior) -> Vec<Problem> {
    let (nx, ny) = (it.nx, it.ny);
    let n = it.levels.len();
    let mut out = Vec::new();
    let mut put = |level: usize, at: Option<usize>, text: String| {
        out.push(Problem { level, at: at.map(|k| [(k % nx) as u16, (k / nx) as u16]), text: format!("{}: {text}", depth_tag(n, level)), blocking: false });
    };
    let keys: Vec<(usize, usize, u8)> = (0..n)
        .flat_map(|li| it.levels[li].furniture.iter().filter(|f| f.kind == "key").map(move |f| (li, f.y as usize * nx + f.x as usize, f.variant.unwrap_or(0))))
        .collect();
    let locks: Vec<(usize, (usize, usize), u8)> = (0..n).flat_map(|li| it.levels[li].doors.iter().filter(|d| d.lock != 0).map(move |d| (li, door_pair(d, nx), d.lock))).collect();
    if keys.is_empty() && locks.is_empty() {
        return out;
    }
    key_counts(&keys, &locks, nx, &mut put);
    // Squares reached with the locks `open`, from the way in.
    let at = |li: usize, kind: &str| it.levels[li].furniture.iter().filter(|f| f.kind == kind).map(|f| f.y as usize * nx + f.x as usize).collect::<Vec<_>>();
    let starts = if at(n - 1, "exit").is_empty() { at(n - 1, "up") } else { at(n - 1, "exit") };
    let blocks: Vec<Vec<bool>> = it
        .levels
        .iter()
        .map(|lv| blocked_squares(&lv.furniture, nx, ny))
        .collect();
    let doors: Vec<crate::core::hash::FastMap<(usize, usize), u8>> = it.levels.iter().map(|lv| lv.doors.iter().map(|d| (door_pair(d, nx), d.lock)).collect()).collect();
    let (downs, ups): (Vec<Vec<usize>>, Vec<Vec<usize>>) = ((0..n).map(|li| at(li, "down")).collect(), (0..n).map(|li| at(li, "up")).collect());
    let flood = |open: &[bool; 32]| {
        let mut seen = vec![vec![false; nx * ny]; n];
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for &k in &starts {
            if it.levels[n - 1].cells[k] >= 0 && !seen[n - 1][k] {
                seen[n - 1][k] = true;
                stack.push((n - 1, k));
            }
        }
        while let Some((li, k)) = stack.pop() {
            let lv = &it.levels[li];
            let mut next: Vec<(usize, usize)> = Vec::new();
            for q in neighbours(k, nx, ny) {
                if lv.cells[q] < 0 || blocks[li][q] {
                    continue;
                }
                // (On a natural level one steps anywhere but through a door still locked: a mine's powder store.)
                let door = doors[li].get(&(k.min(q), k.max(q)));
                if lv.natural {
                    if door.is_some_and(|&c| c != 0 && !open[c as usize & 31]) {
                        continue;
                    }
                } else if lv.cells[q] != lv.cells[k] {
                    match door {
                        Some(&c) if c == 0 || open[c as usize & 31] => {}
                        _ => continue,
                    }
                }
                next.push((li, q));
            }
            // (Down onto the way up below, up onto the way down above.)
            if li > 0 && downs[li].contains(&k) && it.levels[li - 1].cells[k] >= 0 {
                next.push((li - 1, k));
            }
            if li + 1 < n && ups[li].contains(&k) && it.levels[li + 1].cells[k] >= 0 {
                next.push((li + 1, k));
            }
            for (l2, q) in next {
                if !seen[l2][q] {
                    seen[l2][q] = true;
                    stack.push((l2, q));
                }
            }
        }
        seen
    };
    let mut open = [false; 32];
    let seen = loop {
        let seen = flood(&open);
        let mut more = false;
        for &(li, k, c) in &keys {
            if seen[li][k] && !open[c as usize & 31] {
                open[c as usize & 31] = true;
                more = true;
            }
        }
        if !more {
            break seen;
        }
    };
    let all = flood(&[true; 32]);
    for li in 0..n {
        let lost: Vec<usize> = (0..nx * ny).filter(|&k| all[li][k] && !seen[li][k]).collect();
        if let Some(&k) = lost.first() {
            let mut shut: Vec<String> = locks.iter().filter(|l| l.0 == li && !open[l.2 as usize & 31]).map(|l| format!("lock {} at {},{}", l.2, l.1.0 % nx, l.1.0 / nx)).collect();
            if shut.is_empty() {
                // (Cut off by a lock on another level: say which, and where its key lies if that is below it.)
                shut = locks
                    .iter()
                    .filter(|l| !open[l.2 as usize & 31])
                    .map(|l| {
                        let deeper = keys.iter().any(|q| q.2 == l.2 && q.0 < l.0);
                        format!("lock {} on {}{}", l.2, depth_tag(n, l.0), if deeper { ", whose key lies on a level below it" } else { "" })
                    })
                    .collect();
            }
            put(li, Some(k), format!("{} square{} can't be reached before their key ({})", lost.len(), if lost.len() == 1 { "" } else { "s" }, if shut.is_empty() { "a lock on another level".into() } else { shut.join(", ") }));
        }
    }
    out
}
