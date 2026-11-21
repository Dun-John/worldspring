//! `build`: an interior laid out level by level; terraces and styled rooms furnished, chapel doors.

use super::*;

pub(super) fn build(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building) -> Interior {
    build_noting(world, t0, l, settlement, b, &mut Vec::new(), &mut Vec::new())
}

/// `build`, noting the floors a building laid out by zones fell back on the shared layout for
/// (`hub_fallbacks`) and its halls furnished as wholes (`hall_plans`).
pub(super) fn build_noting(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, fell: &mut Vec<i8>, plans: &mut Vec<HallPlan>) -> Interior {
    let bi = b.id as usize;
    let mut rng = Pcg32::new(hash3(world.stream("interior"), town::rng_key(t0, settlement), bi as i64, 0x1a7), 71);
    let arch = arch_of(b);
    let key = func_key(b);

    // Grid along the footprint's long axis.
    let sh = Shell::of(l, b);
    let (nx, ny, origin, u, v) = (sh.nx, sh.ny, sh.origin, sh.u, sh.v);
    let world_at = |x: f64, y: f64| sh.world_at(x, y);
    let inside = &sh.inside;
    // (Roadmap 7, 164: an inn round its yard galleried onto it above its ground floor, `layout_level`.)
    let galleried = sh.yard(b).filter(|_| town::palace::is_inn(b));
    let ext_free = |i: isize, j: isize| sh.ext_free(i, j);
    let is_in = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && inside[j as usize * nx + i as usize];
    // Open exterior squares along each side (0 top, 1 right, 2 bottom, 3 left).
    let open_on = |side: usize| -> usize {
        let (di, dj) = [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)][side];
        (0..ny as isize).flat_map(|j| (0..nx as isize).map(move |i| (i, j))).filter(|&(i, j)| is_in(i, j) && !is_in(i + di, j + dj) && ext_free(i + di, j + dj)).count()
    };

    // The front: the open grid side nearest a street (else nearest the settlement centre).
    let sides = [[0.5 * nx as f64, 0.0], [nx as f64, 0.5 * ny as f64], [0.5 * nx as f64, ny as f64], [0.0, 0.5 * ny as f64]];
    let street_d = |p: P| {
        l.roads
            .iter()
            .flat_map(|(pts, _, w)| pts.windows(2).map(move |s| geom::seg_dist(p, s[0], s[1]) - 0.5 * w))
            .fold(f64::MAX, f64::min)
            .min(town::village::front_d(l, p))
    };
    let open: Vec<usize> = (0..4).map(open_on).collect();
    let any_open = open.iter().any(|&n| n > 0);
    // (A temple's, a shrine's or a monastery's with no street near: a side open all along before one a
    // neighbour stands against, so that its door doesn't open into the gap between them.)
    let edge_on = |side: usize| -> usize {
        let (di, dj) = [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)][side];
        (0..ny as isize).flat_map(|j| (0..nx as isize).map(move |i| (i, j))).filter(|&(i, j)| is_in(i, j) && !is_in(i + di, j + dj)).count()
    };
    let faith = town::temple::is_faith(b);
    let front = (0..4)
        .filter(|&k| !any_open || open[k] > 0)
        .min_by(|&x, &y| {
            let score = |k: usize| {
                let p = world_at(sides[k][0], sides[k][1]);
                let d = street_d(p);
                let shut = if faith && open[k] < edge_on(k) { 1e5 } else { 0.0 };
                if d < 80.0 { d } else { 1e4 + shut + geom::dist(p, l.center) }
            };
            score(x).total_cmp(&score(y))
        })
        .unwrap_or(0);
    // R5 towns: a cemetery's tomb, one chamber on one storey, its door on its footprint's first side
    // (toward its walk: `town::cemetery`).
    let tomb = key == Some("tomb");
    let front = if tomb && b.poly.len() >= 2 {
        let door = geom::lerp(b.poly[0], b.poly[1], 0.5);
        (0..4).min_by(|&x, &y| geom::dist(world_at(sides[x][0], sides[x][1]), door).total_cmp(&geom::dist(world_at(sides[y][0], sides[y][1]), door))).unwrap_or(front)
    } else {
        front
    };
    // (Roadmap 7 P8: a lighthouse's door not onto its keeper's cottage or oil store (`harbour::
    // annexes`): else the side clear of them nearest it.)
    let front = if key == Some("lighthouse") {
        let annex: Vec<&Vec<P>> = l.buildings.iter().filter(|o| town::harbour::is_annex(o) && geom::poly_gap(&o.poly, &b.poly) < 2.0).map(|o| &o.poly).collect();
        let along = |k: usize, t: f64| {
            let d = [[0.0, -0.5], [0.5, 0.0], [0.0, 0.5], [-0.5, 0.0]][k];
            world_at(sides[k][0] + d[0] + t * d[1].abs() * 2.0, sides[k][1] + d[1] + t * d[0].abs() * 2.0)
        };
        let out = |k: usize| along(k, 0.0);
        // (Its door's outside square anywhere along the middle of that side, a battlemap square off them.)
        // (And on the breakwater's made ground, the whole square round it, not out on its armour.)
        let made: Vec<Vec<P>> = l.moles.iter().filter(|m| m.pts.last().is_some_and(|t| geom::dist(*t, geom::centroid(&b.poly)) < 4.0)).flat_map(|m| m.polys()).collect();
        let dry = |p: P| made.is_empty() || [[-2.0, -2.0], [2.0, -2.0], [2.0, 2.0], [-2.0, 2.0]].iter().all(|d| made.iter().any(|g| geom::contains(g, add(p, *d))));
        let clear = |k: usize| dry(out(k)) && annex.iter().all(|a| [-1.0, 0.0, 1.0].iter().all(|&t| geom::poly_gap(a, &[along(k, t)]) >= 3.6));
        // (On a breakwater's head its grid runs along the mole (`shell::lighthouse_back`): the door
        // on the side toward the bank, down the lane its annexes leave along the mole's middle.)
        if shell::lighthouse_back(l, b).is_some() && open[1] > 0 {
            1
        } else if clear(front) {
            front
        } else {
            (0..4).filter(|&k| clear(k) && (!any_open || open[k] > 0)).min_by(|&x, &y| geom::dist(out(x), out(front)).total_cmp(&geom::dist(out(y), out(front)))).unwrap_or(front)
        }
    } else {
        front
    };
    // R5 M20: a building on a deck over the water opens on the deck's lane: the side nearest its
    // centre line.
    let front = match b.deck {
        Some([a, e]) => (0..4).min_by(|&x, &y| geom::seg_dist(world_at(sides[x][0], sides[x][1]), a, e).total_cmp(&geom::seg_dist(world_at(sides[y][0], sides[y][1]), a, e))).unwrap_or(front),
        None => front,
    };
    // (Roadmap 6 R4: a row's house opens on its row's road: the side nearest its `faces` chord.)
    let front = match town::sites::faces(world, t0, l) {
        Some([a, e]) => (0..4).min_by(|&x, &y| geom::seg_dist(world_at(sides[x][0], sides[x][1]), a, e).total_cmp(&geom::seg_dist(world_at(sides[y][0], sides[y][1]), a, e))).unwrap_or(front),
        None => front,
    };

    // A home on a round footprint (under a round roof: `town::round_off`): a yurt, one round room
    // on one storey (no cellar, no stairs).
    let yurt = arch == Arch::House && b.parts.is_empty() && round_ish(b);
    // (R5 towns: a steppe town's round felt hall, one as a yurt is: `felt`.)
    let felt = town::roof::felt_built(b).filter(|_| yurt);
    // Levels: a cellar, the ground floor and the storeys above; at least two levels (a yurt one).
    let floors = b.floors.max(1) as i8;
    // (A tower mill: its stair up the wall, `mill_stairs`.)
    let mill = arch == Arch::Tower && key == Some("windmill");
    // (Roadmap 7 P8: a round tower of more than one level: its stair winds up the inside of its
    // wall, `winder`; no stair block.)
    let winding = arch == Arch::Tower && winds(b);
    let mut zs: Vec<i8> = vec![-1];
    zs.extend(0..floors);
    // Keeps and towers (or any roof drawn so): walk the battlements on the roof.
    let open_roof = battlements(b).is_some();
    // A keep's corner towers rise a storey above its battlements (a keep built of pieces: its own
    // towers, `Shell::towers`).
    let t = if b.parts.is_empty() && corner_towers(b) { keep_tower_squares(nx, ny) } else { 0 };
    // A castle's keep, a big barracks, a palace and a noble estate are laid out by zones (`hub`): a
    // keep of pieces up to its body's top, each piece as high as it rises (its towers under cones: no
    // tops of their own).
    let zoned = zoned_arch(b);
    // (Only a keep has the grid's corner towers.)
    let corner = if arch == Arch::Keep { t } else { 0 };
    let capital = capital_of(t0, settlement);
    let mut plan = if zoned { zone_plan(world, t0, l, settlement, b, &sh, corner) } else { None };
    // A flat roof (or one round a dome) is walked on too: a roof terrace inside its parapet (not on a
    // building laid out by zones: a palace's ranges round its court, a town hall's under its belfry).
    let roof_terrace = if open_roof || zoned { None } else { terrace(l, b) };
    if plan.is_none() && zoned && hub::kind_of(b, key, &sh, capital, city_of(t0, settlement)).is_some() {
        fell.extend(0..floors);
    }
    // (A town hall's belfry: its storeys above the hall's, `hub::Plan::belfry`.)
    let belfry = plan.as_ref().and_then(|p| p.belfry());
    if let Some(p) = plan.as_ref().filter(|p| p.pieces || belfry.is_some()) {
        zs = (-1..=p.reaches()).collect();
    } else if open_roof || roof_terrace.is_some() {
        zs.push(ROOF);
    }
    let towers: Option<Vec<Vec<usize>>> = (arch == Arch::Keep && !b.parts.is_empty()).then(|| sh.towers.iter().map(|t| t.0.clone()).collect());
    // (With their ways in.)
    let rings: Option<Vec<(Vec<usize>, Vec<usize>)>> = towers.as_ref().map(|t| t.iter().cloned().zip(sh.tower_doors.iter().cloned()).collect());
    // (A castle's keep reshaped by hand off its grid: no towers on its grid's corners either.)
    let rings = rings.or_else(|| (arch == Arch::Keep && !corner_towers(b)).then(Vec::new));
    if arch == Arch::Keep && (t > 0 || towers.as_ref().is_some_and(|t| !t.is_empty())) && open_roof && !plan.as_ref().is_some_and(|p| p.pieces) {
        zs.push(TOWER_TOP);
    }
    if yurt || tomb {
        zs = vec![0];
    }
    // (No cellar under a watchtower on its islet's rock, or R5 M20 a building on a deck.)
    let no_cellar = !yurt && !tomb && !has_cellar(b);
    if no_cellar {
        zs.retain(|&z| z != -1);
    }
    let entry_level = if yurt || tomb || no_cellar { 0 } else { 1 };

    // The stair block: at the back of the building, in the middle of the short side (towers:
    // near the centre), on inside squares. Inns and taverns lay out the ground floor first
    // and put the stairs in the taproom, where guests can reach them.
    let mut ground_pre: Option<Level> = None;
    // (On squares the outer wall as drawn doesn't cut; in a building too small for that, as far
    // in from it as they go.)
    // (Off a keep's towers and their ways in: their spiral stairs are theirs.)
    let off_towers = |mut m: Vec<bool>| {
        for k in towers.iter().flatten().flatten().chain(sh.tower_doors.iter().flatten()) {
            m[*k] = false;
        }
        m
    };
    let whole = off_towers(sh.whole.clone());
    // (A tower mill's stair up its wall, a straight one: the stones and the gearing in the middle.)
    let arch_st = if mill { Arch::Warehouse } else { arch };
    // (Roadmap 7 P8: a lighthouse's spiral stair against its wall, two squares by two, from its
    // foot to its roof, the lamp room on its middle squares there: `lighthouse_squares`.)
    let lamp = if arch == Arch::Tower && key == Some("lighthouse") && plan.is_none() { lighthouse_squares(inside, &whole, nx, ny, front) } else { None };
    let mut stairs = match &plan {
        Some(p) => p.stairs,
        None if winding => [0, 0, 1, 1],
        None if lamp.is_some() => lamp.as_ref().map_or([0, 0, 1, 1], |(st, _, _)| *st),
        None if mill => mill_stairs(&whole, nx, ny, front).unwrap_or_else(|| place_stairs(&whole, nx, ny, front, arch_st)),
        None if key == Some("shrine") => shrine_stairs(&whole, nx, ny, front).unwrap_or_else(|| place_stairs(&whole, nx, ny, front, arch_st)),
        None => place_stairs(&whole, nx, ny, front, arch_st),
    };
    let on = |st: [usize; 4], m: &[bool]| (st[1]..st[1] + st[3]).all(|j| (st[0]..st[0] + st[2]).all(|i| m[j * nx + i]));
    if plan.is_none() && lamp.is_none() && !winding && !on(stairs, &whole) {
        for t in [0.1, 0.0, f64::MIN] {
            let m = off_towers(sh.whole_by(t));
            stairs = place_stairs(&m, nx, ny, front, arch_st);
            if on(stairs, &m) {
                break;
            }
        }
        // (Where no two squares side by side are clear of the wall but one is: a ladder's square
        // there, the clear one nearest the stairs' place.)
        if !on(stairs, &whole) {
            let c = [stairs[0] as f64 + 0.5 * stairs[2] as f64, stairs[1] as f64 + 0.5 * stairs[3] as f64];
            let d = |k: usize| ((k % nx) as f64 + 0.5 - c[0]).abs() + ((k / nx) as f64 + 0.5 - c[1]).abs();
            let clear = off_towers(sh.whole_by(WALL_HALF - 1e-3));
            if let Some(k) = (0..nx * ny).filter(|&k| clear[k]).min_by(|&a, &b| d(a).total_cmp(&d(b)).then(a.cmp(&b))) {
                stairs = [k % nx, k / nx, 1, 1];
            }
        }
    }
    // (An inn laid out by zones has its stairs in its taproom already: `hub::inn`.)
    if is_pub(arch) && plan.is_none() {
        let prog = program(arch, 0, floors == 1, key);
        let lvl = layout_level(&inside, nx, ny, &prog, [0, 0, 0, 0], front, true, arch, rings.as_deref(), None, &ext_free, &mut rng);
        // (Clear of the outer wall as drawn if they can be; in the taproom whatever.)
        // (Then on squares the wall cuts if it leaves them clear enough (or none is), else, in a
        // taproom of 12 squares or more, a square beside a door clear of it, else on the cut squares;
        // where the taproom has room for them nowhere, the ground floor all taproom, as a small inn's
        // is, and a square beside a door if need be.)
        let clear = sh.whole_by(WALL_HALF - 1e-3);
        let cut = pub_stairs(&lvl, nx, ny, inside, true, false);
        let roomy = lvl.doors.iter().find(|d| d.kind == "front").is_some_and(|d| lvl.cells.iter().filter(|&&c| c == d.rooms[0]).count() >= 12);
        let st = pub_stairs(&lvl, nx, ny, &sh.whole, true, false)
            .or_else(|| pub_stairs(&lvl, nx, ny, &sh.whole, false, false))
            .or_else(|| cut.filter(|&s| on(s, &clear) || !clear.iter().any(|&c| c)))
            .or_else(|| if roomy { pub_stairs(&lvl, nx, ny, &sh.whole, false, true) } else { None })
            .or(cut);
        if let Some(st) = st {
            stairs = st;
            ground_pre = Some(lvl);
        } else {
            let all = LevelProgram { rooms: prog.rooms[..1].to_vec(), filler: None, ..prog };
            let lvl = layout_level(&inside, nx, ny, &all, [0, 0, 0, 0], front, true, arch, rings.as_deref(), None, &ext_free, &mut rng);
            let st = pub_stairs(&lvl, nx, ny, &sh.whole, true, false)
                .or_else(|| pub_stairs(&lvl, nx, ny, &sh.whole, false, false))
                .or_else(|| pub_stairs(&lvl, nx, ny, inside, true, false))
                .or_else(|| pub_stairs(&lvl, nx, ny, &sh.whole, false, true))
                .or_else(|| pub_stairs(&lvl, nx, ny, inside, false, true));
            if let Some(st) = st {
                stairs = st;
                ground_pre = Some(lvl);
            }
        }
    }

    if yurt {
        stairs = [0, 0, 0, 0];
    }
    // (A tomb's none, a felt hall's (R5 towns): the block a design gives a building without stairs,
    // `design::stairs_of`.)
    if tomb || felt.is_some() {
        stairs = [0, 0, 1, 1];
    }

    // (Furniture stands clear of the outer wall as drawn: in a building laid out by zones (a keep's
    // round tower rooms), off any square its middle line cuts more than a tenth of a square into;
    // elsewhere off a square it cuts deeply, its far corner 0.4 of a square beyond it.)
    // (A tower mill's round wall cuts its rim squares: nothing but the stair stands on one.)
    let floor = sh.whole_by(if plan.is_some() { -0.1 } else if mill { -0.15 } else { -0.4 });
    // Great halls furnished as wholes (`hall`): a castle's, a town's or a guild's, a noble estate's,
    // a barracks' mess hall; thrones only in a castle or a palace.
    let (_, thrones, styled) = hall_kind(world, t0, settlement, b);
    // (A temple's, a shrine's or a monastery's rooms open to every square: `furnish_rooms`, `faith`.)
    let mut levels = Vec::new();
    let mut wind: Option<super::winder::Winder> = None;
    for (li, &z) in zs.iter().enumerate() {
        let top = li + 1 == zs.len() || matches!(zs.get(li + 1), Some(&ROOF) | Some(&TOWER_TOP));
        // (A keep of pieces' top as the shared code would have it, if its zones won't do.)
        let pieces_top = plan.as_ref().is_some_and(|p| p.pieces && z == p.reaches() && p.kind != hub::Kind::ShellKeep);
        // (A hovel's program by its kind: it has no function's key.)
        let hovel = b.func.is_none() && town::catalog::RESIDENTIAL[b.residential as usize] == "hovel";
        let prog = if z == ROOF && roof_terrace.is_some() {
            LevelProgram { name: "Roof terrace", rooms: vec![(TERRACE, 1.0)], filler: None, corridor: false, cells: false, by_size: false }
        } else {
            program(arch, if pieces_top { ROOF } else { z.min(ROOF) }, top, if yurt { Some(felt.map_or("yurt", felt::room)) } else if hovel { Some("hovel") } else { key })
        };
        // (Laid out by zones, if it holds together: every room in one piece and reached.)
        let laid = plan.as_mut().and_then(|p| p.level(z));
        let tried = laid.is_some();
        // (A belfry's storey above the hall's: reached by its spiral stair alone.)
        let tower_only = belfry.is_some_and(|_| z >= floors);
        let reach_st = match belfry {
            Some((sp, _)) if tower_only => [sp % nx, sp / nx, 1, 1],
            _ => stairs,
        };
        let hubbed = laid.and_then(|h| {
            let (name, roof, items, halls) = (h.name, h.roof, h.items.clone(), h.halls.clone());
            // (Its stair hall's squares all the belfry's room: no main stairs there.)
            let mut lvl = hub_level(h, &sh, nx, ny, if tower_only { [0, 0, 0, 0] } else { stairs }, corner, front, z == 0, &ext_free, &mut rng);
            if tower_only {
                lvl.has_stairs = false;
            }
            (rooms_whole(&lvl.cells, nx, ny) && reaches_all(&lvl, nx, ny, reach_st)).then_some((lvl, name, roof, items, halls))
        });
        if tried && hubbed.is_none() {
            fell.push(z);
        }
        let hall_f: Vec<hall::Frame> = hubbed.as_ref().map(|h| h.4.clone()).unwrap_or_default();
        let (mut lvl, hubbed) = match (&ground_pre, z, hubbed) {
            (_, _, Some((lvl, name, roof, items, _))) => (lvl, Some((name, roof, items))),
            (Some(g), 0, None) => (g.clone(), None),
            // (Its lamp room's door toward where the winding stair comes out.)
            (_, ROOF, None) if lamp.is_some() => {
                let land = wind.as_ref().and_then(|w| w.run(li).first().copied()).map_or(stairs, |k| [k % nx, k / nx, 1, 1]);
                (lamp_top(nx, ny, inside, lamp.as_ref().map_or(&[][..], |l| &l.2[..]), land, &mut rng), None)
            }
            (_, TOWER_TOP, None) => (tower_tops(&inside, nx, ny, t, towers.as_deref().unwrap_or_default(), stairs, &mut rng), None),
            _ => {
                // (A monastery's crypt under its church only: `hub::Plan::crypt`.)
                let crypt = if z < 0 { plan.as_ref().and_then(|p| p.crypt()) } else { None };
                (layout_level(crypt.as_deref().unwrap_or(inside), nx, ny, &prog, stairs, front, z == 0, arch, rings.as_deref(), galleried.as_ref().map(|y| y.0.as_slice()), &ext_free, &mut rng), None)
            }
        };
        // The roof is one storey above the top floor, the tower tops one above that.
        let zf = match z {
            ROOF => floors,
            TOWER_TOP => floors + 1,
            _ => z,
        };
        lvl.z = zf;
        // (Roadmap 7, 163: an L, a U or a court, `town::wings`, opens onto its yard.)
        if z == 0 && hubbed.is_none() && town::wings::ranges(l, b).is_some() {
            super::doors::yard_door(&mut lvl, nx, ny, stairs, &ext_free);
        }
        lvl.has_stairs &= !yurt && !tomb;
        lvl.name = if z == TOWER_TOP { "Tower tops".to_string() } else { prog.name.to_string() };
        lvl.elevation_ft = b.pad_ft + STOREY_FT * zf as f32;
        if let Some((name, roof, items)) = &hubbed {
            lvl.name = name.to_string();
            lvl.furniture.extend(items.iter().copied());
            if *roof {
                lvl.roof = true;
                lvl.windows.clear();
            }
        } else if pieces_top {
            lvl.name = "Battlements".to_string();
            lvl.roof = true;
            lvl.windows.clear();
        }
        if z == ROOF || z == TOWER_TOP {
            // Open to the sky: the outer wall is a parapet (no windows).
            lvl.roof = true;
            lvl.windows.clear();
        }
        // Keeps: a spiral stair in each corner tower, from the ground floor to the tower top.
        if arch == Arch::Keep && t > 0 && zf >= 0 {
            for (i, j) in [(0, 0), (nx - 1, 0), (0, ny - 1), (nx - 1, ny - 1)] {
                if lvl.cells[j * nx + i] >= 0 {
                    push_item(&mut lvl.furniture, "spiral_stair", i, j);
                }
            }
        }
        // A round tower's winding stair up its wall, from its cellar (under the ground floor's well)
        // to the floor under its top, its ground floor's run from beside its door; the cellar's
        // ways down laid round it.
        if winding {
            lvl.has_stairs = false;
            if z == 0 {
                let known: Vec<&Level> = levels.iter().chain([&lvl]).collect();
                wind = super::winder::plan(&known, li, zs.len(), inside, nx, ny);
                for (lj, below) in levels.iter_mut().enumerate() {
                    if let Some(w) = &wind {
                        w.lay(below, nx, ny, lj);
                    }
                    if below.z == -1 {
                        cellar_links(Some(below), world, t0, l, b, &sh, stairs, false);
                    }
                }
            }
            if let Some(w) = &wind {
                w.lay(&mut lvl, nx, ny, li);
            }
        }
        // A lighthouse's lamp in the middle of the lamp room on its roof (off its doorway).
        if let Some((_, lp, inner)) = &lamp
            && z == ROOF
        {
            let doorway = super::winder::doorways(&lvl, nx, ny);
            let mid = [0.5 * nx as f64, 0.5 * ny as f64];
            let d = |k: usize| geom::dist([(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5], mid);
            let at = inner.iter().copied().filter(|k| !doorway.contains(k) && lvl.cells[*k] >= 0).min_by(|&a, &b| d(a).total_cmp(&d(b)).then(a.cmp(&b))).or(Some(*lp).filter(|&k| lvl.cells[k] >= 0 && !doorway.contains(&k)));
            if let Some(k) = at {
                push_item(&mut lvl.furniture, "lighthouse_lamp", k % nx, k / nx);
            }
        }
        // A town hall's belfry: its spiral stair from the top floor's landing up.
        if let Some((sp, _)) = belfry
            && z >= floors - 1
            && lvl.cells[sp] >= 0
        {
            push_item(&mut lvl.furniture, "spiral_stair", sp % nx, sp / nx);
        }
        // A castle keep's private stair: its spiral stair at the dais end from the hall's floor up
        // to the lord's rooms (`hub::Plan::private_stair`).
        if let Some((sp, z0, z1)) = plan.as_ref().and_then(|p| p.private_stair())
            && hubbed.is_some()
            && (z0..=z1).contains(&z)
            && lvl.cells[sp] >= 0
        {
            push_item(&mut lvl.furniture, "spiral_stair", sp % nx, sp / nx);
        }
        // A keep built of pieces: a spiral stair in the middle of each of its towers; their rooms
        // round in round towers.
        if arch == Arch::Keep && zf >= 0 {


            for (sq, round, mid) in &sh.towers {
                let r = lvl.cells[*mid];
                if r >= 0 {
                    // (A round one's stair winds up its wall: laid once every floor is built.)
                    if !*round {
                        push_item(&mut lvl.furniture, "spiral_stair", mid % nx, mid / nx);
                    }
                    if *round && sq.iter().all(|&k| lvl.cells[k] == r) && shape::fits(&lvl.cells, nx, r, shape::ROUND) {
                        lvl.rooms[r as usize].shape = shape::ROUND;
                    }
                }
            }
        }
        // A round tower's rooms are round (while their squares fit the shape).
        if arch == Arch::Tower && round_ish(b) {
            for ri in 0..lvl.rooms.len() {
                if lvl.rooms[ri].squares > 0 && shape::fits(&lvl.cells, nx, ri as i16, shape::ROUND) {
                    lvl.rooms[ri].shape = shape::ROUND;
                }
            }
        }
        // (A temple's apse, a chapter house's octagon: drawn so while its squares are the shape's.)
        for ri in 0..lvl.rooms.len() {
            if lvl.rooms[ri].shape != 0 && !shape::fits(&lvl.cells, nx, ri as i16, lvl.rooms[ri].shape) {
                lvl.rooms[ri].shape = 0;
            }
        }
        // (A building laid out by zones, on a level the shared code laid out (a barracks' roof): its
        // corner towers' storey a guardroom, stores, an armory and a garrison, as on its floors.)
        if let Some(p) = plan.as_ref().filter(|_| hubbed.is_none()) {
            p.finish_shared(&mut lvl);
        }
        // (Its great hall furnished as a whole, in the frame its zones gave it or one found in its
        // squares; a castle's minstrels' gallery over an open hall, its musicians' corner.)
        let mut halls: Vec<(i16, hall::Frame, bool, hall::Style)> = Vec::new();
        for &(kind, style) in &styled {
            for ri in (0..lvl.rooms.len()).filter(|&ri| lvl.rooms[ri].kind == kind && lvl.rooms[ri].squares > 0) {
                let ours = |f: &hall::Frame| {
                    let (i, j) = f.grid(f.len as isize / 2, f.wid as isize / 2);
                    i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && lvl.cells[j as usize * nx + i as usize] == ri as i16
                };
                // (A chapel along its length, its altar toward the east where it can be.)
                let found = if style == hall::Style::Chapel { chapel_frame(&lvl, nx, ny, ri as i16, stairs, u, v, Some(&floor)) } else { hall_f.iter().copied().find(ours).map(|f| (f, false)).or_else(|| hall_frame(&lvl, nx, ny, ri as i16, stairs)) };
                if let Some((f, turns)) = found {
                    halls.push((ri as i16, f, turns, style));
                }
            }
        }
        // A roof terrace: its domes standing on it and a few things about (none where its domes leave
        // it in pieces or over the stairs).
        if z == ROOF
            && let Some(domes) = &roof_terrace
            && !furnish_terrace(&mut lvl, nx, ny, stairs, domes, &sh, hash3(world.stream("interior.terrace"), town::rng_key(t0, settlement), bi as i64, 0))
        {
            continue;
        }
        let open_hall = lvl.rooms.iter().any(|r| r.kind == OPEN_BELOW && r.squares > 0);
        let galleries: Vec<i16> = (0..lvl.rooms.len() as i16).filter(|&ri| open_hall && lvl.rooms[ri as usize].kind == "minstrels' gallery").collect();
        let skip: Vec<i16> = halls.iter().map(|h| h.0).chain(galleries.iter().copied()).collect();
        furnish_rooms(&mut lvl, nx, ny, stairs, None, Some(&floor), &skip, thrones, faith, &mut rng);
        if yurt {
            // (Its door on the ring's first side, where the battlemap draws it: `battlePrep.yurtDoor`;
            // turned toward the lane or the centre by `town::round_off`.)
            let mean = mul(b.poly.iter().fold([0.0, 0.0], |a, p| add(a, *p)), 1.0 / b.poly.len() as f64);
            let d = sub(mul(add(b.poly[0], b.poly[1]), 0.5), mean);
            match felt {
                Some(f) => {
                    felt::furnish(&mut lvl, nx, ny, [geom::dot(d, u), geom::dot(d, v)], &|i, j| ext_free(i, j), f);
                    // (A felt temple's candle stands in its god's look.)
                    let god = town::temple::god_look(b);
                    lvl.furniture.iter_mut().filter(|it| it.kind == "candle_stand").for_each(|it| it.variant = Some(god));
                }
                None => furnish_yurt(&mut lvl, nx, ny, [geom::dot(d, u), geom::dot(d, v)], &|i, j| ext_free(i, j)),
            }
        }
        for &(ri, f, turns, style) in &halls {
            let seed = hash3(world.stream("interior.hall"), town::rng_key(t0, settlement), bi as i64, zf as i64);
            let n = lvl.furniture.len();
            let mut f = furnish_styled(&mut lvl, nx, ny, stairs, ri, (f, turns), style, Some(&floor), seed);
            // (A chapel with no altar, a door on its axis at the end: `chapel_door`.)
            if style == hall::Style::Chapel
                && !lvl.furniture[n..].iter().any(|it| it.kind == "altar")
                && let Some(g) = chapel_door(&mut lvl, nx, ny, stairs, ri, f, Some(&floor), seed, n)
            {
                f = g;
            }
            // (A shrine whose god's statue fits nowhere in its frame (a slanted lot's): furnished by its
            // rules instead, its pieces in the god's look.)
            if let hall::Style::Shrine { god } = style
                && !lvl.furniture[n..].iter().any(|it| it.kind == "god_statue")
            {
                lvl.furniture.truncate(n);
                let mut frng = Pcg32::new(hash3(seed, ri as i64, 0x5f1, 0), 77);
                furnish_rooms(&mut lvl, nx, ny, stairs, Some(ri), Some(&floor), &[], thrones, faith, &mut frng);
                for it in lvl.furniture[n..].iter_mut().filter(|it| matches!(it.kind, "god_statue" | "candle_stand")) {
                    it.variant = Some(god);
                }
            }
            // (A noble estate's hall whose zones' frame holds no dais (a slanted lot's end cut across
            // it): in the largest box of its squares.)
            if matches!(style, hall::Style::Estate { .. }) && !lvl.furniture[n..].iter().any(|it| it.kind == "low_dais")
                && let Some(alt) = hall_frame(&lvl, nx, ny, ri, stairs).filter(|a| a.0 != f)
            {
                let kept = lvl.furniture.split_off(n);
                let g = furnish_styled(&mut lvl, nx, ny, stairs, ri, alt, style, Some(&floor), seed);
                if lvl.furniture[n..].iter().any(|it| it.kind == "low_dais") {
                    f = g;
                } else {
                    lvl.furniture.truncate(n);
                    lvl.furniture.extend(kept);
                }
            }
            plans.push(HallPlan { level: li, room: ri, frame: f, style });
        }
        for &ri in &galleries {
            hall::furnish_gallery(&mut lvl, nx, ny, ri);
        }
        // (Laid out by zones: nothing stands on an open top (a platform, a walk).)
        if let Some(p) = &plan {
            p.clear_tops(&mut lvl);
        }
        if z == -1 && !winding {
            cellar_links(Some(&mut lvl), world, t0, l, b, &sh, stairs, false);
        }
        // (A guild's arms in its own look; a thieves' guild hangs none.)
        match arms_look(key) {
            Some(a) => lvl.furniture.iter_mut().filter(|it| it.kind == "guild_arms" && it.variant.is_none()).for_each(|it| it.variant = Some(a)),
            None => lvl.furniture.retain(|it| it.kind != "guild_arms"),
        }
        levels.push(lvl);
    }
    // (Roadmap 7 P8) A keep of pieces' round towers: each one's stair winds up its wall from the
    // ground floor to its top, kept off its doorways, beside the ground floor's way in.

    if arch == Arch::Keep {
        for (sq, _, mid) in sh.towers.iter().filter(|t| t.1) {
            let on: Vec<usize> = (0..levels.len()).filter(|&li| levels[li].z >= 0 && levels[li].cells[*mid] >= 0).collect();
            let mut mask = vec![false; nx * ny];
            for &k in sq {
                mask[k] = true;
            }
            let known: Vec<&Level> = on.iter().map(|&li| &levels[li]).collect();
            let door = known.first().and_then(|g| super::winder::doorways(g, nx, ny).into_iter().find(|&k| mask[k]));
            // (Where no winding stair keeps off its doorways, a spiral stair in its middle.)
            let wind = super::winder::ring_of(&mask, nx, ny).filter(|_| on.len() >= 2).map(|ring| super::winder::plan_ring(ring, &known, 0, on.len(), door, nx, ny)).filter(|w| w.1);
            for (lj, &li) in on.iter().enumerate() {
                match &wind {
                    Some((w, _)) => w.lay(&mut levels[li], nx, ny, lj),
                    None => push_item(&mut levels[li].furniture, "spiral_stair", mid % nx, mid / nx),
                }
            }
        }
    }
    Interior {
        id: format!("b:{}:{}", l.index, bi),
        settlement: settlement as u32,
        building: bi as u32,
        name: b.name.clone(),
        theme: None,
        stone: 0,
        look: 0,
        function: b.label(),
        origin,
        axis: u,
        across: v,
        nx,
        ny,
        levels,
        entry_level,
        stairs,
        sprites: Vec::new(),
        outlines: sh.outlines(),
        yard: sh.yard(b).map(|y| y.1).unwrap_or_default(),
    }
}

/// A roof terrace (`terrace`, one room): its domes as pieces standing on it, each over the squares
/// whose middles lie inside its drum by a quarter of a square or more; then, sparsely, along its
/// parapet water jars, planters, a bench and sacks of grain, out on it a rug and a washing line,
/// clear of the stairs and never cutting it in two. False (no terrace) where a dome stands on the
/// stairs or leaves fewer than six squares, or parts of it out of reach.
fn furnish_terrace(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], domes: &[(P, f64)], sh: &Shell, seed: u64) -> bool {
    let n = nx * ny;
    let Some(ri) = lvl.cells.iter().copied().find(|&c| c >= 0 && lvl.rooms[c as usize].kind == TERRACE) else { return false };
    let mine = |k: usize| lvl.cells[k] == ri;
    let on_st = |k: usize| (st[0]..st[0] + st[2]).contains(&(k % nx)) && (st[1]..st[1] + st[3]).contains(&(k / nx));
    let mut blocked = vec![false; n];
    let mut items: Vec<Item> = Vec::new();
    for &(c, r) in domes {
        let ks: Vec<usize> = (0..n).filter(|&k| mine(k) && geom::dist(sh.world_at((k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5), c) < r - 0.25 * SQUARE_FT).collect();
        if ks.is_empty() {
            continue;
        }
        let (x0, x1) = ks.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
        let (y0, y1) = ks.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
        for k in (y0..=y1).flat_map(|j| (x0..=x1).map(move |i| j * nx + i)) {
            if on_st(k) {
                return false;
            }
            blocked[k] = true;
        }
        items.extend(building_item("roof_dome", x0, y0, x1 - x0 + 1, y1 - y0 + 1));
    }
    // (Every square of it not under a dome reached from the stairs.)
    let whole = |blocked: &[bool]| {
        let mut seen = vec![false; n];
        let mut stack: Vec<usize> = (0..n).filter(|&k| on_st(k) && mine(k)).collect();
        for &k in &stack {
            seen[k] = true;
        }
        let mut count = 0;
        while let Some(k) = stack.pop() {
            count += 1;
            let (i, j) = (k % nx, k / nx);
            for q in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten() {
                if !seen[q] && mine(q) && !blocked[q] {
                    seen[q] = true;
                    stack.push(q);
                }
            }
        }
        (count, (0..n).all(|k| seen[k] || !mine(k) || blocked[k]))
    };
    let (open, ok) = whole(&blocked);
    if !ok || open < 6 + st[2] * st[3] {
        return false;
    }
    // (Its name off its domes: on the open square nearest the open squares' middle.)
    if !items.is_empty() {
        let free: Vec<usize> = (0..n).filter(|&k| mine(k) && !blocked[k] && !on_st(k)).collect();
        let m = free.iter().fold([0.0, 0.0], |a, &k| [a[0] + ((k % nx) as f64 + 0.5) / free.len() as f64, a[1] + ((k / nx) as f64 + 0.5) / free.len() as f64]);
        let d = |k: usize| geom::dist([(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5], m);
        if let Some(&k) = free.iter().min_by(|&&a, &&b| d(a).total_cmp(&d(b)).then(a.cmp(&b))) {
            lvl.rooms[ri as usize].center = [(k % nx) as f32 + 0.5, (k / nx) as f32 + 0.5];
        }
    }
    // Free squares: on it, off the domes, the stairs and their landing squares round them.
    let mut taken = blocked.clone();
    for k in 0..n {
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        if !mine(k) || (-1..=1).any(|dj| (-1..=1).any(|di| (i + di) >= 0 && (j + dj) >= 0 && ((i + di) as usize) < nx && ((j + dj) as usize) < ny && on_st((j + dj) as usize * nx + (i + di) as usize))) {
            taken[k] = true;
        }
    }
    for it in &items {
        for j in (it.y as usize).saturating_sub(1)..((it.y + it.h) as usize + 1).min(ny) {
            for i in (it.x as usize).saturating_sub(1)..((it.x + it.w) as usize + 1).min(nx) {
                taken[j * nx + i] = true;
            }
        }
    }
    let wall = |k: usize| {
        let (i, j) = (k % nx, k / nx);
        i == 0 || j == 0 || i + 1 == nx || j + 1 == ny || [k - 1, k + 1, k - nx, k + nx].iter().any(|&q| !mine(q))
    };
    let mut rng = Pcg32::new(seed, 83);
    let mut spots: Vec<usize> = (0..n).filter(|&k| !taken[k] && wall(k)).collect();
    for k in (1..spots.len()).rev() {
        spots.swap(k, rng.below(k as u32 + 1) as usize);
    }
    // Along the parapet: a few of these, more on a big terrace.
    let along = ["water_jar", "planter", "bench", "water_jar", "sacks", "planter", "water_jar"];
    let want = (open / 10).clamp(2, along.len());
    let mut placed = 0;
    for &k in &spots {
        if placed >= want {
            break;
        }
        if taken[k] {
            continue;
        }
        let kind = along[placed];
        // (A bench two squares along the parapet where there is room, else one.)
        let (i, j) = (k % nx, k / nx);
        let pair = if kind == "bench" { [(i + 1 < nx).then(|| k + 1), (j + 1 < ny).then(|| k + nx)].into_iter().flatten().find(|&q| !taken[q] && wall(q)) } else { None };
        let (w, h) = match pair {
            Some(q) if q == k + 1 => (2, 1),
            Some(_) => (1, 2),
            None => (1, 1),
        };
        let Some(it) = building_item(kind, i, j, w, h) else { continue };
        let cells: Vec<usize> = (j..j + h).flat_map(|y| (i..i + w).map(move |x| y * nx + x)).collect();
        if it.blocks_move {
            let mut b2 = blocked.clone();
            for &q in &cells {
                b2[q] = true;
            }
            if !whole(&b2).1 {
                continue;
            }
            blocked = b2;
        }
        for &q in &cells {
            taken[q] = true;
        }
        items.push(it);
        placed += 1;
    }
    // Out on it (walked over, walked under): a rug laid out, a washing line between its posts.
    for (kind, w, h) in [("rug", 2, 2), ("washing_line", 3, 1), ("washing_line", 1, 3)] {
        if kind == "washing_line" && items.iter().any(|it| it.kind == kind) {
            continue;
        }
        let fits: Vec<(usize, usize)> = (0..ny.saturating_sub(h - 1))
            .flat_map(|j| (0..nx.saturating_sub(w - 1)).map(move |i| (i, j)))
            .filter(|&(i, j)| (j..j + h).all(|y| (i..i + w).all(|x| !taken[y * nx + x])))
            .collect();
        // (Off the parapet where it can be.)
        let inner: Vec<(usize, usize)> = fits.iter().copied().filter(|&(i, j)| (j..j + h).all(|y| (i..i + w).all(|x| !wall(y * nx + x)))).collect();
        let fits = if inner.is_empty() { fits } else { inner };
        if fits.is_empty() {
            continue;
        }
        let (i, j) = fits[rng.below(fits.len() as u32) as usize];
        for y in j..j + h {
            for x in i..i + w {
                taken[y * nx + x] = true;
            }
        }
        items.extend(building_item(kind, i, j, w, h));
    }
    lvl.furniture.extend(items);
    true
}

/// Room `ri` furnished as a hall (`hall::furnish_hall`) in frame `f`; where it may be turned (`f.1`,
/// `hall_frame`: entered on a long wall) and holds no dais at the far end, the other way round. The
/// frame it is furnished in.
#[allow(clippy::too_many_arguments)]
pub(super) fn furnish_styled(lvl: &mut Level, nx: usize, ny: usize, stairs: [usize; 4], ri: i16, f: (hall::Frame, bool), style: hall::Style, whole: Option<&[bool]>, seed: u64) -> hall::Frame {
    let (f, turns) = f;
    let n = lvl.furniture.len();
    if hall::furnish_hall(lvl, nx, ny, stairs, ri, f, style, whole, seed) || !turns {
        return f;
    }
    lvl.furniture.truncate(n);
    let back = hall::Frame { o: f.grid(f.len as isize - 1, 0), ds: (-f.ds.0, -f.ds.1), ..f };
    if hall::furnish_hall(lvl, nx, ny, stairs, ri, back, style, whole, seed) {
        return back;
    }
    lvl.furniture.truncate(n);
    hall::furnish_hall(lvl, nx, ny, stairs, ri, f, style, whole, seed);
    f
}

/// A chapel furnished with no altar at either end of frame `f` where a door opens in that end's wall
/// on its axis (as generated): that door moved along the wall off the axis, between the same rooms,
/// where the altar then has room, the chapel furnished again (its frame; its furniture first cleared
/// from `n` on), the level's walls run again. None, nothing changed, where no such move gives it one.
#[allow(clippy::too_many_arguments)]
fn chapel_door(lvl: &mut Level, nx: usize, ny: usize, stairs: [usize; 4], ri: i16, f: hall::Frame, whole: Option<&[bool]>, seed: u64, n: usize) -> Option<hall::Frame> {
    let style = hall::Style::Chapel;
    let back = hall::Frame { o: f.grid(f.len as isize - 1, 0), ds: (-f.ds.0, -f.ds.1), ..f };
    let kept = lvl.furniture.split_off(n);
    for fr in [f, back] {
        let (l, w) = (fr.len as isize, fr.wid as isize);
        let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
        let room_at = |lvl: &Level, p: (isize, isize)| (p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < nx && (p.1 as usize) < ny).then(|| lvl.cells[p.1 as usize * nx + p.0 as usize]);
        // (A door's two squares, as `pub_stairs` reads its edge.)
        let squares = |d: &Door| {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
        };
        let on_axis: Vec<usize> = (0..lvl.doors.len())
            .filter(|&i| {
                let sq = squares(&lvl.doors[i]);
                (cl..=cr).any(|t| {
                    let (p, q) = (fr.grid(l - 1, t), fr.grid(l, t));
                    (sq[0] == p && sq[1] == q) || (sq[0] == q && sq[1] == p)
                })
            })
            .collect();
        for i in on_axis {
            let (d0, sq) = (lvl.doors[i].clone(), squares(&lvl.doors[i]));
            let pair = (room_at(lvl, sq[0]), room_at(lvl, sq[1]));
            for k in [1isize, -1, 2, -2, 3, -3] {
                let (dx, dy) = (fr.dt.0 * k, fr.dt.1 * k);
                let moved = [(sq[0].0 + dx, sq[0].1 + dy), (sq[1].0 + dx, sq[1].1 + dy)];
                let t = fr.local(moved[0].0, moved[0].1).1.min(fr.local(moved[1].0, moved[1].1).1);
                // (Not where a piece or the stairs already stand in the room beyond.)
                let on = |p: (isize, isize), x: u16, y: u16, w: u16, h: u16| (x as isize..(x + w) as isize).contains(&p.0) && (y as isize..(y + h) as isize).contains(&p.1);
                let held = |p: (isize, isize)| lvl.furniture.iter().any(|it| on(p, it.x, it.y, it.w, it.h)) || on(p, stairs[0] as u16, stairs[1] as u16, stairs[2] as u16, stairs[3] as u16);
                if (cl..=cr).contains(&t) || (room_at(lvl, moved[0]), room_at(lvl, moved[1])) != pair || held(moved[0]) || held(moved[1]) || lvl.doors.iter().enumerate().any(|(j, e)| j != i && squares(e) == moved) {
                    continue;
                }
                (lvl.doors[i].a, lvl.doors[i].b) = ([d0.a[0] + dx as f32, d0.a[1] + dy as f32], [d0.b[0] + dx as f32, d0.b[1] + dy as f32]);
                if hall::furnish_hall(lvl, nx, ny, stairs, ri, fr, style, whole, seed) {
                    // (The walls run past where it was, open where it is.)
                    lvl.walls = wall_runs(&lvl.cells, &lvl.rooms, nx, ny, &lvl.doors);
                    return Some(fr);
                }
                lvl.furniture.truncate(n);
                lvl.doors[i] = d0.clone();
            }
        }
    }
    lvl.furniture.extend(kept);
    None
}
