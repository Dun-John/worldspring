//! Rooms joined by doors (`connect`), wall runs and windows.

use super::*;

/// Doors (a spanning tree over room adjacency from the entrance or stairs, plus a few loops),
/// the front door, and the wall runs. Rooms one can't stand in (`walkable`) get no doors.
#[allow(clippy::too_many_arguments)]
pub(super) fn connect(cells: Vec<i16>, rooms: Vec<Room>, nx: usize, ny: usize, st: [usize; 4], front: usize, ground: bool, public: Option<&str>, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
    connect_with(cells, rooms, nx, ny, st, front, ground, public, ext_free, None, rng)
}

/// `connect`, with door hints.
#[allow(clippy::too_many_arguments)]
pub(super) fn connect_with(cells: Vec<i16>, rooms: Vec<Room>, nx: usize, ny: usize, st: [usize; 4], front: usize, ground: bool, public: Option<&str>, ext_free: &dyn Fn(isize, isize) -> bool, hints: Option<&Hints>, rng: &mut Pcg32) -> Level {
    let n = rooms.len();
    let at = |i: isize, j: isize| if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { cells[j as usize * nx + i as usize] };
    let on_stairs = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
    // Shared edges per room pair: (a, b) -> edges (x, y, vertical) where the edge lies
    // between square (x, y) and its right (vertical) or lower neighbour.
    let mut shared: std::collections::BTreeMap<(i16, i16), Vec<(usize, usize, bool)>> = Default::default();
    for j in 0..ny {
        for i in 0..nx {
            let a = cells[j * nx + i];
            if a < 0 {
                continue;
            }
            for (di, dj, vert) in [(1isize, 0isize, true), (0, 1, false)] {
                let bnb = at(i as isize + di, j as isize + dj);
                // Cells open only onto corridors and guardrooms, never through one another.
                if bnb >= 0 && bnb != a && !(rooms[a as usize].kind == "cell" && rooms[bnb as usize].kind == "cell") && walkable(rooms[a as usize].kind) && walkable(rooms[bnb as usize].kind) {
                    shared.entry((a.min(bnb), a.max(bnb))).or_default().push((i, j, vert));
                }
            }
        }
    }
    let edge_seg = |x: usize, y: usize, vert: bool| -> ([f32; 2], [f32; 2]) {
        if vert { ([(x + 1) as f32, y as f32], [(x + 1) as f32, (y + 1) as f32]) } else { ([x as f32, (y + 1) as f32], [(x + 1) as f32, (y + 1) as f32]) }
    };
    let mut doors: Vec<Door> = Vec::new();
    let mut door_edges: std::collections::BTreeSet<(usize, usize, bool)> = Default::default();
    // Spanning tree (BFS) from the stairs room.
    let stair_room = cells[st[1] * nx + st[0]].max(0) as usize;
    let mut seen = vec![false; n];
    let mut queue = std::collections::VecDeque::new();
    if n > 0 {
        seen[stair_room] = true;
        queue.push_back(stair_room);
    }
    let mut tree: Vec<(i16, i16)> = Vec::new();
    while let Some(r) = queue.pop_front() {
        // Neighbours in a stable order, the widest shared wall first.
        let mut nbs: Vec<(usize, i16)> = shared.iter().filter_map(|(&(a, b), e)| if a as usize == r { Some((e.len(), b)) } else if b as usize == r { Some((e.len(), a)) } else { None }).collect();
        nbs.sort_by(|p, q| q.0.cmp(&p.0).then(p.1.cmp(&q.1)));
        for (_, s) in nbs {
            if !seen[s as usize] {
                seen[s as usize] = true;
                queue.push_back(s as usize);
                tree.push(((r as i16).min(s), (r as i16).max(s)));
            }
        }
    }
    // Circulation (corridors, halls, landings, the public room): every room beside it opens
    // onto it, and passages between two such rooms are dropped (no walking through one room
    // to reach the next). Tree edges still join rooms with no corridor of their own.
    let circ = |r: usize| is_circulation(rooms[r].kind) || public.is_some_and(|k| rooms[r].kind == k);
    let mut onto: std::collections::BTreeSet<(i16, i16)> = Default::default();
    let mut served = vec![false; n];
    if (0..n).any(circ) {
        for r in 0..n {
            if circ(r) {
                continue;
            }
            let best = shared.iter().filter(|&(&(a, b), _)| (a as usize == r && circ(b as usize)) || (b as usize == r && circ(a as usize))).max_by_key(|(_, e)| e.len()).map(|(k, _)| *k);
            // (Onto the one its hint names, where they meet.)
            let hinted = hints.and_then(|h| h.onto.iter().find(|o| o.0 as usize == r)).map(|o| (o.0.min(o.1), o.0.max(o.1))).filter(|k| shared.contains_key(k));
            if let Some(k) = hinted.or(best) {
                onto.insert(k);
                served[r] = true;
            }
        }
        // Circulation spaces open into each other (but those kept apart).
        for &(a, b) in shared.keys() {
            if circ(a as usize) && circ(b as usize) && !hints.is_some_and(|h| h.apart.contains(&(a, b))) {
                onto.insert((a, b));
            }
        }
    }
    for (&(a, b), edges) in &shared {
        let in_tree = tree.contains(&(a, b)) && !(served[a as usize] && served[b as usize]);
        let loops = if served.iter().any(|&s| s) { 0.05 } else { 0.2 };
        let forced = hints.is_some_and(|h| h.link.contains(&(a, b)));
        if !onto.contains(&(a, b)) && !in_tree && !forced && rng.next_f64() > loops {
            continue;
        }
        if !forced && hints.is_some_and(|h| h.apart.contains(&(a, b))) {
            continue;
        }
        // A doorway off the stairs where the wall allows it.
        let clear: Vec<&(usize, usize, bool)> = edges
            .iter()
            .filter(|&&(x, y, vert)| {
                let (nx2, ny2) = if vert { (x + 1, y) } else { (x, y + 1) };
                let kept = |i: usize, j: usize| hints.is_some_and(|h| h.clear.contains(&(j * nx + i)));
                !on_stairs(x, y) && !on_stairs(nx2, ny2) && !kept(x, y) && !kept(nx2, ny2)
            })
            .collect();
        let &(x, y, vert) = if clear.is_empty() { &edges[edges.len() / 2] } else { clear[clear.len() / 2] };
        // (Where its hint puts it, if the rooms meet there off the stairs.)
        let hinted = hints.and_then(|h| h.doors.iter().find(|d| d.0 == (a, b) && clear.contains(&&d.1))).map(|d| d.1);
        let (x, y, vert) = hinted.unwrap_or((x, y, vert));
        door_edges.insert((x, y, vert));
        let (p, q) = edge_seg(x, y, vert);
        doors.push(Door { a: p, b: q, kind: "door", rooms: [a, b], lock: 0 });
    }
    // A room still more than one room from the circulation (here a great or mess hall, and a
    // guardroom, the guards' way through a keep, count too) gets a door to its neighbour
    // nearest it (rooms between two others, corner towers).
    let hub = |r: usize| circ(r) || is_way_through(rooms[r].kind);
    if (0..n).any(hub) {
        for _ in 0..n {
            let mut hops = vec![usize::MAX; n];
            let mut q = std::collections::VecDeque::new();
            for r in (0..n).filter(|&r| hub(r)) {
                hops[r] = 0;
                q.push_back(r);
            }
            while let Some(r) = q.pop_front() {
                for d in &doors {
                    let [a, b] = d.rooms;
                    let o = if a == r as i16 { b } else if b == r as i16 { a } else { continue };
                    if o >= 0 && hops[o as usize] == usize::MAX {
                        hops[o as usize] = hops[r] + 1;
                        q.push_back(o as usize);
                    }
                }
            }
            // (A room that can't be brought nearer itself has the room it is reached through brought
            // nearer, or the one that is reached through, on toward the way in.)
            let nearer = |r: usize| {
                shared
                    .iter()
                    .filter_map(|(&(a, b), e)| {
                        let o = if a as usize == r { b } else if b as usize == r { a } else { return None };
                        (hops[o as usize] + 1 < hops[r]).then_some(((a, b), hops[o as usize], e))
                    })
                    .min_by_key(|x| x.1)
            };
            let deep = (0..n).filter(|&r| hops[r] != usize::MAX && hops[r] >= 3).find_map(|r| {
                nearer(r).or_else(|| {
                    let mut x = r;
                    while hops[x] >= 3 {
                        x = doors
                            .iter()
                            .filter_map(|d| {
                                let [a, b] = d.rooms;
                                let o = if a == x as i16 { b } else if b == x as i16 { a } else { return None };
                                (o >= 0 && hops[o as usize] + 1 == hops[x]).then_some(o as usize)
                            })
                            .min()?;
                        if let Some(e) = nearer(x) {
                            return Some(e);
                        }
                    }
                    None
                })
            });
            let Some(((a, b), _, edges)) = deep else { break };
            let &(x, y, vert) = &edges[edges.len() / 2];
            if door_edges.insert((x, y, vert)) {
                let (p, q) = edge_seg(x, y, vert);
                doors.push(Door { a: p, b: q, kind: "door", rooms: [a, b], lock: 0 });
            } else {
                break;
            }
        }
    }
    // The front door on the ground floor: onto open ground (never against a neighbour's wall),
    // into the public room if it has an outside wall, on the front side if possible, at the
    // middle of that room's run of such walls; off the stairs.
    if ground {
        // (side, i, j, room): side 0 top, 1 right, 2 bottom, 3 left.
        let mut cands: Vec<(usize, usize, usize, i16)> = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let c = cells[j * nx + i];
                if c < 0 || on_stairs(i, j) {
                    continue;
                }
                for (side, (di, dj)) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
                    let (a, b) = (i as isize + di, j as isize + dj);
                    if at(a, b) < 0 && ext_free(a, b) {
                        cands.push((side, i, j, c));
                    }
                }
            }
        }
        let rank = |c: &(usize, usize, usize, i16)| {
            let is_public = public.is_some_and(|k| rooms[c.3 as usize].kind == k);
            (is_public, c.0 == front, rooms[c.3 as usize].squares)
        };
        // (Into the room its hint names, through the first edge it names that is open ground, if
        // it can.)
        let into = hints.and_then(|h| h.front.as_ref()).filter(|(r, _)| cands.iter().any(|c| c.3 == *r));
        let named = into.and_then(|(r, edges)| edges.iter().find_map(|e| cands.iter().find(|c| c.3 == *r && (c.0, c.1, c.2) == *e).copied()));
        let cands: Vec<(usize, usize, usize, i16)> = match into {
            Some((r, _)) => cands.into_iter().filter(|c| c.3 == *r).collect(),
            None => cands,
        };
        if let Some(best) = cands.iter().max_by(|x, y| rank(x).cmp(&rank(y)).then(y.1.cmp(&x.1)).then(y.2.cmp(&x.2))).copied() {
            let mine: Vec<&(usize, usize, usize, i16)> = cands.iter().filter(|c| c.3 == best.3 && c.0 == best.0).collect();
            let &(side, i, j, r) = named.as_ref().unwrap_or(mine[mine.len() / 2]);
            let (p, q) = match side {
                0 => ([i as f32, j as f32], [(i + 1) as f32, j as f32]),
                1 => ([(i + 1) as f32, j as f32], [(i + 1) as f32, (j + 1) as f32]),
                2 => ([i as f32, (j + 1) as f32], [(i + 1) as f32, (j + 1) as f32]),
                _ => ([i as f32, j as f32], [i as f32, (j + 1) as f32]),
            };
            doors.push(Door { a: p, b: q, kind: "front", rooms: [r, -1], lock: 0 });
        }
        // (Open archways onto the street: each edge named, its room's, onto open ground.)
        for (r, edges) in hints.map_or(&[][..], |h| &h.arch[..]) {
            for &(side, i, j) in edges {
                let (di, dj) = [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)][side.min(3)];
                if at(i as isize, j as isize) != *r || at(i as isize + di, j as isize + dj) >= 0 || !ext_free(i as isize + di, j as isize + dj) {
                    continue;
                }
                let (p, q) = match side {
                    0 => ([i as f32, j as f32], [(i + 1) as f32, j as f32]),
                    1 => ([(i + 1) as f32, j as f32], [(i + 1) as f32, (j + 1) as f32]),
                    2 => ([i as f32, (j + 1) as f32], [(i + 1) as f32, (j + 1) as f32]),
                    _ => ([i as f32, j as f32], [i as f32, (j + 1) as f32]),
                };
                doors.push(Door { a: p, b: q, kind: "arch", rooms: [*r, -1], lock: 0 });
            }
        }
    }
    let walls = wall_runs(&cells, &rooms, nx, ny, &doors);
    Level { z: 0, name: String::new(), elevation_ft: 0.0, cells, rooms, walls, doors, windows: Vec::new(), furniture: Vec::new(), roof: false, has_stairs: true, natural: false, paths: Vec::new(), rim: Vec::new(), links: Vec::new(), floor: String::new(), ramps: Vec::new(), landing: None }
}

/// Wall runs: every square edge between different rooms (or a room and the outside) that is not
/// a door, merged along straight lines; where a hall's height left open (`OPEN_BELOW`) meets
/// circulation (a gallery, a passage, a landing), a railing; none along a roof's slopes inside
/// the walls (their eaves).
pub fn wall_runs(cells: &[i16], rooms: &[Room], nx: usize, ny: usize, doors: &[Door]) -> Vec<Wall> {
    let at = |i: isize, j: isize| if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { cells[j as usize * nx + i as usize] };
    let kind = |r: i16| rooms.get(r as usize).map_or("", |r| r.kind);
    // (A coaching inn's gallery railed onto its yard open to the sky.)
    let rail = |a: i16, b: i16| (a >= 0 && b >= 0 && railed(&rooms[a as usize], &rooms[b as usize])) || ((a < 0) != (b < 0) && kind(a.max(b)) == "yard gallery");
    let eave = |a: i16, b: i16| a >= 0 && b >= 0 && (kind(a) == ROOF_SLOPE || kind(b) == ROOF_SLOPE);
    let mut h_edges: Vec<(usize, usize, bool, bool)> = Vec::new(); // (x, y line, exterior, rail) horizontal unit edges at y
    let mut v_edges: Vec<(usize, usize, bool, bool)> = Vec::new(); // (x line, y, exterior, rail)
    let is_door = |a: [f32; 2], b: [f32; 2]| doors.iter().any(|d| (d.a == a && d.b == b) || (d.a == b && d.b == a));
    for j in 0..=ny {
        for i in 0..nx {
            let (up, dn) = (at(i as isize, j as isize - 1), at(i as isize, j as isize));
            if up != dn && (up >= 0 || dn >= 0) && !eave(up, dn) && !is_door([i as f32, j as f32], [(i + 1) as f32, j as f32]) {
                h_edges.push((i, j, up < 0 || dn < 0, rail(up, dn)));
            }
        }
    }
    for i in 0..=nx {
        for j in 0..ny {
            let (lf, rt) = (at(i as isize - 1, j as isize), at(i as isize, j as isize));
            if lf != rt && (lf >= 0 || rt >= 0) && !eave(lf, rt) && !is_door([i as f32, j as f32], [i as f32, (j + 1) as f32]) {
                v_edges.push((i, j, lf < 0 || rt < 0, rail(lf, rt)));
            }
        }
    }
    let mut walls: Vec<Wall> = Vec::new();
    h_edges.sort_by_key(|e| (e.1, e.0));
    for e in &h_edges {
        if let Some(w) = walls.last_mut().filter(|w| w.a[1] == w.b[1] && w.b[1] == e.1 as f32 && w.b[0] == e.0 as f32 && w.exterior == e.2 && w.rail == e.3) {
            w.b[0] += 1.0;
        } else {
            walls.push(Wall { a: [e.0 as f32, e.1 as f32], b: [(e.0 + 1) as f32, e.1 as f32], exterior: e.2, rail: e.3, arcade: false });
        }
    }
    v_edges.sort_by_key(|e| (e.0, e.1));
    for e in &v_edges {
        if let Some(w) = walls.last_mut().filter(|w| w.a[0] == w.b[0] && w.b[0] == e.0 as f32 && w.b[1] == e.1 as f32 && w.exterior == e.2 && w.rail == e.3) {
            w.b[1] += 1.0;
        } else {
            walls.push(Wall { a: [e.0 as f32, e.1 as f32], b: [e.0 as f32, (e.1 + 1) as f32], exterior: e.2, rail: e.3, arcade: false });
        }
    }
    // (An arcade's street side: its outer wall along the line of its front door, wherever the
    // arcade is inside it, arches between piers (`Wall::arcade`): a run there cut where the arcade
    // ends along it, a stair hall's or a tower's wall beside it kept.)
    for d in doors.iter().filter(|d| d.kind == "front" && kind(d.rooms[0]) == "arcade") {
        let (r, flat) = (d.rooms[0], d.a[1] == d.b[1]);
        let mut out = Vec::with_capacity(walls.len());
        for w in walls.drain(..) {
            let on = w.exterior && !w.rail && if flat { w.a[1] == d.a[1] && w.b[1] == d.a[1] } else { w.a[0] == d.a[0] && w.b[0] == d.a[0] };
            if !on {
                out.push(w);
                continue;
            }
            let (x0, x1, y0, y1) = (w.a[0].min(w.b[0]) as isize, w.a[0].max(w.b[0]) as isize, w.a[1].min(w.b[1]) as isize, w.a[1].max(w.b[1]) as isize);
            let (lo, hi) = if flat { (x0, x1) } else { (y0, y1) };
            let open = |k: isize| if flat { at(k, y0 - 1) == r || at(k, y0) == r } else { at(x0 - 1, k) == r || at(x0, k) == r };
            let mut k = lo;
            while k < hi {
                let a = open(k);
                let mut e = k + 1;
                while e < hi && open(e) == a {
                    e += 1;
                }
                let (p, q) = if flat { ([k as f32, w.a[1]], [e as f32, w.a[1]]) } else { ([w.a[0], k as f32], [w.a[0], e as f32]) };
                out.push(Wall { a: p, b: q, exterior: true, rail: false, arcade: a });
                k = e;
            }
        }
        walls = out;
    }
    walls

}

/// Windows on exterior walls above ground: every third square along a run, off doors; none onto
/// open battlements, a walk or a roof (a keep's lower pieces' tops beside a storey), nor in a yurt's
/// lattice wall.
pub(super) fn windows(lvl: &mut Level, nx: usize, ny: usize, _ground: bool) {
    let mut out = Vec::new();
    let open = |x: isize, y: isize| {
        x >= 0 && y >= 0 && (x as usize) < nx && (y as usize) < ny && lvl.cells[y as usize * nx + x as usize] >= 0 && matches!(lvl.rooms[lvl.cells[y as usize * nx + x as usize] as usize].kind, "battlements" | "wall walk" | ROOF_SLOPE | "yurt" | "felt hall" | "felt sanctuary" | "felt common room")
    };
    for w in &lvl.walls {
        if !w.exterior || w.arcade {
            continue;
        }
        let len = ((w.b[0] - w.a[0]).abs() + (w.b[1] - w.a[1]).abs()) as usize;
        // (f32::signum(0.0) is 1.0: compare explicitly.)
        let dir = |d: f32| if d > 0.0 { 1.0 } else if d < 0.0 { -1.0 } else { 0.0 };
        let (dx, dy) = (dir(w.b[0] - w.a[0]), dir(w.b[1] - w.a[1]));
        let mut k = 1;
        while k + 1 < len {
            let p = [w.a[0] + dx * k as f32, w.a[1] + dy * k as f32];
            // (Not where the square inside is an open top: a run may go on from a storey's wall to
            // a platform's parapet.)
            let (x, y) = (p[0].min(p[0] + dx) as isize, p[1].min(p[1] + dy) as isize);
            let sq = if dx == 0.0 { [(x - 1, y), (x, y)] } else { [(x, y - 1), (x, y)] };
            if !sq.iter().any(|&(i, j)| open(i, j)) {
                out.push([p[0], p[1], p[0] + dx, p[1] + dy]);
            }
            k += 3;
        }
    }
    lvl.windows = out;
}

/// Pieces a room must have (drinking rooms: a bar and two tables), tried in smaller sizes
/// and anywhere in the room when the preferred spot is not free.
pub(super) fn required(kind: &str, item: &str) -> bool {
    matches!(
        (kind, item),
        ("common room" | "taproom", "bar" | "table")
            | ("council chamber", "council_table")
            | ("job-board room", "job_board")
            | ("counting house", "counting_desk")
            | ("strongroom" | "warded vault", "strongbox")
            | ("stables", "stable_stall")
            | ("snug", "table")
            | ("private dining room", "long_table")
            | ("best chamber", "bed")
            | ("bedroom" | "hearth room", "bed")
    )
}

/// Inns, taverns and breweries: the public room (taproom) is where guests come in and go up.
pub(super) fn is_pub(arch: Arch) -> bool {
    matches!(arch, Arch::Tavern | Arch::Inn | Arch::Brewery)
}
