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
//! 3b. Junctions away from settlements are squared (`square_junctions`): the straightest pair
//!    of roads runs through, the others meet it square (Ts and crossroads, a fork only where
//!    no pair runs straight), none closer than half a mile to another or to a river crossing.
//! 4. Crossings (bridge / ford / ferry) where roads meet rivers; waystations about every
//!    day's travel along the main roads.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::biome::Biome;
use super::hydro::{Hydro, NO_LAKE};
use super::settle::{Poi, PoiKind, Settlement, Tier};
use crate::World;
use crate::world::SketchTool;
use crate::core::grid::Grid;
use crate::core::noise::{gradient2, smoothstep};

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
    /// The sketch's road stroke it follows, if drawn.
    pub stroke: Option<u32>,
    /// Part of an isolated realm's own track (its way out): where its villages' tracks meet it, it is the through road.
    pub spine: bool,
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
    /// Per stretch between junctions as routed: its traffic and class.
    pub traffic: Vec<SegTraffic>,
    /// Each isolated realm's routes as found (its town's index, the cells' centres in ft): its
    /// track, then its villages'.
    pub realm_paths: Vec<(usize, Vec<[f64; 2]>)>,
}

/// A stretch's class as planned and as built, the most goods it carries (`economy::Traffic`),
/// its length and a point on it (ft).
#[derive(Clone, Debug)]
pub struct SegTraffic {
    pub was: RoadClass,
    pub now: RoadClass,
    pub flow: f32,
    pub length_ft: f64,
    pub at: [f64; 2],
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
    /// The sketch's roads (`drawn`).
    pub drawn: Vec<Drawn>,
    /// Their routes, found once (`Inputs::pre`).
    pub pre: std::cell::OnceCell<DrawnRoutes>,
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
    /// The rivers as drawn (their meander belts: where roads cross them).
    pub rivers: &'a crate::lod::rivers::RiverNet,
    /// The routes `route` found last time (see `RouteKey`), for the next call to reuse.
    pub routes: std::cell::RefCell<Vec<(RouteKey, Option<Vec<u32>>)>>,
    /// What `route` needs of the inputs alone, built on its first call.
    pub fixed: std::cell::OnceCell<RouteFixed>,
}

/// The per-cell tables `route` reads, the same for every call on one `Inputs`.
pub struct RouteFixed {
    costs: Costs,
    /// Open ground, and the same less what `none` lines close (a link's own end stays open).
    free: Vec<bool>,
    open: Vec<bool>,
    /// Smooth cost noise: equal-cost ties on open ground resolve into gentle meanders instead
    /// of grid-aligned runs.
    wobble: Vec<f32>,
}

impl Inputs<'_> {
    /// The drawn roads' routes (found on first use; they depend on no settlement).
    pub fn pre(&self) -> &DrawnRoutes {
        self.pre.get_or_init(|| drawn_routes(self))
    }
}

/// A road drawn in the sketch (`SketchTool::Road`), in T0 cells: a road along its line, or
/// (`class` None) a line no planned road crosses.
#[derive(Clone, Debug)]
pub struct Drawn {
    pub stroke: u32,
    pub class: Option<RoadClass>,
    pub pts: Vec<[f64; 2]>,
}

/// The sketch's roads, in cells of `cell` ft on a `w` × `h` grid.
pub fn drawn(world: &World, w: usize, h: usize, cell: f64) -> Vec<Drawn> {
    let strokes = world.file.sketch.strokes.iter().enumerate().filter(|(_, s)| s.tool == SketchTool::Road && s.pts.len() >= 2);
    strokes
        .map(|(i, s)| Drawn {
            stroke: i as u32,
            class: match s.kind.as_deref() {
                Some("kings_road") => Some(RoadClass::KingsRoad),
                Some("track") => Some(RoadClass::Track),
                Some("none") => None,
                _ => Some(RoadClass::Road),
            },
            pts: s.pts.iter().map(|p| [(p[0] / cell).clamp(0.0, (w - 1) as f64), (p[1] / cell).clamp(0.0, (h - 1) as f64)]).collect(),
        })
        .collect()
}

/// Settlements this close to a drawn road (mi) join it: at its ends it runs on to them,
/// beside it a side road leaves it for them; planned links between two of them are dropped.
const SNAP_MI: f64 = 3.0;
/// A drawn road is routed through waypoints this far apart along its line (cells), each leg
/// within `CORRIDOR` cells of the line (else `CORRIDOR_WIDE`, else `CORRIDOR_ROUND`: round
/// water in the way), its cost growing with the square of the distance from it (`DEVIATE` per
/// cell², less in the wider corridors).
const WAYPOINT: f64 = 6.0;
const CORRIDOR: f64 = 2.5;
const CORRIDOR_WIDE: f64 = 8.0;
const CORRIDOR_ROUND: f64 = 40.0;
const DEVIATE: f64 = 2.0;

/// The drawn roads' routes on the T0 grid and the cells their barriers close.
#[derive(Default)]
pub struct DrawnRoutes {
    /// Per drawn road, its cells (in pieces where it can't be followed): class, cells, stroke.
    pub routes: Vec<(RoadClass, Vec<u32>, u32)>,
    /// Cells planned roads may not enter (within about a cell of a `none` line); empty when
    /// there is none.
    pub blocked: Vec<bool>,
    pub conflicts: Vec<super::sketch::Conflict>,
}

/// One road to route: between two settlements (`End::Town`) or a settlement and a cell.
#[derive(Clone, Copy, Debug)]
pub enum End {
    Town(usize),
    Cell(usize),
    /// The nearest cell of the roads routed so far outside any isolated realm (a realm's track
    /// goes there: `route`).
    Net,
    /// The nearest cell of this realm's (settlement index) track outside its town, which its
    /// hidden villages join.
    Track(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub a: End,
    pub b: End,
    pub class: RoadClass,
    /// A drawn road's stroke (where it runs on to a settlement at its end).
    pub stroke: Option<u32>,
}

/// A route found: its class, cells and drawn road's stroke.
pub type Routed = (RoadClass, Vec<u32>, Option<u32>);

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

pub fn build(inp: &Inputs, settlements: &[Settlement], traffic: &super::economy::Traffic) -> Network {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let p = inp.world.params();
    let river_q = super::hydro::river_q(p.river_density);

    let edges = links(inp, settlements, &[RoadClass::KingsRoad, RoadClass::Road, RoadClass::Track]);
    let (_, paths) = route(inp, settlements, &edges);
    // (A realm's routes are the only ones starting at an isolated place: `links`.)
    let realm_paths: Vec<(usize, Vec<[f64; 2]>)> = paths
        .iter()
        .filter_map(|(_, path, _)| {
            let first = *path.first()?;
            let i = settlements.iter().position(|s| s.isolated && s.cell as u32 == first)?;
            Some((realm_of(settlements, i)?, path.iter().map(|&c| [(c as usize % w) as f64 * cell, (c as usize / w) as f64 * cell]).collect()))
        })
        .collect();

    // The steps of each isolated realm's own track (its way out, from the realm's town).
    let spine_steps: std::collections::BTreeSet<(u32, u32)> = paths
        .iter()
        .filter(|(_, path, _)| path.first().is_some_and(|&f| settlements.iter().any(|s| s.isolated && s.planned >= Tier::Town && s.cell as u32 == f)))
        .flat_map(|(_, path, _)| path.windows(2).map(|p| (p[0].min(p[1]), p[0].max(p[1]))))
        .collect();

    // --- 3. Segments between junctions (and settlements), from the union of routes: per
    // step, the best class over it and the drawn road it is part of (the first).
    let mut links: std::collections::BTreeMap<(u32, u32), (u8, Option<u32>)> = Default::default();
    for (class, path, stroke) in &paths {
        for pair in path.windows(2) {
            let key = (pair[0].min(pair[1]), pair[0].max(pair[1]));
            let e = links.entry(key).or_insert((*class as u8, *stroke));
            e.0 = e.0.min(*class as u8);
            e.1 = e.1.or(*stroke);
        }
    }
    let mut adj: std::collections::BTreeMap<u32, Vec<u32>> = Default::default();
    for &(a, b) in links.keys() {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    // (Where a drawn road begins or ends on another road is a node too.)
    let mut at_cell: crate::core::hash::FastMap<usize, &Settlement> = Default::default();
    let mut town_at_cell: crate::core::hash::FastMap<usize, &Settlement> = Default::default();
    for s in settlements {
        at_cell.entry(s.cell).or_insert(s);
        if s.tier >= Tier::Town {
            town_at_cell.entry(s.cell).or_insert(s);
        }
    }
    let step_stroke = |a: u32, b: u32| links[&(a.min(b), a.max(b))].1;
    let is_node = |c: u32, adj: &std::collections::BTreeMap<u32, Vec<u32>>| adj[&c].len() != 2 || at_cell.contains_key(&(c as usize)) || step_stroke(c, adj[&c][0]) != step_stroke(c, adj[&c][1]);
    let mut used: std::collections::BTreeSet<(u32, u32)> = Default::default();
    let mut segments: Vec<(RoadClass, Vec<u32>, Option<u32>)> = Vec::new();
    let mut traffic_out: Vec<SegTraffic> = Vec::new();
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
            let (mut class, stroke) = links[&key];
            let (mut prev, mut cur) = (start, first);
            while !is_node(cur, &adj) {
                let next = *adj[&cur].iter().find(|&&x| x != prev).unwrap();
                let k2 = (cur.min(next), cur.max(next));
                if used.contains(&k2) {
                    break;
                }
                used.insert(k2);
                class = class.min(links[&k2].0);
                seg.push(next);
                prev = cur;
                cur = next;
            }
            // (Classed below, from the stretch's traffic as a whole.)
            let flow = seg.windows(2).map(|p| traffic.flow.get(&(p[0].min(p[1]), p[0].max(p[1]))).copied().unwrap_or(0.0)).fold(0.0f32, f32::max);
            let length = seg.windows(2).map(|p| dist([(p[0] as usize % w) as f64, (p[0] as usize / w) as f64], [(p[1] as usize % w) as f64, (p[1] as usize / w) as f64])).sum::<f64>() * cell;
            let mid = seg[seg.len() / 2] as usize;
            traffic_out.push(SegTraffic { was: RoadClass::from_u8(class), now: RoadClass::from_u8(class), flow, length_ft: length, at: [(mid % w) as f64 * cell, (mid / w) as f64 * cell] });
            segments.push((RoadClass::from_u8(class), seg, stroke));
        }
    }
    // Each stretch's class as built, one from node to node (a drawn road keeps its own). The routes
    // went by the planned tiers and classes, so this moves none of them; the classes follow the
    // tiers now (after the economy) and the traffic: the king's roads are the ways along the
    // stretches between the cities as they now stand (each pair `link_edges` would join), and any
    // other stretch as busy as a king's road; a quiet one drops (`economy::Traffic::class`).
    {
        let city_at = |c: u32| at_cell.get(&(c as usize)).is_some_and(|s| s.tier >= Tier::City);
        let mut kings = vec![false; segments.len()];
        let mut nodes: Vec<u32> = segments.iter().flat_map(|(_, seg, _)| [seg[0], seg[seg.len() - 1]]).collect();
        nodes.sort_unstable();
        nodes.dedup();
        let id = |c: u32| nodes.binary_search(&c).expect("a stretch end");
        let mut sadj: Vec<Vec<(usize, f64, usize)>> = vec![Vec::new(); nodes.len()];
        for (i, (class, seg, _)) in segments.iter().enumerate() {
            let c = traffic_out[i].length_ft * [0.8, 1.0, 1.2][*class as usize];
            let (a, b) = (id(seg[0]), id(seg[seg.len() - 1]));
            sadj[a].push((b, c, i));
            sadj[b].push((a, c, i));
        }
        let cities: Vec<(usize, &Settlement)> = settlements.iter().enumerate().filter(|(_, s)| s.tier >= Tier::City).collect();
        let by_tier: Vec<Settlement> = settlements.iter().map(|s| Settlement { planned: s.tier, ..s.clone() }).collect();
        let pairs = if cities.len() >= 2 { link_edges(&by_tier, &[RoadClass::KingsRoad], w, cell) } else { Vec::new() };
        let mut src_of: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        for &(a, b, _) in &pairs {
            src_of.entry(a).or_default().push(b);
        }
        let mut cost = vec![f64::INFINITY; nodes.len()];
        let mut via = vec![usize::MAX; nodes.len()];
        for (&a, targets) in &src_of {
            let Ok(sa) = nodes.binary_search(&(settlements[a].cell as u32)) else { continue };
            cost.iter_mut().for_each(|c| *c = f64::INFINITY);
            via.iter_mut().for_each(|v| *v = usize::MAX);
            let mut heap = BinaryHeap::new();
            cost[sa] = 0.0;
            heap.push(Node { f: 0.0, i: sa as u32 });
            while let Some(Node { f: c, i: v }) = heap.pop() {
                let v = v as usize;
                if c > cost[v] {
                    continue;
                }
                for &(u, e, si) in &sadj[v] {
                    let nc = c + e;
                    if nc < cost[u] {
                        cost[u] = nc;
                        via[u] = si;
                        heap.push(Node { f: nc, i: u as u32 });
                    }
                }
            }
            for &b in targets {
                let Ok(mut v) = nodes.binary_search(&(settlements[b].cell as u32)) else { continue };
                if !cost[v].is_finite() {
                    continue;
                }
                while v != sa {
                    let si = via[v];
                    kings[si] = true;
                    let seg = &segments[si].1;
                    v = if id(seg[0]) == v { id(seg[seg.len() - 1]) } else { id(seg[0]) };
                }
            }
        }
        for (i, (class, _, stroke)) in segments.iter_mut().enumerate() {
            if stroke.is_none() {
                *class = if kings[i] { RoadClass::KingsRoad } else { traffic.class(*class, traffic_out[i].flow) };
            }
            traffic_out[i].now = *class;
        }
        // A king's road leads somewhere: each end at a city or up or where another king's road goes
        // on. One that ends anywhere else (a busy stretch between two roads, the way to a city that
        // shrank) is built a road; pruned until none is left, so the order of the stretches changes
        // nothing. (A drawn one keeps its class and counts as a king's road.)
        loop {
            let mut ends: crate::core::hash::FastMap<u32, u32> = Default::default();
            for (class, seg, _) in &segments {
                if *class == RoadClass::KingsRoad {
                    *ends.entry(seg[0]).or_default() += 1;
                    *ends.entry(seg[seg.len() - 1]).or_default() += 1;
                }
            }
            let mut changed = false;
            for (i, (class, seg, stroke)) in segments.iter_mut().enumerate() {
                let loose = |c: u32| ends[&c] < 2 && !city_at(c);
                if *class == RoadClass::KingsRoad && stroke.is_none() && (loose(seg[0]) || loose(seg[seg.len() - 1])) {
                    *class = RoadClass::Road;
                    traffic_out[i].now = RoadClass::Road;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
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
    for (class, seg, stroke) in &segments {
        let mut cells: Vec<[f64; 2]> = seg.iter().map(|&c| [(c as usize % w) as f64, (c as usize / w) as f64]).collect();
        // Ends at a settlement meet it where it actually stands.
        for (end, c) in [(0, seg[0]), (cells.len() - 1, seg[seg.len() - 1])] {
            if let Some(&s) = at_cell.get(&(c as usize)) {
                cells[end] = [s.x / cell, s.y / cell];
            }
        }
        // A drawn road keeps to its line wherever that is dry (the cells only found its way).
        let chords = match stroke.and_then(|st| inp.drawn.iter().find(|d| d.stroke == st)) {
            Some(d) => along_drawn(&cells, &d.pts, &|p| fine_dry(p), &|run| simplify(run, 1.0, w, h, hgrid, class.max_grade(), cell, passable_fn(inp))),
            None => simplify(&cells, 2.5, w, h, hgrid, class.max_grade(), cell, passable_fn(inp)),
        };
        let (mut pts, mut wander) = switchbacks(hgrid, &chords, class.max_grade(), cell, passable_fn(inp), &fine_ground, &fine_dry);
        // Towns and cities are entered at their urban edge (gates); the streets take over.
        for at_start in [true, false] {
            let c = if at_start { seg[0] } else { seg[seg.len() - 1] };
            if let Some(&s) = town_at_cell.get(&(c as usize)) {
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
        roads.push(RoadPath { class: *class, pts: pts.iter().map(|p| [p[0] * cell, p[1] * cell]).collect(), z, wander, stroke: *stroke, spine: seg.len() >= 2 && seg.windows(2).all(|p| spine_steps.contains(&(p[0].min(p[1]), p[0].max(p[1])))) });
    }

    // Junctions outside settlements become Ts and crossroads, clear of river crossings.
    let dry_ft = |x: f64, y: f64| {
        let (i, j) = (crate::core::round(x / cell) as i64, crate::core::round(y / cell) as i64);
        i >= 0 && j >= 0 && (i as usize) < w && (j as usize) < h && passable_fn(inp)(j as usize * w + i as usize)
    };
    // Settlements and how far round them a junction stays put (towns' roads end at their edge).
    let towns: Vec<([f64; 2], f64)> = settlements.iter().map(|s| ([s.x, s.y], crate::town::road_trim_radius(s.tier, s.population).max(0.15 * cell))).collect();
    drop_parallel(&mut roads, &towns);
    drop_shortcuts(&mut roads, &towns);
    join_through(&mut roads, &towns, cell, &dry_ft);
    let ground_ft = |p: [f64; 2]| hgrid.z([p[0] / cell, p[1] / cell]);
    let belts = crate::lod::roads::BeltCache::default();
    let crossings = |line: &[[f64; 2]]| crate::lod::roads::belt_crossings(line, inp.rivers, cell, &belts);
    square_junctions(&mut roads, &towns, cell, &dry_ft, &ground_ft, &crossings, None);
    let cut = end_at_crossing(&mut roads, &towns);
    if !cut.is_empty() {
        square_junctions(&mut roads, &towns, cell, &dry_ft, &ground_ft, &crossings, Some(&cut));
    }
    drop_parallel(&mut roads, &towns);
    join_through(&mut roads, &towns, cell, &dry_ft);
    if drop_lenses(&mut roads, &towns, &ground_ft, &dry_ft, &crossings) {
        join_through(&mut roads, &towns, cell, &dry_ft);
    }
    // (Squaring can lay a branch along another leaving the same junction: one road out to where they part.)
    unshare(&mut roads, &towns, &ground_ft, &dry_ft, &crossings, false);
    join_through(&mut roads, &towns, cell, &dry_ft);
    while drop_triangle(&mut roads, &towns) {
        join_through(&mut roads, &towns, cell, &dry_ft);
    }
    let small: Vec<bool> = settlements.iter().map(|s| s.tier == crate::t0::settle::Tier::Village).collect();
    while drop_twin_link(&mut roads, &towns, &small) {
        join_through(&mut roads, &towns, cell, &dry_ft);
    }
    // (A road dropped above can leave two along one line out of where it met them.)
    unshare(&mut roads, &towns, &ground_ft, &dry_ft, &crossings, false);
    join_through(&mut roads, &towns, cell, &dry_ft);

    // --- 4. Crossings and waystations.
    let mut crossings: Vec<Crossing> = Vec::new();
    for (class, seg, _) in &segments {
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
    // Each inn seeded by where it stands, so one road more or less elsewhere changes none.
    let stream = inp.world.stream("t0.waystation");
    let mut waystations: Vec<Poi> = Vec::new();
    let day = 24.0 * 5280.0;
    // None with generated roads off (roadside inns go in by hand there).
    let generated = inp.world.file.params.generated_roads;
    for r in roads.iter().filter(|r| generated && r.class == RoadClass::KingsRoad) {
        let mut acc = 0.0;
        for pair in r.pts.windows(2) {
            acc += dist(pair[0], pair[1]);
            if acc >= day {
                acc = 0.0;
                let (x, y) = (pair[1][0], pair[1][1]);
                let far_from_town = settlements.iter().all(|s| crate::core::sqrt((s.x - x) * (s.x - x) + (s.y - y) * (s.y - y)) > 10.0 * 5280.0);
                let far_from_other = waystations.iter().all(|q| crate::core::sqrt((q.x - x) * (q.x - x) + (q.y - y) * (q.y - y)) > 15.0 * 5280.0);
                if far_from_town && far_from_other {
                    waystations.push(Poi { kind: PoiKind::Waystation, x, y, seed: crate::core::rng::hash2(stream, (x / 10.0) as i64, (y / 10.0) as i64), stroke: None });
                }
            }
        }
    }
    Network { roads, crossings, waystations, traffic: traffic_out, realm_paths }
}

/// Settlement pairs to join, per class: king's roads join cities, roads join towns and up,
/// tracks join everything. Each class is a relative-neighbourhood graph (no third place
/// closer to both ends) plus a minimum spanning tree so its members stay connected.
fn link_edges(settlements: &[Settlement], classes: &[RoadClass], w: usize, cell: f64) -> Vec<(usize, usize, RoadClass)> {
    let ns = settlements.len();
    let d = |a: usize, b: usize| {
        // On the settlements' cells, not where they stand: snapping to water moves a settlement after
        // the previews, which must not reorder the links (the route memo is keyed on their order).
        let (ca, cb) = (settlements[a].cell, settlements[b].cell);
        let (dx, dy) = (((ca % w) as f64 - (cb % w) as f64) * cell, ((ca / w) as f64 - (cb / w) as f64) * cell);
        crate::core::sqrt(dx * dx + dy * dy)
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
        // (By the tiers they were planned at: the economy's re-tiering routes nothing again.)
        // (An isolated realm's places get none: `links` gives them their track.)
        let members: Vec<usize> = (0..ns).filter(|&i| settlements[i].planned >= min_tier && !settlements[i].isolated).collect();
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
        // (A town or city farther than that from any other still has a road, to the nearest.)
        if class != RoadClass::Track {
            for &a in &members {
                if edges.iter().any(|e| e.0 == a || e.1 == a) {
                    continue;
                }
                if let Some(&b) = members.iter().filter(|&&b| b != a).min_by(|&&x, &&y| d(a, x).total_cmp(&d(a, y)).then(x.cmp(&y))) {
                    add(a, b, &mut edges);
                }
            }
        }
    }

    edges
}

/// The links to route for these classes: the planned ones (`link_edges`) less those a drawn
/// road makes (both ends beside the same one) or a `none` line cuts, then a link from every
/// settlement beside a drawn road to it (at the road's end it carries the road on).
pub fn links(inp: &Inputs, settlements: &[Settlement], classes: &[RoadClass]) -> Vec<Link> {
    let cell = inp.cell_ft;
    let pre = inp.pre();
    let snap = SNAP_MI * 5280.0 / cell;
    let at = |s: &Settlement| [s.x / cell, s.y / cell];
    // The drawn roads each settlement lies beside.
    let beside: Vec<Vec<usize>> = settlements.iter().map(|s| inp.drawn.iter().enumerate().filter(|(_, d)| d.class.is_some() && line_dist(at(s), &d.pts) <= snap).map(|(i, _)| i).collect()).collect();
    let barriers: Vec<&Drawn> = inp.drawn.iter().filter(|d| d.class.is_none()).collect();
    // With generated roads off, only the drawn roads and their spurs.
    let planned = if inp.world.file.params.generated_roads { link_edges(settlements, classes, inp.w, inp.cell_ft) } else { Vec::new() };
    let mut out: Vec<Link> = planned
        .into_iter()
        .filter(|&(a, b, _)| !beside[a].iter().any(|d| beside[b].contains(d)) && !barriers.iter().any(|d| crosses(at(&settlements[a]), at(&settlements[b]), &d.pts)))
        .map(|(a, b, class)| Link { a: End::Town(a), b: End::Town(b), class, stroke: None })
        .collect();
    let w = inp.w;
    for (si, s) in settlements.iter().enumerate() {
        for &di in &beside[si] {
            let d = &inp.drawn[di];
            let near = |k: u32| {
                let (dx, dy) = ((k as usize % w) as f64 - (s.cell % w) as f64, (k as usize / w) as f64 - (s.cell / w) as f64);
                dx * dx + dy * dy
            };
            let cells = pre.routes.iter().filter(|r| r.2 == d.stroke).flat_map(|r| r.1.iter().copied());
            let Some(c) = cells.min_by(|&a, &b| near(a).total_cmp(&near(b)).then(a.cmp(&b))) else { continue };
            if c as usize == s.cell {
                continue;
            }
            let class = d.class.unwrap_or(RoadClass::Road);
            // At its ends the drawn road runs on into the settlement; beside it, a side road of
            // the settlement's own class (no better than the road it leaves) goes to it.
            let end = [d.pts[0], d.pts[d.pts.len() - 1]].iter().any(|&p| dist(p, at(s)) <= snap);
            let own = match s.planned {
                Tier::Village => RoadClass::Track,
                Tier::Town => RoadClass::Road,
                _ => RoadClass::KingsRoad,
            };
            let (class, stroke) = if end { (class, Some(d.stroke)) } else { (class.max(own), None) };
            out.push(Link { a: End::Town(si), b: End::Cell(c as usize), class, stroke });
        }
    }
    // An isolated realm's one track, last (to the nearest road the others make), then its hidden
    // villages' tracks to it. (None with generated roads off; none where a drawn road reaches it.)
    if inp.world.file.params.generated_roads && classes.contains(&RoadClass::Track) {
        for (ri, r) in settlements.iter().enumerate() {
            if !r.isolated || r.planned < Tier::Town || !beside[ri].is_empty() {
                continue;
            }
            out.push(Link { a: End::Town(ri), b: End::Net, class: RoadClass::Track, stroke: None });
            for (vi, v) in settlements.iter().enumerate() {
                if v.isolated && v.planned == Tier::Village && realm_of(settlements, vi) == Some(ri) {
                    out.push(Link { a: End::Town(vi), b: End::Track(ri), class: RoadClass::Track, stroke: None });
                }
            }
        }
    }
    out
}

/// How far apart (cells) two of a realm's villages' tracks join its track: near, so that each has a short
/// spur of its own rather than a far join the track gap between two others would force.
const JOIN_APART: f64 = 0.9;

/// How far (cells) round a realm's town its villages' tracks keep off, joining its track beyond.
fn realm_keep_out(s: &Settlement, cell: f64) -> f64 {
    (crate::town::road_trim_radius(s.planned, s.planned_pop) + 1.5 * 5280.0) / cell + 2.0
}

/// The realm (its town's index) an isolated village belongs to: the nearest isolated town.
pub fn realm_of(settlements: &[Settlement], i: usize) -> Option<usize> {
    let s = &settlements[i];
    if !s.isolated {
        return None;
    }
    if s.planned >= Tier::Town {
        return Some(i);
    }
    let d = |r: &Settlement| (r.x - s.x) * (r.x - s.x) + (r.y - s.y) * (r.y - s.y);
    (0..settlements.len()).filter(|&r| settlements[r].isolated && settlements[r].planned >= Tier::Town).min_by(|&a, &b| d(&settlements[a]).total_cmp(&d(&settlements[b])).then(a.cmp(&b)))
}

/// Distance (cells) from a point to a polyline.
fn line_dist(p: [f64; 2], line: &[[f64; 2]]) -> f64 {
    line.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}

/// Whether `p` is within `tol` of the polyline (`line_dist(p, line) < tol`, skipping segments
/// whose box is farther off).
fn near_line(p: [f64; 2], line: &[[f64; 2]], tol: f64) -> bool {
    line.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        p[0] >= a[0].min(b[0]) - tol && p[0] <= a[0].max(b[0]) + tol && p[1] >= a[1].min(b[1]) - tol && p[1] <= a[1].max(b[1]) + tol && seg_dist(p, a, b) < tol
    })
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    dist(p, [a[0] + t * dx, a[1] + t * dy])
}

/// Whether segment a–b crosses the polyline.
fn crosses(a: [f64; 2], b: [f64; 2], line: &[[f64; 2]]) -> bool {
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    line.windows(2).any(|w| side(a, b, w[0]) * side(a, b, w[1]) < 0.0 && side(w[0], w[1], a) * side(w[0], w[1], b) < 0.0)
}

/// 16 directions (knight moves too) so paths are not limited to 45° headings.
const DIRS: [(i64, i64); 16] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)];

/// A* on the T0 grid, its tables kept between searches.
struct Search {
    g: Vec<f64>,
    came: Vec<u32>,
    touched: Vec<usize>,
}

impl Search {
    fn new(n: usize) -> Search {
        Search { g: vec![f64::INFINITY; n], came: vec![u32::MAX; n], touched: Vec::new() }
    }

    /// The cheapest path from `start` to `goal` within the box `[x0, x1, y0, y1]` (cells), over
    /// cells that are `open` (a knight move passes between two cells, which must be open too);
    /// a step costs its length times `cost(from, to, the cells it passes, its length)`. Gives up
    /// after 400,000 cells.
    fn run(&mut self, w: usize, start: usize, goal: usize, b: [i64; 4], open: impl Fn(usize) -> bool, cost: impl Fn(usize, usize, &[usize; 2], f64) -> f64) -> Option<Vec<u32>> {
        let step: [f64; 16] = std::array::from_fn(|d| crate::core::sqrt((DIRS[d].0 * DIRS[d].0 + DIRS[d].1 * DIRS[d].1) as f64));
        let [bx0, bx1, by0, by1] = b;
        let (g, came) = (&mut self.g, &mut self.came);
        for &k in &self.touched {
            g[k] = f64::INFINITY;
            came[k] = u32::MAX;
        }
        self.touched.clear();
        let (gx, gy) = ((goal % w) as f64, (goal / w) as f64);
        let mut heap = BinaryHeap::new();
        g[start] = 0.0;
        self.touched.push(start);
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
                if !open(nb) {
                    continue;
                }
                // Knight moves pass between two cells; both must be passable, and a river in
                // either still has to be crossed.
                let mids: [usize; 2] = if dx.abs() == 2 || dy.abs() == 2 {
                    let (sx, sy) = (dx.signum(), dy.signum());
                    let (ax, ay) = if dx.abs() == 2 { (ix + sx, iy) } else { (ix, iy + sy) };
                    [ay as usize * w + ax as usize, (ay + if dx.abs() == 2 { sy } else { 0 }) as usize * w + (ax + if dy.abs() == 2 { sx } else { 0 }) as usize]
                } else {
                    [nb, nb]
                };
                if !mids.iter().all(|&m| open(m)) {
                    continue;
                }
                let ng = g[i] + step[d] * cost(i, nb, &mids, step[d]);
                if ng < g[nb] {
                    if g[nb] == f64::INFINITY {
                        self.touched.push(nb);
                    }
                    g[nb] = ng;
                    came[nb] = i as u32;
                    let hdist = crate::core::sqrt((nx as f64 - gx) * (nx as f64 - gx) + (ny as f64 - gy) * (ny as f64 - gy));
                    heap.push(Node { f: ng + 0.35 * hdist, i: nb as u32 });
                }
            }
        }
        found.then(|| {
            let mut path = vec![goal as u32];
            let mut cur = goal;
            while cur != start {
                cur = came[cur] as usize;
                path.push(cur as u32);
            }
            path.reverse();
            path
        })
    }
}

/// What a step costs per cell of its length: the grade (steeply past 15%) and the ground's
/// biome, and crossing a mapped river.
struct Costs {
    pen: Vec<f64>,
    river_q: f64,
}

impl Costs {
    fn new(inp: &Inputs) -> Costs {
        let biome_pen = |k: usize| match Biome::from_u8((inp.biome[k] & 0xff) as u8) {
            Biome::Swamp => 2.0,
            Biome::Jungle | Biome::Blight => 1.0,
            Biome::Alpine => 1.0,
            Biome::Ice => 3.0,
            Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga => 0.3,
            Biome::HotDesert | Biome::ColdDesert => 0.3,
            _ => 0.0,
        };
        Costs { pen: (0..inp.w * inp.h).map(biome_pen).collect(), river_q: super::hydro::river_q(inp.world.params().river_density) }
    }

    fn ground(&self, inp: &Inputs, i: usize, nb: usize, dist: f64) -> f64 {
        let grade = (inp.height[nb] - inp.height[i]).abs() / (dist * inp.cell_ft);
        1.0 + 25.0 * grade * grade + 40.0 * (grade - 0.15).max(0.0) + self.pen[nb]
    }

    /// Stepping onto a mapped river from a cell that is not part of it (0 otherwise).
    fn river(&self, inp: &Inputs, i: usize, nb: usize, mids: &[usize; 2]) -> f64 {
        let q = mids.iter().chain(std::iter::once(&nb)).map(|&m| inp.hydro.discharge[m]).fold(0f32, f32::max) as f64;
        if q >= self.river_q && (inp.hydro.discharge[i] as f64) < q { 1.0 + 3.0 * (q / (self.river_q * 20.0)).min(1.0) } else { 0.0 }
    }
}

/// The drawn roads' routes: through waypoints along each line, each leg the cheapest way
/// within `CORRIDOR` cells of its stretch of the line (costing more the farther from it); where
/// water closes that, within `CORRIDOR_WIDE`, else round it (a conflict says so); else (no way
/// by land) the road breaks there (a conflict). Then the cells the `none` lines close.
fn drawn_routes(inp: &Inputs) -> DrawnRoutes {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let mut out = DrawnRoutes::default();
    if inp.drawn.is_empty() {
        return out;
    }
    let open: Vec<bool> = (0..w * h).map(|k| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE).collect();
    let costs = Costs::new(inp);
    let mut search = Search::new(w * h);
    // The open cell nearest a point within `r` cells.
    let nearest_open = |p: [f64; 2], r: f64| {
        let mut best: Option<(f64, usize)> = None;
        let (i0, i1) = ((p[0] - r).floor().max(0.0) as usize, ((p[0] + r).ceil() as usize).min(w - 1));
        let (j0, j1) = ((p[1] - r).floor().max(0.0) as usize, ((p[1] + r).ceil() as usize).min(h - 1));
        for j in j0..=j1 {
            for i in i0..=i1 {
                let (k, d) = (j * w + i, dist(p, [i as f64, j as f64]));
                if open[k] && d <= r && best.is_none_or(|b| d < b.0) {
                    best = Some((d, k));
                }
            }
        }
        best.map(|b| b.1)
    };
    for d in &inp.drawn {
        let Some(class) = d.class else { continue };
        let s = arc(&d.pts);
        let total = *s.last().unwrap();
        let m = (total / WAYPOINT).ceil().max(1.0) as usize;
        // Waypoints: (place along the line, cell); the ends may look farther for land.
        let mut ways: Vec<(f64, usize)> = Vec::new();
        for i in 0..=m {
            let t = total * i as f64 / m as f64;
            let r = if i == 0 || i == m { 4.0 } else { 2.0 };
            if let Some(k) = nearest_open(point_at(&d.pts, &s, t), r)
                && ways.last().is_none_or(|l| l.1 != k)
            {
                ways.push((t, k));
            }
        }
        let mut piece: Vec<u32> = Vec::new();
        for pair in ways.windows(2) {
            let ((t0, a), (t1, b)) = (pair[0], pair[1]);
            let line = sub_line(&d.pts, &s, t0 - 1.0, t1 + 1.0);
            let mut leg = None;
            for (attempt, (reach, deviate)) in [(CORRIDOR, DEVIATE), (CORRIDOR_WIDE, 0.25 * DEVIATE), (CORRIDOR_ROUND, 0.01 * DEVIATE)].into_iter().enumerate() {
                let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for p in &line {
                    (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
                }
                let bx = [(x0 - reach).floor().max(0.0) as i64, ((x1 + reach).ceil() as i64).min(w as i64 - 1), (y0 - reach).floor().max(0.0) as i64, ((y1 + reach).ceil() as i64).min(h as i64 - 1)];
                // Distance to the line over the box.
                let bw = (bx[1] - bx[0] + 1) as usize;
                let off: Vec<f64> = (0..bw * (bx[3] - bx[2] + 1) as usize).map(|q| line_dist([(bx[0] + (q % bw) as i64) as f64, (bx[2] + (q / bw) as i64) as f64], &line)).collect();
                let off_at = |k: usize| off[((k / w) as i64 - bx[2]) as usize * bw + ((k % w) as i64 - bx[0]) as usize];
                let ok = |k: usize| open[k] && off_at(k) <= reach;
                leg = search.run(w, a, b, bx, ok, |i, nb, mids, run| costs.ground(inp, i, nb, run) + costs.river(inp, i, nb, mids) + deviate * off_at(nb) * off_at(nb));
                if leg.is_some() {
                    if attempt == 2 {
                        let p = point_at(&d.pts, &s, 0.5 * (t0 + t1));
                        out.conflicts.push(super::sketch::Conflict { stroke: d.stroke as usize, message: "This road crosses water here: it goes round".into(), x: p[0] * cell, y: p[1] * cell });
                    }
                    break;
                }
            }
            match leg {
                Some(leg) => piece.extend_from_slice(if piece.is_empty() { &leg } else { &leg[1..] }),
                None => {
                    let p = point_at(&d.pts, &s, 0.5 * (t0 + t1));
                    out.conflicts.push(super::sketch::Conflict { stroke: d.stroke as usize, message: "No way by land along this road here: it stops and starts again".into(), x: p[0] * cell, y: p[1] * cell });
                    if piece.len() >= 2 {
                        out.routes.push((class, std::mem::take(&mut piece), d.stroke));
                    }
                    piece.clear();
                }
            }
        }
        if piece.len() >= 2 {
            out.routes.push((class, piece, d.stroke));
        }
    }
    // A `none` line closes the cells within about a cell of it: no step crosses that band.
    if inp.drawn.iter().any(|d| d.class.is_none()) {
        let mut blocked = vec![false; w * h];
        for d in inp.drawn.iter().filter(|d| d.class.is_none()) {
            for seg in d.pts.windows(2) {
                let (i0, i1) = ((seg[0][0].min(seg[1][0]) - 2.0).floor().max(0.0) as usize, ((seg[0][0].max(seg[1][0]) + 2.0).ceil() as usize).min(w - 1));
                let (j0, j1) = ((seg[0][1].min(seg[1][1]) - 2.0).floor().max(0.0) as usize, ((seg[0][1].max(seg[1][1]) + 2.0).ceil() as usize).min(h - 1));
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        if seg_dist([i as f64, j as f64], seg[0], seg[1]) <= 1.1 {
                            blocked[j * w + i] = true;
                        }
                    }
                }
            }
        }
        out.blocked = blocked;
    }
    out
}

/// Distance along a polyline at each of its points.
fn arc(pts: &[[f64; 2]]) -> Vec<f64> {
    let mut s = vec![0.0; pts.len()];
    for i in 1..pts.len() {
        s[i] = s[i - 1] + dist(pts[i - 1], pts[i]);
    }
    s
}

/// The point `t` along a polyline (`s`: its `arc`), clamped to its ends.
fn point_at(pts: &[[f64; 2]], s: &[f64], t: f64) -> [f64; 2] {
    let k = s.partition_point(|&v| v < t).clamp(1, pts.len() - 1);
    let (a, b) = (pts[k - 1], pts[k]);
    let f = ((t - s[k - 1]) / (s[k] - s[k - 1]).max(1e-9)).clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
}

/// The stretch of a polyline from `t0` to `t1` along it (clamped to its ends).
fn sub_line(pts: &[[f64; 2]], s: &[f64], t0: f64, t1: f64) -> Vec<[f64; 2]> {
    let mut out = vec![point_at(pts, s, t0)];
    out.extend(pts.iter().zip(s).filter(|(_, v)| **v > t0 && **v < t1).map(|(p, _)| *p));
    out.push(point_at(pts, s, t1));
    out
}

/// Where a point falls on a polyline: the distance along it of the nearest point, and how far
/// off it is.
fn project(p: [f64; 2], pts: &[[f64; 2]], s: &[f64]) -> (f64, f64) {
    let mut best = (0.0, f64::INFINITY);
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let d = dist(p, [a[0] + t * dx, a[1] + t * dy]);
        if d < best.1 {
            best = (s[k - 1] + t * (s[k] - s[k - 1]), d);
        }
    }
    best
}

/// A* routes for the links, in order (later routes reuse earlier ones), after the drawn
/// roads. Returns the best (lowest) class per cell (`u8::MAX` = none) and every route.
/// Round a Town+ end, roads and tracks (not king's roads) keep clear of each other: the road
/// trim radius of the biggest possible metropolis, so a change of tier can't move a junction
/// into the town.
const ZONE_FT: f64 = 1.1 * 1.5 * 12.0 * 282.842712474619;
/// The classes that count as a major road to join (those up to this one).
const MAJOR: RoadClass = RoadClass::Road;
/// What stepping off the open ground onto a major road inside the zone costs (cells): large, not infinite.
const JOIN_BAN: f64 = 200.0;
/// Whether segments a-b and c-d cross (properly: not where they only touch).
fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let side = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// How far (miles) the other roads keep off an isolated realm's track, but at its outer end.
pub const REALM_TRACK_APART_MI: f64 = 20.0;
/// What crossing the zone edge off a major road costs (cells): the long way round that is worth a road of its own.
const GATE_ROAD: f64 = 2.0;
const GATE_TRACK: f64 = 1.0;

pub fn route(inp: &Inputs, settlements: &[Settlement], links: &[Link]) -> (Vec<u8>, Vec<Routed>) {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let n = w * h;
    let pre = inp.pre();
    // --- 2. Routing.
    let RouteFixed { costs, free, open, wobble } = inp.fixed.get_or_init(|| {
        let costs = Costs::new(inp);
        let free: Vec<bool> = (0..n).map(|k| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE).collect();
        let open: Vec<bool> = (0..n).map(|k| free[k] && !pre.blocked.get(k).copied().unwrap_or(false)).collect();
        let mut noise = crate::core::noise::Fbm::new(inp.world.stream("t0.road.cost"), 2, 2.0, 0.5);
        let wobble: Vec<f32> = (0..n).map(|k| (1.0 + 0.25 * noise.at((k % w) as f64 / 6.0, (k / w) as f64 / 6.0)) as f32).collect();
        RouteFixed { costs, free, open, wobble }
    });
    let mut road_class: Vec<u8> = vec![u8::MAX; n];
    // Cells on a drawn road only (routes share them at half cost, not a fifth: a planned road
    // takes a drawn one where it goes its way, but no long way round for it).
    let mut drawn_only: Vec<bool> = vec![false; if pre.routes.is_empty() { 0 } else { n }];
    let mut paths: Vec<Routed> = Vec::new();
    // The steps major roads take: a step between two major roads' cells is no ride along one.
    let mut major_steps: crate::core::hash::FastSet<(u32, u32)> = Default::default();
    for (class, cells, stroke) in &pre.routes {
        for &c in cells {
            drawn_only[c as usize] = true;
        }
        for &c in cells {
            road_class[c as usize] = road_class[c as usize].min(*class as u8);
        }
        if *class <= MAJOR {
            major_steps.extend(cells.windows(2).map(|p| (p[0].min(p[1]), p[0].max(p[1]))));
        }
        paths.push((*class, cells.clone(), Some(*stroke)));
    }
    let mut search = Search::new(n);
    let mut memo = inp.routes.borrow_mut();
    let mut same = true;
    // Isolated realms' towns (cell x, y, the radius other roads pay extra inside), and per cell
    // whether a road from outside any realm runs there; each realm's track.
    let avoid = super::settle::REALM_AVOID_MI * 5280.0 / cell;
    let avoid_track = super::settle::REALM_AVOID_TRACK_MI * 5280.0 / cell;
    // (How far other roads keep off a realm's track but where it joins them.)
    let apart_ft = REALM_TRACK_APART_MI * 5280.0;
    let realms: Vec<(f64, f64)> = settlements.iter().filter(|s| s.isolated && s.planned >= Tier::Town).map(|s| ((s.cell % w) as f64, (s.cell / w) as f64)).collect();
    let mut outside: Vec<bool> = vec![false; if realms.is_empty() { 0 } else { n }];
    for (_, cells, _) in &pre.routes {
        for &c in cells {
            if let Some(o) = outside.get_mut(c as usize) {
                *o = true;
            }
        }
    }
    let mut tracks: std::collections::BTreeMap<usize, Vec<u32>> = Default::default();
    // Where each realm's villages' tracks join it.
    let mut joins: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    // Each realm's villages' tracks so far (cells).
    let mut spurs: std::collections::BTreeMap<usize, Vec<Vec<u32>>> = Default::default();
    for (e, link) in links.iter().enumerate() {
        let class = link.class;
        let realm_link = [link.a, link.b].iter().any(|&end| match end {
            End::Town(i) => settlements[i].isolated,
            End::Net | End::Track(_) => true,
            End::Cell(_) => false,
        });
        let from = match link.a {
            End::Town(i) => settlements[i].cell,
            End::Cell(k) => k,
            _ => continue,
        };
        let near = |cells: &mut dyn Iterator<Item = usize>| {
            let (fx, fy) = ((from % w) as f64, (from / w) as f64);
            cells.map(|k| (((k % w) as f64 - fx) * ((k % w) as f64 - fx) + ((k / w) as f64 - fy) * ((k / w) as f64 - fy), k)).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|m| m.1)
        };
        let goal = match link.b {
            End::Town(i) => Some(settlements[i].cell),
            End::Cell(k) => Some(k),
            // (The nearest king's road, the nearest road where that is much nearer; nearer still
            // the way its villages lie, so that it leaves past them: `settle::place_realms`.)
            End::Net => {
                let (fx, fy) = ((from % w) as f64, (from / w) as f64);
                let ri = match link.a {
                    End::Town(i) => i,
                    _ => usize::MAX,
                };
                let (mut ex, mut ey) = (0.0, 0.0);
                for (vi, v) in settlements.iter().enumerate() {
                    if v.isolated && v.planned == Tier::Village && realm_of(settlements, vi) == Some(ri) {
                        let (vx, vy) = ((v.cell % w) as f64 - fx, (v.cell / w) as f64 - fy);
                        let m = crate::core::sqrt(vx * vx + vy * vy).max(1e-9);
                        (ex, ey) = (ex + vx / m, ey + vy / m);
                    }
                }
                let em = crate::core::sqrt(ex * ex + ey * ey);
                let d = |k: usize| {
                    let (dx, dy) = ((k % w) as f64 - fx, (k / w) as f64 - fy);
                    let r = crate::core::sqrt(dx * dx + dy * dy);
                    let off = if em > 1e-6 && r > 1e-9 { 1.0 - (dx * ex + dy * ey) / (em * r) } else { 0.0 };
                    r * (1.0 + 0.4 * off)
                };
                let toward = |cells: &mut dyn Iterator<Item = usize>| cells.map(|k| (d(k), k)).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|m| m.1);
                let any = toward(&mut (0..outside.len()).filter(|&k| outside[k]));
                let king = toward(&mut (0..outside.len()).filter(|&k| outside[k] && road_class[k] == RoadClass::KingsRoad as u8));
                let natural = match (king, any) {
                    (Some(kc), Some(ac)) if d(kc) <= d(ac) + 15.0 * 5280.0 / cell => Some(kc),
                    _ => any,
                };
                // Near a junction it ends at a node instead (a settlement on the roads there, else
                // the junction), a branch of its own rather than a way on past it.
                natural.map(|g| {
                    let reach = apart_ft / cell;
                    let by = |k: usize| ((k % w) as f64 - (g % w) as f64) * ((k % w) as f64 - (g % w) as f64) + ((k / w) as f64 - (g / w) as f64) * ((k / w) as f64 - (g / w) as f64) <= reach * reach;
                    let mut arms: std::collections::BTreeMap<u32, Vec<u32>> = Default::default();
                    for (_, path, _) in &paths {
                        for p in path.windows(2) {
                            if p[0] != p[1] && outside[p[0] as usize] && outside[p[1] as usize] && (by(p[0] as usize) || by(p[1] as usize)) {
                                for (a, b) in [(p[0], p[1]), (p[1], p[0])] {
                                    let v = arms.entry(a).or_default();
                                    if !v.contains(&b) {
                                        v.push(b);
                                    }
                                }
                            }
                        }
                    }
                    let junctions: Vec<usize> = arms.iter().filter(|(k, v)| v.len() >= 3 && by(**k as usize)).map(|(&k, _)| k as usize).collect();
                    if junctions.is_empty() {
                        return g;
                    }
                    let towns: Vec<usize> = settlements.iter().filter(|s| !s.isolated && outside[s.cell] && by(s.cell)).map(|s| s.cell).collect();
                    let pick = if towns.is_empty() { junctions } else { towns };
                    near(&mut pick.into_iter()).unwrap_or(g)
                })
            }
            End::Track(ri) => {
                // (Joining it outside the town, so only the track reaches its gate.)
                let s = &settlements[ri];
                let r = realm_keep_out(s, cell);
                let (sx, sy) = ((s.cell % w) as f64, (s.cell / w) as f64);
                let far = |k: usize| ((k % w) as f64 - sx) * ((k % w) as f64 - sx) + ((k / w) as f64 - sy) * ((k / w) as f64 - sy) > r * r;
                // (Not beside where another of its villages joined: no stub between two junctions.)
                let apart = |k: usize| {
                    joins.get(&ri).is_none_or(|js: &Vec<usize>| js.iter().all(|&j| ((k % w) as f64 - (j % w) as f64) * ((k % w) as f64 - (j % w) as f64) + ((k / w) as f64 - (j / w) as f64) * ((k / w) as f64 - (j / w) as f64) >= JOIN_APART * JOIN_APART))
                };
                // (Nor across another of its villages' tracks: each straight to its own join.)
                let xy = |c: usize| ((c % w) as f64, (c / w) as f64);
                let clear = |k: usize| {
                    let (a, b) = (xy(from), xy(k));
                    spurs.get(&ri).is_none_or(|ss| ss.iter().all(|sp| sp.windows(2).all(|q| !segments_cross(a, b, xy(q[0] as usize), xy(q[1] as usize)))))
                };
                // (But a clear line only where it is not much farther: rather a short spur round the
                // other's join than one alongside the track for miles to its far end.)
                let (fx, fy) = xy(from);
                let gap = |k: usize| crate::core::sqrt((xy(k).0 - fx) * (xy(k).0 - fx) + (xy(k).1 - fy) * (xy(k).1 - fy));
                tracks.get(&ri).and_then(|t| {
                    let best = near(&mut t.iter().map(|&c| c as usize).filter(|&k| far(k) && apart(k)));
                    let straight = near(&mut t.iter().map(|&c| c as usize).filter(|&k| far(k) && apart(k) && clear(k)));
                    match (straight, best) {
                        (Some(s), Some(b)) if gap(s) <= 1.5 * gap(b) + 2.0 * 5280.0 / cell => Some(s),
                        (_, Some(b)) => Some(b),
                        _ => near(&mut t.iter().map(|&c| c as usize).filter(|&k| far(k))),
                    }
                })
            }
        };
        let Some(goal) = goal else { continue };
        // (The joins of its realm's other villages so far.)
        let others: Vec<usize> = match link.b {
            End::Track(ri) => joins.get(&ri).cloned().unwrap_or_default(),
            _ => Vec::new(),
        };
        if let End::Track(ri) = link.b {
            joins.entry(ri).or_default().push(goal);
        }
        let start = from;
        // A hidden village's track keeps off the ground round its realm's town but the town's own
        // track (one gate).
        let keep_out = match link.b {
            End::Track(ri) => {
                let s = &settlements[ri];
                Some(((s.cell % w) as f64, (s.cell / w) as f64, realm_keep_out(s, cell)))
            }
            _ => None,
        };
        // Near a town or city at either end, the king's roads keep to themselves (no shared trunk),
        // each arriving at its own gate. Roads and tracks keep to a fixed zone round the town
        // instead: inside it they share only a major road, and leaving the open ground for the
        // zone costs a gate, so they join a major road outside (see the cost below).
        let kings = class == RoadClass::KingsRoad;
        let town_ends: Vec<(f64, f64, f64)> = [link.a, link.b]
            .iter()
            .filter_map(|&end| match end {
                End::Town(i) if settlements[i].planned >= Tier::Town => Some(i),
                _ => None,
            })
            .map(|i| {
                let r = if kings { crate::town::urban_radius(settlements[i].planned, settlements[i].planned_pop) / cell + 1.5 } else { ZONE_FT / cell + 1.5 };
                ((settlements[i].cell % w) as f64, (settlements[i].cell / w) as f64, r)
            })
            .collect();
        let gate = if class == RoadClass::Road { GATE_ROAD } else { GATE_TRACK };
        // A town no major road reaches yet: its first way out may step onto one passing it (the
        // town's own way to it); once one does, the others ride that.
        let lone = road_class[start] > MAJOR as u8;
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
            let b = [crate::core::ceil(bx0) as i64, crate::core::floor(bx1) as i64, crate::core::ceil(by0) as i64, crate::core::floor(by1) as i64];
            // A realm's track keeps off the other roads (cells from them) but near where it joins
            // them; its villages' tracks off every road but where they join its track (each a spur
            // of its own, straight to it).
            let off: Vec<f64> = if matches!(link.b, End::Net) { super::climate::distance_to(w, h, &outside) } else { Vec::new() };
            let apart = apart_ft / cell;
            // (A margin over that, so none comes closer just beyond the end.)
            let keep = 1.25 * apart;
            let spur = matches!(link.b, End::Track(_));
            // (And a cell's berth off its realm's other villages' tracks, so none slips across one.)
            let berth: crate::core::hash::FastSet<usize> = match link.b {
                End::Track(ri) => spurs
                    .get(&ri)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .flat_map(|&c| {
                        let (x, y) = ((c as usize % w) as i64, (c as usize / w) as i64);
                        (-1i64..=1).flat_map(move |dy| (-1i64..=1).map(move |dx| (x + dx, y + dy)))
                    })
                    .filter(|&(x, y)| x >= 0 && y >= 0 && x < w as i64 && y < h as i64)
                    .map(|(x, y)| y as usize * w + x as usize)
                    .filter(|&k| k != goal)
                    .collect(),
                _ => Default::default(),
            };
            let zone = if class == RoadClass::Track { avoid_track } else { avoid };
            let path = search.run(w, start, goal, b, |k| open[k] || (k == goal && free[k]), |i, nb, mids, run| {
                let mut c = costs.ground(inp, i, nb, run) * wobble[nb] as f64;
                c += costs.river(inp, i, nb, mids);
                let (nx, ny) = ((nb % w) as f64, (nb / w) as f64);
                let near = |x: f64, y: f64| town_ends.iter().any(|&(tx, ty, tr)| (x - tx) * (x - tx) + (y - ty) * (y - ty) <= tr * tr);
                // Round an isolated realm the others go the long way.
                if !realm_link {
                    for &(rx, ry) in &realms {
                        let d2 = ((nx - rx) * (nx - rx) + (ny - ry) * (ny - ry)) / (zone * zone);
                        if d2 < 1.0 {
                            c *= 1.0 + 12.0 * (1.0 - d2);
                        }
                    }
                }
                if let Some(&d) = off.get(nb)
                    && d < keep
                    && (nx - gx) * (nx - gx) + (ny - gy) * (ny - gy) > apart * apart
                {
                    c *= 1.0 + 8.0 * (1.0 - d / keep);
                }
                if spur && ((road_class[nb] != u8::MAX && nb != goal) || berth.contains(&nb)) {
                    c += JOIN_BAN / run;
                }
                if let Some((kx, ky, kr)) = keep_out
                    && road_class[nb] == u8::MAX
                    && (nx - kx) * (nx - kx) + (ny - ky) * (ny - ky) <= kr * kr
                {
                    c += JOIN_BAN / run;
                }
                // (A hidden village's track keeps off its realm's roads beside where the others
                // joined them, so that their junctions stay apart.)
                if road_class[nb] != u8::MAX && nb != goal && !others.is_empty() && others.iter().any(|&j| ((j % w) as f64 - nx) * ((j % w) as f64 - nx) + ((j / w) as f64 - ny) * ((j / w) as f64 - ny) < JOIN_APART * JOIN_APART) {
                    c += JOIN_BAN / run;
                }
                if kings {
                    if road_class[nb] != u8::MAX && !near(nx, ny) {
                        c *= if drawn_only.get(nb).copied().unwrap_or(false) { 0.5 } else { 0.2 };
                    }
                } else {
                    let (ix, iy) = ((i % w) as f64, (i / w) as f64);
                    let major = |k: usize| road_class[k] <= MAJOR as u8;
                    let (zi, zn) = (near(ix, iy), near(nx, ny));
                    let edge = || major_steps.contains(&((i as u32).min(nb as u32), (i as u32).max(nb as u32)));
                    // (At its ends a road rides only a major road's own step: a town's roads leave on the
                    // one that reaches it, not across to another passing by.)
                    let on = if i != start && nb != goal { major(nb) && major(i) } else { edge() };
                    if zn {
                        if on {
                            c *= 0.2;
                        } else if major(nb) && nb != goal && nb != start && !(lone && i == start) {
                            c += JOIN_BAN / run;
                        }
                    } else if road_class[nb] != u8::MAX {
                        c *= if drawn_only.get(nb).copied().unwrap_or(false) { 0.5 } else { 0.2 };
                    }
                    // Crossing the zone edge off a major road, or leaving a major road inside the zone
                    // for a way in of its own, costs a gate.
                    if !on && i != start && nb != goal && (zi != zn || (zi && major(i))) {
                        c += gate / run;
                    }
                }
                c
            });
            memo.truncate(e);
            memo.push((key, path.clone()));
            path
        };
        let Some(path) = found else { continue };
        if !realm_link {
            for &c in &path {
                if let Some(o) = outside.get_mut(c as usize) {
                    *o = true;
                }
            }
        } else if let (End::Town(i), End::Net) = (link.a, link.b) {
            tracks.insert(i, path.clone());
        } else if let End::Track(ri) = link.b {
            // (Each realm village's track a spur of its own to the realm's: no village's joins another's.)
            spurs.entry(ri).or_default().push(path.clone());
        }
        if class <= MAJOR {
            major_steps.extend(path.windows(2).map(|p| (p[0].min(p[1]), p[0].max(p[1]))));
        }
        for &c in &path {
            let c = c as usize;
            road_class[c] = road_class[c].min(class as u8);
            if let Some(d) = drawn_only.get_mut(c) {
                *d = false;
            }
        }
        paths.push((class, path, link.stroke));
    }
    (road_class, paths)
}

/// Where roads of the given classes would run, for placing settlements beside them.
pub struct Preview {
    /// Per cell: routes passing through (saturating).
    pub usage: Vec<u8>,
    /// Per cell: a junction (3+ distinct route edges meet there).
    pub junction: Vec<bool>,
    /// Per cell: the top of a pass, where a route climbs over high ground between valleys.
    pub pass: Vec<bool>,
    /// The routes (the economy's trade runs along them).
    pub paths: Vec<Routed>,
}

pub fn preview(inp: &Inputs, settlements: &[Settlement], classes: &[RoadClass]) -> Preview {
    let edges = links(inp, settlements, classes);
    let (_, paths) = route(inp, settlements, &edges);
    let n = inp.w * inp.h;
    let mut usage = vec![0u8; n];
    let mut pass = vec![false; n];
    let mut links: std::collections::BTreeSet<(u32, u32)> = Default::default();
    // A pass: where a king's route tops a saddle (`settle::saddle_at`, the test a pass town is
    // held to where it stands), the route's way through there from two cells either side.
    let sea = inp.world.params().sea_level_ft;
    let (w, cell) = (inp.w, inp.cell_ft);
    let at = |c: u32| [(c as usize % w) as f64 * cell, (c as usize / w) as f64 * cell];
    for (class, path, _) in &paths {
        let m = path.len();
        for i in 1..m.saturating_sub(1) {
            if *class != RoadClass::KingsRoad || inp.height[path[i] as usize] - sea < 1_000.0 {
                continue;
            }
            let (a, b) = (at(path[i.saturating_sub(2)]), at(path[(i + 2).min(m - 1)]));
            let u = unit2(sub2(b, a));
            if crate::t0::settle::saddle_at(inp.ground, at(path[i]), u, sea) {
                pass[path[i] as usize] = true;
            }
        }
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
    Preview { usage, junction: degree.iter().map(|&d| d >= 3).collect(), pass, paths }
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
/// when it stays within `tol` cells of the grid path, crosses only passable ground and is no steeper than the
/// class allows (or than the path itself was); chords are then resampled to at most one cell
/// per step so the grade check below sees the terrain between vertices.
fn simplify(cells: &[[f64; 2]], tol: f64, w: usize, h: usize, hg: &Plan, gmax: f64, cell: f64, passable: impl Fn(usize) -> bool) -> Vec<[f64; 2]> {
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
        if worst > tol || !clear(a, b, &cells[i..=j]) {
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

/// A drawn road's segment (its cells, the ends in place, in cells): along its drawn `line`
/// between where the segment's ends fall on it, wherever that stretch is dry (`dry`, every
/// quarter cell) and the cells keep near it (within 3 cells); else each half of the cells is
/// tried the same way, down to single steps, and the cells left (round water) are `smooth`ed.
/// The line within a cell of either end is left out, so the road eases onto it from a junction
/// off it. Points at most a cell apart.
fn along_drawn(cells: &[[f64; 2]], line: &[[f64; 2]], dry: &dyn Fn([f64; 2]) -> bool, smooth: &dyn Fn(&[[f64; 2]]) -> Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    /// From cell `i` to cell `j`: the line drawn between them (its points), or the cells.
    type Piece = (usize, usize, Option<Vec<[f64; 2]>>);
    fn fit(cells: &[[f64; 2]], line: &[[f64; 2]], s: &[f64], dry: &dyn Fn([f64; 2]) -> bool, i: usize, j: usize, out: &mut Vec<Piece>) {
        if j > i + 1 {
            let ((ti, _), (tj, _)) = (project(cells[i], line, s), project(cells[j], line, s));
            let near = cells[i..=j].iter().all(|c| project(*c, line, s).1 <= 3.0);
            let (lo, hi) = (ti.min(tj), ti.max(tj));
            if near && hi - lo > 2.5 {
                let mut part = sub_line(line, s, lo + 1.0, hi - 1.0);
                if ti > tj {
                    part.reverse();
                }
                let mut path = vec![cells[i]];
                path.extend_from_slice(&part);
                path.push(cells[j]);
                let wet = path.windows(2).any(|w| {
                    let n = (dist(w[0], w[1]) * 4.0).ceil().max(1.0) as usize;
                    (1..=n).any(|k| !dry([w[0][0] + (w[1][0] - w[0][0]) * k as f64 / n as f64, w[0][1] + (w[1][1] - w[0][1]) * k as f64 / n as f64]))
                });
                if !wet {
                    out.push((i, j, Some(part)));
                    return;
                }
            }
            let m = (i + j) / 2;
            fit(cells, line, s, dry, i, m, out);
            fit(cells, line, s, dry, m, j, out);
            return;
        }
        // (Runs of cells join up: `along_drawn` smooths them together.)
        match out.last_mut() {
            Some((_, end, None)) if *end == i => *end = j,
            _ => out.push((i, j, None)),
        }
    }
    let s = arc(line);
    let mut pieces = Vec::new();
    fit(cells, line, &s, dry, 0, cells.len() - 1, &mut pieces);
    let mut kept = vec![cells[0]];
    for (i, j, part) in pieces {
        match part {
            Some(part) => {
                kept.extend_from_slice(&part);
                kept.push(cells[j]);
            }
            None => kept.extend_from_slice(&smooth(&cells[i..=j])[1..]),
        }
    }
    let mut out = vec![kept[0]];
    for pair in kept.windows(2) {
        let n = dist(pair[0], pair[1]).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
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
    // A route over this many times the straight run is too long.
    const LONG: f64 = 1.5;
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
        let mut route = fine_route(hg, &on_plan, &on_cells, cells[i], cells[j], SLACK * gmax, &[1.0, 1.8], cell, F)
            .or_else(|| fine_route(hg, ground, dry, cells[i], cells[j], SLACK * gmax, &[1.0, 1.8], cell, F_FINE));
        // A climb that can only hold the grade by adding miles comes out as one long finger (out,
        // a U-turn, back beside itself) or a route far longer than the straight run. Spend the
        // grade instead: retry with each leg step allowed 1.2 to 1.8 times the planned grade (the
        // profile averages the bumps), taking the first route that has neither fault.
        if let Some(r) = route.clone() {
            let straight = dist(cells[i], cells[j]) * cell;
            let bad = |r: &[[f64; 2]]| route_len(r) * cell > LONG * straight || route_finger(r, cell);
            if bad(&r) {
                // (The profile can only climb its limit over the length: a route too short for
                // the climb would be cut and filled.)
                let climb = (at(cells[i]) - at(cells[j])).abs();
                // (Of those left, the one without a finger first, then the shortest.)
                let mut best: Option<((bool, f64), Vec<[f64; 2]>)> = None;
                for m in [1.2, 1.4, 1.6, 1.8] {
                    let Some(r2) = fine_route(hg, &on_plan, &on_cells, cells[i], cells[j], SLACK * gmax, &[m], cell, F) else { continue };
                    let len = route_len(&r2);
                    if climb > 0.97 * gmax * len * cell {
                        continue;
                    }
                    if !bad(&r2) {
                        route = Some(r2);
                        best = None;
                        break;
                    }
                    let key = (route_finger(&r2, cell), len);
                    if best.as_ref().is_none_or(|b| key < b.0) {
                        best = Some((key, r2));
                    }
                }
                // None as good as wanted: one that is not a finger where the route is one (however
                // long), else the shortest, if it saves a good part of the length.
                if let Some(((finger, len), r2)) = best {
                    if (route_finger(&r, cell) && !finger) || len < 0.8 * route_len(&r) {
                        route = Some(r2);
                    }
                }
            }
        }
        // A finger that gains no height is cut across (the chord must be passable and keep to the
        // stepped grade); the climb that needs its length stays.
        if let Some(r) = route.as_mut() {
            for _ in 0..4 {
                let Some((fi, fj)) = finger_span(r, cell) else { break };
                let (p, q) = (r[fi], r[fj]);
                let len = dist(p, q) * cell;
                let n = (dist(p, q) * 4.0).ceil().max(1.0) as usize;
                if (at(p) - at(q)).abs() > 1.8 * SLACK * gmax * len || !(0..=n).all(|k| on_cells([p[0] + (q[0] - p[0]) * k as f64 / n as f64, p[1] + (q[1] - p[1]) * k as f64 / n as f64])) {
                    break;
                }
                r.drain(fi + 1..fj);
            }
        }
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
    // Corners (hairpins, and where a switchback stretch meets the road) turn on arcs; within a
    // stretch on wider ones, as far as its legs allow (a kink between legs reads as a bend).
    round_corners_by(&out, &wander, &|k| if wander[k] == 0.0 && k + 1 < wander.len() && wander[k + 1] == 0.0 { 0.6 / F } else { 0.06 }, 0.05)
}

/// A polyline (cells) with each corner turning more than 15° rounded by a circular arc
/// tangent to both sides (radius up to `r_max`, smaller where the sides are short or meet at
/// a narrow angle: a hairpin becomes a tight U-turn), and points at most `step` apart; `vals`
/// (one per point) are carried along (interpolated on lines, the corner's on its arc).
pub fn round_corners(pts: &[[f64; 2]], vals: &[f32], r_max: f64, step: f64) -> (Vec<[f64; 2]>, Vec<f32>) {
    round_corners_by(pts, vals, &|_| r_max, step)
}

/// `round_corners` with the largest radius given per corner (by point index).
fn round_corners_by(pts: &[[f64; 2]], vals: &[f32], r_max: &dyn Fn(usize) -> f64, step: f64) -> (Vec<[f64; 2]>, Vec<f32>) {
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
            // Interior angle between the sides; a turn under 15° stays.
            let alpha = libm::acos(dot2(a, b).clamp(-1.0, 1.0));
            if alpha > 165f64.to_radians() || alpha < 1e-3 {
                return None;
            }
            let tan_half = libm::tan(0.5 * alpha);
            let t_max = 0.45 * dist(pts[k - 1], pts[k]).min(dist(pts[k], pts[k + 1]));
            let r = r_max(k).min(t_max * tan_half);
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

/// Length of a polyline (cells).
fn route_len(r: &[[f64; 2]]) -> f64 {
    r.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Whether a route (cells) runs out and comes back beside itself: two points over 1.5 mi apart
/// along it but within 0.6 mi of each other, with a leg between them longer than 0.6 mi
/// (compact switchback stacks have short legs between their hairpins).
fn route_finger(r: &[[f64; 2]], cell: f64) -> bool {
    finger_span(r, cell).is_some()
}

/// The first finger of a route (`route_finger`) as the indices of the point it leaves the route
/// at and the one it comes back to.
fn finger_span(r: &[[f64; 2]], cell: f64) -> Option<(usize, usize)> {
    const MI: f64 = 5280.0;
    let mut s: Vec<f64> = vec![0.0];
    for w in r.windows(2) {
        s.push(s[s.len() - 1] + dist(w[0], w[1]) * cell);
    }
    if s[s.len() - 1] < 1.5 * MI {
        return None;
    }
    // Hairpin-like corners (turning over 100 degrees across the nodes 250 ft either side).
    let at = |d: f64| {
        let k = s.partition_point(|&v| v < d).min(r.len() - 1);
        r[k]
    };
    let corners: Vec<f64> = (1..r.len() - 1)
        .filter(|&k| {
            let (p, a, b) = (r[k], at(s[k] - 500.0), at(s[k] + 500.0));
            let (u, v) = (sub2(p, a), sub2(b, p));
            let (lu, lv) = (crate::core::sqrt(dot2(u, u)), crate::core::sqrt(dot2(v, v)));
            lu > 0.0 && lv > 0.0 && dot2(u, v) < -0.17 * lu * lv
        })
        .map(|k| s[k])
        .collect();
    for i in 0..r.len() {
        for j in i + 1..r.len() {
            if s[j] - s[i] >= 1.5 * MI && dist(r[i], r[j]) * cell <= 0.6 * MI {
                let mut marks = vec![s[i]];
                marks.extend(corners.iter().copied().filter(|&c| c > s[i] && c < s[j]));
                marks.push(s[j]);
                if marks.windows(2).any(|w| w[1] - w[0] > 0.6 * MI) {
                    return Some((i, j));
                }
                break;
            }
        }
    }
    None
}

/// A* on a grid `f` times finer than T0 inside a corridor, over the `ground` heights at its
/// nodes where `ok` (both at a point in cells); each step's grade must be ≤ gmax (then
/// ≤ 1.8 gmax as a fallback). Returns points in T0 cell units.
#[allow(clippy::too_many_arguments)]
fn fine_route(hg: &Plan, ground: &impl Fn([f64; 2]) -> f64, ok: &impl Fn([f64; 2]) -> bool, a: [f64; 2], b: [f64; 2], gmax: f64, mults: &[f64], cell: f64, f: f64) -> Option<Vec<[f64; 2]>> {
    let pad = 4.0;
    let (x0, y0) = ((a[0].min(b[0]) - pad).max(0.0), (a[1].min(b[1]) - pad).max(0.0));
    // (On whole cells, as the junctions are: an end at a settlement's own spot is put back on it
    // after, so roads still meet at one point.)
    let (x0, y0) = (crate::core::floor(x0), crate::core::floor(y0));
    let (x1, y1) = ((a[0].max(b[0]) + pad).min((hg.cw - 1) as f64), (a[1].max(b[1]) + pad).min((hg.ch - 1) as f64));
    let (fw, fh) = (((x1 - x0) * f) as usize + 1, ((y1 - y0) * f) as usize + 1);
    let to_world = |i: usize| [x0 + (i % fw) as f64 / f, y0 + (i / fw) as f64 / f];
    let hts: Vec<f64> = (0..fw * fh).map(|i| ground(to_world(i))).collect();
    let ok: Vec<bool> = (0..fw * fh).map(|i| ok(to_world(i))).collect();
    let idx = |p: [f64; 2]| (((p[1] - y0) * f).round() as usize).min(fh - 1) * fw + (((p[0] - x0) * f).round() as usize).min(fw - 1);
    let (s, t) = (idx(a), idx(b));
    let step = cell / f;
    const DIRS: [(i32, i32); 16] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)];
    // What turning from one heading to another costs (grid steps): enough that the route climbs
    // in long legs rather than zigzagging every step or two. (The states are a node and the
    // heading it was reached by, so each turn is costed as made.)
    const TURN: f64 = 2.5;
    let turn_cost: Vec<f64> = (0..256)
        .map(|k| {
            let ((ax, ay), (bx, by)) = (DIRS[k / 16], DIRS[k % 16]);
            TURN * (1.0 - (ax * bx + ay * by) as f64 / crate::core::sqrt(((ax * ax + ay * ay) * (bx * bx + by * by)) as f64))
        })
        .collect();
    for limit in mults.iter().map(|m| gmax * m) {
        let mut g = vec![f64::INFINITY; fw * fh * 16];
        let mut came = vec![u32::MAX; fw * fh * 16];
        let mut done = vec![false; fw * fh * 16];
        let mut heap = BinaryHeap::new();
        let (tx, ty) = ((t % fw) as f64, (t / fw) as f64);
        // (The start leaves in any heading.)
        let start = u32::MAX - 1;
        heap.push(Node { f: 0.0, i: start });
        while let Some(Node { i: st, .. }) = heap.pop() {
            let (i, d0) = if st == start { (s, usize::MAX) } else { (st as usize / 16, st as usize % 16) };
            if st != start && i == t {
                let mut path = vec![t];
                let mut cur = st as usize;
                while came[cur] != start {
                    cur = came[cur] as usize;
                    path.push(cur / 16);
                }
                path.push(s);
                path.reverse();
                let mut out: Vec<[f64; 2]> = path.into_iter().map(to_world).collect();
                let k = out.len() - 1;
                (out[0], out[k]) = (a, b);
                // (Not doubling back from there to the next point.)
                if out.len() > 2 && dot2(sub2(out[1], a), sub2(out[2], out[1])) < 0.0 {
                    out.remove(1);
                }
                let k = out.len() - 1;
                if out.len() > 2 && dot2(sub2(out[k - 1], out[k - 2]), sub2(b, out[k - 1])) < 0.0 {
                    out.remove(k - 1);
                }
                return Some(out);
            }
            let g0 = if st == start { 0.0 } else { g[st as usize] };
            if st != start {
                if done[st as usize] {
                    continue;
                }
                done[st as usize] = true;
            }
            let (ix, iy) = ((i % fw) as i32, (i / fw) as i32);
            for (d, (dx, dy)) in DIRS.into_iter().enumerate() {
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
                // Turning costs; climbing costs a little extra.
                let turn = if d0 == usize::MAX { 0.0 } else { turn_cost[d0 * 16 + d] };
                let (ns, ng) = (nb * 16 + d, g0 + run * (1.0 + 2.0 * grade) + turn * step);
                if ng < g[ns] {
                    g[ns] = ng;
                    came[ns] = st;
                    let hd = dist([nx as f64, ny as f64], [tx, ty]) * step;
                    heap.push(Node { f: ng + hd, i: ns as u32 });
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

fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]).max(1e-9);
    [v[0] / l, v[1] / l]
}

fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
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
        if detour < 1.3 * lens[i] && roads[i].stroke.is_none() && roads[i].class != RoadClass::KingsRoad {
            gone[i] = true;
        }
    }
    let mut i = 0;
    roads.retain(|_| {
        i += 1;
        !gone[i - 1]
    });
}

/// How round a small loop may be (ft) for one of its lesser roads to go, and how much longer the
/// way round it may then be.
const TRIANGLE_FT: f64 = 4.0 * 5280.0;
const TRIANGLE_SLACK: f64 = 1.6;
/// The same for a link between two roads into one place.
const TRIANGLE_LINK: f64 = 3.0;
/// How round a loop of three roads between junctions alone (no place at its corners) may be: a
/// rung between two roads that meet a third reads as one too many further out (the way round
/// at most `TRIANGLE_OPEN_SLACK` times as long).
const TRIANGLE_OPEN_FT: f64 = 6.0 * 5280.0;
const TRIANGLE_OPEN_SLACK: f64 = 1.5;

/// Three roads closing a small loop (two lesser roads into one place and a short link between
/// them, or a lesser road's two arms out to the same road) read as a ladder's rung: one lesser
/// road of it goes, a link between two roads into one place first, else the one whose way round
/// (over roads at least as good) is shortest for its length. Drawn roads and king's roads stay.
/// Returns whether one went.
fn drop_triangle(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) -> bool {
    let ends: Vec<((i64, i64), (i64, i64))> = roads.iter().map(|r| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns))).collect();
    let lens: Vec<f64> = roads.iter().map(road_len).collect();
    let mut at: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, e) in ends.iter().enumerate() {
        if e.0 != e.1 {
            at.entry(e.0).or_default().push(i);
            at.entry(e.1).or_default().push(i);
        }
    }
    let far = |i: usize, n: (i64, i64)| if ends[i].0 == n { ends[i].1 } else { ends[i].0 };
    let mut best: Option<(f64, usize)> = None;
    for (i, r) in roads.iter().enumerate() {
        let (a, b) = ends[i];
        if r.stroke.is_some() || r.class == RoadClass::KingsRoad || a == b || lens[i] > TRIANGLE_OPEN_FT {
            continue;
        }
        for &k in &at[&a] {
            let c = far(k, a);
            if k == i || c == b || roads[k].class > r.class {
                continue;
            }
            for &m in at.get(&c).map(|v| v.as_slice()).unwrap_or(&[]) {
                if m == i || m == k || far(m, c) != b || roads[m].class > r.class {
                    continue;
                }
                let round = lens[k] + lens[m];
                let ratio = round / lens[i];
                // (A link between two roads into one place goes first: the place keeps both ways out.)
                let link = c.0 == i64::MIN && a.0 != i64::MIN && b.0 != i64::MIN;
                let (most, rank) = if link { (TRIANGLE_LINK, ratio - TRIANGLE_LINK) } else { (TRIANGLE_SLACK, ratio) };
                let open = a.0 != i64::MIN && b.0 != i64::MIN && c.0 != i64::MIN;
                let small = lens[i] + round <= TRIANGLE_FT;
                if (small || (open && lens[i] + round <= TRIANGLE_OPEN_FT && ratio <= TRIANGLE_OPEN_SLACK)) && ratio <= most && best.is_none_or(|(q, _)| rank < q) {
                    best = Some((rank, i));
                }
            }
        }
    }
    let Some((_, i)) = best else { return false };
    roads.remove(i);
    true
}

/// A village with two lesser roads that both run out to the same better road, at junctions apart (a
/// triangle too big for `drop_triangle`), has one road too many: the worse (then longer) of the two
/// goes when the way round (the other road and the better road between them) is no more than
/// `TRIANGLE_SLACK` times as long. Returns whether one went.
fn drop_twin_link(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], small: &[bool]) -> bool {
    type K = (i64, i64);
    let ends: Vec<(K, K)> = roads.iter().map(|r| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns))).collect();
    let lens: Vec<f64> = roads.iter().map(road_len).collect();
    // Where a point lies along a king's road (ft from its start), if it is on it.
    let along = |king: &RoadPath, p: [f64; 2]| -> Option<f64> {
        let mut run = 0.0;
        for w in king.pts.windows(2) {
            let l = dist(w[0], w[1]);
            if l > 0.0 {
                let t = (((p[0] - w[0][0]) * (w[1][0] - w[0][0]) + (p[1] - w[0][1]) * (w[1][1] - w[0][1])) / (l * l)).clamp(0.0, 1.0);
                let q = [w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1])];
                if dist(q, p) < 60.0 {
                    return Some(run + t * l);
                }
            }
            run += l;
        }
        None
    };
    let mut best: Option<(f64, usize)> = None;
    for t in 0..towns.len() {
        if !small.get(t).copied().unwrap_or(false) {
            continue;
        }
        let tk: K = (i64::MIN, t as i64);
        let mine: Vec<usize> = (0..roads.len()).filter(|&i| roads[i].stroke.is_none() && roads[i].class != RoadClass::KingsRoad && ((ends[i].0 == tk) != (ends[i].1 == tk))).collect();
        // The road's far end, the road itself leaving the village at its start.
        let far = |i: usize| if ends[i].0 == tk { *roads[i].pts.last().unwrap() } else { roads[i].pts[0] };
        for &i in &mine {
            for &k in &mine {
                if k == i || roads[k].class > roads[i].class || (roads[k].class == roads[i].class && (lens[k], k) > (lens[i], i)) {
                    continue;
                }
                let (pb, pa) = (far(i), far(k));
                for king in roads.iter().filter(|r| r.class < roads[i].class) {
                    let (Some(sa), Some(sb)) = (along(king, pa), along(king, pb)) else { continue };
                    if (sa - sb).abs() < 1.0 {
                        continue;
                    }
                    // The way round over road `k` and the king's road between them.
                    let ratio = (lens[k] + (sa - sb).abs()) / lens[i];
                    if ratio <= TRIANGLE_SLACK && best.is_none_or(|(q, _)| ratio < q) {
                        best = Some((ratio, i));
                    }
                }
            }
        }
    }
    let Some((_, i)) = best else { return false };
    roads.remove(i);
    true
}

/// A lesser road that crosses a better one and then runs beside it to where that one ends (round
/// a town, where it may not join it, it crossed over to a join outside) cuts the better road's
/// corner: it ends where it crosses it instead, a junction there (to be squared clear of rivers).
/// So does one that runs beside a better feeder out of a settlement or junction and crosses the
/// road that feeder ends on (a track alongside a town's road to the king's road and over it).
/// Returns those junctions.
fn end_at_crossing(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) -> Vec<[f64; 2]> {
    const NEAR: f64 = 5280.0;
    const FEED_FT: f64 = 3.0 * 5280.0;
    let mut cuts: Vec<[f64; 2]> = Vec::new();
    let in_town = |p: [f64; 2]| towns.iter().any(|&(c, r)| dist(c, p) < 1.2 * r + 100.0);
    // Where segment a–b crosses c–d: the fractions along each.
    let meet = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (r, q) = (sub2(b, a), sub2(d, c));
        let den = r[0] * q[1] - r[1] * q[0];
        if den.abs() < 1e-9 {
            return None;
        }
        let w = sub2(c, a);
        let (u, v) = ((w[0] * q[1] - w[1] * q[0]) / den, (w[0] * r[1] - w[1] * r[0]) / den);
        ((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)).then_some((u, v))
    };
    for _ in 0..50 {
        let mut cut: Option<(usize, usize, f64, f64, [f64; 2], bool)> = None;
        let keys: Vec<((i64, i64), (i64, i64))> = roads.iter().map(|r| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns))).collect();
        // Per road, the roads that may pass by each of its ends (their bounds round it).
        let boxes: Vec<[f64; 4]> = roads.iter().map(|r| r.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])).collect();
        let by_end: Vec<[Vec<usize>; 2]> = roads
            .iter()
            .map(|r| [r.pts[0], *r.pts.last().unwrap()].map(|p| (0..boxes.len()).filter(|&k| p[0] >= boxes[k][0] - 60.0 && p[0] <= boxes[k][2] + 60.0 && p[1] >= boxes[k][1] - 60.0 && p[1] <= boxes[k][3] + 60.0).collect()))
            .collect();
        'find: for (i, r) in roads.iter().enumerate() {
            if r.stroke.is_some() || r.class == RoadClass::KingsRoad || r.pts.len() < 3 {
                continue;
            }
            let rs = arc(&r.pts);
            for at_end in [true, false] {
                let e = if at_end { *r.pts.last().unwrap() } else { r.pts[0] };
                let ek = end_key(e, towns);
                let at_town = ek.0 == i64::MIN;
                // (Or a better road from there ends on the one it crosses, `r` beside that one out
                // to it: the feeder, its far end, its length.)
                let head = r.pts.iter().zip(rs.iter()).filter(|&(_, &sr)| (if at_end { rs[rs.len() - 1] - sr } else { sr }) <= FEED_FT + 1_000.0).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, (p, _)| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
                let feeders: Vec<(usize, [f64; 2], f64, &Vec<usize>)> = (0..roads.len())
                    .filter(|&f| f != i && roads[f].class < r.class && (keys[f].0 == ek) != (keys[f].1 == ek))
                    .map(|f| {
                        let e = (keys[f].0 == ek) as usize;
                        (f, if e == 1 { *roads[f].pts.last().unwrap() } else { roads[f].pts[0] }, road_len(&roads[f]), &by_end[f][e])
                    })
                    .collect();
                for (k, t) in roads.iter().enumerate() {
                    if k == i || t.class >= r.class || t.pts.len() < 2 {
                        continue;
                    }
                    let direct = !at_town && (keys[k].0 == ek || keys[k].1 == ek);
                    let near_t: Vec<&(usize, [f64; 2], f64, &Vec<usize>)> = if direct { Vec::new() } else { feeders.iter().filter(|q| q.0 != k && q.3.binary_search(&k).is_ok()).collect() };
                    if !direct && near_t.is_empty() {
                        continue;
                    }
                    let ts = arc(&t.pts);
                    // (Where each feeder ends on `t`, which, how long.)
                    let feeds: Vec<(f64, usize, f64)> = near_t
                        .iter()
                        .filter_map(|&&(f, far, lf, _)| {
                            let (s, d) = project(far, &t.pts, &ts);
                            (d < 60.0).then_some((s, f, lf))
                        })
                        .collect();
                    if !direct && feeds.is_empty() {
                        continue;
                    }
                    // The crossing nearest that end.
                    let mut best: Option<(f64, f64, [f64; 2])> = None;
                    // (Beside a feeder, within a few miles of that end: those stretches of `t` by it.)
                    let segs: Vec<usize> = if direct {
                        (0..t.pts.len() - 1).collect()
                    } else {
                        (0..t.pts.len() - 1).filter(|&b| t.pts[b].iter().zip(t.pts[b + 1].iter()).enumerate().all(|(d, (p, q))| p.max(*q) >= head[d] && p.min(*q) <= head[d + 2])).collect()
                    };
                    for a in 0..r.pts.len() - 1 {
                        if !direct && (if at_end { rs[rs.len() - 1] - rs[a + 1] } else { rs[a] }) > FEED_FT {
                            continue;
                        }
                        for &b in &segs {
                            if let Some((u, v)) = meet(r.pts[a], r.pts[a + 1], t.pts[b], t.pts[b + 1]) {
                                let x = [r.pts[a][0] + u * (r.pts[a + 1][0] - r.pts[a][0]), r.pts[a][1] + u * (r.pts[a + 1][1] - r.pts[a][1])];
                                let (sr, st) = (rs[a] + u * (rs[a + 1] - rs[a]), ts[b] + v * (ts[b + 1] - ts[b]));
                                let from_end = if at_end { rs[rs.len() - 1] - sr } else { sr };
                                if from_end >= 100.0 && st >= 100.0 && st <= ts[ts.len() - 1] - 100.0 && best.is_none_or(|q| from_end < q.0) {
                                    best = Some((from_end, st, x));
                                }
                            }
                        }
                    }
                    let Some((from_end, st, x)) = best else { continue };
                    let sr = if at_end { rs[rs.len() - 1] - from_end } else { from_end };
                    let (lo, hi) = if at_end { (sr, rs[rs.len() - 1]) } else { (0.0, sr) };
                    let kept = rs[rs.len() - 1] - from_end;
                    if kept < 1000.0 || in_town(x) {
                        continue;
                    }
                    // The stretch past the crossing keeps beside the better road.
                    let rest = sub_line(&r.pts, &rs, lo, hi);
                    let beside = if direct {
                        let t_rest = if end_key(t.pts[0], towns) == ek { st } else { ts[ts.len() - 1] - st };
                        from_end <= 1.5 * t_rest && rest.iter().all(|p| line_dist(*p, &t.pts) < NEAR)
                    } else {
                        // (Not where the way round over the feeder is much the longer.)
                        feeds.iter().any(|&(s, f, lf)| {
                            let t_rest = lf + (st - s).abs();
                            from_end <= 1.5 * t_rest && t_rest <= 1.6 * from_end + 2_000.0 && rest.iter().all(|p| line_dist(*p, &t.pts).min(line_dist(*p, &roads[f].pts)) < NEAR)
                        })
                    };
                    if beside {
                        cut = Some((i, k, sr, st, x, at_end));
                        break 'find;
                    }
                }
            }
        }
        let Some((i, k, sr, st, x, at_end)) = cut else { break };
        let (r, t) = (roads[i].clone(), roads[k].clone());
        let mut pieces = split_road(&r, &arc(&r.pts), &[(sr, x)]);
        let kept = if at_end { pieces.swap_remove(0) } else { pieces.pop().unwrap() };
        let halves = split_road(&t, &arc(&t.pts), &[(st, x)]);
        let (lo, hi) = (i.min(k), i.max(k));
        roads.remove(hi);
        roads.remove(lo);
        roads.push(kept);
        roads.extend(halves);
        cuts.push(x);
    }
    cuts
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
        // (A drawn road and another keep their own names: they stay two.)
        let Some((&(i, si), &(k, sk))) = at.iter().filter(|(key, v)| key.0 != i64::MIN && v.len() == 2 && v[0].0 != v[1].0 && roads[v[0].0].stroke == roads[v[1].0].stroke).map(|(_, v)| (&v[0], &v[1])).next() else { break };
        // Road i ends at the point, road k starts there.
        let a = if si { flipped(&roads[i]) } else { roads[i].clone() };
        let b = if sk { roads[k].clone() } else { flipped(&roads[k]) };
        let joint = a.pts.len() - 1;
        let mut joined = RoadPath { class: a.class.min(b.class), pts: a.pts, z: a.z, wander: a.wander, stroke: a.stroke, spine: a.spine && b.spine };
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

/// A road that runs within a mile of a way at least as good between its two ends, no more
/// than 35% longer, only makes a lens of two roads: it goes. So do two roads of one class
/// running straight on through a crossroads (the other two roads keep it), or through a
/// junction where a side road joins them (a lesser road beside a better one between the same
/// two junctions): the side road goes on along as little of one of the two as it needs, then
/// straight to the way (within `BESIDE`) on a ramp in its grade, dry, crossing no road, its
/// junction clear of river crossings (else the two stay). The best roads are kept first; drawn
/// roads always. Returns whether any went.
fn drop_lenses(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], ground: &dyn Fn([f64; 2]) -> f64, dry: &dyn Fn(f64, f64) -> bool, crossings: &dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>) -> bool {
    const NEAR: f64 = 5280.0;
    const BESIDE: f64 = 2_000.0;
    let mut any = false;
    // (Ones that kept their road once keep it: the network only loses roads.)
    let mut tried: std::collections::BTreeSet<Vec<u64>> = Default::default();
    loop {
        let ends: Vec<((i64, i64), (i64, i64))> = roads.iter().map(|r| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns))).collect();
        let lens: Vec<f64> = roads.iter().map(road_len).collect();
        let mut at: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
        for (i, e) in ends.iter().enumerate() {
            at.entry(e.0).or_default().push(i);
            at.entry(e.1).or_default().push(i);
        }
        // Candidates, worst first: single roads, then straight pairs through a crossroads or a
        // side road's junction.
        // (With the side road joining a pair, and that junction.)
        let mut cands: Vec<(RoadClass, Vec<usize>, (i64, i64), (i64, i64), Option<(usize, (i64, i64))>)> = Vec::new();
        for (i, r) in roads.iter().enumerate() {
            if r.stroke.is_none() && ends[i].0 != ends[i].1 {
                cands.push((r.class, vec![i], ends[i].0, ends[i].1, None));
            }
        }
        for (k, list) in &at {
            if list.len() < 3 || k.0 == i64::MIN {
                continue;
            }
            for (u, &a) in list.iter().enumerate() {
                for &b in &list[u + 1..] {
                    let (ra, rb) = (&roads[a], &roads[b]);
                    if a == b || ra.class != rb.class || ra.stroke.is_some() || rb.stroke.is_some() {
                        continue;
                    }
                    let side = if list.len() == 3 {
                        let s = list.iter().copied().find(|&s| s != a && s != b);
                        match s {
                            Some(s) if roads[s].class >= ra.class && roads[s].stroke.is_none() && ends[s].0 != ends[s].1 => Some((s, *k)),
                            _ => continue,
                        }
                    } else {
                        None
                    };
                    let far = |i: usize| if ends[i].0 == *k { ends[i].1 } else { ends[i].0 };
                    let heading = |i: usize| {
                        let r = &roads[i];
                        let (p, q) = if ends[i].0 == *k { (r.pts[0], r.pts[1.min(r.pts.len() - 1)]) } else { (r.pts[r.pts.len() - 1], r.pts[r.pts.len() - 2]) };
                        unit2(sub2(q, p))
                    };
                    if far(a) != far(b) && dot2(heading(a), heading(b)) <= THROUGH {
                        let mut set = vec![a, b];
                        set.sort_unstable();
                        cands.push((ra.class, set, far(a), far(b), side));
                    }
                }
            }
        }
        cands.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
        // (The side road's new way on: its index, at its start, the points and heights out to
        // the way, and the way's road cut where it meets it.)
        let mut gone: Option<(Vec<usize>, Option<(usize, bool, Vec<[f64; 2]>, Vec<f32>, usize, Option<f64>)>)> = None;
        for (class, set, p, q, side) in cands {
            if !tried.insert(set.iter().map(|&i| lens[i].to_bits()).collect()) {
                continue;
            }
            let len: f64 = set.iter().map(|&i| lens[i]).sum();
            // Shortest way from p to q on roads at least as good, not through the set.
            let mut best: std::collections::BTreeMap<(i64, i64), (f64, Option<usize>)> = Default::default();
            let mut heap = std::collections::BinaryHeap::new();
            best.insert(p, (0.0, None));
            heap.push((std::cmp::Reverse(0u64), p));
            while let Some((std::cmp::Reverse(d), n)) = heap.pop() {
                let d = f64::from_bits(d);
                if n == q || d > 1.35 * len || best.get(&n).is_some_and(|b| b.0 < d) {
                    continue;
                }
                for &e in at.get(&n).map(|v| v.as_slice()).unwrap_or(&[]) {
                    if set.contains(&e) || roads[e].class > class {
                        continue;
                    }
                    let m = if ends[e].0 == n { ends[e].1 } else { ends[e].0 };
                    let nd = d + lens[e];
                    if best.get(&m).is_none_or(|b| nd < b.0) {
                        best.insert(m, (nd, Some(e)));
                        heap.push((std::cmp::Reverse(nd.to_bits()), m));
                    }
                }
            }
            let Some(&(dq, _)) = best.get(&q) else { continue };
            if dq > 1.35 * len {
                continue;
            }
            // The way's roads, back from q.
            let mut way: Vec<usize> = Vec::new();
            let mut n = q;
            while let Some(&(_, Some(e))) = best.get(&n) {
                way.push(e);
                n = if ends[e].0 == n { ends[e].1 } else { ends[e].0 };
                if way.len() > roads.len() {
                    break;
                }
            }
            let near = |x: [f64; 2]| way.iter().any(|&e| roads[e].pts.windows(2).any(|w| seg_dist(x, w[0], w[1]) < NEAR));
            let along = set.iter().all(|&i| {
                let r = &roads[i];
                let s = arc(&r.pts);
                let n = (s[s.len() - 1] / 300.0).ceil().max(1.0) as usize;
                (0..=n).all(|k| near(point_at(&r.pts, &s, s[s.len() - 1] * k as f64 / n as f64)))
            });
            if !along {
                continue;
            }
            let Some((sd, k)) = side else {
                gone = Some((set, None));
                break;
            };
            if way.contains(&sd) {
                continue;
            }
            // The side road on along one of the two from the junction (as little as it can) and
            // then straight to the nearest point of the way.
            let r = &roads[sd];
            let start = ends[sd].0 == k;
            let gmax = r.class.max_grade();
            let attach = |pp: [f64; 2], zp: f64| -> Option<(usize, Option<f64>, f64, Vec<[f64; 2]>, Vec<f32>)> {
                let mut best_e: Option<(usize, f64, f64)> = None;
                for &e in &way {
                    let (t, d) = project(pp, &roads[e].pts, &arc(&roads[e].pts));
                    if best_e.is_none_or(|b| d < b.2) {
                        best_e = Some((e, t, d));
                    }
                }
                let (e, mut t, d) = best_e?;
                if d > BESIDE {
                    return None;
                }
                let ep = &roads[e].pts;
                let es = arc(ep);
                let elen = es[es.len() - 1];
                // (Near an end of that road, its end: the junction there.)
                let mid = t > 300.0 && t < elen - 300.0;
                if !mid {
                    t = if t <= 300.0 { 0.0 } else { elen };
                }
                let qp = if mid { point_at(ep, &es, t) } else if t == 0.0 { ep[0] } else { ep[ep.len() - 1] };
                let zq = val_at(&roads[e].z, &es, t) as f64;
                let dq = dist(pp, qp);
                if dq < 1.0 {
                    return Some((e, mid.then_some(t), 0.0, vec![qp], vec![zq as f32]));
                }
                let n = (dq / 100.0).ceil().max(1.0) as usize;
                let ramp: Vec<[f64; 2]> = (0..=n).map(|i| [pp[0] + (qp[0] - pp[0]) * i as f64 / n as f64, pp[1] + (qp[1] - pp[1]) * i as f64 / n as f64]).collect();
                let fits = (zq - zp).abs() <= 0.85 * gmax * dq
                    && ramp.iter().enumerate().all(|(i, p)| {
                        let u = dq * i as f64 / n as f64;
                        dry(p[0], p[1]) && (ground(*p) - (zp + (zq - zp) * u / dq)).abs() <= 25.0 + gmax * u.min(dq - u)
                    })
                    && crossings(&ramp).is_empty()
                    && (0..roads.len()).filter(|i| !set.contains(i) && *i != sd).all(|i| !ramp[..n].windows(2).any(|w| crosses(w[0], w[1], &roads[i].pts)))
                    && (!mid || crossings(ep).iter().all(|&(x, hw)| dist(x, qp) >= JUNCTION_WET + WET_STRAY * hw));
                let (z0, z1) = (zp as f32, zq as f32);
                fits.then(|| (e, mid.then_some(t), dq, ramp[1..].to_vec(), (1..=n).map(|i| z0 + (z1 - z0) * i as f32 / n as f32).collect()))
            };
            // (Each of the two from the junction, in 100-ft steps: the first that reaches; the
            // shorter way of the two.)
            let mut pick: Option<(f64, Vec<[f64; 2]>, Vec<f32>, usize, Option<f64>)> = None;
            for &i in &set {
                let l = if end_key(roads[i].pts[0], towns) == k { roads[i].clone() } else { flipped(&roads[i]) };
                let s = arc(&l.pts);
                let len = s[s.len() - 1];
                let mut tp: f64 = 0.0;
                loop {
                    let tp_c = tp.min(len);
                    let pp = if tp_c >= len { l.pts[l.pts.len() - 1] } else { point_at(&l.pts, &s, tp_c) };
                    let zp = val_at(&l.z, &s, tp_c);
                    if let Some((e, cut, dq, ramp, zs)) = attach(pp, zp as f64) {
                        if pick.as_ref().is_none_or(|p| tp_c + dq < p.0) {
                            let keep: Vec<usize> = (1..l.pts.len()).filter(|&j| s[j] < tp_c - 5.0).collect();
                            let mut pts: Vec<[f64; 2]> = keep.iter().map(|&j| l.pts[j]).collect();
                            let mut z: Vec<f32> = keep.iter().map(|&j| l.z[j]).collect();
                            if tp_c > 5.0 && dq >= 1.0 {
                                pts.push(pp);
                                z.push(zp);
                            }
                            pts.extend(ramp);
                            z.extend(zs);
                            pick = Some((tp_c + dq, pts, z, e, cut));
                        }
                        break;
                    }
                    if tp_c >= len {
                        break;
                    }
                    tp += 100.0;
                }
            }
            let Some((_, pts, zs, e, cut)) = pick else { continue };
            gone = Some((set, Some((sd, start, pts, zs, e, cut))));
            break;
        }
        let Some((mut set, side)) = gone else { break };
        if let Some((sd, start, pts, zs, e, cut)) = side {
            let r = &mut roads[sd];
            if start {
                r.pts.splice(0..0, pts.iter().rev().copied());
                r.z.splice(0..0, zs.iter().rev().copied());
                r.wander.splice(0..0, std::iter::repeat_n(0.0, pts.len()));
            } else {
                r.pts.extend(pts.iter().copied());
                r.z.extend(zs.iter().copied());
                r.wander.extend(std::iter::repeat_n(0.0, pts.len()));
            }
            if let Some(t) = cut {
                let s = arc(&roads[e].pts);
                let q = point_at(&roads[e].pts, &s, t);
                let mut pieces = split_road(&roads[e], &s, &[(t, q)]);
                roads[e] = pieces.remove(0);
                roads.extend(pieces);
            }
        }
        set.sort_unstable_by(|x, y| y.cmp(x));
        for i in set {
            roads.remove(i);
        }
        any = true;
    }
    any
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
        if (r.stroke.is_none(), r.class, len(r)) < (o.stroke.is_none(), o.class, len(o)) {
            *e = i;
        }
    }
    let keep: std::collections::BTreeSet<usize> = best.values().copied().collect();
    let mut i = 0;
    roads.retain(|r| {
        i += 1;
        let ends = r.pts.len() >= 2 && key(r.pts[0]) != key(*r.pts.last().unwrap());
        !ends || keep.contains(&(i - 1)) || r.stroke.is_some()
    });
}

/// How far (ft) a junction keeps from where a road crosses a river's meander belt, beyond
/// `WET_STRAY` times the belt's half width (the channel wanders within it, and a road pushed out
/// of it crosses where it runs through).
const JUNCTION_WET: f64 = 0.5 * 5280.0;
const WET_STRAY: f64 = 1.5;
/// Junctions closer than this (ft) along a road are merged.
const JUNCTION_GAP: f64 = 0.5 * 5280.0;
/// Two legs within 35° of straight make a through road.
const THROUGH: f64 = -0.819;
/// Within 70°: a through road where no straighter pair can take the other roads' joins.
const BENT: f64 = -0.342;
/// How far a branch's join may slide along the through road, in its offset from it.
const SKEW_MAX: f64 = 2.5;
/// How far off the through road (cells) a branch's meeting best starts from (`fit_branch`).
const WIDE_OFF: f64 = 0.3;

/// What squaring junctions needs: settlements (centre, trim radius), the cell (ft), dry land and
/// the ground (ft), and where roads cross rivers' meander belts (with each belt's half width).
struct Square<'a> {
    towns: &'a [([f64; 2], f64)],
    cell: f64,
    dry: &'a dyn Fn(f64, f64) -> bool,
    ground: &'a dyn Fn([f64; 2]) -> f64,
    /// Where a line crosses rivers' meander belts (each with the belt's half width).
    crossings: &'a dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>,
    wet: Vec<([f64; 2], f64)>,
    /// Short branches from a settlement may be laid anew to take a junction clear of a river.
    relay: bool,
}

impl Square<'_> {
    /// Far enough from every river crossing.
    fn clear(&self, p: [f64; 2]) -> bool {
        self.wet.iter().all(|&(c, hw)| dist(c, p) >= JUNCTION_WET + WET_STRAY * hw)
    }
    /// Where a road end would count as a settlement's (`end_key`), with some room.
    fn in_town(&self, p: [f64; 2]) -> bool {
        self.towns.iter().any(|&(c, r)| dist(c, p) < 1.2 * r + 100.0)
    }
    /// A ramp from `z0` to `z1` along `curve` that the class can climb and that stays near the
    /// ground (the profile is fitted again later; this only keeps a new stretch off a ridge).
    fn ramp_ok(&self, curve: &[[f64; 2]], z0: f32, z1: f32, gmax: f64) -> bool {
        let s = arc(curve);
        let len = s[s.len() - 1].max(1.0);
        if ((z1 - z0).abs() as f64) > gmax * len + 5.0 {
            return false;
        }
        curve.iter().zip(&s).all(|(c, &u)| ((self.ground)(*c) - (z0 as f64 + (z1 - z0) as f64 * u / len)).abs() <= 25.0 + gmax * u.min(len - u))
    }
}

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// A road the other way round.
fn flipped(r: &RoadPath) -> RoadPath {
    let (pts, z, wander) = (r.pts.iter().rev().copied().collect(), r.z.iter().rev().copied().collect(), r.wander.iter().rev().copied().collect());
    RoadPath { class: r.class, pts, z, wander, stroke: r.stroke, spine: r.spine }
}

/// A value per point (`s`: the line's `arc`) at `t` along it.
fn val_at(v: &[f32], s: &[f64], t: f64) -> f32 {
    let k = s.partition_point(|&x| x < t).clamp(1, s.len() - 1);
    let f = ((t - s[k - 1]) / (s[k] - s[k - 1]).max(1e-9)).clamp(0.0, 1.0);
    v[k - 1] + (v[k] - v[k - 1]) * f as f32
}

/// A road cut at `cuts` (distance along it, the point there; in order): the pieces, each
/// starting and ending exactly on its cut points.
fn split_road(r: &RoadPath, s: &[f64], cuts: &[(f64, [f64; 2])]) -> Vec<RoadPath> {
    let n = r.pts.len();
    let mut bounds: Vec<(f64, [f64; 2], f32, f32)> = vec![(0.0, r.pts[0], r.z[0], r.wander[0])];
    bounds.extend(cuts.iter().map(|&(t, q)| (t, q, val_at(&r.z, s, t), val_at(&r.wander, s, t))));
    bounds.push((s[n - 1], r.pts[n - 1], r.z[n - 1], r.wander[n - 1]));
    bounds
        .windows(2)
        .map(|b| {
            let (lo, hi) = (b[0], b[1]);
            let mid = (0..n).filter(|&k| s[k] > lo.0 + 5.0 && s[k] < hi.0 - 5.0);
            let pts = std::iter::once(lo.1).chain(mid.clone().map(|k| r.pts[k])).chain(std::iter::once(hi.1)).collect();
            let z = std::iter::once(lo.2).chain(mid.clone().map(|k| r.z[k])).chain(std::iter::once(hi.2)).collect();
            let wander = std::iter::once(lo.3).chain(mid.map(|k| r.wander[k])).chain(std::iter::once(hi.3)).collect();
            RoadPath { class: r.class, pts, z, wander, stroke: r.stroke, spine: r.spine }
        })
        .collect()
}

/// A branch's way in from `p` (leaving along `h`) to `q` on the road (`n`: the road's normal on
/// `p`'s side): a cubic to a point a little out from `q` (0.6 of `p`'s offset, at most 0.35 cell),
/// arriving square, then straight in. Both ends included, points ~60 ft apart or more.
fn square_curve(p: [f64; 2], h: [f64; 2], q: [f64; 2], n: [f64; 2], cell: f64) -> Vec<[f64; 2]> {
    let d = (0.6 * dot2(n, sub2(p, q))).min(0.35 * cell);
    let a = [q[0] + n[0] * d, q[1] + n[1] * d];
    let span = dist(p, a);
    let (c1, c2) = ([p[0] + h[0] * 0.4 * span, p[1] + h[1] * 0.4 * span], [a[0] + n[0] * 0.4 * span, a[1] + n[1] * 0.4 * span]);
    let step = (0.05 * cell).max(60.0);
    let m = (span / step).ceil().clamp(3.0, 30.0) as usize;
    let mut out: Vec<[f64; 2]> = (0..=m)
        .map(|i| {
            let t = i as f64 / m as f64;
            let u = 1.0 - t;
            let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            [w0 * p[0] + w1 * c1[0] + w2 * c2[0] + w3 * a[0], w0 * p[1] + w1 * c1[1] + w2 * c2[1] + w3 * a[1]]
        })
        .collect();
    let k = (d / step).ceil().max(1.0) as usize;
    out.extend((1..=k).map(|i| {
        let f = i as f64 / k as f64;
        [a[0] + (q[0] - a[0]) * f, a[1] + (q[1] - a[1]) * f]
    }));
    out
}

/// Where a branch meets the through road: `reach` ft along the branch (from the junction) it
/// leaves its line at `p` and curves in to `q`, `at` along the through road, arriving square.
#[derive(Clone)]
struct Fit {
    reach: f64,
    at: f64,
    q: [f64; 2],
    p: [f64; 2],
    /// From `p` to `q`.
    curve: Vec<[f64; 2]>,
    z_p: f32,
    /// On the left of the through road (as it runs).
    left: bool,
    clear: bool,
    /// From well off the road (`WIDE_OFF`).
    wide: bool,
}

/// How branch `b` (running out from the junction) best meets through road `t` (`ts`: its arc):
/// the nearest reach whose point falls on the road at least 0.15 cell off it, meeting it where it
/// falls (or slid along it, up to 1.3 cell and first towards where the branch comes from, to keep
/// clear of river crossings), or at `fixed` along it. Best first: clear of the through road's
/// river crossings (`JUNCTION_WET`), then out of their meander belts, met where it falls before
/// slid further than the branch's point stands off the road (its way in swings across); then
/// slid more than `SKEW_MAX` times that (it runs alongside, a lens of two roads); then in a belt;
/// then a shorter reach and a shorter slide. One from where the branch stands `WIDE_OFF` or more
/// off the road (and clear of the branch's own crossings) is taken over one from nearer it unless
/// it ranks worse than a slide: from nearer, at a narrow fork, the way in turns so short that the
/// branch still leaves at a slant. No meeting doubles back, crosses more rivers than the branch
/// did, enters water or a settlement, or climbs too steeply.
fn fit_branch(sq: &Square, t: &RoadPath, ts: &[f64], b: &RoadPath, fixed: Option<f64>, min_reach: f64) -> Option<Fit> {
    let cell = sq.cell;
    let bs = arc(&b.pts);
    let (blen, total) = (bs[bs.len() - 1], ts[ts.len() - 1]);
    let gmax = b.class.max_grade().min(t.class.max_grade());
    let t_wet = (sq.crossings)(&t.pts);
    // (Clear of where the through road crosses rivers, and of any crossing on the way in; a
    // branch's own crossing further out is no matter: a road leaving one along the bank.)
    let far = |q: [f64; 2], w: &[([f64; 2], f64)], m: f64| w.iter().all(|&(c, hw)| dist(c, q) >= m + WET_STRAY * hw);
    let mut best: Option<(u8, Fit)> = None;
    // (The best meeting from where the branch stands well off the road: one from nearer it, at a
    // narrow fork, turns in so short that it still leaves at a slant. Taken unless worse.)
    let mut wide: Option<(u8, Fit)> = None;
    // (Nor where it meets the road near where the branch itself crosses a river.)
    let mut b_wet: Option<Vec<([f64; 2], f64)>> = None;
    // (A short branch from a settlement to a junction at a river crossing may be laid anew from
    // near the settlement's edge, slid as far as it takes: nothing else would take it clear.)
    let most = if sq.relay && !sq.clear(b.pts[0]) && sq.in_town(b.pts[b.pts.len() - 1]) { blen } else { 0.5 * blen };
    // (A lesser road out to a settlement near the junction may swing in from further along it,
    // past its half: one bending off a better road at a narrow fork meets it square that way.)
    let end_in_town = b.class > t.class && sq.in_town(b.pts[b.pts.len() - 1]);
    let late: Vec<f64> = if end_in_town && most < blen { (1..=6).map(|k| blen - 0.05 * k as f64 * cell).filter(|&r| r > most && r > 0.4 * cell).rev().collect() } else { Vec::new() };
    for reach in [0.4, 0.55, 0.7, 0.9, 1.15, 1.5, 2.0, 2.5].iter().map(|f| (f * cell).max(min_reach)).take_while(|&r| r <= most).chain(late) {
        let p = point_at(&b.pts, &bs, reach);
        let (at0, off) = project(p, &t.pts, ts);
        if off < 0.15 * cell {
            continue;
        }
        let is_wide = off >= WIDE_OFF * cell;
        // Heading at `p` towards the junction.
        let h = unit2(sub2(point_at(&b.pts, &bs, reach - 0.03 * cell), p));
        let z_p = val_at(&b.z, &bs, reach);
        let mut wet_in: Option<usize> = None;
        // Slid towards where the branch comes from first, at each distance.
        let tan0 = unit2(sub2(point_at(&t.pts, ts, at0 + 40.0), point_at(&t.pts, ts, at0 - 40.0)));
        let back = if dot2(h, tan0) > 0.0 { -1.0 } else { 1.0 };
        let slides: &[f64] = if fixed.is_some() { &[0.0] } else { &[0.0, 0.15, -0.15, 0.3, -0.3, 0.5, -0.5, 0.75, -0.75, 1.0, -1.0, 1.3, -1.3] };
        for &d in slides {
            let at = fixed.unwrap_or(at0 + back * d * cell);
            if at < 0.15 * cell || at > total - 0.15 * cell {
                continue;
            }
            let q = point_at(&t.pts, ts, at);
            let slid = if fixed.is_none() { (at - at0).abs() / off } else { 0.0 };
            let rank = |clear: bool, belt: bool| match (slid > SKEW_MAX && most < blen, clear, belt) {
                (false, true, _) => (slid > 1.0) as u8,
                (false, false, true) => 2 + (slid > 1.0) as u8,
                (true, true, _) => 4,
                (true, false, true) => 5,
                _ => 6 + (slid > 1.0) as u8 + (slid > SKEW_MAX) as u8,
            };
            let r0 = rank(far(q, &t_wet, JUNCTION_WET), far(q, &t_wet, 300.0));
            if best.as_ref().is_some_and(|b| b.0 <= r0) && (!is_wide || wide.as_ref().is_some_and(|b| b.0 <= r0)) || sq.in_town(q) {
                continue;
            }
            let tan = unit2(sub2(point_at(&t.pts, ts, at + 40.0), point_at(&t.pts, ts, at - 40.0)));
            let left = tan[0] * (p[1] - q[1]) - tan[1] * (p[0] - q[0]) > 0.0;
            let n = if left { [-tan[1], tan[0]] } else { [tan[1], -tan[0]] };
            if dot2(unit2(sub2(q, p)), h) < -0.1 || dot2(n, sub2(p, q)) < 0.12 * cell {
                continue;
            }
            let curve = square_curve(p, h, q, n, cell);
            let k = curve.len() - 1;
            // Dry, out of settlements, and on its own side of the road until it meets it.
            if !curve.iter().all(|c| (sq.dry)(c[0], c[1]) && !sq.in_town(*c)) || !curve[..k - 1].iter().all(|c| dot2(n, sub2(*c, q)) > 5.0) {
                continue;
            }
            // (Laid anew, it may not cross the road it meets on the way: the road can bend.)
            if most == blen && curve[..k - 1].windows(2).any(|w| crosses(w[0], w[1], &t.pts)) {
                continue;
            }
            if !sq.ramp_ok(&curve, z_p, val_at(&t.z, ts, at), gmax) {
                continue;
            }
            // (No more river crossings than the branch had from `p` in: a way in across a river
            // that its line keeps on one side of puts the branch over it and back.)
            let wet_in = *wet_in.get_or_insert_with(|| (sq.crossings)(&sub_line(&b.pts, &bs, 0.0, reach)).len());
            let mut wet = (sq.crossings)(&curve);
            if wet.len() > if most < blen { wet_in } else { 0 } {
                continue;
            }
            wet.extend_from_slice(&t_wet);
            let (clear, belt) = (far(q, &wet, JUNCTION_WET), far(q, &wet, 300.0));
            let r = rank(clear, belt);
            let is_wide = is_wide && far(q, b_wet.get_or_insert_with(|| (sq.crossings)(&b.pts)), JUNCTION_WET);
            let fit = Fit { reach, at, q, p, curve, z_p, left, clear, wide: is_wide };
            if is_wide && wide.as_ref().is_none_or(|b| r < b.0) {
                if r == 0 {
                    return Some(fit);
                }
                wide = Some((r, fit.clone()));
            }
            if best.as_ref().is_none_or(|b| r < b.0) {
                best = Some((r, fit));
            }
        }
    }
    match (wide, best) {
        (Some(w), Some(b)) if w.0 <= b.0.max(1) => Some(w.1),
        (_, b) => b.map(|b| b.1),
    }
}

/// The branch with its end from `fit.p` replaced by the curve in to `fit.q` (running out from
/// the junction, like `b`), the level easing from `z_q` there to the branch's at `p`.
fn rejoined(b: &RoadPath, fit: &Fit, z_q: f32) -> RoadPath {
    let bs = arc(&b.pts);
    let k = fit.curve.len() - 1;
    let mut pts: Vec<[f64; 2]> = fit.curve.iter().rev().copied().collect();
    (pts[0], pts[k]) = (fit.q, fit.p);
    let mut z: Vec<f32> = (0..=k).map(|i| z_q + (fit.z_p - z_q) * i as f32 / k as f32).collect();
    let mut wander = vec![0.0f32; k + 1];
    let rest = (0..b.pts.len()).filter(|&i| bs[i] > fit.reach + 5.0);
    pts.extend(rest.clone().map(|i| b.pts[i]));
    z.extend(rest.clone().map(|i| b.z[i]));
    wander.extend(rest.map(|i| b.wander[i]));
    RoadPath { class: b.class, pts, z, wander, stroke: b.stroke, spine: b.spine }
}

/// Road ends by point (exact): (road, at its start).
fn ends_by_point(roads: &[RoadPath]) -> std::collections::BTreeMap<(u64, u64), Vec<(usize, bool)>> {
    let key = |p: [f64; 2]| (p[0].to_bits(), p[1].to_bits());
    let mut ends: std::collections::BTreeMap<(u64, u64), Vec<(usize, bool)>> = Default::default();
    for (ri, r) in roads.iter().enumerate() {
        if r.pts.len() >= 2 {
            ends.entry(key(r.pts[0])).or_default().push((ri, true));
            ends.entry(key(r.pts[r.pts.len() - 1])).or_default().push((ri, false));
        }
    }
    ends
}

/// Junctions away from settlements become the joins of real roads. Grid routes meet at a cell,
/// often at a sharp V (two roads leaving a few tens of degrees apart, so travel between them
/// doubles back), and roads sharing a bridge part right on the bank. At each junction:
/// - the straightest pair of roads (within 35° of straight; the better class first) is the
///   through road: one line, its bend at the junction rounded (`fillet`), otherwise as it was
///   (but a pair as near straight that every branch meets from well off it comes first);
/// - every other road (a branch) leaves its line some way out and curves in to meet the through
///   road square, where its point there falls on it (`fit_branch`): the join slides along the
///   through road towards where the branch comes from, so nothing runs alongside and doubles
///   back. Joins are kept `JUNCTION_WET` from river crossings (sliding further, or reaching
///   further out along the branch), so roads sharing a bridge stay one road off it;
/// - two branches meeting it on opposite sides within `JUNCTION_GAP` of each other meet it at one
///   point (a crossroads); two on one side that close join each other further out (the lesser
///   ends on the greater, and that junction is squared in its turn);
/// - where a branch cannot meet the straightest pair, the next pair takes it, then pairs bending
///   up to 70°; at a junction standing in a meander belt, pairs crossing the river there come
///   last (the road along the bank runs on, the one crossing leaves it);
/// - where no pair can (equal roads parting in open ground) the fork stays.
///
/// Then junctions closer than `JUNCTION_GAP` along a road are merged into one (and squared again).
/// Points in ft; nothing is reshaped within a settlement's trim radius, over water or where the
/// new stretch could not keep to its grade (levels ease in as the profile is fitted later).
fn square_junctions(
    roads: &mut Vec<RoadPath>,
    towns: &[([f64; 2], f64)],
    cell: f64,
    dry: &dyn Fn(f64, f64) -> bool,
    ground: &dyn Fn([f64; 2]) -> f64,
    crossings: &dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>,
    only: Option<&[[f64; 2]]>,
) {
    let river_hits = |roads: &[RoadPath]| -> Vec<([f64; 2], f64)> { roads.iter().flat_map(|r| crossings(&r.pts)).collect() };
    unshare(roads, towns, ground, dry, crossings, true);
    let mut sq = Square { towns, cell, dry, ground, crossings, wet: river_hits(roads), relay: only.is_some() };
    // Points already squared (or left as they are) are not looked at again; with `only`, every
    // junction but those.
    let mut settled: std::collections::BTreeSet<(u64, u64)> = Default::default();
    if let Some(only) = only {
        settled.extend(ends_by_point(roads).into_keys().filter(|k| !only.iter().any(|q| (q[0].to_bits(), q[1].to_bits()) == *k)));
    }
    for round in 0..4 {
        loop {
            let ends = ends_by_point(roads);
            let Some((k, list)) = ends.into_iter().find(|(k, l)| l.len() >= 3 && !settled.contains(k)) else { break };
            settled.insert(k);
            let j = [f64::from_bits(k.0), f64::from_bits(k.1)];
            for q in square_at(roads, j, &list, &sq) {
                settled.insert((q[0].to_bits(), q[1].to_bits()));
            }
        }
        if round == 3 || !close_junctions(roads, &sq) {
            break;
        }
        sq.wet = river_hits(roads);
    }
}

/// Two roads leaving a junction (away from settlements) along one line, side by side (the grid
/// routed both along the same cells), become one road out to where they part: the better one
/// (class, then length) carries both, its first stretch taking the better class, and the other
/// leaves it there (that junction is squared in its turn). So does one that wanders off first
/// and comes back to run along the other (two roads down one valley from a pass, each with its
/// own switchbacks); of only two roads there, the stretch to where they part goes. One climbing
/// beside the other out of the junction (a switchback's leg above the other's line) rides it
/// instead, leaving it where a straight ramp in its grade reaches its own line further on; before
/// squaring (`mixed`) a lesser one beside a better one does too (else squaring takes the lesser
/// one's line through and bends the better one to it, a hook). One pair at a time, at most 200.
fn unshare(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], ground: &dyn Fn([f64; 2]) -> f64, dry: &dyn Fn(f64, f64) -> bool, crossings: &dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>, mixed: bool) {
    const TOL: f64 = 150.0;
    const STEP: f64 = 50.0;
    const LATE: f64 = 4.0 * 5280.0;
    const BESIDE: f64 = 1_900.0;
    let mut tried: std::collections::BTreeSet<(u64, u64)> = Default::default();
    for _ in 0..200 {
        let ends = ends_by_point(roads);
        let mut done = false;
        for (k, list) in &ends {
            let j = [f64::from_bits(k.0), f64::from_bits(k.1)];
            if list.len() < 2 || tried.contains(k) || towns.iter().any(|&(t, r)| dist(t, j) < r) || list.iter().any(|e| roads[e.0].stroke.is_some()) {
                continue;
            }
            // (Two ends at a settlement's edge are its ways in, not a spike.)
            if list.len() == 2 && towns.iter().any(|&(t, r)| dist(t, j) < 2.0 * r + 1_000.0) {
                continue;
            }
            let mut ids: Vec<usize> = list.iter().map(|e| e.0).collect();
            ids.sort_unstable();
            ids.dedup();
            if ids.len() != list.len() {
                continue;
            }
            let legs: Vec<RoadPath> = list.iter().map(|&(ri, start)| if start { roads[ri].clone() } else { flipped(&roads[ri]) }).collect();
            let arcs: Vec<Vec<f64>> = legs.iter().map(|l| arc(&l.pts)).collect();
            let mut found = None;
            'pairs: for x in 0..legs.len() {
                for y in x + 1..legs.len() {
                    let key = |i: usize| (legs[i].class, !legs[i].spine, -arcs[i][arcs[i].len() - 1]);
                    let (c, o) = if key(x) <= key(y) { (x, y) } else { (y, x) };
                    let (os, cs) = (&arcs[o], &arcs[c]);
                    let (olen, clen) = (os[os.len() - 1], cs[cs.len() - 1]);
                    // (One that wanders off first, a loop or a switchback, and comes back to run
                    // along the other a good way from near the junction is laid on it all the same.)
                    let head = sub_line(&legs[c].pts, cs, 0.0, clen.min(2.0 * LATE + 5_000.0));
                    let near = |t: f64| near_line(point_at(&legs[o].pts, os, t), &head, TOL);
                    let mut shared = 0.0;
                    while shared + STEP < olen && near(shared + STEP) {
                        shared += STEP;
                    }
                    if shared < 300.0 {
                        let mut from = shared + STEP;
                        while from < LATE && from + STEP < olen && !near(from) {
                            from += STEP;
                        }
                        let mut end = from;
                        while from < LATE && end + STEP < olen && near(end + STEP) {
                            end += STEP;
                        }
                        // (Not where the other's way there is much the longer.)
                        let via = project(point_at(&legs[o].pts, os, from), &legs[c].pts, cs).0;
                        if end - from >= 1_500.0 && via <= 1.5 * from + 500.0 {
                            shared = end;
                        }
                    }
                    // (Beside it out of the junction, a fine step or so off and a good way: rides
                    // it to the end of that, then leaves it on a ramp to its own line.)
                    if shared < 300.0 && (legs[o].class == legs[c].class || mixed) {
                        let beside = |t: f64| near_line(point_at(&legs[o].pts, os, t), &head, BESIDE);
                        let mut end = 0.0;
                        while end + STEP < olen && end < LATE && beside(end + STEP) {
                            end += STEP;
                        }
                        let at = project(point_at(&legs[o].pts, os, end), &legs[c].pts, cs).0;
                        let q = point_at(&legs[c].pts, cs, at);
                        // (Its junction where they part clear of river crossings.)
                        let clear = || crossings(&head).iter().chain(crossings(&legs[o].pts).iter()).all(|&(x, hw)| dist(x, q) >= JUNCTION_WET + WET_STRAY * hw);
                        if end >= 3_000.0 && at > 200.0 && at < clen - 300.0 && clear() {
                            let zq = val_at(&legs[c].z, cs, at) as f64;
                            let gmax = 0.85 * legs[o].class.max_grade();
                            let mut t = end + 2.0 * STEP;
                            while t < (end + 3.0 * 5280.0).min(olen - 300.0) {
                                let p = point_at(&legs[o].pts, os, t);
                                let (d, zp) = (dist(q, p), val_at(&legs[o].z, os, t) as f64);
                                let n = (d / 100.0).ceil().max(2.0) as usize;
                                let ramp: Vec<[f64; 2]> = (0..=n).map(|i| [q[0] + (p[0] - q[0]) * i as f64 / n as f64, q[1] + (p[1] - q[1]) * i as f64 / n as f64]).collect();
                                let fits = d > 600.0
                                    && (zp - zq).abs() <= gmax * d
                                    && at + d <= 1.6 * t + 1_000.0
                                    && ramp.iter().enumerate().all(|(i, r)| {
                                        let u = d * i as f64 / n as f64;
                                        dry(r[0], r[1]) && (ground(*r) - (zq + (zp - zq) * u / d)).abs() <= 25.0 + legs[o].class.max_grade() * u.min(d - u)
                                    })
                                    && !ramp[1..n].windows(2).any(|w| crosses(w[0], w[1], &legs[c].pts));
                                if fits {
                                    let (z0, z1) = (zq as f32, zp as f32);
                                    found = Some((c, o, t, at, ramp[1..n].iter().enumerate().map(|(i, r)| (*r, z0 + (z1 - z0) * (i + 1) as f32 / n as f32)).collect()));
                                    break 'pairs;
                                }
                                t += 2.0 * STEP;
                            }
                        }
                    }
                            // (One along the other all the way keeps only its last bit, to where it ends.)
                    if shared < 300.0 {
                        continue;
                    }
                    let at = project(point_at(&legs[o].pts, os, shared), &legs[c].pts, cs).0;
                    if at > 200.0 && at < clen - 300.0 {
                        found = Some((c, o, shared, at, Vec::new()));
                        break 'pairs;
                    }
                }
            }
            tried.insert(*k);
            let Some((c, o, shared, at, ramp)) = found else { continue };
            let (lc, lo) = (&legs[c], &legs[o]);
            let q = point_at(&lc.pts, &arcs[c], at);
            let mut pieces = split_road(lc, &arcs[c], &[(at, q)]);
            pieces[0].class = lc.class.min(lo.class);
            // (Two roads only, out and back along one line: the stretch to where they part
            // leads nowhere and goes.)
            if list.len() == 2 {
                pieces.remove(0);
            }
            let os = &arcs[o];
            let rest: Vec<usize> = (0..lo.pts.len()).filter(|&i| os[i] > shared + 5.0).collect();
            let mut pts = vec![q];
            let mut z = vec![val_at(&lc.z, &arcs[c], at)];
            let mut wander = vec![0.0f32];
            pts.extend(ramp.iter().map(|r| r.0));
            z.extend(ramp.iter().map(|r| r.1));
            wander.extend(ramp.iter().map(|_| 0.0f32));
            pts.extend(rest.iter().map(|&i| lo.pts[i]));
            z.extend(rest.iter().map(|&i| lo.z[i]));
            wander.extend(rest.iter().map(|&i| lo.wander[i]));
            pieces.push(RoadPath { class: lo.class, pts, z, wander, stroke: None, spine: lo.spine });
            if pieces.iter().any(|p| p.pts.len() < 2 || road_len(p) < 1.0) {
                continue;
            }
            let mut gone = vec![list[c].0, list[o].0];
            gone.sort_unstable_by(|x, y| y.cmp(x));
            for g in gone {
                roads.remove(g);
            }
            roads.extend(pieces);
            done = true;
            break;
        }
        if !done {
            return;
        }
    }
}

/// Squares one junction (`square_junctions`); returns the points branches now meet the through
/// road at (none where it is left as it was).
fn square_at(roads: &mut Vec<RoadPath>, j: [f64; 2], list: &[(usize, bool)], sq: &Square) -> Vec<[f64; 2]> {
    let cell = sq.cell;
    let mut ids: Vec<usize> = list.iter().map(|e| e.0).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() != list.len() || list.iter().any(|e| roads[e.0].stroke.is_some()) || sq.towns.iter().any(|&(t, r)| dist(t, j) < r) {
        return Vec::new();
    }
    let legs: Vec<RoadPath> = list.iter().map(|&(ri, start)| if start { roads[ri].clone() } else { flipped(&roads[ri]) }).collect();
    let dir: Vec<[f64; 2]> = legs
        .iter()
        .map(|l| {
            let s = arc(&l.pts);
            unit2(sub2(point_at(&l.pts, &s, (0.4 * cell).min(0.45 * s[s.len() - 1])), j))
        })
        .collect();
    // The through road: of pairs near straight, the best class, then the straightest; where
    // a branch cannot meet it, the next, then pairs bending up to 70° (the straightest first).
    // A junction standing in a river's meander belt: pairs crossing it there come last (a road
    // along the bank keeps on through, the one that crosses leaves it).
    let wet: Vec<bool> = legs
        .iter()
        .map(|l| {
            let s = arc(&l.pts);
            (sq.crossings)(&sub_line(&l.pts, &s, 0.0, (0.5 * cell).min(s[s.len() - 1]))).iter().any(|&(c, hw)| dist(c, j) < WET_STRAY * hw + 300.0)
        })
        .collect();
    let mut pairs: Vec<((u8, u8, RoadClass, RoadClass, f64), usize, usize)> = Vec::new();
    for a in 0..legs.len() {
        for b in a + 1..legs.len() {
            let c = dot2(dir[a], dir[b]);
            // (A realm's track through: its villages' spurs leave it, whatever lines up with a spur.)
            let w = (wet[a] || wet[b]) as u8 + 2 * !(legs[a].spine && legs[b].spine) as u8;
            if c <= THROUGH {
                pairs.push(((w, 0, legs[a].class.max(legs[b].class), legs[a].class.min(legs[b].class), c), a, b));
            } else if c <= BENT {
                pairs.push(((w, 1, RoadClass::KingsRoad, RoadClass::KingsRoad, c), a, b));
            }
        }
    }
    pairs.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(Ordering::Equal).then((x.1, x.2).cmp(&(y.1, y.2))));
    // (Of pairs as near straight, one its branches meet from well off it, square, comes first:
    // at a narrow fork the two roads close together are the better through road.)
    let mut first: Option<(usize, usize, RoadPath, Vec<f64>, Vec<Fit>)> = None;
    let mut group = None;
    let found = pairs.iter().find_map(|&(key, a, b)| {
        if group.is_some_and(|g| g != (key.0, key.1)) {
            return None;
        }
        let mut t = flipped(&legs[a]);
        let joint = t.pts.len() - 1;
        t.pts.extend_from_slice(&legs[b].pts[1..]);
        t.z.extend_from_slice(&legs[b].z[1..]);
        t.wander.extend_from_slice(&legs[b].wander[1..]);
        t.class = legs[a].class.min(legs[b].class);
        t.spine = legs[a].spine && legs[b].spine;
        t.wander[joint] = 0.0;
        fillet(&mut t, joint, cell, sq.dry);
        let ts = arc(&t.pts);
        let fits: Option<Vec<Fit>> = (0..legs.len()).filter(|&i| i != a && i != b).map(|i| fit_branch(sq, &t, &ts, &legs[i], None, 0.0)).collect();
        let fits = fits?;
        // (Not where that makes a better road a branch: it would leave its own line square
        // and turn back to it, a step; the lesser one meets it as it can.)
        let worse = legs[a].class.max(legs[b].class);
        if fits.iter().all(|f| f.wide && f.clear) && (0..legs.len()).filter(|&i| i != a && i != b).all(|i| legs[i].class >= worse) {
            return Some((a, b, t, ts, fits));
        }
        if first.is_none() {
            group = Some((key.0, key.1));
            first = Some((a, b, t, ts, fits));
        }
        None
    });
    let Some((a, b, t, ts, mut fits)) = found.or(first) else { return Vec::new() };
    let at_j = project(j, &t.pts, &ts).0;
    let branches: Vec<usize> = (0..legs.len()).filter(|&i| i != a && i != b).collect();
    // Branches near each other: opposite sides meet at one point, one side join further out.
    // (`feeds[i]`: branch i ends on branch feeds[i] at that one's `p`, which splits it.)
    let mut feeds: Vec<Option<usize>> = vec![None; fits.len()];
    let mut order: Vec<usize> = (0..fits.len()).collect();
    order.sort_by(|&x, &y| fits[x].at.total_cmp(&fits[y].at).then(x.cmp(&y)));
    for w in 0..order.len().saturating_sub(1) {
        let (x, y) = (order[w], order[w + 1]);
        if feeds[x].is_some() || feeds[y].is_some() || (fits[x].at - fits[y].at).abs() >= JUNCTION_GAP || fits[x].at == fits[y].at {
            continue;
        }
        let (bx, by) = (&legs[branches[x]], &legs[branches[y]]);
        if fits[x].left != fits[y].left {
            // The better road keeps its join; equal ones meet halfway.
            let at = match bx.class.cmp(&by.class) {
                Ordering::Less => fits[x].at,
                Ordering::Greater => fits[y].at,
                Ordering::Equal => 0.5 * (fits[x].at + fits[y].at),
            };
            if let (Some(fx), Some(fy)) = (fit_branch(sq, &t, &ts, bx, Some(at), fits[x].reach), fit_branch(sq, &t, &ts, by, Some(at), fits[y].reach)) {
                if fx.left != fy.left {
                    (fits[x], fits[y]) = (fx, fy);
                    continue;
                }
            }
        } else if same_side(sq, &t, &ts, &legs, &branches, &mut fits, &mut feeds, x, y) {
            continue;
        }
        // Else the lesser (class, then shorter) meets the road far enough from the other.
        let (keep, mv) = if (bx.class, road_len(bx)) <= (by.class, road_len(by)) { (x, y) } else { (y, x) };
        // (On the side the branch comes from.)
        let from = project(fits[mv].p, &t.pts, &ts).0;
        let away = if from >= fits[keep].at { 1.0 } else { -1.0 };
        if let Some(f) = fit_branch(sq, &t, &ts, &legs[branches[mv]], Some(fits[keep].at + away * (JUNCTION_GAP + 0.05 * cell)), 0.0) {
            if f.clear || !fits[mv].clear {
                fits[mv] = f;
            }
        }
    }
    // The through road cut where branches meet it.
    let mut cuts: Vec<(f64, [f64; 2])> = fits.iter().zip(&feeds).filter(|(_, f)| f.is_none()).map(|(f, _)| (f.at, f.q)).collect();
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
    cuts.dedup_by(|x, y| x.0 == y.0);
    let mut pieces = split_road(&t, &ts, &cuts);
    // Each piece's class: the road it mostly was, and the branches' whose traffic now runs along
    // most of it.
    let mut lo = 0.0f64;
    for (k, p) in pieces.iter_mut().enumerate() {
        let hi = if k < cuts.len() { cuts[k].0 } else { ts[ts.len() - 1] };
        let over = |x: f64, y: f64| (hi.min(x.max(y)) - lo.max(x.min(y))).max(0.0);
        let mut class = if over(0.0, at_j) >= 0.5 * (hi - lo) { legs[a].class } else { legs[b].class };
        for (i, f) in fits.iter().enumerate() {
            if feeds[i].is_none() && over(f.at, at_j) > 0.5 * (hi - lo) {
                class = class.min(legs[branches[i]].class);
            }
        }
        p.class = class;
        lo = hi;
    }
    let mut out: Vec<RoadPath> = pieces;
    let mut met = Vec::new();
    for (i, f) in fits.iter().enumerate() {
        let leg = &legs[branches[i]];
        match feeds[i] {
            Some(_) => {
                let z_q = { let m = &legs[branches[feeds[i].unwrap()]]; val_at(&m.z, &arc(&m.pts), fits[feeds[i].unwrap()].reach) };
                out.push(rejoined(leg, f, z_q));
            }
            None => {
                let z_q = val_at(&t.z, &ts, f.at);
                let r = rejoined(leg, f, z_q);
                met.push(f.q);
                // A branch another feeds into is cut where that one meets it.
                if let Some(less) = feeds.iter().position(|x| *x == Some(i)) {
                    let rs = arc(&r.pts);
                    let at = project(fits[less].q, &r.pts, &rs).0;
                    out.extend(split_road(&r, &rs, &[(at, fits[less].q)]));
                } else {
                    out.push(r);
                }
            }
        }
    }
    let mut gone: Vec<usize> = ids;
    gone.sort_unstable_by(|x, y| y.cmp(x));
    for i in gone {
        roads.remove(i);
    }
    roads.extend(out);
    met
}

/// Two branches meeting the through road on one side, near each other: the lesser (class, then
/// shorter) ends on the greater, which meets the road and is met far enough out from it (that
/// junction is squared in its turn). Returns whether they could.
#[allow(clippy::too_many_arguments)]
fn same_side(sq: &Square, t: &RoadPath, ts: &[f64], legs: &[RoadPath], branches: &[usize], fits: &mut [Fit], feeds: &mut [Option<usize>], x: usize, y: usize) -> bool {
    let cell = sq.cell;
    let (bx, by) = (&legs[branches[x]], &legs[branches[y]]);
    let (main, less) = if (bx.class, road_len(bx)) <= (by.class, road_len(by)) { (x, y) } else { (y, x) };
    let Some(fm) = fit_branch(sq, t, ts, &legs[branches[main]], None, fits[main].reach.max(JUNCTION_GAP + 0.1 * cell)) else { return false };
    let lb = &legs[branches[less]];
    let ls = arc(&lb.pts);
    let reach = fits[less].reach;
    let link = [point_at(&lb.pts, &ls, reach), fm.p];
    let n = (dist(link[0], link[1]) / (0.05 * cell).max(60.0)).ceil().max(1.0) as usize;
    let line: Vec<[f64; 2]> = (0..=n).map(|i| i as f64 / n as f64).map(|f| [link[0][0] + (link[1][0] - link[0][0]) * f, link[0][1] + (link[1][1] - link[0][1]) * f]).collect();
    let z_main = val_at(&legs[branches[main]].z, &arc(&legs[branches[main]].pts), fm.reach);
    let z_less = val_at(&lb.z, &ls, reach);
    if fm.clear && line.iter().all(|c| (sq.dry)(c[0], c[1]) && !sq.in_town(*c)) && sq.ramp_ok(&line, z_less, z_main, lb.class.max_grade()) && reach < 0.45 * ls[ls.len() - 1] {
        fits[main] = fm;
        fits[less] = Fit { reach, at: f64::NAN, q: link[1], p: link[0], curve: line, z_p: z_less, left: fits[main].left, clear: true, wide: true };
        feeds[less] = Some(main);
        return true;
    }
    false
}

/// Junctions (away from settlements) joined by a road shorter than `JUNCTION_GAP` become one,
/// halfway along it: each other road there runs on along it to that point. One such pair at a
/// time, shortest first; returns whether any were merged.
fn close_junctions(roads: &mut Vec<RoadPath>, sq: &Square) -> bool {
    let mut merged = false;
    loop {
        let ends = ends_by_point(roads);
        let key = |p: [f64; 2]| (p[0].to_bits(), p[1].to_bits());
        let junction = |k: &(u64, u64)| ends[k].len() >= 3 && !sq.towns.iter().any(|&(t, r)| dist(t, [f64::from_bits(k.0), f64::from_bits(k.1)]) < r);
        let mut short: Vec<(f64, usize)> = roads
            .iter()
            .enumerate()
            .filter(|(_, r)| r.pts.len() >= 2 && r.stroke.is_none())
            .filter(|(_, r)| {
                let (a, b) = (key(r.pts[0]), key(r.pts[r.pts.len() - 1]));
                a != b && junction(&a) && junction(&b)
            })
            .map(|(i, r)| (road_len(r), i))
            .filter(|&(l, _)| l < JUNCTION_GAP)
            .collect();
        short.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        let mut done = false;
        for (len, i) in short {
            let r = &roads[i];
            let (a, b) = (key(r.pts[0]), key(r.pts[r.pts.len() - 1]));
            // Not where another road joins the same two (merging would make it a loop).
            let others: Vec<(usize, bool)> = ends[&a].iter().chain(&ends[&b]).copied().filter(|e| e.0 != i).collect();
            let mut seen: Vec<usize> = others.iter().map(|e| e.0).collect();
            seen.sort_unstable();
            if seen.windows(2).any(|w| w[0] == w[1]) || others.iter().any(|e| roads[e.0].stroke.is_some()) {
                continue;
            }
            let s = arc(&r.pts);
            let m = point_at(&r.pts, &s, 0.5 * len);
            if !(sq.dry)(m[0], m[1]) || !sq.clear(m) {
                continue;
            }
            let halves = split_road(r, &s, &[(0.5 * len, m)]);
            // From each end: the road carrying on along it runs on to the middle; the others
            // end there on a chord from 0.25 cell back (squared again after).
            let to_mid = [halves[0].clone(), flipped(&halves[1])];
            let mut gone = vec![i];
            let mut add = Vec::new();
            let mut ok = true;
            for (side, k) in [a, b].iter().enumerate() {
                let half = &to_mid[side];
                let o = half.pts[0];
                let along = unit2(sub2(point_at(&half.pts, &arc(&half.pts), (0.1 * sq.cell).min(0.5 * len)), o));
                let ins: Vec<(usize, bool, RoadPath)> =
                    ends[k].iter().filter(|e| e.0 != i).map(|&(ri, start)| (ri, start, if start { flipped(&roads[ri]) } else { roads[ri].clone() })).collect();
                let into = |x: &RoadPath| {
                    let s = arc(&x.pts);
                    let l = s[s.len() - 1];
                    unit2(sub2(o, point_at(&x.pts, &s, l - (0.3 * sq.cell).min(0.45 * l))))
                };
                let main = (0..ins.len()).max_by(|&p, &q| dot2(into(&ins[p].2), along).total_cmp(&dot2(into(&ins[q].2), along)).then(q.cmp(&p))).unwrap_or(0);
                for (n, (ri, start, mut x)) in ins.into_iter().enumerate() {
                    if n == main {
                        x.pts.extend_from_slice(&half.pts[1..]);
                        x.z.extend_from_slice(&half.z[1..]);
                        x.wander.extend_from_slice(&half.wander[1..]);
                    } else {
                        let s = arc(&x.pts);
                        let l = s[s.len() - 1];
                        let back = l - (0.25 * sq.cell).min(0.45 * l);
                        let (p, zp, zm) = (point_at(&x.pts, &s, back), val_at(&x.z, &s, back), half.z[half.z.len() - 1]);
                        let keep: Vec<usize> = (0..x.pts.len()).filter(|&q| s[q] < back - 5.0).collect();
                        let c = (dist(p, m) / (0.05 * sq.cell).max(60.0)).ceil().max(1.0) as usize;
                        let chord: Vec<[f64; 2]> = (0..=c).map(|q| [p[0] + (m[0] - p[0]) * q as f64 / c as f64, p[1] + (m[1] - p[1]) * q as f64 / c as f64]).collect();
                        ok &= chord.iter().all(|q| (sq.dry)(q[0], q[1]) && !sq.in_town(*q));
                        let mut pts: Vec<[f64; 2]> = keep.iter().map(|&q| x.pts[q]).collect();
                        let mut z: Vec<f32> = keep.iter().map(|&q| x.z[q]).collect();
                        let mut wander: Vec<f32> = keep.iter().map(|&q| x.wander[q]).collect();
                        pts.extend_from_slice(&chord);
                        z.extend((0..=c).map(|q| zp + (zm - zp) * q as f32 / c as f32));
                        wander.extend(std::iter::repeat_n(0.0, c + 1));
                        let last = pts.len() - 1;
                        pts[last] = m;
                        (x.pts, x.z, x.wander) = (pts, z, wander);
                    }
                    add.push(if start { flipped(&x) } else { x });
                    gone.push(ri);
                }
            }
            if !ok {
                continue;
            }
            gone.sort_unstable_by(|x, y| y.cmp(x));
            for g in gone {
                roads.remove(g);
            }
            roads.extend(add);
            done = true;
            break;
        }
        if !done {
            return merged;
        }
        merged = true;
    }
}

/// Drops control points (ft) that only make the road jog: closer than 40 ft to the point
/// before or after, or where it doubles back (turns over 100°) within 250 ft; never an end,
/// and only where the chord left keeps the road's grade.
pub fn tidy(r: &mut RoadPath) {
    cut_spikes(r);
    let gmax = r.class.max_grade();
    // The first and last stretch stay as shaped (a branch's way into a junction, the way into a gate).
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

/// Eases turns still too sharp for a cart (ft): where the road's heading over 40 ft before a
/// point and 40 ft after it differ by over 70°, the stretch from ~0.05 cell before to as far
/// after becomes a cubic keeping the headings it arrives and leaves with (left as it is where
/// that would cross water). Not where that stretch would reach the first or last two points.
pub fn ease_turns(r: &mut RoadPath, cell: f64, dry: &dyn Fn(f64, f64) -> bool) {
    const CHORD: f64 = 40.0;
    let reach = 0.05 * cell;
    let mut i = 2;
    while i + 2 < r.pts.len() {
        let n = r.pts.len();
        // The point at least `d` ft along the road from `i` (dir ±1), within [1, n - 2].
        let walk = |i: usize, dir: i64, d: f64| {
            let (mut k, mut acc) = (i as i64, 0.0);
            while acc < d && k + dir >= 1 && k + dir <= n as i64 - 2 {
                acc += dist(r.pts[k as usize], r.pts[(k + dir) as usize]);
                k += dir;
            }
            k as usize
        };
        let (a, b) = (walk(i, -1, CHORD), walk(i, 1, CHORD));
        let (u, v) = (unit2([r.pts[i][0] - r.pts[a][0], r.pts[i][1] - r.pts[a][1]]), unit2([r.pts[b][0] - r.pts[i][0], r.pts[b][1] - r.pts[i][1]]));
        if a == i || b == i || dot2(u, v) >= libm::cos(70f64.to_radians()) {
            i += 1;
            continue;
        }
        let (i0, i1) = (walk(i, -1, reach), walk(i, 1, reach));
        // (Near an end the road keeps its way in: a junction's or a gate's.)
        let room = |a: usize, b: usize| (a.min(b)..a.max(b)).map(|k| dist(r.pts[k], r.pts[k + 1])).sum::<f64>() >= reach;
        if !room(i0, i) || !room(i, i1) {
            i += 1;
            continue;
        }
        let (p0, p3) = (r.pts[i0], r.pts[i1]);
        let h0 = unit2([p0[0] - r.pts[i0 - 1][0], p0[1] - r.pts[i0 - 1][1]]);
        let h1 = unit2([r.pts[i1 + 1][0] - p3[0], r.pts[i1 + 1][1] - p3[1]]);
        let span = dist(p0, p3);
        let (c1, c2) = ([p0[0] + h0[0] * 0.4 * span, p0[1] + h0[1] * 0.4 * span], [p3[0] - h1[0] * 0.4 * span, p3[1] - h1[1] * 0.4 * span]);
        let m = (span / 30.0).ceil().clamp(3.0, 40.0) as usize;
        let curve: Vec<[f64; 2]> = (1..m)
            .map(|k| {
                let t = k as f64 / m as f64;
                let w = 1.0 - t;
                let (a, b, c, d) = (w * w * w, 3.0 * w * w * t, 3.0 * w * t * t, t * t * t);
                [a * p0[0] + b * c1[0] + c * c2[0] + d * p3[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p3[1]]
            })
            .collect();
        if i1 <= i0 + 1 || curve.iter().any(|p| !dry(p[0], p[1])) {
            i += 1;
            continue;
        }
        let (z0, z1) = (r.z[i0], r.z[i1]);
        r.pts.splice(i0 + 1..i1, curve);
        r.z.splice(i0 + 1..i1, (1..m).map(|k| z0 + (z1 - z0) * k as f32 / m as f32));
        r.wander.splice(i0 + 1..i1, std::iter::repeat_n(0.0, m - 1));
        i = i0 + m;
    }
}

/// Moves a road (ft) off standing water it still runs into (a lake's water between the cells it
/// was routed on): each run of wet points (not an end) slides sideways, to the side that needs
/// less, by as much as its wettest point needs, eased in and out over `EASE` ft; left as it is
/// where no move within `REACH` ft keeps it dry. One run per call: whether one moved. (The
/// profile is fitted again afterwards.)
pub fn keep_dry(r: &mut RoadPath, dry: &dyn Fn(f64, f64) -> bool) -> bool {
    const REACH: f64 = 1500.0;
    const EASE: f64 = 600.0;
    let n = r.pts.len();
    let wet: Vec<bool> = r.pts.iter().map(|p| !dry(p[0], p[1])).collect();
    let mut k = 1;
    while k + 1 < n {
        if !wet[k] {
            k += 1;
            continue;
        }
        let i0 = k;
        while k + 1 < n && wet[k] {
            k += 1;
        }
        let i1 = k - 1;
        if i1 + 1 >= n - 1 && wet[n - 1] {
            break;
        }
        let normal = |i: usize| {
            let (a, b) = (r.pts[i - 1], r.pts[(i + 1).min(n - 1)]);
            let t = unit2([b[0] - a[0], b[1] - a[1]]);
            [-t[1], t[0]]
        };
        // Per side: the offset the wettest point needs.
        let need = |side: f64| {
            let mut most = 0.0f64;
            for i in i0..=i1 {
                let nrm = normal(i);
                let d = (1..=(REACH / 50.0) as usize).map(|m| m as f64 * 50.0).find(|&d| dry(r.pts[i][0] + side * nrm[0] * d, r.pts[i][1] + side * nrm[1] * d))?;
                most = most.max(d);
            }
            Some(most + 60.0)
        };
        let pick = match (need(1.0), need(-1.0)) {
            (Some(a), Some(b)) => Some(if a <= b { (1.0, a) } else { (-1.0, b) }),
            (Some(a), None) => Some((1.0, a)),
            (None, Some(b)) => Some((-1.0, b)),
            (None, None) => None,
        };
        let Some((side, off)) = pick else {
            continue;
        };
        // Distance along the road from the run, for the easing.
        let mut s = vec![0.0; n];
        for i in 1..n {
            s[i] = s[i - 1] + dist(r.pts[i - 1], r.pts[i]);
        }
        let moved: Vec<(usize, f64, [f64; 2])> = (1..n - 1)
            .filter_map(|i| {
                let gap = if i < i0 { s[i0] - s[i] } else if i > i1 { s[i] - s[i1] } else { 0.0 };
                (gap < EASE).then(|| {
                    // (A point the planned offset leaves wet takes the nearest dry one to it.)
                    let (f, nrm, p) = (1.0 - smooth01(gap / EASE), normal(i), r.pts[i]);
                    let at = |d: f64| [p[0] + side * nrm[0] * d, p[1] + side * nrm[1] * d];
                    let d = (0..=(REACH / 50.0) as usize).flat_map(|m| [off * f + m as f64 * 50.0, off * f - m as f64 * 50.0]).filter(|&d| (0.0..=REACH).contains(&d)).find(|&d| dry(at(d)[0], at(d)[1]));
                    (i, d.unwrap_or(off * f), nrm)
                })
            })
            .collect();
        // Offsets smoothed (raised to their neighbours' mean) where that stays dry: no kinks.
        let place = |m: &[(usize, f64, [f64; 2])]| m.iter().map(|&(i, d, nrm)| (i, [r.pts[i][0] + side * nrm[0] * d, r.pts[i][1] + side * nrm[1] * d])).collect::<Vec<(usize, [f64; 2])>>();
        let mut smooth = moved.clone();
        for _ in 0..3 {
            let prev = smooth.clone();
            for m in 1..smooth.len().saturating_sub(1) {
                smooth[m].1 = prev[m].1.max(0.5 * (prev[m - 1].1 + prev[m + 1].1));
            }
        }
        let (rough, smooth) = (place(&moved), place(&smooth));
        let moved = if smooth.iter().all(|(_, q)| dry(q[0], q[1])) { smooth } else { rough };
        if moved.iter().all(|(_, q)| dry(q[0], q[1])) {
            let (lo, hi) = (moved[0].0, moved[moved.len() - 1].0);
            for (i, q) in moved {
                r.pts[i] = q;
            }
            // Points left jogging there (turning over 45°) go where the chord past them stays dry.
            let chord_dry = |a: [f64; 2], b: [f64; 2]| {
                let m = (dist(a, b) / 50.0).ceil().max(1.0) as usize;
                (0..=m).all(|j| dry(a[0] + (b[0] - a[0]) * j as f64 / m as f64, a[1] + (b[1] - a[1]) * j as f64 / m as f64))
            };
            let mut drop = vec![false; n];
            let mut last = lo - 1;
            for i in lo..=hi {
                let (a, b, c) = (r.pts[last], r.pts[i], r.pts[i + 1]);
                if dot2(unit2(sub2(b, a)), unit2(sub2(c, b))) < libm::cos(45f64.to_radians()) && chord_dry(a, c) {
                    drop[i] = true;
                } else {
                    last = i;
                }
            }
            let keep: Vec<usize> = (0..n).filter(|&i| !drop[i]).collect();
            r.pts = keep.iter().map(|&i| r.pts[i]).collect();
            r.z = keep.iter().map(|&i| r.z[i]).collect();
            r.wander = keep.iter().map(|&i| r.wander[i]).collect();
            return true;
        }
    }
    false
}

fn smooth01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Cuts out-and-back spikes (ft): where a road turns back (over ~155°) and retraces its way
/// within 60 ft (two roads that climbed the same switchbacks, joined into one), the stretch
/// both ways is dropped (and `EASE` ft more each way) and the road goes on from where they
/// part; never at an end. (The profile is fitted again afterwards.)
fn cut_spikes(r: &mut RoadPath) {
    const TOL: f64 = 60.0;
    const EASE: f64 = 200.0;
    let mut k = 1;
    while k + 1 < r.pts.len() {
        let (u, v) = (unit2([r.pts[k][0] - r.pts[k - 1][0], r.pts[k][1] - r.pts[k - 1][1]]), unit2([r.pts[k + 1][0] - r.pts[k][0], r.pts[k + 1][1] - r.pts[k][1]]));
        // (A hairpin's two legs part at once: not a spike.)
        if dot2(u, v) > -0.9 || dist(r.pts[k - 1], r.pts[k + 1]) >= TOL {
            k += 1;
            continue;
        }
        let (mut i, mut j) = (k - 1, k + 1);
        loop {
            if i >= 1 && j + 1 < r.pts.len() && seg_dist(r.pts[i - 1], r.pts[j], r.pts[j + 1]) < TOL {
                i -= 1;
            } else if i >= 1 && j + 1 < r.pts.len() && seg_dist(r.pts[j + 1], r.pts[i - 1], r.pts[i]) < TOL {
                j += 1;
            } else {
                break;
            }
        }
        if i == 0 || j + 1 >= r.pts.len() {
            k += 1;
            continue;
        }
        // A little farther each way (the stubs left would turn too tightly to round).
        let (mut back, mut on) = (0.0, 0.0);
        while i > 1 && back < EASE {
            back += dist(r.pts[i - 1], r.pts[i]);
            i -= 1;
        }
        while j + 2 < r.pts.len() && on < EASE {
            on += dist(r.pts[j], r.pts[j + 1]);
            j += 1;
        }
        r.pts.drain(i + 1..j);
        r.z.drain(i + 1..j);
        r.wander.drain(i + 1..j);
        k = i.max(1);
    }
}

#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    crate::core::sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]))
}

