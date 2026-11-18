//! Town plans: road ends and directions, Voronoi patches, `TownPlan` and `plan_town`.

use super::*;
pub(super) use crate::t0::radii::OUTSKIRTS;

/// What a settlement is laid out from: its ground (`Site`), where its roads arrive (local
/// coords: the road ends and passes near it), and its random stream.
pub(super) fn prepare<'a>(world: &World, t0: &'a T0, index: usize) -> (Site<'a>, Vec<(P, u8)>, Pcg32) {
    let s = settlement(t0, index).expect("layout of a settlement");
    let center = [s.x, s.y];
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
    let site = Site::new(t0, center, lattice, reach(s) + 500.0, river_pieces(t0, center, reach(s) + 500.0), world.params().sea_level_ft);
    let rng = Pcg32::new(hash2(world.stream("town"), rng_key(t0, index), s.seed as i64), 31);
    (site, ends_of(t0, index), rng)
}

/// Where the roads reaching settlement `index` come in (local ft from its centre), with their
/// class: one added by hand meets its own link (a town's at its edge); no network road comes so
/// near it.
pub(crate) fn ends_of(t0: &T0, index: usize) -> Vec<(P, u8)> {
    let s = settlement(t0, index).expect("layout of a settlement");
    let center = [s.x, s.y];
    let r = urban_radius(s.tier, s.population);
    let track = index.checked_sub(t0.settlements.len()).and_then(|poi| t0.made.iter().find(|m| m.poi == poi)).and_then(|m| m.track.first());
    match track {
        Some(&a) => vec![(sub(a, center), crate::t0::made::link_class(s.tier) as u8)],
        None => road_ends(t0, center, r, road_trim_radius(s.tier, s.population)),
    }
}

/// A town's plan (kept by `wards::plan`), without building it; `None` for a village or a site.
pub(super) fn town_plan(world: &World, t0: &T0, index: usize) -> Option<Rc<TownPlan>> {
    let s = culture::settlement(world, t0, index).filter(|s| s.tier >= Tier::Town)?;
    Some(wards::plan(world, index, s.culture, || {
        let (site, road_ends, mut rng) = prepare(world, t0, index);
        plan_town(&site, &s, urban_radius(s.tier, s.population), &road_ends, &mut rng)
    }))
}

/// Buildings with no way in: every point just outside their walls lies inside another
/// building (lot splitting can box one in). They become yards.
pub(super) fn drop_walled_in(l: &mut Layout) {
    const CELL: f64 = 80.0;
    let mut grid: FastMap<(i64, i64), Vec<usize>> = FastMap::default();
    for (k, b) in l.buildings.iter().enumerate() {
        let (x0, y0, x1, y1) = b.poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        for cy in crate::core::floor(y0 / CELL) as i64..=crate::core::floor(y1 / CELL) as i64 {
            for cx in crate::core::floor(x0 / CELL) as i64..=crate::core::floor(x1 / CELL) as i64 {
                grid.entry((cx, cy)).or_default().push(k);
            }
        }
    }
    let covered = |p: P, own: usize| {
        grid.get(&(crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64)).into_iter().flatten().any(|&k| k != own && l.buildings[k].structure == Structure::Roofed && contains(&l.buildings[k].poly, p))
    };
    let walled: Vec<bool> = (0..l.buildings.len())
        .map(|k| {
            let b = &l.buildings[k];
            if b.structure != Structure::Roofed {
                return false;
            }
            let c = centroid(&b.poly);
            let m = b.poly.len();
            // Points 3 ft outside each wall, at its quarters.
            (0..m).all(|e| {
                let (a, q) = (b.poly[e], b.poly[(e + 1) % m]);
                [0.25, 0.5, 0.75].iter().all(|&t| {
                    let p = lerp(a, q, t);
                    let d = sub(p, c);
                    covered(add(p, mul(d, 3.0 / len(d).max(1e-9))), k)
                })
            })
        })
        .collect();
    let mut k = 0;
    l.buildings.retain(|_| {
        k += 1;
        !walled[k - 1]
    });
}

/// Where roads arrive (local coords): the end of every road curve that stops at this
/// settlement (roads to towns are trimmed at the urban edge), with its road class. Roads that
/// pass by without ending are reported by direction at the urban edge (class 2).
fn road_ends(t0: &T0, center: P, r: f64, trim: f64) -> Vec<(P, u8)> {
    let mut out: Vec<(P, u8)> = Vec::new();
    let reach = (1.6 * r).max(1.1 * trim) + 200.0;
    for (ri, k) in t0.roads.segments_near([center[0] - reach, center[1] - reach, center[0] + reach, center[1] + reach], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        let n = rc.pts.len();
        for (kk, t) in [(0usize, 0.0), (n.saturating_sub(2), 1.0)] {
            if k as usize != kk {
                continue;
            }
            let e = sub(rc.eval(kk, t, 5.0, t0.cell_ft).p, center);
            if len(e) < 1.05 * trim + 50.0 && !out.iter().any(|(q, _)| dist(*q, e) < 1.0) {
                out.push((e, rc.class as u8));
            }
        }
    }
    if out.is_empty() {
        out = road_dirs(t0, center, r).into_iter().map(|p| (p, 2)).collect();
    }
    out
}

/// Directions roads come from: the nearest point of each road curve within reach, as a
/// local point on or near the urban edge.
fn road_dirs(t0: &T0, center: P, r: f64) -> Vec<P> {
    let reach = r * 1.6 + 200.0;
    let mut out: Vec<P> = Vec::new();
    for (ri, k) in t0.roads.segments_near([center[0] - reach, center[1] - reach, center[0] + reach, center[1] + reach], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        for j in 0..=8 {
            let p = sub(rc.eval(k as usize, j as f64 / 8.0, 20.0, t0.cell_ft).p, center);
            let d = len(p);
            if d < reach && d > r * 0.5 {
                // One entry per direction.
                let dir = mul(p, 1.0 / d);
                if !out.iter().any(|q| dot(mul(*q, 1.0 / len(*q)), dir) > 0.93) {
                    out.push(mul(dir, r));
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Towns and cities.

pub(super) fn voronoi(sites: &[P], boundary: &[P]) -> Vec<Labeled> {
    let base = Labeled { pts: boundary.to_vec(), labels: vec![-1; boundary.len()] };
    (0..sites.len())
        .map(|i| {
            let si = sites[i];
            // Distances once, not per comparison (the same values, so the same order).
            let d: Vec<f64> = sites.iter().map(|&s| dist(s, si)).collect();
            let mut order: Vec<usize> = (0..sites.len()).filter(|&j| j != i).collect();
            order.sort_by(|&a, &b| d[a].total_cmp(&d[b]));
            let mut poly = base.clone();
            let reach = |poly: &Labeled| poly.pts.iter().map(|p| dist(*p, si)).fold(0.0, f64::max);
            let mut r = reach(&poly);
            for j in order {
                let sj = sites[j];
                if d[j] > 2.0 * r {
                    break;
                }
                let n = sub(sj, si);
                let c = (dot(sj, sj) - dot(si, si)) * 0.5;
                let cut = clip(&poly, n, c, j as i32);
                if cut.pts.len() < 3 {
                    poly = cut;
                    break;
                }
                r = reach(&cut);
                poly = cut;
            }
            poly
        })
        .collect()
}

/// A city's castle patch: room for a castle (its block 18 ft in: at least this many sq ft, and this
/// many ft across), and what a patch on the town wall gains (ft) against the height of one inside.
/// (Below zero: towns stand on gentle ground, mostly a little higher at the edge; so about two
/// castles in five stand inside the town, on 24 worlds.)
const CASTLE_MIN_AREA: f64 = 20_000.0;
const CASTLE_MIN_SHORT: f64 = 120.0;
const WALL_BIAS_FT: f64 = -0.4;

/// Water (the sea, a lake or a river's channel) on a patch side from `a` to `b` or beyond it, away
/// from `inside` (its patch's middle): at two neighbouring points of those every 15 ft out to 120 ft
/// from the side's middle (not a fleck on flat ground at the water's level).
fn water_beyond(site: &Site, a: P, b: P, inside: P) -> bool {
    let (mid, el) = (lerp(a, b, 0.5), dist(a, b));
    if el < 1e-6 {
        return site.wet(mid);
    }
    let mut out = [(a[1] - b[1]) / el, (b[0] - a[0]) / el];
    if dot(out, sub(mid, inside)) < 0.0 {
        out = mul(out, -1.0);
    }
    let wet: Vec<bool> = (0..=8).map(|k| site.wet(add(mid, mul(out, 15.0 * k as f64)))).collect();
    wet.windows(2).any(|w| w[0] && w[1])
}

pub(super) fn plan_town(site: &Site, s: &Settlement, r: f64, road_ends: &[(P, u8)], rng: &mut Pcg32) -> TownPlan {
    let seeds::Seeds { pseed, spacing, r_out, rhombus, n_inner, total, sites, cells } = seeds::plant(site, s, r, rng);
    // Per-patch random streams (`seeds::plant`).
    let roll = |i: usize, stage: i64| seeds::roll(pseed, i, stage);

    // Shared-vertex mesh; tiny edges collapse so patches are mostly quads and pentagons.
    let mut mesh = Mesh::from_cells(&cells);
    mesh.collapse_short_edges((0.3 * spacing).min(100.0));
    let vf = mesh.vertex_faces();
    let inner = |j: i32| j >= 0 && (j as usize) < n_inner;
    // (An outer cell left empty, reaching past the outskirts, is water only if its seed is: it
    // is the country beyond the wall, which stands along it.)
    let wet: Vec<bool> = (0..total)
        .map(|i| if mesh.faces[i].len() < 3 { i < n_inner || site.wet(sites[i]) } else { site.wet(centroid(&mesh.face_pts(i))) })
        .collect();
    let face_ok: Vec<bool> = mesh.faces.iter().map(|f| f.len() >= 3).collect();
    let usable = |i: usize| i < n_inner && !wet[i] && face_ok[i];
    // A patch is on the water by its middle; a town patch's side facing one is on the shore (a
    // quay, no wall) only where water lies beyond it. A town patch on the water with no run of
    // such patches out to the outskirts (a river or lake patch amid the town) is ground the town
    // encloses: no wall round it.
    let mut enclosed: Vec<bool> = (0..total).map(|i| i < n_inner && wet[i] && face_ok[i]).collect();
    let mut open: Vec<usize> = (0..n_inner).filter(|&i| enclosed[i] && mesh.labels[i].iter().any(|&j| !inner(j))).collect();
    while let Some(i) = open.pop() {
        if enclosed[i] {
            enclosed[i] = false;
            open.extend(mesh.labels[i].iter().filter(|&&j| inner(j) && enclosed[j as usize]).map(|&j| j as usize));
        }
    }
    let mut shore_sides: FastSet<(usize, usize)> = Default::default();
    for i in (0..n_inner).filter(|&i| usable(i)) {
        let (face, cen) = (&mesh.faces[i], centroid(&mesh.face_pts(i)));
        for e in 0..face.len() {
            let nb = mesh.labels[i][e];
            let (a, b) = (face[e], face[(e + 1) % face.len()]);
            if nb >= 0 && wet[nb as usize] && water_beyond(site, mesh.pos[a], mesh.pos[b], cen) {
                shore_sides.insert((a.min(b), a.max(b)));
            }
        }
    }
    // (Per patch side: by the water or that enclosed ground, so not on the wall.)
    let by_water: Vec<Vec<bool>> = (0..total)
        .map(|i| {
            let f = &mesh.faces[i];
            (0..f.len())
                .map(|e| {
                    let (nb, a, b) = (mesh.labels[i][e], f[e], f[(e + 1) % f.len()]);
                    nb >= 0 && wet[nb as usize] && (enclosed[nb as usize] || shore_sides.contains(&(a.min(b), a.max(b))))
                })
                .collect()
        })
        .collect();
    // Edges on the river channel, and vertices that must not move (water, river).
    let mut river_edge: FastSet<(usize, usize)> = Default::default();
    for face in &mesh.faces {
        let m = face.len();
        for k in 0..m {
            let (a, b) = (face[k], face[(k + 1) % m]);
            if site.near_river(lerp(mesh.pos[a], mesh.pos[b], 0.5), 2.0) {
                river_edge.insert((a.min(b), a.max(b)));
            }
        }
    }
    let is_river_edge = |a: usize, b: usize| river_edge.contains(&(a.min(b), a.max(b)));
    let pinned: Vec<bool> = (0..mesh.pos.len()).map(|v| site.near_river(mesh.pos[v], 0.3 * spacing) || vf[v].iter().any(|&f| wet[f])).collect();

    let walled = s.tier >= Tier::City || rng.next_f64() < 0.4 || s.isolated;
    // Wall ring: its runs, every corner on them, its sides (`wallring::chains`).
    let (chains, ring, ring_edges) = wallring::chains(site, &mesh, n_inner, &wet, &by_water, &usable, &is_river_edge);
    let on_wall = |a: usize, b: usize| ring_edges.contains(&(a.min(b), a.max(b)));

    // Gates where roads arrive (or two, if none do).
    let mut ends: Vec<(P, u8)> = road_ends.to_vec();
    if ends.is_empty() {
        let a = rng.range(0.0, std::f64::consts::TAU);
        ends = vec![([r * libm::cos(a), r * libm::sin(a)], 2), ([-r * libm::cos(a), -r * libm::sin(a)], 2)];
    }
    // Ring neighbours of each wall corner.
    let mut ring_nb: FastMap<usize, Vec<usize>> = FastMap::default();
    for (chain, closed) in &chains {
        let n = chain.len();
        for k in 0..n {
            if *closed || k > 0 {
                ring_nb.entry(chain[k]).or_default().push(chain[(k + n - 1) % n]);
            }
            if *closed || k + 1 < n {
                ring_nb.entry(chain[k]).or_default().push(chain[(k + 1) % n]);
            }
        }
    }
    // Gates: each road gets its own, never next to another gate: the wall corner (or, for a
    // road that comes in where the wall leaves the waterfront open, a corner of the built
    // edge, from which a main street runs) its approach reaches most cheaply: short, little
    // over water, and not through the town. `gate_of[i]` is the gate for road end i (roads
    // that find none share the nearest).
    let heading_of: Vec<Option<P>> = ends.iter().map(|(e, _)| site.road_heading(*e)).collect();
    let town_faces: Vec<([f64; 4], Vec<P>)> = (0..n_inner)
        .filter(|&i| usable(i))
        .map(|i| {
            let pts = mesh.face_pts(i);
            let bb = bbox(&pts);
            (bb, pts)
        })
        .collect();
    let in_town = |p: P| town_faces.iter().any(|(bb, poly)| p[0] >= bb[0] && p[0] <= bb[2] && p[1] >= bb[1] && p[1] <= bb[3] && contains(poly, p));
    // What the approach from road end k to a gate costs: its length, plus 4× the stretch over
    // water (a bridge), plus 20× the stretch through the town on land (not counting its last
    // 40 ft).
    let way_cost = |k: usize, g: P| {
        let curve = approach_curve(ends[k].0, g, heading_of[k]);
        let total: f64 = curve.windows(2).map(|w| dist(w[0], w[1])).sum();
        let n = (total / 15.0).ceil().max(1.0) as usize;
        let step = total / n as f64;
        let samples: Vec<P> = (0..=n).map(|j| point_along(&curve, step * j as f64).0).collect();
        let (mut wet, mut through) = (0.0, 0.0);
        for p in &samples {
            if site.wet(*p) {
                wet += step;
            } else if dist(*p, g) >= 40.0 && in_town(*p) {
                through += step;
            }
        }
        total + 4.0 * wet + 20.0 * through
    };
    // Corners of the built edge: town corners next to the outskirts, the water, or a corner in
    // the river (town patches reach the channel's centre line).
    let edge_vertices: Vec<usize> = (0..mesh.pos.len())
        .filter(|&v| {
            !site.near_river(mesh.pos[v], 0.0)
                && vf[v].iter().any(|&f| usable(f))
                && (vf[v].iter().any(|&f| !usable(f)) || adj_vertices(&mesh, v).iter().any(|&u| site.near_river(mesh.pos[u], 0.0)))
        })
        .collect();
    // A walled town's main street goes straight in from its gate, so a gate wants a spoke: a side
    // going in between two town patches (its far end off the river and reaching the central
    // patch without passing the gate or running along the wall), as square to the wall as can be. Gates are costed
    // where they will stand (a wall gate's corner is flattened onto its neighbours' line); a
    // spoke turning more than 20° off the wall's normal costs more.
    // (A corner in the river channel, by its position: also used once the corners have moved.)
    let in_river = |p: P| site.near_river(p, 0.0);
    // (The central patch as the plaza will be found, near enough: the castle, chosen once the
    // gates are, may yet push a plaza that is not the rhombus one patch over. The routing
    // tests the plaza itself: `is_spoke`.)
    let (joined, central): (Vec<Vec<usize>>, Vec<usize>) = if walled {
        let c = rhombus.then_some(0).filter(|&i| usable(i)).or_else(|| (0..n_inner).filter(|&i| usable(i)).min_by(|&a, &b| len(sites[a]).total_cmp(&len(sites[b]))));
        (joins(&mesh, site, &vf, &usable, &|a, b| on_wall(a, b) || is_river_edge(a, b)), c.map(|c| mesh.faces[c].clone()).unwrap_or_default())
    } else {
        (Vec::new(), Vec::new())
    };
    // (Whether u reaches one of the corners `to` without passing v.)
    let reaches = |v: usize, u: usize, to: &[usize]| {
        let mut seen: FastSet<usize> = [v, u].into_iter().collect();
        let mut open = vec![u];
        while let Some(x) = open.pop() {
            if to.contains(&x) {
                return true;
            }
            open.extend(joined[x].iter().copied().filter(|&y| seen.insert(y)));
        }
        false
    };
    let spoke_ok = |v: usize, u: usize| {
        !ring.contains(&u) && !in_river(mesh.pos[u]) && !is_river_edge(v, u) && vf[v].iter().filter(|&&f| vf[u].contains(&f) && usable(f)).count() == 2 && reaches(v, u, &central)
    };
    let flat_at = |v: usize| match ring_nb.get(&v) {
        Some(nb) if nb.len() == 2 => lerp(mesh.pos[nb[0]], mesh.pos[nb[1]], 0.5),
        _ => mesh.pos[v],
    };
    // (How far a side from v to u turns off the wall's normal at v, radians.)
    let off_normal = |pos: &[P], v: usize, at: P, u: usize| -> f64 {
        let Some(nb) = ring_nb.get(&v).filter(|nb| nb.len() == 2) else { return 0.0 };
        let (t, d) = (sub(pos[nb[1]], pos[nb[0]]), sub(pos[u], at));
        libm::asin((dot(d, t) / (len(d) * len(t)).max(1e-9)).abs().min(1.0))
    };
    // (Each corner's once: a search to the central patch per side, asked per road end.)
    let spoke_memo: std::cell::RefCell<FastMap<usize, Option<f64>>> = Default::default();
    let spoke_off = |v: usize| {
        if let Some(&off) = spoke_memo.borrow().get(&v) {
            return off;
        }
        let at = flat_at(v);
        let off = adj_vertices(&mesh, v).into_iter().filter(|&u| spoke_ok(v, u)).map(|u| off_normal(&mesh.pos, v, at, u)).min_by(f64::total_cmp);
        spoke_memo.borrow_mut().insert(v, off);
        off
    };
    let gate_at = |v: usize| if walled { flat_at(v) } else { mesh.pos[v] };
    // (Whether the approach from road end k to v crosses the wall off v's own sides.)
    let crosses_wall = |k: usize, v: usize| {
        let curve = approach_curve(ends[k].0, gate_at(v), heading_of[k]);
        let x = |p: P, q: P, a: P, b: P| {
            let c = |o: P, x: P, y: P| cross(sub(x, o), sub(y, o));
            c(p, q, a) * c(p, q, b) < 0.0 && c(a, b, p) * c(a, b, q) < 0.0
        };
        ring_edges
            .iter()
            .filter(|e| e.0 != v && e.1 != v)
            .any(|&(a, b)| curve.windows(2).any(|w| x(w[0], w[1], mesh.pos[a], mesh.pos[b])))
    };
    let mut gates: Vec<usize> = Vec::new();
    let mut gate_of: Vec<Option<usize>> = Vec::new();
    for (k, (e, _)) in ends.iter().enumerate() {
        let mut cands: Vec<usize> = ring.iter().copied().filter(|v| !pinned[*v]).collect();
        cands.sort_by(|&p, &q| dist(mesh.pos[p], *e).total_cmp(&dist(mesh.pos[q], *e)).then(p.cmp(&q)));
        let next_to_gate = |v: usize, gates: &[usize]| ring_nb.get(&v).is_some_and(|nb| nb.iter().any(|x| gates.contains(x)));
        // The nearest few wall corners (in a walled town, every other one with a spoke too), and
        // (costing 150 ft more: a wall gate is preferred) the nearest corners of the built edge
        // (off the water where any is); the cheapest way in wins.
        let free: Vec<usize> = cands.iter().copied().filter(|&v| !gates.contains(&v) && !next_to_gate(v, &gates)).collect();
        let mut options: Vec<(f64, usize)> = free.iter().copied().take(8).map(|v| (way_cost(k, gate_at(v)), v)).collect();
        // (A walled town's other corners with a spoke too.)
        if walled {
            options.extend(free.iter().copied().skip(8).filter(|&v| spoke_off(v).is_some()).map(|v| (way_cost(k, gate_at(v)), v)));
        }
        let mut edge: Vec<usize> = edge_vertices.iter().copied().filter(|v| !gates.contains(v) && !ring.contains(v)).collect();
        edge.sort_by(|&p, &q| dist(mesh.pos[p], *e).total_cmp(&dist(mesh.pos[q], *e)).then(p.cmp(&q)));
        options.extend(edge.into_iter().take(12).map(|v| (way_cost(k, mesh.pos[v]) + 150.0, v)));
        // (A gate over water would have its street cut off at the shore, so one on dry ground wins.)
        if options.iter().any(|o| !site.wet(mesh.pos[o.1])) {
            options.retain(|o| !site.wet(mesh.pos[o.1]));
        }
        // (In a walled town: a gate with no spoke is taken only where none has one, and one
        // whose approach crosses the wall elsewhere last of all.)
        if walled {
            let offs: Vec<Option<f64>> = options.iter().map(|o| spoke_off(o.1)).collect();
            let some_spoke = offs.iter().any(|off| off.is_some());
            for (o, off) in options.iter_mut().zip(offs) {
                o.0 += match off {
                    Some(a) => 2000.0 * (a - 20f64.to_radians()).max(0.0),
                    None if some_spoke => 100_000.0,
                    None => 0.0,
                };
                if crosses_wall(k, o.1) {
                    o.0 += 1_000_000.0;
                }
            }
        }
        let pick = options.iter().min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|o| o.1);
        match pick {
            Some(g) => {
                gates.push(g);
                gate_of.push(Some(g));
            }
            None => gate_of.push(None),
        }
    }
    // Roads that found no gate of their own share the nearest one.
    for (k, (e, _)) in ends.iter().enumerate() {
        if gate_of[k].is_none() {
            gate_of[k] = gates.iter().copied().min_by(|&p, &q| dist(mesh.pos[p], *e).total_cmp(&dist(mesh.pos[q], *e)));
        }
    }
    // A gate sits in a straight run of wall: flatten its corner.
    for &g in &gates {
        if let Some(nb) = ring_nb.get(&g)
            && nb.len() == 2
        {
            let mid = lerp(mesh.pos[nb[0]], mesh.pos[nb[1]], 0.5);
            mesh.try_move(&vf, g, mid);
        }
    }
    // Round the wall: three smoothing passes (gates, water and river corners stay put).
    for (chain, closed) in &chains {
        mesh.smooth_chain(&vf, chain, *closed, &|v| gates.contains(&v) || pinned[v], 3);
    }
    // Citadel: the highest city patch with room for a castle, inside the town (a full curtain of
    // its own) or on its wall (sharing the city wall; `WALL_BIAS_FT`), not by the market square nor
    // at a gate; reshaped toward a regular polygon.
    let plaza_face = rhombus.then_some(0).filter(|&i| usable(i));
    let labels = mesh.labels.clone();
    let next_to = |i: usize, j: usize| i == j || labels[j].contains(&(i as i32));
    let castle_face = if s.tier >= Tier::City {
        // (Its block as `build_town` will have it, near enough: 18 ft in, from the river its quay.)
        let roomy = |i: usize| {
            let pts = mesh.face_pts(i);
            let d: Vec<f64> = (0..pts.len())
                .map(|e| {
                    let mid = lerp(pts[e], pts[(e + 1) % pts.len()], 0.5);
                    if site.near_river(mid, 2.0) { site.nearest_river(mid).map_or(20.0, |r| r.2) + 16.0 } else { 18.0 }
                })
                .collect();
            let block = inset(&pts, &d);
            block.len() >= 3 && area(&block).abs() >= CASTLE_MIN_AREA && obb(&block).short >= CASTLE_MIN_SHORT
        };
        // On the wall: a side of it is the wall's (one touching it at a corner alone would stand
        // aslant just inside it), and none where the wall runs by the water (its quay leaves the
        // curtain just inside that wall); inside: no corner on it.
        let wall_sides = |i: usize, shore: bool| {
            let f = &mesh.faces[i];
            (0..f.len()).any(|e| {
                let nb = labels[i][e];
                ring.contains(&f[e]) && ring.contains(&f[(e + 1) % f.len()]) && (nb < 0 || nb as usize >= n_inner) && by_water[i][e] == shore
            })
        };
        let wall_side = |i: usize| wall_sides(i, false) && !wall_sides(i, true);
        let inside = |i: usize| !mesh.faces[i].iter().any(|v| ring.contains(v));
        let score = |i: usize| {
            let f = &mesh.faces[i];
            let ground = (site.height(sites[i]) + f.iter().map(|&v| site.height(mesh.pos[v])).sum::<f64>()) / (f.len() + 1) as f64;
            ground + if inside(i) { 0.0 } else { WALL_BIAS_FT }
        };
        (0..n_inner)
            .filter(|&i| usable(i) && (wall_side(i) || inside(i)) && !plaza_face.is_some_and(|pz| next_to(i, pz)) && !mesh.faces[i].iter().any(|v| gates.contains(v)) && roomy(i))
            .max_by(|&a, &b| score(a).total_cmp(&score(b)).then(b.cmp(&a)))
    } else {
        None
    };
    if let Some(cf) = castle_face {
        mesh.equalize(&vf, cf, &|v| gates.contains(&v) || pinned[v], 0.75);
    }

    // Street graph over city patch edges (never along the river channel); main streets run
    // from each gate to the plaza (the most central patch, if no market square was laid out:
    // never the castle's nor one beside it).
    let plaza = plaza_face.or_else(|| {
        (0..n_inner)
            .filter(|&i| usable(i) && !castle_face.is_some_and(|cf| next_to(i, cf)))
            .min_by(|&a, &b| len(sites[a]).total_cmp(&len(sites[b])))
    });
    debug_assert!(castle_face.is_none() || castle_face != plaza, "the castle is never on the market square");
    // Vertices in the river channel; streets cross only by explicit links between the banks
    // (each link is a bridge along the street).
    let crossing_links = bank_links(&mesh, site, &vf, &usable);
    let mut adj: Graph = FastMap::default();
    for &(a, _, b) in &crossing_links {
        let w = dist(mesh.pos[a], mesh.pos[b]) + 150.0;
        adj.entry((a as i64, 0)).or_default().push(((b as i64, 0), w));
        adj.entry((b as i64, 0)).or_default().push(((a as i64, 0), w));
    }
    for i in (0..n_inner).filter(|&i| usable(i)) {
        let face = &mesh.faces[i];
        let m = face.len();
        for e in 0..m {
            let (a, b) = (face[e], face[(e + 1) % m]);
            if is_river_edge(a, b) || in_river(mesh.pos[a]) || in_river(mesh.pos[b]) {
                continue;
            }
            let (pa, pb) = (mesh.pos[a], mesh.pos[b]);
            let grade = (site.height(pa) - site.height(pb)).abs() / dist(pa, pb).max(1.0);
            // (Along the castle's sides at twice the cost: the main streets go round it.)
            let castle_side = castle_face.is_some_and(|cf| {
                let f = &mesh.faces[cf];
                (0..f.len()).any(|k| (f[k], f[(k + 1) % f.len()]) == (a, b) || (f[k], f[(k + 1) % f.len()]) == (b, a))
            });
            let w = dist(pa, pb) * (1.0 + 40.0 * grade * grade) * if castle_side { 2.0 } else { 1.0 };
            adj.entry((a as i64, 0)).or_default().push(((b as i64, 0), w));
            adj.entry((b as i64, 0)).or_default().push(((a as i64, 0), w));
        }
    }
    // (A walled town's main streets never run along the wall nor come back to it at another
    // corner (another gate's, or one a street would meet the wall at off any gate); from a gate
    // with no way in but along it, they leave it as soon as they can.)
    let plaza_corners: Vec<usize> = plaza.map(|pz| mesh.faces[pz].clone()).unwrap_or_default();
    let (adj_main, adj_wall): (Graph, Graph) = if walled {
        let wall = |k: &(i64, i64), x: &((i64, i64), f64)| on_wall(k.0 as usize, x.0.0 as usize);
        let back = |x: &((i64, i64), f64)| ring.contains(&(x.0.0 as usize)) && !plaza_corners.contains(&(x.0.0 as usize));
        (
            adj.iter().map(|(k, nbs)| (*k, nbs.iter().copied().filter(|x| !wall(k, x) && !back(x)).collect())).collect(),
            adj.iter().map(|(k, nbs)| (*k, nbs.iter().map(|&x| if wall(k, &x) { (x.0, x.1 * 4.0) } else { x }).collect())).collect(),
        )
    } else {
        (Graph::default(), Graph::default())
    };
    let is_spoke = |g: usize, u: usize| !ring.contains(&u) && vf[g].iter().filter(|&&f| vf[u].contains(&f) && usable(f)).count() == 2 && reaches(g, u, &plaza_corners);
    let mut paths: Vec<Vec<usize>> = Vec::new();
    // (Spoke ends taken by an earlier gate: another gate shares one only if it has no other.)
    let mut taken: Vec<usize> = Vec::new();
    if let Some(pz) = plaza {
        let targets: std::collections::BTreeSet<(i64, i64)> = mesh.faces[pz].iter().map(|&v| (v as i64, 0)).collect();
        for &g in &gates {
            let start = (g as i64, 0);
            // (In a walled town: the first step is the spoke for which the street, its length
            // stretched by how far the spoke turns off the wall's normal, is shortest; failing a
            // spoke, a side going in off the wall, never a bridge. The gate stands where it was
            // flattened to, so its spoke's turn is measured from there.)
            let mut best: Option<(f64, Vec<(i64, i64)>)> = None;
            if walled && !targets.contains(&start) {
                let out = adj_main.get(&start).map_or(&[][..], |v| v.as_slice());
                let mut first: Vec<((i64, i64), f64)> = out.iter().copied().filter(|x| is_spoke(g, x.0.0 as usize)).collect();
                if first.iter().any(|x| !taken.contains(&(x.0.0 as usize))) {
                    first.retain(|x| !taken.contains(&(x.0.0 as usize)));
                }
                if first.is_empty() {
                    let sides = adj_vertices(&mesh, g);
                    first = out.iter().copied().filter(|x| !ring.contains(&(x.0.0 as usize)) && sides.contains(&(x.0.0 as usize))).collect();
                }
                for &(u, w) in &first {
                    let Some(rest) = shortest(&adj_main, u, &targets).filter(|p| !p.contains(&start)) else { continue };
                    let cost: f64 = rest.windows(2).map(|x| adj_main[&x[0]].iter().find(|y| y.0 == x[1]).map_or(0.0, |y| y.1)).sum();
                    // (Past 20° a spoke costs more, as squaring it may fall short; one whose end
                    // cannot be moved, a corner of the plaza or the castle's or by the river,
                    // keeps its turn and costs twice that.)
                    let ui = u.0 as usize;
                    let a = off_normal(&mesh.pos, g, mesh.pos[g], ui);
                    let fixed = pinned[ui] || plaza_corners.contains(&ui) || castle_face.is_some_and(|cf| mesh.faces[cf].contains(&ui));
                    let c = (w + cost) * (1.0 + a + (if fixed { 4.0 } else { 2.0 }) * (a - 20f64.to_radians()).max(0.0));
                    if best.as_ref().is_none_or(|b| c < b.0) {
                        best = Some((c, std::iter::once(start).chain(rest).collect()));
                    }
                }
            }
            if let Some(path) = best.map(|b| b.1).or_else(|| shortest(if walled { &adj_wall } else { &adj }, start, &targets)) {
                if walled && path.len() >= 2 {
                    taken.push(path[1].0 as usize);
                }
                paths.push(path.iter().map(|k| k.0 as usize).collect());
            }
        }
    }
    // Curve the main streets: the patches along them bend with them.
    let mut keep_v: Vec<usize> = plaza.map(|pz| mesh.faces[pz].clone()).unwrap_or_default();
    if let Some(cf) = castle_face {
        keep_v.extend(mesh.faces[cf].iter().copied());
    }
    // A walled town's spoke is squared to the wall. First the gate is slid along the straight line
    // between its wall neighbours (in its middle 40%) toward the foot of the spoke's inner end, as
    // far as keeps its patches convex, off the water, and no road's approach crossing the wall.
    // Then that end is moved toward the wall's normal at the gate, at least 110 ft in: of the spots
    // tried (on the normal at its own distance in, at its distance along the normal, and part way
    // to each; at most 0.6 of the patch spacing off), the one squarest to the wall that keeps the
    // patches convex and is off the river and wet ground. It is kept there while the street is
    // curved. A corner two gates share is squared to the first.
    let mut spoke_end: Vec<usize> = Vec::new();
    for path in paths.iter().filter(|p| walled && p.len() >= 2 && !ring.contains(&p[1])) {
        let (g, v) = (path[0], path[1]);
        if let Some(nb) = ring_nb.get(&g).filter(|nb| nb.len() == 2)
            && !(pinned[v] || keep_v.contains(&v) || gates.contains(&v) || spoke_end.contains(&v))
        {
            let (a, t) = (mesh.pos[nb[0]], sub(mesh.pos[nb[1]], mesh.pos[nb[0]]));
            let n = mul([-t[1], t[0]], 1.0 / len(t).max(1e-9));
            let dry = |q: P| !site.near_river(q, 0.0) && !site.wet(q);
            let foot = add(a, mul(t, (dot(sub(mesh.pos[v], a), t) / dot(t, t).max(1e-9)).clamp(0.3, 0.7)));
            // (Whether a road's approach to the gate at q crosses the wall off the gate: its own
            // sides too, but for their 30 ft at the gate.)
            let crosses = |q: P| {
                let x = |p: P, q: P, a: P, b: P| {
                    let c = |o: P, x: P, y: P| cross(sub(x, o), sub(y, o));
                    c(p, q, a) * c(p, q, b) < 0.0 && c(a, b, p) * c(a, b, q) < 0.0
                };
                let side = |a: usize, b: usize| match (a == g, b == g) {
                    (true, _) => (lerp(q, mesh.pos[b], (30.0 / dist(q, mesh.pos[b]).max(1e-9)).min(1.0)), mesh.pos[b]),
                    (_, true) => (mesh.pos[a], lerp(q, mesh.pos[a], (30.0 / dist(q, mesh.pos[a]).max(1e-9)).min(1.0))),
                    _ => (mesh.pos[a], mesh.pos[b]),
                };
                (0..ends.len()).filter(|&k| gate_of[k] == Some(g)).any(|k| {
                    let curve = approach_curve(ends[k].0, q, heading_of[k]);
                    ring_edges.iter().map(|&(a, b)| side(a, b)).any(|(a, b)| curve.windows(2).any(|w| x(w[0], w[1], a, b)))
                })
            };
            // (Never beside the castle: its bailey is laid against the wall there.)
            let by_castle = castle_face.is_some_and(|cf| vf[g].iter().any(|&f| next_to(f, cf)));
            let crossed = crosses(mesh.pos[g]);
            if !by_castle
                && let Some(q) = [1.0, 0.75, 0.5, 0.25].into_iter().map(|f| lerp(mesh.pos[g], foot, f)).find(|&q| dry(q) && mesh.can_move(&vf, g, q) && (crossed || !crosses(q)))
            {
                mesh.pos[g] = q;
            }
            let (o, p) = (mesh.pos[g], mesh.pos[v]);
            let d = sub(p, o);
            let n = if dot(n, d) < 0.0 { mul(n, -1.0) } else { n };
            let off = |q: P| libm::atan2(cross(n, sub(q, o)).abs(), dot(n, sub(q, o)));
            let mut best: Option<(f64, f64, P)> = None;
            for along in [len(d), dot(d, n)] {
                let target = add(o, mul(n, along.max(110.0)));
                let to = lerp(p, target, (0.6 * spacing / dist(target, p).max(1e-9)).min(1.0));
                for f in [1.0, 0.85, 0.7, 0.55, 0.4, 0.25] {
                    let q = lerp(p, to, f);
                    let key = (off(q), dist(p, q));
                    if best.is_none_or(|b| key.0 < b.0 - 1e-9 || (key.0 < b.0 + 1e-9 && key.1 < b.1)) && dry(q) && mesh.can_move(&vf, v, q) {
                        best = Some((key.0, key.1, q));
                    }
                }
            }
            if let Some((a, _, q)) = best
                && a < off(p)
            {
                mesh.pos[v] = q;
            }
        }
        spoke_end.push(v);
    }
    for path in &paths {
        mesh.smooth_chain(&vf, path, false, &|v| gates.contains(&v) || pinned[v] || ring.contains(&v) || keep_v.contains(&v) || spoke_end.contains(&v), 2);
    }

    // Bridges carry streets: every crossing link a main street uses gets a bridge along it;
    // then more links (watabou's rule u < 1 − 2·bridged/crossings, spaced out) get a bridge
    // with a short street linking the blocks on either bank.
    let mut bridged: Vec<((usize, usize, usize), bool)> = Vec::new();
    for path in &paths {
        for w in path.windows(2) {
            if let Some(&link) = crossing_links.iter().find(|&&(a, _, b)| (a == w[0] && b == w[1]) || (a == w[1] && b == w[0]))
                && !bridged.iter().any(|(k, _)| *k == link)
            {
                bridged.push((link, true));
            }
        }
    }
    let n_cross = crossing_links.len().max(1);
    for &link in &crossing_links {
        let p = river_crossing(site, mesh.pos[link.0], mesh.pos[link.2]);
        if bridged.iter().any(|((a, _, b), _)| dist(river_crossing(site, mesh.pos[*a], mesh.pos[*b]), p) < 1.5 * spacing) {
            continue;
        }
        if rng.next_f64() < 1.0 - 2.0 * bridged.len() as f64 / n_cross as f64 {
            bridged.push((link, false));
        }
    }
    // The river vertex moves onto the straight bank-to-bank line, so the patch edges on
    // either side run with the street and the blocks leave room for it.
    for &((a, v, b), _) in &bridged {
        mesh.pos[v] = river_crossing(site, mesh.pos[a], mesh.pos[b]);
    }
    let mut main_edges: FastSet<(usize, usize)> = FastSet::default();
    for &((a, v, b), _) in &bridged {
        for (p, q) in [(a, v), (v, b)] {
            main_edges.insert((p.min(q), p.max(q)));
        }
    }
    for path in &paths {
        for w in path.windows(2) {
            main_edges.insert((w[0].min(w[1]), w[0].max(w[1])));
        }
    }
    let main_edge = |a: usize, b: usize| main_edges.contains(&(a.min(b), a.max(b)));
    // Wall runs, and how many towers stand along each stretch (fixed here, so a corner moved
    // later never adds or drops one).
    let mut wall_runs: Vec<(Vec<usize>, bool, Vec<usize>)> = Vec::new();
    for (chain, closed) in &chains {
        let mut chain = chain.clone();
        // A closed ring starts at a plain corner so a gate never falls on the seam.
        if *closed && let Some(k) = chain.iter().position(|v| !gates.contains(v)) {
            chain.rotate_left(k);
        }
        let corners: Vec<(P, bool)> = chain.iter().map(|&v| (mesh.pos[v], gates.contains(&v))).collect();
        let along = wall_pieces_spaced(&corners, *closed, None, wallstyle::WallStyle::of(s).spacing).3;
        wall_runs.push((chain, *closed, along));
    }

    // Patches as planned (wards, districts and their names are decided on these).
    let cells: Vec<Labeled> = (0..total).map(|i| Labeled { pts: mesh.face_pts(i), labels: mesh.labels[i].clone() }).collect();
    let gate_pts: Vec<P> = gates.iter().map(|&g| mesh.pos[g]).collect();
    // Patch edges as vertex pairs (edge e of patch i runs from corner e to e + 1).
    let edge_v = |i: usize, e: usize| (mesh.faces[i][e], mesh.faces[i][(e + 1) % mesh.faces[i].len()]);

    // Wards.
    let mut ward: Vec<Option<Ward>> = vec![None; total];
    let neighbors = |i: usize| -> Vec<usize> { cells[i].labels.iter().filter(|&&j| j >= 0).map(|&j| j as usize).collect() };
    if let Some(pz) = plaza {
        ward[pz] = Some(Ward::Plaza);
    }
    let mut free: Vec<usize> = (0..n_inner).filter(|&i| usable(i) && ward[i].is_none()).collect();
    if let Some(castle) = castle_face {
        ward[castle] = Some(Ward::Castle);
        let nobles: Vec<usize> = neighbors(castle).into_iter().filter(|&j| usable(j) && ward[j].is_none()).take(3).collect();
        for j in nobles {
            ward[j] = Some(Ward::Noble);
        }
    }
    let temples = match s.tier {
        Tier::Town => 1,
        Tier::City => 2,
        _ => 3,
    };
    if let Some(pz) = plaza {
        let chosen: Vec<usize> = neighbors(pz).into_iter().filter(|&j| usable(j) && ward[j].is_none()).take(temples).collect();
        for j in chosen {
            ward[j] = Some(Ward::Temple);
        }
    }
    free.retain(|&i| ward[i].is_none());
    let touches_water = |i: usize| neighbors(i).iter().any(|&j| wet[j]) || cells[i].pts.iter().any(|p| site.wet(*p) || site.near_river(*p, 40.0));
    let mut docks = 0;
    for &i in &free {
        if touches_water(i) && docks < (n_inner / 5).max(1) {
            ward[i] = Some(Ward::Docks);
            docks += 1;
        }
    }
    let gate_patch = |i: usize| gate_pts.iter().any(|g| cells[i].pts.iter().any(|p| dist(*p, *g) < 12.0));
    let on_main = |i: usize| (0..cells[i].pts.len()).any(|e| {
        let (a, b) = edge_v(i, e);
        main_edge(a, b)
    });
    // Where a park may be (generated or set by hand): inside the town, with a street to come in from
    // (a side on a dry patch of the town), its block (10 ft in) 9,000 sq ft or more and 60 ft across,
    // no cliff across it (no grade over 0.6 between points 20 ft apart), mostly dry (15% of it on the
    // water at most).
    let park_refused: Vec<Option<&'static str>> = (0..total)
        .map(|i| {
            let block = || inset(&cells[i].pts, &vec![10.0; cells[i].pts.len()]);
            if i >= n_inner || cells[i].pts.len() < 3 {
                Some(PARK_OUT)
            } else if !cells[i].labels.iter().any(|&j| j >= 0 && usable(j as usize)) {
                Some(PARK_SHUT)
            } else if block().len() < 3 || area(&block()).abs() < 9000.0 || obb(&block()).short < 60.0 {
                Some(PARK_SMALL)
            } else if site.max_grade(&cells[i].pts) > 0.6 {
                Some(PARK_CLIFF)
            } else if site.wet_share(&cells[i].pts) > 0.15 {
                Some(PARK_WET)
            } else {
                None
            }
        })
        .collect();
    // Too steep to build a street front on (the dry ground across a patch's block falling more than
    // 18 in 100, no more than 40): a park, a common on the slope (not the market square or a castle),
    // where a park may be, but not by a river (a gorge's drawn banks stand over the plan). Two at most, none beside another; any other steep block is a terraced
    // garden of a few houses, as before (`build_town`).
    let mut steep = vec![false; total];
    for i in 0..n_inner {
        if (0..total).filter(|&j| steep[j]).count() >= 2 || neighbors(i).iter().any(|&j| steep[j]) {
            continue;
        }
        let grade = || site.dry_grade(&inset(&cells[i].pts, &vec![7.0; cells[i].pts.len()]));
        if usable(i)
            && !matches!(ward[i], Some(Ward::Plaza | Ward::Castle))
            && park_refused[i].is_none()
            && (0.18..=0.4).contains(&grade())
            && site.river_share(&cells[i].pts) <= 0.15
        {
            steep[i] = true;
            ward[i] = Some(Ward::Park);
        }
    }
    free.retain(|&i| ward[i].is_none());
    // Parks: a metropolis has one to three, a city most often one, a town now and then a small
    // one (a green; a village keeps its own), those on slopes counted. Each a whole patch on level
    // ground, off the gates, set back from the main streets and the middle of the town, apart from
    // one another; the first of a city's by its castle and nobles where it can be. A metropolis's or
    // a capital city's first is its palace garden (formal), on a slope or not.
    let quota = match s.tier {
        Tier::Town => (roll(0, 70) < 0.25) as usize,
        Tier::City => (roll(0, 70) < 0.7) as usize,
        Tier::Metropolis => 1 + ((roll(0, 70) * 3.0) as usize).min(2),
        Tier::Village => 0,
    };
    let rich = s.tier >= Tier::City && (s.capital || s.tier == Tier::Metropolis);
    let sloped: Vec<usize> = (0..total).filter(|&i| steep[i]).collect();
    let parks = if rich { quota.saturating_sub(sloped.len()).max(1) } else { quota.saturating_sub(sloped.len()) };
    let mut chosen: Vec<usize> = Vec::new();
    // (A metropolis's first is the patch it has always had for its park, where a park can be.)
    if s.tier == Tier::Metropolis
        && parks > 0
        && let Some(&p) = free.iter().find(|&&i| ward[i].is_none() && len(sites[i]) > 0.5 * r)
        && park_refused[p].is_none()
    {
        ward[p] = Some(Ward::Park);
        chosen.push(p);
    }
    for k in chosen.len()..parks {
        let near_rich = |i: usize| neighbors(i).iter().any(|&j| matches!(ward[j], Some(Ward::Castle | Ward::Noble)));
        let score = |i: usize| {
            let size = area(&cells[i].pts).abs() / (spacing * spacing);
            let off = len(centroid(&cells[i].pts)) / r;
            let fit = if s.tier == Tier::Town { -0.6 * size } else { 0.3 * size.min(1.5) };
            roll(i, 71) + fit + if k == 0 && s.tier >= Tier::City && near_rich(i) { 0.6 } else { 0.0 } - 0.5 * on_main(i) as u8 as f64 - 3.0 * site.dry_grade(&cells[i].pts) - 0.4 * (off - 0.6).abs()
        };
        let pick = free
            .iter()
            .copied()
            .filter(|&i| ward[i].is_none() && cells[i].pts.len() >= 3 && !gate_patch(i) && area(&cells[i].pts).abs() > 0.3 * spacing * spacing && site.dry_grade(&cells[i].pts) < 0.15)
            .filter(|&i| park_refused[i].is_none())
            // (Not on a slope down to a river: its drawn banks stand well over the plan's ground.)
            .filter(|&i| site.river_share(&cells[i].pts) <= 0.15 || site.fall(&cells[i].pts) <= 0.1)
            .filter(|&i| chosen.iter().chain(&sloped).all(|&j| dist(centroid(&cells[i].pts), centroid(&cells[j].pts)) > 2.2 * spacing))
            .max_by(|&a, &b| score(a).total_cmp(&score(b)).then(b.cmp(&a)));
        if let Some(p) = pick {
            ward[p] = Some(Ward::Park);
            chosen.push(p);
        }
    }
    let palace = chosen.first().copied().filter(|_| rich);
    for &i in &free {
        if ward[i].is_some() {
            continue;
        }
        let roll = roll(i, 10);
        ward[i] = Some(if s.tier >= Tier::City && gate_patch(i) && roll < 0.5 {
            Ward::Military
        } else if on_main(i) && roll < 0.65 {
            Ward::Merchant
        } else if roll < 0.45 {
            Ward::Craft
        } else if s.tier == Tier::Metropolis && roll > 0.9 {
            Ward::Noble
        } else {
            Ward::Common
        });
    }
    // Shanty towns and farmland round the town (`shanty::grow`).
    shanty::grow(&cells, &wet, n_inner, pseed, spacing, r_out, &gate_pts, &usable, &neighbors, &mut ward);

    // Districts: neighbouring patches of the same ward merge into groups of 1–5 that share
    // one set of block and lot parameters. No street runs between the patches of a group,
    // so their blocks meet at party walls instead of each reading as a Voronoi cell. Main
    // streets, water and the wall line are never merged across.
    let groupable = |w: Option<Ward>| {
        matches!(w, Some(Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Docks | Ward::Military | Ward::Temple))
    };
    let base_seed = hash2(pseed, 0x6a5e, 7);
    let mut group = vec![usize::MAX; total];
    for start in 0..total {
        if group[start] != usize::MAX || !groupable(ward[start]) || cells[start].pts.len() < 3 {
            continue;
        }
        let mut grng = Pcg32::new(hash2(base_seed, start as i64, 0x6e0), 43);
        group[start] = start;
        let mut members = vec![start];
        let mut k = 0;
        'grow: while k < members.len() {
            let i = members[k];
            k += 1;
            let m = cells[i].pts.len();
            for e in 0..m {
                let nb = cells[i].labels[e];
                if nb < 0 {
                    continue;
                }
                let j = nb as usize;
                let (ka, kb) = edge_v(i, e);
                if group[j] != usize::MAX
                    || ward[j] != ward[start]
                    || wet[j]
                    || cells[j].pts.len() < 3
                    || (j < n_inner) != (start < n_inner)
                    || main_edge(ka, kb)
                {
                    continue;
                }
                // Stop growing with probability (n − 3) / n (so 1–5 patches, mostly 2–4).
                let n = members.len() as f64;
                if n >= 5.0 || grng.next_f64() < (n - 3.0) / n {
                    break 'grow;
                }
                group[j] = start;
                members.push(j);
            }
        }
    }

    // Named districts.
    let crossings: Vec<P> = bridged.iter().map(|&((_, v, _), _)| mesh.pos[v]).collect();
    let edge_w: Vec<Vec<f64>> = (0..total)
        .map(|i| {
            let c = &cells[i];
            let m = c.pts.len();
            (0..m)
                .map(|e| {
                    let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
                    let nb = c.labels[e];
                    let (va, vb) = edge_v(i, e);
                    if site.near_river(lerp(a, b, 0.5), 2.0) || (walled && nb >= 0 && (i < n_inner) != ((nb as usize) < n_inner)) {
                        0.0
                    } else if main_edge(va, vb) {
                        0.9
                    } else {
                        1.0
                    }
                })
                .collect()
        })
        .collect();
    let landing: Vec<bool> = (0..total)
        .map(|i| {
            let c = &cells[i];
            let m = c.pts.len();
            (0..m).any(|e| (c.labels[e] >= 0 && wet[c.labels[e] as usize]) || site.near_river(lerp(c.pts[e], c.pts[(e + 1) % m], 0.5), 2.0))
        })
        .collect();
    let quarters = quarters(s, &QuarterInput { cells: &cells, ward: &ward, n_inner, plaza, castle: castle_face, gates: &gate_pts, crossings: &crossings, edge_w: &edge_w, landing: &landing });
    let mut plan = TownPlan {
        spacing,
        mesh,
        vf,
        n_inner,
        wet,
        face_ok,
        shore: shore_sides,
        enclosed,
        pinned,
        walled,
        wall_runs,
        gates,
        ends,
        heading_of,
        gate_of,
        town_faces,
        plaza_face,
        plaza,
        crossing_links,
        bridged,
        paths,
        main_edges,
        ward,
        group,
        pseed,
        base_seed,
        quarters,
        squares: Vec::new(),
        closes: Vec::new(),
        steep,
        palace,
        park_refused,
    };
    // Its other open spaces, on the plan as it stands.
    (plan.squares, plan.closes) = squares::plan(&plan, site, s.tier);
    plan
}

/// Links across the river channel: per vertex in it, the pair of its neighbours on the two banks
/// most nearly opposite each other, across the stream (a street over it is a bridge).
fn bank_links(mesh: &Mesh, site: &Site, vf: &[Vec<usize>], usable: &dyn Fn(usize) -> bool) -> Vec<(usize, usize, usize)> {
    let bank_side = |p: P| site.nearest_river(p).map_or(0.0, |(c, t, _)| cross(t, sub(p, c)).signum());
    let in_river = |v: usize| site.near_river(mesh.pos[v], 0.0);
    let mut links: Vec<(usize, usize, usize)> = Vec::new();
    for v in 0..mesh.pos.len() {
        if !in_river(v) || !vf[v].iter().any(|&f| usable(f)) {
            continue;
        }
        let nbs: Vec<usize> = adj_vertices(mesh, v).into_iter().filter(|&u| !in_river(u) && vf[u].iter().any(|&f| usable(f))).collect();
        let p = mesh.pos[v];
        let mut best: Option<(f64, usize, usize)> = None;
        for (ia, &a) in nbs.iter().enumerate() {
            for &b in &nbs[ia + 1..] {
                if bank_side(mesh.pos[a]) * bank_side(mesh.pos[b]) >= 0.0 {
                    continue;
                }
                let (da, db) = (sub(mesh.pos[a], p), sub(mesh.pos[b], p));
                let c = dot(da, db) / (len(da) * len(db)).max(1e-9);
                let ab = sub(mesh.pos[b], mesh.pos[a]);
                let across = site.nearest_river(p).is_some_and(|(_, t, _)| dot(ab, t).abs() < 0.5 * len(ab));
                if across && c < -0.6 && best.is_none_or(|x| c < x.0) {
                    best = Some((c, a, b));
                }
            }
        }
        if let Some((_, a, b)) = best {
            links.push((a, v, b));
        }
    }
    links
}
/// Per corner, the corners it is joined to by town patches' sides off the river channel (not
/// `cut`), or by links over it.
fn joins(mesh: &Mesh, site: &Site, vf: &[Vec<usize>], usable: &dyn Fn(usize) -> bool, cut: &dyn Fn(usize, usize) -> bool) -> Vec<Vec<usize>> {
    let in_river = |v: usize| site.near_river(mesh.pos[v], 0.0);
    let mut nbs: Vec<Vec<usize>> = vec![Vec::new(); mesh.pos.len()];
    let mut join = |a: usize, b: usize| {
        if !nbs[a].contains(&b) {
            nbs[a].push(b);
            nbs[b].push(a);
        }
    };
    for face in (0..mesh.faces.len()).filter(|&i| usable(i)).map(|i| &mesh.faces[i]) {
        for e in 0..face.len() {
            let (a, b) = (face[e], face[(e + 1) % face.len()]);
            if !cut(a, b) && !in_river(a) && !in_river(b) {
                join(a, b);
            }
        }
    }
    for (a, _, b) in bank_links(mesh, site, vf, usable) {
        join(a, b);
    }
    nbs
}
