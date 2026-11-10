//! Geometry queries for agents (`mapd`'s `nearest`, `road_point`, `search_features within` and
//! `describe_location`): areas, the nearest coast, cliff, river, road or flat ground to a point, the
//! ground and water at a point, and points along a road.
//!
//! Terrain answers are judged and placed on the battlemap's own ground (refined tiles, 5-ft squares);
//! the T0 grids (one cell is `t0.cell_ft`, about 1.2 miles on a 1,200-mile map) only find candidates.
//! Searches walk rings out from the point and stop once nothing nearer can exist: never the whole map.
//! Thresholds are defined once, here (and in docs/MCP.md "nearest").

use serde_json::{Value, json};

use std::rc::Rc;

use crate::World;
use crate::battlemap::{SQ, SQUARE_FT};
use crate::core::hash::FastMap;
use crate::core::tile::{HALO, PADDED, TILE_N, TileKey};
use crate::ground::Shaper;
use crate::lod::terrain_refine::TerrainOut;
use crate::pipeline::Executor;
use crate::world::Edits;
use crate::lod::roads::RoadCurve;
use crate::t0::T0;
use crate::t0::features::Shape;
use crate::t0::hydro;
use crate::town::geom;

type P = [f64; 2];

const MI: f64 = 5280.0;
/// `describe_location` looks for a river or lake this far (ft).
pub const WATER_NEAR_FT: f64 = 1_000.0;
/// ... and for the sea's coast this far (ft).
pub const COAST_NEAR_FT: f64 = 5.0 * MI;
/// `nearest`: the default and the largest `within_mi`.
pub const NEAREST_DEFAULT_MI: f64 = 5.0;
pub const NEAREST_MAX_MI: f64 = 25.0;
/// Flat ground's candidates: gentler than this (degrees, on T0) over the cell and the four beside it.
pub const FLAT_DEG: f64 = 5.0;

// ---------------------------------------------------------------------------------------
// Areas.

/// An area of the map (ft): a box [x0, y0, x1, y1], a polygon or a circle.
#[derive(Clone, Debug)]
pub enum Region {
    Box([f64; 4]),
    Poly(Vec<P>),
    Circle(P, f64),
}

fn poly_has(q: &[P], p: P) -> bool {
    let mut inside = false;
    let mut j = q.len() - 1;
    for i in 0..q.len() {
        if (q[i][1] > p[1]) != (q[j][1] > p[1]) && p[0] < (q[j][0] - q[i][0]) * (p[1] - q[i][1]) / (q[j][1] - q[i][1]) + q[i][0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn seg_dist(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    geom::dist(p, [a[0] + t * dx, a[1] + t * dy])
}

fn closest_on_seg(p: P, a: P, b: P) -> P {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = d[0] * d[0] + d[1] * d[1];
    let t = if l > 0.0 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l).clamp(0.0, 1.0) } else { 0.0 };
    [a[0] + t * d[0], a[1] + t * d[1]]
}

fn orient(a: P, b: P, c: P) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0)
}

impl Region {
    pub fn contains(&self, p: P) -> bool {
        match self {
            Region::Box(b) => p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3],
            Region::Circle(c, r) => geom::dist(p, *c) <= *r,
            Region::Poly(q) => poly_has(q, p),
        }
    }

    pub fn bbox(&self) -> [f64; 4] {
        match self {
            Region::Box(b) => *b,
            Region::Circle(c, r) => [c[0] - r, c[1] - r, c[0] + r, c[1] + r],
            Region::Poly(q) => q.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]),
        }
    }

    pub fn middle(&self) -> P {
        let b = self.bbox();
        [0.5 * (b[0] + b[2]), 0.5 * (b[1] + b[3])]
    }

    fn corners(&self) -> Vec<P> {
        match self {
            Region::Box(b) => vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]],
            Region::Poly(q) => q.clone(),
            Region::Circle(..) => Vec::new(),
        }
    }

    /// 0 inside; else the distance to the area's edge (ft).
    fn dist_to(&self, p: P) -> f64 {
        if self.contains(p) {
            return 0.0;
        }
        match self {
            Region::Circle(c, r) => geom::dist(p, *c) - r,
            _ => {
                let q = self.corners();
                (0..q.len()).map(|i| seg_dist(p, q[i], q[(i + 1) % q.len()])).fold(f64::MAX, f64::min)
            }
        }
    }

    /// Whether the segment a-b touches the area.
    fn hits_segment(&self, a: P, b: P) -> bool {
        if self.contains(a) || self.contains(b) {
            return true;
        }
        match self {
            Region::Circle(c, r) => seg_dist(*c, a, b) <= *r,
            _ => {
                let q = self.corners();
                (0..q.len()).any(|i| segs_cross(a, b, q[i], q[(i + 1) % q.len()]))
            }
        }
    }

    /// Whether a course touches the area.
    pub fn hits_line(&self, pts: &[P]) -> bool {
        let b = self.bbox();
        pts.windows(2).any(|w| (w[0][0].max(w[1][0]) >= b[0] && w[0][0].min(w[1][0]) <= b[2] && w[0][1].max(w[1][1]) >= b[1] && w[0][1].min(w[1][1]) <= b[3]) && self.hits_segment(w[0], w[1]))
    }

    /// Whether a feature's extent overlaps the area: a point's disc, a course, or an area's
    /// cells (tested at up to 400 sample points over the overlap, one per cell or sparser).
    pub fn overlaps(&self, s: &Shape) -> bool {
        let (a, b) = (s.bbox(), self.bbox());
        if a[2] < b[0] || a[0] > b[2] || a[3] < b[1] || a[1] > b[3] {
            return false;
        }
        match s {
            Shape::Point { at, radius_ft } => self.dist_to(*at) <= *radius_ft,
            Shape::Line(pts) => {
                let pts: Vec<P> = pts.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
                self.hits_line(&pts)
            }
            Shape::Area { cell_ft, .. } => {
                // The area's middle and corners in the shape, then the shape's cells in the area.
                if self.corners().into_iter().chain([self.middle()]).any(|p| s.distance(p).0 == 0.0) {
                    return true;
                }
                let o = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
                let step = cell_ft.max((o[2] - o[0]).max(o[3] - o[1]) / 20.0);
                let (nx, ny) = (((o[2] - o[0]) / step).floor() as usize + 1, ((o[3] - o[1]) / step).floor() as usize + 1);
                (0..ny).any(|j| (0..nx).any(|i| {
                    let p = [o[0] + (i as f64 + 0.5) * step.min(o[2] - o[0]), o[1] + (j as f64 + 0.5) * step.min(o[3] - o[1])];
                    self.contains(p) && s.distance(p).0 == 0.0
                }))
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Terrain.

fn cell(t0: &T0) -> f64 {
    t0.cell_ft
}

fn cell_of(t0: &T0, p: P) -> (i64, i64) {
    ((r1(p[0] / cell(t0)) as i64).clamp(0, t0.height.w as i64 - 1), (r1(p[1] / cell(t0)) as i64).clamp(0, t0.height.h as i64 - 1))
}

fn center(t0: &T0, i: i64, j: i64) -> P {
    [i as f64 * cell(t0), j as f64 * cell(t0)]
}

fn h_at(t0: &T0, i: i64, j: i64) -> f64 {
    t0.height.get(i.clamp(0, t0.height.w as i64 - 1) as usize, j.clamp(0, t0.height.h as i64 - 1) as usize) as f64
}

fn slope_of(gx: f64, gy: f64) -> f64 {
    libm::atan(libm::hypot(gx, gy)).to_degrees()
}

/// The ground's slope at a point (degrees), over one T0 cell (half a cell either side).
pub fn slope_deg(t0: &T0, p: P) -> f64 {
    let r = 0.5 * cell(t0);
    let h = |x: f64, y: f64| t0.sample(x, y, cell(t0));
    slope_of((h(p[0] + r, p[1]) - h(p[0] - r, p[1])) / (2.0 * r), (h(p[0], p[1] + r) - h(p[0], p[1] - r)) / (2.0 * r))
}

fn slope_cell(t0: &T0, i: i64, j: i64) -> f64 {
    let c = cell(t0);
    slope_of((h_at(t0, i + 1, j) - h_at(t0, i - 1, j)) / (2.0 * c), (h_at(t0, i, j + 1) - h_at(t0, i, j - 1)) / (2.0 * c))
}

/// Whether a point is in the sea (ocean, bay: not a lake).
pub fn ocean(t0: &T0, p: P) -> bool {
    let (i, j) = cell_of(t0, p);
    t0.coast.get(i as usize, j as usize) > 0.0 && t0.sample_water(p[0], p[1]) != hydro::DRY
}

/// Whether a point is in any standing water (sea or lake; rivers are separate).
fn wet(t0: &T0, p: P) -> bool {
    t0.sample_water(p[0], p[1]) != hydro::DRY
}

fn dry_cell(t0: &T0, i: i64, j: i64) -> bool {
    !wet(t0, center(t0, i, j))
}

/// Cells for which `hit` holds, nearest first (by centre), within `max_ft` of `p`: the nearest and
/// any within one and a half cells of it (at most 12). Walks rings of cells out from `p`'s own.
fn nearest_cells(t0: &T0, p: P, max_ft: f64, hit: &dyn Fn(i64, i64) -> bool) -> Vec<(f64, i64, i64)> {
    let c = cell(t0);
    let (ci, cj) = cell_of(t0, p);
    let rmax = (max_ft / c).ceil() as i64 + 1;
    let (w, h) = (t0.height.w as i64, t0.height.h as i64);
    let mut found: Vec<(f64, i64, i64)> = Vec::new();
    for r in 0..=rmax {
        if let Some(b) = found.iter().map(|f| f.0).reduce(f64::min)
            && (r as f64 - 1.0) * c > b + 1.5 * c
        {
            break;
        }
        let mut visit = |i: i64, j: i64| {
            if i < 0 || j < 0 || i >= w || j >= h {
                return;
            }
            let d = geom::dist(center(t0, i, j), p);
            if d <= max_ft + c && hit(i, j) {
                found.push((d, i, j));
            }
        };
        if r == 0 {
            visit(ci, cj);
            continue;
        }
        for k in -r..=r {
            visit(ci + k, cj - r);
            visit(ci + k, cj + r);
        }
        for k in (-r + 1)..r {
            visit(ci - r, cj + k);
            visit(ci + r, cj + k);
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let best = found.first().map_or(0.0, |f| f.0);
    found.retain(|f| f.0 <= best + 1.5 * c);
    found.truncate(12);
    found
}

/// Where along a to b (state(a) != state(b)) the state flips, to a foot or so.
fn bisect(state: &dyn Fn(P) -> bool, a: P, b: P) -> P {
    let s = state(a);
    let (mut lo, mut hi) = (a, b);
    for _ in 0..14 {
        let m = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])];
        if state(m) == s {
            lo = m;
        } else {
            hi = m;
        }
    }
    [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])]
}

/// The nearest point (within `max_ft`) where `state` differs from its value at `p`, with the unit direction
/// across it from the `p` state's side to the other (None where none).
fn nearest_flip(t0: &T0, p: P, max_ft: f64, state: &dyn Fn(P) -> bool) -> Option<(f64, P, P)> {
    let s = state(p);
    let cells = nearest_cells(t0, p, max_ft, &|i, j| state(center(t0, i, j)) != s);
    let mut best: Option<(f64, P, P)> = None;
    for (_, i, j) in cells {
        let c = center(t0, i, j);
        let f = bisect(state, p, c);
        let d = geom::dist(f, p);
        if d <= max_ft && best.is_none_or(|b| d < b.0) {
            let l = geom::dist(c, p).max(1e-9);
            best = Some((d, f, [(c[0] - p[0]) / l, (c[1] - p[1]) / l]));
        }
    }
    best
}

fn r1(v: f64) -> f64 {
    crate::core::round(v)
}

fn pt(p: P) -> Value {
    json!({ "x_ft": r1(p[0]), "y_ft": r1(p[1]) })
}

fn base(what: &str, p: P, at: P, d: f64) -> Value {
    let mut v = json!({ "what": what, "found": true, "x_ft": r1(at[0]), "y_ft": r1(at[1]), "distance_ft": r1(d), "distance_mi": r1((d / MI) * 100.0) / 100.0 });
    if d > 1.0 {
        v["direction"] = json!(compass(p, at));
    }
    v
}

fn compass(from: P, to: P) -> &'static str {
    let a = libm::atan2(to[1] - from[1], to[0] - from[0]).to_degrees();
    ["E", "SE", "S", "SW", "W", "NW", "N", "NE"][((((a + 22.5) % 360.0) + 360.0) % 360.0 / 45.0) as usize % 8]
}

// ---------------------------------------------------------------------------------------
// Coast, water, cliffs, flat ground.

fn flat_cell(t0: &T0, i: i64, j: i64) -> bool {
    [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(a, b)| dry_cell(t0, i + a, j + b) && h_at(t0, i + a, j + b) > -1e5 && slope_cell(t0, i + a, j + b) < FLAT_DEG)
}

// ---------------------------------------------------------------------------------------
// The battlemap's own ground: refined terrain read through the tile pipeline (the executor
// caches tiles, so a second call nearby is cheap). T0 cells only find candidates; every
// terrain answer is judged and placed on these samples. Each sample is read at its global
// lattice index, so answers are the same whatever was read before.

/// The ground as generated over a box of 5-ft squares (global indices, inclusive), with the water
/// over each: read as a battlemap reads them (square centres are odd samples of the finest tiles).
pub struct Squares {
    pub gx0: i64,
    pub gy0: i64,
    pub w: i64,
    pub h: i64,
    /// The ground (ft, the generator's datum).
    pub g: Vec<f32>,
    /// The water's surface: standing water or a river, the higher (`hydro::DRY` where none).
    pub hw: Vec<f32>,
    /// Standing water's surface alone (sea or lake).
    pub still: Vec<f32>,
}

impl Squares {
    pub fn read(ex: &mut Executor, gx0: i64, gy0: i64, gx1: i64, gy1: i64) -> Squares {
        let lvl = ex.world.geom.max_level;
        let sq = SQ as i64;
        let (w, h) = (gx1 - gx0 + 1, gy1 - gy0 + 1);
        let n = (w * h) as usize;
        let (mut g, mut hw, mut still) = (vec![0f32; n], vec![0f32; n], vec![0f32; n]);
        for ty in gy0.div_euclid(sq)..=gy1.div_euclid(sq) {
            for tx in gx0.div_euclid(sq)..=gx1.div_euclid(sq) {
                let tile = ex.terrain(TileKey::surface(lvl, tx as u32, ty as u32));
                for gy in (ty * sq).max(gy0)..=((ty + 1) * sq - 1).min(gy1) {
                    for gx in (tx * sq).max(gx0)..=((tx + 1) * sq - 1).min(gx1) {
                        let (i, j) = ((gx - tx * sq) as usize, (gy - ty * sq) as usize);
                        let c = (2 * j + 1 + HALO) * PADDED + 2 * i + 1 + HALO;
                        let (x, y) = ((gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT);
                        let k = ((gy - gy0) * w + gx - gx0) as usize;
                        g[k] = tile.padded[c];
                        still[k] = ex.t0.sample_water(x, y);
                        hw[k] = still[k].max(tile.river_water[c]);
                    }
                }
            }
        }
        Squares { gx0, gy0, w, h, g, hw, still }
    }

    /// The squares round a box of world ft (and `pad` squares more), clamped to the map.
    pub fn round(ex: &mut Executor, b: [f64; 4], pad: i64) -> Squares {
        let geom = &ex.world.geom;
        let (mx, my) = ((geom.map_w_ft / SQUARE_FT).ceil() as i64, (geom.map_h_ft / SQUARE_FT).ceil() as i64);
        let sq = |v: f64| crate::core::floor(v / SQUARE_FT) as i64;
        let (gx0, gy0) = ((sq(b[0]) - pad).clamp(0, mx - 1), (sq(b[1]) - pad).clamp(0, my - 1));
        let (gx1, gy1) = ((sq(b[2]) + pad).clamp(gx0, mx - 1), (sq(b[3]) + pad).clamp(gy0, my - 1));
        Squares::read(ex, gx0, gy0, gx1, gy1)
    }

    pub fn at(&self, gx: i64, gy: i64) -> Option<usize> {
        let (i, j) = (gx - self.gx0, gy - self.gy0);
        (i >= 0 && j >= 0 && i < self.w && j < self.h).then_some((j * self.w + i) as usize)
    }

    pub fn rect(&self) -> [f64; 4] {
        [self.gx0 as f64 * SQUARE_FT, self.gy0 as f64 * SQUARE_FT, (self.gx0 + self.w) as f64 * SQUARE_FT, (self.gy0 + self.h) as f64 * SQUARE_FT]
    }

    /// Each square's ground shaped by `edits`.
    pub fn shaped(&self, edits: &Edits) -> Vec<f32> {
        match Shaper::new(edits, self.rect(), SQUARE_FT) {
            None => self.g.clone(),
            Some(s) => (0..self.g.len()).map(|k| s.at_square(self.gx0 + k as i64 % self.w, self.gy0 + k as i64 / self.w, self.g[k], self.hw[k]).0).collect(),
        }
    }
}

/// Squares with the ground as shaped by hand: what the battlemap draws.
struct Patch {
    s: Squares,
    h: Vec<f32>,
}

impl Patch {
    /// The squares within `half_ft` of `c` each way.
    fn read(ex: &mut Executor, c: P, half_ft: f64) -> Patch {
        let s = Squares::round(ex, [c[0] - half_ft, c[1] - half_ft, c[0] + half_ft, c[1] + half_ft], 0);
        let h = s.shaped(&ex.world.file.edits);
        Patch { s, h }
    }

    fn gxy(&self, k: usize) -> (i64, i64) {
        (self.s.gx0 + k as i64 % self.s.w, self.s.gy0 + k as i64 / self.s.w)
    }

    fn centre(&self, k: usize) -> P {
        let (x, y) = self.gxy(k);
        [(x as f64 + 0.5) * SQUARE_FT, (y as f64 + 0.5) * SQUARE_FT]
    }

    /// The square holding `p` (the nearest in the patch).
    fn k_of(&self, p: P) -> usize {
        let sq = |v: f64| crate::core::floor(v / SQUARE_FT) as i64;
        let (gx, gy) = (sq(p[0]).clamp(self.s.gx0, self.s.gx0 + self.s.w - 1), sq(p[1]).clamp(self.s.gy0, self.s.gy0 + self.s.h - 1));
        ((gy - self.s.gy0) * self.s.w + gx - self.s.gx0) as usize
    }

    fn off(&self, k: usize, dx: i64, dy: i64) -> Option<usize> {
        let (x, y) = self.gxy(k);
        self.s.at(x + dx, y + dy)
    }

    fn wet(&self, k: usize) -> bool {
        self.s.hw[k] > self.h[k]
    }

    /// Under a river's water (not only standing water).
    fn river(&self, k: usize) -> bool {
        self.wet(k) && self.s.hw[k] > self.s.still[k]
    }

    /// The slope (degrees) at `k` over `n` squares either side (one side at the patch's edge).
    fn slope(&self, k: usize, n: i64) -> f64 {
        let axis = |dx: i64, dy: i64| {
            let (a, ra) = self.off(k, dx * n, dy * n).map_or((self.h[k], 0), |j| (self.h[j], n));
            let (b, rb) = self.off(k, -dx * n, -dy * n).map_or((self.h[k], 0), |j| (self.h[j], n));
            if ra + rb == 0 { 0.0 } else { (a - b) as f64 / ((ra + rb) as f64 * SQUARE_FT) }
        };
        slope_of(axis(1, 0), axis(0, 1))
    }

    /// The slope (degrees) of the plane fitted to the whole patch.
    fn plane_slope(&self) -> f64 {
        let n = self.h.len() as f64;
        let (mx, my) = (0.5 * (self.s.w - 1) as f64, 0.5 * (self.s.h - 1) as f64);
        let mh = self.h.iter().map(|&v| v as f64).sum::<f64>() / n;
        let (mut sx, mut sy, mut xx, mut yy) = (0.0, 0.0, 0.0, 0.0);
        for k in 0..self.h.len() {
            let (x, y) = ((k as i64 % self.s.w) as f64 - mx, (k as i64 / self.s.w) as f64 - my);
            let d = self.h[k] as f64 - mh;
            (sx, sy, xx, yy) = (sx + x * d, sy + y * d, xx + x * x, yy + y * y);
        }
        slope_of(if xx > 0.0 { sx / xx / SQUARE_FT } else { 0.0 }, if yy > 0.0 { sy / yy / SQUARE_FT } else { 0.0 })
    }

    /// The dry square (not under river or lake water) beside a sea square nearest `p`.
    fn dry_by_sea(&self, t0: &T0, p: P) -> Option<usize> {
        let sea = |k: usize| sea_at(t0, self.centre(k), self.h[k]);
        (0..self.h.len())
            .filter(|&k| !self.wet(k) && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| self.off(k, dx, dy).is_some_and(sea)))
            .map(|k| (geom::dist(self.centre(k), p), k))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|f| f.1)
    }

    /// The square nearest `p` for which `state` (t0, square's middle, its ground) differs from `here`.
    fn nearest_not(&self, t0: &T0, p: P, here: bool, state: State) -> Option<(f64, usize)> {
        (0..self.h.len()).filter(|&k| state(t0, self.centre(k), self.h[k]) != here).map(|k| (geom::dist(self.centre(k), p), k)).min_by(|a, b| a.0.total_cmp(&b.0))
    }
}

/// A state of the refined ground at a point: (t0, the point, its ground in ft) to true or false.
type State<'a> = &'a dyn Fn(&T0, P, f32) -> bool;

/// Whether a point lies on the sea's side of the T0 coast grid (the sea, as against a lake).
fn sea_side(t0: &T0, p: P) -> bool {
    let (i, j) = cell_of(t0, p);
    t0.coast.get(i as usize, j as usize) > 0.0
}

/// In the sea, on refined ground: the sea's surface above the ground.
fn sea_at(t0: &T0, q: P, g: f32) -> bool {
    sea_side(t0, q) && t0.sample_water(q[0], q[1]) > g
}

/// Under standing water (sea or lake), on refined ground.
fn still_at(t0: &T0, q: P, g: f32) -> bool {
    t0.sample_water(q[0], q[1]) > g
}

/// The samples of one tile level (`step` ft apart, at global lattice indices), read lazily.
struct Lattice {
    level: u8,
    step: f64,
    last: (i64, i64),
    tiles: FastMap<(i64, i64), Rc<TerrainOut>>,
}

impl Lattice {
    /// The coarsest refined level whose samples are at most `step_ft` apart.
    fn new(ex: &Executor, step_ft: f64) -> Lattice {
        let g = &ex.world.geom;
        let level = (g.first_refine_level..=g.max_level).find(|&l| g.spacing_ft(l) <= step_ft + 1e-6).unwrap_or(g.max_level);
        let step = g.spacing_ft(level);
        Lattice { level, step, last: ((g.map_w_ft / step) as i64, (g.map_h_ft / step) as i64), tiles: FastMap::default() }
    }

    fn index(&self, p: P) -> (i64, i64) {
        ((crate::core::round(p[0] / self.step) as i64).clamp(0, self.last.0), (crate::core::round(p[1] / self.step) as i64).clamp(0, self.last.1))
    }

    fn pos(&self, i: i64, j: i64) -> P {
        [i as f64 * self.step, j as f64 * self.step]
    }

    fn inside(&self, i: i64, j: i64) -> bool {
        i >= 0 && j >= 0 && i <= self.last.0 && j <= self.last.1
    }

    /// The tile and the padded index holding sample (i, j) (clamped to the map).
    fn sample(&mut self, ex: &mut Executor, i: i64, j: i64) -> (&TerrainOut, usize) {
        let (i, j) = (i.clamp(0, self.last.0), j.clamp(0, self.last.1));
        let n = TILE_N as i64;
        let (nx, ny) = ex.world.geom.tiles_across(self.level);
        let (tx, ty) = (i.div_euclid(n).min(nx as i64 - 1), j.div_euclid(n).min(ny as i64 - 1));
        let level = self.level;
        let tile = self.tiles.entry((tx, ty)).or_insert_with(|| ex.terrain(TileKey::surface(level, tx as u32, ty as u32)));
        let c = (j - ty * n) as usize;
        let r = (i - tx * n) as usize;
        (tile, (c + HALO) * PADDED + r + HALO)
    }

    /// The ground (ft) at sample (i, j).
    fn ground(&mut self, ex: &mut Executor, i: i64, j: i64) -> f32 {
        let (t, c) = self.sample(ex, i, j);
        t.padded[c]
    }

    /// Whether a road is carved at sample (i, j) (None: this tile carries no road mask).
    fn road(&mut self, ex: &mut Executor, i: i64, j: i64) -> Option<bool> {
        let (t, c) = self.sample(ex, i, j);
        t.road_mask.get(c).map(|&m| m != 0)
    }
}

/// The nearest sample to `p` (within `max_ft`) for which `hit` holds, walking rings of samples
/// out from `p`'s and stopping at the first ring past the best.
fn ring(ex: &mut Executor, lat: &mut Lattice, p: P, max_ft: f64, hit: &mut dyn FnMut(&mut Executor, &mut Lattice, i64, i64) -> bool) -> Option<(f64, P)> {
    let (ci, cj) = lat.index(p);
    let rmax = crate::core::ceil(max_ft / lat.step) as i64 + 1;
    let mut best: Option<(f64, P)> = None;
    for r in 0..=rmax {
        if best.is_some_and(|b| (r as f64 - 1.0) * lat.step > b.0) {
            break;
        }
        let mut visit = |ex: &mut Executor, lat: &mut Lattice, i: i64, j: i64| {
            if !lat.inside(i, j) {
                return;
            }
            let q = lat.pos(i, j);
            let d = geom::dist(q, p);
            if d <= max_ft && best.is_none_or(|b| d < b.0) && hit(ex, lat, i, j) {
                best = Some((d, q));
            }
        };
        if r == 0 {
            visit(ex, lat, ci, cj);
            continue;
        }
        for k in -r..=r {
            visit(ex, lat, ci + k, cj - r);
            visit(ex, lat, ci + k, cj + r);
        }
        for k in (-r + 1)..r {
            visit(ex, lat, ci - r, cj + k);
            visit(ex, lat, ci + r, cj + k);
        }
    }
    best
}

/// The nearest sample to `p` (within `max_ft`) in the box `b` for which `hit` holds.
fn in_box(ex: &mut Executor, lat: &mut Lattice, p: P, max_ft: f64, b: [f64; 4], hit: &mut dyn FnMut(&mut Executor, &mut Lattice, i64, i64) -> bool) -> Option<(f64, P)> {
    let (i0, j0) = lat.index([b[0], b[1]]);
    let (i1, j1) = lat.index([b[2], b[3]]);
    let mut best: Option<(f64, P)> = None;
    for j in j0..=j1 {
        for i in i0..=i1 {
            let q = lat.pos(i, j);
            let d = geom::dist(q, p);
            if d <= max_ft && best.is_none_or(|b| d < b.0) && hit(ex, lat, i, j) {
                best = Some((d, q));
            }
        }
    }
    best
}

/// The nearest point to `p` (within `max_ft`, on samples `step` ft apart, then on 5-ft squares about
/// the sample found) where `state` is not `here`: the square found and the patch round it.
fn flip(ex: &mut Executor, p: P, max_ft: f64, step: f64, here: bool, state: State, window: Option<[f64; 4]>) -> Option<(f64, usize, Patch)> {
    let mut lat = Lattice::new(ex, step);
    let mut hit = |ex: &mut Executor, lat: &mut Lattice, i: i64, j: i64| {
        let g = lat.ground(ex, i, j);
        state(&ex.t0, lat.pos(i, j), g) != here
    };
    let (_, q) = match window {
        None => ring(ex, &mut lat, p, max_ft, &mut hit),
        Some(b) => in_box(ex, &mut lat, p, max_ft, b, &mut hit),
    }?;
    let pa = Patch::read(ex, q, 1.5 * lat.step + 10.0);
    let (d, k) = pa.nearest_not(&ex.t0, p, here, state).filter(|f| f.0 <= max_ft)?;
    Some((d, k, pa))
}

/// The ground under `p` as the battlemap draws it: the square there and 4 squares (20 ft) round it.
fn here(ex: &mut Executor, p: P) -> (Patch, usize) {
    let pa = Patch::read(ex, p, 4.0 * SQUARE_FT);
    let k = pa.k_of(p);
    (pa, k)
}

/// How far the refined coast is searched for about the point, and about the T0 coast beyond (ft, in T0 cells):
/// the T0 coast is up to about 2.3 mi from the battlemap's on steep coasts.
const COAST_MARGIN_CELLS: f64 = 3.0;
/// The coast is searched on samples this far apart (ft), then placed on 5-ft squares.
const COAST_STEP_FT: f64 = 80.0;
/// Standing water near a point is searched on samples this far apart (ft).
const WATER_STEP_FT: f64 = 20.0;
/// A cliff: the ground drops at least `CLIFF_RISE_FT` within `CLIFF_RUN_FT`, on 5-ft squares (a cliff coast the same at the water).
pub const CLIFF_RISE_FT: f64 = 15.0;
pub const CLIFF_RUN_FT: f64 = 20.0;
/// Cliffs are looked for in T0 cells steeper than this (degrees over the cell): the nearest two and the steepest two
/// within reach, scanned on samples `CLIFF_STEP_FT` apart; the steepest few spots then measured on squares.
const CLIFF_CELL_DEG: f64 = 12.0;
const CLIFF_STEP_FT: f64 = 40.0;
const CLIFF_SCAN_FT: f32 = 10.0;
/// Flat ground: a `FLAT_PATCH_FT` square patch of dry squares whose heights differ by at most `FLAT_RANGE_FT`.
pub const FLAT_PATCH_FT: f64 = 200.0;
pub const FLAT_RANGE_FT: f32 = 10.0;
/// How many coarse candidates are measured on squares at most.
const FLAT_TRIES: usize = 12;

/// The sea's coast nearest `p` within `max_ft`, on the battlemap's ground: `{distance_ft, x_ft, y_ft (the last dry
/// square), shore: cliff | beach, rise_ft (the land's height within CLIFF_RUN_FT of the water), inland_ft (within
/// 100 ft), in_sea}`. The T0 coast only says whether there is one in reach and where to look beyond the point's own surroundings.
pub fn coast(ex: &mut Executor, p: P, max_ft: f64) -> Option<Value> {
    let margin = COAST_MARGIN_CELLS * cell(&ex.t0);
    let (cd, cf, _) = {
        let t0 = &ex.t0;
        nearest_flip(t0, p, max_ft + margin, &|q| ocean(t0, q))?
    };
    let (pa, k) = here(ex, p);
    let in_sea = sea_at(&ex.t0, pa.centre(k), pa.h[k]);
    let near = margin.min(max_ft);
    let found = flip(ex, p, near, COAST_STEP_FT, in_sea, &sea_at, None).or_else(|| (cd + margin > near).then(|| flip(ex, p, max_ft, COAST_STEP_FT, in_sea, &sea_at, Some([cf[0] - margin, cf[1] - margin, cf[0] + margin, cf[1] + margin]))).flatten());
    let (_, x, pa) = found?;
    // The last dry square: the one found when coming from the sea, else the dry square nearest the water found.
    let t0 = &ex.t0;
    let e = pa.dry_by_sea(t0, p).unwrap_or_else(|| if in_sea { x } else { pa.nearest_not(t0, pa.centre(x), true, &sea_at).map_or(x, |f| f.1) });
    let e = pa.centre(e);
    let pb = Patch::read(ex, e, 100.0);
    let water = ex.world.params().sea_level_ft as f32;
    let height = |reach: f64| (0..pb.h.len()).filter(|&j| !pb.wet(j) && geom::dist(pb.centre(j), e) <= reach).map(|j| pb.h[j] - water).fold(0f32, f32::max) as f64;
    let (rise, inland) = (height(CLIFF_RUN_FT), height(100.0));
    Some(json!({ "distance_ft": r1(geom::dist(p, e)), "x_ft": r1(e[0]), "y_ft": r1(e[1]), "shore": if rise >= CLIFF_RISE_FT { "cliff" } else { "beach" }, "rise_ft": r1(rise), "inland_ft": r1(inland), "in_sea": in_sea }))
}

/// Water at or beside a point within `WATER_NEAR_FT`, on the battlemap's ground: a river (id, name, width, both
/// banks, distance to its water) or a lake or the sea (name if it has one, distance to its water, a shore point),
/// whichever is nearer; None where there is none. `in_water` is the square at the point.
fn water(ex: &mut Executor, p: P, pa: &Patch, k: usize) -> Option<Value> {
    let river = crate::agent::river_at(&ex.world, &ex.t0, p, WATER_NEAR_FT).map(|r| {
        let wet = pa.river(k);
        let d = if wet { 0.0 } else { (r.dist_ft - 0.5 * r.width_ft).max(0.0) };
        let mut v = r.json(Some(p));
        v["kind"] = json!("river");
        v["in_water"] = json!(wet);
        v["distance_ft"] = json!(r1(d));
        (d, v)
    });
    let in_still = still_at(&ex.t0, pa.centre(k), pa.h[k]);
    let edge = flip(ex, p, WATER_NEAR_FT, WATER_STEP_FT, in_still, &still_at, None).map(|(d, j, pb)| (d, pb.centre(j)));
    let lake = match (in_still, edge) {
        (true, e) => Some((0.0, p, e)),
        (false, Some((d, f))) => Some((d, f, Some((d, f)))),
        (false, None) => None,
    };
    let lake = lake.map(|(d, w, e)| {
        let mut v = json!({ "kind": if sea_side(&ex.t0, w) { "sea" } else { "lake" }, "in_water": in_still, "distance_ft": r1(d) });
        if let Some((sd, f)) = e {
            v["shore"] = pt(f);
            if in_still {
                v["shore_ft"] = json!(r1(sd));
            }
        }
        if let Some((id, name)) = crate::agent::water_body_at(&ex.world, &ex.t0, w) {
            v["id"] = json!(id);
            v["name"] = json!(name);
        }
        (d, v)
    });
    match (river, lake) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a.1 } else { b.1 }),
        (a, b) => a.or(b).map(|x| x.1),
    }
}

/// `describe_location`: what is at a point, with the ground there as the battlemap draws it (elevation, under_water,
/// slope over 40 ft, the water near and the coast found on refined terrain).
pub fn describe(ex: &mut Executor, p: P) -> Value {
    let mut v = crate::agent::describe(&ex.world, &ex.t0, p[0], p[1]);
    let sea = ex.world.params().sea_level_ft as f32;
    let (pa, k) = here(ex, p);
    v["elevation_ft"] = json!(r1((pa.h[k] - sea) as f64));
    v["under_water"] = json!(pa.wet(k));
    v["slope_deg"] = json!(r1(pa.slope(k, 4) * 10.0) / 10.0);
    v["water"] = water(ex, p, &pa, k).unwrap_or(Value::Null);
    v["coast"] = coast(ex, p, COAST_NEAR_FT).unwrap_or(Value::Null);
    v
}

/// The steepest drops in a patch within `CLIFF_RUN_FT` (dry top; the foot at its ground or the water over it):
/// (the nearest to `p` of `CLIFF_RISE_FT` or more, the steepest), each (rise, top, foot).
fn drops(pa: &Patch, p: P) -> (Option<(f64, f32, usize, usize)>, Option<(f32, usize, usize)>) {
    let (mut near, mut steep): (Option<(f64, f32, usize, usize)>, Option<(f32, usize, usize)>) = (None, None);
    for k in 0..pa.h.len() {
        for (dx, dy) in [(4, 0), (0, 4), (3, 2), (2, 3), (3, -2), (2, -3)] {
            let Some(j) = pa.off(k, dx, dy) else { continue };
            let (top, foot) = if pa.h[k] >= pa.h[j] { (k, j) } else { (j, k) };
            if pa.wet(top) {
                continue;
            }
            let rise = pa.h[top] - pa.h[foot].max(pa.s.hw[foot]);
            if steep.is_none_or(|s| rise > s.0) {
                steep = Some((rise, top, foot));
            }
            let d = geom::dist(pa.centre(top), p);
            if rise as f64 >= CLIFF_RISE_FT && near.is_none_or(|n| d < n.0) {
                near = Some((d, rise, top, foot));
            }
        }
    }
    (near, steep)
}

/// Points inside hand-made plateaus (at least 100 ft from their edges) within `max_ft` of `p`, on a grid `step` ft apart:
/// at most `per` for each plateau, nearest first, with the plateau's level.
fn plateau_spots(ex: &Executor, p: P, max_ft: f64, step: f64, per: usize) -> Vec<(f64, P, f32)> {
    let mut out = Vec::new();
    for pl in ex.world.file.edits.plateaus.values() {
        let n = pl.pts.len();
        let b = pl.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
        if n < 3 || b[0] > p[0] + max_ft || b[2] < p[0] - max_ft || b[1] > p[1] + max_ft || b[3] < p[1] - max_ft {
            continue;
        }
        let mut v: Vec<(f64, P, f32)> = Vec::new();
        let (nx, ny) = (crate::core::floor((b[2] - b[0]) / step) as i64, crate::core::floor((b[3] - b[1]) / step) as i64);
        for j in 0..=ny {
            for i in 0..=nx {
                let q = [b[0] + i as f64 * step, b[1] + j as f64 * step];
                let d = geom::dist(q, p);
                if d <= max_ft && geom::contains(&pl.pts, q) && (0..n).all(|k| geom::seg_dist(q, pl.pts[k], pl.pts[(k + 1) % n]) >= 0.5 * FLAT_PATCH_FT) {
                    v.push((d, q, pl.level));
                }
            }
        }
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.extend(v.into_iter().take(per));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// The middles of hand-brushed blocks (80 ft) within `max_ft` of `p`, nearest first, with the block's greatest change (ft).
fn block_spots(ex: &Executor, p: P, max_ft: f64) -> Vec<(f64, P, f64)> {
    let side = crate::ground::BLOCK as f64 * SQUARE_FT;
    let mut v: Vec<(f64, P, f64)> = ex
        .world
        .file
        .edits
        .ground
        .iter()
        .filter_map(|(k, b)| {
            let (bx, by) = crate::ground::parse_key(k)?;
            let q = [(bx as f64 + 0.5) * side, (by as f64 + 0.5) * side];
            let d = geom::dist(q, p);
            (d <= max_ft && !b.is_zero()).then(|| (d, q, b.add.iter().map(|&a| (a as f64 * 0.1).abs()).fold(0.0, f64::max)))
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

fn cliff(ex: &mut Executor, p: P, max_ft: f64, none: Value) -> Value {
    let (c, sea) = (cell(&ex.t0), ex.world.params().sea_level_ft as f32);
    // Spots to measure: hand-made plateaus' outlines nearest the point, then the steepest samples in T0 cells steep enough.
    let mut spots: Vec<P> = Vec::new();
    for pl in ex.world.file.edits.plateaus.values() {
        let n = pl.pts.len();
        if let Some(q) = (0..n).map(|i| closest_on_seg(p, pl.pts[i], pl.pts[(i + 1) % n])).min_by(|a, b| geom::dist(*a, p).total_cmp(&geom::dist(*b, p)))
            && geom::dist(q, p) <= max_ft
        {
            spots.push(q);
        }
    }
    spots.sort_by(|a, b| geom::dist(*a, p).total_cmp(&geom::dist(*b, p)));
    spots.truncate(2);
    // Hand-brushed blocks nearest the point (raised or lowered ground can make a cliff).
    spots.extend(block_spots(ex, p, max_ft).into_iter().take(2).map(|b| b.1));
    let cells: Vec<(f64, f64, i64, i64)> = {
        let t0 = &ex.t0;
        let (ci, cj) = cell_of(t0, p);
        let rr = crate::core::ceil(max_ft / c) as i64 + 1;
        let mut v = Vec::new();
        for j in (cj - rr).max(0)..=(cj + rr).min(t0.height.h as i64 - 1) {
            for i in (ci - rr).max(0)..=(ci + rr).min(t0.height.w as i64 - 1) {
                let d = geom::dist(center(t0, i, j), p);
                if d <= max_ft + 0.7 * c && dry_cell(t0, i, j) && slope_cell(t0, i, j) >= CLIFF_CELL_DEG {
                    v.push((d, slope_cell(t0, i, j), i, j));
                }
            }
        }
        v
    };
    let mut pick: Vec<(i64, i64)> = Vec::new();
    let mut by = cells.clone();
    by.sort_by(|a, b| a.0.total_cmp(&b.0));
    pick.extend(by.iter().take(2).map(|x| (x.2, x.3)));
    by.sort_by(|a, b| b.1.total_cmp(&a.1));
    pick.extend(by.iter().filter(|x| !pick.contains(&(x.2, x.3))).take(2).map(|x| (x.2, x.3)).collect::<Vec<_>>());
    let mut lat = Lattice::new(ex, CLIFF_STEP_FT);
    let mut scanned: Vec<(f64, P)> = Vec::new();
    for (ci, cj) in pick {
        let m = center(&ex.t0, ci, cj);
        let (i0, j0) = lat.index([m[0] - 0.5 * c, m[1] - 0.5 * c]);
        let (i1, j1) = lat.index([m[0] + 0.5 * c, m[1] + 0.5 * c]);
        let (mut nearest, mut steepest): (Option<(f64, P)>, Option<(f32, P)>) = (None, None);
        for j in j0..j1.min(lat.last.1) {
            for i in i0..i1.min(lat.last.0) {
                let q = lat.pos(i, j);
                let d = geom::dist(q, p);
                if d > max_ft {
                    continue;
                }
                let h = lat.ground(ex, i, j);
                let rise = (h - lat.ground(ex, i + 1, j)).abs().max((h - lat.ground(ex, i, j + 1)).abs());
                if steepest.is_none_or(|s| rise > s.0) {
                    steepest = Some((rise, q));
                }
                if rise >= CLIFF_SCAN_FT && nearest.is_none_or(|n| d < n.0) {
                    nearest = Some((d, q));
                }
            }
        }
        scanned.extend(nearest);
        scanned.extend(steepest.filter(|s| nearest.is_none_or(|n| n.1 != s.1)).map(|s| (geom::dist(s.1, p), s.1)));
    }
    scanned.sort_by(|a, b| a.0.total_cmp(&b.0));
    spots.extend(scanned.into_iter().map(|s| s.1).take(6));
    let measured = spots.len();
    // Measured on squares: the nearest cliff found, else the steepest ground.
    let (mut best, mut steep): (Option<(f64, f32, P, P, f32)>, Option<(f32, P)>) = (None, None);
    for q in spots {
        let pa = Patch::read(ex, q, 2.0 * CLIFF_STEP_FT);
        let (near, st) = drops(&pa, p);
        if let Some((d, rise, top, foot)) = near.filter(|n| n.0 <= max_ft)
            && best.is_none_or(|b| d < b.0)
        {
            best = Some((d, rise, pa.centre(top), pa.centre(foot), pa.h[top] - sea));
        }
        if let Some((rise, top, _)) = st
            && steep.is_none_or(|s| rise > s.0)
        {
            steep = Some((rise, pa.centre(top)));
        }
    }
    match best {
        Some((d, rise, top, foot, h)) => {
            let mut v = base("cliff", p, top, d);
            let run = geom::dist(top, foot);
            v["drop_ft"] = json!(r1(rise as f64 * 10.0) / 10.0);
            v["run_ft"] = json!(r1(run));
            v["slope_deg"] = json!(r1(libm::atan2(rise as f64, run).to_degrees()));
            v["top_ft"] = json!(r1(h as f64));
            v["foot"] = pt(foot);
            v
        }
        None => {
            let mut v = none;
            let within = max_ft / MI;
            v["note"] = json!(match steep {
                Some((rise, at)) => {
                    v["steepest"] = json!({ "x_ft": r1(at[0]), "y_ft": r1(at[1]), "rise_ft": r1(rise as f64 * 10.0) / 10.0, "distance_mi": r1((geom::dist(at, p) / MI) * 100.0) / 100.0 });
                    format!("no cliff ({CLIFF_RISE_FT} ft within {CLIFF_RUN_FT} ft) among the {measured} steep spots measured within {within} mi (the search is not exhaustive); the steepest ground found is {:.1} ft in {CLIFF_RUN_FT} ft at ({}, {})", rise, r1(at[0]), r1(at[1]))
                }
                None => format!("no cliff ({CLIFF_RISE_FT} ft within {CLIFF_RUN_FT} ft) among the {measured} steep spots measured within {within} mi: no ground there is steeper than {CLIFF_CELL_DEG} degrees over a mile to measure (the search is not exhaustive)"),
            });
            v
        }
    }
}

fn flat(ex: &mut Executor, p: P, max_ft: f64, none: Value) -> Value {
    let sea = ex.world.params().sea_level_ft as f32;
    // Candidates on T0: gentle points in and round the nearest flat cells (their patch reaches a cell and a half from each).
    let spots: Vec<(f64, P)> = {
        let t0 = &ex.t0;
        let c = cell(t0);
        let gentle = |q: P| slope_deg(t0, q) < FLAT_DEG && !wet(t0, q);
        let mut v: Vec<(f64, P)> = Vec::new();
        if flat_cell(t0, cell_of(t0, p).0, cell_of(t0, p).1) && gentle(p) {
            v.push((0.0, p));
        }
        for (_, i, j) in nearest_cells(t0, p, max_ft, &|i, j| flat_cell(t0, i, j)).into_iter().take(4) {
            let m = center(t0, i, j);
            for a in -6..=6 {
                for b in -6..=6 {
                    let q = [m[0] + a as f64 * 0.25 * c, m[1] + b as f64 * 0.25 * c];
                    let d = geom::dist(q, p);
                    if d <= max_ft && d > 0.0 && gentle(q) {
                        v.push((d, q));
                    }
                }
            }
        }
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.dedup_by(|a, b| geom::dist(a.1, b.1) < 1.0);
        v
    };
    // Hand-shaped ground first: points on plateau tops and middles of brushed blocks (levelled ground).
    let mut hand: Vec<(f64, P)> = plateau_spots(ex, p, max_ft, 40.0, 3).into_iter().map(|s| (s.0, s.1)).collect();
    hand.extend(block_spots(ex, p, max_ft).into_iter().take(3).map(|b| (b.0, b.1)));
    hand.sort_by(|a, b| a.0.total_cmp(&b.0));
    let spots: Vec<(f64, P)> = hand.into_iter().take(6).chain(spots).collect();
    let mut roughest: Option<f32> = None;
    let (mut measured, mut wet_n) = (0, 0);
    let whole = (FLAT_PATCH_FT / SQUARE_FT) as i64 + 1;
    for &(d, q) in spots.iter().take(FLAT_TRIES) {
        let pa = Patch::read(ex, q, 0.5 * FLAT_PATCH_FT);
        // The whole patch must be on the map.
        if pa.s.w != whole || pa.s.h != whole {
            continue;
        }
        measured += 1;
        if (0..pa.h.len()).any(|k| pa.wet(k)) {
            wet_n += 1;
            continue;
        }
        let (lo, hi) = pa.h.iter().fold((f32::MAX, f32::MIN), |m, &h| (m.0.min(h), m.1.max(h)));
        if hi - lo > FLAT_RANGE_FT {
            roughest = Some(roughest.map_or(hi - lo, |r: f32| r.min(hi - lo)));
            continue;
        }
        let mut v = base("flat", p, q, d);
        v["slope_deg"] = json!(r1(pa.plane_slope() * 10.0) / 10.0);
        v["elevation_ft"] = json!(r1((pa.h[pa.k_of(q)] - sea) as f64));
        v["range_ft"] = json!(r1((hi - lo) as f64 * 10.0) / 10.0);
        v["patch_ft"] = json!(FLAT_PATCH_FT);
        return v;
    }
    let mut v = none;
    let within = max_ft / MI;
    v["note"] = json!(match roughest {
        Some(r) => format!("no flat among the {measured} spots measured within {within} mi (the search is not exhaustive): the nearest gentle ground (by the coarse map) is rougher on the battlemap, at best {:.1} ft over {FLAT_PATCH_FT} ft (flat is at most {FLAT_RANGE_FT})", r),
        None if measured > 0 => format!("no flat among the {measured} spots measured within {within} mi (the search is not exhaustive): {wet_n} had water in the {FLAT_PATCH_FT}-ft patch"),
        None => format!("no ground under {FLAT_DEG} degrees within {within} mi to measure (a {FLAT_PATCH_FT}-ft patch on the map, dry, found on the coarse map)"),
    });
    v
}

/// The highest ground within `max_ft` of `p`: the T0 cells highest in reach are scanned on 80-ft samples, the highest
/// samples (and hand-shaped ground: plateau tops, brushed blocks, the point itself) measured on squares.
fn highest(ex: &mut Executor, p: P, max_ft: f64) -> Value {
    const CELLS: usize = 10;
    const SAMPLES: usize = 6;
    let (c, sea) = (cell(&ex.t0), ex.world.params().sea_level_ft as f32);
    let cells: Vec<(i64, i64)> = {
        let t0 = &ex.t0;
        let (ci, cj) = cell_of(t0, p);
        let rr = crate::core::ceil(max_ft / c) as i64 + 1;
        let mut v: Vec<(f64, i64, i64)> = Vec::new();
        for j in (cj - rr).max(0)..=(cj + rr).min(t0.height.h as i64 - 1) {
            for i in (ci - rr).max(0)..=(ci + rr).min(t0.height.w as i64 - 1) {
                if geom::dist(center(t0, i, j), p) <= max_ft + 0.7 * c {
                    v.push((h_at(t0, i, j), i, j));
                }
            }
        }
        v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        v.into_iter().take(CELLS).map(|x| (x.1, x.2)).collect()
    };
    let mut lat = Lattice::new(ex, COAST_STEP_FT);
    let mut samples: Vec<(f32, P)> = Vec::new();
    for (ci, cj) in cells {
        let m = center(&ex.t0, ci, cj);
        let (i0, j0) = lat.index([m[0] - 0.5 * c, m[1] - 0.5 * c]);
        let (i1, j1) = lat.index([m[0] + 0.5 * c, m[1] + 0.5 * c]);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let q = lat.pos(i, j);
                if geom::dist(q, p) <= max_ft {
                    let h = lat.ground(ex, i, j);
                    samples.push((h, q));
                }
            }
        }
    }
    // Highest first (ties by position), thinned to those at least 2 steps apart.
    samples.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1[0].total_cmp(&b.1[0])).then(a.1[1].total_cmp(&b.1[1])));
    samples.dedup_by(|a, b| a.1 == b.1);
    let mut spots: Vec<P> = Vec::new();
    for s in &samples {
        if spots.len() < SAMPLES && spots.iter().all(|&q| geom::dist(q, s.1) >= 2.0 * lat.step) {
            spots.push(s.1);
        }
    }
    spots.push(p);
    spots.extend(plateau_spots(ex, p, max_ft, 40.0, 1).into_iter().map(|s| s.1));
    spots.extend(block_spots(ex, p, max_ft).into_iter().take(3).map(|b| b.1));
    let (pa, k) = here(ex, p);
    let at_h = pa.h[k];
    let mut best: Option<(f32, P)> = None;
    for q in spots {
        let pa = Patch::read(ex, q, 1.5 * lat.step + 10.0);
        for j in 0..pa.h.len() {
            let ctr = pa.centre(j);
            if !pa.wet(j) && geom::dist(ctr, p) <= max_ft && best.is_none_or(|b| pa.h[j] > b.0) {
                best = Some((pa.h[j], ctr));
            }
        }
    }
    match best {
        Some((h, at)) => {
            let mut v = base("highest", p, at, geom::dist(p, at));
            v["elevation_ft"] = json!(r1((h - sea) as f64 * 10.0) / 10.0);
            v["rise_ft"] = json!(r1((h - at_h).max(0.0) as f64 * 10.0) / 10.0);
            v
        }
        None => json!({ "what": "highest", "found": false, "within_mi": max_ft / MI, "note": "no dry ground within reach" }),
    }
}

/// The nearest point of a road's drawn surface (the finest tiles' carved road) within `reach_ft` of `p`, with its
/// distance; None where none is, or the tiles there carry no road mask.
pub fn road_surface(ex: &mut Executor, p: P, reach_ft: f64) -> Option<(f64, P)> {
    let mut lat = Lattice::new(ex, 0.0);
    ring(ex, &mut lat, p, reach_ft, &mut |ex, lat, i, j| lat.road(ex, i, j) == Some(true))
}

// ---------------------------------------------------------------------------------------
// Roads.

fn road_dist(t0: &T0, p: P, max_ft: f64) -> Option<(f64, usize, usize, f64, P)> {
    let net = &t0.roads;
    let mut best: Option<(f64, usize, usize, f64, P)> = None;
    for (ri, k) in net.segments_near([p[0] - max_ft, p[1] - max_ft, p[0] + max_ft, p[1] + max_ft], 0.0) {
        let r = &net.roads[ri as usize];
        let (ri, k) = (ri as usize, k as usize);
        let n = ((r.s[k + 1] - r.s[k]) / 250.0).ceil().clamp(2.0, 60.0) as usize;
        for j in 0..=n {
            let t = j as f64 / n as f64;
            let q = r.eval(k, t, 5.0, cell(t0)).p;
            let d = geom::dist(q, p);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, ri, k, t, q));
            }
        }
    }
    // Finer about the best sample, twice.
    let (mut d, ri, k, mut t, mut q) = best?;
    let r = &net.roads[ri];
    let n0 = ((r.s[k + 1] - r.s[k]) / 250.0).ceil().clamp(2.0, 60.0);
    let mut span = 1.0 / n0;
    for _ in 0..2 {
        let c = t;
        for j in -10..=10 {
            let tt = (c + span * j as f64 / 10.0).clamp(0.0, 1.0);
            let qq = r.eval(k, tt, 5.0, cell(t0)).p;
            let dd = geom::dist(qq, p);
            if dd < d {
                (d, t, q) = (dd, tt, qq);
            }
        }
        span *= 0.1;
    }
    (d <= max_ft).then_some((d, ri, k, t, q))
}

fn heading(a: P, b: P) -> f64 {
    // Clockwise from north (screen y points south).
    (libm::atan2(b[0] - a[0], -(b[1] - a[1])).to_degrees() + 360.0) % 360.0
}

// ---------------------------------------------------------------------------------------
// Nearest.

/// The nearest `what` (coast | cliff | river | road | flat) to `p` within `max_ft`: `{what, found, x_ft, y_ft,
/// distance_ft, ...}`; `found: false` where none. See `docs/MCP.md` "nearest" for what each reports.
pub fn nearest(ex: &mut Executor, p: P, what: &str, max_ft: f64) -> Result<Value, String> {
    let none = || json!({ "what": what, "found": false, "within_mi": max_ft / MI });
    let (world, t0) = match what {
        "coast" => {
            return Ok(match coast(ex, p, max_ft) {
                Some(c) => {
                    let at = [c["x_ft"].as_f64().unwrap_or(0.0), c["y_ft"].as_f64().unwrap_or(0.0)];
                    let mut v = base("coast", p, at, c["distance_ft"].as_f64().unwrap_or(0.0));
                    for k in ["shore", "rise_ft", "inland_ft", "in_sea"] {
                        v[k] = c[k].clone();
                    }
                    v
                }
                None => none(),
            });
        }
        "cliff" => return Ok(cliff(ex, p, max_ft, none())),
        "flat" => return Ok(flat(ex, p, max_ft, none())),
        "highest" => return Ok(highest(ex, p, max_ft)),
        _ => (&ex.world, &ex.t0),
    };
    Ok(match what {
        "river" => match crate::agent::river_at(world, t0, p, max_ft) {
            Some(r) => {
                let d = (r.dist_ft - 0.5 * r.width_ft).max(0.0);
                let (bank, _) = r.bank_of(p, 0.0);
                let at = if r.in_water() { p } else { bank };
                let mut v = base("river", p, at, d);
                v["id"] = json!(r.id);
                v["name"] = json!(r.name);
                v["width_ft"] = json!(r1(r.width_ft));
                v["in_water"] = json!(r.in_water());
                v["banks"] = r.json(Some(p))["banks"].clone();
                v
            }
            None => none(),
        },
        "road" => match road_dist(t0, p, max_ft) {
            Some((d, ri, k, t, q)) => {
                let ids = crate::roadids::road_ids(t0);
                let fs = crate::agent::features(world, t0);
                let r = &t0.roads.roads[ri];
                let mut v = base("road", p, q, d);
                v["id"] = json!(ids[ri]);
                v["class"] = json!(crate::roadids::class_name(r.class));
                if let Some(n) = crate::roadids::stretch_name(world, t0, &fs, &ids[ri], r) {
                    v["name"] = json!(n);
                }
                let a = r.eval(k, (t - 0.01).max(0.0), 5.0, cell(t0)).p;
                let b = r.eval(k, (t + 0.01).min(1.0), 5.0, cell(t0)).p;
                v["heading_deg"] = json!(r1(heading(a, b)));
                v
            }
            None => none(),
        },
        _ => return Err(format!("what: \"{what}\" is not one of coast, cliff, river, road, flat, highest")),
    })
}

// ---------------------------------------------------------------------------------------
// Points along a road.

/// A road as one way along it: stretches in order, each walked one way.
pub struct Way {
    pub name: Option<String>,
    pub pieces: Vec<(usize, bool)>,
}

impl Way {
    fn len(t0: &T0, ri: usize) -> f64 {
        *t0.roads.roads[ri].s.last().unwrap_or(&0.0)
    }

    pub fn length_ft(&self, t0: &T0) -> f64 {
        self.pieces.iter().map(|&(ri, _)| Self::len(t0, ri)).sum()
    }

    /// The end points: where the way starts and ends.
    pub fn ends(&self, t0: &T0) -> (P, P) {
        let end = |&(ri, fwd): &(usize, bool), last: bool| {
            let r = &t0.roads.roads[ri];
            if fwd == last { *r.pts.last().unwrap() } else { r.pts[0] }
        };
        (end(&self.pieces[0], false), end(self.pieces.last().unwrap(), true))
    }

    /// The same way walked the other way.
    pub fn reversed(&self) -> Way {
        Way { name: self.name.clone(), pieces: self.pieces.iter().rev().map(|&(ri, f)| (ri, !f)).collect() }
    }
}

/// The way for a road id (`rd:`) or a road's name (renamed stretches walked end to end, or drawn
/// in the sketch); several roads of one name are refused, listing a stretch of each.
pub fn road_way(world: &World, t0: &T0, spec: &str) -> Result<Way, String> {
    if spec.starts_with("rd:") {
        let ri = crate::roadids::road_index(t0, spec).ok_or_else(|| format!("no road stretch {spec} (list_roads gives the ids)"))?;
        if t0.roads.roads[ri].pts.len() < 2 {
            return Err(format!("road: the stretch {spec} has no length"));
        }
        let name = crate::roadids::stretch_name(world, t0, &crate::agent::features(world, t0), spec, &t0.roads.roads[ri]);
        return Ok(Way { name, pieces: vec![(ri, true)] });
    }
    let mut chains = crate::roadids::chains_named(world, t0, spec);
    match chains.len() {
        0 => {
            let fs = crate::agent::features(world, t0);
            let mut names: Vec<String> = fs.iter().filter(|f| f.kind == "road").map(|f| f.name.clone()).collect();
            names.extend(world.file.edits.renames.iter().filter(|(k, _)| k.starts_with("rd:")).map(|(_, n)| n.clone()));
            names.sort();
            names.dedup();
            let hint = suggest(spec, &names).map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default();
            Err(format!("road: no road named \"{spec}\"{hint}; give an rd: id (list_roads) or the name of a drawn or renamed road{}", if names.is_empty() { "" } else { " (names: see list_roads named)" }))
        }
        1 => {
            let c = chains.pop().unwrap();
            Ok(Way { name: Some(c.name), pieces: c.idx.into_iter().zip(c.fwd).collect() })
        }
        n => {
            let ids = crate::roadids::road_ids(t0);
            let list: Vec<String> = chains.iter().take(5).map(|c| format!("{} ({:.1} mi)", ids[c.idx[0]], c.idx.iter().map(|&i| Way::len(t0, i)).sum::<f64>() / MI)).collect();
            Err(format!("road: {n} separate roads are named \"{spec}\": {}; give one of those rd: ids", list.join(", ")))
        }
    }
}

fn suggest<'a>(word: &str, names: &'a [String]) -> Option<&'a str> {
    let w = word.to_lowercase();
    // A name it starts (or that starts it), else one at most 2 letters different.
    let near = |n: &String| {
        let n = n.to_lowercase();
        let prefix = w.len() >= 3 && (n.starts_with(&w) || (n.len() >= 3 && w.starts_with(&n)));
        if prefix { Some(0) } else { Some(edits(&w, &n)).filter(|&d| d <= 2) }
    };
    names.iter().filter_map(|n| near(n).map(|d| (d, n))).min_by_key(|x| x.0).map(|x| x.1.as_str())
}

/// Letters added, taken away or changed between `a` and `b`.
fn edits(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The way a route between two points takes along roads (None where it goes overland).
pub fn route_way(world: &World, t0: &T0, from: P, to: P) -> Result<Way, String> {
    match crate::agent::route_stretches(world, t0, from, to) {
        Some(p) if !p.is_empty() => Ok(Way { name: None, pieces: p }),
        _ => Err("no road route between them (route's by_road is null): the way is overland".into()),
    }
}

/// The point `d` ft along a way from its start: `{x_ft, y_ft, heading_deg, stretch}` (stretch: id, name, class, ends);
/// None past its length.
pub fn point_along(world: &World, t0: &T0, way: &Way, d: f64) -> Option<Value> {
    let mut left = d;
    for (n, &(ri, fwd)) in way.pieces.iter().enumerate() {
        let len = Way::len(t0, ri);
        if left > len && n + 1 < way.pieces.len() {
            left -= len;
            continue;
        }
        if left > len + 0.5 {
            return None;
        }
        let r: &RoadCurve = &t0.roads.roads[ri];
        let s = if fwd { left.min(len) } else { (len - left).max(0.0) };
        let k = r.s.partition_point(|&v| v < s).clamp(1, r.s.len() - 1) - 1;
        let t = ((s - r.s[k]) / (r.s[k + 1] - r.s[k]).max(1e-9)).clamp(0.0, 1.0);
        let c = cell(t0);
        let at = r.eval(k, t, 5.0, c).p;
        let e = 0.5 / (r.s[k + 1] - r.s[k]).max(1.0);
        let (a, b) = (r.eval(k, (t - e).max(0.0), 5.0, c).p, r.eval(k, (t + e).min(1.0), 5.0, c).p);
        let hd = heading(a, b);
        let hd = if fwd { hd } else { (hd + 180.0) % 360.0 };
        let ids = crate::roadids::road_ids(t0);
        let fs = crate::agent::features(world, t0);
        let mut st = json!({ "id": ids[ri], "class": crate::roadids::class_name(r.class), "from": crate::roadids::end_json(world, t0, r.pts[0]), "to": crate::roadids::end_json(world, t0, *r.pts.last().unwrap()), "length_mi": r1((len / MI) * 100.0) / 100.0 });
        if let Some(nm) = crate::roadids::stretch_name(world, t0, &fs, &ids[ri], r) {
            st["name"] = json!(nm);
        }
        return Some(json!({ "x_ft": r1(at[0]), "y_ft": r1(at[1]), "heading_deg": r1(hd), "stretch": st }));
    }
    None
}

// ---------------------------------------------------------------------------------------
// Places by name.

/// Places called `name` now (case-insensitive, the whole name: features, districts, buildings, renamed
/// roads and crossings; hidden ones left out) and, apart from them, up to 5 whose names only hold it:
/// each `{id, kind, name, x_ft, y_ft, ...}` as search gives it.
pub fn places_named(world: &World, t0: &T0, name: &str) -> (Vec<Value>, Vec<Value>) {
    let q = name.trim().to_lowercase();
    let all = crate::agent::search_with(world, t0, &crate::agent::SearchOpts { query: name.to_string(), ..Default::default() }, 100_000);
    let (mut exact, mut close) = (Vec::new(), Vec::new());
    for v in all["results"].as_array().into_iter().flatten() {
        if v["name"].as_str().is_some_and(|n| n.trim().to_lowercase() == q) {
            exact.push(v.clone());
        } else if close.len() < 5 {
            close.push(v.clone());
        }
    }
    (exact, close)
}
