//! Volcanoes: drawn in the sketch, or sited on subduction arcs and hotspots (the tectonic `arc`
//! field), spaced apart; then stamped onto the eroded terrain so cones and craters stay sharp,
//! with gullies down their flanks (on a smooth cone water would run in straight sheets).
//! Active ones get a volcanic-waste halo (biomes) and, later, lava flows and lava tubes.

use serde::Serialize;

use crate::World;
use crate::core::noise::{Fbm, smoothstep};
use crate::core::rng::Pcg32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VolcanoKind {
    Stratovolcano,
    Shield,
    CinderCone,
    /// A wide, collapsed cone: a ring of heights round a broad crater (which may hold a lake).
    Caldera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Active,
    Dormant,
    Extinct,
}

#[derive(Clone, Debug)]
pub struct Volcano {
    /// Cell coordinates on the T0 grid.
    pub cx: f64,
    pub cy: f64,
    pub kind: VolcanoKind,
    pub activity: Activity,
    /// Cone height above the surrounding terrain and base radius, ft.
    pub height_ft: f64,
    pub radius_ft: f64,
    /// The sketch stroke that drew it.
    pub stroke: Option<u32>,
}

/// Cone height (ft) and base radius (mi) ranges per kind.
fn size(kind: VolcanoKind) -> ((f64, f64), (f64, f64)) {
    match kind {
        VolcanoKind::Stratovolcano => ((6_000.0, 10_000.0), (9.0, 14.0)),
        VolcanoKind::Shield => ((4_000.0, 7_000.0), (20.0, 30.0)),
        VolcanoKind::CinderCone => ((1_000.0, 2_000.0), (3.0, 5.0)),
        VolcanoKind::Caldera => ((3_000.0, 6_000.0), (10.0, 16.0)),
    }
}

/// The sketch's volcanoes (cell coordinates on a grid of `cell_ft`): kind and activity as drawn
/// (default a dormant stratovolcano), sized by the stroke's strength within the kind's range.
pub fn drawn(world: &World, cell_ft: f64) -> Vec<Volcano> {
    world
        .file
        .sketch
        .strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tool == crate::world::SketchTool::Volcano)
        .map(|(i, s)| {
            let kind = match s.kind.as_deref() {
                Some("shield") => VolcanoKind::Shield,
                Some("cinder") => VolcanoKind::CinderCone,
                Some("caldera") => VolcanoKind::Caldera,
                _ => VolcanoKind::Stratovolcano,
            };
            let activity = match s.activity.as_deref() {
                Some("active") => Activity::Active,
                Some("extinct") => Activity::Extinct,
                _ => Activity::Dormant,
            };
            let ((h0, h1), (r0, r1)) = size(kind);
            let t = s.strength;
            Volcano { cx: s.pts[0][0] / cell_ft, cy: s.pts[0][1] / cell_ft, kind, activity, height_ft: h0 + (h1 - h0) * t, radius_ft: (r0 + (r1 - r0) * t) * 5280.0, stroke: Some(i as u32) }
        })
        .collect()
}

/// The drawn volcanoes, then `volcanoes` more placed (kept apart from them and each other).
pub fn place(world: &World, w: usize, h: usize, cell_ft: f64, arc: &[f64], height: &[f64]) -> Vec<Volcano> {
    let mut out = drawn(world, cell_ft);
    out.retain(|v| v.cx >= 0.0 && v.cy >= 0.0 && v.cx <= (w - 1) as f64 && v.cy <= (h - 1) as f64);
    let count = out.len() + world.params().volcanoes as usize;
    if count == out.len() {
        return out;
    }
    let mut rng = Pcg32::new(world.stream("t0.volcano"), 3);
    let sea = world.params().sea_level_ft;
    // 70 mi apart, or closer when that many wouldn't fit on the map; none wider than a small
    // map can hold (a shield is cut down to a steeper, smaller cone).
    let (map_w, map_h) = ((w - 1) as f64 * cell_ft, (h - 1) as f64 * cell_ft);
    let sep = (70.0f64 * 5280.0).min(0.35 * crate::core::sqrt(map_w * map_h / count as f64));
    let min_sep = sq(sep / cell_ft);
    let widest = 0.08 * map_w.min(map_h);

    // Candidates: strongest arc cells on land or shallow sea, with jitter to vary picks; then
    // (a small map with no arc on it) any land, higher ground first.
    let mut cands: Vec<(f64, usize)> = (0..w * h)
        .filter(|&k| arc[k] > 0.25 && height[k] > sea - 3000.0)
        .map(|k| (arc[k] + 0.4 * rng.next_f64(), k))
        .collect();
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut inland: Vec<(f64, usize)> = (0..w * h)
        .filter(|&k| height[k] > sea + 100.0)
        .map(|k| ((height[k] - sea) / world.params().max_elev_ft + 0.4 * rng.next_f64(), k))
        .collect();
    inland.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    cands.extend(inland);

    for (_, k) in cands {
        if out.len() >= count {
            break;
        }
        let (cx, cy) = ((k % w) as f64, (k / w) as f64);
        if out.iter().any(|v| sq(v.cx - cx) + sq(v.cy - cy) < min_sep) {
            continue;
        }
        let kind = match rng.below(10) {
            0..=5 => VolcanoKind::Stratovolcano,
            6..=7 => VolcanoKind::Shield,
            _ => VolcanoKind::CinderCone,
        };
        let activity = match rng.below(20) {
            0..=6 => Activity::Active,
            7..=14 => Activity::Dormant,
            _ => Activity::Extinct,
        };
        let ((h0, h1), (r0, r1)) = size(kind);
        let (height_ft, radius_ft) = (rng.range(h0, h1), rng.range(r0, r1) * 5280.0);
        let cut = (widest / radius_ft).min(1.0);
        out.push(Volcano { cx, cy, kind, activity, height_ft: height_ft * libm::pow(cut, 0.5), radius_ft: radius_ft * cut, stroke: None });
    }
    out
}

/// Stamp cones and summit craters onto the height grid (ft): their feet lobed as flows spread
/// them, radial gullies (deeper and more of them toward the foot, a few cells apart at least)
/// and some rough ground.
#[allow(clippy::too_many_arguments)]
pub fn apply(volcanoes: &[Volcano], w: usize, h: usize, cell_ft: f64, sea: f64, max_elev: f64, seed: u64, height: &mut [f64]) {
    let k_cell = crate::core::tile::cell_scale(cell_ft);
    for (vi, v) in volcanoes.iter().enumerate() {
        let r_cells = v.radius_ft / cell_ft;
        let vs = seed ^ crate::core::rng::mix64(vi as u64);
        let (mut furrows, mut rough) = (Fbm::new(vs, 3, 2.0, 0.5), Fbm::new(vs ^ 0x7f4a, 3, 2.0, 0.5));
        let (mut reach, mut lobes) = (Fbm::new(vs ^ 0x2c19, 2, 2.0, 0.5), Fbm::new(vs ^ 0x91d3, 2, 2.0, 0.5));
        // The feet reach this far out where a lobe runs.
        let out = 1.3 * r_cells;
        // Gullies about 4 cells or 1.5 mi apart round the foot, whichever is wider.
        let ring = (r_cells / libm::fmax(4.0, 1.5 * 5280.0 / cell_ft)).max(1.0);
        let caldera = v.kind == VolcanoKind::Caldera;
        let crater = if caldera { 0.45 * r_cells } else { (0.09 * r_cells).max(0.6) };
        // A caldera's rim, round a broad crater floor a little above the land it stands on.
        let rim = v.height_ft * libm::pow(0.55, 1.4);
        let (x0, x1) = ((v.cx - out).floor().max(0.0) as usize, ((v.cx + out).ceil() as usize).min(w - 1));
        let (y0, y1) = ((v.cy - out).floor().max(0.0) as usize, ((v.cy + out).ceil() as usize).min(h - 1));
        // Cone sits on the local base level; offshore ones rise from a shallow seamount so
        // they break the surface as volcanic islands.
        let base = height[v.cy as usize * w + v.cx as usize].max(sea - 1_500.0);
        // A cone on high ground is lower, so its summit keeps below the ceiling (not flattened
        // against it); the last of the way, it only approaches it.
        let tall = v.height_ft.min((0.95 * max_elev - (base - sea)).max(0.35 * v.height_ft));
        let scale = tall / v.height_ft;
        let (cap, room) = (sea + 0.9 * max_elev, 0.1 * max_elev);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let r = crate::core::sqrt(sq(x as f64 - v.cx) + sq(y as f64 - v.cy));
                if r >= out {
                    continue;
                }
                let (ca, sa) = if r > 0.0 { ((x as f64 - v.cx) / r, (y as f64 - v.cy) / r) } else { (1.0, 0.0) };
                let edge = r_cells * (1.0 + 0.3 * lobes.at(ca * 1.1, sa * 1.1)).clamp(0.75, 1.3);
                if r >= edge {
                    continue;
                }
                let t = 1.0 - r / edge;
                let add = if caldera {
                    if r < crater {
                        let u = r / crater;
                        rim - (rim - 0.05 * v.height_ft) * (1.0 - u * u * u * u * u * u)
                    } else {
                        v.height_ft * libm::pow(t, 1.4)
                    }
                } else {
                    let exp = if v.kind == VolcanoKind::Shield { 1.1 } else { 1.8 };
                    let mut add = v.height_ft * libm::pow(t, exp);
                    if r < crater {
                        add -= 0.12 * v.height_ft * (1.0 - r / crater);
                    }
                    add
                };
                // Furrows: the noise along a circle round the summit, its radius growing a
                // little outward so they wander and join.
                let u = r / edge;
                let along = ring * (1.0 + 0.25 * u);
                let f = 1.0 - crate::core::fabs(furrows.at(ca * along, sa * along));
                // Some cut deep and start high, some barely mark the flank.
                let cut = (0.55 + reach.at(ca * 1.6, sa * 1.6)).clamp(0.0, 1.0);
                let depth = 0.12 * v.height_ft * cut * smoothstep(0.45 - 0.35 * cut, 0.6, u) * (1.0 - smoothstep(0.8, 1.0, u));
                let bumps = 0.03 * v.height_ft * t * rough.at(x as f64 * k_cell / 0.8, y as f64 * k_cell / 0.8);
                let add = (add - depth * f * f * f + bumps).max(0.0) * scale;
                // The foot settles onto the ground round it (the cone stands on its centre's
                // ground, which may be above or below it): no step at its edge.
                let k = y * w + x;
                let ground = height[k].max(sea - 1_500.0);
                let mut top = ground + (base - ground) * smoothstep(0.0, 0.55, t) + add;
                if top > cap {
                    top = cap + room * libm::tanh((top - cap) / room);
                }
                height[k] = height[k].max(top);
            }
        }
    }
}

#[inline]
fn sq(x: f64) -> f64 {
    x * x
}
