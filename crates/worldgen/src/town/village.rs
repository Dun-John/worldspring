//! Villages (`Tier::Village`): no wards, patches or walls. Lanes from the roads through the village
//! (and from its green), cottages along them with their long sides to the lane, each on its toft and
//! croft (a plot running back from the lane, hedged or fenced: `Plot`); the church in its walled
//! churchyard by the green (a small village's shrine in a small yard with its graveyard), the mill on
//! the bank where there is water, a manor in its own yard in the bigger villages and some others; the
//! fields round it all, off the plots (`Layout::fields`).

use super::catalog::{self, Ward};
use super::geom::*;
use super::{Building, Layout, Plaza, PlazaKind, Site, Structure, chamfer, overlaps_or_crosses, piers_along, point_along, polar_pt, rect, rotate, shore_run, split_field, temple, voronoi};
use crate::battlemap::{URBAN_CROFT, URBAN_GRAVEYARD, URBAN_YARD};
use crate::core::hash::FastMap;
use crate::core::rng::{Pcg32, hash3, unit};
use crate::t0::biome::Biome;
use crate::t0::settle::{Settlement, Tier};

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
        r <= 0.0 || (!contains(poly, c) && (0..m).all(|k| seg_dist(c, poly[k], poly[(k + 1) % m]) > r + margin))
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
    Building { poly, ward, func: func.map(|f| f as u16), residential: 1, name: None, floors: 1, pad_ft: pad, structure, roof: None, tint: None, material: None, shape: None, shade: None, id: 0, parts: Vec::new(), keep: None }
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
    // The green.
    let green_r = rng.range(35.0, 55.0);
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
        // (Long enough for a house or two; a shorter one that joins a way is a loop's.)
        if lane.len() >= 4 || (end.is_some() && lane.len() >= 3 && length(&lane) > 40.0) {
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
    let spots: Vec<P> = std::iter::once([0.0, 0.0]).chain([10.0, 18.0, 0.45 * green_r, 0.65 * green_r].into_iter().flat_map(|rad| (0..8).map(move |k| polar_pt(rad, std::f64::consts::TAU * k as f64 / 8.0)))).filter(|&p| !site.wet(p)).collect();
    let well = spots.iter().copied().find(|&p| lane_d(p) > 7.0 && !site.near_network_road(p, 6.0)).or_else(|| spots.iter().copied().max_by(|a, b| lane_d(*a).total_cmp(&lane_d(*b)))).unwrap_or([0.0, 0.0]);
    let green = if site.wet(well) { 0.0 } else { green_r };
    if green > 0.0 {
        l.plazas.push(Plaza { pts: circle(well, green_r, 12), kind: PlazaKind::Green });
        // (The lanes off the roads end at its edge: none runs onto it.)
        let off: Vec<(Vec<P>, f64)> = lanes.drain(n_roads..).flat_map(|(pts, w)| outside(&pts, well, green_r + 0.5 * w).into_iter().map(move |q| (q, w))).collect();
        lanes.extend(off);
    }
    if green > 0.0 {
        green_props(site, s.seed, well, green, &lanes[..n_roads], l);
    }
    // (The village's calls, each one draw: a church in the bigger villages, some smaller ones too, a
    // cross plan in the biggest; a manor in the big ones, a few others; how many crofts are hedged.)
    let church = s.population >= 200 || rng.next_f64() < 0.2;
    let big_church = s.population >= 380 && rng.next_f64() < 0.5;
    let manor = (s.population >= 300 && rng.next_f64() < 0.75) || rng.next_f64() < 0.12;
    let hedge_share = hedged(crate::battlemap::land_biome(t0, site.center));
    let mut g = Ground { site, lanes: LaneGrid::new(&lanes), plots: Vec::new(), boxes: Vec::new(), green: (well, green) };
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
    let clear = |poly: &[P], l: &Layout| {
        let c = centroid(poly);
        len(c) > 60.0 && l.buildings.iter().all(|b| !overlaps_or_crosses(&b.poly, poly) && dist(centroid(&b.poly), c) > 14.0)
    };
    // (A house off every lane and road but by a foot, and off the green.)
    let off = |g: &Ground, poly: &[P]| g.off_lanes(poly, 1.0) && g.off_green(poly, 2.0);
    let mut fronts: Vec<(Vec<P>, f64)> = Vec::new();
    // (And how far along a lane from its T its first house may stand: off the way it leaves.)
    let mut leads: Vec<f64> = Vec::new();
    for (li, (pts, w)) in lanes.iter().enumerate() {
        let k = if li < n_roads { (0..pts.len()).min_by(|&a, &b| len(pts[a]).total_cmp(&len(pts[b])).then(a.cmp(&b))).unwrap_or(0) } else { 0 };
        if k >= 1 {
            fronts.push((pts[..=k].iter().rev().copied().collect(), *w));
            leads.push(0.0);
        }
        if pts.len() - k >= 2 {
            fronts.push((pts[k..].to_vec(), *w));
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
    }
    let mut walks: Vec<Walk> = Vec::new();
    for (lane, (pts, _)) in fronts.iter().enumerate() {
        let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        for side in [-1.0, 1.0] {
            walks.push(Walk { lane, side, along: rng.range(0.0, 20.0) + leads[lane], total, last: f64::MIN, house_end: f64::MIN });
        }
    }
    'fill: loop {
        let mut live = false;
        for wk in walks.iter_mut() {
            if wk.along >= wk.total {
                continue;
            }
            live = true;
            if l.buildings.len() >= target {
                break 'fill;
            }
            let (pts, road_w) = (&fronts[wk.lane].0, fronts[wk.lane].1);
            let side = wk.side;
            wk.along = wk.along.max(wk.last);
            let along = wk.along;
            let w = rng.range(22.0, 36.0);
            // (The house square to the lane's chord across its width.)
            let (q0, q1) = (point_along(pts, along).0, point_along(pts, along + w).0);
            let (p, _) = point_along(pts, along + 0.5 * w);
            let chord = sub(q1, q0);
            let u = mul(chord, 1.0 / len(chord).max(1e-9));
            let n = [-u[1], u[0]];
            // (Every plot taken near the green, thinning toward the edge: a village's houses close
            // along its lanes, not strewn.)
            let density = (1.2 - 0.9 * (len(p) / reach).min(1.0)).min(1.0);
            let d = rng.range(16.0, 24.0);
            if rng.next_f64() < density {
                let back = 0.5 * road_w + (5.0 * (1.0 + spread) * rng.range(0.6, 1.4)).min(9.0) + 0.5 * d;
                let house = rect(add(p, mul(n, side * back)), u, w, d);
                // (Its front square to the lane: its two corners nearest the lane about as far from it.)
                let mut ds: Vec<f64> = house.iter().map(|c| pts.windows(2).map(|s| seg_dist(*c, s[0], s[1])).fold(f64::MAX, f64::min)).collect();
                ds.sort_by(f64::total_cmp);
                if ds[1] - ds[0] < 3.0 && square_to(&lanes, &house, u, Some(pts)) && clear(&house, l) && site.buildable(&house) && !g.plot_hit(&house) && off(&g, &house) {
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
            wk.along += w + 8.0 + 16.0 * avg3;
        }
        if !live {
            break;
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
    for (pts, _) in &lanes {
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

/// A branch lane's step as it grows (ft), and how finely each step is looked along (ft).
const STEP_FT: f64 = 25.0;
const PROBE_FT: f64 = 5.0;
/// How near a way's edge a growing lane comes before it turns to join it (ft); how far a lane's end
/// looks for a way to run on to as it grows (ft), and once the village is laid out.
const JOIN_NEAR_FT: f64 = 15.0;
const JOIN_REACH_FT: f64 = 60.0;
const JOIN_LATE_FT: f64 = 40.0;
/// The hardest turn a lane takes to join a way (degrees), and the shallowest its T meets it at.
const JOIN_TURN_DEG: f64 = 45.0;
const T_DEG: f64 = 35.0;
/// How far a lane may run on past its last house or yard and be cut back to it (ft); farther, over
/// open ground, it ends at a farmstead.
const FARM_RUN_FT: f64 = 150.0;

/// Where a branch lane leaves: a point on its parent's centre line, the way it heads, its depth, and
/// its parent (its place in the lanes; `usize::MAX`: none, it leaves the green).
type Start = (P, P, u32, usize);

/// Branch starts every `every` ft along lane `li` (`pts`), on its centre line, alternating sides at
/// random.
fn branch_points(pts: &[P], li: usize, every: f64, rng: &mut Pcg32, out: &mut Vec<Start>, depth: u32) {
    let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let mut a = every * rng.range(0.5, 1.0);
    while a < total - 0.5 * every {
        let (p, u) = point_along(pts, a);
        let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
        let d = rotate([-u[1] * side, u[0] * side], rng.range(-0.35, 0.35));
        out.push((p, d, depth, li));
        a += every * rng.range(0.8, 1.3);
    }
}

/// Where segments `a`-`b` and `c`-`d` cross: the share of the way along each.
fn seg_cross(a: P, b: P, c: P, d: P) -> Option<(f64, f64)> {
    let (e, f) = (sub(b, a), sub(d, c));
    let den = cross(e, f);
    if den.abs() < 1e-9 {
        return None;
    }
    let (t, u) = (cross(sub(c, a), f) / den, cross(sub(c, a), e) / den);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// How many times a lane (from `from` in `lanes`) crosses another lane or a road but within a foot
/// of either's ends.
fn crossings(lanes: &[(Vec<P>, f64)], from: usize) -> usize {
    let mut n = 0;
    for li in from..lanes.len() {
        let a = &lanes[li].0;
        for (j, (b, _)) in lanes.iter().enumerate() {
            if j == li || (j >= from && j < li) || a.len() < 2 || b.len() < 2 {
                continue;
            }
            let ends = [a[0], a[a.len() - 1], b[0], b[b.len() - 1]];
            for s in a.windows(2) {
                for t in b.windows(2) {
                    if let Some((u, _)) = seg_cross(s[0], s[1], t[0], t[1])
                        && ends.iter().all(|e| dist(*e, lerp(s[0], s[1], u)) > 1.0)
                    {
                        n += 1;
                    }
                }
            }
        }
    }
    n
}

/// The point of segment `a`-`b` nearest `p`.
fn nearest_on(p: P, a: P, b: P) -> P {
    let e = sub(b, a);
    let l2 = dot(e, e);
    if l2 < 1e-12 {
        return a;
    }
    lerp(a, b, (dot(sub(p, a), e) / l2).clamp(0.0, 1.0))
}

fn length(pts: &[P]) -> f64 {
    pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// How far along `pts` its point nearest `p` lies (ft).
fn arc_at(pts: &[P], p: P) -> f64 {
    let (mut best, mut at, mut acc) = (f64::MAX, 0.0, 0.0);
    for w in pts.windows(2) {
        let q = nearest_on(p, w[0], w[1]);
        let d = dist(p, q);
        if d < best {
            (best, at) = (d, acc + dist(w[0], q));
        }
        acc += dist(w[0], w[1]);
    }
    at
}

/// `pts` cut `keep` ft along.
fn cut_at(pts: &[P], keep: f64) -> Vec<P> {
    let mut out = vec![pts[0]];
    let mut acc = 0.0;
    for s in pts.windows(2) {
        let d = dist(s[0], s[1]);
        if acc + d >= keep {
            if keep - acc > 0.05 {
                out.push(lerp(s[0], s[1], (keep - acc) / d.max(1e-9)));
            }
            return out;
        }
        out.push(s[1]);
        acc += d;
    }
    out
}

/// Whether `p` lies on a way but lane `li` (within a foot of its centre line).
fn on_way(lanes: &[(Vec<P>, f64)], li: usize, p: P) -> bool {
    lanes.iter().enumerate().any(|(j, (q, _))| j != li && q.windows(2).any(|s| seg_dist(p, s[0], s[1]) <= 1.0))
}

/// `x` on lane `j`'s centre line made one of its points, or the point of it within 2 ft (returned):
/// the node a lane joining there shares.
fn node(lanes: &mut [(Vec<P>, f64)], j: usize, x: P) -> P {
    node_within(lanes, j, x, 2.0)
}

/// `node`, the point of the lane within `snap` ft of `x` taken for it.
fn node_within(lanes: &mut [(Vec<P>, f64)], j: usize, x: P, snap: f64) -> P {
    let pts = &mut lanes[j].0;
    let Some(k) = (0..pts.len().saturating_sub(1)).min_by(|&a, &b| seg_dist(x, pts[a], pts[a + 1]).total_cmp(&seg_dist(x, pts[b], pts[b + 1]))) else { return x };
    for v in [pts[k], pts[k + 1]] {
        if dist(v, x) < snap {
            return v;
        }
    }
    pts.insert(k + 1, x);
    x
}

/// Where a lane ending at the last of `own` (`me`: its place in `lanes`, if there) can run on to join
/// a way in a T: the nearest point of a way within `reach` ft it turns to by `JOIN_TURN_DEG` at most
/// and meets at `T_DEG` or more, the ground on the way there dry, across no other way nor itself, and
/// `clear` (from the end to short of the way); but where `skip` (the way, the point) rules it out.
fn join_to(site: &Site, own: &[P], me: usize, lanes: &[(Vec<P>, f64)], reach: f64, skip: &dyn Fn(usize, P) -> bool, clear: &dyn Fn(P, P, f64) -> bool) -> Option<(usize, P)> {
    let n = own.len();
    if n < 2 {
        return None;
    }
    let p = own[n - 1];
    let d = sub(p, own[n - 2]);
    let d = mul(d, 1.0 / len(d).max(1e-9));
    let (turn, t_min) = (libm::cos(JOIN_TURN_DEG.to_radians()), libm::sin(T_DEG.to_radians()));
    let mut cands: Vec<(f64, usize, P, f64)> = Vec::new();
    for (j, (q, w)) in lanes.iter().enumerate() {
        if j == me {
            continue;
        }
        for s in q.windows(2) {
            let (a, b) = (s[0], s[1]);
            let el = dist(a, b);
            if el < 1e-6 || seg_dist(p, a, b) > reach {
                continue;
            }
            let tu = mul(sub(b, a), 1.0 / el);
            let ray = add(p, mul(d, reach));
            let m = (el / 6.0).ceil() as usize;
            let xs = std::iter::once(nearest_on(p, a, b)).chain(seg_cross(p, ray, a, b).map(|(t, _)| lerp(p, ray, t))).chain((0..=m).map(|k| lerp(a, b, k as f64 / m as f64)));
            for x in xs {
                let v = sub(x, p);
                let vl = len(v);
                if vl < 0.5 || vl > reach || skip(j, x) {
                    continue;
                }
                let v = mul(v, 1.0 / vl);
                let c = dot(v, d);
                if c < turn || cross(v, tu).abs() < t_min {
                    continue;
                }
                cands.push((vl + 30.0 * (1.0 - c), j, x, 0.5 * w));
            }
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    'c: for (_, j, x, hw) in cands.into_iter().take(40) {
        let vl = dist(p, x);
        let m = (vl / 4.0).ceil() as usize;
        if (1..=m).any(|k| {
            let s = lerp(p, x, k as f64 / m as f64);
            site.wet(s) || site.near_river(s, 6.0)
        }) {
            continue;
        }
        for (i, (q, _)) in lanes.iter().enumerate() {
            if i == me {
                continue;
            }
            for s in q.windows(2) {
                if seg_cross(p, x, s[0], s[1]).is_some_and(|(t, _)| t * vl > 0.5 && t * vl < vl - 1.0) {
                    continue 'c;
                }
            }
        }
        if own[..n - 1].windows(2).any(|s| seg_cross(p, x, s[0], s[1]).is_some_and(|(t, _)| t * vl > 0.5)) || !clear(p, x, hw) {
            continue;
        }
        return Some((j, x));
    }
    None
}

/// Grow a lane from `p0` on its parent's centre line (`parent`: its place in `lanes`) heading `d0`
/// (after watabou's road trackers): 25-ft steps, each turning (by up to 20°) toward the most level
/// ground, looked along every 5 ft. It stops at water, the river's banks or `max_r`; where it crosses
/// a way, or comes within `JOIN_NEAR_FT` of one's edge, it runs on to it (`join_to`) and ends on its
/// centre line in a T, as its far end does where a way is near. Its points, and the way it joins at
/// its end and where.
fn grow(site: &Site, p0: P, d0: P, max_r: f64, lanes: &[(Vec<P>, f64)], parent: usize, rng: &mut Pcg32) -> (Vec<P>, Option<(usize, P)>) {
    let mut pts = vec![p0];
    let (mut p, mut d) = (p0, d0);
    let wander = rng.range(-0.08, 0.08);
    // (Its parent by its start is no way to join, nor what meets its parent there (the next piece of a
    // road, a lane leaving it); nor, leaving the green, what crosses its middle.)
    let skip = |j: usize, x: P| (j == parent && dist(x, p0) < 60.0) || dist(x, p0) < 45.0;
    let t_min = libm::sin(T_DEG.to_radians());
    let me = usize::MAX;
    let no_band = |_: P, _: P, _: f64| true;
    for _ in 0..(max_r / STEP_FT) as usize + 4 {
        let h = site.height(p);
        let mut best: Option<(f64, P)> = None;
        for turn in [-0.35, 0.0, 0.35] {
            let nd = rotate(d, turn + wander);
            let q = add(p, mul(nd, STEP_FT));
            let cost = (site.height(q) - h).abs() + if turn == 0.0 { 0.0 } else { 0.6 };
            if best.is_none_or(|b| cost < b.0) {
                best = Some((cost, nd));
            }
        }
        d = best.unwrap().1;
        let q = add(p, mul(d, STEP_FT));
        let probes: Vec<P> = (1..=5).map(|k| add(p, mul(d, PROBE_FT * k as f64))).collect();
        if len(q) > max_r || probes.iter().any(|&s| dist(s, p0) > 12.0 && (site.wet(s) || site.near_river(s, 10.0))) {
            break;
        }
        // (Across a way: it ends on it there, if at a T's angle; else it turns onto one, or stops.)
        let mut hit: Option<(f64, usize, P, P)> = None;
        for (j, (lp, _)) in lanes.iter().enumerate() {
            for s in lp.windows(2) {
                let Some((t, _)) = seg_cross(p, q, s[0], s[1]) else { continue };
                let x = lerp(p, q, t);
                if dist(x, p0) > 2.0 && (parent != usize::MAX || dist(x, p0) > 45.0) && hit.is_none_or(|h| t < h.0) {
                    hit = Some((t, j, x, sub(s[1], s[0])));
                }
            }
        }
        let near = || lanes.iter().enumerate().any(|(j, (lp, w))| lp.windows(2).any(|s| probes.iter().any(|&pr| seg_dist(pr, s[0], s[1]) < 0.5 * w + JOIN_NEAR_FT && !skip(j, nearest_on(pr, s[0], s[1])))));
        if hit.is_some() || near() {
            // (From the step's end if it crosses nothing, else from its start.)
            let mut joined = None;
            if hit.is_none() {
                pts.push(q);
                joined = join_to(site, &pts, me, lanes, JOIN_REACH_FT, &skip, &no_band);
                if joined.is_none() {
                    pts.pop();
                }
            }
            if let Some((j, x)) = joined.or_else(|| join_to(site, &pts, me, lanes, JOIN_REACH_FT, &skip, &no_band)) {
                pts.push(x);
                return (pts, Some((j, x)));
            }
            if let Some((_, j, x, tg)) = hit {
                if cross(d, mul(tg, 1.0 / len(tg).max(1e-9))).abs() >= t_min {
                    pts.push(x);
                    return (pts, Some((j, x)));
                }
                return (pts, None);
            }
        }
        pts.push(q);
        p = q;
    }
    match join_to(site, &pts, me, lanes, JOIN_REACH_FT, &skip, &no_band) {
        Some((j, x)) => {
            pts.push(x);
            (pts, Some((j, x)))
        }
        None => (pts, None),
    }
}

/// Distance from `p` to `poly`'s outline (0 inside it).
fn outline_d(poly: &[P], p: P) -> f64 {
    if contains(poly, p) {
        return 0.0;
    }
    let m = poly.len();
    (0..m).map(|k| seg_dist(p, poly[k], poly[(k + 1) % m])).fold(f64::MAX, f64::min)
}

/// A lane may end at `p`: a building within 10 ft, a plot within 7.5 with a building within 14 (a
/// croft cut back off the sand after, `fields::crofts_off_sand`, leaves its cottage by the lane).
fn at_home(g: &Ground, l: &Layout, p: P) -> bool {
    near_home(g, l, p, 10.0)
}

/// `at_home`, a building within `m` ft.
fn near_home(g: &Ground, l: &Layout, p: P, m: f64) -> bool {
    let by = |bb: [f64; 4], m: f64| p[0] > bb[0] - m && p[0] < bb[2] + m && p[1] > bb[1] - m && p[1] < bb[3] + m;
    let house = |m: f64| l.buildings.iter().any(|b| by(bbox(&b.poly), m) && outline_d(&b.poly, p) <= m);
    house(m) || (g.boxes.iter().zip(&g.plots).any(|(bb, pl)| by(*bb, 7.5) && outline_d(&pl.pts, p) <= 7.5) && house(14.0))
}

/// By the water: on it, within 30 ft of the river or with water 30 ft off.
fn by_water(site: &Site, p: P) -> bool {
    site.wet(p) || site.near_river(p, 30.0) || (0..8).any(|k| site.wet(add(p, polar_pt(30.0, std::f64::consts::TAU * k as f64 / 8.0))))
}

/// Every lane's two ends joined or there on purpose, once the village is laid out: an end on a way or
/// at the green's rim stays; else it runs on to a way within `JOIN_LATE_FT` where the ground between
/// is clear of the plots and buildings; else, by the water, it is cut back to its last house or yard
/// (about 6 ft past it); else, run on more than `FARM_RUN_FT` past it, it ends at a farmstead
/// (`farmstead`), or is cut back to it. A lane is never cut short of a junction on it (where a lane
/// leaves it or ends on it: it may end at that T); one with nothing along it nor on it is dropped.
/// Children first (a lane's after its parent's), again until nothing changes.
fn settle_ends(g: &mut Ground, l: &mut Layout, lanes: &mut Vec<(Vec<P>, f64)>, n_roads: usize, hedge_share: f64, rng: &mut Pcg32) {
    g.lanes = LaneGrid::new(lanes);
    for _ in 0..4 {
        let mut changed = false;
        for li in (n_roads..lanes.len()).rev() {
            for end in [1, 0] {
                if lanes[li].0.len() < 2 {
                    continue;
                }
                if end == 0 {
                    lanes[li].0.reverse();
                }
                let done = settle_end(g, l, lanes, li, n_roads, hedge_share, rng);
                if end == 0 {
                    lanes[li].0.reverse();
                }
                if done {
                    changed = true;
                    g.lanes = LaneGrid::new(lanes);
                }
            }
        }
        if !changed {
            break;
        }
    }
    lanes.retain(|(pts, _)| pts.len() >= 2);
    g.lanes = LaneGrid::new(lanes);
}

/// `settle_ends` for lane `li`'s last point: whether it changed.
fn settle_end(g: &mut Ground, l: &mut Layout, lanes: &mut [(Vec<P>, f64)], li: usize, n_roads: usize, hedge_share: f64, rng: &mut Pcg32) -> bool {
    let pts = lanes[li].0.clone();
    let e = pts[pts.len() - 1];
    let (well, green) = g.green;
    if on_way(lanes, li, e) {
        return false;
    }
    // (Run on to a way near it: its band clear of the buildings, the plots and the green, short of the way's edge.)
    let clear = |a: P, b: P, hw: f64| {
        let v = sub(b, a);
        let vl = len(v) - hw - 2.0;
        if vl < 0.5 {
            return true;
        }
        let u = mul(v, 1.0 / len(v));
        let band = quad(a, u, LANE_W, vl);
        // (A house it runs by square to it, as `square_to`.)
        let (cos, sin) = (libm::cos(SQUARE_DEG.to_radians()), libm::sin(SQUARE_DEG.to_radians()));
        let square = l.buildings.iter().filter(|h| h.structure == Structure::Roofed && h.poly.len() == 4 && poly_gap(&h.poly, &[a, b]) < 0.5 * LANE_W + SQUARE_FT).all(|h| {
            let e = sub(h.poly[1], h.poly[0]);
            let k = (dot(e, u) / len(e).max(1e-9)).abs();
            k > cos || k < sin
        });
        square && !building_hit(l, &band, None) && !g.plot_hit(&band) && g.off_green(&band, 1.0)
    };
    // (The farthest junction on it: it keeps to there at least.)
    let anchor = lanes.iter().enumerate().filter(|&(j, (q, _))| j != li && j >= n_roads && q.len() >= 2).flat_map(|(_, (q, _))| [q[0], q[q.len() - 1]]).filter(|&a| pts.windows(2).any(|s| seg_dist(a, s[0], s[1]) <= 1.0)).map(|a| (arc_at(&pts, a), a)).max_by(|x, y| x.0.total_cmp(&y.0));
    let floor = anchor.map_or(0.0, |a| a.0);
    let total = length(&pts);
    // (Its last house or yard, a house's front within 10 ft of its edge.)
    let reach = 0.5 * lanes[li].1 + 10.0;
    let mut last = None;
    let mut at = total - 2.0;
    while at > floor {
        if near_home(g, l, point_along(&pts, at).0, reach) {
            last = Some(at);
            break;
        }
        at -= 2.0;
    }
    // (Across a way just before it, past its last house, a crossroads: it ends there.)
    let mut acc = 0.0;
    let mut cross_at = None;
    for k in 1..pts.len() - 1 {
        acc += dist(pts[k - 1], pts[k]);
        if total - acc <= JOIN_LATE_FT && acc >= floor - 0.5 && last.is_none_or(|s| s <= acc + 2.0) && on_way(lanes, li, pts[k]) {
            cross_at = Some(k);
            break;
        }
    }
    if let Some(k) = cross_at {
        lanes[li].0.truncate(k + 1);
        return true;
    }
    if green > 0.0 && dist(e, well) <= green + 0.5 * LANE_W + 2.0 {
        return false;
    }
    // (A few feet short of a way: on it.)
    let near = lanes.iter().enumerate().filter(|&(j, _)| j != li).flat_map(|(j, (q, _))| q.windows(2).map(move |s| (j, nearest_on(e, s[0], s[1])))).min_by(|a, b| dist(e, a.1).total_cmp(&dist(e, b.1)));
    // (No harder a turn for it than a join's.)
    let turn_ok = |x: P| {
        let n = pts.len();
        n < 3 || {
            let (u, v) = (sub(pts[n - 2], pts[n - 3]), sub(x, pts[n - 2]));
            dot(u, v) >= libm::cos(JOIN_TURN_DEG.to_radians()) * len(u) * len(v)
        }
    };
    if let Some((j, x)) = near
        && dist(e, x) <= 6.0
        && dist(pts[pts.len() - 2], e) > 10.0
        && turn_ok(x)
    {
        let x = node(lanes, j, x);
        let n = lanes[li].0.len();
        lanes[li].0[n - 1] = x;
        return true;
    }
    if let Some((j, x)) = join_to(g.site, &pts, li, lanes, JOIN_LATE_FT, &|_, _| false, &clear) {
        let x = node(lanes, j, x);
        lanes[li].0.push(x);
        return true;
    }
    let water = by_water(g.site, e);
    if at_home(g, l, e) {
        return false;
    }
    let from = last.unwrap_or(floor);
    if !water && total - from >= FARM_RUN_FT {
        if let Some(keep) = farmstead(g, l, lanes, li, from + 40.0, hedge_share, rng) {
            lanes[li].0 = cut_at(&pts, keep);
            return true;
        }
    }
    match (last, anchor) {
        (Some(at), _) => lanes[li].0 = cut_at(&pts, at),
        (None, Some((at, a))) => {
            let mut cut = cut_at(&pts, at);
            let n = cut.len();
            if n >= 2 && dist(cut[n - 1], a) < 1.5 {
                cut[n - 1] = a;
            } else {
                cut.push(a);
            }
            lanes[li].0 = cut;
        }
        (None, None) => lanes[li].0.clear(),
    }
    if lanes[li].0.len() < 2 || length(&lanes[li].0) < 1.0 {
        lanes[li].0.clear();
    }
    true
}

/// A farmstead where lane `li` runs on over open ground: a farmhouse (`Ward::Farm`) across the back of
/// a yard (`PlotKind::Farmyard`, hedged or fenced, as the manor's), a barn (the catalog's stables, its
/// stalls inside) along one side, the lane run to its gate. The yard's front 7 ft past the lane's end,
/// on ground open (`yard_ok`) and level enough to work; else with the lane cut every 20 ft back, to
/// `from` ft along it. How much of the lane is kept, if it stands.
fn farmstead(g: &mut Ground, l: &mut Layout, lanes: &[(Vec<P>, f64)], li: usize, from: f64, hedge_share: f64, rng: &mut Pcg32) -> Option<f64> {
    let barn_f = catalog::index_of("stables")?;
    let pts = &lanes[li].0;
    let (hw_, hd) = (rng.range(34.0, 44.0), rng.range(20.0, 26.0));
    let (bw, bd) = (rng.range(38.0, 50.0), rng.range(22.0, 28.0));
    let yw = rng.range(76.0, 96.0).max(2.0 * bd + 30.0).max(hw_ + 14.0);
    let yd = rng.range(72.0, 92.0).max(bw + hd + 24.0);
    let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
    let bound = if rng.next_f64() < hedge_share { Bound::Hedge } else { Bound::Fence };
    let total = length(pts);
    let mut at = total;
    while at >= from {
        let (e, u) = point_along(pts, at);
        let t = [u[1], -u[0]];
        let m = add(e, mul(u, 7.0));
        let yard = quad(m, u, yw, yd);
        let barn = rect(add(m, add(mul(t, side * (0.5 * yw - 6.0 - 0.5 * bd)), mul(u, 8.0 + 0.5 * bw))), u, bw, bd);
        let house = rect(add(m, add(mul(t, -side * (0.5 * yw - 6.0 - 0.5 * hw_)), mul(u, yd - 6.0 - 0.5 * hd))), t, hw_, hd);
        let (lo, hi) = g.site.relief(&yard);
        if hi - lo < 8.0 && g.site.buildable(&house) && g.site.buildable(&barn) {
            // (Off the lanes as they would be: this one cut there.)
            let mut trial = lanes.to_vec();
            trial[li].0 = cut_at(pts, at);
            let held = std::mem::replace(&mut g.lanes, LaneGrid::new(&trial));
            if yard_ok(g, l, &yard) && square_to(&trial, &house, t, None) && square_to(&trial, &barn, u, None) {
                let mut home = new_building(g.site, house, Ward::Farm, None, Structure::Roofed);
                home.residential = catalog::residential_for(Ward::Farm, false) as u8;
                l.buildings.push(home);
                l.buildings.push(new_building(g.site, barn, Ward::Rural, Some(barn_f), Structure::Roofed));
                g.add(Plot { pts: yard, kind: PlotKind::Farmyard, bound });
                return Some(at);
            }
            g.lanes = held;
        }
        at -= 20.0;
    }
    None
}

/// The green's pond (on some) and its tree beside the well, each on dry ground off the roads across
/// it (world ft props); their ways round the well keyed by the village's seed.
fn green_props(site: &Site, seed: u64, well: P, r: f64, roads: &[(Vec<P>, f64)], l: &mut Layout) {
    let h = |k: i64| unit(hash3(seed, k, 0, 0x9e3d));
    let off_roads = |p: P, room: f64| {
        !site.near_network_road(p, room + 2.0) && roads.iter().all(|(q, w)| q.windows(2).all(|s| seg_dist(p, s[0], s[1]) > 0.5 * w + room + 2.0))
    };
    let spot = |a0: f64, rad: f64, room: f64| {
        (0..8).map(|k| polar_pt(rad, a0 + std::f64::consts::TAU * k as f64 / 8.0)).map(|p| add(well, p)).find(|&p| {
            (0..8).all(|j| !site.wet(add(p, polar_pt(room + 3.0, std::f64::consts::TAU * j as f64 / 8.0)))) && !site.near_river(p, room + 6.0) && off_roads(p, room)
        })
    };
    let a0 = std::f64::consts::TAU * h(0);
    let mut put = |at: P, kind: crate::battlemap::Kind, variant: u8, scale: f64, rot: f64| {
        l.props.push(super::Prop { at: add(at, site.center), rot: rot as f32, kind, variant, scale: scale as f32 });
    };
    // (A pond 11-14 ft across the radius, half way out; a tree on the far side, nearer the edge.)
    let pond_r = 11.0 + 3.0 * h(1);
    let pond = (h(2) < 0.55).then(|| spot(a0, 0.5 * r, pond_r)).flatten();
    if let Some(p) = pond {
        put(p, crate::battlemap::Kind::Pond, 0, pond_r / 10.0, std::f64::consts::TAU * h(3));
    }
    let tree_r = 6.0 + 2.0 * h(4);
    if let Some(p) = spot(a0 + std::f64::consts::PI, (0.62 * r).min(r - tree_r - 2.0), tree_r) {
        let kind = if pond.is_some() && h(5) < 0.5 { crate::battlemap::Kind::TreeWillow } else { crate::battlemap::Kind::TreeDeciduous };
        put(p, kind, (h(6) * 4.0) as u8, 0.9 + 0.35 * h(7), std::f64::consts::TAU * h(8));
    }
}

/// Whether a house along `u` stands square to every lane and road within `SQUARE_FT` of it (but by
/// `SQUARE_DEG`): none runs past it at a slant.
/// (By `JUNCTION_DEG` but along `own`, the lane it fronts: a house by a junction square to both ways.)
fn square_to(lanes: &[(Vec<P>, f64)], house: &[P], u: P, own: Option<&[P]>) -> bool {
    let (c, r) = (centroid(house), house.iter().map(|q| dist(*q, centroid(house))).fold(0.0, f64::max));
    let (cos, sin) = (libm::cos(SQUARE_DEG.to_radians()), libm::sin(SQUARE_DEG.to_radians()));
    let (cos_j, sin_j) = (libm::cos(JUNCTION_DEG.to_radians()), libm::sin(JUNCTION_DEG.to_radians()));
    let mine = |s: &[P]| own.is_some_and(|o| o.windows(2).any(|w| seg_dist(lerp(s[0], s[1], 0.5), w[0], w[1]) < 0.01));
    lanes.iter().all(|(pts, w)| {
        pts.windows(2).all(|s| {
            let e = sub(s[1], s[0]);
            let el = len(e);
            if el < 1e-6 || seg_dist(c, s[0], s[1]) > r + 0.5 * w + SQUARE_FT || poly_gap(house, s) > 0.5 * w + SQUARE_FT {
                return true;
            }
            // (Along it or across it.)
            let k = (dot(e, u) / el).abs();
            if mine(s) { k > cos || k < sin } else { k > cos_j || k < sin_j }
        })
    })
}

/// How near a lane's edge a house must stand square to it (ft), and by how much (degrees).
const SQUARE_FT: f64 = 12.0;
const SQUARE_DEG: f64 = 12.0;
const JUNCTION_DEG: f64 = 8.0;

/// A cottage's toft and croft on lane `pts`, side `side`, from `a0` to `a1` along it: its front
/// `front` ft out from the lane's middle, its sides square to the lane where they leave it (so the
/// crofts beside it share theirs), running back `depth` ft or until they meet water or a shore's
/// beach, a lane or a road (`CROFT_LANE_FT` off it; its front half a foot off its own), the green,
/// another plot or building, or ground too steep to work; at least to `house` ft (the cottage's back)
/// and 10 more, else none; deeper than wide and not a wedge. `own`: the cottage's place in the
/// buildings.
#[allow(clippy::too_many_arguments)]
fn croft(g: &Ground, l: &Layout, pts: &[P], side: f64, a0: f64, a1: f64, front: f64, house: f64, depth: f64, own: usize) -> Option<Vec<P>> {
    let (p0, u0) = point_along(pts, a0);
    let (p1, u1) = point_along(pts, a1);
    let (n0, n1) = (mul([-u0[1], u0[0]], side), mul([-u1[1], u1[0]], side));
    let (f0, f1) = (add(p0, mul(n0, front)), add(p1, mul(n1, front)));
    if len(add(n0, n1)) < 1e-6 {
        return None;
    }
    let own_poly = &l.buildings[own].poly;
    let need = house - front + 10.0;
    // (Lines back from the front across it, sampled every 10 ft, the stop found to a foot or so: the
    // depth it reaches. Its sides 1.5 ft in, off the side a croft beside it shares.)
    let mut reach = depth;
    let inward = mul(sub(f1, f0), 1.5 / dist(f0, f1).max(1.0));
    let (s0, s1) = (add(f0, inward), sub(f1, inward));
    for k in 0..=4 {
        let t = k as f64 / 4.0;
        let (f, n) = (lerp(s0, s1, t), {
            let m = lerp(n0, n1, t);
            mul(m, 1.0 / len(m).max(1e-9))
        });
        let ok = |p: P| contains(own_poly, p) || g.open(p);
        let mut prev = g.site.height(f);
        let mut t = 10.0;
        while t <= reach {
            let p = add(f, mul(n, t));
            let h = g.site.height(p);
            if !ok(p) || (h - prev).abs() > CROFT_STEP_FT {
                let (mut lo, mut hi) = (t - 10.0, t);
                if ok(p) {
                    // (Too steep: stopped at the last sample.)
                    hi = lo;
                }
                while hi - lo > 1.5 {
                    let m = 0.5 * (lo + hi);
                    if ok(add(f, mul(n, m))) { lo = m } else { hi = m }
                }
                reach = lo;
                break;
            }
            prev = h;
            t += 10.0;
        }
    }
    let mut d = reach;
    while d >= need {
        let poly = vec![f0, f1, add(f1, mul(n1, d)), add(f0, mul(n0, d))];
        let (front_e, back_e) = (sub(poly[1], poly[0]), sub(poly[2], poly[3]));
        if dot(front_e, back_e) <= 0.0 || len(back_e) < 12.0 || area(&poly).abs() < 1.0 {
            d -= 20.0;
            continue;
        }
        // (Deeper than wide, and most of the rectangle round it: no wide wedge at a bend.)
        let (wide, deep) = frame(&poly);
        if wide > deep {
            return None;
        }
        if area(&poly).abs() < 0.5 * wide * deep {
            d = (0.75 * d).min(d - 15.0);
            continue;
        }
        let core = inset(&poly, &[0.3; 4]);
        // (Its cottage inside it, its sides and back 3 ft off the house: no hedge under a roof.)
        let held = inset(&poly, &[0.0, HOUSE_BOUND_FT, HOUSE_BOUND_FT, HOUSE_BOUND_FT]);
        if held.len() < 3 || !own_poly.iter().all(|c| contains(&held, *c)) {
            return None;
        }
        // (Its sides and back clear of the lanes and the roads, and dry, every 8 ft round: behind its
        // front strip, which is on the lane.)
        let behind = inset(&poly, &[CROFT_LANE_FT + 4.0, 0.5, 0.5, 0.5]);
        let every = |q: &[P], f: &dyn Fn(P) -> bool| {
            (0..q.len()).all(|k| {
                let (a, b) = (q[k], q[(k + 1) % q.len()]);
                let m = (dist(a, b) / 8.0).ceil().max(1.0) as usize;
                (0..m).all(|j| f(lerp(a, b, j as f64 / m as f64)))
            })
        };
        let round_ok = behind.len() >= 3 && every(&behind, &|q| !g.site.near_network_road(q, CROFT_LANE_FT)) && every(&poly, &|q| g.dry(q));
        // (And dry across it, every 8 ft or so.)
        let (nw, nd) = ((dist(f0, f1) / 8.0).ceil() as usize, (d / 8.0).ceil() as usize);
        let inside_ok = || (1..nd).all(|j| (1..nw).all(|i| g.dry(lerp(lerp(poly[0], poly[1], i as f64 / nw as f64), lerp(poly[3], poly[2], i as f64 / nw as f64), j as f64 / nd as f64))));
        if core.len() >= 3 && behind.len() >= 3 && !g.plot_hit(&core) && !building_hit(l, &core, Some(own)) && !g.lanes.touches(&behind, CROFT_LANE_FT) && !g.lanes.touches(&poly, 0.5) && g.off_green(&poly, 1.0) && round_ok && inside_ok() && g.plot_overlap(&poly) < 1.0 {
            return Some(poly);
        }
        d = (0.75 * d).min(d - 15.0);
    }
    None
}

/// A croft's width along its front and its depth square to it (the rectangle round it).
fn frame(poly: &[P]) -> (f64, f64) {
    let t = sub(poly[1], poly[0]);
    let tu = mul(t, 1.0 / len(t).max(1e-9));
    let span = |u: P| poly.iter().map(|q| dot(sub(*q, poly[0]), u)).fold((f64::MAX, f64::MIN), |a, v| (a.0.min(v), a.1.max(v)));
    let ((a0, a1), (b0, b1)) = (span(tu), span([-tu[1], tu[0]]));
    (a1 - a0, b1 - b0)
}

/// The pieces of lane `pts` outside the circle round `c` of radius `r`, each at least 25 ft long.
fn outside(pts: &[P], c: P, r: f64) -> Vec<Vec<P>> {
    let mut out: Vec<Vec<P>> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    let r2 = r * r;
    let inside = |p: P| dot(sub(p, c), sub(p, c)) < r2;
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        let (da, ab) = (sub(a, c), sub(b, a));
        // (Where the segment crosses the circle: |da + t ab| = r.)
        let (qa, qb, qc) = (dot(ab, ab), 2.0 * dot(da, ab), dot(da, da) - r2);
        let disc = qb * qb - 4.0 * qa * qc;
        let (t0, t1) = if qa > 1e-12 && disc > 0.0 {
            let sq = crate::core::sqrt(disc);
            ((-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa))
        } else {
            (2.0, 2.0)
        };
        if !inside(a) && cur.is_empty() {
            cur.push(a);
        }
        if t0 > 0.0 && t0 < 1.0 {
            cur.push(add(a, mul(ab, t0)));
            out.push(std::mem::take(&mut cur));
        }
        if t1 > 0.0 && t1 < 1.0 {
            cur.push(add(a, mul(ab, t1)));
        }
        if !inside(b) {
            cur.push(b);
        }
    }
    out.push(cur);
    out.retain(|q| q.len() >= 2 && q.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>() >= 25.0);
    out
}

/// The church on its lot inside a walled churchyard on the green's edge (east of it first: the
/// church's door toward the green, its apse away, `temple::shape`), its graveyard behind it; a cross
/// plan in a big village, else a hall. Whether there was room.
fn churchyard(g: &mut Ground, l: &mut Layout, green_r: f64, big: bool, rng: &mut Pcg32) -> bool {
    let (Some(temple_f), Some(grave_f)) = (catalog::index_of("temple"), catalog::index_of("graveyard")) else { return false };
    let (cl, cw) = if big { (rng.range(68.0, 80.0), rng.range(46.0, 52.0)) } else { (rng.range(46.0, 60.0), rng.range(27.0, 34.0)) };
    let (front, back, side) = (12.0, rng.range(26.0, 40.0), rng.range(14.0, 22.0));
    let (depth, width) = (front + cl + back, cw + 2.0 * side);
    let a0 = rng.range(-0.3, 0.3);
    // (Its gate on a lane first, else on the green; on level ground first: a churchyard is not
    // laid over a knoll.)
    let level = |yard: &[P]| {
        let (lo, hi) = g.site.relief(yard);
        hi - lo < 6.0
    };
    for (ring, lane, flat) in [(green_r + 6.0, true, true), (green_r + 6.0, false, true), (green_r + 6.0, true, false), (green_r + 6.0, false, false), (green_r + 50.0, false, false)] {
        for step in ROUND {
            let a = a0 + step * std::f64::consts::PI / 6.0;
            let n = [libm::cos(a), libm::sin(a)];
            let m = add(g.green.0, mul(n, ring));
            let yard = quad(m, n, width, depth);
            if (lane && g.lanes.dist(m) > GATE_LANE_FT) || (flat && !level(&yard)) || !yard_ok(g, l, &yard) {
                continue;
            }
            let lot = rect(add(m, mul(n, front + 0.5 * cl)), n, cl, cw);
            if !g.site.buildable(&lot) {
                continue;
            }
            // (The door's end nearer the gate.)
            let door = |q: &[super::Part]| mul(add(q[0].poly[0], q[0].poly[3]), 0.5);
            let shaped = [false, true].map(|b| temple::shape(&lot, b, false));
            let best = shaped.into_iter().flatten().min_by(|x, y| dist(door(&x.1), m).total_cmp(&dist(door(&y.1), m)));
            let Some((outline, parts)) = best else { continue };
            let grave = rect(add(m, mul(n, front + cl + 0.5 * back + 1.0)), n, back - 6.0, width - 8.0);
            let mut b = new_building(g.site, outline, Ward::Temple, Some(temple_f), Structure::Roofed);
            b.parts = parts;
            l.grounds.push((l.buildings.len() as u32, lot));
            l.buildings.push(b);
            l.buildings.push(new_building(g.site, grave, Ward::Rural, Some(grave_f), Structure::Open));
            g.add(Plot { pts: yard, kind: PlotKind::Churchyard, bound: Bound::Wall });
            return true;
        }
    }
    false
}

/// A small village's shrine at the front of a small yard by the green, its graveyard behind it.
fn shrine_yard(g: &mut Ground, l: &mut Layout, green_r: f64, bound: Bound, rng: &mut Pcg32) -> bool {
    let (Some(shrine_f), Some(grave_f)) = (catalog::index_of("shrine"), catalog::index_of("graveyard")) else { return false };
    let (width, depth) = (rng.range(38.0, 48.0), rng.range(52.0, 66.0));
    let (sw, sd) = (rng.range(22.0, 28.0), rng.range(16.0, 20.0));
    let a0 = rng.range(0.0, std::f64::consts::TAU);
    for (ring, lane) in [(green_r + 6.0, true), (green_r + 6.0, false), (green_r + 50.0, false)] {
        for step in ROUND {
            let a = a0 + step * std::f64::consts::PI / 6.0;
            let n = [libm::cos(a), libm::sin(a)];
            let m = add(g.green.0, mul(n, ring));
            let yard = quad(m, n, width, depth);
            let shrine = rect(add(m, mul(n, 6.0 + 0.5 * sd)), [n[1], -n[0]], sw, sd);
            if (lane && g.lanes.dist(m) > GATE_LANE_FT) || !yard_ok(g, l, &yard) || !g.site.buildable(&shrine) {
                continue;
            }
            let g0 = 6.0 + sd + 4.0;
            let grave = rect(add(m, mul(n, 0.5 * (g0 + depth - 3.0))), n, depth - 3.0 - g0, width - 6.0);
            l.buildings.push(new_building(g.site, shrine, Ward::Rural, Some(shrine_f), Structure::Roofed));
            l.buildings.push(new_building(g.site, grave, Ward::Rural, Some(grave_f), Structure::Open));
            g.add(Plot { pts: yard, kind: PlotKind::ShrineYard, bound });
            return true;
        }
    }
    false
}

/// The water mill: on the river's bank where a river runs by (the bank nearer the green), its
/// water-side wall a foot or few from the water: a lane along the bank there goes round its back
/// (`round_mill`). Else across that lane from the water, else by the wharf; its long side along
/// the bank, its footprint within `MILL_WATER_FT` of the water. Else none (`rural::place`: a
/// windmill on a rise).
fn mill(g: &mut Ground, l: &mut Layout, r: f64, wharf: Option<&[P]>, lanes: &mut Vec<(Vec<P>, f64)>, n_roads: usize, rng: &mut Pcg32) -> bool {
    let Some(mill_f) = catalog::index_of("mill") else { return false };
    let (w, d) = (rng.range(30.0, 40.0), rng.range(20.0, 26.0));
    let site = g.site;
    // (Its wheel's room too: off the piers and bridges by 8 ft.)
    let decks = |l: &Layout, poly: &[P]| {
        let (c, u) = (centroid(poly), sub(poly[1], poly[0]));
        let room = rect(c, u, len(u) + 16.0, dist(poly[1], poly[2]) + 16.0);
        !l.piers.iter().chain(&l.bridges).any(|q| overlaps_or_crosses(q, &room))
    };
    let near = |g: &Ground, l: &Layout, poly: &[P]| g.off_green(poly, 6.0) && site.buildable(poly) && !building_hit(l, poly, None) && !g.plot_hit(poly) && g.off_roads(poly, 2.0) && decks(l, poly) && waterside(site, poly, MILL_WATER_FT);
    let fits = |g: &Ground, l: &Layout, poly: &[P]| near(g, l, poly) && !g.lanes.touches(poly, 2.0);
    // (River pieces by their distance from the green, nearer first: the mill's middle, its way
    // along the bank, and its way from the water.)
    let mut pieces: Vec<(f64, usize)> = site.river.iter().enumerate().map(|(i, &(a, b, _))| (len(lerp(a, b, 0.5)), i)).filter(|&(dd, _)| dd > 0.3 * r && dd < 1.1 * r).collect();
    pieces.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let mut banks: Vec<(P, P, P, f64)> = Vec::new();
    for &(_, i) in pieces.iter().take(24) {
        let (a, b, hw) = site.river[i];
        let ab = sub(b, a);
        let al = len(ab);
        if al < 1e-6 {
            continue;
        }
        let q = lerp(a, b, 0.5);
        let t = mul(ab, 1.0 / al);
        let mut nrm = [-t[1], t[0]];
        if dot(nrm, q) > 0.0 {
            nrm = mul(nrm, -1.0);
        }
        banks.push((q, t, nrm, hw));
    }
    // (Else the wharf's shore: where the water starts out from points along it.)
    let mut shore: Vec<(P, P, P)> = Vec::new();
    if banks.is_empty()
        && let Some(wh) = wharf
    {
        let total: f64 = wh.windows(2).map(|s| dist(s[0], s[1])).sum();
        for at in [0.2, 0.8, 0.35, 0.65, 0.1, 0.9, 0.5] {
            let (p, u) = point_along(wh, at * total);
            let mut n = [-u[1], u[0]];
            if dot(n, p) > 0.0 {
                n = mul(n, -1.0);
            }
            if let Some(s) = (0..=40).find(|&s| site.wet(sub(p, mul(n, s as f64)))) {
                shore.push((sub(p, mul(n, s as f64 - 0.5)), u, n));
            }
        }
    }
    let put = |l: &mut Layout, poly: Vec<P>| l.buildings.push(new_building(site, poly, Ward::Rural, Some(mill_f), Structure::Roofed));
    // On the bank: its middle 12.5 ft off the channel at least (`buildable`).
    let off0 = (12.5 - 0.5 * d).max(1.0);
    let edges = banks.iter().map(|&(q, t, nrm, hw)| (add(q, mul(nrm, hw)), t, nrm, off0)).chain(shore.iter().map(|&(e, t, nrm)| (e, t, nrm, 2.0)));
    for (e, t, nrm, o) in edges {
        for off in [o, o + 3.0] {
            let c = add(e, mul(nrm, 0.5 * d + off));
            let poly = rect(c, t, w, d);
            if fits(g, l, &poly) {
                put(l, poly);
                return true;
            }
            if !near(g, l, &poly) {
                continue;
            }
            // (A lane runs there: round the mill's back, or ending at it.)
            let mut moved = lanes.clone();
            let mut ok = true;
            for (li, (pts, lw)) in moved.iter_mut().enumerate() {
                if !LaneGrid::new(&[(pts.clone(), *lw)]).touches(&poly, 2.0) {
                    continue;
                }
                match (li >= n_roads).then(|| round_mill(g, l, pts, *lw, &poly, c, t, nrm, w, d)).flatten() {
                    Some(np) => *pts = np,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            let grid = LaneGrid::new(&moved);
            if grid.touches(&poly, 2.0) || crossings(&moved, n_roads) > crossings(lanes, n_roads) {
                continue;
            }
            *lanes = moved;
            g.lanes = grid;
            put(l, poly);
            return true;
        }
    }
    // (Across the riverside lane from the water.)
    let mut spots: Vec<Vec<P>> = Vec::new();
    for &(q, t, nrm, hw) in &banks {
        for off in [4.0, 8.0, 14.0, 22.0] {
            spots.push(rect(add(q, mul(nrm, hw + 0.5 * d + off)), t, w, d));
        }
    }
    if spots.is_empty()
        && let Some(wh) = wharf
    {
        let total: f64 = wh.windows(2).map(|s| dist(s[0], s[1])).sum();
        for at in [0.2, 0.8, 0.35, 0.65] {
            let (p, u) = point_along(wh, at * total);
            let mut n = [-u[1], u[0]];
            if dot(n, p) > 0.0 {
                n = mul(n, -1.0);
            }
            spots.push(rect(add(p, mul(n, 6.0 + 0.5 * d + 3.0)), u, w, d));
        }
    }
    let Some(poly) = spots.into_iter().find(|p| fits(g, l, p)) else { return false };
    put(l, poly);
    true
}

/// A lane `lw` wide that meets the mill `poly` (middle `c`, `t` along the bank, `nrm` from the
/// water, `w` x `d`) taken round its back, clear of it by 3 ft, or cut short of it where the lane
/// ends there; the new way checked (dry, off the green, the plots, the buildings and the roads).
#[allow(clippy::too_many_arguments)]
fn round_mill(g: &Ground, l: &Layout, pts: &[P], lw: f64, poly: &[P], c: P, t: P, nrm: P, w: f64, d: f64) -> Option<Vec<P>> {
    let hw = 0.5 * lw;
    let m = poly.len();
    let hits = |p: P| contains(poly, p) || (0..m).any(|k| seg_dist(p, poly[k], poly[(k + 1) % m]) < hw + 3.0);
    // (The lane every 2 ft: which piece each sample lies on.)
    let mut samples: Vec<(usize, P)> = Vec::new();
    for (j, s) in pts.windows(2).enumerate() {
        let n = (dist(s[0], s[1]) / 2.0).ceil().max(1.0) as usize;
        samples.extend((0..n).map(|k| (j, lerp(s[0], s[1], k as f64 / n as f64))));
    }
    samples.push((pts.len() - 1, pts[pts.len() - 1]));
    let first = samples.iter().position(|&(_, p)| hits(p))?;
    let last = samples.iter().rposition(|&(_, p)| hits(p))?;
    let at = |p: P| [dot(sub(p, c), t), dot(sub(p, c), nrm)];
    // (Its corners out past the clearance: the bend rounded off below cuts into them.)
    let (w2, d2) = (0.5 * w + hw + 7.0, 0.5 * d + hw + 7.0);
    let corner = |p: P| add(c, add(mul(t, if at(p)[0] < 0.0 { -w2 } else { w2 }), mul(nrm, d2)));
    let mut head: Vec<P> = Vec::new();
    let mut fresh: Vec<P> = Vec::new();
    let mut tail: Vec<P> = Vec::new();
    if first > 0 {
        let (ja, pa) = samples[first - 1];
        head.extend_from_slice(&pts[..=ja]);
        fresh.push(pa);
        if last + 1 < samples.len() {
            fresh.push(corner(pa));
        }
    }
    if last + 1 < samples.len() {
        let (jb, pb) = samples[last + 1];
        if first > 0 {
            let b = corner(pb);
            if dist(b, fresh[fresh.len() - 1]) > 0.5 {
                fresh.push(b);
            }
        }
        fresh.push(pb);
        tail.extend_from_slice(&pts[(jb + 1).min(pts.len())..]);
    }
    // (Rounded: the corners cut twice, from the lane's last point before to its first after, those
    // two kept.)
    let mut way: Vec<P> = head.last().copied().into_iter().chain(fresh.iter().copied()).chain(tail.first().copied()).collect();
    way.dedup_by(|a, b| dist(*a, *b) < 0.5);
    for _ in 0..2 {
        if way.len() < 3 {
            break;
        }
        let mut cut = vec![way[0]];
        for s in way.windows(2) {
            cut.push(lerp(s[0], s[1], 0.25));
            cut.push(lerp(s[0], s[1], 0.75));
        }
        cut.push(way[way.len() - 1]);
        cut.remove(1);
        cut.remove(cut.len() - 2);
        way = cut;
    }
    let mut out: Vec<P> = head.iter().copied().take(head.len().saturating_sub(1)).chain(way.iter().copied()).chain(tail.iter().copied().skip(1)).collect();
    if head.is_empty() || tail.is_empty() {
        // (Cut short of it: the way is what is left.)
        out = head.iter().copied().chain(fresh.iter().copied()).chain(tail.iter().copied()).collect();
        way = fresh.clone();
    }
    out.dedup_by(|a, b| dist(*a, *b) < 0.5);
    if out.len() < 2 || out.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>() < 25.0 {
        return None;
    }
    // (No hard turn left in it: else the mill goes elsewhere.)
    if out.windows(3).any(|q| {
        let (u, v) = (sub(q[1], q[0]), sub(q[2], q[1]));
        len(u) > 1e-6 && len(v) > 1e-6 && dot(u, v) < libm::cos(MILL_TURN_DEG.to_radians()) * len(u) * len(v)
    }) {
        return None;
    }
    // (The new way: its band clear of what is there and of the mill, its ground dry.)
    let (pa, pb) = (way[0], way[way.len() - 1]);
    for s in way.windows(2) {
        let e = sub(s[1], s[0]);
        let el = len(e);
        if el < 0.5 {
            continue;
        }
        let band = quad(s[0], mul(e, 1.0 / el), lw, el);
        if building_hit(l, &band, None) || g.plot_hit(&band) || !g.off_green(&band, 1.0) || poly_gap(&band, poly) < 2.0 || overlaps_or_crosses(&band, poly) {
            return None;
        }
        // (Off the channel; off the roads but where it leaves the old way.)
        let n = (el / 3.0).ceil() as usize;
        if (0..=n).any(|k| {
            let p = lerp(s[0], s[1], k as f64 / n as f64);
            g.site.near_river(p, 1.0) || g.site.wet(p) || (dist(p, pa).min(dist(p, pb)) > lw && g.site.near_network_road(p, hw + 1.0))
        }) {
            return None;
        }
    }
    Some(out)
}

/// Water within `reach` ft of `poly`'s outline, looking straight out from each side (every 5 ft).
fn waterside(site: &Site, poly: &[P], reach: f64) -> bool {
    let n = poly.len();
    let ccw = area(poly) > 0.0;
    (0..n).any(|k| {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        let e = sub(b, a);
        let el = len(e).max(1e-9);
        let out = if ccw { [e[1] / el, -e[0] / el] } else { [-e[1] / el, e[0] / el] };
        (0..=4).any(|j| {
            let p = lerp(a, b, j as f64 / 4.0);
            let steps = (reach / 5.0) as usize;
            (1..=steps).any(|s| site.wet(add(p, mul(out, 5.0 * s as f64))))
        })
    })
}

/// The manor house in its yard on a lane toward the village's edge (or by the church), hedged or
/// walled, its gate on the lane.
fn manor_yard(g: &mut Ground, l: &mut Layout, r: f64, lanes: &[(Vec<P>, f64)], rng: &mut Pcg32) -> bool {
    let (yw, yd) = (rng.range(80.0, 110.0), rng.range(70.0, 100.0));
    let (hw_, hd) = (rng.range(48.0, 62.0), rng.range(30.0, 38.0));
    let bound = if rng.next_f64() < 0.4 { Bound::Wall } else { Bound::Hedge };
    let by_church = rng.next_f64() < 0.5;
    let church = l.buildings.iter().find(|b| temple::is_temple(b)).map(|b| centroid(&b.poly));
    let jit = hash3(rng.next_f64().to_bits(), 0, 0, 0x3a40);
    // (Spots every 40 ft along the lanes, either side, 0.45-1.0 of the radius out; the nearest the
    // church first, else the farthest out.)
    let mut spots: Vec<(f64, P, P)> = Vec::new();
    for (li, (pts, w)) in lanes.iter().enumerate() {
        let total: f64 = pts.windows(2).map(|s| dist(s[0], s[1])).sum();
        let mut a = 20.0;
        let mut k = 0;
        while a < total {
            let (p, u) = point_along(pts, a);
            let rr = len(p);
            if rr > 0.45 * r && rr < r {
                for side in [-1.0, 1.0] {
                    let n = mul([-u[1], u[0]], side);
                    let m = add(p, mul(n, 0.5 * w + 2.0));
                    let score = match church.filter(|_| by_church) {
                        Some(c) => dist(m, c),
                        None => -rr,
                    } + 30.0 * unit(hash3(jit, li as i64, k, side as i64));
                    spots.push((score, m, n));
                }
            }
            a += 40.0;
            k += 1;
        }
    }
    spots.sort_by(|x, y| x.0.total_cmp(&y.0));
    for &(_, m, n) in spots.iter().take(60) {
        let yard = quad(m, n, yw, yd);
        let house = rect(add(m, mul(n, 0.55 * yd)), [n[1], -n[0]], hw_, hd);
        if !g.site.buildable(&house) || !yard_ok(g, l, &yard) {
            continue;
        }
        l.buildings.push(new_building(g.site, house, Ward::Noble, None, Structure::Roofed));
        g.add(Plot { pts: yard, kind: PlotKind::ManorYard, bound });
        return true;
    }
    false
}

/// What a village's layout set out for a purpose `assign_functions` leaves be: its church stands for
/// its shrine; its mill is the layout's or none; its manor (the noble estate, `Ward::Noble`) is no
/// one's business.
pub(super) fn reserve(l: &Layout, needs: &mut Vec<usize>, taken: &mut [bool]) {
    if l.buildings.iter().any(temple::is_temple)
        && let Some(sh) = catalog::index_of("shrine")
        && let Some(k) = needs.iter().position(|&f| f == sh)
    {
        needs.remove(k);
    }
    // (A village's mill stands only by the water, where its layout put it: no mill on a dry lot.)
    if let Some(mill) = catalog::index_of("mill") {
        needs.retain(|&f| f != mill);
    }
    // (Nor a farmstead's farmhouse: a farmer's.)
    for (i, b) in l.buildings.iter().enumerate() {
        if b.ward == Ward::Noble || b.ward == Ward::Farm {
            taken[i] = true;
        }
    }
}

/// Whether a house `assign_functions` adds to make up the numbers (local ft) keeps off a village's
/// lanes, roads, green and plots; anywhere in any other layout.
pub(super) fn spare_ok(site: &Site, l: &Layout, poly: &[P]) -> bool {
    if !village_of(l) {
        return true;
    }
    let m = poly.len();
    let edges = || (0..m).map(|k| [poly[k], poly[(k + 1) % m]]);
    let lanes_ok = l.streets.iter().all(|s| s.windows(2).all(|w| !contains(poly, w[0]) && edges().all(|e| poly_gap(&e, w) > 7.0)));
    let roads_ok = edges().all(|e| (0..=8).all(|j| !site.near_network_road(lerp(e[0], e[1], j as f64 / 8.0), 1.0)));
    let green_ok = l.plazas.iter().filter(|p| p.kind == PlazaKind::Green).all(|g| {
        let (c, r) = (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0]));
        !contains(poly, c) && edges().all(|e| seg_dist(c, e[0], e[1]) > r + 2.0)
    });
    lanes_ok && roads_ok && green_ok && l.crofts.iter().all(|p| !overlaps_or_crosses(&p.pts, poly))
}

/// The way the lane or road nearest `c` runs (within 80 ft) in a village: a house `assign_functions`
/// adds stands square to it.
pub(super) fn spare_dir(l: &Layout, c: P) -> Option<P> {
    if !village_of(l) {
        return None;
    }
    let segs = l.streets.iter().map(|s| s.as_slice()).chain(l.roads.iter().map(|r| r.0.as_slice())).flat_map(|s| s.windows(2));
    let (d, s) = segs.map(|w| (seg_dist(c, w[0], w[1]), w)).filter(|x| dist(x.1[0], x.1[1]) > 1e-6).min_by(|a, b| a.0.total_cmp(&b.0))?;
    (d < 80.0).then(|| sub(s[1], s[0]))
}

/// What `assign_functions` adds to a village house's score for an inn or a tavern by the green
/// (most on its edge, none 80 ft off it): the inn fronts the green where a house there can be one.
pub(super) fn green_bonus(l: &Layout, poly: &[P], key: &str) -> f64 {
    if !village_of(l) || !matches!(key, "inn" | "tavern") {
        return 0.0;
    }
    let Some(g) = l.plazas.iter().find(|p| p.kind == PlazaKind::Green) else { return 0.0 };
    let (c, r) = (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0]));
    let off = poly.iter().map(|q| dist(*q, c)).fold(f64::MAX, f64::min) - r;
    8.0 * (1.0 - off / 80.0).clamp(0.0, 1.0)
}

fn village_of(l: &Layout) -> bool {
    l.tier == Tier::Village && !l.site
}

/// A plot's boundary as runs (hedge, fence or wall): a croft's sides and back, a yard's whole round
/// but its gate.
fn runs(p: &Plot) -> Vec<(P, P)> {
    let n = p.pts.len();
    let mut out = Vec::new();
    for k in 0..n {
        let (a, b) = (p.pts[k], p.pts[(k + 1) % n]);
        if k == 0 {
            if p.kind == PlotKind::Croft {
                continue;
            }
            let e = dist(a, b);
            if e > GATE_FT + 4.0 {
                let h = 0.5 * GATE_FT / e;
                out.push((a, lerp(a, b, 0.5 - h)));
                out.push((lerp(a, b, 0.5 + h), b));
            }
            continue;
        }
        out.push((a, b));
    }
    out
}

/// Each plot's hedges, fences and walls as drawn (`runs`), but where they run along a plot's before it
/// (within 3 ft, pieces of 4 ft or so): one hedge between two crofts, not two side by side.
pub fn bounds(l: &Layout) -> Vec<(P, P, Bound, PlotKind)> {
    let mut out = Vec::new();
    let boxes: Vec<[f64; 4]> = l.crofts.iter().map(|p| bbox(&p.pts)).collect();
    for (k, p) in l.crofts.iter().enumerate() {
        let shared = |q: P| {
            (0..k).any(|j| {
                let (b, o) = (boxes[j], &l.crofts[j].pts);
                q[0] > b[0] - 3.0 && q[0] < b[2] + 3.0 && q[1] > b[1] - 3.0 && q[1] < b[3] + 3.0 && (0..o.len()).any(|i| seg_dist(q, o[i], o[(i + 1) % o.len()]) < 3.0)
            })
        };
        for (a, b) in runs(p) {
            let m = (dist(a, b) / 4.0).ceil().max(1.0) as usize;
            let mut from: Option<usize> = None;
            for j in 0..=m {
                let keep = j < m && !shared(lerp(a, b, (j as f64 + 0.5) / m as f64));
                match (keep, from) {
                    (true, None) => from = Some(j),
                    (false, Some(f)) => {
                        out.push((lerp(a, b, f as f64 / m as f64), lerp(a, b, j as f64 / m as f64), p.bound, p.kind));
                        from = None;
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

/// The church's way from the churchyard's gate to its door (a 6-ft strip), if it stands in `p`.
fn church_path(l: &Layout, p: &Plot) -> Option<Vec<P>> {
    let b = l.buildings.iter().find(|b| temple::shaped(b) && contains(&p.pts, centroid(&b.poly)))?;
    let door = mul(add(b.parts[0].poly[0], b.parts[0].poly[3]), 0.5);
    let gate = lerp(p.pts[0], p.pts[1], 0.5);
    let dl = dist(door, gate);
    (dl > 1.0).then(|| rect(lerp(gate, door, 0.5), sub(door, gate), dl + 3.0, 6.0))
}

/// A village's plots on the battlemap's ground (`battlemap`'s settlement codes): crofts and yards
/// `URBAN_CROFT`, a churchyard `URBAN_GRAVEYARD` (its headstones) with its way to the door trodden
/// bare.
pub fn stamp(l: &Layout, put: &mut dyn FnMut(&[P], u8)) {
    if !village_of(l) {
        return;
    }
    for p in &l.crofts {
        if p.kind == PlotKind::Churchyard {
            // (Its headstones kept off the wall: a band of grass inside it.)
            put(&p.pts, URBAN_CROFT);
            let inner = inset(&p.pts, &[5.0; 4]);
            if inner.len() >= 3 {
                put(&inner, URBAN_GRAVEYARD);
            }
            if let Some(path) = church_path(l, p) {
                put(&path, URBAN_YARD);
            }
        } else {
            put(&p.pts, URBAN_CROFT);
        }
    }
}

/// The low walls of a village's walled yards (the churchyard's, a walled manor's), as the battlemap
/// raises a palace forecourt's.
pub fn wall_runs(l: &Layout) -> Vec<(P, P)> {
    if !village_of(l) {
        return Vec::new();
    }
    // (A churchyard's wall is a low one, seen over: `rural::battle` draws it as a dry-stone run.)
    l.crofts.iter().filter(|p| p.bound == Bound::Wall && p.kind != PlotKind::Churchyard).flat_map(runs).collect()
}

/// A village's plan zoom polygons (`payload::pack_sites`, before its buildings): its green's well
/// and the churchyard's wall. The green's and a churchyard's grass are the land's own (no fill, so
/// they match the biome's ground round them); the crofts and their bounds are seen on the
/// battlemap only.
pub fn plan_polys(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    if !village_of(l) {
        return;
    }
    for pl in l.plazas.iter().filter(|p| p.kind == PlazaKind::Green) {
        push(&circle(centroid(&pl.pts), 3.0, 10), PLAN_STONE);
    }
    for (a, b, bound, _) in bounds(l).into_iter().filter(|b| b.3 == PlotKind::Churchyard) {
        let e = dist(a, b);
        if bound == Bound::Wall && e > 0.5 {
            push(&rect(lerp(a, b, 0.5), sub(b, a), e + WALL_FT, WALL_FT), PLAN_WALL);
        }
    }
}

/// A village's headstones at plan zoom (after its buildings): rows of ticks across each churchyard
/// (off the church and its way to the door) and each graveyard of a shrine's yard.
pub fn plan_ticks(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    if !village_of(l) {
        return;
    }
    let near_roofed = |q: P| {
        l.buildings.iter().filter(|b| b.structure == Structure::Roofed).any(|b| {
            let bb = bbox(&b.poly);
            q[0] > bb[0] - 4.0 && q[0] < bb[2] + 4.0 && q[1] > bb[1] - 4.0 && q[1] < bb[3] + 4.0 && (contains(&b.poly, q) || b.poly.iter().zip(b.poly.iter().cycle().skip(1)).any(|(a, c)| seg_dist(q, *a, *c) < 3.0))
        })
    };
    let mut rows = |frame: &[P], skip_axis: bool, salt: i64| {
        let (f0, t, n) = (frame[0], sub(frame[1], frame[0]), sub(frame[3], frame[0]));
        let (w, d) = (len(t), len(n));
        if w < 6.0 || d < 6.0 {
            return;
        }
        let (tu, nu) = (mul(t, 1.0 / w), mul(n, 1.0 / d));
        // (Rows 7 ft apart, 4.5 ft along them, each stone a foot or so off its place and a little
        // turned; 5 ft in from a churchyard's wall.)
        let m = if skip_axis { 5.0 } else { 2.5 };
        let (mut row, mut s) = (0i64, m + 1.5);
        while s < d - m {
            let (mut col, mut x) = (0i64, m + 1.0);
            while x < w - m {
                let h = |k: i64| unit(hash3(l.index as u64, salt * 4096 + row, col, 0x6e57 + k));
                let q = add(f0, add(mul(tu, x + h(1) - 0.5), mul(nu, s + 1.6 * (h(2) - 0.5))));
                let turn = 0.25 * (h(3) - 0.5);
                let dir = add(tu, mul(nu, turn));
                if h(0) < 0.85 && !(skip_axis && (x - 0.5 * w).abs() < 5.0) && !near_roofed(q) {
                    push(&rect(q, dir, 2.4, 1.0), PLAN_HEADSTONE);
                }
                x += 4.5;
                col += 1;
            }
            s += 7.0;
            row += 1;
        }
    };
    for (k, p) in l.crofts.iter().enumerate() {
        if p.kind == PlotKind::Churchyard {
            rows(&p.pts, true, k as i64);
        }
    }
    // (A shrine's graveyard: the open building in its yard.)
    for (k, b) in l.buildings.iter().enumerate() {
        if b.structure == Structure::Open && b.poly.len() == 4 && l.crofts.iter().any(|p| p.kind == PlotKind::ShrineYard && contains(&p.pts, centroid(&b.poly))) {
            rows(&b.poly, false, 1000 + k as i64);
        }
    }
}

/// A village's lanes off the roads (`Layout::streets`) as plan zoom draws roads (`payload`: x, y,
/// width, class): narrow dirt tracks.
pub fn plan_lanes(l: &Layout) -> Vec<Vec<[f64; 4]>> {
    if !village_of(l) {
        return Vec::new();
    }
    let track = crate::t0::roads::RoadClass::Track as u8 as f64;
    l.streets.iter().map(|s| s.iter().map(|p| [p[0], p[1], LANE_W, track]).collect()).collect()
}

/// How far `p` (world ft) is from a village's lanes (a croft's front, a lane's edge): where a
/// cottage's door faces (`interior`); far for any other layout.
pub fn front_d(l: &Layout, p: P) -> f64 {
    if !village_of(l) {
        return f64::MAX;
    }
    let fronts = l.crofts.iter().map(|c| seg_dist(p, c.pts[0], c.pts[1]));
    let lanes = l.streets.iter().flat_map(|s| s.windows(2).map(move |w| seg_dist(p, w[0], w[1]) - 5.0));
    fronts.chain(lanes).fold(f64::MAX, f64::min)
}
