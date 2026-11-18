//! `build_town`: a town plan turned into wards, streets, walls and buildings.

use super::*;

/// Build a planned town with the ward editor's changes (`wards::Changes`; none: as planned):
/// its mesh corners moved first (`wards::moved_mesh`), then patches given another ward, lot
/// size, merge or roll, the walls on or off. Each building's `id` is left as the patch it
/// stands on (`generate` numbers them, `wards::inherit` matches them).
/// Lays the town out into `l`; the walls of castles set by hand (their places in `l.walls`).
pub(super) fn build_town(site: &Site, s: &Settlement, r: f64, plan: &TownPlan, ch: &wards::Changes, l: &mut Layout, survey: Option<&mut Vec<stairs::Profile>>) -> Vec<usize> {
    let mut hand_walls = Vec::new();
    let mut mesh = wards::moved_mesh(plan, &ch.moves);
    let total = mesh.faces.len();
    let n_inner = plan.n_inner;
    let wet = &plan.wet;
    let (ward, group) = ch.wards(plan);
    let (ward, group) = (&ward, &group);
    let walled = ch.walls.unwrap_or(plan.walled);
    let usable = |i: usize| plan.usable(i);
    l.plaza_patch = plan.plaza.map(|i| i as u32);
    // The streets as they are laid, for the steps on them.
    let mut pieces = stairs::Pieces::default();

    // Bridges: the river vertex back on the bank-to-bank line, and the deck along it.
    let mut bridge_streets: Vec<(Vec<P>, bool)> = Vec::new();
    // (R5 M20: the main streets' bridges with their water's extent, for the lined bridge.)
    let mut over_water: Vec<(f64, P, P, f64, f64)> = Vec::new();
    for &((a, v, b), main) in &plan.bridged {
        mesh.pos[v] = river_crossing(site, mesh.pos[a], mesh.pos[b]);
        bridge_streets.push((vec![mesh.pos[a], mesh.pos[v], mesh.pos[b]], main));
    }
    for (&((a, v, b), _), (pts, main)) in plan.bridged.iter().zip(&bridge_streets) {
        let dir = sub(pts[2], pts[0]);
        let dl = len(dir).max(1e-9);
        let u = mul(dir, 1.0 / dl);
        // Span from bank to bank along the street, plus a landing on each side.
        let (fwd, back) = (site.dry_reach(pts[1], u, 0.0, 1.0, 0.0) + 14.0, site.dry_reach(pts[1], u, 0.0, -1.0, 0.0) + 14.0);
        // A span far wider than the river means the street runs along the water, not over it.
        let hw = site.nearest_river(pts[1]).map_or(0.0, |r| r.2);
        if fwd + back > 4.0 * hw + 80.0 {
            continue;
        }
        // Cities build in stone, as wide as the street's gap between blocks (12 ft a side).
        let width = if s.tier >= Tier::City { 24.0 } else if *main { 16.0 } else { 12.0 };
        // Long enough that every corner stands on the bank, crossing at a slant too.
        let (fwd, back) = (site.dry_reach(pts[1], u, width, 1.0, fwd - 14.0) + 14.0, site.dry_reach(pts[1], u, width, -1.0, back - 14.0) + 14.0);
        let c = add(pts[1], mul(u, 0.5 * (fwd - back)));
        l.bridges.push(rect(c, u, fwd + back, width));
        if *main {
            let (wf, wb) = (site.dry_reach(pts[1], u, 0.0, 1.0, 0.0), site.dry_reach(pts[1], u, 0.0, -1.0, 0.0));
            over_water.push((wf + wb, pts[1], u, wf, wb));
        }
        pieces.bridge((a, v, b), pts, if *main { 18.0 } else { 12.0 }, (!main).then_some(l.roads.len()));
        if !main {
            l.roads.push((pts.clone(), 4, 12.0));
        }
    }

    lined_bridge(site, s, &over_water, l);

    // Materialise: patches, walls, gates and streets as points.
    let cells: Vec<Labeled> = (0..total).map(|i| Labeled { pts: mesh.face_pts(i), labels: mesh.labels[i].clone() }).collect();
    let edge_v = |i: usize, e: usize| (mesh.faces[i][e], mesh.faces[i][(e + 1) % mesh.faces[i].len()]);
    // (R5 towns, M18b: less the gates taken out by hand, and those put in: `wallgates`.)
    let (gates, gate_ways) = wallgates::settle(site, plan, &mesh, ward, &ch.wall, &mut l.hand_wall);
    l.gates = gates.iter().map(|&g| mesh.pos[g]).collect();
    l.gate_spots = if walled { gates.iter().map(|&g| g as u32).collect() } else { Vec::new() };
    // (Its outline by its style, item 104: `outline::lay`.)
    let mut wall_laid = outline::lay(site, plan, &mesh, ward, &l.wall_style, &gates, &ch.wall);
    // A round wall's curtains bowed out, before the outskirts step back from them (`curtain`).
    let wall_bows = if walled || plan.walled {
        // (A hand gate's track is laid after: the curtain keeps well off it, ~30 ft face to edge.)
        let with_ways: Vec<(Vec<P>, u8, f64)>;
        let roads = if gate_ways.is_empty() {
            &l.roads
        } else {
            with_ways = l.roads.iter().cloned().chain(gate_ways.iter().map(|(_, pts)| (pts.clone(), 2, wallgates::WAY_WIDTH_FT + 52.0))).collect();
            &with_ways
        };
        curtain::bow(site, plan, &mesh, ward, &l.wall_style, roads, &l.bridges, &mut wall_laid.0, &mut wall_laid.1, &wall_laid.2, (&gates, &ch.wall))
    } else {
        Default::default()
    };
    // (The wall's line every 4 ft, which the outskirts' blocks keep off: `insets`.)
    let wall_dense: Vec<P> = if walled || plan.walled {
        wall_laid
            .0
            .iter()
            .flat_map(|w| w.windows(2))
            .flat_map(|s| {
                let n = crate::core::ceil(dist(s[0], s[1]) / 4.0).max(1.0) as usize;
                (0..=n).map(move |j| lerp(s[0], s[1], j as f64 / n as f64))
            })
            .collect()
    } else {
        Vec::new()
    };
    // (A hand tower's tag where its curtain carried it: the towers laid are those along the
    // straight runs, one for one.)
    if !ch.wall.towers.is_empty() {
        let (_, raw, _) = wallgates::pieces(plan, &mesh, &gates, &ch.wall);
        if raw.len() == wall_laid.1.len() {
            for t in &mut l.hand_wall.tags {
                if let Some(i) = (0..raw.len()).find(|&i| geom::dist(raw[i], t.0) < 0.5) {
                    t.0 = wall_laid.1[i];
                }
            }
        }
    }
    outline::tidy(&mut wall_laid.0, &mut wall_laid.1);
    if walled {
        l.walls.extend(wall_laid.0.iter().cloned());
        l.towers.extend(wall_laid.1.iter().copied());
        l.gate_towers.extend(wall_laid.2.iter().copied());
        (l.bastions, l.bastions_dropped) = (wall_laid.3.placed.clone(), wall_laid.3.dropped);
    }
    // Each tower's shape by the town's walls (item 104): a D faces out from the middle.
    let (style, laid) = (l.wall_style, l.towers.clone());
    wallstyle::shape_towers(l, &style, &laid, [0.0, 0.0], false);
    // The town wall as it is laid along its runs (whether it stands or has been taken down), less
    // the stretches `walls_off_river` cuts at the river: a castle's side along it is that wall
    // (`castle`, `castle_wall_edge`); and its towers, which a castle's gate and way keep clear of.
    let mut town_segs: Vec<(P, P)> = Vec::new();
    let mut town_towers: Vec<(P, f64)> = Vec::new();
    if walled || plan.walled {
        {
            let (pieces, towers, gate_towers) = (&wall_laid.0, &wall_laid.1, &wall_laid.2);
            // (A square tower reaches further across its corners.)
            let reach = if l.wall_style.has_square() { 15.6 } else { 11.0 };
            town_towers.extend(towers.iter().map(|t| (*t, reach)).chain(gate_towers.iter().map(|t| (*t, 15.0))));
            for w in pieces {
                for s in w.windows(2) {
                    let n = crate::core::ceil(dist(s[0], s[1]) / 5.0).max(1.0) as usize;
                    let mut from: Option<P> = None;
                    for j in 0..n {
                        let (a, b) = (lerp(s[0], s[1], j as f64 / n as f64), lerp(s[0], s[1], (j + 1) as f64 / n as f64));
                        let dry = !site.near_river(lerp(a, b, 0.5), 3.0);
                        match (dry, from) {
                            (true, None) => from = Some(a),
                            (false, Some(f)) => {
                                town_segs.push((f, a));
                                from = None;
                            }
                            _ => {}
                        }
                    }
                    if let Some(f) = from {
                        town_segs.push((f, s[1]));
                    }
                }
            }
        }
    }
    let gate_pts: Vec<P> = gates.iter().map(|&g| mesh.pos[g]).collect();
    // The main streets, the roads in to the gates and a hand gate's way out (`approach`); and the
    // stretches of road and street a gate taken out cut off.
    let freed = approach::lay(site, s, r, plan, ch, &mesh, walled, &wall_laid.0, gate_ways, &mut pieces, l);

    // Quays and landings along the shore, and the docks' piers (`quays`).
    quays::lay(site, plan, n_inner, &cells, ward, &usable, &edge_v, l);

    // A side of a castle's patch along which the town wall is the castle's own (`castle`): the wall
    // laid along the whole of it, no main street running between it and the castle and no town gate
    // on it but at its ends (the castle's corner there is cut back from the gate: `castle`); those
    // stay the town's, outside the castle: its curtain stands back as on any other side.
    // (The streets and roads laid so far: main streets, approaches, streets bent to a bridge.)
    let street_segs: Vec<(P, P)> = l.roads.iter().filter(|r| r.1 != 3).flat_map(|r| r.0.windows(2).map(|q| (q[0], q[1])).collect::<Vec<_>>()).collect();
    let castle_wall_edge = |i: usize, e: usize| -> bool {
        let c = &cells[i];
        let m = c.pts.len();
        let (va, vb) = edge_v(i, e);
        let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
        let el = dist(a, b);
        if el < 1.0 || gate_pts.iter().any(|g| seg_dist(*g, a, b) < 60.0 && dist(*g, a).min(dist(*g, b)) > 60.0) {
            return false;
        }
        let u = mul(sub(b, a), 1.0 / el);
        let n = crate::core::ceil(el / 5.0) as usize;
        // (A main street along it runs between where its paved strip does, inside the wall; where
        // a stream cuts the strip away it lies on the wall's own line, and the strip is grass.)
        if plan.main_edge(va, vb) {
            let inward = if area(&c.pts) > 0.0 { [-u[1], u[0]] } else { [u[1], -u[0]] };
            let o = mul(inward, 15.0);
            if !(0..=n).any(|j| site.near_river(lerp(add(a, o), add(b, o), j as f64 / n as f64), 15.0)) {
                return false;
            }
        }
        // (Through a gate at its end the wall has its passage; where a stream crosses it, its
        // water gap, but where a street crosses the stream there into the patch.)
        let bridged = street_segs.iter().any(|&(q, r)| {
            let (o1, o2) = (cross(sub(b, a), sub(q, a)), cross(sub(b, a), sub(r, a)));
            let (o3, o4) = (cross(sub(r, q), sub(a, q)), cross(sub(r, q), sub(b, q)));
            o1 * o2 < 0.0 && o3 * o4 < 0.0
        });
        (0..=n).all(|j| {
            let p = lerp(a, b, j as f64 / n as f64);
            gate_pts.iter().any(|g| dist(*g, p) < 45.0)
                || (!bridged && site.near_river(p, 6.0))
                || town_segs.iter().any(|&(q, r)| {
                let ql = dist(q, r);
                ql > 0.5 && cross(u, mul(sub(r, q), 1.0 / ql)).abs() < 0.12 && seg_dist(p, q, r) <= 3.0
            })
        })
    };
    // How far each side of a patch its block stands back (streets, quays, the wall's strip).
    let insets = |i: usize, w: Ward| -> Vec<f64> {
        let c = &cells[i];
        let m = c.pts.len();
        // (A castle's sides on the town wall: and how far each would stand back off it.)
        let mut on_wall = vec![false; m];
        let mut back = vec![0.0; m];
        let mut d: Vec<f64> = (0..m)
            .map(|e| {
                let (va, vb) = edge_v(i, e);
                let main = plan.main_edge(va, vb);
                let nb = c.labels[e];
                // On the river: blocks end at a quay 16 ft from the water.
                let mid = lerp(c.pts[e], c.pts[(e + 1) % m], 0.5);
                if site.near_river(mid, 2.0) {
                    return site.nearest_river(mid).map_or(20.0, |r| r.2) + 16.0;
                }
                // On the sea or a lake: a 14 ft quay between the block and the water.
                if nb >= 0 && plan.shore.contains(&(va.min(vb), va.max(vb))) {
                    return 14.0;
                }
                if w == Ward::Farm {
                    return 7.0;
                }
                // The market square keeps its full cell; its neighbours step back from it.
                if w == Ward::Plaza {
                    return 0.0;
                }
                if nb >= 0 && Some(nb as usize) == plan.plaza && plan.plaza_face.is_some() {
                    return 13.0_f64.max(if main { 12.0 } else { 7.0 });
                }
                if nb >= 0 && group[i] != usize::MAX && group[nb as usize] == group[i] {
                    return 0.0;
                }
                // (A planned wall taken down leaves its strip as a street round the town.)
                // (Nor round the water the town encloses.)
                let wall = (walled || plan.walled) && i < n_inner && (!plan.inner(nb) || (wet[nb as usize] && !plan.enclosed[nb as usize]));
                back[e] = (if main { 12.0 } else { 7.0 }) + if wall { 18.0 } else { 0.0 };
                // A castle's side on the town wall: that wall is the castle's there (`castle`; with
                // the wall taken down, a curtain of its own on that side).
                if w == Ward::Castle && wall && castle_wall_edge(i, e) {
                    on_wall[e] = true;
                    return 0.0;
                }
                back[e]
            })
            .collect();
        // (Outside the town, a side on its wall steps back from the curtain bowed out over it; and
        // any side the wall's line comes in past, as a curtain bridging a notch bowed out over a
        // neighbour's side: from the deepest of it in the patch, by the side it is nearest.)
        if i >= n_inner {
            for (e, de) in d.iter_mut().enumerate() {
                let (va, vb) = edge_v(i, e);
                if let Some(b) = wall_bows.get(&(va.min(vb), va.max(vb))) {
                    *de = de.max(7.0 + b);
                }
            }
            if !wall_dense.is_empty() {
                let bb = bbox(&c.pts);
                for p in wall_dense.iter().filter(|p| p[0] >= bb[0] && p[0] <= bb[2] && p[1] >= bb[1] && p[1] <= bb[3] && contains(&c.pts, **p)) {
                    // (By distance to each side, not across its line: a patch moved by hand may be concave.)
                    let (e, depth) = (0..m).map(|e| (e, seg_dist(*p, c.pts[e], c.pts[(e + 1) % m]))).min_by(|x, y| x.1.total_cmp(&y.1)).unwrap_or((0, 0.0));
                    d[e] = d[e].max(7.0 + depth);
                }
            }
        }
        // A castle's side on the town wall beside one standing back from nearly the same line: the
        // curtain would leave the wall at a slant, a second wall just inside it. It stands back too.
        while w == Ward::Castle {
            let dir = |e: usize| {
                let v = sub(c.pts[(e + 1) % m], c.pts[e]);
                mul(v, 1.0 / len(v).max(1e-9))
            };
            let slant = (0..m).find(|&e| {
                on_wall[e]
                    && [(e + m - 1) % m, (e + 1) % m].iter().any(|&f| {
                        let (cs, sn) = (dot(dir(e), dir(f)), cross(dir(e), dir(f)).abs());
                        d[f] > 0.0 && cs > 0.0 && d[f] * cs > 25.0 * sn
                    })
            });
            let Some(e) = slant else { break };
            on_wall[e] = false;
            d[e] = back[e];
        }
        d
    };

    // Street lines bounding patch `i`'s block (outward normal, offset; `d`: its insets): lots need
    // frontage on one.
    let street_lines = |i: usize, d: &[f64]| -> Vec<(P, f64)> {
        let c = &cells[i];
        let m = c.pts.len();
        let ccw = area(&c.pts) > 0.0;
        (0..m)
            .filter(|&e| d[e] > 0.0)
            .filter_map(|e| {
                let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
                let ev = sub(b, a);
                let el = len(ev);
                (el > 1e-9).then(|| {
                    let nrm = if ccw { [ev[1] / el, -ev[0] / el] } else { [-ev[1] / el, ev[0] / el] };
                    (nrm, dot(a, nrm) - d[e])
                })
            })
            .collect()
    };

    // Roads into the city and the ground before each gate stay open: the sprawl outside the
    // walls grows up to them, not over them.
    let mut keep_clear: Vec<(P, P, f64)> = l.roads.iter().filter(|r| r.1 != 3).flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * w + 3.0))).collect();
    keep_clear.extend(freed.iter().copied());
    let within = OUTSKIRTS * r * 1.1;
    let c = site.center;
    for (ri, k) in site.t0.roads.segments_near([c[0] - within, c[1] - within, c[0] + within, c[1] + within], 0.0) {
        let rc = &site.t0.roads.roads[ri as usize];
        let k = k as usize;
        let m = ((rc.s[k + 1] - rc.s[k]) / 25.0).ceil().clamp(1.0, 400.0) as usize;
        let pts: Vec<P> = (0..=m).map(|j| sub(rc.eval(k, j as f64 / m as f64, 5.0, site.t0.cell_ft).p, c)).collect();
        for s in pts.windows(2) {
            if len(s[0]) < within {
                keep_clear.push((s[0], s[1], 0.5 * rc.class.width_ft() + 4.0));
            }
        }
    }
    const CLEAR_CELL: f64 = 80.0;
    let mut clear_grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
    for (i, &(a, b, h)) in keep_clear.iter().enumerate() {
        let (x0, x1) = (crate::core::floor((a[0].min(b[0]) - h) / CLEAR_CELL) as i64, crate::core::floor((a[0].max(b[0]) + h) / CLEAR_CELL) as i64);
        let (y0, y1) = (crate::core::floor((a[1].min(b[1]) - h) / CLEAR_CELL) as i64, crate::core::floor((a[1].max(b[1]) + h) / CLEAR_CELL) as i64);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                clear_grid.entry((cx, cy)).or_default().push(i as u32);
            }
        }
    }
    // (And the ground before a gate taken out, as it was with the gate: nothing built nearer.)
    let gates: Vec<P> = l.gates.iter().copied().chain(ch.wall.remove.iter().filter(|_| walled).map(|&v| mesh.pos[v])).collect();
    let clear_of = |b: &Building| {
        if gates.iter().any(|g| b.poly.iter().any(|p| dist(*p, *g) < 40.0) || contains(&b.poly, *g)) {
            return false;
        }
        let (x0, y0, x1, y1) = b.poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        for cy in crate::core::floor(y0 / CLEAR_CELL) as i64..=crate::core::floor(y1 / CLEAR_CELL) as i64 {
            for cx in crate::core::floor(x0 / CLEAR_CELL) as i64..=crate::core::floor(x1 / CLEAR_CELL) as i64 {
                for &i in clear_grid.get(&(cx, cy)).into_iter().flatten() {
                    let (a, bb, h) = keep_clear[i as usize];
                    if poly_seg_dist(&b.poly, a, bb) < h {
                        return false;
                    }
                }
            }
        }
        true
    };

    // Patch `i`'s lots as ward `w` (its district's, or as rolled again or sized by hand): their
    // sizes and the stream that cuts them; a close's houses on lots as a common ward's (a ring a
    // lot deep: a temple's own are too big for one).
    let lots_of = |i: usize, w: Ward, close: bool| {
        let key = if group[i] != usize::MAX { group[i] } else { i };
        let roll = ch.reroll.get(&i).copied().unwrap_or(0);
        let salt = |h: u64| if roll == 0 { h } else { hash3(h, roll as i64, 0x7e70, 1) };
        let mut dp = District::roll(w, salt(hash2(plan.base_seed, key as i64, 0xd15)));
        match ch.lots.get(&i) {
            Some(f) => dp.min_sq = District::base(w) * f,
            None if close => dp.min_sq *= District::base(Ward::Common) / District::base(Ward::Temple),
            None => {}
        }
        (dp, Pcg32::new(salt(hash2(plan.base_seed, i as i64, 0x107)), 47))
    };

    // The other open spaces (`squares`): squares where streets meet (the lots at their corners
    // trimmed back once laid out, so nothing else moves), temples' closes where they fit: their
    // temple and churchyard, with houses round them (the block laid out aside to see) most of the
    // way round.
    let (li, lc, lr, lt) = (l.index, l.center, l.radius, l.tier);
    let fits = |p: usize| {
        let d = insets(p, Ward::Temple);
        let b = inset(&cells[p].pts, &d);
        if b.len() < 3 || site.grade(&b) > 0.18 {
            return false;
        }
        let fronts = street_lines(p, &d);
        let (dp, mut prng) = lots_of(p, Ward::Temple, true);
        let mut aside = Layout::empty(li, lc, lr, lt);
        build_block(site, Block::Plain(&b), &Front::of(&b, &fronts), Ward::Temple, &dp, &mut prng, &mut aside, None);
        if !squares::close(site, &b, &fronts, plan.roll(p, 65), 0, 0, &mut aside) {
            return false;
        }
        aside.buildings.retain(|b| clear_of(b));
        aside.plazas.last().is_some_and(|o| squares::ringed(&b, &fronts, &o.pts, &aside.buildings) >= 0.65)
    };
    let room: Vec<bool> = (0..cells.len()).map(|p| ward.get(p) == Some(&Some(Ward::Temple)) && fits(p)).collect();
    let (spots, closes) = squares::spots(plan, ch, ward, group, &|p| room.get(p).is_some_and(|&r| r));
    let cuts = squares::cuts(plan, &mesh, &spots);
    l.close_room = (0..cells.len()).filter(|&p| room[p]).map(|p| p as u32).collect();

    // Blocks → alleys → lots → buildings.
    // Castles' ways from their gates to the streets (nothing built on them by a later patch).
    let mut castle_ways: Vec<Vec<P>> = Vec::new();
    // Hand castles on patches too small for a curtain: (patch, its outline), laid as tower houses.
    let mut tower_houses: Vec<(usize, Vec<P>)> = Vec::new();
    // v63 countryside: the farmland belt's plain radius each way (`plan_town`'s `farmed`), which
    // its fields fray past or fall short of (`fields::reach`), and the roads they run on along.
    let (a1, a2, f1, f2) = (plan.roll(0, 30) * 0.15 + 0.1, plan.roll(0, 31) * 0.1 + 0.05, plan.roll(0, 32) * std::f64::consts::TAU, plan.roll(0, 33) * std::f64::consts::TAU);
    let belt = |p: P| {
        let th = libm::atan2(p[1], p[0]);
        (1.0 + a1 * libm::sin(th + f1) + a2 * libm::sin(2.0 * th + f2)) * OUTSKIRTS * r * 0.8
    };
    let far_roads: Vec<(Vec<P>, u8, f64)> = l.roads.iter().filter(|x| x.1 < 3).cloned().collect();
    // Farmland plots (`farm`): those of the planned farm patches, then those of the open ground past them.
    let mut plots: Vec<(Vec<P>, u8)> = Vec::new();
    // R5 towns: the cemeteries the town wants, and the common that is one (`cemetery`).
    let (cem_want, cem_inner) = cemetery::inner_pick(s, plan, ward, ch);
    let mut cem_laid = 0;
    // (A conversion to a hand town reads each patch as laid out: `hand::convert`. Nothing here
    // changes what is built.)
    let capturing = hand::capturing(l.index);
    if capturing {
        let d: Vec<Vec<f64>> = (0..total).map(|i| insets(i, ward[i].unwrap_or(Ward::Farm))).collect();
        hand::cap_town(&mesh.faces, &cells, d, ward, group, walled, &gate_pts);
    }
    for i in 0..total {
        let Some(w) = ward[i] else {
            // Open ground past the farmland: fields where the belt frays out over it.
            let c = &cells[i];
            if i >= plan.n_inner && !plan.wet[i] && c.pts.len() >= 3 && !ch.ward.contains_key(&i) && c.pts.iter().any(|p| len(*p) < 1.9 * belt(*p)) {
                let block = inset(&c.pts, &insets(i, Ward::Farm));
                if block.len() >= 3 {
                    let mut frng = Pcg32::new(hash2(plan.pseed, i as i64, 0xf1e1d), 61);
                    if capturing {
                        let (st, inc) = frng.state();
                        hand::cap_fill(i, &Fill::Farm { class: 2, rng: Pcg32::from_state(st, inc) }, &block, &[], &|_| 0.0);
                    }
                    farm(site, &block, &mut frng, l, &mut plots, 2, false);
                    if capturing {
                        hand::cap_built(i, &[]);
                    }
                }
            }
            continue;
        };
        let c = &cells[i];
        let m = c.pts.len();
        if i < n_inner {
            l.districts.push(c.pts.clone());
        }
        let d = insets(i, w);
        // Paved streets inside the town: each side of a street paves the strip between its
        // patch edge and its block (quays and the waterfront are plazas already).
        // (A park's streets too: it is a block like any other, laid out as a garden.)
        // A side's paved strip (its line, between its corners), if it has one: not on the water's side,
        // nor where the river cuts across the strip. (A park's ways in are on these sides alone.)
        let ccw = area(&c.pts) > 0.0;
        let strip = |e: usize, d: &[f64]| -> Option<(P, P)> {
            let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
            let nb = c.labels[e];
            let el = dist(a, b);
            if d[e] <= 0.0 || el < 1.0 || (nb >= 0 && wet[nb as usize]) || site.near_river(lerp(a, b, 0.5), 2.0) {
                return None;
            }
            let ev = mul(sub(b, a), 1.0 / el);
            let inward = if ccw { [-ev[1], ev[0]] } else { [ev[1], -ev[0]] };
            let o = mul(inward, 0.5 * d[e]);
            let (pp, q) = (add(a, o), add(b, o));
            let n = (el / 5.0).ceil() as usize;
            (!(0..=n).any(|j| site.near_river(lerp(pp, q, j as f64 / n as f64), 0.5 * d[e]))).then_some((pp, q))
        };
        let mut paved = |d: &[f64], l: &mut Layout| {
            for e in 0..m {
                let Some((pp, q)) = strip(e, d) else { continue };
                let (va, vb) = edge_v(i, e);
                pieces.edge(va, vb, pp, q, d[e] + 1.0, false, l.roads.len());
                l.roads.push((vec![pp, q], 3, d[e] + 1.0));
            }
        };
        // (A castle's once its block is known to be there: never a patch of paved strips alone.)
        if i < n_inner && !matches!(w, Ward::Farm | Ward::Plaza | Ward::Castle) {
            paved(&d, l);
        }
        let block = inset(&c.pts, &d);
        if block.len() < 3 {
            continue;
        }
        let roads_at = l.roads.len();
        if i < n_inner && w == Ward::Castle {
            paved(&d, l);
        }
        let streets = street_lines(i, &d);
        // Too steep to build a street front on (and not a common on the slope): the block is a
        // terraced garden of a few houses.
        // (R6 P8) A docks ward on a sloping shore is built down to its quay all the same: lots
        // narrow enough to stand on the slope (about 7 ft of fall across each), each on its own
        // pad a step below the next (`Ground::stepped`), as far as the slope allows.
        let grade = if block.len() >= 3 { site.grade(&block) } else { 0.0 };
        let sparse = matches!(w, Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Temple) && grade > 0.18;
        let w = if sparse { Ward::Park } else { w };
        // (Rolled again by hand: its own streams.)
        let (mut dp, mut prng) = lots_of(i, w, w == Ward::Temple && closes.contains(&i));
        docks_on_slope(&mut dp, w, grade);
        if !matches!(w, Ward::Plaza | Ward::Farm | Ward::Park) {
            l.blocks.push(block.clone());
        }
        let (first, towers_at, gates_at, walls_at) = (l.buildings.len(), l.towers.len(), l.gate_towers.len(), l.walls.len());
        match w {
            // The gate faces the centre (the plaza, if the citadel backs onto it).
            Ward::Castle => {
                // (The town wall as laid near it: the sides along it are that wall, as `insets` sets
                // them.)
                let (x0, y0, x1, y1) = c.pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
                let on_wall: Vec<(P, P)> = town_segs
                    .iter()
                    .copied()
                    .filter(|(a, b)| a[0].max(b[0]) >= x0 - 80.0 && a[0].min(b[0]) <= x1 + 80.0 && a[1].max(b[1]) >= y0 - 80.0 && a[1].min(b[1]) <= y1 + 80.0)
                    .collect();
                // A castle inside the town (no corner on the wall): a full curtain of its own, its gate
                // toward the market square (where it was planned: a corner of the square moved by
                // hand turns no castle across the town).
                let inside = !plan.mesh.faces[i].iter().any(|v| plan.wall_runs.iter().any(|(chain, _, _)| chain.contains(v)));
                let on_wall = if inside { Vec::new() } else { on_wall };
                let toward = match plan.plaza {
                    Some(pz) if inside && pz != i => centroid(&mesh.faces[pz].iter().map(|&v| if ch.moves.iter().any(|m| m.0 == v) { plan.mesh.pos[v] } else { mesh.pos[v] }).collect::<Vec<_>>()),
                    _ => [0.0, 0.0],
                };
                // Its size: a city's about 35,000 sq ft, a metropolis's 50,000, and 50,000 more for an
                // inner bailey (a capital's, 60,000 and broader, and about two in five others'; 280 ft
                // across at least, as an inner curtain needs). The patch is cut to it with
                // one chord, on the low side and away from the town wall (inside the town, towards
                // the market): a lane along the cut, and the rest of the patch a noble ward's lots,
                // unless that would leave out a side on the wall or too little for lots.
                let want_inner = s.capital || plan.roll(i, 73) < CASTLE_INNER;
                let target = if s.tier == Tier::Metropolis { 50_000.0 } else { 35_000.0 } + if s.capital { 60_000.0 } else if want_inner { 50_000.0 } else { 0.0 };
                let wall_sides: Vec<(P, P)> = (0..m).filter(|&e| i < n_inner && !plan.inner(c.labels[e])).map(|e| (c.pts[e], c.pts[(e + 1) % m])).collect();
                let away = match plan.plaza {
                    Some(pz) if inside && pz != i => sub(toward, centroid(&c.pts)),
                    _ if !wall_sides.is_empty() => {
                        let k = 1.0 / wall_sides.len() as f64;
                        sub(centroid(&c.pts), wall_sides.iter().fold([0.0, 0.0], |a, &(p, q)| add(a, mul(lerp(p, q, 0.5), k))))
                    }
                    _ => [0.0, 0.0],
                };
                let away = mul(away, 1.0 / len(away).max(1e-9));
                // (Measured on the patch: its block's about an eighth less, 20 ft narrower.)
                let wide = if s.capital { 320.0 } else if want_inner { 280.0 } else { 120.0 };
                // v63 castles: a citadel's moat by roll, its room taken from the
                // patch's leftover (the castle's part cut the bigger for it).
                let rolled = castle_moat(s, want_inner, l.roof_age, plan.roll(i, 163), plan.roll(i, 164));
                // (A capital's castle keeps its inner curtain first: should the moat's room leave
                // none, the castle is laid again without a moat.)
                let before = (s.capital && rolled.is_some()).then(|| l.clone());
                let mut moat = rolled;
                let (laid, ways_at, blocks_at) = loop {
                    let castles_at = l.castles.len();
                    let (target, wide) = moat.map_or((target, wide), |m| (target + 4.0 * crate::core::sqrt(target) * m.width, wide + 2.0 * m.width));
                    let cut = castle::cut_bound(&block, &c.pts, &wall_sides, away, &|p| site.height(p), 1.12 * target, wide + 20.0);
                    let (blocks_at, cblock) = (l.blocks.len(), cut.as_ref().map_or(block.clone(), |k| k.kept.clone()));
                    // (As it was, should the castle's part hold no keep after all: the castle on the whole.)
                    let uncut = cut.is_ok().then(|| l.clone());
                    if let Ok(k) = &cut {
                        l.roads.push((vec![k.lane.0, k.lane.1], 3, castle::LANE));
                        if let Some(b) = l.blocks.last_mut() {
                            *b = k.kept.clone();
                        }
                        l.blocks.push(k.left.clone());
                        // (Its own sides' streets alone: the same lots whether the town wall stands or not.)
                        let nl = k.left.len();
                        let mut streets: Vec<(P, f64)> = street_lines(i, &d)
                            .into_iter()
                            .filter(|&(n, off)| (0..nl).any(|e| (dot(lerp(k.left[e], k.left[(e + 1) % nl], 0.5), n) - off).abs() < 1.0))
                            .collect();
                        streets.push(k.line);
                        let (dp, mut prng) = lots_of(i, Ward::Noble, false);
                        build_block(site, Block::Plain(&k.left), &Front::of(&k.left, &streets), Ward::Noble, &dp, &mut prng, l, None);
                    }
                    l.castle_cut = cut.as_ref().map_or_else(|r| *r, |_| castle::CUT_DONE);
                    let set = castle::Setting {
                        follow: true,
                        capital: s.capital,
                        wall_up: walled,
                        planned: plan.ward[i] == Some(Ward::Castle),
                        inner: cut.is_ok().then_some(want_inner),
                        hall: cut.is_ok() && (s.capital || s.tier == Tier::Metropolis),
                        moat,
                    };
                    let mut ways_at = l.roads.len();
                    let mut laid = castle(site, &cblock, toward, w, &dp, set, &on_wall, &gate_pts, (&town_segs, &town_towers), &mut prng, l, true, true, None);
                    // (Nor a ring with a corner under 35 degrees between long sides: a wedge of yard.)
                    let spike = |r: &Vec<P>| {
                        let k = r.len();
                        (0..k).any(|j| {
                            let (a, b) = (sub(r[(j + k - 1) % k], r[j]), sub(r[(j + 1) % k], r[j]));
                            len(a) >= 40.0 && len(b) >= 40.0 && dot(a, b) / (len(a) * len(b)) > libm::cos(35.0_f64.to_radians())
                        })
                    };
                    if let Some(was) = uncut.filter(|_| laid.as_ref().is_none_or(spike) || !l.buildings[first..].iter().any(|b| b.keep.is_some())) {
                        *l = was;
                        prng = lots_of(i, w, false).1;
                        l.castle_cut = castle::CUT_KEEP;
                        ways_at = l.roads.len();
                        let set = castle::Setting { inner: None, hall: false, ..set };
                        laid = castle(site, &block, toward, w, &dp, set, &on_wall, &gate_pts, (&town_segs, &town_towers), &mut prng, l, true, true, None);
                    }
                    match &before {
                        Some(was) if moat.is_some() && (laid.is_none() || l.castles.len() < castles_at + 2) => {
                            *l = was.clone();
                            prng = lots_of(i, w, false).1;
                            moat = None;
                        }
                        _ => break (laid, ways_at, blocks_at),
                    }
                };
                // No room for its curtain after all: the patch is a noble ward's (its insets, streets and lots).
                if laid.is_some() {
                    l.castle_patch = Some(i as u32);
                    castle_ways.extend(l.roads[ways_at..].iter().map(|r| r.0.clone()));
                } else {
                    let d = insets(i, Ward::Noble);
                    let block = inset(&c.pts, &d);
                    l.castle_cut = 0;
                    if block.len() >= 3 {
                        l.roads.truncate(roads_at);
                        l.buildings.truncate(first);
                        l.blocks.truncate(blocks_at);
                        if let Some(b) = l.blocks.last_mut() {
                            *b = block.clone();
                        }
                        // (Set by hand: the castle's after all, a tower house, `towerhouse`; its
                        // block left open.)
                        if plan.ward[i] == Some(Ward::Castle) {
                            let streets = street_lines(i, &d);
                            let (dp, mut prng) = lots_of(i, Ward::Noble, false);
                            build_block(site, Block::Plain(&block), &Front::of(&block, &streets), Ward::Noble, &dp, &mut prng, l, None);
                        } else {
                            tower_houses.push((i, c.pts.clone()));
                        }
                    }
                }
            }
            // Every other block's fill (`fill_block`), as its ward and its patch decide.
            _ => {
                let fill = match w {
                    // A market square: a monument (by roll in `fill_block`), and what it is for.
                    Ward::Plaza => Fill::Square(square_kind(plan.roll(i, 44), s.tier >= Tier::City)),
                    Ward::Farm => Fill::Farm { class: if ch.ward.contains_key(&i) { 0 } else { 1 }, rng: prng },
                    Ward::Park if sparse => Fill::Lots { ward: w, dp, rng: prng, thin: None, close: false },
                    // (A block left too small for a park once its streets are cut, 6,000 sq ft or 40 ft
                    // across: built on as a common ward's, `PARK_SLIVER`.)
                    Ward::Park if area(&block).abs() < 6000.0 || obb(&block).short < 40.0 => {
                        let (dp, prng) = lots_of(i, Ward::Common, false);
                        l.blocks.push(block.clone());
                        Fill::Lots { ward: Ward::Common, dp, rng: prng, thin: None, close: false }
                    }
                    // A park: as set by hand, else a formal garden by the castle (palace grounds), most
                    // often by the nobles, now and then in a capital or a metropolis (its first always), a
                    // common elsewhere (a town's always, and one on a slope). Its ways in are on its sides
                    // along a street (not the water's).
                    Ward::Park => {
                        let rich = s.capital || s.tier == Tier::Metropolis;
                        let by = |w: Ward| (0..m).any(|e| c.labels[e] >= 0 && ward[c.labels[e] as usize] == Some(w));
                        let t = plan.roll(i, 72);
                        // (Formal only on ground level enough to lay out as a terrace: 6 ft of relief at most.)
                        let (lo, hi) = site.dry_relief(&block);
                        let level = hi - lo <= 6.0;
                        let formal = level && (plan.palace == Some(i) || (!plan.steep[i] && s.tier >= Tier::City && (by(Ward::Castle) || (by(Ward::Noble) && t < 0.6) || (rich && t < 0.35))));
                        let kind = ch.park.get(&i).copied().unwrap_or(if formal { ParkKind::Formal } else { ParkKind::Common });
                        // (R5 towns: the common picked for a cemetery, `cemetery::inner_pick`.)
                        let kind = if kind == ParkKind::Common && !ch.park.contains_key(&i) && cem_inner == Some(i) { ParkKind::Cemetery } else { kind };
                        // (R5 towns, 114: a steppe town's horse pens and market in place of a formal garden.)
                        let kind = if kind == ParkKind::Formal && !ch.park.contains_key(&i) && s.culture % 5 == 4 { ParkKind::Horses } else { kind };
                        let ways: Vec<(P, f64)> = street_lines(i, &d)
                            .into_iter()
                            .filter(|&(n, off)| {
                                (0..m).any(|e| {
                                    let mid = lerp(c.pts[e], c.pts[(e + 1) % m], 0.5);
                                    (dot(mid, n) - off - d[e]).abs() < 0.5 && strip(e, &d).is_some() && i < n_inner
                                })
                            })
                            .collect();
                        let key = hash3(plan.pseed, i as i64, ch.reroll.get(&i).copied().unwrap_or(0) as i64, 0x9a4c);
                        let cemetery = (kind == ParkKind::Cemetery).then(|| cemetery::Style::of(s, i, false));
                        if kind == ParkKind::Cemetery {
                            cem_laid += (!ch.park.contains_key(&i)) as usize;
                        }
                        Fill::Park { kind, ways, key, cemetery }
                    }
                    // (R6 P8) The sprawl outside the walls: a street grid of small lots, alleys as
                    // often as a common ward's (not a slum's few, which left most lots without a
                    // front and the rest scattered huts). As generated it thins out away from the
                    // city a block between alleys at a time, kept with a probability that falls
                    // with distance from the corners this patch shares with the city; set by
                    // hand, it is built up as a common ward is.
                    Ward::Slum if i >= n_inner => {
                        let mut dp = dp;
                        dp.block_size *= 0.5;
                        dp.grid_chaos = (2.0 * dp.grid_chaos).min(1.0);
                        let thin = (!ch.ward.contains_key(&i)).then(|| {
                            let anchors: Vec<P> = (0..m)
                                .filter(|&e| {
                                    let nb = c.labels[e];
                                    nb >= 0 && usable(nb as usize)
                                })
                                .flat_map(|e| [c.pts[e], c.pts[(e + 1) % m]])
                                .collect();
                            (anchors, c.pts.iter().map(|q| dist(*q, centroid(&c.pts))).fold(0.0, f64::max))
                        });
                        Fill::Lots { ward: w, dp, rng: prng, thin, close: false }
                    }
                    // A temple's close where `fits` saw that it does (else, with no lots that would
                    // do, its temple alone).
                    _ => Fill::Lots { ward: w, dp, rng: prng, thin: None, close: w == Ward::Temple && closes.contains(&i) },
                };
                if capturing {
                    hand::cap_fill(i, &fill, &block, &streets, &|k| plan.roll(i, k));
                }
                fill_block(site, &block, streets, fill, &|k| plan.roll(i, k), places::Spot::Patch(i as u32), l, &mut plots);
            }
        }
        if capturing {
            hand::cap_built(i, &l.buildings[first..]);
        }
        // A square where streets meet trims the lots at its corner of the block.
        for cut in &cuts {
            cut.trim(i, &mut l.buildings, first);
        }
        for b in &mut l.buildings[first..] {
            b.id = i as u32;
        }
        // A castle set by hand: its towers after every other (no `t:` id moves).
        if w == Ward::Castle && plan.ward[i] != Some(Ward::Castle) {
            let gates: Vec<(P, bool)> = l.gate_towers.drain(gates_at..).map(|t| (t, true)).collect();
            let towers: Vec<(P, bool)> = l.towers.drain(towers_at..).map(|t| (t, false)).collect();
            l.extra_towers.extend(towers.into_iter().chain(gates));
            hand_walls.extend(walls_at..l.walls.len());
        }
    }
    // The fields: as much farmland as the farm patches hold, taken outward by each plot's reach
    // (`fields::belt_order`), so the belt's edge frays over the open ground and short of it.
    let want: f64 = plots.iter().filter(|p| p.1 < 2).map(|p| area(&p.0).abs()).sum();
    // (None on a road running across open ground.)
    plots.retain(|p| p.1 == 0 || !far_roads.iter().any(|r| r.0.windows(2).any(|s| poly_seg_dist(&p.0, s[0], s[1]) < 0.5 * r.2 + 4.0)));
    // (Off sand: cut back to the land, `fields::off_sand`; the belt takes plots farther out for
    // those lost.)
    let plots: Vec<(Vec<P>, u8)> = plots.into_iter().filter_map(|(f, class)| fields::off_sand(site, &f).map(|f| (f, class))).collect();
    let picked = fields::belt_order(site, plan.pseed, &plots, want, &|c| {
        let b = belt(c);
        len(c) / (b * fields::reach(site, plan.pseed, c, &far_roads, b))
    });
    l.fields.extend(picked);
    // R5 towns: the rest of the town's cemeteries beside its roads out (`cemetery::by_roads`).
    cemetery::by_roads(site, s, plan, l, cem_want.saturating_sub(cem_laid));
    // The squares the chords enclose; a green's lanes stop at its edge (a lawn, not paving), where
    // a path runs before the houses fronting it (laid once the lanes are cut).
    let (mut greens, mut green_paths): (Vec<Vec<P>>, Vec<(Vec<P>, f64)>) = (Vec::new(), Vec::new());
    for (cut, &(v, _)) in cuts.iter().zip(&spots) {
        if let Some(pts) = cut.outline() {
            if capturing {
                hand::cap_square(v, cut.kind);
            }
            if cut.kind == PlazaKind::Green {
                greens.push(pts.clone());
                green_paths.extend(cut.edge_paths(5.0));
            }
            l.plazas.push(Plaza { pts, kind: cut.kind, spot: places::Spot::Corner(v as u32) });
        }
    }
    // Off the roads into the city and the ground before each gate (`clear_of`), a bent entrance's
    // gate block, and off a castle's way to its gate.
    let bent_blocks = if walled || plan.walled { gatehouse::bent_blocks(l, &wall_laid.0, &wall_laid.2) } else { Vec::new() };
    // (R5 towns, M18: by a gate taken out, nothing nearer the wall than the town stood with it.)
    let gone: Vec<(P, f64)> = if walled { ch.wall.keep_off.iter().map(|&(v, k)| (mesh.pos[v], k)).collect() } else { Vec::new() };
    let off_gone = |b: &Building| {
        b.ward == Ward::Castle
            || gone.iter().all(|&(g, keep)| !b.poly.iter().any(|p| dist(*p, g) < wallgates::GATE_GROUND_FT) || wallgates::wall_off(&wall_laid.0, &b.poly) >= keep)
    };
    l.buildings.retain(|b| clear_of(b) && off_gone(b) && outline::off_bastions(&wall_laid.3.placed, &b.poly) && (b.ward == Ward::Castle || !gatehouse::on_bent_block(&bent_blocks, &b.poly)) && (b.ward == Ward::Castle || castle_ways.iter().all(|w| w.windows(2).all(|s| poly_seg_dist(&b.poly, s[0], s[1]) >= 5.0))));
    for (i, patch) in &tower_houses {
        let mut prng = lots_of(*i, Ward::Castle, false).1;
        if let Some(mut k) = towerhouse::tower_house(site, patch, l, &town_segs, &town_towers, &gate_pts, &mut prng) {
            l.buildings.retain(|b| b.ward == Ward::Castle || !towerhouse::under(&k.poly, &b.poly));
            let c = centroid(&k.poly);
            l.castles.push((c, k.poly.iter().map(|p| dist(*p, c)).fold(0.0, f64::max)));
            k.id = *i as u32;
            l.buildings.push(k);
        }
    }
    // Steps on steep streets, judged before the streets are cut at the water (wet ground is
    // level), so every piece is still as recorded.
    if s.tier >= stairs::STAIR_TIER || survey.is_some() {
        let prm = stairs::StairParams::DEFAULT;
        let profiles = stairs::profiles(site, &mesh.pos, &pieces, l, if survey.is_some() { stairs::SURVEY_FROM } else { prm.g_off });
        if s.tier >= stairs::STAIR_TIER {
            l.stairs = stairs::flights(&profiles, &prm);
        }
        if let Some(out) = survey {
            out.extend(profiles);
        }
    }
    // Paved streets stop at the water's edge unless a bridge carries them (and lanes at a green).
    let bridges = l.bridges.clone();
    let on_water = |p: P| site.wet(p) && !bridges.iter().any(|b| contains(b, p));
    let mut clipped: Vec<(Vec<P>, u8, f64)> = Vec::new();
    for (pts, class, w) in std::mem::take(&mut l.roads) {
        if class < 3 {
            clipped.push((pts, class, w));
            continue;
        }
        let on_water = |q: P| on_water(q) || (class == 3 && greens.iter().any(|g| contains(g, q)));
        let mut run: Vec<P> = Vec::new();
        for seg in pts.windows(2) {
            let n = (dist(seg[0], seg[1]) / 5.0).ceil().max(1.0) as usize;
            for j in 0..=n {
                let q = lerp(seg[0], seg[1], j as f64 / n as f64);
                if on_water(q) {
                    if run.len() >= 2 {
                        clipped.push((std::mem::take(&mut run), class, w));
                    }
                    run.clear();
                } else if run.last().is_none_or(|r| dist(*r, q) > 1e-6) {
                    // Keep only corners and run ends: interior samples on a straight segment
                    // add nothing.
                    if j == 0 || j == n || run.is_empty() {
                        run.push(q);
                    } else if j + 1 <= n && on_water(lerp(seg[0], seg[1], (j + 1) as f64 / n as f64)) {
                        run.push(q);
                    }
                }
            }
        }
        if run.len() >= 2 {
            clipped.push((run, class, w));
        }
    }
    clipped.extend(green_paths.into_iter().map(|(pts, w)| (pts, 3, w)));
    l.roads = clipped;
    // Districts keep the patches and names they were planned with; their outlines and labels
    // follow the moved corners.
    l.quarters = districts::of(plan, ch)
        .iter()
        .map(|(q, members, touched)| {
            let patches: Vec<Vec<P>> = members.iter().map(|&i| cells[i].pts.clone()).collect();
            // (R5 towns: one changed by hand, at its largest connected piece; an emptied one, none.)
            let over: Vec<Vec<P>> = match (*touched, members.is_empty()) {
                (_, true) => Vec::new(),
                (true, false) => districts::label_piece(plan, &cells, members).iter().map(|&i| cells[i].pts.clone()).collect(),
                _ => patches.clone(),
            };
            let mut label = if over.is_empty() { Vec::new() } else { label_curve(&over).0 };
            // (Not across a park's middle, its basin or its hub: over one of its sides.)
            if let Some(pk) = label.get(label.len() / 2).and_then(|&m| l.parks.iter().find(|pk| contains(&pk.pts, m))) {
                let o = obb(&pk.pts);
                let shift = mul([-o.axis[1], o.axis[0]], 0.3 * o.short);
                label.iter_mut().for_each(|p| *p = add(*p, shift));
            }
            // (Its middle over the district: a piece bent round a corner, or its corners moved
            // since, has its middle off it; moved onto the middle of its nearest patch.)
            if let Some(&m) = label.get(label.len() / 2)
                && !over.iter().any(|f| contains(f, m))
                && let Some(f) = over.iter().min_by(|a, b| dist(centroid(a), m).total_cmp(&dist(centroid(b), m)))
            {
                let c = if contains(f, centroid(f)) { centroid(f) } else { mean(f) };
                let shift = sub(c, m);
                label.iter_mut().for_each(|p| *p = add(*p, shift));
            }
            Quarter { name: q.name.clone(), kind: q.kind, patches, label }
        })
        .collect();
    hand_walls
}
