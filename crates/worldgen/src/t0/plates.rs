//! Simplified plate tectonics. Warped Voronoi plates with motion vectors produce:
//! a continental-crust field (thresholded into the land mask), a rock-uplift field that
//! erosion turns into mountain ranges, rift and mid-ocean-ridge fields, and volcanic-arc
//! suitability. Collisions build broad ranges; subduction builds coastal ranges + arcs.
//!
//! Coordinates: the plates are laid out in "reference" units, the default map's width being 1
//! (`REF_MAP_W_FT`), so plates, ranges and hotspots keep their size in miles on any map and a
//! small map sees a piece of the mosaic. The coastline is map-relative ("unit": x in [0, 1]
//! across the map, y in [0, aspect]): it is fractal, and every size gets a whole landmass.

use crate::World;
use crate::core::noise::{Fbm, Ridged, smoothstep};
use crate::core::rng::Pcg32;

pub struct Tectonics {
    pub w: usize,
    pub h: usize,
    /// Continental-ness; land is where it exceeds the threshold matching the land fraction.
    pub crust: Vec<f64>,
    /// Rock uplift rate, roughly 0..1.5.
    pub uplift: Vec<f64>,
    /// Divergence on continental crust (rift valleys), 0..1.
    pub rift: Vec<f64>,
    /// Divergent ocean boundaries (mid-ocean ridges), 0..1.
    pub ridge: Vec<f64>,
    /// Suitability for volcanoes (subduction arcs, hotspots), 0..1.
    pub arc: Vec<f64>,
    /// Hotspot centers in reference coordinates.
    pub hotspots: Vec<[f64; 2]>,
}

struct Plate {
    pos: [f64; 2],
    vel: [f64; 2],
    continental: bool,
}

#[inline]
fn gauss(d: f64, w: f64) -> f64 {
    libm::exp(-(d / w) * (d / w))
}

pub fn build(world: &World, w: usize, h: usize) -> Tectonics {
    let p = world.params();
    let aspect = world.geom.map_h_ft / world.geom.map_w_ft;
    // The map's width in reference units. The plates' domain reaches a little past it, on a
    // small map half a plate's width (so the plates there are as big as anywhere).
    let s = world.geom.map_w_ft / crate::core::tile::REF_MAP_W_FT;
    let per = 1.1 * 0.85 / p.plate_count as f64;
    let margin = (0.5 * crate::core::sqrt(per) - 0.5 * s * aspect.min(1.0)).max(0.05);
    let (dw, dh) = (s + 2.0 * margin, s * aspect + 2.0 * margin);
    let mut rng = Pcg32::new(world.stream("t0.plates"), 1);
    // `plate_count` is per default map (its domain 1.1 × 0.85).
    let n_plates = ((dw * dh / per).round() as usize).clamp(2, 60);
    // So few that a boundary could miss the map: the first two seeds either side of its middle.
    let across = n_plates <= 4;

    // Plate seeds: best-candidate sampling for even spacing, allowed slightly off-map.
    let min_d = 0.55 * crate::core::sqrt(dw * dh / n_plates as f64);
    let mut plates: Vec<Plate> = Vec::with_capacity(n_plates);
    let middle = [0.5 * s, 0.5 * s * aspect];
    let turn = rng.range(0.0, std::f64::consts::TAU);
    while plates.len() < n_plates {
        let mut pos = [0.0; 2];
        if across && plates.len() < 2 {
            // (Off the middle a little, so the boundary isn't always through it.)
            let off = rng.range(-0.15, 0.15) * min_d;
            let side = if plates.is_empty() { 0.5 } else { -0.5 } * min_d;
            pos = [middle[0] + (side + off) * libm::cos(turn), middle[1] + (side + off) * libm::sin(turn)];
        }
        for _ in 0..if across && plates.len() < 2 { 0 } else { 30 } {
            pos = [rng.range(-margin, s + margin), rng.range(-margin, s * aspect + margin)];
            if plates.iter().all(|q| dist(q.pos, pos) >= min_d) {
                break;
            }
        }
        let ang = rng.range(0.0, std::f64::consts::TAU);
        let speed = rng.range(0.3, 1.0);
        plates.push(Plate { pos, vel: [speed * libm::cos(ang), speed * libm::sin(ang)], continental: false });
    }

    // One to three on a default map; on a smaller one each is there as often as its area allows.
    let keep = (s * s * aspect / 0.75).min(1.0);
    let hotspots: Vec<[f64; 2]> = (0..1 + rng.below(3))
        .map(|_| ([rng.range(0.15 * s, 0.85 * s), rng.range(0.15 * s * aspect, 0.85 * s * aspect)], rng.next_f64()))
        .filter(|(_, u)| *u < keep)
        .map(|(c, _)| c)
        .collect();

    let st = |name: &str| world.stream(name);
    let (s_wx, s_wy, s_crust, s_base, s_old, s_age) =
        (st("t0.warp.x"), st("t0.warp.y"), st("t0.crust"), st("t0.base"), st("t0.old"), st("t0.age"));

    // Pass 1: nearest two plates per cell (in warped space) and the distance to their boundary.
    let n = w * h;
    let mut near = vec![(0u16, 0u16, 0.0f64); n];
    let mut warped = vec![[0.0f64; 2]; n];
    // The coastline's own warp, in unit coordinates (the same as the plates' on a default map).
    let mut warped_unit = if s == 1.0 { Vec::new() } else { vec![[0.0f64; 2]; n] };
    let mut area = vec![0usize; n_plates];
    let (mut warp_x, mut warp_y) = (Fbm::new(s_wx, 5, 2.0, 0.55), Fbm::new(s_wy, 5, 2.0, 0.55));
    let mut warp = |x: f64, y: f64| [x + 0.16 * warp_x.at(x * 2.5, y * 2.5), y + 0.16 * warp_y.at(x * 2.5 + 7.1, y * 2.5 + 2.3)];
    for j in 0..h {
        let y = j as f64 / (h - 1) as f64 * aspect;
        for i in 0..w {
            let x = i as f64 / (w - 1) as f64;
            let q = warp(x * s, y * s);
            let (mut a, mut b) = ((usize::MAX, f64::INFINITY), (usize::MAX, f64::INFINITY));
            for (k, pl) in plates.iter().enumerate() {
                let d = dist2(pl.pos, q);
                if d < a.1 {
                    b = a;
                    a = (k, d);
                } else if d < b.1 {
                    b = (k, d);
                }
            }
            // Distance to the bisector between the two nearest seeds.
            let bd = (b.1 - a.1) / (2.0 * dist(plates[a.0].pos, plates[b.0].pos));
            let idx = j * w + i;
            near[idx] = (a.0 as u16, b.0 as u16, bd);
            warped[idx] = q;
            if !warped_unit.is_empty() {
                warped_unit[idx] = warp(x, y);
            }
            area[a.0] += 1;
        }
    }

    // Continental plates: those nearest the map center (with jitter) until the area covers
    // a bit more than the target land fraction; thresholding trims the rest.
    let center = [0.5 * s, 0.5 * s * aspect];
    let mut order: Vec<(f64, usize)> =
        plates.iter().enumerate().map(|(k, pl)| (dist(pl.pos, center) + rng.range(0.0, 0.25) * s, k)).collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut covered = 0usize;
    for (_, k) in order {
        if covered as f64 >= (p.land_fraction * 1.15).min(0.97) * n as f64 {
            break;
        }
        plates[k].continental = true;
        covered += area[k];
    }

    let mut t = Tectonics {
        w,
        h,
        crust: vec![0.0; n],
        uplift: vec![0.0; n],
        rift: vec![0.0; n],
        ridge: vec![0.0; n],
        arc: vec![0.0; n],
        hotspots,
    };

    let (mut base, mut old_r, mut age) = (Fbm::new(s_base, 4, 2.0, 0.5), Ridged::new(s_old, 4), Fbm::new(s_age, 3, 2.0, 0.5));
    let (mut coast, mut coast_fine) = (Fbm::new(s_crust, 7, 2.0, 0.55), Fbm::new(s_crust ^ 0x51, 5, 2.0, 0.55));
    for idx in 0..n {
        let (ka, kb, bd) = near[idx];
        let (pa, pb) = (&plates[ka as usize], &plates[kb as usize]);
        let q = warped[idx];
        let qu = if warped_unit.is_empty() { q } else { warped_unit[idx] };
        let (i, j) = (idx % w, idx / w);
        let (x, y) = (i as f64 / (w - 1) as f64, j as f64 / (h - 1) as f64 * aspect);

        // Relative motion across the boundary: closing speed and shear.
        let nx = pb.pos[0] - pa.pos[0];
        let ny = pb.pos[1] - pa.pos[1];
        let nl = crate::core::sqrt(nx * nx + ny * ny).max(1e-9);
        let (nx, ny) = (nx / nl, ny / nl);
        let rv = [pa.vel[0] - pb.vel[0], pa.vel[1] - pb.vel[1]];
        let closing = rv[0] * nx + rv[1] * ny;
        let shear = crate::core::fabs(rv[0] * ny - rv[1] * nx);

        let mut crust = match (pa.continental, pb.continental) {
            (true, true) => 1.0,
            (false, false) => 0.0,
            (true, false) => 0.5 + 0.5 * smoothstep(0.0, 0.12, bd),
            (false, true) => 0.5 - 0.5 * smoothstep(0.0, 0.12, bd),
        };
        let mut up = 0.0;
        if closing > 0.0 {
            match (pa.continental, pb.continental) {
                (true, true) => up += closing * gauss(bd, 0.045),
                (true, false) => {
                    up += closing * 0.9 * gauss(bd - 0.035, 0.025);
                    t.arc[idx] = t.arc[idx].max(closing * gauss(bd - 0.028, 0.012));
                }
                (false, false) => {
                    // Ocean-ocean subduction: island arcs.
                    crust += 0.5 * closing * gauss(bd - 0.02, 0.012);
                    up += 0.6 * closing * gauss(bd - 0.02, 0.012);
                    t.arc[idx] = t.arc[idx].max(0.8 * closing * gauss(bd - 0.02, 0.01));
                }
                // The ocean's side: the coastal range's seaward foot (`bd` is measured the
                // other way here), so uplift doesn't step at the boundary.
                (false, true) => {
                    up += closing * 0.9 * gauss(-bd - 0.035, 0.025);
                    t.arc[idx] = t.arc[idx].max(closing * gauss(-bd - 0.028, 0.012));
                }
            }
        } else if pa.continental {
            // (Between continent and ocean, only on the continent's side, from nothing at the
            // boundary.)
            let taper = if pb.continental { 1.0 } else { smoothstep(0.0, 0.02, bd) };
            t.rift[idx] = -closing * gauss(bd, 0.02) * taper;
        } else if !pb.continental {
            t.ridge[idx] = -closing * gauss(bd, 0.03);
        }
        if pa.continental || pb.continental {
            up += 0.15 * shear * gauss(bd, 0.03);
        }

        // Interior: gentle hills everywhere plus a few ancient, worn ranges.
        let hills = 0.01 + 0.04 * (0.5 + 0.5 * base.at(q[0] * 3.0, q[1] * 3.0));
        up += hills;
        let old = old_r.at(q[0] * 2.5, q[1] * 2.5);
        up += 0.35 * old * old * smoothstep(0.15, 0.5, age.at(q[0] * 1.5, q[1] * 1.5));

        for hs in &t.hotspots {
            let d = dist(*hs, [x * s, y * s]);
            up += 0.5 * gauss(d, 0.02);
            crust += 0.7 * gauss(d, 0.025);
            t.arc[idx] = t.arc[idx].max(gauss(d, 0.015));
        }

        // Fractal coast and islands; keep the map border at sea.
        // Plates bias where land is; noise decides the actual coastline (peninsulas, gulfs,
        // islands). The map edge only takes over in the outer margin.
        crust = 0.55 * crust + 1.0 * coast.at(qu[0] * 2.6, qu[1] * 2.6);
        crust += 0.3 * coast_fine.at(qu[0] * 9.0, qu[1] * 9.0);
        let (ex, ey) = ((x - 0.5) / 0.5, (y - 0.5 * aspect) / (0.5 * aspect));
        crust -= 1.5 * smoothstep(0.8, 1.1, crate::core::sqrt(ex * ex + ey * ey));

        t.crust[idx] = crust;
        // With fewer of the plates' mountains kept, only the hills stay whole: ranges, old
        // ranges and hotspots are scaled.
        t.uplift[idx] = (hills + p.procedural_mountains * (up - hills)) * p.ruggedness.max(0.02);
    }
    t
}

#[inline]
fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])
}

#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    crate::core::sqrt(dist2(a, b))
}

/// Threshold such that exactly `fraction` of `values` lies above it.
pub fn threshold_for_fraction(values: &[f64], fraction: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let k = ((1.0 - fraction) * v.len() as f64) as usize;
    v[k.min(v.len() - 1)]
}
