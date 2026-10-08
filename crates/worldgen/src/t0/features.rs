//! Named features (the T0 part of the gazetteer): continent, islands, oceans and seas, bays,
//! mountain ranges, peaks (by topographic prominence), passes (their key cols), volcanoes,
//! lakes, rivers and biome regions. Each gets a stable id, a name, and label placement
//! (anchor at the pole of inaccessibility, angle along the principal axis, size in feet).

use serde::Serialize;

use super::biome::Biome;
use super::climate::{Climate, distance_to};
use super::flood::{D8, neighbors};
use super::hydro::{Hydro, LakeKind, Mouth};
use super::names::{NameKind, Namer};
use super::settle::{Poi, PoiKind, Settlement, Tier};
use super::volcano::{Activity, Volcano, VolcanoKind};
use crate::World;
use crate::core::rng::{Pcg32, hash2};

#[derive(Clone, Debug, Serialize)]
pub struct Feature {
    pub id: String,
    pub kind: &'static str,
    pub name: String,
    /// Label anchor, ft.
    pub x: f64,
    pub y: f64,
    /// Label angle, radians (0 = horizontal, clockwise positive since y points down).
    pub angle: f64,
    /// Characteristic size, ft: decides the zoom band where the label shows.
    pub extent_ft: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elev_ft: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Overlay {
    pub features: Vec<Feature>,
    /// Naming culture of each settlement (same order as the settlements).
    #[serde(skip)]
    pub settlement_cultures: Vec<u8>,
    /// What the sketch asked for that the world could not follow exactly.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<super::sketch::Conflict>,
    /// Where each feature lies (same order as the features), for agents' "what is near".
    #[serde(skip)]
    pub shapes: Vec<Shape>,
}

/// The ground a feature covers: an area's cells, a river's course, or a point and its radius.
/// Kept beside the overlay (never serialized), so nothing generated depends on it.
#[derive(Clone, Debug)]
pub enum Shape {
    Point { at: [f64; 2], radius_ft: f64 },
    /// A course (ft).
    Line(Vec<[f32; 2]>),
    /// Cells of a grid `cell_ft` apart (cell (i, j) centred at (i, j)·cell_ft): a bit per cell of
    /// the box `x0, y0, w, h` (cells), and the cells on its edge.
    Area { cell_ft: f64, x0: u32, y0: u32, w: u32, h: u32, bits: Vec<u64>, edge: Vec<u32>, cells: u32 },
}

impl Shape {
    fn area(gw: usize, cell_ft: f64, comp: &[u32]) -> Shape {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for &k in comp {
            let (x, y) = (k % gw as u32, k / gw as u32);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut bits = vec![0u64; (w as usize * h as usize).div_ceil(64)];
        for &k in comp {
            let b = ((k / gw as u32 - y0) * w + k % gw as u32 - x0) as usize;
            bits[b / 64] |= 1 << (b % 64);
        }
        let mut s = Shape::Area { cell_ft, x0, y0, w, h, bits, edge: Vec::new(), cells: comp.len() as u32 };
        let edge: Vec<u32> = comp
            .iter()
            .map(|&k| (k / gw as u32 - y0) * w + k % gw as u32 - x0)
            .filter(|&b| {
                let (i, j) = ((b % w) as i64, (b / w) as i64);
                [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| !s.has(i + dx, j + dy))
            })
            .collect();
        if let Shape::Area { edge: e, .. } = &mut s {
            *e = edge;
        }
        s
    }

    /// Whether box cell (i, j) is part of the area.
    fn has(&self, i: i64, j: i64) -> bool {
        match self {
            Shape::Area { w, h, bits, .. } => {
                if i < 0 || j < 0 || i >= *w as i64 || j >= *h as i64 {
                    return false;
                }
                let b = (j * *w as i64 + i) as usize;
                (bits[b / 64] >> (b % 64)) & 1 == 1
            }
            _ => false,
        }
    }

    /// Bounding box (ft): x0, y0, x1, y1.
    pub fn bbox(&self) -> [f64; 4] {
        match self {
            Shape::Point { at, radius_ft: r } => [at[0] - r, at[1] - r, at[0] + r, at[1] + r],
            Shape::Line(pts) => pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
                [b[0].min(p[0] as f64), b[1].min(p[1] as f64), b[2].max(p[0] as f64), b[3].max(p[1] as f64)]
            }),
            Shape::Area { cell_ft: c, x0, y0, w, h, .. } => {
                [(*x0 as f64 - 0.5) * c, (*y0 as f64 - 0.5) * c, ((x0 + w) as f64 - 0.5) * c, ((y0 + h) as f64 - 0.5) * c]
            }
        }
    }

    /// Area (sq ft) of an area or a point's disc; 0 for a course.
    pub fn area_sq_ft(&self) -> f64 {
        match self {
            Shape::Point { radius_ft: r, .. } => std::f64::consts::PI * r * r,
            Shape::Line(_) => 0.0,
            Shape::Area { cell_ft: c, cells, .. } => *cells as f64 * c * c,
        }
    }

    /// A course's length (ft); 0 otherwise.
    pub fn length_ft(&self) -> f64 {
        match self {
            Shape::Line(pts) => pts.windows(2).map(|s| f64::hypot((s[1][0] - s[0][0]) as f64, (s[1][1] - s[0][1]) as f64)).sum(),
            _ => 0.0,
        }
    }

    /// Distance (ft) from `p` to the shape (0 inside an area or a point's radius), and the
    /// nearest point of it.
    pub fn distance(&self, p: [f64; 2]) -> (f64, [f64; 2]) {
        match self {
            Shape::Point { at, radius_ft: r } => ((f64::hypot(p[0] - at[0], p[1] - at[1]) - r).max(0.0), *at),
            Shape::Line(pts) => {
                let mut best = (f64::MAX, p);
                for s in pts.windows(2) {
                    let (a, b) = ([s[0][0] as f64, s[0][1] as f64], [s[1][0] as f64, s[1][1] as f64]);
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let len2 = dx * dx + dy * dy;
                    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                    let q = [a[0] + t * dx, a[1] + t * dy];
                    let d = f64::hypot(p[0] - q[0], p[1] - q[1]);
                    if d < best.0 {
                        best = (d, q);
                    }
                }
                best
            }
            Shape::Area { cell_ft: c, x0, y0, w, edge, .. } => {
                let (i, j) = ((p[0] / c).round() as i64 - *x0 as i64, (p[1] / c).round() as i64 - *y0 as i64);
                if self.has(i, j) {
                    return (0.0, p);
                }
                // To the nearest edge cell's square.
                let mut best = (f64::MAX, p);
                for &b in edge {
                    let (cx, cy) = (((b % w + x0) as f64) * c, ((b / w + y0) as f64) * c);
                    let q = [p[0].clamp(cx - 0.5 * c, cx + 0.5 * c), p[1].clamp(cy - 0.5 * c, cy + 0.5 * c)];
                    let d = f64::hypot(p[0] - q[0], p[1] - q[1]);
                    if d < best.0 {
                        best = (d, [cx, cy]);
                    }
                }
                best
            }
        }
    }
}

/// How far round its point a site reaches (ft), for "what is near".
pub const SITE_RADIUS_FT: f64 = 300.0;

pub struct Inputs<'a> {
    pub world: &'a World,
    pub w: usize,
    pub h: usize,
    pub cell_ft: f64,
    pub height: &'a [f64],
    pub land: &'a [bool],
    pub biome: &'a [u32],
    pub clim: &'a Climate,
    pub hydro: &'a Hydro,
    pub volcanoes: &'a [Volcano],
    pub settlements: &'a [Settlement],
    pub pois: &'a [Poi],
}

struct Builder<'a> {
    inp: &'a Inputs<'a>,
    namer: Namer,
    cultures: Vec<(f64, f64, usize)>,
    out: Overlay,
    ids: std::collections::BTreeSet<String>,
}

pub fn extract(inp: &Inputs) -> Overlay {
    let mut b = Builder {
        inp,
        namer: Namer::new(inp.world.stream("t0.names")),
        cultures: culture_seeds(inp),
        out: Overlay::default(),
        ids: Default::default(),
    };
    b.landmasses();
    b.oceans();
    b.bays();
    b.ranges_peaks_passes();
    b.volcanoes();
    b.lakes();
    b.rivers();
    b.regions();
    b.sites();
    b.out
}

fn culture_seeds(inp: &Inputs) -> Vec<(f64, f64, usize)> {
    let mut rng = Pcg32::new(inp.world.stream("t0.cultures"), 5);
    let land: Vec<usize> = (0..inp.w * inp.h).filter(|&k| inp.land[k]).collect();
    if land.is_empty() {
        return vec![(0.0, 0.0, 1)];
    }
    (0..6)
        .map(|_| {
            let k = land[rng.below(land.len() as u32) as usize];
            let (t, p) = (inp.clim.temp[k], inp.clim.precip[k]);
            let c = if t < 2.0 {
                0
            } else if t > 15.0 && p < 400.0 {
                3
            } else if p < 650.0 {
                4
            } else if t > 20.0 {
                2
            } else {
                1
            };
            ((k % inp.w) as f64, (k / inp.w) as f64, c)
        })
        .collect()
}

impl Builder<'_> {
    fn culture_at(&self, cx: f64, cy: f64) -> usize {
        self.cultures
            .iter()
            .min_by(|a, b| {
                let da = (a.0 - cx) * (a.0 - cx) + (a.1 - cy) * (a.1 - cy);
                let db = (b.0 - cx) * (b.0 - cx) + (b.1 - cy) * (b.1 - cy);
                da.total_cmp(&db)
            })
            .map(|c| c.2)
            .unwrap_or(1)
    }

    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, kind: &'static str, nk: NameKind, cx: f64, cy: f64, angle: f64, extent_ft: f64, elev_ft: Option<f64>, detail: Option<String>) -> String {
        let culture = self.culture_at(cx, cy);
        let name = self.namer.name(nk, culture);
        let mut id = format!("{kind}:{:x}", hash2(self.inp.world.seed, (cx / 8.0) as i64, (cy / 8.0) as i64) & 0xffff_ffff);
        while !self.ids.insert(id.clone()) {
            id.push('b');
        }
        let c = self.inp.cell_ft;
        self.out.features.push(Feature { id: id.clone(), kind, name, x: cx * c, y: cy * c, angle, extent_ft, elev_ft, detail });
        self.out.shapes.push(Shape::Point { at: [cx * c, cy * c], radius_ft: 0.0 });
        id
    }

    /// The feature just pushed covers these cells (of a grid `gw` wide, `cell_ft` apart).
    fn covers(&mut self, gw: usize, cell_ft: f64, comp: &[u32]) {
        *self.out.shapes.last_mut().expect("just pushed") = Shape::area(gw, cell_ft, comp);
    }

    /// The feature just pushed is a point with this radius (ft).
    fn radius(&mut self, r: f64) {
        if let Some(Shape::Point { radius_ft, .. }) = self.out.shapes.last_mut() {
            *radius_ft = r;
        }
    }

    /// The feature just pushed keeps the name drawn with it (if its stroke has one).
    fn drawn_name(&mut self, stroke: Option<u32>) {
        let name = stroke.and_then(|i| self.inp.world.file.sketch.strokes.get(i as usize)).and_then(|s| s.name.as_deref()).map(str::trim).filter(|n| !n.is_empty());
        if let Some(name) = name {
            self.out.features.last_mut().expect("just pushed").name = name.to_string();
        }
    }

    fn above_sea(&self, k: usize) -> f64 {
        self.inp.height[k] - self.inp.world.params().sea_level_ft
    }

    fn landmasses(&mut self) {
        let inp = self.inp;
        let comps = components(inp.w, inp.h, |k| inp.land[k], true);
        let inside = distance_to(inp.w, inp.h, &inp.land.iter().map(|l| !l).collect::<Vec<_>>());
        let largest = comps.iter().map(Vec::len).max().unwrap_or(0);
        for comp in &comps {
            if comp.len() < 6 {
                continue;
            }
            let (ax, ay) = pole(inp.w, comp, &inside);
            let (angle, _) = principal_axis(inp.w, comp, 0.35);
            let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
            if comp.len() == largest {
                self.push("continent", NameKind::Continent, ax, ay, angle, extent, None, None);
            } else {
                self.push("island", NameKind::Island, ax, ay, angle, extent, None, None);
            }
            self.covers(inp.w, inp.cell_ft, comp);
        }
    }

    fn oceans(&mut self) {
        let inp = self.inp;
        let (w, h) = (inp.w, inp.h);
        let comps = components(w, h, |k| !inp.land[k], false);
        let target: Vec<bool> = (0..w * h)
            .map(|k| inp.land[k] || k % w == 0 || k / w == 0 || k % w == w - 1 || k / w == h - 1)
            .collect();
        let dist = distance_to(w, h, &target);
        for comp in &comps {
            let border = comp.iter().any(|&k| {
                let k = k as usize;
                k % w == 0 || k / w == 0 || k % w == w - 1 || k / w == h - 1
            });
            if !border && comp.len() < 300 {
                continue;
            }
            let (ax, ay) = pole(w, comp, &dist);
            let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
            let (kind, nk) = if border { ("ocean", NameKind::Ocean) } else { ("sea", NameKind::Sea) };
            self.push(kind, nk, ax, ay, 0.0, extent, None, None);
            self.covers(w, inp.cell_ft, comp);
        }
    }

    /// Bays: stretches of sea mostly enclosed by land (ray casting on a coarse grid).
    fn bays(&mut self) {
        let inp = self.inp;
        let (w, h) = (inp.w, inp.h);
        const STEP: usize = 4;
        let (cw, ch) = (w / STEP, h / STEP);
        let rays: Vec<(f64, f64)> =
            (0..16).map(|r| (libm::cos(r as f64 * std::f64::consts::TAU / 16.0), libm::sin(r as f64 * std::f64::consts::TAU / 16.0))).collect();
        let mut enclosed = vec![false; cw * ch];
        for cj in 0..ch {
            for ci in 0..cw {
                let (i, j) = (ci * STEP, cj * STEP);
                if inp.land[j * w + i] {
                    continue;
                }
                let mut hits = 0;
                for (r, &(dx, dy)) in rays.iter().enumerate() {
                    // Decided either way: 11 hits, or too few rays left to reach 11.
                    if hits >= 11 || hits + (rays.len() - r) < 11 {
                        break;
                    }
                    for s in 1..50 {
                        let (x, y) = (i as f64 + dx * s as f64, j as f64 + dy * s as f64);
                        if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 {
                            break;
                        }
                        if inp.land[y as usize * w + x as usize] {
                            hits += 1;
                            break;
                        }
                    }
                }
                enclosed[cj * cw + ci] = hits >= 11;
            }
        }
        for comp in components(cw, ch, |k| enclosed[k], false) {
            if comp.len() < 4 {
                continue;
            }
            let (sx, sy) = comp.iter().fold((0.0, 0.0), |a, &k| (a.0 + (k as usize % cw) as f64, a.1 + (k as usize / cw) as f64));
            let (cx, cy) = (sx / comp.len() as f64 * STEP as f64, sy / comp.len() as f64 * STEP as f64);
            let extent = (comp.len() as f64).sqrt() * STEP as f64 * inp.cell_ft;
            self.push("bay", NameKind::Bay, cx, cy, 0.0, extent, None, None);
            self.covers(cw, STEP as f64 * inp.cell_ft, &comp);
        }
    }

    fn ranges_peaks_passes(&mut self) {
        let inp = self.inp;
        let (w, h) = (inp.w, inp.h);
        let p = inp.world.params();
        let mountain = |k: usize| inp.land[k] && self.above_sea(k) >= (0.28 * p.max_elev_ft).max(2500.0);
        let mask: Vec<bool> = (0..w * h).map(mountain).collect();
        let inside = distance_to(w, h, &mask.iter().map(|m| !m).collect::<Vec<_>>());
        for comp in components(w, h, |k| mask[k], true) {
            if comp.len() < 25 {
                continue;
            }
            let (ax, ay) = pole(w, &comp, &inside);
            let (angle, len) = principal_axis(w, &comp, 1.2);
            self.push("range", NameKind::Range, ax, ay, angle, len * inp.cell_ft, None, None);
            self.covers(w, inp.cell_ft, &comp);
        }

        // Topographic prominence by union-find over land cells, highest first.
        let mut order: Vec<usize> = (0..w * h).filter(|&k| inp.land[k]).collect();
        order.sort_by(|&a, &b| inp.height[b].total_cmp(&inp.height[a]).then(a.cmp(&b)));
        let mut parent: Vec<u32> = vec![u32::MAX; w * h];
        let mut summit: Vec<u32> = vec![u32::MAX; w * h];
        fn find(parent: &mut [u32], mut x: usize) -> usize {
            while parent[x] as usize != x {
                let g = parent[parent[x] as usize];
                parent[x] = g;
                x = g as usize;
            }
            x
        }
        let mut peaks: Vec<(f64, usize, usize)> = Vec::new(); // (prominence, summit, col)
        for &c in &order {
            // The distinct sets among the (at most eight) neighbours.
            let (mut found, mut nr) = ([0usize; 8], 0);
            for (nb, _) in neighbors(w, h, c) {
                if parent[nb] != u32::MAX {
                    let r = find(&mut parent, nb);
                    if !found[..nr].contains(&r) {
                        found[nr] = r;
                        nr += 1;
                    }
                }
            }
            let roots = &mut found[..nr];
            parent[c] = c as u32;
            if roots.is_empty() {
                summit[c] = c as u32;
                continue;
            }
            roots.sort_by(|&a, &b| {
                inp.height[summit[b] as usize].total_cmp(&inp.height[summit[a] as usize]).then(a.cmp(&b))
            });
            let main = roots[0];
            for &r in &roots[1..] {
                let s = summit[r] as usize;
                peaks.push((inp.height[s] - inp.height[c], s, c));
                parent[r] = main as u32;
            }
            parent[c] = main as u32;
        }
        // Each landmass's highest point: prominence is its height above the sea.
        for &c in &order {
            if find(&mut parent, c) == c {
                let s = summit[c] as usize;
                peaks.push((self.above_sea(s), s, s));
            }
        }
        let min_prom = (0.1 * p.max_elev_ft).max(1500.0);
        peaks.retain(|&(prom, s, _)| prom >= min_prom && self.above_sea(s) >= 3000.0);
        peaks.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let near_volcano = |k: usize| {
            inp.volcanoes.iter().any(|v| {
                let (dx, dy) = ((k % w) as f64 - v.cx, (k / w) as f64 - v.cy);
                dx * dx + dy * dy < 9.0
            })
        };
        let mut passes: Vec<(f64, f64)> = Vec::new();
        let pass_sep = 25.0 * 5280.0 / inp.cell_ft;
        for &(prom, s, col) in peaks.iter().take(60) {
            if near_volcano(s) {
                continue;
            }
            let elev = self.above_sea(s);
            let (sx, sy) = ((s % w) as f64, (s / w) as f64);
            let extent = (prom * 12.0).max(8.0 * inp.cell_ft);
            self.push("peak", NameKind::Peak, sx, sy, 0.0, extent, Some(elev.round()), Some(format!("{} ft", fmt_thousands(elev))));

            let col_elev = self.above_sea(col);
            let (cx, cy) = ((col % w) as f64, (col / w) as f64);
            if col != s
                && passes.len() < 25
                && col_elev >= 1500.0
                && passes.iter().all(|&(px, py)| (px - cx) * (px - cx) + (py - cy) * (py - cy) >= pass_sep * pass_sep)
            {
                passes.push((cx, cy));
                self.push("pass", NameKind::Pass, cx, cy, 0.0, extent * 0.6, Some(col_elev.round()), Some(format!("{} ft", fmt_thousands(col_elev))));
            }
        }
    }

    /// Settlements (by tier) and points of interest. Settlement ids are stable per cell so
    /// later layout jobs and agents can refer to them.
    fn sites(&mut self) {
        let inp = self.inp;
        let c = inp.cell_ft;
        for s in inp.settlements {
            let (cx, cy) = (s.x / c, s.y / c);
            let k = s.cell;
            let kind = match s.tier {
                Tier::Metropolis => "metropolis",
                Tier::City => "city",
                Tier::Town => "town",
                Tier::Village => "village",
            };
            // Label extent: roughly the settlement footprint scaled up so it shows at the
            // zoom where it matters on the map.
            let extent = match s.tier {
                Tier::Metropolis => 60.0,
                Tier::City => 30.0,
                Tier::Town => 12.0,
                Tier::Village => 5.0,
            } * 5280.0;
            let mut detail = format!("{} {}, pop. {}", s.kind.name(), s.tier.name(), fmt_thousands(s.population as f64));
            if s.capital {
                detail.push_str(", capital");
            }
            let elev = self.above_sea(k);
            let culture = self.culture_at(cx, cy) as u8;
            self.out.settlement_cultures.push(culture);
            self.push(kind, NameKind::Settlement, cx, cy, 0.0, extent, Some(elev.round()), Some(detail));
            self.radius(crate::town::urban_radius(s.tier, s.population));
            // A pinned settlement keeps the name drawn with it.
            let pinned = s.pin.and_then(|i| inp.world.file.sketch.strokes.get(i as usize)).and_then(|st| st.name.as_deref()).map(str::trim).filter(|n| !n.is_empty());
            if let Some(name) = pinned {
                self.out.features.last_mut().expect("just pushed").name = name.to_string();
            }
        }
        for p in inp.pois {
            let (cx, cy) = (p.x / c, p.y / c);
            let (kind, nk, extent) = match p.kind {
                PoiKind::Ruin => ("ruin", NameKind::Ruin, 3.0),
                PoiKind::Tower => ("tower", NameKind::Tower, 3.0),
                PoiKind::Camp => ("camp", NameKind::Camp, 2.0),
                PoiKind::Waystation => ("waystation", NameKind::Settlement, 2.0),
                PoiKind::Cave => ("cave", NameKind::Cave, 2.0),
                PoiKind::Mine => ("mine", NameKind::Mine, 2.0),
                PoiKind::LavaTube => ("lava_tube", NameKind::LavaTube, 2.0),
                PoiKind::Entrance => ("entrance", NameKind::Ruin, 2.0),
                PoiKind::Building => ("building", NameKind::Settlement, 0.5),
            };
            let k = (cy.round() as usize).min(inp.h - 1) * inp.w + (cx.round() as usize).min(inp.w - 1);
            let elev = self.above_sea(k);
            let id = self.push(kind, nk, cx, cy, 0.0, extent * 5280.0, Some(elev.round()), None);
            self.radius(SITE_RADIUS_FT);
            if p.kind == PoiKind::Waystation {
                let f = self.out.features.iter_mut().rev().find(|f| f.id == id).unwrap();
                f.name = format!("{} Inn", f.name);
                f.detail = Some("roadside inn".into());
            }
        }
    }

    fn volcanoes(&mut self) {
        let vs: Vec<Volcano> = self.inp.volcanoes.to_vec();
        for v in vs {
            let k = v.cy as usize * self.inp.w + v.cx as usize;
            let mut elev = self.above_sea(k);
            if v.kind == VolcanoKind::Caldera {
                // (Its height is its rim's, round the crater.)
                let r = (v.radius_ft / self.inp.cell_ft).ceil() as i64;
                let (ci, cj) = (v.cx as i64, v.cy as i64);
                for j in (cj - r).max(0)..=(cj + r).min(self.inp.h as i64 - 1) {
                    for i in (ci - r).max(0)..=(ci + r).min(self.inp.w as i64 - 1) {
                        elev = elev.max(self.above_sea(j as usize * self.inp.w + i as usize));
                    }
                }
            }
            let kind = match v.kind {
                VolcanoKind::Stratovolcano => "stratovolcano",
                VolcanoKind::Shield => "shield volcano",
                VolcanoKind::CinderCone => "cinder cone",
                VolcanoKind::Caldera => "caldera",
            };
            let act = match v.activity {
                Activity::Active => "active",
                Activity::Dormant => "dormant",
                Activity::Extinct => "extinct",
            };
            let detail = format!("{act} {kind}, {} ft", fmt_thousands(elev));
            self.push("volcano", NameKind::Volcano, v.cx, v.cy, 0.0, v.radius_ft * 4.0, Some(elev.round()), Some(detail));
            self.radius(v.radius_ft);
            self.drawn_name(v.stroke);
        }
    }

    fn lakes(&mut self) {
        let inp = self.inp;
        let lakes = inp.hydro.lakes.clone();
        let water: Vec<bool> = (0..inp.w * inp.h).map(|k| inp.hydro.lake_of[k] == super::hydro::NO_LAKE).collect();
        let inside = distance_to(inp.w, inp.h, &water);
        for lake in &lakes {
            if lake.cells.len() < 4 {
                continue;
            }
            let (ax, ay) = pole(inp.w, &lake.cells, &inside);
            let (angle, _) = principal_axis(inp.w, &lake.cells, 0.5);
            let extent = (lake.cells.len() as f64).sqrt() * inp.cell_ft;
            let (kind, nk) = match lake.kind {
                LakeKind::Fresh => ("lake", NameKind::Lake),
                LakeKind::Salt => ("salt_lake", NameKind::SaltLake),
                LakeKind::SaltFlat => ("salt_flat", NameKind::SaltFlat),
            };
            let elev = lake.level_ft - inp.world.params().sea_level_ft;
            self.push(kind, nk, ax, ay, angle, extent, Some(elev.round()), None);
            self.covers(inp.w, inp.cell_ft, &lake.cells);
            self.drawn_name(lake.stroke);
        }
    }

    fn rivers(&mut self) {
        let inp = self.inp;
        let w = inp.w;
        let chains = inp.hydro.rivers.clone();
        let mut falls: Vec<(f64, f64, f64, usize)> = Vec::new();
        for r in &chains {
            if r.cells.len() < 3 {
                continue;
            }
            let pts_cells: Vec<[f64; 2]> = r.cells.iter().map(|&c| [(c as usize % w) as f64, (c as usize / w) as f64]).collect();
            let (pts, q) = chaikin(&pts_cells, &r.q, 2);
            let named = r.cells.len() >= if r.mouth == Mouth::Confluence { 14 } else { 20 };
            if named {
                let m = pts.len() * 11 / 20;
                let a = pts[m.saturating_sub(3)];
                let b = pts[(m + 3).min(pts.len() - 1)];
                let angle = upright(libm::atan2(b[1] - a[1], b[0] - a[0]));
                let len = r.cells.len() as f64 * inp.cell_ft;
                self.push("river", NameKind::River, pts[m][0], pts[m][1], angle, len, None, None);
                *self.out.shapes.last_mut().expect("just pushed") = Shape::Line(pts.iter().map(|p| [(p[0] * inp.cell_ft) as f32, (p[1] * inp.cell_ft) as f32]).collect());
            }
            let _ = q;

            // Waterfalls: the steepest drop along the river, if it is a real knickpoint.
            let mut best: Option<(f64, usize)> = None;
            for k in 0..if named { r.cells.len() - 1 } else { 0 } {
                let (a, b) = (r.cells[k] as usize, r.cells[k + 1] as usize);
                if !inp.land[b] || inp.hydro.lake_of[b] != super::hydro::NO_LAKE {
                    continue;
                }
                let drop = inp.height[a] - inp.height[b];
                if drop >= 400.0 && drop / inp.cell_ft >= 0.06 && best.is_none_or(|(d, _)| drop > d) {
                    best = Some((drop, k));
                }
            }
            if let Some((drop, k)) = best {
                let c = r.cells[k] as usize;
                let (cx, cy) = ((c % w) as f64 + 0.5 * ((r.cells[k + 1] as usize % w) as f64 - (c % w) as f64), (c / w) as f64 + 0.5 * ((r.cells[k + 1] as usize / w) as f64 - (c / w) as f64));
                falls.push((drop, cx, cy, c));
            }
        }
        // Only the most dramatic drops become named landmarks.
        falls.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.3.cmp(&b.3)));
        for &(drop, cx, cy, c) in falls.iter().take(20) {
            let elev = self.above_sea(c);
            self.push("waterfall", NameKind::Waterfall, cx, cy, 0.0, 30.0 * inp.cell_ft, Some(elev.round()), Some(format!("drop ~{} ft", fmt_thousands(drop))));
        }
    }

    fn regions(&mut self) {
        let inp = self.inp;
        let groups: [(&'static str, NameKind, &[Biome], usize); 8] = [
            ("forest", NameKind::Forest, &[Biome::TemperateForest, Biome::TemperateRainforest], 120),
            ("jungle", NameKind::Jungle, &[Biome::Jungle], 120),
            ("taiga", NameKind::Taiga, &[Biome::Taiga], 150),
            ("desert", NameKind::Desert, &[Biome::HotDesert, Biome::ColdDesert], 150),
            ("swamp", NameKind::Swamp, &[Biome::Swamp], 15),
            ("plains", NameKind::Plains, &[Biome::Grassland, Biome::Steppe, Biome::Savanna], 250),
            ("tundra", NameKind::Tundra, &[Biome::Tundra], 150),
            ("glacier", NameKind::Glacier, &[Biome::Ice], 40),
        ];
        for (kind, nk, members, min_cells) in groups {
            let member: [bool; 256] = std::array::from_fn(|b| members.contains(&Biome::from_u8(b as u8)));
            let mask: Vec<bool> = inp.biome.iter().map(|&b| member[(b & 0xff) as usize]).collect();
            let comps: Vec<Vec<u32>> = components(inp.w, inp.h, |k| mask[k], false).into_iter().filter(|c| c.len() >= min_cells).collect();
            if comps.is_empty() {
                continue;
            }
            let inside = distance_to(inp.w, inp.h, &mask.iter().map(|m| !m).collect::<Vec<_>>());
            for comp in comps {
                let (ax, ay) = pole(inp.w, &comp, &inside);
                let (angle, _) = principal_axis(inp.w, &comp, 0.35);
                let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
                self.push(kind, nk, ax, ay, angle, extent, None, None);
                self.covers(inp.w, inp.cell_ft, &comp);
            }
        }
    }
}

/// Connected components of `mask` (8- or 4-connected), each a list of cell indices.
pub fn components(w: usize, h: usize, mask: impl Fn(usize) -> bool, eight: bool) -> Vec<Vec<u32>> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for s in 0..w * h {
        if seen[s] || !mask(s) {
            continue;
        }
        seen[s] = true;
        let mut comp = vec![s as u32];
        let mut k = 0;
        while k < comp.len() {
            let c = comp[k] as usize;
            k += 1;
            let mut visit = |nb: usize, d: f64| {
                if (eight || d == 1.0) && !seen[nb] && mask(nb) {
                    seen[nb] = true;
                    comp.push(nb as u32);
                }
            };
            // Off the grid's edge every neighbour is there, at fixed offsets (in D8 order).
            let (x, y) = (c % w, c / w);
            if x > 0 && y > 0 && x + 1 < w && y + 1 < h {
                for &(dx, dy, d) in &D8 {
                    visit((c as isize + dy as isize * w as isize + dx as isize) as usize, d);
                }
            } else {
                for (nb, d) in neighbors(w, h, c) {
                    visit(nb, d);
                }
            }
        }
        out.push(comp);
    }
    out
}

/// The component cell farthest from its edge (pole of inaccessibility), cell coordinates.
fn pole(w: usize, comp: &[u32], inside: &[f64]) -> (f64, f64) {
    let best = comp.iter().copied().max_by(|&a, &b| inside[a as usize].total_cmp(&inside[b as usize]).then(b.cmp(&a))).unwrap();
    ((best as usize % w) as f64, (best as usize / w) as f64)
}

/// Principal-axis angle (clamped to ±max_angle, kept upright) and axis length in cells.
fn principal_axis(w: usize, comp: &[u32], max_angle: f64) -> (f64, f64) {
    let n = comp.len() as f64;
    let (mut sx, mut sy) = (0.0, 0.0);
    for &k in comp {
        sx += (k as usize % w) as f64;
        sy += (k as usize / w) as f64;
    }
    let (mx, my) = (sx / n, sy / n);
    let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
    for &k in comp {
        let (dx, dy) = ((k as usize % w) as f64 - mx, (k as usize / w) as f64 - my);
        cxx += dx * dx;
        cyy += dy * dy;
        cxy += dx * dy;
    }
    let (cxx, cyy, cxy) = (cxx / n, cyy / n, cxy / n);
    let angle = 0.5 * libm::atan2(2.0 * cxy, cxx - cyy);
    let tr = cxx + cyy;
    let det = cxx * cyy - cxy * cxy;
    let l1 = tr / 2.0 + crate::core::sqrt((tr * tr / 4.0 - det).max(0.0));
    (upright(angle).clamp(-max_angle, max_angle), 4.0 * crate::core::sqrt(l1))
}

/// Keep text readable: map an angle into (-π/2, π/2].
fn upright(a: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let mut a = a;
    while a > pi / 2.0 {
        a -= pi;
    }
    while a <= -pi / 2.0 {
        a += pi;
    }
    a
}

/// Chaikin corner cutting, keeping both endpoints; per-point values are interpolated.
fn chaikin(pts: &[[f64; 2]], q: &[f32], iterations: usize) -> (Vec<[f64; 2]>, Vec<f32>) {
    let (mut p, mut v) = (pts.to_vec(), q.to_vec());
    for _ in 0..iterations {
        if p.len() < 3 {
            break;
        }
        let mut np = vec![p[0]];
        let mut nv = vec![v[0]];
        for k in 0..p.len() - 1 {
            let (a, b) = (p[k], p[k + 1]);
            np.push([0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]]);
            np.push([0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]);
            nv.push(0.75 * v[k] + 0.25 * v[k + 1]);
            nv.push(0.25 * v[k] + 0.75 * v[k + 1]);
        }
        np.push(*p.last().unwrap());
        nv.push(*v.last().unwrap());
        p = np;
        v = nv;
    }
    (p, v)
}

pub fn fmt_thousands(v: f64) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}
