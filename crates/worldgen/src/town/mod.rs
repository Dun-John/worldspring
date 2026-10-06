//! Settlement layouts, generated on demand from T0 (pure functions of world + settlement).
//!
//! Towns and cities (after watabou's Medieval Fantasy City Generator): Voronoi patches on a
//! spiral, relaxed; the inner ones are wards (plaza, castle on high ground, temples by the
//! plaza, docks on water, noble quarters by the castle, merchants on the main streets, craft
//! and common elsewhere), the outer ring is farmland with slums by the gates. Walls follow the
//! inner boundary with gates where the roads arrive; main streets run from each gate to the
//! plaza along patch edges. Each ward is cut into lots by recursive splits across the long
//! axis; buildings are lots shrunk by an alley gap. Villages grow along their roads.
//!
//! Every building gets a ground-floor level (`pad_ft`, for interiors) and a function from the
//! catalog: required ones first (an inn in every village,
//! the whole catalog in a metropolis), then residential.

pub mod catalog;
pub mod geom;
pub mod mesh;
pub mod sites;

use std::cell::RefCell;
use std::collections::{BTreeMap, BinaryHeap};
use std::rc::Rc;

use catalog::{CATALOG, Naming, RESIDENTIAL, Ward};
use geom::*;
use mesh::Mesh;

use crate::World;
use crate::core::hash::{FastMap, FastSet};
use crate::core::rng::{Pcg32, hash2, hash3, unit};
use crate::t0::T0;
use crate::t0::names::Namer;
use crate::t0::settle::{Settlement, Tier};

#[derive(Clone, Debug)]
pub struct Building {
    /// Footprint (world ft), convex (buildings drawn by hand: any simple polygon).
    pub poly: Vec<P>,
    pub ward: Ward,
    /// Catalog index; `None` for a residence.
    pub func: Option<u16>,
    /// Index into `catalog::RESIDENTIAL` when `func` is `None`.
    pub residential: u8,
    pub name: Option<String>,
    pub floors: u8,
    /// Ground-floor pad elevation (ft).
    pub pad_ft: f32,
    pub structure: Structure,
    /// The roof drawn by hand (`None`: as its kind has it, see `interior::battlements`).
    pub roof: Option<RoofStyle>,
    /// Roof colour (index into the battlemap's tints), else picked.
    pub tint: Option<u8>,
}

/// A roof chosen for a building drawn by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofStyle {
    Hip,
    Battlements,
    /// Sloping up from every side to the middle (a cone on a round tower).
    Cone,
}

/// What stands on a footprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    /// Walls and a roof (enterable).
    Roofed,
    /// An open-air enclosure (graveyard).
    Open,
    /// Broken walls, no roof.
    Ruin,
}

impl Building {
    pub fn label(&self) -> &'static str {
        match self.func {
            Some(f) => CATALOG[f as usize].name,
            None => RESIDENTIAL[self.residential as usize],
        }
    }
}

/// Named district kinds (watabou's DistrictType): what a district grew from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuarterKind {
    Center,
    Castle,
    Docks,
    Bridge,
    Gate,
    Bank,
    Park,
    Sprawl,
    Regular,
}

impl QuarterKind {
    pub fn name(self) -> &'static str {
        match self {
            QuarterKind::Center => "center",
            QuarterKind::Castle => "castle",
            QuarterKind::Docks => "docks",
            QuarterKind::Bridge => "bridge",
            QuarterKind::Gate => "gate",
            QuarterKind::Bank => "riverbank",
            QuarterKind::Park => "park",
            QuarterKind::Sprawl => "sprawl",
            QuarterKind::Regular => "residential",
        }
    }
}

/// A named district: a run of neighbouring patches grown from an anchor.
#[derive(Clone, Debug)]
pub struct Quarter {
    pub name: String,
    pub kind: QuarterKind,
    /// Its patches (world ft).
    pub patches: Vec<Vec<P>>,
    /// Label baseline (world ft), left to right: a gentle curve along the long axis.
    pub label: Vec<P>,
}

#[derive(Clone, Debug)]
pub struct Layout {
    /// Settlement index in `T0::settlements`.
    pub index: u32,
    pub center: P,
    pub radius: f64,
    pub tier: Tier,
    /// On the sea, a lake or a river (water trades are guaranteed only then).
    pub on_water: bool,
    /// City walls (open where water guards the edge) and castle curtain walls.
    pub walls: Vec<Vec<P>>,
    pub towers: Vec<P>,
    pub gates: Vec<P>,
    /// Main streets from the gates to the plaza.
    pub streets: Vec<Vec<P>>,
    /// Streets drawn as roads (world ft): approaches from each road's end to its gate (road
    /// class 0–2), paved streets (3: the strip between a block and its patch edge, one per
    /// side), and paved main streets and streets over bridges (4). (points, class, width ft)
    pub roads: Vec<(Vec<P>, u8, f64)>,
    pub plazas: Vec<Vec<P>>,
    pub fields: Vec<Vec<P>>,
    /// Built-up patches (streets are the ground inside these that no building covers).
    pub districts: Vec<Vec<P>>,
    /// Built blocks (ward patches minus their streets): drawn instead of buildings when
    /// zoomed out.
    pub blocks: Vec<Vec<P>>,
    pub buildings: Vec<Building>,
    /// Bridge decks over the river (plank rectangles).
    pub bridges: Vec<Vec<P>>,
    /// Piers at the docks.
    pub piers: Vec<Vec<P>>,
    /// Towers flanking the gates (larger than wall towers).
    pub gate_towers: Vec<P>,
    /// Plaza monuments (plinth footprints).
    pub monuments: Vec<Vec<P>>,
    /// Named districts (towns and up).
    pub quarters: Vec<Quarter>,
    /// Castle curtains: centre and radius (world ft), so their towers know which way is in.
    pub castles: Vec<(P, f64)>,
    /// Bounding box of everything (world ft): x0, y0, x1, y1.
    pub bbox: [f64; 4],
    /// A point-of-interest site (ruin, tower, roadside inn), not a settlement.
    pub site: bool,
    /// Ways underground (see `under`).
    pub entrances: Vec<Entrance>,
    /// Bare packed earth levelled to a height (ft): a camp's clearing.
    pub yards: Vec<(Vec<P>, f32)>,
    /// Battlemap objects the layout sets out itself (a camp's tents, fire, bedrolls).
    pub props: Vec<Prop>,
}

/// A battlemap object placed by a layout: where (world ft), facing (rad), kind, variant, size.
#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub at: P,
    pub rot: f32,
    pub kind: crate::battlemap::Kind,
    pub variant: u8,
    pub scale: f32,
}

/// A way underground: the opening (world ft), the way the passage runs in (unit), and the
/// kind of site below.
#[derive(Clone, Copy, Debug)]
pub struct Entrance {
    pub at: P,
    pub dir: P,
    pub kind: crate::under::UnderKind,
}

/// Built-up radius (ft) by tier and population.
pub fn urban_radius(tier: Tier, population: u32) -> f64 {
    let p = crate::core::sqrt(population as f64);
    match tier {
        Tier::Village => 150.0 + 12.0 * p,
        Tier::Town => 11.0 * p,
        Tier::City | Tier::Metropolis => 12.0 * p,
    }
}

/// Where network roads stop short of a town (ft from its centre): outside the walls and
/// most of the sprawl; the town draws the last stretch to its gate.
pub fn road_trim_radius(tier: Tier, population: u32) -> f64 {
    1.5 * urban_radius(tier, population)
}

/// How far a settlement's layout (fields included) can reach from its center.
pub fn reach(s: &Settlement) -> f64 {
    let r = urban_radius(s.tier, s.population);
    if s.tier == Tier::Village { r * 2.2 } else { r * (OUTSKIRTS + 0.15) }
}

/// How far (ft) from its centre a settlement's pad reaches: the terrain eases from the
/// ground its layout was planned on (all of the layout's reach) back to the land round it
/// over a broad band, so whatever stands between them (fine detail, a river valley the
/// land outside lacks) is a gentle slope rather than a rim.
pub fn pad_extent(reach: f64) -> f64 {
    reach + (0.6 * reach).max(900.0)
}

/// The settlement pad's weight at `d` ft from its centre: 1 over the layout, 0 past
/// `pad_extent`.
pub fn pad_weight(d: f64, reach: f64) -> f64 {
    1.0 - crate::core::noise::smoothstep(reach, pad_extent(reach), d)
}

// ---------------------------------------------------------------------------------------
// Memo: layouts are pure, so each worker builds a settlement once and keeps it.

thread_local! {
    static CACHE: RefCell<(u64, FastMap<u32, Rc<Layout>>)> = RefCell::new((0, FastMap::default()));
}

pub fn layout(world: &World, t0: &T0, index: usize) -> Rc<Layout> {
    let key = index as u32;
    if let Some(l) = CACHE.with(|c| {
        let c = c.borrow();
        if c.0 == world.hash { c.1.get(&key).cloned() } else { None }
    }) {
        return l;
    }
    let n = t0.settlements.len();
    let l = Rc::new(if index < n { generate(world, t0, index) } else { sites::generate(world, t0, index - n, key) });
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != world.hash {
            c.0 = world.hash;
            c.1.clear();
        }
        c.1.insert(key, l.clone());
    });
    l
}

/// Forget cached layouts from `index` on (created sites changed: they follow the generated
/// ones, whose layouts stay cached).
pub fn forget_from(index: usize) {
    CACHE.with(|c| c.borrow_mut().1.retain(|&k, _| (k as usize) < index));
}

/// Settlement and site layouts whose reach intersects the rectangle (world ft). Layout index
/// `i` is settlement `i`, then POI `i - settlements.len()`.
pub fn layouts_near(world: &World, t0: &T0, rect: [f64; 4]) -> Vec<Rc<Layout>> {
    let hit = |x: f64, y: f64, r: f64| x + r >= rect[0] && x - r <= rect[2] && y + r >= rect[1] && y - r <= rect[3];
    let n = t0.settlements.len();
    let towns = t0.settlements.iter().enumerate().filter(|(_, s)| hit(s.x, s.y, reach(s))).map(|(i, _)| i);
    let pois = t0.pois.iter().enumerate().filter(|&(i, p)| hit(p.x, p.y, sites::SITE_REACH_FT) && !t0.created_site(i).is_some_and(|c| c.removed)).map(|(i, _)| n + i);
    towns.chain(pois).map(|i| layout(world, t0, i)).collect()
}

/// Number of layouts (settlements then sites).
pub fn layout_count(t0: &T0) -> usize {
    t0.settlements.len() + t0.pois.len()
}

// ---------------------------------------------------------------------------------------
// Terrain context.

struct Site<'a> {
    t0: &'a T0,
    center: P,
    /// Spacing (ft) of the last coarse terrain level, for `T0::ground_at`, and its nodes
    /// near the settlement (a town asks for the ground many thousands of times).
    lattice_ft: f64,
    nodes: (i64, i64, usize, Vec<f64>),
    /// Standing water (sea or lake) within the settlement's T0 neighbourhood: shore banks
    /// only matter then.
    lakeside: bool,
    /// River channel pieces near the settlement: (a, b, half width) in local coords.
    river: Vec<(P, P, f64)>,
    /// Pieces by grid cell (`RIVER_CELL` ft), each listed in every cell its bbox touches.
    river_grid: FastMap<(i64, i64), Vec<u32>>,
    river_max_hw: f64,
    /// How far the river valleys lower the ground (`RiverNet::valley`, ≤ 0), at the nodes of
    /// a `VALLEY_CELL`-ft lattice (local coords), filled as `height` asks.
    valley: RefCell<FastMap<(i64, i64), f32>>,
    /// Network roads near the settlement as ~20-ft chords (a, b, half width; local), sampled
    /// on first use.
    roads: std::cell::OnceCell<(Vec<(P, P, f64)>, FastMap<(i64, i64), Vec<u32>>)>,
    /// How far from the centre (ft) the settlement's layout can reach.
    reach: f64,
}

const VALLEY_CELL: f64 = 50.0;

const RIVER_CELL: f64 = 250.0;
/// Network road chords near a settlement are binned by this (`Site::near_network_road`, margins
/// up to 10 ft).
const ROAD_CELL: f64 = 100.0;

impl<'a> Site<'a> {
    fn new(t0: &'a T0, center: P, lattice_ft: f64, layout_reach: f64, river: Vec<(P, P, f64)>) -> Site<'a> {
        let mut river_grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
        let mut river_max_hw = 0.0f64;
        for (i, &(a, b, hw)) in river.iter().enumerate() {
            river_max_hw = river_max_hw.max(hw);
            let (x0, x1) = (crate::core::floor((a[0].min(b[0]) - hw) / RIVER_CELL) as i64, crate::core::floor((a[0].max(b[0]) + hw) / RIVER_CELL) as i64);
            let (y0, y1) = (crate::core::floor((a[1].min(b[1]) - hw) / RIVER_CELL) as i64, crate::core::floor((a[1].max(b[1]) + hw) / RIVER_CELL) as i64);
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    river_grid.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        // Shore banks reach at most about a T0 cell from wet cells.
        let reach = 12_000.0 + 2.0 * t0.cell_ft;
        let step = 0.25 * t0.cell_ft;
        let n = (2.0 * reach / step).ceil() as i64;
        let lakeside = (0..=n).any(|j| (0..=n).any(|i| t0.sample_lake(center[0] - reach + i as f64 * step, center[1] - reach + j as f64 * step).0 > crate::t0::hydro::DRY));
        // The lattice nodes around the settlement, precomputed (x0, y0, width, values).
        let (nx0, ny0) = (crate::core::floor((center[0] - reach) / lattice_ft) as i64 - 2, crate::core::floor((center[1] - reach) / lattice_ft) as i64 - 2);
        let (nx1, ny1) = (crate::core::floor((center[0] + reach) / lattice_ft) as i64 + 3, crate::core::floor((center[1] + reach) / lattice_ft) as i64 + 3);
        let nw = (nx1 - nx0 + 1) as usize;
        let values: Vec<f64> = (ny0..=ny1).flat_map(|j| (nx0..=nx1).map(move |i| (i, j))).map(|(i, j)| t0.ground_node(i, j, lattice_ft)).collect();
        Site { t0, center, lattice_ft, nodes: (nx0, ny0, nw, values), lakeside, river, river_grid, river_max_hw, valley: RefCell::new(FastMap::default()), roads: std::cell::OnceCell::new(), reach: layout_reach }
    }
}

impl Site<'_> {
    fn world(&self, p: P) -> P {
        add(p, self.center)
    }
    /// How far from `c` along `u` (by `sign`) a deck `width` wide first reaches ground that is dry
    /// all the way across it (clear of standing water and 2 ft clear of the river), searching from
    /// `from` in 5-ft steps: its corners stand on the banks.
    fn dry_reach(&self, c: P, u: P, width: f64, sign: f64, from: f64) -> f64 {
        let v = [-u[1], u[0]];
        let wet_across = |t: f64| {
            (0..=4).any(|k| {
                let p = add(add(c, mul(u, sign * t)), mul(v, (k as f64 / 4.0 - 0.5) * width));
                self.wet_standing(p) || self.near_river(p, 2.0)
            })
        };
        let mut t = from;
        while t < 600.0 && wet_across(t) {
            t += 5.0;
        }
        t
    }
    /// The ground the settlement is planned on: the T0 surface in its river valleys (as the
    /// terrain carves them; settlement pads only take the fine detail away).
    fn height(&self, p: P) -> f64 {
        let g = self.ground(p);
        // The valleys' depth is smooth: interpolated between lattice nodes.
        let (u, v) = (p[0] / VALLEY_CELL, p[1] / VALLEY_CELL);
        let (i0, j0) = (crate::core::floor(u), crate::core::floor(v));
        let (fu, fv) = (u - i0, v - j0);
        let mut cache = self.valley.borrow_mut();
        let mut at = |i: i64, j: i64| {
            *cache.entry((i, j)).or_insert_with(|| {
                let q = [i as f64 * VALLEY_CELL, j as f64 * VALLEY_CELL];
                let (w, h) = (self.world(q), self.ground(q));
                (self.t0.rivers.valley(h, w[0], w[1]) - h) as f32
            }) as f64
        };
        let (i, j) = (i0 as i64, j0 as i64);
        let top = at(i, j) + (at(i + 1, j) - at(i, j)) * fu;
        let bottom = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu;
        g + top + (bottom - top) * fv
    }
    /// The T0 surface (with shore banks), as the coarsest terrain level holds it.
    fn ground(&self, p: P) -> f64 {
        let w = self.world(p);
        let (x0, y0, nw, values) = &self.nodes;
        self.t0.ground_with(w[0], w[1], self.lattice_ft, self.lakeside, &mut |i, j| {
            let (a, b) = (i - x0, j - y0);
            if a >= 0 && b >= 0 && (a as usize) < *nw && ((b as usize) * nw + a as usize) < values.len() {
                values[b as usize * nw + a as usize]
            } else {
                self.t0.ground_node(i, j, self.lattice_ft)
            }
        })
    }
    /// Sea or lake (not the river channel): inside the drawn shoreline (the same water mask
    /// the terrain renders), so buildings, quays and piers agree with the visible shore.
    fn wet_standing(&self, p: P) -> bool {
        let w = self.world(p);
        // The water lookup gives a level wherever a nearby cell is wet; it is only water
        // where that level is above the ground (as the terrain draws it).
        let level = self.t0.sample_water(w[0], w[1]);
        level > crate::t0::hydro::DRY && (level as f64) > self.height(p)
    }
    fn wet(&self, p: P) -> bool {
        self.wet_standing(p) || self.near_river(p, 0.0)
    }
    fn near_river(&self, p: P, margin: f64) -> bool {
        if self.river.is_empty() {
            return false;
        }
        let r = self.river_max_hw + margin;
        let (x0, x1) = (crate::core::floor((p[0] - r) / RIVER_CELL) as i64, crate::core::floor((p[0] + r) / RIVER_CELL) as i64);
        let (y0, y1) = (crate::core::floor((p[1] - r) / RIVER_CELL) as i64, crate::core::floor((p[1] + r) / RIVER_CELL) as i64);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if let Some(ids) = self.river_grid.get(&(cx, cy)) {
                    for &i in ids {
                        let (a, b, hw) = self.river[i as usize];
                        if seg_dist(p, a, b) < hw + margin {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    /// A deck (pier) within 6 ft of a network road anywhere round its outline (every 5 ft).
    fn deck_on_road(&self, poly: &[P]) -> bool {
        let m = poly.len();
        (0..m).any(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
            (0..n).any(|j| self.near_network_road(lerp(a, b, j as f64 / n as f64), 6.0))
        }) || self.near_network_road(centroid(poly), 6.0)
    }
    /// Within `margin` ft of a network road's edge (`p` local; the curve every ~20 ft).
    fn near_network_road(&self, p: P, margin: f64) -> bool {
        let roads = self.roads.get_or_init(|| {
            let (c, r) = (self.center, self.reach);
            let mut out = Vec::new();
            for (ri, k) in self.t0.roads.segments_near([c[0] - r, c[1] - r, c[0] + r, c[1] + r], 0.0) {
                let rc = &self.t0.roads.roads[ri as usize];
                let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(1.0, 64.0) as usize;
                let pts: Vec<P> = (0..=n).map(|j| sub(rc.eval(k as usize, j as f64 / n as f64, 5.0, self.t0.cell_ft).p, c)).collect();
                out.extend(pts.windows(2).map(|q| (q[0], q[1], 0.5 * rc.class.width_ft())));
            }
            // By `ROAD_CELL`-ft cell, padded by the widest road's half width and a deck's margin.
            let mut grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
            for (i, &(a, b, hw)) in out.iter().enumerate() {
                let m = hw + 10.0;
                for cy in crate::core::floor((a[1].min(b[1]) - m) / ROAD_CELL) as i64..=crate::core::floor((a[1].max(b[1]) + m) / ROAD_CELL) as i64 {
                    for cx in crate::core::floor((a[0].min(b[0]) - m) / ROAD_CELL) as i64..=crate::core::floor((a[0].max(b[0]) + m) / ROAD_CELL) as i64 {
                        grid.entry((cx, cy)).or_default().push(i as u32);
                    }
                }
            }
            (out, grid)
        });
        let cell = (crate::core::floor(p[0] / ROAD_CELL) as i64, crate::core::floor(p[1] / ROAD_CELL) as i64);
        roads.1.get(&cell).into_iter().flatten().any(|&i| {
            let (a, b, hw) = roads.0[i as usize];
            seg_dist(p, a, b) < hw + margin
        })
    }
    /// Direction (unit, local) a network road travels as it arrives at its end `e` (local).
    fn road_heading(&self, e: P) -> Option<P> {
        let w = self.world(e);
        for (ri, k) in self.t0.roads.segments_near([w[0] - 5.0, w[1] - 5.0, w[0] + 5.0, w[1] + 5.0], 0.0) {
            let rc = &self.t0.roads.roads[ri as usize];
            let n = rc.pts.len();
            for (kk, t, back) in [(0usize, 0.0, 0.08), (n.saturating_sub(2), 1.0, 0.92)] {
                if k as usize != kk {
                    continue;
                }
                let end = rc.eval(kk, t, 5.0, self.t0.cell_ft).p;
                if dist(end, w) < 1.0 {
                    let before = rc.eval(kk, back, 5.0, self.t0.cell_ft).p;
                    let d = sub(end, before);
                    let l = len(d);
                    return (l > 1e-6).then(|| mul(d, 1.0 / l));
                }
            }
        }
        None
    }
    /// Nearest point of the river centreline within ~400 ft: (point, unit tangent, half width).
    fn nearest_river(&self, p: P) -> Option<(P, P, f64)> {
        let r = self.river_max_hw + 400.0;
        let (x0, x1) = (crate::core::floor((p[0] - r) / RIVER_CELL) as i64, crate::core::floor((p[0] + r) / RIVER_CELL) as i64);
        let (y0, y1) = (crate::core::floor((p[1] - r) / RIVER_CELL) as i64, crate::core::floor((p[1] + r) / RIVER_CELL) as i64);
        let mut best: Option<(f64, P, P, f64)> = None;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                for &i in self.river_grid.get(&(cx, cy)).into_iter().flatten() {
                    let (a, b, hw) = self.river[i as usize];
                    let ab = sub(b, a);
                    let l2 = dot(ab, ab).max(1e-12);
                    let t = (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0);
                    let q = add(a, mul(ab, t));
                    let d = dist(p, q);
                    if best.is_none_or(|x| d < x.0) {
                        best = Some((d, q, mul(ab, 1.0 / crate::core::sqrt(l2)), hw));
                    }
                }
            }
        }
        best.map(|(_, q, t, hw)| (q, t, hw))
    }

    /// Buildable: dry, off the channel, not too steep.
    fn buildable(&self, poly: &[P]) -> bool {
        let c = centroid(poly);
        if self.wet(c) || self.near_river(c, 12.0) {
            return false;
        }
        let (lo, hi) = self.relief(poly);
        hi - lo < 6.0 && self.dry_outline(poly, 10.0)
    }
    /// No point of the outline (every `step` ft or less) on water: a channel at least `step`
    /// wide cannot cross the polygon unseen.
    fn dry_outline(&self, poly: &[P], step: f64) -> bool {
        let m = poly.len();
        (0..m).all(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            let n = (dist(a, b) / step).ceil().max(1.0) as usize;
            (0..n).all(|j| !self.wet(lerp(a, b, j as f64 / n as f64)))
        })
    }
    /// Lowest and highest ground over a polygon (corners, edge midpoints, centre).
    fn relief(&self, poly: &[P]) -> (f64, f64) {
        let m = poly.len();
        let pts = (0..m).flat_map(|k| [poly[k], lerp(poly[k], poly[(k + 1) % m], 0.5)]).chain(std::iter::once(centroid(poly)));
        pts.map(|p| self.height(p)).fold((f64::MAX, f64::MIN), |(lo, hi), h| (lo.min(h), hi.max(h)))
    }
    /// Mean grade across a polygon: its relief over its width.
    fn grade(&self, poly: &[P]) -> f64 {
        let c = centroid(poly);
        let width = 2.0 * poly.iter().map(|p| dist(*p, c)).fold(0.0, f64::max);
        let (lo, hi) = self.relief(poly);
        (hi - lo) / width.max(1.0)
    }
    fn pad(&self, poly: &[P]) -> f32 {
        let c = centroid(poly);
        let hs = poly.iter().map(|p| self.height(*p)).chain(std::iter::once(self.height(c)));
        let (sum, n) = hs.fold((0.0, 0.0), |(s, n), h| (s + h, n + 1.0));
        (sum / n) as f32
    }
}

/// Shoreline points of standing water (sea or lake) near the settlement: where the wet test
/// changes between neighbouring points of a `step` grid, refined by bisection. With each:
/// the unit normal pointing into the water.
fn shore_points(site: &Site, step: f64, within: f64) -> Vec<(P, P)> {
    let n = (2.0 * within / step).ceil() as i64;
    let at = |i: i64, j: i64| [-within + i as f64 * step, -within + j as f64 * step];
    let wet_grid: Vec<bool> = (0..=n).flat_map(|j| (0..=n).map(move |i| (i, j))).map(|(i, j)| {
        let p = at(i, j);
        len(p) < within && site.wet_standing(p)
    }).collect();
    let idx = |i: i64, j: i64| (j * (n + 1) + i) as usize;
    let mut out: Vec<(P, P)> = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            for (di, dj) in [(1i64, 0i64), (0, 1)] {
                let (i2, j2) = (i + di, j + dj);
                if i2 > n || j2 > n || wet_grid[idx(i, j)] == wet_grid[idx(i2, j2)] {
                    continue;
                }
                let (mut dry, mut wetp) = if wet_grid[idx(i, j)] { (at(i2, j2), at(i, j)) } else { (at(i, j), at(i2, j2)) };
                if len(dry) >= within || len(wetp) >= within {
                    continue;
                }
                for _ in 0..6 {
                    let m = lerp(dry, wetp, 0.5);
                    if site.wet_standing(m) { wetp = m } else { dry = m }
                }
                let q = lerp(dry, wetp, 0.5);
                // Normal from the local wet gradient (8 samples around the point).
                let mut g = [0.0, 0.0];
                for k in 0..8 {
                    let ang = std::f64::consts::TAU * k as f64 / 8.0;
                    let d = [libm::cos(ang), libm::sin(ang)];
                    if site.wet_standing(add(q, mul(d, 0.5 * step))) {
                        g = add(g, d);
                    }
                }
                let gl = len(g);
                if gl > 1e-6 {
                    out.push((q, mul(g, 1.0 / gl)));
                }
            }
        }
    }
    out
}

/// Mesh vertices joined to `v` by an edge.
fn adj_vertices(mesh: &Mesh, v: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for face in &mesh.faces {
        let m = face.len();
        for k in 0..m {
            if face[k] == v {
                for u in [face[(k + m - 1) % m], face[(k + 1) % m]] {
                    if !out.contains(&u) {
                        out.push(u);
                    }
                }
            }
        }
    }
    out.sort_unstable();
    out
}

/// Points along the river centreline every `every` ft within `within` of the centre:
/// (point, unit tangent, half width).
fn river_samples(site: &Site, every: f64, within: f64) -> Vec<(P, P, f64)> {
    let mut out = Vec::new();
    let mut acc = 0.5 * every;
    for &(a, b, hw) in &site.river {
        let l = dist(a, b);
        if l < 1e-9 {
            continue;
        }
        let t = mul(sub(b, a), 1.0 / l);
        while acc <= l {
            let p = add(a, mul(t, acc));
            if len(p) < within {
                out.push((p, t, hw));
            }
            acc += every;
        }
        acc -= l;
    }
    out
}

fn river_pieces(t0: &T0, center: P, r: f64) -> Vec<(P, P, f64)> {
    let mut out = Vec::new();
    for (ri, k) in t0.rivers.segments_near(center[0] - r, center[1] - r, center[0] + r, center[1] + r, 0.0) {
        let rc = &t0.rivers.rivers[ri as usize];
        let len = rc.s[k as usize + 1] - rc.s[k as usize];
        // Fine enough to follow meanders and wiggles to a few feet (banks, quays, piers).
        let n = (len / 20.0).ceil().clamp(2.0, 1200.0) as usize;
        let mut prev: Option<(P, f64)> = None;
        for j in 0..=n {
            let cp = rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft);
            let p = sub(cp.p, center);
            if let Some((q, w)) = prev {
                out.push((q, p, 0.5 * w.max(cp.w)));
            }
            prev = Some((p, cp.w));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------

pub fn generate(world: &World, t0: &T0, index: usize) -> Layout {
    let s = &t0.settlements[index];
    let center = [s.x, s.y];
    let r = urban_radius(s.tier, s.population);
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
    let site = Site::new(t0, center, lattice, reach(s) + 500.0, river_pieces(t0, center, reach(s) + 500.0));
    let mut rng = Pcg32::new(hash2(world.stream("town"), index as i64, s.seed as i64), 31);
    let mut l = Layout {
        index: index as u32,
        center,
        radius: r,
        tier: s.tier,
        on_water: false,
        walls: Vec::new(),
        towers: Vec::new(),
        gates: Vec::new(),
        streets: Vec::new(),
        roads: Vec::new(),
        plazas: Vec::new(),
        fields: Vec::new(),
        districts: Vec::new(),
        blocks: Vec::new(),
        buildings: Vec::new(),
        bridges: Vec::new(),
        piers: Vec::new(),
        gate_towers: Vec::new(),
        monuments: Vec::new(),
        quarters: Vec::new(),
        castles: Vec::new(),
        bbox: [0.0; 4],
        site: false,
        entrances: Vec::new(),
        yards: Vec::new(),
        props: Vec::new(),
    };
    // Where roads arrive (local coords): the road ends and passes near the settlement.
    let road_ends = road_ends(t0, center, r, road_trim_radius(s.tier, s.population));
    if s.tier == Tier::Village {
        village(&site, s, r, &road_ends, &mut rng, &mut l);
    } else {
        town(&site, s, r, &road_ends, &mut rng, &mut l);
        walls_off_river(&site, &mut l);
    }
    drop_walled_in(&mut l);
    let on_water = s.coastal || s.river || !site.river.is_empty();
    l.on_water = on_water;
    assign_functions(&site, s, on_water, &mut rng, &mut l);
    // Local → world.
    let tw = |p: &mut P| *p = add(*p, center);
    for b in &mut l.buildings {
        b.poly.iter_mut().for_each(tw);
    }
    for (pts, _, _) in &mut l.roads {
        pts.iter_mut().for_each(tw);
    }
    for poly in l.walls.iter_mut().chain(&mut l.streets).chain(&mut l.plazas).chain(&mut l.fields).chain(&mut l.districts).chain(&mut l.blocks).chain(&mut l.bridges).chain(&mut l.piers) {
        poly.iter_mut().for_each(tw);
    }
    l.towers.iter_mut().chain(&mut l.gates).chain(&mut l.gate_towers).for_each(tw);
    for poly in &mut l.monuments {
        poly.iter_mut().for_each(tw);
    }
    for q in &mut l.quarters {
        q.patches.iter_mut().flatten().chain(&mut q.label).for_each(tw);
    }
    for c in &mut l.castles {
        tw(&mut c.0);
    }
    // Sewers and catacombs under cities.
    if s.tier >= Tier::City {
        l.entrances = crate::under::city_entrances(&l, t0, s.seed);
    }
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in l.buildings.iter().flat_map(|b| b.poly.iter()).chain(l.fields.iter().flatten()).chain(l.walls.iter().flatten()).chain(l.plazas.iter().flatten()) {
        bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
    }
    l.bbox = if bb[0] <= bb[2] { bb } else { [center[0], center[1], center[0], center[1]] };
    l
}

/// Buildings with no way in: every point just outside their walls lies inside another
/// building (lot splitting can box one in). They become yards.
fn drop_walled_in(l: &mut Layout) {
    const CELL: f64 = 80.0;
    let mut grid: FastMap<(i64, i64), Vec<usize>> = FastMap::default();
    for (k, b) in l.buildings.iter().enumerate() {
        let (x0, y0, x1, y1) = b.poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        for cy in crate::core::floor(y0 / CELL) as i64..=crate::core::floor(y1 / CELL) as i64 {
            for cx in crate::core::floor(x0 / CELL) as i64..=crate::core::floor(x1 / CELL) as i64 {
                grid.entry((cx, cy)).or_default().push(k);
            }
        }
    }
    let covered = |p: P, own: usize| {
        grid.get(&(crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64)).into_iter().flatten().any(|&k| k != own && l.buildings[k].structure == Structure::Roofed && contains(&l.buildings[k].poly, p))
    };
    let walled: Vec<bool> = (0..l.buildings.len())
        .map(|k| {
            let b = &l.buildings[k];
            if b.structure != Structure::Roofed {
                return false;
            }
            let c = centroid(&b.poly);
            let m = b.poly.len();
            // Points 3 ft outside each wall, at its quarters.
            (0..m).all(|e| {
                let (a, q) = (b.poly[e], b.poly[(e + 1) % m]);
                [0.25, 0.5, 0.75].iter().all(|&t| {
                    let p = lerp(a, q, t);
                    let d = sub(p, c);
                    covered(add(p, mul(d, 3.0 / len(d).max(1e-9))), k)
                })
            })
        })
        .collect();
    let mut k = 0;
    l.buildings.retain(|_| {
        k += 1;
        !walled[k - 1]
    });
}

/// Where roads arrive (local coords): the end of every road curve that stops at this
/// settlement (roads to towns are trimmed at the urban edge), with its road class. Roads that
/// pass by without ending are reported by direction at the urban edge (class 2).
fn road_ends(t0: &T0, center: P, r: f64, trim: f64) -> Vec<(P, u8)> {
    let mut out: Vec<(P, u8)> = Vec::new();
    let reach = (1.6 * r).max(1.1 * trim) + 200.0;
    for (ri, k) in t0.roads.segments_near([center[0] - reach, center[1] - reach, center[0] + reach, center[1] + reach], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        let n = rc.pts.len();
        for (kk, t) in [(0usize, 0.0), (n.saturating_sub(2), 1.0)] {
            if k as usize != kk {
                continue;
            }
            let e = sub(rc.eval(kk, t, 5.0, t0.cell_ft).p, center);
            if len(e) < 1.05 * trim + 50.0 && !out.iter().any(|(q, _)| dist(*q, e) < 1.0) {
                out.push((e, rc.class as u8));
            }
        }
    }
    if out.is_empty() {
        out = road_dirs(t0, center, r).into_iter().map(|p| (p, 2)).collect();
    }
    out
}

/// Directions roads come from: the nearest point of each road curve within reach, as a
/// local point on or near the urban edge.
fn road_dirs(t0: &T0, center: P, r: f64) -> Vec<P> {
    let reach = r * 1.6 + 200.0;
    let mut out: Vec<P> = Vec::new();
    for (ri, k) in t0.roads.segments_near([center[0] - reach, center[1] - reach, center[0] + reach, center[1] + reach], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        for j in 0..=8 {
            let p = sub(rc.eval(k as usize, j as f64 / 8.0, 20.0, t0.cell_ft).p, center);
            let d = len(p);
            if d < reach && d > r * 0.5 {
                // One entry per direction.
                let dir = mul(p, 1.0 / d);
                if !out.iter().any(|q| dot(mul(*q, 1.0 / len(*q)), dir) > 0.93) {
                    out.push(mul(dir, r));
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Towns and cities.

const GOLDEN: f64 = 2.399_963_229_728_653;

fn voronoi(sites: &[P], boundary: &[P]) -> Vec<Labeled> {
    let base = Labeled { pts: boundary.to_vec(), labels: vec![-1; boundary.len()] };
    (0..sites.len())
        .map(|i| {
            let si = sites[i];
            // Distances once, not per comparison (the same values, so the same order).
            let d: Vec<f64> = sites.iter().map(|&s| dist(s, si)).collect();
            let mut order: Vec<usize> = (0..sites.len()).filter(|&j| j != i).collect();
            order.sort_by(|&a, &b| d[a].total_cmp(&d[b]));
            let mut poly = base.clone();
            let reach = |poly: &Labeled| poly.pts.iter().map(|p| dist(*p, si)).fold(0.0, f64::max);
            let mut r = reach(&poly);
            for j in order {
                let sj = sites[j];
                if d[j] > 2.0 * r {
                    break;
                }
                let n = sub(sj, si);
                let c = (dot(sj, sj) - dot(si, si)) * 0.5;
                let cut = clip(&poly, n, c, j as i32);
                if cut.pts.len() < 3 {
                    poly = cut;
                    break;
                }
                r = reach(&cut);
                poly = cut;
            }
            poly
        })
        .collect()
}

#[inline]
fn qkey(p: P) -> (i64, i64) {
    (crate::core::round(p[0] * 2.0) as i64, crate::core::round(p[1] * 2.0) as i64)
}

/// Outskirts (farmland) reach this many urban radii from the centre.
const OUTSKIRTS: f64 = 2.15;

fn town(site: &Site, s: &Settlement, r: f64, road_ends: &[(P, u8)], rng: &mut Pcg32, l: &mut Layout) {
    let pop = s.population as f64;
    let n_spiral = match s.tier {
        Tier::Town => (pop / 220.0).clamp(9.0, 26.0),
        _ => (pop / 420.0).clamp(12.0, 110.0),
    } as usize;
    // Per-patch random streams: a roll depends only on (settlement, patch, stage), so a
    // change in one patch never reshuffles the rest.
    let pseed = rng.next_u32() as u64 | (rng.next_u32() as u64) << 32;
    let roll = |i: usize, stage: i64| unit(hash3(pseed, i as i64, stage, 0x9a7c));
    let spacing = r * crate::core::sqrt(std::f64::consts::PI / n_spiral as f64);
    let r_edge = r * crate::core::sqrt((n_spiral as f64 + 0.5) / n_spiral as f64);
    // Outskirts: seed radius grows linearly with the index, so patches (and the fields cut
    // from them) grow larger away from town; cells reaching past `r_out` are dropped, which
    // leaves an organic edge instead of a circle.
    let n_outer = ((n_spiral as f64 * 1.6) as usize).max(12);
    let r_out = OUTSKIRTS * r;
    let step = (r_out - r_edge) / n_outer as f64;
    // The plaza: a seed at the centre ringed by four at half-diagonals s and L, so the
    // centre's cell is an s × L rectangle (watabou's market square).
    let polar = |d: f64, a: f64| [d * libm::cos(a), d * libm::sin(a)];
    let pa = roll(0, 1) * std::f64::consts::TAU;
    let plaza_s = spacing * (0.55 + 0.25 * roll(0, 2));
    let plaza_l = plaza_s * (1.0 + 0.5 * roll(0, 3));
    let rhombus = !site.near_river([0.0, 0.0], plaza_l);
    let mut seeds: Vec<(P, bool)> = Vec::new();
    if rhombus {
        let h = std::f64::consts::FRAC_PI_2;
        seeds.push(([0.0, 0.0], true));
        for (d, a) in [(plaza_s, pa), (plaza_l, pa + h), (plaza_s, pa + 2.0 * h), (plaza_l, pa + 3.0 * h)] {
            seeds.push((polar(d, a), true));
        }
    }
    for i in 0..n_spiral + n_outer {
        if rhombus && i < 5 {
            continue;
        }
        let mut sr = Pcg32::new(hash2(pseed, i as i64, 0x5eed), 61);
        let (rad, local) = if i < n_spiral {
            (r * crate::core::sqrt((i as f64 + 0.5) / n_spiral as f64), spacing)
        } else {
            let rad = r_edge + ((i - n_spiral) as f64 + 0.5) * step;
            (rad, crate::core::sqrt(std::f64::consts::TAU * rad * step))
        };
        let a = i as f64 * GOLDEN + sr.range(-0.3, 0.3);
        let j = sr.range(0.0, 0.3 * local);
        let ja = sr.range(0.0, std::f64::consts::TAU);
        let p = [rad * libm::cos(a) + j * libm::cos(ja), rad * libm::sin(a) + j * libm::sin(ja)];
        // Near a river, seeds come in mirrored pairs across the channel instead (below).
        if site.near_river(p, 0.6 * spacing) {
            continue;
        }
        seeds.push((p, false));
    }
    // River: pairs of seeds mirrored across the centreline put a patch edge on the channel,
    // so blocks end at quays along it instead of losing buildings to the water one by one.
    for (c, t, hw) in river_samples(site, 0.9 * spacing, r_out * 1.1) {
        let n = [-t[1], t[0]];
        let off = hw + 0.45 * spacing;
        for q in [add(c, mul(n, off)), sub(c, mul(n, off))] {
            if !seeds.iter().any(|(p, fixed)| *fixed && dist(*p, q) < 0.4 * spacing) {
                seeds.push((q, true));
            }
        }
    }
    // Coast and lake shore: like the river, mirrored seed pairs across the shoreline put a
    // patch edge on it, so blocks line the water with a quay instead of straddling it.
    let shore = shore_points(site, 0.5 * spacing, r_out * 1.1);
    let mut shore_used: Vec<P> = Vec::new();
    for &(q, n) in &shore {
        if shore_used.iter().any(|u| dist(*u, q) < 0.85 * spacing) || site.near_river(q, 0.6 * spacing) {
            continue;
        }
        shore_used.push(q);
        seeds.push((sub(q, mul(n, 0.45 * spacing)), true));
        seeds.push((add(q, mul(n, 0.45 * spacing)), true));
    }
    seeds.retain(|(p, fixed)| *fixed || shore_used.iter().all(|u| dist(*u, *p) > 0.55 * spacing));
    // Inner (city) seeds first.
    let (inner_seeds, outer_seeds): (Vec<_>, Vec<_>) = seeds.into_iter().partition(|(p, _)| len(*p) < r_edge);
    let n_inner = inner_seeds.len();
    let seeds: Vec<(P, bool)> = inner_seeds.into_iter().chain(outer_seeds).collect();
    let total = seeds.len();
    let mut sites: Vec<P> = seeds.iter().map(|x| x.0).collect();
    let fixed_seed: Vec<bool> = seeds.iter().map(|x| x.1).collect();
    let boundary = circle([0.0, 0.0], r_out * 1.3, 48);
    let mut cells = voronoi(&sites, &boundary);
    for _ in 0..2 {
        for i in 0..n_inner {
            if cells[i].pts.len() >= 3 && !fixed_seed[i] {
                sites[i] = centroid(&cells[i].pts);
            }
        }
        cells = voronoi(&sites, &boundary);
    }
    for c in cells.iter_mut().skip(n_inner) {
        if c.pts.iter().any(|p| len(*p) > r_out) {
            *c = Labeled { pts: Vec::new(), labels: Vec::new() };
        }
    }

    // Shared-vertex mesh; tiny edges collapse so patches are mostly quads and pentagons.
    let mut mesh = Mesh::from_cells(&cells);
    mesh.collapse_short_edges((0.3 * spacing).min(100.0));
    let vf = mesh.vertex_faces();
    let inner = |j: i32| j >= 0 && (j as usize) < n_inner;
    let wet: Vec<bool> = (0..total).map(|i| mesh.faces[i].len() < 3 || site.wet(centroid(&mesh.face_pts(i)))).collect();
    let face_ok: Vec<bool> = mesh.faces.iter().map(|f| f.len() >= 3).collect();
    let usable = |i: usize| i < n_inner && !wet[i] && face_ok[i];
    // Edges on the river channel, and vertices that must not move (water, river).
    let mut river_edge: FastSet<(usize, usize)> = Default::default();
    for face in &mesh.faces {
        let m = face.len();
        for k in 0..m {
            let (a, b) = (face[k], face[(k + 1) % m]);
            if site.near_river(lerp(mesh.pos[a], mesh.pos[b], 0.5), 2.0) {
                river_edge.insert((a.min(b), a.max(b)));
            }
        }
    }
    let is_river_edge = |a: usize, b: usize| river_edge.contains(&(a.min(b), a.max(b)));
    let pinned: Vec<bool> = (0..mesh.pos.len()).map(|v| site.near_river(mesh.pos[v], 0.3 * spacing) || vf[v].iter().any(|&f| wet[f])).collect();

    // Wall ring: edges of city patches that face the outskirts (open where water guards).
    let walled = s.tier >= Tier::City || rng.next_f64() < 0.4;
    let mut edge_next: BTreeMap<usize, usize> = BTreeMap::new();
    for i in (0..n_inner).filter(|&i| usable(i)) {
        let (face, lb) = (&mesh.faces[i], &mesh.labels[i]);
        let m = face.len();
        for e in 0..m {
            let nb = lb[e];
            let outside = !inner(nb) || wet[nb as usize];
            let water_edge = nb >= 0 && wet[nb as usize];
            let (a, b) = (face[e], face[(e + 1) % m]);
            if outside && !water_edge && !is_river_edge(a, b) {
                edge_next.insert(a, b);
            }
        }
    }
    let targets: std::collections::BTreeSet<usize> = edge_next.values().copied().collect();
    let mut used: std::collections::BTreeSet<usize> = Default::default();
    let mut chains: Vec<(Vec<usize>, bool)> = Vec::new();
    // Open chains (broken by water) start where nothing leads in; then closed loops.
    let starts: Vec<usize> = edge_next.keys().copied().filter(|k| !targets.contains(k)).chain(edge_next.keys().copied()).collect();
    for start in starts {
        if used.contains(&start) {
            continue;
        }
        let mut chain = vec![start];
        let mut cur = start;
        let mut closed = false;
        while let Some(&nx) = edge_next.get(&cur) {
            if !used.insert(cur) {
                break;
            }
            if nx == start {
                closed = true;
                break;
            }
            chain.push(nx);
            cur = nx;
        }
        if chain.len() >= 3 {
            chains.push((chain, closed));
        }
    }
    chains.sort_by_key(|(c, _)| std::cmp::Reverse(c.len()));
    let ring: std::collections::BTreeSet<usize> = chains.iter().flat_map(|(c, _)| c.iter().copied()).collect();

    // Gates where roads arrive (or two, if none do).
    let mut ends: Vec<(P, u8)> = road_ends.to_vec();
    if ends.is_empty() {
        let a = rng.range(0.0, std::f64::consts::TAU);
        ends = vec![([r * libm::cos(a), r * libm::sin(a)], 2), ([-r * libm::cos(a), -r * libm::sin(a)], 2)];
    }
    // Ring neighbours of each wall corner.
    let mut ring_nb: FastMap<usize, Vec<usize>> = FastMap::default();
    for (chain, closed) in &chains {
        let n = chain.len();
        for k in 0..n {
            if *closed || k > 0 {
                ring_nb.entry(chain[k]).or_default().push(chain[(k + n - 1) % n]);
            }
            if *closed || k + 1 < n {
                ring_nb.entry(chain[k]).or_default().push(chain[(k + 1) % n]);
            }
        }
    }
    // Gates: each road gets its own, never next to another gate: the wall corner (or, for a
    // road that comes in where the wall leaves the waterfront open, a corner of the built
    // edge, from which a main street runs) its approach reaches most cheaply: short, little
    // over water, and not through the town. `gate_of[i]` is the gate for road end i (roads
    // that find none share the nearest).
    let heading_of: Vec<Option<P>> = ends.iter().map(|(e, _)| site.road_heading(*e)).collect();
    let town_faces: Vec<([f64; 4], Vec<P>)> = (0..n_inner)
        .filter(|&i| usable(i))
        .map(|i| {
            let pts = mesh.face_pts(i);
            let bb = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
            (bb, pts)
        })
        .collect();
    let in_town = |p: P| town_faces.iter().any(|(bb, poly)| p[0] >= bb[0] && p[0] <= bb[2] && p[1] >= bb[1] && p[1] <= bb[3] && contains(poly, p));
    // What the approach from road end k to a gate costs: its length, plus 4× the stretch over
    // water (a bridge), plus 20× the stretch through the town on land (not counting its last
    // 40 ft).
    let way_cost = |k: usize, g: P| {
        let curve = approach_curve(ends[k].0, g, heading_of[k]);
        let total: f64 = curve.windows(2).map(|w| dist(w[0], w[1])).sum();
        let n = (total / 15.0).ceil().max(1.0) as usize;
        let step = total / n as f64;
        let samples: Vec<P> = (0..=n).map(|j| point_along(&curve, step * j as f64).0).collect();
        let (mut wet, mut through) = (0.0, 0.0);
        for p in &samples {
            if site.wet(*p) {
                wet += step;
            } else if dist(*p, g) >= 40.0 && in_town(*p) {
                through += step;
            }
        }
        total + 4.0 * wet + 20.0 * through
    };
    // Corners of the built edge: town corners next to the outskirts, the water, or a corner in
    // the river (town patches reach the channel's centre line).
    let edge_vertices: Vec<usize> = (0..mesh.pos.len())
        .filter(|&v| {
            !site.near_river(mesh.pos[v], 0.0)
                && vf[v].iter().any(|&f| usable(f))
                && (vf[v].iter().any(|&f| !usable(f)) || adj_vertices(&mesh, v).iter().any(|&u| site.near_river(mesh.pos[u], 0.0)))
        })
        .collect();
    let mut gates: Vec<usize> = Vec::new();
    let mut gate_of: Vec<Option<usize>> = Vec::new();
    for (k, (e, _)) in ends.iter().enumerate() {
        let mut cands: Vec<usize> = ring.iter().copied().filter(|v| !pinned[*v]).collect();
        cands.sort_by(|&p, &q| dist(mesh.pos[p], *e).total_cmp(&dist(mesh.pos[q], *e)).then(p.cmp(&q)));
        let next_to_gate = |v: usize, gates: &[usize]| ring_nb.get(&v).is_some_and(|nb| nb.iter().any(|x| gates.contains(x)));
        // The nearest few wall corners, and (costing 150 ft more: a wall gate is preferred) the
        // nearest corners of the built edge; the cheapest way in wins.
        let mut options: Vec<(f64, usize)> = cands.iter().copied().filter(|&v| !gates.contains(&v) && !next_to_gate(v, &gates)).take(8).map(|v| (way_cost(k, mesh.pos[v]), v)).collect();
        let mut edge: Vec<usize> = edge_vertices.iter().copied().filter(|v| !gates.contains(v) && !ring.contains(v)).collect();
        edge.sort_by(|&p, &q| dist(mesh.pos[p], *e).total_cmp(&dist(mesh.pos[q], *e)).then(p.cmp(&q)));
        options.extend(edge.into_iter().take(12).map(|v| (way_cost(k, mesh.pos[v]) + 150.0, v)));
        let pick = options.iter().min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|o| o.1);
        match pick {
            Some(g) => {
                gates.push(g);
                gate_of.push(Some(g));
            }
            None => gate_of.push(None),
        }
    }
    // Roads that found no gate of their own share the nearest one.
    for (k, (e, _)) in ends.iter().enumerate() {
        if gate_of[k].is_none() {
            gate_of[k] = gates.iter().copied().min_by(|&p, &q| dist(mesh.pos[p], *e).total_cmp(&dist(mesh.pos[q], *e)));
        }
    }
    // A gate sits in a straight run of wall: flatten its corner.
    for &g in &gates {
        if let Some(nb) = ring_nb.get(&g)
            && nb.len() == 2
        {
            let mid = lerp(mesh.pos[nb[0]], mesh.pos[nb[1]], 0.5);
            mesh.try_move(&vf, g, mid);
        }
    }
    // Round the wall: three smoothing passes (gates, water and river corners stay put).
    for (chain, closed) in &chains {
        mesh.smooth_chain(&vf, chain, *closed, &|v| gates.contains(&v) || pinned[v], 3);
    }
    // Citadel: the highest city patch on the wall (so it shares the city wall), reshaped
    // toward a regular polygon.
    let plaza_face = rhombus.then_some(0).filter(|&i| usable(i));
    let castle_face = if s.tier >= Tier::City {
        (0..n_inner)
            .filter(|&i| usable(i) && Some(i) != plaza_face && len(sites[i]) > 0.25 * r && mesh.faces[i].iter().any(|v| ring.contains(v)))
            .max_by(|&a, &b| site.height(sites[a]).total_cmp(&site.height(sites[b])).then(b.cmp(&a)))
    } else {
        None
    };
    if let Some(cf) = castle_face {
        mesh.equalize(&vf, cf, &|v| gates.contains(&v) || pinned[v], 0.75);
    }

    // Street graph over city patch edges (never along the river channel); main streets run
    // from each gate to the plaza.
    let plaza = plaza_face.or_else(|| (0..n_inner).filter(|&i| usable(i)).min_by(|&a, &b| len(sites[a]).total_cmp(&len(sites[b]))));
    // Vertices in the river channel; streets cross only by explicit links between the banks
    // (each link is a bridge along the street).
    let in_river = |v: usize| site.near_river(mesh.pos[v], 0.0);
    let bank_side = |p: P| site.nearest_river(p).map_or(0.0, |(c, t, _)| cross(t, sub(p, c)).signum());
    let mut crossing_links: Vec<(usize, usize, usize)> = Vec::new();
    for v in 0..mesh.pos.len() {
        if !in_river(v) || !vf[v].iter().any(|&f| usable(f)) {
            continue;
        }
        let nbs: Vec<usize> = adj_vertices(&mesh, v).into_iter().filter(|&u| !in_river(u) && vf[u].iter().any(|&f| usable(f))).collect();
        let p = mesh.pos[v];
        let mut best: Option<(f64, usize, usize)> = None;
        for (ia, &a) in nbs.iter().enumerate() {
            for &b in &nbs[ia + 1..] {
                if bank_side(mesh.pos[a]) * bank_side(mesh.pos[b]) >= 0.0 {
                    continue;
                }
                let (da, db) = (sub(mesh.pos[a], p), sub(mesh.pos[b], p));
                let c = dot(da, db) / (len(da) * len(db)).max(1e-9);
                let ab = sub(mesh.pos[b], mesh.pos[a]);
                let across = site.nearest_river(p).is_some_and(|(_, t, _)| dot(ab, t).abs() < 0.5 * len(ab));
                if across && c < -0.6 && best.is_none_or(|x| c < x.0) {
                    best = Some((c, a, b));
                }
            }
        }
        if let Some((_, a, b)) = best {
            crossing_links.push((a, v, b));
        }
    }
    let mut adj: Graph = FastMap::default();
    for &(a, _, b) in &crossing_links {
        let w = dist(mesh.pos[a], mesh.pos[b]) + 150.0;
        adj.entry((a as i64, 0)).or_default().push(((b as i64, 0), w));
        adj.entry((b as i64, 0)).or_default().push(((a as i64, 0), w));
    }
    for i in (0..n_inner).filter(|&i| usable(i)) {
        let face = &mesh.faces[i];
        let m = face.len();
        for e in 0..m {
            let (a, b) = (face[e], face[(e + 1) % m]);
            if is_river_edge(a, b) || in_river(a) || in_river(b) {
                continue;
            }
            let (pa, pb) = (mesh.pos[a], mesh.pos[b]);
            let grade = (site.height(pa) - site.height(pb)).abs() / dist(pa, pb).max(1.0);
            let w = dist(pa, pb) * (1.0 + 40.0 * grade * grade);
            adj.entry((a as i64, 0)).or_default().push(((b as i64, 0), w));
            adj.entry((b as i64, 0)).or_default().push(((a as i64, 0), w));
        }
    }
    let mut paths: Vec<Vec<usize>> = Vec::new();
    if let Some(pz) = plaza {
        let targets: std::collections::BTreeSet<(i64, i64)> = mesh.faces[pz].iter().map(|&v| (v as i64, 0)).collect();
        for &g in &gates {
            if let Some(path) = shortest(&adj, (g as i64, 0), &targets) {
                paths.push(path.iter().map(|k| k.0 as usize).collect());
            }
        }
    }
    // Curve the main streets: the patches along them bend with them.
    let mut keep_v: Vec<usize> = plaza.map(|pz| mesh.faces[pz].clone()).unwrap_or_default();
    if let Some(cf) = castle_face {
        keep_v.extend(mesh.faces[cf].iter().copied());
    }
    for path in &paths {
        mesh.smooth_chain(&vf, path, false, &|v| gates.contains(&v) || pinned[v] || ring.contains(&v) || keep_v.contains(&v), 2);
    }

    // Bridges carry streets: every crossing link a main street uses gets a bridge along it;
    // then more links (watabou's rule u < 1 − 2·bridged/crossings, spaced out) get a bridge
    // with a short street linking the blocks on either bank.
    let mut bridged: Vec<((usize, usize, usize), bool)> = Vec::new();
    for path in &paths {
        for w in path.windows(2) {
            if let Some(&link) = crossing_links.iter().find(|&&(a, _, b)| (a == w[0] && b == w[1]) || (a == w[1] && b == w[0]))
                && !bridged.iter().any(|(k, _)| *k == link)
            {
                bridged.push((link, true));
            }
        }
    }
    let n_cross = crossing_links.len().max(1);
    for &link in &crossing_links {
        let p = river_crossing(site, mesh.pos[link.0], mesh.pos[link.2]);
        if bridged.iter().any(|((a, _, b), _)| dist(river_crossing(site, mesh.pos[*a], mesh.pos[*b]), p) < 1.5 * spacing) {
            continue;
        }
        if rng.next_f64() < 1.0 - 2.0 * bridged.len() as f64 / n_cross as f64 {
            bridged.push((link, false));
        }
    }
    // The river vertex moves onto the straight bank-to-bank line, so the patch edges on
    // either side run with the street and the blocks leave room for it.
    let mut bridge_streets: Vec<(Vec<P>, bool)> = Vec::new();
    for &((a, v, b), main) in &bridged {
        mesh.pos[v] = river_crossing(site, mesh.pos[a], mesh.pos[b]);
        bridge_streets.push((vec![mesh.pos[a], mesh.pos[v], mesh.pos[b]], main));
    }
    for (pts, main) in &bridge_streets {
        let dir = sub(pts[2], pts[0]);
        let dl = len(dir).max(1e-9);
        let u = mul(dir, 1.0 / dl);
        // Span from bank to bank along the street, plus a landing on each side.
        let (fwd, back) = (site.dry_reach(pts[1], u, 0.0, 1.0, 0.0) + 14.0, site.dry_reach(pts[1], u, 0.0, -1.0, 0.0) + 14.0);
        // A span far wider than the river means the street runs along the water, not over it.
        let hw = site.nearest_river(pts[1]).map_or(0.0, |r| r.2);
        if fwd + back > 4.0 * hw + 80.0 {
            continue;
        }
        // Cities build in stone, as wide as the street's gap between blocks (12 ft a side).
        let width = if s.tier >= Tier::City { 24.0 } else if *main { 16.0 } else { 12.0 };
        // Long enough that every corner stands on the bank, crossing at a slant too.
        let (fwd, back) = (site.dry_reach(pts[1], u, width, 1.0, fwd - 14.0) + 14.0, site.dry_reach(pts[1], u, width, -1.0, back - 14.0) + 14.0);
        let c = add(pts[1], mul(u, 0.5 * (fwd - back)));
        l.bridges.push(rect(c, u, fwd + back, width));
        if !main {
            l.roads.push((pts.clone(), 4, 12.0));
        }
    }

    // Materialise: patches, walls, gates and streets as points.
    let cells: Vec<Labeled> = (0..total).map(|i| Labeled { pts: mesh.face_pts(i), labels: mesh.labels[i].clone() }).collect();
    l.gates = gates.iter().map(|&g| mesh.pos[g]).collect();
    if walled {
        for (chain, closed) in &chains {
            let mut chain = chain.clone();
            // A closed ring starts at a plain corner so a gate never falls on the seam.
            if *closed && let Some(k) = chain.iter().position(|v| !gates.contains(v)) {
                chain.rotate_left(k);
            }
            let (pieces, towers, gate_towers) = wall_pieces(&chain.iter().map(|&v| (mesh.pos[v], gates.contains(&v))).collect::<Vec<_>>(), *closed);
            l.walls.extend(pieces);
            l.towers.extend(towers);
            l.gate_towers.extend(gate_towers);
        }
    }
    let mut main_edges: std::collections::BTreeSet<((i64, i64), (i64, i64))> = Default::default();
    for &((a, v, b), _) in &bridged {
        for (p, q) in [(a, v), (v, b)] {
            let (kp, kq) = (qkey(mesh.pos[p]), qkey(mesh.pos[q]));
            main_edges.insert((kp.min(kq), kp.max(kq)));
        }
    }
    for path in &paths {
        for w in path.windows(2) {
            let (a, b) = (qkey(mesh.pos[w[0]]), qkey(mesh.pos[w[1]]));
            main_edges.insert((a.min(b), a.max(b)));
        }
        l.streets.push(path.iter().map(|&v| mesh.pos[v]).collect());
        let mut drawn: Vec<P> = Vec::new();
        for (k, &v) in path.iter().enumerate() {
            if k > 0
                && crossing_links.iter().any(|&(a, _, b)| (a == path[k - 1] && b == v) || (a == v && b == path[k - 1]))
            {
                drawn.push(river_crossing(site, mesh.pos[path[k - 1]], mesh.pos[v]));
            }
            drawn.push(mesh.pos[v]);
        }
        l.roads.push((drawn, 4, 18.0));
    }
    // Each road continues from where it ends to its gate (`approach_curve`).
    for (k, (e, class)) in ends.iter().enumerate() {
        if let Some(Some(g)) = gate_of.get(k) {
            let gp = mesh.pos[*g];
            let span = dist(*e, gp);
            if span > 1.0 {
                let curve = approach_curve(*e, gp, heading_of[k]);
                let width = [24.0, 16.0, 10.0][(*class as usize).min(2)];
                // Where the approach crosses the river it goes straight over a bridge: the
                // wet stretch of the curve becomes the deck's line, bank to bank.
                let total: f64 = curve.windows(2).map(|w| dist(w[0], w[1])).sum();
                let steps = (total / 5.0).ceil().max(1.0) as usize;
                let samples: Vec<P> = (0..=steps).map(|j| point_along(&curve, total * j as f64 / steps as f64).0).collect();
                let mut straight: Vec<P> = Vec::new();
                let mut j = 0;
                while j < samples.len() {
                    if !site.near_river(samples[j], 0.0) {
                        // One point in eight (40 ft) where it runs on land, and its ends.
                        if j % 8 == 0 || j + 1 == samples.len() || site.near_river(samples[(j + 1).min(samples.len() - 1)], 0.0) {
                            straight.push(samples[j]);
                        }
                        j += 1;
                        continue;
                    }
                    let start = j;
                    while j + 1 < samples.len() && site.near_river(samples[j + 1], 0.0) {
                        j += 1;
                    }
                    let (a, b) = (samples[start.saturating_sub(1)], samples[(j + 1).min(samples.len() - 1)]);
                    if straight.last().is_none_or(|p| dist(*p, a) > 1e-9) {
                        straight.push(a);
                    }
                    straight.push(b);
                    let span = dist(a, b);
                    if span > 1.0 {
                        let u = mul(sub(b, a), 1.0 / span);
                        let deck = if s.tier >= Tier::City { f64::max(width + 4.0, 24.0) } else { width + 4.0 };
                        // From bank to bank at every corner, plus a landing each side.
                        let m = lerp(a, b, 0.5);
                        let (fwd, back) = (site.dry_reach(m, u, deck, 1.0, 0.5 * span) + 8.0, site.dry_reach(m, u, deck, -1.0, 0.5 * span) + 8.0);
                        l.bridges.push(rect(add(m, mul(u, 0.5 * (fwd - back))), u, fwd + back, deck));
                    }
                    j += 2;
                }
                straight.dedup_by(|a, b| dist(*a, *b) < 1e-9);
                let curve = if straight.len() >= 2 { straight } else { curve };
                let total: f64 = curve.windows(2).map(|w| dist(w[0], w[1])).sum();
                let samples: Vec<P> = (0..=steps).map(|j| point_along(&curve, total * j as f64 / steps as f64).0).collect();
                // Once it is in the town (on land, short of the gate itself) the road is a
                // town street, paved like the main streets.
                match samples.iter().position(|p| dist(*p, gp) >= 40.0 && !site.wet(*p) && in_town(*p)) {
                    Some(entry) if entry > 0 => {
                        let at = total * entry as f64 / steps as f64;
                        let mut acc = 0.0;
                        let (mut outside, mut inside) = (vec![curve[0]], vec![samples[entry]]);
                        for w in curve.windows(2) {
                            acc += dist(w[0], w[1]);
                            if acc < at { outside.push(w[1]) } else { inside.push(w[1]) }
                        }
                        outside.push(samples[entry]);
                        l.roads.push((outside, *class, width));
                        l.roads.push((inside, 4, width));
                    }
                    _ => l.roads.push((curve, *class, width)),
                }
            }
        }
    }

    // Wards.
    let mut ward: Vec<Option<Ward>> = vec![None; total];
    let neighbors = |i: usize| -> Vec<usize> { cells[i].labels.iter().filter(|&&j| j >= 0).map(|&j| j as usize).collect() };
    if let Some(pz) = plaza {
        ward[pz] = Some(Ward::Plaza);
    }
    let mut free: Vec<usize> = (0..n_inner).filter(|&i| usable(i) && ward[i].is_none()).collect();
    if let Some(castle) = castle_face {
        ward[castle] = Some(Ward::Castle);
        let nobles: Vec<usize> = neighbors(castle).into_iter().filter(|&j| usable(j) && ward[j].is_none()).take(3).collect();
        for j in nobles {
            ward[j] = Some(Ward::Noble);
        }
    }
    let temples = match s.tier {
        Tier::Town => 1,
        Tier::City => 2,
        _ => 3,
    };
    if let Some(pz) = plaza {
        let chosen: Vec<usize> = neighbors(pz).into_iter().filter(|&j| usable(j) && ward[j].is_none()).take(temples).collect();
        for j in chosen {
            ward[j] = Some(Ward::Temple);
        }
    }
    free.retain(|&i| ward[i].is_none());
    let touches_water = |i: usize| neighbors(i).iter().any(|&j| wet[j]) || cells[i].pts.iter().any(|p| site.wet(*p) || site.near_river(*p, 40.0));
    let mut docks = 0;
    for &i in &free {
        if touches_water(i) && docks < (n_inner / 5).max(1) {
            ward[i] = Some(Ward::Docks);
            docks += 1;
        }
    }
    let gate_patch = |i: usize| l.gates.iter().any(|g| cells[i].pts.iter().any(|p| dist(*p, *g) < 12.0));
    let on_main = |i: usize| {
        let m = cells[i].pts.len();
        (0..m).any(|e| {
            let (a, b) = (qkey(cells[i].pts[e]), qkey(cells[i].pts[(e + 1) % m]));
            main_edges.contains(&(a.min(b), a.max(b)))
        })
    };
    if s.tier == Tier::Metropolis
        && let Some(&p) = free.iter().find(|&&i| ward[i].is_none() && len(sites[i]) > 0.5 * r)
    {
        ward[p] = Some(Ward::Park);
    }
    for &i in &free {
        if ward[i].is_some() {
            continue;
        }
        let roll = roll(i, 10);
        ward[i] = Some(if s.tier >= Tier::City && gate_patch(i) && roll < 0.5 {
            Ward::Military
        } else if on_main(i) && roll < 0.65 {
            Ward::Merchant
        } else if roll < 0.45 {
            Ward::Craft
        } else if s.tier == Tier::Metropolis && roll > 0.9 {
            Ward::Noble
        } else {
            Ward::Common
        });
    }
    // Shanty towns (watabou): outside patches touching the city grow outward one by one,
    // favouring the gates, the centre and the shore; then farmland to an irregular radius,
    // wild beyond.
    let outer_ok = |i: usize| i >= n_inner && !wet[i] && cells[i].pts.len() >= 3;
    let mut slum = vec![false; total];
    let u = roll(0, 20);
    let mut budget = (crate::core::sqrt(n_inner as f64) * (1.0 + u * u * u)) as usize;
    let mut round = 0;
    while budget > 0 && round < 400 {
        round += 1;
        let mut cands: Vec<(usize, f64)> = Vec::new();
        for i in (0..total).filter(|&i| outer_ok(i) && !slum[i]) {
            let touching = neighbors(i).iter().filter(|&&j| usable(j) || slum[j]).count();
            if touching < 2 {
                continue;
            }
            let c = centroid(&cells[i].pts);
            let gate_d = l.gates.iter().map(|g| dist(*g, c)).fold(f64::MAX, f64::min);
            let shore_d = if neighbors(i).iter().any(|&j| wet[j]) { 0.5 * spacing } else { f64::MAX };
            let nearest = (3.0 * len(c)).min(2.0 * gate_d).min(shore_d).max(1.0);
            cands.push((i, 1.0 / (nearest * nearest)));
        }
        if cands.is_empty() {
            break;
        }
        let total_w: f64 = cands.iter().map(|c| c.1).sum();
        let mut pick = unit(hash3(pseed, round, 22, 0x51)) * total_w;
        let mut chosen = cands[cands.len() - 1].0;
        for &(i, w) in &cands {
            if pick < w {
                chosen = i;
                break;
            }
            pick -= w;
        }
        slum[chosen] = true;
        budget -= 1;
    }
    let (a1, a2, f1, f2) = (roll(0, 30) * 0.15 + 0.1, roll(0, 31) * 0.1 + 0.05, roll(0, 32) * std::f64::consts::TAU, roll(0, 33) * std::f64::consts::TAU);
    for i in n_inner..total {
        if !outer_ok(i) {
            continue;
        }
        let c = centroid(&cells[i].pts);
        let th = libm::atan2(c[1], c[0]);
        let farmed = len(c) < (1.0 + a1 * libm::sin(th + f1) + a2 * libm::sin(2.0 * th + f2)) * r_out * 0.8;
        ward[i] = if slum[i] { Some(Ward::Slum) } else if farmed { Some(Ward::Farm) } else { None };
    }

    // Districts: neighbouring patches of the same ward merge into groups of 1–5 that share
    // one set of block and lot parameters. No street runs between the patches of a group,
    // so their blocks meet at party walls instead of each reading as a Voronoi cell. Main
    // streets, water and the wall line are never merged across.
    let groupable = |w: Option<Ward>| {
        matches!(w, Some(Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Docks | Ward::Military | Ward::Temple))
    };
    let base_seed = hash2(pseed, 0x6a5e, 7);
    let mut group = vec![usize::MAX; total];
    for start in 0..total {
        if group[start] != usize::MAX || !groupable(ward[start]) || cells[start].pts.len() < 3 {
            continue;
        }
        let mut grng = Pcg32::new(hash2(base_seed, start as i64, 0x6e0), 43);
        group[start] = start;
        let mut members = vec![start];
        let mut k = 0;
        'grow: while k < members.len() {
            let i = members[k];
            k += 1;
            let m = cells[i].pts.len();
            for e in 0..m {
                let nb = cells[i].labels[e];
                if nb < 0 {
                    continue;
                }
                let j = nb as usize;
                let (ka, kb) = (qkey(cells[i].pts[e]), qkey(cells[i].pts[(e + 1) % m]));
                if group[j] != usize::MAX
                    || ward[j] != ward[start]
                    || wet[j]
                    || cells[j].pts.len() < 3
                    || (j < n_inner) != (start < n_inner)
                    || main_edges.contains(&(ka.min(kb), ka.max(kb)))
                {
                    continue;
                }
                // Stop growing with probability (n − 3) / n (so 1–5 patches, mostly 2–4).
                let n = members.len() as f64;
                if n >= 5.0 || grng.next_f64() < (n - 3.0) / n {
                    break 'grow;
                }
                group[j] = start;
                members.push(j);
            }
        }
    }

    // Quays and landings: city patches on the sea or a lake get a paved quay along the shore;
    // dock patches get piers every ~80 ft, square to the shore, starting at the quay. A pier is
    // built only over open water and clear of other piers.
    for i in 0..n_inner {
        if !usable(i) {
            continue;
        }
        let c = &cells[i];
        let m = c.pts.len();
        let cen = centroid(&c.pts);
        for e in 0..m {
            let nb = c.labels[e];
            if nb < 0 || !wet[nb as usize] {
                continue;
            }
            let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
            let el = dist(a, b);
            if el < 1.0 {
                continue;
            }
            let u = mul(sub(b, a), 1.0 / el);
            let mut out = [-u[1], u[0]];
            if dot(out, sub(lerp(a, b, 0.5), cen)) < 0.0 {
                out = mul(out, -1.0);
            }
            let quay = vec![a, b, sub(b, mul(out, 14.0)), sub(a, mul(out, 14.0))];
            if !site.wet(centroid(&quay)) && !site.near_river(centroid(&quay), 10.0) {
                l.plazas.push(quay);
            }
            if ward[i] != Some(Ward::Docks) {
                continue;
            }
            let n_piers = ((el - 60.0) / 80.0).floor().max(0.0) as usize + 1;
            for k in 0..n_piers {
                if el < 60.0 {
                    break;
                }
                let edge_pt = add(a, mul(u, 30.0 + (el - 60.0) * (k as f64 + 0.5) / n_piers as f64));
                // Anchor on the real shore: walk from the patch edge to the water's edge.
                let mut t = -60.0;
                while t < 60.0 && !site.wet(add(edge_pt, mul(out, t))) {
                    t += 4.0;
                }
                if t >= 60.0 || t <= -60.0 {
                    continue;
                }
                let base = add(edge_pt, mul(out, t - 4.0));
                let tip = add(base, mul(out, 105.0));
                let over_water = (1..=8).all(|j| site.wet(add(base, mul(out, 105.0 * j as f64 / 8.0))));
                let clear = l.piers.iter().all(|pp| {
                    let pc = centroid(pp);
                    seg_dist(pc, base, tip) > 16.0
                });
                let pier = rect(add(base, mul(out, 52.5)), out, 105.0, 10.0);
                let free = !site.deck_on_road(&pier) && !l.bridges.iter().any(|d| overlaps(d, &pier));
                if over_water && clear && free {
                    l.piers.push(pier);
                }
            }
        }
    }

    // Blocks → alleys → lots → buildings.
    for i in 0..total {
        let Some(w) = ward[i] else { continue };
        let c = &cells[i];
        let m = c.pts.len();
        if i < n_inner {
            l.districts.push(c.pts.clone());
        }
        let d: Vec<f64> = (0..m)
            .map(|e| {
                let (a, b) = (qkey(c.pts[e]), qkey(c.pts[(e + 1) % m]));
                let main = main_edges.contains(&(a.min(b), a.max(b)));
                let nb = c.labels[e];
                // On the river: blocks end at a quay 16 ft from the water.
                let mid = lerp(c.pts[e], c.pts[(e + 1) % m], 0.5);
                if site.near_river(mid, 2.0) {
                    return site.nearest_river(mid).map_or(20.0, |r| r.2) + 16.0;
                }
                // On the sea or a lake: a 14 ft quay between the block and the water.
                if nb >= 0 && wet[nb as usize] {
                    return 14.0;
                }
                if w == Ward::Farm {
                    return 7.0;
                }
                // The market square keeps its full cell; its neighbours step back from it.
                if w == Ward::Plaza {
                    return 0.0;
                }
                if nb >= 0 && Some(nb as usize) == plaza && plaza_face.is_some() {
                    return 13.0_f64.max(if main { 12.0 } else { 7.0 });
                }
                if nb >= 0 && group[i] != usize::MAX && group[nb as usize] == group[i] {
                    return 0.0;
                }
                let wall = walled && i < n_inner && (!inner(nb) || wet[nb as usize]);
                (if main { 12.0 } else { 7.0 }) + if wall { 18.0 } else { 0.0 }
            })
            .collect();
        // Paved streets inside the town: each side of a street paves the strip between its
        // patch edge and its block (quays and the waterfront are plazas already).
        if i < n_inner && !matches!(w, Ward::Farm | Ward::Plaza | Ward::Park) {
            let ccw = area(&c.pts) > 0.0;
            for e in 0..m {
                let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
                let nb = c.labels[e];
                let el = dist(a, b);
                if d[e] <= 0.0 || el < 1.0 || (nb >= 0 && wet[nb as usize]) || site.near_river(lerp(a, b, 0.5), 2.0) {
                    continue;
                }
                let ev = mul(sub(b, a), 1.0 / el);
                let inward = if ccw { [-ev[1], ev[0]] } else { [ev[1], -ev[0]] };
                let o = mul(inward, 0.5 * d[e]);
                // Not where the river cuts across the strip.
                let (p, q) = (add(a, o), add(b, o));
                let n = (el / 5.0).ceil() as usize;
                if (0..=n).any(|j| site.near_river(lerp(p, q, j as f64 / n as f64), 0.5 * d[e])) {
                    continue;
                }
                l.roads.push((vec![p, q], 3, d[e] + 1.0));
            }
        }
        let block = inset(&c.pts, &d);
        if block.len() < 3 {
            continue;
        }
        // Street lines bounding the block (outward normal, offset): lots need frontage on one.
        let ccw = area(&c.pts) > 0.0;
        let mut streets: Vec<(P, f64)> = (0..m)
            .filter(|&e| d[e] > 0.0)
            .filter_map(|e| {
                let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
                let ev = sub(b, a);
                let el = len(ev);
                (el > 1e-9).then(|| {
                    let nrm = if ccw { [ev[1] / el, -ev[0] / el] } else { [-ev[1] / el, ev[0] / el] };
                    (nrm, dot(a, nrm) - d[e])
                })
            })
            .collect();
        let key = if group[i] != usize::MAX { group[i] } else { i };
        // Too steep to build a street front on: the block is a terraced garden.
        let w = if matches!(w, Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Docks | Ward::Temple) && block.len() >= 3 && site.grade(&block) > 0.18 {
            Ward::Park
        } else {
            w
        };
        let dp = District::roll(w, hash2(base_seed, key as i64, 0xd15));
        let mut prng = Pcg32::new(hash2(base_seed, i as i64, 0x107), 47);
        if !matches!(w, Ward::Plaza | Ward::Farm | Ward::Park) {
            l.blocks.push(block.clone());
        }
        match w {
            Ward::Plaza => {
                // A monument, pushed toward the square's longest side (watabou's market).
                let mr = roll(i, 40);
                if mr < 0.9 {
                    let mb = block.len();
                    let e = (0..mb).max_by(|&x, &y| dist(block[x], block[(x + 1) % mb]).total_cmp(&dist(block[y], block[(y + 1) % mb]))).unwrap_or(0);
                    let (a, b) = (block[e], block[(e + 1) % mb]);
                    let cen = centroid(&block);
                    let at = lerp(cen, lerp(a, b, 0.5), 0.2 + 0.4 * roll(i, 41));
                    let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
                    let size = 13.0 + 13.0 * roll(i, 42);
                    let plinth = if mr < 0.6 {
                        rect(at, u, size, 13.0 + 13.0 * roll(i, 43))
                    } else {
                        let o = circle(at, 0.6 * size, 8);
                        o.iter().map(|p| add(at, rotate(sub(*p, at), libm::atan2(u[1], u[0])))).collect()
                    };
                    l.monuments.push(plinth);
                }
                l.plazas.push(block)
            }
            Ward::Farm => farm(site, &block, &mut prng, l),
            Ward::Castle => {
                // Curtain wall with towers on every corner but the gate, which faces the
                // centre (the plaza, if the citadel backs onto it).
                let curtain = inset(&block, &vec![6.0; block.len()]);
                if curtain.len() >= 3 {
                    let n = curtain.len();
                    let gate_e = (0..n).min_by(|&x, &y| len(lerp(curtain[x], curtain[(x + 1) % n], 0.5)).total_cmp(&len(lerp(curtain[y], curtain[(y + 1) % n], 0.5)))).unwrap_or(0);
                    // Walk the curtain from the far end of the gate edge back to its start,
                    // with a gatehouse in the middle of the gate edge.
                    let gate_mid = lerp(curtain[gate_e], curtain[(gate_e + 1) % n], 0.5);
                    let mut ring: Vec<(P, bool)> = vec![(gate_mid, true)];
                    for k in 1..=n {
                        ring.push((curtain[(gate_e + k) % n], false));
                    }
                    ring.push((gate_mid, true));
                    let (pieces, towers, gate_towers) = wall_pieces(&ring, false);
                    l.walls.extend(pieces);
                    l.towers.extend(towers);
                    l.gate_towers.extend(gate_towers);
                    let cc = centroid(&curtain);
                    l.castles.push((cc, curtain.iter().map(|p| dist(*p, cc)).fold(0.0, f64::max)));
                }
                let yard = inset(&block, &vec![22.0; block.len()]);
                if yard.len() >= 3 {
                    // The keep: the largest rectangle that fits well inside the yard.
                    let zone = inset(&block, &vec![50.0; block.len()]);
                    let keep = if zone.len() >= 3 {
                        let o = obb(&zone);
                        let c = centroid(&zone);
                        let mut f = 0.75;
                        let mut k = rect(c, o.axis, o.long * f, o.short * f);
                        for _ in 0..8 {
                            if k.iter().all(|p| contains(&zone, *p)) {
                                break;
                            }
                            f *= 0.85;
                            k = rect(c, o.axis, o.long * f, o.short * f);
                        }
                        k.iter().all(|p| contains(&zone, *p)).then_some(k)
                    } else {
                        None
                    };
                    // Buildings line the inside of the curtain, clear of the keep.
                    let first = l.buildings.len();
                    let mut inner_lines = edge_lines(&yard);
                    build_block(site, &yard, &mut inner_lines, w, &dp, &mut prng, l, None);
                    if let Some(keep) = keep {
                        let mut kept: Vec<Building> = l.buildings.drain(first..).collect();
                        kept.retain(|b| !overlaps(&b.poly, &keep));
                        l.buildings.extend(kept);
                        if site.buildable(&keep) {
                            let pad = site.pad(&keep);
                            l.buildings.push(Building { poly: keep, ward: w, func: None, residential: 4, name: None, floors: 4, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None });
                        }
                    }
                }
            }
            Ward::Slum if i >= n_inner => {
                // Sprawl thins out away from the city: lots are kept with a probability that
                // falls with distance from the corners this patch shares with the city.
                let anchors: Vec<P> = (0..m)
                    .filter(|&e| {
                        let nb = c.labels[e];
                        nb >= 0 && usable(nb as usize)
                    })
                    .flat_map(|e| [c.pts[e], c.pts[(e + 1) % m]])
                    .collect();
                let reach = c.pts.iter().map(|p| dist(*p, centroid(&c.pts))).fold(0.0, f64::max);
                build_block(site, &block, &mut streets, w, &dp, &mut prng, l, Some((&anchors, reach)))
            }
            _ => build_block(site, &block, &mut streets, w, &dp, &mut prng, l, None),
        }
    }
    // Roads into the city and the ground before each gate stay open: the sprawl outside the
    // walls grows up to them, not over them.
    let mut keep_clear: Vec<(P, P, f64)> = l.roads.iter().filter(|r| r.1 != 3).flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * w + 3.0))).collect();
    let within = OUTSKIRTS * r * 1.1;
    let c = site.center;
    for (ri, k) in site.t0.roads.segments_near([c[0] - within, c[1] - within, c[0] + within, c[1] + within], 0.0) {
        let rc = &site.t0.roads.roads[ri as usize];
        let k = k as usize;
        let m = ((rc.s[k + 1] - rc.s[k]) / 25.0).ceil().clamp(1.0, 400.0) as usize;
        let pts: Vec<P> = (0..=m).map(|j| sub(rc.eval(k, j as f64 / m as f64, 5.0, site.t0.cell_ft).p, c)).collect();
        for s in pts.windows(2) {
            if len(s[0]) < within {
                keep_clear.push((s[0], s[1], 0.5 * rc.class.width_ft() + 4.0));
            }
        }
    }
    const CLEAR_CELL: f64 = 80.0;
    let mut clear_grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
    for (i, &(a, b, h)) in keep_clear.iter().enumerate() {
        let (x0, x1) = (crate::core::floor((a[0].min(b[0]) - h) / CLEAR_CELL) as i64, crate::core::floor((a[0].max(b[0]) + h) / CLEAR_CELL) as i64);
        let (y0, y1) = (crate::core::floor((a[1].min(b[1]) - h) / CLEAR_CELL) as i64, crate::core::floor((a[1].max(b[1]) + h) / CLEAR_CELL) as i64);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                clear_grid.entry((cx, cy)).or_default().push(i as u32);
            }
        }
    }
    let gates = l.gates.clone();
    l.buildings.retain(|b| {
        if gates.iter().any(|g| b.poly.iter().any(|p| dist(*p, *g) < 40.0) || contains(&b.poly, *g)) {
            return false;
        }
        let (x0, y0, x1, y1) = b.poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        for cy in crate::core::floor(y0 / CLEAR_CELL) as i64..=crate::core::floor(y1 / CLEAR_CELL) as i64 {
            for cx in crate::core::floor(x0 / CLEAR_CELL) as i64..=crate::core::floor(x1 / CLEAR_CELL) as i64 {
                for &i in clear_grid.get(&(cx, cy)).into_iter().flatten() {
                    let (a, bb, h) = keep_clear[i as usize];
                    if poly_seg_dist(&b.poly, a, bb) < h {
                        return false;
                    }
                }
            }
        }
        true
    });
    // Paved streets stop at the water's edge unless a bridge carries them.
    let bridges = l.bridges.clone();
    let on_water = |p: P| site.wet(p) && !bridges.iter().any(|b| contains(b, p));
    let mut clipped: Vec<(Vec<P>, u8, f64)> = Vec::new();
    for (pts, class, w) in std::mem::take(&mut l.roads) {
        if class < 3 {
            clipped.push((pts, class, w));
            continue;
        }
        let mut run: Vec<P> = Vec::new();
        for seg in pts.windows(2) {
            let n = (dist(seg[0], seg[1]) / 5.0).ceil().max(1.0) as usize;
            for j in 0..=n {
                let q = lerp(seg[0], seg[1], j as f64 / n as f64);
                if on_water(q) {
                    if run.len() >= 2 {
                        clipped.push((std::mem::take(&mut run), class, w));
                    }
                    run.clear();
                } else if run.last().is_none_or(|r| dist(*r, q) > 1e-6) {
                    // Keep only corners and run ends: interior samples on a straight segment
                    // add nothing.
                    if j == 0 || j == n || run.is_empty() {
                        run.push(q);
                    } else if j + 1 <= n && on_water(lerp(seg[0], seg[1], (j + 1) as f64 / n as f64)) {
                        run.push(q);
                    }
                }
            }
        }
        if run.len() >= 2 {
            clipped.push((run, class, w));
        }
    }
    l.roads = clipped;
    let crossings: Vec<P> = bridge_streets.iter().map(|(pts, _)| pts[1]).collect();
    let edge_w: Vec<Vec<f64>> = (0..total)
        .map(|i| {
            let c = &cells[i];
            let m = c.pts.len();
            (0..m)
                .map(|e| {
                    let (a, b) = (c.pts[e], c.pts[(e + 1) % m]);
                    let nb = c.labels[e];
                    let (ka, kb) = (qkey(a), qkey(b));
                    if site.near_river(lerp(a, b, 0.5), 2.0) || (walled && nb >= 0 && (i < n_inner) != ((nb as usize) < n_inner)) {
                        0.0
                    } else if main_edges.contains(&(ka.min(kb), ka.max(kb))) {
                        0.9
                    } else {
                        1.0
                    }
                })
                .collect()
        })
        .collect();
    let landing: Vec<bool> = (0..total)
        .map(|i| {
            let c = &cells[i];
            let m = c.pts.len();
            (0..m).any(|e| (c.labels[e] >= 0 && wet[c.labels[e] as usize]) || site.near_river(lerp(c.pts[e], c.pts[(e + 1) % m], 0.5), 2.0))
        })
        .collect();
    l.quarters = quarters(s, &QuarterInput { cells: &cells, ward: &ward, n_inner, plaza, castle: castle_face, gates: &l.gates, crossings: &crossings, edge_w: &edge_w, landing: &landing });
}

struct QuarterInput<'a> {
    cells: &'a [Labeled],
    ward: &'a [Option<Ward>],
    n_inner: usize,
    plaza: Option<usize>,
    castle: Option<usize>,
    gates: &'a [P],
    /// Bridge crossing points (river vertices).
    crossings: &'a [P],
    /// Per patch edge: how readily a district grows across it (0 wall or river, 0.9 main
    /// street, 1 otherwise).
    edge_w: &'a [Vec<f64>],
    /// Patches on the water.
    landing: &'a [bool],
}

/// Named districts, after watabou's DistrictBuilder: seeds at the plaza, the castle, parks,
/// gates, bridges, the riverbank and the docks, plus as many random patches; a random
/// √(patches) of them become districts, which grow over neighbouring patches (gate, bridge
/// and castle districts slowly; never across the wall or the river; docks and parks only
/// over their own kind; not between the waterfront and inland). Leftover patches seed more.
fn quarters(s: &Settlement, q: &QuarterInput) -> Vec<Quarter> {
    use QuarterKind as K;
    let total = q.cells.len();
    let in_city: Vec<bool> = (0..total).map(|i| q.cells[i].pts.len() >= 3 && q.ward[i].is_some_and(|w| w != Ward::Farm)).collect();
    let city: Vec<usize> = (0..total).filter(|&i| in_city[i]).collect();
    if city.is_empty() {
        return Vec::new();
    }
    let mut rng = Pcg32::new(hash2(s.seed, 0xd157, 2), 59);
    let kind_of = |i: usize| match q.ward[i] {
        Some(Ward::Castle) => K::Castle,
        Some(Ward::Park) => K::Park,
        Some(Ward::Docks) if q.landing[i] => K::Docks,
        _ if i < q.n_inner => K::Regular,
        _ => K::Sprawl,
    };
    let at = |p: P| -> Vec<usize> { city.iter().copied().filter(|&i| q.cells[i].pts.iter().any(|v| dist(*v, p) < 2.0)).collect() };

    // Seeds: anchors first (the centre and castle always make it), then random patches.
    let mut fixed: Vec<(Vec<usize>, K)> = Vec::new();
    if let Some(c) = q.castle.filter(|&c| in_city[c]) {
        fixed.push((vec![c], K::Castle));
    }
    let centre = q.plaza.filter(|&p| in_city[p]).unwrap_or_else(|| *city.iter().min_by(|&&a, &&b| len(centroid(&q.cells[a].pts)).total_cmp(&len(centroid(&q.cells[b].pts)))).unwrap());
    fixed.push((vec![centre], K::Center));
    let mut seeds: Vec<(Vec<usize>, K)> = Vec::new();
    for &i in &city {
        if q.ward[i] == Some(Ward::Park) {
            seeds.push((vec![i], K::Park));
        }
    }
    for &g in q.gates {
        let ps = at(g);
        if !ps.is_empty() {
            seeds.push((ps, K::Gate));
        }
    }
    for &b in q.crossings {
        let ps = at(b);
        if !ps.is_empty() {
            seeds.push((ps, K::Bridge));
        }
    }
    let banks: Vec<usize> = city.iter().copied().filter(|&i| q.landing[i] && kind_of(i) != K::Docks && q.edge_w[i].contains(&0.0)).collect();
    for _ in 0..2 {
        if !banks.is_empty() {
            seeds.push((vec![banks[rng.below(banks.len() as u32) as usize]], K::Bank));
        }
    }
    if let Some(&i) = city.iter().find(|&&i| kind_of(i) == K::Docks) {
        seeds.push((vec![i], K::Docks));
    }
    for _ in 0..seeds.len() {
        let i = city[rng.below(city.len() as u32) as usize];
        seeds.push((vec![i], kind_of(i)));
    }
    let target = crate::core::sqrt(city.len() as f64) as usize;
    for k in (1..seeds.len()).rev() {
        seeds.swap(k, rng.below(k as u32 + 1) as usize);
    }
    seeds.truncate(target.saturating_sub(fixed.len()));
    let mut selected = fixed;
    selected.extend(seeds);
    while selected.len() < target {
        let i = city[rng.below(city.len() as u32) as usize];
        selected.push((vec![i], kind_of(i)));
    }
    // One docks district (the others' patches join it or their neighbours).
    let mut seen_docks = false;
    selected.retain(|(_, k)| *k != K::Docks || !std::mem::replace(&mut seen_docks, true));

    let mut owner = vec![usize::MAX; total];
    let mut qs: Vec<(K, Vec<usize>)> = Vec::new();
    for (ps, k) in selected {
        let free: Vec<usize> = ps.iter().copied().filter(|&i| owner[i] == usize::MAX).collect();
        // A multi-patch anchor (gate, bridge) needs all its patches.
        if free.is_empty() || free.len() < ps.len() {
            continue;
        }
        for &i in &free {
            owner[i] = qs.len();
        }
        qs.push((k, free));
    }
    let grow = |id: usize, qs: &mut Vec<(K, Vec<usize>)>, owner: &mut Vec<usize>, rng: &mut Pcg32| -> bool {
        let k = qs[id].0;
        let rate = match k {
            K::Castle | K::Bridge | K::Gate => 0.1,
            K::Bank => 0.5,
            _ => 1.0,
        };
        if rng.next_f64() < 1.0 - rate {
            return true;
        }
        let mut cands: Vec<usize> = Vec::new();
        for &i in &qs[id].1 {
            for (e, &nb) in q.cells[i].labels.iter().enumerate() {
                if nb < 0 || !in_city[nb as usize] || owner[nb as usize] != usize::MAX {
                    continue;
                }
                let j = nb as usize;
                let same = match k {
                    K::Docks => kind_of(j) == K::Docks,
                    K::Park => q.ward[j] == Some(Ward::Park),
                    _ => q.landing[i] == q.landing[j],
                };
                if same && rng.next_f64() < q.edge_w[i][e] {
                    cands.push(j);
                }
            }
        }
        if cands.is_empty() {
            return false;
        }
        let j = cands[rng.below(cands.len() as u32) as usize];
        owner[j] = id;
        qs[id].1.push(j);
        true
    };
    let mut growers: Vec<usize> = (0..qs.len()).collect();
    while !growers.is_empty() && city.iter().any(|&i| owner[i] == usize::MAX) {
        let mut order = growers.clone();
        for k in (1..order.len()).rev() {
            order.swap(k, rng.below(k as u32 + 1) as usize);
        }
        for g in order {
            if !grow(g, &mut qs, &mut owner, &mut rng) {
                growers.retain(|&x| x != g);
            }
        }
    }
    loop {
        let free: Vec<usize> = city.iter().copied().filter(|&i| owner[i] == usize::MAX).collect();
        if free.is_empty() {
            break;
        }
        let i = free[rng.below(free.len() as u32) as usize];
        owner[i] = qs.len();
        qs.push((kind_of(i), vec![i]));
        let id = qs.len() - 1;
        while grow(id, &mut qs, &mut owner, &mut rng) {}
    }

    // Names (watabou's grammar) and label curves.
    let mut namer = Namer::new(hash2(s.seed, 0xd157, 3));
    let culture = s.culture as usize;
    let mut used: std::collections::BTreeSet<String> = Default::default();
    let mut out = Vec::new();
    for (k, members) in qs {
        let patches: Vec<Vec<P>> = members.iter().map(|&i| q.cells[i].pts.clone()).collect();
        let (label, mid) = label_curve(&patches);
        let size = members.len() + rng.below(3) as usize;
        let noun = if size <= 2 {
            "Quarter"
        } else if size < 6 {
            "Ward"
        } else if size < 12 {
            "District"
        } else {
            "Town"
        };
        let dir = [(1.0, 0.0, "East"), (-1.0, 0.0, "West"), (0.0, 1.0, "South"), (0.0, -1.0, "North")]
            .iter()
            .max_by(|a, b| (a.0 * mid[0] + a.1 * mid[1]).total_cmp(&(b.0 * mid[0] + b.1 * mid[1])))
            .unwrap()
            .2;
        let either = |rng: &mut Pcg32, a: &str, b: &str| if rng.next_f64() < 0.5 { a.to_string() } else { b.to_string() };
        let base = match k {
            K::Center => format!("{} {noun}", either(&mut rng, "Market", "Old")),
            K::Castle => format!("{} {noun}", either(&mut rng, "Crown", "Keep")),
            K::Docks => format!("{} {noun}", either(&mut rng, "Harbor", "Wharf")),
            K::Bridge => format!("Bridge {noun}"),
            K::Gate => format!("{dir} Gate"),
            K::Bank => format!("Riverbank {noun}"),
            K::Park => format!("Garden {noun}"),
            K::Sprawl => format!("Outer {noun}"),
            K::Regular => format!("{} {noun}", namer.word(culture, 1, 2)),
        };
        let mut name = base.clone();
        if used.contains(&name) {
            name = format!("{dir} {base}");
        }
        while used.contains(&name) {
            name = format!("{} {noun}", namer.word(culture, 1, 2));
        }
        used.insert(name.clone());
        out.push(Quarter { name, kind: k, patches, label });
    }
    out
}

/// A label baseline for a district: along the principal axis of its patch centroids
/// (area-weighted), 70% of its extent, bent toward where the patches lie. Also returns the
/// area-weighted centre.
fn label_curve(patches: &[Vec<P>]) -> (Vec<P>, P) {
    let cs: Vec<(P, f64)> = patches.iter().map(|p| (centroid(p), area(p).abs().max(1.0))).collect();
    let wsum: f64 = cs.iter().map(|c| c.1).sum();
    let c = cs.iter().fold([0.0, 0.0], |acc, (p, w)| add(acc, mul(*p, w / wsum)));
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in patches.iter().flatten() {
        let d = sub(*p, c);
        sxx += d[0] * d[0];
        syy += d[1] * d[1];
        sxy += d[0] * d[1];
    }
    let th = 0.5 * libm::atan2(2.0 * sxy, sxx - syy);
    let mut u = [libm::cos(th), libm::sin(th)];
    if u[0] < 0.0 {
        u = mul(u, -1.0);
    }
    let nrm = [-u[1], u[0]];
    let (t0, t1) = patches.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), p| {
        let t = dot(sub(*p, c), u);
        (a.min(t), b.max(t))
    });
    let (tm, span) = (0.5 * (t0 + t1), t1 - t0);
    let h = 0.25 * span;
    let mut pts: Vec<P> = (0..5)
        .map(|k| {
            let t = tm + (k as f64 / 4.0 - 0.5) * 0.7 * span;
            let (mut num, mut den) = (0.0, 1e-9);
            for (p, w) in &cs {
                let d = sub(*p, c);
                let z = (dot(d, u) - t) / h.max(1.0);
                let g = w * libm::exp(-z * z);
                num += g * dot(d, nrm);
                den += g;
            }
            let off = (num / den).clamp(-0.15 * span, 0.15 * span);
            add(c, add(mul(u, t), mul(nrm, off)))
        })
        .collect();
    // Smooth the bend once so the text flows.
    let raw = pts.clone();
    for k in 1..4 {
        pts[k] = lerp(lerp(raw[k - 1], raw[k + 1], 0.5), raw[k], 0.5);
    }
    (pts, c)
}

/// Distance from a polygon to a segment (0 if they touch or the segment is inside).
fn poly_seg_dist(poly: &[P], a: P, b: P) -> f64 {
    if contains(poly, a) || contains(poly, b) {
        return 0.0;
    }
    let m = poly.len();
    let mut d = f64::MAX;
    for e in 0..m {
        let (c, q) = (poly[e], poly[(e + 1) % m]);
        let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(q, a)));
        let (o3, o4) = (cross(sub(q, c), sub(a, c)), cross(sub(q, c), sub(b, c)));
        if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
            return 0.0;
        }
        d = d.min(seg_dist(c, a, b)).min(seg_dist(a, c, q)).min(seg_dist(b, c, q));
    }
    d
}

/// Block and lot parameters shared by a district (after watabou's per-district rolls).
struct District {
    /// Target lot area (ft²).
    min_sq: f64,
    /// 0.2–1: how irregular alley cuts are allowed to be.
    grid_chaos: f64,
    /// 0.4–1: how much lot and block sizes vary.
    size_chaos: f64,
    /// Lots per block before an alley cuts it (4–14).
    block_size: f64,
}

impl District {
    fn roll(w: Ward, seed: u64) -> District {
        let mut r = Pcg32::new(seed, 53);
        let base = match w {
            Ward::Merchant => 1400.0,
            Ward::Craft => 1600.0,
            Ward::Noble => 4200.0,
            Ward::Slum => 650.0,
            Ward::Docks => 2600.0,
            Ward::Military => 2400.0,
            Ward::Temple => 2200.0,
            Ward::Castle => 2600.0,
            Ward::Park => 9000.0,
            _ => 1100.0,
        };
        let spread = r.next_f64();
        let mut d = District {
            min_sq: base * (0.75 + 0.9 * spread),
            grid_chaos: 0.2 + 0.8 * r.next_f64(),
            size_chaos: 0.4 + 0.6 * r.next_f64(),
            block_size: 4.0 + 10.0 * r.next_f64(),
        };
        if w == Ward::Slum {
            d.grid_chaos *= 0.5;
            d.block_size *= 2.0;
        }
        d
    }
}

/// Split a wall line at its gates: a 22 ft passage through the wall at each gate corner
/// between two gate towers, towers on every other corner, and more towers along long runs.
/// Input: corners with a gate flag; `closed` joins the last corner back to the first.
fn wall_pieces(corners: &[(P, bool)], closed: bool) -> (Vec<Vec<P>>, Vec<P>, Vec<P>) {
    const TOWER_SPACING: f64 = 160.0;
    // Gate towers (15 ft radius) stand 26 ft either side of the gate: a 22 ft passage.
    const GATE_HALF: f64 = 26.0;
    let mut pts: Vec<(P, bool)> = corners.to_vec();
    if closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    let (mut pieces, mut towers, mut gate_towers) = (Vec::new(), Vec::new(), Vec::new());
    let mut cur: Vec<P> = Vec::new();
    for k in 0..pts.len() {
        let (p, gate) = pts[k];
        if gate {
            let prev = if k > 0 { Some(pts[k - 1].0) } else { None };
            let next = if k + 1 < pts.len() { Some(pts[k + 1].0) } else { None };
            if let Some(q) = prev {
                let e = add(p, mul(sub(q, p), GATE_HALF / dist(p, q).max(1e-9)));
                cur.push(e);
                gate_towers.push(e);
            }
            if cur.len() >= 2 {
                pieces.push(std::mem::take(&mut cur));
            }
            cur.clear();
            if let Some(q) = next {
                let e = add(p, mul(sub(q, p), GATE_HALF / dist(p, q).max(1e-9)));
                cur.push(e);
                gate_towers.push(e);
            }
            continue;
        }
        // Towers along a long run since the last corner.
        if let Some(&last) = cur.last() {
            let d = dist(last, p);
            let extra = (d / TOWER_SPACING).floor() as usize;
            for j in 1..=extra {
                towers.push(lerp(last, p, j as f64 / (extra + 1) as f64));
            }
        }
        cur.push(p);
        if !(closed && k + 1 == pts.len()) {
            towers.push(p);
        }
    }
    if cur.len() >= 2 {
        pieces.push(cur);
    }
    (pieces, towers, gate_towers)
}

/// Two convex polygons overlap (a corner of one inside the other).
fn overlaps(a: &[P], b: &[P]) -> bool {
    a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p))
}

fn rotate(v: P, ang: f64) -> P {
    let (c, s) = (libm::cos(ang), libm::sin(ang));
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

fn smoothstep_f(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0).max(1e-9)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Every edge of a convex polygon as a street line (outward normal, offset).
fn edge_lines(poly: &[P]) -> Vec<(P, f64)> {
    let m = poly.len();
    let ccw = area(poly) > 0.0;
    (0..m)
        .filter_map(|e| {
            let (a, b) = (poly[e], poly[(e + 1) % m]);
            let ev = sub(b, a);
            let el = len(ev);
            (el > 1e-9).then(|| {
                let nrm = if ccw { [ev[1] / el, -ev[0] / el] } else { [-ev[1] / el, ev[0] / el] };
                (nrm, dot(a, nrm))
            })
        })
        .collect()
}

/// Dijkstra over the street graph from `start` to any of `targets`.
/// Street graph: quantized vertex → (neighbour, cost).
type Graph = FastMap<(i64, i64), Vec<((i64, i64), f64)>>;

fn shortest(adj: &Graph, start: (i64, i64), targets: &std::collections::BTreeSet<(i64, i64)>) -> Option<Vec<(i64, i64)>> {
    #[derive(PartialEq)]
    struct N(f64, (i64, i64));
    impl Eq for N {}
    impl Ord for N {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
        }
    }
    impl PartialOrd for N {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    if !adj.contains_key(&start) {
        return None;
    }
    let mut g: FastMap<(i64, i64), f64> = FastMap::default();
    let mut came: FastMap<(i64, i64), (i64, i64)> = FastMap::default();
    let mut heap = BinaryHeap::new();
    g.insert(start, 0.0);
    heap.push(N(0.0, start));
    while let Some(N(d, k)) = heap.pop() {
        if targets.contains(&k) {
            let mut path = vec![k];
            let mut cur = k;
            while let Some(&p) = came.get(&cur) {
                path.push(p);
                cur = p;
            }
            path.reverse();
            return Some(path);
        }
        if d > *g.get(&k).unwrap_or(&f64::MAX) {
            continue;
        }
        let mut nbs = adj[&k].clone();
        nbs.sort_by_key(|a| a.0);
        for (nb, w) in nbs {
            let nd = d + w;
            if nd < *g.get(&nb).unwrap_or(&f64::MAX) {
                g.insert(nb, nd);
                came.insert(nb, k);
                heap.push(N(nd, nb));
            }
        }
    }
    None
}

/// Alleys, lots and buildings for one block (after watabou's Bisector). Big blocks are cut
/// by 16 ft alleys into blocks of about `block_size` lots; blocks are cut into lots square to
/// their street frontage; lots without frontage stay open (yards and gardens); buildings
/// fill their lots wall to wall, some standing back from the street.
#[allow(clippy::too_many_arguments)]
fn build_block(site: &Site, block: &[P], streets: &mut Vec<(P, f64)>, ward: Ward, dp: &District, rng: &mut Pcg32, l: &mut Layout, thin: Option<(&[P], f64)>) {
    let mut parts: Vec<Vec<P>> = Vec::new();
    let alley_area = dp.min_sq * dp.block_size;
    if area(block).abs() > alley_area * libm::pow(2.0, dp.size_chaos * (2.0 * rng.next_f64() - 1.0)) {
        let mut chords: Vec<(P, P)> = Vec::new();
        bisect(block, alley_area, (16.0 * dp.grid_chaos).max(2.0), ALLEY_HALF_FT, rng, &mut parts, streets, &mut chords, 0);
        // Alleys in towns are paved like their streets (the sprawl outside keeps dirt lanes).
        if thin.is_none() && l.tier >= Tier::Town {
            for (a, b) in chords {
                l.roads.push((vec![a, b], 3, 2.0 * ALLEY_HALF_FT + 1.0));
            }
        }
    } else {
        parts.push(block.to_vec());
    }
    let lot_variance = (4.0 * dp.size_chaos).max(1.2);
    for part in parts {
        let mut lots: Vec<Vec<P>> = Vec::new();
        bisect(&part, dp.min_sq, lot_variance, 0.0, rng, &mut lots, &mut Vec::new(), &mut Vec::new(), 0);
        for lot in lots {
            let lot = clean(&blunt(&clean(&lot), 60.0, 10.0));
            // No triangles, slivers or scraps: those corners stay open ground.
            if lot.len() < 4 {
                continue;
            }
            let la = area(&lot).abs();
            let o = obb(&lot);
            if la < dp.min_sq / 4.0 || o.short < 16.0 || la < 0.62 * o.long * o.short {
                continue;
            }
            // Frontage: a street or alley line one of the lot's edges lies on.
            let Some(&(n, c)) = streets.iter().find(|(n, c)| fronts(&lot, *n, *c)) else { continue };
            if ward == Ward::Park && rng.next_f64() > 0.15 {
                continue;
            }
            if let Some((anchors, reach)) = thin {
                let c = centroid(&lot);
                let d = anchors.iter().map(|a| dist(*a, c)).fold(f64::MAX, f64::min);
                let keep = if anchors.is_empty() { 0.35 } else { 1.0 - smoothstep_f(0.0, 0.9 * reach, d) };
                if rng.next_f64() > keep {
                    continue;
                }
            }
            // Irregular street fronts: some buildings stand back from the street line.
            let indent = rng.next_f64() * (crate::core::sqrt(la) / 3.0).min(16.0);
            let poly = if indent > 6.5 { clean(&clip_plain(&lot, n, c - indent)) } else { lot };
            if poly.len() < 3 || area(&poly).abs() < 150.0 || !site.buildable(&poly) {
                continue;
            }
            let pad = site.pad(&poly);
            l.buildings.push(Building { poly, ward, func: None, residential: 0, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None });
        }
    }
}

/// Cut off corners sharper than `min_deg` with a chord square to the corner's bisector,
/// `depth` ft in from the tip (keeps the polygon convex; houses don't come to a point).
fn blunt(poly: &[P], min_deg: f64, depth: f64) -> Vec<P> {
    let m = poly.len();
    if m < 3 {
        return poly.to_vec();
    }
    let cos_min = libm::cos(min_deg.to_radians());
    let mut out = poly.to_vec();
    for i in 0..m {
        let (p, a, b) = (poly[i], poly[(i + m - 1) % m], poly[(i + 1) % m]);
        let (u, v) = (sub(a, p), sub(b, p));
        let (lu, lv) = (len(u), len(v));
        if lu < 1e-9 || lv < 1e-9 {
            continue;
        }
        let (u, v) = (mul(u, 1.0 / lu), mul(v, 1.0 / lv));
        if dot(u, v) <= cos_min {
            continue;
        }
        // Inward bisector; keep the part at least `depth` along it from the tip.
        let bis = add(u, v);
        let bl = len(bis).max(1e-9);
        let n = mul(bis, -1.0 / bl);
        out = clip_plain(&out, n, dot(p, n) - depth);
        if out.len() < 3 {
            return out;
        }
    }
    out
}

/// Half the width of an alley cut through a block (ft).
const ALLEY_HALF_FT: f64 = 8.0;

/// A lot edge of useful length lies on the line (normal `n`, offset `c`).
fn fronts(lot: &[P], n: P, c: f64) -> bool {
    let m = lot.len();
    (0..m).any(|i| {
        let (a, b) = (lot[i], lot[(i + 1) % m]);
        (dot(a, n) - c).abs() < 0.5 && (dot(b, n) - c).abs() < 0.5 && dist(a, b) > 6.0
    })
}

/// Recursive straight-cut bisection of a convex polygon (watabou's Bisector, straight cuts
/// only so pieces stay convex). The cut crosses the long axis of the minimum bounding box
/// near the middle; it runs square to the crossed edge that is most parallel to that axis,
/// so pieces meet their frontage at right angles. Cuts that split too unevenly are retried
/// in frames rotated by 36°. `gap` > 0 leaves a lane of width 2·gap along each cut, whose
/// sides are recorded in `cuts` as street lines.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn bisect(poly: &[P], min_area: f64, variance: f64, gap: f64, rng: &mut Pcg32, out: &mut Vec<Vec<P>>, cuts: &mut Vec<(P, f64)>, chords: &mut Vec<(P, P)>, depth: u32) {
    let a = area(poly).abs();
    let avg4 = (rng.next_f64() + rng.next_f64() + rng.next_f64() + rng.next_f64()) / 4.0;
    if depth > 18 || a < min_area * libm::pow(variance, (avg4 * 2.0 - 1.0).abs()) {
        out.push(poly.to_vec());
        return;
    }
    let frame = obb(poly).axis;
    let c = centroid(poly);
    let m = poly.len();
    for attempt in 0..10 {
        let ang = attempt as f64 * std::f64::consts::PI / 5.0;
        let (ca, sa) = (libm::cos(ang), libm::sin(ang));
        let axis = [frame[0] * ca - frame[1] * sa, frame[0] * sa + frame[1] * ca];
        let (lo, hi) = extent(poly, axis);
        if hi - lo < 1e-6 {
            continue;
        }
        let avg3 = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0;
        let cf = (dot(c, axis) - lo) / (hi - lo);
        let pos = lo + (hi - lo) * (cf + avg3) / 2.0;
        // The crossed edge most parallel to the axis: the cut leaves it at a right angle.
        let mut best: Option<(f64, P, P)> = None;
        for i in 0..m {
            let (p, q) = (poly[i], poly[(i + 1) % m]);
            let (dp_, dq) = (dot(p, axis) - pos, dot(q, axis) - pos);
            if (dp_ <= 0.0) == (dq <= 0.0) {
                continue;
            }
            let e = sub(q, p);
            let el = len(e);
            if el < 1e-9 {
                continue;
            }
            let u = mul(e, 1.0 / el);
            let cs = dot(u, axis).abs();
            if best.is_none_or(|b| cs > b.0) {
                best = Some((cs, lerp(p, q, dp_ / (dp_ - dq)), u));
            }
        }
        let Some((_, x, nrm)) = best else { continue };
        let cc = dot(x, nrm);
        let p1 = clean(&clip_plain(poly, nrm, cc - gap));
        let p2 = clean(&clip_plain(poly, mul(nrm, -1.0), -(cc + gap)));
        if p1.len() < 3 || p2.len() < 3 {
            continue;
        }
        let (a1, a2) = (area(&p1).abs(), area(&p2).abs());
        if a1.max(a2) > 2.0 * variance * a1.min(a2).max(1e-9) {
            continue;
        }
        if gap > 0.0 {
            cuts.push((nrm, cc - gap));
            cuts.push((mul(nrm, -1.0), -(cc + gap)));
            // The alley's centre line across this piece.
            let dir = [-nrm[1], nrm[0]];
            let m = poly.len();
            let hits: Vec<P> = (0..m)
                .filter_map(|k| {
                    let (a, b) = (poly[k], poly[(k + 1) % m]);
                    let (da, db) = (dot(a, nrm) - cc, dot(b, nrm) - cc);
                    (da * db < 0.0).then(|| lerp(a, b, da / (da - db)))
                })
                .collect();
            let lo = hits.iter().min_by(|a, b| dot(**a, dir).total_cmp(&dot(**b, dir)));
            let hi = hits.iter().max_by(|a, b| dot(**a, dir).total_cmp(&dot(**b, dir)));
            if let (Some(&a), Some(&b)) = (lo, hi) {
                chords.push((a, b));
            }
        }
        bisect(&p1, min_area, variance, gap, rng, out, cuts, chords, depth + 1);
        bisect(&p2, min_area, variance, gap, rng, out, cuts, chords, depth + 1);
        return;
    }
    out.push(poly.to_vec());
}

/// Farmland (after watabou's fields): the patch is split into plots, square to the long
/// side of each piece's bounding box with some skewed cuts, leaving hedged lanes between
/// plots; corners are chamfered; one plot in five gets a farmhouse on its longest side.
fn farm(site: &Site, block: &[P], rng: &mut Pcg32, l: &mut Layout) {
    let spread = rng.next_f64();
    let mut plots: Vec<Vec<P>> = Vec::new();
    split_field(block, 67_600.0 * (1.0 + spread), rng, &mut plots, 0);
    for plot in plots {
        let f = clean(&chamfer(&plot, 17.0));
        if f.len() < 3 || area(&f).abs() < 2_000.0 || site.wet(centroid(&f)) || !site.dry_outline(&f, 8.0) || site.grade(&f) > 0.08 {
            continue;
        }
        if rng.next_f64() < 0.2 {
            let m = f.len();
            let e = (0..m).max_by(|&x, &y| dist(f[x], f[(x + 1) % m]).total_cmp(&dist(f[y], f[(y + 1) % m]))).unwrap_or(0);
            let (a, b) = (f[e], f[(e + 1) % m]);
            let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
            let mut inward = [-u[1], u[0]];
            if dot(inward, sub(centroid(&f), a)) < 0.0 {
                inward = mul(inward, -1.0);
            }
            let (w, dp) = (rng.range(52.0, 65.0), rng.range(26.0, 39.0));
            let house = rect(add(add(a, mul(u, 0.5 * w + 6.0)), mul(inward, 0.5 * dp + 6.0)), u, w, dp);
            if house.iter().all(|p| contains(&f, *p)) && site.buildable(&house) {
                let pad = site.pad(&house);
                l.buildings.push(Building { poly: house, ward: Ward::Farm, func: None, residential: 5, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None });
            }
        }
        l.fields.push(f);
    }
}

fn split_field(poly: &[P], max_area: f64, rng: &mut Pcg32, out: &mut Vec<Vec<P>>, depth: u32) {
    if area(poly).abs() <= max_area || depth > 10 {
        out.push(poly.to_vec());
        return;
    }
    let axis = obb(poly).axis;
    let (lo, hi) = extent(poly, axis);
    let cut = lo + (hi - lo) * (0.5 + rng.range(-0.2, 0.2));
    let mut n = axis;
    if rng.next_f64() < 0.5 {
        let t = rng.range(-std::f64::consts::FRAC_PI_8, std::f64::consts::FRAC_PI_8);
        let (c, s) = (libm::cos(t), libm::sin(t));
        n = [axis[0] * c - axis[1] * s, axis[0] * s + axis[1] * c];
    }
    let cen = centroid(poly);
    let at = add(cen, mul(axis, cut - dot(cen, axis)));
    let cc = dot(at, n);
    const LANE_HALF: f64 = 10.0;
    for half in [clean(&clip_plain(poly, n, cc - LANE_HALF)), clean(&clip_plain(poly, mul(n, -1.0), -(cc + LANE_HALF)))] {
        if half.len() >= 3 && area(&half).abs() > 1.0 {
            split_field(&half, max_area, rng, out, depth + 1);
        }
    }
}

/// Trim every corner: a chord between points `t` ft along both edges from the corner.
fn chamfer(poly: &[P], t: f64) -> Vec<P> {
    let m = poly.len();
    let mut out = poly.to_vec();
    for i in 0..m {
        let (p, a, b) = (poly[i], poly[(i + m - 1) % m], poly[(i + 1) % m]);
        let (la, lb) = (dist(p, a), dist(p, b));
        if la < 1e-6 || lb < 1e-6 {
            continue;
        }
        let tt = t.min(0.3 * la.min(lb));
        let (u, v) = (mul(sub(a, p), 1.0 / la), mul(sub(b, p), 1.0 / lb));
        let bis = add(u, v);
        let bl = len(bis);
        if bl < 1e-6 {
            continue;
        }
        let n = mul(bis, -1.0 / bl);
        out = clip_plain(&out, n, dot(add(p, mul(u, tt)), n));
        if out.len() < 3 {
            return out;
        }
    }
    out
}

/// Rectangle centred at `c`, long side `w` along `dir`, depth `d`.
pub(crate) fn rect(c: P, dir: P, w: f64, d: f64) -> Vec<P> {
    let dl = len(dir).max(1e-9);
    let u = mul(dir, 1.0 / dl);
    let v = [-u[1], u[0]];
    let (hu, hv) = (mul(u, 0.5 * w), mul(v, 0.5 * d));
    vec![sub(sub(c, hu), hv), sub(add(c, hu), hv), add(add(c, hu), hv), add(sub(c, hu), hv)]
}

// ---------------------------------------------------------------------------------------
// Villages.

fn village(site: &Site, s: &Settlement, r: f64, _road_ends: &[(P, u8)], rng: &mut Pcg32, l: &mut Layout) {
    let t0 = site.t0;
    let target = ((s.population as f64 / 5.0).round() as usize).clamp(6, 110);
    let spread = rng.next_f64();
    // The green.
    let green = circle([0.0, 0.0], rng.range(35.0, 55.0), 12);
    if !site.wet([0.0, 0.0]) {
        l.plazas.push(green.clone());
    }
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
                lanes.push((link, 10.0));
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
            lanes.push((lane, 10.0));
            made += 1;
        }
    }
    // Houses along both sides of every lane (watabou's buildAlong): a house of width
    // 22–36 ft, a gap, the next; set back from the lane; denser near the green.
    let clear = |poly: &[P], l: &Layout| {
        let c = centroid(poly);
        len(c) > 60.0 && l.buildings.iter().all(|b| !overlaps(&b.poly, poly) && dist(centroid(&b.poly), c) > 14.0)
    };
    'lanes: for (pts, road_w) in &lanes {
        for side in [-1.0, 1.0] {
            let mut along = rng.range(0.0, 20.0);
            let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
            while along < total {
                if l.buildings.len() >= target {
                    break 'lanes;
                }
                let w = rng.range(22.0, 36.0);
                let (p, u) = point_along(pts, along + 0.5 * w);
                let n = [-u[1], u[0]];
                let density = 1.0 - 0.75 * (len(p) / reach).min(1.0);
                let d = rng.range(16.0, 24.0);
                if rng.next_f64() < density {
                    let back = 0.5 * road_w + 5.0 * (1.0 + spread) * rng.range(0.6, 1.4) + 0.5 * d;
                    let house = rect(add(p, mul(n, side * back)), u, w, d);
                    if clear(&house, l) && site.buildable(&house) {
                        let pad = site.pad(&house);
                        l.buildings.push(Building { poly: house, ward: Ward::Rural, func: None, residential: 1, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None });
                    }
                }
                let avg3 = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0;
                along += w + 22.0 * avg3 * 2.0;
            }
        }
    }
    for (pts, _) in lanes.iter().skip(n_roads) {
        l.streets.push(pts.clone());
    }
    // Last resort for cramped sites: a ring around the green.
    let mut tries = 0;
    while l.buildings.len() < target / 2 && tries < target * 30 {
        tries += 1;
        let rad = rng.range(70.0, r);
        let a = rng.range(0.0, std::f64::consts::TAU);
        let c = [rad * libm::cos(a), rad * libm::sin(a)];
        let house = rect(c, [-libm::sin(a), libm::cos(a)], rng.range(22.0, 34.0), rng.range(16.0, 22.0));
        if clear(&house, l) && site.buildable(&house) {
            let pad = site.pad(&house);
            l.buildings.push(Building { poly: house, ward: Ward::Rural, func: None, residential: 1, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None });
        }
    }
    // Fields: Voronoi cells seeded along the lanes and on a jittered ring, split into plots
    // and claimed outward from the village until there is enough farmland for its people.
    let spacing = 260.0 - 120.0 * spread;
    let mut seeds: Vec<P> = Vec::new();
    for (pts, _) in &lanes {
        let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        let mut a = 0.5 * spacing;
        while a < total {
            let (p, u) = point_along(pts, a);
            let n = [-u[1], u[0]];
            for side in [-1.0, 1.0] {
                seeds.push(add(p, mul(n, side * 0.6 * spacing)));
            }
            a += spacing;
        }
    }
    let ring_n = ((std::f64::consts::TAU * 1.7 * r) / spacing) as usize;
    for k in 0..ring_n {
        let a = std::f64::consts::TAU * (k as f64 + rng.range(-0.3, 0.3)) / ring_n as f64;
        for rad in [1.35 * r, 1.9 * r] {
            seeds.push(polar_pt(rad * rng.range(0.9, 1.1), a));
        }
    }
    seeds.retain(|p| len(*p) > 0.7 * r && len(*p) < 2.1 * r);
    let cells = voronoi(&seeds, &circle([0.0, 0.0], 2.1 * r, 40));
    let mut order: Vec<usize> = (0..cells.len()).filter(|&i| cells[i].pts.len() >= 3).collect();
    order.sort_by(|&a, &b| len(seeds[a]).total_cmp(&len(seeds[b])).then(a.cmp(&b)));
    let want = l.buildings.len() as f64 * 22_000.0 * (0.7 + 0.6 * spread);
    let mut have = 0.0;
    for i in order {
        if have >= want {
            break;
        }
        let cell = inset(&cells[i].pts, &vec![5.0; cells[i].pts.len()]);
        if cell.len() < 3 || site.wet(centroid(&cell)) || l.buildings.iter().any(|b| overlaps(&b.poly, &cell)) {
            continue;
        }
        let mut plots = Vec::new();
        split_field(&cell, 20_000.0 * (1.0 + spread), rng, &mut plots, 0);
        for plot in plots {
            let f = clean(&chamfer(&plot, 10.0));
            if f.len() >= 3 && area(&f).abs() > 1_500.0 && !site.wet(centroid(&f)) && site.dry_outline(&f, 8.0) {
                have += area(&f).abs();
                l.fields.push(f);
            }
        }
    }
}

/// The shore a village faces: rays from the centre find the water's edge; the longest run
/// of consecutive hits (in angle) is the shoreline seen from the village. Returned as a lane
/// 14 ft inland of it, smoothed and resampled every 25 ft. `None` if there's no water near.
fn shore_run(site: &Site, reach: f64) -> Option<Vec<P>> {
    const RAYS: usize = 72;
    let mut hits: Vec<Option<P>> = vec![None; RAYS];
    for (a, hit) in hits.iter_mut().enumerate() {
        let ang = std::f64::consts::TAU * a as f64 / RAYS as f64;
        let u = [libm::cos(ang), libm::sin(ang)];
        let mut t = 20.0;
        while t < reach {
            let q = mul(u, t);
            if site.wet(q) {
                // Refine the edge to a few feet, then step back inland.
                let (mut lo, mut hi) = (t - 20.0, t);
                for _ in 0..5 {
                    let m = 0.5 * (lo + hi);
                    if site.wet(mul(u, m)) { hi = m } else { lo = m }
                }
                *hit = Some(mul(u, (lo - 14.0).max(10.0)));
                break;
            }
            t += 20.0;
        }
    }
    // Longest circular run of hits.
    let mut best: (usize, usize) = (0, 0);
    for start in 0..RAYS {
        if hits[start].is_none() || hits[(start + RAYS - 1) % RAYS].is_some() && hits.iter().any(|h| h.is_none()) {
            continue;
        }
        let mut n = 0;
        while n < RAYS && hits[(start + n) % RAYS].is_some() {
            n += 1;
        }
        if n > best.1 {
            best = (start, n);
        }
    }
    if best.1 < 5 {
        return None;
    }
    let raw: Vec<P> = (0..best.1).map(|k| hits[(best.0 + k) % RAYS].unwrap()).collect();
    // Smooth (Chaikin) and resample.
    let mut pts = raw;
    for _ in 0..2 {
        let mut next = vec![pts[0]];
        for w in pts.windows(2) {
            next.push(lerp(w[0], w[1], 0.25));
            next.push(lerp(w[0], w[1], 0.75));
        }
        next.push(*pts.last().unwrap());
        pts = next;
    }
    let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    if total < 120.0 {
        return None;
    }
    let n = (total / 25.0).ceil() as usize;
    Some((0..=n).map(|k| point_along(&pts, total * k as f64 / n as f64).0).collect())
}

/// Piers along a wharf: every 70–120 ft, square to the shore, 40–80 ft into the water (a
/// river pier reaches at most a third of the way across). Built only where the water starts
/// right at the wharf and the whole pier stands over water.
fn piers_along(site: &Site, wharf: &[P], rng: &mut Pcg32, l: &mut Layout) {
    let total: f64 = wharf.windows(2).map(|w| dist(w[0], w[1])).sum();
    let mut at = rng.range(30.0, 70.0);
    while at < total - 20.0 {
        let (p, u) = point_along(wharf, at);
        let mut out = [-u[1], u[0]];
        if !site.wet(add(p, mul(out, 30.0))) {
            out = mul(out, -1.0);
        }
        let mut length = rng.range(40.0, 80.0);
        if let Some((_, _, hw)) = site.nearest_river(p)
            && site.near_river(add(p, mul(out, 30.0)), 0.0)
        {
            length = length.min(0.66 * hw);
        }
        // Anchor on the real water's edge along the pier line: the first wet point out from
        // the wharf (backing up if the wharf point is already wet), refined to a foot.
        let wet_at = |t: f64| site.wet(add(p, mul(out, t)));
        let mut t = 0.0;
        while wet_at(t) && t > -40.0 {
            t -= 4.0;
        }
        while !wet_at(t) && t < 80.0 {
            t += 4.0;
        }
        if wet_at(t) && !wet_at(t - 4.0) {
            let (mut lo, mut hi) = (t - 4.0, t);
            for _ in 0..3 {
                let m = 0.5 * (lo + hi);
                if wet_at(m) { hi = m } else { lo = m }
            }
            // The pier starts 6 ft on land and runs out over the water.
            let (base, end) = (lo - 6.0, hi + length);
            let over_water = (0..=8).all(|k| wet_at(hi + 2.0 + (length - 2.0) * k as f64 / 8.0));
            let pier = rect(add(p, mul(out, 0.5 * (base + end))), out, end - base, 8.0);
            // Never on a road (or its bridge) or another deck.
            let clear = !site.deck_on_road(&pier) && !l.bridges.iter().chain(&l.piers).any(|d| overlaps(d, &pier));
            if length >= 12.0 && over_water && !wet_at(base) && clear {
                l.piers.push(pier);
            }
        }
        at += rng.range(70.0, 120.0);
    }
}

/// Where the straight line a→b crosses the river centreline (a and b on opposite banks).
fn river_crossing(site: &Site, a: P, b: P) -> P {
    let side = |p: P| site.nearest_river(p).map_or(0.0, |(c, t, _)| cross(t, sub(p, c)));
    let sa = side(a);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let m = 0.5 * (lo + hi);
        if side(lerp(a, b, m)) * sa > 0.0 { lo = m } else { hi = m }
    }
    lerp(a, b, 0.5 * (lo + hi))
}

fn polar_pt(d: f64, a: f64) -> P {
    [d * libm::cos(a), d * libm::sin(a)]
}

/// The street from a road's end `e` to its gate `g` (local coords): a cubic that carries on
/// the way the road was heading and arrives square to the town edge, each only where that
/// leads towards the other end (else it heads straight there: no loops or doubling back).
fn approach_curve(e: P, g: P, heading: Option<P>) -> Vec<P> {
    let span = dist(e, g);
    let into = mul(sub(g, e), 1.0 / span.max(1e-9));
    let start = heading.filter(|h| dot(*h, into) > 0.3).unwrap_or(into);
    let inward = mul(g, -1.0 / len(g).max(1e-9));
    let end = if dot(inward, into) > 0.3 { inward } else { into };
    let (c1, c2) = (add(e, mul(start, 0.4 * span)), sub(g, mul(end, 0.35 * span)));
    let n = (span / 40.0).ceil().clamp(2.0, 24.0) as usize;
    (0..=n)
        .map(|j| {
            let t = j as f64 / n as f64;
            let u = 1.0 - t;
            add(add(mul(e, u * u * u), mul(c1, 3.0 * u * u * t)), add(mul(c2, 3.0 * u * t * t), mul(g, t * t * t)))
        })
        .collect()
}

/// Walls end at the bank: wall runs are cut where they cross a river channel (the water is
/// the defence there), each cut end gets a tower on the bank, and towers standing in the
/// channel go.
fn walls_off_river(site: &Site, l: &mut Layout) {
    if site.river.is_empty() {
        return;
    }
    let wet = |p: P| site.near_river(p, 3.0);
    let mut walls: Vec<Vec<P>> = Vec::new();
    for w in std::mem::take(&mut l.walls) {
        let mut cur: Vec<P> = Vec::new();
        for (k, seg) in w.windows(2).enumerate() {
            let (a, b) = (seg[0], seg[1]);
            if k == 0 && !wet(a) {
                cur.push(a);
            }
            let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
            let mut prev = a;
            for j in 1..=n {
                let p = lerp(a, b, j as f64 / n as f64);
                let (was, is) = (wet(prev), wet(p));
                if was != is {
                    // The bank between the two samples, by bisection.
                    let (mut lo, mut hi) = (prev, p);
                    for _ in 0..6 {
                        let m = lerp(lo, hi, 0.5);
                        if wet(m) == was { lo = m } else { hi = m }
                    }
                    let edge = if was { hi } else { lo };
                    if is {
                        cur.push(edge);
                        if cur.len() >= 2 {
                            l.towers.push(edge);
                            walls.push(std::mem::take(&mut cur));
                        }
                        cur.clear();
                    } else {
                        l.towers.push(edge);
                        cur.push(edge);
                    }
                }
                if j == n && !is {
                    cur.push(b);
                }
                prev = p;
            }
        }
        if cur.len() >= 2 {
            walls.push(cur);
        }
    }
    walls.retain(|w| w.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>() > 10.0);
    l.walls = walls;
    l.towers.retain(|t| !site.near_river(*t, 2.0));
    l.gate_towers.retain(|t| !site.near_river(*t, 2.0));
}

/// Point and unit direction `at` ft along a polyline.
fn point_along(pts: &[P], at: f64) -> (P, P) {
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let d = dist(w[0], w[1]);
        if acc + d >= at && d > 1e-9 {
            let u = mul(sub(w[1], w[0]), 1.0 / d);
            return (lerp(w[0], w[1], (at - acc) / d), u);
        }
        acc += d;
    }
    let n = pts.len();
    let d = dist(pts[n - 2], pts[n - 1]).max(1e-9);
    (pts[n - 1], mul(sub(pts[n - 1], pts[n - 2]), 1.0 / d))
}

/// Branch starts every `every` ft along a lane, alternating sides at random.
fn branch_points(pts: &[P], every: f64, rng: &mut Pcg32, out: &mut Vec<(P, P, u32)>, depth: u32) {
    let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let mut a = every * rng.range(0.5, 1.0);
    while a < total - 0.5 * every {
        let (p, u) = point_along(pts, a);
        let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
        let d = rotate([-u[1] * side, u[0] * side], rng.range(-0.35, 0.35));
        out.push((add(p, mul(d, 20.0)), d, depth));
        a += every * rng.range(0.8, 1.3);
    }
}

/// Grow a lane from `p` heading `d`: 25 ft steps, each turning (by up to 20°) toward the
/// most level ground; stops at water, the edge, or on meeting another lane.
fn track(site: &Site, p: P, d: P, max_r: f64, lanes: &[(Vec<P>, f64)], rng: &mut Pcg32) -> Vec<P> {
    const STEP: f64 = 25.0;
    let mut pts = vec![p];
    let (mut p, mut d) = (p, d);
    let wander = rng.range(-0.08, 0.08);
    for _ in 0..(max_r / STEP) as usize + 4 {
        let h = site.height(p);
        let mut best: Option<(f64, P)> = None;
        for turn in [-0.35, 0.0, 0.35] {
            let nd = rotate(d, turn + wander);
            let q = add(p, mul(nd, STEP));
            let cost = (site.height(q) - h).abs() + if turn == 0.0 { 0.0 } else { 0.6 };
            if best.is_none_or(|b| cost < b.0) {
                best = Some((cost, nd));
            }
        }
        d = best.unwrap().1;
        let q = add(p, mul(d, STEP));
        if site.wet(q) || site.near_river(q, 10.0) || len(q) > max_r {
            break;
        }
        pts.push(q);
        let met = lanes.iter().any(|(lp, _)| lp.windows(2).any(|w| seg_dist(q, w[0], w[1]) < 15.0));
        if met && pts.len() > 3 {
            break;
        }
        p = q;
    }
    pts
}

// ---------------------------------------------------------------------------------------
// Functions, names, floors.

fn assign_functions(site: &Site, s: &Settlement, on_water: bool, rng: &mut Pcg32, l: &mut Layout) {
    let tier = s.tier;
    let mut needs: Vec<usize> = Vec::new();
    for (fi, f) in CATALOG.iter().enumerate() {
        let n = catalog::required_count(f, tier, on_water);
        needs.extend(std::iter::repeat_n(fi, n));
        if n == 0 && tier >= f.min_tier && (!f.water || on_water) && tier != Tier::Metropolis && rng.next_f64() < 0.3 {
            needs.push(fi);
        }
    }
    // Enough buildings for the guarantees, even on a cramped or flooded site.
    let mut k = 0;
    while l.buildings.len() < needs.len() && k < 400 {
        k += 1;
        let a = rng.range(0.0, std::f64::consts::TAU);
        let rad = rng.range(30.0, l.radius.max(80.0));
        let c = [rad * libm::cos(a), rad * libm::sin(a)];
        let house = rect(c, [-libm::sin(a), libm::cos(a)], 30.0, 20.0);
        let clear = l.buildings.iter().all(|b| dist(centroid(&b.poly), c) > 28.0);
        if clear && (site.buildable(&house) || (k > 300 && !site.wet(c) && site.dry_outline(&house, 10.0))) {
            let pad = site.pad(&house);
            let ward = if tier == Tier::Village { Ward::Rural } else { Ward::Common };
            l.buildings.push(Building { poly: house, ward, func: None, residential: 1, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None });
        }
    }
    // Big functions first so they get the big lots.
    needs.sort_by_key(|&fi| (!CATALOG[fi].big, fi));
    let areas: Vec<f64> = l.buildings.iter().map(|b| area(&b.poly).abs()).collect();
    let max_area = areas.iter().cloned().fold(1.0, f64::max);
    let mut taken = vec![false; l.buildings.len()];
    // The keep at the heart of a castle ward (4 storeys, raised by the ward layout) is the
    // castle.
    if let Some(castle) = CATALOG.iter().position(|f| f.key == "castle")
        && let Some(keep) = l.buildings.iter().position(|b| b.ward == Ward::Castle && b.func.is_none() && b.residential == 4 && b.floors == 4)
    {
        l.buildings[keep].func = Some(castle as u16);
        taken[keep] = true;
        if let Some(k) = needs.iter().position(|&fi| fi == castle) {
            needs.remove(k);
        }
    }
    for fi in needs {
        let f = &CATALOG[fi];
        let mut best: Option<(f64, usize)> = None;
        for (bi, b) in l.buildings.iter().enumerate() {
            if taken[bi] {
                continue;
            }
            let ward_score = f.wards.iter().position(|w| *w == b.ward).map_or(0.0, |p| 10.0 - 2.0 * p as f64);
            let size = areas[bi] / max_area;
            let size_score = if f.big { 6.0 * size } else { -2.0 * size };
            // A business needs room for its fittings (a bar and tables, a counter): a shed-sized
            // lot only when nothing else is left.
            let cramped = if areas[bi] < 500.0 { 100.0 } else { 0.0 };
            let score = ward_score + size_score - cramped + rng.next_f64() * 1.5;
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, bi));
            }
        }
        let Some((_, bi)) = best else { break };
        taken[bi] = true;
        l.buildings[bi].func = Some(fi as u16);
        if f.key == "graveyard" {
            l.buildings[bi].structure = Structure::Open;
        }
    }
    // Names and floors.
    let mut namer = Namer::new(hash2(s.seed, 0x7a3e, 1));
    let mut used: std::collections::BTreeSet<String> = Default::default();
    let culture = s.culture as usize;
    for b in &mut l.buildings {
        let big = area(&b.poly).abs() > 2500.0;
        match b.func {
            Some(fi) => {
                let f = &CATALOG[fi as usize];
                for _ in 0..12 {
                    match business_name(f, rng, &mut namer, culture) {
                        Some(n) if used.insert(n.clone()) => {
                            b.name = Some(n);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
                b.floors = match f.key {
                    "castle" | "palace" => 4,
                    "inn" | "tavern" | "town_hall" | "library" | "arcane_academy" | "guildhall" => 2 + (rng.next_f64() < 0.5) as u8,
                    "temple" | "warehouse" | "arena" | "graveyard" | "shrine" => 1,
                    _ => 1 + (tier >= Tier::Town && rng.next_f64() < 0.6) as u8,
                };
            }
            None => {
                b.residential = catalog::residential_for(b.ward, big) as u8;
                b.floors = match b.ward {
                    Ward::Noble | Ward::Merchant | Ward::Castle => 2 + (rng.next_f64() < 0.4) as u8,
                    Ward::Slum | Ward::Common | Ward::Craft | Ward::Docks => 1 + (tier >= Tier::City && rng.next_f64() < 0.5) as u8,
                    _ => 1,
                };
            }
        }
    }
}

/// A business's name in its function's style (`None` for plain ones: warehouses, graveyards).
pub fn business_name(f: &catalog::Func, rng: &mut Pcg32, namer: &mut Namer, culture: usize) -> Option<String> {
    match f.naming {
        Naming::Sign => Some(format!("The {} {}", pick(rng, catalog::SIGN_ADJ), pick(rng, catalog::SIGN_NOUN))),
        Naming::Owner => Some(format!("{}'s {}", namer.word(culture, 1, 2), f.trade)),
        Naming::Institution => Some(match f.key {
            "temple" | "shrine" | "monastery" => format!("{} of {}", f.trade, pick(rng, catalog::DEITY)),
            _ => format!("The {} {}", pick(rng, catalog::INSTITUTION_ADJ), f.trade),
        }),
        Naming::Plain => None,
    }
}

fn pick<'a>(rng: &mut Pcg32, xs: &[&'a str]) -> &'a str {
    xs[rng.below(xs.len() as u32) as usize]
}
