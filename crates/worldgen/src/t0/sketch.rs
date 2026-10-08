//! Sketch constraints (`world::Sketch`) on a T0-style grid (the full grid, or the coarse one a
//! sketch preview uses):
//!
//! - land and sea strokes decide the land mask (outlines are coastlines; once any land is
//!   drawn, undrawn map is sea), with a natural, noisy coast unless drawn hard;
//! - ranges add rock uplift along their lines and massifs over their outlines, before erosion
//!   carves them;
//! - elevation outlines raise or lower the land after erosion; a sketched world never ponds
//!   water the land itself wouldn't (`fill`, `breach`);
//! - volcanoes are stamped where drawn (`volcano::drawn`);
//! - rivers are carved strictly downhill along their lines and fed at their sources, so the
//!   hydrology maps them; lakes are carved on their beds, each with its outlet;
//! - biome paint overrides the climate's biomes, blending at its edges;
//! - pins place settlements (snapped onto land, kept apart).
//!
//! What can't be honoured exactly (a river forced through a gorge, a pin at sea) is reported
//! as a conflict, with where it happened.

use serde::Serialize;

use super::biome::{ALL as BIOMES, Biome};
use super::hydro::NO_LAKE;
use super::settle::Tier;
use crate::World;
use crate::core::hash::FastMap;
use crate::core::noise::{fbm, ridged, smoothstep};
use crate::world::{SketchTool, Stroke};

const MI: f64 = 5280.0;

/// Something drawn that the world could not follow exactly.
#[derive(Clone, Debug, Serialize)]
pub struct Conflict {
    /// Index of the stroke in the sketch.
    pub stroke: usize,
    pub message: String,
    /// Where (world ft).
    pub x: f64,
    pub y: f64,
}

/// A settlement the sketch places.
#[derive(Clone, Debug)]
pub struct Pin {
    pub stroke: usize,
    pub tier: Tier,
    pub x: f64,
    pub y: f64,
    pub cell: usize,
}

/// The grid the sketch is applied to: `w` × `h` points `cell` ft apart, point (i, j) at
/// (i * cell, j * cell).
#[derive(Clone, Copy)]
pub struct Raster {
    pub w: usize,
    pub h: usize,
    pub cell: f64,
}

impl Raster {
    fn cell_of(&self, p: [f64; 2]) -> usize {
        let i = (p[0] / self.cell).round().clamp(0.0, (self.w - 1) as f64) as usize;
        let j = (p[1] / self.cell).round().clamp(0.0, (self.h - 1) as f64) as usize;
        j * self.w + i
    }

    fn at(&self, k: usize) -> [f64; 2] {
        [(k % self.w) as f64 * self.cell, (k / self.w) as f64 * self.cell]
    }
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    crate::core::sqrt(qx * qx + qy * qy)
}

/// Distance (ft) from every grid point to a polyline (closed: back to its start), up to
/// `reach`; farther points get `reach`.
fn distance(g: Raster, pts: &[[f64; 2]], closed: bool, reach: f64) -> Vec<f32> {
    let mut d = vec![reach as f32; g.w * g.h];
    let segs: Vec<([f64; 2], [f64; 2])> = if pts.len() == 1 {
        vec![(pts[0], pts[0])]
    } else {
        let mut s: Vec<_> = pts.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && pts.len() > 2 {
            s.push((pts[pts.len() - 1], pts[0]));
        }
        s
    };
    for (a, b) in segs {
        let i0 = ((a[0].min(b[0]) - reach) / g.cell).floor().max(0.0) as usize;
        let i1 = (((a[0].max(b[0]) + reach) / g.cell).ceil().max(0.0) as usize).min(g.w - 1);
        let j0 = ((a[1].min(b[1]) - reach) / g.cell).floor().max(0.0) as usize;
        let j1 = (((a[1].max(b[1]) + reach) / g.cell).ceil().max(0.0) as usize).min(g.h - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let k = j * g.w + i;
                let v = seg_dist([i as f64 * g.cell, j as f64 * g.cell], a, b) as f32;
                if v < d[k] {
                    d[k] = v;
                }
            }
        }
    }
    d
}

/// Grid points inside a closed outline (even-odd, by scanline).
fn inside(g: Raster, pts: &[[f64; 2]]) -> Vec<bool> {
    let mut out = vec![false; g.w * g.h];
    if pts.len() < 3 {
        return out;
    }
    let mut xs: Vec<f64> = Vec::new();
    for j in 0..g.h {
        let y = j as f64 * g.cell;
        xs.clear();
        for e in 0..pts.len() {
            let (a, b) = (pts[e], pts[(e + 1) % pts.len()]);
            if (a[1] <= y) != (b[1] <= y) {
                xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks(2) {
            if pair.len() < 2 {
                break;
            }
            let i0 = (pair[0] / g.cell).ceil().max(0.0) as usize;
            let i1 = (pair[1] / g.cell).floor().min((g.w - 1) as f64);
            if i1 < 0.0 {
                continue;
            }
            for i in i0..=(i1 as usize) {
                out[j * g.w + i] = true;
            }
        }
    }
    out
}

/// How far into a stroke's area each point is (ft): positive inside a closed outline or
/// within the brush radius of the line, negative outside; clamped to ±`reach`.
fn signed(g: Raster, s: &Stroke, reach: f64) -> Vec<f64> {
    if s.closed && s.pts.len() >= 3 {
        let d = distance(g, &s.pts, true, reach);
        let ins = inside(g, &s.pts);
        d.iter().zip(&ins).map(|(&d, &i)| if i { d as f64 } else { -(d as f64) }).collect()
    } else {
        let d = distance(g, &s.pts, false, reach + s.radius_ft);
        d.iter().map(|&d| (s.radius_ft - d as f64).clamp(-reach, reach)).collect()
    }
}

/// Unit coordinates (as `plates` uses) of grid point `k`.
fn unit(world: &World, g: Raster, k: usize) -> (f64, f64) {
    let x = (k % g.w) as f64 * g.cell / world.geom.map_w_ft;
    let y = (k / g.w) as f64 * g.cell / world.geom.map_w_ft;
    (x, y)
}

fn strokes(world: &World, tool: SketchTool) -> impl Iterator<Item = (usize, &Stroke)> {
    world.file.sketch.strokes.iter().enumerate().filter(move |(_, s)| s.tool == tool)
}

/// The land signal from drawn land and sea, as a replacement for the plates' crust field
/// (land where it is above 0), or `None` if the sketch draws neither. `crust` and `thr` are the
/// plates' own field and threshold, kept where only sea is drawn.
pub fn land_crust(world: &World, g: Raster, crust: &[f64], thr: f64) -> Option<Vec<f64>> {
    let lands: Vec<&Stroke> = strokes(world, SketchTool::Land).map(|(_, s)| s).collect();
    let seas: Vec<&Stroke> = strokes(world, SketchTool::Sea).map(|(_, s)| s).collect();
    if lands.is_empty() && seas.is_empty() {
        return None;
    }
    let n = g.w * g.h;
    let reach = 60.0 * MI;
    // A natural coast wanders off the drawn line: fractal, from bays and headlands tens of
    // miles across down to coves; a hard one keeps within a few hundred yards (ragged, not
    // ruled).
    let s_coast = world.stream("t0.sketch.coast");
    let wander: Vec<f64> = (0..n)
        .map(|k| {
            let (x, y) = unit(world, g, k);
            // Wavelengths ~200, 60, 20 and 6 mi; amplitudes falling more slowly than them
            // (a coast's fractal roughness).
            let mut v = 0.0;
            for (o, (freq, amp)) in [(6.0, 18.0), (20.0, 10.0), (60.0, 5.0), (200.0, 2.2)].into_iter().enumerate() {
                v += amp * fbm(s_coast ^ (o as u64 * 0x9e37), x * freq, y * freq, 2, 2.0, 0.5);
            }
            MI * v
        })
        .collect();
    // (The wander's amplitudes add up to ~35 mi: a hard coast's to ~0.3 mi.)
    let amount = |s: &Stroke| if s.hard { 0.0085 } else { 1.0 };
    // Signals in miles: positive on drawn land (or sea), the coast where they cross zero.
    let mut land = vec![-reach / MI; n];
    for s in &lands {
        let a = amount(s);
        for (k, v) in signed(g, s, reach).into_iter().enumerate() {
            land[k] = land[k].max((v + a * wander[k]) / MI);
        }
    }
    let mut sea = vec![f64::NEG_INFINITY; n];
    for s in &seas {
        let a = amount(s);
        for (k, v) in signed(g, s, reach).into_iter().enumerate() {
            sea[k] = sea[k].max((v - a * wander[k]) / MI);
        }
    }
    // Settlements pinned on the coast keep the ground they stand on: only their own point
    // (within about half a cell), so a pin on the coast stays on the coast. One farther out
    // (more than a couple of miles into the sea, as drawn or as the natural coast wandered) is
    // moved onto the nearest land by `pins`, which says so.
    if !lands.is_empty() {
        let near: Vec<&Stroke> = strokes(world, SketchTool::Pin).map(|(_, s)| s).filter(|s| land[g.cell_of(s.pts[0])] >= -2.0).collect();
        for s in near {
            let reach = 0.75 * g.cell;
            for (k, d) in distance(g, &s.pts[..1], false, reach).into_iter().enumerate() {
                // (Points out of reach hold `reach` as an f32.)
                if d < reach as f32 {
                    land[k] = land[k].max((reach - d as f64) / MI);
                }
            }
        }
    }
    let edge = 4.0;
    Some(
        (0..n)
            .map(|k| {
                let own = if lands.is_empty() { (crust[k] - thr) * 10.0 } else { land[k] };
                let mut c = own.min(-sea[k]);
                // The map's border stays sea.
                let (i, j) = ((k % g.w) as f64, (k / g.w) as f64);
                let to_edge = i.min(j).min((g.w - 1) as f64 - i).min((g.h - 1) as f64 - j);
                c -= 100.0 * (1.0 - smoothstep(0.0, edge, to_edge));
                c
            })
            .collect(),
    )
}

/// Rock uplift along drawn ranges and over drawn massifs (added to the plates' field before
/// erosion, which halves when either is drawn). `reference` (the plates' field with all their
/// mountains, when fewer are kept; else empty) gets the same.
pub fn add_ranges(world: &World, g: Raster, uplift: &mut [f64], reference: &mut [f64]) {
    let s_rid = world.stream("t0.sketch.range");
    let rugged = world.params().ruggedness.max(0.3);
    // Drawn ranges are the main ones: the plates' own ranges are kept, but lower.
    if strokes(world, SketchTool::Range).chain(strokes(world, SketchTool::Massif)).next().is_some() {
        uplift.iter_mut().for_each(|u| *u *= 0.5);
        reference.iter_mut().for_each(|u| *u *= 0.5);
    }
    let mut add = |k: usize, v: f64| {
        uplift[k] += v;
        if let Some(r) = reference.get_mut(k) {
            *r += v;
        }
    };
    for (_, s) in strokes(world, SketchTool::Range) {
        let r = s.radius_ft.max(2.0 * g.cell);
        let amp = (0.2 + 1.3 * s.strength) * rugged;
        let d = distance(g, &s.pts, false, 2.5 * r);
        for (k, &d) in d.iter().enumerate() {
            let d = d as f64;
            if d >= 2.5 * r {
                continue;
            }
            let (x, y) = unit(world, g, k);
            let crest = 0.6 + 0.5 * ridged(s_rid, x * 10.0, y * 10.0, 4);
            add(k, amp * crest * libm::exp(-(d / r) * (d / r)));
        }
    }
    let s_mas = world.stream("t0.sketch.massif");
    for (si, s) in strokes(world, SketchTool::Massif) {
        let amp = (0.2 + 1.3 * s.strength) * rugged;
        let foot = s.radius_ft.max(g.cell);
        let area = Area::of(g, &s.pts, foot);
        let Some(deepest) = area.cells().map(|(_, d)| d).reduce(f64::max) else { continue };
        // Ridges run along the trend (as drawn, else the outline's long axis): ~70 mi long,
        // ~20 mi apart, eroded into valleys between them.
        let angle = s.trend.map_or_else(|| area.long_axis(g), f64::to_radians);
        let (ca, sa) = (libm::cos(angle), libm::sin(angle));
        let seed = s_mas ^ crate::core::rng::mix64(si as u64);
        // Up from the edge (foothills outside it) to the core, the inner 40% of its depth.
        let core = 0.6 * deepest;
        let outside = distance(g, &s.pts, true, foot);
        for k in area.bbox(g) {
            let d = area.depth(g, k);
            let f = if d > 0.0 {
                0.15 + 0.85 * smoothstep(0.0, core, d)
            } else {
                let o = outside[k] as f64;
                if o >= foot {
                    continue;
                }
                let t = 1.0 - o / foot;
                0.15 * t * t
            };
            let [x, y] = g.at(k);
            let (u, v) = ((x * ca + y * sa) / MI, (-x * sa + y * ca) / MI);
            add(k, amp * f * (0.5 + 0.6 * ridged(seed, u / 70.0, v / 20.0, 4)));
        }
    }
}

/// The grid points inside a closed outline, with how deep inside each is (ft, by a chamfer
/// distance to the nearest point outside), over the outline's box (grown by `margin` ft).
struct Area {
    i0: usize,
    j0: usize,
    w: usize,
    h: usize,
    /// Over the box: depth (ft), 0 outside.
    depth: Vec<f64>,
}

impl Area {
    fn of(g: Raster, pts: &[[f64; 2]], margin: f64) -> Area {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in pts {
            (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
        }
        // (A point past the outline on every side, so the box's border is outside it.)
        let lo = |v: f64| (((v - margin) / g.cell).floor() - 1.0).max(0.0) as usize;
        let (i1, j1) = ((((x1 + margin) / g.cell).ceil().max(0.0) as usize + 1).min(g.w - 1), (((y1 + margin) / g.cell).ceil().max(0.0) as usize + 1).min(g.h - 1));
        let (i0, j0) = (lo(x0).min(i1), lo(y0).min(j1));
        let (w, h) = (i1 - i0 + 1, j1 - j0 + 1);
        let sub = Raster { w, h, cell: g.cell };
        let shifted: Vec<[f64; 2]> = pts.iter().map(|p| [p[0] - i0 as f64 * g.cell, p[1] - j0 as f64 * g.cell]).collect();
        let ins = inside(sub, &shifted);
        let out: Vec<bool> = ins.iter().map(|i| !i).collect();
        let depth = super::climate::distance_to(w, h, &out).into_iter().zip(&ins).map(|(d, &i)| if i { d * g.cell } else { 0.0 }).collect();
        Area { i0, j0, w, h, depth }
    }

    /// Grid points of the box.
    fn bbox(&self, g: Raster) -> impl Iterator<Item = usize> + '_ {
        (0..self.h).flat_map(move |j| (0..self.w).map(move |i| (self.j0 + j) * g.w + self.i0 + i))
    }

    /// How deep inside grid point `k` is (0 outside, or off the box).
    fn depth(&self, g: Raster, k: usize) -> f64 {
        let (i, j) = (k % g.w, k / g.w);
        if i < self.i0 || j < self.j0 || i >= self.i0 + self.w || j >= self.j0 + self.h {
            return 0.0;
        }
        self.depth[(j - self.j0) * self.w + i - self.i0]
    }

    /// The points inside (box-local indices, see `grid`) and their depth.
    fn cells(&self) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.depth.iter().enumerate().filter(|(_, d)| **d > 0.0).map(|(k, &d)| (k, d))
    }

    /// The grid index of a box-local one.
    fn grid(&self, g: Raster, local: usize) -> usize {
        (self.j0 + local / self.w) * g.w + self.i0 + local % self.w
    }

    /// The direction (radians) of the area's long axis.
    fn long_axis(&self, g: Raster) -> f64 {
        let pts: Vec<[f64; 2]> = self.cells().map(|(k, _)| g.at(self.grid(g, k))).collect();
        let n = pts.len().max(1) as f64;
        let (mx, my) = (pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n);
        let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
        for p in &pts {
            let (dx, dy) = (p[0] - mx, p[1] - my);
            (sxx, syy, sxy) = (sxx + dx * dx, syy + dy * dy, sxy + dx * dy);
        }
        0.5 * libm::atan2(2.0 * sxy, sxx - syy)
    }
}

/// Raise or lower the land inside drawn elevation outlines by their `delta_ft` (after erosion,
/// so no ridges form; before volcanoes, rivers and lakes), easing in across an edge `radius_ft`
/// wide centred on the line (wavering unless drawn exact). Never above the highest ground.
/// (`breach` then keeps water from ponding where it didn't stand before.)
pub fn elevate(world: &World, g: Raster, land: &[bool], height: &mut [f64]) {
    let p = world.params();
    let s_edge = world.stream("t0.sketch.elevation");
    for (_, s) in strokes(world, SketchTool::Elevation) {
        let delta = s.delta_ft.unwrap_or(0.0);
        let width = s.radius_ft.max(g.cell);
        for (k, v) in signed(g, s, width).into_iter().enumerate() {
            if !land[k] || v <= -width {
                continue;
            }
            let [x, y] = g.at(k);
            let wobble = if s.hard { 0.0 } else { 0.4 * width * fbm(s_edge, x / (25.0 * MI), y / (25.0 * MI), 3, 2.0, 0.5) };
            let m = smoothstep(-0.5 * width, 0.5 * width, v + wobble);
            if m > 0.0 {
                height[k] = (height[k] + delta * m).min(p.sea_level_ft + p.max_elev_ft);
            }
        }
    }
}

/// How deep water would stand at each point (ft; 0 where it drains).
pub fn depths(g: Raster, land: &[bool], height: &[f64]) -> Vec<f64> {
    let sinks: Vec<bool> = land.iter().map(|l| !l).collect();
    super::flood::priority_flood(g.w, g.h, height, &sinks, 0.01).filled.iter().zip(height).map(|(f, h)| f - h).collect()
}

/// Fill hollows where water would stand deeper than `before` (`depths` of the ground it should
/// hold water like) up to where it would stand that deep: flat floors at the brim.
pub fn fill(g: Raster, land: &[bool], height: &mut [f64], before: &[f64]) {
    let now = depths(g, land, height);
    for k in 0..g.w * g.h {
        if land[k] && now[k] > before[k] + 1.0 {
            height[k] += now[k] - before[k];
        }
    }
}

/// Cut outlets for water the ground would pond deeper than `before` (`depths` of the ground it
/// should hold water like): from each such hollow's lowest point (at the depth it had), down
/// the way water would spill, cut until the ground beyond drains lower, as a river older than
/// the land around it keeps its course. A few rounds (an outlet can open into another hollow).
pub fn breach(g: Raster, land: &[bool], height: &mut [f64], before: &[f64]) {
    let sinks: Vec<bool> = land.iter().map(|l| !l).collect();
    for _ in 0..4 {
        let fl = super::flood::priority_flood(g.w, g.h, height, &sinks, 0.01);
        let deeper: Vec<bool> = (0..g.w * g.h).map(|k| land[k] && fl.filled[k] - height[k] > before[k] + 1.0).collect();
        let (rec, _) = super::flood::receivers(g.w, g.h, &fl.filled);
        let mut seen = vec![false; g.w * g.h];
        let mut cut = false;
        for s in 0..g.w * g.h {
            if !deeper[s] || seen[s] {
                continue;
            }
            // The hollow, and its lowest point at the depth it had.
            let mut comp = vec![s];
            seen[s] = true;
            let mut q = 0;
            while q < comp.len() {
                let c = comp[q];
                q += 1;
                for nb in neighbours8(g, c) {
                    if deeper[nb] && !seen[nb] {
                        seen[nb] = true;
                        comp.push(nb);
                    }
                }
            }
            let level = |k: usize| height[k] + before[k];
            let low = *comp.iter().min_by(|&&a, &&b| level(a).total_cmp(&level(b)).then(a.cmp(&b))).unwrap();
            let (mut cur, mut z) = (low, level(low));
            for _ in 0..g.w * g.h {
                let k = rec[cur] as usize;
                if k == cur || !land[k] || fl.filled[k] < z {
                    break;
                }
                z -= MIN_FALL;
                if height[k] > z {
                    height[k] = z;
                    cut = true;
                }
                z = height[k];
                cur = k;
            }
        }
        if !cut {
            break;
        }
    }
}

/// Ranges and massifs drawn mostly over the sea (they raise nothing there).
pub fn check_ranges(world: &World, g: Raster, land: &[bool], conflicts: &mut Vec<Conflict>) {
    for (i, s) in strokes(world, SketchTool::Range) {
        let wet = s.pts.iter().filter(|p| !land[g.cell_of(**p)]).count();
        if wet * 2 > s.pts.len() {
            let p = s.pts[s.pts.len() / 2];
            conflicts.push(Conflict { stroke: i, message: "This range is mostly at sea: draw land under it".into(), x: p[0], y: p[1] });
        }
    }
    for (i, s) in strokes(world, SketchTool::Massif) {
        let area = Area::of(g, &s.pts, 0.0);
        let (wet, all) = area.cells().fold((0, 0), |(w, a), (k, _)| (w + usize::from(!land[area.grid(g, k)]), a + 1));
        if wet * 2 > all {
            let p = s.pts[0];
            conflicts.push(Conflict { stroke: i, message: "This massif is mostly at sea: draw land under it".into(), x: p[0], y: p[1] });
        }
    }
}

/// A drawn lake on the grid.
pub struct DrawnLake {
    pub stroke: usize,
    /// Its cells (inside the outline) and how deep inside each is (ft).
    cells: Vec<(usize, f64)>,
    /// The water's level (ft; set when carved).
    pub level: f64,
    pub salt: bool,
    /// A drawn river leaves it (its outlet).
    pub outflow: bool,
}

/// The sketch's lakes: where each lies (`lakes`, before the rivers), then carved (`carve_lakes`,
/// after them).
#[derive(Default)]
pub struct Lakes {
    pub list: Vec<DrawnLake>,
    /// Per grid point: the drawn lake it is in, or `NONE`.
    pub of: Vec<u32>,
}

impl Lakes {
    fn at(&self, k: usize) -> Option<usize> {
        self.of.get(k).filter(|&&l| l != NONE).map(|&l| l as usize)
    }
}

/// The drawn rivers as carved: each course's cells (source to mouth) and which cells are
/// channels.
pub struct Courses {
    pub cells: Vec<Vec<usize>>,
    pub channel: Vec<bool>,
}

/// Where the drawn lakes lie: the cells inside each outline (the cell under it, if it is
/// smaller than a cell), but those beside the sea or another lake, so a strip of shore always
/// keeps them apart.
pub fn lakes(world: &World, g: Raster, land: &[bool], conflicts: &mut Vec<Conflict>) -> Lakes {
    let mut lakes = Lakes { of: vec![NONE; g.w * g.h], ..Default::default() };
    for (si, s) in strokes(world, SketchTool::Lake) {
        let area = Area::of(g, &s.pts, 0.0);
        let mut cells: Vec<(usize, f64)> = area.cells().map(|(k, d)| (area.grid(g, k), d)).collect();
        if cells.is_empty() {
            let n = s.pts.len() as f64;
            cells.push((g.cell_of([s.pts.iter().map(|p| p[0]).sum::<f64>() / n, s.pts.iter().map(|p| p[1]).sum::<f64>() / n]), 0.5 * g.cell));
        }
        let had = cells.len();
        let apart = |k: usize| std::iter::once(k).chain(neighbours8(g, k)).all(|c| land[c] && lakes.of[c] == NONE);
        cells.retain(|&(k, _)| apart(k));
        let at = s.pts[0];
        if cells.is_empty() {
            conflicts.push(Conflict { stroke: si, message: "This lake lies in the sea or another lake: no lake made".into(), x: at[0], y: at[1] });
            continue;
        }
        if cells.len() < had {
            conflicts.push(Conflict { stroke: si, message: "This lake reaches the sea or another lake: a strip of shore is kept between them".into(), x: at[0], y: at[1] });
        }
        let id = lakes.list.len() as u32;
        for &(k, _) in &cells {
            lakes.of[k] = id;
        }
        lakes.list.push(DrawnLake { stroke: si, cells, level: 0.0, salt: s.salt, outflow: false });
    }
    lakes
}

/// Carve the drawn lakes onto the ground the rivers left: each basin dug below its level (deeper
/// toward the middle) and its shore raised where lower than the water, but at its outlet, from
/// which it drains at that level (cut down the way water would go where the ground beyond is
/// higher). The outlet is where a drawn river leaves it, else the lowest point of its shore (a
/// river's mouth into it aside); the level is drawn, else the outlet's.
pub fn carve_lakes(world: &World, g: Raster, height: &mut [f64], land: &[bool], lakes: &mut Lakes, courses: &Courses, conflicts: &mut Vec<Conflict>) {
    if lakes.list.is_empty() {
        return;
    }
    let sea = world.params().sea_level_ft;
    let mut outlets = Vec::new();
    for (id, l) in lakes.list.iter_mut().enumerate() {
        let id = id as u32;
        let s = &world.file.sketch.strokes[l.stroke];
        let at = s.pts[0];
        let mut shore: Vec<usize> = l.cells.iter().flat_map(|&(k, _)| neighbours8(g, k)).filter(|&nb| lakes.of[nb] != id).collect();
        shore.sort_unstable();
        shore.dedup();
        // Drawn rivers at its shore: where they leave it (outflows) and where they come in.
        let (mut outs, mut ins) = (Vec::new(), Vec::new());
        for c in &courses.cells {
            for w in c.windows(2) {
                match (lakes.of[w[0]] == id, lakes.of[w[1]] == id) {
                    (true, false) => outs.push(w[1]),
                    (false, true) => ins.push(w[0]),
                    _ => {}
                }
            }
            if c.last().is_some_and(|&e| shore.contains(&e)) {
                ins.push(*c.last().unwrap());
            }
        }
        let lowest = |v: &mut dyn Iterator<Item = usize>| v.min_by(|&a, &b| height[a].total_cmp(&height[b]).then(a.cmp(&b)));
        let outlet = lowest(&mut outs.iter().copied().filter(|k| shore.contains(k))).or_else(|| lowest(&mut shore.iter().copied().filter(|k| !ins.contains(k)))).unwrap_or(shore[0]);
        l.outflow = outs.contains(&outlet);
        l.level = s.level_ft.map_or(height[outlet], |v| sea + v);
        let level = l.level;
        // The basin: 30 ft deep at the shore, a foot more every 125 ft out (800 at most).
        for &(k, d) in &l.cells {
            height[k] = height[k].min(level - (30.0 + 0.008 * d).min(800.0));
        }
        height[outlet] = height[outlet].min(level);
        // (Drawn rivers' channels at the shore keep their beds: where they come in, they are
        // above the water.)
        let mut raised = 0.0f64;
        for &k in &shore {
            if k != outlet && !courses.channel.get(k).is_some_and(|&c| c) && land[k] {
                raised = raised.max(level + 2.0 - height[k]);
                height[k] = height[k].max(level + 2.0);
            }
        }
        if raised > 50.0 {
            conflicts.push(Conflict { stroke: l.stroke, message: format!("This lake stands up to {:.0} ft above the land around it: its shores are raised", (raised / 10.0).round() * 10.0), x: at[0], y: at[1] });
        }
        outlets.push(outlet);
    }
    // From each outlet, down the way water would flow, cut until the ground beyond drains lower
    // than the lake.
    let sinks: Vec<bool> = land.iter().map(|l| !l).collect();
    let fl = super::flood::priority_flood(g.w, g.h, height, &sinks, 0.01);
    let (rec, _) = super::flood::receivers(g.w, g.h, &fl.filled);
    for (l, &outlet) in lakes.list.iter().zip(&outlets) {
        let (mut cur, mut z, mut cut) = (outlet, l.level, 0.0f64);
        loop {
            let k = rec[cur] as usize;
            if k == cur || !land[k] || fl.filled[k] < z || lakes.of[k] != NONE {
                break;
            }
            z -= MIN_FALL;
            if height[k] > z {
                cut = cut.max(height[k] - z);
                height[k] = z;
            }
            z = height[k];
            cur = k;
        }
        if cut > 300.0 {
            let p = g.at(outlet);
            conflicts.push(Conflict { stroke: l.stroke, message: format!("To drain at its level this lake's outlet cuts a {:.0}-ft gorge", (cut / 100.0).round() * 100.0), x: p[0], y: p[1] });
        }
    }
}

/// Grid cells along a polyline, in order, 8-connected, without repeats (a loop is cut out).
fn cells_along(g: Raster, pts: &[[f64; 2]]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    let mut at = std::collections::HashMap::new();
    let mut add = |k: usize, out: &mut Vec<usize>| {
        if out.last() == Some(&k) {
            return;
        }
        if let Some(&p) = at.get(&k) {
            for c in out.drain(p + 1..) {
                at.remove(&c);
            }
            return;
        }
        at.insert(k, out.len());
        out.push(k);
    };
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]));
        let steps = (len / (0.4 * g.cell)).ceil().max(1.0) as usize;
        for t in 0..=steps {
            let f = t as f64 / steps as f64;
            add(g.cell_of([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]), &mut out);
        }
    }
    if pts.len() == 1 {
        add(g.cell_of(pts[0]), &mut out);
    }
    out
}

/// How far (ft) past its drawn end a river is carried on to the sea or another river.
const REACH_FT: f64 = 10.0 * MI;
/// The least fall (ft per cell) of a drawn river, so it keeps flowing over a flat plain.
const MIN_FALL: f64 = 0.05;
/// A river's last cell above the sea it flows into (ft).
const MOUTH_FT: f64 = 3.0;
/// A valley's sides: how fast they climb away from the river (per ft across).
const SIDE: f64 = 0.05;
pub(crate) const NONE: u32 = u32::MAX;

/// Where a drawn river's course ends.
#[derive(Clone, Copy, PartialEq)]
enum End {
    Sea,
    /// Joins another course at this cell (one of that course's cells).
    Join(usize),
    /// Flows into this drawn lake.
    Lake(usize),
    /// Nowhere to go: it ends on land.
    Inland,
}

/// A drawn river on the grid: its cells from source to mouth (land only; a join's cell is the
/// other river's).
struct Course {
    stroke: usize,
    cells: Vec<usize>,
    end: Option<End>,
    /// The valley's half-width (cells) and the feed at the source.
    base: f64,
    feed: f64,
}

fn neighbours8(g: Raster, k: usize) -> impl Iterator<Item = usize> {
    let (i, j) = ((k % g.w) as i64, (k / g.w) as i64);
    (-1..=1i64).flat_map(move |dj| (-1..=1i64).map(move |di| (i + di, j + dj))).filter(move |&(x, y)| x >= 0 && y >= 0 && x < g.w as i64 && y < g.h as i64 && (x, y) != (i, j)).map(move |(x, y)| y as usize * g.w + x as usize)
}

/// Carve the drawn rivers into `height` and return the discharge (mm·cells) to add at each
/// source so the hydrology maps them as rivers, and which cells their channels take.
///
/// - A river runs the way it was drawn (unless drawn from the sea inland).
/// - It ends at the sea, or joins the first other drawn river it meets (a tributary ends on
///   that river's channel, at its level). One that stops short of the sea or another river
///   within `REACH_FT` is carried on to it in a straight line (through a drawn lake it ends in).
///   Drawn lakes are carved after (`carve_lakes`), on the rivers' beds.
/// - Its profile never rises and falls at least `MIN_FALL` per cell (more on a high, short
///   course, up to a foot), and stays above the water it ends in: where that needs the river
///   above the ground, the ground along it is raised rather than the river dug under the sea.
/// - Every course is set first, then the valleys: banks a little above the water (never over
///   any river's channel), sides sloping up to the valley's edge.
pub fn carve_rivers(world: &World, g: Raster, height: &mut [f64], land: &[bool], lakes: &Lakes, conflicts: &mut Vec<Conflict>) -> (Vec<(usize, f64)>, Courses) {
    let threshold = super::hydro::RIVER_Q / world.params().river_density;
    let sea = world.params().sea_level_ft;
    let n = g.w * g.h;

    // Courses: the drawn line's land cells, up to the sea.
    let mut courses: Vec<Course> = Vec::new();
    for (si, s) in strokes(world, SketchTool::River) {
        let mut path = cells_along(g, &s.pts);
        if path.len() < 2 {
            continue;
        }
        // Rivers run from their first point; one drawn from the sea inland is turned round.
        if !land[path[0]] && land[*path.last().unwrap()] {
            path.reverse();
        }
        let Some(start) = path.iter().position(|&k| land[k]) else {
            let p = s.pts[0];
            conflicts.push(Conflict { stroke: si, message: "This river is entirely at sea".into(), x: p[0], y: p[1] });
            continue;
        };
        let mut cells: Vec<usize> = Vec::new();
        let mut end: Option<End> = None;
        for (idx, &k) in path.iter().enumerate().skip(start) {
            if !land[k] {
                if path[idx..].iter().filter(|&&c| land[c]).count() >= 6 {
                    let p = g.at(k);
                    conflicts.push(Conflict { stroke: si, message: "This river reaches the sea before its end; the rest is ignored".into(), x: p[0], y: p[1] });
                }
                end = Some(End::Sea);
                break;
            }
            cells.push(k);
        }
        if cells.len() < 3 {
            let p = g.at(cells.first().copied().unwrap_or(path[start]));
            conflicts.push(Conflict { stroke: si, message: "This river is too short to map".into(), x: p[0], y: p[1] });
            continue;
        }
        let (base, feed) = ((s.radius_ft / g.cell).clamp(1.0, 5.0), threshold * (1.2 + 10.0 * s.strength * s.strength));
        courses.push(Course { stroke: si, cells, end, base, feed });
    }

    // Where each one ends: those reaching the sea first; then each joins the first resolved
    // course it touches (a course whose first touch is one not yet resolved waits for it), or is
    // carried on to the sea or a resolved course within reach. When none can go on, the first
    // waiting one is settled on its own.
    let mut owner = vec![NONE; n];
    let claim = |owner: &mut [u32], cells: &[usize], ci: usize| {
        for &k in cells {
            if owner[k] == NONE {
                owner[k] = ci as u32;
            }
        }
    };
    for (ci, c) in courses.iter().enumerate() {
        claim(&mut owner, &c.cells, ci);
    }
    let mut order: Vec<usize> = (0..courses.len()).filter(|&ci| courses[ci].end.is_some()).collect();
    let mut resolved: Vec<bool> = courses.iter().map(|c| c.end.is_some()).collect();
    // The first cell (past its source) where a course touches another: (index, the other's cell).
    // Not where the other ends (it flows into this one there: a river doesn't wait for, or
    // join, its own tributary).
    let touch = |courses: &[Course], owner: &[u32], ci: usize, only: &dyn Fn(usize) -> bool| -> Option<(usize, usize)> {
        let tail = |o: usize, c: usize| {
            let cs = &courses[o].cells;
            let last = *cs.last().unwrap();
            cs[cs.len().saturating_sub(5)..].contains(&c) || neighbours8(g, last).any(|x| x == c)
        };
        for (idx, &k) in courses[ci].cells.iter().enumerate().skip(2) {
            let own = |c: usize| owner[c] != NONE && owner[c] as usize != ci && only(owner[c] as usize) && !tail(owner[c] as usize, c);
            if own(k) {
                return Some((idx, k));
            }
            if let Some(nb) = neighbours8(g, k).find(|&c| own(c)) {
                return Some((idx + 1, nb));
            }
        }
        None
    };
    let join = |courses: &mut [Course], owner: &mut [u32], ci: usize, idx: usize, k: usize| {
        for &c in &courses[ci].cells[idx..] {
            if owner[c] == ci as u32 {
                owner[c] = NONE;
            }
        }
        courses[ci].cells.truncate(idx);
        courses[ci].end = Some(End::Join(k));
    };
    while order.len() < courses.len() {
        let mut progress = false;
        for ci in 0..courses.len() {
            if resolved[ci] {
                continue;
            }
            match touch(&courses, &owner, ci, &|_| true) {
                Some((idx, k)) if resolved[owner[k] as usize] => join(&mut courses, &mut owner, ci, idx, k),
                // Flows into one not yet settled: wait for it.
                Some(_) => continue,
                None => {
                    let had = courses[ci].cells.len();
                    let Some(end) = carry_on(g, land, lakes, &owner, &resolved, &mut courses[ci], ci) else { continue };
                    courses[ci].end = Some(end);
                    claim(&mut owner, &courses[ci].cells[had..], ci);
                }
            }
            resolved[ci] = true;
            order.push(ci);
            progress = true;
        }
        if !progress {
            // Courses waiting on each other: the first goes on by itself, into the first settled
            // course it touches, or on to the sea or one in reach.
            let ci = (0..courses.len()).find(|&ci| !resolved[ci]).unwrap();
            if let Some((idx, k)) = touch(&courses, &owner, ci, &|o| resolved[o]) {
                join(&mut courses, &mut owner, ci, idx, k);
            } else {
                let had = courses[ci].cells.len();
                let end = carry_on(g, land, lakes, &owner, &resolved, &mut courses[ci], ci).unwrap_or(End::Inland);
                courses[ci].end = Some(end);
                claim(&mut owner, &courses[ci].cells[had..], ci);
            }
            resolved[ci] = true;
            order.push(ci);
        }
    }

    // A course into a drawn lake goes after those running through it: the lake will take its
    // level from where they leave it.
    let order = {
        let through = |cj: usize, l: usize| courses[cj].end != Some(End::Lake(l)) && courses[cj].cells.iter().any(|&k| lakes.at(k) == Some(l));
        let mut sorted = Vec::with_capacity(order.len());
        let mut placed = vec![false; courses.len()];
        let mut stack: Vec<(usize, bool)> = order.iter().rev().map(|&ci| (ci, false)).collect();
        while let Some((ci, ready)) = stack.pop() {
            if ready {
                sorted.push(ci);
                continue;
            }
            if placed[ci] {
                continue;
            }
            placed[ci] = true;
            stack.push((ci, true));
            let deps: Vec<usize> = match courses[ci].end {
                Some(End::Join(k)) if owner[k] != NONE => vec![owner[k] as usize],
                Some(End::Lake(l)) => (0..courses.len()).filter(|&cj| through(cj, l)).collect(),
                _ => Vec::new(),
            };
            stack.extend(deps.into_iter().rev().filter(|&d| !placed[d]).map(|d| (d, false)));
        }
        sorted
    };

    // Profiles, in that order (a tributary after the river it joins): each on the ground as the
    // valleys before it left it.
    let mut zc = vec![f64::NAN; n];
    // Valley cells: (side target, distance in cells, the river's level there, lifted).
    let mut valley: FastMap<usize, (f64, f64, f64, bool)> = FastMap::default();
    let mut feed = Vec::new();
    for &ci in &order {
        let c = &courses[ci];
        let si = c.stroke;
        let end = c.end.unwrap();
        // (A channel already set is the ground there: a later course never raises it.)
        let ground = |k: usize| {
            let h = valley.get(&k).map_or(height[k], |v| height[k].min(v.0));
            if zc[k].is_nan() { h } else { h.min(zc[k]) }
        };
        let m = c.cells.len();
        // The level the course must stay above at its last cell.
        let floor_end = match end {
            End::Sea => Some(sea + MOUTH_FT),
            End::Join(k) => Some(zc[k] + MIN_FALL),
            // The level the lake will take: where a river set before leaves it, else the lowest
            // point of its shore (never below the sea).
            End::Lake(l) => {
                let exit = order.iter().take_while(|&&cj| cj != ci).flat_map(|&cj| courses[cj].cells.windows(2)).filter(|w| lakes.at(w[0]) == Some(l) && lakes.at(w[1]) != Some(l)).map(|w| zc[w[1]]).fold(f64::INFINITY, f64::min);
                let level = if exit.is_finite() {
                    exit
                } else {
                    lakes.list[l].cells.iter().flat_map(|&(k, _)| neighbours8(g, k)).filter(|&k| lakes.at(k) != Some(l) && zc[k].is_nan() && !c.cells.contains(&k)).map(ground).fold(f64::INFINITY, f64::min)
                };
                Some(level.max(sea + MOUTH_FT) + MOUTH_FT)
            }
            End::Inland => None,
        };
        let h0 = ground(c.cells[0]);
        let low = floor_end.unwrap_or_else(|| ground(c.cells[m - 1]));
        let fall = ((h0 - low) / m as f64).clamp(MIN_FALL, 1.0);
        let floor = |idx: usize| floor_end.map_or(f64::NEG_INFINITY, |f| f + (m - 1 - idx) as f64 * MIN_FALL);
        let mut z = Vec::with_capacity(m);
        let mut lifted = Vec::with_capacity(m);
        let (mut cut, mut cut_at, mut lift, mut lift_at) = (0.0f64, c.cells[0], 0.0f64, c.cells[0]);
        for (idx, &k) in c.cells.iter().enumerate() {
            let h = ground(k);
            let v = if idx == 0 { h } else { h.min(z[idx - 1] - fall) }.max(floor(idx));
            if h - v > cut {
                (cut, cut_at) = (h - v, k);
            }
            if v - h > lift {
                (lift, lift_at) = (v - h, k);
            }
            z.push(v);
            lifted.push(v > h);
        }
        if cut > 800.0 {
            let p = g.at(cut_at);
            conflicts.push(Conflict { stroke: si, message: format!("To keep flowing downhill this river cuts a {:.0}-ft gorge", (cut / 100.0).round() * 100.0), x: p[0], y: p[1] });
        }
        if lift > 20.0 {
            let p = g.at(lift_at);
            let into = if end == End::Sea { "the sea" } else { "the river it joins" };
            conflicts.push(Conflict { stroke: si, message: format!("To flow into {into} this river runs up to {:.0} ft above the ground; the land along it is raised", lift.round()), x: p[0], y: p[1] });
        }
        if end == End::Inland {
            let p = g.at(c.cells[m - 1]);
            conflicts.push(Conflict { stroke: si, message: "This river ends inland, far from the sea or another river: its water may pool there".into(), x: p[0], y: p[1] });
        }
        for (idx, &k) in c.cells.iter().enumerate() {
            zc[k] = if zc[k].is_nan() { z[idx] } else { zc[k].min(z[idx]) };
        }
        // The valley: banks beside the channel, sides sloping up to the valley's edge. A deep cut
        // widens the valley until its sides meet the ground at a walkable slope (no canal-like
        // walls).
        for (idx, &k) in c.cells.iter().enumerate() {
            let (ci, cj) = ((k % g.w) as i64, (k / g.w) as i64);
            let lifted = lifted[idx];
            let reach = ((height[k] - z[idx]) / (g.cell * SIDE)).clamp(c.base, 14.0);
            let r = reach.ceil() as i64;
            for dj in -r..=r {
                for di in -r..=r {
                    let (i, j) = (ci + di, cj + dj);
                    if i < 0 || j < 0 || i >= g.w as i64 || j >= g.h as i64 || (di == 0 && dj == 0) {
                        continue;
                    }
                    let d = crate::core::sqrt((di * di + dj * dj) as f64);
                    if d > reach {
                        continue;
                    }
                    let nb = j as usize * g.w + i as usize;
                    let target = z[idx] + 2.0 + d * g.cell * SIDE;
                    let e = valley.entry(nb).or_insert((target, d, z[idx], lifted));
                    if target < e.0 {
                        *e = (target, d, z[idx], lifted);
                    }
                }
            }
        }
        feed.push((c.cells[0], c.feed));
        // (Fed again where it leaves a drawn lake, which may lose all it brings to the air.)
        for w in c.cells.windows(2) {
            if lakes.at(w[0]).is_some() && lakes.at(w[1]).is_none() {
                feed.push((w[1], c.feed));
            }
        }
    }

    // The valleys, never over a channel: banks between 10 ft above the water and the side's
    // height there (no levees standing over the plain, which would pond it); where the river
    // was raised above the ground, the ground beside it rises with it.
    let mut cells: Vec<(&usize, &(f64, f64, f64, bool))> = valley.iter().collect();
    cells.sort_by_key(|(k, _)| **k);
    for (&k, &(target, d, z, lifted)) in cells {
        if !land[k] || !zc[k].is_nan() || lakes.at(k).is_some() {
            continue;
        }
        height[k] = if d < 1.5 {
            height[k].clamp(z + 10.0, target)
        } else if lifted {
            height[k].clamp(z + 2.0 + (d - 1.5) * g.cell * 0.002, target)
        } else {
            height[k].min(target)
        };
    }
    for k in 0..n {
        if !zc[k].is_nan() {
            height[k] = zc[k];
        }
    }
    let channel = if courses.is_empty() { Vec::new() } else { zc.iter().map(|z| !z.is_nan()).collect() };
    (feed, Courses { cells: courses.into_iter().map(|c| c.cells).collect(), channel })
}

/// Carry a course that stops short on to the nearest sea cell or resolved course within
/// `REACH_FT` of its end (straight there; it joins whatever course it meets on the way). One
/// that ends in a drawn lake (or at its shore) is carried on through it, as far again as the
/// lake is wide: the lake will sit on its course. `None` when there is nothing in reach yet but
/// some course is still unresolved (one may come within reach); `Inland` (or `Lake`, ending at
/// one) when nothing can.
#[allow(clippy::too_many_arguments)]
fn carry_on(g: Raster, land: &[bool], lakes: &Lakes, owner: &[u32], resolved: &[bool], c: &mut Course, ci: usize) -> Option<End> {
    let last = *c.cells.last().unwrap();
    let (ei, ej) = ((last % g.w) as i64, (last / g.w) as i64);
    let goal = |k: usize| !land[k] || (owner[k] != NONE && owner[k] as usize != ci && resolved[owner[k] as usize]);
    let mut best: Option<(i64, usize)> = None;
    let lake = lakes.at(last).or_else(|| neighbours8(g, last).find_map(|k| lakes.at(k)));
    let across = lake.map_or(0.0, |l| {
        let cells = &lakes.list[l].cells;
        let (i0, i1) = cells.iter().fold((usize::MAX, 0), |(a, b), &(k, _)| (a.min(k % g.w), b.max(k % g.w)));
        let (j0, j1) = cells.iter().fold((usize::MAX, 0), |(a, b), &(k, _)| (a.min(k / g.w), b.max(k / g.w)));
        crate::core::sqrt(((i1 - i0) * (i1 - i0) + (j1 - j0) * (j1 - j0)) as f64) * g.cell
    });
    let reach = (crate::core::ceil((REACH_FT + across) / g.cell) as i64).max(2);
    for dj in -reach..=reach {
        for di in -reach..=reach {
            let (i, j) = (ei + di, ej + dj);
            let d2 = di * di + dj * dj;
            if i < 0 || j < 0 || i >= g.w as i64 || j >= g.h as i64 || d2 > reach * reach {
                continue;
            }
            let k = j as usize * g.w + i as usize;
            if goal(k) && best.is_none_or(|b| d2 < b.0) {
                best = Some((d2, k));
            }
        }
    }
    let Some((_, target)) = best else {
        return if resolved.iter().enumerate().all(|(i, &r)| r || i == ci) { Some(lake.map_or(End::Inland, End::Lake)) } else { None };
    };
    // Its last cell is another course's: it joins there.
    if target == last {
        c.cells.pop();
        return Some(End::Join(last));
    }
    let mut extra = Vec::new();
    for k in cells_along(g, &[g.at(last), g.at(target)]).into_iter().skip(1) {
        if !land[k] {
            c.cells.extend(extra);
            return Some(End::Sea);
        }
        if owner[k] != NONE && owner[k] as usize != ci && resolved[owner[k] as usize] {
            c.cells.extend(extra);
            return Some(End::Join(k));
        }
        if c.cells.contains(&k) {
            break;
        }
        extra.push(k);
    }
    // (The line found its way blocked by the course itself.)
    Some(lake.map_or(End::Inland, End::Lake))
}

/// Paint drawn biomes over the classified ones (on land, not lakes): the painted biome inside,
/// an ecotone with the old one at the edge.
pub fn paint_biomes(world: &World, g: Raster, land: &[bool], lake_of: &[u32], biome: &mut [u32]) {
    let s_edge = world.stream("t0.sketch.biome");
    for (_, s) in strokes(world, SketchTool::Biome) {
        let Some(target) = s.biome.as_deref().and_then(|b| BIOMES.iter().copied().find(|x| x.name() == b)) else { continue };
        if matches!(target, Biome::Ocean | Biome::Lake) {
            continue;
        }
        let width = if s.hard { 0.6 * g.cell } else { (0.35 * s.radius_ft).max(2.0 * g.cell) };
        let sd = signed(g, s, 3.0 * width);
        for (k, &v) in sd.iter().enumerate() {
            if !land[k] || lake_of[k] != NO_LAKE {
                continue;
            }
            let (x, y) = unit(world, g, k);
            let wobble = if s.hard { 0.0 } else { 0.8 * width * fbm(s_edge, x * 30.0, y * 30.0, 3, 2.0, 0.5) };
            let m = smoothstep(-width, width, v + wobble);
            if m <= 0.0 {
                continue;
            }
            let b1 = biome[k] & 0xff;
            if m > 0.5 {
                let blend = ((1.0 - m) * 2.0 * 255.0) as u32;
                biome[k] = target as u32 | b1 << 8 | blend.min(255) << 16;
            } else {
                let blend = (m * 2.0 * 255.0) as u32;
                biome[k] = b1 | (target as u32) << 8 | blend.min(255) << 16;
            }
        }
    }
}

/// The settlements the sketch pins: snapped onto land (within 25 mi) and at least 3 mi apart.
pub fn pins(world: &World, g: Raster, land: &[bool], conflicts: &mut Vec<Conflict>) -> Vec<Pin> {
    let mut out: Vec<Pin> = Vec::new();
    for (si, s) in strokes(world, SketchTool::Pin) {
        let tier = match s.tier.as_deref() {
            Some("metropolis") => Tier::Metropolis,
            Some("city") => Tier::City,
            Some("town") => Tier::Town,
            _ => Tier::Village,
        };
        let p = s.pts[0];
        let mut k = g.cell_of(p);
        let (mut x, mut y) = (p[0].clamp(0.0, (g.w - 1) as f64 * g.cell), p[1].clamp(0.0, (g.h - 1) as f64 * g.cell));
        if !land[k] {
            let r = (25.0 * MI / g.cell).ceil() as i64;
            let (ci, cj) = ((k % g.w) as i64, (k / g.w) as i64);
            let mut best: Option<(i64, usize)> = None;
            for dj in -r..=r {
                for di in -r..=r {
                    let (i, j) = (ci + di, cj + dj);
                    if i < 0 || j < 0 || i >= g.w as i64 || j >= g.h as i64 || di * di + dj * dj > r * r {
                        continue;
                    }
                    let c = j as usize * g.w + i as usize;
                    if land[c] && best.is_none_or(|b| di * di + dj * dj < b.0) {
                        best = Some((di * di + dj * dj, c));
                    }
                }
            }
            match best {
                Some((_, c)) => {
                    k = c;
                    [x, y] = g.at(c);
                    conflicts.push(Conflict { stroke: si, message: "This pin was in the water: moved onto the nearest land".into(), x, y });
                }
                None => {
                    conflicts.push(Conflict { stroke: si, message: "This pin is far out at sea: no settlement placed".into(), x: p[0], y: p[1] });
                    continue;
                }
            }
        }
        if out.iter().any(|q| crate::core::sqrt((q.x - x) * (q.x - x) + (q.y - y) * (q.y - y)) < 3.0 * MI) {
            conflicts.push(Conflict { stroke: si, message: "This pin is within 3 miles of another: no settlement placed".into(), x, y });
            continue;
        }
        out.push(Pin { stroke: si, tier, x, y, cell: k });
    }
    out
}
