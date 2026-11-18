use crate::prelude::*;

/// The `temples` rules' counts, in order.
pub(crate) const TEMPLE_COUNTS: [&str; 34] = [
    "temples left on their lots (too small for a hall with an apse)",
    "temples, shrines and monasteries laid out by zones: floors fallen back on",
    "big temples whose footprint is no cross with a round apse",
    "small temples whose footprint is no hall with a round apse",
    "temples missing a room of their programme (or its apse)",
    "temples' front doors not into the nave",
    "temples with no altar rail between the nave and the sanctuary or a side chapel",
    "temples' god's statues not on the axis at the far end",
    "temples with no altar before the god's statue",
    "temples whose candle stands by the altar are missing or not mirrored",
    "big temples whose arcades are missing or not mirrored",
    "big temples whose crossing lacks its four piers",
    "big temples with no carved screen across the nave from pier to gate before the crossing, or its gate shut or missing",
    "temples whose pews are missing, not mirrored (but by the stairs) or not facing the altar",
    "temples with no font",
    "side chapels with no side altar against the far wall",
    "vestries with nothing in them",
    "temples' pieces not in their god's look",
    "shrines not of one room",
    "shrines with no god's statue or altar",
    "monasteries not round a cloister (kept their lot)",
    "monasteries missing a room round the walk",
    "monasteries' rooms opening neither onto the walk nor onto a room that does",
    "monasteries whose chapter house is not drawn as an octagon",
    "monasteries whose church is not along a long side",
    "monasteries whose garth is not open to the sky on the map",
    "monasteries with objects scattered in the garth",
    "monasteries with no cloister well or walks in the garth",
    "monasteries whose refectory has no high table or long tables",
    "monasteries with another building on their footprint, or no arcade round the garth",
    "temple, shrine and monastery rooms the designer's Furnish lays out otherwise than generation",
    "temples, shrines and monasteries whose front door opens onto ground that reaches no street (in 50 ft)",
    "temple, shrine and monastery rooms with squares their furniture shuts off",
    "monasteries with a crypt beyond their church (and the foot of its stairs)",
];

/// `temples`: temples, shrines and monasteries as laid out (`interior::hub`'s `temple` and
/// `monastery`, `town::temple`): no floor fallen back on the shared layout; a big temple's
/// footprint a cross (its transept wider than its body both ways) ending in an apse beyond the body,
/// a small one's a hall ending in one; the rooms of each programme (a big one's nave, sanctuary in
/// its apse, two side chapels and a vestry; a small one's single hall in its apse), the front door
/// into the nave, an altar rail between the nave and the sanctuary and each side chapel; the god's
/// statue on the axis at the far end, the altar before it, candle stands mirrored by it; a big nave's
/// arcades mirrored, the crossing's four piers, the carved screen on the row before the crossing
/// mirrored with its gate on the axis clear; pews mirrored, facing the altar; the font; each side
/// chapel's altar against its far wall; the vestry furnished; every temple piece in the god's look
/// (`town::temple::god_look`); a shrine of one room with the god's statue and an altar; a monastery
/// round its cloister on its merged footprint (nothing else on it), its church along a long side, the
/// chapter house (drawn as an octagon), refectory, dorter, scriptorium and kitchen round the walk (each
/// opening onto it or onto a room that does), the garth open to the sky on the map with nothing
/// scattered in it but its well, its cloister well and walks inside, the arcade round it, the refectory
/// furnished as a whole (its high table and long tables); every front door onto ground that reaches a
/// street on the battlemap (within 50 ft), every square of every room no piece blocks reached from
/// its doors and stairs; a monastery's crypt under its church only.
pub(crate) fn temples(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&TEMPLE_COUNTS);
    let n = town::layout_count(&ctx.ex.t0);
    for li in 0..n {
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, li);
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
            if !matches!(key, Some("temple" | "shrine" | "monastery")) {
                continue;
            }
            temple_rules(ctx, &l, li, b, key.unwrap_or_default(), r);
        }
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn temple_rules(ctx: &mut Ctx, l: &Layout, li: usize, b: &town::Building, key: &str, r: &mut Report) {
    use interior::hall::Style;
    use interior::hub::Kind;
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let id = format!("b:{li}:{}", b.id);
    let kind = interior::hub_kind(t0, l, b);
    let tag = format!("{id} ({})", kind.map_or(key, |k| k.name()));
    let at_b = at(centroid(&b.poly));
    r.tally(&format!("  {}s", if key == "shrine" && kind.is_some() { "shrine hall" } else { kind.map_or(key, |k| k.name()) }), 1);
    if key == "temple" && kind.is_none() {
        r.add(TEMPLE_COUNTS[0], area(&b.poly).abs(), format!("{tag} at {at_b}: {:.0} sq ft", area(&b.poly).abs()));
    }
    if key == "monastery" && kind != Some(Kind::Monastery) {
        r.add(TEMPLE_COUNTS[20], area(&b.poly).abs(), format!("{tag} at {at_b}: {:.0} sq ft", area(&b.poly).abs()));
    }
    let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
    if !fell.is_empty() {
        r.add(TEMPLE_COUNTS[1], fell.len() as f64, format!("{tag}: z {fell:?}"));
    }
    let Some(it) = interior::generate_id(world, t0, &id) else { return };
    let (nx, ny) = (it.nx, it.ny);
    let Some(gi) = it.levels.iter().position(|lv| lv.z == 0) else { return };
    let g = &it.levels[gi];
    let room_at = |lv: &interior::Level, (i, j): (isize, isize)| if i < 0 || j < 0 || i as usize >= nx || j as usize >= ny { -1 } else { lv.cells[j as usize * nx + i as usize] };
    // The front door's outside on the battlemap: open ground reached from a street (or 50 ft off).
    if let Some(d) = g.doors.iter().find(|d| d.kind == "front")
        && let Some(o) = door_squares(d).into_iter().find(|&p| room_at(g, p) < 0)
    {
        let p = interior::to_world(&it, o.0 as f64 + 0.5, o.1 as f64 + 0.5);
        let bb = bbox(&b.poly);
        let w = ctx.window([bb[0] - 50.0, bb[1] - 50.0, bb[2] + 50.0, bb[3] + 50.0]);
        let seen = w.flood(|k| w.street[k] || w.edge(k));
        let ok = w.index(p).is_some_and(|k| (!w.block[k] && seen[k]) || (w.block[k] && w.around(k).any(|m| seen[m] && !contains(&b.poly, w.centre(m)))));
        if !ok {
            r.add(TEMPLE_COUNTS[31], 1.0, format!("{tag}: its door's outside at {}", at(p)));
        }
    }
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    // Every square of every room no piece blocks reached from the stairs and the front door, through
    // doors.
    for (li, lv) in it.levels.iter().enumerate() {
        let mut blocked = vec![false; nx * ny];
        for f in lv.furniture.iter().filter(|f| f.blocks_move) {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    blocked[j * nx + i] = true;
                }
            }
        }
        let st = it.stairs;
        let mut starts: Vec<usize> = Vec::new();
        if lv.has_stairs {
            starts.extend((st[1]..st[1] + st[3]).flat_map(|j| (st[0]..st[0] + st[2]).map(move |i| j * nx + i)).filter(|&k| lv.cells[k] >= 0));
        }
        if li == it.entry_level {
            for d in lv.doors.iter().filter(|d| d.kind == "front") {
                starts.extend(door_squares(d).into_iter().filter(|&p| room_at(lv, p) >= 0).map(|p| p.1 as usize * nx + p.0 as usize));
            }
        }
        starts.extend(lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize));
        let door_between = |a: usize, c: usize| lv.doors.iter().any(|d| {
            let s = door_squares(d);
            let sq = |k: usize| ((k % nx) as isize, (k / nx) as isize);
            s.contains(&sq(a)) && s.contains(&sq(c))
        });
        let mut seen = vec![false; nx * ny];
        let mut stack = starts.clone();
        starts.iter().for_each(|&k| seen[k] = true);
        while let Some(k) = stack.pop() {
            let (i, j) = ((k % nx) as isize, (k / nx) as isize);
            for (a, c) in [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)] {
                if a < 0 || c < 0 || a as usize >= nx || c as usize >= ny {
                    continue;
                }
                let q = c as usize * nx + a as usize;
                if seen[q] || lv.cells[q] < 0 || blocked[q] || (lv.cells[q] != lv.cells[k] && !door_between(k, q)) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        for (ri, rm) in lv.rooms.iter().enumerate().filter(|(_, rm)| interior::walkable(rm.kind) && rm.squares > 0) {
            let shut = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16 && !blocked[k] && !seen[k]).count();
            if shut > 0 {
                r.add(TEMPLE_COUNTS[32], shut as f64, format!("{tag} {}: the {} has {shut} squares shut off", lv.name, rm.kind));
            }
        }
    }
    let kinds: Vec<&str> = g.rooms.iter().filter(|rm| rm.squares > 0).map(|rm| rm.kind).collect();
    let count = |k: &str| kinds.iter().filter(|&&x| x == k).count();
    let plans = interior::hall_plans(world, t0, li, b.id as usize);
    let god = town::temple::god_look(b);
    // The designer's Furnish on each room furnished as a whole, cleared: what generation gave it (the
    // pieces, their places, backs and the god's looks; the looks rolled aside).
    if let Ok((d, _)) = interior::design::design_of(world, t0, &id, false) {
        let faith = |s: Style| matches!(s, Style::Nave { .. } | Style::Sanctuary { .. } | Style::SideChapel { .. } | Style::Shrine { .. } | Style::Refectory);
        for p in plans.iter().filter(|p| faith(p.style)) {
            let Some(cells) = worldgen::under::design::decode(&d.levels[p.level].cells, nx * ny) else { continue };
            let mine = |x: u16, y: u16| cells[y as usize * nx + x as usize] == p.room;
            let mut dd = d.clone();
            dd.levels[p.level].items.retain(|f| !mine(f.x, f.y));
            dd.furnish(world, t0, &id, p.level, p.room as usize, 7);
            let look = |k: &str, v: Option<u8>| if matches!(k, "god_statue" | "candle_stand" | "carved_screen" | "screen_gate" | "side_altar" | "low_dais") { v } else { None };
            let mut got: Vec<(String, u16, u16, u16, u16, Option<u8>, Option<u8>)> = dd.levels[p.level].items.iter().filter(|f| mine(f.x, f.y)).map(|f| (f.kind.clone(), f.x, f.y, f.w, f.h, f.back, look(&f.kind, f.variant))).collect();
            let mut want: Vec<(String, u16, u16, u16, u16, Option<u8>, Option<u8>)> = it.levels[p.level].furniture.iter().filter(|f| mine(f.x, f.y)).map(|f| (f.kind.to_string(), f.x, f.y, f.w, f.h, f.back, look(f.kind, f.variant))).collect();
            got.sort();
            want.sort();
            if got != want {
                let kind = it.levels[p.level].rooms[p.room as usize].kind;
                r.add(TEMPLE_COUNTS[30], 1.0, format!("{tag} {}: the {kind}: {} pieces, generated {}", it.levels[p.level].name, got.len(), want.len()));
            }
        }
    }
    // (Every temple piece in the god's look.)
    let wrong: Vec<String> = it.levels.iter().flat_map(|lv| lv.furniture.iter()).filter(|f| matches!(f.kind, "god_statue" | "candle_stand" | "carved_screen" | "screen_gate" | "side_altar") && f.variant != Some(god)).map(|f| format!("{} v{:?}", f.kind, f.variant)).collect();
    if !wrong.is_empty() {
        r.add(TEMPLE_COUNTS[17], wrong.len() as f64, format!("{tag}: god {god}: {}", wrong.join(", ")));
    }
    // (A piece's box in a frame: s0..s1, t0..t1.)
    let local = |f: &interior::hall::Frame, it: &interior::Item| {
        let (a, c) = (f.local(it.x as isize, it.y as isize), f.local((it.x + it.w) as isize - 1, (it.y + it.h) as isize - 1));
        (a.0.min(c.0), a.0.max(c.0), a.1.min(c.1), a.1.max(c.1))
    };
    let in_room = |lv: &interior::Level, it: &interior::Item, ri: i16| lv.cells[it.y as usize * nx + it.x as usize] == ri;
    // (Pieces of `kind` in room `ri` mirrored across frame `f`'s axis, every one: their boxes.)
    let mirrored = |lv: &interior::Level, f: &interior::hall::Frame, ri: i16, kind: &str| -> (usize, bool) {
        let boxes: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == kind && in_room(lv, it, ri)).map(|it| local(f, it)).collect();
        let w = f.wid as isize;
        let ok = boxes.iter().all(|&(s0, s1, a, c)| boxes.contains(&(s0, s1, w - 1 - c, w - 1 - a)));
        (boxes.len(), ok)
    };
    if key == "shrine" {
        if kinds.len() != 1 {
            r.add(TEMPLE_COUNTS[18], kinds.len() as f64, format!("{tag} at {at_b}: {}", kinds.join(", ")));
        }
        let has = |k: &str| g.furniture.iter().any(|f| f.kind == k);
        if !has("god_statue") || !has("altar") {
            r.add(TEMPLE_COUNTS[19], 1.0, format!("{tag} at {at_b}: {} squares", g.rooms.iter().map(|rm| rm.squares).sum::<usize>()));
        }
        return;
    }
    if key == "temple" {
        let Some(kind) = kind else { return };
        let big = kind == Kind::Church;
        // The footprint: a cross ending in an apse (a hall ending in one).
        let body = b.parts.iter().find(|p| p.kind == town::PartKind::Body);
        let apse = b.parts.iter().find(|p| p.kind == town::PartKind::Apse);
        let wing = b.parts.iter().find(|p| p.kind == town::PartKind::Wing);
        let shape_ok = match (body, apse) {
            (Some(bd), Some(ap)) if bd.poly.len() == 4 => {
                let (es, et) = (sub(bd.poly[1], bd.poly[0]), sub(bd.poly[3], bd.poly[0]));
                let (le, lt) = (len(es), len(et));
                let proj = |p: P| (dot(sub(p, bd.poly[0]), es) / le, dot(sub(p, bd.poly[0]), et) / lt);
                // (The apse beyond the body's end; the transept past the body both ways.)
                let tip = ap.poly.iter().map(|q| proj(*q).0).fold(f64::MIN, f64::max);
                let round = tip >= le + 0.4 * lt - 1.0 && ap.poly.len() >= 10;
                let cross = wing.is_some_and(|w| {
                    let ts: Vec<f64> = w.poly.iter().map(|q| proj(*q).1).collect();
                    ts.iter().copied().fold(f64::MAX, f64::min) < -4.0 && ts.iter().copied().fold(f64::MIN, f64::max) > lt + 4.0
                });
                round && (cross == big) && b.poly.len() >= if big { 20 } else { 12 }
            }
            _ => false,
        };
        if !shape_ok {
            r.add(TEMPLE_COUNTS[if big { 2 } else { 3 }], 1.0, format!("{tag} at {at_b}: {} pieces, outline of {}", b.parts.len(), b.poly.len()));
        }
        // The programme.
        let apse_kept = |k: &str| g.rooms.iter().any(|rm| rm.kind == k && (3..=6).contains(&rm.shape));
        let missing: Vec<&str> = if big {
            [("nave", 1), ("sanctuary", 1), ("side chapel", 2), ("vestry", 1)].iter().filter(|(k, n)| count(k) != *n).map(|(k, _)| *k).chain((!apse_kept("sanctuary")).then_some("the sanctuary's apse")).collect()
        } else {
            [("nave", 1)].iter().filter(|(k, n)| count(k) != *n || kinds.len() != 1).map(|(k, _)| *k).chain((!apse_kept("nave")).then_some("the hall's apse")).collect()
        };
        if !missing.is_empty() {
            r.add(TEMPLE_COUNTS[4], missing.len() as f64, format!("{tag} at {at_b}: {} (has {})", missing.join(", "), kinds.join(", ")));
        }
        if let Some(fr) = g.doors.iter().find(|d| d.kind == "front")
            && g.rooms[fr.rooms[0] as usize].kind != "nave"
        {
            r.add(TEMPLE_COUNTS[5], 1.0, format!("{tag}: into the {}", g.rooms[fr.rooms[0] as usize].kind));
        }
        if big {
            // (A rail on the edge between the nave and each of them.)
            let railed = |k: &str| {
                g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == k && rm.squares > 0).all(|(ri, _)| {
                    g.walls.iter().any(|w| {
                        let d = interior::Door { a: w.a, b: [if w.a[0] == w.b[0] { w.a[0] } else { w.a[0] + 1.0 }, if w.a[1] == w.b[1] { w.a[1] } else { w.a[1] + 1.0 }], kind: "rail", rooms: [-1, -1], lock: 0 };
                        let [p, q] = door_squares(&d);
                        let (rp, rq) = (room_at(g, p), room_at(g, q));
                        w.rail && ((rp == ri as i16 && rq >= 0 && g.rooms[rq as usize].kind == "nave") || (rq == ri as i16 && rp >= 0 && g.rooms[rp as usize].kind == "nave"))
                    })
                })
            };
            if !railed("sanctuary") || !railed("side chapel") {
                r.add(TEMPLE_COUNTS[6], 1.0, format!("{tag} at {at_b}"));
            }
        }
        // The god's end: in the sanctuary's frame (a hall's in its nave's).
        let god_plan = plans.iter().find(|p| matches!(p.style, Style::Sanctuary { .. }) || matches!(p.style, Style::Nave { cross: None, .. }));
        match god_plan {
            Some(p) => {
                let lv = &it.levels[p.level];
                let f = &p.frame;
                let w = f.wid as isize;
                let statue = lv.furniture.iter().find(|it| it.kind == "god_statue" && in_room(lv, it, p.room)).map(|it| local(f, it));
                match statue {
                    Some((s0, _, a, c)) if a + c == w - 1 && s0 >= f.len as isize - 3 => {
                        let altar = lv.furniture.iter().filter(|it| it.kind == "altar" && in_room(lv, it, p.room)).map(|it| local(f, it)).any(|(as0, _, aa, ac)| aa + ac == w - 1 && as0 < s0 && as0 >= s0 - 2);
                        if !altar {
                            r.add(TEMPLE_COUNTS[8], 1.0, format!("{tag} at {at_b}"));
                        }
                    }
                    other => r.add(TEMPLE_COUNTS[7], 1.0, format!("{tag} at {at_b}: {other:?} in a {} x {} frame", f.len, f.wid)),
                }
                let (n, ok) = mirrored(lv, f, p.room, "candle_stand");
                if n < 2 || !ok {
                    r.add(TEMPLE_COUNTS[9], 1.0, format!("{tag} at {at_b}: {n} candle stands"));
                }
            }
            None => r.add(TEMPLE_COUNTS[7], 1.0, format!("{tag} at {at_b}: no frame furnished")),
        }
        // The nave.
        if let Some(p) = plans.iter().find(|p| matches!(p.style, Style::Nave { .. })) {
            let lv = &it.levels[p.level];
            let f = &p.frame;
            let w = f.wid as isize;
            if let Style::Nave { cross: Some((c0, c1)), .. } = p.style {
                let (c0, c1) = (c0 as isize, c1 as isize);
                let pillars: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == "pillar" && in_room(lv, it, p.room)).map(|it| local(f, it)).collect();
                let arcade = pillars.iter().filter(|q| q.0 < c0).count();
                let (_, mirror) = mirrored(lv, f, p.room, "pillar");
                if arcade < 4 || !mirror {
                    r.add(TEMPLE_COUNTS[10], 1.0, format!("{tag} at {at_b}: {arcade} pillars before the crossing"));
                }
                if pillars.iter().filter(|q| q.0 == c0 || q.0 == c1).count() < 4 {
                    r.add(TEMPLE_COUNTS[11], 1.0, format!("{tag} at {at_b}"));
                }
                // (The screen on the row before the crossing, from the piers' line to the gate on the
                // axis, mirrored; its gate clear.)
                let (n, ok) = mirrored(lv, f, p.room, "carved_screen");
                let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
                let pier = if w >= 11 { 2 } else { 1 };
                let boxes: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == "carved_screen" && in_room(lv, it, p.room)).map(|it| local(f, it)).collect();
                let on_row = boxes.iter().all(|q| q.0 == c0 - 1) && boxes.iter().any(|q| q.2 == pier && q.3 == cl - 1);
                let gate = (cl..=cr).all(|t| {
                    let (i, j) = f.grid(c0 - 1, t);
                    let on = |it: &&interior::Item| (i as u16) >= it.x && (i as u16) < it.x + it.w && (j as u16) >= it.y && (j as u16) < it.y + it.h;
                    !lv.furniture.iter().filter(on).any(|it| it.blocks_move) && lv.furniture.iter().filter(on).any(|it| it.kind == "screen_gate")
                });
                if n < 2 || !ok || !on_row || !gate {
                    r.add(TEMPLE_COUNTS[12], 1.0, format!("{tag} at {at_b}: {n} pieces"));
                }
            }
            // Pews, mirrored, their backs to the door's end.
            let back = {
                let d = (-f.ds.0, -f.ds.1);
                match d {
                    (0, -1) => 0,
                    (1, 0) => 1,
                    (0, 1) => 2,
                    _ => 3,
                }
            };
            // (Mirrored but beside the stairs and their foot, which may take one side.)
            let st = it.stairs;
            let by_stairs: Vec<isize> = (st[1] as isize - 1..=(st[1] + st[3]) as isize).flat_map(|j| (st[0] as isize - 1..=(st[0] + st[2]) as isize).map(move |i| f.local(i, j).0)).collect();
            let pews: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == "pew" && in_room(lv, it, p.room)).map(|it| local(f, it)).collect();
            let n = pews.len();
            let ok = pews.iter().all(|&(s0, s1, a, c)| by_stairs.contains(&s0) || pews.contains(&(s0, s1, w - 1 - c, w - 1 - a)));
            let facing = lv.furniture.iter().filter(|it| it.kind == "pew" && in_room(lv, it, p.room)).all(|it| it.back == Some(back));
            if n < 2 || !ok || !facing {
                r.add(TEMPLE_COUNTS[13], 1.0, format!("{tag} at {at_b}: {n} pews"));
            }
            if !lv.furniture.iter().any(|it| it.kind == "font" && in_room(lv, it, p.room)) {
                r.add(TEMPLE_COUNTS[14], 1.0, format!("{tag} at {at_b}"));
            }
        }
        if big {
            for p in plans.iter().filter(|p| matches!(p.style, Style::SideChapel { .. })) {
                let lv = &it.levels[p.level];
                let ok = lv.furniture.iter().filter(|it| it.kind == "side_altar" && in_room(lv, it, p.room)).any(|it| local(&p.frame, it).1 == p.frame.len as isize - 1);
                if !ok {
                    r.add(TEMPLE_COUNTS[15], 1.0, format!("{tag} at {at_b}"));
                }
            }
            if count("side chapel") != plans.iter().filter(|p| matches!(p.style, Style::SideChapel { .. })).count() {
                r.add(TEMPLE_COUNTS[15], 1.0, format!("{tag} at {at_b}: a chapel not furnished as a whole"));
            }
            for (ri, _) in g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == "vestry" && rm.squares > 0) {
                if !g.furniture.iter().any(|it| in_room(g, it, ri as i16)) {
                    r.add(TEMPLE_COUNTS[16], 1.0, format!("{tag} at {at_b}"));
                }
            }
        }
        return;
    }
    // A monastery round its cloister.
    if kind != Some(Kind::Monastery) {
        return;
    }
    // (The crypt: under the church, the stairs' foot and the way beside them; no more squares than
    // those and four.)
    if let Some(ri) = g.rooms.iter().position(|rm| rm.kind == "nave") {
        let church = (0..nx * ny).filter(|&k| g.cells[k] == ri as i16).count();
        for lv in it.levels.iter().filter(|lv| lv.z < 0) {
            let under = (0..nx * ny).filter(|&k| lv.cells[k] >= 0).count();
            let off = (0..nx * ny).filter(|&k| lv.cells[k] >= 0 && g.cells[k] != ri as i16).count();
            if off > 4 {
                r.add(TEMPLE_COUNTS[33], 1.0, format!("{tag} at {at_b}: {under} squares under it, {off} not under the church ({church})"));
            }
        }
    }
    let need = ["nave", "entrance hall", "cloister walk", "garth", "chapter house", "refectory", "dorter", "scriptorium", "kitchen"];
    let missing: Vec<&str> = need.iter().copied().filter(|k| count(k) == 0).collect();
    if !missing.is_empty() {
        r.add(TEMPLE_COUNTS[21], missing.len() as f64, format!("{tag} at {at_b}: {}", missing.join(", ")));
    }
    // (The rooms each one has a door to.)
    let doors_of = |ri: i16| -> Vec<i16> { g.doors.iter().filter(|d| d.rooms.contains(&ri)).map(|d| if d.rooms[0] == ri { d.rooms[1] } else { d.rooms[0] }).filter(|&o| o >= 0).collect() };
    let is = |ri: i16, k: &str| ri >= 0 && g.rooms[ri as usize].kind == k;
    for k in ["chapter house", "refectory", "dorter", "scriptorium", "kitchen"] {
        for (ri, _) in g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == k && rm.squares > 0) {
            let ok = doors_of(ri as i16).iter().any(|&o| is(o, "cloister walk") || doors_of(o).iter().any(|&q| is(q, "cloister walk")));
            if !ok {
                r.add(TEMPLE_COUNTS[22], 1.0, format!("{tag} at {at_b}: the {k}"));
            }
        }
    }
    if !g.rooms.iter().any(|rm| rm.kind == "chapter house" && matches!(rm.shape, interior::shape::OCTAGON | interior::shape::CHAMFERED)) {
        r.add(TEMPLE_COUNTS[23], 1.0, format!("{tag} at {at_b}"));
    }
    // (The church: the nave's squares a band the footprint's whole length along one side.)
    if let Some(ri) = g.rooms.iter().position(|rm| rm.kind == "nave") {
        let sq: Vec<usize> = (0..nx * ny).filter(|&k| g.cells[k] == ri as i16).collect();
        let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
        let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
        let long = nx.max(ny);
        let along = (x1 - x0 + 1 == long && (y0 == 0 || y1 == ny - 1) && nx >= ny) || (y1 - y0 + 1 == long && (x0 == 0 || x1 == nx - 1) && ny >= nx);
        if !along || sq.len() != (x1 - x0 + 1) * (y1 - y0 + 1) {
            r.add(TEMPLE_COUNTS[24], 1.0, format!("{tag} at {at_b}"));
        }
    }
    match interior::court_plan(world, t0, l, b) {
        None => r.add(TEMPLE_COUNTS[25], 1.0, format!("{tag} at {at_b}")),
        Some(c) => {
            let bb = bbox(&c.court);
            let w = ctx.window([bb[0], bb[1], bb[2], bb[3]]);
            let inside: Vec<String> = w.objects.iter().filter(|(p, name)| contains(&c.court, *p) && name.as_str() != "well").map(|(p, name)| format!("{name} at {}", at(*p))).collect();
            if !inside.is_empty() {
                r.add(TEMPLE_COUNTS[26], inside.len() as f64, format!("{tag}: {}", inside.join(", ")));
            }
        }
    }
    let garth = g.rooms.iter().position(|rm| rm.kind == "garth").map(|ri| ri as i16);
    let pieces = |k: &str| garth.is_some_and(|ri| g.furniture.iter().any(|it| it.kind == k && in_room(g, it, ri)));
    if !pieces("cloister_well") || !pieces("garden_path") {
        r.add(TEMPLE_COUNTS[27], 1.0, format!("{tag} at {at_b}"));
    }
    let refectory = plans.iter().find(|p| p.style == Style::Refectory);
    let fed = refectory.is_some_and(|p| {
        let lv = &it.levels[p.level];
        lv.furniture.iter().any(|it| it.kind == "high_table" && in_room(lv, it, p.room)) && lv.furniture.iter().any(|it| it.kind == "long_table" && in_room(lv, it, p.room))
    });
    if !fed {
        r.add(TEMPLE_COUNTS[28], 1.0, format!("{tag} at {at_b}"));
    }
    let crowded = l.buildings.iter().filter(|o| o.id != b.id && dist(centroid(&o.poly), centroid(&b.poly)) < 200.0 && poly_overlap(&o.poly, &shrink(&b.poly, 1.0))).count();
    let arcade = garth.is_some_and(|gr| g.walls.iter().any(|w| w.rail && {
        let d = interior::Door { a: w.a, b: [if w.a[0] == w.b[0] { w.a[0] } else { w.a[0] + 1.0 }, if w.a[1] == w.b[1] { w.a[1] } else { w.a[1] + 1.0 }], kind: "rail", rooms: [-1, -1], lock: 0 };
        let [p, q] = door_squares(&d);
        room_at(g, p) == gr || room_at(g, q) == gr
    }));
    if crowded > 0 || !arcade {
        r.add(TEMPLE_COUNTS[29], crowded as f64, format!("{tag} at {at_b}: {crowded} buildings on it{}", if arcade { "" } else { ", no arcade" }));
    }
}

// ---- Hashes. ----

/// The `civic` rules' counts, listed even when none.
pub(crate) const CIVIC_COUNTS: [&str; 11] = [
    "civic buildings laid out by zones: floors fallen back on",
    "civic buildings breaking an interior rule (as in interiors)",
    "civic buildings missing a room of their programme",
    "civic halls with no low dais and chair of office at the far end",
    "civic rooms over 2.5:1 (ways about the floor aside)",
    "arcades not on the street front (no open side, or the front door not into it)",
    "civic buildings missing a room of their programme, one storey (reasoned absent)",
    "civic buildings below the threshold (as other halls)",
    "city town halls with no belfry (why)",
    "belfries breaking a rule (zones, a room and its spiral stair on every floor to the top, the bell)",
    "civic rooms without their piece (council table, strongbox, counting desk, job board, stalls)",
];

/// The pieces a room kind must hold (A2): in a council chamber its council table (else the long
/// table stands in), in a strongroom and a warded vault a strongbox, in a counting house the counting
/// desk, in a job-board room the job board, in stables the stalls.
pub(crate) const ROOM_PIECES: [(&str, &str); 6] = [
    ("council chamber", "council_table"),
    ("strongroom", "strongbox"),
    ("warded vault", "strongbox"),
    ("counting house", "counting_desk"),
    ("job-board room", "job_board"),
    ("stables", "stable_stall"),
];

/// Interior `it`'s pieces by the rooms that must hold them (`ROOM_PIECES`): each room of those kinds
/// tallied, a room without its piece listed; every guild's arms tallied (on the hall's dais apart).
pub(crate) fn room_pieces(it: &worldgen::interior::Interior, tag: &str, r: &mut Report) {
    for lv in &it.levels {
        for (ri, room) in lv.rooms.iter().enumerate() {
            if room.squares == 0 {
                continue;
            }
            let held = |kind: &str| lv.furniture.iter().filter(|f| f.kind == kind && lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16).count();
            for (rk, piece) in ROOM_PIECES {
                if room.kind != rk {
                    continue;
                }
                let n = held(piece);
                r.tally(&format!("  rooms: {rk}"), 1);
                r.tally(&format!("    {rk}: {piece} pieces"), n);
                if n == 0 {
                    r.tally(&format!("    {rk}: without one"), 1);
                    let stand = if rk == "council chamber" && held("long_table") > 0 { ", a long table stands in" } else { "" };
                    r.add(CIVIC_COUNTS[10], room.squares as f64, format!("{tag} {}: {rk} of {} squares{stand}", lv.name, room.squares));
                }
            }
            let arms = held("guild_arms");
            if arms > 0 {
                r.tally(if room.kind == "great hall" { "  guild's arms in halls" } else { "  guild's arms in other rooms" }, arms);
            }
            if room.kind == "great hall" && held("job_board") > 0 {
                r.tally("  job boards in halls", 1);
            }
        }
    }
}

/// A belfried town hall's interior `it` (its body's top floor `floors - 1`): the spiral stair on the
/// top floor's landing, then the tower's room on each floor above to `floors - 1 + STOREYS` (a
/// belfry stair, at the top the belfry), the spiral stair in it, the bell at the top; the problems.
pub(crate) fn belfry_rules(it: &worldgen::interior::Interior, floors: i8) -> Vec<String> {
    let top = floors - 1 + town::belfry::STOREYS;
    let mut bad = Vec::new();
    if !it.levels.iter().find(|lv| lv.z == floors - 1).is_some_and(|lv| lv.furniture.iter().any(|f| f.kind == "spiral_stair")) {
        bad.push(format!("z {}: no spiral stair up", floors - 1));
    }
    for z in floors..=top {
        let Some(lv) = it.levels.iter().find(|lv| lv.z == z) else {
            bad.push(format!("no level {z}"));
            continue;
        };
        let want = if z == top { "belfry" } else { "belfry stair" };
        let Some(ri) = lv.rooms.iter().position(|r| r.kind == want && r.squares > 0) else {
            bad.push(format!("z {z}: no {want}"));
            continue;
        };
        let inr = |f: &worldgen::interior::Item| lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16;
        if !lv.furniture.iter().any(|f| f.kind == "spiral_stair" && inr(f)) {
            bad.push(format!("z {z}: no spiral stair"));
        }
        if z == top && !lv.furniture.iter().any(|f| f.kind == "bell" && inr(f)) {
            bad.push(format!("z {z}: no bell"));
        }
        if lv.has_stairs || lv.rooms.iter().filter(|r| r.squares > 0).count() != 1 {
            bad.push(format!("z {z}: not the tower's alone"));
        }

    }
    if it.levels.iter().any(|lv| lv.z > top) {
        bad.push("levels above the bell loft".to_string());
    }
    bad
}

/// R5 towns (103): where a village's lanes (`Layout::streets`) end and cross, both ends of each,
/// anywhere. An end is joined on a way (within 1 ft of another lane's or a road's centre line: the
/// network roads as `villages` samples them, and the layout's; a ring closed on itself); else at a
/// building (15 ft), a yard or croft (8 ft) or the water, or at nothing; one by the green (within 35 ft
/// of its rim) is also counted as not on its ring. Ends not joined 1-40 ft from a way's centre line are
/// near one; those whose last 40 ft cross a way's centre line and stop
/// past its far edge overshoot it. Crossings: a lane's piece across another lane's or a road's, but
/// within 1 ft of the lane's ends or of a node both share. Farmsteads: farmyards by count.
/// `LANE_DUMP=L<n>` prints that village's lanes, roads and what stands by each lane's ends (stderr);
/// `NEAR_LIST=1` every end near a way but not joined (on the ring or not, ft past the green's rim,
/// by water, the village's centre).
pub(crate) fn lane_ends(world: &World, t0: &T0, l: &Layout, tag: &str, r: &mut Report) {
    let off = |p: P| format!("({:.0}, {:.0})", p[0] - l.center[0], p[1] - l.center[1]);
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    let reach = 1.3 * l.radius;
    let c = l.center;
    let mut roads: Vec<Vec<(P, f64)>> = Vec::new();
    for (ri, k) in t0.roads.segments_near([c[0] - reach, c[1] - reach, c[0] + reach, c[1] + reach], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 40.0).ceil().clamp(1.0, 400.0) as usize;
        roads.push((0..=n).map(|j| (rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p, 0.5 * rc.class.width_ft())).collect());
    }
    let mut river: Vec<(P, P, f64)> = Vec::new();
    let rr = reach + 200.0;
    for (ri, k) in t0.rivers.segments_near(c[0] - rr, c[1] - rr, c[0] + rr, c[1] + rr, 0.0) {
        let rc = &t0.rivers.rivers[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(2.0, 1200.0) as usize;
        let pts: Vec<(P, f64)> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft)).map(|c| (c.p, c.w)).collect();
        river.extend(pts.windows(2).map(|s| (s[0].0, s[1].0, 0.5 * s[0].1.max(s[1].1))));
    }
    let by_water = |p: P| river.iter().any(|&(a, b, hw)| seg_dist(p, a, b) <= hw + 30.0) || wet(p) || [12.0, 25.0, 30.0].iter().any(|&d| (0..8).any(|k| wet(add(p, [d * libm::cos(k as f64 * 0.785), d * libm::sin(k as f64 * 0.785)]))));
    // (Every way as segments with half widths, and whose: the lane's index, or none for a road.)
    let mut segs: Vec<(P, P, f64, Option<usize>)> = Vec::new();
    for (si, s) in l.streets.iter().enumerate() {
        segs.extend(s.windows(2).map(|w| (w[0], w[1], 0.5 * town::village::LANE_W, Some(si))));
    }
    for q in &roads {
        segs.extend(q.windows(2).map(|w| (w[0].0, w[1].0, w[0].1, None)));
    }
    let n_cross = segs.len();
    segs.extend(l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w, None))));
    let green = l.plazas.iter().find(|g| g.kind == PlazaKind::Green).map(|g| (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0])));
    if std::env::var("LANE_DUMP").is_ok_and(|v| v == tag) {
        for (si, st) in l.streets.iter().enumerate() {
            eprintln!("lane {si}: {}", st.iter().map(|p| off(*p)).collect::<Vec<_>>().join(" "));
        }
        for q in &roads {
            eprintln!("road: {}", q.iter().map(|p| off(p.0)).collect::<Vec<_>>().join(" "));
        }
        eprintln!("green {:?} centre ({:.0}, {:.0})", green.map(|g| (off(g.0), g.1)), l.center[0], l.center[1]);
        for st in &l.streets {
            for e in [st[0], st[st.len() - 1]] {
                for b in l.buildings.iter().filter(|b| edge_dist(&b.poly, e) < 40.0) {
                    eprintln!("  end {} building {:?} {:?} {:.1} ft, {} pts", off(e), b.ward, b.func, edge_dist(&b.poly, e), b.poly.len());
                }
                for p in l.crofts.iter().filter(|p| edge_dist(&p.pts, e) < 40.0) {
                    eprintln!("  end {} plot {:?} {:.1} ft", off(e), p.kind, edge_dist(&p.pts, e));
                }
            }
        }
    }
    let hit = |a: P, b: P, p: P, q: P| -> Option<P> {
        let (e, f) = (sub(b, a), sub(q, p));
        let den = cross(e, f);
        if den.abs() < 1e-9 {
            return None;
        }
        let (t, u) = (cross(sub(p, a), f) / den, cross(sub(p, a), e) / den);
        (t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0).then(|| add(a, mul(e, t)))
    };
    for (si, s) in l.streets.iter().enumerate() {
        if s.len() < 2 {
            continue;
        }
        for end in [0, 1] {
            r.tally("lane ends", 1);
            let (e, way): (P, Vec<P>) = if end == 0 { (s[0], s.to_vec()) } else { (s[s.len() - 1], s.iter().rev().copied().collect()) };
            let others = || segs.iter().filter(|w| w.3 != Some(si));
            let d = others().map(|w| seg_dist(e, w.0, w.1)).fold(f64::MAX, f64::min);
            // (A ring round the green closed on itself: its ends one node.)
            if d <= 1.0 || dist(s[0], s[s.len() - 1]) < 0.01 {
                r.tally("lane ends joined", 1);
                continue;
            }
            // (Its last 40 ft across a way and past its far edge.)
            let mut back = vec![way[0]];
            let mut acc = 0.0;
            for w in way.windows(2) {
                let k = dist(w[0], w[1]);
                if acc + k >= 40.0 {
                    back.push(lerp(w[0], w[1], (40.0 - acc) / k.max(1e-9)));
                    break;
                }
                back.push(w[1]);
                acc += k;
            }
            let start = way[way.len() - 1];
            let over = others().find(|w| back.windows(2).any(|b| hit(b[0], b[1], w.0, w.1).is_some_and(|x| dist(x, e) > 1.0 && dist(x, start) > 1.0)) && seg_dist(e, w.0, w.1) > w.2);
            if let Some(w) = over {
                r.add("lane ends overshooting a way", 1.0, format!("{tag} {} by {:.0} ft (lane {si}, {} pts, {:.0} ft)", off(e), seg_dist(e, w.0, w.1) - w.2, s.len(), s.windows(2).map(|q| dist(q[0], q[1])).sum::<f64>()));
            }
            // (Within the ring lane's reach of the green: R5 towns 103, lanes there end on the ring.)
            let ring = green.is_some_and(|(gc, gr)| s.len() >= 4 && s[1..s.len() - 1].iter().all(|p| (dist(*p, gc) - gr - 0.5 * town::village::LANE_W).abs() < 2.0));
            if ring {
                r.add("ring lane ends not on a way", 1.0, format!("{tag} {} (lane {si}, way {d:.0} ft{})", off(e), if by_water(e) { ", water" } else { "" }));
            } else if green.is_some_and(|(gc, gr)| dist(e, gc) <= gr + town::village::LANE_W + 25.0) {
                r.add("lane ends at the green's rim but not on the ring", 1.0, format!("{tag} {} (lane {si}, way {d:.0} ft{})", off(e), if by_water(e) { ", water" } else { "" }));
            }
            let at_building = l.buildings.iter().any(|b| contains(&b.poly, e) || edge_dist(&b.poly, e) <= 15.0);
            let at_yard = l.crofts.iter().any(|p| contains(&p.pts, e) || edge_dist(&p.pts, e) <= 8.0);
            if d <= 40.0 && std::env::var("NEAR_LIST").is_ok() {
                eprintln!("NEAR {tag} {} {d:.0} ring {ring} green {:.0} water {} centre ({:.0}, {:.0})", off(e), green.map_or(-1.0, |(gc, gr)| dist(e, gc) - gr), by_water(e), l.center[0], l.center[1]);
            }
            if d <= 40.0 {
                r.add("lane ends near a way but not joined", 1.0, format!("{tag} {} {d:.0} ft off (lane {si}, {} pts, {}{}{})", off(e), s.len(), if at_building { "house " } else { "" }, if at_yard { "yard " } else { "" }, if by_water(e) { "water" } else { "" }));
            }
            if !at_building && !at_yard && !by_water(e) {
                let near_b = l.buildings.iter().map(|b| edge_dist(&b.poly, e)).fold(f64::MAX, f64::min);
                let near_p = l.crofts.iter().map(|p| edge_dist(&p.pts, e)).fold(f64::MAX, f64::min);
                r.add("lane ends not joined, at nothing", 1.0, format!("{tag} {} {} ft out (lane {si}, {} pts, way {d:.0} ft, building {near_b:.0}, plot {near_p:.0})", off(e), dist(e, l.center) as i64, s.len()));
            }
        }
        // (Across a later lane or a road.)
        for w in s.windows(2) {
            for o in segs[..n_cross].iter().filter(|o| o.3.is_none_or(|j| j > si)) {
                let Some(x) = hit(w[0], w[1], o.0, o.1) else { continue };
                if dist(x, s[0]) <= 1.0 || dist(x, s[s.len() - 1]) <= 1.0 {
                    continue;
                }
                // (A road across a lane at a point of the lane's own: a crossroads.)
                if o.3.is_none() && s.iter().any(|p| dist(*p, x) <= 1.0) {
                    continue;
                }
                if let Some(j) = o.3 {
                    let q = &l.streets[j];
                    if dist(x, q[0]) <= 1.0 || dist(x, q[q.len() - 1]) <= 1.0 || (s.iter().any(|p| dist(*p, x) <= 1.0) && q.iter().any(|p| dist(*p, x) <= 1.0)) {
                        continue;
                    }
                }
                r.add("lanes crossing a lane or road away from a shared node", 1.0, format!("{tag} {} {} (lane {si} {} to {})", off(x), if o.3.is_some() { "lane" } else { "road" }, off(s[0]), off(s[s.len() - 1])));
            }
        }
    }
    r.tally("farmsteads", l.crofts.iter().filter(|p| format!("{:?}", p.kind) == "Farmyard").count());
}

/// `NEAR_DUMP=L<n>:<x>,<y>,<r>` prints what stands within `r` ft of the offset (x, y) from that
/// village's middle: buildings, plots, lanes, roads, `rural::battle` runs and fields (stderr).
pub(crate) fn near_dump(l: &Layout, bt: &worldgen::town::rural::Battle, tag: &str) {
    let Ok(v) = std::env::var("NEAR_DUMP") else { return };
    let Some((t, rest)) = v.split_once(':') else { return };
    let n: Vec<f64> = rest.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if t != tag || n.len() < 3 {
        return;
    }
    let (q, rad) = (add(l.center, [n[0], n[1]]), n[2]);
    let off = |p: P| format!("({:.0},{:.0})", p[0] - l.center[0], p[1] - l.center[1]);
    let poly = |ps: &[P]| ps.iter().map(|p| off(*p)).collect::<Vec<_>>().join(" ");
    let near = |ps: &[P]| contains(ps, q) || edge_dist(ps, q) < rad;
    for (i, b) in l.buildings.iter().enumerate().filter(|(_, b)| near(&b.poly)) {
        eprintln!("bldg {i} c {} {:?} {:?} {:?} {}", off(centroid(&b.poly)), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.structure, poly(&b.poly));
    }
    for (i, p) in l.crofts.iter().enumerate().filter(|(_, p)| near(&p.pts)) {
        let held = l.buildings.iter().filter(|b| contains(&p.pts, centroid(&b.poly))).count();
        eprintln!("plot {i} {:?} {:?} holds {held}: {}", p.kind, p.bound, poly(&p.pts));
    }
    for (i, s) in l.streets.iter().enumerate().filter(|(_, s)| s.windows(2).any(|w| seg_dist(q, w[0], w[1]) < rad)) {
        eprintln!("lane {i}: {}", poly(s));
    }
    for (pts, _, w) in l.roads.iter().filter(|(s, _, _)| s.windows(2).any(|w| seg_dist(q, w[0], w[1]) < rad)) {
        eprintln!("layout road w {w:.0}: {}", poly(pts));
    }
    for &(a, b, k) in bt.runs.iter().filter(|r| seg_dist(q, r.0, r.1) < rad) {
        eprintln!("run {k:?} {} {}", off(a), off(b));
    }
    for (i, f) in l.fields.iter().enumerate().filter(|(_, f)| near(f)) {
        let x = &l.field_info[i];
        eprintln!("field {i} {:?} {:?}: {}", x.kind, x.bound, poly(f));
    }
}

/// R5 towns (103 fix): plots (crofts, yards) holding no building; farmyards with no lane to their
/// gate; a plot's sides and back (as drawn, a croft's front open) within 6 ft (a yard's 4) of a building it does
/// not hold, one in no other plot (a cottage beside it with no croft of its own, a hedge across its
/// corner); the battlemap's runs
/// (hedges, fences, walls, field rows) within 2 ft of any building, roofed or not; lanes through a
/// building (the lane's band over it).
pub(crate) fn plot_checks(l: &Layout, bt: &worldgen::town::rural::Battle, tag: &str, r: &mut Report) {
    let off = |p: P| format!("({:.0}, {:.0})", p[0] - l.center[0], p[1] - l.center[1]);
    // (A footprint's middle by its corners' mean: `centroid` loses a round home's small area in world ft.)
    let mid = |q: &[P]| mul(q.iter().fold([0.0, 0.0], |s, p| add(s, *p)), 1.0 / q.len().max(1) as f64);
    let holds = |p: &town::village::Plot, b: &town::Building| contains(&p.pts, mid(&b.poly));
    for p in &l.crofts {
        if !l.buildings.iter().any(|b| holds(p, b)) {
            r.add("plots holding no building", 1.0, format!("{tag} {:?} {}", p.kind, off(mid(&p.pts))));
        }
        let n = p.pts.len();
        let first = if p.kind == town::village::PlotKind::Croft { 1 } else { 0 };
        for k in first..n {
            let (a, e) = (p.pts[k], p.pts[(k + 1) % n]);
            for b in l.buildings.iter().filter(|b| !holds(p, b)) {
                // (A croft's 6 ft; a yard's 4, the cottages laid out after it: `village::off_yards`.)
                let clear = if p.kind == town::village::PlotKind::Croft { 6.0 } else { 4.0 };
                if seg_poly(a, e, &b.poly) >= clear && !contains(&b.poly, a) {
                    continue;
                }
                // (In a plot of its own: that plot keeps its bounds off it, along this one's or not.)
                let shared = l.crofts.iter().any(|q| !std::ptr::eq(q, p) && holds(q, b));
                if !shared {
                    r.add("plot bounds by a building not theirs", 1.0, format!("{tag} {:?} side {} {} by {:?} {:?} at {:.1} ft", p.kind, off(a), off(e), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key), seg_poly(a, e, &b.poly)));
                }
            }
        }
    }
    for p in l.crofts.iter().filter(|p| p.kind == town::village::PlotKind::Farmyard) {
        let gate = lerp(p.pts[0], p.pts[1], 0.5);
        if !l.streets.iter().any(|s| s.windows(2).any(|w| seg_dist(gate, w[0], w[1]) < 8.0)) {
            r.add("farmyards with no lane to their gate", 1.0, format!("{tag} {}", off(gate)));
        }
    }
    // (A churchyard's graveyard is a building of its own, open: its wall runs round it.)
    let graveyard = town::catalog::index_of("graveyard").map(|f| f as u16);
    for &(a, e, k) in &bt.runs {
        if let Some(b) = l.buildings.iter().filter(|b| b.func.is_none() || b.func != graveyard).find(|b| contains(&b.poly, a) || contains(&b.poly, e) || seg_poly(a, e, &b.poly) < 2.0) {
            r.add("runs by any building", 1.0, format!("{tag} {k:?} {} {} by {:?} {:?} {:?}", off(a), off(e), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.structure));
        }
    }
    for (si, s) in l.streets.iter().enumerate() {
        for w in s.windows(2) {
            if let Some(b) = l.buildings.iter().find(|b| contains(&b.poly, w[0]) || seg_poly(w[0], w[1], &b.poly) < 0.5 * town::village::LANE_W - 0.5) {
                r.add("lanes through a building", 1.0, format!("{tag} lane {si} {} {} by {:?} {:?}", off(w[0]), off(w[1]), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key)));
                break;
            }
        }
    }
}
