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
use crate::core::noise::{gradient2, smoothstep};
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

/// How a road of `class` crosses a river `width_ft` wide: a track wades a creek under 30 ft, a
/// river over 250 ft wide is crossed by ferry (a king's road always has its bridge), else a bridge.
pub fn crossing_kind(class: RoadClass, width_ft: f64) -> CrossingKind {
    if class == RoadClass::Track && width_ft < 30.0 {
        CrossingKind::Ford
    } else if width_ft > 250.0 && class != RoadClass::KingsRoad {
        CrossingKind::Ferry
    } else {
        CrossingKind::Bridge
    }
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

/// The ground roads are planned on (T0 cell units in, ft out): the T0 surface as the
/// terrain's coarsest refined level holds it, cut by the river valleys (`RiverNet::valley`),
/// sampled every half cell. Switchbacks, bends and profiles follow it, so a road planned on
/// it lies on the ground the terrain is built from (not in a trench or on a dike).
pub struct Plan {
    pub grid: Grid<f32>,
    /// Size in T0 cells.
    pub cw: usize,
    pub ch: usize,
}

impl Plan {
    pub const SCALE: f64 = 2.0;
    pub fn z(&self, p: [f64; 2]) -> f64 {
        self.grid.sample_cubic(p[0] * Self::SCALE, p[1] * Self::SCALE)
    }
}

pub struct Inputs<'a> {
    pub world: &'a World,
    pub plan: &'a Plan,
    /// The plan's ground at any point (ft): what the plan's grid samples every half cell.
    pub ground: &'a dyn Fn(f64, f64) -> f64,
    pub w: usize,
    pub h: usize,
    pub cell_ft: f64,
    pub height: &'a [f64],
    pub land: &'a [bool],
    pub biome: &'a [u32],
    pub hydro: &'a Hydro,
    /// The routes `route` found last time (see `RouteKey`), for the next call to reuse.
    pub routes: std::cell::RefCell<Vec<(RouteKey, Option<Vec<u32>>)>>,
}

/// What a route depends on besides the routes before it: its end cells, class and the towns
/// at its ends (centre cell, radius bits). Calls whose links begin alike (the king's roads,
/// placed before the towns and villages) find those routes alike.
#[derive(Clone, PartialEq)]
pub struct RouteKey(usize, usize, RoadClass, Vec<(u64, u64, u64)>);

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
    let hgrid = inp.plan;
    // For switchbacks the plan's grid cannot route: the ground at any point (a narrow valley
    // between its nodes), and dry land finer than whole cells (a cell counted as sea still
    // has land on its shore side, where a coastal settlement's road comes down).
    let fine_ground = |p: [f64; 2]| (inp.ground)(p[0] * cell, p[1] * cell);
    let sea = inp.world.params().sea_level_ft;
    let fine_dry = |p: [f64; 2]| {
        let k = (crate::core::round(p[1]) as usize).min(h - 1) * w + (crate::core::round(p[0]) as usize).min(w - 1);
        passable_fn(inp)(k) || (inp.hydro.lake_of[k] == NO_LAKE && fine_ground(p) > sea + 3.0)
    };
    let mut roads: Vec<RoadPath> = Vec::new();
    for (class, seg) in &segments {
        let mut cells: Vec<[f64; 2]> = seg.iter().map(|&c| [(c as usize % w) as f64, (c as usize / w) as f64]).collect();
        // Ends at a settlement meet it where it actually stands.
        for (end, c) in [(0, seg[0]), (cells.len() - 1, seg[seg.len() - 1])] {
            if let Some(s) = settlements.iter().find(|s| s.cell == c as usize) {
                cells[end] = [s.x / cell, s.y / cell];
            }
        }
        let chords = simplify(&cells, w, h, hgrid, class.max_grade(), cell, passable_fn(inp));
        let (mut pts, mut wander) = switchbacks(hgrid, &chords, class.max_grade(), cell, passable_fn(inp), &fine_ground, &fine_dry);
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
        follow_terrain(&mut pts, &mut wander, hgrid, *class, cell, seed, &|x, y| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && passable_fn(inp)(y as usize * w + x as usize));
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
            let (gx, gy) = (hgrid.z([p[0] + 0.5, p[1]]) - hgrid.z([p[0] - 0.5, p[1]]), hgrid.z([p[0], p[1] + 0.5]) - hgrid.z([p[0], p[1] - 0.5]));
            let slope = crate::core::sqrt(gx * gx + gy * gy) / cell;
            *wt *= (smoothstep(0.6, 2.2, clear) * (1.0 - 0.85 * smoothstep(0.01, 0.05, slope))) as f32;
        }
        let z = profile(hgrid, &pts, class.max_grade(), cell);
        roads.push(RoadPath { class: *class, pts: pts.iter().map(|p| [p[0] * cell, p[1] * cell]).collect(), z, wander });
    }

    // Junctions outside settlements become Ys.
    let dry_ft = |x: f64, y: f64| {
        let (i, j) = (crate::core::round(x / cell) as i64, crate::core::round(y / cell) as i64);
        i >= 0 && j >= 0 && (i as usize) < w && (j as usize) < h && passable_fn(inp)(j as usize * w + i as usize)
    };
    // Settlements and how far round them a junction stays put (towns' roads end at their edge).
    let towns: Vec<([f64; 2], f64)> = settlements.iter().map(|s| ([s.x, s.y], crate::town::road_trim_radius(s.tier, s.population).max(0.15 * cell))).collect();
    drop_parallel(&mut roads, &towns);
    drop_shortcuts(&mut roads, &towns);
    join_through(&mut roads, &towns, cell, &dry_ft);
    merge_junctions(&mut roads, &towns, cell, &dry_ft);
    drop_parallel(&mut roads, &towns);
    join_through(&mut roads, &towns, cell, &dry_ft);

    // --- 4. Crossings and waystations.
    let mut crossings: Vec<Crossing> = Vec::new();
    for (class, seg) in &segments {
        for pair in seg.windows(2) {
            let (a, b) = (pair[0] as usize, pair[1] as usize);
            // The road steps onto a river cell from a cell that is not part of it.
            let q = inp.hydro.discharge[b] as f64;
            if q >= river_q && (inp.hydro.discharge[a] as f64) < q * 0.999 {
                let width = crate::lod::rivers::width_ft(q);
                let kind = crossing_kind(*class, width);
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
    let open: Vec<bool> = (0..n).map(|k| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE).collect();
    let passable = |k: usize| open[k];
    let pen: Vec<f64> = (0..n).map(biome_pen).collect();
    // Smooth cost noise: equal-cost ties on open ground resolve into gentle meanders instead
    // of grid-aligned runs.
    let mut noise = crate::core::noise::Fbm::new(inp.world.stream("t0.road.cost"), 2, 2.0, 0.5);
    let wobble: Vec<f32> = (0..n).map(|k| (1.0 + 0.25 * noise.at((k % w) as f64 / 6.0, (k / w) as f64 / 6.0)) as f32).collect();
    // 16 directions (knight moves too) so paths are not limited to 45° headings.
    const DIRS: [(i64, i64); 16] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)];
    let step: [f64; 16] = std::array::from_fn(|d| crate::core::sqrt((DIRS[d].0 * DIRS[d].0 + DIRS[d].1 * DIRS[d].1) as f64));
    let mut road_class: Vec<u8> = vec![u8::MAX; n];
    let mut paths: Vec<(RoadClass, Vec<u32>)> = Vec::new();
    let mut g = vec![f64::INFINITY; n];
    let mut came = vec![u32::MAX; n];
    let mut touched: Vec<usize> = Vec::new();
    let mut memo = inp.routes.borrow_mut();
    let mut same = true;
    for (e, &(a, b, class)) in edges.iter().enumerate() {
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
        let key = RouteKey(start, goal, class, town_ends.iter().map(|t| (t.0.to_bits(), t.1.to_bits(), t.2.to_bits())).collect());
        // The same links so far as the last call: the same routes.
        same = same && memo.get(e).is_some_and(|m| m.0 == key);
        let found = if same {
            memo[e].1.clone()
        } else {
            let (gx, gy) = ((goal % w) as f64, (goal / w) as f64);
            // Search box: endpoints' bounding box grown by 40% + margin.
            let (sx, sy) = ((start % w) as f64, (start / w) as f64);
            let pad = 0.4 * (sx - gx).abs().max((sy - gy).abs()) + 25.0;
            let (bx0, bx1) = ((sx.min(gx) - pad).max(0.0), (sx.max(gx) + pad).min((w - 1) as f64));
            let (by0, by1) = ((sy.min(gy) - pad).max(0.0), (sy.max(gy) + pad).min((h - 1) as f64));
            // The box in whole cells (a cell index is inside exactly when it is between these).
            let (bx0, bx1, by0, by1) = (crate::core::ceil(bx0) as i64, crate::core::floor(bx1) as i64, crate::core::ceil(by0) as i64, crate::core::floor(by1) as i64);
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
                for (d, (dx, dy)) in DIRS.into_iter().enumerate() {
                    let (nx, ny) = (ix + dx, iy + dy);
                    if nx < bx0 || nx > bx1 || ny < by0 || ny > by1 {
                        continue;
                    }
                    let nb = ny as usize * w + nx as usize;
                    if !passable(nb) {
                        continue;
                    }
                    // Knight moves pass between two cells; both must be passable, and a river in
                    // either still has to be crossed.
                    let mids: &[usize] = &if dx.abs() == 2 || dy.abs() == 2 {
                        let (sx, sy) = (dx.signum(), dy.signum());
                        let (ax, ay) = if dx.abs() == 2 { (ix + sx, iy) } else { (ix, iy + sy) };
                        [ay as usize * w + ax as usize, (ay + if dx.abs() == 2 { sy } else { 0 }) as usize * w + (ax + if dy.abs() == 2 { sx } else { 0 }) as usize]
                    } else {
                        [nb, nb]
                    };
                    if !mids.iter().all(|&m| passable(m)) {
                        continue;
                    }
                    let dist = step[d];
                    let grade = (inp.height[nb] - inp.height[i]).abs() / (dist * cell);
                    let mut c = (1.0 + 25.0 * grade * grade + 40.0 * (grade - 0.15).max(0.0) + pen[nb]) * wobble[nb] as f64;
                    let q = mids.iter().chain(std::iter::once(&nb)).map(|&m| inp.hydro.discharge[m]).fold(0f32, f32::max) as f64;
                    if q >= river_q && (inp.hydro.discharge[i] as f64) < q {
                        c += 1.0 + 3.0 * (q / (river_q * 20.0)).min(1.0);
                    }
                    let by_town = || town_ends.iter().any(|&(tx, ty, tr)| (nx as f64 - tx) * (nx as f64 - tx) + (ny as f64 - ty) * (ny as f64 - ty) <= tr * tr);
                    if road_class[nb] != u8::MAX && !by_town() {
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
            let path = found.then(|| {
                let mut path = vec![goal as u32];
                let mut cur = goal;
                while cur != start {
                    cur = came[cur] as usize;
                    path.push(cur as u32);
                }
                path.reverse();
                path
            });
            memo.truncate(e);
            memo.push((key, path.clone()));
            path
        };
        let Some(path) = found else { continue };
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
fn follow_terrain(pts: &mut Vec<[f64; 2]>, wander: &mut Vec<f32>, hgrid: &Plan, class: RoadClass, cell: f64, seed: u64, dry: &dyn Fn(i64, i64) -> bool) {
    const STEP: f64 = 0.05;
    const REACH: f64 = 0.35;
    if pts.len() < 2 {
        return;
    }
    let gmax = class.max_grade();
    // Resample every `STEP` (switchback legs too: they stay where they are, but the profile
    // needs points along them to follow the ground).
    let (mut rp, mut rw): (Vec<[f64; 2]>, Vec<f32>) = (vec![pts[0]], vec![wander[0]]);
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let n = (dist(a, b) / STEP).ceil().max(1.0) as usize;
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
    let hz = |p: [f64; 2]| hgrid.z(p);
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
fn simplify(cells: &[[f64; 2]], w: usize, h: usize, hg: &Plan, gmax: f64, cell: f64, passable: impl Fn(usize) -> bool) -> Vec<[f64; 2]> {
    const TOL: f64 = 2.5;
    let steepest = |pts: &mut dyn Iterator<Item = [f64; 2]>| {
        let mut prev: Option<([f64; 2], f64)> = None;
        let mut worst = 0.0f64;
        for p in pts {
            let z = hg.z(p);
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

/// Replace too-steep stretches (on the plan between points, or on `ground` over any quarter
/// of one) with grade-limited paths on a 4× grid (T0 cell units in/out), or where that finds
/// none, an 8× grid on `ground` within `dry` (both at a point, in cells).
/// Also returns the wander weight per point (0 on switchback legs, which must stay put).
fn switchbacks(
    hg: &Plan,
    cells: &[[f64; 2]],
    gmax: f64,
    cell: f64,
    passable: impl Fn(usize) -> bool,
    ground: &impl Fn([f64; 2]) -> f64,
    dry: &impl Fn([f64; 2]) -> bool,
) -> (Vec<[f64; 2]>, Vec<f32>) {
    const F: f64 = 4.0;
    const F_FINE: f64 = 8.0;
    // Legs are planned this far under the limit: the ground the profile meets is rougher than
    // the plan, and a route at the limit leaves no room (the profile would cut and fill by
    // a hundred feet).
    const SLACK: f64 = 0.85;
    let at = |p: [f64; 2]| hg.z(p);
    // Steeper than the limit over any quarter of the stretch on the ground itself.
    let steep = |a: [f64; 2], b: [f64; 2]| {
        let q: Vec<f64> = (0..=4).map(|k| ground([a[0] + (b[0] - a[0]) * k as f64 / 4.0, a[1] + (b[1] - a[1]) * k as f64 / 4.0])).collect();
        let run = 0.25 * dist(a, b) * cell;
        q.windows(2).any(|w| (w[1] - w[0]).abs() > gmax * run)
    };
    let mut out: Vec<[f64; 2]> = vec![cells[0]];
    let mut wander: Vec<f32> = vec![1.0];
    let mut i = 0;
    while i + 1 < cells.len() {
        let (a, b) = (cells[i], cells[i + 1]);
        let run = crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])) * cell;
        let grade = (at(b) - at(a)).abs() / run;
        if grade <= gmax && !steep(a, b) {
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
            if (at(e) - at(c)).abs() / r <= gmax && !steep(c, e) {
                break;
            }
            j += 1;
        }
        let on_plan = |p: [f64; 2]| hg.z(p);
        let on_cells = |p: [f64; 2]| passable((p[1].round() as usize).min(hg.ch - 1) * hg.cw + (p[0].round() as usize).min(hg.cw - 1));
        let route = fine_route(hg, &on_plan, &on_cells, cells[i], cells[j], SLACK * gmax, cell, F)
            .or_else(|| fine_route(hg, ground, dry, cells[i], cells[j], SLACK * gmax, cell, F_FINE));
        match route {
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
    // Corners (hairpins, and where a switchback stretch meets the road) turn on arcs.
    round_corners(&out, &wander, 0.06, 0.05)
}

/// A polyline (cells) with each corner turning more than 30° rounded by a circular arc
/// tangent to both sides (radius up to `r_max`, smaller where the sides are short or meet at
/// a narrow angle: a hairpin becomes a tight U-turn), and points at most `step` apart; `vals`
/// (one per point) are carried along (interpolated on lines, the corner's on its arc).
pub fn round_corners(pts: &[[f64; 2]], vals: &[f32], r_max: f64, step: f64) -> (Vec<[f64; 2]>, Vec<f32>) {
    let n = pts.len();
    if n < 3 {
        return (pts.to_vec(), vals.to_vec());
    }
    // Per corner: (distance cut back along each side, arc centre, radius); None: kept sharp.
    let fillet: Vec<Option<(f64, [f64; 2], f64)>> = (0..n)
        .map(|k| {
            if k == 0 || k == n - 1 {
                return None;
            }
            let (a, b) = (unit2([pts[k - 1][0] - pts[k][0], pts[k - 1][1] - pts[k][1]]), unit2([pts[k + 1][0] - pts[k][0], pts[k + 1][1] - pts[k][1]]));
            // Interior angle between the sides; a turn under 30° stays.
            let alpha = libm::acos(dot2(a, b).clamp(-1.0, 1.0));
            if alpha > 150f64.to_radians() || alpha < 1e-3 {
                return None;
            }
            let tan_half = libm::tan(0.5 * alpha);
            let t_max = 0.45 * dist(pts[k - 1], pts[k]).min(dist(pts[k], pts[k + 1]));
            let r = r_max.min(t_max * tan_half);
            let t = r / tan_half;
            let bis = unit2([a[0] + b[0], a[1] + b[1]]);
            let c = r / libm::sin(0.5 * alpha);
            Some((t, [pts[k][0] + bis[0] * c, pts[k][1] + bis[1] * c], r))
        })
        .collect();
    let mut out: Vec<[f64; 2]> = vec![pts[0]];
    let mut ov: Vec<f32> = vec![vals[0]];
    let line = |out: &mut Vec<[f64; 2]>, ov: &mut Vec<f32>, b: [f64; 2], vb: f32| {
        let (a, va) = (*out.last().unwrap(), *ov.last().unwrap());
        let m = (dist(a, b) / step).ceil().max(1.0) as usize;
        out.extend((1..=m).map(|i| [a[0] + (b[0] - a[0]) * i as f64 / m as f64, a[1] + (b[1] - a[1]) * i as f64 / m as f64]));
        ov.extend((1..=m).map(|i| va + (vb - va) * i as f32 / m as f32));
    };
    // Points closer than this to the last are left out (no slivers of segments).
    let min_gap = 0.05 * step;
    let toward = |from: [f64; 2], to: [f64; 2], d: f64| {
        let u = unit2([to[0] - from[0], to[1] - from[1]]);
        [from[0] + u[0] * d, from[1] + u[1] * d]
    };
    for k in 1..n {
        let Some((t, c, r)) = fillet[k] else {
            line(&mut out, &mut ov, pts[k], vals[k]);
            continue;
        };
        let (p0, p3) = (toward(pts[k], pts[k - 1], t), toward(pts[k], pts[k + 1], t));
        if dist(p0, *out.last().unwrap()) >= min_gap {
            line(&mut out, &mut ov, p0, vals[k]);
        }
        // Round the arc from p0 to p3 about c, the short way.
        let (a0, a1) = (libm::atan2(p0[1] - c[1], p0[0] - c[0]), libm::atan2(p3[1] - c[1], p3[0] - c[0]));
        let mut sweep = a1 - a0;
        while sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        }
        while sweep < -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }
        // At least a point every 15° (a tight hairpin is still round) and every `step`.
        let m = ((sweep.abs() * r) / step).max(sweep.abs() / 15f64.to_radians()).ceil().clamp(2.0, 60.0) as usize;
        for i in 1..=m {
            let a = a0 + sweep * i as f64 / m as f64;
            let q = [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)];
            if dist(q, *out.last().unwrap()) >= min_gap {
                out.push(q);
                ov.push(vals[k]);
            }
        }
    }
    (out, ov)
}

/// A* on a grid `f` times finer than T0 inside a corridor, over the `ground` heights at its
/// nodes where `ok` (both at a point in cells); each step's grade must be ≤ gmax (then
/// ≤ 1.8 gmax as a fallback). Returns points in T0 cell units.
#[allow(clippy::too_many_arguments)]
fn fine_route(hg: &Plan, ground: &impl Fn([f64; 2]) -> f64, ok: &impl Fn([f64; 2]) -> bool, a: [f64; 2], b: [f64; 2], gmax: f64, cell: f64, f: f64) -> Option<Vec<[f64; 2]>> {
    let pad = 4.0;
    let (x0, y0) = ((a[0].min(b[0]) - pad).max(0.0), (a[1].min(b[1]) - pad).max(0.0));
    let (x1, y1) = ((a[0].max(b[0]) + pad).min((hg.cw - 1) as f64), (a[1].max(b[1]) + pad).min((hg.ch - 1) as f64));
    let (fw, fh) = (((x1 - x0) * f) as usize + 1, ((y1 - y0) * f) as usize + 1);
    let to_world = |i: usize| [x0 + (i % fw) as f64 / f, y0 + (i / fw) as f64 / f];
    let hts: Vec<f64> = (0..fw * fh).map(|i| ground(to_world(i))).collect();
    let ok: Vec<bool> = (0..fw * fh).map(|i| ok(to_world(i))).collect();
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
fn profile(hg: &Plan, pts: &[[f64; 2]], gmax: f64, cell: f64) -> Vec<f32> {
    let terrain: Vec<f64> = pts.iter().map(|p| hg.z(*p)).collect();
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

/// Where network roads meet: their shared ends, and where one crosses another (or ends on its
/// middle). Each such place becomes a point of every road there (put in where it falls between
/// two), so their profiles can be made to agree on it (`level_profiles`). Returns per place the
/// roads and their point indices there. `wander` gets a point wherever `pts` does.
pub fn meeting_points(pts: &mut [Vec<[f64; 2]>], wander: &mut [Vec<f32>]) -> Vec<Vec<(usize, usize)>> {
    use crate::core::hash::{FastMap, FastSet};
    // Places (by 1-ft key), each with the roads at it: (road, segment, fraction along it).
    let mut places: Vec<([f64; 2], Vec<(usize, usize, f64)>)> = Vec::new();
    let mut at: FastMap<(i64, i64), usize> = FastMap::default();
    let mut add = |p: [f64; 2], r: usize, k: usize, t: f64| {
        let (kx, ky) = (crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64);
        let found = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (kx + dx, ky + dy))).filter_map(|key| at.get(&key).copied()).find(|&i| dist(places[i].0, p) <= 1.0);
        let i = found.unwrap_or_else(|| {
            places.push((p, Vec::new()));
            at.insert((kx, ky), places.len() - 1);
            places.len() - 1
        });
        places[i].1.push((r, k, t));
    };
    for (r, q) in pts.iter().enumerate() {
        if q.len() >= 2 {
            add(q[0], r, 0, 0.0);
            add(q[q.len() - 1], r, q.len() - 2, 1.0);
        }
    }
    // Crossings: segments binned on a coarse grid, each pair of roads' segments in a bin tried once.
    const BIN: f64 = 2_000.0;
    let mut bins: FastMap<(i64, i64), Vec<(usize, usize)>> = FastMap::default();
    for (r, q) in pts.iter().enumerate() {
        for k in 0..q.len().saturating_sub(1) {
            let (a, b) = (q[k], q[k + 1]);
            let (x0, x1) = (crate::core::floor(a[0].min(b[0]) / BIN) as i64, crate::core::floor(a[0].max(b[0]) / BIN) as i64);
            let (y0, y1) = (crate::core::floor(a[1].min(b[1]) / BIN) as i64, crate::core::floor(a[1].max(b[1]) / BIN) as i64);
            for by in y0..=y1 {
                for bx in x0..=x1 {
                    bins.entry((bx, by)).or_default().push((r, k));
                }
            }
        }
    }
    let mut keys: Vec<(i64, i64)> = bins.keys().copied().collect();
    keys.sort_unstable();
    let mut tried: FastSet<(usize, usize, usize, usize)> = FastSet::default();
    for key in keys {
        let list = &bins[&key];
        for (n, &(r1, k1)) in list.iter().enumerate() {
            for &(r2, k2) in &list[n + 1..] {
                if r1 == r2 || !tried.insert((r1, k1, r2, k2)) {
                    continue;
                }
                let (a, b, c, d) = (pts[r1][k1], pts[r1][k1 + 1], pts[r2][k2], pts[r2][k2 + 1]);
                let (e, f) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
                let den = e[0] * f[1] - e[1] * f[0];
                if den.abs() < 1e-9 {
                    continue;
                }
                let t = ((c[0] - a[0]) * f[1] - (c[1] - a[1]) * f[0]) / den;
                let u = ((c[0] - a[0]) * e[1] - (c[1] - a[1]) * e[0]) / den;
                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    let p = [a[0] + e[0] * t, a[1] + e[1] * t];
                    add(p, r1, k1, t);
                    add(p, r2, k2, u);
                }
            }
        }
    }
    // Each place on each road there: an existing point within a foot, else a new one.
    let mut cuts: Vec<Vec<(usize, f64, usize)>> = vec![Vec::new(); pts.len()];
    let mut meets: Vec<Vec<(usize, usize)>> = Vec::new();
    for (_, list) in &places {
        let roads: FastSet<usize> = list.iter().map(|e| e.0).collect();
        if roads.len() < 2 {
            continue;
        }
        let id = meets.len();
        meets.push(Vec::new());
        for &(r, k, t) in list {
            cuts[r].push((k, t, id));
        }
    }
    let point: Vec<[f64; 2]> = places.iter().filter(|(_, l)| l.iter().map(|e| e.0).collect::<FastSet<usize>>().len() >= 2).map(|(p, _)| *p).collect();
    for r in 0..pts.len() {
        if cuts[r].is_empty() {
            continue;
        }
        cuts[r].sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        let old = std::mem::take(&mut pts[r]);
        let oldw = std::mem::take(&mut wander[r]);
        let (mut np, mut nw) = (Vec::with_capacity(old.len() + cuts[r].len()), Vec::with_capacity(old.len() + cuts[r].len()));
        // Old point index → new; places on an old point are resolved once all are in.
        let mut map = vec![0usize; old.len()];
        let mut on_point: Vec<(usize, usize)> = Vec::new();
        let mut ci = 0;
        for k in 0..old.len() {
            map[k] = np.len();
            np.push(old[k]);
            nw.push(oldw[k]);
            while ci < cuts[r].len() && cuts[r][ci].0 == k {
                let (_, t, id) = cuts[r][ci];
                ci += 1;
                let next = (k + 1).min(old.len() - 1);
                let len = dist(old[k], old[next]);
                if t * len < 1.0 {
                    on_point.push((k, id));
                } else if (1.0 - t) * len < 1.0 {
                    on_point.push((next, id));
                } else {
                    let q = point[id];
                    if dist(*np.last().unwrap(), q) >= 1.0 {
                        np.push(q);
                        nw.push(oldw[k] + (oldw[next] - oldw[k]) * t as f32);
                    }
                    meets[id].push((r, np.len() - 1));
                }
            }
        }
        for (k, id) in on_point {
            meets[id].push((r, map[k]));
        }
        pts[r] = np;
        wander[r] = nw;
    }
    for m in &mut meets {
        m.sort_unstable();
        m.dedup();
    }
    meets.retain(|m| m.len() >= 2);
    meets
}

/// Profiles for the whole network: each road fitted to `terrain` within its grade
/// (`fit_profile`), never below `floor` (bridge decks), and every road at a meeting place
/// (`meeting_points`) at one level there: the ground, or the greatest road's own level where it
/// stands off the ground (the lesser ones ramp to it); a road whose route is too steep to reach
/// that from the places it already meets comes as near as it can. The greater roads' places
/// are levelled first. Bounds that fall away from each level and floor at the grade limit keep
/// each fit within its grade.
pub fn level_profiles(pts: &[Vec<[f64; 2]>], class: &[RoadClass], terrain: &[Vec<f64>], floor: &[Vec<f64>], meets: &[Vec<(usize, usize)>]) -> Vec<Vec<f32>> {
    let n = pts.len();
    let s: Vec<Vec<f64>> = pts
        .iter()
        .map(|q| {
            let mut acc = 0.0;
            let mut v = vec![0.0; q.len()];
            for i in 1..q.len() {
                acc += dist(q[i - 1], q[i]);
                v[i] = acc;
            }
            v
        })
        .collect();
    let g: Vec<f64> = class.iter().map(|c| 0.999 * c.max_grade()).collect();
    // Each road's floor, ramping down from every deck at the grade limit.
    let ramp: Vec<Vec<f64>> = (0..n)
        .map(|r| {
            let mut v = floor[r].clone();
            for i in 1..v.len() {
                v[i] = v[i].max(v[i - 1] - g[r] * (s[r][i] - s[r][i - 1]));
            }
            for i in (1..v.len()).rev() {
                v[i - 1] = v[i - 1].max(v[i] - g[r] * (s[r][i] - s[r][i - 1]));
            }
            v
        })
        .collect();
    // Each road's own fit, as it would stand with no one to meet.
    let free: Vec<Vec<f32>> = (0..n).map(|r| fit_profile(&pts[r], &terrain[r], class[r].max_grade())).collect();
    let mut pins: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let band = |pins: &[(usize, f64)], r: usize, i: usize| {
        let (mut lo, mut hi) = (ramp[r][i], f64::INFINITY);
        for &(j, z) in pins {
            let d = g[r] * (s[r][i] - s[r][j]).abs();
            lo = lo.max(z - d);
            hi = hi.min(z + d);
        }
        (lo, hi)
    };
    // The greater roads' places first, then west to east (a stable order).
    let mut order: Vec<usize> = (0..meets.len()).collect();
    let key = |m: &Vec<(usize, usize)>| (m.iter().map(|&(r, _)| class[r]).min().unwrap_or(RoadClass::Track), pts[m[0].0][m[0].1]);
    order.sort_by(|&a, &b| {
        let (ka, kb) = (key(&meets[a]), key(&meets[b]));
        ka.0.cmp(&kb.0).then(ka.1[0].total_cmp(&kb.1[0])).then(ka.1[1].total_cmp(&kb.1[1]))
    });
    for m in order {
        let (r0, i0) = meets[m][0];
        // The ground, unless the greatest roads there would all stand off it (on an embankment
        // up a slope too steep for them, say): then the nearest of their levels, and the lesser
        // roads ramp to it. Each road comes as near that as its grade allows from the places it
        // already meets (one whose route is too steep keeps the step to itself).
        let best = meets[m].iter().map(|&(r, _)| class[r]).min().unwrap_or(RoadClass::Track);
        let (mut a, mut b) = (f64::INFINITY, f64::MIN);
        for &(r, i) in meets[m].iter().filter(|&&(r, _)| class[r] == best) {
            let (lo, hi) = band(&pins[r], r, i);
            let v = (free[r][i] as f64).min(hi).max(lo);
            (a, b) = (a.min(v), b.max(v));
        }
        let level = terrain[r0][i0].max(a).min(b);
        for &(r, i) in &meets[m] {
            let (lo, hi) = band(&pins[r], r, i);
            pins[r].push((i, level.min(hi).max(lo)));
        }
    }
    // Each road's final profile, the greater roads first: a lesser road running alongside one
    // already done near where they meet (a Y's converging legs) keeps to its level there, so
    // no strip between them is cut or banked.
    const ALONG_FT: f64 = 600.0;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&r| (class[r], r));
    let mut done: Vec<Option<Vec<f32>>> = vec![None; n];
    let at_road = |q: usize, near: usize, p: [f64; 2], zq: &[f32]| -> Option<(f64, f64)> {
        // Nearest point of road q within ALONG_FT (along it) of its point `near`: (distance, level).
        let mut best: Option<(f64, f64)> = None;
        let k0 = s[q].partition_point(|&v| v < s[q][near] - 1.5 * ALONG_FT).saturating_sub(1);
        let k1 = s[q].partition_point(|&v| v <= s[q][near] + 1.5 * ALONG_FT).min(pts[q].len() - 1);
        for k in k0..k1 {
            let (a, b) = (pts[q][k], pts[q][k + 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
            let d = dist(p, [a[0] + dx * t, a[1] + dy * t]);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, zq[k] as f64 + (zq[k + 1] - zq[k]) as f64 * t));
            }
        }
        best
    };
    for &r in &order {
        for m in meets.iter().filter(|m| m.iter().any(|&(q, _)| q == r)) {
            let i = m.iter().find(|&&(q, _)| q == r).map(|e| e.1).unwrap_or(0);
            for &(q, j) in m.iter().filter(|&&(q, _)| q != r) {
                let Some(zq) = done[q].as_deref() else { continue };
                let half = 0.5 * (class[r].width_ft() + class[q].width_ft());
                for dir in [-1i64, 1] {
                    let mut k = i as i64 + dir;
                    while k >= 0 && (k as usize) < pts[r].len() && (s[r][k as usize] - s[r][i]).abs() <= ALONG_FT {
                        let ku = k as usize;
                        let (lo, hi) = band(&pins[r], r, ku);
                        let own = (free[r][ku] as f64).min(hi).max(lo);
                        match at_road(q, j, pts[r][ku], zq) {
                            // Too close for a bank between them (beds' sides fall 1 in 2).
                            Some((d, z)) if d - half <= 2.0 * (own - z).abs() + 10.0 => {
                                pins[r].push((ku, z.min(hi).max(lo)));
                            }
                            _ => break,
                        }
                        k += dir;
                    }
                }
            }
        }
        let fit = &free[r];
        let m = pts[r].len();
        let mut lo = ramp[r].clone();
        let mut hi = vec![f64::INFINITY; m];
        for &(i, z) in &pins[r] {
            lo[i] = lo[i].max(z);
            hi[i] = hi[i].min(z);
        }
        for i in 1..m {
            let l = g[r] * (s[r][i] - s[r][i - 1]);
            lo[i] = lo[i].max(lo[i - 1] - l);
            hi[i] = hi[i].min(hi[i - 1] + l);
        }
        for i in (1..m).rev() {
            let l = g[r] * (s[r][i] - s[r][i - 1]);
            lo[i - 1] = lo[i - 1].max(lo[i] - l);
            hi[i - 1] = hi[i - 1].min(hi[i] + l);
        }
        let mut z: Vec<f32> = (0..m).map(|i| (fit[i] as f64).min(hi[i]).max(lo[i]) as f32).collect();
        // f32 rounding must not push a step over the limit.
        for i in 1..m {
            let l = (g[r] * (s[r][i] - s[r][i - 1]) * 0.999) as f32;
            z[i] = z[i].clamp(z[i - 1] - l, z[i - 1] + l);
        }
        done[r] = Some(z);
    }
    done.into_iter().map(|z| z.unwrap_or_default()).collect()
}

/// A road's end at a junction: (road, at its start, index of a point some way along it, that
/// point, heading from it into the junction).
type Leg = (usize, bool, usize, [f64; 2], [f64; 2]);

fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]).max(1e-9);
    [v[0] / l, v[1] / l]
}

fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// Each road end at junction `j`: its point `reach` ft along it (or before its middle).
fn legs_at(roads: &[RoadPath], list: &[(usize, bool)], j: [f64; 2], reach: f64) -> Vec<Leg> {
    list.iter()
        .map(|&(ri, start)| {
            let pts: Vec<[f64; 2]> = if start { roads[ri].pts.clone() } else { roads[ri].pts.iter().rev().copied().collect() };
            let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
            let want = reach.min(0.45 * total);
            let (mut acc, mut idx) = (0.0, 1);
            while idx + 1 < pts.len() && acc + dist(pts[idx - 1], pts[idx]) < want {
                acc += dist(pts[idx - 1], pts[idx]);
                idx += 1;
            }
            let (a, b) = (pts[idx], pts[(idx + 1).min(pts.len() - 1)]);
            let heading = if idx + 1 < pts.len() { unit2([a[0] - b[0], a[1] - b[1]]) } else { unit2([j[0] - a[0], j[1] - a[1]]) };
            (ri, start, idx, a, heading)
        })
        .collect()
}

/// The point where roads to `pts` (with weights) joined would be shortest (Weiszfeld), from `from`.
fn fermat(pts: &[([f64; 2], f64)], from: [f64; 2]) -> [f64; 2] {
    let mut m = from;
    for _ in 0..60 {
        let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
        for &(p, w) in pts {
            let d = dist(m, p).max(1.0);
            sx += w * p[0] / d;
            sy += w * p[1] / d;
            sw += w / d;
        }
        m = [sx / sw, sy / sw];
    }
    m
}

/// A cubic from `p0` (leaving along `out`) to a leg's point (arriving along its heading
/// reversed: the road carries on the way it went), points at least ~60 ft apart (`tidy`
/// drops closer ones).
fn leg_curve(p0: [f64; 2], out: [f64; 2], l: &Leg, cell: f64) -> Vec<[f64; 2]> {
    let p3 = l.3;
    let span = dist(p0, p3);
    let (c1, c2) = ([p0[0] + out[0] * 0.4 * span, p0[1] + out[1] * 0.4 * span], [p3[0] + l.4[0] * 0.4 * span, p3[1] + l.4[1] * 0.4 * span]);
    let n = (span / (0.05 * cell).max(60.0)).ceil().clamp(3.0, 30.0) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            [a * p0[0] + b * c1[0] + c * c2[0] + d * p3[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p3[1]]
        })
        .collect()
}

/// Replace a road's end before its leg point with `curve` (from the new end to the leg point),
/// the level easing from `z_end` there to the road's at the leg point.
fn reshape_end(r: &mut RoadPath, l: &Leg, curve: &[[f64; 2]], z_end: f32) {
    if !l.1 {
        r.pts.reverse();
        r.z.reverse();
        r.wander.reverse();
    }
    let z_leg = r.z[l.2];
    let k = curve.len() - 1;
    let mut pts: Vec<[f64; 2]> = curve[..k].to_vec();
    let mut z: Vec<f32> = (0..k).map(|i| z_end + (z_leg - z_end) * i as f32 / k.max(1) as f32).collect();
    let mut wander = vec![0.0f32; k];
    pts.extend_from_slice(&r.pts[l.2..]);
    z.extend_from_slice(&r.z[l.2..]);
    wander.extend_from_slice(&r.wander[l.2..]);
    (r.pts, r.z, r.wander) = (pts, z, wander);
    if !l.1 {
        r.pts.reverse();
        r.z.reverse();
        r.wander.reverse();
    }
}

/// Where a settlement or junction is.
fn end_key(p: [f64; 2], towns: &[([f64; 2], f64)]) -> (i64, i64) {
    match towns.iter().position(|&(c, r)| dist(c, p) < 1.1 * r + 50.0) {
        Some(t) => (i64::MIN, t as i64),
        None => ((p[0] / 10.0).round() as i64, (p[1] / 10.0).round() as i64),
    }
}

fn road_len(r: &RoadPath) -> f64 {
    r.pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Two roads into the same town from junctions joined by a third make a loop just outside it:
/// a road from junction X into a town goes where the way through a neighbouring junction Y
/// (X–Y, then Y's road into the same town) is under 30% longer.
fn drop_shortcuts(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) {
    let ends: Vec<Option<((i64, i64), (i64, i64))>> = roads.iter().map(|r| (r.pts.len() >= 2).then(|| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns)))).collect();
    let town = |k: (i64, i64)| k.0 == i64::MIN;
    let lens: Vec<f64> = roads.iter().map(road_len).collect();
    // Shortest road between two ends.
    let link = |a: (i64, i64), b: (i64, i64), skip: usize| {
        (0..roads.len()).filter(|&i| i != skip).filter_map(|i| ends[i].filter(|e| (e.0 == a && e.1 == b) || (e.0 == b && e.1 == a)).map(|_| lens[i])).fold(f64::MAX, f64::min)
    };
    let mut gone = vec![false; roads.len()];
    for i in 0..roads.len() {
        let Some((a, b)) = ends[i] else { continue };
        let (x, z) = match (town(a), town(b)) {
            (false, true) => (a, b),
            (true, false) => (b, a),
            _ => continue,
        };
        // Neighbouring junctions Y of X with a road into z.
        let detour = (0..roads.len())
            .filter(|&k| k != i && !gone[k])
            .filter_map(|k| ends[k].and_then(|(p, q)| if p == x && !town(q) { Some((q, lens[k])) } else if q == x && !town(p) { Some((p, lens[k])) } else { None }))
            .map(|(y, xy)| xy + link(y, z, i))
            .fold(f64::MAX, f64::min);
        if detour < 1.3 * lens[i] {
            gone[i] = true;
        }
    }
    let mut i = 0;
    roads.retain(|_| {
        i += 1;
        !gone[i - 1]
    });
}

/// Roads that only meet each other (two ends at a point outside settlements) become one road,
/// the bend where they met rounded (a curve from ~0.3 cell before it to as far after, keeping
/// each side's heading; left as it is where the curve would cross water).
fn join_through(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], cell: f64, dry: &dyn Fn(f64, f64) -> bool) {
    loop {
        let mut at: std::collections::BTreeMap<(i64, i64), Vec<(usize, bool)>> = Default::default();
        for (i, r) in roads.iter().enumerate() {
            if r.pts.len() >= 2 {
                at.entry(end_key(r.pts[0], towns)).or_default().push((i, true));
                at.entry(end_key(*r.pts.last().unwrap(), towns)).or_default().push((i, false));
            }
        }
        let Some((&(i, si), &(k, sk))) = at.iter().filter(|(key, v)| key.0 != i64::MIN && v.len() == 2 && v[0].0 != v[1].0).map(|(_, v)| (&v[0], &v[1])).next() else { break };
        // Road i ends at the point, road k starts there.
        let flip = |r: &RoadPath| RoadPath { class: r.class, pts: r.pts.iter().rev().copied().collect(), z: r.z.iter().rev().copied().collect(), wander: r.wander.iter().rev().copied().collect() };
        let a = if si { flip(&roads[i]) } else { roads[i].clone() };
        let b = if sk { roads[k].clone() } else { flip(&roads[k]) };
        let joint = a.pts.len() - 1;
        let mut joined = RoadPath { class: a.class.min(b.class), pts: a.pts, z: a.z, wander: a.wander };
        joined.pts.extend_from_slice(&b.pts[1..]);
        joined.z.extend_from_slice(&b.z[1..]);
        joined.wander.extend_from_slice(&b.wander[1..]);
        // The ends' wander was tapered away by the curve's ends; mid-road it would jog.
        joined.wander[joint] = 0.0;
        fillet(&mut joined, joint, cell, dry);
        let (lo, hi) = (i.min(k), i.max(k));
        roads.remove(hi);
        roads.remove(lo);
        roads.push(joined);
    }
}

/// Rounds the bend of a road at point `at`: the points within ~0.3 cell (less on a short
/// side) either side become a cubic keeping the headings it arrives and leaves with.
fn fillet(r: &mut RoadPath, at: usize, cell: f64, dry: &dyn Fn(f64, f64) -> bool) {
    let n = r.pts.len();
    if at == 0 || at + 1 >= n {
        return;
    }
    let (u, v) = (unit2([r.pts[at][0] - r.pts[at - 1][0], r.pts[at][1] - r.pts[at - 1][1]]), unit2([r.pts[at + 1][0] - r.pts[at][0], r.pts[at + 1][1] - r.pts[at][1]]));
    if dot2(u, v) > libm::cos(30f64.to_radians()) {
        return;
    }
    // The points `reach` along the road either side (at most halfway to its ends).
    let walk = |dir: i64| {
        let mut i = at as i64;
        let mut acc = 0.0;
        let limit = if dir < 0 { (0..at).map(|k| dist(r.pts[k], r.pts[k + 1])).sum::<f64>() } else { (at..n - 1).map(|k| dist(r.pts[k], r.pts[k + 1])).sum::<f64>() };
        let want = (0.3 * cell).min(0.45 * limit);
        while i + dir >= 0 && ((i + dir) as usize) < n && acc < want {
            acc += dist(r.pts[i as usize], r.pts[(i + dir) as usize]);
            i += dir;
        }
        i as usize
    };
    let (i0, i1) = (walk(-1), walk(1));
    if i0 == 0 && i1 == n - 1 || i0 >= at || i1 <= at {
        return;
    }
    let (p0, p3) = (r.pts[i0], r.pts[i1]);
    // Headings there: along the road into p0, out of p3.
    let h0 = if i0 > 0 { unit2([p0[0] - r.pts[i0 - 1][0], p0[1] - r.pts[i0 - 1][1]]) } else { u };
    let h1 = if i1 + 1 < n { unit2([r.pts[i1 + 1][0] - p3[0], r.pts[i1 + 1][1] - p3[1]]) } else { v };
    let span = dist(p0, p3);
    let (c1, c2) = ([p0[0] + h0[0] * 0.4 * span, p0[1] + h0[1] * 0.4 * span], [p3[0] - h1[0] * 0.4 * span, p3[1] - h1[1] * 0.4 * span]);
    let m = (span / (0.05 * cell).max(60.0)).ceil().clamp(3.0, 30.0) as usize;
    let curve: Vec<[f64; 2]> = (1..m)
        .map(|i| {
            let t = i as f64 / m as f64;
            let w = 1.0 - t;
            let (a, b, c, d) = (w * w * w, 3.0 * w * w * t, 3.0 * w * t * t, t * t * t);
            [a * p0[0] + b * c1[0] + c * c2[0] + d * p3[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p3[1]]
        })
        .collect();
    if curve.iter().any(|p| !dry(p[0], p[1])) {
        return;
    }
    let (z0, z1) = (r.z[i0], r.z[i1]);
    let z: Vec<f32> = (1..m).map(|i| z0 + (z1 - z0) * i as f32 / m as f32).collect();
    r.pts.splice(i0 + 1..i1, curve);
    r.z.splice(i0 + 1..i1, z);
    r.wander.splice(i0 + 1..i1, std::iter::repeat_n(0.0, m - 1));
}

/// Of roads joining the same two ends (the grid's routes can part and meet again; two roads
/// from one junction into the same town, ending at different points of its edge), only the
/// best (class, then shortest) is kept: the other only made a loop.
fn drop_parallel(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) {
    // An end near a settlement (its road ends are trimmed at its edge) is that settlement.
    let key = |p: [f64; 2]| end_key(p, towns);
    let len = road_len;
    let mut best: std::collections::BTreeMap<((i64, i64), (i64, i64)), usize> = Default::default();
    for (i, r) in roads.iter().enumerate() {
        if r.pts.len() < 2 {
            continue;
        }
        let (a, b) = (key(r.pts[0]), key(*r.pts.last().unwrap()));
        if a == b {
            continue;
        }
        let e = best.entry((a.min(b), a.max(b))).or_insert(i);
        let o = &roads[*e];
        if (r.class, len(r)) < (o.class, len(o)) {
            *e = i;
        }
    }
    let keep: std::collections::BTreeSet<usize> = best.values().copied().collect();
    let mut i = 0;
    roads.retain(|r| {
        i += 1;
        let ends = r.pts.len() >= 2 && key(r.pts[0]) != key(*r.pts.last().unwrap());
        !ends || keep.contains(&(i - 1))
    });
}

/// Roads routed on the grid meet at a cell, often at a sharp V: two roads leaving a junction
/// a few tens of degrees apart, so travel between them doubles back. At junctions away from
/// settlements, roads leaving less than 100° apart merge as a Y:
/// - where four or more meet, the two closest merge first into a new stem from the junction
///   (their fork where roads to the junction and to them would be shortest, the stem weighted
///   1.6: they part about 37° either side of its line), until three are left;
/// - of three, the two closest (the branches) merge into the third (the trunk): the junction
///   moves to that fork, the branches curve in along the trunk's line and the trunk leaves
///   along it; where all three leave the same side (no trunk) it moves to where they meet at
///   even angles instead, or (where they fan out from one of them) up that one.
///
/// Points on each road are taken 0.6 cell out, else nearer, until every road reaches the fork
/// head on. Points in ft; a junction stays as it is where the new stretches would cross water
/// or enter a settlement.
fn merge_junctions(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], cell: f64, dry: &dyn Fn(f64, f64) -> bool) {
    let key = |p: [f64; 2]| (p[0].to_bits(), p[1].to_bits());
    // Road ends by junction point: (road, at its start).
    let mut ends: std::collections::BTreeMap<(u64, u64), Vec<(usize, bool)>> = Default::default();
    for (ri, r) in roads.iter().enumerate() {
        if r.pts.len() >= 2 {
            ends.entry(key(r.pts[0])).or_default().push((ri, true));
            ends.entry(key(*r.pts.last().unwrap())).or_default().push((ri, false));
        }
    }
    let fits = |m: [f64; 2], j: [f64; 2]| dist(m, j) > 0.03 * cell && dry(m[0], m[1]) && !towns.iter().any(|&(c, r)| dist(c, m) < r);
    // A road reaches `m` from its leg point without turning back.
    let ahead = |m: [f64; 2], l: &Leg| dot2(l.4, unit2([m[0] - l.3[0], m[1] - l.3[1]])) > 0.3;
    let all_dry = |c: &[[f64; 2]]| c.iter().all(|p| dry(p[0], p[1]));
    for (k, mut list) in ends {
        let j = [f64::from_bits(k.0), f64::from_bits(k.1)];
        if list.len() < 3 || towns.iter().any(|&(t, r)| dist(t, j) < r) {
            continue;
        }
        let z_j = {
            let (ri, start) = list[0];
            if start { roads[ri].z[0] } else { *roads[ri].z.last().unwrap() }
        };
        // Four or more: merge the closest two into a stem, again and again.
        'pairs: while list.len() >= 4 {
            for frac in [0.6, 0.4, 0.25] {
                let legs = legs_at(roads, &list, j, frac * cell);
                let dir = |l: &Leg| unit2([l.3[0] - j[0], l.3[1] - j[1]]);
                let mut best: Option<(f64, usize, usize)> = None;
                for a in 0..legs.len() {
                    for b in a + 1..legs.len() {
                        let c = dot2(dir(&legs[a]), dir(&legs[b]));
                        if best.is_none_or(|x| c > x.0) {
                            best = Some((c, a, b));
                        }
                    }
                }
                let Some((c, b1, b2)) = best else { break 'pairs };
                if c < libm::cos(100f64.to_radians()) {
                    break 'pairs;
                }
                let m = fermat(&[(j, 1.6), (legs[b1].3, 1.0), (legs[b2].3, 1.0)], j);
                let away = unit2([m[0] - j[0], m[1] - j[1]]);
                let ok = fits(m, j)
                    && [b1, b2].iter().all(|&i| ahead(m, &legs[i]) && dot2(unit2([legs[i].3[0] - m[0], legs[i].3[1] - m[1]]), away) > 0.2);
                if !ok {
                    continue;
                }
                let curves = [leg_curve(m, away, &legs[b1], cell), leg_curve(m, away, &legs[b2], cell)];
                let n = (dist(j, m) / (0.05 * cell).max(60.0)).ceil().max(1.0) as usize;
                let stem: Vec<[f64; 2]> = (0..=n).map(|i| [j[0] + (m[0] - j[0]) * i as f64 / n as f64, j[1] + (m[1] - j[1]) * i as f64 / n as f64]).collect();
                if !all_dry(&curves[0]) || !all_dry(&curves[1]) || !all_dry(&stem) {
                    continue;
                }
                for (i, curve) in [b1, b2].iter().zip(&curves) {
                    let l = legs[*i];
                    reshape_end(&mut roads[l.0], &l, curve, z_j);
                }
                let class = roads[legs[b1].0].class.min(roads[legs[b2].0].class);
                roads.push(RoadPath { class, z: vec![z_j; stem.len()], wander: vec![0.0; stem.len()], pts: stem });
                list.retain(|e| *e != (legs[b1].0, legs[b1].1) && *e != (legs[b2].0, legs[b2].1));
                list.push((roads.len() - 1, true));
                continue 'pairs;
            }
            break;
        }
        if list.len() != 3 {
            continue;
        }
        'reach: for frac in [0.6, 0.4, 0.25] {
            let legs = legs_at(roads, &list, j, frac * cell);
            // The branches: the two legs leaving closest together.
            let dir = |l: &Leg| unit2([l.3[0] - j[0], l.3[1] - j[1]]);
            let pairs = [(0, 1, 2), (0, 2, 1), (1, 2, 0)];
            let &(b1, b2, t) = pairs.iter().max_by(|x, y| dot2(dir(&legs[x.0]), dir(&legs[x.1])).total_cmp(&dot2(dir(&legs[y.0]), dir(&legs[y.1])))).unwrap();
            if dot2(dir(&legs[b1]), dir(&legs[b2])) < libm::cos(100f64.to_radians()) {
                break 'reach;
            }
            let weighted = |tw: f64| fermat(&[(legs[t].3, tw), (legs[b1].3, 1.0), (legs[b2].3, 1.0)], j);
            // A Y: each branch comes at the merge point from behind it, along the trunk's line.
            let m = weighted(1.6);
            let along = unit2([legs[t].3[0] - m[0], legs[t].3[1] - m[1]]);
            let y = fits(m, j)
                && [b1, b2].iter().all(|&i| {
                    let to = unit2([m[0] - legs[i].3[0], m[1] - legs[i].3[1]]);
                    dot2(to, along) > 0.2 && ahead(m, &legs[i])
                });
            // Else a fork at even angles, or up the road they fan out from.
            let mut fork: Option<usize> = None;
            let (m, along) = if y {
                (m, Some(along))
            } else {
                let mut m = weighted(1.0);
                fork = (0..3).find(|&i| dist(m, legs[i].3) < 0.08 * cell);
                if let Some(i) = fork {
                    m = legs[i].3;
                }
                if !fits(m, j) || !(0..3).all(|i| Some(i) == fork || ahead(m, &legs[i])) {
                    continue 'reach;
                }
                (m, None)
            };
            let curves: Vec<Vec<[f64; 2]>> = legs
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    if Some(i) == fork {
                        return vec![m];
                    }
                    let out = match along {
                        Some(a) if i == t => a,
                        Some(a) => [-a[0], -a[1]],
                        None => unit2([l.3[0] - m[0], l.3[1] - m[1]]),
                    };
                    leg_curve(m, out, l, cell)
                })
                .collect();
            if !curves.iter().all(|c| all_dry(c)) {
                continue 'reach;
            }
            // The junction's level: the fork road's there, else the old junction's.
            let z_m = match fork {
                Some(i) => {
                    let r = &roads[legs[i].0];
                    if legs[i].1 { r.z[legs[i].2] } else { r.z[r.z.len() - 1 - legs[i].2] }
                }
                None => z_j,
            };
            for (l, curve) in legs.iter().zip(&curves) {
                reshape_end(&mut roads[l.0], l, curve, z_m);
            }
            break;
        }
    }
}

/// Drops control points (ft) that only make the road jog: closer than 40 ft to the point
/// before or after, or where it doubles back (turns over 100°) within 250 ft; never an end,
/// and only where the chord left keeps the road's grade.
pub fn tidy(r: &mut RoadPath) {
    let gmax = r.class.max_grade();
    // The first and last stretch stay as shaped (a Y's merge curve, the way into a gate).
    let mut k = 2;
    while k + 2 < r.pts.len() {
        let (a, b, c) = (r.pts[k - 1], r.pts[k], r.pts[k + 1]);
        let (ab, bc) = (dist(a, b), dist(b, c));
        let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
        let back = u[0] * v[0] + u[1] * v[1] < -0.17 * ab * bc;
        let jog = ab.min(bc) < 40.0 || (back && ab.min(bc) < 250.0);
        if jog && ((r.z[k + 1] - r.z[k - 1]).abs() as f64) <= gmax * dist(a, c) {
            r.pts.remove(k);
            r.z.remove(k);
            r.wander.remove(k);
            k = k.saturating_sub(1).max(2);
        } else {
            k += 1;
        }
    }
}

#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    crate::core::sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]))
}
