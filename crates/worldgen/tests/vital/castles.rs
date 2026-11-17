//! Castles: grounds, works, reach and moats.

use super::*;

//// Every city's castle in seeds 1 to 8 holds together (`castle_works`: its keep laid out by zones,
/// about half its halls open two storeys), an inner curtain in about half of them, each at least
/// `GATES_APART` (90 ft) from its outer gate.
#[test]
fn castles_guarantee() {
    use worldgen::t0::settle::Tier;
    use worldgen::town::{self, geom};
    let (mut castles, mut inner, mut open_halls, mut closed_halls) = (0, 0, 0, 0);
    for seed in 1..=8 {
        let w = World::from_json(&world_json(seed)).unwrap();
        let t = worldgen::t0::T0::generate(&w);
        for si in (0..t.settlements.len()).filter(|&i| t.settlements[i].tier >= Tier::City) {
            let l = town::layout(&w, &t, si);
            // A castle the town planned stands on its own patch, never the market square's, and is built.
            if let Some(cp) = l.castle_patch {
                assert!(l.plaza_patch != Some(cp), "#{si} (seed {seed}): the castle on the market square's patch {cp}");
                assert!(!l.castles.is_empty(), "#{si} (seed {seed}): the castle planned on patch {cp} has no curtain");
            }
            if l.castles.is_empty() {
                continue;
            }
            castle_works(&w, &t, &l, si, &mut open_halls, &mut closed_halls);
            castle_ground(&w, &t, &l, si, &l.plazas.iter().find(|p| p.kind == town::PlazaKind::Bailey).expect("a bailey").pts);
            castles += 1;
            if l.castles.len() > 1 {
                inner += 1;
                // The gates: pairs of gate towers 52 ft apart in the castle; the inner one the nearest
                // the inner bailey's middle.
                let g: Vec<[f64; 2]> = l.gate_towers.iter().copied().filter(|p| l.castles.iter().any(|(c, r)| geom_dist(*p, *c) <= r + 1.0)).collect();
                let mut mids = Vec::new();
                for a in 0..g.len() {
                    for b in a + 1..g.len() {
                        if (geom_dist(g[a], g[b]) - 52.0).abs() < 0.5 {
                            mids.push(geom::lerp(g[a], g[b], 0.5));
                        }
                    }
                }
                let ci = l.castles[1].0;
                let (k, m) = mids.iter().enumerate().min_by(|a, b| geom_dist(*a.1, ci).total_cmp(&geom_dist(*b.1, ci))).expect("an inner gate");
                let gap = mids.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, o)| geom_dist(*o, *m)).fold(f64::MAX, f64::min);
                assert!(gap >= 89.0, "#{si} (seed {seed}): the inner gate {gap:.0} ft from the outer one");
                // The inner curtain stands the outer bailey's width (40 ft at least) inside the outer
                // one all along, not only at its corners.
                let bailey = &l.plazas.iter().find(|p| p.kind == town::PlazaKind::Bailey).expect("a bailey").pts;
                let n = bailey.len();
                let depth = |p: [f64; 2]| (0..n).map(|k| geom::seg_dist(p, bailey[k], bailey[(k + 1) % n])).fold(f64::MAX, f64::min);
                for w in l.walls.iter().filter(|w| w.iter().all(|p| geom::contains(bailey, *p) && depth(*p) > 20.0)) {
                    for s in w.windows(2) {
                        let steps = (geom_dist(s[0], s[1]) / 5.0).ceil().max(1.0) as usize;
                        for j in 0..=steps {
                            let p = geom::lerp(s[0], s[1], j as f64 / steps as f64);
                            assert!(depth(p) >= 40.0, "#{si} (seed {seed}): the inner curtain {:.0} ft from the outer at {p:?}", depth(p));
                        }
                    }
                }
            }
        }
    }
    assert!(castles >= 40, "too few castles checked ({castles})");
    // About half the keeps with a floor over the hall have it open two storeys.
    assert!(open_halls * 4 >= open_halls + closed_halls && closed_halls * 4 >= open_halls + closed_halls, "{open_halls} halls open two storeys, {closed_halls} not");
    assert!(inner * 10 >= castles * 3 && inner * 10 <= castles * 7, "{inner} of {castles} castles with an inner curtain");
}

/// A town's castle keeps to its ground: no town gate opens straight into its bailey and no street or
/// road runs through it but the castle's own way to its gate; its bailey is closed all round, by its
/// curtain, the town wall or a tower, but where the water or a river's channel is; and no curtain
/// runs alongside the town wall within 30 ft of it for more than 30 ft (where the town wall is the
/// castle's, it has none of its own). `ring`: its bailey.
pub(super) fn castle_ground(world: &World, t0: &worldgen::t0::T0, l: &worldgen::town::Layout, si: usize, ring: &[[f64; 2]]) {
    use worldgen::town::geom;
    let n = ring.len();
    let depth = |p: [f64; 2]| {
        let d = (0..n).map(|k| geom::seg_dist(p, ring[k], ring[(k + 1) % n])).fold(f64::MAX, f64::min);
        if geom::contains(ring, p) { d } else { -d }
    };
    // Points every 5 ft along a polyline, and the way it runs there.
    let along = |pts: &[[f64; 2]]| -> Vec<([f64; 2], [f64; 2])> {
        let mut v = Vec::new();
        for s in pts.windows(2) {
            let d = geom_dist(s[0], s[1]);
            if d < 1e-6 {
                continue;
            }
            let u = geom::mul(geom::sub(s[1], s[0]), 1.0 / d);
            let m = (d / 5.0).ceil() as usize;
            v.extend((0..=m).map(|k| (geom::lerp(s[0], s[1], k as f64 / m as f64), u)));
        }
        v
    };
    let crosses = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (o1, o2) = (geom::cross(geom::sub(b, a), geom::sub(c, a)), geom::cross(geom::sub(b, a), geom::sub(d, a)));
        let (o3, o4) = (geom::cross(geom::sub(d, c), geom::sub(a, c)), geom::cross(geom::sub(d, c), geom::sub(b, c)));
        o1 * o2 < 0.0 && o3 * o4 < 0.0
    };
    let towers = worldgen::interior::towers(l);
    let segs: Vec<([f64; 2], [f64; 2])> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    // The castle's gates: pairs of gate towers a passage apart in it (one set in the town by hand has
    // its towers among `extra_towers`); its own way, from its outer gate.
    let g: Vec<[f64; 2]> = l.gate_towers.iter().copied().chain(l.extra_towers.iter().filter(|t| t.1).map(|t| t.0)).filter(|p| l.castles.iter().any(|(c, r)| geom_dist(*p, *c) <= r + 1.0)).collect();
    let mut gates = Vec::new();
    for a in 0..g.len() {
        for b in a + 1..g.len() {
            if (geom_dist(g[a], g[b]) - 52.0).abs() < 0.5 {
                gates.push(geom::lerp(g[a], g[b], 0.5));
            }
        }
    }
    let outer = gates.iter().copied().min_by(|a, b| depth(*a).abs().total_cmp(&depth(*b).abs())).expect("a castle gate");
    let own = |r: &(Vec<[f64; 2]>, u8, f64)| r.1 == 3 && (r.2 - 10.0).abs() < 1e-9 && geom_dist(r.0[0], outer) < 12.0;
    // No town gate opens into the bailey: from its middle no straight way into it crosses no wall or tower.
    for &tg in l.gates.iter().filter(|q| depth(**q) > -60.0) {
        for k in 0..36 {
            let a = k as f64 * std::f64::consts::TAU / 36.0;
            for r in [15.0, 25.0, 35.0, 45.0] {
                let p = [tg[0] + r * a.cos(), tg[1] + r * a.sin()];
                let open = depth(p) > 4.0 && !segs.iter().any(|s| crosses(tg, p, s.0, s.1) || geom::seg_dist(p, s.0, s.1) < 4.5) && !towers.iter().any(|t| geom::seg_dist(t.0, tg, p) < t.1);
                assert!(!open, "#{si}: the town gate at {tg:?} opens into the bailey (to {p:?})");
            }
        }
    }
    // No street or road through the bailey but its own way.
    for st in l.streets.iter().chain(l.roads.iter().filter(|r| !own(r)).map(|r| &r.0)) {
        let inside = along(st).iter().filter(|x| depth(x.0) > 3.0).count() as f64 * 5.0;
        assert!(inside <= 15.0, "#{si}: a street runs {inside:.0} ft inside the bailey");
    }
    // Closed all round, but by the water.
    let by_river = |p: [f64; 2]| {
        let c = worldgen::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 6.0, t0.cell_ft);
        (c.0 - p[0]).abs() + (c.1 - p[1]).abs() > 0.01
    };
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
    let mut closed = ring.to_vec();
    closed.push(ring[0]);
    let mut open = 0.0;
    for (p, _) in along(&closed) {
        let shut = segs.iter().any(|s| geom::seg_dist(p, s.0, s.1) <= 10.0) || towers.iter().any(|t| geom_dist(t.0, p) < t.1 + 6.0) || gates.iter().any(|m| geom_dist(*m, p) < 30.0);
        let wet = t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice) || by_river(p);
        open = if shut || wet { 0.0 } else { open + 5.0 };
        assert!(open <= 15.0, "#{si}: the bailey is open on dry ground at {p:?}");
    }
    // No curtain just inside the town wall: the castle's walls are those on its rings (the outer
    // bailey's, or wholly inside it, an inner curtain's), the town's the rest.
    let on_ring = |p: [f64; 2]| depth(p).abs() < 1.5;
    let (mut ours, mut town) = (Vec::new(), Vec::new());
    for w in &l.walls {
        let pts = along(w);
        let on = pts.iter().filter(|x| on_ring(x.0)).count();
        if w.iter().all(|p| depth(*p) > 20.0) || on * 10 >= pts.len() * 8 {
            ours.push(w);
        } else {
            town.extend(w.windows(2).map(|s| (s[0], s[1])));
        }
    }
    for w in ours {
        let mut run = 0.0;
        for (p, u) in along(w) {
            let beside = town.iter().any(|&(a, b)| {
                let d = geom_dist(a, b);
                d > 1.0 && geom::cross(u, geom::mul(geom::sub(b, a), 1.0 / d)).abs() < 0.26 && (3.0..30.0).contains(&geom::seg_dist(p, a, b))
            });
            run = if beside { run + 5.0 } else { 0.0 };
            assert!(run <= 35.0, "#{si}: a curtain runs within 30 ft of the town wall for {run:.0} ft at {p:?}");
        }
    }
}

/// A castle holds together: its own towers and gatehouses (within its curtains) stand clear of each
/// other, nothing built stands on its walls or towers, and play mode can walk in through its gates
/// to every building in it and all but a few squares of its baileys (`castle_reach`); its keep (the
/// castle) is made of convex pieces whose union is its footprint, stands clear of the curtain, and
/// inside has a room in each of its towers on every floor (round in a round tower) with a spiral
/// stair in its middle, reached through one way in at most (the rest of the tower its wall), keeps
/// every rule interiors keep (`check_interior`), and has its great hall furnished as a whole: the
/// throne dais at its far end, nothing shutting a square of it off.
pub(super) fn castle_works(world: &World, t0: &worldgen::t0::T0, l: &worldgen::town::Layout, si: usize, open_halls: &mut usize, closed_halls: &mut usize) {
    use worldgen::interior;
    use worldgen::town::{self, PartKind, Structure, geom};
    let towers = interior::towers(l);
    let ours = |p: [f64; 2]| l.castles.iter().any(|(c, r)| geom_dist(p, *c) <= r + 1.0);
    // (A square tower reaches further across its corners.)
    let reach = |t: &([f64; 2], f64)| if l.square_tower(t.0).is_some() { t.1 * std::f64::consts::SQRT_2 } else { t.1 };
    for (a, ta) in towers.iter().enumerate().filter(|(_, t)| ours(t.0)) {
        for tb in towers[a + 1..].iter().filter(|t| ours(t.0)) {
            assert!(geom_dist(ta.0, tb.0) >= reach(&(ta.0, ta.1)) + reach(&(tb.0, tb.1)) - 3.0, "#{si}: towers at {:?} and {:?} stand on each other", ta.0, tb.0);
        }
    }
    let near_wall = |p: &[[f64; 2]]| l.walls.iter().flat_map(|w| w.windows(2)).any(|s| p.iter().any(|q| geom::seg_dist(*q, s[0], s[1]) < 4.0) || geom::contains(p, s[0]));
    let fi = town::catalog::index_of("castle").unwrap() as u16;
    let mut keeps = 0;
    // (A temple's pieces are no keep's: `town::temple`.)
    for b in l.buildings.iter().filter(|b| b.func == Some(fi) || (!b.parts.is_empty() && !town::temple::shaped(b) && town::belfry::of(b).is_none())) {
        let tag = format!("#{si} b:{si}:{}", b.id);
        assert!(!near_wall(&b.poly), "{tag}: the keep stands on a wall");
        assert!(towers.iter().all(|t| !geom::contains(&b.poly, t.0)), "{tag}: a tower stands in the keep");
        if b.parts.is_empty() {
            continue;
        }
        keeps += 1;
        assert!(b.parts[0].kind == PartKind::Body, "{tag}: its first piece is not its body");
        for p in &b.parts {
            let n = p.poly.len();
            let turns: Vec<f64> = (0..n).map(|k| geom::cross(geom::sub(p.poly[(k + 1) % n], p.poly[k]), geom::sub(p.poly[(k + 2) % n], p.poly[(k + 1) % n]))).collect();
            assert!(turns.iter().all(|&t| t >= -1e-6) || turns.iter().all(|&t| t <= 1e-6), "{tag}: a piece is not convex");
            // (Its corners' mean: `centroid` on a small piece far out in world ft is not exact.)
            let c = geom::mul(p.poly.iter().fold([0.0, 0.0], |a, q| geom::add(a, *q)), 1.0 / n as f64);
            assert!(geom::contains(&b.poly, c), "{tag}: a piece lies outside its footprint");
        }
        let whole: f64 = geom::area(&b.poly).abs();
        let biggest = b.parts.iter().map(|p| geom::area(&p.poly).abs()).fold(0.0, f64::max);
        assert!(whole >= biggest - 1.0, "{tag}: its footprint is smaller than its body");
        if b.structure != Structure::Roofed {
            continue;
        }
        let it = interior::generate(world, t0, si, b.id as usize).expect("a keep can be entered");
        // Laid out by zones on every floor, none fallen back on the shared layout.
        let fell = interior::hub_fallbacks(world, t0, si, b.id as usize);
        assert!(fell.is_empty(), "{tag}: floors {fell:?} not laid out by zones");
        let sh = interior::Shell::of(l, b);
        let (nx, ny) = (it.nx, it.ny);
        assert_eq!(sh.towers.len(), b.parts.iter().filter(|p| matches!(p.kind, PartKind::Tower { .. })).count(), "{tag}: a tower has no room");
        // Each tower (`Shell::towers`): a room filling it on the ground floor and every floor up to
        // its top (two at least), round in a round tower, its spiral stair at its middle, a way in.
        for (ti, (sq, round, mid)) in sh.towers.iter().enumerate() {
            let on: Vec<usize> = (0..it.levels.len()).filter(|&li| it.levels[li].z >= 0 && it.levels[li].cells[*mid] >= 0).collect();
            assert!(on.len() >= 2 && on[0] == it.entry_level && on.windows(2).all(|w| w[1] == w[0] + 1), "{tag}: tower {ti} stands on levels {on:?}");
            for &li in &on {
                let lv = &it.levels[li];
                let room = lv.cells[*mid];
                assert!(sq.iter().all(|&k| lv.cells[k] == room), "{tag} {}: tower {ti}'s room doesn't fill it", lv.name);
                assert!((lv.rooms[room as usize].shape == interior::shape::ROUND) == *round, "{tag} {}: tower {ti}'s room is drawn {} round", lv.name, if *round { "not" } else { "" });
                assert!(lv.furniture.iter().any(|f| f.kind == "spiral_stair" && f.y as usize * nx + f.x as usize == *mid), "{tag} {}: tower {ti} without its spiral stair", lv.name);
                assert!(lv.doors.iter().any(|d| d.rooms.contains(&room)), "{tag} {}: tower {ti}'s room has no way in", lv.name);
            }
        }
        // Laid out by zones (`interior::hub`): its great hall on the ground floor (a great tower's
        // on the first, over its stores); a hall, L or T keep entered through its screens passage
        // (or a forebuilding's entrance hall opening onto it). Entered at its low end (the user's
        // call, 2026-10-27): from the main stair, and from the front door where the hall is on the
        // ground floor, the way into the hall (fewest doors) comes through its screens passage where
        // it has one, and enters it on the half away from its throne dais, never behind it; the
        // lord's private spiral stair in a room the hall opens into at the dais end, up to the floor
        // above (where its hall has a screens passage and a floor over it). A hall open two
        // storeys: its height left open above it (no way into it), a minstrels' gallery over it
        // reached.
        let kind = interior::hub::keep_kind(b).expect("a keep of pieces");
        use interior::hub::Kind;
        let hall_z = if matches!(kind, Kind::Great | Kind::Plain) && b.floors >= 2 { 1 } else { 0 };
        let li = it.levels.iter().position(|lv| lv.z == hall_z).expect("the hall's floor");
        let g = &it.levels[li];
        let hall = g.rooms.iter().position(|r| r.kind == "great hall").unwrap_or_else(|| panic!("{tag} ({}): no great hall on {}", kind.name(), g.name)) as i16;
        if kind != Kind::ShellKeep {
            let at = |r: i16| {
                let sq: Vec<usize> = (0..nx * ny).filter(|&k| g.cells[k] == r).collect();
                [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64]
            };
            let st = it.stairs;
            let hc = at(hall);
            let dais = g.furniture.iter().find(|f| f.kind == "throne_dais" && g.cells[f.y as usize * nx + f.x as usize] == hall).map(|f| [f.x as f64 + 0.5 * f.w as f64, f.y as f64 + 0.5 * f.h as f64]);
            let screens = g.rooms.iter().position(|r| r.kind == "screens passage" && r.squares > 0).map(|s| s as i16);
            // (The way from a room into the hall, fewest doors first: the room it comes from and the
            // middle of the door it enters by.)
            let way_in = |from: i16| -> Option<(i16, [f64; 2])> {
                let mut prev: Vec<Option<(i16, usize)>> = vec![None; g.rooms.len()];
                let mut q = std::collections::VecDeque::from([from]);
                let mut seen = vec![false; g.rooms.len()];
                seen[from as usize] = true;
                while let Some(a) = q.pop_front() {
                    if a == hall {
                        let (p, di) = prev[a as usize]?;
                        let d = &g.doors[di];
                        return Some((p, [0.5 * (d.a[0] + d.b[0]) as f64, 0.5 * (d.a[1] + d.b[1]) as f64]));
                    }
                    for (di, d) in g.doors.iter().enumerate().filter(|(_, d)| d.rooms.contains(&a) && d.rooms.iter().all(|&o| o >= 0)) {
                        let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
                        if !seen[o as usize] {
                            seen[o as usize] = true;
                            prev[o as usize] = Some((a, di));
                            q.push_back(o);
                        }
                    }
                }
                None
            };
            let mut starts = vec![("main stair", g.cells[st[1] * nx + st[0]])];
            if g.z == 0
                && let Some(front) = g.doors.iter().find(|d| d.kind == "front")
            {
                starts.push(("front door", front.rooms[0]));
            }
            for (what, from) in starts {
                assert!(from >= 0 && from != hall, "{tag} ({}): the {what} opens straight into the hall", kind.name());
                let (before, door) = way_in(from).unwrap_or_else(|| panic!("{tag} ({}): no way from the {what} into the hall", kind.name()));
                if let Some(d) = dais {
                    assert!(geom::dot(geom::sub(door, hc), geom::sub(d, hc)) < 0.0, "{tag} ({}): the way in from the {what} reaches the hall at its dais end or its middle", kind.name());
                }
                if let Some(s) = screens {
                    assert!(before == s, "{tag} ({}): the way in from the {what} comes into the hall from the {}, not the screens passage", kind.name(), g.rooms[before as usize].kind);
                }
            }
            // (The private stair: a spiral stair not a tower's, at a grid corner or a piece tower's
            // middle.)
            let towered = |k: usize| sh.towers.iter().any(|t| t.2 == k) || [0, nx - 1, nx * (ny - 1), nx * ny - 1].contains(&k);
            let private = g.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize).find(|&k| !towered(k));
            if let Some(up) = it.levels.get(li + 1).filter(|lv| lv.z == g.z + 1 && !lv.roof && screens.is_some()) {
                let k = private.unwrap_or_else(|| panic!("{tag} ({}): no private stair from the hall's dais end", kind.name()));
                let r = g.cells[k];
                assert!(r >= 0 && r != hall && g.doors.iter().any(|d| d.rooms == [hall.min(r), hall.max(r)]), "{tag} ({}): the private stair isn't in a room off the hall", kind.name());
                if let Some(d) = dais {
                    assert!(geom::dot(geom::sub([(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5], hc), geom::sub(d, hc)) > 0.0, "{tag} ({}): the private stair isn't at the dais end", kind.name());
                }
                assert!(up.cells[k] >= 0 && up.furniture.iter().any(|f| f.kind == "spiral_stair" && f.y as usize * nx + f.x as usize == k), "{tag} ({}): the private stair doesn't rise to {}", kind.name(), up.name);
            }
            if matches!(kind, Kind::Hall | Kind::L | Kind::T) {
                let front = g.doors.iter().find(|d| d.kind == "front").expect("a front door");
                let r = front.rooms[0];
                let screens = |o: i16| g.rooms[o as usize].kind == "screens passage";
                let into = screens(r) || (g.rooms[r as usize].kind == "entrance hall" && g.doors.iter().any(|d| d.rooms.contains(&r) && d.rooms.iter().any(|&o| o >= 0 && screens(o))));
                assert!(into, "{tag} ({}): the front door opens into the {}", kind.name(), g.rooms[r as usize].kind);
            }
        }
        if let Some(up) = it.levels.get(li + 1).filter(|lv| lv.rooms.iter().any(|r| r.kind == interior::OPEN_BELOW && r.squares > 0)) {
            if kind != Kind::ShellKeep {
                let gallery = up.rooms.iter().position(|r| r.kind == "minstrels' gallery").unwrap_or_else(|| panic!("{tag} {}: an open hall without its gallery", up.name));
                assert!(up.doors.iter().any(|d| d.rooms.contains(&(gallery as i16))), "{tag} {}: the minstrels' gallery has no way in", up.name);
                // (Reached from the main stair through passages only, never through a room.)
                let st = it.stairs;
                let mut seen = vec![false; up.rooms.len()];
                let mut stack = vec![up.cells[st[1] * nx + st[0]]];
                while let Some(a) = stack.pop() {
                    if a < 0 || seen[a as usize] || !interior::is_circulation(up.rooms[a as usize].kind) {
                        continue;
                    }
                    seen[a as usize] = true;
                    stack.extend(up.doors.iter().filter(|d| d.rooms.contains(&a)).map(|d| if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] }));
                }
                assert!(seen[gallery], "{tag} {}: the minstrels' gallery is reached only through a room", up.name);
                // (Its open height over the hall below.)
                for k in (0..nx * ny).filter(|&k| up.cells[k] >= 0 && up.rooms[up.cells[k] as usize].kind == interior::OPEN_BELOW) {
                    assert!(g.cells[k] == hall, "{tag} {}: open to below over the {}", up.name, g.rooms[g.cells[k].max(0) as usize].kind);
                }
                *open_halls += 1;
            }
        } else if kind != Kind::ShellKeep && it.levels.iter().any(|lv| lv.z == hall_z + 1 && lv.z < b.floors as i8) {
            *closed_halls += 1;
        }
        assert!(it.outlines.len() >= b.parts.len(), "{tag}: {} outline parts for {} pieces", it.outlines.len(), b.parts.len());
        check_interior(world, t0, l, si, &format!("b:{si}:{}", b.id), &building_buckets(l));
        // Its great hall furnished as a whole (`interior::hall`): its throne dais at the far end of
        // the hall (from the screens to the dais end), and no square of its floor shut off by what
        // stands on it.
        let plans = interior::hall_plans(world, t0, si, b.id as usize);
        let plan = plans.iter().find(|p| it.levels[p.level].rooms[p.room as usize].kind == "great hall").unwrap_or_else(|| panic!("{tag} ({}): its great hall not furnished as one", kind.name()));
        let lv = &it.levels[plan.level];
        let f = plan.frame;
        let dais = lv.furniture.iter().find(|d| d.kind == "throne_dais" && lv.cells[d.y as usize * nx + d.x as usize] == plan.room).unwrap_or_else(|| panic!("{tag} ({}): no throne dais in its hall", kind.name()));
        let far = [(dais.x, dais.y), (dais.x + dais.w - 1, dais.y + dais.h - 1)].iter().map(|&(x, y)| f.local(x as isize, y as isize).0).max().unwrap();
        assert!(far >= f.len as isize - 3, "{tag} ({}): the throne dais {} rows from the far end of its hall", kind.name(), f.len as isize - 1 - far);
        let mut blocked = vec![false; nx * ny];
        for it in lv.furniture.iter().filter(|it| it.blocks_move) {
            for j in it.y as usize..(it.y + it.h) as usize {
                for i in it.x as usize..(it.x + it.w) as usize {
                    blocked[j * nx + i] = true;
                }
            }
        }
        let door_sq = |d: &interior::Door| {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
        };
        let ways: Vec<usize> = lv.doors.iter().flat_map(door_sq).filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] == plan.room).collect();
        let mut seen = vec![false; nx * ny];
        let mut stack = ways[..1.min(ways.len())].to_vec();
        for &k in &stack {
            seen[k] = true;
        }
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (a, c) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < nx && c < ny && !seen[c * nx + a] && lv.cells[c * nx + a] == plan.room && !blocked[c * nx + a] {
                    seen[c * nx + a] = true;
                    stack.push(c * nx + a);
                }
            }
        }
        assert!(ways.iter().all(|&k| seen[k]) && (0..nx * ny).all(|k| lv.cells[k] != plan.room || blocked[k] || seen[k]), "{tag} ({}): its hall's furniture shuts part of it off", kind.name());
    }
    // (A castle laid out by a town or drawn by hand keeps its keep: of pieces, but a hand castle's
    // round or drawn one.)
    let whole = l.buildings.iter().any(|b| b.func == Some(fi) && b.parts.is_empty() && b.keep.is_some());
    assert!(keeps >= 1 || whole || !l.buildings.iter().any(|b| b.func == Some(fi)), "#{si}: its keep is not made of pieces");
    for b in l.buildings.iter().filter(|b| b.parts.is_empty() && b.ward == town::catalog::Ward::Castle && geom_dist(geom::centroid(&b.poly), l.castles[0].0) < l.castles[0].1) {
        assert!(!near_wall(&b.poly), "#{si} b:{si}:{}: a building stands on the curtain", b.id);
    }
    castle_reach(l, si);
}

/// Play mode walks into a castle: on its 5-ft squares, blocked as play mode blocks them (a building
/// on a square's middle, a wall within 5.5 ft, a tower within its reach and 1.5 ft), from the
/// streets and the ground round it every building in its bailey has a square beside it reached (its
/// middle within a square and a half of a wall), and
/// all but 3% of its bailey's open squares are (slivers between a range and the curtain may not
/// be).
fn castle_reach(l: &worldgen::town::Layout, si: usize) {
    castle_reach_by(l, si, &|_| 0);
}

/// `castle_reach` with a moat (v63 castles): `moat` 2 where a square is blocked, 1 where it is by
/// the moat (a street there is no way in).
fn castle_reach_by(l: &worldgen::town::Layout, si: usize, moat: &dyn Fn([f64; 2]) -> u8) {
    use worldgen::town::{PlazaKind, geom};
    let Some(bailey) = l.plazas.iter().find(|p| p.kind == PlazaKind::Bailey).map(|p| &p.pts) else { panic!("#{si}: a castle without its bailey") };
    let (cc, cr) = l.castles[0];
    let s = 5.0;
    let (x0, y0) = (((cc[0] - cr - 60.0) / s).floor() * s, ((cc[1] - cr - 60.0) / s).floor() * s);
    let n = ((2.0 * cr + 120.0) / s).ceil() as usize;
    let mid = |k: usize| [x0 + ((k % n) as f64 + 0.5) * s, y0 + ((k / n) as f64 + 0.5) * s];
    let near = |q: &[[f64; 2]], p: [f64; 2], m: f64| q.iter().any(|v| (v[0] - p[0]).abs() < m + 200.0 && (v[1] - p[1]).abs() < m + 200.0);
    let bs: Vec<&worldgen::town::Building> = l.buildings.iter().filter(|b| near(&b.poly, cc, cr)).collect();
    let blocked: Vec<bool> = (0..n * n)
        .map(|k| {
            let p = mid(k);
            bs.iter().any(|b| geom::contains(&b.poly, p))
                || l.walls.iter().any(|w| w.windows(2).any(|q| geom::seg_dist(p, q[0], q[1]) <= 5.5))
                || l.towers.iter().any(|t| match l.square_tower(*t) {
                    Some(u) => {
                        let d = geom::sub(p, *t);
                        geom::dot(d, u).abs() <= 12.5 && geom::cross(u, d).abs() <= 12.5
                    }
                    None => geom_dist(p, *t) <= 12.5,
                })
                || l.gate_towers.iter().any(|t| geom_dist(p, *t) <= 16.5)
                || l.extra_towers.iter().any(|t| geom_dist(p, t.0) <= if t.1 { 16.5 } else { 12.5 })
                || moat(p) == 2
        })
        .collect();
    let mut seen = vec![false; n * n];
    let mut q = std::collections::VecDeque::new();
    for k in 0..n * n {
        let (i, j) = (k % n, k / n);
        let p = mid(k);
        let street = !geom::contains(bailey, p) && moat(p) == 0 && l.roads.iter().any(|(pts, _, w)| pts.windows(2).any(|q| geom::seg_dist(p, q[0], q[1]) <= 0.5 * w));
        if !blocked[k] && (i == 0 || j == 0 || i == n - 1 || j == n - 1 || street) {
            seen[k] = true;
            q.push_back(k);
        }
    }
    while let Some(k) = q.pop_front() {
        let (i, j) = (k % n, k / n);
        for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
            if a < n && b < n && !blocked[b * n + a] && !seen[b * n + a] {
                seen[b * n + a] = true;
                q.push_back(b * n + a);
            }
        }
    }
    let open: Vec<usize> = (0..n * n).filter(|&k| !blocked[k] && geom::contains(bailey, mid(k))).collect();
    let sealed = open.iter().filter(|&&k| !seen[k]).count();
    assert!(sealed * 100 <= 3 * open.len(), "#{si}: {sealed} of the bailey's {} open squares can't be reached", open.len());
    for b in bs.iter().filter(|b| b.ward == worldgen::town::catalog::Ward::Castle && geom::contains(bailey, geom::centroid(&b.poly))) {
        let m = b.poly.len();
        let by = (0..n * n).any(|k| seen[k] && (0..m).any(|e| geom::seg_dist(mid(k), b.poly[e], b.poly[(e + 1) % m]) < 7.5));
        assert!(by, "#{si} b:{si}:{}: a building in the castle can't be reached", b.id);
    }
}

/// Castles' moats: over seeds 1-4 every city castle with one (wet and dry both seen): its chunks
/// seamless across the moat (heights and water bit for bit on shared edges), a wet moat's squares
/// under water (deep in the middle), a dry ditch's difficult ground, the drawbridge a deck; and
/// play reaches every building in the bailey (the keep too) from outside over the drawbridge alone,
/// the moat's squares blocked.
#[test]
fn castle_moats() {
    use worldgen::battlemap::{HS, SQ, SQUARE_FT, Surface};
    use worldgen::t0::settle::Tier;
    let (mut wet, mut dry) = (0, 0);
    for seed in 1..=4u32 {
        let world = World::from_json(&world_json(seed)).unwrap();
        let mut ex = Executor::new(world);
        let size = ex.world.geom.tile_size_ft(ex.world.geom.max_level);
        let max = ex.world.geom.max_level;
        let cities: Vec<usize> = (0..ex.t0.settlements.len()).filter(|&i| ex.t0.settlements[i].tier >= Tier::City).collect();
        for si in cities {
            let l = worldgen::town::layout(&ex.world, &ex.t0, si);
            let Some(m) = l.moats.first().cloned() else { continue };
            if m.wet {
                wet += 1;
            } else {
                dry += 1;
            }
            let tag = format!("seed {seed} #{si} ({} moat by {:?})", if m.wet { "wet" } else { "dry" }, m.bridges[0].at);
            for b in &m.bridges {
                assert!(b.span >= m.width && b.span <= m.width + 40.0, "{tag}: a drawbridge {:.0} ft long over a moat {:.0} ft wide", b.span, m.width);
            }
            castle_reach_by(&l, si, &|p| match m.spot(p) {
                Some(s) if s.inside() && !m.bridged(p) => 2,
                Some(_) => 1,
                None => 0,
            });
            let k = |v: f64| (v / size).floor() as u32;
            let (kx0, ky0, kx1, ky1) = (k(m.bbox[0]), k(m.bbox[1]), k(m.bbox[2]), k(m.bbox[3]));
            let (mut band, mut good, mut deep, mut deck) = (0, 0, 0, 0);
            let mut chunks = std::collections::HashMap::new();
            for ky in ky0..=ky1 {
                for kx in kx0..=kx1 {
                    chunks.insert((kx, ky), ex.battlemap(TileKey::surface(max, kx, ky)));
                }
            }
            for ky in ky0..=ky1 {
                for kx in kx0..=kx1 {
                    let c = &chunks[&(kx, ky)];
                    for q in 0..SQ * SQ {
                        let p = [kx as f64 * size + ((q % SQ) as f64 + 0.5) * SQUARE_FT, ky as f64 * size + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                        let Some(s) = m.spot(p).filter(|s| s.inside()) else { continue };
                        if m.bridged(p) {
                            deck += (c.road_halo[(q / SQ + 1) * HS + q % SQ + 1] & 0x80 != 0) as usize;
                            continue;
                        }
                        band += 1;
                        good += if m.wet { (c.water_level[q] > c.height[q]) as usize } else { (c.edges[q] & 16 != 0 || s.outer < worldgen::town::moat::BANK_FT) as usize };
                        deep += (c.surface[q] == Surface::Deep) as usize;
                    }
                    // Seamless: this chunk's east and south halo rows are its neighbours' first.
                    if kx < kx1 {
                        let east = &chunks[&(kx + 1, ky)];
                        for j in 0..SQ {
                            assert_eq!(c.halo_height[(j + 1) * HS + SQ + 1].to_bits(), east.height[j * SQ].to_bits(), "{tag}: chunk ({kx}, {ky}) height seam at row {j}");
                            assert_eq!(c.halo_water[(j + 1) * HS + SQ + 1].to_bits(), east.water_level[j * SQ].to_bits(), "{tag}: chunk ({kx}, {ky}) water seam at row {j}");
                        }
                    }
                    if ky < ky1 {
                        let south = &chunks[&(kx, ky + 1)];
                        for i in 0..SQ {
                            assert_eq!(c.halo_height[(SQ + 1) * HS + i + 1].to_bits(), south.height[i].to_bits(), "{tag}: chunk ({kx}, {ky}) height seam at column {i}");
                            assert_eq!(c.halo_water[(SQ + 1) * HS + i + 1].to_bits(), south.water_level[i].to_bits(), "{tag}: chunk ({kx}, {ky}) water seam at column {i}");
                        }
                    }
                }
            }
            assert!(band > 0 && good * 10 >= band * 9, "{tag}: {good} of its {band} squares {}", if m.wet { "under water" } else { "difficult" });
            assert!(!m.wet || deep * 3 >= band, "{tag}: {deep} of its {band} squares deep water");
            assert!(deck > 0, "{tag}: no drawbridge deck");
        }
    }
    assert!(wet >= 1 && dry >= 1 && wet + dry >= 8, "{wet} wet and {dry} dry moats");
}

// v63 underground
