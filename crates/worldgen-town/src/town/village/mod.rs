//! Villages (`Tier::Village`): no wards, patches or walls. Lanes from the roads through the village
//! (and from its green), a lane round the green that they meet in Ts (`ring`), cottages along them
//! with their long sides to the lane (round the green, facing it across the ring), each on its toft
//! and croft (a plot running back from the lane, hedged or fenced: `Plot`); the church in its walled
//! churchyard by the green (a small village's shrine in a small yard with its graveyard), the mill on
//! the bank where there is water, a manor in its own yard in the bigger villages and some others; the
//! fields round it all, off the plots (`Layout::fields`).

use super::catalog::{self, Ward};
use super::geom::*;
use super::{Building, Layout, Plaza, PlazaKind, Site, Structure, chamfer, overlaps_or_crosses, piers_along, point_along, polar_pt, rect, rotate, shore_run, split_field, temple, voronoi};
use crate::battle::{URBAN_CROFT, URBAN_GRAVEYARD, URBAN_YARD};
use crate::core::hash::FastMap;
use crate::core::rng::{Pcg32, hash3, unit};
use crate::t0::biome::Biome;
use crate::t0::settle::{Settlement, Tier};

mod lanes;
mod ring;
mod plots;
mod plan;

use lanes::*;
use ring::*;
pub(super) use plots::*;
pub use plan::*;

/// What a village's plot is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotKind {
    /// A cottage's toft (the house and its yard at the lane) and croft (the strip behind).
    Croft,
    /// The church's, round it, its graveyard behind it.
    Churchyard,
    /// A small village's shrine at the front, its graveyard behind.
    ShrineYard,
    /// The manor's yard round the manor house.
    ManorYard,
    /// A farmstead's yard at a lane's end: its farmhouse and barn (R5 towns, 103).
    Farmyard,
}

/// What a plot is closed by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Hedge,
    Fence,
    /// A low stone wall (a garden wall's, `interior::FORE_WALL_FT` thick).
    Wall,
}

/// A village's plot (world ft): a convex quad, its first edge (`pts[0]`, `pts[1]`) its front on the
/// lane or the green; `pts[3]` behind `pts[0]`. A croft is open along its front; a yard has its gate
/// (`GATE_FT`) in the middle of it.
#[derive(Clone, Debug)]
pub struct Plot {
    pub pts: Vec<P>,
    pub kind: PlotKind,
    pub bound: Bound,
}

/// Plan zoom's site polygons (`payload::pack_sites`, `gen/protocol.ts`; v63 countryside, 20-23).
pub const PLAN_HEADSTONE: u32 = 20;
pub const PLAN_WALL: u32 = 22;
/// A plan polygon in the monuments' pale stone (the green's well).
const PLAN_STONE: u32 = 7;

/// A yard's gate (ft), and how near a lane's edge it is wanted (else it opens onto the green).
const GATE_FT: f64 = 10.0;
const GATE_LANE_FT: f64 = 30.0;
/// A churchyard wall's width at plan zoom (ft).
const WALL_FT: f64 = 2.4;
/// A croft's width (ft) and depth back from the lane.
const CROFT_W: (f64, f64) = (30.0, 60.0);
const CROFT_D: (f64, f64) = (70.0, 150.0);
/// How near a lane or a road's edge a croft may come but at its front (ft).
const CROFT_LANE_FT: f64 = 2.5;
/// How far a croft's sides and back keep off its own cottage (ft).
const HOUSE_BOUND_FT: f64 = 4.0;
/// The hardest turn a lane taken round a mill may make (degrees).
const MILL_TURN_DEG: f64 = 50.0;
/// How near the water a mill's footprint must stand (ft).
const MILL_WATER_FT: f64 = 30.0;
/// How far ground may rise or fall between two croft samples 10 ft apart and still be worked (ft).
const CROFT_STEP_FT: f64 = 5.0;

/// The village's lanes by `CELL`-ft cell, for asking whether a point is on one.
struct LaneGrid {
    segs: Vec<(P, P, f64)>,
    cells: FastMap<(i64, i64), Vec<u32>>,
}

/// A branch lane's width (ft): what the plots keep clear of and what plan zoom draws.
pub const LANE_W: f64 = 10.0;
const CELL: f64 = 40.0;
/// The most margin `LaneGrid::near` is asked for (ft).
const LANE_MARGIN: f64 = 4.0;

impl LaneGrid {
    fn new(lanes: &[(Vec<P>, f64)]) -> LaneGrid {
        let mut segs = Vec::new();
        for (pts, w) in lanes {
            segs.extend(pts.windows(2).map(|s| (s[0], s[1], 0.5 * w)));
        }
        let mut cells: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
        for (i, &(a, b, hw)) in segs.iter().enumerate() {
            let m = hw + LANE_MARGIN;
            for cy in crate::core::floor((a[1].min(b[1]) - m) / CELL) as i64..=crate::core::floor((a[1].max(b[1]) + m) / CELL) as i64 {
                for cx in crate::core::floor((a[0].min(b[0]) - m) / CELL) as i64..=crate::core::floor((a[0].max(b[0]) + m) / CELL) as i64 {
                    cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        LaneGrid { segs, cells }
    }

    /// How far `p` is from the nearest lane's edge (every lane looked at).
    fn dist(&self, p: P) -> f64 {
        self.segs.iter().map(|&(a, b, hw)| seg_dist(p, a, b) - hw).fold(f64::MAX, f64::min)
    }

    /// A lane comes within `margin` ft (at most `LANE_MARGIN`) of convex `poly`, or crosses it.
    fn touches(&self, poly: &[P], margin: f64) -> bool {
        let bb = bbox(poly);
        let (cx0, cy0) = (crate::core::floor(bb[0] / CELL) as i64, crate::core::floor(bb[1] / CELL) as i64);
        let (cx1, cy1) = (crate::core::floor(bb[2] / CELL) as i64, crate::core::floor(bb[3] / CELL) as i64);
        let m = poly.len();
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let Some(ids) = self.cells.get(&(cx, cy)) else { continue };
                for &i in ids {
                    let (a, b, hw) = self.segs[i as usize];
                    let r = hw + margin;
                    if contains(poly, a) || contains(poly, b) || (0..m).any(|k| poly_gap(&[poly[k], poly[(k + 1) % m]], &[a, b]) < r) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Within `margin` ft of a lane's edge.
    fn near(&self, p: P, margin: f64) -> bool {
        let key = (crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64);
        self.cells.get(&key).is_some_and(|ids| {
            ids.iter().any(|&i| {
                let (a, b, hw) = self.segs[i as usize];
                seg_dist(p, a, b) < hw + margin
            })
        })
    }
}

fn bbox(poly: &[P]) -> [f64; 4] {
    poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
}

fn boxes_meet(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// The area two convex polygons share (`a` clipped by each of `b`'s sides).
fn overlap_area(a: &[P], b: &[P]) -> f64 {
    let mut c = a.to_vec();
    let ccw = area(b) > 0.0;
    for i in 0..b.len() {
        let (p, q) = (b[i], b[(i + 1) % b.len()]);
        let e = sub(q, p);
        let el = len(e).max(1e-9);
        let nrm = if ccw { [e[1] / el, -e[0] / el] } else { [-e[1] / el, e[0] / el] };
        c = clip_plain(&c, nrm, dot(p, nrm));
        if c.len() < 3 {
            return 0.0;
        }
    }
    area(&c).abs()
}

/// The ground as the village takes it up: its lanes and the plots laid out so far.
struct Ground<'s, 'a> {
    site: &'s Site<'a>,
    lanes: LaneGrid,
    plots: Vec<Plot>,
    boxes: Vec<[f64; 4]>,
    /// The green's middle and radius (0: none).
    green: (P, f64),
    /// The green with the ground its ring lane takes in where it runs out to the roads (convex; empty:
    /// the circle alone).
    green_hull: Vec<P>,
}

impl Ground<'_, '_> {
    fn in_plot(&self, p: P) -> bool {
        self.boxes.iter().zip(&self.plots).any(|(b, pl)| p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3] && contains(&pl.pts, p))
    }

    /// How much of convex `poly` lies in the plots (sq ft).
    fn plot_overlap(&self, poly: &[P]) -> f64 {
        let bb = bbox(poly);
        self.boxes.iter().zip(&self.plots).filter(|(b, _)| boxes_meet(**b, bb)).map(|(_, pl)| overlap_area(poly, &pl.pts)).sum()
    }

    fn plot_hit(&self, poly: &[P]) -> bool {
        let bb = bbox(poly);
        self.boxes.iter().zip(&self.plots).any(|(b, pl)| boxes_meet(*b, bb) && overlaps_or_crosses(&pl.pts, poly))
    }

    /// Dry ground a plot can take: off the water, the river's banks and a shore's beach.
    fn dry(&self, p: P) -> bool {
        !self.site.wet(p) && !self.site.near_river(p, 12.0) && !self.site.below_shore(p)
    }

    /// Open ground a plot can take: dry, off the lanes and the roads, in no plot.
    fn open(&self, p: P) -> bool {
        self.dry(p) && !self.lanes.near(p, 1.0) && !self.site.near_network_road(p, 3.0) && !self.in_plot(p) && dist(p, self.green.0) > self.green.1 + 1.0
    }

    /// Convex `poly` clear of the green by `margin` ft.
    fn off_green(&self, poly: &[P], margin: f64) -> bool {
        let (c, r) = self.green;
        let m = poly.len();
        let h = &self.green_hull;
        let by_hull = || !h.is_empty() && poly.iter().any(|&p| dist(p, c) < r + 80.0) && (overlaps_or_crosses(h, poly) || poly_gap(h, poly) <= margin);
        r <= 0.0 || (!contains(poly, c) && (0..m).all(|k| seg_dist(c, poly[k], poly[(k + 1) % m]) > r + margin) && !by_hull())
    }

    /// Lane piece `a`-`b` off the green's grass: its centre line on or outside the ring's (a lane
    /// may run on from the ring or up to it, its edge at the rim), the hull between the ring and
    /// the roads (`off_green`'s) open to it.
    fn lane_off_green(&self, a: P, b: P) -> bool {
        let (c, r) = self.green;
        r <= 0.0 || seg_dist(c, a, b) >= r + 0.5 * LANE_W - 1.0
    }

    /// Clear of every lane and road by `margin` ft (their edges): its outline every 4 ft off the
    /// network roads as drawn, and off the lanes.
    fn off_lanes(&self, poly: &[P], margin: f64) -> bool {
        !self.lanes.touches(poly, margin) && self.off_roads(poly, margin)
    }

    /// `off_lanes` for the network roads only.
    fn off_roads(&self, poly: &[P], margin: f64) -> bool {
        let m = poly.len();
        (0..m).all(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            let n = (dist(a, b) / 4.0).ceil().max(1.0) as usize;
            (0..n).all(|j| !self.site.near_network_road(lerp(a, b, j as f64 / n as f64), margin))
        })
    }

    fn add(&mut self, plot: Plot) {
        self.boxes.push(bbox(&plot.pts));
        self.plots.push(plot);
    }
}

/// Whether `poly` meets a building of `l` (but the one at `skip`).
fn building_hit(l: &Layout, poly: &[P], skip: Option<usize>) -> bool {
    let bb = bbox(poly);
    l.buildings.iter().enumerate().any(|(i, b)| Some(i) != skip && boxes_meet(bbox(&b.poly), bb) && overlaps_or_crosses(&b.poly, poly))
}

/// A plot's quad: its front `w` wide, its middle at `m`, running back `d` along `n` (unit).
fn quad(m: P, n: P, w: f64, d: f64) -> Vec<P> {
    let t = [n[1], -n[0]];
    let (f0, f1) = (sub(m, mul(t, 0.5 * w)), add(m, mul(t, 0.5 * w)));
    vec![f0, f1, add(f1, mul(n, d)), add(f0, mul(n, d))]
}

/// A yard (`quad`) the village can give: open ground (every 12 ft or so across it, and its corners),
/// clear of the buildings and the plots.
fn yard_ok(g: &Ground, l: &Layout, yard: &[P]) -> bool {
    let (f0, t, n) = (yard[0], sub(yard[1], yard[0]), sub(yard[3], yard[0]));
    let (nw, nd) = ((len(t) / 12.0).ceil().max(1.0) as usize, (len(n) / 12.0).ceil().max(1.0) as usize);
    for j in 0..=nd {
        for i in 0..=nw {
            let p = add(f0, add(mul(t, i as f64 / nw as f64), mul(n, j as f64 / nd as f64)));
            if !g.open(p) {
                return false;
            }
        }
    }
    !building_hit(l, yard, None) && !g.plot_hit(yard) && g.off_lanes(yard, 1.0) && g.off_green(yard, 1.0)
}

fn new_building(site: &Site, poly: Vec<P>, ward: Ward, func: Option<usize>, structure: Structure) -> Building {
    let pad = site.pad(&poly);
    Building { poly, ward, func: func.map(|f| f as u16), residential: 1, name: None, floors: 1, pad_ft: pad, structure, roof: None, tint: None, material: None, shape: None, shade: None, id: 0, parts: Vec::new(), keep: None, deck: None }
}

/// The share of a village's crofts hedged (the rest fenced), by the land round it.
fn hedged(b: Biome) -> f64 {
    match b {
        Biome::TemperateForest | Biome::TemperateRainforest | Biome::Grassland => 0.75,
        Biome::Taiga | Biome::Swamp | Biome::Jungle => 0.4,
        Biome::Steppe | Biome::Savanna => 0.25,
        Biome::HotDesert | Biome::ColdDesert | Biome::Alpine | Biome::Tundra | Biome::Ice | Biome::SaltFlat => 0.05,
        _ => 0.35,
    }
}

/// The ways round the green a yard by it is tried, from its first (in steps of 30°).
const ROUND: [f64; 12] = [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0, 4.0, -4.0, 5.0, -5.0, 6.0];

/// A village laid out (local coords): see the module's notes.
pub(super) fn village(site: &Site, s: &Settlement, r: f64, rng: &mut Pcg32, l: &mut Layout) {
    let t0 = site.t0;
    let target = ((s.population as f64 / 5.0).round() as usize).clamp(6, 110);
    let spread = rng.next_f64();
    // The green (wide enough for the lane round it and a row of cottages facing it across it).
    let green_r0 = rng.range(35.0, 55.0);
    let green_r = GREEN_FT.0 + (GREEN_FT.1 - GREEN_FT.0) * (green_r0 - 35.0) / 20.0;
    let reach = r * 1.3;
    // Main lanes: the roads through the village.
    let mut lanes: Vec<(Vec<P>, f64)> = Vec::new();
    for (ri, k) in t0.roads.segments_near([site.center[0] - reach, site.center[1] - reach, site.center[0] + reach, site.center[1] + reach], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 40.0).ceil().clamp(1.0, 400.0) as usize;
        let pts: Vec<P> = (0..=n).map(|j| sub(rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p, site.center)).filter(|p| len(*p) < reach).collect();
        if pts.len() >= 2 {
            lanes.push((pts, rc.class.width_ft()));
        }
    }
    // (One added by hand, `t0::made`: its link runs in through its centre, as a road through one would.)
    if let Some(m) = t0.made.iter().find(|m| m.s.seed == s.seed && m.s.x == s.x && m.s.y == s.y) {
        let pts: Vec<P> = m.track.iter().map(|p| sub(*p, site.center)).take_while(|p| len(*p) < reach).collect();
        if pts.len() >= 2 {
            lanes.push((pts, m.class.width_ft()));
        }
    }
    let n_roads = lanes.len();
    // Waterfront (after ProcGenArcana's wharf): the stretch of shore facing the village
    // becomes a wharf lane with houses on its landward side and piers into the water; a
    // lane runs from the green down to it (or to the first way it meets: a T there).
    let wharf = shore_run(site, reach);
    if let Some(wh) = &wharf {
        let (mid, _) = point_along(wh, 0.5 * wh.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>());
        let to = sub(mid, [0.0, 0.0]);
        let tl = len(to);
        let mut link = None;
        if tl > 60.0 {
            // (From the point of the roads nearest the middle, a T, else from the middle.)
            let from = (0..n_roads).flat_map(|j| lanes[j].0.windows(2).map(move |s| (j, nearest_on([0.0, 0.0], s[0], s[1])))).min_by(|a, b| len(a.1).total_cmp(&len(b.1)));
            let (parent, p0) = match from {
                Some((j, p)) if len(p) < 0.5 * tl && len(sub(mid, p)) > 60.0 => (j, p),
                _ => (usize::MAX, mul(to, 20.0 / tl)),
            };
            let d = mul(sub(mid, p0), 1.0 / len(sub(mid, p0)));
            let mut ways = lanes.clone();
            ways.push((wh.clone(), 12.0));
            let (pts, end) = grow(site, p0, d, tl + 10.0, &ways, parent, rng);
            if pts.len() >= 2 {
                link = Some((pts, end, parent));
            }
        }
        lanes.push((wh.clone(), 12.0));
        // (Where a road crosses it, a point of its own: a crossroads.)
        let wi = lanes.len() - 1;
        let xs: Vec<P> = (0..n_roads).flat_map(|j| lanes[j].0.windows(2).flat_map(|s| wh.windows(2).filter_map(move |w| seg_cross(w[0], w[1], s[0], s[1]).map(|(t, _)| lerp(w[0], w[1], t))))).collect();
        for x in xs {
            node_within(&mut lanes, wi, x, 0.05);
        }
        if let Some((mut pts, end, parent)) = link {
            if parent < n_roads {
                pts[0] = node(&mut lanes, parent, pts[0]);
            }
            if let Some((j, x)) = end {
                let n = pts.len();
                pts[n - 1] = node(&mut lanes, j, x);
            }
            lanes.push((pts, LANE_W));
        }
        piers_along(site, wh, rng, l);
    }
    // Branch lanes (after watabou's road trackers): from the roads (or the green), lanes
    // leave every `min_block` ft, step 25 ft at a time turning toward level ground, and stop
    // at water, the village edge, or where they meet another lane.
    let min_block = (4.0 + 8.0 * rng.next_f64()) * 28.0;
    let mut starts: Vec<Start> = Vec::new();
    if lanes.is_empty() {
        let a0 = rng.range(0.0, std::f64::consts::TAU);
        for k in 0..3 {
            let a = a0 + std::f64::consts::TAU * k as f64 / 3.0 + rng.range(-0.4, 0.4);
            let d = [libm::cos(a), libm::sin(a)];
            starts.push((mul(d, 20.0), d, 0, usize::MAX));
        }
    } else {
        for (li, (pts, _)) in lanes.iter().enumerate() {
            branch_points(pts, li, min_block, rng, &mut starts, 1);
        }
    }
    // (A lane cut short by the way it joins counts for its length: up to 150 ft, a part of one.)
    let mut made = 0.0;
    let max_lanes = 3 + (s.population / 80) as usize;
    let mut k = 0;
    while k < starts.len() && made < max_lanes as f64 {
        let (p0, d0, depth, parent) = starts[k];
        k += 1;
        let (mut lane, end) = grow(site, p0, d0, r * 1.05, &lanes, parent, rng);
        // (Long enough for a house or two; a shorter one that joins a way is a loop's; none beside the
        // lane it joins all the way.)
        if (lane.len() >= 4 || (end.is_some() && lane.len() >= 3 && length(&lane) > 40.0)) && !sliver(&lane, end, &lanes) {
            // (Its ends points of the ways it leaves and joins: shared nodes.)
            if parent < lanes.len() {
                lane[0] = node(&mut lanes, parent, p0);
            }
            if let Some((j, x)) = end {
                let n = lane.len();
                lane[n - 1] = node(&mut lanes, j, x);
            }
            if depth < 2 {
                let mut more = Vec::new();
                branch_points(&lane, lanes.len(), min_block * 1.5, rng, &mut more, depth + 1);
                starts.extend(more);
            }
            lanes.push((lane, LANE_W));
            made += (length(&lanes[lanes.len() - 1].0) / 150.0).min(1.0);
        }
    }
    // (A lane that stopped short of a way laid out after it runs on to join it.)
    for li in n_roads..lanes.len() {
        let own = lanes[li].0.clone();
        if on_way(&lanes, li, own[own.len() - 1]) {
            continue;
        }
        if let Some((j, x)) = join_to(site, &own, li, &lanes, JOIN_REACH_FT, &|_, _| false, &|_, _, _| true) {
            let x = node(&mut lanes, j, x);
            lanes[li].0.push(x);
        }
    }
    // (The green round the village's middle, its well at its heart off the lanes and roads through it:
    // the first spot of rings round the middle clear of them.)
    let lane_d = |p: P| lanes.iter().flat_map(|(q, w)| q.windows(2).map(move |s| seg_dist(p, s[0], s[1]) - 0.5 * w)).fold(f64::MAX, f64::min);
    let spots: Vec<P> = std::iter::once([0.0, 0.0]).chain([10.0, 18.0, 0.45 * green_r0, 0.65 * green_r0].into_iter().flat_map(|rad| (0..8).map(move |k| polar_pt(rad, std::f64::consts::TAU * k as f64 / 8.0)))).filter(|&p| !site.wet(p)).collect();
    // (A road ending at the middle, a spur to the village, ends on the ring: the green beyond its end.)
    let spur = spur_end(&lanes, n_roads);
    let spur_well = spur.as_ref().and_then(|(_, t)| spur_well(site, *t, green_r, &|p| !site.near_network_road(p, 6.0)));
    let spur = if spur_well.is_some() { spur.map(|(pc, _)| pc).unwrap_or_default() } else { Vec::new() };
    let well = spur_well.or_else(|| spots.iter().copied().find(|&p| lane_d(p) > 7.0 && !site.near_network_road(p, 6.0))).or_else(|| spots.iter().copied().max_by(|a, b| lane_d(*a).total_cmp(&lane_d(*b)))).unwrap_or([0.0, 0.0]);
    let green = if site.wet(well) { 0.0 } else { green_r };
    // (The ring lane's centre line round it, and the points its runs reach out to on the roads.)
    let rr = green + 0.5 * LANE_W;
    let mut reach_out = Vec::new();
    if green > 0.0 {
        l.plazas.push(Plaza { pts: circle(well, green_r, 12), kind: PlazaKind::Green, spot: super::places::Spot::None });
        // (The lanes off the roads stop short of it, none running along the ring nor onto the green,
        // and run on to the lane round it in Ts.)
        let off: Vec<(Vec<P>, f64)> = lanes.drain(n_roads..).flat_map(|(pts, w)| outside(&pts, well, rr + RING_CLEAR_FT).into_iter().map(move |q| (q, w))).collect();
        lanes.extend(off);
        let first = lanes.len();
        reach_out = ring(site, well, rr, &mut lanes, n_roads, &spur);
        let last = lanes.len();
        to_ring(site, well, rr, &mut lanes, n_roads, first..last);
        lanes.retain(|(pts, _)| pts.len() >= 2);
    }
    if green > 0.0 {
        green_props(site, s.seed, well, green, &lanes[..n_roads], l);
    }
    // (The village's calls, each one draw: a church in the bigger villages, some smaller ones too, a
    // cross plan in the biggest; a manor in the big ones, a few others; how many crofts are hedged.)
    let church = s.population >= 200 || rng.next_f64() < 0.2;
    let big_church = s.population >= 380 && rng.next_f64() < 0.5;
    let manor = (s.population >= 300 && rng.next_f64() < 0.75) || rng.next_f64() < 0.12;
    let hedge_share = hedged(crate::battle::land_biome(t0, site.center));
    let green_hull = if reach_out.is_empty() { Vec::new() } else { hull(circle(well, green, 24).into_iter().chain(reach_out).collect()) };
    let mut g = Ground { site, lanes: LaneGrid::new(&lanes), plots: Vec::new(), boxes: Vec::new(), green: (well, green), green_hull };
    // The church in its churchyard by the green, else a shrine in its yard.
    let faith = church && churchyard(&mut g, l, green_r, big_church, rng);
    if !faith {
        let bound = if rng.next_f64() < hedge_share { Bound::Hedge } else { Bound::Fence };
        shrine_yard(&mut g, l, green_r, bound, rng);
    }
    mill(&mut g, l, r, wharf.as_deref(), &mut lanes, n_roads, rng);
    if manor {
        manor_yard(&mut g, l, r, &lanes[..], rng);
    }
    // Cottages along both sides of every lane (watabou's buildAlong): a house of width 22–36 ft, a
    // gap, the next; set back from the lane, its long side along it; denser near the green. Each on
    // its toft and croft, running back from the lane (`croft`). The roads through the village are
    // walked out from the point of each nearest the green; every lane and side takes its next house in
    // turn, so the houses there are go round all of them.
    // (With no green, the middle kept open; with one, the green itself kept clear, `off_green`.)
    let clear = |poly: &[P], l: &Layout| {
        let c = centroid(poly);
        (green > 0.0 || len(c) > 60.0) && l.buildings.iter().all(|b| !overlaps_or_crosses(&b.poly, poly) && dist(centroid(&b.poly), c) > 14.0)
    };
    // (A house off every lane and road but by a foot, and off the green.)
    let off = |g: &Ground, poly: &[P]| g.off_lanes(poly, 1.0) && g.off_green(poly, 2.0);
    let mut fronts: Vec<(Vec<P>, f64)> = Vec::new();
    // (The ring's fronts: cottages on its outer side only, facing the green; its inner side is the green.)
    let mut inner: Vec<bool> = Vec::new();
    // (And how far along a lane from its T its first house may stand: off the way it leaves.)
    let mut leads: Vec<f64> = Vec::new();
    for (li, (pts, w)) in lanes.iter().enumerate() {
        let k = if li < n_roads { (0..pts.len()).min_by(|&a, &b| len(pts[a]).total_cmp(&len(pts[b])).then(a.cmp(&b))).unwrap_or(0) } else { 0 };
        if k >= 1 {
            fronts.push((pts[..=k].iter().rev().copied().collect(), *w));
            leads.push(0.0);
            inner.push(false);
        }
        if pts.len() - k >= 2 {
            fronts.push((pts[k..].to_vec(), *w));
            inner.push(li >= n_roads && green > 0.0 && on_ring(pts, well, rr));
            let from = lanes.iter().enumerate().filter(|&(j, (q, _))| j != li && li >= n_roads && q.windows(2).any(|s| seg_dist(pts[0], s[0], s[1]) <= 1.0)).map(|(_, (_, wj))| 0.5 * wj + 3.0).fold(0.0, f64::max);
            leads.push(from);
        }
    }
    struct Walk {
        lane: usize,
        side: f64,
        along: f64,
        total: f64,
        // (Where the last croft on this side ends along the lane, and the last house.)
        last: f64,
        house_end: f64,
        // (On the ring, the middle of the piece last tried: the next try is on past it, `ring_spot`.)
        tried: f64,
    }
    let mut walks: Vec<Walk> = Vec::new();
    for (lane, (pts, _)) in fronts.iter().enumerate() {
        let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        for side in [-1.0, 1.0] {
            // (A ring runs anticlockwise round the green: its left, +1, is the green's side.)
            if inner[lane] && side > 0.0 {
                continue;
            }
            walks.push(Walk { lane, side, along: rng.range(0.0, 20.0) + leads[lane], total, last: f64::MIN, house_end: f64::MIN, tried: f64::MIN });
        }
    }
    // (The ring's walks first, to their ends: the green lined with cottages before the roads take
    // the ground by it; then every walk in turn.)
    for ring_first in [true, false] {
        'fill: loop {
            let mut live = false;
            for wk in walks.iter_mut() {
                if wk.along >= wk.total || (ring_first && !inner[wk.lane]) {
                    continue;
                }
                live = true;
                if l.buildings.len() >= target {
                    break 'fill;
                }
                let (pts, road_w) = (&fronts[wk.lane].0, fronts[wk.lane].1);
                let side = wk.side;
                let (own_deg, wk_lane) = (if inner[wk.lane] { 90.0 } else { SQUARE_DEG }, wk.lane);
                wk.along = wk.along.max(wk.last);
                let w = rng.range(22.0, 36.0);
                // (Round the green, the cottages a little narrower, close along it: `RING_HOUSE_W`.)
                let w = if inner[wk.lane] { RING_HOUSE_W.0 + (w - 22.0) * (RING_HOUSE_W.1 - RING_HOUSE_W.0) / 14.0 } else { w };
                // (The house square to the lane's chord across its width; on the ring, square to the piece
                // its middle is on, at that piece's middle: its front to the green, `ring_spot`.)
                let (q0, q1) = (point_along(pts, wk.along).0, point_along(pts, wk.along + w).0);
                let (mut p, _) = point_along(pts, wk.along + 0.5 * w);
                let chord = sub(q1, q0);
                let mut u = mul(chord, 1.0 / len(chord).max(1e-9));
                if inner[wk.lane] {
                    if let Some((m, pu, a)) = ring_spot(pts, wk.along + 0.5 * w, (wk.house_end + 0.5 * w + 2.0).max(wk.tried + 1.0)) {
                        (p, u, wk.along, wk.tried) = (m, pu, a - 0.5 * w, a);
                    } else {
                        wk.along = wk.total;
                        continue;
                    }
                }
                let along = wk.along;
                let n = [-u[1], u[0]];
                // (Every plot taken near the green, thinning toward the edge: a village's houses close
                // along its lanes, not strewn.)
                let density = (1.2 - 0.9 * (len(p) / reach).min(1.0)).min(1.0);
                let d = rng.range(16.0, 24.0);
                let mut housed = false;
                if rng.next_f64() < density || inner[wk.lane] {
                    let back = 0.5 * road_w + (5.0 * (1.0 + spread) * rng.range(0.6, 1.4)).min(9.0) + 0.5 * d;
                    let fits = |house: &[P], u: P| {
                        // (Its front square to the lane: its two corners nearest the lane about as far from it.)
                        let mut ds: Vec<f64> = house.iter().map(|c| pts.windows(2).map(|s| seg_dist(*c, s[0], s[1])).fold(f64::MAX, f64::min)).collect();
                        ds.sort_by(f64::total_cmp);
                        ds[1] - ds[0] < 3.0 && square_to(&lanes, house, u, Some(pts), own_deg) && (!inner[wk_lane] || square_to_nearest(pts, house, u)) && clear(house, l) && site.buildable(house) && !g.plot_hit(house) && off(&g, house)
                    };
                    // (Off a yard's bounds by `CLEAR_FT`, `off_yards`: else a step or two on along the lane.)
                    let mut placed = None;
                    let house = rect(add(p, mul(n, side * back)), u, w, d);
                    if fits(&house, u) {
                        if off_yards(&g, &house) {
                            placed = Some((house, 0.0));
                        } else if !inner[wk.lane] {
                            for shift in [5.0, -5.0, 10.0, -10.0, 15.0, 20.0] {
                                if along + shift + w > wk.total || along + shift < wk.house_end.max(0.0) + 1.0 {
                                    continue;
                                }
                                let (q0, q1) = (point_along(pts, along + shift).0, point_along(pts, along + shift + w).0);
                                let u = mul(sub(q1, q0), 1.0 / len(sub(q1, q0)).max(1e-9));
                                let house = rect(add(point_along(pts, along + shift + 0.5 * w).0, mul([-u[1], u[0]], side * back)), u, w, d);
                                if fits(&house, u) && off_yards(&g, &house) {
                                    placed = Some((house, shift));
                                    break;
                                }
                            }
                        }
                    }
                    housed = placed.is_some();
                    if let Some((house, shift)) = placed {
                        let along = along + shift;
                        wk.along += shift;
                        l.buildings.push(new_building(site, house, Ward::Rural, None, Structure::Roofed));
                        let own = l.buildings.len() - 1;
                        let (e0, e1) = (rng.range(3.0, 10.0), rng.range(3.0, 10.0));
                        let depth = rng.range(CROFT_D.0, CROFT_D.1);
                        let bound = if rng.next_f64() < hedge_share { Bound::Hedge } else { Bound::Fence };
                        let mut a0 = (along - e0).max(wk.house_end + 1.0);
                        // (Up to the last croft beside it if that is near: no sliver between them.)
                        let pinned = wk.last > f64::MIN && a0 < wk.last + 12.0;
                        if pinned {
                            a0 = wk.last;
                        }
                        let prev_end = wk.house_end;
                        wk.house_end = along + w;
                        let a1 = (along + w + e1).max(a0 + CROFT_W.0).min(a0 + CROFT_W.1).max(along + w + 1.0);
                        // (Wider where the lane bends and the house's corner would reach its side.)
                        for ext in [0.0, 5.0, 10.0] {
                            let b0 = if pinned { a0 } else { (a0 - ext).max(prev_end + 1.0) };
                            if let Some(pl) = croft(&g, l, pts, side, b0, a1 + ext, 0.5 * road_w + 1.5, back + 0.5 * d, depth, own) {
                                g.add(Plot { pts: pl, kind: PlotKind::Croft, bound });
                                wk.last = a1 + ext;
                                break;
                            }
                        }
                    }
                }
                let avg3 = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0;
                // (Along the ring, a spot that will not take one is tried again a step on.)
                wk.along += if !inner[wk.lane] { w + 8.0 + 16.0 * avg3 } else if housed { w + 4.0 + 8.0 * avg3 } else { RING_STEP_FT };
            }
            if !live {
                break;
            }
        }
    }
    // Last resort for a village with no lanes at all: a ring around the green.
    let mut tries = 0;
    while lanes.is_empty() && l.buildings.len() < target / 2 && tries < target * 30 {
        tries += 1;
        let rad = rng.range(70.0, r);
        let a = rng.range(0.0, std::f64::consts::TAU);
        let c = [rad * libm::cos(a), rad * libm::sin(a)];
        let house = rect(c, [-libm::sin(a), libm::cos(a)], rng.range(22.0, 34.0), rng.range(16.0, 22.0));
        if clear(&house, l) && site.buildable(&house) && !g.plot_hit(&house) && off(&g, &house) {
            l.buildings.push(new_building(site, house, Ward::Rural, None, Structure::Roofed));
        }
    }
    // Every lane's ends joined or on purpose (`settle_ends`); the lanes as they are now the layout's.
    clear_crofts(&mut g, l);
    settle_ends(&mut g, l, &mut lanes, n_roads, hedge_share, rng);
    for (pts, _) in lanes.iter().skip(n_roads) {
        l.streets.push(pts.clone());
    }
    // Fields: Voronoi cells seeded along the lanes and scattered round the village (rings of their
    // own counts and spins, seeds jittered, cells of many sizes), split into plots and claimed
    // outward until there is enough farmland for its people; none on the village's plots. Outward
    // by each plot's reach (`fields::reach`): the belt's edge wanders, runs on along the roads and
    // the river, and its last fields are thinned (`fields::belt_order`). (Their own random stream:
    // what the village draws after them never shifts with the fields.)
    let mut frng = Pcg32::new(hash3(s.seed, 0xf1e1d, 0, 7), 61);
    let spacing = 260.0 - 120.0 * spread;
    let mut seeds: Vec<P> = Vec::new();
    for (pts, _) in lanes.iter().filter(|(pts, _)| green <= 0.0 || !on_ring(pts, well, rr)) {
        let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        let mut a = 0.5 * spacing;
        while a < total {
            let (p, u) = point_along(pts, a);
            let n = [-u[1], u[0]];
            for side in [-1.0, 1.0] {
                seeds.push(add(p, mul(n, side * frng.range(0.45, 0.8) * spacing)));
            }
            a += spacing * frng.range(0.75, 1.3);
        }
    }
    let ring_n = (std::f64::consts::TAU * 1.7 * r) / spacing;
    for rad in [1.3, 1.7, 2.05] {
        let n = ((ring_n * rad / 1.7 * frng.range(0.6, 1.2)) as usize).max(4);
        let spin = frng.range(0.0, std::f64::consts::TAU);
        for k in 0..n {
            let a = spin + std::f64::consts::TAU * (k as f64 + frng.range(-0.45, 0.45)) / n as f64;
            seeds.push(polar_pt(rad * r * frng.range(0.85, 1.15), a));
        }
    }
    // (Within 2.18 r: the ground the village's pad covers, `town::reach`.)
    let rim = 2.18 * r;
    seeds.retain(|p| len(*p) > 0.5 * r && len(*p) < rim);
    let cells = voronoi(&seeds, &circle([0.0, 0.0], rim, 64));
    let roads = l.roads.clone();
    let reach = |c: P| super::fields::reach(site, s.seed, c, &roads, 1.75 * r);
    let far = |c: P| len(c) / (1.75 * r * reach(c));
    let mut order: Vec<(f64, usize)> = (0..cells.len()).filter(|&i| cells[i].pts.len() >= 3).map(|i| (far(seeds[i]), i)).collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let want = l.buildings.len() as f64 * 22_000.0 * (0.7 + 0.6 * spread);
    // (A third more than wanted, for the edge to be chosen from.)
    let mut have = 0.0;
    let mut plots: Vec<(Vec<P>, u8)> = Vec::new();
    for (_, i) in order {
        if have >= 1.33 * want {
            break;
        }
        let cell = inset(&cells[i].pts, &vec![5.0; cells[i].pts.len()]);
        if cell.len() < 3 || site.wet(centroid(&cell)) {
            continue;
        }
        let mut cut = Vec::new();
        split_field(&cell, 20_000.0 * (1.0 + spread) * frng.range(0.6, 1.6), &mut frng, &mut cut, 0);
        for plot in cut {
            let f = clean(&chamfer(&plot, 10.0));
            // (Short of the rim where the reach is short, and now and then by it: no round edge.)
            let c = centroid(&f);
            let w = add(c, site.center);
            let by_rim = len(c) > 0.86 * rim && unit(hash3(s.seed, crate::core::round(w[0]) as i64, crate::core::round(w[1]) as i64, 0x41e)) < 0.25;
            if len(c) > rim * (0.76 + 0.24 * reach(c)).min(1.0) - 40.0 || by_rim {
                continue;
            }
            if f.len() >= 3 && area(&f).abs() > 1_500.0 && !site.wet(c) && site.dry_outline(&f, 8.0) && !g.plot_hit(&f) && !l.buildings.iter().any(|b| super::overlaps(&b.poly, &f)) {
                // (Cut back off sand, `fields::off_sand`: plots farther out make up for it.)
                let Some(f) = super::fields::off_sand(site, &f) else { continue };
                have += area(&f).abs();
                plots.push((f, 1));
            }
        }
    }
    l.fields.extend(super::fields::belt_order(site, s.seed, &plots, want, &far));
    l.crofts = g.plots;
}

// ---------------------------------------------------------------------------------------
// R5 towns (103): lanes that join their ways.

