//! Geometry queries for agents (`mapd`'s `nearest`, `road_point`, `search_features within` and
//! `describe_location`'s `water`, `coast` and `slope_deg`): areas, the nearest coast, cliff, river,
//! road or flat ground to a point, and points along a road.
//!
//! Terrain is read from the T0 grids (one cell is `t0.cell_ft`, about 1,500 ft on a 1,200-mile map),
//! so a cliff or beach narrower than a cell reads gentler than it stands on a battlemap. Searches
//! walk rings of cells out from the point and stop once nothing nearer can exist: never the whole map.
//! Thresholds are defined once, here (and in docs/MCP.md "nearest").

use serde_json::{Value, json};

use crate::World;
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
/// A cliff: ground steeper than this (degrees, over one T0 cell, which is coarse: on a 1,200-mile map 8 degrees is the steepest few percent of land).
pub const CLIFF_DEG: f64 = 8.0;
/// Flat ground: gentler than this (degrees) over the cell and the four beside it.
pub const FLAT_DEG: f64 = 5.0;
/// A cliff coast: the land rises this much (ft) within one T0 cell of the shore; less is a beach.
pub const COAST_CLIFF_RISE_FT: f64 = 300.0;

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
    (((p[0] / cell(t0)).round() as i64).clamp(0, t0.height.w as i64 - 1), ((p[1] / cell(t0)).round() as i64).clamp(0, t0.height.h as i64 - 1))
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

/// The nearest cell (within `max_ft`) satisfying `hit` and, refined, the nearest point of it satisfying `at_point`.
fn nearest_spot(t0: &T0, p: P, max_ft: f64, hit: &dyn Fn(i64, i64) -> bool, at_point: &dyn Fn(P) -> bool) -> Option<(f64, P)> {
    let c = cell(t0);
    if hit(cell_of(t0, p).0, cell_of(t0, p).1) && at_point(p) {
        return Some((0.0, p));
    }
    let mut best: Option<(f64, P)> = None;
    for (_, i, j) in nearest_cells(t0, p, max_ft, hit).into_iter().take(4) {
        let m = center(t0, i, j);
        let mut here = (geom::dist(m, p), m);
        for a in -2..=2 {
            for b in -2..=2 {
                let q = [m[0] + a as f64 * 0.25 * c, m[1] + b as f64 * 0.25 * c];
                let d = geom::dist(q, p);
                if d < here.0 && at_point(q) {
                    here = (d, q);
                }
            }
        }
        if here.0 <= max_ft && best.is_none_or(|b| here.0 < b.0) {
            best = Some(here);
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
    let mut v = json!({ "what": what, "found": true, "x_ft": r1(at[0]), "y_ft": r1(at[1]), "distance_ft": r1(d), "distance_mi": ((d / MI) * 100.0).round() / 100.0 });
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

/// The sea's coast nearest `p` within `max_ft`: `{distance_ft, x_ft, y_ft, shore: cliff | beach, rise_ft, in_sea}`.
/// The shore is a cliff when the land rises `COAST_CLIFF_RISE_FT` or more within one T0 cell of it.
pub fn coast(t0: &T0, p: P, max_ft: f64) -> Option<Value> {
    let state = |q: P| ocean(t0, q);
    let (d, f, dir) = nearest_flip(t0, p, max_ft, &state)?;
    let in_sea = state(p);
    // Inland from the shore: back toward p when p is on land, on through the shore when p is at sea.
    let land = if in_sea { dir } else { [-dir[0], -dir[1]] };
    let c = cell(t0);
    let rise = [0.5, 1.0].iter().map(|k| t0.sample(f[0] + land[0] * k * c, f[1] + land[1] * k * c, c) - t0.sample(f[0], f[1], c)).fold(f64::MIN, f64::max).max(0.0);
    Some(json!({ "distance_ft": r1(d), "x_ft": r1(f[0]), "y_ft": r1(f[1]), "shore": if rise >= COAST_CLIFF_RISE_FT { "cliff" } else { "beach" }, "rise_ft": r1(rise), "in_sea": in_sea }))
}

/// Water at or beside a point within `WATER_NEAR_FT`: a river (id, name, width, both banks, distance
/// to its water) or a lake or the sea (name if it has one, distance to its water, a shore point),
/// whichever is nearer; None where there is none. `named` gives the name and id of a water body at a point.
pub fn water(world: &World, t0: &T0, p: P, named: &dyn Fn(P) -> Option<(String, String)>) -> Option<Value> {
    let river = crate::agent::river_at(world, t0, p, WATER_NEAR_FT).map(|r| {
        let d = (r.dist_ft - 0.5 * r.width_ft).max(0.0);
        let mut v = r.json(Some(p));
        v["kind"] = json!("river");
        v["in_water"] = json!(r.in_water());
        v["distance_ft"] = json!(r1(d));
        (d, v)
    });
    let state = |q: P| wet(t0, q);
    // (Distance to the water or, in it, to its shore if that is near; the shore point if there is one.)
    let lake = nearest_flip(t0, p, WATER_NEAR_FT, &state).map(|(d, f, _)| (d, Some(f))).or_else(|| state(p).then_some((0.0, None)));
    let lake = lake.map(|(d, f)| {
        let in_water = state(p);
        // The water's own point: p when in it, else the shore found.
        let w = if in_water { p } else { f.unwrap_or(p) };
        let sea = ocean(t0, w);
        let mut v = json!({ "kind": if sea { "sea" } else { "lake" }, "in_water": in_water, "distance_ft": if in_water { 0.0 } else { r1(d) } });
        if let Some(f) = f {
            v["shore"] = pt(f);
            if in_water {
                v["shore_ft"] = json!(r1(d));
            }
        }
        if let Some((id, name)) = named(w) {
            v["id"] = json!(id);
            v["name"] = json!(name);
        }
        (if in_water { 0.0 } else { d }, v)
    });
    match (river, lake) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a.1 } else { b.1 }),
        (Some(a), None) => Some(a.1),
        (None, Some(b)) => Some(b.1),
        (None, None) => None,
    }
}

fn cliff_cell(t0: &T0, i: i64, j: i64) -> bool {
    dry_cell(t0, i, j) && slope_cell(t0, i, j) >= CLIFF_DEG
}

fn flat_cell(t0: &T0, i: i64, j: i64) -> bool {
    [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(a, b)| dry_cell(t0, i + a, j + b) && h_at(t0, i + a, j + b) > -1e5 && slope_cell(t0, i + a, j + b) < FLAT_DEG)
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
pub fn nearest(world: &World, t0: &T0, p: P, what: &str, max_ft: f64) -> Result<Value, String> {
    let none = || json!({ "what": what, "found": false, "within_mi": max_ft / MI });
    Ok(match what {
        "coast" => match coast(t0, p, max_ft) {
            Some(c) => {
                let at = [c["x_ft"].as_f64().unwrap_or(0.0), c["y_ft"].as_f64().unwrap_or(0.0)];
                let mut v = base("coast", p, at, c["distance_ft"].as_f64().unwrap_or(0.0));
                v["shore"] = c["shore"].clone();
                v["rise_ft"] = c["rise_ft"].clone();
                v["in_sea"] = c["in_sea"].clone();
                v
            }
            None => none(),
        },
        "cliff" => match nearest_spot(t0, p, max_ft, &|i, j| cliff_cell(t0, i, j), &|q| slope_deg(t0, q) >= CLIFF_DEG && !wet(t0, q)) {
            Some((d, q)) => {
                let c = cell(t0);
                let (hi, lo) = [(c, 0.0), (-c, 0.0), (0.0, c), (0.0, -c)].iter().map(|&(a, b)| t0.sample(q[0] + a, q[1] + b, c)).fold((f64::MIN, f64::MAX), |m, h| (m.0.max(h), m.1.min(h)));
                let mut v = base("cliff", p, q, d);
                v["slope_deg"] = json!(r1(slope_deg(t0, q)));
                v["drop_ft"] = json!(r1(hi - lo));
                v
            }
            None => none(),
        },
        "flat" => match nearest_spot(t0, p, max_ft, &|i, j| flat_cell(t0, i, j), &|q| slope_deg(t0, q) < FLAT_DEG && !wet(t0, q)) {
            Some((d, q)) => {
                let mut v = base("flat", p, q, d);
                v["slope_deg"] = json!(r1(slope_deg(t0, q) * 10.0) / 10.0);
                v["elevation_ft"] = json!(r1(t0.sample(q[0], q[1], cell(t0)) - world.params().sea_level_ft));
                v["patch_ft"] = json!(r1(3.0 * cell(t0)));
                v
            }
            None => none(),
        },
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
        _ => return Err(format!("what: \"{what}\" is not one of coast, cliff, river, road, flat")),
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
    names.iter().find(|n| n.to_lowercase().starts_with(&w) || w.starts_with(&n.to_lowercase())).map(String::as_str)
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
        let mut st = json!({ "id": ids[ri], "class": crate::roadids::class_name(r.class), "from": crate::roadids::end_json(world, t0, r.pts[0]), "to": crate::roadids::end_json(world, t0, *r.pts.last().unwrap()), "length_mi": ((len / MI) * 100.0).round() / 100.0 });
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
