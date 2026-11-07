//! Villages (`Tier::Village`): no wards, patches or walls. Lanes from the roads through the village
//! (and from its green), cottages along them with their long sides to the lane, each on its toft and
//! croft (a plot running back from the lane, hedged or fenced: `Plot`); the church in its walled
//! churchyard by the green (a small village's shrine in a small yard with its graveyard), the mill on
//! the bank where there is water, a manor in its own yard in the bigger villages and some others; the
//! fields round it all, off the plots (`Layout::fields`).

use super::catalog::{self, Ward};
use super::geom::*;
use super::{Building, Layout, Plaza, PlazaKind, Site, Structure, branch_points, chamfer, overlaps_or_crosses, piers_along, point_along, polar_pt, rect, shore_run, split_field, temple, track, voronoi};
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

/// Plan zoom's site polygons (`payload::pack_sites`, `gen/protocol.ts`; v63 countryside, 16-23).
pub const PLAN_CROFT: u32 = 16;
pub const PLAN_HEDGE: u32 = 17;
pub const PLAN_FENCE: u32 = 18;
pub const PLAN_CHURCHYARD: u32 = 19;
pub const PLAN_HEADSTONE: u32 = 20;
pub const PLAN_GREEN: u32 = 21;
pub const PLAN_WALL: u32 = 22;
/// A plan polygon in the monuments' pale stone (the green's well).
const PLAN_STONE: u32 = 7;

/// A yard's gate (ft), and how near a lane's edge it is wanted (else it opens onto the green).
const GATE_FT: f64 = 10.0;
const GATE_LANE_FT: f64 = 30.0;
/// A hedge's, a fence's and a yard wall's width at plan zoom (ft): about the same line.
const HEDGE_FT: f64 = 2.6;
const FENCE_FT: f64 = 2.0;
const WALL_FT: f64 = 2.4;
/// A croft's width (ft) and depth back from the lane.
const CROFT_W: (f64, f64) = (30.0, 60.0);
const CROFT_D: (f64, f64) = (70.0, 150.0);
/// How near a lane or a road's edge a croft may come but at its front (ft).
const CROFT_LANE_FT: f64 = 2.5;
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
pub(super) const LANE_W: f64 = 10.0;
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
        let m = poly.len();
        !self.lanes.touches(poly, margin)
            && (0..m).all(|k| {
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
    // lane runs from the green down to it.
    let wharf = shore_run(site, reach);
    if let Some(wh) = &wharf {
        let (mid, _) = point_along(wh, 0.5 * wh.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>());
        let to = sub(mid, [0.0, 0.0]);
        let tl = len(to);
        if tl > 60.0 {
            let d = mul(to, 1.0 / tl);
            let link = track(site, mul(d, 45.0), d, tl + 10.0, &[(wh.clone(), 12.0)], rng);
            if link.len() >= 2 {
                lanes.push((link, LANE_W));
            }
        }
        lanes.push((wh.clone(), 12.0));
        piers_along(site, wh, rng, l);
    }
    // Branch lanes (after watabou's road trackers): from the roads (or the green), lanes
    // leave every `min_block` ft, step 25 ft at a time turning toward level ground, and stop
    // at water, the village edge, or where they meet another lane.
    let min_block = (4.0 + 8.0 * rng.next_f64()) * 28.0;
    let mut starts: Vec<(P, P, u32)> = Vec::new();
    if lanes.is_empty() {
        let a0 = rng.range(0.0, std::f64::consts::TAU);
        for k in 0..3 {
            let a = a0 + std::f64::consts::TAU * k as f64 / 3.0 + rng.range(-0.4, 0.4);
            let d = [libm::cos(a), libm::sin(a)];
            starts.push((mul(d, 50.0), d, 0));
        }
    } else {
        for (pts, _) in &lanes {
            branch_points(pts, min_block, rng, &mut starts, 1);
        }
    }
    let mut made = 0;
    let max_lanes = 3 + (s.population / 80) as usize;
    let mut k = 0;
    while k < starts.len() && made < max_lanes {
        let (p0, d0, depth) = starts[k];
        k += 1;
        let lane = track(site, p0, d0, r * 1.05, &lanes, rng);
        if lane.len() >= 4 {
            if depth < 2 {
                let mut more = Vec::new();
                branch_points(&lane, min_block * 1.5, rng, &mut more, depth + 1);
                starts.extend(more);
            }
            lanes.push((lane, LANE_W));
            made += 1;
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
    for (pts, _) in lanes.iter().skip(n_roads) {
        l.streets.push(pts.clone());
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
    mill(&mut g, l, r, wharf.as_deref(), rng);
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
    for (li, (pts, w)) in lanes.iter().enumerate() {
        let k = if li < n_roads { (0..pts.len()).min_by(|&a, &b| len(pts[a]).total_cmp(&len(pts[b])).then(a.cmp(&b))).unwrap_or(0) } else { 0 };
        if k >= 1 {
            fronts.push((pts[..=k].iter().rev().copied().collect(), *w));
        }
        if pts.len() - k >= 2 {
            fronts.push((pts[k..].to_vec(), *w));
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
            walks.push(Walk { lane, side, along: rng.range(0.0, 20.0), total, last: f64::MIN, house_end: f64::MIN });
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
            let density = 1.0 - 0.75 * (len(p) / reach).min(1.0);
            let d = rng.range(16.0, 24.0);
            if rng.next_f64() < density {
                let back = 0.5 * road_w + (5.0 * (1.0 + spread) * rng.range(0.6, 1.4)).min(9.0) + 0.5 * d;
                let house = rect(add(p, mul(n, side * back)), u, w, d);
                // (Its front square to the lane: its two corners nearest the lane about as far from it.)
                let mut ds: Vec<f64> = house.iter().map(|c| pts.windows(2).map(|s| seg_dist(*c, s[0], s[1])).fold(f64::MAX, f64::min)).collect();
                ds.sort_by(f64::total_cmp);
                if ds[1] - ds[0] < 3.0 && clear(&house, l) && site.buildable(&house) && !g.plot_hit(&house) && off(&g, &house) {
                    l.buildings.push(new_building(site, house, Ward::Rural, None, Structure::Roofed));
                    let own = l.buildings.len() - 1;
                    let (e0, e1) = (rng.range(3.0, 10.0), rng.range(3.0, 10.0));
                    let depth = rng.range(CROFT_D.0, CROFT_D.1);
                    let bound = if rng.next_f64() < hedge_share { Bound::Hedge } else { Bound::Fence };
                    let mut a0 = (along - e0).max(wk.house_end + 1.0);
                    // (Up to the last croft beside it if that is near: no sliver between them.)
                    if wk.last > f64::MIN && a0 < wk.last + 12.0 {
                        a0 = wk.last;
                    }
                    wk.house_end = along + w;
                    let a1 = (along + w + e1).max(a0 + CROFT_W.0).min(a0 + CROFT_W.1).max(along + w + 1.0);
                    if let Some(pl) = croft(&g, l, pts, side, a0, a1, 0.5 * road_w + 1.5, back + 0.5 * d, depth, own) {
                        g.add(Plot { pts: pl, kind: PlotKind::Croft, bound });
                        wk.last = a1;
                    }
                }
            }
            let avg3 = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0;
            wk.along += w + 22.0 * avg3 * 2.0;
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
                have += area(&f).abs();
                plots.push((f, 1));
            }
        }
    }
    l.fields.extend(super::fields::belt_order(site, s.seed, &plots, want, &far));
    l.crofts = g.plots;
}

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

/// The water mill: on the river's bank where a river runs by (the bank nearer the green; across the
/// lane along it from the water if one runs there), else by the wharf; its long side along the bank,
/// its footprint within `MILL_WATER_FT` of the water. Else none (`rural::place`: a windmill on a rise).
fn mill(g: &mut Ground, l: &mut Layout, r: f64, wharf: Option<&[P]>, rng: &mut Pcg32) -> bool {
    let Some(mill_f) = catalog::index_of("mill") else { return false };
    let (w, d) = (rng.range(30.0, 40.0), rng.range(20.0, 26.0));
    let site = g.site;
    let fits = |g: &Ground, l: &Layout, poly: &[P]| {
        g.off_green(poly, 6.0) && site.buildable(poly) && !building_hit(l, poly, None) && !g.plot_hit(poly) && g.off_lanes(poly, 2.0) && waterside(site, poly, MILL_WATER_FT)
    };
    let mut spots: Vec<Vec<P>> = Vec::new();
    // (River pieces by their distance from the green, nearer first.)
    let mut pieces: Vec<(f64, usize)> = site.river.iter().enumerate().map(|(i, &(a, b, _))| (len(lerp(a, b, 0.5)), i)).filter(|&(dd, _)| dd > 0.3 * r && dd < 1.1 * r).collect();
    pieces.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    for &(_, i) in pieces.iter().take(24) {
        let (a, b, hw) = site.river[i];
        let q = lerp(a, b, 0.5);
        let ab = sub(b, a);
        let al = len(ab);
        if al < 1e-6 {
            continue;
        }
        let t = mul(ab, 1.0 / al);
        let mut nrm = [-t[1], t[0]];
        if dot(nrm, q) > 0.0 {
            nrm = mul(nrm, -1.0);
        }
        // (On the bank, else across the riverside lane from it.)
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
    l.buildings.push(new_building(site, poly, Ward::Rural, Some(mill_f), Structure::Roofed));
    true
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
    for (i, b) in l.buildings.iter().enumerate() {
        if b.ward == Ward::Noble {
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
pub fn bounds(l: &Layout) -> Vec<(P, P, Bound)> {
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
                        out.push((lerp(a, b, f as f64 / m as f64), lerp(a, b, j as f64 / m as f64), p.bound));
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
    l.crofts.iter().filter(|p| p.bound == Bound::Wall).flat_map(runs).collect()
}

/// A village's plan zoom polygons (`payload::pack_sites`, before its buildings): its green and its
/// well, each plot's ground and its hedges, fences or walls (`bounds`).
pub fn plan_polys(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    if !village_of(l) {
        return;
    }
    for pl in l.plazas.iter().filter(|p| p.kind == PlazaKind::Green) {
        push(&pl.pts, PLAN_GREEN);
        push(&circle(centroid(&pl.pts), 3.0, 10), PLAN_STONE);
    }
    for p in &l.crofts {
        push(&p.pts, if matches!(p.kind, PlotKind::Churchyard | PlotKind::ShrineYard) { PLAN_CHURCHYARD } else { PLAN_CROFT });
    }
    for (a, b, bound) in bounds(l) {
        let (kind, w) = match bound {
            Bound::Hedge => (PLAN_HEDGE, HEDGE_FT),
            Bound::Fence => (PLAN_FENCE, FENCE_FT),
            Bound::Wall => (PLAN_WALL, WALL_FT),
        };
        let e = dist(a, b);
        if e > 0.5 {
            push(&rect(lerp(a, b, 0.5), sub(b, a), e + w, w), kind);
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
