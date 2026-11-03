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
//! `DUMP=file.json` writes every road and settlement (for drawing over a preview).
//!
//! Usage: cargo run --release -p worldgen --example roadcheck -- [seed|world.json...] [--json]   (default 1 2 3)
//! `--json`: one JSON array on stdout, per world `{world, settlements, sites, counts: {class: n}, worst: {class: [[x, y, size]...]}}`
//! (the text report goes to stderr).
mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Map, Value};
use worldgen::t0::T0;
use worldgen::town::geom::{centroid, contains, dist, lerp, seg_dist, sub};

/// `--json`: the text report goes to stderr, stdout carries only the JSON.
static JSON: AtomicBool = AtomicBool::new(false);
macro_rules! say {
    ($($t:tt)*) => {
        if JSON.load(Ordering::Relaxed) { eprintln!($($t)*) } else { println!($($t)*) }
    };
}

type P = [f64; 2];
const CELL: f64 = 100.0;

/// River channel pieces (fine curve, ~20-ft chords) near a rectangle, in a grid.
struct Channel {
    segs: Vec<(P, P, f64)>,
    grid: HashMap<(i64, i64), Vec<usize>>,
}

impl Channel {
    fn new(t0: &T0, r: [f64; 4]) -> Channel {
        let mut segs = Vec::new();
        for (ri, k) in t0.rivers.segments_near(r[0], r[1], r[2], r[3], 0.0) {
            let rc = &t0.rivers.rivers[ri as usize];
            let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(2.0, 2000.0) as usize;
            let mut prev: Option<(P, f64)> = None;
            for j in 0..=n {
                let c = rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft);
                if let Some((q, w)) = prev {
                    segs.push((q, c.p, 0.5 * w.min(c.w)));
                }
                prev = Some((c.p, c.w));
            }
        }
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, &(a, b, hw)) in segs.iter().enumerate() {
            for cy in ((a[1].min(b[1]) - hw) / CELL).floor() as i64..=((a[1].max(b[1]) + hw) / CELL).floor() as i64 {
                for cx in ((a[0].min(b[0]) - hw) / CELL).floor() as i64..=((a[0].max(b[0]) + hw) / CELL).floor() as i64 {
                    grid.entry((cx, cy)).or_default().push(i);
                }
            }
        }
        Channel { segs, grid }
    }
    /// Inside the channel (less `inset` ft from its edge counts as outside).
    fn inside(&self, p: P, inset: f64) -> bool {
        let (cx, cy) = ((p[0] / CELL).floor() as i64, (p[1] / CELL).floor() as i64);
        self.grid.get(&(cx, cy)).into_iter().flatten().any(|&i| {
            let (a, b, hw) = self.segs[i];
            seg_dist(p, a, b) < hw - inset
        })
    }
}

/// Points over a polygon: corners, every ~5 ft along the edges, and the centroid.
fn outline(poly: &[P]) -> Vec<P> {
    let m = poly.len();
    let mut out = vec![centroid(poly)];
    for k in 0..m {
        let (a, b) = (poly[k], poly[(k + 1) % m]);
        let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
        out.extend((0..n).map(|j| lerp(a, b, j as f64 / n as f64)));
    }
    out
}

/// Points along a polyline every ~`step` ft (ends included).
fn along(pts: &[P], step: f64) -> Vec<P> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        let n = (dist(w[0], w[1]) / step).ceil().max(1.0) as usize;
        out.extend((0..n).map(|j| lerp(w[0], w[1], j as f64 / n as f64)));
    }
    if let Some(&l) = pts.last() {
        out.push(l);
    }
    out
}

fn angle(a: P, b: P) -> f64 {
    let (la, lb) = ((a[0] * a[0] + a[1] * a[1]).sqrt(), (b[0] * b[0] + b[1] * b[1]).sqrt());
    if la < 1e-9 || lb < 1e-9 {
        return 0.0;
    }
    ((a[0] * b[0] + a[1] * b[1]) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Sharpest turn (deg) along a polyline between chords of about `chord` ft, and where.
fn sharpest(pts: &[P], chord: f64) -> (f64, P) {
    let s = along(pts, 5.0);
    let k = (chord / 5.0).round().max(1.0) as usize;
    let mut best = (0.0, pts[0]);
    for i in k..s.len().saturating_sub(k) {
        let t = angle(sub(s[i], s[i - k]), sub(s[i + k], s[i]));
        if t > best.0 {
            best = (t, s[i]);
        }
    }
    best
}

#[derive(Default)]
struct Tally {
    n: usize,
    worst: Vec<(f64, P, String)>,
}

impl Tally {
    fn add(&mut self, size: f64, at: P, what: String) {
        self.n += 1;
        self.worst.push((size, at, what));
    }
    /// Prints the count and the worst few, and records them under `key` in `counts` and `worst`.
    fn print(&mut self, label: &str, key: &str, counts: &mut Map<String, Value>, worst: &mut Map<String, Value>) {
        self.worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        let top = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        say!("  {label}: {}", self.n);
        for (_, _, w) in self.worst.iter().take(top) {
            say!("      {w}");
        }
        counts.insert(key.into(), json!(self.n));
        worst.insert(key.into(), self.worst.iter().take(top).map(|(size, at, _)| json!([at[0].round(), at[1].round(), size])).collect());
    }
}

fn main() {
    let args = common::args();
    JSON.store(args.json, Ordering::Relaxed);
    for a in args.pos.iter().filter(|a| !common::is_world(a)) {
        eprintln!("{a}: not a seed or a world file, ignored");
    }
    let worlds: Vec<String> = args.pos.iter().filter(|a| common::is_world(a)).cloned().collect();
    let worlds = if worlds.is_empty() { vec!["1".into(), "2".into(), "3".into()] } else { worlds };
    let mut report = Vec::new();
    for arg in &worlds {
        let t = std::time::Instant::now();
        let world = common::world(arg);
        let t0 = T0::generate(&world);
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
        let standing = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
        let n_layouts = worldgen::town::layout_count(&t0);
        say!("{}: {} settlements, {} sites, T0 {:.1} s", common::label(arg), t0.settlements.len(), n_layouts - t0.settlements.len(), t.elapsed().as_secs_f64());
        let t = std::time::Instant::now();
        let mut walls = Tally::default();
        let mut towers = Tally::default();
        let mut buildings = Tally::default();
        let mut fields = Tally::default();
        let mut streets = Tally::default();
        let mut through = Tally::default();
        let mut over = Tally::default();
        let mut turns = Tally::default();
        let mut bridge_x = Tally::default();
        let mut gate_counts: Vec<(worldgen::t0::settle::Tier, usize)> = Vec::new();
        let mut pier_x = Tally::default();
        for i in 0..n_layouts {
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
            for b in &l.buildings {
                let o = outline(&b.poly);
                let n = o.iter().filter(|p| wet(**p, 2.0)).count();
                if n > 0 {
                    let c = centroid(&b.poly);
                    buildings.add(n as f64, c, format!("{name}: building {:?} ({}/{} points wet) at ({:.0}, {:.0})", b.func, n, o.len(), c[0], c[1]));
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
        let net = network(&t0, &standing, &gate_counts);
        realms(&world, &t0);
        let econ = economy(&t0);
        let drawn = drawn_roads(&world, &t0, &standing);
        let steps = stairs(&world, &t0);
        // `DUMP=file.json`: every road (class, drawn stroke, control points) and settlement, for
        // drawing over a preview.
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
            (&mut fields, "fields over a river", "fields_over_river"),
            (&mut streets, "streets over a river with no bridge", "streets_unbridged"),
            (&mut through, "approaches through the town (its districts)", "approaches_through_town"),
            (&mut over, "approaches over buildings", "approaches_over_buildings"),
            (&mut turns, "approaches turning > 60° within 30 ft", "approach_turns"),
            (&mut kinks, "network road turns > 75° within 40 ft", "network_road_turns"),
        ] {
            tally.print(label, key, &mut counts, &mut worst);
        }
        say!("  ({n_layouts} layouts in {layout_s:.1} s)");
        report.push(json!({ "world": common::label(arg), "settlements": t0.settlements.len(), "sites": n_layouts - t0.settlements.len(), "counts": counts, "worst": worst, "drawn_roads": drawn, "stairs": steps, "network": net, "economy": econ }));
    }
    if args.json {
        println!("{}", Value::Array(report));
    }
}

/// Street grades in towns and up (`town::stairs::survey`, the stair rule's own stations), per tier
/// and kind of street: length, length at or above some grades, grade percentiles (by length), and
/// the flights of steps (count, stepped length) with steps from 0.12, 0.15 (the default) and 0.20,
/// stopping at 80% of that. Settlements listed with their tier for `TOP`.
/// The economy pass: tier moves (with the biggest movers and their wealth's parts), the rank-size
/// curve of the towns and up (log population against log rank: Zipf's rule is a slope of -1), and
/// the roads' traffic per class (and the stretches whose class it changed).
fn economy(t0: &T0) -> Value {
    use worldgen::t0::roads::RoadClass;
    use worldgen::t0::settle::Tier;
    let Some(extra) = t0.extra.as_ref() else { return Value::Null };
    let places = &extra.economy.places;
    let sts = &t0.settlements;
    let top = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let (up, down): (Vec<_>, Vec<_>) = places.iter().filter(|p| sts[p.index].tier != p.was).partition(|p| sts[p.index].tier > p.was);
    // Rank-size: least squares of ln(population) on ln(rank).
    let mut pops: Vec<f64> = sts.iter().filter(|s| s.tier >= Tier::Town).map(|s| s.population as f64).collect();
    pops.sort_by(|a, b| b.total_cmp(a));
    let pts: Vec<(f64, f64)> = pops.iter().enumerate().map(|(r, &p)| (((r + 1) as f64).ln(), p.ln())).collect();
    let nn = pts.len().max(1) as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / nn, pts.iter().map(|p| p.1).sum::<f64>() / nn);
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    let syy: f64 = pts.iter().map(|p| (p.1 - my) * (p.1 - my)).sum();
    let slope = sxy / sxx.max(1e-9);
    let r2 = sxy * sxy / (sxx * syy).max(1e-9);
    say!("  economy: tier moves up {}, down {}; rank-size (towns and up, n {}): slope {slope:.2}, r2 {r2:.2}; populations {}", up.len(), down.len(), pts.len(), [0usize, 1, 4, 9, 19].iter().filter_map(|&r| pops.get(r).map(|p| format!("#{} {:.0}", r + 1, p))).collect::<Vec<_>>().join(", "));
    let line = |what: &str, p: &worldgen::t0::economy::Place| {
        let s = &sts[p.index];
        say!("      {what} #{} at ({:.0}, {:.0}): {:?} -> {:?}, pop {}, wealth {:.2} (food {:.2}, industry {:.2}, gateway {:.2}, trade {:.2}), {}", p.index, s.x, s.y, p.was, s.tier, s.population, p.wealth, p.food, p.industry, p.gateway, p.trade, s.reason.name());
    };
    for p in up.iter().take(top) {
        line("grew", p);
    }
    for p in down.iter().take(top) {
        line("shrank", p);
    }
    // Traffic per class as built, and the stretches whose class it changed.
    let names = ["king's road", "road", "track"];
    let tr = &extra.traffic;
    let mut parts = Vec::new();
    let mut by = Vec::new();
    for c in 0..3u8 {
        let segs: Vec<_> = tr.iter().filter(|t| t.now as u8 == c).collect();
        let len: f64 = segs.iter().map(|t| t.length_ft).sum();
        let mean = segs.iter().map(|t| t.flow as f64 * t.length_ft).sum::<f64>() / len.max(1.0);
        let mut f: Vec<f64> = segs.iter().map(|t| t.flow as f64).collect();
        f.sort_by(|a, b| a.total_cmp(b));
        parts.push(format!("{} {:.0} mi mean {:.2} p90 {:.2}", names[c as usize], len / MI, mean, pct(&f, 0.9)));
        by.push(json!({ "class": names[c as usize], "mi": len / MI, "mean_flow": mean }));
    }
    let changed = |a: RoadClass, b: RoadClass| tr.iter().filter(|t| t.was == a && t.now == b).collect::<Vec<_>>();
    let (r_k, k_r, r_t) = (changed(RoadClass::Road, RoadClass::KingsRoad), changed(RoadClass::KingsRoad, RoadClass::Road), changed(RoadClass::Road, RoadClass::Track));
    let mi_of = |v: &[&worldgen::t0::roads::SegTraffic]| v.iter().map(|t| t.length_ft).sum::<f64>() / MI;
    say!("  traffic per class (flow-weighted mean, p90 per stretch): {}; king's road threshold {:.2}", parts.join(", "), extra.economy.traffic.kings);
    say!("    class from traffic: road -> king's road {} ({:.0} mi), king's road -> road {} ({:.0} mi), road -> track {} ({:.0} mi)", r_k.len(), mi_of(&r_k), k_r.len(), mi_of(&k_r), r_t.len(), mi_of(&r_t));
    let mut big = r_k.clone();
    big.sort_by(|a, b| b.length_ft.total_cmp(&a.length_ft));
    for t in big.iter().take(top) {
        say!("      to king's road at ({:.0}, {:.0}): {:.1} mi, flow {:.2}", t.at[0], t.at[1], t.length_ft / MI, t.flow);
    }
    for t in k_r.iter().chain(&r_t).take(top) {
        say!("      dropped {:?} -> {:?} at ({:.0}, {:.0}): {:.1} mi, flow {:.2}", t.was, t.now, t.at[0], t.at[1], t.length_ft / MI, t.flow);
    }
    json!({ "up": up.len(), "down": down.len(), "rank_size_slope": slope, "rank_size_r2": r2, "traffic": by, "road_to_kings": r_k.len(), "kings_to_road": k_r.len(), "road_to_track": r_t.len() })
}

fn stairs(world: &worldgen::World, t0: &T0) -> Value {
    use worldgen::t0::settle::Tier;
    use worldgen::town::stairs::{self, Kind, StairParams};
    const GRADES: [f64; 6] = [0.10, 0.12, 0.15, 0.20, 0.25, 0.33];
    let sets: Vec<StairParams> = [0.12, 0.15, 0.20].iter().map(|&g| StairParams { g_on: g, g_off: 0.8 * g, ..StairParams::DEFAULT }).collect();
    #[derive(Default)]
    struct Row {
        len: f64,
        at: [f64; 6],
        grades: Vec<(f64, f64)>,
        flights: [usize; 3],
        stepped: [f64; 3],
        towns: [usize; 3],
    }
    let kinds = [("strip", Kind::Edge, 3u8), ("main", Kind::Edge, 4), ("alley", Kind::Alley, 3), ("approach", Kind::Approach, 4)];
    let mut rows: std::collections::BTreeMap<(u8, usize), Row> = Default::default();
    // Steepest flights at the default: (grade, middle, rise, length, settlement).
    let mut steep: Vec<(f64, P, f64, f64, usize)> = Vec::new();
    let t = std::time::Instant::now();
    for (si, st) in t0.settlements.iter().enumerate().filter(|(_, st)| st.tier >= Tier::Town) {
        let profiles = stairs::survey(world, t0, si);
        let mut any = [[false; 3]; 4];
        for p in &profiles {
            let ki = kinds.iter().position(|k| k.1 == p.kind && k.2 == p.class).unwrap_or(0);
            let row = rows.entry((st.tier as u8, ki)).or_default();
            row.len += p.len;
            for k in 1..p.s.len() {
                let ds = p.s[k] - p.s[k - 1];
                let g = (p.h[k] - p.h[k - 1]).abs() / ds.max(1e-9);
                row.grades.push((g, ds));
                for (j, &lim) in GRADES.iter().enumerate() {
                    if g >= lim {
                        row.at[j] += ds;
                    }
                }
            }
            for (j, prm) in sets.iter().enumerate() {
                let runs = stairs::runs(p, prm);
                row.flights[j] += runs.len();
                row.stepped[j] += runs.iter().map(|r| r.1 - r.0).sum::<f64>();
                any[ki][j] |= !runs.is_empty();
            }
        }
        for (ki, a) in any.iter().enumerate() {
            let row = rows.entry((st.tier as u8, ki)).or_default();
            for j in 0..3 {
                row.towns[j] += a[j] as usize;
            }
        }
        for f in stairs::flights(&profiles, &StairParams::DEFAULT).iter().filter(|f| f.pts.len() >= 2) {
            let length: f64 = f.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
            if steep.last().is_some_and(|l| l.4 == si && l.2 == f.rise as f64 && (l.3 - length).abs() < 1e-6) {
                continue;
            }
            steep.push((f.rise as f64 / length.max(1.0), lerp(f.pts[0], f.pts[f.pts.len() - 1], 0.5), f.rise as f64, length, si));
        }
    }
    say!("  steep streets (towns and up, {:.1} s): length, at grade >= {GRADES:?}; p50 p90 p99; flights / stepped ft / settlements with any, steps from 0.12 | 0.15 | 0.20", t.elapsed().as_secs_f64());
    let mut out = Vec::new();
    for ((tier, ki), row) in &mut rows {
        if row.len <= 0.0 {
            continue;
        }
        row.grades.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total: f64 = row.grades.iter().map(|g| g.1).sum();
        let pct = |q: f64| {
            let mut acc = 0.0;
            row.grades.iter().find(|g| {
                acc += g.1;
                acc >= q * total
            }).map_or(0.0, |g| g.0)
        };
        let tier_name = [Tier::Village, Tier::Town, Tier::City, Tier::Metropolis].iter().find(|t| **t as u8 == *tier).map_or("?".to_string(), |t| format!("{t:?}"));
        let at: Vec<String> = row.at.iter().map(|a| format!("{:.1}%", 100.0 * a / row.len)).collect();
        let fl: Vec<String> = (0..3).map(|j| format!("{} / {:.0} / {}", row.flights[j], row.stepped[j], row.towns[j])).collect();
        say!(
            "    {tier_name:<10} {:<8} {:>8.0} ft  {}  p {:.3} {:.3} {:.3}  {}",
            kinds[*ki].0,
            row.len,
            at.join(" "),
            pct(0.5),
            pct(0.9),
            pct(0.99),
            fl.join(" | ")
        );
        out.push(json!({ "tier": tier_name, "street": kinds[*ki].0, "length_ft": row.len.round(), "at_grade": GRADES.iter().zip(&row.at).map(|(g, a)| json!([g, a.round()])).collect::<Vec<_>>(), "p50": pct(0.5), "p90": pct(0.9), "p99": pct(0.99), "flights": row.flights, "stepped_ft": row.stepped.map(|x| x.round()), "settlements": row.towns }));
    }
    steep.sort_by(|a, b| b.0.total_cmp(&a.0));
    let top = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    for (g, at, rise, length, si) in steep.iter().take(top) {
        say!("      flight at ({:.0}, {:.0}) in settlement {si} ({:?}): rise {rise:.1} ft over {length:.0} ft (grade {g:.2})", at[0], at[1], t0.settlements[*si].tier);
    }
    Value::Array(out)
}

/// Drawn roads against the roads made from them: per road stroke, the line sampled every
/// 500 ft (on land, and outside towns, where streets take over) and each sample's distance to the nearest
/// point of a road following it; per `none` line, the roads crossing it.
fn drawn_roads(world: &worldgen::World, t0: &T0, wet: &dyn Fn(P) -> bool) -> Vec<Value> {
    const MI: f64 = 5280.0;
    let mut out = Vec::new();
    for (si, s) in world.file.sketch.strokes.iter().enumerate().filter(|(_, s)| s.tool == worldgen::world::SketchTool::Road) {
        let name = s.name.clone().unwrap_or_else(|| format!("stroke {si}"));
        let curve = |rc: &worldgen::lod::roads::RoadCurve| {
            let mut pts: Vec<P> = Vec::new();
            for k in 0..rc.pts.len() - 1 {
                let n = ((rc.s[k + 1] - rc.s[k]) / 100.0).ceil().max(1.0) as usize;
                for j in 0..n {
                    pts.push(rc.eval(k, j as f64 / n as f64, 5.0, t0.cell_ft).p);
                }
            }
            pts
        };
        if s.kind.as_deref() == Some("none") {
            let mut crossings = 0;
            for rc in &t0.roads.roads {
                let pts = curve(rc);
                for w in pts.windows(2) {
                    if s.pts.windows(2).any(|l| seg_cross(w[0], w[1], l[0], l[1])) {
                        crossings += 1;
                    }
                }
            }
            say!("  drawn {name} (no road): {crossings} road crossings");
            out.push(json!({ "stroke": si, "name": name, "none": true, "crossings": crossings }));
            continue;
        }
        // The roads following it, as points in 1-mi buckets.
        let mut grid: HashMap<(i64, i64), Vec<P>> = HashMap::new();
        let mut pieces = 0;
        for rc in t0.roads.roads.iter().filter(|rc| rc.stroke == Some(si as u32)) {
            pieces += 1;
            for p in curve(rc) {
                grid.entry(((p[0] / MI).floor() as i64, (p[1] / MI).floor() as i64)).or_default().push(p);
            }
        }
        let nearest = |p: P| {
            let (ci, cj) = ((p[0] / MI).floor() as i64, (p[1] / MI).floor() as i64);
            let mut best = f64::INFINITY;
            for r in 0..12i64 {
                for j in cj - r..=cj + r {
                    for i in ci - r..=ci + r {
                        if (i - ci).abs() != r && (j - cj).abs() != r {
                            continue;
                        }
                        for q in grid.get(&(i, j)).into_iter().flatten() {
                            best = best.min(dist(p, *q));
                        }
                    }
                }
                if best < (r as f64) * MI {
                    break;
                }
            }
            best
        };
        // (Inside a town the streets take over: roads end at its edge.)
        let in_town = |p: P| t0.settlements.iter().any(|st| dist([st.x, st.y], p) < worldgen::town::road_trim_radius(st.tier, st.population));
        let samples: Vec<P> = along(&s.pts, 500.0).into_iter().filter(|p| !in_town(*p) && !wet(*p)).collect();
        let mut off: Vec<f64> = samples.iter().map(|p| nearest(*p)).collect();
        let len: f64 = s.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        let covered = off.iter().filter(|d| **d <= MI).count() as f64 / off.len().max(1) as f64;
        off.sort_by(|a, b| a.total_cmp(b));
        let mean = off.iter().filter(|d| d.is_finite()).sum::<f64>() / off.iter().filter(|d| d.is_finite()).count().max(1) as f64;
        let p95 = off[((off.len() as f64 * 0.95) as usize).min(off.len() - 1)];
        let max = *off.last().unwrap();
        say!("  drawn {name}: {:.0} mi in {pieces} pieces, {:.1}% within a mile, offset mean {:.2} p95 {:.2} max {:.2} mi", len / MI, covered * 100.0, mean / MI, p95 / MI, max / MI);
        out.push(json!({ "stroke": si, "name": name, "length_mi": len / MI, "pieces": pieces, "within_mi": covered, "mean_mi": mean / MI, "p95_mi": p95 / MI, "max_mi": max / MI }));
    }
    out
}

fn seg_cross(a: P, b: P, c: P, d: P) -> bool {
    let side = |p: P, q: P, r: P| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0
}

const MI: f64 = 5280.0;

/// Value at fraction `q` of a sorted list.
fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() { 0.0 } else { v[((q * v.len() as f64) as usize).min(v.len() - 1)] }
}

fn dijkstra(adj: &[Vec<(usize, f64)>], src: usize) -> Vec<f64> {
    use std::cmp::Reverse;
    let mut d = vec![f64::INFINITY; adj.len()];
    let mut heap = std::collections::BinaryHeap::new();
    d[src] = 0.0;
    heap.push(Reverse((0u64, src)));
    while let Some(Reverse((k, u))) = heap.pop() {
        let du = f64::from_bits(k);
        if du > d[u] {
            continue;
        }
        for &(v, w) in &adj[u] {
            let nd = du + w;
            if nd < d[v] {
                d[v] = nd;
                heap.push(Reverse((nd.to_bits(), v)));
            }
        }
    }
    d
}

/// Points within `tol` ft of each other get one id (the first one's place stays).
struct Spots {
    tol: f64,
    pos: Vec<P>,
    grid: HashMap<(i64, i64), Vec<usize>>,
}

impl Spots {
    fn id(&mut self, p: P) -> usize {
        let (cx, cy) = ((p[0] / self.tol).floor() as i64, (p[1] / self.tol).floor() as i64);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &i in self.grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if dist(self.pos[i], p) <= self.tol {
                        return i;
                    }
                }
            }
        }
        self.pos.push(p);
        self.grid.entry((cx, cy)).or_default().push(self.pos.len() - 1);
        self.pos.len() - 1
    }
}

/// The point `reach` ft along `pts` from index `from` towards `to` (the far end if it is shorter).
fn along_from(pts: &[P], from: usize, to: usize, reach: f64) -> P {
    let step: isize = if to > from { 1 } else { -1 };
    let (mut i, mut acc) = (from as isize, 0.0);
    while i != to as isize {
        let (a, b) = (pts[i as usize], pts[(i + step) as usize]);
        let l = dist(a, b);
        if acc + l >= reach {
            return lerp(a, b, (reach - acc) / l.max(1e-9));
        }
        acc += l;
        i += step;
    }
    pts[to]
}

/// The road network as a traveller and a surveyor see it: trips between neighbouring settlements
/// against the straight line, road length against a minimum spanning tree, junctions outside
/// settlements (arms, shape, stubs, crossings of two roads), river crossings, roads running beside
/// roads, gates per town and up, settlements by tier.
/// Isolated realms: kind, tier, centre, its tracks, villages, the nearest road not its own, walls and gates.
fn realms(world: &worldgen::World, t0: &T0) {
    use worldgen::t0::settle::Tier;
    let sts = &t0.settlements;
    let d = |a: P, b: P| ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])).sqrt();
    let realms: Vec<usize> = (0..sts.len()).filter(|&i| sts[i].isolated && sts[i].tier >= Tier::Town).collect();
    say!("  realms: {}", realms.len());
    for &i in &realms {
        let s = &sts[i];
        let c = [s.x, s.y];
        let villages: Vec<usize> = (0..sts.len()).filter(|&v| sts[v].isolated && sts[v].tier == Tier::Village && d([sts[v].x, sts[v].y], c) < 12.0 * MI).collect();
        let trim = worldgen::town::road_trim_radius(s.tier, s.population) + 0.3 * MI;
        let at = |p: P| d(p, c) < trim || villages.iter().any(|&v| d(p, [sts[v].x, sts[v].y]) < 0.3 * MI);
        let rs = &t0.roads.roads;
        // Its own roads: those mostly along its routes as found (its track and its villages').
        let mine: Vec<P> = t0.extra.as_ref().map(|e| e.realm_paths.iter().filter(|p| p.0 == i).flat_map(|p| p.1.iter().copied()).collect()).unwrap_or_default();
        let own: Vec<bool> = rs.iter().map(|r| at(r.pts[0]) || at(r.pts[r.pts.len() - 1]) || 2 * r.pts.iter().filter(|&&p| mine.iter().any(|&q| d(p, q) < 1.5 * t0.cell_ft)).count() >= r.pts.len()).collect();
        let tracks: Vec<usize> = (0..rs.len()).filter(|&r| rs[r].pts.len() >= 2 && (d(rs[r].pts[0], c) < trim || d(rs[r].pts[rs[r].pts.len() - 1], c) < trim)).collect();
        let ends: Vec<String> = tracks
            .iter()
            .map(|&r| {
                let p = &rs[r].pts;
                let far = if d(p[0], c) < trim { p[p.len() - 1] } else { p[0] };
                format!("{:?} to ({:.0}, {:.0}) {:.1} mi", rs[r].class, far[0], far[1], d(far, c) / MI)
            })
            .collect();
        let (other, by) = (0..rs.len()).filter(|&r| !own[r]).flat_map(|r| rs[r].pts.iter().map(move |&p| (d(p, c), r))).fold((f64::INFINITY, 0), |a, b| if b.0 < a.0 { b } else { a });
        let by = rs.get(by).map(|r| format!("{:?} ({:.0}, {:.0})-({:.0}, {:.0})", r.class, r.pts[0][0], r.pts[0][1], r.pts[r.pts.len() - 1][0], r.pts[r.pts.len() - 1][1])).unwrap_or_default();
        let l = worldgen::town::layout(world, t0, i);
        let v: Vec<String> = villages.iter().map(|&v| format!("({:.0}, {:.0}) {:.1} mi", sts[v].x, sts[v].y, d([sts[v].x, sts[v].y], c) / MI)).collect();
        // The gap round it: from the town or any of its villages to the nearest town and up not
        // its own, and to the nearest king's road; and the roads not its own within 2 miles of
        // the walls.
        let parts: Vec<P> = std::iter::once(c).chain(villages.iter().map(|&v| [sts[v].x, sts[v].y])).collect();
        let gap = |p: P| parts.iter().map(|&q| d(p, q)).fold(f64::INFINITY, f64::min);
        let town_gap = sts.iter().filter(|o| !o.isolated && o.tier >= Tier::Town).map(|o| gap([o.x, o.y])).fold(f64::INFINITY, f64::min);
        let any_gap = sts.iter().filter(|o| !o.isolated).map(|o| gap([o.x, o.y])).fold(f64::INFINITY, f64::min);
        let king_gap = rs.iter().filter(|r| r.class == worldgen::t0::roads::RoadClass::KingsRoad).flat_map(|r| r.pts.iter().map(|&p| gap(p))).fold(f64::INFINITY, f64::min);
        let close = (0..rs.len()).filter(|&r| !own[r] && rs[r].pts.iter().any(|&p| d(p, c) < trim + 2.0 * MI)).count();
        let b = t0.sample_biome(c[0], c[1], t0.biome_warp(c[0], c[1]));
        // Its own roads in standing water, and along a river (within 600 ft of its line).
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
        let own_pts: Vec<P> = (0..rs.len()).filter(|&r| own[r]).flat_map(|r| rs[r].pts.iter().copied()).collect();
        let wet = own_pts.iter().filter(|p| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice)).count();
        let river_pts: Vec<P> = t0.rivers.rivers.iter().flat_map(|r| r.pts.iter().copied()).filter(|&p| d(p, c) < 20.0 * MI).collect();
        let along = own_pts.iter().filter(|&&p| river_pts.iter().any(|&q| d(p, q) < 600.0)).count();
        // Each village's bearing off the track's way out (its first route, 5 miles from the town).
        let out_at = t0.extra.as_ref().and_then(|e| e.realm_paths.iter().find(|p| p.0 == i)).and_then(|p| p.1.iter().find(|&&q| d(q, c) >= 5.0 * MI).copied());
        let bearings: Vec<String> = villages
            .iter()
            .map(|&v| match out_at {
                Some(o) => {
                    let (a, b) = ([o[0] - c[0], o[1] - c[1]], [sts[v].x - c[0], sts[v].y - c[1]]);
                    format!("{:.0}", ((a[0] * b[0] + a[1] * b[1]) / (d(o, c) * d([sts[v].x, sts[v].y], c))).clamp(-1.0, 1.0).acos().to_degrees())
                }
                None => "-".into(),
            })
            .collect();
        say!("      own road points {}: in water {wet}, along a river {along}; villages off the way out (deg) [{}]", own_pts.len(), bearings.join(" "));
        say!(
            "    realm #{i} {:?} {} at ({:.0}, {:.0}), {:.0} ft up, biome {}: gap to town+ {:.1} mi, any {:.1} mi, king's road {:.1} mi, other roads within 2 mi {close}",
            s.tier,
            s.kind.name(),
            c[0],
            c[1],
            t0.sample(c[0], c[1], 0.0),
            b[0],
            town_gap / MI,
            any_gap / MI,
            king_gap / MI
        );
        say!(
            "      pop {}: {} track(s) [{}]; nearest other road {:.1} mi ({by}); walls {} gates {}; villages {} [{}]",
            s.population,
            tracks.len(),
            ends.join("; "),
            other / MI,
            l.walls.len(),
            l.gates.len(),
            villages.len(),
            v.join(", ")
        );
    }
}

fn network(t0: &T0, standing: &dyn Fn(P) -> bool, gates: &[(worldgen::t0::settle::Tier, usize)]) -> Value {
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
    // Dead ends: a road end at no settlement and no other road (those within 0.3 mi of a river
    // crossing counted apart: a road may end at a ferry or ford).
    let dead: Vec<usize> = (0..nroad).filter(|&n| deg[n] == 1 && !attached(spots.pos[n])).collect();
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
    for s in spike_wet.iter().take(top) {
        say!("      road {} turns {:.0}° at ({:.0}, {:.0})", s.2, s.1, s.0[0], s.0[1]);
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
        for ri in 0..=12u8 {
            let r = Reason::from_u8(ri);
            let c: Vec<usize> = tiers.iter().map(|t| sts.iter().filter(|s| s.reason == r && s.tier == *t).count()).collect();
            if c.iter().sum::<usize>() > 0 {
                say!("    {:<20} {:>4} {:>4} {:>4} {:>4}", r.name(), c[0], c[1], c[2], c[3]);
            }
        }
    }
    json!({
        "trips": trips_all, "trips_dry": trips_dry,
        "length_mi": { "total": total / MI, "king": by_class[0] / MI, "road": by_class[1] / MI, "track": by_class[2] / MI, "mst": mst / MI, "ratio": total / mst.max(1.0) },
        "junctions": { "n": njunc, "arms": arms_n, "shape_t_cross": shape_n[0], "shape_y": shape_n[1], "shape_other": shape_n[2], "stubs_half_mi": stub_half, "stubs_half_mi_shared": stub_doubled, "stubs_2_mi": stub_two, "x_crossings": x_cross, "near_crossing": near_cross, "out_and_backs": spikes.len(), "out_and_backs_wet": spike_wet.len() },
        "crossings": { "n": crossings.len(), "bridges": kind_n[0], "fords": kind_n[1], "ferries": kind_n[2], "under_45": under(0.707), "under_30": under(0.5), "within_1_mi": within(MI), "within_3_mi": within(3.0 * MI) },
        "beside_pct": beside_pct, "gates": gate_json, "gates_mean": gate_mean, "tiers": tier_n,
    })
}
