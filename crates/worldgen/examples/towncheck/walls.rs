use crate::prelude::*;

pub(crate) fn walls(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::battlemap::URBAN_TOWER_FLOOR;
    use worldgen::town::wallstyle::{self, TowerShape};
    const GROUPS: [&str; 5] = ["North", "West", "South", "Dry", "Steppe"];
    const SHAPES: [&str; 4] = ["round", "square", "D", "octagon"];
    r.counts(&[
        "square towers where the style has none",
        "towers not of their style's shapes",
        "towers checked on the battlemap",
        "towers whose battlemap squares don't follow their shape",
        "towers by the water with their door in it (not counted above)",
        "buildings under or within 4 ft of a town wall",
        "town walls crossing a road or street off a gate",
        "town walls crossing themselves",
        "town walls on the water (5-ft steps)",
        "gate passages blocked on the battlemap",
        "bastions over a road, a building or the water",
        "gate passages not walkable end to end on the battlemap",
        "gatehouses over a road, a building or the water",
        "castle gates not of their group's form",
        "town towers under 22 ft apart",
        "buildings in the town's patches the wall leaves outside",
        "places listed",
    ]);
    const OUTLINES: [&str; 3] = ["straight", "star", "round"];
    const MATERIALS: [&str; 3] = ["stone", "palisade", "rammed earth"];
    let lattice = ctx.ex.world.geom.spacing_ft(ctx.ex.world.geom.first_refine_level - 1);
    let all = std::env::var("WALLS_ALL").is_ok_and(|v| v == "1");
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let ns = t0.settlements.len();
    let mut listed = [0usize; 5];
    let mut to_check: Vec<(usize, Rc<Layout>)> = Vec::new();
    let mut to_walk: Vec<(usize, Rc<Layout>)> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut order: Vec<usize> = (0..town::layout_count(t0)).collect();
    // (The biggest places first: a group's cities before its towns.)
    order.sort_by_key(|&li| (std::cmp::Reverse(t0.settlements.get(li).map_or(0, |s| s.population)), li));
    for li in order {
        let l = town::layout(world, t0, li);
        let g = (l.wall_style.group % 5) as usize;
        if li < ns && t0.settlements[li].tier >= worldgen::t0::settle::Tier::Town {
            r.tally(&format!("{}: towns and up", GROUPS[g]), 1);
        }
        if l.walls.is_empty() {
            continue;
        }
        r.tally(&format!("{}: walled places", GROUPS[g]), 1);
        for t in l.towers.iter().copied().chain(l.extra_towers.iter().filter(|e| !e.1).map(|e| e.0)) {
            let (shape, _) = l.tower_form(t);
            let castle = l.tower_forms.iter().any(|f| (f.0[0] - t[0]).abs() + (f.0[1] - t[1]).abs() < 1e-6 && f.1 & 16 != 0);
            let style = l.wall_style_at(t);
            r.tally(&format!("{}: {} towers {}", GROUPS[g], if castle { "castle" } else { "wall" }, SHAPES[shape as usize]), 1);
            if wallstyle::is_corner(&l.walls, t) {
                r.tally(&format!("{}: corner towers {}", GROUPS[g], SHAPES[shape as usize]), 1);
            }
            if shape == TowerShape::Square && !style.has_square() {
                r.add("square towers where the style has none", 1.0, format!("L{li} {}", at(t)));
            }
            if shape != style.tower && shape != style.corner {
                r.add("towers not of their style's shapes", 1.0, format!("L{li} {} {}", SHAPES[shape as usize], at(t)));
            }
        }
        close_towers(&l, li, r);
        wall_rules(&l, li, lattice, t0, r);
        gate_rules(&l, li, lattice, t0, r);
        // How round the curtains run (castles' apart): the turning at each corner; runs of gentle steps
        // (12 degrees or less, one way) turning 15 degrees or more are rounded corners, each with its radius
        // (its steps' shorter sides over their turning, weighted by turning).
        for w in l.walls.iter().filter(|w| w.len() >= 3 && !l.castles.iter().any(|(c, rad)| dist(*c, w[w.len() / 2]) <= rad + 10.0)) {
            let mut run = (0.0f64, 0.0f64, 0.0f64);
            let flush = |run: &mut (f64, f64, f64), r: &mut Report| {
                if run.1.abs() >= 15.0 {
                    r.tally(&format!("{}: rounded corners", GROUPS[g]), 1);
                    r.tally(&format!("{}: rounded corners' radius ft (sum)", GROUPS[g]), (run.2 / run.1.abs()).round() as usize);
                    r.tally(&format!("{}: curtain turning deg in rounded corners", GROUPS[g]), run.1.abs().round() as usize);
                }
                *run = (0.0, 0.0, 0.0);
            };
            for k in 1..w.len() - 1 {
                let (u, v) = (sub(w[k], w[k - 1]), sub(w[k + 1], w[k]));
                let turn = libm::atan2(cross(u, v), dot(u, v)).to_degrees();
                r.tally(&format!("{}: curtain turning deg", GROUPS[g]), turn.abs().round() as usize);
                if turn.abs() > 12.0 || turn.abs() < 0.5 || run.0 * turn < 0.0 {
                    flush(&mut run, r);
                }
                if turn.abs() <= 12.0 && turn.abs() >= 0.5 {
                    let rad = len(u).min(len(v)) / turn.abs().to_radians();
                    run = (turn.signum(), run.1 + turn, run.2 + rad * turn.abs());
                }
            }
            flush(&mut run, r);
        }
        // How far the long curtains bow (slice 2c), and the town's buildings the wall leaves outside.
        let (bows, outside, inside) = curtain_bows(ctx, &l, li);
        for b in bows {
            r.value(&format!("{}: curtain bow ft (runs 150 ft and longer)", GROUPS[g]), b.0, b.1);
        }
        r.tally("buildings in the town's patches inside its wall", inside);
        for w in outside {
            r.add("buildings in the town's patches the wall leaves outside", 1.0, w);
        }
        let st = l.wall_style;
        r.tally(&format!("{}: outline {}", GROUPS[g], OUTLINES[st.outline as usize]), 1);
        r.tally(&format!("{}: material {}", GROUPS[g], MATERIALS[st.material as usize]), 1);
        r.tally(&format!("{}: bastions placed", GROUPS[g]), l.bastions.len());
        r.tally(&format!("{}: bastions dropped", GROUPS[g]), l.bastions_dropped as usize);
        // (Each bastion's faces (flank tip to point), flanks, and how far its point stands out from the curtain's
        // corner it replaced: ft.)
        for q in l.bastions.iter().filter(|q| q.len() == 6) {
            let at = format!("L{li} ({:.0}, {:.0})", q[5][0], q[5][1]);
            r.value("bastion face ft", dist(q[1], q[2]).max(dist(q[2], q[3])), at.clone());
            r.value("bastion flank ft", dist(q[0], q[1]).max(dist(q[3], q[4])), at.clone());
            r.value("bastion point out from the curtain ft", dist(q[2], q[5]), at);
        }
        // Listed: settlements, up to 3 a group.
        if li >= ns || listed[g] >= 3 {
            continue;
        }
        listed[g] += 1;
        r.tally("places listed", 1);
        let s = &t0.settlements[li];
        let bb = bbox(&l.walls.iter().flatten().copied().collect::<Vec<_>>());
        let (cx, cy) = (0.5 * (bb[0] + bb[2]), 0.5 * (bb[1] + bb[3]));
        let span = (bb[2] - bb[0]).max(bb[3] - bb[1]).max(100.0);
        let whole = libm::log2(1100.0 / span);
        // (A gate tower of the town's, not a castle's: the first in order.)
        let town_gate = l.gate_towers.iter().find(|p| !l.castles.iter().any(|(c, rad)| dist(*c, **p) <= rad + 1.0));
        let gate = town_gate.or(l.gate_towers.first()).copied().unwrap_or(l.towers.first().copied().unwrap_or([cx, cy]));
        let st = l.wall_style;
        lines.push(format!(
            "    {} L{li} {} wealth {:.2}{}{}: wall fx {:.6} fy {:.6} zoom {whole:.1}; gate tower fx {:.6} fy {:.6} zoom 1.5 ({} towers, {} castle)",
            GROUPS[g],
            s.tier.name(),
            wallstyle::wealth(s),
            if st.rich { " rich" } else { "" },
            if st.poor { " poor" } else { "" },
            cx / mw,
            cy / mh,
            gate[0] / mw,
            gate[1] / mh,
            l.towers.len() + l.gate_towers.len(),
            if l.castles.is_empty() { "no" } else { "a" },
        ));
        if let Some(q) = l.bastions.first() {
            lines.push(format!("      bastion fx {:.6} fy {:.6} zoom 0.5", q[5][0] / mw, q[5][1] / mh));
        }
        if all || listed[g] == 1 {
            to_check.push((li, l.clone()));
        }
        to_walk.push((li, l.clone()));
    }
    // Every gate of the places listed walked through on the battlemap.
    for (li, l) in to_walk {
        gate_walks(ctx, &l, li, r);
        bastion_walks(ctx, &l, li, r);
    }
    // The battlemap round each tower of the places checked.
    for (li, l) in to_check {
        // (Each town gate's way through: open squares from 15 ft out to 15 ft in.)
        let houses = town::gatehouse::houses(&l);
        for g in l.gates.iter().copied() {
            // (A hand gate on a bend has its towers spread along the sides, up to 60 ft out: its
            // way through between them.)
            let mut pair: Vec<P> = l.gate_towers.iter().copied().filter(|t| dist(*t, g) < 62.0).collect();
            pair.sort_by(|a, b| dist(*a, g).total_cmp(&dist(*b, g)));
            pair.truncate(2);
            let g = if pair.len() == 2 { lerp(pair[0], pair[1], 0.5) } else { g };
            // (A bent entrance's way turns: walked through below.)
            if pair.len() < 2 || town::gatehouse::of_tower(&houses, pair[0]).is_some_and(|h| h.0.form == wallstyle::Gatehouse::Bent) {
                continue;
            }
            let u = sub(pair[1], pair[0]);
            let n = mul([-u[1], u[0]], 1.0 / len(u).max(1e-9));
            let w = ctx.window([g[0] - 30.0, g[1] - 30.0, g[0] + 30.0, g[1] + 30.0]);
            let shut = (-3..=3).map(|k| add(g, mul(n, 5.0 * k as f64))).filter(|p| {
                let (i, j) = (((p[0] - w.ox) / SQUARE_FT).floor() as i64, ((p[1] - w.oy) / SQUARE_FT).floor() as i64);
                i >= 0 && j >= 0 && (i as usize) < w.nx && (j as usize) < w.ny && w.block[j as usize * w.nx + i as usize]
            }).count();
            if shut > 0 {
                r.add("gate passages blocked on the battlemap", shut as f64, format!("L{li} gate {} ({shut} squares)", at(g)));
            }
        }
        for (k, &(t, rad, gate)) in interior::towers(&l).iter().enumerate() {
            let (shape, ras) = wallstyle::tower_raster(&l, t, rad, gate, SQUARE_FT);
            let x0 = ras.i0 as f64 * SQUARE_FT - 40.0;
            let y0 = ras.j0 as f64 * SQUARE_FT - 40.0;
            let w = ctx.window([x0, y0, x0 + ras.nx as f64 * SQUARE_FT + 80.0, y0 + ras.ny as f64 * SQUARE_FT + 80.0]);
            r.tally("towers checked on the battlemap", 1);
            let wi = |gi: i64, gj: i64| -> Option<usize> {
                let (i, j) = (gi - (w.ox / SQUARE_FT).round() as i64, gj - (w.oy / SQUARE_FT).round() as i64);
                (i >= 0 && j >= 0 && (i as usize) < w.nx && (j as usize) < w.ny).then(|| j as usize * w.nx + i as usize)
            };
            let mut bad: Vec<String> = Vec::new();
            let mut door: Option<usize> = None;
            let mut floors = 0;
            let mut wet = false;
            for q in 0..ras.nx * ras.ny {
                let v = ras.cells[q];
                if v == 0 {
                    continue;
                }
                let (gi, gj) = (ras.i0 + (q % ras.nx) as i64, ras.j0 + (q / ras.nx) as i64);
                let Some(a) = wi(gi, gj) else { continue };
                let (kx, ky) = (gi.div_euclid(SQ as i64), gj.div_euclid(SQ as i64));
                let c = ctx.chunk(kx as u32, ky as u32);
                let k = (gj - ky * SQ as i64) as usize * SQ + (gi - kx * SQ as i64) as usize;
                let (b, u) = (c.building[k], c.urban[k]);
                // (A building on it, the water, a deck: theirs.)
                // (A way underground at its foot: its.)
                if b != 0 || c.water_level[k] > c.height[k] || matches!(u, worldgen::battlemap::URBAN_DECK | worldgen::battlemap::URBAN_ENTRANCE) {
                    if v == wallstyle::DOOR && b != 0 {
                        bad.push("door under a building".into());
                    }
                    wet |= v == wallstyle::DOOR && c.water_level[k] > c.height[k];
                    continue;
                }
                match v {
                    wallstyle::RING if u != URBAN_TOWER => bad.push(format!("ring square open ({u})")),
                    wallstyle::FLOOR | wallstyle::DOOR if u != URBAN_TOWER_FLOOR && u != URBAN_TOWER => bad.push(format!("floor square {u}")),
                    _ => {}
                }
                if v != wallstyle::RING && u == URBAN_TOWER_FLOOR {
                    floors += 1;
                }
                if v == wallstyle::DOOR {
                    door = Some(a);
                }
            }
            if floors == 0 {
                bad.push("no open floor".into());
            }
            // The door reached from the ground 30 ft out (square to square past what blocks).
            match door {
                None if !ras.cells.contains(&wallstyle::DOOR) => bad.push("no door".into()),
                None => {}
                Some(d) => {
                    let mut seen = vec![false; w.nx * w.ny];
                    let mut q = VecDeque::from([d]);
                    seen[d] = true;
                    let mut out = false;
                    while let Some(a) = q.pop_front() {
                        let p = [w.ox + ((a % w.nx) as f64 + 0.5) * SQUARE_FT, w.oy + ((a / w.nx) as f64 + 0.5) * SQUARE_FT];
                        if dist(p, t) > rad + 30.0 {
                            out = true;
                            break;
                        }
                        let (x, y) = ((a % w.nx) as i64, (a / w.nx) as i64);
                        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            let (nx, ny) = (x + dx, y + dy);
                            if nx < 0 || ny < 0 || nx >= w.nx as i64 || ny >= w.ny as i64 {
                                continue;
                            }
                            let n = ny as usize * w.nx + nx as usize;
                            if !seen[n] && !w.block[n] {
                                seen[n] = true;
                                q.push_back(n);
                            }
                        }
                    }
                    if !out {
                        bad.push("door not reached from outside".into());
                    }
                }
            }
            if wet {
                r.add("towers by the water with their door in it (not counted above)", 1.0, format!("t:{li}:{k} {} {}", SHAPES[shape as usize], at(t)));
                continue;
            }
            if !bad.is_empty() {
                bad.dedup();
                r.add("towers whose battlemap squares don't follow their shape", bad.len() as f64, format!("t:{li}:{k} {} {}: {}", SHAPES[shape as usize], at(t), bad.join(", ")));
            }
        }
    }
    for line in lines {
        say!("{line}");
    }
}

/// The town wall's rules (item 104) on one layout: buildings off it, roads and streets through it only
/// at a gate, never crossing itself or standing on the water, its bastions clear.
pub(crate) fn wall_rules(l: &Layout, li: usize, lattice: f64, t0: &T0, r: &mut Report) {
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    let half = 0.5 * l.wall_style.thickness();
    let segs: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let segx = |p: P, q: P, a: P, b: P| {
        let c = |o: P, x: P, y: P| cross(sub(x, o), sub(y, o));
        c(p, q, a) * c(p, q, b) < 0.0 && c(a, b, p) * c(a, b, q) < 0.0
    };
    let poly_seg = |poly: &[P], a: P, b: P| {
        let n = poly.len();
        let d = (0..n).map(|k| seg_dist(poly[k], a, b).min(seg_dist(a, poly[k], poly[(k + 1) % n])).min(seg_dist(b, poly[k], poly[(k + 1) % n]))).fold(f64::MAX, f64::min);
        if contains(poly, a) || (0..n).any(|k| segx(poly[k], poly[(k + 1) % n], a, b)) { 0.0 } else { d }
    };
    let bb = bbox(&l.walls.iter().flatten().copied().collect::<Vec<_>>());
    for b in l.buildings.iter().filter(|b| b.poly.iter().any(|p| p[0] > bb[0] - 30.0 && p[0] < bb[2] + 30.0 && p[1] > bb[1] - 30.0 && p[1] < bb[3] + 30.0)) {
        // (Castles' own curtains have their rule.)
        if l.castles.iter().any(|(c, rad)| dist(*c, centroid(&b.poly)) <= rad + 20.0) {
            continue;
        }
        let d = segs.iter().map(|&(a, q)| poly_seg(&b.poly, a, q)).fold(f64::MAX, f64::min) - half;
        // (The sprawl outside stands 7 ft off the wall's line, as its patches are laid: counted apart.)
        if d < 4.0 && d > -half && matches!(b.ward, Ward::Slum | Ward::Farm) {
            r.add("sprawl outside within 4 ft of a town wall's face (not under it)", 4.0 - d, format!("L{li} b:{li}:{} {:.1} ft", b.id, d));
        } else if d < 4.0 {
            r.add("buildings under or within 4 ft of a town wall", 4.0 - d, format!("L{li} b:{li}:{} {:?} {:?} {:.1} ft at {}", b.id, b.ward, b.structure, d, at(centroid(&b.poly))));
        }
    }
    let gate_near = |p: P| l.gates.iter().chain(&l.gate_towers).any(|g| dist(*g, p) < 32.0) || l.castles.iter().any(|(c, rad)| dist(*c, p) <= rad + 30.0);
    for (pts, what) in l.streets.iter().map(|s| (s, 9)).chain(l.roads.iter().filter(|r| r.1 != 3).map(|r| (&r.0, r.1))) {
        for q in pts.windows(2) {
            for &(a, b) in &segs {
                if segx(q[0], q[1], a, b) {
                    let m = lerp(a, b, 0.5);
                    if !gate_near(q[0]) && !gate_near(q[1]) && !gate_near(m) {
                        r.add("town walls crossing a road or street off a gate", 1.0, format!("L{li} {} at {}", if what == 9 { "street".to_string() } else { format!("road {what}") }, at(q[0])));
                    }
                }
            }
        }
    }
    for (i, &(a, b)) in segs.iter().enumerate() {
        for &(c, d) in &segs[i + 1..] {
            if segx(a, b, c, d) && !l.castles.iter().any(|(o, rad)| dist(*o, a) <= rad + 40.0) {
                r.add("town walls crossing themselves", 1.0, format!("L{li} at {} and {}", at(a), at(c)));
            }
        }
        let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
        let k = (0..n).filter(|&j| wet(lerp(a, b, (j as f64 + 0.5) / n as f64))).count();
        if k > 0 && !l.castles.iter().any(|(c, rad)| dist(*c, a) <= rad + 10.0) {
            r.add("town walls on the water (5-ft steps)", k as f64, format!("L{li} at {}", at(a)));
        }
    }
    for q in &l.bastions {
        let on_b = l.buildings.iter().find(|b| b.poly.iter().any(|p| contains(q, *p)) || q.iter().any(|p| contains(&b.poly, *p)));
        // (A road's line more than 3 ft into it, or its paving more than 6 ft: the town's paved strips
        // run along the wall's line, and those ending at the corner spread their paving into the gorge.)
        let paved = |pts: &[P], w: f64| {
            pts.windows(2).any(|s| {
                let (u, n) = (sub(s[1], s[0]), (dist(s[0], s[1]) / 2.5).ceil().max(1.0) as usize);
                let nrm = mul([-u[1], u[0]], 1.0 / len(u).max(1e-9));
                (0..=n).any(|j| [-0.5 * w + 1.0, 0.0, 0.5 * w - 1.0].iter().any(|o| {
                    let p = add(lerp(s[0], s[1], j as f64 / n as f64), mul(nrm, *o));
                    contains(q, p) && depth(q, p) > if *o == 0.0 { 3.0 } else { 6.0 }
                }))
            })
        };
        let on_r = l.streets.iter().map(|s| (s.as_slice(), 9, 4.0)).chain(l.roads.iter().map(|r| (r.0.as_slice(), r.1, r.2))).find(|s| paved(s.0, s.2));
        let on_w = wet(centroid(q)) || q.iter().any(|p| wet(*p));
        if on_b.is_some() || on_r.is_some() || on_w {
            let what = on_b.map(|b| format!("b:{li}:{} {:?}", b.id, b.ward)).or(on_r.map(|s| format!("road {}", s.1))).unwrap_or("water".into());
            r.add("bastions over a road, a building or the water", 1.0, format!("L{li} at {}: {what}", at(q[2])));
        }
    }
}

pub(crate) const FORMS: [&str; 5] = ["square tower", "twin drums", "arched", "bent", "log"];

/// The gatehouses' rules (item 104) on one layout: their forms counted, none over a road (but its own
/// way through), a building or the water, a castle's gate of its group's form.
pub(crate) fn gate_rules(l: &Layout, li: usize, lattice: f64, t0: &T0, r: &mut Report) {
    use worldgen::town::gatehouse;
    const GROUPS: [&str; 5] = ["North", "West", "South", "Dry", "Steppe"];
    let g = GROUPS[(l.wall_style.group % 5) as usize];
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    let segx = |p: P, q: P, a: P, b: P| {
        let c = |o: P, x: P, y: P| cross(sub(x, o), sub(y, o));
        c(p, q, a) * c(p, q, b) < 0.0 && c(a, b, p) * c(a, b, q) < 0.0
    };
    let cuts = |poly: &[P], a: P, b: P| contains(poly, a) || contains(poly, b) || (0..poly.len()).any(|k| segx(poly[k], poly[(k + 1) % poly.len()], a, b));
    for h in gatehouse::houses(l) {
        r.tally(&format!("{g}: {}gatehouses {}", if h.castle { "castle " } else { "" }, FORMS[h.form as usize]), 1);
        if h.castle && h.form != l.wall_style.castle().gatehouse {
            r.add("castle gates not of their group's form", 1.0, format!("L{li} {} at {}", FORMS[h.form as usize], at(h.a)));
        }
        let solids = h.solids();
        let way = h.passage();
        let near = |p: &P| dist(*p, h.a) < 140.0;
        let on_b = l.buildings.iter().filter(|b| b.poly.iter().any(near)).find(|b| solids.iter().any(|q| b.poly.iter().any(|p| contains(q, *p)) || q.iter().any(|p| contains(&b.poly, *p))));
        // (A road's line more than 3 ft into a tower or the block: not the gate's own way, through it.)
        let into = |p: P| solids.iter().any(|z| contains(z, p) && (0..z.len()).all(|k| seg_dist(p, z[k], z[(k + 1) % z.len()]) > 3.0));
        let on_r = l.streets.iter().map(|s| (s, 9)).chain(l.roads.iter().filter(|r| r.1 != 3).map(|r| (&r.0, r.1))).find(|(pts, _)| {
            !pts.windows(2).any(|q| way.iter().any(|w| cuts(w, q[0], q[1]))) && pts.windows(2).any(|q| {
                let n = (dist(q[0], q[1]) / 2.5).ceil().max(1.0) as usize;
                (near(&q[0]) || near(&q[1])) && solids.iter().any(|z| cuts(z, q[0], q[1])) && (0..=n).any(|j| into(lerp(q[0], q[1], j as f64 / n as f64)))
            })
        });
        let on_w = solids.iter().any(|q| wet(centroid(q)) || q.iter().any(|p| wet(*p)));
        if on_b.is_some() || on_r.is_some() || on_w {
            let hit = on_r.and_then(|(pts, _)| pts.windows(2).flat_map(|q| (0..=8).map(move |j| lerp(q[0], q[1], j as f64 / 8.0))).find(|p| into(*p)));
            let fr = hit.map(|p| { let u = mul(sub(h.b, h.a), 1.0 / h.len()); let d = sub(p, h.a); format!(" s {:.1} c {:.1} len {:.1}", dot(d, u), dot(d, h.n), h.len()) }).unwrap_or_default();
            let what = on_b.map(|b| format!("b:{li}:{} {:?}", b.id, b.ward)).or(on_r.map(|s| format!("road {}{fr}", s.1))).unwrap_or("water".into());
            r.add("gatehouses over a road, a building or the water", 1.0, format!("L{li} {} at {}: {what}", FORMS[h.form as usize], at(h.a)));
        }
    }
}

/// Each gate of `l` walked through on the battlemap: square to square (not corner to corner) from 8 ft
/// out past its outer mouth to 8 ft past its inner one, within 40 ft of it.
pub(crate) fn gate_walks(ctx: &mut Ctx, l: &Layout, li: usize, r: &mut Report) {
    for h in town::gatehouse::houses(l) {
        let (o, i) = h.ends();
        let m = lerp(h.a, h.b, 0.5);
        let w = ctx.window([m[0] - 70.0, m[1] - 70.0, m[0] + 70.0, m[1] + 70.0]);
        let ix = |p: P| {
            let (i, j) = (((p[0] - w.ox) / SQUARE_FT).floor() as i64, ((p[1] - w.oy) / SQUARE_FT).floor() as i64);
            (i >= 0 && j >= 0 && (i as usize) < w.nx && (j as usize) < w.ny).then(|| j as usize * w.nx + i as usize)
        };
        // (From any open square within 5 ft of each end: the ground past a mouth may be built on.)
        let open_near = |p: P| (0..w.nx * w.ny).filter(|&k| !w.block[k]).find(|&k| dist(p, [w.ox + ((k % w.nx) as f64 + 0.5) * SQUARE_FT, w.oy + ((k / w.nx) as f64 + 0.5) * SQUARE_FT]) <= 5.0);
        let (Some(_), Some(_)) = (ix(o), ix(i)) else { continue };
        let (a, b) = (open_near(o), open_near(i));
        let why = if a.is_none() || b.is_none() {
            Some(if a.is_none() { "its way in shut" } else { "its way out shut" })
        } else {
            let (a, b) = (a.unwrap(), b.unwrap());
            let mut seen = vec![false; w.nx * w.ny];
            let mut q = VecDeque::from([a]);
            seen[a] = true;
            while let Some(k) = q.pop_front() {
                let (x, y) = ((k % w.nx) as i64, (k / w.nx) as i64);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w.nx as i64 || ny >= w.ny as i64 {
                        continue;
                    }
                    let n = ny as usize * w.nx + nx as usize;
                    let p = [w.ox + (nx as f64 + 0.5) * SQUARE_FT, w.oy + (ny as f64 + 0.5) * SQUARE_FT];
                    if !seen[n] && !w.block[n] && dist(p, m) <= h.len() + 40.0 {
                        seen[n] = true;
                        q.push_back(n);
                    }
                }
            }
            (!seen[b]).then_some("no way through")
        };
        r.tally("gates walked through on the battlemap", 1);
        if let Some(why) = why {
            r.add("gate passages not walkable end to end on the battlemap", 1.0, format!("L{li} {} at {}: {why}", FORMS[h.form as usize], at(m)));
        }
    }
}

/// Each bastion's top walked on the battlemap: every open square inside its outline reached from the
/// town (from the open back, by the ramp), and the props on it counted.
pub(crate) fn bastion_walks(ctx: &mut Ctx, l: &Layout, li: usize, r: &mut Report) {
    for q in l.bastions.iter().filter(|q| q.len() == 6) {
        let b = bbox(q);
        let w = ctx.window([b[0] - 30.0, b[1] - 30.0, b[2] + 30.0, b[3] + 30.0]);
        let v = q[5];
        let o = sub(q[2], v);
        let o = mul(o, 1.0 / len(o).max(1e-9));
        let centre = |k: usize| [w.ox + ((k % w.nx) as f64 + 0.5) * SQUARE_FT, w.oy + ((k / w.nx) as f64 + 0.5) * SQUARE_FT];
        let from = sub(v, mul(o, 8.0));
        let Some(a) = (0..w.nx * w.ny).filter(|&k| !w.block[k]).min_by(|&x, &y| dist(centre(x), from).total_cmp(&dist(centre(y), from))) else { continue };
        let reach = dist(q[2], v) + 30.0;
        let mut seen = vec![false; w.nx * w.ny];
        let mut todo = VecDeque::from([a]);
        seen[a] = true;
        while let Some(k) = todo.pop_front() {
            let (x, y) = ((k % w.nx) as i64, (k / w.nx) as i64);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w.nx as i64 || ny >= w.ny as i64 {
                    continue;
                }
                let n = ny as usize * w.nx + nx as usize;
                if !seen[n] && !w.block[n] && dist(centre(n), v) <= reach {
                    seen[n] = true;
                    todo.push_back(n);
                }
            }
        }
        let top: Vec<usize> = (0..w.nx * w.ny).filter(|&k| contains(q, centre(k)) && !w.block[k]).collect();
        let shut: Vec<usize> = top.iter().copied().filter(|&k| !seen[k]).collect();
        r.tally("bastion tops walked on the battlemap", 1);
        r.tally("bastion top squares open", top.len());
        if !shut.is_empty() {
            let far = shut.iter().map(|&k| dist(centre(k), q[2])).fold(0.0, f64::max);
            r.add("bastion top squares not reached from the town", shut.len() as f64, format!("L{li} at {}: {} ft from its point at most", at(q[2]), far.round()));
        }
        for (p, name) in &w.objects {
            if contains(q, *p) {
                r.tally(&format!("bastion props: {name}"), 1);
            }
        }
        let laid = town::wallside::bastion_props(q, l.wall_style_at(centroid(q)).thickness());
        for p in &laid {
            r.tally(&format!("bastion props laid out: {:?}", p.kind), 1);
            if !w.objects.iter().any(|(o, _)| dist(*o, p.at) < 0.5) {
                r.add("bastion props laid out but not on the battlemap", 1.0, format!("L{li} {:?} at {}", p.kind, at(p.at)));
            }
        }
    }
}

/// R5 M20: big ports (`town::harbour`): their breakwaters (dry and paved, armoured), the
/// lighthouse on each (its front door reached from the bank over the breakwater, on the
/// battlemap: dry open squares, steps of at most 2.5 ft), their harbour masters; every place's
/// moored boats (on open water, never on a deck or dry ground).
pub(crate) fn ports(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::town::harbour::{self, Afloat};
    r.counts(&[
        "big ports",
        "breakwaters",
        "big ports without a breakwater",
        "big ports on deep water, no breakwater (info)",
        "breakwater squares wet",
        "breakwater squares unpaved",
        "breakwater armour pieces",
        "lighthouses",
        "breakwaters without a lighthouse",
        "lighthouse doors not reached from the bank",
        "harbour masters in big ports",
        "big ports without a harbour master",
        "moored boats",
        "village rowboats",
        "cogs",
        "buoys",
        "boats on the battlemap",
        "boats on a dry or deck square",
        "boats left off the battlemap",
        "quay cranes",
        "quay fronts",
        "timber quay fronts",
        "quay front squares wet",
        "quay front squares unpaved",
        "coastal cities",
        "coastal cities without a quay front",
        "coastal cities short of the shore (info)",
        "quay front ft",
        "quay face samples",
        "quay face to water ft (sum)",
        "quay face to water 0-2 ft",
        "quay face to water 3-5 ft",
        "quay face to water 6-10 ft",
        "quay face to water 11-30 ft",
        "quay face samples over 10 ft from the water",
        "quay face samples with no water within 30 ft",
        "bollards",
        "net racks",
    ]);
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    let hm = town::catalog::index_of("harbor_master").map(|f| f as u16);
    let chunks_round = |ctx: &mut Ctx, b: [f64; 4]| -> Vec<(f64, f64, Rc<Chunk>)> {
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let mut out = Vec::new();
        for ky in k(b[1])..=k(b[3]) {
            for kx in k(b[0])..=k(b[2]) {
                out.push((kx as f64 * size, ky as f64 * size, ctx.chunk(kx, ky)));
            }
        }
        out
    };
    for si in 0..ctx.ex.t0.settlements.len() {
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, si);
        // P2 160: a city's quay fronts, dry and paved on the battlemap.
        let st = &ctx.ex.t0.settlements[si];
        if l.tier >= worldgen::t0::settle::Tier::City && st.coastal {
            r.tally("coastal cities", 1);
            // (Only where its patches meet the water: a quay strip or a pier. Seeds 4's and 5's L5
            // are coastal in T0 but have no patch side on the shore, so nothing to front.)
            let meets = !l.piers.is_empty() || l.plazas.iter().any(|p| p.kind == town::PlazaKind::Quay);
            if l.quays.is_empty() && meets {
                r.add("coastal cities without a quay front", 1.0, format!("L{si} {:?} at ({:.0}, {:.0}) river {}", l.tier, st.x, st.y, st.river));
            } else if l.quays.is_empty() {
                r.add("coastal cities short of the shore (info)", 1.0, format!("L{si} {:?} at ({:.0}, {:.0})", l.tier, st.x, st.y));
            }
        }
        for qf in &l.quays {
            let face = qf.face();
            r.tally("quay front ft", face.windows(2).map(|w| town::geom::dist(w[0], w[1])).sum::<f64>() as usize);
            // (How the face follows the shore: from each point every ~5 ft along it, out square to
            // it, the first wet square on the battlemap within 30 ft.)
            let sq_wet = |ctx: &mut Ctx, p: [f64; 2]| -> bool {
                let k = |v: f64| (v / size).floor().max(0.0) as u32;
                let c = ctx.chunk(k(p[0]), k(p[1]));
                let (i, j) = (((p[0] - k(p[0]) as f64 * size) / SQUARE_FT) as usize, ((p[1] - k(p[1]) as f64 * size) / SQUARE_FT) as usize);
                let q = j.min(SQ - 1) * SQ + i.min(SQ - 1);
                c.water_level[q] >= c.height[q]
            };
            // (Not the last 3 steps at an end, where it eases onto the beach and a slip runs down.)
            let nw = face.len().saturating_sub(1);
            for (wi, w) in face.windows(2).enumerate() {
                let d = town::geom::dist(w[0], w[1]);
                if d < 0.5 || wi < 3 || wi + 3 >= nw {
                    continue;
                }
                let u = [(w[1][0] - w[0][0]) / d, (w[1][1] - w[0][1]) / d];
                let mid = town::geom::lerp(w[0], w[1], 0.5);
                let mut v = [-u[1], u[0]];
                if contains(&qf.pts, [mid[0] + v[0], mid[1] + v[1]]) {
                    v = [u[1], -u[0]];
                }
                // (A pier or breakwater going out from it is no gap.)
                let on_works = |t: f64| {
                    let q = [mid[0] + v[0] * t, mid[1] + v[1] * t];
                    l.piers.iter().any(|pp| town::geom::poly_gap(pp, &[q]) < 3.0) || l.moles.iter().any(|m| town::geom::poly_gap(&m.pts, &[q]) < 3.0)
                };
                if (0..=6).any(|j| on_works(5.0 * j as f64)) {
                    continue;
                }
                let off = (0..=30).map(|t| t as f64).find(|&t| sq_wet(ctx, [mid[0] + v[0] * t, mid[1] + v[1] * t]));
                r.tally("quay face samples", 1);
                match off {
                    Some(t) => {
                        r.tally("quay face to water ft (sum)", t as usize);
                        r.tally(if t <= 2.0 { "quay face to water 0-2 ft" } else if t <= 5.0 { "quay face to water 3-5 ft" } else if t <= 10.0 { "quay face to water 6-10 ft" } else { "quay face to water 11-30 ft" }, 1);
                        if t > 10.0 {
                            r.add("quay face samples over 10 ft from the water", 1.0, format!("L{si} {:.0} ft at {}", t, at(mid)));
                        }
                    }
                    None => r.add("quay face samples with no water within 30 ft", 1.0, format!("L{si} at {}", at(mid))),
                }
            }
            r.tally("quay fronts", 1);
            if std::env::var("QUAY_ENDS").is_ok() {
                say!("    quay end L{si} {} / {}", at(qf.pts[0]), at(qf.pts[qf.pts.len() / 2 - 1]));
            }
            r.tally("timber quay fronts", qf.timber as usize);
            let bb = town::geom::bbox(&qf.pts);
            let (mut wet, mut unpaved, mut n) = (0, 0, 0);
            for (cx, cy, c) in &chunks_round(ctx, [bb[0] - 10.0, bb[1] - 10.0, bb[2] + 10.0, bb[3] + 10.0]) {
                for q in 0..SQ * SQ {
                    let p = [cx + ((q % SQ) as f64 + 0.5) * SQUARE_FT, cy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    if !contains(&qf.pts, p) || town::geom::poly_gap(&qf.pts, &[p]) < 2.5 || c.building[q] != 0 || c.height[q] > qf.level + 0.5 {
                        continue;
                    }
                    n += 1;
                    if c.water_level[q] >= c.height[q] {
                        wet += 1;
                    } else if !matches!(
                        c.urban[q],
                        // (A drain grate's or a way down's mouth, and a wall tower's floor, sit on the
                        // quay's paving: their codes only keep props off.)
                        worldgen::battlemap::URBAN_PLAZA | worldgen::battlemap::URBAN_STREET | worldgen::battlemap::URBAN_DECK | worldgen::battlemap::URBAN_ENTRANCE | worldgen::battlemap::URBAN_TOWER_FLOOR
                    ) {
                        unpaved += 1;
                    }
                }
            }
            let qtag = format!("L{si} {:?} quay front at {} level {:.1}", l.tier, at(qf.pts[0]), qf.level);
            if wet > 0 {
                r.add("quay front squares wet", wet as f64, format!("{qtag}: {wet} of {n}"));
            }
            if unpaved > 0 {
                r.add("quay front squares unpaved", unpaved as f64, format!("{qtag}: {unpaved} of {n}"));
            }
        }
        if l.piers.is_empty() {
            continue;
        }
        // Boats: their squares on the battlemap.
        let tag0 = format!("L{si} {:?}", l.tier);
        r.tally("moored boats", l.moored.iter().filter(|m| matches!(m.kind, Afloat::Rowboat | Afloat::FishingBoat | Afloat::Cog)).count());
        r.tally("cogs", l.moored.iter().filter(|m| m.kind == Afloat::Cog).count());
        r.tally("buoys", l.moored.iter().filter(|m| m.kind == Afloat::Buoy).count());
        if l.tier == worldgen::t0::settle::Tier::Village {
            for m in l.moored.iter().filter(|m| m.kind == Afloat::Rowboat) {
                r.add("village rowboats", 1.0, format!("{tag0} at {}", at(m.at)));
            }
        }
        for m in &l.moored {
            let c = ctx.chunk((m.at[0] / size) as u32, (m.at[1] / size) as u32);
            let q = (((m.at[1] / size).fract() * SQ as f64) as usize) * SQ + ((m.at[0] / size).fract() * SQ as f64) as usize;
            if c.water_level[q] <= c.height[q] || c.urban[q] == worldgen::battlemap::URBAN_DECK {
                r.add("boats on a dry or deck square", 1.0, format!("{tag0} {:?} at {}", m.kind, at(m.at)));
            }
            if m.kind == Afloat::Cog {
                continue;
            }
            let kind = match m.kind {
                Afloat::Rowboat => Kind::Rowboat,
                Afloat::FishingBoat => Kind::FishingBoat,
                _ => Kind::Buoy,
            };
            let (ox, oy) = ((m.at[0] / size).floor() * size, (m.at[1] / size).floor() * size);
            let found = c.objects.iter().any(|o| o.kind == kind && dist([ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT], m.at) < 1.0);
            if found {
                r.tally("boats on the battlemap", 1);
            } else {
                r.add("boats left off the battlemap", 1.0, format!("{tag0} {:?} at {}", m.kind, at(m.at)));
            }
        }
        let Some(why) = l.port else { continue };
        r.tally("big ports", 1);
        let tag = format!("L{si} {:?} {} piers at {}", l.tier, l.piers.len(), at(l.center));
        if l.buildings.iter().any(|b| b.func.is_some() && b.func == hm) {
            r.tally("harbour masters in big ports", 1);
        } else {
            r.add("big ports without a harbour master", 1.0, tag.clone());
        }
        // (Water too deep for one is the coast's, no miss: seed 5's L18 lies on ~190 ft of water.)
        if l.moles.is_empty() && why == harbour::TOO_DEEP {
            r.add("big ports on deep water, no breakwater (info)", 1.0, tag.clone());
            continue;
        }
        if l.moles.is_empty() {
            r.add("big ports without a breakwater", 1.0, format!("{tag}: {why}"));
            say!("  big port {tag}: no breakwater ({why})");
            continue;
        }
        for m in &l.moles {
            r.tally("breakwaters", 1);
            let length: f64 = m.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
            let polys = m.polys();
            let bb = m.bbox();
            let chunks = chunks_round(ctx, [bb[0] - 60.0, bb[1] - 60.0, bb[2] + 60.0, bb[3] + 60.0]);
            let (mut wet, mut unpaved, mut n) = (0, 0, 0);
            let mut armour = 0;
            for (cx, cy, c) in &chunks {
                for q in 0..SQ * SQ {
                    let p = [cx + ((q % SQ) as f64 + 0.5) * SQUARE_FT, cy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    // (Squares well inside its made ground, under no building.)
                    let inside = polys.iter().any(|pl| contains(pl, p) && town::geom::poly_gap(pl, &[p]) >= 2.5);
                    // (Its root on a bank above its level is the bank's.)
                    if !inside || c.building[q] != 0 || c.height[q] > m.level + 0.5 {
                        continue;
                    }
                    n += 1;
                    if c.water_level[q] >= c.height[q] {
                        wet += 1;
                    // (A town street run onto its root is paved too.)
                    } else if !matches!(c.urban[q], worldgen::battlemap::URBAN_PLAZA | worldgen::battlemap::URBAN_STREET) {
                        unpaved += 1;
                    }
                }
                armour += c.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Breakwater && s.pts.iter().any(|q| polys.iter().any(|pl| town::geom::poly_gap(pl, &[[cx + q[0] as f64 * SQUARE_FT, cy + q[1] as f64 * SQUARE_FT]]) < 2.0))).count();
            }
            let mtag = format!("{tag}: breakwater {length:.0} ft, level {:.1}, at {}", m.level, at(*m.pts.last().unwrap()));
            if wet > 0 {
                r.add("breakwater squares wet", wet as f64, format!("{mtag}: {wet} of {n}"));
            }
            if unpaved > 0 {
                r.add("breakwater squares unpaved", unpaved as f64, format!("{mtag}: {unpaved} of {n}"));
            }
            r.tally("breakwater armour pieces", armour);
            // Its lighthouse and the way to its door.
            let tip = *m.pts.last().unwrap();
            let Some(lh) = l.buildings.iter().find(|b| harbour::is_lighthouse(b) && dist(mean(&b.poly), tip) < 2.0) else {
                r.add("breakwaters without a lighthouse", 1.0, mtag.clone());
                continue;
            };
            r.tally("lighthouses", 1);
            let reached = interior::generate(&ctx.ex.world, &ctx.ex.t0, si, lh.id as usize).and_then(|it| {
                let lv = it.levels.get(it.entry_level)?;
                let d = lv.doors.iter().find(|d| d.kind == "front")?;
                let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
                let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
                let out = sq.into_iter().find(|&(i, j)| i < 0 || j < 0 || i >= it.nx as isize || j >= it.ny as isize || lv.cells[j as usize * it.nx + i as usize] < 0)?;
                Some(interior::to_world(&it, out.0 as f64 + 0.5, out.1 as f64 + 0.5))
            });
            let ok = reached.is_some_and(|door| {
                let win = ctx.window([bb[0] - 60.0, bb[1] - 60.0, bb[2] + 60.0, bb[3] + 60.0]);
                let (nx, ny) = (win.nx, win.ny);
                let mut h = vec![f32::NAN; nx * ny];
                for (cx, cy, c) in &chunks_round(ctx, [bb[0] - 60.0, bb[1] - 60.0, bb[2] + 60.0, bb[3] + 60.0]) {
                    let (ci, cj) = (((cx - win.ox) / SQUARE_FT).round() as usize, ((cy - win.oy) / SQUARE_FT).round() as usize);
                    for q in 0..SQ * SQ {
                        let a = (cj + q / SQ) * nx + ci + q % SQ;
                        if a < nx * ny && c.height[q] > c.water_level[q] && !win.block[a] {
                            h[a] = c.height[q];
                        }
                    }
                }
                let idx = |p: P| {
                    let (i, j) = (((p[0] - win.ox) / SQUARE_FT).floor(), ((p[1] - win.oy) / SQUARE_FT).floor());
                    (i >= 0.0 && j >= 0.0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
                };
                // (The door's outside square in the interior's grid, which need not line up with the
                // battlemap's: an open battlemap square at it or touching it.)
                let near = [[0.0, 0.0], [-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]].map(|d: [f64; 2]| idx(add(door, mul(d, 0.25 * SQUARE_FT))));
                let Some(start) = near.into_iter().flatten().find(|&a| !h[a].is_nan()) else { return false };
                let mut seen = vec![false; nx * ny];
                let mut queue = VecDeque::from([start]);
                seen[start] = true;
                while let Some(a) = queue.pop_front() {
                    let p = [win.ox + ((a % nx) as f64 + 0.5) * SQUARE_FT, win.oy + ((a / nx) as f64 + 0.5) * SQUARE_FT];
                    if polys.iter().all(|pl| town::geom::poly_gap(pl, &[p]) > 15.0) {
                        return true;
                    }
                    let (i, j) = (a % nx, a / nx);
                    for (di, dj) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                        let (ni, nj) = (i as i64 + di, j as i64 + dj);
                        if ni < 0 || nj < 0 || ni >= nx as i64 || nj >= ny as i64 {
                            continue;
                        }
                        let b = nj as usize * nx + ni as usize;
                        if !seen[b] && !h[b].is_nan() && (h[b] - h[a]).abs() <= 2.5 {
                            seen[b] = true;
                            queue.push_back(b);
                        }
                    }
                }
                false
            });
            if !ok {
                r.add("lighthouse doors not reached from the bank", 1.0, format!("{mtag} b:{si}:{}{}", lh.id, if reached.is_none() { " (no front door)" } else { "" }));
            }
            say!("  {mtag}: {n} squares, {wet} wet, {unpaved} unpaved, {armour} armour pieces, lighthouse b:{si}:{} {} storeys{}", lh.id, lh.floors, if ok { "" } else { ", door not reached" });
        }
    }
    // Cranes, bollards and net racks set out on the battlemaps looked at.
    for c in ctx.chunks.values() {
        r.tally("quay cranes", c.objects.iter().filter(|o| o.kind == Kind::DockCrane).count());
        r.tally("bollards", c.objects.iter().filter(|o| o.kind == Kind::Bollard).count());
        r.tally("net racks", c.objects.iter().filter(|o| o.kind == Kind::NetRack).count());
    }
}

/// R5 M20: the world's islets by its cities (`t0::islets`): how many, their battlemaps dry, piers
/// kept off them, bridges over them flat and dry, watchtowers built on them.
pub(crate) fn harbours(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&[
        "islets",
        "river islets",
        "coast islets",
        "islets with a watchtower",
        "watchtowers built",
        "watchtowers missing",
        "islets under 70% dry",
        "piers within 20 ft of an islet",
        "piers overlapping an islet",
        "bridges over an islet",
        "deck squares over an islet wet",
        "deck squares over an islet stepped (> 1 ft)",
        "metropolises",
        "lined bridges",
        "bridge houses",
        "bridge houses with a cellar",
        "bridge houses with a level below the deck",
        "bridge houses with the front door off the lane",
        "bridge houses off the deck level (> 0.5 ft)",
        "lined lane squares wet or built on",
        "lined lane steps (> 1 ft)",
    ]);
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    let islets: Vec<_> = worldgen::t0::islets::islets(&ctx.ex.t0).to_vec();
    for isl in &islets {
        let tag = format!("L{} {} r {:.0} at {}", isl.settlement, if isl.river { "river" } else { "coast" }, isl.r, at(isl.at));
        r.tally("islets", 1);
        r.tally(if isl.river { "river islets" } else { "coast islets" }, 1);
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, isl.settlement as usize);
        let tower = l.buildings.iter().find(|b| town::is_watchtower(b) && contains(&isl.pts, centroid(&b.poly)));
        if isl.tower {
            r.tally("islets with a watchtower", 1);
            match tower {
                Some(_) => r.tally("watchtowers built", 1),
                None => r.add("watchtowers missing", 1.0, tag.clone()),
            }
        }
        say!("  islet {tag}{}, level {:.1} over water {:.1}", if tower.is_some() { ", watchtower" } else { "" }, isl.level, isl.water);
        for (i, p) in l.piers.iter().enumerate() {
            let gap = town::geom::poly_gap(p, &isl.pts);
            if gap == 0.0 {
                r.add("piers overlapping an islet", 1.0, format!("{tag} pier {i}"));
            } else if gap < 20.0 {
                r.add("piers within 20 ft of an islet", gap, format!("{tag} pier {i} {gap:.0} ft"));
            }
        }
        // The squares whose centres are inside its ring: dry ones (ground over the water).
        let (mut n, mut dry) = (0usize, 0usize);
        let b = isl.bbox;
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let mut chunks: Vec<(f64, f64, Rc<Chunk>)> = Vec::new();
        for ky in k(b[1] - 30.0)..=k(b[3] + 30.0) {
            for kx in k(b[0] - 30.0)..=k(b[2] + 30.0) {
                chunks.push((kx as f64 * size, ky as f64 * size, ctx.chunk(kx, ky)));
            }
        }
        for (cx, cy, c) in &chunks {
            for q in 0..SQ * SQ {
                let p = [cx + ((q % SQ) as f64 + 0.5) * SQUARE_FT, cy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                if contains(&isl.pts, p) {
                    n += 1;
                    dry += (c.height[q] > c.water_level[q]) as usize;
                }
            }
        }
        let share = dry as f64 / n.max(1) as f64;
        r.value("islet squares dry (share)", share, tag.clone());
        if share < 0.7 {
            r.add("islets under 70% dry", share, format!("{tag}: {dry} of {n}"));
        }
        // Bridges over it: their deck squares over the islet's ring and 10 ft round it.
        for (i, d) in l.bridges.iter().enumerate().filter(|(_, d)| town::geom::poly_gap(d, &isl.pts) < 1.0) {
            r.add("bridges over an islet", 1.0, format!("{tag} bridge {i}"));
            let mut tops: Vec<f32> = Vec::new();
            for (cx, cy, c) in &chunks {
                for q in 0..SQ * SQ {
                    let p = [cx + ((q % SQ) as f64 + 0.5) * SQUARE_FT, cy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    if contains(d, p) && c.urban[q] == worldgen::battlemap::URBAN_DECK && (contains(&isl.pts, p) || (0..isl.pts.len()).any(|e| seg_dist(p, isl.pts[e], isl.pts[(e + 1) % isl.pts.len()]) < 10.0)) {
                        if c.water_level[q] >= c.height[q] {
                            r.add("deck squares over an islet wet", 1.0, format!("{tag} bridge {i} at {}", at(p)));
                        }
                        tops.push(c.height[q]);
                    }
                }
            }
            let (lo, hi) = tops.iter().fold((f32::MAX, f32::MIN), |(a, b), &t| (a.min(t), b.max(t)));
            if !tops.is_empty() && hi - lo > 1.0 {
                r.add("deck squares over an islet stepped (> 1 ft)", (hi - lo) as f64, format!("{tag} bridge {i}: {lo:.1} to {hi:.1} ft"));
            }
        }
    }
    // R5 M20: lined bridges. Every house's pad against the deck squares just outside its walls
    // (on the span, not under a building), and the lane down the middle walked end to end.
    for si in 0..ctx.ex.t0.settlements.len() {
        if ctx.ex.t0.settlements[si].tier != worldgen::t0::settle::Tier::Metropolis {
            continue;
        }
        r.tally("metropolises", 1);
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, si);
        // (Its longest bridge, landings and all: over 148 ft, it may span 120 ft of water.)
        let longest = l.bridges.iter().map(|d| dist(d[0], d[1]).max(dist(d[1], d[2]))).fold(0.0, f64::max);
        say!("  metropolis L{si}: longest bridge {longest:.0} ft{}", if l.lined.is_some() { ", lined" } else { "" });
        let Some((deck, level)) = l.lined.clone() else { continue };
        let houses: Vec<&town::Building> = l.buildings.iter().filter(|b| b.deck.is_some()).collect();
        let (a, b) = (lerp(deck[0], deck[3], 0.5), lerp(deck[1], deck[2], 0.5));
        let tag = format!("L{si} span {:.0} ft at {}", dist(a, b), at(lerp(a, b, 0.5)));
        r.add("lined bridges", houses.len() as f64, format!("{tag}: {} houses, level {level:.1}", houses.len()));
        r.tally("bridge houses", houses.len());
        let bb = bbox(&deck);
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let mut chunks: Vec<(f64, f64, Rc<Chunk>)> = Vec::new();
        for ky in k(bb[1] - 10.0)..=k(bb[3] + 10.0) {
            for kx in k(bb[0] - 10.0)..=k(bb[2] + 10.0) {
                chunks.push((kx as f64 * size, ky as f64 * size, ctx.chunk(kx, ky)));
            }
        }
        let squares = |f: &mut dyn FnMut(P, &Chunk, usize)| {
            for (cx, cy, c) in &chunks {
                for q in 0..SQ * SQ {
                    let p = [cx + ((q % SQ) as f64 + 0.5) * SQUARE_FT, cy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    if contains(&deck, p) {
                        f(p, c, q);
                    }
                }
            }
        };
        let mut worst = 0.0f64;
        for h in &houses {
            if interior::has_cellar(h) {
                r.add("bridge houses with a cellar", 1.0, format!("{tag} b:{si}:{}", h.id));
            }
            // (Its interior: every level at the deck or above, the front door on the lane's side.)
            if let Some(it) = interior::generate(&ctx.ex.world, &ctx.ex.t0, si, h.id as usize) {
                if it.levels.iter().any(|v| v.z < 0) {
                    r.add("bridge houses with a level below the deck", 1.0, format!("{tag} b:{si}:{}", h.id));
                }
                let door = it.levels.get(it.entry_level).and_then(|v| v.doors.iter().find(|d| d.kind == "front"));
                let off = door.map_or(f64::MAX, |d| {
                    let m = [0.5 * (d.a[0] + d.b[0]) as f64, 0.5 * (d.a[1] + d.b[1]) as f64];
                    let w = add(it.origin, add(mul(it.axis, m[0] * SQUARE_FT), mul(it.across, m[1] * SQUARE_FT)));
                    seg_dist(w, a, b)
                });
                if off > 15.0 {
                    r.add("bridge houses with the front door off the lane", off, format!("{tag} b:{si}:{} {off:.0} ft from the lane's middle", h.id));
                }
            }
            let mut gap = 0.0f64;
            squares(&mut |p, c, q| {
                let near = (0..h.poly.len()).any(|e| seg_dist(p, h.poly[e], h.poly[(e + 1) % h.poly.len()]) < 4.0) && !contains(&h.poly, p);
                if near && c.building[q] == 0 {
                    gap = gap.max(((c.height[q] - h.pad_ft) as f64).abs());
                }
            });
            worst = worst.max(gap);
            if gap > 0.5 {
                r.add("bridge houses off the deck level (> 0.5 ft)", gap, format!("{tag} b:{si}:{} {gap:.2} ft", h.id));
            }
        }
        r.measure("bridge house |pad - deck| (worst)", false, worst, tag.clone());
        // The lane: squares within 4 ft of the centre line.
        let mut lane: Vec<(f64, f32)> = Vec::new();
        squares(&mut |p, c, q| {
            if seg_dist(p, a, b) < 4.0 {
                if c.water_level[q] >= c.height[q] || c.building[q] != 0 {
                    r.add("lined lane squares wet or built on", 1.0, format!("{tag} at {}", at(p)));
                }
                lane.push((dot(sub(p, a), sub(b, a)) / dist(a, b), c.height[q]));
            }
        });
        lane.sort_by(|x, y| x.0.total_cmp(&y.0));
        let step = lane.windows(2).map(|w| (w[1].1 - w[0].1).abs()).fold(0.0f32, f32::max);
        if step > 1.0 {
            r.add("lined lane steps (> 1 ft)", step as f64, format!("{tag}: {step:.1} ft"));
        }
        say!("  lined bridge {tag}: {} houses, level {level:.1}, worst |pad - deck| {worst:.2}, lane step {step:.2}", houses.len());
    }
    ports(ctx, r);
}

/// `slums`: the out-of-town slum patches as generated (not set by hand), and those of them with no
/// building standing in them (a building's middle in the patch); `SLUMS_LIST=1` lists every patch.
pub(crate) fn slums(world: &worldgen::World, t0: &worldgen::t0::T0, label: &str, r: &mut Report) {
    r.counts(&["outer slum patches", "their buildings", "outer slum patches with no building"]);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town {
            continue;
        }
        let Ok(plan) = town::wards::plan_json(world, t0, li) else { continue };
        let pt = |v: &serde_json::Value| -> P { [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)] };
        let corners: HashMap<u64, P> = plan["corners"].as_array().into_iter().flatten().filter_map(|c| Some((c["corner"].as_u64()?, pt(&c["at"])))).collect();
        for p in plan["patches"].as_array().into_iter().flatten().filter(|p| p["in_town"] == false && p["ward"] == "slum" && p.get("ward_generated").is_none()) {
            let ring: Vec<P> = p["corners"].as_array().into_iter().flatten().filter_map(|c| corners.get(&c.as_u64()?).copied()).collect();
            let ring: Vec<P> = if area(&ring) < 0.0 { ring.into_iter().rev().collect() } else { ring };
            let n = l.buildings.iter().filter(|h| contains(&ring, centroid(&h.poly))).count();
            r.tally("outer slum patches", 1);
            r.tally("their buildings", n);
            if std::env::var("SLUMS_LIST").is_ok() {
                println!("slum {label} L{li} patch {} at {} local {} area {:.0} buildings {n}", p["patch"], at(pt(&p["at"])), at(sub(pt(&p["at"]), l.center)), area(&ring).abs());
            }
            if n == 0 {
                r.add("outer slum patches with no building", area(&ring).abs(), format!("{label} L{li} patch {} at {}", p["patch"], at(pt(&p["at"]))));
            }
        }
    }
}
