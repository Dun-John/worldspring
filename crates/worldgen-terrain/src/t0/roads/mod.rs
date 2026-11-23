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

mod join;
mod route;
mod shape;
mod profile;
mod prune;
mod junction;
mod tidy;

pub use join::*;
pub use route::*;
pub use shape::*;
pub use profile::*;
use prune::*;
use junction::*;
pub use tidy::*;

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
    /// The ferries over the sea to near islands (`ferries`; their land legs are among `roads`).
    pub ferries: Vec<super::ferries::SeaFerry>,
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
    /// Per cell, its land mass (`islands::label`), built on first use: no link crosses the sea.
    pub masses: std::cell::OnceCell<super::islands::Islands>,
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
    /// The land masses (per cell: `of`, `super::islands::NONE` at sea).
    pub fn masses(&self) -> &super::islands::Islands {
        self.masses.get_or_init(|| super::islands::label(self.w, self.h, self.cell_ft, self.land, &self.hydro.lake_of, self.biome))
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
    let (_, mut paths) = route(inp, settlements, &edges);
    // Ferries to near islands: their land legs routed as tracks (`ferries`), each ending at its landing.
    let ferries = super::ferries::plan(inp, settlements, &paths);
    let landing: std::collections::BTreeMap<u32, [f64; 2]> = ferries.iter().flat_map(|f| f.legs.iter().map(|(cells, end)| (*cells.last().expect("a leg"), *end))).collect();
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

    paths.extend(ferries.iter().flat_map(|f| f.legs.iter().map(|(cells, _)| (RoadClass::Track, cells.clone(), None))));

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
        let pairs = if cities.len() >= 2 { link_edges(&by_tier, &[RoadClass::KingsRoad], w, cell, None) } else { Vec::new() };
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
            } else if let Some(p) = landing.get(&c) {
                cells[end] = [p[0] / cell, p[1] / cell];
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
                let r = crate::t0::radii::road_trim_radius(s.tier, s.population) / cell;
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
        // A realm village's spur wanders a little more: on flat ground it would run ruler-straight.
        let spur_end = |c: u32| settlements.iter().any(|v| v.isolated && v.planned == Tier::Village && v.cell as u32 == c);
        let realm_spur = *class == RoadClass::Track && (spur_end(seg[0]) || spur_end(seg[seg.len() - 1]));
        // So does any long dead end into a settlement (a spur nothing else joins), of any class: its cells
        // are 1.17 mi, so over flat ground it would otherwise run straight for miles. Roads sway a little less.
        let dead_end = |c: u32| adj[&c].len() == 1 && at_cell.contains_key(&(c as usize));
        let length_mi = pts.windows(2).map(|p| dist(p[0], p[1])).sum::<f64>() * cell / 5280.0;
        let long_spur = stroke.is_none() && length_mi > 5.0 && (dead_end(seg[0]) || dead_end(seg[seg.len() - 1]));
        if realm_spur || long_spur {
            let scale = if realm_spur { 1.0 } else { 1.0 - 0.3 * (*class <= RoadClass::Road) as u8 as f64 };
            let seed = crate::core::rng::hash2(inp.world.stream("t0.road.spur"), seg[0] as i64, seg[seg.len() - 1] as i64);
            meander_spur(&mut pts, &mut wander, seed, scale, &|x, y| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && passable_fn(inp)(y as usize * w + x as usize));
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
    let towns: Vec<([f64; 2], f64)> = settlements.iter().map(|s| ([s.x, s.y], crate::t0::radii::road_trim_radius(s.tier, s.population).max(0.15 * cell))).collect();
    // (Routes laid across each other meet where they cross.)
    join_crossings(&mut roads, &towns, cell);
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
                    waystations.push(Poi { kind: PoiKind::Waystation, x, y, seed: crate::core::rng::hash2(stream, (x / 10.0) as i64, (y / 10.0) as i64), stroke: None, fall: None });
                }
            }
        }
    }
    Network { roads, crossings, waystations, traffic: traffic_out, realm_paths, ferries: ferries.into_iter().map(|f| f.ferry).collect() }
}

/// Settlement pairs to join, per class: king's roads join cities, roads join towns and up,
/// tracks join everything. Each class is a relative-neighbourhood graph (no third place
/// closer to both ends) plus a minimum spanning tree so its members stay connected.
/// `mass`: per settlement, its land mass, and the mainland's; links join only those on one (with
/// `None`, all), and an island's tracks join all its places.
fn link_edges(settlements: &[Settlement], classes: &[RoadClass], w: usize, cell: f64, mass: Option<(&[u32], u32)>) -> Vec<(usize, usize, RoadClass)> {
    let ns = settlements.len();
    let one = |a: usize, b: usize| mass.is_none_or(|(m, _)| m[a] == m[b]);
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
                if dab < max_link && one(a, b) {
                    pairs.push((dab, a, b));
                }
            }
        }
        pairs.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        for &(dab, a, b) in &pairs {
            if members.iter().all(|&c| c == a || c == b || !one(a, c) || d(a, c).max(d(b, c)) >= dab) {
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
        // (A town or city farther than that from any other still has a road, to the nearest on
        // its land.)
        if class != RoadClass::Track {
            for &a in &members {
                if edges.iter().any(|e| e.0 == a || e.1 == a) {
                    continue;
                }
                if let Some(&b) = members.iter().filter(|&&b| b != a && one(a, b)).min_by(|&&x, &&y| d(a, x).total_cmp(&d(a, y)).then(x.cmp(&y))) {
                    add(a, b, &mut edges);
                }
            }
        }
        if class == RoadClass::Track
            && let Some((m, mainland)) = mass
        {
            island_tracks(&members, m, mainland, &d, &mut edges);
        }
    }

    edges
}

/// An island's places the links so far leave in more than one group: joined by tracks, the
/// nearest pair between two groups first, however far (`members`: those on tracks; `mass`: per
/// settlement, its land mass).
fn island_tracks(members: &[usize], mass: &[u32], mainland: u32, d: &dyn Fn(usize, usize) -> f64, edges: &mut Vec<(usize, usize, RoadClass)>) {
    let mut parent: Vec<usize> = (0..mass.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for &(a, b, _) in edges.iter() {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        parent[ra] = rb;
    }
    let on: Vec<usize> = members.iter().copied().filter(|&a| mass[a] != mainland && mass[a] != super::islands::NONE).collect();
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (ia, &a) in on.iter().enumerate() {
        for &b in &on[ia + 1..] {
            if mass[a] == mass[b] {
                pairs.push((d(a, b), a, b));
            }
        }
    }
    pairs.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
    for (_, a, b) in pairs {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
            edges.push((a.min(b), a.max(b), RoadClass::Track));
        }
    }
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
    let planned = if inp.world.file.params.generated_roads {
        let m = inp.masses();
        let mass: Vec<u32> = settlements.iter().map(|s| m.of[s.cell]).collect();
        link_edges(settlements, classes, inp.w, inp.cell_ft, Some((&mass, m.mainland)))
    } else {
        Vec::new()
    };
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

/// A polyline's segments bucketed on a grid (`size` ft) by their boxes grown by `tol`, so a
/// query looks only at those whose box can hold the point (`near`: the same answer as
/// `near_line`) or meet a box (`cands`).
struct NearLine<'a> {
    line: &'a [[f64; 2]],
    tol: f64,
    min: [f64; 2],
    size: f64,
    nx: usize,
    ny: usize,
    start: Vec<u32>,
    segs: Vec<u32>,
}

impl<'a> NearLine<'a> {
    fn new(line: &'a [[f64; 2]], tol: f64, size: f64) -> Self {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in line {
            for d in 0..2 {
                lo[d] = lo[d].min(p[d]);
                hi[d] = hi[d].max(p[d]);
            }
        }
        let min = [lo[0] - tol, lo[1] - tol];
        let n = |d: usize| if line.len() < 2 { 0 } else { (crate::core::floor((hi[d] + tol - min[d]) / size) as usize + 1).min(4096) };
        let (nx, ny) = (n(0), n(1));
        let mut out = NearLine { line, tol, min, size, nx, ny, start: vec![0; nx * ny + 1], segs: Vec::new() };
        // (Counted, then filled: each bucket's segments in order.)
        let span = |out: &NearLine, a: [f64; 2], b: [f64; 2]| {
            let (x0, y0) = (out.bucket(0, a[0].min(b[0]) - tol), out.bucket(1, a[1].min(b[1]) - tol));
            let (x1, y1) = (out.bucket(0, a[0].max(b[0]) + tol), out.bucket(1, a[1].max(b[1]) + tol));
            (x0, x1, y0, y1)
        };
        for w in line.windows(2) {
            let (x0, x1, y0, y1) = span(&out, w[0], w[1]);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    out.start[y * nx + x + 1] += 1;
                }
            }
        }
        for i in 0..nx * ny {
            out.start[i + 1] += out.start[i];
        }
        let mut fill: Vec<u32> = out.start[..nx * ny].to_vec();
        out.segs = vec![0; out.start[nx * ny] as usize];
        for (k, w) in line.windows(2).enumerate() {
            let (x0, x1, y0, y1) = span(&out, w[0], w[1]);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let b = y * nx + x;
                    out.segs[fill[b] as usize] = k as u32;
                    fill[b] += 1;
                }
            }
        }
        out
    }

    fn bucket(&self, d: usize, v: f64) -> usize {
        let n = if d == 0 { self.nx } else { self.ny };
        (crate::core::floor((v - self.min[d]) / self.size).max(0.0) as usize).min(n.saturating_sub(1))
    }

    /// The segments (in order, each once) whose grown boxes share a bucket with the box `lo`–`hi`.
    fn cands(&self, lo: [f64; 2], hi: [f64; 2], out: &mut Vec<u32>) {
        out.clear();
        if self.nx == 0 || self.ny == 0 {
            return;
        }
        for y in self.bucket(1, lo[1])..=self.bucket(1, hi[1]) {
            for x in self.bucket(0, lo[0])..=self.bucket(0, hi[0]) {
                let b = y * self.nx + x;
                out.extend_from_slice(&self.segs[self.start[b] as usize..self.start[b + 1] as usize]);
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    fn near(&self, p: [f64; 2]) -> bool {
        if self.nx == 0 || self.ny == 0 {
            return false;
        }
        let b = self.bucket(1, p[1]) * self.nx + self.bucket(0, p[0]);
        let tol = self.tol;
        self.segs[self.start[b] as usize..self.start[b + 1] as usize].iter().any(|&k| {
            let (a, b) = (self.line[k as usize], self.line[k as usize + 1]);
            p[0] >= a[0].min(b[0]) - tol && p[0] <= a[0].max(b[0]) + tol && p[1] >= a[1].min(b[1]) - tol && p[1] <= a[1].max(b[1]) + tol && seg_dist_or_start(p, a, b) < tol
        })
    }
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

/// Whether a road may enter cell `k` (as `route` found it; nothing before `route` has run), and
/// what a step from `i` to its neighbour `nb` (`d` cells) costs per cell of its length, as
/// `route` costs it but for its wobble (`ferries`' land legs).
pub(super) fn open_cell(inp: &Inputs, k: usize) -> bool {
    inp.fixed.get().is_some_and(|f| f.open[k])
}

pub(super) fn step_cost(inp: &Inputs, i: usize, nb: usize, d: f64) -> f64 {
    inp.fixed.get().map_or(f64::INFINITY, |f| f.costs.ground(inp, i, nb, d) + f.costs.river(inp, i, nb, &[nb, nb]))
}

