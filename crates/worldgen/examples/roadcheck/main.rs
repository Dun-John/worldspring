//! QA: how roads, rivers and settlements meet. Counts what should never happen, with the
//! worst places (world ft) to go and look:
//! - walls, towers, buildings and fields over a river channel or standing water;
//! - town streets and approaches over the river with no bridge under them;
//! - approaches (road end to gate) running through built blocks, or turning sharply;
//! - sharp turns on the network roads;
//! - drawn roads (a sketch's road strokes): how closely the road follows each line drawn (share
//!   of the line within a mile of it, mean, 95th percentile and worst offset), and planned roads
//!   crossing a `none` line.
//! - the network: trips between neighbouring settlements against the straight line, road length against the
//!   settlements' minimum spanning tree, junctions outside settlements (arms, shape, stubs), river crossings
//!   (angle, spacing), out-and-backs (near crossings listed), roads running beside roads, gates per town and up, settlements by tier;
//! - steep streets in towns and up: length by grade per tier and kind of street, and the flights
//!   of steps the stair rule would make at three thresholds; `TOP=n` lists the steepest flights.
//! - settlements on water (their cell or a neighbour on a coast, lake shore or river) per tier, and per reason;
//!   `LAYOUTS=0` skips the layouts (T0 and network lines only).
//! `DUMP=file.json` writes every road and settlement (for drawing over a preview).
//!
//! Usage: cargo run --release -p worldgen --example roadcheck -- [seed|world.json...] [--json]   (default 1 2 3)
//! `--json`: one JSON array on stdout, per world `{world, settlements, sites, counts: {class: n}, worst: {class: [[x, y, size]...]}}`
//! (the text report goes to stderr).
/// `--json`: the text report goes to stderr, stdout carries only the JSON.
pub(crate) static JSON: AtomicBool = AtomicBool::new(false);
macro_rules! say {
    ($($t:tt)*) => {
        if JSON.load(Ordering::Relaxed) { eprintln!($($t)*) } else { println!($($t)*) }
    };
}
#[path = "../common/mod.rs"]
mod common;
mod prelude;
mod shared;
mod economy;
mod realms;
mod islands;
mod sea;
mod network;

use prelude::*;

fn main() {
    let args = common::args();
    JSON.store(args.json, Ordering::Relaxed);
    for a in args.pos.iter().filter(|a| !common::is_world(a)) {
        eprintln!("{a}: not a seed or a world file, ignored");
    }
    let worlds: Vec<String> = args.pos.iter().filter(|a| common::is_world(a)).cloned().collect();
    let worlds = if worlds.is_empty() { vec!["1".into(), "2".into(), "3".into()] } else { worlds };
    let mut report = Vec::new();
    // Distance (mi) from each generated site to the nearest settlement or road, per kind, over every world.
    let mut poi_mi: Vec<Vec<f64>> = vec![Vec::new(); 5];
    for arg in &worlds {
        let t = std::time::Instant::now();
        let world = common::world(arg);
        let t0 = T0::generate(&world);
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
        let standing = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
        let n_layouts = worldgen::town::layout_count(&t0);
        say!("{}: {} settlements, {} sites, T0 {:.1} s", common::label(arg), t0.settlements.len(), n_layouts - t0.settlements.len(), t.elapsed().as_secs_f64());
        // Spurs over 5 mi (one end at a settlement, the other joining no road): the longest straight run
        // (points within 200 ft of its chord) and the share of spur length on runs over 3 mi.
        {
            let near_settlement = |q: P| t0.settlements.iter().any(|s| dist(q, [s.x, s.y]) < 6000.0);
            let joins = |ri: usize, q: P| t0.roads.roads.iter().enumerate().any(|(rj, r)| rj != ri && r.pts.windows(2).any(|w| seg_dist(q, w[0], w[1]) < 400.0));
            let (mut n, mut tot, mut on_long, mut worst) = (0, 0.0, 0.0, 0.0f64);
            let mut spurs: Vec<(f64, P, P)> = Vec::new();
            for (ri, rc) in t0.roads.roads.iter().enumerate() {
                let (a, b) = (rc.pts[0], *rc.pts.last().unwrap());
                let len: f64 = rc.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
                let dead = (near_settlement(a) && !joins(ri, b)) || (near_settlement(b) && !joins(ri, a));
                if len < 5.0 * MI || !dead {
                    continue;
                }
                let (mut i, mut longest) = (0, 0.0f64);
                while i + 1 < rc.pts.len() {
                    let mut j = i + 1;
                    let mut best = j;
                    while j < rc.pts.len() && (i + 1..j).all(|m| seg_dist(rc.pts[m], rc.pts[i], rc.pts[j]) < 200.0) {
                        best = j;
                        j += 1;
                    }
                    let run = dist(rc.pts[i], rc.pts[best]);
                    longest = longest.max(run);
                    if run > 3.0 * MI {
                        on_long += run;
                    }
                    i = best;
                }
                n += 1;
                tot += len;
                worst = worst.max(longest);
                if std::env::var("SPURDBG").is_ok() { say!("    spur {:.1} mi ({:.0}, {:.0}) -> ({:.0}, {:.0}) pts {} longest run {:.1} mi {:?}", len / MI, a[0], a[1], b[0], b[1], rc.pts.len(), longest / MI, rc.class); }
                spurs.push((len, a, b));
            }
            spurs.sort_by(|x, y| y.0.total_cmp(&x.0));
            say!("  long spurs (> 5 mi): {n}, {:.0} mi; longest straight run {:.1} mi; length on straight runs over 3 mi {:.0}%; longest: {}", tot / MI, worst / MI, 100.0 * on_long / tot.max(1.0), spurs.iter().take(3).map(|s| format!("{:.0} mi ({:.0}, {:.0}) -> ({:.0}, {:.0})", s.0 / MI, s.1[0], s.1[1], s.2[0], s.2[1])).collect::<Vec<_>>().join("; "));
        }
        for p in t0.pois.iter().filter(|p| p.stroke.is_none()) {
            use worldgen::t0::settle::PoiKind as K;
            let Some(i) = [K::Cave, K::Mine, K::LavaTube, K::Ruin, K::Tower].iter().position(|k| *k == p.kind) else { continue };
            let q = [p.x, p.y];
            let mut d = t0.settlements.iter().map(|s| dist(q, [s.x, s.y])).fold(f64::MAX, f64::min);
            for rc in &t0.roads.roads {
                for w in rc.pts.windows(2) {
                    d = d.min(seg_dist(q, w[0], w[1]));
                }
            }
            poi_mi[i].push(d / 5280.0);
        }
        let t = std::time::Instant::now();
        let mut walls = Tally::default();
        let mut towers = Tally::default();
        let mut buildings = Tally::default();
        let mut afloat = Tally::default();
        let mut fields = Tally::default();
        let mut streets = Tally::default();
        let mut through = Tally::default();
        let mut over = Tally::default();
        let mut turns = Tally::default();
        let mut offroad = Tally::default();
        let mut checked = 0usize;
        let mut bridge_x = Tally::default();
        let mut gate_counts: Vec<(worldgen::t0::settle::Tier, usize)> = Vec::new();
        let mut pier_x = Tally::default();
        // (`LAYOUTS=0`: no layouts, only the T0 and network lines.)
        let layouts = if std::env::var("LAYOUTS").is_ok_and(|v| v == "0") { 0 } else { n_layouts };
        for i in 0..layouts {
            let l = worldgen::town::layout(&world, &t0, i);
            let name = if i < t0.settlements.len() { format!("{:?} {i}", l.tier) } else { format!("site {i}") };
            if i < t0.settlements.len() && l.tier >= worldgen::t0::settle::Tier::Town {
                gate_counts.push((l.tier, l.gates.len()));
            }
            let m = 600.0;
            let ch = Channel::new(&t0, [l.bbox[0] - m, l.bbox[1] - m, l.bbox[2] + m, l.bbox[3] + m]);
            let wet = |p: P, inset: f64| (!ch.segs.is_empty() && ch.inside(p, inset)) || standing(p);
            // Walls: length over water (more than 2 ft inside the channel), towers standing in it.
            for w in &l.walls {
                let s = along(w, 5.0);
                let n = s.iter().filter(|p| ch.inside(**p, 2.0)).count();
                if n > 0 {
                    let p = *s.iter().find(|p| ch.inside(**p, 2.0)).unwrap();
                    walls.add(n as f64 * 5.0, p, format!("{name}: {:.0} ft of wall in the river near ({:.0}, {:.0})", n as f64 * 5.0, p[0], p[1]));
                }
            }
            for &tw in l.towers.iter().chain(&l.gate_towers) {
                if ch.inside(tw, 2.0) {
                    towers.add(1.0, tw, format!("{name}: tower in the river at ({:.0}, {:.0})", tw[0], tw[1]));
                }
            }
            // (R5 M20: a house on a bridge's or pier's deck, an islet's watchtower and a breakwater's
            // lighthouse stand over the water on purpose.)
            for b in &l.buildings {
                let o = outline(&b.poly);
                let n = o.iter().filter(|p| wet(**p, 2.0)).count();
                if n > 0 {
                    let c = centroid(&b.poly);
                    let (tower, light) = (worldgen::town::is_watchtower(b), worldgen::town::harbour::is_lighthouse(b));
                    if b.deck.is_some() || tower || light {
                        let what = if tower { "islet watchtower" } else if light { "lighthouse" } else { "house on a deck" };
                        afloat.add(n as f64, c, format!("{name}: {what} ({}/{} points wet) at ({:.0}, {:.0})", n, o.len(), c[0], c[1]));
                    } else {
                        buildings.add(n as f64, c, format!("{name}: building {:?} ({}/{} points wet) at ({:.0}, {:.0})", b.func, n, o.len(), c[0], c[1]));
                    }
                }
            }
            for f in &l.fields {
                let o = outline(f);
                let n = o.iter().filter(|p| ch.inside(**p, 2.0)).count();
                if n > 0 {
                    let c = centroid(f);
                    fields.add(n as f64, c, format!("{name}: field over the river ({n} points) at ({:.0}, {:.0})", c[0], c[1]));
                }
            }
            // Decks overlapping: bridges with bridges, piers with bridges or network roads.
            let overlap = |a: &[P], b: &[P]| a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p)) || outline(a).iter().any(|p| contains(b, *p));
            for (i, a) in l.bridges.iter().enumerate() {
                for b in &l.bridges[i + 1..] {
                    if overlap(a, b) {
                        let c = centroid(a);
                        bridge_x.add(1.0, c, format!("{name}: bridges overlap at ({:.0}, {:.0})", c[0], c[1]));
                    }
                }
            }
            for pier in &l.piers {
                let c = centroid(pier);
                if l.bridges.iter().any(|b| overlap(pier, b)) {
                    pier_x.add(1.0, c, format!("{name}: pier on a bridge at ({:.0}, {:.0})", c[0], c[1]));
                }
                let on_road = t0.roads.segments_near([c[0] - 100.0, c[1] - 100.0, c[0] + 100.0, c[1] + 100.0], 0.0).iter().any(|&(ri, k)| {
                    let rc = &t0.roads.roads[ri as usize];
                    (0..=8).any(|j| {
                        let q = rc.eval(k as usize, j as f64 / 8.0, 5.0, t0.cell_ft).p;
                        outline(pier).iter().any(|p| dist(*p, q) < 0.5 * rc.class.width_ft() + 2.0)
                    })
                });
                if on_road {
                    pier_x.add(1.0, c, format!("{name}: pier on a network road at ({:.0}, {:.0})", c[0], c[1]));
                }
            }
            // Streets and approaches over the river with no bridge deck under them.
            for (pts, class, _) in &l.roads {
                let s = along(pts, 5.0);
                let bad: Vec<&P> = s.iter().filter(|p| ch.inside(**p, 2.0) && !l.bridges.iter().any(|b| contains(b, **p))).collect();
                if !bad.is_empty() {
                    streets.add(bad.len() as f64, *bad[0], format!("{name}: class {class} street, {} ft over the river unbridged near ({:.0}, {:.0})", bad.len() * 5, bad[0][0], bad[0][1]));
                }
            }
            // Approaches (road classes 0–2): through built blocks or buildings, and their sharpest turn.
            for (pts, class, _) in l.roads.iter().filter(|r| r.1 <= 2) {
                let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
                let s = along(pts, 10.0);
                let keep = ((total - 40.0) / 10.0).max(0.0) as usize;
                let inside = s.iter().take(keep).filter(|p| !wet(**p, 0.0) && l.districts.iter().any(|b| contains(b, **p))).count();
                if inside > 0 {
                    through.add(inside as f64, pts[0], format!("{name}: class {class} approach crosses {} ft of the town, from ({:.0}, {:.0})", inside * 10, pts[0][0], pts[0][1]));
                }
                let on = s.iter().take(keep).filter(|p| l.buildings.iter().any(|b| contains(&b.poly, **p))).count();
                if on > 0 {
                    over.add(on as f64, pts[0], format!("{name}: class {class} approach runs over {} ft of buildings, from ({:.0}, {:.0})", on * 10, pts[0][0], pts[0][1]));
                }
                // Its outer end on the network road's curve (within 3 ft).
                if i < t0.settlements.len() {
                    let a = pts[0];
                    let mut d = f64::MAX;
                    for (ri, k) in t0.roads.segments_near([a[0] - 200.0, a[1] - 200.0, a[0] + 200.0, a[1] + 200.0], 0.0) {
                        let rc = &t0.roads.roads[ri as usize];
                        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 2.0).ceil().max(1.0) as usize;
                        for j in 0..=n {
                            d = d.min(dist(a, rc.eval(k as usize, j as f64 / n as f64, 5.0, t0.cell_ft).p));
                        }
                    }
                    checked += (d < 1e9) as usize;
                    if d > 3.0 && d < 1e9 {
                        offroad.add(d, a, format!("{name}: class {class} approach starts {d:.0} ft off the network road at ({:.0}, {:.0})", a[0], a[1]));
                    }
                }
                if pts.len() >= 3 {
                    let (deg, at) = sharpest(pts, 30.0);
                    if deg > 60.0 {
                        turns.add(deg, at, format!("{name}: approach turns {deg:.0}° at ({:.0}, {:.0})", at[0], at[1]));
                    }
                }
            }
        }
        let layout_s = t.elapsed().as_secs_f64();
        // Network roads: turns sharper than 75° between 40-ft chords; and the join with each approach.
        let mut kinks = Tally::default();
        if let Some(near) = std::env::var("NEAR").ok() {
            let v: Vec<f64> = near.split(',').filter_map(|x| x.parse().ok()).collect();
            for (ri, rc) in t0.roads.roads.iter().enumerate() {
                let (a, b) = (rc.pts[0], *rc.pts.last().unwrap());
                if [a, b].iter().any(|p| dist(*p, [v[0], v[1]]) < v[2]) {
                    say!("  road {ri} {:?} ({:.0}, {:.0}) -> ({:.0}, {:.0}) {:.0} ft", rc.class, a[0], a[1], b[0], b[1], rc.s.last().unwrap());
                }
            }
        }
        if let Some(ri) = std::env::var("ROAD").ok().and_then(|v| v.parse::<usize>().ok()) {
            let rc = &t0.roads.roads[ri];
            for (k, p) in rc.pts.iter().enumerate() {
                let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
                let plan = t0.rivers.valley(t0.ground_at(p[0], p[1], lattice), p[0], p[1]);
                say!("  road {ri} pt {k}: ({:.0}, {:.0}) z {:.0} (ground {plan:.0}) wander {:.2} s {:.0}", p[0], p[1], rc.z[k], rc.wander[k], rc.s[k]);
            }
            for (si, st) in t0.settlements.iter().enumerate().filter(|(_, st)| rc.pts.iter().any(|p| dist(*p, [st.x, st.y]) < 3.0 * t0.cell_ft)) {
                say!("  settlement {si} {:?} at ({:.0}, {:.0}), trim {:.0}", st.tier, st.x, st.y, worldgen::town::road_trim_radius(st.tier, st.population));
            }
        }
        for (ri, rc) in t0.roads.roads.iter().enumerate() {
            let mut pts: Vec<P> = Vec::new();
            for k in 0..rc.pts.len() - 1 {
                let n = ((rc.s[k + 1] - rc.s[k]) / 10.0).ceil().max(1.0) as usize;
                for j in 0..n {
                    pts.push(rc.eval(k, j as f64 / n as f64, 5.0, t0.cell_ft).p);
                }
            }
            pts.push(rc.eval(rc.pts.len() - 2, 1.0, 5.0, t0.cell_ft).p);
            let k = 4;
            let mut last: Option<P> = None;
            for i in k..pts.len().saturating_sub(k) {
                let deg = angle(sub(pts[i], pts[i - k]), sub(pts[i + k], pts[i]));
                if deg > 75.0 && last.is_none_or(|q| dist(q, pts[i]) > 300.0) {
                    last = Some(pts[i]);
                    let near = t0.settlements.iter().map(|st| (dist([st.x, st.y], pts[i]) / worldgen::town::road_trim_radius(st.tier, st.population), st.tier)).min_by(|a, b| a.0.total_cmp(&b.0));
                    let near = near.map(|(r, tier)| format!(", {r:.2} trim radii from a {tier:?}")).unwrap_or_default();
                    let from_end = (i as f64 * 10.0).min((pts.len() - 1 - i) as f64 * 10.0);
                    kinks.add(deg, pts[i], format!("road {ri} ({:?}) turns {deg:.0}° at ({:.0}, {:.0}), {from_end:.0} ft from its end{near}", rc.class, pts[i][0], pts[i][1]));
                }
            }
        }
        // Map lines: each road's curve sampled every 1,000 ft (as a mid-zoom map draws it), the
        // turn at each sample: the sharpest, and how many turn over 45° (one per 2,000 ft).
        {
            let (mut worst, mut over45, mut worst_at) = (0.0f64, 0usize, [0.0, 0.0]);
            // (Of those: on switchback legs (no wander), within 1,500 ft of a road's end.)
            let (mut on_legs, mut at_ends) = (0usize, 0usize);
            let top = std::env::var("TOP").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            let mut listed: Vec<(f64, String)> = Vec::new();
            for rc in &t0.roads.roads {
                let total = *rc.s.last().unwrap();
                let n = (total / 1000.0).ceil().max(1.0) as usize;
                let (mut k, mut q, mut ks) = (0usize, Vec::with_capacity(n + 1), Vec::with_capacity(n + 1));
                for j in 0..=n {
                    let d = total * j as f64 / n as f64;
                    while k + 2 < rc.pts.len() && rc.s[k + 1] <= d {
                        k += 1;
                    }
                    let t = ((d - rc.s[k]) / (rc.s[k + 1] - rc.s[k]).max(1e-9)).clamp(0.0, 1.0);
                    q.push(rc.eval(k, t, 500.0, t0.cell_ft).p);
                    ks.push((k, d));
                }
                let mut last: Option<P> = None;
                for i in 1..q.len().saturating_sub(1) {
                    let deg = angle(sub(q[i], q[i - 1]), sub(q[i + 1], q[i]));
                    if deg > worst {
                        (worst, worst_at) = (deg, q[i]);
                    }
                    if deg > 45.0 && last.is_none_or(|p| dist(p, q[i]) > 2000.0) {
                        over45 += 1;
                        last = Some(q[i]);
                        let (k, d) = ks[i];
                        let leg = rc.wander[k] == 0.0 && rc.wander[k + 1] == 0.0;
                        let end = d.min(total - d) < 1500.0;
                        on_legs += leg as usize;
                        at_ends += (end && !leg) as usize;
                        if top > 0 {
                            let (a, b) = ((0..k).rev().find(|&j| rc.s[k] - rc.s[j] >= 500.0).unwrap_or(0), (k..rc.pts.len()).find(|&j| rc.s[j] - rc.s[k] >= 500.0).unwrap_or(rc.pts.len() - 1));
                            let grade = (rc.z[b] - rc.z[a]).abs() as f64 / (rc.s[b] - rc.s[a]).max(1.0) / rc.class.max_grade();
                            listed.push((deg, format!("      {deg:.0}° {:?} at ({:.0}, {:.0}) pt {k} {:.0} ft from an end of {total:.0}, grade {grade:.2} of max{}{}", rc.class, q[i][0], q[i][1], d.min(total - d), if leg { " leg" } else { "" }, if end { " end" } else { "" })));
                        }
                    }
                }
            }
            say!("  map lines (every 1,000 ft): sharpest turn {worst:.0}° at ({:.0}, {:.0}), turns > 45°: {over45} (switchback legs {on_legs}, near an end {at_ends})", worst_at[0], worst_at[1]);
            listed.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (_, l) in listed.iter().take(top) {
                say!("{l}");
            }
        }
        // Bends: the heading 750 ft before a point and 750 ft after differing by over 60° (at the
        // sharpest point, one per 1,500 ft); zigzag where the bend next to it, under 3,000 ft
        // away, turns the other way; hairpins over 150°; sharp (a V, not a rounded turn) where it
        // turns over 90° within 300 ft. Per 10 mi of climbing road (over 4% across 500 ft).
        let (mut bends, mut hairpins, mut sharp, mut zig, mut climb_ft) = (0usize, 0usize, 0usize, 0usize, 0.0f64);
        let mut hp_at: Vec<(P, f64, bool, bool)> = Vec::new();
        let mut fingers: [Vec<(P, P, f64)>; 2] = [Vec::new(), Vec::new()];
        let mut stacks: Vec<(P, usize)> = Vec::new();
        let sang = |u: P, v: P| libm::atan2(u[0] * v[1] - u[1] * v[0], u[0] * v[0] + u[1] * v[1]).to_degrees();
        // (`CLIMB=1`: climbing road by class, and by what it leads to: the least place at its ends.)
        let mut climb_by: std::collections::BTreeMap<String, (f64, f64)> = Default::default();
        for rc in t0.roads.roads.iter() {
            let before = climb_ft;
            for k in 0..rc.pts.len() - 1 {
                let (s0, z0) = (rc.s[k], rc.z[k]);
                if let Some(j) = (k..rc.pts.len()).find(|&j| rc.s[j] - s0 >= 500.0) {
                    if ((rc.z[j] - z0).abs() as f64) > 0.04 * (rc.s[j] - s0) {
                        climb_ft += rc.s[k + 1] - s0;
                    }
                }
            }
            if std::env::var("CLIMB").is_ok_and(|v| v == "1") {
                let reach = |st: &worldgen::t0::settle::Settlement| worldgen::town::road_trim_radius(st.tier, st.population).max(300.0) * 1.15 + 100.0;
                let at_end = [rc.pts[0], rc.pts[rc.pts.len() - 1]].iter().filter_map(|p| t0.settlements.iter().find(|st| dist([st.x, st.y], *p) <= reach(st))).min_by_key(|st| st.tier);
                let to = match at_end {
                    Some(st) if st.tier == worldgen::t0::settle::Tier::Village => format!("village/{}", st.reason.name()),
                    Some(st) => format!("{:?}", st.tier),
                    None => "junctions".into(),
                };
                let len = *rc.s.last().unwrap();
                for key in [format!("class {:?}", rc.class), format!("to {to}")] {
                    let e = climb_by.entry(key).or_default();
                    e.0 += climb_ft - before;
                    e.1 += len;
                }
            }
            let mut q: Vec<P> = Vec::new();
            for k in 0..rc.pts.len() - 1 {
                let n = ((rc.s[k + 1] - rc.s[k]) / 50.0).ceil().max(1.0) as usize;
                q.extend((0..n).map(|j| rc.eval(k, j as f64 / n as f64, 5.0, t0.cell_ft).p));
            }
            // DUMP=x,y,r: the road's points (every 1,000 ft) within r ft of (x, y).
            if let Some(d) = std::env::var("DUMP").ok().map(|v| v.split(',').filter_map(|s| s.parse::<f64>().ok()).collect::<Vec<f64>>()).filter(|d| d.len() == 3) {
                let near: Vec<String> = q.iter().step_by(20).filter(|p| dist(**p, [d[0], d[1]]) <= d[2]).map(|p| format!("{:.0},{:.0}", p[0], p[1])).collect();
                if !near.is_empty() {
                    say!("    dump {:?} n{}: {}", rc.class, rc.pts.len(), near.join(" "));
                }
            }
            const L: usize = 15;
            let turn = |i: usize, l: usize| sang(sub(q[i], q[i - l]), sub(q[i + l], q[i]));
            let mut ev: Vec<(usize, f64, f64)> = Vec::new();
            let mut i = L;
            while i + L < q.len() {
                if turn(i, L).abs() <= 60.0 {
                    i += 1;
                    continue;
                }
                let best = (i..(i + L).min(q.len() - L)).max_by(|&x, &y| turn(x, L).abs().total_cmp(&turn(y, L).abs())).unwrap_or(i);
                let tight = if best >= 6 && best + 6 < q.len() { turn(best, 6).abs() } else { 0.0 };
                ev.push((best, turn(best, L), tight));
                i = best + 2 * L;
            }
            // Stacks: two or more hairpins (> 120° between their legs) in a row, turning alternately, each leg
            // between them 0.3 to 1.2 mi; not zigzags (nor fingers, over its legs and one more each way).
            // (A hairpin's turn is the heading change between its legs: each leg's heading over its
            // middle stretch, from half to all of the way to the bend beside it, at most 3,000 ft.)
            let pin = |f: usize| {
                let (i, gap) = (ev[f].0, |g: usize| ev[g].0.abs_diff(ev[f].0) / 2);
                let mut l = 60usize.min(i).min(q.len() - 1 - i);
                if f > 0 {
                    l = l.min(gap(f - 1));
                }
                if f + 1 < ev.len() {
                    l = l.min(gap(f + 1));
                }
                l >= L && sang(sub(q[i - l / 2], q[i - l]), sub(q[i + l], q[i + l / 2])).abs() > 120.0
            };
            let leg_ok = |f: usize| (ev[f].1 * ev[f + 1].1 < 0.0) && ((ev[f + 1].0 - ev[f].0) as f64 * 50.0 / MI) >= 0.3 && ((ev[f + 1].0 - ev[f].0) as f64 * 50.0 / MI) <= 1.2;
            let mut in_stack = vec![false; ev.len()];
            let mut spans: Vec<(usize, usize)> = Vec::new();
            let mut f = 0;
            while f < ev.len() {
                let mut g = f;
                while pin(f) && g + 1 < ev.len() && pin(g + 1) && leg_ok(g) {
                    g += 1;
                }
                if g > f {
                    let longest = (f..g).map(|x| ev[x + 1].0 - ev[x].0).max().unwrap_or(0);
                    in_stack[f..=g].iter_mut().for_each(|v| *v = true);
                    spans.push((ev[f].0.saturating_sub(longest + 30), ev[g].0 + longest + 30));
                    stacks.push((q[ev[f].0], g - f + 2));
                    f = g + 1;
                } else {
                    f += 1;
                }
            }
            for (e, &(i, a, t)) in ev.iter().enumerate() {
                let z = !in_stack[e] && [e.wrapping_sub(1), e + 1].iter().any(|&f| f < ev.len() && !in_stack[f] && ev[f].0.abs_diff(i) < 60 && ev[f].1 * a < 0.0);
                bends += 1;
                hairpins += (a.abs() > 150.0) as usize;
                sharp += (t > 90.0) as usize;
                zig += z as usize;
                hp_at.push((q[i], a, t > 90.0, z));
            }
            // Fingers: two points >= 1.5 mi apart along the road but within 0.3 mi of each other
            // (the road runs out and comes back beside itself), unless every leg between them
            // (from bend to bend, and from each point to its nearest bend) is <= 0.6 mi: a
            // compact switchback stack. One per such span. (Every 5th point: 250 ft apart.)
            // (Also counted within 0.6 mi: wider fingers.)
            for (wide, near) in [0.3, 0.6].into_iter().enumerate() {
                let (gap, near, leg) = ((1.5 * MI / 50.0) as usize, near * MI, 0.6 * MI / 50.0);
                let mut i = 0;
                while i + gap < q.len() {
                    let Some(j) = (i + gap..q.len()).step_by(5).find(|&j| dist(q[i], q[j]) <= near) else {
                        i += 5;
                        continue;
                    };
                    let mut marks: Vec<usize> = vec![i];
                    marks.extend(ev.iter().map(|e| e.0).filter(|&e| e > i && e < j));
                    marks.push(j);
                    if marks.windows(2).any(|w| (w[1] - w[0]) as f64 > leg) && !spans.iter().any(|&(x, y)| x <= i && j <= y) {
                        let tip = (i..=j).max_by(|&x, &y| dist(q[i], q[x]).total_cmp(&dist(q[i], q[y]))).unwrap_or(i);
                        fingers[wide].push((q[i], q[tip], (j - i) as f64 * 50.0 / MI));
                    }
                    i = j + 5;
                }
            }
        }
        let per10 = |n: usize| if climb_ft > 0.0 { n as f64 / (climb_ft / (10.0 * MI)) } else { 0.0 };
        say!(
            "  bends (> 60° over 750 ft): {bends}, zigzag {zig} ({:.1} per 10 mi of {:.0} mi climbing road), hairpins > 150° {hairpins}, sharp (V, > 90° within 300 ft) {sharp}",
            per10(zig),
            climb_ft / MI
        );
        for (k, (c, l)) in &climb_by {
            say!("    CLIMB {k}: {:.1} mi of {:.1} mi", c / MI, l / MI);
        }
        // (`CLIMB=1`: each spring village's ground against the 5x5 cells round it: above their
        // lowest, below their highest.)
        if std::env::var("CLIMB").is_ok_and(|v| v == "1") {
            let (hw, hh) = (t0.height.w, t0.height.h);
            let (mut n, mut up, mut sum_lo, mut sum_hi, mut sloped) = (0usize, 0usize, 0.0f64, 0.0f64, 0usize);
            for st in t0.settlements.iter().filter(|st| st.reason == worldgen::t0::settle::Reason::Spring) {
                let (x, y) = (st.cell % hw, st.cell / hw);
                let h0 = t0.height.get(x, y) as f64;
                let (mut lo, mut hi) = (h0, h0);
                for yy in y.saturating_sub(2)..(y + 3).min(hh) {
                    for xx in x.saturating_sub(2)..(x + 3).min(hw) {
                        let v = t0.height.get(xx, yy) as f64;
                        (lo, hi) = (lo.min(v), hi.max(v));
                    }
                }
                n += 1;
                sum_lo += h0 - lo;
                sum_hi += hi - h0;
                up += (h0 - lo > 0.5 * (hi - lo)) as usize;
                sloped += (h0 - lo > 150.0) as usize;
            }
            say!("    CLIMB springs {n}: mean {:.0} ft above the lowest of 5x5, {:.0} below the highest; in the upper half {up}, over 150 ft above the lowest {sloped}", sum_lo / n.max(1) as f64, sum_hi / n.max(1) as f64);
        }
        for h in hp_at.iter().filter(|h| h.2 || h.1.abs() > 150.0).take(std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4)) {
            say!("      bend {:.0}° at ({:.0}, {:.0}){}{}", h.1, h.0[0], h.0[1], if h.2 { " sharp" } else { "" }, if h.3 { " zigzag" } else { "" });
        }
        say!(
            "  stacks (2+ alternating hairpins > 120°, legs 0.3-1.2 mi): {}, legs {}, most legs {}",
            stacks.len(),
            stacks.iter().map(|s| s.1).sum::<usize>(),
            stacks.iter().map(|s| s.1).max().unwrap_or(0)
        );
        for s in stacks.iter().filter(|s| s.1 > 8).take(4) {
            say!("      stack of {} legs at ({:.0}, {:.0})", s.1, s.0[0], s.0[1]);
        }
        say!("  fingers (out and back beside itself, >= 1.5 mi along within 0.3 mi, not a compact switchback stack): {}; within 0.6 mi {}", fingers[0].len(), fingers[1].len());
        for f in fingers[1].iter().take(std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4)) {
            say!("      finger {:.1} mi from ({:.0}, {:.0}), tip ({:.0}, {:.0})", f.2, f.0[0], f.0[1], f.1[0], f.1[1]);
        }
        let net = network(&t0, &standing, &gate_counts);
        realms(&world, &t0);
        islands(&world, &t0);
        sea_lanes(&t0, world.geom.spacing_ft(world.geom.first_refine_level - 1));
        sea_ferries(&world, &t0);
        if let Ok(v) = std::env::var("SHORE_PROBE") {
            shore_probe(&world, &t0, &v);
        }
        let econ = economy(&t0);
        let drawn = drawn_roads(&world, &t0, &standing);
        let steps = stairs(&world, &t0);
        // `DUMP=file.json`: every road (class, drawn stroke, control points) and settlement, for
        // drawing over a preview.
        // (`NAME=x`: where the features named x are.)
        if let (Ok(q), Some(e)) = (std::env::var("NAME"), t0.extra.as_ref()) {
            for f in e.overlay.features.iter().filter(|f| f.name.contains(&q)) {
                say!("  named {} ({}) at ({:.0}, {:.0})", f.name, f.kind, f.x, f.y);
            }
        }
        if let Ok(path) = std::env::var("DUMP") {
            let roads: Vec<Value> = t0.roads.roads.iter().map(|rc| json!({ "class": rc.class as u8, "stroke": rc.stroke, "pts": rc.pts.iter().map(|p| [p[0].round(), p[1].round()]).collect::<Vec<_>>() })).collect();
            let places = t0.extra.as_ref().map(|e| &e.economy.places[..]).unwrap_or(&[]);
            let towns: Vec<Value> = t0
                .settlements
                .iter()
                .enumerate()
                .map(|(i, st)| {
                    let e = places.iter().find(|p| p.index == i).map(|p| json!({ "was": p.was as u8, "wealth": p.wealth, "food": p.food, "industry": p.industry, "gateway": p.gateway, "trade": p.trade }));
                    json!({ "tier": st.tier as u8, "kind": st.kind.name(), "reason": st.reason.name(), "x": st.x.round(), "y": st.y.round(), "population": st.population, "economy": e })
                })
                .collect();
            std::fs::write(&path, json!({ "cell_ft": t0.cell_ft, "roads": roads, "settlements": towns }).to_string()).expect("DUMP file");
        }
        let (mut counts, mut worst) = (Map::new(), Map::new());
        for (tally, label, key) in [
            (&mut bridge_x, "town bridges overlapping each other", "bridges_overlapping"),
            (&mut pier_x, "piers on bridges or network roads", "piers_on_bridges_or_roads"),
            (&mut walls, "wall runs in a river", "walls_in_river"),
            (&mut towers, "towers in a river", "towers_in_river"),
            (&mut buildings, "buildings over water", "buildings_over_water"),
            (&mut afloat, "buildings over water on purpose (decks, islet towers, lighthouses)", "buildings_afloat"),
            (&mut fields, "fields over a river", "fields_over_river"),
            (&mut streets, "streets over a river with no bridge", "streets_unbridged"),
            (&mut through, "approaches through the town (its districts)", "approaches_through_town"),
            (&mut over, "approaches over buildings", "approaches_over_buildings"),
            (&mut turns, "approaches turning > 60° within 30 ft", "approach_turns"),
            (&mut kinks, "network road turns > 75° within 40 ft", "network_road_turns"),
            (&mut offroad, "approaches starting off the network road (> 3 ft)", "approaches_off_road"),
        ] {
            tally.print(label, key, &mut counts, &mut worst);
        }
        say!("  ({n_layouts} layouts in {layout_s:.1} s; {checked} approaches checked against the network road)");
        report.push(json!({ "world": common::label(arg), "settlements": t0.settlements.len(), "sites": n_layouts - t0.settlements.len(), "counts": counts, "worst": worst, "drawn_roads": drawn, "stairs": steps, "network": net, "economy": econ }));
    }
    for (name, v) in ["cave", "mine", "lava tube", "ruin", "tower"].iter().zip(poi_mi.iter_mut()) {
        v.sort_by(|a, b| a.total_cmp(b));
        let n = v.len().max(1) as f64;
        let share = |f: &dyn Fn(f64) -> bool| v.iter().filter(|d| f(**d)).count() as f64 / n * 100.0;
        say!("pois {name}: n {}, within 50 mi {:.0}%, median {:.1} mi, 5-25 mi {:.0}%", v.len(), share(&|d| d <= 50.0), v.get(v.len() / 2).copied().unwrap_or(0.0), share(&|d| (5.0..=25.0).contains(&d)));
    }
    // The waterside shares and reasons over every world.
    if worlds.len() > 1 {
        let mut on_water = [[0usize; 2]; 4];
        let mut reasons = vec![0usize; 16];
        for r in &report {
            if let Some(a) = r["network"]["on_water"].as_array() {
                for (t, c) in a.iter().enumerate().take(4) {
                    on_water[t][0] += c[0].as_u64().unwrap_or(0) as usize;
                    on_water[t][1] += c[1].as_u64().unwrap_or(0) as usize;
                }
            }
            if let Some(a) = r["network"]["reasons"].as_array() {
                for (i, c) in a.iter().enumerate().take(16) {
                    reasons[i] += c.as_u64().unwrap_or(0) as usize;
                }
            }
        }
        say!("total ({} worlds): {}", report.len(), water_line(&on_water, &reasons));
    }
    if args.json {
        println!("{}", Value::Array(report));
    }
}
