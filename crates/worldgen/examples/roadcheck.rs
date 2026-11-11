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
        ] {
            tally.print(label, key, &mut counts, &mut worst);
        }
        say!("  ({n_layouts} layouts in {layout_s:.1} s)");
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
        // (`WET=1`: where.)
        if std::env::var("WET").is_ok() {
            for p in own_pts.iter().filter(|p| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice)) {
                say!("        wet ({:.0}, {:.0}) water {:.0} ground {:.0}", p[0], p[1], t0.sample_water(p[0], p[1]), t0.ground_at(p[0], p[1], lattice));
            }
        }
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
        // Its track (the route from the town) and its villages' spurs: the nearest road not its own
        // to the track but its outer end (20 miles round where it joins), the spurs, and those
        // joining anything but the track (cross-links).
        let rp: Vec<&(usize, Vec<P>)> = t0.extra.as_ref().map(|e| e.realm_paths.iter().filter(|p| p.0 == i).collect()).unwrap_or_default();
        let main = rp.iter().find(|p| p.1.first().is_some_and(|&q| d(q, c) < t0.cell_ft)).map(|p| p.1.clone()).unwrap_or_default();
        let spurs: Vec<&Vec<P>> = rp.iter().filter(|p| p.1.first().is_some_and(|&q| d(q, c) >= t0.cell_ft)).map(|p| &p.1).collect();
        let outer = main.last().copied().unwrap_or(c);
        let far: Vec<P> = main.iter().copied().filter(|&q| d(q, outer) > 20.0 * MI).collect();
        let others: Vec<P> = (0..rs.len()).filter(|&r| !own[r]).flat_map(|r| rs[r].pts.iter().copied()).collect();
        let to_track = far.iter().flat_map(|&q| others.iter().map(move |&o| d(o, q))).fold(f64::INFINITY, f64::min);
        let on = |q: P, path: &[P]| path.iter().any(|&m| d(m, q) < 1.5 * t0.cell_ft);
        let cross = spurs
            .iter()
            .enumerate()
            .filter(|&(si, sp)| {
                let end = sp[sp.len() - 1];
                let body = &sp[..sp.len().saturating_sub(2)];
                !on(end, &main) || spurs.iter().enumerate().any(|(sj, o)| sj != si && body.iter().any(|&q| o.iter().any(|&m| d(m, q) < 0.5 * t0.cell_ft)))
            })
            .count();
        say!(
            "      clearance: town+ {:.1} mi, king's road {:.1} mi, other road to the track (but its end) {:.1} mi; track {:.1} mi to ({:.0}, {:.0}); spurs {}, cross-links {cross}",
            town_gap / MI,
            king_gap / MI,
            to_track / MI,
            d(outer, c) / MI,
            outer[0],
            outer[1],
            spurs.len()
        );
        // Each spur: length, straightness, its join's place along the track, its angle there, and the
        // distance (along the track) to the nearest bend of the track over 45 degrees.
        {
            let mut cum = vec![0.0f64];
            for q in main.windows(2) {
                cum.push(cum[cum.len() - 1] + d(q[0], q[1]));
            }
            let at = |s: f64| -> P {
                let k = cum.partition_point(|&v| v < s).clamp(1, main.len().max(2) - 1);
                let (a, b) = (main[k - 1], main[k]);
                let t = if cum[k] > cum[k - 1] { ((s - cum[k - 1]) / (cum[k] - cum[k - 1])).clamp(0.0, 1.0) } else { 0.0 };
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            };
            let total = cum.last().copied().unwrap_or(0.0);
            let turn = |s: f64| -> f64 {
                let (a, b, c2) = (at((s - MI).max(0.0)), at(s), at((s + MI).min(total)));
                let (h1, h2) = ((b[1] - a[1]).atan2(b[0] - a[0]), (c2[1] - b[1]).atan2(c2[0] - b[0]));
                let mut t = (h2 - h1).abs().to_degrees();
                if t > 180.0 {
                    t = 360.0 - t;
                }
                t
            };
            let mut bends: Vec<f64> = Vec::new();
            let mut s = MI;
            while s < total.min(60.0 * MI) {
                let t = turn(s);
                if t > 45.0 && turn(s - 0.25 * MI) <= t && turn(s + 0.25 * MI) < t {
                    bends.push(s);
                }
                s += 0.25 * MI;
            }
            let mut rows: Vec<(f64, String)> = Vec::new();
            for sp in &spurs {
                let (a, e) = (sp[0], sp[sp.len() - 1]);
                let len: f64 = sp.windows(2).map(|q| d(q[0], q[1])).sum();
                let k = (0..main.len()).min_by(|&x, &y| d(main[x], e).total_cmp(&d(main[y], e))).unwrap_or(0);
                let js = cum[k];
                let nb = bends.iter().map(|&b| (b - js).abs()).fold(f64::INFINITY, f64::min);
                let (p0, p1) = (sp[sp.len().saturating_sub(4)], e);
                let (m0, m1) = (main[k.saturating_sub(3)], main[(k + 3).min(main.len() - 1)]);
                let (h1, h2) = ((p1[1] - p0[1]).atan2(p1[0] - p0[0]), (m1[1] - m0[1]).atan2(m1[0] - m0[0]));
                let mut ang = (h1 - h2).abs().to_degrees() % 180.0;
                if ang > 90.0 {
                    ang = 180.0 - ang;
                }
                rows.push((js, format!("spur from ({:.0}, {:.0}) len {:.1} mi straight-dist {:.1} mi ratio {:.2}; join at {:.1} mi along track, angle {:.0} deg, nearest bend {} mi", a[0], a[1], len / MI, d(a, e) / MI, len / d(a, e).max(1.0), js / MI, ang, if nb.is_finite() { format!("{:.1}", nb / MI) } else { "none".into() })));
            }
            rows.sort_by(|x, y| x.0.total_cmp(&y.0));
            let gaps: Vec<String> = rows.windows(2).map(|r| format!("{:.1}", (r[1].0 - r[0].0) / MI)).collect();
            say!("      cell {:.0} ft", t0.cell_ft);
            say!("      SPURS bends(>45deg, first 60 mi) at {:?} mi; join gaps {:?} mi", bends.iter().map(|b| (b / MI * 10.0).round() / 10.0).collect::<Vec<_>>(), gaps);
            for r in &rows {
                say!("      SPUR {}", r.1);
            }
            // Each spur as built: its road from the village, length against the chord and the furthest it strays from it.
            for sp in &spurs {
                let a = sp[0];
                let built = t0.roads.roads.iter().filter(|r| r.pts.len() >= 2 && (d(r.pts[0], a) < 0.3 * t0.cell_ft || d(r.pts[r.pts.len() - 1], a) < 0.3 * t0.cell_ft)).max_by(|x, y| x.pts.len().cmp(&y.pts.len()));
                if let Some(r) = built {
                    let pts: Vec<P> = if d(r.pts[0], a) < 0.3 * t0.cell_ft { r.pts.clone() } else { r.pts.iter().rev().copied().collect() };
                    let (p0, p1) = (pts[0], pts[pts.len() - 1]);
                    let len: f64 = pts.windows(2).map(|q| d(q[0], q[1])).sum();
                    let ch = d(p0, p1).max(1.0);
                    let dev = pts.iter().map(|q| ((p1[0] - p0[0]) * (p0[1] - q[1]) - (p0[0] - q[0]) * (p1[1] - p0[1])).abs() / ch).fold(0.0f64, f64::max);
                    say!("      SPUR-BUILT from ({:.0}, {:.0}) len {:.1} mi chord {:.1} mi ratio {:.3} max stray {:.2} mi", a[0], a[1], len / MI, ch / MI, len / ch, dev / MI);
                }
            }
        }
    }
}

/// The share on water per tier (realms left out) and the count per reason.
/// Islands per size band: how many, how many settled, their settlements by tier; the share of
/// all settlements on islands; road points out at open sea (the cell and all round it sea);
/// island settlements with no road while others share their island, and islands whose
/// settlements the roads split into more than one group.
fn islands(world: &worldgen::World, t0: &T0) {
    use worldgen::t0::biome::Biome;
    use worldgen::t0::islands;
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let b = |k: usize| Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
    let land: Vec<bool> = (0..w * h).map(|k| b(k) != Biome::Ocean).collect();
    let lake: Vec<u32> = (0..w * h).map(|k| if b(k) == Biome::Lake { 0 } else { worldgen::t0::hydro::NO_LAKE }).collect();
    let isl = islands::label(w, h, cell, &land, &lake, &t0.biome.data);
    let bands = [(0.0, 10.0, "<10"), (10.0, 60.0, "10-60"), (60.0, 400.0, "60-400"), (400.0, 1500.0, "400-1500"), (1500.0, f64::INFINITY, ">1500")];
    let sts = &t0.settlements;
    let on: Vec<Option<u32>> = sts.iter().map(|s| isl.island_at(s.cell)).collect();
    let mut lines = Vec::new();
    for (lo, hi, name) in bands {
        let ids: Vec<u32> = (0..isl.parts.len() as u32).filter(|&c| c != isl.mainland && isl.parts[c as usize].area_mi2 >= lo && isl.parts[c as usize].area_mi2 < hi).collect();
        let settled = ids.iter().filter(|&&c| on.contains(&Some(c))).count();
        let mut tiers = [0usize; 4];
        for (i, s) in sts.iter().enumerate() {
            if on[i].is_some_and(|c| ids.contains(&c)) {
                tiers[s.tier as usize] += 1;
            }
        }
        lines.push(format!("{name}: {} islands, {settled} settled, v/t/c/m {}/{}/{}/{}", ids.len(), tiers[0], tiers[1], tiers[2], tiers[3]));
    }
    let n_on = on.iter().filter(|c| c.is_some()).count();
    // (`AT=x,y`: the land mass there.)
    if let Some(at) = std::env::var("AT").ok().and_then(|v| v.split_once(',').and_then(|(x, y)| Some([x.parse::<f64>().ok()?, y.parse::<f64>().ok()?]))) {
        let k = (at[1] / cell).round() as usize * w + (at[0] / cell).round() as usize;
        let c = isl.of[k];
        say!("  at ({:.0}, {:.0}): land mass {c}{}", at[0], at[1], if c == isl.mainland { " (mainland)".to_string() } else if c == islands::NONE { " (sea)".to_string() } else { format!(" ({:.0} sq mi)", isl.parts[c as usize].area_mi2) });
    }
    // (Those of 10 sq mi or more left empty: why.)
    let seed = world.stream("t0.islands");
    let mut empty = Vec::new();
    for c in 0..isl.parts.len() as u32 {
        let p = &isl.parts[c as usize];
        if c != isl.mainland && p.area_mi2 >= 10.0 && !on.contains(&Some(c)) {
            let o = isl.owed(c, seed, (0.0, 0.0));
            empty.push(format!("{:.0} sq mi {:.0}% barren owed {} at ({:.0}, {:.0})", p.area_mi2, 100.0 * p.barren, o.villages, (p.first % w) as f64 * cell, (p.first / w) as f64 * cell));
        }
    }
    if !empty.is_empty() {
        say!("  islands left empty (10 sq mi and up): {}", empty.join("; "));
    }
    let shore: Vec<&worldgen::t0::settle::Settlement> = sts.iter().zip(&on).filter(|(s, c)| c.is_some() && s.coastal && s.tier == worldgen::t0::settle::Tier::Village).map(|(s, _)| s).collect();
    let fishing = shore.iter().filter(|s| s.kind == worldgen::t0::settle::SettleKind::Fishing).count();
    say!("  islands: {}; on islands {n_on}/{} settlements ({:.1}%), shore villages fishing {fishing}/{}", lines.join("; "), sts.len(), 100.0 * n_on as f64 / sts.len().max(1) as f64, shore.len());
    // Roads: out at open sea, and on the islands.
    let cell_of = |p: P| {
        let (i, j) = ((p[0] / cell).round() as i64, (p[1] / cell).round() as i64);
        (i >= 0 && j >= 0 && i < w as i64 && j < h as i64).then(|| j as usize * w + i as usize)
    };
    let open_sea = |k: usize| !land[k] && worldgen::t0::flood::neighbors(w, h, k).all(|(nb, _)| !land[nb]);
    let (mut sea_pts, mut sea_roads) = (0usize, 0usize);
    let mut island_roads: Vec<(usize, Vec<P>)> = Vec::new();
    for (ri, r) in t0.roads.roads.iter().enumerate() {
        let pts = along(&r.pts, 0.25 * cell);
        let wet = pts.iter().filter(|&&p| cell_of(p).is_some_and(open_sea)).count();
        sea_pts += wet;
        sea_roads += (wet > 0) as usize;
        if pts.iter().any(|&p| cell_of(p).is_some_and(|k| isl.island_at(k).is_some())) {
            island_roads.push((ri, pts));
        }
    }
    // (Union-find over the island settlements and roads: a settlement on a road within a mile of
    // it; two roads joined where one's end lies within 300 ft of the other.)
    let ns = sts.len();
    let mut parent: Vec<usize> = (0..ns + island_roads.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let near = |a: &[P], q: P, r: f64| a.iter().any(|p| dist(*p, q) <= r);
    let mut roadless = 0;
    for (i, s) in sts.iter().enumerate() {
        let Some(c) = on[i] else { continue };
        let mut any = false;
        for (j, (_, pts)) in island_roads.iter().enumerate() {
            if near(pts, [s.x, s.y], 5280.0) {
                let (a, b) = (find(&mut parent, i), find(&mut parent, ns + j));
                parent[a] = b;
                any = true;
            }
        }
        if !any && on.iter().filter(|&&o| o == Some(c)).count() > 1 {
            roadless += 1;
            let nearest = sts.iter().enumerate().filter(|&(j, _)| j != i && on[j] == Some(c)).map(|(_, o)| dist([o.x, o.y], [s.x, s.y]) / 5280.0).fold(f64::INFINITY, f64::min);
            say!("      no road: {} at ({:.0}, {:.0}){}, nearest on its island {nearest:.1} mi", s.tier.name(), s.x, s.y, if s.isolated { " (realm)" } else { "" });
        }
    }
    for a in 0..island_roads.len() {
        for b in 0..island_roads.len() {
            let (pa, pb) = (&island_roads[a].1, &island_roads[b].1);
            if a != b && !pa.is_empty() && (near(pb, pa[0], 300.0) || near(pb, pa[pa.len() - 1], 300.0)) {
                let (x, y) = (find(&mut parent, a + ns), find(&mut parent, b + ns));
                parent[x] = y;
            }
        }
    }
    let mut split = 0;
    for c in 0..isl.parts.len() as u32 {
        let mut groups: Vec<usize> = (0..ns).filter(|&i| on[i] == Some(c)).map(|i| find(&mut parent, i)).collect();
        groups.sort();
        groups.dedup();
        if groups.len() > 1 {
            split += 1;
            let members: Vec<String> = (0..ns).filter(|&i| on[i] == Some(c)).map(|i| format!("{}{}", find(&mut parent, i) % 1000, &sts[i].tier.name()[..1])).collect();
            say!("      split: {:.0} sq mi island, {} groups: {}", isl.parts[c as usize].area_mi2, groups.len(), members.join(" "));
        }
    }
    say!("  island roads: {} roads on islands; road points at open sea {sea_pts} on {sea_roads} roads; island settlements with no road (others on the island) {roadless}; islands split by roads {split}", island_roads.len());
}

/// The sea ferries (`t0::ferries`): how many, their water (miles), the settled islands they serve
/// against those with bigger land with roads within `WATER_MAX_MI` (cell centres less two cells);
/// with `FERRY_MAP=1`, the battlemap chunks out from each of the first few landings: their ferry
/// shapes, decks and squares of road bed on the water (`FERRY_PROFILE=1`: each line's water at L10).
fn sea_ferries(world: &worldgen::world::World, t0: &T0) {
    use worldgen::t0::biome::Biome;
    use worldgen::t0::{ferries, islands};
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let b = |k: usize| Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
    let land: Vec<bool> = (0..w * h).map(|k| b(k) != Biome::Ocean).collect();
    let lake: Vec<u32> = (0..w * h).map(|k| if b(k) == Biome::Lake { 0 } else { worldgen::t0::hydro::NO_LAKE }).collect();
    let isl = islands::label(w, h, cell, &land, &lake, &t0.biome.data);
    let mut settled = vec![false; isl.parts.len()];
    for s in t0.settlements.iter().filter(|s| !s.isolated) {
        if isl.of[s.cell] != islands::NONE {
            settled[isl.of[s.cell] as usize] = true;
        }
    }
    let at_cell = |p: P| (((p[1] / cell).round() as usize).min(h - 1)) * w + ((p[0] / cell).round() as usize).min(w - 1);
    let ferried: std::collections::BTreeSet<u32> = t0.ferries.iter().map(|f| isl.of[at_cell(f.a)]).collect();
    let reach = (ferries::WATER_MAX_MI * 5280.0 / cell + 2.2).ceil() as i64;
    let mut within = 0;
    for (m, part) in isl.parts.iter().enumerate() {
        if m as u32 == isl.mainland || !settled[m] {
            continue;
        }
        let near = (0..w * h).filter(|&k| isl.of[k] == m as u32).any(|k| {
            let (x, y) = ((k % w) as i64, (k / w) as i64);
            (-reach..=reach).any(|dy| (-reach..=reach).any(|dx| {
                let (i, j) = (x + dx, y + dy);
                if i < 0 || j < 0 || i >= w as i64 || j >= h as i64 || ((dx * dx + dy * dy) as f64).sqrt() - 2.2 > ferries::WATER_MAX_MI * 5280.0 / cell {
                    return false;
                }
                let o = isl.of[j as usize * w + i as usize];
                o != islands::NONE && o != m as u32 && settled[o as usize] && isl.parts[o as usize].area_mi2 > part.area_mi2
            }))
        });
        within += near as usize;
    }
    let mut mi: Vec<f64> = t0.ferries.iter().map(|f| f.mi()).collect();
    mi.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| mi.get(((mi.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(0.0);
    say!("  ferries: {}, water min/median/max {:.1}/{:.1}/{:.1} mi; islands ferried {} / settled islands by bigger land with roads (cell centres within the reach + 2.2 cells) {within}", t0.ferries.len(), q(0.0), q(0.5), q(1.0), ferried.len());
    let fer = worldgen::t0::ferries::overlay_entries(t0);
    if !fer.is_empty() {
        let o = t0.overlay_json();
        say!("      overlay ferries (c 3): {} in sea_lanes, {}", o.matches("{\"c\":3,").count(), fer.split("],\"p\"").map(|v| v.rsplit("\"e\":[").next().unwrap_or("")).filter(|v| !v.is_empty() && !v.contains(']')).map(|v| format!("e [{v}]")).collect::<Vec<_>>().join(", "));
    }
    for f in &t0.ferries {
        say!("      ferry ({:.0}, {:.0}) -> ({:.0}, {:.0}), {:.2} mi", f.a[0], f.a[1], f.b[0], f.b[1], f.mi());
    }
    // Each landing's track (the road ending at it): real miles, climb and range of its bed (ft).
    for f in &t0.ferries {
        let t = std::time::Instant::now();
        let sh = f.shores(world, t0);
        say!("        stages ({:.0}, {:.0}) {:.0} ft out, ({:.0}, {:.0}) {:.0} ft out ({} ms)", sh[0][0], sh[0][1], dist(sh[0], f.a), sh[1][0], sh[1][1], dist(sh[1], f.b), t.elapsed().as_millis());
        for p in [f.a, f.b] {
            let near = |q: &P| dist(*q, p) < ferries::INLAND_FT + 30.0;
            let Some(r) = t0.roads.roads.iter().find(|r| r.pts.first().is_some_and(near) || r.pts.last().is_some_and(near)) else {
                say!("        landing ({:.0}, {:.0}): no track ends here", p[0], p[1]);
                continue;
            };
            let len: f64 = r.pts.windows(2).map(|s| dist(s[0], s[1])).sum();
            let climb: f32 = r.z.windows(2).map(|z| (z[1] - z[0]).abs()).sum();
            let (lo, hi) = r.z.iter().fold((f32::MAX, f32::MIN), |(a, b), &z| (a.min(z), b.max(z)));
            say!("        landing ({:.0}, {:.0}): track {:.1} mi, climb {:.0} ft, bed {:.0}..{:.0} ft", p[0], p[1], len / 5280.0, climb, lo, hi);
        }
    }
    if !std::env::var("FERRY_MAP").is_ok_and(|v| v == "1") {
        return;
    }
    let g = &world.geom;
    if std::env::var("FERRY_PROBE").is_ok() {
        // Out from each landing every 25 ft: the plan's ground and the finest tile's, less the water.
        use worldgen::core::tile::{HALO, PADDED, TileKey};
        let level = g.max_level;
        let (s, size) = (g.spacing_ft(level), g.tile_size_ft(level));
        let lattice = g.spacing_ft(g.first_refine_level.saturating_sub(1));
        let mut cache: std::collections::BTreeMap<(u32, u32), Vec<f32>> = Default::default();
        for f in &t0.ferries {
            for (p, q) in [(f.a, f.b), (f.b, f.a)] {
                let l = dist(p, q);
                let mut row = String::new();
                for k in 0..=48 {
                    let t = k as f64 * 25.0 - 100.0;
                    let (x, y) = (p[0] + (q[0] - p[0]) * t / l, p[1] + (q[1] - p[1]) * t / l);
                    let (tx, ty) = ((x / size) as u32, (y / size) as u32);
                    let pad = cache.entry((tx, ty)).or_insert_with(|| {
                        let mut parent: Option<Vec<f32>> = None;
                        for lv in 0..=level {
                            let pk = TileKey::surface(lv, tx >> (level - lv), ty >> (level - lv));
                            parent = Some(worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref()).padded.to_vec());
                        }
                        parent.unwrap()
                    });
                    let (ox, oy) = (tx as f64 * size, ty as f64 * size);
                    let (i, j) = (((x - ox) / s).round() as usize + HALO, ((y - oy) / s).round() as usize + HALO);
                    let wv = t0.sample_water(x, y) as f64;
                    let gp = t0.rivers.valley(t0.ground_at(x, y, lattice), x, y);
                    row += &format!(" {:.0}:{:.1}/{:.1}", t, gp - wv.max(-1e3), pad[j * PADDED + i] as f64 - wv.max(-1e3));
                }
                say!("        probe from ({:.0}, {:.0}) t:plan-water/tile-water:{row}", p[0], p[1]);
            }
        }
    }
    if std::env::var("FERRY_PROFILE").is_ok() {
        use worldgen::core::tile::{HALO, PADDED, TileKey};
        let level = 10u8;
        let s = g.spacing_ft(level);
        let size = g.tile_size_ft(level);
        let sea = world.params().sea_level_ft;
        let mut cache: std::collections::BTreeMap<(u32, u32), Vec<f32>> = Default::default();
        for f in &t0.ferries {
            let l = dist(f.a, f.b);
            let row: String = (0..=(l / 100.0) as usize).map(|k| {
                let t = k as f64 * 100.0;
                let (x, y) = (f.a[0] + (f.b[0] - f.a[0]) * t / l, f.a[1] + (f.b[1] - f.a[1]) * t / l);
                let (tx, ty) = ((x / size) as u32, (y / size) as u32);
                let pad = cache.entry((tx, ty)).or_insert_with(|| {
                    let mut parent: Option<Vec<f32>> = None;
                    for lv in 0..=level {
                        let pk = TileKey::surface(lv, tx >> (level - lv), ty >> (level - lv));
                        parent = Some(worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref()).padded.to_vec());
                    }
                    parent.unwrap()
                });
                let (ox, oy) = (tx as f64 * size, ty as f64 * size);
                let (i, j) = (((x - ox) / s).round() as usize + HALO, ((y - oy) / s).round() as usize + HALO);
                let z = pad[j * PADDED + i];
                if (z as f64) < sea as f64 { '~' } else { '#' }
            }).collect();
            say!("        line L10 every 100 ft: {row}");
        }
    }
    for f in t0.ferries.iter().take(3) {
        // (Each chunk along the line out to where its stage may stand.)
        let level = g.max_level;
        let size = g.tile_size_ft(level);
        let mut keys: Vec<(P, worldgen::core::tile::TileKey)> = Vec::new();
        for (p, q) in [(f.a, f.b), (f.b, f.a)] {
            let l = dist(p, q);
            for m in 0..=13 {
                let t = m as f64 * 100.0 - 60.0;
                let c = [p[0] + (q[0] - p[0]) * t / l, p[1] + (q[1] - p[1]) * t / l];
                let key = worldgen::core::tile::TileKey::surface(level, (c[0] / size) as u32, (c[1] / size) as u32);
                if !keys.iter().any(|k| k.1 == key) {
                    keys.push((p, key));
                }
            }
        }
        for (p, key) in keys {
            let mut parent: Option<Vec<f32>> = None;
            let mut tile = None;
            for l in 0..=level {
                let pk = worldgen::core::tile::TileKey::surface(l, key.x >> (level - l), key.y >> (level - l));
                let t = worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref());
                parent = Some(t.padded.clone());
                tile = Some(t);
            }
            let tile = tile.expect("a tile");
            let chunk = worldgen::battlemap::generate(world, t0, &key, &tile);
            let shapes = chunk.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Ferry).count();
            let hs = (chunk.road_halo.len() as f64).sqrt() as usize;
            let (mut bed, mut deck, mut wet) = (0, 0, 0);
            for i in 0..hs * hs {
                if chunk.halo_water[i] > chunk.halo_height[i] - 0.01 || chunk.road_halo[i] & 0x80 != 0 {
                    wet += 1;
                    if chunk.road_halo[i] & 0x80 != 0 {
                        deck += 1;
                    } else if chunk.road_halo[i] != 0 {
                        bed += 1;
                    }
                }
            }
            let roads = chunk.road_halo.iter().filter(|&&r| r != 0 && r & 0x80 == 0).count();
            say!("      landing ({:.0}, {:.0}) chunk L{level} ({}, {}): ferry shapes {shapes}, water squares {wet}, decks {deck}, road bed on water {bed}, road squares {roads}", p[0], p[1], key.x, key.y);
        }
    }
}

/// The sea lanes: segments, miles (all and per class: busy, normal, quiet), the routed links'
/// lengths, the share of link length within 1 mi of land (outside the last 3 mi into each port),
/// lane points on land (but the ports' own), island ports linked, and island vs mainland ports'
/// trade.
/// `SHORE_PROBE=x,y;x,y`: per place, the road end nearest it and along that road's first 1,000
/// ft (every 100 ft) its bed's height, the finest tile's ground on the line, and the tile's ground
/// 250 ft to each side (past the bed's side slopes), less sea level (ft).
fn shore_probe(world: &worldgen::world::World, t0: &T0, spec: &str) {
    use worldgen::core::tile::{HALO, PADDED, TileKey};
    let g = &world.geom;
    let level = g.max_level;
    let (s, size) = (g.spacing_ft(level), g.tile_size_ft(level));
    let sea = world.params().sea_level_ft;
    let mut cache: std::collections::BTreeMap<(u32, u32), Vec<f32>> = Default::default();
    let mut tile = |x: f64, y: f64| -> f64 {
        let (tx, ty) = ((x / size) as u32, (y / size) as u32);
        let pad = cache.entry((tx, ty)).or_insert_with(|| {
            let mut parent: Option<Vec<f32>> = None;
            for lv in 0..=level {
                let pk = TileKey::surface(lv, tx >> (level - lv), ty >> (level - lv));
                parent = Some(worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref()).padded.to_vec());
            }
            parent.unwrap()
        });
        let (i, j) = (((x - tx as f64 * size) / s).round() as usize + HALO, ((y - ty as f64 * size) / s).round() as usize + HALO);
        pad[j * PADDED + i] as f64 - sea
    };
    if let Some(c) = std::env::var("SHORE_GRID").ok().map(|v| v.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).collect::<Vec<f64>>()).filter(|v| v.len() == 3) {
        // (A grid of the finest ground, ft above sea, `c[2]` ft apart, 25 x 25 round (c[0], c[1]).)
        for j in -12..=12 {
            let row: Vec<String> = (-12..=12).map(|i| format!("{:4.0}", tile(c[0] + i as f64 * c[2], c[1] + j as f64 * c[2]))).collect();
            say!("    grid {:+3}: {}", j, row.join(""));
        }
        let lattice = g.spacing_ft(g.first_refine_level.saturating_sub(1));
        for j in [-6, -4, -2] {
            let (x, y) = (c[0] + (-(j + 10) as f64 / 2.0 + 0.0) * c[2], c[1] + j as f64 * c[2]);
            let gr = t0.ground_at(x, y, lattice);
            say!("    at ({x:.0}, {y:.0}): ground_at {:.1}, valley {:.1}, water {:.1}, lake {:?}, tile {:.1} (less sea)", gr - sea, t0.rivers.valley(gr, x, y) - sea, t0.sample_water(x, y) as f64 - sea, t0.sample_lake(x, y), tile(x, y));
        }
    }
    for p in spec.split(';').filter_map(|v| { let v: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect(); (v.len() == 2).then(|| [v[0], v[1]]) }) {
        let Some((r, first)) = t0.roads.roads.iter().flat_map(|r| [(r, true), (r, false)]).filter(|(r, _)| r.pts.len() >= 2).min_by(|a, b| {
            let e = |(r, f): &(&worldgen::lod::roads::RoadCurve, bool)| dist(if *f { r.pts[0] } else { r.pts[r.pts.len() - 1] }, p);
            e(a).total_cmp(&e(b))
        }) else { continue };
        // (The drawn curve, every ~20 ft, as the tiles carve it.)
        let (mut pts, mut z): (Vec<P>, Vec<f32>) = (Vec::new(), Vec::new());
        for k in 0..r.pts.len() - 1 {
            let n = ((r.s[k + 1] - r.s[k]) / 20.0).ceil().max(1.0) as usize;
            for j in 0..n {
                let e = r.eval(k, j as f64 / n as f64, s, t0.cell_ft);
                pts.push(e.p);
                z.push(e.z as f32);
            }
        }
        let e = r.eval(r.pts.len() - 2, 1.0, s, t0.cell_ft);
        pts.push(e.p);
        z.push(e.z as f32);
        if !first {
            pts.reverse();
            z.reverse();
        }
        let mut row = String::new();
        let (mut at, mut k, mut worst) = (0.0, 0usize, f64::MIN);
        let far = std::env::var("SHORE_FT").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(1000.0);
        for m in 0..=(far / 100.0) as usize {
            let t = m as f64 * 100.0;
            while k + 2 < pts.len() && at + dist(pts[k], pts[k + 1]) < t {
                at += dist(pts[k], pts[k + 1]);
                k += 1;
            }
            let l = dist(pts[k], pts[k + 1]).max(1e-9);
            let f = ((t - at) / l).clamp(0.0, 1.0);
            let q = [pts[k][0] + (pts[k + 1][0] - pts[k][0]) * f, pts[k][1] + (pts[k + 1][1] - pts[k][1]) * f];
            let bed = z[k] as f64 + (z[k + 1] - z[k]) as f64 * f - sea;
            let n = [-(pts[k + 1][1] - pts[k][1]) / l, (pts[k + 1][0] - pts[k][0]) / l];
            let side = 0.5 * (tile(q[0] + n[0] * 250.0, q[1] + n[1] * 250.0) + tile(q[0] - n[0] * 250.0, q[1] - n[1] * 250.0));
            let on = tile(q[0], q[1]);
            worst = worst.max(side - on);
            let lattice = g.spacing_ft(g.first_refine_level.saturating_sub(1));
            let (lv, mask) = t0.sample_lake(q[0], q[1]);
            let base = T0::shore_bank(t0.ground_at(q[0], q[1], lattice), lv, mask) - sea;
            row += &format!(" {t:.0}:{bed:.1}/{on:.1}/{side:.1}/{base:.1}");
        }
        say!("    shore probe ({:.0}, {:.0}): road end ({:.0}, {:.0}) {:.0} ft away, {:.1} mi; ft along: bed/tile/side-avg/T0 banked (less sea level):{row}; deepest cut below the sides {worst:.1} ft", p[0], p[1], pts[0][0], pts[0][1], dist(pts[0], p), r.s.last().copied().unwrap_or(0.0) / 5280.0);
    }
}

fn sea_lanes(t0: &T0, world_lattice: f64) {
    use worldgen::t0::biome::Biome;
    use worldgen::t0::{islands, sealanes};
    let Some(extra) = t0.extra.as_ref() else { return };
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let mi = cell / 5280.0;
    let b = |k: usize| Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
    let land: Vec<bool> = (0..w * h).map(|k| b(k) != Biome::Ocean).collect();
    let lake: Vec<u32> = (0..w * h).map(|k| if b(k) == Biome::Lake { 0 } else { worldgen::t0::hydro::NO_LAKE }).collect();
    let isl = islands::label(w, h, cell, &land, &lake, &t0.biome.data);
    let lanes = &t0.sea_lanes;
    let length = |p: &[P]| p.windows(2).map(|q| ((q[1][0] - q[0][0]).powi(2) + (q[1][1] - q[0][1]).powi(2)).sqrt()).sum::<f64>() / 5280.0;
    let mut by = [(0usize, 0.0f64); 3];
    for l in lanes {
        by[l.class as usize].0 += 1;
        by[l.class as usize].1 += length(&l.pts);
    }
    let total: f64 = by.iter().map(|v| v.1).sum();
    let on_land = lanes
        .iter()
        .map(|l| {
            let n = l.pts.len();
            (0..n)
                .filter(|&i| !((i == 0 && l.ends[0] != sealanes::JUNCTION) || (i + 1 == n && l.ends[1] != sealanes::JUNCTION)))
                .filter(|&i| {
                    let p = l.pts[i];
                    let (x, y) = ((p[0] / cell).round() as usize, (p[1] / cell).round() as usize);
                    land[y.min(h - 1) * w + x.min(w - 1)]
                })
                .count()
        })
        .sum::<usize>();
    let links: Vec<_> = extra.economy.sea_links.iter().filter(|l| !l.cells.is_empty()).collect();
    let mut lens: Vec<f64> = links.iter().map(|l| l.mi).collect();
    lens.sort_by(|a, b| a.total_cmp(b));
    // (Each link's cells walked: the miles within 1 mi of the shore, half a cell short of the
    // nearest land cell's centre, past 3 mi from either end.)
    let (mut near, mut open) = (0.0, 0.0);
    for l in &links {
        let c = &l.cells;
        let at = |k: u32| [(k as usize % w) as f64, (k as usize / w) as f64];
        let steps: Vec<f64> = c.windows(2).map(|q| { let (a, b) = (at(q[0]), at(q[1])); ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() * mi }).collect();
        let mut from = 0.0;
        for (i, &s) in steps.iter().enumerate() {
            let mid = from + 0.5 * s;
            from += s;
            if mid < 3.0 || l.mi - mid < 3.0 {
                continue;
            }
            let k = c[i + 1] as usize;
            open += s;
            if t0.coast.data[k] as f64 - 0.5 * cell <= 5280.0 || land[k] {
                near += s;
            }
        }
    }
    let ports: Vec<usize> = (0..t0.settlements.len()).filter(|&i| { let s = &t0.settlements[i]; s.coastal && s.tier >= worldgen::t0::settle::Tier::Town }).collect();
    let linked = |i: usize| lanes.iter().any(|l| l.ends.contains(&(i as u32)));
    let island_ports: Vec<usize> = ports.iter().copied().filter(|&i| isl.island_at(t0.settlements[i].cell).is_some()).collect();
    let trade = |set: &[usize]| {
        let v: Vec<f32> = extra.economy.places.iter().filter(|p| set.contains(&p.index)).map(|p| p.trade).collect();
        if v.is_empty() { f32::NAN } else { v.iter().sum::<f32>() / v.len() as f32 }
    };
    let mainland: Vec<usize> = ports.iter().copied().filter(|i| !island_ports.contains(i)).collect();
    let q = |f: f64| lens.get(((lens.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(0.0);
    let straight: f64 = links.iter().map(|l| { let (a, b) = (&t0.settlements[l.ports[0] as usize], &t0.settlements[l.ports[1] as usize]); ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt() / 5280.0 }).sum();
    let points: usize = lanes.iter().map(|l| l.pts.len()).sum();
    {
        let shortest = |l: &[sealanes::SeaLane]| l.iter().map(|l| length(&l.pts)).fold(f64::MAX, f64::min);
        let mut before = sealanes::lanes_with(&extra.economy.sea_links, &extra.settlements, &land, w, h, cell, false);
        sealanes::clip_ports(&mut before, &|x, y| t0.sample_water(x, y) as f64 > t0.ground_at(x, y, world_lattice));
        for l in lanes.iter().filter(|l| length(&l.pts) < 5.0) {
            say!("    short {:.1} mi ends {:?} pts {:?}", length(&l.pts), l.ends.map(|e| e as i64), l.pts);
        }
        say!("  sea lanes shortest segment {:.1} mi before stub merge ({} segments) -> {:.1} mi after ({})", shortest(&before), before.len(), shortest(lanes), lanes.len());
    }
    say!("  sea lanes JSON: {} points, {:.1} KB", points, sealanes::overlay_json(lanes).len() as f64 / 1024.0);
    say!("  sea lanes: {} segments, {total:.0} mi (busy {} / {:.0} mi, normal {} / {:.0} mi, quiet {} / {:.0} mi); links {} (of {}), length min/median/max {:.1}/{:.1}/{:.1} mi, routed/straight {:.2}; within 1 mi of land (past 3 mi from the ports) {:.1}%; points on land {on_land}; island ports linked {}/{}; trade island ports {:.2}, mainland ports {:.2}",
        lanes.len(), by[0].0, by[0].1, by[1].0, by[1].1, by[2].0, by[2].1, links.len(), extra.economy.sea_links.len(), q(0.0), q(0.5), q(1.0), lens.iter().sum::<f64>() / straight.max(1e-9), 100.0 * near / open.max(1e-9),
        island_ports.iter().filter(|&&i| linked(i)).count(), island_ports.len(), trade(&island_ports), trade(&mainland));
    lane_berth(t0, &land);
    for &i in island_ports.iter().filter(|&&i| !linked(i)) {
        let s = &t0.settlements[i];
        let others = ports.iter().filter(|&&j| j != i && isl.island_at(t0.settlements[j].cell) == isl.island_at(s.cell)).count();
        say!("      island port with no lane: {} at ({:.0}, {:.0}){}, {others} other ports on its island", s.tier.name(), s.x, s.y, if s.isolated { " (realm)" } else { "" });
    }
}

/// The lanes' berth and approaches: the share of lane length within 5 mi of land outside 10 mi of
/// every port, and the near-parallel strands (two segments 0.2-1.5 mi apart for over 2 mi, sampled
/// every 0.1 mi along the first: closer, they are drawn as one; not by an end they share).
fn lane_berth(t0: &T0, land: &[bool]) {
    use worldgen::t0::sealanes::JUNCTION;
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let lanes = &t0.sea_lanes;
    let d = |a: P, b: P| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let ports: Vec<P> = lanes.iter().flat_map(|l| l.ends.iter().filter(|&&e| e != JUNCTION).map(|&e| [t0.settlements[e as usize].x, t0.settlements[e as usize].y])).collect();
    let samples = |pts: &[P], step: f64| {
        let mut v = Vec::new();
        for q in pts.windows(2) {
            let l = d(q[0], q[1]);
            let n = (l / step).ceil().max(1.0) as usize;
            for k in 0..n {
                let t = k as f64 / n as f64;
                v.push(([q[0][0] + (q[1][0] - q[0][0]) * t, q[0][1] + (q[1][1] - q[0][1]) * t], l / n as f64));
            }
        }
        v
    };
    let (mut near, mut open) = (0.0, 0.0);
    for l in lanes {
        for (p, s) in samples(&l.pts, 264.0) {
            if ports.iter().any(|&q| d(p, q) < 10.0 * 5280.0) {
                continue;
            }
            let k = ((p[1] / cell).round() as usize).min(h - 1) * w + ((p[0] / cell).round() as usize).min(w - 1);
            open += s;
            if land[k] || t0.coast.data[k] as f64 - 0.5 * cell <= 5.0 * 5280.0 {
                near += s;
            }
        }
    }
    let seg_d = |p: P, pts: &[P]| {
        pts.windows(2)
            .map(|q| {
                let (dx, dy) = (q[1][0] - q[0][0], q[1][1] - q[0][1]);
                let t = (((p[0] - q[0][0]) * dx + (p[1] - q[0][1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
                let at = [q[0][0] + dx * t, q[0][1] + dy * t];
                (d(p, at), at)
            })
            .fold((f64::MAX, [0.0; 2]), |a, b| if b.0 < a.0 { b } else { a })
    };
    if let Some(port) = std::env::var("LINK_DUMP").ok().and_then(|v| v.parse::<u32>().ok()) {
        for (li, l) in t0.extra.as_ref().map(|e| &e.economy.sea_links[..]).unwrap_or(&[]).iter().enumerate().filter(|(_, l)| l.ports.contains(&port)) {
            let mut c = l.cells.clone();
            if l.ports[1] == port {
                c.reverse();
            }
            let v: Vec<String> = c.iter().take(14).map(|&k| format!("{},{}", k as usize % w, k as usize / w)).collect();
            say!("    link {li} {:?}: {}", l.ports, v.join(" "));
        }
    }
    if std::env::var("LANE_DUMP").is_ok() {
        for (i, l) in lanes.iter().enumerate() {
            let n = l.pts.len();
            say!("    lane {i}: ends {:?} class {} {:.1} mi ({:.0}, {:.0}) -> ({:.0}, {:.0})", l.ends.map(|e| e as i64), l.class, l.pts.windows(2).map(|q| d(q[0], q[1])).sum::<f64>() / 5280.0, l.pts[0][0], l.pts[0][1], l.pts[n - 1][0], l.pts[n - 1][1]);
        }
    }
    let mut strands = Vec::new();
    for i in 0..lanes.len() {
        let si = samples(&lanes[i].pts, 528.0);
        for j in i + 1..lanes.len() {
            let pj = &lanes[j].pts;
            // (Not where they leave one end together: a fork, if no sharper than ~25 degrees: both
            // the sample and the nearest point on the other over 1.5 mi from it.)
            // (A port end by its settlement: the lane was cut at the waterline short of it.)
            let ends = |l: &worldgen::t0::sealanes::SeaLane| {
                let n = l.pts.len();
                [(l.ends[0], l.pts[0]), (l.ends[1], l.pts[n - 1])].map(|(e, p)| if e == JUNCTION { (e, p) } else { (e, [t0.settlements[e as usize].x, t0.settlements[e as usize].y]) })
            };
            let shared: Vec<P> = ends(&lanes[i]).into_iter().filter(|&(e, p)| ends(&lanes[j]).iter().any(|&(f, q)| e == f && d(p, q) < 100.0)).map(|v| v.1).collect();
            let (mut run, mut best, mut at) = (0.0, 0.0, [0.0; 2]);
            for &(p, s) in &si {
                let (dq, q) = seg_d(p, pj);
                if (0.2 * 5280.0..1.5 * 5280.0).contains(&dq) && !shared.iter().any(|&e| d(p, e) < 1.5 * 5280.0 || d(q, e) < 1.5 * 5280.0) {
                    run += s;
                    if run > best {
                        (best, at) = (run, p);
                    }
                } else {
                    run = 0.0;
                }
            }
            if best > 2.0 * 5280.0 {
                strands.push(format!("{i}/{j} {:.1} mi to ({:.0}, {:.0})", best / 5280.0, at[0], at[1]));
            }
        }
    }
    say!("  sea lane berth: within 5 mi of land past 10 mi from ports {:.1}% of {:.0} mi; near-parallel strands {}: {}", 100.0 * near / open.max(1e-9), open / 5280.0, strands.len(), strands.join("; "));
}

fn water_line(on_water: &[[usize; 2]; 4], reasons: &[usize]) -> String {
    let tiers = ["village", "town", "city", "metropolis"];
    let shares: Vec<String> = (0..4).map(|t| format!("{} {}/{} ({:.0}%)", tiers[t], on_water[t][0], on_water[t][1], 100.0 * on_water[t][0] as f64 / on_water[t][1].max(1) as f64)).collect();
    let rs: Vec<String> = reasons.iter().enumerate().filter(|&(_, &c)| c > 0).map(|(i, c)| format!("{} {c}", worldgen::t0::settle::Reason::from_u8(i as u8).name())).collect();
    format!("on water: {}; reasons: {}", shares.join(", "), rs.join(", "))
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
