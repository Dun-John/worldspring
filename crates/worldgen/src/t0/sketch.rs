//! Sketch constraints (`world::Sketch`) on a T0-style grid (the full grid, or the coarse one a
//! sketch preview uses):
//!
//! - land and sea strokes decide the land mask (outlines are coastlines; once any land is
//!   drawn, undrawn map is sea), with a natural, noisy coast unless drawn hard;
//! - ranges add rock uplift along their lines, before erosion carves them;
//! - rivers are carved strictly downhill along their lines and fed at their sources, so the
//!   hydrology maps them;
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

/// Rock uplift along drawn ranges (added to the plates' field before erosion, which halves
/// when any range is drawn).
pub fn add_ranges(world: &World, g: Raster, uplift: &mut [f64]) {
    let s_rid = world.stream("t0.sketch.range");
    let rugged = world.params().ruggedness.max(0.3);
    // Drawn ranges are the main ones: the plates' own ranges are kept, but lower.
    if strokes(world, SketchTool::Range).next().is_some() {
        uplift.iter_mut().for_each(|u| *u *= 0.5);
    }
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
            uplift[k] += amp * crest * libm::exp(-(d / r) * (d / r));
        }
    }
}

/// Ranges drawn mostly over the sea (they raise nothing there).
pub fn check_ranges(world: &World, g: Raster, land: &[bool], conflicts: &mut Vec<Conflict>) {
    for (i, s) in strokes(world, SketchTool::Range) {
        let wet = s.pts.iter().filter(|p| !land[g.cell_of(**p)]).count();
        if wet * 2 > s.pts.len() {
            let p = s.pts[s.pts.len() / 2];
            conflicts.push(Conflict { stroke: i, message: "This range is mostly at sea: draw land under it".into(), x: p[0], y: p[1] });
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
const NONE: u32 = u32::MAX;

/// Where a drawn river's course ends.
#[derive(Clone, Copy, PartialEq)]
enum End {
    Sea,
    /// Joins another course at this cell (one of that course's cells).
    Join(usize),
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
/// source so the hydrology maps them as rivers.
///
/// - A river runs the way it was drawn (unless drawn from the sea inland).
/// - It ends at the sea, or joins the first other drawn river it meets (a tributary ends on
///   that river's channel, at its level). One that stops short of the sea or another river
///   within `REACH_FT` is carried on to it in a straight line.
/// - Its profile never rises and falls at least `MIN_FALL` per cell (more on a high, short
///   course, up to a foot), and stays above the water it ends in: where that needs the river
///   above the ground, the ground along it is raised rather than the river dug under the sea.
/// - Every course is set first, then the valleys: banks a little above the water (never over
///   any river's channel), sides sloping up to the valley's edge.
pub fn carve_rivers(world: &World, g: Raster, height: &mut [f64], land: &[bool], conflicts: &mut Vec<Conflict>) -> Vec<(usize, f64)> {
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
        let mut end = None;
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
            let p = g.at(cells[0]);
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
                    let Some(end) = carry_on(g, land, &owner, &resolved, &mut courses[ci], ci) else { continue };
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
                let end = carry_on(g, land, &owner, &resolved, &mut courses[ci], ci).unwrap_or(End::Inland);
                courses[ci].end = Some(end);
                claim(&mut owner, &courses[ci].cells[had..], ci);
            }
            resolved[ci] = true;
            order.push(ci);
        }
    }

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
    }

    // The valleys, never over a channel: banks between 10 ft above the water and the side's
    // height there (no levees standing over the plain, which would pond it); where the river
    // was raised above the ground, the ground beside it rises with it.
    let mut cells: Vec<(&usize, &(f64, f64, f64, bool))> = valley.iter().collect();
    cells.sort_by_key(|(k, _)| **k);
    for (&k, &(target, d, z, lifted)) in cells {
        if !land[k] || !zc[k].is_nan() {
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
    feed
}

/// Carry a course that stops short on to the nearest sea cell or resolved course within
/// `REACH_FT` of its end (straight there; it joins whatever course it meets on the way).
/// `None` when there is nothing in reach yet but some course is still unresolved (one may come
/// within reach); `Inland` when nothing can.
fn carry_on(g: Raster, land: &[bool], owner: &[u32], resolved: &[bool], c: &mut Course, ci: usize) -> Option<End> {
    let last = *c.cells.last().unwrap();
    let (ei, ej) = ((last % g.w) as i64, (last / g.w) as i64);
    let goal = |k: usize| !land[k] || (owner[k] != NONE && owner[k] as usize != ci && resolved[owner[k] as usize]);
    let mut best: Option<(i64, usize)> = None;
    let reach = (crate::core::ceil(REACH_FT / g.cell) as i64).max(2);
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
        return if resolved.iter().enumerate().all(|(i, &r)| r || i == ci) { Some(End::Inland) } else { None };
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
    Some(End::Inland)
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
