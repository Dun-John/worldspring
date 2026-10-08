//! QA: how roads, rivers and settlements meet. Counts what should never happen, with the
//! worst places (world ft) to go and look:
//! - walls, towers, buildings and fields over a river channel or standing water;
//! - town streets and approaches over the river with no bridge under them;
//! - approaches (road end to gate) running through built blocks, or turning sharply;
//! - sharp turns on the network roads;
//! - drawn roads (a sketch's road strokes): how closely the road follows each line drawn (share
//!   of the line within a mile of it, mean, 95th percentile and worst offset), and planned roads
//!   crossing a `none` line.
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
        let mut pier_x = Tally::default();
        for i in 0..n_layouts {
            let l = worldgen::town::layout(&world, &t0, i);
            let name = if i < t0.settlements.len() { format!("{:?} {i}", l.tier) } else { format!("site {i}") };
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
        let drawn = drawn_roads(&world, &t0, &standing);
        // `DUMP=file.json`: every road (class, drawn stroke, control points) and settlement, for
        // drawing over a preview.
        if let Ok(path) = std::env::var("DUMP") {
            let roads: Vec<Value> = t0.roads.roads.iter().map(|rc| json!({ "class": rc.class as u8, "stroke": rc.stroke, "pts": rc.pts.iter().map(|p| [p[0].round(), p[1].round()]).collect::<Vec<_>>() })).collect();
            let towns: Vec<Value> = t0.settlements.iter().map(|st| json!({ "tier": st.tier as u8, "x": st.x.round(), "y": st.y.round() })).collect();
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
        report.push(json!({ "world": common::label(arg), "settlements": t0.settlements.len(), "sites": n_layouts - t0.settlements.len(), "counts": counts, "worst": worst, "drawn_roads": drawn }));
    }
    if args.json {
        println!("{}", Value::Array(report));
    }
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
