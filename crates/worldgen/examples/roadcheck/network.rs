use crate::prelude::*;

pub(crate) fn water_line(on_water: &[[usize; 2]; 4], reasons: &[usize]) -> String {
    let tiers = ["village", "town", "city", "metropolis"];
    let shares: Vec<String> = (0..4).map(|t| format!("{} {}/{} ({:.0}%)", tiers[t], on_water[t][0], on_water[t][1], 100.0 * on_water[t][0] as f64 / on_water[t][1].max(1) as f64)).collect();
    let rs: Vec<String> = reasons.iter().enumerate().filter(|&(_, &c)| c > 0).map(|(i, c)| format!("{} {c}", worldgen::t0::settle::Reason::from_u8(i as u8).name())).collect();
    format!("on water: {}; reasons: {}", shares.join(", "), rs.join(", "))
}

pub(crate) fn network(t0: &T0, standing: &dyn Fn(P) -> bool, gates: &[(worldgen::t0::settle::Tier, usize)]) -> Value {
    use worldgen::lod::roads::river_crossings;
    use worldgen::t0::roads::{meeting_points, CrossingKind};
    use worldgen::t0::settle::Tier;
    let t = std::time::Instant::now();
    let cell = t0.cell_ft;
    let sts = &t0.settlements;
    let class_names = ["king's road", "road", "track"];
    // --- Key points: road ends, and the places roads meet (on clones, as the curves are made).
    let mut pts: Vec<Vec<P>> = t0.roads.roads.iter().map(|r| r.pts.clone()).collect();
    let mut wander: Vec<Vec<f32>> = t0.roads.roads.iter().map(|r| r.wander.clone()).collect();
    let meets = meeting_points(&mut pts, &mut wander);
    let class: Vec<usize> = t0.roads.roads.iter().map(|r| r.class as usize).collect();
    let mut keys: Vec<Vec<usize>> = pts.iter().map(|q| if q.len() >= 2 { vec![0, q.len() - 1] } else { Vec::new() }).collect();
    for m in &meets {
        for &(r, i) in m {
            keys[r].push(i);
        }
    }
    let mut spots = Spots { tol: 60.0, pos: Vec::new(), grid: HashMap::new() };
    // Edges between consecutive key points of a road: (node, node, chord length, road, from idx, to idx).
    let mut edges: Vec<(usize, usize, f64, usize, usize, usize)> = Vec::new();
    // Per node: the (road, point index) key points there.
    let mut at_node: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();
    let mut road_nodes: Vec<std::collections::HashSet<usize>> = vec![Default::default(); pts.len()];
    for r in 0..pts.len() {
        keys[r].sort_unstable();
        keys[r].dedup();
        let ids: Vec<usize> = keys[r].iter().map(|&i| spots.id(pts[r][i])).collect();
        while at_node.len() < spots.pos.len() {
            at_node.push(Vec::new());
        }
        for (&i, &n) in keys[r].iter().zip(&ids) {
            at_node[n].push((r, i));
            road_nodes[r].insert(n);
        }
        if let (Some(&f), Some(&l)) = (ids.first(), ids.last()) {
            ends.push(f);
            ends.push(l);
        }
        for k in 1..keys[r].len() {
            if ids[k - 1] != ids[k] {
                let len: f64 = pts[r][keys[r][k - 1]..=keys[r][k]].windows(2).map(|w| dist(w[0], w[1])).sum();
                edges.push((ids[k - 1], ids[k], len, r, keys[r][k - 1], keys[r][k]));
            }
        }
    }
    ends.sort_unstable();
    ends.dedup();
    let nroad = spots.pos.len();
    let reach = |s: &worldgen::t0::settle::Settlement| worldgen::town::road_trim_radius(s.tier, s.population).max(300.0) * 1.15 + 100.0;
    // --- Trips.
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nroad + sts.len()];
    for &(u, v, l, ..) in &edges {
        adj[u].push((v, l));
        adj[v].push((u, l));
    }
    for (si, s) in sts.iter().enumerate() {
        for &e in &ends {
            let d = dist([s.x, s.y], spots.pos[e]);
            if d <= reach(s) {
                adj[nroad + si].push((e, d));
                adj[e].push((nroad + si, d));
            }
        }
    }
    // (A sea ferry joins the road ends at its two landings: a trip may cross it.)
    let landing_end = |p: P| ends.iter().copied().filter(|&e| dist(spots.pos[e], p) <= 600.0).min_by(|&x, &y| dist(spots.pos[x], p).total_cmp(&dist(spots.pos[y], p)));
    for f in &t0.ferries {
        if let (Some(u), Some(v)) = (landing_end(f.a), landing_end(f.b)) {
            adj[u].push((v, dist(f.a, f.b)));
            adj[v].push((u, dist(f.a, f.b)));
        }
    }
    let dry = |a: P, b: P| {
        let n = (dist(a, b) / 1000.0).ceil().max(1.0) as usize;
        (0..=n).all(|j| !standing(lerp(a, b, j as f64 / n as f64)))
    };
    let (mut all, mut dryf, mut unreach, mut unreach_dry) = (Vec::new(), Vec::new(), 0usize, 0usize);
    // (The same, from and to places outside the isolated realms, which are cut off by design.)
    let (mut open_trips, mut unreach_open) = (Vec::new(), 0usize);
    for (si, s) in sts.iter().enumerate() {
        let mut near: Vec<(f64, usize)> = sts.iter().enumerate().filter(|(j, _)| *j != si).map(|(j, o)| (dist([s.x, s.y], [o.x, o.y]), j)).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let d = dijkstra(&adj, nroad + si);
        for &(straight, j) in near.iter().take(4) {
            let wet = !dry([s.x, s.y], [sts[j].x, sts[j].y]);
            let nd = d[nroad + j];
            let open = !s.isolated && !sts[j].isolated;
            if open {
                if nd.is_finite() { open_trips.push(nd / straight.max(1.0)) } else { unreach_open += 1 }
            }
            if nd.is_finite() {
                all.push(nd / straight.max(1.0));
                if !wet {
                    dryf.push(nd / straight.max(1.0));
                }
            } else {
                unreach += 1;
                unreach_dry += !wet as usize;
            }
        }
    }
    all.sort_by(|a, b| a.total_cmp(b));
    dryf.sort_by(|a, b| a.total_cmp(b));
    let share = |v: &[f64], x: f64| 100.0 * v.iter().filter(|f| **f > x).count() as f64 / v.len().max(1) as f64;
    let trips = |label: &str, v: &[f64], un: usize| {
        say!("  trips {label}: {} reachable, {un} unreachable; route factor p50 {:.2} p90 {:.2} p99 {:.2}; > 1.5: {:.1}%, > 2: {:.1}%", v.len(), pct(v, 0.5), pct(v, 0.9), pct(v, 0.99), share(v, 1.5), share(v, 2.0));
        json!({ "n": v.len(), "unreachable": un, "p50": pct(v, 0.5), "p90": pct(v, 0.9), "p99": pct(v, 0.99), "over_1_5": share(v, 1.5), "over_2": share(v, 2.0) })
    };
    let trips_all = trips("(each settlement to its 4 nearest)", &all, unreach);
    open_trips.sort_by(|a, b| a.total_cmp(b));
    trips("(the same, outside realms)", &open_trips, unreach_open);
    let trips_dry = trips("(pairs with a dry straight line)", &dryf, unreach_dry);
    // --- Length against the settlements' minimum spanning tree.
    let mut by_class = [0.0f64; 3];
    for (r, q) in pts.iter().enumerate() {
        by_class[class[r]] += q.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>();
    }
    let total: f64 = by_class.iter().sum();
    let mst = {
        let n = sts.len();
        let (mut best, mut done, mut sum) = (vec![f64::INFINITY; n], vec![false; n], 0.0);
        if n > 0 {
            best[0] = 0.0;
        }
        for _ in 0..n {
            let Some(u) = (0..n).filter(|&i| !done[i] && best[i].is_finite()).min_by(|&a, &b| best[a].total_cmp(&best[b])) else { break };
            done[u] = true;
            sum += best[u];
            for v in 0..n {
                let d = dist([sts[u].x, sts[u].y], [sts[v].x, sts[v].y]);
                if !done[v] && d <= 400.0 * MI && d < best[v] {
                    best[v] = d;
                }
            }
        }
        sum
    };
    say!(
        "  road length {:.0} mi ({}), settlements' MST {:.0} mi: {:.2}x",
        total / MI,
        (0..3).map(|c| format!("{} {:.0}", class_names[c], by_class[c] / MI)).collect::<Vec<_>>().join(", "),
        mst / MI,
        total / mst.max(1.0)
    );
    // --- River crossings, tile by tile (40,000 ft).
    const TILE: f64 = 40_000.0;
    let bb = pts.iter().flatten().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let mut crossings: Vec<([f64; 2], f64, CrossingKind)> = Vec::new();
    let mut seen: std::collections::HashSet<(i64, i64)> = Default::default();
    if bb[0] <= bb[2] {
        for ty in (bb[1] / TILE).floor() as i64..=(bb[3] / TILE).floor() as i64 {
            for tx in (bb[0] / TILE).floor() as i64..=(bb[2] / TILE).floor() as i64 {
                let rect = [tx as f64 * TILE, ty as f64 * TILE, (tx + 1) as f64 * TILE, (ty + 1) as f64 * TILE];
                if t0.roads.segments_near(rect, 0.0).is_empty() {
                    continue;
                }
                let rp = worldgen::lod::roads::pieces(&t0.roads, rect, 0.0, 40.0);
                let vp = worldgen::lod::rivers::pieces(&t0.rivers, rect, &|_| 0.0, 40.0, cell);
                for c in river_crossings(&rp, &vp) {
                    if seen.insert(((c.at[0] / 30.0).round() as i64, (c.at[1] / 30.0).round() as i64)) {
                        crossings.push((c.at, c.sin, c.kind));
                    }
                }
            }
        }
    }
    let kinds = [CrossingKind::Bridge, CrossingKind::Ford, CrossingKind::Ferry];
    let kind_n: Vec<usize> = kinds.iter().map(|k| crossings.iter().filter(|c| c.2 == *k).count()).collect();
    let nc = crossings.len().max(1) as f64;
    let under = |s: f64| 100.0 * crossings.iter().filter(|c| c.1 < s).count() as f64 / nc;
    let within = |r: f64| 100.0 * crossings.iter().enumerate().filter(|(i, c)| crossings.iter().enumerate().any(|(j, o)| j != *i && dist(c.0, o.0) <= r)).count() as f64 / nc;
    say!(
        "  river crossings: {} (bridges {}, fords {}, ferries {}); under 45 deg {:.1}%, under 30 deg {:.1}%; another within 1 mi {:.1}%, within 3 mi {:.1}%",
        crossings.len(),
        kind_n[0],
        kind_n[1],
        kind_n[2],
        under(0.707),
        under(0.5),
        within(MI),
        within(3.0 * MI)
    );
    // (`SHALLOW=n`: the first n crossings under 30°.)
    let shallow = std::env::var("SHALLOW").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    for c in crossings.iter().filter(|c| c.1 < 0.5).take(shallow) {
        say!("      crossing at {:.0}° at ({:.0}, {:.0})", c.1.asin().to_degrees(), c.0[0], c.0[1]);
    }
    // (`RIVERS=x,y,r`: river centre lines within r ft of (x, y), with their width.)
    if let Some(v) = std::env::var("RIVERS").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect::<Vec<f64>>()) {
        for (ri, r) in t0.rivers.rivers.iter().enumerate() {
            let near: Vec<String> = r.pts.iter().zip(&r.q).filter(|(p, _)| dist(**p, [v[0], v[1]]) < v[2]).map(|(p, q)| format!("({:.0}, {:.0}) w {:.0}", p[0], p[1], worldgen::lod::rivers::width_ft(*q as f64))).collect();
            if !near.is_empty() {
                say!("      river {ri}: {}", near.join(" "));
            }
        }
    }
    // (`CROSS=x,y,r`: the crossings within r ft of (x, y).)
    if let Some(v) = std::env::var("CROSS").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect::<Vec<f64>>()) {
        for c in crossings.iter().filter(|c| dist(c.0, [v[0], v[1]]) < v[2]) {
            say!("      crossing {:?} at {:.0}° at ({:.0}, {:.0})", c.2, c.1.asin().to_degrees(), c.0[0], c.0[1]);
        }
    }
    // --- Junctions outside settlements.
    let mut deg = vec![0usize; nroad];
    for &(u, v, ..) in &edges {
        deg[u] += 1;
        deg[v] += 1;
    }
    let attached = |p: P| sts.iter().any(|s| dist([s.x, s.y], p) <= reach(s));
    let junction: Vec<bool> = (0..nroad).map(|n| deg[n] >= 3 && !attached(spots.pos[n])).collect();
    let (mut arms_n, mut shape_n, mut shape_arms) = ([0usize; 3], [0usize; 3], [[0usize; 3]; 3]);
    let mut shape_of = vec![0usize; nroad];
    for n in (0..nroad).filter(|&n| junction[n]) {
        arms_n[(deg[n] - 3).min(2)] += 1;
        // Arm directions ~0.3 cell out, from each edge at this node.
        let mut v: Vec<P> = Vec::new();
        for &(a, b, _, r, i0, i1) in &edges {
            if a == n {
                v.push(sub(along_from(&pts[r], i0, i1, 0.3 * cell), pts[r][i0]));
            }
            if b == n {
                v.push(sub(along_from(&pts[r], i1, i0, 0.3 * cell), pts[r][i1]));
            }
        }
        let mut straight = false;
        let mut smallest = 360.0f64;
        for i in 0..v.len() {
            for j in i + 1..v.len() {
                let a = angle(v[i], v[j]);
                smallest = smallest.min(a);
                if a >= 155.0 && (0..v.len()).filter(|m| *m != i && *m != j).all(|m| (70.0..=110.0).contains(&angle(v[m], v[i]))) {
                    straight = true;
                }
            }
        }
        let shape = if straight { 0 } else if smallest < 60.0 { 1 } else { 2 };
        // `JAT=x,y,r`: each junction within r ft of (x, y), its arms' headings (degrees).
        if let Some(q) = std::env::var("JAT").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect::<Vec<f64>>()) {
            if q.len() == 3 && dist(spots.pos[n], [q[0], q[1]]) < q[2] {
                let heads: Vec<String> = v.iter().map(|d| format!("{:.0}", d[1].atan2(d[0]).to_degrees())).collect();
                say!("      JAT junction ({:.0}, {:.0}) {} arms, shape {shape}, headings {}", spots.pos[n][0], spots.pos[n][1], deg[n], heads.join(" "));
            }
        }
        shape_of[n] = shape;
        shape_n[shape] += 1;
        shape_arms[shape][(deg[n] - 3).min(2)] += 1;
    }
    let (mut stub_half, mut stub_two, mut stub_doubled) = (0usize, 0usize, 0usize);
    for &(u, v, l, r, i0, i1) in &edges {
        if junction[u] && junction[v] {
            stub_half += (l < 0.5 * MI) as usize;
            stub_two += (l < 2.0 * MI) as usize;
            // (Of the short ones, those along a stretch two roads share: they cross at every point.)
            if l < 0.5 * MI {
                let m = along_from(&pts[r], i0, i1, 0.5 * l);
                stub_doubled += pts.iter().enumerate().any(|(o, q)| o != r && q.windows(2).any(|w| seg_dist(m, w[0], w[1]) < 60.0)) as usize;
            }
        }
    }
    // Two roads passing through one place (neither ends there).
    let x_nodes: Vec<usize> = (0..nroad)
        .filter(|&n| !attached(spots.pos[n]) && at_node[n].iter().filter(|&&(r, i)| i != 0 && i != pts[r].len() - 1).map(|e| e.0).collect::<std::collections::HashSet<_>>().len() >= 2)
        .collect();
    let x_cross = x_nodes.len();
    for &n in x_nodes.iter().take(4) {
        say!("      two roads through ({:.0}, {:.0})", spots.pos[n][0], spots.pos[n][1]);
    }
    let near: Vec<usize> = (0..nroad).filter(|&n| junction[n] && crossings.iter().any(|c| dist(c.0, spots.pos[n]) <= 0.5 * MI)).collect();
    let near_cross = near.len();
    let njunc = junction.iter().filter(|j| **j).count();
    say!("  junctions outside settlements: {njunc} (3 arms {}, 4 arms {}, 5+ {})", arms_n[0], arms_n[1], arms_n[2]);
    say!(
        "    shape: T/cross {} (3 arms {}, 4 arms {}), Y {} (3 arms {}), other {} (3 arms {}); between junctions: < 0.5 mi {stub_half} ({stub_doubled} on shared stretches), < 2 mi {stub_two}; two roads through one place {x_cross}; within 0.5 mi of a river crossing {near_cross}",
        shape_n[0],
        shape_arms[0][0],
        shape_arms[0][1],
        shape_n[1],
        shape_arms[1][0],
        shape_n[2],
        shape_arms[2][0]
    );
    // (The ones near a crossing, with the junction's shape: 0 T/cross, 1 Y, 2 other.)
    let top = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    for &(u, v, l, r, ..) in edges.iter().filter(|e| junction[e.0] && junction[e.1] && e.2 < 0.5 * MI).take(top) {
        say!("      stub of {l:.0} ft on road {r} from ({:.0}, {:.0}) ({} arms) to ({:.0}, {:.0}) ({} arms)", spots.pos[u][0], spots.pos[u][1], deg[u], spots.pos[v][0], spots.pos[v][1], deg[v]);
    }
    for &n in near.iter().take(top) {
        let c = crossings.iter().map(|c| dist(c.0, spots.pos[n])).fold(f64::MAX, f64::min);
        say!("      junction at ({:.0}, {:.0}), {} arms, shape {}: a crossing {:.0} ft away", spots.pos[n][0], spots.pos[n][1], deg[n], shape_of[n], c);
    }
    // (`JLIST=1`: every Y and every junction near a crossing, with its roads: class, whether it
    // ends there, and the settlements at its two ends.)
    if std::env::var("JLIST").is_ok_and(|v| v == "1") {
        let end_at = |p: P| sts.iter().find(|s| dist([s.x, s.y], p) <= reach(s)).map(|s| format!("{:?}/{}", s.tier, s.reason.name())).unwrap_or_else(|| "-".into());
        for n in (0..nroad).filter(|&n| junction[n] && (shape_of[n] == 1 || near.contains(&n))) {
            let c = crossings.iter().map(|c| dist(c.0, spots.pos[n])).fold(f64::MAX, f64::min);
            let roads: Vec<String> = at_node[n]
                .iter()
                .map(|&(r, i)| {
                    let q = &pts[r];
                    let end = if i == 0 || i == q.len() - 1 { "ends" } else { "on" };
                    format!("{r} {} {end} [{} .. {}]", class_names[class[r]], end_at(q[0]), end_at(q[q.len() - 1]))
                })
                .collect();
            say!("      JLIST ({:.0}, {:.0}) shape {} crossing {:.0} ft: {}", spots.pos[n][0], spots.pos[n][1], shape_of[n], c, roads.join("; "));
        }
        // (Spurs: a road from a settlement to a junction outside settlements, by length.)
        let mut spur = [0usize; 4];
        for &(u, v, l, ..) in &edges {
            let (s_end, j_end) = (deg[u] == 1 || attached(spots.pos[u]), junction[v]);
            let (s_end2, j_end2) = (deg[v] == 1 || attached(spots.pos[v]), junction[u]);
            if (s_end && j_end) || (s_end2 && j_end2) {
                spur[if l < 0.5 * MI { 0 } else if l < MI { 1 } else if l < 2.0 * MI { 2 } else { 3 }] += 1;
            }
        }
        say!("      JLIST spurs < 0.5 mi {}, 0.5-1 {}, 1-2 {}, 2+ {}", spur[0], spur[1], spur[2], spur[3]);
    }
    // Dead ends: a road end at no settlement and no other road (those within 0.3 mi of a river
    // crossing counted apart: a road may end at a ferry or ford).
    let at_ferry = |p: P| t0.ferries.iter().any(|f| dist(f.a, p) <= 600.0 || dist(f.b, p) <= 600.0);
    let dead: Vec<usize> = (0..nroad).filter(|&n| deg[n] == 1 && !attached(spots.pos[n]) && !at_ferry(spots.pos[n])).collect();
    let dead_wet = dead.iter().filter(|&&n| crossings.iter().any(|c| dist(c.0, spots.pos[n]) <= 0.3 * MI)).count();
    say!("  dead ends (no settlement, no other road): {} ({} at a river crossing)", dead.len(), dead_wet);
    for &n in dead.iter().take(top) {
        let &(r, _) = &at_node[n][0];
        let s = sts.iter().map(|s| (dist([s.x, s.y], spots.pos[n]), s)).min_by(|a, b| a.0.total_cmp(&b.0));
        let near_s = s.map(|(d, s)| format!("{:?} at ({:.0}, {:.0}) {:.0} ft away", s.tier, s.x, s.y, d)).unwrap_or_default();
        let len: f64 = pts[r].windows(2).map(|w| dist(w[0], w[1])).sum();
        say!("      dead end at ({:.0}, {:.0}): road {r} ({}, {:.1} mi); nearest {near_s}", spots.pos[n][0], spots.pos[n][1], class_names[class[r]], len / MI);
    }
    // (`BESIDE=r`: along road r, every 20th point, the nearest other road and how far.)
    if let Some(r) = std::env::var("BESIDE").ok().and_then(|v| v.parse::<usize>().ok()) {
        for (k, p) in pts[r].iter().enumerate().step_by(20) {
            let near = (0..pts.len()).filter(|&o| o != r).map(|o| (pts[o].windows(2).map(|w| seg_dist(*p, w[0], w[1])).fold(f64::MAX, f64::min), o)).min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((d, o)) = near {
                say!("      {r} pt {k} ({:.0}, {:.0}) {}: nearest road {o} ({}) {d:.0} ft", p[0], p[1], class_names[class[r]], class_names[class[o]]);
            }
        }
    }
    // (`KENDS=1`: each king's road end that no other king's road touches, with what is there.)
    if std::env::var("KENDS").is_ok() {
        for r in (0..pts.len()).filter(|&r| class[r] == 0 && pts[r].len() >= 2) {
            for p in [pts[r][0], *pts[r].last().unwrap()] {
                let kings = (0..pts.len()).filter(|&o| o != r && class[o] == 0 && pts[o].windows(2).any(|w| seg_dist(p, w[0], w[1]) < 60.0)).count();
                if kings > 0 {
                    continue;
                }
                let others: Vec<String> = (0..pts.len()).filter(|&o| o != r && pts[o].windows(2).any(|w| seg_dist(p, w[0], w[1]) < 60.0)).map(|o| format!("{o} {}", class_names[class[o]])).collect();
                let s = sts.iter().filter(|s| dist([s.x, s.y], p) <= reach(s)).map(|s| format!("{:?} pop {} at ({:.0}, {:.0})", s.tier, s.population, s.x, s.y)).collect::<Vec<_>>().join("; ");
                let len: f64 = pts[r].windows(2).map(|w| dist(w[0], w[1])).sum();
                say!("      king's end ({:.0}, {:.0}) road {r} {:.1} mi: settlement [{s}] other roads [{}]", p[0], p[1], len / MI, others.join(", "));
            }
        }
    }
    // --- Out-and-backs: the road's heading 200 ft before a point and 200 ft after (100 ft at least,
    // near its ends) differing by over 100°; those within a mile of a river crossing listed.
    let mut spikes: Vec<(P, f64, usize)> = Vec::new();
    for (r, q) in pts.iter().enumerate() {
        let mut line: Vec<P> = Vec::new();
        for w in q.windows(2) {
            let n = (dist(w[0], w[1]) / 50.0).ceil().max(1.0) as usize;
            line.extend((0..n).map(|j| [w[0][0] + (w[1][0] - w[0][0]) * j as f64 / n as f64, w[0][1] + (w[1][1] - w[0][1]) * j as f64 / n as f64]));
        }
        line.extend(q.last().copied());
        let mut last: Option<P> = None;
        for i in 2..line.len().saturating_sub(2) {
            let (a, b) = (i.saturating_sub(4), (i + 4).min(line.len() - 1));
            let deg = angle(sub(line[i], line[a]), sub(line[b], line[i]));
            if deg > 100.0 && last.is_none_or(|p| dist(p, line[i]) > 400.0) {
                last = Some(line[i]);
                spikes.push((line[i], deg, r));
            }
        }
    }
    let spike_wet: Vec<&(P, f64, usize)> = spikes.iter().filter(|s| crossings.iter().any(|c| dist(c.0, s.0) <= MI)).collect();
    say!("  out-and-backs (> 100° over 200 ft): {} ({} within a mile of a river crossing)", spikes.len(), spike_wet.len());
    for s in spikes.iter().take(top) {
        say!("      road {} turns {:.0}° at ({:.0}, {:.0}){}", s.2, s.1, s.0[0], s.0[1], if crossings.iter().any(|c| dist(c.0, s.0) <= MI) { " (near a crossing)" } else { "" });
    }
    // --- Roads running beside roads: share of each class's length (sampled every 200 ft, more than
    // a mile from its ends) within 1,500 ft of another road, where the two share no place within a mile.
    const GRID: f64 = 3000.0;
    let mut segs: Vec<(usize, P, P)> = Vec::new();
    let mut sgrid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (r, q) in pts.iter().enumerate() {
        for w in q.windows(2) {
            let k = segs.len();
            segs.push((r, w[0], w[1]));
            for cy in ((w[0][1].min(w[1][1]) - 1500.0) / GRID).floor() as i64..=((w[0][1].max(w[1][1]) + 1500.0) / GRID).floor() as i64 {
                for cx in ((w[0][0].min(w[1][0]) - 1500.0) / GRID).floor() as i64..=((w[0][0].max(w[1][0]) + 1500.0) / GRID).floor() as i64 {
                    sgrid.entry((cx, cy)).or_default().push(k);
                }
            }
        }
    }
    let (mut samples, mut beside) = ([0usize; 3], [0usize; 3]);
    for (r, q) in pts.iter().enumerate() {
        let len: f64 = q.windows(2).map(|w| dist(w[0], w[1])).sum();
        if len <= 2.0 * MI {
            continue;
        }
        let mut s = MI;
        let (mut seg, mut acc) = (0usize, 0.0);
        while s < len - MI {
            while seg + 2 < q.len() && acc + dist(q[seg], q[seg + 1]) < s {
                acc += dist(q[seg], q[seg + 1]);
                seg += 1;
            }
            let p = lerp(q[seg], q[seg + 1], ((s - acc) / dist(q[seg], q[seg + 1]).max(1e-9)).clamp(0.0, 1.0));
            s += 200.0;
            samples[class[r]] += 1;
            let found = sgrid.get(&((p[0] / GRID).floor() as i64, (p[1] / GRID).floor() as i64)).into_iter().flatten().any(|&k| {
                let (o, a, b) = segs[k];
                o != r && seg_dist(p, a, b) < 1500.0 && !road_nodes[r].intersection(&road_nodes[o]).any(|&n| dist(spots.pos[n], p) < MI)
            });
            beside[class[r]] += found as usize;
        }
    }
    let beside_pct: Vec<f64> = (0..3).map(|c| 100.0 * beside[c] as f64 / samples[c].max(1) as f64).collect();
    say!("  roads beside other roads (within 1,500 ft): {}", (0..3).map(|c| format!("{} {:.1}% of {:.0} mi", class_names[c], beside_pct[c], samples[c] as f64 * 200.0 / MI)).collect::<Vec<_>>().join(", "));
    // --- Gates per town and up, and settlements by tier.
    let tiers = [Tier::Village, Tier::Town, Tier::City, Tier::Metropolis];
    let mut gate_json = Vec::new();
    let mut gate_line = Vec::new();
    for tier in &tiers[1..] {
        let g: Vec<usize> = gates.iter().filter(|g| g.0 == *tier).map(|g| g.1).collect();
        if !g.is_empty() {
            let mean = g.iter().sum::<usize>() as f64 / g.len() as f64;
            let max = *g.iter().max().unwrap();
            gate_line.push(format!("{tier:?} {mean:.2} (max {max}, n {})", g.len()));
            gate_json.push(json!({ "tier": format!("{tier:?}"), "n": g.len(), "mean": mean, "max": max }));
        }
    }
    let gate_mean = gates.iter().map(|g| g.1).sum::<usize>() as f64 / gates.len().max(1) as f64;
    say!("  gates per town and up: mean {gate_mean:.2}; {}", gate_line.join(", "));
    let tier_n: Vec<usize> = tiers.iter().map(|t| sts.iter().filter(|s| s.tier == *t).count()).collect();
    say!("  settlements by tier: village {}, town {}, city {}, metropolis {}  (network {:.1} s)", tier_n[0], tier_n[1], tier_n[2], tier_n[3], t.elapsed().as_secs_f64());
    // Why they stand where they are: counts per reason (rows) and tier (village, town, city, metropolis).
    {
        use worldgen::t0::settle::Reason;
        say!("  settlements by reason and tier (village town city metropolis):");
        for ri in 0..=14u8 {
            let r = Reason::from_u8(ri);
            let c: Vec<usize> = tiers.iter().map(|t| sts.iter().filter(|s| s.reason == r && s.tier == *t).count()).collect();
            if c.iter().sum::<usize>() > 0 {
                say!("    {:<20} {:>4} {:>4} {:>4} {:>4}", r.name(), c[0], c[1], c[2], c[3]);
            }
        }
    }
    // On water: the settlement's cell or one of its 8 neighbours on a coast, a lake shore or a river
    // (`settle::Ground`), per tier (village, town, city, metropolis), as [on water, all].
    let mut on_water = [[0usize; 2]; 4];
    let reasons: Vec<usize> = (0..16u8).map(|ri| sts.iter().filter(|s| s.reason as u8 == ri).count()).collect();
    if let Some(e) = t0.extra.as_ref().filter(|e| !e.waterside.is_empty()) {
        let (w, h) = (t0.height.w, t0.height.h);
        for s in sts.iter().filter(|s| !s.isolated) {
            let (x, y) = ((s.cell % w) as i64, (s.cell / w) as i64);
            let wet = (-1..=1i64).any(|dy| (-1..=1i64).any(|dx| {
                let (i, j) = (x + dx, y + dy);
                i >= 0 && j >= 0 && i < w as i64 && j < h as i64 && e.waterside[j as usize * w + i as usize]
            }));
            on_water[s.tier as usize][0] += wet as usize;
            on_water[s.tier as usize][1] += 1;
        }
        let dry = |k: usize| t0.water.data[k] == worldgen::t0::hydro::DRY;
        let land: Vec<usize> = (0..w * h).filter(|&k| dry(k)).collect();
        let wet_land = land.iter().filter(|&&k| {
            let (x, y) = ((k % w) as i64, (k / w) as i64);
            (-1..=1i64).any(|dy| (-1..=1i64).any(|dx| {
                let (i, j) = (x + dx, y + dy);
                i >= 0 && j >= 0 && i < w as i64 && j < h as i64 && e.waterside[j as usize * w + i as usize]
            }))
        }).count();
        say!("  {} (dry land cells on water {:.0}%)", water_line(&on_water, &reasons), 100.0 * wet_land as f64 / land.len().max(1) as f64);
    }
    let on_water_json: Vec<Value> = on_water.iter().map(|c| json!(c)).collect();
    json!({
        "trips": trips_all, "trips_dry": trips_dry,
        "length_mi": { "total": total / MI, "king": by_class[0] / MI, "road": by_class[1] / MI, "track": by_class[2] / MI, "mst": mst / MI, "ratio": total / mst.max(1.0) },
        "junctions": { "n": njunc, "arms": arms_n, "shape_t_cross": shape_n[0], "shape_y": shape_n[1], "shape_other": shape_n[2], "stubs_half_mi": stub_half, "stubs_half_mi_shared": stub_doubled, "stubs_2_mi": stub_two, "x_crossings": x_cross, "near_crossing": near_cross, "out_and_backs": spikes.len(), "out_and_backs_wet": spike_wet.len() },
        "crossings": { "n": crossings.len(), "bridges": kind_n[0], "fords": kind_n[1], "ferries": kind_n[2], "under_45": under(0.707), "under_30": under(0.5), "within_1_mi": within(MI), "within_3_mi": within(3.0 * MI) },
        "beside_pct": beside_pct, "gates": gate_json, "gates_mean": gate_mean, "tiers": tier_n,
        "on_water": on_water_json, "reasons": reasons,
    })
}
