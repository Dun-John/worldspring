//! Road network at continent scale.
//!
//! 1. Links: a relative-neighbourhood graph over settlements (natural "next town" links) plus
//!    a minimum spanning tree so every landmass is connected. Class from the smaller endpoint:
//!    king's road (cities), road (towns), track (villages).
//! 2. Routing: A* on the T0 grid; cost grows steeply with grade, water crossings and hard
//!    biomes cost extra, lakes and sea are impassable, and stepping on an existing road is
//!    cheap, so trunks are shared and roads find passes and valleys.
//! 3. Segments between junctions; stretches steeper than the class's maximum grade are
//!    re-routed on a 4× finer grid under a hard per-step grade limit, which is what produces
//!    switchbacks. A grade-limited height profile then fixes cut and fill.
//! 4. Crossings (bridge / ford / ferry) where roads meet rivers; waystations about every
//!    day's travel along the main roads.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::biome::Biome;
use super::hydro::{Hydro, NO_LAKE};
use super::settle::{Poi, PoiKind, Settlement, Tier};
use crate::World;
use crate::core::grid::Grid;
use crate::core::noise::{fbm, gradient2, smoothstep};
use crate::core::rng::Pcg32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum RoadClass {
    KingsRoad = 0,
    Road,
    Track,
}

impl RoadClass {
    pub fn from_u8(v: u8) -> RoadClass {
        [RoadClass::KingsRoad, RoadClass::Road, RoadClass::Track][(v as usize).min(2)]
    }
    /// Maximum sustained grade (rise/run).
    pub fn max_grade(self) -> f64 {
        [0.08, 0.10, 0.15][self as usize]
    }
    pub fn width_ft(self) -> f64 {
        [24.0, 16.0, 8.0][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CrossingKind {
    Bridge = 0,
    Ford,
    Ferry,
}

#[derive(Clone, Debug)]
pub struct RoadPath {
    pub class: RoadClass,
    /// Control points (ft) and the road surface elevation at each.
    pub pts: Vec<[f64; 2]>,
    pub z: Vec<f32>,
    /// 1 where the road may wander sideways at fine scale, 0 on switchback legs.
    pub wander: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct Crossing {
    pub kind: CrossingKind,
    pub x: f64,
    pub y: f64,
    pub class: RoadClass,
    pub river_width_ft: f64,
}

pub struct Network {
    pub roads: Vec<RoadPath>,
    pub crossings: Vec<Crossing>,
    pub waystations: Vec<Poi>,
}

pub struct Inputs<'a> {
    pub world: &'a World,
    pub w: usize,
    pub h: usize,
    pub cell_ft: f64,
    pub height: &'a [f64],
    pub land: &'a [bool],
    pub biome: &'a [u32],
    pub hydro: &'a Hydro,
}

#[derive(PartialEq)]
struct Node {
    f: f64,
    i: u32,
}
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f).then_with(|| o.i.cmp(&self.i))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

pub fn build(inp: &Inputs, settlements: &[Settlement]) -> Network {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let p = inp.world.params();
    let river_q = 90_000.0 / p.river_density;

    let edges = link_edges(settlements, &[RoadClass::KingsRoad, RoadClass::Road, RoadClass::Track]);
    let (_, paths) = route(inp, settlements, &edges);

    // --- 3. Segments between junctions (and settlements), from the union of routes.
    let mut links: std::collections::BTreeMap<(u32, u32), u8> = Default::default();
    for (class, path) in &paths {
        for pair in path.windows(2) {
            let key = (pair[0].min(pair[1]), pair[0].max(pair[1]));
            let e = links.entry(key).or_insert(*class as u8);
            *e = (*e).min(*class as u8);
        }
    }
    let mut adj: std::collections::BTreeMap<u32, Vec<u32>> = Default::default();
    for &(a, b) in links.keys() {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    let is_node = |c: u32, adj: &std::collections::BTreeMap<u32, Vec<u32>>| adj[&c].len() != 2 || settlements.iter().any(|s| s.cell as u32 == c);
    let mut used: std::collections::BTreeSet<(u32, u32)> = Default::default();
    let mut segments: Vec<(RoadClass, Vec<u32>)> = Vec::new();
    for (&start, nbs) in &adj {
        if !is_node(start, &adj) {
            continue;
        }
        for &first in nbs {
            let key = (start.min(first), start.max(first));
            if used.contains(&key) {
                continue;
            }
            let mut seg = vec![start, first];
            used.insert(key);
            let mut class = links[&key];
            let (mut prev, mut cur) = (start, first);
            while !is_node(cur, &adj) {
                let next = *adj[&cur].iter().find(|&&x| x != prev).unwrap();
                let k2 = (cur.min(next), cur.max(next));
                if used.contains(&k2) {
                    break;
                }
                used.insert(k2);
                class = class.min(links[&k2]);
                seg.push(next);
                prev = cur;
                cur = next;
            }
            segments.push((RoadClass::from_u8(class), seg));
        }
    }

    // --- Switchbacks and profile.
    let hgrid = Grid::from_vec(w, h, inp.height.iter().map(|&v| v as f32).collect());
    let mut roads: Vec<RoadPath> = Vec::new();
    for (class, seg) in &segments {
        let mut cells: Vec<[f64; 2]> = seg.iter().map(|&c| [(c as usize % w) as f64, (c as usize / w) as f64]).collect();
        // Ends at a settlement meet it where it actually stands.
        for (end, c) in [(0, seg[0]), (cells.len() - 1, seg[seg.len() - 1])] {
            if let Some(s) = settlements.iter().find(|s| s.cell == c as usize) {
                cells[end] = [s.x / cell, s.y / cell];
            }
        }
        let chords = simplify(&cells, w, h, &hgrid, class.max_grade(), cell, passable_fn(inp));
        let (mut pts, mut wander) = switchbacks(&hgrid, &chords, class.max_grade(), cell, passable_fn(inp));
        // Towns and cities are entered at their urban edge (gates); the streets take over.
        for at_start in [true, false] {
            let c = if at_start { seg[0] } else { seg[seg.len() - 1] };
            if let Some(s) = settlements.iter().find(|s| s.cell == c as usize && s.tier >= Tier::Town) {
                let r = crate::town::road_trim_radius(s.tier, s.population) / cell;
                if !at_start {
                    pts.reverse();
                    wander.reverse();
                }
                trim_start(&mut pts, &mut wander, [s.x / cell, s.y / cell], r);
                if !at_start {
                    pts.reverse();
                    wander.reverse();
                }
            }
        }
        // Bend with the land instead of running ruler-straight across it.
        let seed = crate::core::rng::hash2(inp.world.stream("t0.road.bends"), seg[0] as i64, seg[seg.len() - 1] as i64);
        follow_terrain(&mut pts, &mut wander, &hgrid, *class, cell, seed, &|x, y| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && passable_fn(inp)(y as usize * w + x as usize));
        // Less wander near water so bends never push the road into a lake or the sea.
        for (p, wt) in pts.iter().zip(wander.iter_mut()) {
            let mut clear = 2.5f64;
            for dy in -2i64..=2 {
                for dx in -2i64..=2 {
                    let (x, y) = (crate::core::round(p[0]) as i64 + dx, crate::core::round(p[1]) as i64 + dy);
                    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 || !passable_fn(inp)(y as usize * w + x as usize) {
                        clear = clear.min(dist(*p, [x as f64, y as f64]));
                    }
                }
            }
            // Wandering across a slope means cut and fill; keep hillside roads on their line.
            let (gx, gy) = (hgrid.sample_cubic(p[0] + 0.5, p[1]) - hgrid.sample_cubic(p[0] - 0.5, p[1]), hgrid.sample_cubic(p[0], p[1] + 0.5) - hgrid.sample_cubic(p[0], p[1] - 0.5));
            let slope = crate::core::sqrt(gx * gx + gy * gy) / cell;
            *wt *= (smoothstep(0.6, 2.2, clear) * (1.0 - 0.85 * smoothstep(0.01, 0.05, slope))) as f32;
        }
        let z = profile(&hgrid, &pts, class.max_grade(), cell);
        roads.push(RoadPath { class: *class, pts: pts.iter().map(|p| [p[0] * cell, p[1] * cell]).collect(), z, wander });
    }

    // --- 4. Crossings and waystations.
    let mut crossings: Vec<Crossing> = Vec::new();
    for (class, seg) in &segments {
        for pair in seg.windows(2) {
            let (a, b) = (pair[0] as usize, pair[1] as usize);
            // The road steps onto a river cell from a cell that is not part of it.
            let q = inp.hydro.discharge[b] as f64;
            if q >= river_q && (inp.hydro.discharge[a] as f64) < q * 0.999 {
                let width = crate::lod::rivers::width_ft(q);
                let kind = if *class == RoadClass::Track && width < 30.0 {
                    CrossingKind::Ford
                } else if width > 350.0 && *class != RoadClass::KingsRoad {
                    CrossingKind::Ferry
                } else {
                    CrossingKind::Bridge
                };
                let (x, y) = ((b % w) as f64 * cell, (b / w) as f64 * cell);
                if !crossings.iter().any(|c| (c.x - x).abs() < cell * 0.5 && (c.y - y).abs() < cell * 0.5) {
                    crossings.push(Crossing { kind, x, y, class: *class, river_width_ft: width });
                }
            }
        }
    }
    let mut rng = Pcg32::new(inp.world.stream("t0.waystation"), 23);
    let mut waystations: Vec<Poi> = Vec::new();
    let day = 24.0 * 5280.0;
    for r in roads.iter().filter(|r| r.class == RoadClass::KingsRoad) {
        let mut acc = 0.0;
        for pair in r.pts.windows(2) {
            acc += dist(pair[0], pair[1]);
            if acc >= day {
                acc = 0.0;
                let (x, y) = (pair[1][0], pair[1][1]);
                let far_from_town = settlements.iter().all(|s| crate::core::sqrt((s.x - x) * (s.x - x) + (s.y - y) * (s.y - y)) > 10.0 * 5280.0);
                let far_from_other = waystations.iter().all(|q| crate::core::sqrt((q.x - x) * (q.x - x) + (q.y - y) * (q.y - y)) > 15.0 * 5280.0);
                if far_from_town && far_from_other {
                    waystations.push(Poi { kind: PoiKind::Waystation, x, y, seed: rng.next_u32() as u64 | (rng.next_u32() as u64) << 32 });
                }
            }
        }
    }
    Network { roads, crossings, waystations }
}

/// Settlement pairs to join, per class: king's roads join cities, roads join towns and up,
/// tracks join everything. Each class is a relative-neighbourhood graph (no third place
/// closer to both ends) plus a minimum spanning tree so its members stay connected.
fn link_edges(settlements: &[Settlement], classes: &[RoadClass]) -> Vec<(usize, usize, RoadClass)> {
    let ns = settlements.len();
    let d = |a: usize, b: usize| {
        let (sa, sb) = (&settlements[a], &settlements[b]);
        crate::core::sqrt((sa.x - sb.x) * (sa.x - sb.x) + (sa.y - sb.y) * (sa.y - sb.y))
    };
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut edges: Vec<(usize, usize, RoadClass)> = Vec::new();
    for (class, min_tier, max_mi) in [(RoadClass::KingsRoad, Tier::City, 450.0), (RoadClass::Road, Tier::Town, 160.0), (RoadClass::Track, Tier::Village, 45.0)] {
        if !classes.contains(&class) {
            continue;
        }
        let max_link = max_mi * 5280.0;
        let members: Vec<usize> = (0..ns).filter(|&i| settlements[i].tier >= min_tier).collect();
        let add = |a: usize, b: usize, edges: &mut Vec<(usize, usize, RoadClass)>| {
            let (a, b) = (a.min(b), a.max(b));
            if !edges.iter().any(|e| e.0 == a && e.1 == b) {
                edges.push((a, b, class));
            }
        };
        let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
        for (ia, &a) in members.iter().enumerate() {
            for &b in &members[ia + 1..] {
                let dab = d(a, b);
                if dab < max_link {
                    pairs.push((dab, a, b));
                }
            }
        }
        pairs.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        for &(dab, a, b) in &pairs {
            if members.iter().all(|&c| c == a || c == b || d(a, c).max(d(b, c)) >= dab) {
                add(a, b, &mut edges);
            }
        }
        let mut parent: Vec<usize> = (0..ns).collect();
        for &(_, a, b) in &pairs {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra != rb {
                parent[ra] = rb;
                add(a, b, &mut edges);
            }
        }
    }

    edges
}

/// A* routes for the links, in order (later routes reuse earlier ones). Returns the best
/// (lowest) class per cell (`u8::MAX` = none) and each route's cells.
pub fn route(inp: &Inputs, settlements: &[Settlement], edges: &[(usize, usize, RoadClass)]) -> (Vec<u8>, Vec<(RoadClass, Vec<u32>)>) {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let p = inp.world.params();
    let river_q = 90_000.0 / p.river_density;
    let n = w * h;
    // --- 2. Routing.
    let biome_pen = |k: usize| match Biome::from_u8((inp.biome[k] & 0xff) as u8) {
        Biome::Swamp => 2.0,
        Biome::Jungle => 1.0,
        Biome::Alpine => 1.0,
        Biome::Ice => 3.0,
        Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga => 0.3,
        Biome::HotDesert | Biome::ColdDesert => 0.3,
        _ => 0.0,
    };
    let passable = |k: usize| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE;
    // Smooth cost noise: equal-cost ties on open ground resolve into gentle meanders instead
    // of grid-aligned runs.
    let noise_seed = inp.world.stream("t0.road.cost");
    let wobble: Vec<f32> = (0..n).map(|k| (1.0 + 0.25 * fbm(noise_seed, (k % w) as f64 / 6.0, (k / w) as f64 / 6.0, 2, 2.0, 0.5)) as f32).collect();
    // 16 directions (knight moves too) so paths are not limited to 45° headings.
    const DIRS: [(i64, i64); 16] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)];
    let mut road_class: Vec<u8> = vec![u8::MAX; n];
    let mut paths: Vec<(RoadClass, Vec<u32>)> = Vec::new();
    let mut g = vec![f64::INFINITY; n];
    let mut came = vec![u32::MAX; n];
    let mut touched: Vec<usize> = Vec::new();
    for &(a, b, class) in edges {
        let (start, goal) = (settlements[a].cell, settlements[b].cell);
        // Near a town or city at either end, roads keep to themselves (no shared trunk), so
        // each arrives at its own gate instead of merging outside the walls.
        let town_ends: Vec<(f64, f64, f64)> = [a, b]
            .iter()
            .filter(|&&i| settlements[i].tier >= Tier::Town)
            .map(|&i| {
                let r = crate::town::urban_radius(settlements[i].tier, settlements[i].population) / cell;
                ((settlements[i].cell % w) as f64, (settlements[i].cell / w) as f64, r + 1.5)
            })
            .collect();
        let (gx, gy) = ((goal % w) as f64, (goal / w) as f64);
        // Search box: endpoints' bounding box grown by 40% + margin.
        let (sx, sy) = ((start % w) as f64, (start / w) as f64);
        let pad = 0.4 * (sx - gx).abs().max((sy - gy).abs()) + 25.0;
        let (bx0, bx1) = ((sx.min(gx) - pad).max(0.0), (sx.max(gx) + pad).min((w - 1) as f64));
        let (by0, by1) = ((sy.min(gy) - pad).max(0.0), (sy.max(gy) + pad).min((h - 1) as f64));
        for &k in &touched {
            g[k] = f64::INFINITY;
            came[k] = u32::MAX;
        }
        touched.clear();
        let mut heap = BinaryHeap::new();
        g[start] = 0.0;
        touched.push(start);
        heap.push(Node { f: 0.0, i: start as u32 });
        let mut found = false;
        let mut expanded = 0;
        while let Some(Node { i, .. }) = heap.pop() {
            let i = i as usize;
            if i == goal {
                found = true;
                break;
            }
            expanded += 1;
            if expanded > 400_000 {
                break;
            }
            let (ix, iy) = ((i % w) as i64, (i / w) as i64);
            for (dx, dy) in DIRS {
                let (nx, ny) = (ix + dx, iy + dy);
                if (nx as f64) < bx0 || (nx as f64) > bx1 || (ny as f64) < by0 || (ny as f64) > by1 {
                    continue;
                }
                let nb = ny as usize * w + nx as usize;
                // Knight moves pass between two cells; both must be passable, and a river in
                // either still has to be crossed.
                let mids: &[usize] = &if dx.abs() == 2 || dy.abs() == 2 {
                    let (sx, sy) = (dx.signum(), dy.signum());
                    let (ax, ay) = if dx.abs() == 2 { (ix + sx, iy) } else { (ix, iy + sy) };
                    [ay as usize * w + ax as usize, (ay + if dx.abs() == 2 { sy } else { 0 }) as usize * w + (ax + if dy.abs() == 2 { sx } else { 0 }) as usize]
                } else {
                    [nb, nb]
                };
                if !passable(nb) || !mids.iter().all(|&m| passable(m)) {
                    continue;
                }
                let dist = crate::core::sqrt((dx * dx + dy * dy) as f64);
                let grade = (inp.height[nb] - inp.height[i]).abs() / (dist * cell);
                let mut c = (1.0 + 25.0 * grade * grade + 40.0 * (grade - 0.15).max(0.0) + biome_pen(nb)) * wobble[nb] as f64;
                let q = mids.iter().chain(std::iter::once(&nb)).map(|&m| inp.hydro.discharge[m]).fold(0f32, f32::max) as f64;
                if q >= river_q && (inp.hydro.discharge[i] as f64) < q {
                    c += 1.0 + 3.0 * (q / (river_q * 20.0)).min(1.0);
                }
                let by_town = town_ends.iter().any(|&(tx, ty, tr)| (nx as f64 - tx) * (nx as f64 - tx) + (ny as f64 - ty) * (ny as f64 - ty) <= tr * tr);
                if road_class[nb] != u8::MAX && !by_town {
                    c *= 0.2;
                }
                let ng = g[i] + dist * c;
                if ng < g[nb] {
                    if g[nb] == f64::INFINITY {
                        touched.push(nb);
                    }
                    g[nb] = ng;
                    came[nb] = i as u32;
                    let hdist = crate::core::sqrt((nx as f64 - gx) * (nx as f64 - gx) + (ny as f64 - gy) * (ny as f64 - gy));
                    heap.push(Node { f: ng + 0.35 * hdist, i: nb as u32 });
                }
            }
        }
        if !found {
            continue;
        }
        let mut path = vec![goal as u32];
        let mut cur = goal;
        while cur != start {
            cur = came[cur] as usize;
            path.push(cur as u32);
        }
        path.reverse();
        for &c in &path {
            let c = c as usize;
            road_class[c] = road_class[c].min(class as u8);
        }
        paths.push((class, path));
    }

    (road_class, paths)
}

/// Where roads of the given classes would run, for placing settlements beside them.
pub struct Preview {
    /// Per cell: routes passing through (saturating).
    pub usage: Vec<u8>,
    /// Per cell: a junction (3+ distinct route edges meet there).
    pub junction: Vec<bool>,
}

pub fn preview(inp: &Inputs, settlements: &[Settlement], classes: &[RoadClass]) -> Preview {
    let edges = link_edges(settlements, classes);
    let (_, paths) = route(inp, settlements, &edges);
    let n = inp.w * inp.h;
    let mut usage = vec![0u8; n];
    let mut links: std::collections::BTreeSet<(u32, u32)> = Default::default();
    for (_, path) in &paths {
        for &c in path {
            usage[c as usize] = usage[c as usize].saturating_add(1);
        }
        for pair in path.windows(2) {
            links.insert((pair[0].min(pair[1]), pair[0].max(pair[1])));
        }
    }
    let mut degree = vec![0u8; n];
    for &(a, b) in &links {
        degree[a as usize] = degree[a as usize].saturating_add(1);
        degree[b as usize] = degree[b as usize].saturating_add(1);
    }
    Preview { usage, junction: degree.iter().map(|&d| d >= 3).collect() }
}

fn passable_fn<'a>(inp: &'a Inputs) -> impl Fn(usize) -> bool + 'a {
    move |k| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE
}

/// Gives a road (points in cells, with their wander weights) the bends of a real one, baked
/// into its points so its profile and bed follow them: resampled every `STEP` cells, each
/// point is first offset sideways by gentle noise bends (wavelengths of about 3,700 and
/// 1,400 ft, straighter for a king's road, more winding for a track), then slides (up to
/// `REACH` cells) to ease the grades either side of it, kept smooth and near its bend: on a
/// hillside the road curves along the slope. Switchback legs (wander 0), the ends and water
/// stay as they are; wander is then 0 where the road bends here (the noise wander added
/// when it is drawn would ignore its profile and cut the hillside).
fn follow_terrain(pts: &mut Vec<[f64; 2]>, wander: &mut Vec<f32>, hgrid: &Grid<f32>, class: RoadClass, cell: f64, seed: u64, dry: &dyn Fn(i64, i64) -> bool) {
    const STEP: f64 = 0.05;
    const REACH: f64 = 0.35;
    if pts.len() < 2 || wander.iter().all(|w| *w < 0.5) {
        return;
    }
    let gmax = class.max_grade();
    // Resample (keeping switchback legs' points as they are).
    let (mut rp, mut rw): (Vec<[f64; 2]>, Vec<f32>) = (vec![pts[0]], vec![wander[0]]);
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let n = if wander[k - 1] < 0.5 || wander[k] < 0.5 { 1 } else { (dist(a, b) / STEP).ceil().max(1.0) as usize };
        for j in 1..=n {
            let t = j as f64 / n as f64;
            rp.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            rw.push(wander[k - 1] + (wander[k] - wander[k - 1]) * t as f32);
        }
    }
    let n = rp.len();
    if n < 3 {
        return;
    }
    let mut s = vec![0.0; n];
    for i in 1..n {
        s[i] = s[i - 1] + dist(rp[i - 1], rp[i]);
    }
    // Distance along the road to the nearest point that stays (an end or a switchback leg):
    // bends and sliding fade in over the first `EASE` cells from it.
    const EASE: f64 = 0.3;
    let stays = |i: usize| i == 0 || i == n - 1 || rw[i] < 0.5;
    let mut fixed = vec![f64::MAX; n];
    let mut last = f64::MIN;
    for i in 0..n {
        if stays(i) {
            last = s[i];
        }
        fixed[i] = s[i] - last;
    }
    let mut next = f64::MAX;
    for i in (0..n).rev() {
        if stays(i) {
            next = s[i];
        }
        fixed[i] = fixed[i].min(next - s[i]);
    }
    let ease: Vec<f64> = fixed.iter().map(|f| smoothstep(0.0, EASE, *f)).collect();
    // Bends: noise offsets along the line's normal.
    let amp = [0.75, 1.0, 1.3][class as usize];
    let normal = |i: usize| {
        let (a, b) = (rp[i.saturating_sub(1)], rp[(i + 1).min(n - 1)]);
        let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
        let tl = crate::core::sqrt(tx * tx + ty * ty).max(1e-9);
        [-ty / tl, tx / tl]
    };
    let mut orig = rp.clone();
    for i in 1..n - 1 {
        if ease[i] <= 0.0 {
            continue;
        }
        let off = ease[i] * amp * (0.045 * gradient2(seed, s[i] / 0.6, 0.31) + 0.012 * gradient2(seed ^ 0xb3, s[i] / 0.22, 0.77));
        let nrm = normal(i);
        for f in [1.0, 0.5] {
            let q = [rp[i][0] + nrm[0] * off * f, rp[i][1] + nrm[1] * off * f];
            if dry(crate::core::round(q[0]) as i64, crate::core::round(q[1]) as i64) {
                orig[i] = q;
                break;
            }
        }
    }
    let mut rp = orig.clone();
    let room: Vec<f64> = ease.iter().map(|e| REACH * e).collect();
    let hz = |p: [f64; 2]| hgrid.sample_cubic(p[0], p[1]);
    let mut z: Vec<f64> = rp.iter().map(|p| hz(*p)).collect();
    let grade = |za: f64, zb: f64, a: [f64; 2], b: [f64; 2]| (zb - za) / (dist(a, b) * cell).max(1.0);
    let cost = |g: f64| 100.0 * g * g + if g.abs() > gmax { 1_000.0 * (g.abs() - gmax) * (g.abs() - gmax) } else { 0.0 };
    for d in [0.08, 0.08, 0.05, 0.05, 0.03, 0.03, 0.02, 0.02, 0.01, 0.01] {
        for i in 1..n - 1 {
            if room[i] <= 0.0 {
                continue;
            }
            let (a, b) = (rp[i - 1], rp[i + 1]);
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let tl = crate::core::sqrt(tx * tx + ty * ty).max(1e-9);
            let nrm = [-ty / tl, tx / tl];
            let energy = |q: [f64; 2], zq: f64| {
                let bend = [a[0] - 2.0 * q[0] + b[0], a[1] - 2.0 * q[1] + b[1]];
                let off = dist(q, orig[i]) / REACH;
                cost(grade(z[i - 1], zq, a, q)) + cost(grade(zq, z[i + 1], q, b)) + (bend[0] * bend[0] + bend[1] * bend[1]) / (STEP * STEP) + 0.2 * off * off
            };
            let mut best = (energy(rp[i], z[i]), rp[i], z[i]);
            for sign in [-1.0, 1.0] {
                let q = [rp[i][0] + nrm[0] * d * sign, rp[i][1] + nrm[1] * d * sign];
                if dist(q, orig[i]) > room[i] || !dry(crate::core::round(q[0]) as i64, crate::core::round(q[1]) as i64) {
                    continue;
                }
                let zq = hz(q);
                let e = energy(q, zq);
                if e < best.0 {
                    best = (e, q, zq);
                }
            }
            (rp[i], z[i]) = (best.1, best.2);
        }
    }
    *pts = rp;
    *wander = rw.iter().zip(&ease).map(|(w, e)| if *e > 0.0 { 0.0 } else { *w }).collect();
}

/// Drop the part of a path inside radius `r` of `c` at its start (cut exactly at the circle).
fn trim_start(pts: &mut Vec<[f64; 2]>, wander: &mut Vec<f32>, c: [f64; 2], r: f64) {
    let Some(k) = pts.iter().position(|p| dist(*p, c) >= r) else { return };
    if k == 0 || pts.len() - k < 1 {
        return;
    }
    let (a, b) = (pts[k - 1], pts[k]);
    let (da, db) = (dist(a, c), dist(b, c));
    let t = ((r - da) / (db - da).max(1e-9)).clamp(0.0, 1.0);
    pts[k - 1] = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    pts.drain(..k - 1);
    wander.drain(..k - 1);
    if let Some(w) = wander.first_mut() {
        *w = 0.0;
    }
}

/// Any-angle path from a grid path (string pulling): Douglas-Peucker that accepts a chord
/// when it stays near the grid path, crosses only passable ground and is no steeper than the
/// class allows (or than the path itself was); chords are then resampled to at most one cell
/// per step so the grade check below sees the terrain between vertices.
fn simplify(cells: &[[f64; 2]], w: usize, h: usize, hg: &Grid<f32>, gmax: f64, cell: f64, passable: impl Fn(usize) -> bool) -> Vec<[f64; 2]> {
    const TOL: f64 = 2.5;
    let steepest = |pts: &mut dyn Iterator<Item = [f64; 2]>| {
        let mut prev: Option<([f64; 2], f64)> = None;
        let mut worst = 0.0f64;
        for p in pts {
            let z = hg.sample_cubic(p[0], p[1]);
            if let Some((q, zq)) = prev {
                worst = worst.max((z - zq).abs() / (dist(p, q) * cell).max(1.0));
            }
            prev = Some((p, z));
        }
        worst
    };
    let clear = |a: [f64; 2], b: [f64; 2], path: &[[f64; 2]]| {
        let n = (dist(a, b) * 4.0).ceil().max(1.0) as usize;
        let sample = |s: usize| {
            let t = s as f64 / n as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        };
        let dry = (0..=n).all(|s| {
            let p = sample(s);
            passable((crate::core::round(p[1]) as usize).min(h - 1) * w + (crate::core::round(p[0]) as usize).min(w - 1))
        });
        dry && {
            let chord = steepest(&mut (0..=n).step_by(2).map(sample));
            chord <= gmax || chord <= steepest(&mut path.iter().copied())
        }
    };
    let mut keep = vec![false; cells.len()];
    keep[0] = true;
    *keep.last_mut().unwrap() = true;
    let mut stack = vec![(0usize, cells.len() - 1)];
    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (a, b) = (cells[i], cells[j]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = crate::core::sqrt(dx * dx + dy * dy).max(1e-9);
        let (mut worst, mut at) = (0.0, i + 1);
        for (k, p) in cells.iter().enumerate().take(j).skip(i + 1) {
            let d = ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / len;
            if d > worst {
                worst = d;
                at = k;
            }
        }
        if worst > TOL || !clear(a, b, &cells[i..=j]) {
            keep[at] = true;
            stack.push((i, at));
            stack.push((at, j));
        }
    }
    let kept: Vec<[f64; 2]> = cells.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p).collect();
    let mut out = vec![kept[0]];
    for pair in kept.windows(2) {
        let n = dist(pair[0], pair[1]).ceil().max(1.0) as usize;
        for s in 1..=n {
            let t = s as f64 / n as f64;
            out.push([pair[0][0] + (pair[1][0] - pair[0][0]) * t, pair[0][1] + (pair[1][1] - pair[0][1]) * t]);
        }
    }
    out
}

/// Replace too-steep stretches with grade-limited paths on a 4× grid (T0 cell units in/out).
/// Also returns the wander weight per point (0 on switchback legs, which must stay put).
fn switchbacks(hg: &Grid<f32>, cells: &[[f64; 2]], gmax: f64, cell: f64, passable: impl Fn(usize) -> bool) -> (Vec<[f64; 2]>, Vec<f32>) {
    const F: f64 = 4.0;
    let at = |p: [f64; 2]| hg.sample_cubic(p[0], p[1]);
    let mut out: Vec<[f64; 2]> = vec![cells[0]];
    let mut wander: Vec<f32> = vec![1.0];
    let mut i = 0;
    while i + 1 < cells.len() {
        let (a, b) = (cells[i], cells[i + 1]);
        let run = crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])) * cell;
        let grade = (at(b) - at(a)).abs() / run;
        if grade <= gmax {
            out.push(b);
            wander.push(1.0);
            i += 1;
            continue;
        }
        // Extend the steep stretch while it stays steep (up to 12 cells).
        let mut j = i + 1;
        while j + 1 < cells.len() && j - i < 12 {
            let (c, e) = (cells[j], cells[j + 1]);
            let r = crate::core::sqrt((e[0] - c[0]) * (e[0] - c[0]) + (e[1] - c[1]) * (e[1] - c[1])) * cell;
            if (at(e) - at(c)).abs() / r <= gmax {
                break;
            }
            j += 1;
        }
        match fine_route(hg, cells[i], cells[j], gmax, cell, F, &passable) {
            Some(route) => {
                // Drop collinear fine-grid points; keep the legs and hairpins.
                let mut legs: Vec<[f64; 2]> = vec![route[0]];
                for k in 1..route.len() - 1 {
                    let (p, q, r) = (legs[legs.len() - 1], route[k], route[k + 1]);
                    if ((q[0] - p[0]) * (r[1] - q[1]) - (q[1] - p[1]) * (r[0] - q[0])).abs() > 1e-9 {
                        legs.push(q);
                    }
                }
                legs.push(route[route.len() - 1]);
                if let Some(w) = wander.last_mut() {
                    *w = 0.0;
                }
                out.extend_from_slice(&legs[1..]);
                wander.extend(std::iter::repeat_n(0.0, legs.len() - 1));
            }
            None => {
                out.extend_from_slice(&cells[i + 1..=j]);
                wander.extend(std::iter::repeat_n(1.0, j - i));
            }
        }
        i = j;
    }
    (out, wander)
}

/// A* on a grid `f` times finer than T0 inside a corridor; each step's grade must be ≤ gmax
/// (then ≤ 1.8 gmax as a fallback). Returns points in T0 cell units.
fn fine_route(hg: &Grid<f32>, a: [f64; 2], b: [f64; 2], gmax: f64, cell: f64, f: f64, passable: &impl Fn(usize) -> bool) -> Option<Vec<[f64; 2]>> {
    let pad = 4.0;
    let (x0, y0) = ((a[0].min(b[0]) - pad).max(0.0), (a[1].min(b[1]) - pad).max(0.0));
    let (x1, y1) = ((a[0].max(b[0]) + pad).min((hg.w - 1) as f64), (a[1].max(b[1]) + pad).min((hg.h - 1) as f64));
    let (fw, fh) = (((x1 - x0) * f) as usize + 1, ((y1 - y0) * f) as usize + 1);
    let to_world = |i: usize| [x0 + (i % fw) as f64 / f, y0 + (i / fw) as f64 / f];
    let hts: Vec<f64> = (0..fw * fh).map(|i| hg.sample_cubic(to_world(i)[0], to_world(i)[1])).collect();
    let ok: Vec<bool> = (0..fw * fh)
        .map(|i| {
            let p = to_world(i);
            passable((p[1].round() as usize).min(hg.h - 1) * hg.w + (p[0].round() as usize).min(hg.w - 1))
        })
        .collect();
    let idx = |p: [f64; 2]| (((p[1] - y0) * f).round() as usize).min(fh - 1) * fw + (((p[0] - x0) * f).round() as usize).min(fw - 1);
    let (s, t) = (idx(a), idx(b));
    let step = cell / f;
    const DIRS: [(i32, i32); 16] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)];
    for limit in [gmax, gmax * 1.8] {
        let mut g = vec![f64::INFINITY; fw * fh];
        let mut came = vec![u32::MAX; fw * fh];
        let mut heap = BinaryHeap::new();
        g[s] = 0.0;
        heap.push(Node { f: 0.0, i: s as u32 });
        let (tx, ty) = ((t % fw) as f64, (t / fw) as f64);
        while let Some(Node { i, .. }) = heap.pop() {
            let i = i as usize;
            if i == t {
                let mut path = vec![t];
                let mut cur = t;
                while cur != s {
                    cur = came[cur] as usize;
                    path.push(cur);
                }
                path.reverse();
                return Some(path.into_iter().map(to_world).collect());
            }
            let (ix, iy) = ((i % fw) as i32, (i / fw) as i32);
            for (dx, dy) in DIRS {
                let (nx, ny) = (ix + dx, iy + dy);
                if nx < 0 || ny < 0 || nx >= fw as i32 || ny >= fh as i32 {
                    continue;
                }
                let nb = ny as usize * fw + nx as usize;
                if !ok[nb] {
                    continue;
                }
                let run = crate::core::sqrt((dx * dx + dy * dy) as f64) * step;
                let grade = (hts[nb] - hts[i]).abs() / run;
                if grade > limit {
                    continue;
                }
                // Mild turn cost keeps legs straight; climbing costs a little extra.
                let prev = came[i];
                let turn = if prev == u32::MAX {
                    0.0
                } else {
                    let (px, py) = ((prev as usize % fw) as i32, (prev as usize / fw) as i32);
                    let (ax, ay) = ((ix - px) as f64, (iy - py) as f64);
                    let cosang = (ax * dx as f64 + ay * dy as f64) / (crate::core::sqrt(ax * ax + ay * ay) * crate::core::sqrt((dx * dx + dy * dy) as f64)).max(1e-9);
                    0.3 * (1.0 - cosang)
                };
                let ng = g[i] + run * (1.0 + 2.0 * grade) + turn * step;
                if ng < g[nb] {
                    g[nb] = ng;
                    came[nb] = i as u32;
                    let hd = dist([nx as f64, ny as f64], [tx, ty]) * step;
                    heap.push(Node { f: ng + hd, i: nb as u32 });
                }
            }
        }
    }
    None
}

/// Road surface elevation along the path, limited to the class grade: the average of the
/// cut-only envelope (largest grade-limited profile below the terrain) and the fill-only
/// one (smallest above it). Both are exactly grade-limited, so their average is too, and it
/// balances cuttings through bumps against embankments over dips.
fn profile(hg: &Grid<f32>, pts: &[[f64; 2]], gmax: f64, cell: f64) -> Vec<f32> {
    let terrain: Vec<f64> = pts.iter().map(|p| hg.sample_cubic(p[0], p[1])).collect();
    let ft: Vec<[f64; 2]> = pts.iter().map(|p| [p[0] * cell, p[1] * cell]).collect();
    fit_profile(&ft, &terrain, gmax)
}

/// Road surface heights for points (ft) over the given ground heights: as close to the
/// ground as the grade limit allows (the mean of the cut-only and fill-only envelopes).
pub fn fit_profile(pts: &[[f64; 2]], terrain: &[f64], gmax: f64) -> Vec<f32> {
    let lim: Vec<f64> = (0..pts.len())
        .map(|i| if i == 0 { 0.0 } else { gmax * dist(pts[i], pts[i - 1]) })
        .collect();
    let envelope = |sign: f64| {
        // sign = 1: cut-only (min-plus); sign = -1: fill-only (max-minus).
        let mut z: Vec<f64> = terrain.iter().map(|v| v * sign).collect();
        for i in 1..z.len() {
            z[i] = z[i].min(z[i - 1] + lim[i]);
        }
        for i in (1..z.len()).rev() {
            z[i - 1] = z[i - 1].min(z[i] + lim[i]);
        }
        z.into_iter().map(|v| v * sign).collect::<Vec<f64>>()
    };
    let (cut, fill) = (envelope(1.0), envelope(-1.0));
    // f32 rounding must not push a step over the limit: shave a hair off by re-clamping.
    let mut z: Vec<f32> = cut.iter().zip(&fill).map(|(a, b)| (0.5 * (a + b)) as f32).collect();
    for i in 1..z.len() {
        let l = (lim[i] * 0.999) as f32;
        z[i] = z[i].clamp(z[i - 1] - l, z[i - 1] + l);
    }
    z
}

/// `fit_profile`, but never below `floor` (a bridge deck's level at a river crossing, else
/// `f64::MIN`): ramps fall away from each floor at the grade limit, and the profile is the
/// higher of those and the fit (both grade-limited, so it is too).
pub fn fit_profile_over(pts: &[[f64; 2]], terrain: &[f64], floor: &[f64], gmax: f64) -> Vec<f32> {
    let mut z = fit_profile(pts, terrain, gmax);
    if floor.iter().all(|f| *f == f64::MIN) {
        return z;
    }
    let lim: Vec<f64> = (0..pts.len()).map(|i| if i == 0 { 0.0 } else { 0.999 * gmax * dist(pts[i], pts[i - 1]) }).collect();
    let mut ramp = floor.to_vec();
    for i in 1..ramp.len() {
        ramp[i] = ramp[i].max(ramp[i - 1] - lim[i]);
    }
    for i in (1..ramp.len()).rev() {
        ramp[i - 1] = ramp[i - 1].max(ramp[i] - lim[i]);
    }
    for (v, r) in z.iter_mut().zip(&ramp) {
        *v = v.max(*r as f32);
    }
    z
}

/// Drops control points (ft) that only make the road jog: closer than 40 ft to the point
/// before or after, or where it doubles back (turns over 100°) within 250 ft; never an end,
/// and only where the chord left keeps the road's grade.
pub fn tidy(r: &mut RoadPath) {
    let gmax = r.class.max_grade();
    let mut k = 1;
    while k + 1 < r.pts.len() {
        let (a, b, c) = (r.pts[k - 1], r.pts[k], r.pts[k + 1]);
        let (ab, bc) = (dist(a, b), dist(b, c));
        let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
        let back = u[0] * v[0] + u[1] * v[1] < -0.17 * ab * bc;
        let jog = ab.min(bc) < 40.0 || (back && ab.min(bc) < 250.0);
        if jog && ((r.z[k + 1] - r.z[k - 1]).abs() as f64) <= gmax * dist(a, c) {
            r.pts.remove(k);
            r.z.remove(k);
            r.wander.remove(k);
            k = k.saturating_sub(1).max(1);
        } else {
            k += 1;
        }
    }
}

#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    crate::core::sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]))
}
