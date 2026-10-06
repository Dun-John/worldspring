//! QA: how roads, rivers and settlements meet. Counts what should never happen, with the
//! worst places (world ft) to go and look:
//! - walls, towers, buildings and fields over a river channel or standing water;
//! - town streets and approaches over the river with no bridge under them;
//! - approaches (road end to gate) running through built blocks, or turning sharply;
//! - sharp turns on the network roads.
//!
//! Usage: cargo run --release -p worldgen --example roadcheck -- [seed...]   (default 1 2 3)
use std::collections::HashMap;

use worldgen::t0::T0;
use worldgen::town::geom::{centroid, contains, dist, lerp, seg_dist, sub};
use worldgen::{World, WorldFile};

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
    worst: Vec<(f64, String)>,
}

impl Tally {
    fn add(&mut self, size: f64, what: String) {
        self.n += 1;
        self.worst.push((size, what));
    }
    fn print(&mut self, label: &str) {
        self.worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("  {label}: {}", self.n);
        for (_, w) in self.worst.iter().take(std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4)) {
            println!("      {w}");
        }
    }
}

fn main() {
    let seeds: Vec<u32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let seeds = if seeds.is_empty() { vec![1, 2, 3] } else { seeds };
    for seed in seeds {
        let t = std::time::Instant::now();
        let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
        let t0 = T0::generate(&world);
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
        let standing = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
        let n_layouts = worldgen::town::layout_count(&t0);
        println!("seed {seed}: {} settlements, {} sites, T0 {:.1} s", t0.settlements.len(), n_layouts - t0.settlements.len(), t.elapsed().as_secs_f64());
        let t = std::time::Instant::now();
        let mut walls = Tally::default();
        let mut towers = Tally::default();
        let mut buildings = Tally::default();
        let mut fields = Tally::default();
        let mut streets = Tally::default();
        let mut through = Tally::default();
        let mut over = Tally::default();
        let mut turns = Tally::default();
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
                    walls.add(n as f64 * 5.0, format!("{name}: {:.0} ft of wall in the river near ({:.0}, {:.0})", n as f64 * 5.0, p[0], p[1]));
                }
            }
            for &tw in l.towers.iter().chain(&l.gate_towers) {
                if ch.inside(tw, 2.0) {
                    towers.add(1.0, format!("{name}: tower in the river at ({:.0}, {:.0})", tw[0], tw[1]));
                }
            }
            for b in &l.buildings {
                let o = outline(&b.poly);
                let n = o.iter().filter(|p| wet(**p, 2.0)).count();
                if n > 0 {
                    let c = centroid(&b.poly);
                    buildings.add(n as f64, format!("{name}: building {:?} ({}/{} points wet) at ({:.0}, {:.0})", b.func, n, o.len(), c[0], c[1]));
                }
            }
            for f in &l.fields {
                let o = outline(f);
                let n = o.iter().filter(|p| ch.inside(**p, 2.0)).count();
                if n > 0 {
                    let c = centroid(f);
                    fields.add(n as f64, format!("{name}: field over the river ({n} points) at ({:.0}, {:.0})", c[0], c[1]));
                }
            }
            // Streets and approaches over the river with no bridge deck under them.
            for (pts, class, _) in &l.roads {
                let s = along(pts, 5.0);
                let bad: Vec<&P> = s.iter().filter(|p| ch.inside(**p, 2.0) && !l.bridges.iter().any(|b| contains(b, **p))).collect();
                if !bad.is_empty() {
                    streets.add(bad.len() as f64, format!("{name}: class {class} street, {} ft over the river unbridged near ({:.0}, {:.0})", bad.len() * 5, bad[0][0], bad[0][1]));
                }
            }
            // Approaches (road classes 0–2): through built blocks or buildings, and their sharpest turn.
            for (pts, class, _) in l.roads.iter().filter(|r| r.1 <= 2) {
                let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
                let s = along(pts, 10.0);
                let keep = ((total - 40.0) / 10.0).max(0.0) as usize;
                let inside = s.iter().take(keep).filter(|p| !wet(**p, 0.0) && l.districts.iter().any(|b| contains(b, **p))).count();
                if inside > 0 {
                    through.add(inside as f64, format!("{name}: class {class} approach crosses {} ft of the town, from ({:.0}, {:.0})", inside * 10, pts[0][0], pts[0][1]));
                }
                let on = s.iter().take(keep).filter(|p| l.buildings.iter().any(|b| contains(&b.poly, **p))).count();
                if on > 0 {
                    over.add(on as f64, format!("{name}: class {class} approach runs over {} ft of buildings, from ({:.0}, {:.0})", on * 10, pts[0][0], pts[0][1]));
                }
                if pts.len() >= 3 {
                    let (deg, at) = sharpest(pts, 30.0);
                    if deg > 60.0 {
                        turns.add(deg, format!("{name}: approach turns {deg:.0}° at ({:.0}, {:.0})", at[0], at[1]));
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
                    println!("  road {ri} {:?} ({:.0}, {:.0}) -> ({:.0}, {:.0}) {:.0} ft", rc.class, a[0], a[1], b[0], b[1], rc.s.last().unwrap());
                }
            }
        }
        if let Some(ri) = std::env::var("ROAD").ok().and_then(|v| v.parse::<usize>().ok()) {
            let rc = &t0.roads.roads[ri];
            for (k, p) in rc.pts.iter().enumerate() {
                println!("  road {ri} pt {k}: ({:.0}, {:.0}) z {:.0} wander {:.2} s {:.0}", p[0], p[1], rc.z[k], rc.wander[k], rc.s[k]);
            }
            for (si, st) in t0.settlements.iter().enumerate().filter(|(_, st)| rc.pts.iter().any(|p| dist(*p, [st.x, st.y]) < 3.0 * t0.cell_ft)) {
                println!("  settlement {si} {:?} at ({:.0}, {:.0}), trim {:.0}", st.tier, st.x, st.y, worldgen::town::road_trim_radius(st.tier, st.population));
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
                    let near = t0.settlements.iter().map(|st| (dist([st.x, st.y], pts[i]) / worldgen::town::road_trim_radius(st.tier, st.population), st.tier)).min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
                    let from_end = (i as f64 * 10.0).min((pts.len() - 1 - i) as f64 * 10.0);
                    kinks.add(deg, format!("road {ri} ({:?}) turns {deg:.0}° at ({:.0}, {:.0}), {from_end:.0} ft from its end, {:.2} trim radii from a {:?}", rc.class, pts[i][0], pts[i][1], near.0, near.1));
                }
            }
        }
        walls.print("wall runs in a river");
        towers.print("towers in a river");
        buildings.print("buildings over water");
        fields.print("fields over a river");
        streets.print("streets over a river with no bridge");
        through.print("approaches through the town (its districts)");
        over.print("approaches over buildings");
        turns.print("approaches turning > 60° within 30 ft");
        kinks.print("network road turns > 75° within 40 ft");
        println!("  ({n_layouts} layouts in {layout_s:.1} s)");
    }
}
