//! Named features (the T0 part of the gazetteer): continent, islands, oceans and seas, bays,
//! mountain ranges, peaks (by topographic prominence), passes (their key cols), volcanoes,
//! lakes, rivers and biome regions. Each gets a stable id, a name, and label placement
//! (anchor at the pole of inaccessibility, angle along the principal axis, size in feet).
//!
//! The sketch names what it makes (a named range, massif, river, lake, volcano, painted biome,
//! land or sea stroke names the features made from it), and region strokes name what lies
//! where they are drawn, or draw a region of their own. A range or region with several names in
//! it is split between them (each part the ground nearest its name, borders following rivers);
//! a region left very large is split into parts of a size a map names.

use serde::Serialize;

use super::biome::Biome;
use super::climate::{Climate, distance_to};
use super::flood::{D8, neighbors};
use super::hydro::{Hydro, LakeKind, Mouth};
use super::names::{NameKind, Namer};
use super::settle::{Poi, PoiKind, Settlement, Tier};
use super::sketch::{Conflict, Raster, stroke_cells};
use super::volcano::{Activity, Volcano, VolcanoKind};
use crate::World;
use crate::core::rng::{Pcg32, hash2};
use crate::world::{REGION_KINDS, SketchTool};

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
    /// The drawn rivers' courses: (stroke, cells).
    pub courses: &'a [(usize, Vec<usize>)],
    /// The road network (named drawn roads become features).
    pub roads: &'a [super::roads::RoadPath],
}

/// Biome regions: (kind, how they are named, member biomes, fewest cells to be named).
const GROUPS: [(&str, NameKind, &[Biome], usize); 10] = [
    ("forest", NameKind::Forest, &[Biome::TemperateForest, Biome::TemperateRainforest], 120),
    ("jungle", NameKind::Jungle, &[Biome::Jungle], 120),
    ("taiga", NameKind::Taiga, &[Biome::Taiga], 150),
    ("desert", NameKind::Desert, &[Biome::HotDesert, Biome::ColdDesert], 150),
    ("swamp", NameKind::Swamp, &[Biome::Swamp], 15),
    ("plains", NameKind::Plains, &[Biome::Grassland, Biome::Steppe, Biome::Savanna], 250),
    ("tundra", NameKind::Tundra, &[Biome::Tundra], 150),
    ("glacier", NameKind::Glacier, &[Biome::Ice], 40),
    ("blight", NameKind::Blight, &[Biome::Blight], 15),
    ("ashlands", NameKind::Ashlands, &[Biome::Ashland], 15),
];

/// The region group of a packed biome (by its main biome).
fn group_of(b: u32) -> Option<usize> {
    let b = Biome::from_u8((b & 0xff) as u8);
    GROUPS.iter().position(|g| g.2.contains(&b))
}

/// How far a name drawn as a point reaches through what it names (mi, by the steps between).
const LABEL_REACH_MI: f64 = 60.0;
/// A name asked for a kind of feature may name one this near (mi) when not drawn on it.
const SNAP_MI: f64 = 15.0;
/// Crossing a mapped river (or a mountain) costs this many steps: borders follow them.
const BARRIER: u32 = 20;
/// A region left larger than this (sq mi) is split into parts about this size.
const REGION_CAP_SQ_MI: f64 = 12_000.0;

/// What a name in the sketch names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    /// A biome region (`GROUPS`).
    Region(usize),
    /// A region drawn as an outline: its own cells.
    Outline,
    Range,
    Lake,
    River,
    Land,
    Water,
}

/// A name the sketch gives.
struct Label {
    stroke: usize,
    name: String,
    target: Target,
    /// The kind it asked for (a region stroke's `kind`): it may name one near, not only under it.
    asked: Option<&'static str>,
    /// Grid cells it stands on.
    cells: Vec<u32>,
    /// How far its name reaches (steps) through what it names.
    reach: u32,
    used: bool,
}

struct Builder<'a> {
    inp: &'a Inputs<'a>,
    namer: Namer,
    cultures: Vec<(f64, f64, usize)>,
    out: Overlay,
    ids: std::collections::BTreeSet<String>,
    /// Mountain ground (ranges are its components).
    mountain: Vec<bool>,
    labels: Vec<Label>,
    /// Features named by the sketch: feature index → the stroke.
    named: Vec<(usize, usize)>,
    /// Grid-sized scratch for `Local` (all `u32::MAX` between uses).
    scratch: Vec<u32>,
}

pub fn extract(inp: &Inputs) -> Overlay {
    let p = inp.world.params();
    let sea = p.sea_level_ft;
    let mountain: Vec<bool> = (0..inp.w * inp.h).map(|k| inp.land[k] && inp.height[k] - sea >= (0.28 * p.max_elev_ft).max(2500.0)).collect();
    let (labels, notes) = labels(inp, &mountain);
    let mut b = Builder {
        inp,
        namer: Namer::with_sites(inp.world.stream("t0.names"), inp.world.stream("t0.names.sites")),
        cultures: culture_seeds(inp),
        out: Overlay::default(),
        ids: Default::default(),
        mountain,
        labels,
        named: Vec::new(),
        scratch: vec![u32::MAX; inp.w * inp.h],
    };
    b.out.conflicts = notes;
    b.landmasses();
    b.oceans();
    b.bays();
    b.ranges_peaks_passes();
    b.volcanoes();
    b.lakes();
    b.rivers();
    b.regions();
    b.sites();
    b.place_labels();
    b.roads();
    b.out
}

/// The sketch's names: what each names, and where; and notes on names asking for a kind of
/// region or mountains none of which lies near (they name the region or mountains there).
fn labels(inp: &Inputs, mountain: &[bool]) -> (Vec<Label>, Vec<Conflict>) {
    let g = Raster { w: inp.w, h: inp.h, cell: inp.cell_ft };
    let reach = (LABEL_REACH_MI * 5280.0 / inp.cell_ft).round() as u32;
    let resolve = |k: usize| {
        if inp.hydro.lake_of[k] != super::hydro::NO_LAKE {
            Target::Lake
        } else if !inp.land[k] {
            Target::Water
        } else if mountain[k] {
            Target::Range
        } else {
            group_of(inp.biome[k]).map_or(Target::Land, Target::Region)
        }
    };
    let target_of = |kind: &str, k: usize| match kind {
        "range" => Target::Range,
        "lake" => Target::Lake,
        "river" => Target::River,
        "island" | "continent" => Target::Land,
        "bay" | "sea" | "ocean" => Target::Water,
        g => GROUPS.iter().position(|x| x.0 == g).map_or_else(|| resolve(k), Target::Region),
    };
    let dry = |k: usize| inp.land[k] && inp.hydro.lake_of[k] == super::hydro::NO_LAKE;
    // Whether a cell of `target` lies within `SNAP_MI` of `k`.
    let snap = (SNAP_MI * 5280.0 / inp.cell_ft).ceil() as i64;
    let near = |target: Target, k: usize| {
        let (ci, cj) = ((k % inp.w) as i64, (k / inp.w) as i64);
        let is = |c: usize| match target {
            Target::Region(g) => dry(c) && group_of(inp.biome[c]) == Some(g),
            Target::Range => mountain[c],
            _ => true,
        };
        ((cj - snap).max(0)..=(cj + snap).min(inp.h as i64 - 1))
            .any(|j| ((ci - snap).max(0)..=(ci + snap).min(inp.w as i64 - 1)).any(|i| (i - ci) * (i - ci) + (j - cj) * (j - cj) <= snap * snap && is(j as usize * inp.w + i as usize)))
    };
    let (mut out, mut notes) = (Vec::new(), Vec::new());
    for (si, s) in inp.world.file.sketch.strokes.iter().enumerate() {
        let Some(name) = s.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) else { continue };
        let asked = s.kind.as_deref().and_then(|k| REGION_KINDS.iter().find(|&&x| x == k).copied()).filter(|_| s.tool == SketchTool::Region);
        let (target, cells, reach) = match s.tool {
            SketchTool::Region if s.closed && s.pts.len() >= 3 => {
                let cells = stroke_cells(g, s, 0.0);
                match asked {
                    Some(k) if k != "region" && !GROUPS.iter().any(|x| x.0 == k) => (target_of(k, cells[0]), cells, reach),
                    _ => (Target::Outline, cells.into_iter().filter(|&k| dry(k)).collect(), 0),
                }
            }
            SketchTool::Region => {
                let k = g.cell_of(s.pts[0]);
                let mut target = asked.map_or_else(|| resolve(k), |a| target_of(a, k));
                if asked.is_some() && matches!(target, Target::Region(_) | Target::Range) && !near(target, k) && matches!(resolve(k), Target::Region(_) | Target::Range) {
                    let there = match resolve(k) {
                        Target::Region(g) => GROUPS[g].0,
                        _ => "mountains",
                    };
                    notes.push(Conflict { stroke: si, message: format!("No {} near this name: it names the {there} here", asked.unwrap_or_default()), x: s.pts[0][0], y: s.pts[0][1] });
                    target = resolve(k);
                }
                (target, vec![k], reach)
            }
            SketchTool::Range => (Target::Range, stroke_cells(g, s, 0.5 * s.radius_ft), reach),
            SketchTool::Massif => (Target::Range, stroke_cells(g, s, 0.0), reach),
            SketchTool::River => (Target::River, inp.courses.iter().find(|c| c.0 == si).map(|c| c.1.clone()).unwrap_or_default(), 0),
            SketchTool::Land => (Target::Land, stroke_cells(g, s, s.radius_ft), 0),
            SketchTool::Sea => (Target::Water, stroke_cells(g, s, s.radius_ft), 0),
            SketchTool::Biome => {
                let Some(b) = s.biome.as_deref().and_then(|b| super::biome::ALL.iter().position(|x| x.name() == b)) else { continue };
                // What the paint made of its own biome.
                let cells: Vec<usize> = stroke_cells(g, s, s.radius_ft).into_iter().filter(|&k| dry(k) && (inp.biome[k] & 0xff) as usize == b).collect();
                match GROUPS.iter().position(|x| x.2.contains(&Biome::from_u8(b as u8))) {
                    Some(gi) => (Target::Region(gi), cells, 3),
                    None => (Target::Outline, cells, 0),
                }
            }
            // Settlements, lakes, volcanoes and sites are named where they are made.
            _ => continue,
        };
        out.push(Label { stroke: si, name: name.to_string(), target, asked, cells: cells.into_iter().map(|k| k as u32).collect(), reach, used: false });
    }
    (out, notes)
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

    /// Named drawn roads: one feature each over all their pieces (in order along the line
    /// drawn), labelled at the middle of the longest.
    fn roads(&mut self) {
        let strokes = &self.inp.world.file.sketch.strokes;
        let mut done: Vec<u32> = Vec::new();
        for r in self.inp.roads {
            let Some(st) = r.stroke.filter(|st| !done.contains(st)) else { continue };
            done.push(st);
            let Some(name) = strokes.get(st as usize).and_then(|s| s.name.as_deref()).map(str::trim).filter(|n| !n.is_empty()) else { continue };
            let line = &strokes[st as usize].pts;
            let d2 = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]);
            let len = |q: &super::roads::RoadPath| q.pts.windows(2).map(|w| crate::core::sqrt(d2(w[0], w[1]))).sum::<f64>();
            // Where along the line drawn a point falls (ft along it).
            let along = |m: [f64; 2]| {
                let mut best = (f64::INFINITY, 0.0);
                let mut acc = 0.0;
                for w in line.windows(2) {
                    let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
                    let l2 = (dx * dx + dy * dy).max(1e-9);
                    let t = (((m[0] - w[0][0]) * dx + (m[1] - w[0][1]) * dy) / l2).clamp(0.0, 1.0);
                    let d = d2(m, [w[0][0] + t * dx, w[0][1] + t * dy]);
                    if d < best.0 {
                        best = (d, acc + t * crate::core::sqrt(l2));
                    }
                    acc += crate::core::sqrt(l2);
                }
                best.1
            };
            // The pieces in order along it, each running its way.
            let mut pieces: Vec<(f64, Vec<[f64; 2]>, &super::roads::RoadPath)> = self
                .inp
                .roads
                .iter()
                .filter(|q| q.stroke == Some(st) && q.pts.len() >= 2)
                .map(|q| {
                    let (a, b) = (along(q.pts[0]), along(q.pts[q.pts.len() - 1]));
                    let pts = if a <= b { q.pts.clone() } else { q.pts.iter().rev().copied().collect() };
                    (a.min(b), pts, q)
                })
                .collect();
            pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
            let course: Vec<[f32; 2]> = pieces.iter().flat_map(|q| q.1.iter().map(|p| [p[0] as f32, p[1] as f32])).collect();
            let pieces: Vec<&super::roads::RoadPath> = pieces.iter().map(|q| q.2).collect();
            let Some(longest) = pieces.iter().max_by(|a, b| len(a).total_cmp(&len(b))) else { continue };
            // The middle of the longest piece, and its heading there.
            let half = 0.5 * len(longest);
            let (mut acc, mut k) = (0.0, 1);
            while k + 1 < longest.pts.len() && acc + crate::core::sqrt(d2(longest.pts[k], longest.pts[k - 1])) < half {
                acc += crate::core::sqrt(d2(longest.pts[k], longest.pts[k - 1]));
                k += 1;
            }
            let (a, b) = (longest.pts[k.saturating_sub(3)], longest.pts[(k + 3).min(longest.pts.len() - 1)]);
            let angle = upright(libm::atan2(b[1] - a[1], b[0] - a[0]));
            let at = longest.pts[k];
            let total: f64 = pieces.iter().map(|q| len(q)).sum();
            let class = ["king's road", "road", "track"][longest.class as usize];
            let mut id = format!("road:{:x}", hash2(self.inp.world.seed, (at[0] / 8.0 / self.inp.cell_ft) as i64, (at[1] / 8.0 / self.inp.cell_ft) as i64) & 0xffff_ffff);
            while !self.ids.insert(id.clone()) {
                id.push('b');
            }
            self.out.features.push(Feature { id, kind: "road", name: name.to_string(), x: at[0], y: at[1], angle, extent_ft: total, elev_ft: None, detail: Some(class.into()) });
            self.out.shapes.push(Shape::Line(course));
        }
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
        if let (Some(name), Some(si)) = (name, stroke) {
            self.out.features.last_mut().expect("just pushed").name = name.to_string();
            self.named.push((self.out.features.len() - 1, si as usize));
        }
    }

    /// The feature just pushed takes label `l`'s name.
    fn label_name(&mut self, l: usize) {
        self.labels[l].used = true;
        self.out.features.last_mut().expect("just pushed").name = self.labels[l].name.clone();
        self.named.push((self.out.features.len() - 1, self.labels[l].stroke));
    }

    /// A conflict for label `l`'s stroke, at its first cell.
    fn label_conflict(&mut self, l: usize, message: String) {
        let c = self.labels[l].cells.first().copied().unwrap_or(0) as usize;
        let (x, y) = ((c % self.inp.w) as f64 * self.inp.cell_ft, (c / self.inp.w) as f64 * self.inp.cell_ft);
        self.out.conflicts.push(Conflict { stroke: self.labels[l].stroke, message, x, y });
    }

    /// Labels of `target` with the cells they claim in `mask`: their own cells there, else (a
    /// point that asked for this kind) the nearest such cell within `SNAP_MI`.
    fn claims(&self, target: Target, mask: &[bool]) -> Vec<(usize, Vec<u32>)> {
        let (w, h) = (self.inp.w, self.inp.h);
        let snap = (SNAP_MI * 5280.0 / self.inp.cell_ft).ceil() as i64;
        let mut out = Vec::new();
        for (li, l) in self.labels.iter().enumerate().filter(|(_, l)| l.target == target) {
            let mut cells: Vec<u32> = l.cells.iter().copied().filter(|&k| mask[k as usize]).collect();
            if cells.is_empty() && l.asked.is_some() && l.cells.len() == 1 {
                let c = l.cells[0] as usize;
                let (ci, cj) = ((c % w) as i64, (c / w) as i64);
                let mut best: Option<(i64, usize)> = None;
                for j in (cj - snap).max(0)..=(cj + snap).min(h as i64 - 1) {
                    for i in (ci - snap).max(0)..=(ci + snap).min(w as i64 - 1) {
                        let (d2, k) = ((i - ci) * (i - ci) + (j - cj) * (j - cj), j as usize * w + i as usize);
                        if d2 <= snap * snap && mask[k] && best.is_none_or(|b| d2 < b.0) {
                            best = Some((d2, k));
                        }
                    }
                }
                cells.extend(best.map(|b| b.1 as u32));
            }
            if !cells.is_empty() {
                out.push((li, cells));
            }
        }
        out
    }

    /// Push `kind` features over a component of `mask`, split between the names claiming it (each
    /// part what is fewest steps from its name, within its reach; `cost` per cell entered), the
    /// rest (in parts of `cap` cells at most, if it has at least `min` cells) named by the namer.
    #[allow(clippy::too_many_arguments)]
    fn split_push(&mut self, kind: &'static str, nk: NameKind, comp: &[u32], claims: &[(usize, Vec<u32>)], min: usize, cap: usize, cost: &dyn Fn(usize) -> u32) {
        let w = self.inp.w;
        let mut at = std::mem::take(&mut self.scratch);
        let mut pieces: Vec<(Vec<u32>, Option<usize>)> = Vec::new();
        if claims.is_empty() {
            if comp.len() >= min {
                pieces.extend(cap_split(w, comp, cap, cost, &mut at).into_iter().map(|p| (p, None)));
            }
        } else {
            let mut far_parts: Vec<Vec<u32>> = Vec::new();
            let mut own: Vec<Vec<u32>> = vec![Vec::new(); claims.len()];
            {
                let l = Local::new(w, comp, &mut at);
                let (mut dist, mut owner) = (vec![u32::MAX; comp.len()], vec![u32::MAX; comp.len()]);
                // (A cell several names stand on goes to the most particular: a name dropped on a
                // drawn range names its own part of it.)
                let mut first: Vec<(usize, u32)> = vec![(usize::MAX, u32::MAX); comp.len()];
                for (ci, c) in claims.iter().enumerate() {
                    for i in c.1.iter().filter_map(|&k| l.get(k as usize)) {
                        if (c.1.len(), ci as u32) < first[i] {
                            first[i] = (c.1.len(), ci as u32);
                        }
                    }
                }
                let seeds: Vec<(u32, usize)> = (0..comp.len()).filter(|&i| first[i].1 != u32::MAX).map(|i| (first[i].1, i)).collect();
                grow(&l, &seeds, cost, &mut dist, &mut owner);
                // Ground out of every name's reach, enough of it to name, is named on its own.
                let far: Vec<bool> = (0..comp.len()).map(|i| owner[i] == u32::MAX || dist[i] > self.labels[claims[owner[i] as usize].0].reach).collect();
                let mut seen = vec![false; comp.len()];
                for s in 0..comp.len() {
                    if seen[s] {
                        continue;
                    }
                    if !far[s] {
                        own[owner[s] as usize].push(comp[s]);
                        continue;
                    }
                    let mut part = vec![s];
                    seen[s] = true;
                    let mut q = 0;
                    while q < part.len() {
                        let i = part[q];
                        q += 1;
                        for j in l.around(i) {
                            if far[j] && !seen[j] {
                                seen[j] = true;
                                part.push(j);
                            }
                        }
                    }
                    if part.len() >= min || part.iter().any(|&i| owner[i] == u32::MAX) {
                        far_parts.push(part.iter().map(|&i| comp[i]).collect());
                    } else {
                        for &i in &part {
                            own[owner[i] as usize].push(comp[i]);
                        }
                    }
                }
            }
            for part in far_parts {
                if part.len() >= min {
                    pieces.extend(cap_split(w, &part, cap, cost, &mut at).into_iter().map(|p| (p, None)));
                }
            }
            for (ci, cells) in own.into_iter().enumerate() {
                if !cells.is_empty() {
                    pieces.push((cells, Some(claims[ci].0)));
                }
            }
        }
        for (mut piece, label) in pieces {
            piece.sort_unstable();
            let (ax, ay) = piece_pole(w, &piece, &mut at);
            let (angle, len) = principal_axis(w, &piece, if kind == "range" { 1.2 } else { 0.35 });
            let extent = if kind == "range" { len * self.inp.cell_ft } else { (piece.len() as f64).sqrt() * self.inp.cell_ft };
            self.push(kind, nk, ax, ay, angle, extent, None, None);
            self.covers(w, self.inp.cell_ft, &piece);
            if let Some(l) = label {
                self.label_name(l);
            }
        }
        self.scratch = at;
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
        let mask = std::mem::take(&mut self.mountain);
        let inside = distance_to(w, h, &mask.iter().map(|m| !m).collect::<Vec<_>>());
        // Named ranges, massifs and range names: each claims the mountains it is drawn on.
        let claims = self.claims(Target::Range, &mask);
        let mut by_comp: crate::core::hash::FastMap<u32, Vec<(usize, Vec<u32>)>> = Default::default();
        let comps = components(w, h, |k| mask[k], true);
        let mut comp_of = vec![u32::MAX; w * h];
        for (ci, comp) in comps.iter().enumerate() {
            for &k in comp {
                comp_of[k as usize] = ci as u32;
            }
        }
        for (li, cells) in &claims {
            let mut per: Vec<(u32, Vec<u32>)> = Vec::new();
            for &k in cells {
                let c = comp_of[k as usize];
                match per.iter_mut().find(|x| x.0 == c) {
                    Some(x) => x.1.push(k),
                    None => per.push((c, vec![k])),
                }
            }
            // (A drawn range reaching a range of its own names that, not the specks beside it.)
            if per.iter().any(|x| comps[x.0 as usize].len() >= 25) {
                per.retain(|x| comps[x.0 as usize].len() >= 25);
            }
            for (c, ks) in per {
                by_comp.entry(c).or_default().push((*li, ks));
            }
        }
        for (ci, comp) in comps.iter().enumerate() {
            match by_comp.get(&(ci as u32)) {
                Some(claims) => self.split_push("range", NameKind::Range, comp, &claims.clone(), 25, usize::MAX, &|_| 1),
                None if comp.len() >= 25 => {
                    let (ax, ay) = pole(w, comp, &inside);
                    let (angle, len) = principal_axis(w, comp, 1.2);
                    self.push("range", NameKind::Range, ax, ay, angle, len * inp.cell_ft, None, None);
                    self.covers(w, inp.cell_ft, comp);
                }
                None => {}
            }
        }
        // A named range or massif drawn where no mountains rose (low hills) is still named, over
        // the ground it was drawn on.
        let lone: Vec<usize> = (0..self.labels.len())
            .filter(|&l| {
                let lb = &self.labels[l];
                lb.target == Target::Range && !lb.used && lb.asked.is_none() && matches!(inp.world.file.sketch.strokes[lb.stroke].tool, SketchTool::Range | SketchTool::Massif)
            })
            .collect();
        for l in lone {
            let cells: Vec<u32> = self.labels[l].cells.iter().copied().filter(|&k| inp.land[k as usize]).collect();
            if !cells.is_empty() {
                self.split_push("range", NameKind::Range, &cells, &[(l, cells.clone())], 1, usize::MAX, &|_| 1);
            }
        }
        self.mountain = mask;

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
            self.drawn_name(s.pin);
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
                PoiKind::Castle => ("castle", NameKind::Settlement, 1.0),
                PoiKind::Wall => ("wall", NameKind::Settlement, 1.0),
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
            self.drawn_name(p.stroke);
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
        // A drawn river's name goes to the rivers mapped mostly on its course (its channel, which
        // the hydrology follows; not the tributaries that only meet it); a river name drawn as a
        // point, to the nearest river within `SNAP_MI`.
        let mut along: crate::core::hash::FastMap<u32, usize> = Default::default();
        for (li, l) in self.labels.iter().enumerate().filter(|(_, l)| l.target == Target::River && l.cells.len() > 1) {
            for &c in &l.cells {
                along.entry(c).or_insert(li);
            }
        }
        let mut pointed: Vec<Option<usize>> = vec![None; chains.len()];
        let snap = (SNAP_MI * 5280.0 / inp.cell_ft).ceil() as i64;
        for (li, l) in self.labels.iter().enumerate().filter(|(_, l)| l.target == Target::River && l.cells.len() == 1) {
            let c = l.cells[0] as usize;
            let (ci, cj) = ((c % w) as i64, (c / w) as i64);
            let d2 = |k: u32| ((k as usize % w) as i64 - ci).pow(2) + ((k as usize / w) as i64 - cj).pow(2);
            let best = chains
                .iter()
                .enumerate()
                .filter(|(_, r)| r.cells.len() >= 3)
                .filter_map(|(ri, r)| r.cells.iter().map(|&k| d2(k)).min().map(|d| (d, ri)))
                .filter(|&(d, _)| d <= snap * snap)
                .min();
            if let Some((_, ri)) = best
                && pointed[ri].is_none()
            {
                pointed[ri] = Some(li);
            }
        }
        let mut falls: Vec<(f64, f64, f64, usize)> = Vec::new();
        for (ri, r) in chains.iter().enumerate() {
            if r.cells.len() < 3 {
                continue;
            }
            let pts_cells: Vec<[f64; 2]> = r.cells.iter().map(|&c| [(c as usize % w) as f64, (c as usize / w) as f64]).collect();
            let (pts, q) = chaikin(&pts_cells, &r.q, 2);
            let mut votes: Vec<(usize, usize)> = Vec::new();
            for c in &r.cells {
                if let Some(&li) = along.get(c) {
                    match votes.iter_mut().find(|v| v.0 == li) {
                        Some(v) => v.1 += 1,
                        None => votes.push((li, 1)),
                    }
                }
            }
            let drawn = votes.iter().max_by_key(|v| (v.1, std::cmp::Reverse(v.0))).filter(|v| v.1 >= 3 && 2 * v.1 >= r.cells.len()).map(|v| v.0);
            let label = drawn.or(pointed[ri]);
            let named = label.is_some() || r.cells.len() >= if r.mouth == Mouth::Confluence { 14 } else { 20 };
            if named {
                let m = pts.len() * 11 / 20;
                let a = pts[m.saturating_sub(3)];
                let b = pts[(m + 3).min(pts.len() - 1)];
                let angle = upright(libm::atan2(b[1] - a[1], b[0] - a[0]));
                let len = r.cells.len() as f64 * inp.cell_ft;
                self.push("river", NameKind::River, pts[m][0], pts[m][1], angle, len, None, None);
                *self.out.shapes.last_mut().expect("just pushed") = Shape::Line(pts.iter().map(|p| [(p[0] * inp.cell_ft) as f32, (p[1] * inp.cell_ft) as f32]).collect());
                if let Some(l) = label {
                    self.label_name(l);
                }
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

    /// Biome regions: drawn outlines first (their own ground), then each group's components,
    /// split between the names in them or, left very large, into parts of `REGION_CAP_SQ_MI`.
    fn regions(&mut self) {
        let inp = self.inp;
        let (w, h) = (inp.w, inp.h);
        let n = w * h;
        let mut taken = vec![false; n];
        for l in 0..self.labels.len() {
            if self.labels[l].target != Target::Outline {
                continue;
            }
            let cells: Vec<u32> = self.labels[l].cells.iter().copied().filter(|&k| !taken[k as usize]).collect();
            if cells.is_empty() {
                self.label_conflict(l, "This region has no dry land left to name (or another region has it)".into());
                self.labels[l].used = true;
                continue;
            }
            for &k in &cells {
                taken[k as usize] = true;
            }
            // Of the kind asked, else the land's own kind there (most of it), else a region.
            let mut count = [0usize; GROUPS.len()];
            for &k in &cells {
                if let Some(g) = group_of(inp.biome[k as usize]) {
                    count[g] += 1;
                }
            }
            let most = (0..GROUPS.len()).max_by_key(|&g| (count[g], std::cmp::Reverse(g))).filter(|&g| count[g] * 2 > cells.len());
            let group = self.labels[l].asked.and_then(|k| GROUPS.iter().position(|x| x.0 == k)).or(if self.labels[l].asked == Some("region") { None } else { most });
            let (kind, nk) = group.map_or(("region", NameKind::Region), |g| (GROUPS[g].0, GROUPS[g].1));
            self.split_push(kind, nk, &cells, &[(l, cells.clone())], 1, usize::MAX, &|_| 1);
        }
        // Mapped rivers and mountains are borders.
        let mut barrier = self.mountain.clone();
        for r in inp.hydro.rivers.iter().filter(|r| r.cells.len() >= 20) {
            for &c in &r.cells {
                barrier[c as usize] |= inp.land[c as usize];
            }
        }
        let cost = |k: usize| if barrier[k] { 1 + BARRIER } else { 1 };
        let cell_mi = inp.cell_ft / 5280.0;
        let cap = (REGION_CAP_SQ_MI / (cell_mi * cell_mi)).round() as usize;
        for (gi, &(kind, nk, members, min_cells)) in GROUPS.iter().enumerate() {
            let member: [bool; 256] = std::array::from_fn(|b| members.contains(&Biome::from_u8(b as u8)));
            let mask: Vec<bool> = (0..n).map(|k| member[(inp.biome[k] & 0xff) as usize] && !taken[k]).collect();
            let claims = self.claims(Target::Region(gi), &mask);
            if claims.is_empty() && !mask.iter().any(|&m| m) {
                continue;
            }
            let comps = components(w, h, |k| mask[k], false);
            let mut comp_of = vec![u32::MAX; if claims.is_empty() { 0 } else { n }];
            if !claims.is_empty() {
                for (ci, comp) in comps.iter().enumerate() {
                    for &k in comp {
                        comp_of[k as usize] = ci as u32;
                    }
                }
            }
            for (ci, comp) in comps.iter().enumerate() {
                let mine: Vec<(usize, Vec<u32>)> = claims
                    .iter()
                    .map(|(l, cells)| (*l, cells.iter().copied().filter(|&k| comp_of[k as usize] == ci as u32).collect::<Vec<u32>>()))
                    .filter(|(_, cells)| !cells.is_empty())
                    .collect();
                if mine.is_empty() && comp.len() < min_cells {
                    continue;
                }
                self.split_push(kind, nk, comp, &mine, min_cells, cap, &cost);
            }
        }
    }

    /// Names drawn on islands, seas, bays and lakes (one name each; a region stroke as a point
    /// names the most local one there), and what no name found to name.
    fn place_labels(&mut self) {
        let cell = self.inp.cell_ft;
        let w = self.inp.w;
        for l in 0..self.labels.len() {
            let (target, asked) = (self.labels[l].target, self.labels[l].asked);
            let kinds: &[&str] = match (target, asked) {
                (Target::Land, Some(k)) => if k == "continent" { &["continent"] } else { &["island"] },
                (Target::Land, None) => &["island", "continent"],
                (Target::Water, Some(k)) => match k {
                    "bay" => &["bay"],
                    _ => &["sea", "ocean"],
                },
                (Target::Water, None) => &["bay", "sea", "ocean"],
                (Target::Lake, _) => &["lake", "salt_lake", "salt_flat"],
                _ => &[],
            };
            if kinds.is_empty() || self.labels[l].used {
                continue;
            }
            // The feature holding most of its cells (a sample), the smallest of those; else the
            // nearest within reach (a point that asked for this kind).
            let cells = &self.labels[l].cells;
            let step = cells.len().div_ceil(64).max(1);
            let at = |k: u32| [(k as usize % w) as f64 * cell, (k as usize / w) as f64 * cell];
            let mut best: Option<(usize, f64, usize)> = None;
            for (fi, _) in self.out.features.iter().enumerate().filter(|(_, f)| kinds.contains(&f.kind)) {
                let shape = &self.out.shapes[fi];
                let hits = cells.iter().step_by(step).filter(|&&k| shape.distance(at(k)).0 == 0.0).count();
                let area = shape.area_sq_ft();
                if hits > 0 && best.is_none_or(|b| hits > b.0 || (hits == b.0 && area < b.1)) {
                    best = Some((hits, area, fi));
                }
            }
            let mut pick = best.map(|b| b.2);
            if pick.is_none() && cells.len() == 1 {
                let reach = if asked.is_some() { SNAP_MI } else { 2.0 } * 5280.0;
                pick = self
                    .out
                    .features
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| kinds.contains(&f.kind))
                    .map(|(fi, _)| (self.out.shapes[fi].distance(at(cells[0])).0, fi))
                    .filter(|&(d, _)| d <= reach)
                    .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                    .map(|(_, fi)| fi);
            }
            let Some(fi) = pick else { continue };
            self.labels[l].used = true;
            if let Some(&(_, by)) = self.named.iter().find(|n| n.0 == fi) {
                let other = self.inp.world.file.sketch.strokes[by].name.clone().unwrap_or_default();
                self.label_conflict(l, format!("This name falls on {}, already named {}: not used", self.out.features[fi].kind.replace('_', " "), other.trim()));
                continue;
            }
            self.out.features[fi].name = self.labels[l].name.clone();
            self.named.push((fi, self.labels[l].stroke));
        }
        for l in 0..self.labels.len() {
            if !self.labels[l].used {
                self.labels[l].used = true;
                let what = match self.labels[l].target {
                    Target::Region(g) => GROUPS[g].0,
                    Target::Outline => "region",
                    Target::Range => "range",
                    Target::Lake => "lake",
                    Target::River => "river",
                    Target::Land => "island",
                    Target::Water => "sea",
                };
                self.label_conflict(l, format!("No {what} here for this name: not used"));
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

/// A set of cells of a grid `w` wide, with each one's place in it: `at` is grid-sized scratch,
/// `u32::MAX` where not in the set (and again when this is dropped).
struct Local<'a> {
    w: usize,
    cells: &'a [u32],
    at: &'a mut [u32],
}

impl<'a> Local<'a> {
    fn new(w: usize, cells: &'a [u32], at: &'a mut [u32]) -> Self {
        for (i, &k) in cells.iter().enumerate() {
            at[k as usize] = i as u32;
        }
        Local { w, cells, at }
    }

    fn get(&self, k: usize) -> Option<usize> {
        self.at.get(k).filter(|&&i| i != u32::MAX).map(|&i| i as usize)
    }

    /// The set's 4-neighbours of its `i`th cell (by place).
    fn around(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let c = self.cells[i] as usize;
        let w = self.w;
        [c.wrapping_sub(1), c + 1, c.wrapping_sub(w), c + w].into_iter().filter(move |&nb| (nb % w).abs_diff(c % w) <= 1).filter_map(|nb| self.get(nb))
    }
}

impl Drop for Local<'_> {
    fn drop(&mut self) {
        for &k in self.cells {
            self.at[k as usize] = u32::MAX;
        }
    }
}

/// Grow seeds through the set: each cell goes to the seed set (`owner`) fewest steps away
/// (`cost` per cell entered; ties to the lower set), `dist` the steps. Both hold what is known
/// before (`u32::MAX`: nothing); `seeds` (set, place) start at 0.
fn grow(l: &Local, seeds: &[(u32, usize)], cost: &dyn Fn(usize) -> u32, dist: &mut [u32], owner: &mut [u32]) {
    use std::cmp::Reverse;
    let mut heap = std::collections::BinaryHeap::new();
    for &(o, i) in seeds {
        if dist[i] != 0 || o < owner[i] {
            (dist[i], owner[i]) = (0, o);
            heap.push(Reverse((0u32, o, i)));
        }
    }
    while let Some(Reverse((d, o, i))) = heap.pop() {
        if d != dist[i] || o != owner[i] {
            continue;
        }
        for j in l.around(i) {
            let nd = d + cost(l.cells[j] as usize);
            if nd < dist[j] || (nd == dist[j] && o < owner[j]) {
                (dist[j], owner[j]) = (nd, o);
                heap.push(Reverse((nd, o, j)));
            }
        }
    }
}

/// `comp` in parts of about `cap` cells (one part if it is no larger): seeds spread out from its
/// pole (each next where the ground is most steps from those before), moved a few times to their
/// parts' middles to even them out; then each cell to its nearest by `cost`.
fn cap_split(w: usize, comp: &[u32], cap: usize, cost: &dyn Fn(usize) -> u32, at: &mut [u32]) -> Vec<Vec<u32>> {
    let k = comp.len().div_ceil(cap.max(1));
    if k <= 1 {
        return vec![comp.to_vec()];
    }
    let (px, py) = piece_pole(w, comp, at);
    let l = Local::new(w, comp, at);
    let n = comp.len();
    let (mut dist, mut owner) = (vec![u32::MAX; n], vec![u32::MAX; n]);
    let mut seeds: Vec<usize> = vec![l.get(py as usize * w + px as usize).expect("in it")];
    grow(&l, &[(0, seeds[0])], &|_| 1, &mut dist, &mut owner);
    while seeds.len() < k {
        let far = (0..n).max_by(|&a, &b| dist[a].cmp(&dist[b]).then(comp[b].cmp(&comp[a]))).expect("cells");
        seeds.push(far);
        grow(&l, &[(seeds.len() as u32 - 1, far)], &|_| 1, &mut dist, &mut owner);
    }
    for round in 0..5 {
        if round > 0 {
            dist.fill(u32::MAX);
            owner.fill(u32::MAX);
            let all: Vec<(u32, usize)> = seeds.iter().enumerate().map(|(o, &i)| (o as u32, i)).collect();
            grow(&l, &all, &|_| 1, &mut dist, &mut owner);
        }
        // Each seed to the cell of its part nearest the part's middle.
        let mut sum = vec![(0.0f64, 0.0f64, 0usize); k];
        for i in 0..n {
            let s = &mut sum[owner[i] as usize];
            (s.0, s.1, s.2) = (s.0 + (comp[i] as usize % w) as f64, s.1 + (comp[i] as usize / w) as f64, s.2 + 1);
        }
        let mut best = vec![(f64::MAX, usize::MAX); k];
        for i in 0..n {
            let o = owner[i] as usize;
            let (mx, my) = (sum[o].0 / sum[o].2 as f64, sum[o].1 / sum[o].2 as f64);
            let (dx, dy) = ((comp[i] as usize % w) as f64 - mx, (comp[i] as usize / w) as f64 - my);
            if dx * dx + dy * dy < best[o].0 {
                best[o] = (dx * dx + dy * dy, i);
            }
        }
        seeds = best.into_iter().map(|b| b.1).filter(|&i| i != usize::MAX).collect();
    }
    dist.fill(u32::MAX);
    owner.fill(u32::MAX);
    let all: Vec<(u32, usize)> = seeds.iter().enumerate().map(|(o, &i)| (o as u32, i)).collect();
    grow(&l, &all, cost, &mut dist, &mut owner);
    let mut parts: Vec<Vec<u32>> = vec![Vec::new(); seeds.len()];
    for (i, &o) in owner.iter().enumerate() {
        parts[o as usize].push(comp[i]);
    }
    parts.retain(|p| !p.is_empty());
    parts
}

/// The cell of `piece` (a grid `w` wide) most steps in from its edge, cell coordinates (the
/// lowest such cell).
fn piece_pole(w: usize, piece: &[u32], at: &mut [u32]) -> (f64, f64) {
    let l = Local::new(w, piece, at);
    let mut depth = vec![u32::MAX; piece.len()];
    let mut queue: std::collections::VecDeque<usize> = Default::default();
    for i in 0..piece.len() {
        if l.around(i).count() < 4 {
            depth[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        for j in l.around(i) {
            if depth[j] == u32::MAX {
                depth[j] = depth[i] + 1;
                queue.push_back(j);
            }
        }
    }
    let best = (0..piece.len()).max_by(|&a, &b| depth[a].cmp(&depth[b]).then(piece[b].cmp(&piece[a]))).expect("cells");
    ((piece[best] as usize % w) as f64, (piece[best] as usize / w) as f64)
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
