use crate::prelude::*;

/// `villages`: v63 village layouts (not part of `all`). Per village: its cottages, how many stand
/// along each 100 ft of lane, those not square to the lane they front (> 8°), crofts overlapping
/// another or a building they do not hold or a lane; the battlemap's bounds (hedges, fences, walls:
/// `rural::battle`) within 2 ft of a roof or across a lane; hard kinks (> 50°) in the lanes; the green's
/// props. Places are offsets from the village's middle (ft).
pub(crate) fn villages(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["villages", "cottages", "cottages not square to their lane", "croft overlaps", "crofts over a building", "crofts over a lane", "bounds by a roof", "bounds across a lane", "lane kinks", "lanes ending at nothing inside the village", "greens with props", "greens without their well (of the first 24)"]);
    r.counts(&["cottages by no lane or road", "lane ends", "lane ends joined", "lane ends not joined, at nothing", "lane ends near a way but not joined", "lane ends overshooting a way", "lanes crossing a lane or road away from a shared node", "farmsteads"]);
    r.counts(&["plots holding no building", "farmyards with no lane to their gate", "plot bounds by a building not theirs", "runs by any building", "lanes through a building"]);
    r.counts(&["greens without a ring lane", "lanes across the green", "lane ends at the green's rim but not on the ring", "ring lane ends not on a way"]);
    r.counts(&["cottages facing the ring", "rings with fewer than 3 cottages facing them", "ways ending inside the green"]);
    let mut greens: Vec<(P, String)> = Vec::new();
    let mut gpolys: Vec<Vec<P>> = Vec::new();
    let mut fchunks: Vec<(P, String)> = Vec::new();
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.tier != worldgen::t0::settle::Tier::Village || l.site {
            continue;
        }
        let tag = format!("L{li}");
        let off = |p: P| format!("({:.0}, {:.0})", p[0] - l.center[0], p[1] - l.center[1]);
        r.tally("villages", 1);
        // (Lanes: the village's own and its roads, as segments with their half widths.)
        let mut ways: Vec<(P, P, f64)> = l.streets.iter().flat_map(|s| s.windows(2).map(|w| (w[0], w[1], 0.5 * town::village::LANE_W))).collect();
        ways.extend(l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w))));
        let reach = 1.3 * l.radius;
        let c = l.center;
        for (ri, k) in t0.roads.segments_near([c[0] - reach, c[1] - reach, c[0] + reach, c[1] + reach], 0.0) {
            let rc = &t0.roads.roads[ri as usize];
            let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 40.0).ceil().clamp(1.0, 400.0) as usize;
            let pts: Vec<P> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p).collect();
            ways.extend(pts.windows(2).map(|q| (q[0], q[1], 0.5 * rc.class.width_ft())));
        }
        let lane_ft: f64 = ways.iter().filter(|&&(a, b, _)| dist(lerp(a, b, 0.5), l.center) < l.radius).map(|&(a, b, _)| dist(a, b)).sum();
        let roofed: Vec<&town::Building> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed).collect();
        let cottages: Vec<&&town::Building> = roofed.iter().filter(|b| b.func.is_none() && b.ward == Ward::Rural).collect();
        r.tally("cottages", cottages.len());
        let fronting = roofed.iter().filter(|b| ways.iter().any(|&(a, e, h)| seg_poly(a, e, &b.poly) < h + 14.0)).count();
        r.tally("cottages by no lane or road", cottages.iter().filter(|b| !ways.iter().any(|&(a, e, h)| seg_poly(a, e, &b.poly) < h + 14.0)).count());
        if lane_ft > 150.0 {
            r.measure("houses fronting a lane per 100 ft of lane (fewest)", true, 100.0 * fronting as f64 / lane_ft.max(1.0), format!("{tag} {} houses, {lane_ft:.0} ft", roofed.len()));
            r.value("houses fronting a lane per 100 ft of lane", 100.0 * fronting as f64 / lane_ft, tag.clone());
        }
        r.measure("cottages (fewest)", true, cottages.len() as f64, tag.clone());
        let mill = town::catalog::index_of("mill").map(|f| f as u16);
        for b in &roofed {
            if b.poly.len() != 4 || (b.func.is_some() && b.func == mill) {
                continue;
            }
            let n_lane = l.streets.iter().map(|q| q.len().saturating_sub(1)).sum::<usize>();
            // (Nearest by its gap; a tie, where a house's front runs past a bend in the lane, to the piece by its middle.)
            let key = |a: P, e: P| seg_poly(a, e, &b.poly) + 0.05 * seg_dist(centroid(&b.poly), a, e);
            let Some((wi, &(a, e, h))) = ways.iter().enumerate().filter(|&(_, &(a, e, h))| seg_poly(a, e, &b.poly) < h + 10.0).min_by(|x, y| key(x.1.0, x.1.1).total_cmp(&key(y.1.0, y.1.1))) else { continue };
            let u = sub(e, a);
            let s0 = sub(b.poly[1], b.poly[0]);
            let c = (dot(u, s0) / (len(u) * len(s0)).max(1e-9)).abs();
            // (Either side square to it: the angle off the nearer axis.)
            let deg = libm::acos(c.min(1.0)).to_degrees();
            let deg = deg.min(90.0 - deg);
            if deg > 8.0 {
                let held = l.crofts.iter().any(|p| contains(&p.pts, centroid(&b.poly)));
                r.add("cottages not square to their lane", deg, format!("{tag} {} {deg:.0}° func {:?} croft {held} gap {:.1} by a {}, {:.0} ft from a lane's end", off(centroid(&b.poly)), b.func.map(|f| town::catalog::CATALOG[f as usize].key), seg_poly(a, e, &b.poly) - h, if wi < n_lane { "lane" } else { "road" }, l.streets.iter().flat_map(|q| [q[0], q[q.len() - 1]]).map(|q| dist(q, centroid(&b.poly))).fold(f64::MAX, f64::min)));
            }
        }
        for (i, p) in l.crofts.iter().enumerate() {
            let core = shrink(&p.pts, 1.0);
            for q in l.crofts.iter().skip(i + 1) {
                if poly_overlap(&core, &shrink(&q.pts, 1.0)) {
                    r.add("croft overlaps", 1.0, format!("{tag} {:?} {:?} {}", p.kind, q.kind, off(centroid(&p.pts))));
                }
            }
            for b in &l.buildings {
                if !b.poly.iter().all(|c| contains(&p.pts, *c) || edge_dist(&p.pts, *c) < 0.6) && poly_overlap(&core, &b.poly) {
                    r.add("crofts over a building", 1.0, format!("{tag} {:?} {} func {:?} {:?} {:.0} sq ft", p.kind, off(centroid(&b.poly)), b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.structure, area(&b.poly).abs()));
                }
            }
            if let Some(&(a, e, _)) = ways.iter().find(|&&(a, e, h)| seg_poly(a, e, &core) < h - 0.5 || contains(&core, a)) {
                r.add("crofts over a lane", 1.0, format!("{tag} {:?} {}", p.kind, off(lerp(a, e, 0.5))));
            }
        }
        let mut bt = worldgen::town::rural::Battle::default();
        let bb = l.crofts.iter().flat_map(|p| p.pts.iter()).fold([l.center[0] - 10.0, l.center[1] - 10.0, l.center[0] + 10.0, l.center[1] + 10.0], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        worldgen::town::rural::battle(&l, t0, [bb[0] - 20.0, bb[1] - 20.0, bb[2] + 20.0, bb[3] + 20.0], &|_| false, None, &mut bt);
        near_dump(&l, &bt, &tag);
        plot_checks(&l, &bt, &tag, r);
        for &(a, e, _) in &bt.runs {
            let pts: Vec<P> = samples(&[a, e], 1.0).into_iter().map(|s| s.0).collect();
            if let Some(p) = pts.iter().find(|p| roofed.iter().any(|b| contains(&b.poly, **p) || edge_dist(&b.poly, **p) < 2.0)) {
                r.add("bounds by a roof", 1.0, format!("{tag} {}", off(*p)));
            }
            if let Some(p) = pts.iter().find(|p| ways.iter().any(|&(s, t, h)| seg_dist(**p, s, t) < h)) {
                r.add("bounds across a lane", 1.0, format!("{tag} {}", off(*p)));
            }
        }
        for (si, s) in l.streets.iter().enumerate() {
            let e = s[s.len() - 1];
            let met = l.streets.iter().enumerate().any(|(j, q)| j != si && q.windows(2).any(|w| seg_dist(e, w[0], w[1]) < 15.0)) || ways.iter().skip(l.streets.iter().map(|q| q.len() - 1).sum()).any(|&(a, b, h)| seg_dist(e, a, b) < h + 15.0);
            // (A ring closed on itself has no end.)
            if !met && dist(s[0], e) > 0.01 && dist(e, l.center) < 0.9 * l.radius && !l.buildings.iter().any(|b| contains(&b.poly, e) || edge_dist(&b.poly, e) < 20.0) && !l.crofts.iter().any(|p| edge_dist(&p.pts, e) < 12.0) {
                r.add("lanes ending at nothing inside the village", 1.0, format!("{tag} {}", off(e)));
            }
            for w in s.windows(3) {
                let (u, v) = (sub(w[1], w[0]), sub(w[2], w[1]));
                if len(u) < 2.0 || len(v) < 2.0 {
                    continue;
                }
                let deg = libm::acos((dot(u, v) / (len(u) * len(v))).clamp(-1.0, 1.0)).to_degrees();
                if deg > 50.0 {
                    r.add("lane kinks", deg, format!("{tag} {} {deg:.0}° legs {:.0} {:.0}", off(w[1]), len(u), len(v)));
                }
            }
        }
        lane_ends(world, t0, &l, &tag, r);
        fields_rows(&l, t0, &tag, r);
        if fchunks.len() < 8 {
            if let Some(f) = l.fields.iter().enumerate().filter(|(i, _)| l.field_info[*i].kind != town::fields::FieldUse::Orchard).max_by(|a, b| area(a.1).abs().total_cmp(&area(b.1).abs())) {
                fchunks.push((centroid(f.1), tag.clone()));
            }
        }
        if let Some(g) = l.plazas.iter().find(|p| p.kind == PlazaKind::Green) {
            let (c, rad) = (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0]));
            gpolys.push(g.pts.clone());
            // (R5 towns, 103: the lane round it, its points but its ends on its centre line; no lane on its grass.)
            let rr = rad + 0.5 * town::village::LANE_W;
            if !l.streets.iter().any(|s| s.len() >= 4 && s[1..s.len() - 1].iter().all(|p| (dist(*p, c) - rr).abs() < 2.0)) {
                r.add("greens without a ring lane", 1.0, format!("{tag} green {} r {rad:.0}, {} roads by", off(c), l.roads.len()));
            }
            for s in &l.streets {
                if let Some(w) = s.windows(2).find(|w| seg_dist(c, w[0], w[1]) < rad - 1.0) {
                    r.add("lanes across the green", 1.0, format!("{tag} {} to {}", off(w[0]), off(w[1])));
                }
            }
            // (Cottages facing the ring: front within 15 ft of a ring lane's edge, square to its nearest piece within 15°, on its outer side.)
            let on_c = |p: P| (dist(p, c) - rr).abs() < 2.0;
            let pieces: Vec<(P, P)> = l.streets.iter().flat_map(|s| s.windows(2).filter(|w| on_c(w[0]) && on_c(w[1])).map(|w| (w[0], w[1]))).collect();
            if !pieces.is_empty() {
                // (Houses: the cottages and those given a trade, an inn or a smithy; not the church, shrine or mill.)
                let house = |b: &&&town::Building| b.func.is_none_or(|f| !["church", "chapel", "shrine", "mill", "graveyard", "watermill", "windmill"].iter().any(|k| town::catalog::CATALOG[f as usize].key.contains(k)));
                let facing = roofed.iter().filter(house).filter(|b| b.poly.len() == 4 && dist(centroid(&b.poly), c) > rr).filter(|b| {
                    let key = |a: P, e: P| seg_poly(a, e, &b.poly) + 0.05 * seg_dist(centroid(&b.poly), a, e);
                    let Some((a, e)) = pieces.iter().copied().min_by(|x, y| key(x.0, x.1).total_cmp(&key(y.0, y.1))) else { return false };
                    let (u, s0) = (sub(e, a), sub(b.poly[1], b.poly[0]));
                    let deg = libm::acos((dot(u, s0) / (len(u) * len(s0)).max(1e-9)).abs().min(1.0)).to_degrees();
                    seg_poly(a, e, &b.poly) - 0.5 * town::village::LANE_W <= 15.0 && deg.min(90.0 - deg) <= 15.0
                }).count();
                r.tally("cottages facing the ring", facing);
                if std::env::var("FACING_LIST").is_ok() {
                    eprintln!("facing {tag} {facing} green {} r {rad:.0} centre {:.0} {:.0}", off(c), l.center[0], l.center[1]);
                }
                r.value("cottages facing a ring", facing as f64, tag.clone());
                if facing < 3 {
                    r.add("rings with fewer than 3 cottages facing them", 1.0, format!("{tag} {facing} facing, green {} r {rad:.0}", off(c)));
                }
            }
            // (Ways ending inside the green, on no other way: lanes, the layout's roads and the network's.)
            let mut ends: Vec<P> = l.streets.iter().flat_map(|s| [s[0], s[s.len() - 1]]).collect();
            ends.extend(l.roads.iter().flat_map(|(q, _, _)| [q[0], q[q.len() - 1]]));
            let mut net: Vec<Vec<P>> = Vec::new();
            for (ri, k) in t0.roads.segments_near([c[0] - rad, c[1] - rad, c[0] + rad, c[1] + rad], 0.0) {
                let rc = &t0.roads.roads[ri as usize];
                let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 10.0).ceil().clamp(1.0, 400.0) as usize;
                net.push((0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p).collect());
                if k == 0 {
                    ends.push(rc.eval(0, 0.0, 2.5, t0.cell_ft).p);
                }
                if k as usize + 2 == rc.pts.len() {
                    ends.push(rc.eval(k as usize, 1.0, 2.5, t0.cell_ft).p);
                }
            }
            let all: Vec<(P, P)> = l.streets.iter().chain(l.roads.iter().map(|q| &q.0)).chain(net.iter()).flat_map(|s| s.windows(2).map(|w| (w[0], w[1]))).collect();
            for e in ends.into_iter().filter(|e| dist(*e, c) < rad - 2.0) {
                // (An end shared with another way's piece passing it is a joint, not a dead end.)
                let by = all.iter().filter(|&&(a, b)| seg_dist(e, a, b) < 1.0).count();
                let mine = all.iter().filter(|&&(a, b)| dist(a, e) < 1.0 || dist(b, e) < 1.0).count();
                if by <= mine.max(1) && mine <= 1 {
                    r.add("ways ending inside the green", 1.0, format!("{tag} {} green {} r {rad:.0}", off(e), off(c)));
                }
            }
            if l.props.iter().any(|p| dist(p.at, c) < rad) {
                r.tally("greens with props", 1);
            }
            if greens.len() < 24 {
                greens.push((c, tag.clone()));
            }
        }
    }
    // (The first two dozen greens' wells on the battlemap.)
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    // (Field cover: in the chunk under each of the first villages' biggest field, the 32x32 windows
    // mostly of field squares, every 4 squares.)
    for (c, tag) in fchunks {
        let (kx, ky) = ((c[0] / size).floor() as u32, (c[1] / size).floor() as u32);
        let ch = ctx.chunk(kx, ky);
        let field = |k: usize| ch.urban[k] == worldgen::battlemap::URBAN_FIELD;
        let hedge = |k: usize| ch.yard.get(k).is_some_and(|&y| y & 8 != 0);
        for o in ch.objects.iter().filter(|o| worldgen::battlemap::info(o.kind as u16).cover >= 1) {
            let (x, y) = ((o.x as usize).min(SQ - 1), (o.y as usize).min(SQ - 1));
            if !field(y * SQ + x) {
                continue;
            }
            let edge = (-2i64..=2).any(|dy| (-2i64..=2).any(|dx| {
                let (i, j) = (x as i64 + dx, y as i64 + dy);
                i < 0 || j < 0 || i >= SQ as i64 || j >= SQ as i64 || { let q = j as usize * SQ + i as usize; !field(q) || hedge(q) }
            }));
            // (Orchards' and copses' trees aside.)
            if !matches!(o.kind, Kind::Bush | Kind::RockPile | Kind::Thicket | Kind::WaterTrough) {
                continue;
            }
            r.tally(if edge { "field cover by an edge or hedge" } else { "field cover mid-field" }, 1);
        }
        let dist = worldgen::battlemap::cover_distance(&ch);
        let far = (0..SQ * SQ).filter(|&k| field(k) && ch.water_level[k] <= ch.height[k]).map(|k| dist[k]).max().unwrap_or(0);
        r.measure("field square farthest from cover in a chunk (squares)", false, far as f64, tag.clone());
        for wy in (0..=SQ - 32).step_by(4) {
            for wx in (0..=SQ - 32).step_by(4) {
                let n = (0..32 * 32).filter(|&q| field((wy + q / 32) * SQ + wx + q % 32)).count();
                if n * 2 < 32 * 32 {
                    continue;
                }
                let inw = |o: &worldgen::battlemap::Object| o.x >= wx as f32 && o.x < (wx + 32) as f32 && o.y >= wy as f32 && o.y < (wy + 32) as f32;
                let cover = ch.objects.iter().filter(|o| worldgen::battlemap::info(o.kind as u16).cover >= 1 && inw(o)).count();
                r.value("cover objects per 32x32 field window", cover as f64, format!("{tag} ({wx},{wy})"));
            }
        }
    }
    // (What stands on the greens: objects by kind, of the first two dozen.)
    for g in gpolys.iter().take(24) {
        let c = centroid(g);
        let (kx, ky) = ((c[0] / size).floor() as u32, (c[1] / size).floor() as u32);
        let ch = ctx.chunk(kx, ky);
        let (ox, oy) = (kx as f64 * size, ky as f64 * size);
        r.tally("hedge or wall squares on greens (of 24)", (0..SQ * SQ).filter(|&k| ch.yard.get(k).is_some_and(|&y| y & 24 == 24)).count());
        for o in &ch.objects {
            if contains(g, [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT]) {
                r.tally(&format!("on greens: {:?}", o.kind), 1);
            }
        }
    }
    for (c, tag) in greens {
        let (kx, ky) = ((c[0] / size).floor() as u32, (c[1] / size).floor() as u32);
        let ch = ctx.chunk(kx, ky);
        let (ox, oy) = (kx as f64 * size, ky as f64 * size);
        if !ch.objects.iter().any(|o| o.kind == Kind::Well && dist([ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT], c) < 6.0) {
            r.add("greens without their well (of the first 24)", 1.0, tag);
        }
    }
}

/// The fields' rows (`rural::rows`) and how far their squares lie from a hedgerow or wall (bounds,
/// rows, crofts' hedges: `rural::battle` over the layout's fields, chunk by chunk, roads not cut).
pub(crate) fn fields_rows(l: &Layout, t0: &T0, tag: &str, r: &mut Report) {
    use worldgen::town::fields::{FieldBound, FieldUse};
    use worldgen::town::rural;
    let live: Vec<usize> = (0..l.fields.len()).filter(|&i| l.field_info.get(i).is_some_and(|x| x.kind != FieldUse::Orchard)).collect();
    if live.is_empty() {
        return;
    }
    let bb = live.iter().flat_map(|&i| l.fields[i].iter()).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let g0 = [(bb[0] / 5.0).floor() * 5.0 - 100.0, (bb[1] / 5.0).floor() * 5.0 - 100.0];
    let (nx, ny) = (((bb[2] - g0[0]) / 5.0) as usize + 21, ((bb[3] - g0[1]) / 5.0) as usize + 21);
    let mut d = vec![i32::MAX; nx * ny];
    let mut q = VecDeque::new();
    let blk = 640.0;
    let (bx0, by0) = ((g0[0] / blk).floor() as i64, (g0[1] / blk).floor() as i64);
    let (bx1, by1) = (((g0[0] + nx as f64 * 5.0) / blk).floor() as i64, ((g0[1] + ny as f64 * 5.0) / blk).floor() as i64);
    for by in by0..=by1 {
        for bx in bx0..=bx1 {
            let mut bt = rural::Battle::default();
            let rect = [bx as f64 * blk, by as f64 * blk, (bx + 1) as f64 * blk, (by + 1) as f64 * blk];
            rural::battle(l, t0, rect, &|_| false, None, &mut bt);
            r.measure("runs in a chunk (most, of the 900 a chunk holds)", false, bt.runs.len() as f64, format!("{tag} block ({bx},{by})"));
            for &(a, e, kind) in &bt.runs {
                if matches!(kind, rural::RunKind::Fence | rural::RunKind::Corral) {
                    continue;
                }
                let m = (dist(a, e) / 1.25).ceil().max(1.0) as usize;
                for j in 0..=m {
                    let p = lerp(a, e, j as f64 / m as f64);
                    let (i, k) = (((p[0] - g0[0]) / 5.0).floor() as i64, ((p[1] - g0[1]) / 5.0).floor() as i64);
                    if i >= 0 && k >= 0 && (i as usize) < nx && (k as usize) < ny && d[k as usize * nx + i as usize] != 0 {
                        d[k as usize * nx + i as usize] = 0;
                        q.push_back(k as usize * nx + i as usize);
                    }
                }
            }
        }
    }
    while let Some(k) = q.pop_front() {
        let (x, y) = ((k % nx) as i64, (k / nx) as i64);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (i, j) = (x + dx, y + dy);
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && d[j as usize * nx + i as usize] > d[k] + 1 {
                d[j as usize * nx + i as usize] = d[k] + 1;
                q.push_back(j as usize * nx + i as usize);
            }
        }
    }
    for &i in &live {
        let f = &l.fields[i];
        let info = &l.field_info[i];
        let rows = rural::rows(l, i);
        let a = info.angle as f64;
        let v = [-libm::sin(a), libm::cos(a)];
        let (lo, hi) = f.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(dot(*p, v)), hi.max(dot(*p, v))));
        r.value("field width across furrows (ft)", hi - lo, format!("{tag} f{i}"));
        r.value("field area (sq ft)", area(f).abs(), format!("{tag} f{i}"));
        r.value("rows per field", rows.len() as f64, format!("{tag} f{i} {:.0} ft wide", hi - lo));
        r.tally(if info.bound == FieldBound::None { "fields unbounded" } else { "fields bounded" }, 1);
        for (pa, pb, gates) in &rows {
            r.tally("rows", 1);
            r.tally("row gates", gates.len());
            r.value("row ft per gate", dist(*pa, *pb) / gates.len().max(1) as f64, format!("{tag} f{i}"));
        }
        let copse = rural::copse(l, i);
        let fb = f.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        let mut worst = 0;
        let (i0, j0) = (((fb[0] - g0[0]) / 5.0) as usize, ((fb[1] - g0[1]) / 5.0) as usize);
        let (i1, j1) = (((fb[2] - g0[0]) / 5.0) as usize, ((fb[3] - g0[1]) / 5.0) as usize);
        for j in j0..=j1.min(ny - 1) {
            for ii in i0..=i1.min(nx - 1) {
                let p = [g0[0] + (ii as f64 + 0.5) * 5.0, g0[1] + (j as f64 + 0.5) * 5.0];
                if !contains(f, p) || copse.is_some_and(|(c, rr)| dist(c, p) < 1.2 * rr + 10.0) {
                    continue;
                }
                let dd = d[j * nx + ii];
                worst = worst.max(dd);
                r.tally("field squares", 1);
                if dd > 12 {
                    r.tally("field squares over 60 ft from a hedge or wall", 1);
                }
            }
        }
        r.value("farthest square from a hedge or wall per field (ft)", 5.0 * worst as f64, format!("{tag} f{i} {:?} {:?} {:.0} ft wide", info.kind, info.bound, hi - lo));
    }
}

/// `piers`: dock piers (not part of `all`). Per town with piers: how many, how many are shorter than
/// the 105 ft a sea pier gets (cut back on a river), and the narrowest open channel between piers that
/// face each other across the water (outline to outline); pairs closer than 40 ft, piers overlapping
/// another, and piers within 30 ft of a bridge deck.
pub(crate) fn piers(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["towns with piers", "piers", "piers shorter than 105 ft", "facing pairs", "facing pairs under 40 ft", "piers overlapping another", "piers within 30 ft of a bridge"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.piers.is_empty() {
            continue;
        }
        let tag = format!("L{li} {:?}", l.tier);
        r.tally("towns with piers", 1);
        r.tally("piers", l.piers.len());
        let axis = |p: &Vec<[f64; 2]>| {
            let d = [p[1][0] - p[0][0], p[1][1] - p[0][1]];
            let n = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
            ([d[0] / n, d[1] / n], n)
        };
        for (i, a) in l.piers.iter().enumerate() {
            let (ua, la) = axis(a);
            if la < 104.0 {
                r.tally("piers shorter than 105 ft", 1);
            }
            if l.bridges.iter().any(|b| town::geom::poly_gap(a, b) < 30.0) {
                r.add("piers within 30 ft of a bridge", 1.0, format!("{tag} pier {i}, {la:.0} ft long"));
            }
            for (j, b) in l.piers.iter().enumerate().skip(i + 1) {
                let (ca, cb) = (centroid(a), centroid(b));
                let d = [cb[0] - ca[0], cb[1] - ca[1]];
                let dn = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                let (ub, _) = axis(b);
                let gap = town::geom::poly_gap(a, b);
                if gap == 0.0 {
                    r.add("piers overlapping another", 1.0, format!("{tag} piers {i} {j} at {:.0} {:.0}, {la:.0} and {:.0} ft long", ca[0], ca[1], axis(b).1));
                }
                // Facing: roughly end to end along their length, not side by side.
                if (ua[0] * ub[0] + ua[1] * ub[1]).abs() > 0.5 && ((d[0] * ua[0] + d[1] * ua[1]) / dn).abs() > 0.7 && dn < 400.0 {
                    r.tally("facing pairs", 1);
                    if gap < 40.0 {
                        r.add("facing pairs under 40 ft", gap, format!("{tag} piers {i} {j} gap {gap:.0}"));
                    }
                    r.measure("narrowest open channel between facing piers (ft)", true, gap, format!("{tag} piers {i} {j}"));
                    // (River piers are cut short of 105 ft; sea, lake and harbour ones are not.)
                    if la < 104.0 && axis(b).1 < 104.0 {
                        r.measure("  of those, both cut short (river, wharf): narrowest (ft)", true, gap, format!("{tag} piers {i} {j}"));
                    }
                }
            }
        }
    }
}

/// Town halls and the four guilds laid out by zones (`interior::hub::civic`): no floor fallen back
/// on, every interior rule, every room of the programme (the council chamber, the strongroom, the
/// archive, the guild's own; on a single storey some may be absent), the hall's low dais and chair
/// of office at the far end of its frame (with the `halls` rules over it), rooms no longer than
/// 2.5:1, an arcade open along the street front with the front door into it.
pub(crate) fn civic(ctx: &mut Ctx, r: &mut Report) {
    use interior::hub::Kind;
    r.counts(&["civic buildings", "  laid out by zones"]);
    r.counts(&CIVIC_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    const KEYS: [&str; 5] = ["town_hall", "merchant_guild", "craft_guild", "adventurers_guild", "mages_guild"];
    let key_of = |b: &town::Building| b.func.map(|f| town::catalog::CATALOG[f as usize].key).filter(|k| KEYS.contains(k));
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        // (Belfries: on every city's or metropolis's town hall one goes on, else why not.)
        if l.tier >= worldgen::t0::settle::Tier::City {
            for b in l.buildings.iter().filter(|b| town::belfry::is_town_hall(b)) {
                let id = format!("b:{li}:{}", b.id);
                if town::belfry::of(b).is_some() {
                    r.tally("belfries", 1);
                    // (BELFRY_AT=1: where each stands, world ft.)
                    if std::env::var("BELFRY_AT").is_ok() {
                        let c = centroid(&b.poly);
                        say!("belfry {id} at {:.0} {:.0} ({}, {} floors)", c[0], c[1], interior::hub_kind(t0, &l, b).map_or("-", |k| k.name()), b.floors);
                    }
                    let fell = interior::hub_kind(t0, &l, b).is_none();
                    let bad = if fell { vec!["not laid out by zones".to_string()] } else { interior::generate_id(world, t0, &id).map(|it| belfry_rules(&it, b.floors.max(1) as i8)).unwrap_or_default() };
                    if !bad.is_empty() {
                        r.add(CIVIC_COUNTS[9], bad.len() as f64, format!("{id}: {}", bad.join("; ")));
                    }
                    continue;
                }
                let why = if !town::belfry::eligible(b) { "not roofed, a castle's or in pieces".to_string() } else { format!("{:?}", town::belfry::raise(&l, b).err()) };
                r.tally(&format!("  no belfry: {why}"), 1);
                r.add(CIVIC_COUNTS[8], 1.0, format!("{id}: {why}"));
            }
        }
        // (Stables' stalls.)
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == "stables")) {
            let id = format!("b:{li}:{}", b.id);
            if let Some(it) = interior::generate_id(world, t0, &id) {
                room_pieces(&it, &format!("{id} (stables)"), r);
            }
        }
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let Some(key) = key_of(b) else { continue };
            r.tally("civic buildings", 1);
            r.tally(&format!("  {key}s"), 1);
            let id = format!("b:{li}:{}", b.id);
            let Some(kind) = interior::hub_kind(t0, &l, b) else {
                let n = interior::Shell::of(&l, b).inside.iter().filter(|&&i| i).count();
                r.add(CIVIC_COUNTS[7], n as f64, format!("{id} ({key}): {n} squares"));
                continue;
            };
            r.tally("  laid out by zones", 1);
            r.tally(&format!("    {}s", kind.name()), 1);
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            let tag = format!("{id} ({})", kind.name());
            room_pieces(&it, &tag, r);

            if std::env::var("PLAN").is_ok_and(|p| p.trim() == id) {
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
                                } else {
                                    (b'a' + (c as u8 % 26)) as char
                                }
                            })
                            .collect();
                        say!("  {row}");
                    }
                    let pieces: Vec<String> = lv.furniture.iter().filter(|f| std::env::var("ALL").is_ok() || matches!(f.kind, "low_dais" | "chair_of_office" | "pillar")).map(|f| format!("{}@{},{}", f.kind, f.x, f.y)).collect();
                    say!("  pieces: {}", pieces.join(" "));
                }
                for p in interior::hall_plans(world, t0, li, b.id as usize) {
                    say!("  hall frame: level {} room {} o {:?} ds {:?} dt {:?} {}x{}", p.level, p.room, p.frame.o, p.frame.ds, p.frame.dt, p.frame.len, p.frame.wid);
                }
            }
            let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
            if !fell.is_empty() {
                r.add(CIVIC_COUNTS[0], fell.len() as f64, format!("{tag}: z {fell:?}"));
            }
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            let bad = check_interior(world, t0, &l, li, &id, &it, buckets);
            if !bad.is_empty() {
                r.add(CIVIC_COUNTS[1], bad.len() as f64, format!("{tag}: {}", bad.join("; ")));
            }
            // (The programme.)
            let mut want = vec!["great hall", "council chamber", "strongroom", "archive"];
            want.extend(match kind {
                Kind::MerchantGuild => vec!["counting house", "sample room"],
                Kind::CraftGuild => vec!["masterpiece gallery", "workshop"],
                Kind::AdventurersGuild => vec!["job-board room", "trophy hall", "armory", "bunk room"],
                Kind::MagesGuild => vec!["library", "laboratory", "warded vault"],
                Kind::ArcadeHall => vec!["arcade"],
                _ => vec![],
            });
            let above = it.levels.iter().filter(|lv| lv.z >= 0 && !lv.roof).count();
            let missing: Vec<&str> = want.iter().copied().filter(|k| !it.levels.iter().any(|lv| lv.rooms.iter().any(|r| r.kind == *k && r.squares > 0))).collect();
            if !missing.is_empty() {
                let row = if above <= 1 && !missing.contains(&"great hall") { CIVIC_COUNTS[6] } else { CIVIC_COUNTS[2] };
                r.add(row, missing.len() as f64, format!("{tag}, {above} storeys: no {}", missing.join(", ")));
            }
            // (The hall's dais and chair of office in its frame's last two rows.)
            let plans = interior::hall_plans(world, t0, li, b.id as usize);
            let dais = plans.iter().any(|p| {
                let lv = &it.levels[p.level];
                let f = p.frame;
                let (i0, j0) = f.grid(f.len as isize - 2, 0);
                let (i1, j1) = f.grid(f.len as isize - 1, f.wid as isize - 1);
                let far = |x: u16, y: u16| {
                    let (x, y) = (x as isize, y as isize);
                    x >= i0.min(i1) && x <= i0.max(i1) && y >= j0.min(j1) && y <= j0.max(j1)
                };
                lv.rooms.get(p.room as usize).is_some_and(|r| r.kind == "great hall")
                    && lv.furniture.iter().any(|f| f.kind == "low_dais" && far(f.x, f.y))
                    && lv.furniture.iter().any(|f| f.kind == "chair_of_office" && far(f.x, f.y))
            });
            if !dais {
                r.add(CIVIC_COUNTS[3], 1.0, tag.clone());
            }
            // (Proportions: every room one stands in but a way about the floor, by its box.)
            for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
                for (ri, room) in lv.rooms.iter().enumerate() {
                    if room.squares < 4 || interior::is_circulation(room.kind) || !interior::walkable(room.kind) {
                        continue;
                    }
                    let sq: Vec<usize> = (0..it.nx * it.ny).filter(|&k| lv.cells[k] == ri as i16).collect();
                    let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % it.nx), a.1.max(k % it.nx)));
                    let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / it.nx), a.1.max(k / it.nx)));
                    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
                    if 2 * w.max(h) > 5 * w.min(h) {
                        r.add(CIVIC_COUNTS[4], w.max(h) as f64 / w.min(h) as f64, format!("{tag} {}: {} {w}x{h}", lv.name, room.kind));
                    }
                }
            }
            // (An arcade: open along the street front, the front door into it.)
            if kind == Kind::ArcadeHall {
                let ok = it.levels.iter().find(|lv| lv.z == 0).is_some_and(|lv| {
                    let arc = lv.rooms.iter().position(|r| r.kind == "arcade").map(|a| a as i16);
                    // (Open along 4 squares or more of the front.)
                    let open: f32 = lv.walls.iter().filter(|w| w.arcade).map(|w| (w.b[0] - w.a[0]).abs() + (w.b[1] - w.a[1]).abs()).sum();
                    r.tally("  arcade fronts' open squares", open as usize);
                    arc.is_some_and(|a| open >= 4.0 && lv.doors.iter().any(|d| d.kind == "front" && d.rooms.contains(&a)))
                });
                if !ok {
                    r.add(CIVIC_COUNTS[5], 1.0, tag.clone());
                }
            }
        }
        // (The `halls` rules over their halls.)
        layout_halls(world, t0, &l, li, &format!("#{li}"), &|b| key_of(b).is_some() && interior::hub_kind(t0, &l, b).is_some(), r);
    }
}

pub(crate) const INN_COUNTS: [&str; 15] = [
    "inns laid out by zones: floors fallen back on",
    "inns breaking an interior rule (as in interiors)",
    "inns missing a room of their programme",
    "inns missing a guest floor's rooms, one storey (reasoned absent)",
    "inns missing a snug, scullery or private dining room, a front 9 or less (reasoned absent)",
    "inn rooms over 2.5:1 (ways about the floor aside)",
    "inns over the threshold no frame fits (as small inns)",
    "inns below the threshold (as small inns)",
    "coaching inns whose yard no arch 2 squares wide opens onto from the street",
    "coaching inns with stables not opening onto the yard",
    "coaching inn rooms upstairs off neither the gallery nor a corridor",
    "coaching inns with another building on their ground",
    "coaching inns whose front door or arch opens 20 ft or more from a street",
    "coaching inns (merged, `town::palace::is_coaching`) not round a yard",
    "coaching inns round a yard (where, by the yard open to the sky: squares)",
];

/// Yurts (round homes on `town::round_off`'s rings): their doors face the nearest lane (within 70 ft) or
/// the layout's centre, and the interior's door lies on the ring's door side (its first side). Up to 8
/// per world are listed (`TOP=8`): seed, layout, building id, world ft x,y, the door's angle (degrees,
/// 0 east, 90 south: y runs down) and the aim's.
pub(crate) fn yurts(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["yurts", "doors within 30 degrees of their lane or centre", "doors facing south, within 22.5 degrees", "interior doors over 1.5 squares from the ring's door", "doors with their first 10 ft out under another roof", "yurts with no clear side (10 ft out, 10 ft wide, any side)", "yurts listed"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut listed = 0;
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.poly.len() == town::roof::ROUND_SIDES && b.parts.is_empty()) {
            let id = format!("b:{li}:{}", b.id);
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            let Some(lv) = it.levels.get(it.entry_level) else { continue };
            let Some(yi) = lv.rooms.iter().position(|q| q.kind == "yurt") else { continue };
            r.tally("yurts", 1);
            let n = b.poly.len() as f64;
            let c = b.poly.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
            let door = [0.5 * (b.poly[0][0] + b.poly[1][0]), 0.5 * (b.poly[0][1] + b.poly[1][1])];
            let ang = libm::atan2(door[1] - c[1], door[0] - c[0]).to_degrees();
            let mut near: Option<(f64, [f64; 2])> = None;
            for w in l.roads.iter().map(|w| &w.0).chain(l.streets.iter()) {
                for e in w.windows(2) {
                    let ab = [e[1][0] - e[0][0], e[1][1] - e[0][1]];
                    let t = (((c[0] - e[0][0]) * ab[0] + (c[1] - e[0][1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
                    let q = [e[0][0] + ab[0] * t, e[0][1] + ab[1] * t];
                    let d = libm::hypot(q[0] - c[0], q[1] - c[1]);
                    if d <= 70.0 && near.is_none_or(|b| d < b.0) {
                        near = Some((d, q));
                    }
                }
            }
            let aim = near.map(|n| n.1).unwrap_or(l.center);
            let aim_ang = libm::atan2(aim[1] - c[1], aim[0] - c[0]).to_degrees();
            let off = |a: f64, b: f64| ((a - b + 540.0) % 360.0 - 180.0).abs();
            if off(ang, aim_ang) <= 30.0 {
                r.tally("doors within 30 degrees of their lane or centre", 1);
            }
            if off(ang, 90.0) <= 22.5 {
                r.tally("doors facing south, within 22.5 degrees", 1);
            }
            let n = b.poly.len();
            let clear = |e: usize| {
                let m = [0.5 * (b.poly[e][0] + b.poly[(e + 1) % n][0]), 0.5 * (b.poly[e][1] + b.poly[(e + 1) % n][1])];
                let cc = town::geom::centroid(&b.poly);
                let d = [m[0] - cc[0], m[1] - cc[1]];
                let k = 1.0 / libm::hypot(d[0], d[1]).max(1e-9);
                [0.25, 0.5, 0.75, 1.0].iter().flat_map(|&t| [-5.0, -2.5, 0.0, 2.5, 5.0].map(|w| (t, w))).all(|(t, w)| {
                    let p = [m[0] + d[0] * k * town::YURT_CLEAR_FT * t - d[1] * k * w, m[1] + d[1] * k * town::YURT_CLEAR_FT * t + d[0] * k * w];
                    !l.buildings.iter().any(|o| o.id != b.id && o.structure == town::Structure::Roofed && libm::hypot(o.poly[0][0] - cc[0], o.poly[0][1] - cc[1]) < 400.0 && town::geom::contains(&o.poly, p))
                })
            };
            if !clear(0) {
                r.tally(if (0..n).any(clear) { "doors with their first 10 ft out under another roof" } else { "yurts with no clear side (10 ft out, 10 ft wide, any side)" }, 1);
            }
            if let Some(d) = lv.doors.iter().find(|d| d.kind == "front" && d.rooms.contains(&(yi as i16))) {
                let g = [0.5 * (d.a[0] + d.b[0]) as f64 * interior::SQUARE_FT, 0.5 * (d.a[1] + d.b[1]) as f64 * interior::SQUARE_FT];
                let w = [it.origin[0] + it.axis[0] * g[0] + it.across[0] * g[1], it.origin[1] + it.axis[1] * g[0] + it.across[1] * g[1]];
                let gap = libm::hypot(w[0] - door[0], w[1] - door[1]);
                if gap > 1.5 * interior::SQUARE_FT {
                    r.add("interior doors over 1.5 squares from the ring's door", gap, format!("{id} seed {}: {gap:.1} ft off, at {:.0},{:.0}", world.file.seed, c[0], c[1]));
                }
            }
            if listed < 8 {
                listed += 1;
                r.add("yurts listed", 0.0, format!("seed {} layout {li} {id} at {:.0},{:.0} door {ang:.0} aim {aim_ang:.0}", world.file.seed, c[0], c[1]));
            }
        }
    }
    yurt_looks(ctx, r);
    yurt_yards(ctx, r);
}

/// R5 towns (115): a steppe town's yurts (homes roofed round on 16-sided rings) by size (ft across) and
/// felt (`roof::Look::shade`), and their clusters (walls within 14 ft of one another): those whose doors
/// all lie within 60 degrees of one another, and those whose yurts all have one lane in reach (70 ft).
pub(crate) fn yurt_looks(ctx: &mut Ctx, r: &mut Report) {
    let bands = ["steppe yurts", "  under 14 ft across", "  14-16 ft", "  16-19 ft", "  19-22 ft", "  22-24 ft", "  over 24 ft"];
    r.counts(&bands);
    r.counts(&["  felt 0 cream", "  felt 1 off-white", "  felt 2 grey", "  felt 3 warm cream", "  felt 4 dusty grey", "  felt 5 smoke-browned", "  felt 6 red band", "  felt 7 sooty crown"]);
    r.counts(&["yurt clusters (2 or more)", "  clusters with one lane in reach of all", "  clusters whose doors share a side (all within 60 degrees)", "  of those with one lane: doors sharing a side"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let felts = ["cream", "off-white", "grey", "warm cream", "dusty grey", "smoke-browned", "red band", "sooty crown"];
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.culture % 5 != 4 {
            continue;
        }
        let ys: Vec<&town::Building> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.poly.len() == town::roof::ROUND_SIDES && b.parts.is_empty() && b.func.is_none() && town::roof::look(&l, b).shape == town::roof::Shape::Round).collect();
        let fr: Vec<(P, f64, f64)> = ys
            .iter()
            .map(|b| {
                let c = mean(&b.poly);
                let m = lerp(b.poly[0], b.poly[1], 0.5);
                (c, dist(b.poly[0], c), libm::atan2(m[1] - c[1], m[0] - c[0]).to_degrees())
            })
            .collect();
        for (k, b) in ys.iter().enumerate() {
            r.tally("steppe yurts", 1);
            let d = 2.0 * fr[k].1;
            r.tally(bands[if d < 13.9 { 1 } else if d < 16.0 { 2 } else if d < 19.0 { 3 } else if d < 22.0 { 4 } else if d <= 24.1 { 5 } else { 6 }], 1);
            let s = town::roof::look(&l, b).shade as usize;
            r.tally(&format!("  felt {s} {}", felts.get(s).unwrap_or(&"?")), 1);
        }
        let n = ys.len();
        let mut up: Vec<usize> = (0..n).collect();
        fn root(up: &mut [usize], mut a: usize) -> usize {
            while up[a] != a {
                a = up[a];
            }
            a
        }
        for a in 0..n {
            for b in a + 1..n {
                if dist(fr[a].0, fr[b].0) < fr[a].1 + fr[b].1 + 14.0 {
                    let (ra, rb) = (root(&mut up, a), root(&mut up, b));
                    if ra != rb {
                        up[ra.max(rb)] = ra.min(rb);
                    }
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for a in 0..n {
            let ra = root(&mut up, a);
            groups.entry(ra).or_default().push(a);
        }
        let ways: Vec<&Vec<P>> = l.roads.iter().map(|w| &w.0).chain(l.streets.iter()).collect();
        let off = |a: f64, b: f64| ((a - b + 540.0) % 360.0 - 180.0).abs();
        for g in groups.values().filter(|g| g.len() >= 2) {
            r.tally("yurt clusters (2 or more)", 1);
            let lane = ways.iter().any(|w| g.iter().all(|&k| w.windows(2).any(|e| seg_dist(fr[k].0, e[0], e[1]) <= 70.0)));
            let share = g.iter().all(|&a| g.iter().all(|&b| off(fr[a].2, fr[b].2) <= 60.0));
            if lane {
                r.tally("  clusters with one lane in reach of all", 1);
            }
            if share {
                r.tally("  clusters whose doors share a side (all within 60 degrees)", 1);
            }
            if share && lane {
                r.tally("  of those with one lane: doors sharing a side", 1);
            }
        }
    }
}

/// Whether two outlines touch or overlap (a corner of one in the other, or their edges crossing).
pub(crate) fn outlines_touch(a: &[P], b: &[P]) -> bool {
    let o = |p: P, q: P, s: P| cross(sub(q, p), sub(s, p));
    a.iter().any(|&p| contains(b, p))
        || b.iter().any(|&p| contains(a, p))
        || (0..a.len()).any(|i| {
            let (p, q) = (a[i], a[(i + 1) % a.len()]);
            (0..b.len()).any(|j| {
                let (s, t) = (b[j], b[(j + 1) % b.len()]);
                o(p, q, s) * o(p, q, t) < 0.0 && o(s, t, p) * o(s, t, q) < 0.0
            })
        })
}

/// R5 towns (115): a steppe town's yurts' trodden ground, pens and carts (`town::yurts`): counted, their
/// rules (none over a building, a lane, another's pen or cart, a door's open ground, or off its lot), and
/// on the battlemap of the first 40 yurts with a pen or cart and 40 without per world: the trodden ring
/// drawn as dirt, the props and fences laid, nothing else over a pen or a cart.
pub(crate) fn yurt_yards(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["yurts with trodden ground", "yurts with a pen", "  horses in pens", "yurts with a cart", "  with a haystack too", "yurts drawn up toward their door"]);
    r.counts(&["R pens and carts over a building", "R pens and carts over another's pen, cart or prop", "R pens and carts on a lane", "R pens and carts in a door's open ground", "R pens and carts off their lot"]);
    r.counts(&["battlemap: yurts looked at", "battlemap: trodden rings drawn (dirt by the wall)", "battlemap: yurt props planned", "battlemap: yurt props laid", "battlemap: pen fence ft planned", "battlemap: pen fence ft laid", "R battlemap: objects over a pen or cart"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let size = world.geom.tile_size_ft(world.geom.max_level);
    let mut look: Vec<(String, P, f64, Vec<[P; 4]>, Vec<(P, Kind)>, f64)> = Vec::new();
    let (mut with, mut without, mut listed) = (0, 0, 0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.yurts.is_empty() {
            continue;
        }
        let byid: HashMap<u32, &town::Building> = l.buildings.iter().map(|b| (b.id, b)).collect();
        // (Every yurt's door ground: 10 ft out from its door side, 10 ft wide.)
        let doors: Vec<(u32, Vec<P>)> = l
            .yurts
            .iter()
            .filter_map(|y| byid.get(&y.id).map(|b| (y.id, b)))
            .map(|(id, b)| {
                let c = mean(&b.poly);
                let m = lerp(b.poly[0], b.poly[1], 0.5);
                let n = mul(sub(m, c), 1.0 / dist(m, c).max(1e-9));
                (id, seg_rect(m, add(m, mul(n, town::YURT_CLEAR_FT)), 10.0).to_vec())
            })
            .collect();
        let mut all: Vec<(u32, Vec<P>)> = Vec::new();
        for y in &l.yurts {
            r.tally("yurts with trodden ground", (y.trodden.len() >= 3) as usize);
            let Some(b) = byid.get(&y.id) else { continue };
            let c = mean(&b.poly);
            let rr = dist(b.poly[0], c);
            let mut boxes: Vec<[P; 4]> = Vec::new();
            let mut planned: Vec<(P, Kind)> = Vec::new();
            let mut fence = 0.0;
            if let Some(p) = y.pen {
                r.tally("yurts with a pen", 1);
                if listed < 4 {
                    listed += 1;
                    r.add("  pens listed (fx, fy: the map's)", 0.0, format!("seed {} b:{li}:{} at {} fx {:.6} fy {:.6}", world.file.seed, y.id, at(c), c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft));
                }
                boxes.push(p.pts);
                fence = (0..4).map(|e| dist(p.pts[e], p.pts[(e + 1) % 4])).sum::<f64>() - town::yurts::PEN_GATE_FT;
            }
            let mine: Vec<&town::Prop> = l.props.iter().filter(|p| matches!(p.kind, Kind::Horse | Kind::Cart | Kind::Haystack) && contains(&y.lot, p.at)).collect();
            for p in &mine {
                planned.push((p.at, p.kind));
                match p.kind {
                    Kind::Horse => r.tally("  horses in pens", 1),
                    Kind::Cart => {
                        r.tally("yurts with a cart", 1);
                        boxes.push(horse_foot(p.kind, p.at, p.rot, p.scale).unwrap());
                    }
                    _ => {
                        r.tally("  with a haystack too", 1);
                        boxes.push(horse_foot(p.kind, p.at, p.rot, p.scale).unwrap());
                    }
                }
            }
            let ring = town::roof::round_footprint_sized(&y.lot, &|_| rr);
            if ring.is_some_and(|g| dist(mean(&g), c) > 0.25) {
                r.tally("yurts drawn up toward their door", 1);
            }
            let here = format!("seed {} b:{li}:{} at {}", world.file.seed, y.id, at(c));
            for bx in &boxes {
                if l.buildings.iter().any(|o| o.id != y.id && o.structure == town::Structure::Roofed && dist(o.poly[0], c) < 150.0 && outlines_touch(bx, &o.poly)) {
                    r.add("R pens and carts over a building", 0.0, here.clone());
                }
                if all.iter().any(|(id, o)| *id != y.id && outlines_touch(bx, o)) || l.props.iter().any(|p| !mine.iter().any(|q| std::ptr::eq(*q, p)) && contains(bx, p.at)) {
                    r.add("R pens and carts over another's pen, cart or prop", 0.0, here.clone());
                }
                let lane = l.roads.iter().any(|w| w.0.windows(2).any(|s| bx.iter().any(|&p| seg_dist(p, s[0], s[1]) < 0.5 * w.2) || boxes_cross(bx, &seg_rect(s[0], s[1], w.2))))
                    || l.streets.iter().any(|w| w.windows(2).any(|s| boxes_cross(bx, &seg_rect(s[0], s[1], 10.0))));
                if lane {
                    r.add("R pens and carts on a lane", 0.0, here.clone());
                }
                if doors.iter().any(|(_, d)| boxes_cross(bx, d)) {
                    r.add("R pens and carts in a door's open ground", 0.0, here.clone());
                }
                if !bx.iter().all(|&p| contains(&y.lot, p)) {
                    r.add("R pens and carts off their lot", 0.0, here.clone());
                }
            }
            for bx in &boxes {
                all.push((y.id, bx.to_vec()));
            }
            let extra = !boxes.is_empty();
            if (extra && with < 40) || (!extra && without < 40) {
                if extra {
                    with += 1;
                } else {
                    without += 1;
                }
                look.push((here, c, rr, if y.pen.is_some() { vec![y.pen.unwrap().pts] } else { boxes.clone() }, planned, fence));
            }
        }
    }
    for (here, c, rr, boxes, planned, fence) in look {
        r.tally("battlemap: yurts looked at", 1);
        let reach = rr + 30.0;
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let (mut dirt, mut laid, mut flen, mut over) = (0usize, 0usize, 0.0f64, 0usize);
        for ky in k(c[1] - reach)..=k(c[1] + reach) {
            for kx in k(c[0] - reach)..=k(c[0] + reach) {
                let ch = ctx.chunk(kx, ky);
                let (ox, oy) = (kx as f64 * size, ky as f64 * size);
                for q in 0..SQ * SQ {
                    let p = [ox + ((q % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    let d = dist(p, c);
                    if d > rr && d < rr + town::yurts::TRODDEN_FT + 2.5 && ch.building[q] == 0 && ch.surface[q] == Surface::Dirt {
                        dirt += 1;
                    }
                }
                for o in &ch.objects {
                    let p = [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
                    if planned.iter().any(|q| q.1 == o.kind && dist(q.0, p) < 0.5) {
                        laid += 1;
                    } else if boxes.iter().any(|b| contains(b, p)) {
                        over += 1;
                        r.add("R battlemap: objects over a pen or cart", 0.0, format!("{here}: {} at {}", ch.name(o), at(p)));
                    }
                }
                for s in ch.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Fence && s.pts.len() >= 2) {
                    let f = |q: [f32; 2]| [ox + q[0] as f64 * SQUARE_FT, oy + q[1] as f64 * SQUARE_FT];
                    let (a, e) = (f(s.pts[0]), f(s.pts[1]));
                    if boxes.iter().any(|b| (0..4).any(|j| seg_dist(lerp(a, e, 0.5), b[j], b[(j + 1) % 4]) < 1.0)) {
                        flen += dist(a, e);
                    }
                }
            }
        }
        let _ = over;
        r.tally("battlemap: trodden rings drawn (dirt by the wall)", (dirt > 0) as usize);
        r.tally("battlemap: yurt props planned", planned.len());
        r.tally("battlemap: yurt props laid", laid);
        r.tally("battlemap: pen fence ft planned", fence as usize);
        r.tally("battlemap: pen fence ft laid", flen as usize);
        if dirt == 0 || laid < planned.len() || flen + 1.0 < fence {
            r.add("battlemap: yurts short of dirt, props or fence", 0.0, format!("{here}: dirt squares {dirt}, props {laid}/{}, fence {flen:.0}/{fence:.0} ft", planned.len()));
        }
    }
}

/// Inns and caravanserais (`interior::hub::inn`): the rules in the header's `inns`.
pub(crate) fn inns(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["inns and caravanserais", "  laid out by zones"]);
    r.counts(&INN_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let letter = |x: i16| if x < 0 { '-' } else { (b'a' + (x as u8 % 26)) as char };
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let Some(key) = b.func.map(|f| town::catalog::CATALOG[f as usize].key).filter(|k| matches!(*k, "inn" | "caravanserai")) else { continue };
            r.tally("inns and caravanserais", 1);
            r.tally(&format!("  {key}s"), 1);
            let id = format!("b:{li}:{}", b.id);
            let Some(kind) = interior::hub_kind(t0, &l, b) else {
                let sh = interior::Shell::of(&l, b);
                let n = sh.inside.iter().filter(|&&i| i).count();
                let row = if n >= interior::hub::INN_LEAST { 6 } else { 7 };
                // (Its box and how full it is: a slanted or ragged lot's under 80%.)
                let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
                for k in (0..sh.nx * sh.ny).filter(|&k| sh.inside[k]) {
                    (x0, x1, y0, y1) = (x0.min(k % sh.nx), x1.max(k % sh.nx), y0.min(k / sh.nx), y1.max(k / sh.nx));
                }
                let (bw, bh) = (x1.wrapping_sub(x0).wrapping_add(1), y1.wrapping_sub(y0).wrapping_add(1));
                let full = 100 * n / (bw * bh).max(1);
                r.add(INN_COUNTS[row], n as f64, format!("{id} ({key}): {n} squares, box {bw}x{bh} {full}% full, {} floors", b.floors));
                continue;
            };
            r.tally("  laid out by zones", 1);
            r.tally(&format!("    {}s", kind.name()), 1);
            r.tally(&format!("    {} storeys", b.floors.max(1)), 1);
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            let tag = format!("{id} ({})", kind.name());
            if std::env::var("PLAN").is_ok_and(|p| p.trim() == id) {
                for lv in &it.levels {
                    say!("-- {} (z {}): {}", lv.name, lv.z, lv.rooms.iter().enumerate().map(|(k, r)| format!("{}={}{}", letter(k as i16), r.kind, r.squares)).collect::<Vec<_>>().join(", "));
                    for j in 0..it.ny {
                        let row: String = (0..it.nx)
                            .map(|i| {
                                let st = it.stairs;
                                if lv.has_stairs && i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3] { '#' } else if lv.cells[j * it.nx + i] < 0 { ' ' } else { letter(lv.cells[j * it.nx + i]) }
                            })
                            .collect();
                        say!("  {row}");
                    }
                    let doors: Vec<String> = lv.doors.iter().map(|d| format!("{}{}{}", letter(d.rooms[0]), letter(d.rooms[1]), if d.kind == "front" { "!" } else { "" })).collect();
                    say!("  doors: {}", doors.join(" "));
                    let pieces: Vec<String> = lv.furniture.iter().map(|f| format!("{}@{},{}", f.kind, f.x, f.y)).collect();
                    say!("  pieces: {}", pieces.join(" "));
                }
            }
            let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
            if !fell.is_empty() {
                r.add(INN_COUNTS[0], fell.len() as f64, format!("{tag}: z {fell:?}"));
            }
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            let bad = check_interior(world, t0, &l, li, &id, &it, buckets);
            if !bad.is_empty() {
                r.add(INN_COUNTS[1], bad.len() as f64, format!("{tag}: {}", bad.join("; ")));
            }
            // (The programme: the ground floor's, then the guest floors'.)
            let has = |k: &str| it.levels.iter().any(|lv| lv.rooms.iter().any(|r| r.kind == k && r.squares > 0));
            let ground = ["taproom", "snug", "private dining room", "kitchen", "scullery", "storeroom"];
            let upper = ["best chamber", "guest room", "dormitory", "innkeeper's quarters"];
            let above = it.levels.iter().filter(|lv| lv.z >= 0 && !lv.roof).count();
            let wid = interior::zone_plan_of(world, t0, li, b.id as usize).map(|p| p.frame().wid);
            let (mut miss, mut narrow, mut one) = (Vec::new(), Vec::new(), Vec::new());
            for k in ground.iter().chain(&upper).copied().filter(|k| !has(k)) {
                if upper.contains(&k) && above <= 1 {
                    one.push(k);
                } else if matches!(k, "snug" | "scullery" | "private dining room") && wid.is_some_and(|w| w <= 9) {
                    narrow.push(k);
                } else {
                    miss.push(k);
                }
            }
            for (row, list) in [(2, &miss), (3, &one), (4, &narrow)] {
                if !list.is_empty() {
                    r.add(INN_COUNTS[row], list.len() as f64, format!("{tag}, {above} storeys, front {wid:?}: no {}", list.join(", ")));
                }
            }
            if kind == interior::hub::Kind::Caravanserai {
                r.tally(if has("stables") { "  caravanserais with stables" } else { "  caravanserais without stables (no room)" }, 1);
            }
            // (A coaching inn's yard: reached from the street through its arch, its stables onto it,
            // every room upstairs off the gallery, nothing on its ground, its doors on a street.)
            if town::palace::is_coaching(&l, b) {
                let yard = interior::zone_plan_of(world, t0, li, b.id as usize).and_then(|p| p.yard());
                let Some(((ys0, ys1, yt0, yt1), _)) = yard else {
                    r.add(INN_COUNTS[13], 1.0, format!("{tag}: {} floors at {}", b.floors, at(centroid(&b.poly))));
                    continue;
                };
                r.tally(&format!("  coaching inns round a yard: {:?} {key}s", l.tier), 1);
                r.add(INN_COUNTS[14], (ys1 - ys0) as f64 * (yt1 - yt0) as f64, format!("{tag}: {:?}, yard open {}x{}, {} floors at {}", l.tier, ys1 - ys0, yt1 - yt0, b.floors, at(centroid(&b.poly))));
                r.tally(&format!("    yards open {}x{}", ys1 - ys0, yt1 - yt0), 1);
                let g = it.levels.iter().find(|lv| lv.z == 0).expect("ground floor");
                let yr = g.rooms.iter().position(|rm| rm.kind == "inn yard").map(|k| k as i16);
                let arches = g.doors.iter().filter(|d| d.kind == "arch" && Some(d.rooms[0]) == yr).count();
                if arches < 2 {
                    r.add(INN_COUNTS[8], 1.0, format!("{tag}: {arches} arch edges"));
                }
                for (ri, _) in g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == "stables" && rm.squares > 0) {
                    if !g.doors.iter().any(|d| d.rooms.contains(&(ri as i16)) && yr.is_some_and(|y| d.rooms.contains(&y))) {
                        r.add(INN_COUNTS[9], 1.0, format!("{tag}: stables {ri}"));
                    }
                }
                for lv in it.levels.iter().filter(|lv| lv.z >= 1) {
                    let way = |x: i16| x >= 0 && matches!(lv.rooms[x as usize].kind, "yard gallery" | "corridor");
                    for (ri, rm) in lv.rooms.iter().enumerate().filter(|(_, rm)| rm.squares > 0 && matches!(rm.kind, "best chamber" | "guest room" | "dormitory" | "innkeeper's quarters")) {
                        if !lv.doors.iter().any(|d| d.rooms.contains(&(ri as i16)) && (way(d.rooms[0]) || way(d.rooms[1]))) {
                            r.add(INN_COUNTS[10], 1.0, format!("{tag} {}: {} {ri}", lv.name, rm.kind));
                        }
                    }
                }
                for (_, gr) in l.grounds.iter().filter(|g| g.0 == b.id) {
                    for o in l.buildings.iter().filter(|o| o.id != b.id && poly_overlap(&shrink(&o.poly, 1.0), &shrink(gr, 1.0))) {
                        r.add(INN_COUNTS[11], 1.0, format!("{tag}: b:{li}:{} at {}", o.id, at(centroid(&o.poly))));
                    }
                }
                let sh = interior::Shell::of(&l, b);
                let street = |p: [f64; 2]| l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| seg_dist(p, q[0], q[1]) - 0.5 * w)).fold(f64::MAX, f64::min);
                for d in g.doors.iter().filter(|d| matches!(d.kind, "front" | "arch")) {
                    let m = sh.world_at(0.5 * (d.a[0] + d.b[0]) as f64, 0.5 * (d.a[1] + d.b[1]) as f64);
                    if street(m) >= 20.0 {
                        r.add(INN_COUNTS[12], street(m), format!("{tag}: {} door {:.0} ft from a street at {}", d.kind, street(m), at(m)));
                    }
                }
            }
            // (Rooms per kind, their proportions by their boxes, and the pieces that make them what
            // they are.)
            for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
                for (ri, room) in lv.rooms.iter().enumerate() {
                    if room.squares == 0 {
                        continue;
                    }
                    r.tally(&format!("  rooms: {}", room.kind), 1);
                    let piece = match room.kind {
                        "best chamber" => Some("bed"),
                        "dormitory" => Some("cot"),
                        "snug" => Some("table"),
                        "private dining room" => Some("long_table"),
                        "stables" => Some("stable_stall"),
                        _ => None,
                    };
                    if let Some(piece) = piece
                        && !lv.furniture.iter().any(|f| f.kind == piece && lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16)
                    {
                        r.tally(&format!("    {} without a {piece}", room.kind), 1);
                    }
                    if room.squares < 4 || interior::is_circulation(room.kind) || !interior::walkable(room.kind) {
                        continue;
                    }
                    let sq: Vec<usize> = (0..it.nx * it.ny).filter(|&k| lv.cells[k] == ri as i16).collect();
                    let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % it.nx), a.1.max(k % it.nx)));
                    let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / it.nx), a.1.max(k / it.nx)));
                    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
                    if 2 * w.max(h) > 5 * w.min(h) {
                        r.add(INN_COUNTS[5], w.max(h) as f64 / w.min(h) as f64, format!("{tag} {}: {} {w}x{h}", lv.name, room.kind));
                    }
                }
            }
        }
    }
}

pub(crate) fn hashes(ctx: &mut Ctx, label: &str, r: &mut Report) {
    use worldgen::core::hash::fnv64;
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let castle = if !l.castles.is_empty() || l.buildings.iter().any(|b| b.ward == Ward::Castle) { "C" } else { "-" };
        // (Without the keep's mark, nor an empty list of forecourts or grounds, so that the hashes
        // compare with trees from before them.)
        let mut text = format!("{l:?}").replace(", lined: None }", " }").replace(", deck: None }", " }").replace(", grounds: [] }", " }").replace(", keep: false }", " }").replace(", keep: true }", " }").replace(", keep: None }", " }").replace(", forecourts: [] }", " }").replace(", wheels: [] }", " }").replace(", crofts: [] }", " }").replace(", wings: []", "");
        for k in ["Great", "Hall", "L", "T", "Shell", "Plain"] {
            text = text.replace(&format!(", keep: Some({k}) }}"), " }");
        }
        // (Nor the fields' info, v63: their furrows, use and bounds.)
        if let Some(a) = text.find(", field_info: [") {
            if let Some(b) = text[a..].find(']') {
                text.replace_range(a..a + b + 1, "");
            }
        }
        // (Nor the street pattern, recorded since 189: a town on the grid is laid out as before.)
        if let Some(a) = text.find(", pattern: 0, pattern_mark: 0") {
            text.replace_range(a..a + ", pattern: 0, pattern_mark: 0".len(), "");
        }
        // (Nor the castle's and the market's patches, recorded later.)
        if let Some(a) = text.find(", castle_patch: ")
            && let Some(b) = text[a..].find(", bbox: ")
        {
            text.replace_range(a..a + b, "");
        }
        println!("{label} L:{li} {:016x} {castle} {}", fnv64(text.as_bytes()), l.buildings.len());
        r.tally("layouts hashed", 1);
        // (Each building's own, with what it is: which buildings a change of the layout's is in.)
        for b in &l.buildings {
            println!("{label} B:{li}:{} {:016x} {castle} {}", b.id, fnv64(format!("{b:?}").as_bytes()), b.label().replace(' ', "_"));
        }
        let mut ids: Vec<String> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed).map(|b| format!("b:{li}:{}", b.id)).collect();
        ids.extend((0..interior::towers(&l).len()).map(|k| format!("t:{li}:{k}")));
        for id in ids {
            match interior::generate_id(world, t0, &id) {
                Some(it) => println!("{label} {id} {:016x} {castle} {}", fnv64(serde_json::to_string(&it).unwrap().as_bytes()), it.function.replace(' ', "_")),
                None => println!("{label} {id} none {castle}"),
            }
            r.tally("interiors hashed", 1);
        }
    }
}

/// `same`: every layout's hash, and every town's with a scripted street-editor edit, against a baseline.
pub(crate) fn same(ctx: &mut Ctx, label: &str, r: &mut Report) {
    use worldgen::core::hash::fnv64;
    r.counts(&["layouts hashed", "edited layouts hashed", "edits refused", "layouts differing", "layouts not in the baseline"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut lines: Vec<String> = Vec::new();
    // (P7: a moat's one drawbridge became a list; printed as before, a moat with one compares
    // with the baseline.)
    let print = |l: &Layout| {
        let s = format!("{l:?}");
        if s.contains("bridges: [Drawbridge {") { s.replace("bridges: [Drawbridge {", "bridge: Drawbridge {").replace("}], pieces: ", "}, pieces: ") } else { s }
    };
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        lines.push(format!("L:{li} {:016x} {}", fnv64(print(&l).as_bytes()), l.buildings.len()));
        r.tally("layouts hashed", 1);
    }
    let edits: usize = std::env::var("SAME_EDITS").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let towns = (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town);
    for ci in towns.take(edits) {
        let edit = match scripted_edit(world, t0, ci) {
            Some(Ok(edit)) => edit,
            None => continue,
            Some(Err((e, req))) => {
                r.add("edits refused", 1.0, format!("{label} #{ci}: {e} ({req})"));
                continue;
            }
        };
        let mut file = world.file.clone();
        file.edits.towns.insert(town::wards::key(ci), edit);
        let w = World::new(file).unwrap();
        let l = town::layout(&w, t0, ci);
        lines.push(format!("E:{ci} {:016x} {}", fnv64(print(&l).as_bytes()), l.buildings.len()));
        r.tally("edited layouts hashed", 1);
    }
    let dir = std::path::PathBuf::from(std::env::var("SAME_DIR").unwrap_or_else(|_| "target/towncheck-same".into()));
    let path = dir.join(format!("{}.txt", label.replace(['/', '\\'], "_")));
    let base = std::fs::read_to_string(&path).ok().filter(|_| std::env::var("SAME").as_deref() != Ok("write"));
    let Some(base) = base else {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        say!("  baseline written: {} ({} lines)", path.display(), lines.len());
        return;
    };
    let was: HashMap<&str, &str> = base.lines().filter_map(|x| x.split_once(' ')).collect();
    for x in &lines {
        let (k, v) = x.split_once(' ').unwrap();
        match was.get(k) {
            None => r.add("layouts not in the baseline", 1.0, format!("{label} {k}")),
            Some(b) if b != &v => r.add("layouts differing", 1.0, format!("{label} {k}: {b} -> {v}")),
            _ => {}
        }
    }
}

/// SVGs written by `fills` so far (up to 6 a run).
pub(crate) static FILL_SVGS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `fills`: hand-laid blocks filled through `town::qa_fill` (`Block::Region`). Each town's blocks made concave four
/// ways (`fill_variant`), each filled twice and once with its rings rotated, checked (`fill_rules`), and its built area
/// per area set against the convex blocks it came from filled the same way.
pub(crate) fn fills(ctx: &mut Ctx, label: &str, r: &mut Report) {
    use town::clip::{self, Shape};
    use worldgen::core::hash::fnv64;
    const KINDS: [&str; 4] = ["corner cut, fronting", "corner cut, not fronting", "courtyard", "joined"];
    r.counts(&["fills", "buildings", "variants not made", "buildings outside their region or over a hole", "buildings overlapping", "buildings with no front", "fills not the same twice", "fills not the same with rings rotated", "convex blocks' buildings (as one region)", "convex blocks' buildings with no front"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let dir = std::path::PathBuf::from(std::env::var("SVG_DIR").unwrap_or_else(|_| "target/towncheck-fills".into()));
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town {
            continue;
        }
        // Each block anticlockwise, with the ward of a building in it (blocks with none left out).
        let blocks: Vec<(Vec<P>, Ward)> = l
            .blocks
            .iter()
            .filter_map(|b| {
                let b: Vec<P> = if area(b) < 0.0 { b.iter().rev().copied().collect() } else { b.clone() };
                let w = l.buildings.iter().find(|h| contains(&b, centroid(&h.poly)))?.ward;
                Some((b, w))
            })
            .collect();
        // A convex block alone, filled with the lots of its ward: (built area, area).
        // (And its buildings with no front, to set the rule's own misses against.)
        let convex = |ring: &[P], ward: Ward, seed: u64| {
            let fronts = fronts_on(ring, &edge_lines(ring, None));
            let (houses, alleys) = town::qa_fill(&[Shape::ring(ring.to_vec())], &fronts, ward, seed, l.tier);
            let fronted = fronted_by(&fronts, &alleys);
            (houses.iter().map(|h| area(h).abs()).sum::<f64>(), area(ring).abs(), houses.len(), houses.iter().filter(|h| !fronted(h)).count())
        };
        for (bi, (b, ward)) in blocks.iter().enumerate() {
            let seed = fnv64(format!("{label} {li} {bi}").as_bytes());
            let o = centroid(b);
            let local = |ring: &[P]| ring.iter().map(|p| sub(*p, o)).collect::<Vec<P>>();
            let ring = local(b);
            for (k, kind) in KINDS.iter().enumerate() {
                let tag = format!("{label} L{li} block {bi} ({kind})");
                let Some((region, lines, refs)) = fill_variant(k, &ring, &blocks.iter().enumerate().filter(|(j, _)| *j != bi).map(|(j, x)| (j, local(&x.0))).collect::<Vec<_>>()) else {
                    r.tally("variants not made", 1);
                    continue;
                };
                let fronts = fronts_on(&region[0].outer, &lines);
                let (houses, alleys) = town::qa_fill(&region, &fronts, *ward, seed, l.tier);
                r.tally("fills", 1);
                r.tally("buildings", houses.len());
                if town::qa_fill(&region, &fronts, *ward, seed, l.tier) != (houses.clone(), alleys.clone()) {
                    r.add("fills not the same twice", 1.0, tag.clone());
                }
                let rot = |v: &Vec<P>| {
                    let mut v = v.clone();
                    v.rotate_left(1);
                    v
                };
                let turned: Vec<Shape> = region.iter().map(|s| Shape { outer: rot(&s.outer), holes: s.holes.iter().map(rot).collect() }).collect();
                if town::qa_fill(&turned, &fronts, *ward, seed, l.tier) != (houses.clone(), alleys.clone()) {
                    r.add("fills not the same with rings rotated", 1.0, tag.clone());
                }
                fill_rules(&region, &fronts, &houses, &alleys, &tag, o, r);
                let (mut built, mut whole) = (0.0, 0.0);
                for (j, rr) in refs.iter().enumerate() {
                    let (bt, a, n, lone) = convex(rr, *ward, seed ^ j as u64);
                    built += bt;
                    whole += a;
                    r.tally("convex blocks' buildings (as one region)", n);
                    r.tally("convex blocks' buildings with no front", lone);
                }
                let share = houses.iter().map(|h| area(h).abs()).sum::<f64>() / clip::area(&region).max(1.0);
                if built > 0.0 {
                    r.value(&format!("built share against convex: {kind}"), share / (built / whole), tag.clone());
                }
                // (The first six, each a variant in turn from a later town; `FILL_SVG=text`: also every fill whose tag holds it.)
                let n = FILL_SVGS.load(Ordering::Relaxed);
                let asked = std::env::var("FILL_SVG").is_ok_and(|t| tag.contains(&t));
                if asked || (n < 6 && n % 4 == k && li >= 3 * n) {
                    FILL_SVGS.store(n + 1, Ordering::Relaxed);
                    std::fs::create_dir_all(&dir).unwrap();
                    let path = dir.join(format!("fills-{label}-{li}-{bi}-{k}.svg"));
                    std::fs::write(&path, fill_svg(&region, &fronts, &houses, &alleys)).unwrap();
                    say!("  svg: {}", path.display());
                }
            }
        }
    }
}

/// The street line along edge `e` of an anticlockwise ring (outward normal, offset), as `town` keeps them.
pub(crate) fn edge_line(ring: &[P], e: usize) -> Option<(P, f64)> {
    let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
    let el = dist(a, b);
    (el > 1e-9).then(|| {
        let n = [(b[1] - a[1]) / el, -(b[0] - a[0]) / el];
        (n, dot(a, n))
    })
}

/// The street lines along every edge of an anticlockwise ring but `skip`.
pub(crate) fn edge_lines(ring: &[P], skip: Option<usize>) -> Vec<(P, f64)> {
    (0..ring.len()).filter(|&e| Some(e) != skip).filter_map(|e| edge_line(ring, e)).collect()
}

/// Fronts on `lines` where `outer` runs along them (within 0.5 ft); lines it doesn't run along are left out.
pub(crate) fn fronts_on(outer: &[P], lines: &[(P, f64)]) -> Vec<town::Front> {
    let m = outer.len();
    lines
        .iter()
        .filter_map(|&(n, c)| {
            let t = [-n[1], n[0]];
            let on = |p: P| (dot(p, n) - c).abs() < 0.5;
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for i in 0..m {
                let (p, q) = (outer[i], outer[(i + 1) % m]);
                if on(p) && on(q) {
                    lo = lo.min(dot(p, t)).min(dot(q, t));
                    hi = hi.max(dot(p, t)).max(dot(q, t));
                }
            }
            (hi > lo).then(|| town::Front { n, c, a: add(mul(n, c), mul(t, lo)), b: add(mul(n, c), mul(t, hi)) })
        })
        .collect()
}

/// A block (anticlockwise, about its centroid) made concave: 0, a corner cut out with its new sides fronting a
/// street; 1, the same with them not fronting; 2, a courtyard hole; 3, joined to a neighbour of `others` across their
/// street (two edges facing 2 to 40 ft apart along 30 ft or more). Gives the region (one shape), its street lines and
/// the convex blocks it came from; none when the variant can't be made (or doesn't come out concave).
pub(crate) fn fill_variant(k: usize, ring: &[P], others: &[(usize, Vec<P>)]) -> Option<(Vec<town::clip::Shape>, Vec<(P, f64)>, Vec<Vec<P>>)> {
    use town::clip::{self, Shape};
    let m = ring.len();
    let unit = |v: P| mul(v, 1.0 / len(v).max(1e-9));
    let (region, lines, refs) = match k {
        0 | 1 => {
            let s = 0.4 * dist(ring[0], ring[1]).min(dist(ring[m - 1], ring[0]));
            if s < 25.0 {
                return None;
            }
            let (v0, u) = (ring[0], unit(sub(ring[1], ring[0])));
            let w = [-u[1], u[0]];
            let q = |x: f64, y: f64| add(v0, add(mul(u, x), mul(w, y)));
            let cut = vec![q(-2000.0, -2000.0), q(s, -2000.0), q(s, s), q(-2000.0, s)];
            let mut lines = edge_lines(ring, None);
            if k == 0 {
                lines.push((mul(w, -1.0), dot(q(0.0, s), mul(w, -1.0))));
                lines.push((mul(u, -1.0), dot(q(s, 0.0), mul(u, -1.0))));
            }
            (clip::difference(&[Shape::ring(ring.to_vec())], &[Shape::ring(cut)]), lines, vec![ring.to_vec()])
        }
        2 => {
            let hole: Vec<P> = ring.iter().rev().map(|p| mul(*p, 0.4)).collect();
            (clip::normalize(&[Shape { outer: ring.to_vec(), holes: vec![hole] }]), edge_lines(ring, None), vec![ring.to_vec()])
        }
        _ => {
            let mut found = None;
            'pairs: for (_, other) in others {
                let mo = other.len();
                for ei in 0..m {
                    let (p0, p1) = (ring[ei], ring[(ei + 1) % m]);
                    let (u, li) = (unit(sub(p1, p0)), dist(p0, p1));
                    let n = [u[1], -u[0]];
                    for ej in 0..mo {
                        let (q0, q1) = (other[ej], other[(ej + 1) % mo]);
                        let uj = unit(sub(q1, q0));
                        let (g0, g1) = (dot(sub(q0, p0), n), dot(sub(q1, p0), n));
                        if dot(u, uj) > -0.98 || !(2.0..40.0).contains(&g0) || !(2.0..40.0).contains(&g1) {
                            continue;
                        }
                        let (tq0, tq1) = (dot(sub(q0, p0), u), dot(sub(q1, p0), u));
                        let (t0, t1) = (tq0.min(tq1).max(0.0), tq0.max(tq1).min(li));
                        if t1 - t0 < 30.0 {
                            continue;
                        }
                        let foot = |x: P| add(add(q0, mul(uj, dot(sub(x, q0), uj))), n);
                        let (a0, a1) = (add(p0, mul(u, t0)), add(p0, mul(u, t1)));
                        let bridge = vec![sub(a0, n), sub(a1, n), foot(a1), foot(a0)];
                        found = Some((ei, other.clone(), ej, bridge));
                        break 'pairs;
                    }
                }
            }
            let (ei, other, ej, bridge) = found?;
            let mut lines = edge_lines(ring, Some(ei));
            lines.extend(edge_lines(&other, Some(ej)));
            (clip::union_all(&[Shape::ring(ring.to_vec()), Shape::ring(other.clone()), Shape::ring(bridge)]), lines, vec![ring.to_vec(), other])
        }
    };
    let concave = region.len() == 1 && (!region[0].holes.is_empty() || !clip::is_convex(&region[0].outer));
    concave.then_some((region, lines, refs))
}

/// One fill's rules: buildings outside the region or over a hole (by more than 1 ft²), overlapping (the same), and
/// with no front (`fronted_by`).
pub(crate) fn fill_rules(region: &[town::clip::Shape], fronts: &[town::Front], houses: &[Vec<P>], alleys: &[(P, P)], tag: &str, o: P, r: &mut Report) {
    use town::clip::{self, Shape};
    let fronted = fronted_by(fronts, alleys);
    let boxes: Vec<[f64; 4]> = houses.iter().map(|h| bbox(h)).collect();
    for (i, h) in houses.iter().enumerate() {
        let c = add(o, centroid(h));
        let out = clip::area(&clip::difference(&[Shape::ring(h.clone())], region));
        if out > 1.0 {
            r.add("buildings outside their region or over a hole", out, format!("{tag}: {out:.0} ft² at {}", at(c)));
        }
        for j in i + 1..houses.len() {
            let (a, b) = (boxes[i], boxes[j]);
            if a[0] > b[2] || b[0] > a[2] || a[1] > b[3] || b[1] > a[3] {
                continue;
            }
            let x = clip::area(&clip::intersection(&[Shape::ring(h.clone())], &[Shape::ring(houses[j].clone())]));
            if x > 1.0 {
                r.add("buildings overlapping", x, format!("{tag}: {x:.0} ft² at {}", at(c)));
            }
        }
        if !fronted(h) {
            r.add("buildings with no front", area(h).abs(), format!("{tag}: at {}", at(c)));
        }
    }
}

/// Whether a building fronts a street of `fronts` or a side of an alley (centre lines, 16 ft wide): an edge over 4 ft
/// on its line or up to 16.5 ft back from it, along its stretch by more than 4 ft.
pub(crate) fn fronted_by(fronts: &[town::Front], alleys: &[(P, P)]) -> impl Fn(&[P]) -> bool {
    let mut all: Vec<town::Front> = fronts.to_vec();
    for &(a, b) in alleys {
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        for n in [[u[1], -u[0]], [-u[1], u[0]]] {
            let half = 8.0;
            all.push(town::Front { n, c: dot(a, n) - half, a: sub(a, mul(n, half)), b: sub(b, mul(n, half)) });
        }
    }
    move |h: &[P]| {
        let m = h.len();
        (0..m).any(|i| {
            let (p, q) = (h[i], h[(i + 1) % m]);
            dist(p, q) > 4.0
                && all.iter().any(|f| {
                    let (dp, dq) = (f.c - dot(p, f.n), f.c - dot(q, f.n));
                    let t = [-f.n[1], f.n[0]];
                    let (s0, s1) = (dot(f.a, t).min(dot(f.b, t)), dot(f.a, t).max(dot(f.b, t)));
                    (-0.5..16.5).contains(&dp) && (dp - dq).abs() < 0.5 && dot(p, t).max(dot(q, t)).min(s1) - dot(p, t).min(dot(q, t)).max(s0) > 4.0
                })
        })
    }
}

/// A fill drawn: the region (grey, holes white), its fronts (red), alleys (dashed) and buildings (brown).
pub(crate) fn fill_svg(region: &[town::clip::Shape], fronts: &[town::Front], houses: &[Vec<P>], alleys: &[(P, P)]) -> String {
    let pts: Vec<P> = region.iter().flat_map(|s| s.outer.iter().copied()).collect();
    let b = bbox(&pts);
    let pad = 20.0;
    let (w, h) = (b[2] - b[0] + 2.0 * pad, b[3] - b[1] + 2.0 * pad);
    let xy = |p: P| format!("{:.1},{:.1}", p[0] - b[0] + pad, b[3] - p[1] + pad);
    let ring = |r: &[P]| format!("M{}Z", r.iter().map(|p| xy(*p)).collect::<Vec<_>>().join(" L"));
    let mut s = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{:.0}\" height=\"{:.0}\">\n<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n", w * 2.0, h * 2.0);
    for sh in region {
        let d: String = std::iter::once(&sh.outer).chain(sh.holes.iter()).map(|r| ring(r)).collect();
        s += &format!("<path d=\"{d}\" fill=\"#ddd\" fill-rule=\"evenodd\" stroke=\"#888\" stroke-width=\"0.5\"/>\n");
    }
    for &(a, c) in alleys {
        s += &format!("<path d=\"M{} L{}\" stroke=\"#999\" stroke-width=\"16\" stroke-opacity=\"0.4\"/>\n", xy(a), xy(c));
    }
    for f in fronts {
        s += &format!("<path d=\"M{} L{}\" stroke=\"#c33\" stroke-width=\"2\"/>\n", xy(f.a), xy(f.b));
    }
    for p in houses {
        s += &format!("<path d=\"{}\" fill=\"#b8865a\" stroke=\"#5a3a1a\" stroke-width=\"0.6\"/>\n", ring(p));
    }
    s + "</svg>\n"
}

/// `under`: every underground site of every layout (`u:` ways down, a few sewer sections each, keep dungeons
/// `k:`) with a hash of its serialized interior on stdout (`U <label> <id> <hash> <theme> <levels>`), and the
/// totals: sites, levels, rooms, doors, secret doors, loops (doors beyond a spanning tree of each level's
/// rooms), shortcut loops (each saving 4 or more hops: `shortcut_loops`) and the hops they save, and the sites failing `under::design::check`; locks, keys (and those on a level above their lock's), sites
/// whose keys are out of order (`under::design::key_order`), and built sites and their locks by size (`keep`, `sewer` for `k:`, `w:`).
pub(crate) fn under(ctx: &mut Ctx, r: &mut Report, label: &str) {
    use worldgen::core::hash::fnv64;
    use worldgen::under::{self, UnderKind};
    r.counts(&["sites", "levels", "rooms", "doors", "secret doors", "loops", "shortcut loops", "hops they save", "sites breaking a rule", "locks", "keys", "keys on an earlier level", "sites with keys out of order"]);
    for s in ["small", "medium", "large", "huge", "keep", "sewer"] {
        r.counts(&[&format!("sites {s}"), &format!("locks {s}")]);
    }
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut ids: Vec<String> = Vec::new();
        let mut sections: Vec<[i64; 2]> = Vec::new();
        for (k, e) in l.entrances.iter().enumerate() {
            if e.kind == UnderKind::Sewer {
                let o = under::sewer_section(e.at);
                let q = [(o[0] / under::SEWER_SECTION_FT) as i64, (o[1] / under::SEWER_SECTION_FT) as i64];
                if sections.len() < 4 && !sections.contains(&q) {
                    sections.push(q);
                    ids.push(format!("w:{li}:{}:{}", q[0], q[1]));
                }
            } else {
                ids.push(format!("u:{li}:{k}"));
            }
        }
        ids.extend(l.buildings.iter().filter(|b| interior::has_deep_dungeon(b)).map(|b| format!("k:{li}:{}", b.id)));
        for id in ids {
            let t = std::time::Instant::now();
            let Some(it) = interior::generate_id(world, t0, &id) else {
                println!("U {label} {id} none");
                continue;
            };
            // THEME=a,b: only sites of those themes (`keep` keep dungeons, `undercroft` sewer sections).
            let theme = it.theme.unwrap_or(if id.starts_with('k') { "keep" } else if id.starts_with('w') { "undercroft" } else { "-" });
            if let Ok(th) = std::env::var("THEME")
                && !th.split(',').any(|t| t == theme)
            {
                continue;
            }
            r.measure("slowest site (ms)", false, t.elapsed().as_secs_f64() * 1e3, id.clone());
            // (A sewer section's own level too: the undercroft below it may change alone.)
            let top = if id.starts_with('w') { format!(" top {:016x}", fnv64(serde_json::to_string(it.levels.last().unwrap()).unwrap().as_bytes())) } else { String::new() };
            println!("U {label} {id} {:016x} {} {}{top}", fnv64(serde_json::to_string(&it).unwrap().as_bytes()), it.theme.unwrap_or("-").replace(' ', "_"), it.levels.len());
            r.tally("sites", 1);
            r.tally("levels", it.levels.len());
            // Cave-ins by the age of the place above (levels with open gaps; a collapsed fortress's own left out).
            if theme != "collapsed_fortress" && it.levels.iter().all(|lv| !lv.natural) {
                let class = if id.starts_with('u') && li >= t0.settlements.len() { "ruin" } else { "living" };
                let caved = it.levels.iter().filter(|lv| lv.doors.iter().any(|d| d.kind == "arch")).count();
                r.tally(&format!("built sites {class}"), 1);
                r.tally(&format!("built levels {class}"), it.levels.len());
                r.tally(&format!("caved sites {class}"), (caved > 0) as usize);
                r.tally(&format!("caved levels {class}"), caved);
            }
            for lv in &it.levels {
                r.tally("rooms", lv.rooms.len());
                r.tally("doors", lv.doors.len());
                r.tally("secret doors", lv.doors.iter().filter(|d| d.kind == "secret").count());
                r.tally("portcullises", lv.doors.iter().filter(|d| d.kind == "portcullis").count());
                for f in lv.furniture.iter().filter(|f| matches!(f.kind, "water_shallow" | "water_deep" | "rope_bridge" | "stepping_stone")) {
                    r.tally(&format!("{} squares", f.kind), (f.w * f.h) as usize);
                }
                // (Union-find over the rooms: a door joining rooms already joined is a loop.)
                let mut up: Vec<usize> = (0..lv.rooms.len()).collect();
                fn root(up: &mut [usize], mut i: usize) -> usize {
                    while up[i] != i {
                        up[i] = up[up[i]];
                        i = up[i];
                    }
                    i
                }
                for d in &lv.doors {
                    let [a, b] = d.rooms;
                    if a < 0 || b < 0 || a as usize >= up.len() || b as usize >= up.len() {
                        continue;
                    }
                    let (ra, rb) = (root(&mut up, a as usize), root(&mut up, b as usize));
                    if ra == rb {
                        r.tally("loops", 1);
                    } else {
                        up[ra] = rb;
                    }
                }
                if !lv.natural {
                    let (n, saved) = shortcut_loops(lv, it.nx, it.ny);
                    r.tally("shortcut loops", n);
                    r.tally("hops they save", saved);
                }
            }
            // Locks and keys (levels run bottom to top: an earlier level is a later index).
            let locks: Vec<(usize, u8)> = it.levels.iter().enumerate().flat_map(|(li, lv)| lv.doors.iter().filter(|d| d.lock != 0).map(move |d| (li, d.lock))).collect();
            let keys: Vec<(usize, u8)> = it.levels.iter().enumerate().flat_map(|(li, lv)| lv.furniture.iter().filter(|f| f.kind == "key").map(move |f| (li, f.variant.unwrap_or(0)))).collect();
            r.tally("locks", locks.len());
            r.tally("keys", keys.len());
            r.tally("keys on an earlier level", keys.iter().filter(|k| locks.iter().any(|l| l.1 == k.1 && l.0 < k.0)).count());
            let size = under::site_size(&it).map(|s| s.name()).unwrap_or(if id.starts_with('k') { "keep" } else { "sewer" });
            if it.levels.iter().all(|lv| !lv.natural) {
                // (Built sites only: caves, mines and lava tubes have no doors.)
                r.tally(&format!("sites {size}"), 1);
                r.tally(&format!("locks {size}"), locks.len());
            }
            r.measure("most locks in a site", false, locks.len() as f64, id.clone());
            let order = under::design::key_order(&it);
            if !order.is_empty() {
                r.add("sites with keys out of order", order.len() as f64, format!("{id} ({}): {}", it.theme.unwrap_or("-"), order.into_iter().map(|p| p.text).collect::<Vec<_>>().join("; ")));
            }
            // (Its rules are a designed `u:` site's. A keep dungeon is entered by the stairs up to the keep, an undercroft
            // by its way up to the sewer section above: each checked as if that were its way in from the surface, the
            // sewer section's own level left out.)
            let bad: Vec<String> = if id.starts_with('u') {
                under::design::check(&it, None).into_iter().filter(|p| p.blocking).map(|p| p.text).collect()
            } else {
                let mut c = it.clone();
                if id.starts_with('w') {
                    c.levels.pop();
                    c.entry_level = c.levels.len().saturating_sub(1);
                }
                if let Some(top) = c.levels.last_mut() {
                    for f in top.furniture.iter_mut().filter(|f| f.kind == "up") {
                        f.kind = "exit";
                    }
                }
                if c.levels.is_empty() { Vec::new() } else { under::design::check(&c, None).into_iter().filter(|p| p.blocking).map(|p| p.text).collect() }
            };
            if !bad.is_empty() {
                r.add("sites breaking a rule", bad.len() as f64, format!("{id} ({}): {}", it.theme.unwrap_or("-"), bad.join("; ")));
            }
            if std::env::var("TERRAIN").is_ok() {
                under_terrain(it, &id, r);
            }
        }
    }
}

/// A built level's shortcut loops and the hops they save: doors in turn, each counted (and then taken away, so a
/// loop counts once) if the way round without it is 4 or more hops longer (on foot: a square 1, a door 8, a hop 8).
pub(crate) fn shortcut_loops(lv: &interior::Level, nx: usize, ny: usize) -> (usize, usize) {
    let pair = |d: &interior::Door| {
        let (x, y) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize);
        if d.a[1] == d.b[1] { (y.saturating_sub(1) * nx + x, y * nx + x) } else { (y * nx + x.saturating_sub(1), y * nx + x) }
    };
    let mut open: std::collections::HashSet<(usize, usize)> = lv.doors.iter().map(pair).collect();
    let walk = |open: &std::collections::HashSet<(usize, usize)>, from: usize, to: usize| {
        let mut cost = vec![u32::MAX; nx * ny];
        let mut heap = std::collections::BinaryHeap::new();
        cost[from] = 0;
        heap.push(std::cmp::Reverse((0u32, from)));
        while let Some(std::cmp::Reverse((c, k))) = heap.pop() {
            if k == to {
                return Some(c);
            }
            if c > cost[k] {
                continue;
            }
            let (i, j) = (k % nx, k / nx);
            for m in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten() {
                let (a, b) = (lv.cells[k], lv.cells[m]);
                if b < 0 || (a != b && !open.contains(&(k.min(m), k.max(m)))) {
                    continue;
                }
                let nc = c + if a == b { 1 } else { 8 };
                if nc < cost[m] {
                    cost[m] = nc;
                    heap.push(std::cmp::Reverse((nc, m)));
                }
            }
        }
        None
    };
    let (mut n, mut saved) = (0, 0);
    for d in &lv.doors {
        let e = pair(d);
        if e.1 >= nx * ny || lv.cells[e.0] < 0 || lv.cells[e.1] < 0 {
            continue;
        }
        open.remove(&e);
        match walk(&open, e.0, e.1) {
            Some(c) if c >= 8 + 32 => {
                n += 1;
                saved += (c - 8) as usize / 8;
            }
            _ => {
                open.insert(e);
            }
        }
    }
    (n, saved)
}

/// `under` with TERRAIN set: the terrain tools on a copy of the site, every level (a chasm across the biggest room, or
/// the whole level if natural; then water, wet and flooded by turns; rope bridges and stepping stones by turns): what
/// they laid, the squares left unreached (0 expected) and the rules the site then breaks.
pub(crate) fn under_terrain(mut it: interior::Interior, id: &str, r: &mut Report) {
    use worldgen::under::{self, Ground, Span, Wetness};
    r.counts(&["terrain: levels", "terrain: unreached squares", "terrain: sites breaking a rule", "terrain: sites with keys out of order"]);
    let (nx, ny) = (it.nx, it.ny);
    let seed = worldgen::core::hash::fnv64(id.as_bytes());
    let t = std::time::Instant::now();
    for (li, lv) in it.levels.iter_mut().enumerate() {
        let s = worldgen::core::rng::hash2(seed, li as i64, 0);
        let biggest = (!lv.natural).then(|| (0..lv.rooms.len()).max_by_key(|&q| (lv.rooms[q].squares, std::cmp::Reverse(q))).map(|q| q as i16)).flatten();
        let span = if li % 2 == 0 { Span::Bridge } else { Span::Stones };
        let items = under::chasm(&Ground::of_level(lv, nx, ny), biggest, span, s ^ 1);
        lv.furniture.extend(items);
        let items = under::water(&Ground::of_level(lv, nx, ny), if (li + seed as usize).is_multiple_of(2) { Wetness::Wet } else { Wetness::Flooded }, s);
        lv.furniture.extend(items);
        r.tally("terrain: levels", 1);
        r.tally("terrain: unreached squares", Ground::of_level(lv, nx, ny).unreached());
        for f in &lv.furniture {
            let sq = (f.w * f.h) as usize;
            match f.kind {
                "water_shallow" | "water_deep" | "chasm" | "rope_bridge" | "stepping_stone" => r.tally(&format!("terrain: {} squares", f.kind), sq),
                _ => {}
            }
            if f.kind == "rope_bridge" {
                r.tally("terrain: rope bridges", 1);
            }
        }
    }
    r.measure("terrain: slowest site (ms)", false, t.elapsed().as_secs_f64() * 1e3, id.to_string());
    let bad: Vec<String> = if id.starts_with('u') { under::design::check(&it, None).into_iter().filter(|p| p.blocking).map(|p| p.text).collect() } else { Vec::new() };
    if !bad.is_empty() {
        r.add("terrain: sites breaking a rule", bad.len() as f64, format!("{id}: {}", bad.join("; ")));
    }
    let order = under::design::key_order(&it);
    if !order.is_empty() {
        r.add("terrain: sites with keys out of order", order.len() as f64, format!("{id}: {}", order.into_iter().map(|p| p.text).collect::<Vec<_>>().join("; ")));
    }
}

// ---- Walls by culture group (item 104). ----

/// `walls`: per culture group (`l.culture % 5`), walled places and their towers by shape (wall towers,
/// castle towers, corner towers); square towers where the style has none; towers whose battlemap squares
/// don't follow their shape (the ring of wall blocking, the floor open, the door reached from outside);
/// and up to 3 walled settlements per group listed with a camera on the whole wall and one on a tower
/// by a gate (`fx, fy` as `scripts/shot.mjs` views take them). Towers of the first listed place per
/// group are checked on the battlemap (`WALLS_ALL=1`: every listed place's).
/// The bow of each long curtain (slice 2c): per side of the town's ring 150 ft and longer, the town
/// wall along it (5-ft steps within 80 ft of it, beside the side), its greatest distance off the chord
/// between its points nearest the side's two corners. Also the buildings centred in the town's patches
/// that the wall leaves outside (a line to the town's middle crosses it an odd number of times), and
/// how many it has inside.
pub(crate) fn curtain_bows(ctx: &Ctx, l: &Layout, li: usize) -> (Vec<(f64, String)>, Vec<String>, usize) {
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut out = (Vec::new(), Vec::new(), 0);
    let Ok(plan) = town::wards::plan_json(world, t0, li) else { return out };
    if plan["walls"]["built"] != true {
        return out;
    }
    let pt = |v: &serde_json::Value| -> P { [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)] };
    let corners: HashMap<u64, P> = plan["corners"].as_array().into_iter().flatten().filter_map(|c| Some((c["corner"].as_u64()?, pt(&c["at"])))).collect();
    let mut sides: Vec<(u64, u64)> = Vec::new();
    let mut patches: Vec<Vec<P>> = Vec::new();
    for p in plan["patches"].as_array().into_iter().flatten().filter(|p| p["in_town"] == true) {
        let cs: Vec<u64> = p["corners"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()).collect();
        patches.push(cs.iter().filter_map(|c| corners.get(c).copied()).collect());
        sides.extend((0..cs.len()).map(|k| (cs[k], cs[(k + 1) % cs.len()])));
    }
    let shared: std::collections::HashSet<(u64, u64)> = sides.iter().copied().collect();
    let town: Vec<&Vec<P>> = l.walls.iter().filter(|w| w.len() >= 2 && !l.castles.iter().any(|(c, rad)| dist(*c, w[w.len() / 2]) <= rad + 10.0)).collect();
    let pts: Vec<P> = town.iter().flat_map(|w| samples(w, 5.0).into_iter().map(|s| s.0).chain(std::iter::once(w[w.len() - 1]))).collect();
    // (Each wall point counts for the ring side nearest it, slice 2e: bows reach 80 ft out.)
    let ring: Vec<(P, P)> = sides.iter().filter(|(a, b)| !shared.contains(&(*b, *a))).filter_map(|(a, b)| Some((*corners.get(a)?, *corners.get(b)?))).collect();
    let side_of = |q: P| (0..ring.len()).min_by(|&x, &y| seg_dist(q, ring[x].0, ring[x].1).total_cmp(&seg_dist(q, ring[y].0, ring[y].1)));
    let owner: Vec<Option<usize>> = pts.iter().map(|q| side_of(*q)).collect();
    for (k, &(pa, pb)) in ring.iter().enumerate() {
        let d = dist(pa, pb);
        if d < 150.0 {
            continue;
        }
        let u = mul(sub(pb, pa), 1.0 / d);
        let near: Vec<P> = pts.iter().zip(&owner).filter(|(q, o)| **o == Some(k) && seg_dist(**q, pa, pb) < 80.0 && (0.0..=d).contains(&dot(sub(**q, pa), u))).map(|x| *x.0).collect();
        let nearest = |c: P| near.iter().copied().min_by(|x, y| dist(*x, c).total_cmp(&dist(*y, c)));
        let (Some(q0), Some(q1)) = (nearest(pa), nearest(pb)) else { continue };
        if dist(q0, q1) < 100.0 {
            continue;
        }
        let bow = near.iter().map(|q| seg_dist(*q, q0, q1)).fold(0.0, f64::max);
        let m = lerp(pa, pb, 0.5);
        out.0.push((bow, format!("L{li} side at {} fx {:.6} fy {:.6}", at(m), m[0] / world.geom.map_w_ft, m[1] / world.geom.map_h_ft)));
    }
    let segs: Vec<(P, P)> = town.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let crosses = |p: P, q: P, a: P, b: P| {
        let (o1, o2) = (cross(sub(q, p), sub(a, p)), cross(sub(q, p), sub(b, p)));
        let (o3, o4) = (cross(sub(b, a), sub(p, a)), cross(sub(b, a), sub(q, a)));
        o1 * o2 < 0.0 && o3 * o4 < 0.0
    };
    for bd in l.buildings.iter().filter(|bd| bd.poly.len() >= 3 && patches.iter().any(|f| contains(f, centroid(&bd.poly)))) {
        let c = centroid(&bd.poly);
        if segs.iter().filter(|&&(a, b)| crosses(c, l.center, a, b)).count() % 2 == 1 {
            out.1.push(format!("L{li} b:{li}:{} at {}", bd.id, at(c)));
        } else {
            out.2 += 1;
        }
    }
    out
}

/// Town wall towers (a castle's apart) and gate towers under 22 ft apart, centre to centre: two
/// wall runs' corners meeting, each with its own tower (a gate's own pair apart).
pub(crate) fn close_towers(l: &Layout, li: usize, r: &mut Report) {
    let castle = |t: P| l.tower_forms.iter().any(|f| (f.0[0] - t[0]).abs() + (f.0[1] - t[1]).abs() < 1e-6 && f.1 & 16 != 0);
    let ts: Vec<(P, bool)> = l.towers.iter().filter(|t| !castle(**t)).map(|t| (*t, false)).chain(l.gate_towers.iter().map(|t| (*t, true))).collect();
    for i in 0..ts.len() {
        for j in i + 1..ts.len() {
            let d = dist(ts[i].0, ts[j].0);
            // (A gate's pair: across its passage, both gate towers.)
            if d < 22.0 && !(ts[i].1 && ts[j].1 && d > 8.0) {
                let kind = match (ts[i].1, ts[j].1) {
                    (false, false) => "wall towers",
                    (true, true) => "gate towers",
                    _ => "a gate tower and a wall tower",
                };
                r.add("town towers under 22 ft apart", 1.0, format!("L{li} {kind} {d:.1} ft apart at {} and {}", at(ts[i].0), at(ts[j].0)));
            }
        }
    }
}
