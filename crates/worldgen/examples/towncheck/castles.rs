use crate::prelude::*;

pub(crate) fn castles(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["castles", "castles with an inner curtain", "gate ways"]);
    r.counts(&CASTLE_COUNTS);
    r.counts(&CASTLE_MORE);
    r.counts(&["mottes", "objects on a motte's toe (its last 8 ft)"]);
    r.counts(&["moats: wet", "moats: dry", "castles without a moat", "wet moats under 90% water", "dry moats under 90% difficult", "drawbridges with no deck"]);
    r.counts(&CASTLE_SPOTS);
    r.counts(&TOWN_RING);
    let n = town::layout_count(&ctx.ex.t0);
    for li in 0..n {
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, li);
        if l.site {
            continue;
        }
        castle_spot(ctx, &l, &format!("L{li} {:?}", l.tier), r);
        town_ring(ctx, &l, li, &format!("L{li} {:?}", l.tier), r);
        if l.castles.is_empty() {
            continue;
        }
        let capital = li < ctx.ex.t0.settlements.len() && ctx.ex.t0.settlements[li].capital;
        let tag = format!("L{li} {:?}", l.tier);
        castle_checks(&ctx.ex.world, &ctx.ex.t0, &l, &tag, capital, r);
        motte_objects(ctx, &l, &tag, r);
        moat_checks(ctx, &l, &tag, r);
    }
}

/// v63 castles: moats by castle (wet, dry, none; by tier) and on the battlemap: a wet moat's squares
/// under water (deep in its middle), a dry ditch's difficult, its drawbridge a deck.
pub(crate) fn moat_checks(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
    let Some(m) = l.moats.first() else {
        r.tally("castles without a moat", 1);
        r.tally(&format!("{:?} castles without a moat", l.tier), 1);
        return;
    };
    let kind = if m.wet { "wet" } else { "dry" };
    let far = add(m.bridges[0].at, mul(m.bridges[0].dir, m.bridges[0].span));
    r.add(&format!("moats: {kind}"), m.width, format!("{tag}: {:.0} ft wide, drawbridge {} to {}", m.width, at(m.bridges[0].at), at(far)));
    r.tally(&format!("{:?} castles with a {kind} moat", l.tier), 1);
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    let k = |v: f64| (v / size).floor().max(0.0) as u32;
    let b = m.bbox;
    let (mut band, mut good, mut deep, mut deck) = (0, 0, 0, 0);
    for ky in k(b[1])..=k(b[3]) {
        for kx in k(b[0])..=k(b[2]) {
            let c = ctx.chunk(kx, ky);
            for q in 0..SQ * SQ {
                let p = [kx as f64 * size + ((q % SQ) as f64 + 0.5) * SQUARE_FT, ky as f64 * size + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                let Some(s) = m.spot(p) else { continue };
                if !s.inside() {
                    continue;
                }
                if m.bridged(p) {
                    deck += (c.road_halo[(q / SQ + 1) * (SQ + 2) + q % SQ + 1] & 0x80 != 0) as usize;
                    continue;
                }
                band += 1;
                good += if m.wet { (c.water_level[q] > c.height[q]) as usize } else { (c.edges[q] & 16 != 0 || s.outer < town::moat::BANK_FT) as usize };
                deep += (c.surface[q] == Surface::Deep) as usize;
            }
        }
    }
    r.tally("moat squares", band);
    r.tally(&format!("moat squares deep water ({kind} moats)"), deep);
    if good * 10 < band * 9 {
        r.add(&format!("{kind} moats under 90% {}", if m.wet { "water" } else { "difficult" }), (band - good) as f64, format!("{tag}: {good} of {band} at {}", at(m.bridges[0].at)));
    }
    if deck == 0 {
        r.add("drawbridges with no deck", 1.0, format!("{tag}: at {}", at(m.bridges[0].at)));
    }
}

pub(crate) const TOWN_RING: [&str; 7] = [
    "buildings over a river channel",
    "built blocks centred on a river channel",
    "walled towns",
    "town-wall ring sides",
    "town-wall ring sides with 15 ft+ of dry ground and no wall (gates aside)",
    "quays",
    "quays with no water beyond them (a paved strip on dry ground)",
];

/// A town's quays all on the water, its buildings and blocks off the rivers' channels; and a walled
/// town's ring (the sides of its patches facing out of town, not those round water the town encloses):
/// every dry stretch of it has the wall on it (a town wall or a castle's curtain there), but at its gates.
pub(crate) fn town_ring(ctx: &Ctx, l: &Layout, li: usize, tag: &str, r: &mut Report) {
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    // (The rivers' channels as `town` follows them.)
    let reach = 2.0 * l.radius + 1000.0;
    let mut river: Vec<(P, P, f64)> = Vec::new();
    for (ri, k) in t0.rivers.segments_near(l.center[0] - reach, l.center[1] - reach, l.center[0] + reach, l.center[1] + reach, 0.0) {
        let rc = &t0.rivers.rivers[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(2.0, 1200.0) as usize;
        let pts: Vec<(P, f64)> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft)).map(|c| (c.p, c.w)).collect();
        river.extend(pts.windows(2).map(|s| (s[0].0, s[1].0, 0.5 * s[0].1.max(s[1].1))));
    }
    let by_water = |p: P| {
        river.iter().any(|&(a, b, hw)| seg_dist(p, a, b) <= hw + 25.0)
            || wet(p)
            || [12.0, 25.0].iter().any(|&d| {
                (0..8).any(|k| {
                    let a = k as f64 * 0.785;
                    wet(add(p, [d * libm::cos(a), d * libm::sin(a)]))
                })
            })
    };
    // (Water beyond a side at `mid`, out to 150 ft: a patch of the sea, a lake or a harbour.)
    let water_beyond = |mid: P, out: P| [0.0, 30.0, 60.0, 100.0, 150.0].iter().any(|&d| by_water(add(mid, mul(out, d))));
    let in_channel = |p: P| river.iter().any(|&(a, b, hw)| seg_dist(p, a, b) < hw);
    let over_channel = |poly: &[P]| poly.len() >= 3 && (poly.iter().any(|&p| in_channel(p)) || in_channel(centroid(poly)));
    for b in l.buildings.iter().filter(|b| over_channel(&b.poly)) {
        r.add("buildings over a river channel", area(&b.poly).abs(), format!("{tag}: {} at {}", b.id, at(centroid(&b.poly))));
    }
    // (A block's corner may reach into the channel where the bank bends: its middle may not.)
    for b in l.blocks.iter().filter(|b| b.len() >= 3 && in_channel(centroid(b))) {
        r.add("built blocks centred on a river channel", area(b).abs(), format!("{tag}: block at {}", at(centroid(b))));
    }
    for q in l.plazas.iter().filter(|q| q.kind == PlazaKind::Quay && q.pts.len() == 4) {
        r.tally("quays", 1);
        let out = mul(sub(q.pts[0], q.pts[3]), 1.0 / dist(q.pts[0], q.pts[3]).max(1e-9));
        let mid = lerp(q.pts[0], q.pts[1], 0.5);
        if !water_beyond(mid, out) {
            r.add("quays with no water beyond them (a paved strip on dry ground)", dist(q.pts[0], q.pts[1]), format!("{tag}: {:.0} ft at {}", dist(q.pts[0], q.pts[1]), at(mid)));
        }
    }
    if l.walls.is_empty() {
        return;
    }
    let Ok(plan) = town::wards::plan_json(world, t0, li) else { return };
    if plan["walls"]["built"] != true {
        return;
    }
    r.tally("walled towns", 1);
    let pt = |v: &serde_json::Value| -> P { [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)] };
    let corners: HashMap<u64, P> = plan["corners"].as_array().into_iter().flatten().filter_map(|c| Some((c["corner"].as_u64()?, pt(&c["at"])))).collect();
    // (Each patch's sides, and whether its corners run anticlockwise.)
    let mut sides: Vec<(u64, u64, bool)> = Vec::new();
    for p in plan["patches"].as_array().into_iter().flatten().filter(|p| p["in_town"] == true) {
        let cs: Vec<u64> = p["corners"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()).collect();
        let ccw = area(&cs.iter().filter_map(|c| corners.get(c).copied()).collect::<Vec<_>>()) > 0.0;
        sides.extend((0..cs.len()).map(|k| (cs[k], cs[(k + 1) % cs.len()], ccw)));
    }
    let shared: std::collections::HashSet<(u64, u64)> = sides.iter().map(|s| (s.0, s.1)).collect();
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let by_gate = |p: P| l.gates.iter().any(|g| dist(*g, p) < 60.0);
    // (Ground the town encloses, a river or lake patch amid it: every way out of it, 16 of them,
    // meets a side of the town's patches within 2,000 ft.)
    let town_sides: Vec<(P, P)> = sides.iter().filter_map(|(a, b, _)| Some((*corners.get(a)?, *corners.get(b)?))).collect();
    let crosses = |p: P, q: P, a: P, b: P| {
        let (o1, o2) = (cross(sub(q, p), sub(a, p)), cross(sub(q, p), sub(b, p)));
        let (o3, o4) = (cross(sub(b, a), sub(p, a)), cross(sub(b, a), sub(q, a)));
        o1 * o2 < 0.0 && o3 * o4 < 0.0
    };
    let enclosed = |p: P| {
        (0..16).all(|k| {
            let a = k as f64 * std::f64::consts::TAU / 16.0;
            let q = add(p, [2000.0 * libm::cos(a), 2000.0 * libm::sin(a)]);
            town_sides.iter().any(|&(s, t)| crosses(p, q, s, t))
        })
    };
    for &(a, b, ccw) in sides.iter().filter(|(a, b, _)| !shared.contains(&(*b, *a))) {
        let (Some(&pa), Some(&pb)) = (corners.get(&a), corners.get(&b)) else { continue };
        r.tally("town-wall ring sides", 1);
        let u = mul(sub(pb, pa), 1.0 / dist(pa, pb).max(1e-9));
        let out = if ccw { [u[1], -u[0]] } else { [-u[1], u[0]] };
        if water_beyond(lerp(pa, pb, 0.5), out) {
            continue;
        }
        let (mut bare, mut run, mut first) = (0.0f64, 0.0, None);
        for (p, _) in samples(&[pa, pb], 5.0) {
            // (Behind a bastion: its gorge, the bastion's walls round it.)
            // (A round wall's curtains bowed out by up to 45 ft (Steppe 70), slice 2e: within 75 ft; or,
            // where a notch of the ring was bridged and bowed, the wall met out along the side's outward
            // normal within 200 ft.)
            let beyond = add(p, mul(out, 200.0));
            if by_gate(p) || by_water(p) || walls.iter().any(|&(q, s)| seg_dist(p, q, s) <= 75.0 || crosses(p, beyond, q, s)) || l.bastions.iter().any(|q| depth(q, p) > -8.0) {
                run = 0.0;
            } else {
                run += 5.0;
                bare = bare.max(run);
                first.get_or_insert(p);
            }
        }
        if bare >= 15.0 && !enclosed(add(lerp(pa, pb, 0.5), mul(out, 20.0))) {
            r.add("town-wall ring sides with 15 ft+ of dry ground and no wall (gates aside)", bare, format!("{tag}: {bare:.0} ft from {}", at(first.unwrap_or(pa))));
        }
    }
}

// The worst walled towns by main-street hugging over every world (`streetsin`): ft and where.
thread_local! {
    static HUGGERS: std::cell::RefCell<Vec<(f64, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `streetsin` (R7): each walled town's main streets (`Layout::streets`, gate to market square). Hugging: the
/// length (5-ft steps, past its first 40 ft, which a street square to the wall needs to get 30 ft off it) whose
/// centre line is within 30 ft of the town wall's inner face (its centre line less half its thickness); along:
/// the part of that heading within 45 degrees of the nearest wall piece. Straight run: from the gate to the start
/// of the first piece of the street (3 ft or longer) heading more than 20 degrees off the gate's inward normal (the
/// line best fitting the wall within 50 ft of the gate, turned toward the walls' mean point). Off square: the chord from the gate to the
/// street's point 100 ft along (its end if shorter) against that normal.
pub(crate) fn streets_in(ctx: &mut Ctx, label: &str, r: &mut Report) {
    r.counts(&["walled towns", "main streets", "main streets hugging the wall", "main streets hugging and running along the wall", "main streets straight for under 100 ft (longer streets)", "main streets under 100 ft long", "main streets from an open edge (no wall at the gate)", "main streets' first 100 ft over 20 deg off square", "main streets' first 100 ft over 30 deg off square"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let (mut hug_all, mut along_all) = (0.0f64, 0.0f64);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.walls.is_empty() || l.streets.is_empty() {
            continue;
        }
        let castle = Castle::of(&l);
        let walls: Vec<&[P]> = match &castle {
            Some(c) => c.town.clone(),
            None => l.walls.iter().map(|w| w.as_slice()).collect(),
        };
        let segs: Vec<(P, P)> = walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).filter(|s| dist(s.0, s.1) > 1e-6).collect();
        if segs.is_empty() {
            continue;
        }
        r.tally("walled towns", 1);
        let half = 0.5 * l.wall_style.thickness();
        // (The town's middle, to orient each gate's normal inward: the walls' points averaged.)
        let pts: Vec<P> = walls.iter().flat_map(|w| w.iter().copied()).collect();
        let centre = mul(pts.iter().fold([0.0, 0.0], |a, &p| add(a, p)), 1.0 / pts.len() as f64);
        let name = worldgen::agent::feature_of_layout(world, t0, li).and_then(|id| worldgen::agent::name_of(world, t0, &id)).unwrap_or_else(|| "?".into());
        let tag = format!("L{li} {name}");
        let (mut town_hug, mut worst_street) = (0.0f64, String::new());
        for (si, st) in l.streets.iter().enumerate() {
            r.tally("main streets", 1);
            let total: f64 = st.windows(2).map(|w| dist(w[0], w[1])).sum();
            let (mut hug, mut along, mut s) = (0.0f64, 0.0f64, 0.0f64);
            let mut first_hug: Option<P> = None;
            for w in st.windows(2) {
                let d = dist(w[0], w[1]);
                if d < 1e-6 {
                    continue;
                }
                let u = mul(sub(w[1], w[0]), 1.0 / d);
                let n = (d / 5.0).ceil().max(1.0) as usize;
                let step = d / n as f64;
                for k in 0..n {
                    let m = lerp(w[0], w[1], (k as f64 + 0.5) / n as f64);
                    let sm = s + (k as f64 + 0.5) * step;
                    if sm < 40.0 {
                        continue;
                    }
                    let (near, wd) = segs.iter().map(|&(a, b)| (seg_dist(m, a, b), (a, b))).min_by(|x, y| x.0.total_cmp(&y.0)).unwrap();
                    if near - half <= 30.0 {
                        hug += step;
                        first_hug.get_or_insert(m);
                        let wu = mul(sub(wd.1, wd.0), 1.0 / dist(wd.0, wd.1));
                        if dot(u, wu).abs() > std::f64::consts::FRAC_1_SQRT_2 {
                            along += step;
                        }
                    }
                }
                s += d;
            }
            if hug > 0.0 {
                r.add("main streets hugging the wall", hug, format!("{tag} street {si}: {hug:.0} ft from {}", at(first_hug.unwrap_or(st[0]))));
            }
            if along > 0.0 {
                r.add("main streets hugging and running along the wall", along, format!("{tag} street {si}: {along:.0} ft"));
            }
            r.value("main streets' hugging ft", hug, format!("{tag} street {si}"));
            r.value("main streets' along-the-wall ft", along, format!("{tag} street {si}"));
            hug_all += hug;
            along_all += along;
            if hug > 0.0 && (worst_street.is_empty() || hug > town_hug) {
                worst_street = format!("street {si} {hug:.0} ft");
            }
            town_hug += hug;
            // (The gate's inward normal: across the line the wall within 50 ft of it best follows, its
            // points every 2 ft taken (the gate towers' short pieces barely turn it).)
            let g = st[0];
            let near: Vec<P> = segs
                .iter()
                .filter(|(a, b)| seg_dist(g, *a, *b) < 50.0)
                .flat_map(|&(a, b)| {
                    let n = (dist(a, b) / 2.0).ceil().max(1.0) as usize;
                    (0..=n).map(move |j| lerp(a, b, j as f64 / n as f64))
                })
                .filter(|p| dist(*p, g) < 50.0)
                .collect();
            let m = mul(near.iter().fold([0.0, 0.0], |a, &p| add(a, p)), 1.0 / near.len().max(1) as f64);
            let (sxx, syy, sxy) = near.iter().fold((0.0, 0.0, 0.0), |c, p| {
                let d = sub(*p, m);
                (c.0 + d[0] * d[0], c.1 + d[1] * d[1], c.2 + d[0] * d[1])
            });
            if near.len() < 2 || sxx + syy < 1e-6 {
                r.add("main streets from an open edge (no wall at the gate)", 1.0, format!("{tag} street {si} at {}", at(g)));
                continue;
            }
            let th = 0.5 * libm::atan2(2.0 * sxy, sxx - syy);
            let wu = [libm::cos(th), libm::sin(th)];
            let mut nrm = [-wu[1], wu[0]];
            if dot(nrm, sub(centre, g)) < 0.0 {
                nrm = mul(nrm, -1.0);
            }
            // (Squareness: the chord over the first 100 ft (or the whole street) against the inward normal.)
            let (mut s, mut p100) = (0.0f64, st[st.len() - 1]);
            for w in st.windows(2) {
                let d = dist(w[0], w[1]);
                if s + d >= 100.0 {
                    p100 = lerp(w[0], w[1], (100.0 - s) / d.max(1e-9));
                    break;
                }
                s += d;
            }
            if dist(p100, g) > 1e-6 {
                let c = dot(mul(sub(p100, g), 1.0 / dist(p100, g)), nrm).clamp(-1.0, 1.0);
                let off = libm::acos(c).to_degrees();
                r.value("main streets' first 100 ft, deg off square", off, format!("{tag} street {si}"));
                if off > 20.0 {
                    r.add("main streets' first 100 ft over 20 deg off square", off, format!("{tag} street {si}: {off:.0} deg at {}", at(g)));
                }
                if off > 20.0 && std::env::var("STREETS_OFF").is_ok() {
                    eprintln!("off {label} {tag} street {si}: {off:.0} deg, hug {hug:.0} ft, at {}", at(g));
                }
                if off > 30.0 {
                    r.add("main streets' first 100 ft over 30 deg off square", off, format!("{tag} street {si}: {off:.0} deg at {}", at(g)));
                }
            }
            if std::env::var("STREETS_DUMP").is_ok_and(|v| v == tag || v == "all") {
                eprintln!("{tag} street {si}: gate {g:?} wall dir {wu:?} normal {nrm:?} half {half}");
                for &(a, b) in segs.iter().filter(|(a, b)| seg_dist(g, *a, *b) < 50.0) {
                    eprintln!("  wall piece {a:?} -> {b:?} ({:.0} ft)", dist(a, b));
                }
                for w in st.windows(2) {
                    let d = dist(w[0], w[1]);
                    let wd = segs.iter().map(|&(a, b)| seg_dist(w[1], a, b)).fold(f64::MAX, f64::min);
                    eprintln!("  -> {:?} len {d:.0} cos {:.2} wall {wd:.0}", w[1], dot(mul(sub(w[1], w[0]), 1.0 / d.max(1e-9)), nrm));
                }
            }
            let (mut run, mut bent) = (0.0f64, false);
            for w in st.windows(2) {
                let d = dist(w[0], w[1]);
                if d >= 3.0 && dot(mul(sub(w[1], w[0]), 1.0 / d), nrm) < libm::cos(20f64.to_radians()) {
                    bent = true;
                    break;
                }
                run += d;
            }
            r.value("main streets' straight run ft", run, format!("{tag} street {si}"));
            if total < 100.0 {
                r.add("main streets under 100 ft long", 100.0 - total, format!("{tag} street {si}: {total:.0} ft"));
            } else if bent && run < 100.0 {
                r.add("main streets straight for under 100 ft (longer streets)", 100.0 - run, format!("{tag} street {si}: {run:.0} ft"));
            }
        }
        if town_hug > 0.0 {
            HUGGERS.with(|h| h.borrow_mut().push((town_hug, format!("{label} {tag}: {town_hug:.0} ft (worst {worst_street})"))));
        }
    }
    r.measure("hugging ft, all streets", false, hug_all, label.to_string());
    r.measure("along the wall ft, all streets", false, along_all, label.to_string());
}

/// `streetsin`'s line over every world: hugging and along-the-wall ft summed, and the worst 5 towns.
pub(crate) fn streets_in_summary(all: &Report) {
    let count = |label: &str| all.rows.iter().find(|r| r.0 == label).map_or(0, |r| if let Row::Count { n, .. } = r.1 { n } else { 0 });
    let sum = |label: &str| all.rows.iter().find(|r| r.0 == label).map_or(0.0, |r| if let Row::Values { v } = &r.1 { v.iter().map(|x| x.0).sum() } else { 0.0 });
    let median = |label: &str| {
        let mut v: Vec<f64> = all.rows.iter().find(|r| r.0 == label).map_or(Vec::new(), |r| if let Row::Values { v } = &r.1 { v.iter().map(|x| x.0).collect() } else { Vec::new() });
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    HUGGERS.with(|h| {
        let mut h = h.borrow_mut();
        h.sort_by(|a, b| b.0.total_cmp(&a.0));
        say!(
            "streetsin summary: {} walled towns, {} main streets; hugging {:.0} ft ({} streets), of which along the wall {:.0} ft ({} streets); straight under 100 ft: {}; under 100 ft long: {}; open-edge gates: {}; first 100 ft off square: median {:.1} deg, over 20 deg {}, over 30 deg {}",
            count("walled towns"),
            count("main streets"),
            sum("main streets' hugging ft"),
            count("main streets hugging the wall"),
            sum("main streets' along-the-wall ft"),
            count("main streets hugging and running along the wall"),
            count("main streets straight for under 100 ft (longer streets)"),
            count("main streets under 100 ft long"),
            count("main streets from an open edge (no wall at the gate)"),
            median("main streets' first 100 ft, deg off square"),
            count("main streets' first 100 ft over 20 deg off square"),
            count("main streets' first 100 ft over 30 deg off square")
        );
        for (_, w) in h.iter().take(5) {
            say!("    {w}");
        }
    });
}

pub(crate) const CASTLE_SPOTS: [&str; 19] = [
    "town castles (planned or set)",
    "castle patches cut, a noble block left",
    "cuts skipped: patch too small",
    "cuts skipped: a side on the wall",
    "cuts skipped: leftover too small or narrow",
    "cuts skipped: castle's part too narrow or sharp",
    "cuts undone: no keep or a sharp ring in the castle's part",
    "castles with free-standing yard buildings",
    "free-standing yard buildings",
    "castles on the market square's patch",
    "town castles with no bailey",
    "interior castles (no bailey corner within 50 ft of the town wall)",
    "bailey squares",
    "bailey squares paved (street or cobbles)",
    "baileys over 5% paved",
    "castles on raised ground (8 ft over the town, standing 6 ft over its foot)",
    "castles with water at their middle",
    "castles on a spur (6 ft over its foot)",
    "castles on a spur as planned (5 ft over its foot)",
];

/// A town's castle patch: not the market square's, with a bailey; inside the town or on its wall;
/// its bailey not paved; the bailey's size against its keep's.
pub(crate) fn castle_spot(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
    let Some(cp) = l.castle_patch else { return };
    r.tally("town castles (planned or set)", 1);
    let c = l.center;
    // (Where: the bailey's middle, else the town's.)
    let c = Castle::of(l).map_or(c, |k| centroid(&k.ring));
    let cut = ["", "castle patches cut, a noble block left", "cuts skipped: patch too small", "cuts skipped: a side on the wall", "cuts skipped: leftover too small or narrow", "cuts skipped: castle's part too narrow or sharp", "cuts undone: no keep or a sharp ring in the castle's part"];
    if let Some(k) = cut.get(l.castle_cut as usize).filter(|k| !k.is_empty()) {
        r.add(k, 1.0, format!("{tag}: patch {cp}, castle at {}", at(c)));
    }
    if l.castle_yard > 0 {
        r.add("castles with free-standing yard buildings", l.castle_yard as f64, format!("{tag}: {}, castle at {}", l.castle_yard, at(c)));
        r.tally("free-standing yard buildings", l.castle_yard as usize);
    }
    if l.plaza_patch == Some(cp) {
        r.add("castles on the market square's patch", 1.0, format!("{tag}: patch {cp} (town at {})", at(c)));
    }
    let c = l.center;
    let Some(castle) = Castle::of(l) else {
        r.add("town castles with no bailey", 1.0, format!("{tag}: patch {cp} (town at {})", at(c)));
        return;
    };
    let ring = &castle.ring;
    let mid = centroid(ring);
    let near_wall = ring.iter().any(|p| castle.town.iter().any(|w| w.windows(2).any(|s| seg_dist(*p, s[0], s[1]) < 50.0)));
    if !near_wall {
        r.add("interior castles (no bailey corner within 50 ft of the town wall)", 1.0, format!("{tag}: bailey at {}", at(mid)));
    }
    let keep: f64 = l.buildings.iter().filter(|b| b.ward == Ward::Castle && is_keep(b) && contains(ring, centroid(&b.poly))).map(|b| area(&b.poly).abs()).fold(0.0, f64::max);
    if keep > 0.0 {
        r.value("bailey area / keep area", area(ring).abs() / keep, format!("{tag} at {}", at(mid)));
        r.value("keep area (sq ft)", keep, format!("{tag} at {}", at(mid)));
    }
    // (Keep kinds, and the bailey's trades: stables and a smithy in every castle.)
    for b in l.buildings.iter().filter(|b| b.ward == Ward::Castle && contains(ring, centroid(&b.poly))) {
        if let Some(k) = b.keep {
            r.tally(&format!("keeps: {k:?}"), 1);
        }
    }
    let has = |key: &str| l.buildings.iter().any(|b| b.ward == Ward::Castle && contains(ring, centroid(&b.poly)) && b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == key));
    for key in ["stables", "blacksmith"] {
        if !has(key) {
            r.add(&format!("town castles with no {key}"), area(ring).abs(), format!("{tag}: bailey of {:.0} sq ft at {}", area(ring).abs(), at(mid)));
        }
    }
    // (171: the castle's ground against its town's: its bailey's mean height over the mean of its
    // town's buildings (on the ground it was planned on, with its river valleys, both), and over
    // its foot, the dry ground 120 ft out from its curtain's corners (on the battlemap with its moat
    // and motte; on the ground planned).)
    let t0 = &ctx.ex.t0;
    let lattice = ctx.ex.world.geom.spacing_ft(ctx.ex.world.geom.first_refine_level.saturating_sub(1));
    let b = l.buildings.iter().fold(bbox(ring), |a, b| bbox(&b.poly).iter().enumerate().fold(a, |mut a, (k, v)| {
        a[k] = if k < 2 { a[k].min(*v) } else { a[k].max(*v) };
        a
    }));
    let rect = [b[0] - 800.0, b[1] - 800.0, b[2] + 800.0, b[3] + 800.0];
    let planned = worldgen::lod::plan_ground::PlanGround::new(t0, rect, rect, lattice, worldgen::lod::plan_ground::VALLEY_CELL);
    let h0 = |p: P| planned.ground(p[0], p[1], false) + planned.depth(p[0], p[1]);
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    let town: Vec<f64> = l.buildings.iter().filter(|b| b.ward != Ward::Castle).map(|b| h0(centroid(&b.poly))).collect();
    let raise = town.is_empty().then_some(0.0).unwrap_or_else(|| (ring.iter().map(|p| h0(*p)).sum::<f64>() + h0(mid)) / (ring.len() + 1) as f64 - town.iter().sum::<f64>() / town.len().max(1) as f64);
    let foot0 = ring.iter().map(|p| h0(add(*p, mul(sub(*p, mid), 120.0 / len(sub(*p, mid)).max(1.0))))).sum::<f64>() / ring.len() as f64;
    let over0 = (ring.iter().map(|p| h0(*p)).sum::<f64>() + h0(mid)) / (ring.len() + 1) as f64 - foot0;
    let mut ground = |p: P| {
        let (kx, ky) = ((p[0] / size).floor().max(0.0) as u32, (p[1] / size).floor().max(0.0) as u32);
        let c = ctx.chunk(kx, ky);
        let (i, j) = ((((p[0] - kx as f64 * size) / SQUARE_FT) as usize).min(SQ - 1), (((p[1] - ky as f64 * size) / SQUARE_FT) as usize).min(SQ - 1));
        let q = j * SQ + i;
        (c.height[q] as f64, c.water_level[q] > c.height[q])
    };
    let bailey = (ring.iter().map(|p| ground(*p).0).sum::<f64>() + ground(mid).0) / (ring.len() + 1) as f64;
    let out: Vec<f64> = ring.iter().map(|p| ground(add(*p, mul(sub(*p, mid), 120.0 / len(sub(*p, mid)).max(1.0))))).filter(|g| !g.1).map(|g| g.0).collect();
    let fall = bailey - out.iter().sum::<f64>() / out.len().max(1) as f64;
    if std::env::var("TC_CASTLE_GROUND").is_ok() {
        eprintln!("CASTLE171 {tag} at {}: {raise:.1} over the town, {fall:.1} over its foot, as planned {over0:.1}", at(mid));
    }
    r.value("castle height over the town's mean (ft)", raise, format!("{tag} at {}", at(mid)));
    r.value("castle height over its foot, 120 ft out from its curtain (ft)", fall, format!("{tag} at {}", at(mid)));
    if raise >= 8.0 && fall >= 6.0 {
        r.add("castles on raised ground (8 ft over the town, standing 6 ft over its foot)", raise, format!("{tag} at {}", at(mid)));
    }
    if fall >= 6.0 {
        r.add("castles on a spur (6 ft over its foot)", fall, format!("{tag} at {}", at(mid)));
    }
    // (The ground as planned, before the castle's own moat and motte: 5 ft over its foot.)
    r.value("castle height over its foot as planned (ft)", over0, format!("{tag} at {}", at(mid)));
    if over0 >= 5.0 {
        r.add("castles on a spur as planned (5 ft over its foot)", over0, format!("{tag} at {}", at(mid)));
    }
    if (0..=8).map(|k| [mid[0] + 20.0 * (k % 3) as f64 - 20.0, mid[1] + 20.0 * (k / 3) as f64 - 20.0]).any(|q| ground(q).1) {
        r.add("castles with water at their middle", 1.0, format!("{tag} at {}", at(mid)));
    }
    let w = ctx.window(bbox(ring));
    if !w.objects.iter().any(|(p, name)| name == "well" && contains(ring, *p)) {
        r.add("town castles with no well", area(ring).abs(), format!("{tag}: bailey of {:.0} sq ft at {}", area(ring).abs(), at(mid)));
    }
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    let (kx0, ky0) = ((w.ox / size) as u32, (w.oy / size) as u32);
    let (mut inside, mut paved) = (0, 0);
    for k in 0..w.nx * w.ny {
        let p = w.centre(k);
        if !contains(ring, p) {
            continue;
        }
        let (i, j) = (k % w.nx, k / w.nx);
        let ch = ctx.chunk(kx0 + (i / SQ) as u32, ky0 + (j / SQ) as u32);
        inside += 1;
        let q = (j % SQ) * SQ + i % SQ;
        paved += (ch.urban[q] == URBAN_STREET || ch.surface[q] == Surface::Cobble) as usize;
    }
    r.tally("bailey squares", inside);
    r.tally("bailey squares paved (street or cobbles)", paved);
    if paved * 20 > inside {
        r.add("baileys over 5% paved", paved as f64 / inside.max(1) as f64, format!("{tag}: {paved} of {inside} squares at {}", at(mid)));
    }
}

// ---- Play reach. ----

pub(crate) fn reach(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&[
        "castles flooded",
        "castle buildings checked",
        "castle buildings cut off",
        "outer gates not reached",
        "bailey squares open",
        "bailey squares sealed",
        "baileys over 3% sealed",
    ]);
    r.counts(&["parks flooded", "park squares open", "park squares sealed", "parks with sealed squares"]);
    let n = town::layout_count(&ctx.ex.t0);
    for li in 0..n {
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, li);
        if !l.castles.is_empty() && !l.site {
            castle_reach(ctx, &l, &format!("L{li} {:?}", l.tier), r);
        }
        for (pi, park) in l.parks.iter().enumerate() {
            let b = bbox(&park.pts);
            let w = ctx.window([b[0] - 60.0, b[1] - 60.0, b[2] + 60.0, b[3] + 60.0]);
            let in_park: Vec<bool> = (0..w.nx * w.ny).map(|k| contains(&park.pts, w.centre(k))).collect();
            let seen = w.flood(|k| !in_park[k] || w.edge(k));
            let sealed: Vec<bool> = (0..w.nx * w.ny).map(|k| in_park[k] && !w.block[k] && !seen[k]).collect();
            let n_sealed = sealed.iter().filter(|s| **s).count();
            r.tally("parks flooded", 1);
            r.tally("park squares open", (0..w.nx * w.ny).filter(|&k| in_park[k] && !w.block[k]).count());
            r.tally("park squares sealed", n_sealed);
            if n_sealed > 0 {
                let p = w.pockets(&sealed);
                r.add(
                    "parks with sealed squares",
                    n_sealed as f64,
                    format!("L{li} park {pi} ({:?}): {n_sealed} squares sealed, the largest pocket {} at {}", park.kind, p[0].0, at(p[0].1)),
                );
            }
        }
    }
}

pub(crate) fn castle_reach(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
    let Some(c) = Castle::of(l) else { return };
    let (cc, cr) = l.castles[0];
    let w = ctx.window([cc[0] - cr - 60.0, cc[1] - cr - 60.0, cc[0] + cr + 60.0, cc[1] + cr + 60.0]);
    // From outside: the window's edge and its streets.
    let seen = w.flood(|k| w.street[k] || w.edge(k));
    let bailey: Vec<bool> = (0..w.nx * w.ny).map(|k| contains(&c.ring, w.centre(k))).collect();
    let open = (0..w.nx * w.ny).filter(|&k| bailey[k] && !w.block[k]).count();
    let sealed: Vec<bool> = (0..w.nx * w.ny).map(|k| bailey[k] && !w.block[k] && !seen[k]).collect();
    let n_sealed = sealed.iter().filter(|s| **s).count();
    r.tally("castles flooded", 1);
    r.tally("bailey squares open", open);
    r.tally("bailey squares sealed", n_sealed);
    if n_sealed > 0 {
        let p = w.pockets(&sealed);
        r.measure("largest sealed pocket in a bailey (squares)", false, p[0].0 as f64, format!("{tag} at {}", at(p[0].1)));
        if n_sealed * 100 > 3 * open {
            r.add("baileys over 3% sealed", n_sealed as f64, format!("{tag}: {n_sealed} of {open} squares sealed, the largest pocket {} at {}", p[0].0, at(p[0].1)));
        }
    }
    if let Some(g) = c.outer_gate {
        let reached = w.index(g).is_some_and(|k| seen[k] || w.around(k).any(|m| seen[m]));
        if !reached {
            r.add("outer gates not reached", 1.0, format!("{tag}: the outer gate at {}", at(g)));
        }
    }
    // Castle buildings in the bailey: some open square within 6 ft of the outline reached.
    for b in l.buildings.iter().filter(|b| b.ward == Ward::Castle && b.poly.iter().all(|p| depth(&c.ring, *p) > -10.0)) {
        let bb = bbox(&b.poly);
        let (i0, j0) = (w.index([bb[0] - 6.0, bb[1] - 6.0]), w.index([bb[2] + 6.0, bb[3] + 6.0]));
        let (Some(k0), Some(k1)) = (i0, j0) else { continue };
        let near: Vec<usize> = (k0 / w.nx..=k1 / w.nx)
            .flat_map(|j| (k0 % w.nx..=k1 % w.nx).map(move |i| j * w.nx + i))
            .filter(|&k| {
                let p = w.centre(k);
                !contains(&b.poly, p) && edge_dist(&b.poly, p) < 6.0
            })
            .collect();
        if near.is_empty() {
            continue;
        }
        r.tally("castle buildings checked", 1);
        if !near.iter().any(|&k| seen[k] && !w.block[k]) {
            r.add(
                "castle buildings cut off",
                1.0,
                format!("{tag}: b:{}:{} ({}) at {}", l.index, b.id, b.func.map_or("home", |f| town::catalog::CATALOG[f as usize].name), at(centroid(&b.poly))),
            );
        }
    }
}

// ---- Interiors. ----

/// A layout's roofed buildings by 100-ft bucket, for "does this door open into a neighbour's wall".
pub(crate) fn building_buckets(l: &Layout) -> HashMap<(i64, i64), Vec<usize>> {
    let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (k, b) in l.buildings.iter().enumerate().filter(|(_, b)| b.structure == town::Structure::Roofed) {
        let r = bbox(&b.poly);
        for by in (r[1] / 100.0).floor() as i64..=(r[3] / 100.0).floor() as i64 {
            for bx in (r[0] / 100.0).floor() as i64..=(r[2] / 100.0).floor() as i64 {
                buckets.entry((bx, by)).or_default().push(k);
            }
        }
    }
    buckets
}

/// The corridor rule on a floor of 8 rooms or more with a hall or corridor: the most rooms a room is
/// from one (through the doors; `front`: the room the front door opens into counted as one too),
/// with the way to it. `None` where the rule doesn't apply.
pub(crate) fn corridor_hops(lv: &interior::Level, front: bool) -> Option<(usize, String)> {
    let entry = lv.doors.iter().find(|d| d.kind == "front").and_then(|d| d.rooms.iter().copied().find(|&r| r >= 0));
    let circ = |r: usize| interior::is_way_through(lv.rooms[r].kind);
    if lv.rooms.len() < 8 || !(0..lv.rooms.len()).any(circ) {
        return None;
    }
    let n = lv.rooms.len();
    let (mut hops, mut from) = (vec![usize::MAX; n], vec![usize::MAX; n]);
    let mut q = VecDeque::new();
    for r in (0..n).filter(|&r| circ(r) || (front && entry == Some(r as i16))) {
        hops[r] = 0;
        q.push_back(r);
    }
    while let Some(r) = q.pop_front() {
        for d in &lv.doors {
            let [a, b] = d.rooms;
            let other = if a == r as i16 {
                b
            } else if b == r as i16 {
                a
            } else {
                continue;
            };
            if other >= 0 && hops[other as usize] == usize::MAX {
                hops[other as usize] = hops[r] + 1;
                from[other as usize] = r;
                q.push_back(other as usize);
            }
        }
    }
    let worst = (0..n).filter(|&r| lv.rooms[r].squares > 0 && hops[r] != usize::MAX && interior::walkable(lv.rooms[r].kind)).max_by_key(|&r| (hops[r], std::cmp::Reverse(r)))?;
    let mut path = vec![lv.rooms[worst].kind.to_string()];
    let mut k = worst;
    while from[k] != usize::MAX {
        k = from[k];
        path.push(format!("{}{}", lv.rooms[k].kind, if entry == Some(k as i16) { " (front)" } else { "" }));
    }
    path.reverse();
    Some((hops[worst], path.join(" > ")))
}

/// The rules every interior keeps (vital's `check_interior`), each broken one as a line.
pub(crate) fn check_interior(world: &World, t0: &T0, l: &Layout, si: usize, id: &str, it: &interior::Interior, buckets: &HashMap<(i64, i64), Vec<usize>>) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    macro_rules! must {
        ($c:expr, $($t:tt)*) => {
            if !$c {
                bad.push(format!($($t)*));
            }
        };
    }
    let (nx, ny) = (it.nx, it.ny);
    for lv in &it.levels {
        for (ri, r) in lv.rooms.iter().enumerate() {
            must!(r.shape == 0 || interior::shape::fits(&lv.cells, nx, ri as i16, r.shape), "{}: the {} drawn shaped ({}) off its squares", lv.name, r.kind, r.shape);
        }
    }
    if !it.outlines.is_empty() {
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
        let half = interior::WALL_HALF as f32;
        for lv in &it.levels {
            if let Some(k) = (0..nx * ny).find(|&k| lv.cells[k] >= 0 && inset((k % nx) as f32 + 0.5, (k / nx) as f32 + 0.5) < half + 0.1 - 1e-3) {
                bad.push(format!("{}: square {},{} only {:.2} inside its outer wall", lv.name, k % nx, k / nx, inset((k % nx) as f32 + 0.5, (k / nx) as f32 + 0.5)));
            }
            let st = it.stairs;
            let room = (0..nx * ny).any(|k| {
                let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                lv.cells[k] >= 0 && [(i, j), (i + 1.0, j), (i, j + 1.0), (i + 1.0, j + 1.0)].iter().all(|c| inset(c.0, c.1) >= half - 1e-3)
            });
            if lv.has_stairs && room && !world.file.edits.designs.contains_key(id) {
                let d = [(st[0], st[1]), (st[0] + st[2], st[1]), (st[0], st[1] + st[3]), (st[0] + st[2], st[1] + st[3])]
                    .iter()
                    .map(|&(x, y)| inset(x as f32, y as f32))
                    .fold(f32::MAX, f32::min);
                must!(d >= half - 1e-3, "{}: the outer wall cuts the stairs {st:?} ({d:.2} in)", lv.name);
            }
        }
        let g = &it.levels[it.entry_level];
        // (A keep's towers in grid units: a square of wall round each one's room.)
        let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok()).and_then(|k| l.building(k));
        let towers: Vec<Vec<P>> = own
            .map(|b| {
                b.parts
                    .iter()
                    .filter(|p| matches!(p.kind, town::PartKind::Tower { .. }))
                    .map(|p| {
                        p.poly
                            .iter()
                            .map(|q| {
                                let d = sub(*q, it.origin);
                                [dot(d, it.axis) / 5.0, dot(d, it.across) / 5.0]
                            })
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let by_tower = |v: &[f32; 2]| {
            let p = [v[0] as f64, v[1] as f64];
            towers.iter().any(|t| contains(t, p) || (0..t.len()).any(|k| seg_dist(p, t[k], t[(k + 1) % t.len()]) <= 1.0))
        };
        for v in it.outlines.iter().flatten() {
            let d = (0..nx * ny)
                .filter(|&k| g.cells[k] >= 0)
                .map(|k| {
                    let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                    let (dx, dy) = ((i - v[0]).max(v[0] - i - 1.0).max(0.0), (j - v[1]).max(v[1] - j - 1.0).max(0.0));
                    (dx * dx + dy * dy).sqrt()
                })
                .fold(f32::MAX, f32::min);
            if d > if by_tower(v) { 1.5 } else { 1.0 } {
                bad.push(format!("its outer wall stands {d:.2} squares off its floor at {v:?}"));
                break;
            }
        }
    }
    // (R5 towns: a cemetery's tomb is one chamber on one storey, `town::cemetery`.)
    let tomb = l.building(id.rsplit(':').next().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX)).is_some_and(|b| b.func == town::catalog::index_of("tomb").map(|f| f as u16));
    // (A yurt is one round room on one storey, not built from a design: `town::yurts`.)
    let yurt = it.levels.len() == 1 && it.levels[0].rooms.iter().any(|q| q.kind == "yurt");
    must!(it.levels.len() >= 2 || tomb || yurt || world.file.edits.designs.contains_key(id), "only {} level", it.levels.len());
    if id.starts_with("b:") {
        match interior::design::design_of(world, t0, id, false) {
            Ok((d, _)) => match interior::design::problems(world, t0, id, &d) {
                Ok((built, problems)) => {
                    must!(problems.is_empty(), "as a design: {problems:?}");
                    must!(yurt || serde_json::to_string(&built).unwrap() == serde_json::to_string(it).unwrap(), "built again from its design, it differs");
                }
                Err(e) => bad.push(format!("as a design: {e}")),
            },
            Err(e) => bad.push(format!("no design: {e}")),
        }
    }
    let chapels = it.levels.iter().flat_map(|lv| lv.rooms.iter()).filter(|r| r.kind == "chapel").count();
    must!(chapels <= 1, "{chapels} chapels");
    let spirals: Vec<Vec<(u16, u16)>> = it.levels.iter().map(|lv| lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| (f.x, f.y)).collect()).collect();
    let mut at: Vec<(u16, u16)> = spirals.iter().flatten().copied().collect();
    at.sort();
    at.dedup();
    for s in at {
        let on: Vec<usize> = (0..spirals.len()).filter(|&li| spirals[li].contains(&s)).collect();
        must!(on.len() >= 2, "a spiral stair at {s:?} on one level only");
        must!(on.windows(2).all(|w| w[1] == w[0] + 1), "the spiral stair at {s:?} skips a level");
    }
    for lv in &it.levels {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, r)| r.kind == "cell") {
            let sq: Vec<usize> = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            if sq.is_empty() {
                continue;
            }
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            must!(x1 - x0 < 3 && y1 - y0 < 3, "{}: a {}x{} ft cell", lv.name, 5 * (x1 - x0 + 1), 5 * (y1 - y0 + 1));
            let doors: Vec<i16> = lv.doors.iter().filter(|d| d.rooms.contains(&(ri as i16))).map(|d| if d.rooms[0] == ri as i16 { d.rooms[1] } else { d.rooms[0] }).collect();
            must!(!doors.is_empty(), "{}: a cell with no door", lv.name);
            must!(doors.iter().all(|&o| o < 0 || lv.rooms[o as usize].kind != "cell"), "{}: a cell entered through another cell", lv.name);
        }
    }
    const DRINKING: [&str; 10] = ["Inn", "Tavern", "Alehouse", "Fine dining", "Brewery", "Winery", "Distillery", "Gambling den", "Smugglers' den", "Caravanserai"];
    if DRINKING.contains(&it.function) {
        let g = &it.levels[it.entry_level];
        let bars = g.furniture.iter().filter(|f| f.kind == "bar").count();
        let tables = g.furniture.iter().filter(|f| f.kind == "table" || f.kind == "booth_table").count();
        must!(bars >= 1 && tables >= 2, "{bars} bars and {tables} tables on the ground floor");
        let public = |r: i16| r >= 0 && matches!(g.rooms[r as usize].kind, "common room" | "taproom");
        match g.doors.iter().find(|d| d.kind == "front") {
            Some(front) => must!(public(front.rooms[0]), "the front door opens into the {}", g.rooms.get(front.rooms[0].max(0) as usize).map_or("?", |r| r.kind)),
            None => bad.push("no front door".into()),
        }
        let stair_room = g.cells[it.stairs[1] * nx + it.stairs[0]];
        must!(public(stair_room), "the stairs are in the {}", g.rooms[stair_room.max(0) as usize].kind);
    }
    let st = it.stairs;
    let on_stairs = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
    let door_sq = |d: &interior::Door| -> [(isize, isize); 2] {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
    };
    let inb = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny;
    for lv in &it.levels {
        if lv.has_stairs {
            let out = (st[1]..st[1] + st[3]).any(|j| (st[0]..st[0] + st[2]).any(|i| lv.cells[j * nx + i] < 0));
            must!(!out, "{}: stairs outside the building", lv.name);
        }
        let fronts = lv.doors.iter().filter(|d| d.kind == "front").count();
        must!(fronts == if lv.z == 0 { 1 } else { 0 }, "{}: {fronts} front doors", lv.name);
        if let Some(d) = lv.doors.iter().find(|d| d.kind == "front") {
            match door_sq(d).into_iter().find(|&(i, j)| !inb((i, j)) || lv.cells[j as usize * nx + i as usize] < 0) {
                Some(outside) => {
                    let p = interior::to_world(it, outside.0 as f64 + 0.5, outside.1 as f64 + 0.5);
                    let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok());
                    let hit = buckets
                        .get(&((p[0] / 100.0).floor() as i64, (p[1] / 100.0).floor() as i64))
                        .into_iter()
                        .flatten()
                        .find(|&&k| Some(l.buildings[k].id as usize) != own && contains(&l.buildings[k].poly, p));
                    must!(hit.is_none(), "the front door opens into building {}", hit.unwrap());
                }
                None => bad.push(format!("{}: the front door isn't on an outside wall", lv.name)),
            }
        }
        let mut blocked = vec![false; nx * ny];
        let mut misplaced = None;
        for f in &lv.furniture {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    if lv.has_stairs && on_stairs(i, j) {
                        misplaced.get_or_insert(format!("{}: {} on the stairs", lv.name, f.name));
                    }
                    if !interior::hall::is_platform(f.kind) && lv.doors.iter().any(|d| door_sq(d).contains(&(i as isize, j as isize))) {
                        misplaced.get_or_insert(format!("{}: {} in a doorway", lv.name, f.name));
                    }
                    if f.blocks_move {
                        blocked[j * nx + i] = true;
                    }
                }
            }
        }
        bad.extend(misplaced);
        let door_between = |a: (usize, usize), b: (usize, usize)| {
            lv.doors.iter().any(|d| {
                let s = door_sq(d);
                s.contains(&(a.0 as isize, a.1 as isize)) && s.contains(&(b.0 as isize, b.1 as isize))
            })
        };
        let mut starts: Vec<usize> =
            if lv.has_stairs { vec![st[1] * nx + st[0]] } else { lv.furniture.iter().filter(|f| matches!(f.kind, "spiral_stair" | "winder" | "stair_well")).map(|f| f.y as usize * nx + f.x as usize).collect() };
        if starts.is_empty() && lv.z == 0 {
            starts =
                lv.doors.iter().filter(|d| d.rooms[1] < 0).flat_map(door_sq).filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] >= 0).collect();
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
                if seen[q] || lv.cells[q] < 0 || blocked[q] || (lv.cells[q] != lv.cells[k] && !door_between((i, j), (a as usize, b as usize))) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        for d in &lv.doors {
            must!(door_sq(d).into_iter().filter(|&p| inb(p)).any(|(i, j)| seen[j as usize * nx + i as usize]), "{}: a {} door is cut off", lv.name, d.kind);
        }
        // (A spiral stair reached from the stairs, and from its room's doors with nothing but a rug
        // in the way: the way on up to the rooms above it.)
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
            must!(seen[k0], "{}: the spiral stair at ({}, {}) is cut off", lv.name, f.x, f.y);
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
            must!(got[k0], "{}: furniture shuts the spiral stair at ({}, {}) off from its room's doors", lv.name, f.x, f.y);
        }
        for (ri, room) in lv.rooms.iter().enumerate() {
            let open = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && !blocked[k]);
            let reached = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && seen[k]);
            if !interior::walkable(room.kind) {
                must!(!reached && !lv.doors.iter().any(|d| d.rooms.contains(&(ri as i16))), "{}: the {} has a way in", lv.name, room.kind);
                continue;
            }
            must!(!open || reached, "{}: the {} is unreachable", lv.name, room.kind);
        }
        if let Some((hops, path)) = corridor_hops(lv, true).filter(|h| h.0 > 2) {
            bad.push(format!("{}: {} rooms from a corridor ({path})", lv.name, hops - 1));
        }
    }
    bad
}
