use crate::prelude::*;

pub(crate) fn interiors(ctx: &mut Ctx, keeps: bool, r: &mut Report) {
    r.counts(&["building interiors checked", "tower interiors checked", "interiors breaking a rule", "functions with interiors breaking a rule"]);
    r.counts(&["floors the corridor rule applies to", "floors with a room over 2 rooms from a corridor", "the same, the front room not counted as a corridor"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    // Per function: checked, failing.
    let mut by_func: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let buckets = building_buckets(&l);
        // (R5 towns: a steppe town's round felt halls checked by `steppe`.)
        let mut ids: Vec<String> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && (!keeps || is_keep(b)) && town::roof::felt_built(b).is_none()).map(|b| format!("b:{li}:{}", b.id)).collect();
        if !keeps {
            ids.extend((0..interior::towers(&l).len()).map(|k| format!("t:{li}:{k}")));
        }
        for id in ids {
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            r.tally(if id.starts_with('b') { "building interiors checked" } else { "tower interiors checked" }, 1);
            let bad = check_interior(world, t0, &l, li, &id, &it, &buckets);
            let e = by_func.entry(it.function).or_default();
            e.0 += 1;
            if !bad.is_empty() {
                e.1 += 1;
                r.add("interiors breaking a rule", bad.len() as f64, format!("{id} ({}): {}", it.function, bad.join("; ")));
            }
            for lv in &it.levels {
                if let Some((hops, _)) = corridor_hops(lv, true) {
                    r.tally("floors the corridor rule applies to", 1);
                    if hops > 2 {
                        r.add("floors with a room over 2 rooms from a corridor", hops as f64, format!("{id} ({}) {}", it.function, lv.name));
                    }
                }
                if let Some((hops, path)) = corridor_hops(lv, false).filter(|h| h.0 > 2) {
                    r.add("the same, the front room not counted as a corridor", hops as f64, format!("{id} ({}) {}: {} rooms ({path})", it.function, lv.name, hops - 1));
                }
            }
        }
    }
    for (f, (n, b)) in by_func.iter().filter(|f| f.1.1 > 0) {
        r.add("functions with interiors breaking a rule", *b as f64, format!("{f}: {b} of {n}"));
    }
}

// ---- Buildings laid out by zones. ----

pub(crate) const HUB_COUNTS: [&str; 30] = [
    "floors the zones fell back on",
    "breaking an interior rule (as in interiors)",
    "great or mess halls not one, on their floor",
    "front doors into the wrong room",
    "entry route reaches hall at dais end",
    "main stairs or front doors not into the hall through its screens passage",
    "private stairs not from the dais end up to the lord's floor",
    "main stairs standing in a room",
    "great chambers or solars over 2 squares from the lord's stair",
    "solars nearer the hall's low end than the lord's stair (their middles)",
    "great chambers or solars under 9 squares",
    "keeps with no great chamber or solar",
    "minstrels' galleries not reached along passages",
    "windows on open tops",
    "thrones in mess halls",
    "storeys above the hall with one room size for over half their rooms",
    "storeys above the hall with half their rooms on the squares of the storey below",
    "rooms above the hall longer than 2.5:1",
    "rooms above the hall filling under 60% of their box (its floor)",
    "rooms above the hall over 40 squares (a great chamber 60)",
    "keeps missing a room of their programme",
    "roofs that don't fill their box",
    "tower rooms all alike on a floor, or plain 'tower room'",
    "landings over 12 squares",
    "screens passages not one straight row",
    "service ends short of their rooms",
    "barracks with no officers' end",
    "rooms a square wide",
    "shell keeps' yards not round",
    "levels with furniture on an open top",
];

/// The way from room `from` into room `hall` on a floor, fewest doors first: the rooms on it (from
/// `from` to `hall`) and the middle of the door it enters `hall` by (None where it starts in it).
pub(crate) fn way_into(lv: &interior::Level, from: i16, hall: i16) -> Option<(Vec<i16>, Option<P>)> {
    if from < 0 {
        return None;
    }
    let n = lv.rooms.len();
    let mut prev: Vec<Option<(i16, usize)>> = vec![None; n];
    let mut seen = vec![false; n];
    let mut q = VecDeque::from([from]);
    seen[from as usize] = true;
    while let Some(a) = q.pop_front() {
        if a == hall {
            let mut path = vec![a];
            let mut door = None;
            let mut c = a;
            while let Some((p, di)) = prev[c as usize] {
                if door.is_none() {
                    let d = &lv.doors[di];
                    door = Some([0.5 * (d.a[0] + d.b[0]) as f64, 0.5 * (d.a[1] + d.b[1]) as f64]);
                }
                path.push(p);
                c = p;
            }
            path.reverse();
            return Some((path, door));
        }
        for (di, d) in lv.doors.iter().enumerate().filter(|(_, d)| d.rooms.contains(&a) && d.rooms.iter().all(|&o| o >= 0)) {
            let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
            if !seen[o as usize] {
                seen[o as usize] = true;
                prev[o as usize] = Some((a, di));
                q.push_back(o);
            }
        }
    }
    None
}

/// The squares either side of a door's edge.
pub(crate) fn door_squares(d: &interior::Door) -> [(isize, isize); 2] {
    let (x0, y0) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize);
    if d.a[0] == d.b[0] { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
}

/// `hub`: castle keeps and barracks of 150 squares or more, laid out by zones (`interior::hub`),
/// against the rules their layout keeps (the user's calls): no floor fallen back on the shared
/// layout; the hall on its floor (a great tower's and a tower keep's on the first); the front door
/// into the screens passage (or a forebuilding's entrance hall onto it), a great tower's entrance
/// hall, a barracks' mess hall, a shell keep's gate passage; the main stair landing in passages or
/// landings, at the low end: the front door (a hall on the ground floor) and the main stair reach
/// the hall through its screens passage, entering it on the half away from its dais (the user's
/// call, 2026-10-27); the lord's private spiral stair from a room off the hall at the dais end up to
/// the floor above; the great chamber and solar by the lord's stair (the private one, else the main:
/// a square of each within 2 squares of it, a solar's middle nearer it than the hall's low end), 9
/// squares or more; a
/// minstrels' gallery reached from the stair through passages; no windows or furniture on open tops;
/// no throne in a mess hall; rooms of different sizes, no longer than 2.5:1, filling 60% of the floor
/// in their box, no storey laid out as the one below it (a forebuilding's storeys aside); tower rooms
/// of different kinds; passage-sized landings, a straight screens passage, the service end's rooms, a
/// barracks' officers' end; no room a square wide; a shell keep's yard round. Street-editor castles'
/// keeps too (`editor`).
pub(crate) fn hub(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["buildings laid out by zones"]);
    r.counts(&HUB_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        for b in &l.buildings {
            if !interior::hub_kind(t0, &l, b).is_some_and(|k| k.is_keep()) {
                continue;
            }
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            keep_rules(world, t0, &l, li, b, "", buckets, r);
        }
    }
}

/// A building laid out by zones through `check_interior` and `hub_rules` (`tag`: what to name it by
/// beside its id).
#[allow(clippy::too_many_arguments)]
pub(crate) fn keep_rules(world: &World, t0: &T0, l: &Layout, li: usize, b: &town::Building, tag: &str, buckets: &HashMap<(i64, i64), Vec<usize>>, r: &mut Report) {
    let Some(kind) = interior::hub_kind(t0, l, b).filter(|k| k.is_keep()) else { return };
    let id = format!("b:{li}:{}", b.id);
    let Some(it) = interior::generate_id(world, t0, &id) else { return };
    r.tally("buildings laid out by zones", 1);
    r.tally(&format!("  {}{}", kind.name(), if kind.name().ends_with('s') { "" } else { "s" }), 1);
    let named = format!("{tag}{id}");
    // (`PLAN=<tag and id>`: its text plan, one letter a room.)
    if std::env::var("PLAN").is_ok_and(|p| p.trim() == named.trim()) {
        let c = centroid(&b.poly);
        say!("-- {named} ({}) at ({:.0}, {:.0}) ft", kind.name(), c[0], c[1]);
        for lv in &it.levels {
            say!("-- {} (z {}): {}", lv.name, lv.z, lv.rooms.iter().map(|r| format!("{}={}", r.kind, r.squares)).collect::<Vec<_>>().join(", "));
            for j in 0..it.ny {
                let row: String = (0..it.nx)
                    .map(|i| {
                        let c = lv.cells[j * it.nx + i];
                        let st = it.stairs;
                        if lv.has_stairs && i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3] {
                            '#'
                        } else if c < 0 {
                            ' '
                        } else if lv.rooms[c as usize].kind == interior::OPEN_BELOW {
                            ':'
                        } else {
                            (b'a' + (c as u8 % 26)) as char
                        }
                    })
                    .collect();
                say!("  {row}");
            }
        }
    }
    let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
    if !fell.is_empty() {
        r.add("floors the zones fell back on", fell.len() as f64, format!("{named} ({}): z {fell:?}", kind.name()));
    }
    let bad = check_interior(world, t0, l, li, &id, &it, buckets);
    if !bad.is_empty() {
        r.add("breaking an interior rule (as in interiors)", bad.len() as f64, format!("{named} ({}): {}", kind.name(), bad.join("; ")));
    }
    hub_rules(l, b, kind, &named, &it, r);
}

pub(crate) fn hub_rules(l: &Layout, b: &town::Building, kind: interior::hub::Kind, id: &str, it: &interior::Interior, r: &mut Report) {
    use interior::hub::Kind;
    let (nx, ny) = (it.nx, it.ny);
    let tag = format!("{id} ({})", kind.name());
    let sh = interior::Shell::of(l, b);
    let squares = |lv: &interior::Level, ri: usize| -> Vec<usize> { (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect() };
    let mid = |sq: &[usize]| -> P { [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64] };
    let room_at = |lv: &interior::Level, (i, j): (isize, isize)| -> i16 { if i < 0 || j < 0 || i as usize >= nx || j as usize >= ny { -1 } else { lv.cells[j as usize * nx + i as usize] } };
    let between = |lv: &interior::Level, a: i16, c: i16| lv.doors.iter().any(|d| d.rooms == [a.min(c), a.max(c)]);
    let open_top = |k: &str| matches!(k, "battlements" | "wall walk") || k == interior::ROOF_SLOPE;
    let barracks = kind == Kind::Barracks;
    let hall_kind = if barracks { "mess hall" } else { "great hall" };
    let hall_z = if matches!(kind, Kind::Great | Kind::Plain) && b.floors >= 2 { 1 } else { 0 };
    let halls: Vec<(usize, usize)> = it.levels.iter().enumerate().filter(|(_, lv)| lv.z >= 0).flat_map(|(li, lv)| (0..lv.rooms.len()).filter(|&ri| lv.rooms[ri].kind == hall_kind && lv.rooms[ri].squares > 0).map(move |ri| (li, ri))).collect();
    if halls.len() != 1 || it.levels[halls[0].0].z != hall_z {
        r.add("great or mess halls not one, on their floor", halls.len() as f64, format!("{tag}: {:?}", halls.iter().map(|h| it.levels[h.0].z).collect::<Vec<_>>()));
    }
    let st = it.stairs;
    let sc: P = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
    // Every floor: the main stair lands in a passage, a landing, a yard or on a walk.
    for lv in it.levels.iter().filter(|lv| lv.z >= 0 && lv.has_stairs) {
        let ri = lv.cells[st[1] * nx + st[0]];
        if ri >= 0 && !interior::is_way_through(lv.rooms[ri as usize].kind) {
            r.add("main stairs standing in a room", 1.0, format!("{tag} {}: in the {}", lv.name, lv.rooms[ri as usize].kind));
        }
    }
    // The front door.
    if let Some(g0) = it.levels.iter().find(|lv| lv.z == 0)
        && let Some(front) = g0.doors.iter().find(|d| d.kind == "front")
    {
        let fr = front.rooms[0];
        let k = g0.rooms[fr as usize].kind;
        let ok = match kind {
            Kind::Hall | Kind::L | Kind::T => k == "screens passage" || (k == "entrance hall" && g0.rooms.iter().enumerate().any(|(o, ro)| ro.kind == "screens passage" && between(g0, fr, o as i16))),
            Kind::Great | Kind::Plain => k == "entrance hall" || (b.floors < 2 && k == "great hall"),
            Kind::Barracks => k == "mess hall",
            Kind::ShellKeep => k == "gate passage",
            Kind::Palace | Kind::OldPalace | Kind::HallHouse | Kind::Manor | Kind::Church | Kind::TempleHall | Kind::Monastery => true,
            _ => true,
        };
        if !ok {
            r.add("front doors into the wrong room", 1.0, format!("{tag}: into the {k}"));
        }
    }
    // Windows and furniture on open tops.
    for lv in &it.levels {
        for w in &lv.windows {
            let d = interior::Door { a: [w[0], w[1]], b: [w[2], w[3]], kind: "window", rooms: [-1, -1], lock: 0 };
            if door_squares(&d).iter().any(|&s| room_at(lv, s) >= 0 && open_top(lv.rooms[room_at(lv, s) as usize].kind)) {
                r.add("windows on open tops", 1.0, format!("{tag} {}: ({}, {})", lv.name, w[0], w[1]));
            }
        }
        let on_top = lv.furniture.iter().filter(|f| f.kind != "spiral_stair" && lv.cells[f.y as usize * nx + f.x as usize] >= 0 && open_top(lv.rooms[lv.cells[f.y as usize * nx + f.x as usize] as usize].kind)).count();
        if on_top > 0 {
            r.add("levels with furniture on an open top", on_top as f64, format!("{tag} {}: {on_top} pieces", lv.name));
        }
        for f in lv.furniture.iter().filter(|f| f.kind == "throne") {
            let c = lv.cells[f.y as usize * nx + f.x as usize];
            if c >= 0 && lv.rooms[c as usize].kind == "mess hall" {
                r.add("thrones in mess halls", 1.0, format!("{tag} {}: ({}, {})", lv.name, f.x, f.y));
            }
        }
    }
    // Rooms: sizes, slivers, tower rooms, storeys alike, landings.
    let mut below: Option<&interior::Level> = None;
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        let mut tower_rooms: Vec<i16> = sh.towers.iter().map(|t| lv.cells[t.2]).filter(|&c| c >= 0).collect();
        // (A barracks' corner towers: rooms in a 3-square box at a corner of the grid.)
        if barracks {
            for (ri, room) in lv.rooms.iter().enumerate() {
                let sq = squares(lv, ri);
                let corner = |v: usize, n: usize| v < 3 || v + 3 >= n;
                if room.squares > 0 && room.squares <= 9 && sq.iter().all(|&k| corner(k % nx, nx) && corner(k / nx, ny)) {
                    tower_rooms.push(ri as i16);
                }
            }
        }
        let ordinary = |ri: usize| {
            let k = lv.rooms[ri].kind;
            lv.rooms[ri].squares > 0 && interior::walkable(k) && !interior::is_way_through(k) && !tower_rooms.contains(&(ri as i16)) && k != "guard post"
        };
        let sizes: Vec<usize> = (0..lv.rooms.len()).filter(|&ri| ordinary(ri)).map(|ri| lv.rooms[ri].squares).collect();
        if sizes.len() >= 5 && lv.z > hall_z {
            let most = sizes.iter().map(|s| sizes.iter().filter(|t| *t == s).count()).max().unwrap_or(0);
            if most * 2 > sizes.len() {
                r.add("storeys above the hall with one room size for over half their rooms", most as f64 / sizes.len() as f64, format!("{tag} {}: {sizes:?}", lv.name));
            }
        }
        for ri in (0..lv.rooms.len()).filter(|&ri| ordinary(ri) && lv.rooms[ri].squares >= 2) {
            let mine = |i: usize, j: usize| i < nx && j < ny && lv.cells[j * nx + i] == ri as i16;
            if !(0..nx * ny).any(|k| mine(k % nx, k / nx) && mine(k % nx + 1, k / nx) && mine(k % nx, k / nx + 1) && mine(k % nx + 1, k / nx + 1)) {
                r.add("rooms a square wide", lv.rooms[ri].squares as f64, format!("{tag} {}: the {} ({} squares)", lv.name, lv.rooms[ri].kind, lv.rooms[ri].squares));
            }
        }
        let kinds: Vec<&str> = tower_rooms.iter().map(|&c| lv.rooms[c as usize].kind).filter(|&k| k != "tower top").collect();
        if kinds.contains(&"tower room") || (kinds.len() >= 3 && kinds.iter().all(|k| *k == kinds[0])) {
            r.add("tower rooms all alike on a floor, or plain 'tower room'", kinds.len() as f64, format!("{tag} {}: {kinds:?}", lv.name));
        }
        for (ri, room) in lv.rooms.iter().enumerate().filter(|(_, room)| room.kind == "landing" && room.squares > 12) {
            r.add("landings over 12 squares", room.squares as f64, format!("{tag} {}: {} squares (room {ri})", lv.name, room.squares));
        }
        // Above the hall: ordinary rooms (not halls, passages, galleries or towers) no longer than
        // 2.5 times their width and of 40 squares at most (a great chamber 60), and half the rooms
        // or more not on just the squares of a room of the storey below.
        let upper: Vec<usize> = (0..lv.rooms.len()).filter(|&ri| lv.z > hall_z && !lv.roof && ordinary(ri) && !open_top(lv.rooms[ri].kind)).collect();
        for &ri in &upper {
            let (room, sq) = (&lv.rooms[ri], squares(lv, ri));
            let (xs, ys): (Vec<usize>, Vec<usize>) = sq.iter().map(|&k| (k % nx, k / nx)).unzip();
            let (w, h) = (xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1, ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1);
            if 2 * w.max(h) > 5 * w.min(h) {
                r.add("rooms above the hall longer than 2.5:1", w.max(h) as f64 / w.min(h) as f64, format!("{tag} {}: the {} {w} x {h} ({} squares)", lv.name, room.kind, room.squares));
            }
            // (A room with a tail a square wide: the floor in its box mostly other rooms'; a shell
            // keep's lean-tos bend round its ring.)
            let (x0, y0) = (xs.iter().min().unwrap(), ys.iter().min().unwrap());
            let floor = (*y0..y0 + h).flat_map(|j| (*x0..x0 + w).map(move |i| j * nx + i)).filter(|&k| lv.cells[k] >= 0).count();
            if sq.len() * 5 < floor * 3 && kind != Kind::ShellKeep {
                r.add("rooms above the hall filling under 60% of their box (its floor)", floor as f64 / sq.len() as f64, format!("{tag} {}: the {} of {} squares in a {w} x {h} box", lv.name, room.kind, room.squares));
            }
            let cap = if room.kind == "great chamber" { 60 } else { 40 };
            if room.squares > cap {
                r.add("rooms above the hall over 40 squares (a great chamber 60)", room.squares as f64, format!("{tag} {}: the {} of {} squares", lv.name, room.kind, room.squares));
            }
        }
        // (But a forebuilding's: its storeys one room each, the entrance hall and the chapel over it.)
        let fore = |sq: &[usize]| sq.iter().all(|&k| sh.owner.get(k).and_then(|&o| b.parts.get(o)).is_some_and(|p| p.kind == town::PartKind::Forebuilding));
        if let Some(lb) = below.filter(|lb| !lb.roof && lb.z + 1 == lv.z && upper.len() >= 2) {
            let below_sq: Vec<Vec<usize>> = (0..lb.rooms.len()).map(|ri| squares(lb, ri)).collect();
            let same = upper.iter().filter(|&&ri| !fore(&squares(lv, ri)) && below_sq.contains(&squares(lv, ri))).count();
            if 2 * same >= upper.len() {
                r.add("storeys above the hall with half their rooms on the squares of the storey below", same as f64 / upper.len() as f64, format!("{tag} {} over {}: {same} of {} rooms", lv.name, lb.name, upper.len()));
            }
        }
        below = Some(lv);
    }
    // Roofs: hipped over the box they fill (a shell keep's ring round its yard aside).
    for lv in it.levels.iter().filter(|_| kind != Kind::ShellKeep) {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, room)| room.kind == interior::ROOF_SLOPE && room.squares > 0) {
            let sq = squares(lv, ri);
            let (xs, ys): (Vec<usize>, Vec<usize>) = sq.iter().map(|&k| (k % nx, k / nx)).unzip();
            let (w, h) = (xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1, ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1);
            if w * h != sq.len() {
                r.add("roofs that don't fill their box", (w * h - sq.len()) as f64, format!("{tag} {}: {} squares in a {w} x {h} box", lv.name, sq.len()));
            }
        }
    }
    // A keep's programme: the great chamber, the solar, the chapel, a treasury, an armory, a
    // garrison or stores.
    if !barracks {
        let has = |k: &str| it.levels.iter().any(|lv| lv.rooms.iter().any(|ro| ro.kind == k && ro.squares > 0));
        let missing: Vec<&str> = ["great chamber", "solar", "chapel", "treasury", "armory", "garrison"].into_iter().filter(|k| !has(k) && !(*k == "garrison" && has("storeroom"))).collect();
        if !missing.is_empty() {
            r.add("keeps missing a room of their programme", missing.len() as f64, format!("{tag}: no {}", missing.join(", ")));
        }
    }
    if kind == Kind::ShellKeep {
        for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
            for (ri, room) in lv.rooms.iter().enumerate().filter(|(_, room)| room.kind == "keep yard" || room.kind == interior::OPEN_BELOW) {
                if room.shape != interior::shape::ROUND {
                    r.add("shell keeps' yards not round", 1.0, format!("{tag} {}: the {} (room {ri})", lv.name, room.kind));
                }
            }
        }
        return;
    }
    let Some(&(hl, hall)) = halls.first() else { return };
    let g = &it.levels[hl];
    let hall = hall as i16;
    let first = |k: &str| g.rooms.iter().position(|ro| ro.kind == k && ro.squares > 0).map(|ri| ri as i16);
    // The low end: the screens passage, else the kitchen.
    let low = first("screens passage").or_else(|| first("kitchen")).map(|ri| mid(&squares(g, ri as usize)));
    let hc = mid(&squares(g, hall as usize));
    // The way in: from the main stair, and from the front door where the hall is on the ground
    // floor, into the hall through its screens passage (where it has one) on the half away from its
    // dais.
    let dais: Option<P> = g.furniture.iter().find(|f| matches!(f.kind, "throne_dais" | "low_dais") && g.cells[f.y as usize * nx + f.x as usize] == hall).map(|f| [f.x as f64 + 0.5 * f.w as f64, f.y as f64 + 0.5 * f.h as f64]);
    let screens = first("screens passage");
    let mut starts = vec![("main stair", g.cells[st[1] * nx + st[0]], sc)];
    if g.z == 0
        && let Some(front) = g.doors.iter().find(|d| d.kind == "front")
    {
        starts.push(("front door", front.rooms[0], [0.5 * (front.a[0] + front.b[0]) as f64, 0.5 * (front.a[1] + front.b[1]) as f64]));
    }
    for (what, from, at) in starts {
        let Some((path, door)) = way_into(g, from, hall) else {
            r.add("entry route reaches hall at dais end", 1.0, format!("{tag}: no way from the {what} into the hall"));
            continue;
        };
        let into = door.unwrap_or(at);
        if dais.is_some_and(|d| dot(sub(into, hc), sub(d, hc)) >= 0.0) {
            let by: Vec<&str> = path.iter().map(|&ri| g.rooms[ri as usize].kind).collect();
            r.add("entry route reaches hall at dais end", 1.0, format!("{tag}: from the {what} by {}", by.join(" > ")));
        }
        if screens.is_some_and(|s| path.len() < 2 || path[path.len() - 2] != s) {
            r.add("main stairs or front doors not into the hall through its screens passage", 1.0, format!("{tag}: from the {what} by the {}", if path.len() >= 2 { g.rooms[path[path.len() - 2] as usize].kind } else { "hall itself" }));
        }
    }
    // The lord's private stair: a spiral stair on the hall's floor (not a tower's: at a grid corner or
    // a piece tower's middle) in a room the hall opens into at the dais end, up to the floor above.
    let towered = |k: usize| sh.towers.iter().any(|t| t.2 == k) || [0, nx - 1, nx * (ny - 1), nx * ny - 1].contains(&k);
    let private: Option<usize> = g.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize).find(|&k| !towered(k));
    let above = it.levels.iter().find(|lv| lv.z == g.z + 1 && !lv.roof);
    match private {
        Some(k) => {
            let pr = g.cells[k];
            let up = above.is_some_and(|lv| lv.cells[k] >= 0 && lv.furniture.iter().any(|f| f.kind == "spiral_stair" && f.y as usize * nx + f.x as usize == k));
            let at_dais = dais.is_none_or(|d| dot(sub([(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5], hc), sub(d, hc)) > 0.0);
            if pr < 0 || pr == hall || !between(g, pr, hall) || !up || !at_dais {
                r.add("private stairs not from the dais end up to the lord's floor", 1.0, format!("{tag}: in the {}{}{}", if pr >= 0 { g.rooms[pr as usize].kind } else { "-" }, if up { "" } else { ", not up" }, if at_dais { "" } else { ", at the low end" }));
            }
        }
        None if screens.is_some() && above.is_some() => r.add("private stairs not from the dais end up to the lord's floor", 1.0, format!("{tag}: none")),
        None => {}
    }
    // (The stair the lord's rooms go by, as a block: the private one, else the main.)
    let st = private.map_or(st, |k| [k % nx, k / nx, 1, 1]);
    let sc: P = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
    if matches!(kind, Kind::Hall | Kind::L | Kind::T) {
        if let Some(scr) = first("screens passage") {
            let sq = squares(g, scr as usize);
            let (xs, ys): (Vec<usize>, Vec<usize>) = sq.iter().map(|&k| (k % nx, k / nx)).unzip();
            let (w, h) = (xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1, ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1);
            if w.min(h) > 1 {
                r.add("screens passages not one straight row", sq.len() as f64, format!("{tag}: {w} x {h}, {} squares", sq.len()));
            }
        }
        if first("kitchen").is_none() || (first("buttery").is_none() && first("pantry").is_none()) {
            r.add("service ends short of their rooms", 1.0, format!("{tag}: {:?}", g.rooms.iter().filter(|ro| matches!(ro.kind, "kitchen" | "buttery" | "pantry")).map(|ro| (ro.kind, ro.squares)).collect::<Vec<_>>()));
        }
    }
    if barracks {
        if first("kitchen").is_none() || first("officers' quarters").is_none() {
            r.add("barracks with no officers' end", 1.0, format!("{tag}: {:?}", g.rooms.iter().filter(|ro| matches!(ro.kind, "kitchen" | "officers' quarters" | "armory")).map(|ro| (ro.kind, ro.squares)).collect::<Vec<_>>()));
        }
        return;
    }
    // The lord's rooms: by the lord's stair, of a good size; a solar's middle nearer the stair than the
    // hall's low end's (the screens passage's, else the kitchen's).
    let mut lords = 0;
    for lv in it.levels.iter().filter(|lv| lv.z > hall_z) {
        for (ri, room) in lv.rooms.iter().enumerate().filter(|(_, room)| matches!(room.kind, "great chamber" | "solar") && room.squares > 0) {
            lords += 1;
            if room.squares < 9 {
                r.add("great chambers or solars under 9 squares", 9.0 - room.squares as f64, format!("{tag} {}: the {} of {} squares", lv.name, room.kind, room.squares));
            }
            // (Its nearest square's gap to the stair block, in squares.)
            let gap = squares(lv, ri)
                .iter()
                .map(|&k| {
                    let (i, j) = ((k % nx) as f64, (k / nx) as f64);
                    let dx = (st[0] as f64 - i - 1.0).max(i - (st[0] + st[2]) as f64).max(0.0);
                    let dy = (st[1] as f64 - j - 1.0).max(j - (st[1] + st[3]) as f64).max(0.0);
                    dx.max(dy)
                })
                .fold(f64::MAX, f64::min);
            if gap > 2.0 {
                r.add("great chambers or solars over 2 squares from the lord's stair", gap, format!("{tag} {}: the {} {gap:.0} squares from the stair", lv.name, room.kind));
            }
            if let Some(lo) = low.filter(|_| room.kind == "solar") {
                let c = mid(&squares(lv, ri));
                let (ds, dl) = (dist(c, sc), dist(c, lo));
                if ds > dl {
                    r.add("solars nearer the hall's low end than the lord's stair (their middles)", ds - dl, format!("{tag} {}: {ds:.1} from the stair, {dl:.1} from the low end", lv.name));
                }
            }
        }
    }
    if lords == 0 {
        r.add("keeps with no great chamber or solar", 1.0, tag.clone());
    }
    // A hall open two storeys: its gallery reached from the stair through passages only.
    if let Some(up) = it.levels.get(hl + 1).filter(|up| up.rooms.iter().any(|ro| ro.kind == interior::OPEN_BELOW && ro.squares > 0))
        && let Some(gal) = up.rooms.iter().position(|ro| ro.kind == "minstrels' gallery")
    {
        let start = up.cells[st[1] * nx + st[0]];
        let mut seen = vec![false; up.rooms.len()];
        let mut stack = vec![start];
        while let Some(a) = stack.pop() {
            if a < 0 || seen[a as usize] || !interior::is_circulation(up.rooms[a as usize].kind) {
                continue;
            }
            seen[a as usize] = true;
            for d in up.doors.iter().filter(|d| d.rooms.contains(&a)) {
                stack.push(if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] });
            }
        }
        if !seen[gal] {
            r.add("minstrels' galleries not reached along passages", 1.0, format!("{tag} {}: from the {}", up.name, if start >= 0 { up.rooms[start as usize].kind } else { "-" }));
        }
    }
}

// ---- Great halls furnished as wholes. ----

pub(crate) const HALL_COUNTS: [&str; 25] = [
    "castle halls and throne rooms with no throne dais",
    "town, guild, estate, banqueting halls and audience chambers of 5 x 3 or more with no dais",
    "estates' halls laid out by zones with no dais",
    "daises not at the far end",
    "daises off the axis",
    "halls of 6 x 5 or more with no carpet runner (castle, town, guild, estate)",
    "runners off the axis",
    "hall pieces with no mirror across the axis",
    "halls with a square shut off",
    "chairs of office not on a dais",
    "estates' and banqueting halls' high tables not on their dais",
    "mess halls and ballrooms with a dais, a throne, a high table or a chair of office",
    "open halls with no musicians on the gallery",
    "long galleries under 3 squares across",
    "long gallery pieces off its long walls or across its walk",
    "long galleries with their walk cut",
    "chapels with no altar on the axis at the far end",
    "chapel pews facing other than the altar",
    "chapel pews across the aisle",
    "audience chambers with no chair of state on their dais",
    "thrones outside castles and palaces",
    "castles flying more than one house",
    "banners in a castle not of its house's colours given",
    "halls of 6 x 5 or more with no tables",
    "pillars before the hearth, a hanging or a door, or across the way by a long wall",
];

/// One of a castle's buildings (its keep, or in its ward).
pub(crate) fn of_castle(b: &town::Building) -> bool {
    b.keep.is_some() || b.ward == Ward::Castle || is_keep(b)
}

/// `halls`: the great halls furnished as wholes (`interior::hall_plans`) and the thrones and banners
/// of every building (`hall_rules`), each layout of the world.
pub(crate) fn halls(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["halls furnished as wholes"]);
    r.counts(&HALL_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        layout_halls(world, t0, &l, li, &format!("#{li}"), &|_| true, r);
    }
}

/// The hall rules over a layout's buildings (`only`: those to look at): each interior's thrones,
/// every banner of a castle's buildings one house, and each hall furnished as a whole (`hall_rules`).
pub(crate) fn layout_halls(world: &World, t0: &T0, l: &Layout, li: usize, tag: &str, only: &dyn Fn(&town::Building) -> bool, r: &mut Report) {
    let mut houses: BTreeMap<u8, String> = BTreeMap::new();
    for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && only(b)) {
        let id = format!("b:{li}:{}", b.id);
        let Some(it) = interior::generate_id(world, t0, &id) else { continue };
        let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
        let thrones = of_castle(b) || key == Some("palace");
        for lv in &it.levels {
            for f in lv.furniture.iter().filter(|f| matches!(f.kind, "throne" | "throne_dais")) {
                if !thrones {
                    r.add("thrones outside castles and palaces", 1.0, format!("{tag} {id} ({}) {}: a {} at ({}, {})", it.function, lv.name, f.kind, f.x, f.y));
                }
            }
            if of_castle(b) {
                for f in lv.furniture.iter().filter(|f| matches!(f.kind, "wall_banner" | "standing_banner")) {
                    match f.variant {
                        Some(v) => {
                            houses.entry(v).or_insert_with(|| format!("{id} ({})", it.function));
                        }
                        None => r.add("banners in a castle not of its house's colours given", 1.0, format!("{tag} {id} {}: ({}, {})", lv.name, f.x, f.y)),
                    }
                }
            }
        }
        let zoned = interior::hub_kind(t0, l, b).is_some();
        for plan in interior::hall_plans(world, t0, li, b.id as usize) {
            hall_rules(&it, &plan, &format!("{tag} {id} ({})", it.function), zoned, r);
        }
    }
    if houses.len() > 1 {
        r.add("castles flying more than one house", houses.len() as f64, format!("{tag}: {:?}", houses));
    }
}

/// A hall furnished as a whole keeps its composition: the dais (a castle's throne dais, a low dais
/// elsewhere, none in a mess hall) at the far end of its frame on the axis, a chair of office on it, an
/// estate's high table on it; the runner on the axis; every table, bench, pillar, banner, tapestry,
/// brazier and high table mirrored across the axis (but a tapestry across from the hearth); no square
/// of its floor shut off by what blocks the way; an open hall's gallery with its musicians; an
/// audience chamber's chair of state on its dais. A long gallery's pieces along its long walls, its
/// walk clear; a chapel's altar at its far end on the axis, its pews facing it either side of the
/// aisle. `zoned`: the building is laid out by zones (an estate's hall has its dais there).
pub(crate) fn hall_rules(it: &interior::Interior, plan: &interior::HallPlan, tag: &str, zoned: bool, r: &mut Report) {
    use interior::hall::Style;
    let (nx, ny) = (it.nx, it.ny);
    let lv = &it.levels[plan.level];
    let f = plan.frame;
    let style = plan.style;
    let name = match style {
        Style::Castle { .. } => "castle",
        Style::Civic { .. } => "town or guild",
        Style::Estate { .. } => "estate",
        Style::Mess { .. } => "mess",
        Style::Throne { .. } => "throne room",
        Style::Banquet { .. } => "banqueting",
        Style::Ballroom => "ballroom",
        Style::Presence { .. } => "audience chamber",
        Style::Gallery => "long gallery",
        Style::Chapel => "chapel",
        // (A temple's, a shrine's and a monastery's: the `temples` rules.)
        Style::Nave { .. } | Style::Sanctuary { .. } | Style::SideChapel { .. } | Style::Shrine { .. } | Style::Refectory => return,
    };
    r.tally("halls furnished as wholes", 1);
    r.tally(&format!("  {name} halls"), 1);
    let tag = format!("{tag} {}", lv.name);
    r.add(&format!("  the biggest {name} halls (frame squares)"), (f.len * f.wid) as f64, format!("{tag}: {} x {}", f.len, f.wid));
    // (The rules for a hall big enough for them: a dais in a frame 5 long and 3 across or more, a
    // runner and tables in one 6 by 5.)
    let roomy = f.len >= 5 && f.wid >= 3;
    let big = f.len >= 6 && f.wid >= 5;
    let ri = plan.room;
    let in_room = |x: u16, y: u16| (x as usize) < nx && (y as usize) < ny && lv.cells[y as usize * nx + x as usize] == ri;
    let items: Vec<&interior::Item> = lv.furniture.iter().filter(|it| in_room(it.x, it.y)).collect();
    // An item's box in the frame: s0, s1, t0, t1 (inclusive).
    let local = |it: &interior::Item| {
        let (a, b) = (f.local(it.x as isize, it.y as isize), f.local((it.x + it.w) as isize - 1, (it.y + it.h) as isize - 1));
        (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1))
    };
    let w = f.wid as isize;
    let off_axis = |b: (isize, isize, isize, isize)| ((b.2 + b.3) as f64 * 0.5 - (w - 1) as f64 * 0.5).abs() > 0.01;
    // A long gallery: 3 squares across or more, its pieces a square deep against its long walls, a
    // way down its middle from end to end.
    if style == Style::Gallery {
        if f.wid < 3 {
            r.add("long galleries under 3 squares across", f.wid as f64, format!("{tag}: {} x {}", f.len, f.wid));
        }
        for p in &items {
            let b = local(p);
            if !(b.2 == b.3 && (b.2 == 0 || b.2 == w - 1)) {
                r.add("long gallery pieces off its long walls or across its walk", 1.0, format!("{tag}: the {} at ({}, {})", p.name, p.x, p.y));
            }
        }
        let blocked_at = |s: isize, t: isize| items.iter().any(|p| p.blocks_move && local(p).0 <= s && s <= local(p).1 && local(p).2 <= t && t <= local(p).3);
        let cut = (0..f.len as isize).filter(|&s| (1..w - 1).any(|t| blocked_at(s, t))).count();
        if f.wid >= 3 && cut > 0 {
            r.add("long galleries with their walk cut", cut as f64, format!("{tag}: {cut} rows"));
        }
        return;
    }
    // A chapel: the altar on the axis at its far end (the end away from its way in: the end wall
    // a door opens in, its main door, else its doors on the mean; either where that is midway),
    // the pews facing it (their backs to the way in) and clear of the aisle.
    if style == Style::Chapel {
        let mid = 0.5 * (f.len as f64 - 1.0);
        let m = interior::way_in(lv, nx, ny, ri, it.stairs, f).unwrap_or(mid);
        let far = |b: (isize, isize, isize, isize)| (m <= mid + 0.01 && b.1 >= f.len as isize - 1) || (m >= mid - 0.01 && b.0 <= 0);
        match items.iter().find(|p| p.kind == "altar") {
            Some(a) if far(local(a)) && !off_axis(local(a)) => {}
            _ => r.add("chapels with no altar on the axis at the far end", 1.0, format!("{tag}: a {} x {} frame, entered at {m:.1}", f.len, f.wid)),
        }
        // (The way in: the frame's -s step, as `Item::back` codes it.)
        let (gi, gj) = (-f.ds.0, -f.ds.1);
        let way_in = match (gi, gj) {
            (0, -1) => 0,
            (1, 0) => 1,
            (0, 1) => 2,
            _ => 3,
        };
        let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
        for p in items.iter().filter(|p| p.kind == "pew") {
            if p.back != Some(way_in) {
                r.add("chapel pews facing other than the altar", 1.0, format!("{tag}: the pew at ({}, {})", p.x, p.y));
            }
            let b = local(p);
            if b.2 <= cr && b.3 >= cl {
                r.add("chapel pews across the aisle", 1.0, format!("{tag}: the pew at ({}, {})", p.x, p.y));
            }
        }
        return;
    }
    let mess = matches!(style, Style::Mess { .. } | Style::Ballroom | Style::Gallery | Style::Chapel);
    let throne = matches!(style, Style::Castle { .. } | Style::Throne { .. });
    let dais_kind = if throne { "throne_dais" } else { "low_dais" };
    let daises: Vec<&&interior::Item> = items.iter().filter(|it| it.kind == dais_kind).collect();
    if mess {
        if items.iter().any(|it| matches!(it.kind, "throne_dais" | "low_dais" | "throne" | "high_table" | "chair_of_office")) {
            r.add("mess halls and ballrooms with a dais, a throne, a high table or a chair of office", 1.0, tag.clone());
        }
    } else if let Some(d) = daises.first() {
        let b = local(d);
        if b.1 < f.len as isize - 3 {
            r.add("daises not at the far end", (f.len as isize - 1 - b.1) as f64, format!("{tag}: rows {}-{} of {}", b.0, b.1, f.len));
        }
        if off_axis(b) {
            r.add("daises off the axis", 1.0, format!("{tag}: across {}-{} of {}", b.2, b.3, f.wid));
        }
    } else if roomy || throne {
        r.add(if throne { "castle halls and throne rooms with no throne dais" } else { "town, guild, estate, banqueting halls and audience chambers of 5 x 3 or more with no dais" }, 1.0, format!("{tag}: a {} x {} frame", f.len, f.wid));
        if zoned && matches!(style, Style::Estate { .. }) && roomy {
            r.add("estates' halls laid out by zones with no dais", 1.0, format!("{tag}: a {} x {} frame", f.len, f.wid));
        }
    }
    if matches!(style, Style::Presence { .. }) && !daises.is_empty() && !items.iter().any(|c| c.kind == "chair_of_office" && daises.iter().any(|d| d.x <= c.x && c.x < d.x + d.w && d.y <= c.y && c.y < d.y + d.h)) {
        r.add("audience chambers with no chair of state on their dais", 1.0, tag.clone());
    }
    let on_dais = |it: &interior::Item| {
        daises.iter().any(|d| {
            let (a, b) = (local(d), local(it));
            b.0 >= a.0 && b.1 <= a.1 && b.2 >= a.2 && b.3 <= a.3
        })
    };
    for c in items.iter().filter(|it| it.kind == "chair_of_office") {
        if !on_dais(c) {
            r.add("chairs of office not on a dais", 1.0, format!("{tag}: ({}, {})", c.x, c.y));
        }
    }
    if matches!(style, Style::Estate { .. } | Style::Banquet { .. }) {
        for t in items.iter().filter(|it| it.kind == "high_table") {
            if !on_dais(t) {
                r.add("estates' and banqueting halls' high tables not on their dais", 1.0, format!("{tag}: ({}, {})", t.x, t.y));
            }
        }
    }
    // A ballroom's dance floor and musicians on the axis.
    if style == Style::Ballroom && big {
        for kind in ["dance_floor", "musicians_corner"] {
            match items.iter().find(|it| it.kind == kind) {
                Some(it) if off_axis(local(it)) => r.add("ballrooms' dance floors or musicians off the axis", 1.0, format!("{tag}: the {}", it.name)),
                Some(_) => {}
                None => r.add("ballrooms of 6 x 5 or more with no dance floor or musicians", 1.0, format!("{tag}: no {kind} in a {} x {} frame", f.len, f.wid)),
            }
        }
    }
    if !mess && !daises.is_empty() {
        match items.iter().find(|it| it.kind == "carpet_runner") {
            Some(run) if off_axis(local(run)) => r.add("runners off the axis", 1.0, tag.clone()),
            Some(_) => {}
            None if big => r.add("halls of 6 x 5 or more with no carpet runner (castle, town, guild, estate)", 1.0, format!("{tag}: a {} x {} frame", f.len, f.wid)),
            None => {}
        }
    }
    if big && !matches!(style, Style::Throne { .. } | Style::Ballroom | Style::Presence { .. }) && !items.iter().any(|it| it.kind == "long_table") {
        r.add("halls of 6 x 5 or more with no tables", 1.0, format!("{tag} ({name}): a {} x {} frame", f.len, f.wid));
    }
    // Mirrors.
    let mirrored = ["long_table", "bench", "pillar", "wall_banner", "tapestry", "standing_banner", "brazier", "high_table", "harpsichord", "refreshment_table"];
    let hearths: Vec<(isize, isize, isize, isize)> = items.iter().filter(|it| it.kind == "hearth").map(|it| local(it)).collect();
    for it in items.iter().filter(|it| mirrored.contains(&it.kind) && !(matches!(style, Style::Estate { .. } | Style::Banquet { .. }) && it.kind == "high_table")) {
        let b = local(it);
        let m = (b.0, b.1, w - 1 - b.3, w - 1 - b.2);
        let found = items.iter().any(|o| o.kind == it.kind && local(o) == m) || (it.kind == "tapestry" && hearths.contains(&m));
        if !found {
            r.add("hall pieces with no mirror across the axis", 1.0, format!("{tag}: the {} at ({}, {})", it.name, it.x, it.y));
        }
    }
    // Pillars: the squares between each and its long wall on its row clear (no hearth, nothing hung
    // there, nothing across the way by the wall, no door's square) and none in a door's way (no
    // door's square next to it).
    let held_at = |s: isize, t: isize| items.iter().any(|o| o.kind != "pillar" && local(o).0 <= s && s <= local(o).1 && local(o).2 <= t && t <= local(o).3);
    let mut door = vec![false; nx * ny];
    for d in &lv.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        for (i, j) in if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] } {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && in_room(i as u16, j as u16) {
                door[j as usize * nx + i as usize] = true;
            }
        }
    }
    let door_at = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && door[j as usize * nx + i as usize];
    for p in items.iter().filter(|it| it.kind == "pillar") {
        let (s, t) = (local(p).0, local(p).2);
        let (i, j) = (p.x as isize, p.y as isize);
        if (if 2 * t < w - 1 { 0..t } else { t + 1..w }).any(|q| held_at(s, q) || door_at(f.grid(s, q))) || [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].into_iter().any(door_at) {
            r.add("pillars before the hearth, a hanging or a door, or across the way by a long wall", 1.0, format!("{tag}: the pillar at ({}, {})", p.x, p.y));
        }
    }
    // Nothing shut off: from the hall's ways in (its doors, the stairs), every square nothing blocking
    // stands on.
    let mut blocked = vec![false; nx * ny];
    for it in &lv.furniture {
        if it.blocks_move {
            for j in it.y as usize..(it.y + it.h) as usize {
                for i in it.x as usize..(it.x + it.w) as usize {
                    blocked[j * nx + i] = true;
                }
            }
        }
    }
    let mut starts: Vec<usize> = lv.doors.iter().flat_map(door_squares).filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] == ri).collect();
    let st = it.stairs;
    if lv.has_stairs {
        starts.extend((st[1]..st[1] + st[3]).flat_map(|j| (st[0]..st[0] + st[2]).map(move |i| j * nx + i)).filter(|&k| lv.cells[k] == ri));
    }
    if let Some(&k0) = starts.first() {
        let mut seen = vec![false; nx * ny];
        let mut stack = vec![k0];
        seen[k0] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < nx && b < ny && !seen[b * nx + a] && lv.cells[b * nx + a] == ri && !blocked[b * nx + a] {
                    seen[b * nx + a] = true;
                    stack.push(b * nx + a);
                }
            }
        }
        let shut = (0..nx * ny).filter(|&k| lv.cells[k] == ri && !blocked[k] && !seen[k]).count() + starts.iter().filter(|&&k| !seen[k]).count();
        if shut > 0 {
            r.add("halls with a square shut off", shut as f64, format!("{tag}: {shut} squares"));
        }
    }
    // An open hall's gallery: its musicians (a castle's, a palace's, an estate's).
    if matches!(style, Style::Castle { .. } | Style::Estate { .. })
        && let Some(up) = it.levels.get(plan.level + 1).filter(|up| up.rooms.iter().any(|ro| ro.kind == interior::OPEN_BELOW && ro.squares > 0))
        && let Some(g) = up.rooms.iter().position(|ro| ro.kind == "minstrels' gallery")
        && !up.furniture.iter().any(|it| it.kind == "musicians_corner" && up.cells[it.y as usize * nx + it.x as usize] == g as i16)
    {
        r.add("open halls with no musicians on the gallery", 1.0, format!("{tag}: {}", up.name));
    }
}

// ---- Palaces and noble estates. ----

pub(crate) const PALACE_COUNTS: [&str; 31] = [
    "capital palaces not round a court",
    "capital palaces' courts under 7 squares across or deep",
    "capital palaces' fronts an even number of squares across",
    "capital palaces with no walled forecourt",
    "capital forecourts' gates on no street and with no way on to one",
    "capital forecourts' and their ways' squares over water",
    "capital palaces' royal floors whose great chamber or bedchamber isn't the biggest room but the gallery",
    "capital palaces' royal floors' wardrobes over 15 squares",
    "older palaces' great halls with no high table",
    "city palaces as big as their realm's capital's or bigger",
    "capital palaces' state floor not guard room, audience chamber, throne room",
    "capital palaces' guard rooms under 12 squares or over 40",
    "palaces' rooms of their programme twice on a floor (great chamber, bedchamber)",
    "long galleries under 3 squares across (palaces)",
    "older palaces with a ballroom or a banqueting hall",
    "manors without a kitchen of 12 squares, a buttery and a pantry of 6",
    "manors' great halls over 35% of the ground floor",
    "floors the zones fell back on",
    "breaking an interior rule (as in interiors)",
    "missing a room of their programme",
    "capital palaces laid out as older ones",
    "capital palaces' front door off the axis",
    "capital palaces' floors not mirrored across the axis (the ground floor's front range)",
    "capital palaces' state rooms not one into the next (the landing round to the throne room)",
    "capital palaces' state rooms' doors not mirrored across the axis",
    "throne rooms' dais off the palace's axis",
    "throne rooms with no throne dais",
    "rooms longer than 2.5:1 (but long galleries)",
    "rooms filling under 60% of their box",
    "rooms over 60 squares (but halls, state rooms, courts, galleries, chapels)",
    "rooms a square wide",
];

/// `palaces`: every palace and noble estate laid out by zones (`palace_rules`).
pub(crate) fn palaces(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["palaces and estates laid out by zones"]);
    r.counts(&PALACE_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    // (The capital's palace's floor and the biggest other palace's (sq ft), with where it stands.)
    let (mut capital, mut city) = (None::<f64>, None::<(f64, String)>);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        for b in &l.buildings {
            if town::palace::is_palace(b) && b.structure == town::Structure::Roofed {
                let a = town::geom::area(&b.poly).abs();
                if t0.settlements.get(li).is_some_and(|s| s.capital) {
                    capital = Some(capital.map_or(a, |c| c.max(a)));
                } else if city.as_ref().is_none_or(|c| a > c.0) {
                    city = Some((a, format!("b:{li}:{}", b.id)));
                }
                // (Its formal garden beside it: `palace::gardens`.)
                r.tally("palaces", 1);
                let garden = l.parks.iter().find(|pk| pk.kind == town::ParkKind::Formal && pk.pts.iter().any(|p| (0..b.poly.len()).any(|k| town::geom::seg_dist(*p, b.poly[k], b.poly[(k + 1) % b.poly.len()]) < 12.0)));
                if let Some(pk) = garden {
                    r.tally("palaces with a formal garden beside them", 1);
                    r.tally("capital palaces with a formal garden beside them", t0.settlements.get(li).is_some_and(|s| s.capital) as usize);
                    r.value("palace garden area (sq ft)", town::geom::area(&pk.pts).abs(), format!("b:{li}:{}", b.id));
                }
            }
            let Some(kind) = interior::hub_kind(t0, &l, b).filter(|k| !k.is_keep() && !k.is_faith() && !k.is_civic() && !k.is_inn()) else { continue };
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            palace_rules(world, t0, &l, li, b, kind, buckets, r);
        }
    }
    if let (Some(c), Some((a, id))) = (capital, city)
        && a >= c
    {
        r.add("city palaces as big as their realm's capital's or bigger", a / c, format!("{id}: {a:.0} sq ft, the capital's {c:.0}"));
    }
}

/// The rules a palace or a noble estate laid out by zones keeps (`palaces`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn palace_rules(world: &World, t0: &T0, l: &Layout, li: usize, b: &town::Building, kind: interior::hub::Kind, buckets: &HashMap<(i64, i64), Vec<usize>>, r: &mut Report) {
    use interior::hub::Kind;
    let id = format!("b:{li}:{}", b.id);
    let Some(it) = interior::generate_id(world, t0, &id) else { return };
    r.tally("palaces and estates laid out by zones", 1);
    r.tally(&format!("  {}s", kind.name()), 1);
    let tag = format!("{id} ({})", kind.name());
    let (nx, ny) = (it.nx, it.ny);
    let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
    if !fell.is_empty() {
        r.add("floors the zones fell back on", fell.len() as f64, format!("{tag}: z {fell:?}"));
    }
    let bad = check_interior(world, t0, l, li, &id, &it, buckets);
    if !bad.is_empty() {
        r.add("breaking an interior rule (as in interiors)", bad.len() as f64, format!("{tag}: {}", bad.join("; ")));
    }
    // The programme.
    let has = |k: &str| it.levels.iter().any(|lv| lv.rooms.iter().any(|ro| ro.kind == k && ro.squares > 0));
    let plan = interior::zone_plan_of(world, t0, li, b.id as usize);
    let court = plan.as_ref().and_then(|p| p.palace()).is_some_and(|z| z.3);
    let squares = interior::Shell::of(l, b).inside.iter().filter(|&&i| i).count();
    let want: Vec<&str> = match kind {
        Kind::Palace => {
            let mut w = vec!["entrance hall", "guardroom", "throne room", "audience chamber", "ballroom", "banqueting hall", "royal chapel", "kitchen", "antechamber"];
            if b.floors >= 3 {
                w.push("long gallery");
            }
            if court {
                w.push("courtyard");
            }
            w
        }
        Kind::OldPalace => {
            let mut w = vec!["great hall", "guardroom", "chapel", "audience chamber", "solar", "kitchen"];
            if b.floors >= 3 {
                w.push("long gallery");
            }
            w
        }
        Kind::Manor => {
            let mut w = vec!["great hall", "kitchen", "parlor", "stair hall", "great chamber", "solar"];
            if squares >= 260 {
                w.push("chapel");
            }
            w
        }
        _ => vec!["great hall", "kitchen", "parlor", "solar"],
    };
    let missing: Vec<&str> = want.iter().copied().filter(|k| !has(k)).collect();
    if !missing.is_empty() {
        r.add("missing a room of their programme", missing.len() as f64, format!("{tag}: no {}", missing.join(", ")));
    }
    // Rooms of sound proportions.
    let big = |k: &str| matches!(k, "great hall" | "banqueting hall" | "throne room" | "ballroom" | "audience chamber" | "courtyard" | "royal chapel" | "chapel" | "entrance hall" | "long gallery");
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        for (ri, ro) in lv.rooms.iter().enumerate() {
            if ro.squares == 0 || !interior::walkable(ro.kind) || (interior::is_circulation(ro.kind) && ro.kind != "long gallery") {
                continue;
            }
            let sq: Vec<usize> = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let named = format!("{tag} {}: the {} ({} squares, {w} x {h})", lv.name, ro.kind, sq.len());
            if ro.kind != "long gallery" && 2 * w.max(h) > 5 * w.min(h) {
                r.add("rooms longer than 2.5:1 (but long galleries)", w.max(h) as f64 / w.min(h) as f64, named.clone());
            }
            if sq.len() * 5 < w * h * 3 {
                r.add("rooms filling under 60% of their box", sq.len() as f64 / (w * h) as f64, named.clone());
            }
            if sq.len() > 60 && !big(ro.kind) {
                r.add("rooms over 60 squares (but halls, state rooms, courts, galleries, chapels)", sq.len() as f64, named.clone());
            }
            let wide = sq.iter().any(|&k| k % nx + 1 < nx && [k + 1, k + nx, k + nx + 1].iter().all(|q| sq.contains(q)));
            if !wide && sq.len() >= 2 {
                r.add("rooms a square wide", sq.len() as f64, named);
            }
        }
    }
    // Long galleries 3 squares across or more; a palace's great chamber and bedchamber once a floor.
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, ro)| ro.kind == "long gallery" && ro.squares > 0) {
            let sq: Vec<usize> = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            let across = (x1 - x0 + 1).min(y1 - y0 + 1);
            if across < 3 {
                r.add("long galleries under 3 squares across (palaces)", across as f64, format!("{tag} {}: {} x {}", lv.name, x1 - x0 + 1, y1 - y0 + 1));
            }
        }
        if matches!(kind, Kind::Palace | Kind::OldPalace) {
            for k in ["great chamber", "bedchamber"] {
                let n = lv.rooms.iter().filter(|ro| ro.kind == k && ro.squares > 0).count();
                if n > 1 {
                    r.add("palaces' rooms of their programme twice on a floor (great chamber, bedchamber)", n as f64, format!("{tag} {}: {n} {k}s", lv.name));
                }
            }
        }
    }
    match kind {
        Kind::OldPalace => {
            if has("ballroom") || has("banqueting hall") {
                r.add("older palaces with a ballroom or a banqueting hall", 1.0, tag.clone());
            }
            // (Its great hall serves as the banqueting hall: its high table.)
            for lv in &it.levels {
                for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, ro)| ro.kind == "great hall" && ro.squares > 0) {
                    if !lv.furniture.iter().any(|f| f.kind == "high_table" && lv.cells[f.y as usize * nx + f.x as usize] == ri as i16) {
                        r.add("older palaces' great halls with no high table", 1.0, format!("{tag} {}", lv.name));
                    }
                }
            }
        }
        Kind::Manor => {
            // A hall with a service wing: kitchen, buttery and pantry of sound size, the hall a third
            // of the ground floor or so.
            if let Some(g) = it.levels.iter().find(|lv| lv.z == 0) {
                let size = |k: &str| g.rooms.iter().filter(|ro| ro.kind == k).map(|ro| ro.squares as usize).max().unwrap_or(0);
                if size("kitchen") < 12 || size("buttery") < 6 || size("pantry") < 6 {
                    r.add("manors without a kitchen of 12 squares, a buttery and a pantry of 6", 1.0, format!("{tag}: kitchen {}, buttery {}, pantry {}", size("kitchen"), size("buttery"), size("pantry")));
                }
                let all: usize = g.rooms.iter().map(|ro| ro.squares as usize).sum();
                let hall = size("great hall");
                if hall * 100 > 35 * all {
                    r.add("manors' great halls over 35% of the ground floor", hall as f64 / all as f64, format!("{tag}: {hall} of {all} squares"));
                }
            }
        }
        _ => {}
    }
    if kind != Kind::Palace {
        return;
    }
    // The capital's palace: laid out as one, mirrored across its axis.
    let Some((f, (front, _, _, _))) = plan.as_ref().and_then(|p| p.palace().map(|z| (p.frame(), z))) else {
        r.add("capital palaces laid out as older ones", 1.0, format!("{tag}: {}", plan.as_ref().map_or("no plan", |p| p.kind().name())));
        return;
    };
    if plan.as_ref().and_then(|p| p.palace()).is_some_and(|z| z.3) {
        r.tally("  capital palaces round a court", 1);
    } else {
        r.add("capital palaces not round a court", 1.0, tag.clone());
    }
    if let Some((_, back, side, true)) = plan.as_ref().and_then(|p| p.palace()) {
        let (across, deep) = (f.wid - 2 * side, back - front);
        if across < 7 || deep < 7 {
            r.add("capital palaces' courts under 7 squares across or deep", across.min(deep) as f64, format!("{tag}: {across} x {deep}"));
        }
    }
    if f.wid % 2 == 0 {
        r.add("capital palaces' fronts an even number of squares across", f.wid as f64, tag.clone());
    }
    if interior::court_plan(world, t0, l, b).is_none_or(|c| c.forecourt.is_none()) {
        r.add("capital palaces with no walled forecourt", 1.0, tag.clone());
    }
    // (Its gate on a street, else a paved way on to one.)
    if let Some(fc) = l.forecourts.iter().find(|fc| fc.palace == b.id && fc.yard.len() == 4) {
        let m = [0.5 * (fc.yard[0][0] + fc.yard[3][0]), 0.5 * (fc.yard[0][1] + fc.yard[3][1])];
        let on = l.roads.iter().any(|(pts, _, w)| pts.windows(2).any(|s| town::geom::seg_dist(m, s[0], s[1]) <= 0.5 * w + 1.0));
        if fc.way.is_empty() && !on {
            r.add("capital forecourts' gates on no street and with no way on to one", 1.0, tag.clone());
        }
        // (Its yard and its paved way on dry ground: the squares whose middles are in the yard or
        // within 6 ft of the way's centre line, on the water.)
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
        let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
        let bb = bbox(&fc.yard.iter().chain(&fc.way).copied().collect::<Vec<P>>());
        let (mut yard, mut way) = (0usize, 0usize);
        for j in (libm::floor(bb[1] / 5.0) as i64 - 2)..=(libm::floor(bb[3] / 5.0) as i64 + 2) {
            for i in (libm::floor(bb[0] / 5.0) as i64 - 2)..=(libm::floor(bb[2] / 5.0) as i64 + 2) {
                let c = [i as f64 * 5.0 + 2.5, j as f64 * 5.0 + 2.5];
                if town::geom::contains(&fc.yard, c) {
                    yard += wet(c) as usize;
                } else if fc.way.windows(2).any(|s| town::geom::seg_dist(c, s[0], s[1]) < 6.0) {
                    way += wet(c) as usize;
                }
            }
        }
        if yard + way > 0 {
            r.add("capital forecourts' and their ways' squares over water", (yard + way) as f64, format!("{tag}: yard {yard}, way {way}"));
        }
    }
    // The royal floor: the great chamber and the bedchamber its biggest rooms but the gallery, the
    // wardrobes small.
    if let Some(lv) = it.levels.iter().find(|lv| lv.name == "Royal floor") {
        let size = |k: &str| lv.rooms.iter().filter(|ro| ro.kind == k).map(|ro| ro.squares as usize).max().unwrap_or(0);
        let royal = size("great chamber").min(size("bedchamber"));
        let other = lv.rooms.iter().filter(|ro| !matches!(ro.kind, "great chamber" | "bedchamber" | "long gallery") && !interior::is_circulation(ro.kind) && interior::walkable(ro.kind)).map(|ro| ro.squares as usize).max().unwrap_or(0);
        if royal <= other {
            r.add("capital palaces' royal floors whose great chamber or bedchamber isn't the biggest room but the gallery", royal as f64, format!("{tag}: {royal} squares, another {other}"));
        }
        if let Some(w) = lv.rooms.iter().filter(|ro| ro.kind == "wardrobe" && ro.squares > 15).map(|ro| ro.squares).max() {
            r.add("capital palaces' royal floors' wardrobes over 15 squares", w as f64, format!("{tag}: {w} squares"));
        }
    }
    // Its guard rooms of sound size; on the state floor the throne room reached from the landing
    // only through a guard room, then the audience chamber (or the ballroom across the axis), then a
    // guard room again.
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        for ro in lv.rooms.iter().filter(|ro| ro.kind == "guardroom" && ro.squares > 0) {
            if ro.squares < 12 || ro.squares > 40 {
                r.add("capital palaces' guard rooms under 12 squares or over 40", ro.squares as f64, format!("{tag} {}", lv.name));
            }
        }
    }
    if let Some(lv) = it.levels.iter().find(|lv| lv.z == 1) {
        let room_of = |k: &str| (0..lv.rooms.len()).find(|&ri| lv.rooms[ri].kind == k && lv.rooms[ri].squares > 0).map(|ri| ri as i16);
        let state = |k: &str| matches!(k, "landing" | "guardroom" | "antechamber" | "audience chamber" | "ballroom" | "throne room");
        // (The rooms reached from the landing through state rooms, but those of kinds `skip`.)
        let reach = |skip: &[&str]| {
            let mut seen = vec![false; lv.rooms.len()];
            let mut stack: Vec<i16> = room_of("landing").into_iter().collect();
            for &s in &stack {
                seen[s as usize] = true;
            }
            while let Some(a) = stack.pop() {
                for d in lv.doors.iter().filter(|d| d.rooms.contains(&a)) {
                    let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
                    if o >= 0 && !seen[o as usize] && state(lv.rooms[o as usize].kind) && !skip.contains(&lv.rooms[o as usize].kind) {
                        seen[o as usize] = true;
                        stack.push(o);
                    }
                }
            }
            seen
        };
        let throne = room_of("throne room");
        let guarded = throne.is_some_and(|t| lv.doors.iter().any(|d| d.rooms.contains(&t) && d.rooms.iter().any(|&o| o >= 0 && lv.rooms[o as usize].kind == "guardroom")));
        let ok = guarded
            && room_of("audience chamber").is_some_and(|a| !reach(&["guardroom"])[a as usize])
            && throne.is_some_and(|t| !reach(&["audience chamber", "ballroom"])[t as usize] && reach(&[])[t as usize]);
        if !ok {
            r.add("capital palaces' state floor not guard room, audience chamber, throne room", 1.0, tag.clone());
        }
    }
    let w = f.wid as isize;
    let mirror = |(i, j): (isize, isize)| {
        let (s, t) = f.local(i, j);
        f.grid(s, w - 1 - t)
    };
    let cell = |lv: &interior::Level, (i, j): (isize, isize)| if i < 0 || j < 0 || i as usize >= nx || j as usize >= ny { -1 } else { lv.cells[j as usize * nx + i as usize] };
    if let Some(g0) = it.levels.iter().find(|lv| lv.z == 0)
        && let Some(d) = g0.doors.iter().find(|d| d.kind == "front")
    {
        let sq = door_squares(d);
        let inner = sq.iter().copied().find(|&p| cell(g0, p) >= 0).unwrap_or(sq[0]);
        let (_, t) = f.local(inner.0, inner.1);
        if (2 * t - (w - 1)).abs() > 1 {
            r.add("capital palaces' front door off the axis", 1.0, format!("{tag}: across {t} of {w}"));
        }
    }
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        // (Each square's room and its mirror's: one room to one room.)
        let (mut to, mut from): (HashMap<i16, i16>, HashMap<i16, i16>) = (HashMap::new(), HashMap::new());
        let mut broken = 0;
        for k in 0..nx * ny {
            let p = ((k % nx) as isize, (k / nx) as isize);
            let (s, _) = f.local(p.0, p.1);
            if lv.z == 0 && s >= front as isize {
                continue;
            }
            let (a, m) = (lv.cells[k], cell(lv, mirror(p)));
            if (a < 0) != (m < 0) {
                broken += 1;
                continue;
            }
            if a < 0 {
                continue;
            }
            if *to.entry(a).or_insert(m) != m || *from.entry(m).or_insert(a) != a {
                broken += 1;
            }
        }
        if broken > 0 {
            r.add("capital palaces' floors not mirrored across the axis (the ground floor's front range)", broken as f64, format!("{tag} {}: {broken} squares", lv.name));
        }
    }
    // The state floor: from the landing round to the throne room through the state rooms, both ways.
    if let Some(lv) = it.levels.iter().find(|lv| lv.z == 1) {
        let state = |k: &str| matches!(k, "landing" | "guardroom" | "antechamber" | "audience chamber" | "ballroom" | "throne room");
        let room_of = |k: &str| (0..lv.rooms.len()).find(|&ri| lv.rooms[ri].kind == k && lv.rooms[ri].squares > 0).map(|ri| ri as i16);
        let reach = |skip: Option<i16>| {
            let mut seen = vec![false; lv.rooms.len()];
            let mut stack: Vec<i16> = room_of("landing").into_iter().collect();
            for &s in &stack {
                seen[s as usize] = true;
            }
            while let Some(a) = stack.pop() {
                for d in lv.doors.iter().filter(|d| d.rooms.contains(&a)) {
                    let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
                    if o >= 0 && !seen[o as usize] && Some(o) != skip && state(lv.rooms[o as usize].kind) {
                        seen[o as usize] = true;
                        stack.push(o);
                    }
                }
            }
            seen
        };
        let throne = room_of("throne room");
        let ok = throne.is_some_and(|t| ["audience chamber", "ballroom"].iter().all(|side| room_of(side).is_some_and(|s| reach(room_of(if *side == "ballroom" { "audience chamber" } else { "ballroom" }))[t as usize] && reach(None)[s as usize])));
        if !ok {
            r.add("capital palaces' state rooms not one into the next (the landing round to the throne room)", 1.0, format!("{tag} {}", lv.name));
        }
        for d in lv.doors.iter().filter(|d| d.rooms.iter().all(|&ro| ro >= 0 && state(lv.rooms[ro as usize].kind))) {
            let sq = door_squares(d);
            let m = [mirror(sq[0]), mirror(sq[1])];
            if !lv.doors.iter().any(|o| {
                let q = door_squares(o);
                (q[0] == m[0] && q[1] == m[1]) || (q[0] == m[1] && q[1] == m[0])
            }) {
                r.add("capital palaces' state rooms' doors not mirrored across the axis", 1.0, format!("{tag} {}: the door at ({}, {})", lv.name, d.a[0], d.a[1]));
            }
        }
        match lv.furniture.iter().find(|it| it.kind == "throne_dais") {
            Some(dais) => {
                let (a, c) = (f.local(dais.x as isize, dais.y as isize), f.local((dais.x + dais.w) as isize - 1, (dais.y + dais.h) as isize - 1));
                if a.1 + c.1 != w - 1 {
                    r.add("throne rooms' dais off the palace's axis", 1.0, format!("{tag}: across {}-{} of {w}", a.1.min(c.1), a.1.max(c.1)));
                }
            }
            None => r.add("throne rooms with no throne dais", 1.0, tag.clone()),
        }
    }
}

// ---- The street editor. ----

/// A hand castle laid as a tower house (no bailey, its one building a small tower keep, on a patch
/// too small for a curtain): off the streets, walls and towers, on no building. False if not one.
pub(crate) fn tower_house(l: &Layout, tag: &str, r: &mut Report) -> bool {
    let keeps: Vec<&town::Building> = l.buildings.iter().filter(|b| b.ward == Ward::Castle).collect();
    if l.plazas.iter().any(|p| p.kind == PlazaKind::Bailey) || keeps.len() != 1 || keeps[0].keep != Some(town::KeepKind::Plain) {
        return false;
    }
    let b = keeps[0];
    let name = format!("{tag}: b:{}:{} at {}", l.index, b.id, at(centroid(&b.poly)));
    r.tally("placements as a tower house (a patch too small for a curtain)", 1);
    let wd = l.walls.iter().flat_map(|w| w.windows(2)).map(|s| if contains(&b.poly, s[0]) { -1.0 } else { seg_poly(s[0], s[1], &b.poly) }).fold(f64::MAX, f64::min);
    if wd < 4.0 {
        r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: {wd:.1} ft from a wall"));
    }
    let towers = l.towers.iter().chain(&l.gate_towers).chain(l.extra_towers.iter().map(|t| &t.0));
    if let Some(t) = towers.filter(|t| -depth(&b.poly, **t) < 12.0).next() {
        r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: a tower at {} within 12 ft", at(*t)));
    }
    if let Some(o) = l.buildings.iter().find(|o| o.ward != Ward::Castle && poly_overlap(&shrink(&b.poly, 0.5), &shrink(&o.poly, 0.5))) {
        r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: overlaps b:{}:{}", l.index, o.id));
    }
    let ways = l.roads.iter().map(|x| (&x.0, x.2)).chain(l.streets.iter().map(|s| (s, 18.0)));
    for (pts, w) in ways {
        let d = pts.windows(2).map(|s| if contains(&b.poly, s[0]) { 0.0 } else { seg_poly(s[0], s[1], &b.poly) }).fold(f64::MAX, f64::min);
        if d < 0.5 * w - 1.0 {
            r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: {d:.1} ft from a way {w:.0} ft wide"));
        }
    }
    true
}

/// Castles set in the street editor at the town wall, walls on and off.
pub(crate) fn editor(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&[
        "placements",
        "placements with no castle",
        "placements as a tower house (a patch too small for a curtain)",
        "tower houses within 4 ft of a wall, on a tower or a building, or on a street",
        "placements with any non-castle building changed",
        "non-castle buildings changed or gone, walls off against on",
        "new non-castle ids with the walls off",
        "towns with a castle, walls taken down",
        "their non-castle buildings changed or gone",
    ]);
    r.counts(&["castles", "castles with an inner curtain", "gate ways"]);
    r.counts(&CASTLE_COUNTS);
    r.counts(&CASTLE_MORE[..4]);
    r.counts(&["buildings laid out by zones"]);
    r.counts(&HUB_COUNTS);
    r.counts(&["halls furnished as wholes"]);
    r.counts(&HALL_COUNTS);
    r.counts(&MONASTERY_EDIT_COUNTS);
    let places: usize = std::env::var("PLACES").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let with = |ci: usize, edit: worldgen::world::TownEdit| {
        let mut file = base.file.clone();
        file.edits.towns.insert(town::wards::key(ci), edit);
        World::new(file).unwrap()
    };
    let n = t0.settlements.len();
    let mut placed = 0;
    for ci in (0..n).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        let g = town::layout(base, t0, ci);
        // Every town castle's town with its walls taken down: no other building changes.
        if !g.castles.is_empty() && !g.walls.is_empty() {
            if let Ok((Some(edit), _)) = town::wards::change(base, t0, ci, &serde_json::from_str(r#"{"walls":false}"#).unwrap()) {
                let off = town::layout(&with(ci, edit), t0, ci);
                r.tally("towns with a castle, walls taken down", 1);
                same_buildings(&g, &off, &format!("#{ci} (its castle, walls down)"), "their non-castle buildings changed or gone", r);
            }
        }
        if placed >= places || !g.castles.is_empty() || g.walls.is_empty() {
            continue;
        }
        let Ok(plan) = town::wards::plan_json(base, t0, ci) else { continue };
        // The patches in town nearest the town wall (by their middle), up to 3, 250 ft apart.
        let pt = |v: &serde_json::Value| [v["at"][0].as_f64().unwrap(), v["at"][1].as_f64().unwrap()];
        let to_wall = |p: P| g.walls.iter().flat_map(|w| w.windows(2)).map(|s| seg_dist(p, s[0], s[1])).fold(f64::MAX, f64::min);
        let mut near: Vec<&serde_json::Value> = plan["patches"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["in_town"] == true && matches!(p["ward"].as_str(), Some("common" | "craft" | "merchant" | "patriciate" | "slum")))
            .collect();
        near.sort_by(|a, b| to_wall(pt(a)).total_cmp(&to_wall(pt(b))));
        let mut picked: Vec<&serde_json::Value> = Vec::new();
        for p in near {
            if picked.len() < 3 && picked.iter().all(|q| dist(pt(q), pt(p)) > 250.0) {
                picked.push(p);
            }
        }
        for patch in picked {
            let mut both = Vec::new();
            for walls in ["", r#","walls":false"#] {
                let req = format!(r#"{{"patches":[{{"patch":{},"ward":"castle"}}]{walls}}}"#, patch["patch"]);
                let Ok((Some(edit), _)) = town::wards::change(base, t0, ci, &serde_json::from_str(&req).unwrap()) else { continue };
                let world = with(ci, edit);
                let l = town::layout(&world, t0, ci);
                let tag = format!("#{ci} p{} walls {}", patch["patch"], if walls.is_empty() { "on" } else { "off" });
                if l.castles.is_empty() {
                    r.add("placements with no castle", 1.0, format!("{tag}: no castle"));
                } else if tower_house(&l, &tag, r) {
                } else {
                    castle_checks(&world, t0, &l, &tag, false, r);
                    // Its keep and barracks as the castle's laid out by zones, and their halls.
                    let buckets = building_buckets(&l);
                    for b in l.buildings.iter().filter(|b| of_castle(b)) {
                        keep_rules(&world, t0, &l, ci, b, &format!("{tag} "), &buckets, r);
                    }
                    layout_halls(&world, t0, &l, ci, &tag, &of_castle, r);
                }
                both.push(l);
            }
            if both.len() == 2 {
                placed += 1;
                r.tally("placements", 1);
                let tag = format!("#{ci} p{}", patch["patch"]);
                let changed = same_buildings(&both[0], &both[1], &tag, "non-castle buildings changed or gone, walls off against on", r);
                let new = both[1].buildings.iter().filter(|y| y.ward != Ward::Castle && !both[0].buildings.iter().any(|x| x.id == y.id)).count();
                if new > 0 {
                    r.add("new non-castle ids with the walls off", new as f64, format!("{tag}: {new} new ids"));
                }
                if changed + new > 0 {
                    r.add("placements with any non-castle building changed", (changed + new) as f64, format!("{tag}: {changed} changed or gone, {new} new"));
                }
            }
        }
    }
    monastery_edits(base, t0, r);
    // R5 towns
    district_edits(base, t0, r);
}

/// `monastery_edits`' counts, in order.
pub(crate) const MONASTERY_EDIT_COUNTS: [&str; 6] = [
    "monastery edits",
    "coaching inn edits",
    "coaching inns no longer round a yard after an edit of their own patch or a far one",
    "buildings on another's ground (`Layout::grounds`), as generated",
    "buildings on another's ground after an edit",
    "buildings by a monastery with no front door after an edit",
];

/// Every monastery's town edited round it (`wards::change`: its patch's lots made small, its patch
/// rerolled, and a far edit: the walls taken down, else the patch farthest from it rerolled): no
/// building on another's ground (`Layout::grounds`, by more than a foot), and every building within
/// 150 ft of it with a front door.
pub(crate) fn monastery_edits(base: &World, t0: &T0, r: &mut Report) {
    let on_ground = |l: &Layout, tag: &str, label: &str, r: &mut Report| {
        for (id, g) in &l.grounds {
            let (small, gb) = (shrink(g, 1.0), bbox(g));
            for b in l.buildings.iter().filter(|b| b.id != *id) {
                let bb = bbox(&b.poly);
                if bb[0] > gb[2] || bb[2] < gb[0] || bb[1] > gb[3] || bb[3] < gb[1] || !poly_overlap(&shrink(&b.poly, 1.0), &small) {
                    continue;
                }
                r.add(label, 1.0, format!("{tag}: building {} on the ground of {id} at {}", b.id, at(centroid(&b.poly))));
            }
        }
    };
    let patch_at = |plan: &serde_json::Value, p: P, far: bool| -> Option<u64> {
        let at = |v: &serde_json::Value| [v["at"][0].as_f64().unwrap(), v["at"][1].as_f64().unwrap()];
        let by = plan["patches"].as_array()?.iter().filter(|q| q["in_town"] == true);
        let q = if far { by.max_by(|a, b| dist(at(a), p).total_cmp(&dist(at(b), p))) } else { by.min_by(|a, b| dist(at(a), p).total_cmp(&dist(at(b), p))) };
        q?["patch"].as_u64()
    };
    for li in 0..town::layout_count(t0) {
        let g = town::layout(base, t0, li);
        on_ground(&g, &format!("#{li}"), "buildings on another's ground (`Layout::grounds`), as generated", r);
        let mons: Vec<&town::Building> = g.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && (b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == "monastery") || town::palace::is_coaching(&g, b))).collect();
        if mons.is_empty() {
            continue;
        }
        let Ok(plan) = town::wards::plan_json(base, t0, li) else { continue };
        for m in mons {
            let c = centroid(&m.poly);
            let Some(own) = patch_at(&plan, c, false) else { continue };
            let mut reqs = vec![format!(r#"{{"patches":[{{"patch":{own},"lots":"small"}}]}}"#), format!(r#"{{"patches":[{{"patch":{own},"reroll":true}}]}}"#)];
            if !g.walls.is_empty() {
                reqs.push(r#"{"walls":false}"#.to_string());
            } else if let Some(far) = patch_at(&plan, c, true).filter(|&q| q != own) {
                reqs.push(format!(r#"{{"patches":[{{"patch":{far},"reroll":true}}]}}"#));
            }
            for req in reqs {
                let Ok((Some(edit), _)) = town::wards::change(base, t0, li, &serde_json::from_str(&req).unwrap()) else { continue };
                let mut file = base.file.clone();
                file.edits.towns.insert(town::wards::key(li), edit);
                let world = World::new(file).unwrap();
                let l = town::layout(&world, t0, li);
                let tag = format!("b:{li}:{} {req}", m.id);
                if town::palace::is_coaching(&g, m) {
                    r.tally("coaching inn edits", 1);
                    if !l.buildings.iter().any(|b| b.id == m.id && town::palace::is_coaching(&l, b) && interior::zone_plan_of(&world, t0, li, b.id as usize).and_then(|p| p.yard()).is_some()) {
                        let now = l.buildings.iter().find(|b| b.id == m.id);
                        let why = match now {
                            None => "gone".to_string(),
                            Some(b) if !town::palace::is_coaching(&l, b) => format!("not merged ({:?}, {} corners, as generated: {})", b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.poly.len(), b.poly == m.poly),
                            Some(b) => format!("no yard frame (as generated: {})", b.poly == m.poly),
                        };
                        r.add("coaching inns no longer round a yard after an edit of their own patch or a far one", 1.0, format!("{tag} at {}: {why}", at(c)));
                    }
                } else {
                    r.tally("monastery edits", 1);
                }
                on_ground(&l, &tag, "buildings on another's ground after an edit", r);
                for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && dist(centroid(&b.poly), c) < 150.0) {
                    let Some(it) = interior::generate(&world, t0, li, b.id as usize) else { continue };
                    if !it.levels.iter().any(|lv| lv.doors.iter().any(|d| d.kind == "front")) {
                        r.add("buildings by a monastery with no front door after an edit", 1.0, format!("{tag}: b:{li}:{} at {}", b.id, at(centroid(&b.poly))));
                    }
                }
            }
        }
    }
}

/// Non-castle buildings of `a` by id in `b`: the same outline, ward, function, kind, name and floors.
/// Returns how many changed or went.
pub(crate) fn same_buildings(a: &Layout, b: &Layout, tag: &str, label: &str, r: &mut Report) -> usize {
    let by_id: HashMap<u32, &town::Building> = b.buildings.iter().map(|y| (y.id, y)).collect();
    let mut n = 0;
    for x in a.buildings.iter().filter(|x| x.ward != Ward::Castle) {
        let what = match by_id.get(&x.id) {
            Some(y) if y.poly.len() == x.poly.len() && y.poly.iter().zip(&x.poly).all(|(p, q)| dist(*p, *q) < 1e-3) && y.ward == x.ward && y.func == x.func && y.residential == x.residential && y.name == x.name && y.floors == x.floors => continue,
            Some(y) => format!("{:?}/{:?} -> {:?}/{:?}, moved {:.1} ft", x.ward, x.func, y.ward, y.func, dist(centroid(&x.poly), centroid(&y.poly))),
            None => "gone".into(),
        };
        n += 1;
        r.add(label, 1.0, format!("{tag}: id {} {what} at {}", x.id, at(centroid(&x.poly))));
    }
    n
}

// ---- Temples, shrines and monasteries. ----
