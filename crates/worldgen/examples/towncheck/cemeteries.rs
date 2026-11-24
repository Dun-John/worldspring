use crate::prelude::*;

// R5 towns
/// Cemeteries (M21): per tier and culture group, how many and what they hold; the rules (each
/// should be 0): props or tombs on a walk, over the boundary or outside the plot, tomb doors not
/// reached from the way in, kurgans overlapping, a North or West lychgate off the way in, a tomb
/// planned and not built. Then the editor: a park patch set to a cemetery by hand, walls on and off.
pub(crate) fn cemeteries(ctx: &mut Ctx, r: &mut Report) {
    const GROUPS: [&str; 5] = ["North", "West", "South", "Dry", "Steppe"];
    r.counts(&CEM_RULES);
    r.counts(&["hand cemeteries refused", "hand cemeteries not laid"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let mut cams: Vec<String> = Vec::new();
    let mut shown = [0usize; 5];
    // (Up to 2 a group looked at on the battlemap: plot, props planned.)
    let mut look: Vec<(usize, Vec<P>, usize)> = Vec::new();
    let mut ground: Vec<(usize, town::Park)> = Vec::new();
    r.counts(&CEM_GROUND);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town {
            continue;
        }
        let g = (l.culture % 5) as usize;
        let n = l.parks.iter().filter(|p| p.kind == town::ParkKind::Cemetery).count();
        r.tally(&format!("{} {}: places", l.tier.name(), GROUPS[g]), 1);
        r.tally(&format!("{} {}: cemeteries", l.tier.name(), GROUPS[g]), n);
        let ps = if n > 0 { town::places::of(world, t0, &l) } else { Vec::new() };
        for pk in l.parks.iter().filter(|p| p.kind == town::ParkKind::Cemetery) {
            let by_road = pk.cemetery.is_some_and(|c| c.by_road);
            cemetery_label(&ps, li, pk, &GROUPS, r);
            r.tally(&format!("{}: cemeteries {}", GROUPS[g], if by_road { "by a road" } else { "in town" }), 1);
            check_cemetery(world, t0, &l, li, pk, &GROUPS, r);
            ground.push((li, pk.clone()));
            if shown[g] < 2 {
                shown[g] += 1;
                look.push((g, pk.pts.clone(), town::cemetery::plan(pk).map_or(0, |pl| pl.props.len())));
                let c = centroid(&pk.pts);
                let span = pk.pts.iter().map(|q| dist(*q, c)).fold(0.0, f64::max) * 2.0;
                cams.push(format!("    {} L{li} {}{}: fx {:.6} fy {:.6} zoom {:.1} (battlemap zoom 1.5)", GROUPS[g], l.tier.name(), if by_road { " by a road" } else { " in town" }, c[0] / mw, c[1] / mh, libm::log2(1100.0 / span.max(100.0))));
            }
        }
    }
    // The editor: up to 3 towns' first park-able patch set to a cemetery by hand, as walled and not.
    let with = |ci: usize, edit: worldgen::world::TownEdit| {
        let mut file = world.file.clone();
        file.edits.towns.insert(town::wards::key(ci), edit);
        World::new(file).unwrap()
    };
    let mut tried = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        if tried >= 3 {
            break;
        }
        let Ok(plan) = town::wards::plan_json(world, t0, ci) else { continue };
        let pick = plan["patches"].as_array().unwrap().iter().find(|p| p["in_town"] == true && p.get("park_refused").is_none() && !matches!(p["ward"].as_str(), Some("plaza" | "castle" | "temple"))).and_then(|p| p["patch"].as_u64());
        let Some(p) = pick else { continue };
        tried += 1;
        let walled = !town::layout(world, t0, ci).walls.is_empty();
        for walls in [walled, !walled] {
            let json = format!(r#"{{"patches":[{{"patch":{p},"ward":"park","park":"cemetery"}}],"walls":{walls}}}"#);
            let Ok((Some(edit), _)) = town::wards::change(world, t0, ci, &serde_json::from_str(&json).unwrap()) else {
                r.add("hand cemeteries refused", 1.0, format!("#{ci} patch {p} walls {walls}"));
                continue;
            };
            let w2 = with(ci, edit);
            let l = town::layout(&w2, t0, ci);
            let at = plan["patches"].as_array().unwrap().iter().find(|q| q["patch"].as_u64() == Some(p)).map(|q| [q["at"][0].as_f64().unwrap(), q["at"][1].as_f64().unwrap()]).unwrap();
            let hand = l.parks.iter().find(|pk| pk.kind == town::ParkKind::Cemetery && !pk.cemetery.is_some_and(|c| c.by_road) && dist(centroid(&pk.pts), at) < 200.0);
            r.tally(&format!("hand cemeteries (walls {})", if walls { "on" } else { "off" }), 1);
            match hand {
                Some(pk) => {
                    check_cemetery(&w2, t0, &l, ci, pk, &GROUPS, r);
                    let fi = town::catalog::index_of("tomb").map(|f| f as u16);
                    let tombs = l.buildings.iter().filter(|b| b.func == fi && contains(&pk.pts, centroid(&b.poly))).count();
                    r.tally("hand cemeteries' tombs", tombs);
                    if shown.iter().sum::<usize>() < 12 {
                        let c = centroid(&pk.pts);
                        cams.push(format!("    by hand #{ci} patch {p} walls {walls}: fx {:.6} fy {:.6} zoom 1.5", c[0] / mw, c[1] / mh));
                    }
                }
                None => r.add("hand cemeteries not laid", 1.0, format!("#{ci} patch {p} walls {walls}")),
            }
        }
    }
    for (g, pts, planned) in look {
        let b = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
        let w = ctx.window(b);
        const NAMES: [&str; 7] = ["headstone", "cairn", "standing stone", "yew", "lychgate", "kurgan", "tomb slab"];
        let laid = w.objects.iter().filter(|(p, n)| contains(&pts, *p) && NAMES.contains(&n.as_str())).count();
        r.tally(&format!("{}: battlemap props planned (looked at)", GROUPS[g]), planned);
        r.tally(&format!("{}: battlemap props laid (looked at)", GROUPS[g]), laid);
    }
    for (li, pk) in &ground {
        cemetery_ground(ctx, *li, pk, &GROUPS, r);
    }
    say!("  cemetery cameras:");
    for c in cams {
        say!("{c}");
    }
    graveyard_lots(ctx, r);
}

// Roadmap 7 (193)
pub(crate) const LOT_RULES: [&str; 6] = [
    "R lot stairs not fitting inside the wall",
    "R lot stair entrances off the plan",
    "R lot headstones outside the wall or on the walk",
    "R lot gates with a wall across them (looked at)",
    "R lot stairs drawn otherwise than planned (looked at)",
    "R lot stairs with no catacomb site",
];

/// Graveyard lots (Roadmap 7, 193): each laid to its plot (`cemetery::lot`): its stair on its walk inside
/// its wall, the layout's entrance there; on the battlemap (the first 8 turned off the map's axes): the
/// headstones laid, the wall's gap at the gate, the stair drawn as planned.
pub(crate) fn graveyard_lots(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&LOT_RULES);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let mut look: Vec<(usize, town::cemetery::Lot, Vec<P>)> = Vec::new();
    let mut cams = Vec::new();
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Open) {
            let Some(lp) = town::cemetery::lot(&l, b) else {
                r.tally("lots with no plan", 1);
                continue;
            };
            r.tally(&format!("lots {} ({})", if lp.own { "own" } else { "in a close or village" }, l.tier.name()), 1);
            r.tally(if lp.street { "lots facing a street" } else { "lots by their longest side" }, 1);
            // (Catacombs under cities alone.)
            let city = l.tier >= worldgen::t0::settle::Tier::City;
            if city {
                r.tally(if !lp.fits { "city lots: stair fitting nowhere" } else if lp.mausoleum { "city lots: mausoleum" } else { "city lots: plain stair" }, 1);
            }
            r.tally("lot headstones planned", lp.stones.len());
            let ang = libm::atan2(lp.inward[1], lp.inward[0]).to_degrees().rem_euclid(90.0);
            let turned = ang > 5.0 && ang < 85.0;
            r.tally(if turned { "lots turned off the map's axes" } else { "lots square to the map" }, 1);
            if city && !lp.fits {
                r.add("R lot stairs not fitting inside the wall", 1.0, format!("L{li} b{} {}", b.id, at(centroid(&b.poly))));
            }
            let ents: Vec<_> = l.entrances.iter().enumerate().filter(|(_, e)| e.kind == worldgen::under::UnderKind::Catacombs && contains(&b.poly, e.at)).collect();
            if city && !ents.iter().any(|(_, e)| dist(e.at, lp.stair) < 0.01 && dist(e.dir, lp.inward) < 1e-6) {
                r.add("R lot stair entrances off the plan", 1.0, format!("L{li} b{} {}", b.id, at(lp.stair)));
            }
            for &(p, _) in &lp.stones {
                if !contains(&lp.wall, p) || seg_dist(p, lp.walk.0, lp.walk.1) < 0.5 * lp.walk.2 {
                    r.add("R lot headstones outside the wall or on the walk", 1.0, format!("L{li} b{} {}", b.id, at(p)));
                }
            }
            if turned && lp.own && (city || look.len() < 4) && look.len() < 8 {
                if let Some((k, _)) = ents.first() {
                    let site = worldgen::under::generate(world, t0, li, *k);
                    if site.is_none() {
                        r.add("R lot stairs with no catacomb site", 1.0, format!("L{li} b{}", b.id));
                    }
                }
                let c = centroid(&b.poly);
                cams.push(format!("    L{li} b{} {} {}{}: fx {:.6} fy {:.6} ({:.0}° off axis) stair {} site u:{li}:{}", b.id, l.tier.name(), if lp.mausoleum { "mausoleum" } else { "plain stair" }, if lp.street { "" } else { ", no street side" }, c[0] / mw, c[1] / mh, ang.min(90.0 - ang), at(lp.stair), ents.first().map_or(-1, |e| e.0 as i64)));
                look.push((li, lp, b.poly.clone()));
            }
        }
    }
    for (li, lp, poly) in look {
        let w = ctx.window(bbox(&poly));
        let laid = w.objects.iter().filter(|(p, n)| contains(&poly, *p) && n == "headstone").count();
        r.tally("lot headstones planned (looked at)", lp.stones.len());
        r.tally("lot headstones laid (looked at)", laid);
        let runs = w.runs.iter().filter(|(a, b)| contains(&poly, lerp(*a, *b, 0.5))).count();
        r.tally("lot wall runs (looked at)", runs);
        if w.runs.iter().any(|(a, b)| seg_dist(lp.gate, *a, *b) < 2.0) {
            r.add("R lot gates with a wall across them (looked at)", 1.0, format!("L{li} {}", at(lp.gate)));
        }
        // The chunk's entrance shape at the stair: style 2 a mausoleum, else a plain stair.
        let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
        let (kx, ky) = ((lp.stair[0] / size).floor() as u32, (lp.stair[1] / size).floor() as u32);
        let ch = ctx.chunk(kx, ky);
        let o = [kx as f64 * size, ky as f64 * size];
        let style = ch.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Entrance).find(|s| dist([o[0] + s.pts[0][0] as f64 * SQUARE_FT, o[1] + s.pts[0][1] as f64 * SQUARE_FT], lp.stair) < 1.0).map(|s| (s.size as u32) >> 4);
        if style.is_some() && style != Some(if lp.mausoleum { 2 } else { 0 }) {
            r.add("R lot stairs drawn otherwise than planned (looked at)", 1.0, format!("L{li} {} style {style:?}", at(lp.stair)));
        }
    }
    say!("  graveyard lot cameras:");
    for c in cams {
        say!("{c}");
    }
}

pub(crate) const CEM_RULES: [&str; 12] = [
    "R props or tombs on a walk",
    "R props or tombs over the boundary",
    "R props or tombs outside the plot",
    "R tomb doors not reached from the way in",
    "R kurgans overlapping",
    "R lychgates not on the way in (North, West)",
    "R tombs planned and not built",
    "R tombs with no tomb chamber",
    "R plots by a road within 25 ft of a road as drawn",
    "R tombs within 5 ft of the boundary's face",
    "R yews within 10 ft of the gate or 2 ft of a walk",
    "R cemetery labels on a walk, a tomb, a big prop or the boundary",
];

// R5 towns (M21 fixes): a cemetery as the battlemap lays it.
pub(crate) const CEM_GROUND: [&str; 4] = ["R cemetery squares on a road", "R walk samples off the gravel", "R boundary samples with no wall or hedge", "R ways in not facing the road beside"];

/// A cemetery's battlemap: no road square inside its boundary; its walks (and the link from the road)
/// gravel all along (as the ground shader smooths the squares' gravel: a tent 1.5 squares round, at
/// least half); its wall or hedge all round but for the gate; a way in by a road faces it.
pub(crate) fn cemetery_ground(ctx: &mut Ctx, li: usize, pk: &town::Park, groups: &[&str; 5], r: &mut Report) {
    let Some(pl) = town::cemetery::plan(pk) else { return };
    let g = pk.cemetery.map_or(1, |c| c.group as usize % 5);
    let here = format!("L{li} {} at {}", groups[g], at(town::geom::mean(&pk.pts)));
    let b = pk.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
    let w = ctx.window([b[0] - 60.0, b[1] - 60.0, b[2] + 60.0, b[3] + 60.0]);
    let on_road = (0..w.nx * w.ny).filter(|&k| w.road[k] && contains(&pl.boundary, w.centre(k))).count();
    if on_road > 0 {
        r.add(CEM_GROUND[0], on_road as f64, format!("{here}: {on_road} squares"));
    }
    let cover = |p: P| -> f64 {
        let gx = (p[0] - w.ox) / SQUARE_FT - 0.5;
        let gy = (p[1] - w.oy) / SQUARE_FT - 0.5;
        let (ix, iy) = (gx.floor() as i64, gy.floor() as i64);
        let (mut sum, mut ws) = (0.0, 0.0);
        for j in -1..=2i64 {
            for i in -1..=2i64 {
                let (dx, dy) = (gx - (ix + i) as f64, gy - (iy + j) as f64);
                let wt = (1.5 - dx.abs()).max(0.0) * (1.5 - dy.abs()).max(0.0);
                let (a, c) = (ix + i, iy + j);
                let on = a >= 0 && c >= 0 && (a as usize) < w.nx && (c as usize) < w.ny && w.gravel[c as usize * w.nx + a as usize];
                sum += wt * on as u8 as f64;
                ws += wt;
            }
        }
        sum / ws.max(1e-9)
    };
    let mut walks: Vec<(P, P)> = pl.walks.iter().map(|&(a, e, _)| (a, e)).collect();
    walks.extend(pl.link);
    let (mut bad, mut all) = (0, 0);
    for (a, e) in walks {
        let l = dist(a, e);
        let n = (l / 2.5) as usize;
        for k in 1..n {
            let p = lerp(a, e, k as f64 * 2.5 / l);
            // (Where the link meets the road, the road's.)
            let by_road = (-1..=1).any(|j| (-1..=1).any(|i| w.index(add(p, [i as f64 * SQUARE_FT, j as f64 * SQUARE_FT])).is_some_and(|k| w.road[k])));
            if by_road {
                continue;
            }
            all += 1;
            if cover(p) < 0.5 {
                bad += 1;
            }
        }
    }
    r.tally(&format!("{}: walk samples", groups[g]), all);
    if bad > 0 {
        r.add(CEM_GROUND[1], bad as f64, format!("{here}: {bad}/{all}"));
    }
    if pl.bound != town::cemetery::Bound::None {
        let m = pl.boundary.len();
        let (mut miss, mut all) = (0, 0);
        for e in 0..m {
            let (a, c) = (pl.boundary[e], pl.boundary[(e + 1) % m]);
            let n = (dist(a, c) / 5.0) as usize;
            for k in 1..n {
                let p = lerp(a, c, k as f64 / n as f64);
                if dist(p, pl.gate) < 0.5 * pl.gap + 2.0 {
                    continue;
                }
                all += 1;
                if !w.runs.iter().any(|&(u, v)| seg_dist(p, u, v) < 1.5) {
                    miss += 1;
                }
            }
        }
        r.tally(&format!("{}: boundary samples", groups[g]), all);
        if miss > 0 {
            r.add(CEM_GROUND[2], miss as f64, format!("{here}: {miss}/{all}"));
        }
    }
    if pk.cemetery.is_some_and(|c| c.by_road) {
        let near = |p: P| (0..w.nx * w.ny).filter(|&k| w.road[k]).map(|k| dist(w.centre(k), p)).fold(f64::MAX, f64::min);
        let side = |p: P| (0..w.nx * w.ny).filter(|&k| w.road[k]).map(|k| w.centre(k)).map(|q| if contains(&pk.pts, q) { 0.0 } else { (0..pk.pts.len()).map(|e| seg_dist(q, pk.pts[e], pk.pts[(e + 1) % pk.pts.len()])).fold(f64::MAX, f64::min) }).fold(p[0] * 0.0 + f64::MAX, f64::min);
        let (dg, dm) = (near(pl.gate), side(pl.gate));
        if dm < 40.0 && dg > dm + BOUND_SLACK {
            r.add(CEM_GROUND[3], 1.0, format!("{here}: gate {dg:.0} ft from a road, plot {dm:.0}"));
        }
    }
}

/// How much farther (ft) than the plot's nearest edge a way in by a road may be from it.
pub(crate) const BOUND_SLACK: f64 = 20.0;

/// A cemetery's label (`places::of`): its baseline's every 2 ft off the walks, the tombs, the yews,
/// kurgans and lychgate and 2 ft inside the boundary.
pub(crate) fn cemetery_label(ps: &[town::places::Place], li: usize, pk: &town::Park, groups: &[&str; 5], r: &mut Report) {
    let Some(pl) = town::cemetery::plan(pk) else { return };
    let g = pk.cemetery.map_or(1, |c| c.group as usize % 5);
    let Some(p) = ps.iter().find(|p| p.sort == "cemetery" && p.label.len() >= 2 && contains(&pk.pts, p.label[p.label.len() / 2])) else {
        r.tally(&format!("{}: cemeteries with no label line", groups[g]), 1);
        return;
    };
    r.tally(&format!("{}: cemetery labels", groups[g]), 1);
    let len: f64 = p.label.windows(2).map(|q| dist(q[0], q[1])).sum();
    r.tally(&format!("{}: cemetery label ft", groups[g]), len as usize);
    if len < 36.0 {
        r.tally(&format!("{}: cemetery labels under 36 ft", groups[g]), 1);
    }
    let m = pl.boundary.len();
    let bad = p.label.windows(2).any(|q| {
        let n = (dist(q[0], q[1]) / 2.0).ceil().max(1.0) as usize;
        (0..=n).any(|k| {
            let x = lerp(q[0], q[1], k as f64 / n as f64);
            !contains(&pl.boundary, x)
                || (0..m).any(|e| seg_dist(x, pl.boundary[e], pl.boundary[(e + 1) % m]) < 2.0)
                || pl.walks.iter().any(|&(a, b, w)| seg_dist(x, a, b) < 0.5 * w + 1.0)
                || pl.tombs.iter().any(|t| contains(t, x) || (0..t.len()).any(|e| seg_dist(x, t[e], t[(e + 1) % t.len()]) < 1.0))
                || pl.props.iter().any(|q| match q.2 {
                    Kind::Kurgan => dist(x, q.0) < town::cemetery::KURGAN_R * q.4 as f64,
                    Kind::Yew => dist(x, q.0) < town::cemetery::YEW_R * q.4 as f64,
                    Kind::Lychgate => dist(x, q.0) < 6.0,
                    _ => false,
                })
        })
    });
    if bad {
        r.add(CEM_RULES[11], 1.0, format!("L{li} {} at {}: {}", groups[g], at(town::geom::mean(&pk.pts)), p.name));
    }
}

pub(crate) fn check_cemetery(world: &World, t0: &T0, l: &Layout, li: usize, pk: &town::Park, groups: &[&str; 5], r: &mut Report) {
    let Some(pl) = town::cemetery::plan(pk) else {
        r.add("cemeteries too small to lay out", 1.0, format!("L{li} at {}", at(town::geom::mean(&pk.pts))));
        return;
    };
    let g = pk.cemetery.map_or(1, |c| c.group as usize % 5);
    let here = format!("L{li} {} at {}", groups[g], at(town::geom::mean(&pk.pts)));
    for (k, name) in [(Kind::Headstone, "headstones"), (Kind::Cairn, "cairns"), (Kind::StandingStone, "standing stones"), (Kind::Yew, "yews"), (Kind::Lychgate, "lychgates"), (Kind::Kurgan, "kurgans"), (Kind::TombSlab, "tomb slabs")] {
        let c = pl.props.iter().filter(|p| p.2 == k).count();
        if c > 0 {
            r.tally(&format!("{}: {name}", groups[g]), c);
        }
    }
    r.tally(&format!("{}: tomb buildings", groups[g]), pl.tombs.len());
    let depth = |poly: &[P], p: P| if contains(poly, p) { (0..poly.len()).map(|e| seg_dist(p, poly[e], poly[(e + 1) % poly.len()])).fold(f64::MAX, f64::min) } else { 0.0 };
    let foot = |k: Kind, s: f32| match k {
        Kind::Yew => 2.0,
        Kind::Kurgan => town::cemetery::KURGAN_R * s as f64,
        Kind::TombSlab => 1.5,
        Kind::Cairn => 2.0,
        Kind::StandingStone => 1.2,
        _ => 1.0,
    };
    for &(p, _, k, _, s) in &pl.props {
        if k != Kind::Lychgate && pl.walks.iter().any(|&(a, b, w)| seg_dist(p, a, b) < 0.5 * w + 1.0) {
            r.add(CEM_RULES[0], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
        if k != Kind::Lychgate && depth(&pl.boundary, p) < foot(k, s) {
            r.add(CEM_RULES[1], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
        if !contains(&pk.pts, p) {
            r.add(CEM_RULES[2], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
    }
    // R5 towns (last fix): a plot by a road clear of every road as the map draws it at every level
    // the plot is seen at (the tiles' pieces, `lod::roads::pieces`) and of the layout's roads.
    {
        let b = pk.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
        let geo = &world.geom;
        let mut worst: Option<(f64, String)> = None;
        let mut note = |gap: f64, what: String| {
            if worst.as_ref().is_none_or(|w| gap < w.0) {
                worst = Some((gap, what));
            }
        };
        for lv in 0..=geo.max_level {
            let sp = geo.spacing_ft(lv);
            if sp > worldgen::lod::roads::CARVE_MAX_SPACING_FT {
                continue;
            }
            for pc in worldgen::lod::roads::pieces(&t0.roads, b, 0.0, sp.max(20.0)) {
                let hw = 0.5 * pc.class.width_ft();
                for q in pc.pts.windows(2) {
                    note(town::geom::poly_gap(&pk.pts, &[q[0].p, q[1].p]) - hw, format!("level {lv} ({sp:.0} ft)"));
                }
            }
        }
        for (k, (pts, c, w)) in l.roads.iter().enumerate() {
            for q in pts.windows(2) {
                note(town::geom::poly_gap(&pk.pts, &[q[0], q[1]]) - 0.5 * w, format!("layout road {k} (class {c}, {w:.0} ft)"));
            }
        }
        if let Some((gap, what)) = worst.filter(|_| pk.cemetery.is_some_and(|c| c.by_road)) {
            if gap < town::cemetery::VERGE - 5.0 {
                r.add(CEM_RULES[8], 1.0, format!("{here}: {gap:.1} ft from the road at {what}"));
            }
        }
    }
    let face = |pl: &town::cemetery::Plan| if pl.bound == town::cemetery::Bound::None { 0.0 } else { 1.875 };
    for t in &pl.tombs {
        if t.iter().any(|&q| depth(&pl.boundary, q) < 5.0 + face(&pl)) {
            r.add(CEM_RULES[9], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
    }
    let gap_seg = (sub(pl.gate, mul(pl.along, 0.5 * pl.gap)), add(pl.gate, mul(pl.along, 0.5 * pl.gap)));
    for &(p, _, k, _, s) in pl.props.iter().filter(|p| p.2 == Kind::Yew) {
        let rr = town::cemetery::YEW_R * s as f64;
        if seg_dist(p, gap_seg.0, gap_seg.1) < rr + 10.0 || pl.walks.iter().any(|&(a, b, w)| seg_dist(p, a, b) < 0.5 * w + rr + 2.0) {
            r.add(CEM_RULES[10], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
    }
    let seg_poly = |t: &[P], a: P, b: P| town::geom::poly_gap(t, &[a, b]);
    for t in &pl.tombs {
        if pl.walks.iter().any(|&(a, b, w)| seg_poly(t, a, b) < 0.5 * w + 1.0 || contains(t, a) || contains(t, b)) {
            r.add(CEM_RULES[0], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
        if t.iter().any(|&q| depth(&pl.boundary, q) <= 0.0) {
            r.add(CEM_RULES[1], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
        if t.iter().any(|&q| !contains(&pk.pts, q)) {
            r.add(CEM_RULES[2], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
    }
    // Doors reached: a 2.5-ft grid inside the boundary (and the gate's gap), off the tombs and what
    // blocks the way, from just inside the gate.
    let (x0, y0, x1, y1) = pl.boundary.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    const C: f64 = 2.5;
    let (nx, ny) = (((x1 - x0) / C) as usize + 3, ((y1 - y0) / C) as usize + 3);
    let cell = |p: P| -> Option<usize> {
        let (i, j) = (((p[0] - x0) / C).floor() as i64 + 1, ((p[1] - y0) / C).floor() as i64 + 1);
        (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    let mid = |k: usize| [x0 + ((k % nx) as f64 - 0.5) * C, y0 + ((k / nx) as f64 - 0.5) * C];
    let blockers: Vec<(P, f64)> = pl.props.iter().filter(|p| worldgen::battlemap::info(p.2 as u16).blocks_move).map(|p| (p.0, foot(p.2, p.4))).collect();
    let open: Vec<bool> = (0..nx * ny)
        .map(|k| {
            let p = mid(k);
            (contains(&pl.boundary, p) || dist(p, pl.gate) < 0.5 * pl.gap)
                && !pl.tombs.iter().any(|t| contains(t, p) || depth(t, p) > 0.0)
                && !blockers.iter().any(|&(q, rr)| dist(p, q) < rr)
        })
        .collect();
    let mut seen = vec![false; nx * ny];
    if let Some(s0) = cell(add(pl.gate, mul(pl.inward, 1.5))).filter(|&k| open[k]) {
        let mut q = VecDeque::from([s0]);
        seen[s0] = true;
        while let Some(k) = q.pop_front() {
            let (x, y) = ((k % nx) as i64, (k / nx) as i64);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (x + dx, y + dy);
                if a < 0 || b < 0 || a >= nx as i64 || b >= ny as i64 {
                    continue;
                }
                let n = b as usize * nx + a as usize;
                if !seen[n] && open[n] {
                    seen[n] = true;
                    q.push_back(n);
                }
            }
        }
    }
    for t in &pl.tombs {
        let door = lerp(t[0], t[1], 0.5);
        let out = sub(door, town::geom::mean(t));
        let front = add(door, mul(out, 2.0 / len(out).max(1e-9)));
        if !cell(front).is_some_and(|k| seen[k]) {
            r.add(CEM_RULES[3], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
    }
    let ks: Vec<(P, f64)> = pl.props.iter().filter(|p| p.2 == Kind::Kurgan).map(|p| (p.0, town::cemetery::KURGAN_R * p.4 as f64)).collect();
    for a in 0..ks.len() {
        for b in a + 1..ks.len() {
            if dist(ks[a].0, ks[b].0) < ks[a].1 + ks[b].1 {
                r.add(CEM_RULES[4], 1.0, format!("{here}: at {}", at(ks[a].0)));
            }
        }
    }
    if g <= 1 && !pl.walks.is_empty() && !pl.props.iter().any(|p| p.2 == Kind::Lychgate && dist(p.0, pl.gate) < 2.0) {
        r.add(CEM_RULES[5], 1.0, here.clone());
    }
    let fi = town::catalog::index_of("tomb").map(|f| f as u16);
    for t in &pl.tombs {
        let Some(b) = l.buildings.iter().find(|b| b.func == fi && b.poly.len() == t.len() && b.poly.iter().zip(t).all(|(p, q)| dist(*p, *q) < 0.01)) else {
            r.add(CEM_RULES[6], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
            continue;
        };
        let id = format!("b:{li}:{}", b.id);
        let lv = interior::generate_id(world, t0, &id).and_then(|it| it.levels.get(it.entry_level).cloned());
        let chamber = lv.as_ref().is_some_and(|lv| lv.rooms.iter().any(|q| q.kind == "tomb chamber"));
        if !chamber {
            r.add(CEM_RULES[7], 1.0, format!("{here}: {id}"));
        } else if lv.is_some_and(|lv| lv.furniture.iter().any(|f| f.kind == "sarcophagus")) {
            r.tally(&format!("{}: tombs with a sarcophagus", groups[g]), 1);
        }
    }
}

// R5 towns
/// `district_edits`' counts, in order.
pub(crate) const DISTRICT_COUNTS: [&str; 16] = [
    "district edits",
    "district edits refused",
    "patches in no district or two after a district edit",
    "patches not in the district they were put in",
    "district labels outside their district",
    "districts emptied with a label left, or with patches and none",
    "untouched districts with their id, name, kind or patches changed",
    "district edits giving other districts with the walls off",
    "new districts not named as asked",
    "renamed buildings laid out anew",
    "renames kept with their id",
    "renames carried",
    "renames carried to another function",
    "renames reported lost",
    "renames lost without a report",
    "district ids reused after one was emptied",
];

/// Districts drawn by hand (`patches[].district`), in up to `PLACES` towns (default 8) of four
/// districts or more: a patch into a neighbour's district, into a far one, two into a new named
/// one, and every patch of a small district moved out; each with the walls as generated and the
/// other way. A patch is in one district after, where it was put; the label of a changed one is on
/// it, an emptied one has none; the others are as generated (id, name, kind, patches) and the walls
/// change none. Then a renamed business's patch rerolled, the castle's keep renamed and its patch
/// rerolled, and a noted home's patch made empty: each rename and note is kept with its id, carried
/// (`renames_carried`, to the same function) or reported (`renames_lost`), never lost silently.
pub(crate) fn district_edits(base: &World, t0: &T0, r: &mut Report) {
    use serde_json::{Value, json};
    r.counts(&DISTRICT_COUNTS);
    let places: usize = std::env::var("PLACES").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let with = |w: &World, ci: usize, edit: Option<worldgen::world::TownEdit>| {
        let mut file = w.file.clone();
        if let Some(e) = edit {
            file.edits.towns.insert(town::wards::key(ci), e);
        }
        World::new(file).unwrap()
    };
    let qi_of = |d: &Value| d["id"].as_str().and_then(|s| s.rsplit(':').next()).and_then(|q| q.parse::<usize>().ok());
    let summary = |plan: &Value| -> Vec<(String, String, String, Vec<u64>)> {
        plan["districts"].as_array().into_iter().flatten().map(|d| (d["id"].as_str().unwrap_or("").to_string(), d["name"].as_str().unwrap_or("").to_string(), d["kind"].as_str().unwrap_or("").to_string(), d["patches"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect())).collect()
    };
    let mut done = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        if done >= places {
            break;
        }
        let Ok(plan) = town::wards::plan_json(base, t0, ci) else { continue };
        let ds = summary(&plan);
        if ds.len() < 4 {
            continue;
        }
        done += 1;
        let g = town::layout(base, t0, ci);
        let n = ds.len();
        let pats: Vec<&Value> = plan["patches"].as_array().unwrap().iter().collect();
        let dq = |p: u64| pats.iter().find(|x| x["patch"].as_u64() == Some(p)).and_then(|x| qi_of(&x["district"]));
        let nbs = |p: u64| pats.iter().find(|x| x["patch"].as_u64() == Some(p)).map(|x| x["neighbours"].as_array().unwrap().iter().filter_map(Value::as_u64).collect::<Vec<_>>()).unwrap_or_default();
        let did = |q: usize| format!("d:{ci}:{q}");
        // (name, request patches, where each should be after, a district to come out empty)
        let mut cases: Vec<(&str, Vec<Value>, Vec<(u64, usize)>, Option<usize>)> = Vec::new();
        let placed: Vec<(u64, usize)> = pats.iter().filter_map(|x| Some((x["patch"].as_u64()?, dq(x["patch"].as_u64()?)?))).filter(|(_, q)| ds[*q].3.len() >= 2).collect();
        if let Some((p, b)) = placed.iter().find_map(|&(p, a)| nbs(p).into_iter().filter_map(dq).find(|&b| b != a).map(|b| (p, b))) {
            cases.push(("into a neighbour's", vec![json!({ "patch": p, "district": did(b) })], vec![(p, b)], None));
        }
        if let Some((p, c)) = placed.iter().find_map(|&(p, a)| {
            let near: Vec<usize> = nbs(p).into_iter().filter_map(dq).collect();
            (0..n).rev().find(|&c| c != a && !near.contains(&c) && !ds[c].3.is_empty()).map(|c| (p, c))
        }) {
            cases.push(("into a far one", vec![json!({ "patch": p, "district": did(c) })], vec![(p, c)], None));
        }
        if let (Some(&(p1, a)), Some(&(p2, _))) = (placed.first(), placed.iter().rev().find(|x| x.1 != placed[0].1)) {
            let _ = a;
            cases.push(("two into a new one", vec![json!({ "patch": p1, "district": "Checked Ward" }), json!({ "patch": p2, "district": "checked ward" })], vec![(p1, n), (p2, n)], None));
        }
        if let Some(e) = (0..n).find(|&e| (1..=4).contains(&ds[e].3.len()) && ds[e].3.iter().all(|&m| nbs(m).into_iter().filter_map(dq).any(|q| q != e))) {
            let to: Vec<(u64, usize)> = ds[e].3.iter().map(|&m| (m, nbs(m).into_iter().filter_map(dq).find(|&q| q != e).unwrap())).collect();
            cases.push(("every patch moved out", to.iter().map(|&(m, q)| json!({ "patch": m, "district": did(q) })).collect(), to, Some(e)));
        }
        // (R5 towns: a new district emptied, then another made: a new id, not the emptied one's.)
        if let Some(&(p1, _)) = placed.first() {
            let step = |w: &World, d: &str| -> Option<Option<worldgen::world::TownEdit>> {
                let req: town::wards::TownRequest = serde_json::from_value(json!({ "patches": [{ "patch": p1, "district": d }] })).ok()?;
                town::wards::change(w, t0, ci, &req).ok().map(|x| x.0)
            };
            let got = step(base, "Checked Ward").flatten().and_then(|e1| {
                let w1 = with(base, ci, Some(e1));
                let w2 = with(base, ci, step(&w1, "auto")?);
                let w3 = with(base, ci, step(&w2, "Later Ward")??.into());
                let plan3 = town::wards::plan_json(&w3, t0, ci).ok()?;
                plan3["patches"].as_array()?.iter().find(|x| x["patch"].as_u64() == Some(p1)).and_then(|x| x["district"]["id"].as_str().map(str::to_string))
            });
            r.tally("district ids checked after one was emptied", 1);
            if got.as_deref() != Some(did(n + 1).as_str()) {
                r.add("district ids reused after one was emptied", 1.0, format!("#{ci}: patch {p1} in {got:?}, not {}", did(n + 1)));
            }
        }
        let walled = plan["walls"]["built"].as_bool().unwrap_or(false);
        for (name, req, want, emptied) in cases {
            let tag = format!("#{ci} {name}");
            let mut outs: Vec<Vec<(String, String, String, Vec<u64>)>> = Vec::new();
            for walls in [None, Some(!walled)] {
                let mut rq = json!({ "patches": req });
                if let Some(w) = walls {
                    rq["walls"] = json!(w);
                }
                r.tally("district edits", 1);
                let req: town::wards::TownRequest = serde_json::from_value(rq).unwrap();
                let edit = match town::wards::change(base, t0, ci, &req) {
                    Ok((Some(e), _)) => e,
                    Ok((None, _)) => {
                        r.add("district edits refused", 1.0, format!("{tag}: no edit"));
                        continue;
                    }
                    Err(why) => {
                        r.add("district edits refused", 1.0, format!("{tag}: {why}"));
                        continue;
                    }
                };
                let w = with(base, ci, Some(edit));
                let (plan2, l) = (town::wards::plan_json(&w, t0, ci).unwrap(), town::layout(&w, t0, ci));
                let d2 = summary(&plan2);
                let tag = format!("{tag} walls {}", if walls.is_some() { "toggled" } else { "as generated" });
                // One district each.
                let mut count: BTreeMap<u64, usize> = BTreeMap::new();
                for (_, _, _, m) in &d2 {
                    for &p in m {
                        *count.entry(p).or_default() += 1;
                    }
                }
                for (p, _) in placed.iter().chain(&want) {
                    let k = count.get(p).copied().unwrap_or(0);
                    if k != 1 {
                        r.add("patches in no district or two after a district edit", 1.0, format!("{tag}: patch {p} in {k}"));
                    }
                }
                if count.values().any(|&k| k > 1) {
                    r.add("patches in no district or two after a district edit", 1.0, format!("{tag}: a patch in two"));
                }
                for &(p, q) in &want {
                    if !d2.get(q).is_some_and(|d| d.3.contains(&p)) {
                        r.add("patches not in the district they were put in", 1.0, format!("{tag}: patch {p} not in d:{ci}:{q}"));
                    }
                }
                if name == "two into a new one" && (d2.get(n).is_none_or(|d| d.1 != "Checked Ward") || l.quarters.get(n).is_none_or(|q| q.name != "Checked Ward") || d2.len() != n + 1) {
                    r.add("new districts not named as asked", 1.0, format!("{tag}: {:?}", d2.get(n).map(|d| &d.1)));
                }
                // Labels: on a changed district, none on an emptied one.
                for (qi, q) in l.quarters.iter().enumerate() {
                    let changed = plan2["districts"][qi]["changed"] == true || plan2["districts"][qi]["by_hand"] == true;
                    if q.patches.is_empty() != q.label.is_empty() || (Some(qi) == emptied && !q.patches.is_empty()) {
                        r.add("districts emptied with a label left, or with patches and none", 1.0, format!("{tag}: d:{ci}:{qi} {} patches, {} label points", q.patches.len(), q.label.len()));
                    }
                    if changed && !q.label.is_empty() {
                        let mid = q.label[q.label.len() / 2];
                        let off = q.patches.iter().map(|poly| if contains(poly, mid) { 0.0 } else { poly.iter().zip(poly.iter().cycle().skip(1)).map(|(a, b)| seg_dist(mid, *a, *b)).fold(f64::MAX, f64::min) }).fold(f64::MAX, f64::min);
                        if off > 15.0 {
                            r.add("district labels outside their district", off, format!("{tag}: d:{ci}:{qi} label {off:.0} ft out at {}", at(mid)));
                        }
                    }
                    // The rest as generated (their labels too, with the walls as generated: one
                    // over a park moves off its middle as the park is laid out).
                    if qi < n && !changed && (d2[qi] != ds[qi] || g.quarters.get(qi).is_none_or(|o| o.name != q.name || o.kind != q.kind || (walls.is_none() && o.label != q.label))) {
                        r.add("untouched districts with their id, name, kind or patches changed", 1.0, format!("{tag}: d:{ci}:{qi} {:?} -> {:?}", ds[qi], d2[qi]));
                    }
                }
                outs.push(d2);
            }
            if outs.len() == 2 && outs[0] != outs[1] {
                r.add("district edits giving other districts with the walls off", 1.0, format!("{tag}: districts differ"));
            }
        }
        // Renames and notes kept when a patch is laid out anew.
        let corner: BTreeMap<u64, P> = plan["corners"].as_array().unwrap().iter().map(|c| (c["corner"].as_u64().unwrap(), [c["at"][0].as_f64().unwrap(), c["at"][1].as_f64().unwrap()])).collect();
        let patch_of = |p: P| pats.iter().find(|x| contains(&x["corners"].as_array().unwrap().iter().filter_map(|v| corner.get(&v.as_u64()?).copied()).collect::<Vec<_>>(), p)).and_then(|x| x["patch"].as_u64());
        let castle = town::catalog::index_of("castle").map(|c| c as u16);
        let business = g.buildings.iter().find(|b| b.func.is_some() && b.func != castle && b.keep.is_none() && patch_of(centroid(&b.poly)).is_some_and(|p| pats.iter().any(|x| x["patch"].as_u64() == Some(p) && x["in_town"] == true)));
        let keep = g.buildings.iter().find(|b| b.keep.is_some());
        let home = g.buildings.iter().find(|b| b.func.is_none() && b.structure == town::Structure::Roofed && patch_of(centroid(&b.poly)).is_some_and(|p| pats.iter().any(|x| x["patch"].as_u64() == Some(p) && x["in_town"] == true && x["ward"] == "common")));
        let mut f2: Vec<(&str, &town::Building, Value, bool)> = Vec::new();
        for (what, b, note) in [("a renamed business rerolled", business, false), ("the castle's keep renamed, rerolled", keep, false), ("a noted home's patch made empty", home, true)] {
            let Some(b) = b else { continue };
            let Some(p) = patch_of(centroid(&b.poly)) else { continue };
            let req = if note { json!({ "patches": [{ "patch": p, "ward": "empty" }] }) } else { json!({ "patches": [{ "patch": p, "reroll": true }] }) };
            f2.push((what, b, req, note));
        }
        for (what, b, req, note) in f2 {
            let tag = format!("#{ci} {what}");
            let key = format!("b:{ci}:{}", b.id);
            let mut file = base.file.clone();
            if note {
                file.edits.notes.insert(key.clone(), worldgen::world::Note { text: "Checked note".into(), tags: Vec::new(), at: None });
            } else {
                file.edits.renames.insert(key.clone(), "Checked Name".into());
            }
            let w1 = World::new(file).unwrap();
            let req: town::wards::TownRequest = serde_json::from_value(req).unwrap();
            let Ok((edit, rep)) = town::wards::change(&w1, t0, ci, &req) else { continue };
            let after = town::layout(&with(&w1, ci, edit), t0, ci);
            r.tally("renamed buildings laid out anew", 1);
            let listed = |k: &str, field: &str| rep[k].as_array().into_iter().flatten().find(|x| x[field] == key.as_str()).cloned();
            if after.buildings.iter().any(|x| x.id == b.id) {
                r.tally("renames kept with their id", 1);
            } else if let Some(c) = listed("renames_carried", "from") {
                r.tally("renames carried", 1);
                let to: u32 = c["to"].as_str().and_then(|s| s.rsplit(':').next()).and_then(|s| s.parse().ok()).unwrap_or(u32::MAX);
                let nb = after.buildings.iter().find(|x| x.id == to);
                let ok = nb.is_some_and(|x| x.func == b.func || (b.keep.is_some() && x.keep.is_some()));
                if !ok {
                    r.add("renames carried to another function", 1.0, format!("{tag}: {key} -> {to}"));
                }
            } else if listed("renames_lost", "id").is_some() {
                r.add("renames reported lost", 1.0, format!("{tag}: {key}"));
            } else {
                r.add("renames lost without a report", 1.0, format!("{tag}: {key} at {}", at(centroid(&b.poly))));
            }
        }
    }
}

// R5 towns (M16b): squares, parks and gates (`town::places`).

pub(crate) const PLACE_COUNTS: [&str; 8] = [
    "places sharing an id",
    "places unnamed",
    "place labels over a wall or building",
    "place ids moved or gone after an unrelated patch edit",
    "place ids moved or gone with the walls toggled",
    "generated place names changed by an edit elsewhere",
    "gates listed while the town is unwalled",
    "unrelated patch edits refused",
];

/// `places`: per tier, the squares, parks and gates with ids and names; that no two share an id,
/// every one is named, no label lies over a wall or building, an edit to a patch none of them
/// stands on moves none of their ids, nor do the walls toggled (gates aside: only while walled).
/// Towns and up, `PLACES` of them (default 12); a few towns' places printed per tier.
pub(crate) fn places(base: &World, t0: &T0, r: &mut Report) {
    use serde_json::json;
    use worldgen::town::places::{Place, label_blocked, of};
    r.counts(&PLACE_COUNTS);
    let most: usize = std::env::var("PLACES").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let mut shown: HashMap<&str, usize> = HashMap::new();
    let tiers = ["village", "town", "city", "metropolis"];
    let mut done = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        if done >= most {
            break;
        }
        done += 1;
        let tier = tiers[t0.settlements[ci].tier as usize];
        let l = town::layout(base, t0, ci);
        let ps = of(base, t0, &l);
        for k in ["square", "park", "gate"] {
            r.tally(&format!("{tier}: {k}s"), ps.iter().filter(|p| p.kind == k).count());
        }
        r.tally(&format!("{tier}: labels"), ps.iter().filter(|p| !p.label.is_empty()).count());
        r.tally(&format!("{tier}: towns"), 1);
        let n = shown.entry(tier).or_default();
        if *n < 2 {
            *n += 1;
            let list: Vec<String> = ps.iter().map(|p| format!("{} {} ({})", p.id, p.name, p.sort)).collect();
            say!("    {tier} {ci}: {}", list.join("; "));
        }
        let mut ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        for w in ids.windows(2).filter(|w| w[0] == w[1]) {
            r.add("places sharing an id", 1.0, format!("{ci}: {}", w[0]));
        }
        for p in &ps {
            if p.name.trim().is_empty() {
                r.add("places unnamed", 1.0, format!("{ci}: {}", p.id));
            }
            if label_blocked(&l, &p.label) {
                r.add("place labels over a wall or building", 1.0, format!("{ci}: {} {}", p.id, p.name));
            }
        }
        let gates_off = |l: &Layout, ps: &[Place]| l.walls.is_empty() && ps.iter().any(|p| p.kind == "gate");
        if gates_off(&l, &ps) {
            r.add("gates listed while the town is unwalled", 1.0, format!("{ci}"));
        }
        // Each of `ps` (but those `skip` keeps out) by its id in `now`, where it was.
        let kept = |now: &[Place], skip: &dyn Fn(&Place) -> bool, label: &str, tag: &str, r: &mut Report| {
            for p in ps.iter().filter(|p| !skip(p)) {
                match now.iter().find(|q| q.id == p.id) {
                    Some(q) if dist(q.at, p.at) <= 40.0 && q.kind == p.kind => {
                        if q.name != p.name {
                            r.add("generated place names changed by an edit elsewhere", 1.0, format!("{ci} {tag}: {} {} -> {}", p.id, p.name, q.name));
                        }
                    }
                    Some(q) => r.add(label, dist(q.at, p.at), format!("{ci} {tag}: {} moved {:.0} ft", p.id, dist(q.at, p.at))),
                    None => r.add(label, 1.0, format!("{ci} {tag}: {} {} gone", p.id, p.name)),
                }
            }
        };
        let Ok(plan) = town::wards::plan_json(base, t0, ci) else { continue };
        // A patch built on lots none of them stands on, nor any square at its corners, farthest
        // from the centre among the inner ones.
        let corner_of = |p: &Place| p.id.rsplit(':').next().and_then(|s| s.parse::<u64>().ok());
        let used_corners: Vec<u64> = ps.iter().filter(|p| p.kind == "square").filter_map(corner_of).collect();
        let far = plan["patches"].as_array().into_iter().flatten().filter(|x| {
            let p = x["patch"].as_u64().unwrap_or(0) as u32;
            matches!(x["ward"].as_str(), Some("common" | "craft" | "merchant" | "noble" | "slum")) && x["in_town"] == true
                && !ps.iter().any(|q| q.patch == Some(p))
                && !x["corners"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()).any(|c| used_corners.contains(&c))
        }).max_by(|a, b| {
            let c0 = [plan["center"][0].as_f64().unwrap_or(0.0), plan["center"][1].as_f64().unwrap_or(0.0)];
            let d = |x: &serde_json::Value| dist([x["at"][0].as_f64().unwrap_or(0.0), x["at"][1].as_f64().unwrap_or(0.0)], c0);
            d(a).total_cmp(&d(b))
        }).and_then(|x| Some((x["patch"].as_u64()?, if x["ward"] == "noble" { "common" } else { "noble" })));
        if let Some((p, to)) = far {
            let req: town::wards::TownRequest = serde_json::from_value(json!({ "patches": [{ "patch": p, "ward": to }] })).unwrap();
            match town::wards::change(base, t0, ci, &req) {
                Ok((Some(e), _)) => {
                    let l2 = town::wards::with_edit(base, t0, ci, &e);
                    // (A road's cemetery may come or go as the town's commons change.)
                    kept(&of(base, t0, &l2), &|q| q.id.contains(":r"), "place ids moved or gone after an unrelated patch edit", &format!("patch {p} to {to}"), r);
                }
                Ok((None, _)) => r.add("unrelated patch edits refused", 1.0, format!("{ci}: patch {p}: no change")),
                Err(e) => r.add("unrelated patch edits refused", 1.0, format!("{ci}: patch {p}: {e}")),
            }
        }
        let walled = plan["walls"]["built"].as_bool().unwrap_or(false);
        let req: town::wards::TownRequest = serde_json::from_value(json!({ "walls": !walled })).unwrap();
        if let Ok((Some(e), _)) = town::wards::change(base, t0, ci, &req) {
            let l2 = town::wards::with_edit(base, t0, ci, &e);
            let now = of(base, t0, &l2);
            if gates_off(&l2, &now) {
                r.add("gates listed while the town is unwalled", 1.0, format!("{ci} walls toggled"));
            }
            kept(&now, &|q| q.kind == "gate" || q.id.contains(":r"), "place ids moved or gone with the walls toggled", "walls toggled", r);
        }
    }
}
