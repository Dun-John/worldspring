//! Climate: temperature from latitude and elevation; precipitation from air parcels swept
//! along the prevailing wind. Parcels pick up moisture over (warm) ocean and drop it inland,
//! much more where the ground rises, so windward slopes are wet and lee sides sit in rain
//! shadow. Latitude belts add the wet tropics and dry subtropics.

use crate::World;
use crate::core::noise::smoothstep;
use crate::world::Wind;

pub struct Climate {
    /// Mean annual temperature, °C.
    pub temp: Vec<f32>,
    /// Annual precipitation, mm.
    pub precip: Vec<f32>,
}

/// Lapse rate: 6.5 °C per km, in °C per ft.
const LAPSE_C_PER_FT: f64 = 0.0065 * 0.3048;

pub fn latitude(world: &World, j: usize, h: usize) -> f64 {
    let p = world.params();
    p.lat_top + (p.lat_bottom - p.lat_top) * j as f64 / (h - 1).max(1) as f64
}

pub fn sea_level_temp(lat: f64) -> f64 {
    28.0 - 0.0064 * lat * lat
}

pub fn build(world: &World, w: usize, h: usize, cell_ft: f64, height: &[f64], land: &[bool]) -> Climate {
    let p = world.params();
    let n = w * h;
    let sea = p.sea_level_ft;
    let cell_mi = cell_ft / 5280.0;
    let pickup = 1.0 - libm::exp(-cell_mi / 120.0);
    let rain_base = 1.0 - libm::exp(-cell_mi / 700.0);
    let mm_scale = 1300.0 / rain_base;
    let noise_seed = world.stream("t0.climate.noise");

    // Orographic lift follows the large-scale terrain, not every small ridge.
    let mut smooth = height.to_vec();
    for _ in 0..3 {
        smooth = box_blur(w, h, &smooth, land);
    }

    let mut temp = vec![0f32; n];
    let mut precip = vec![0f64; n];
    let mut belt_noise = crate::core::noise::Fbm::new(world.stream("t0.climate.belt"), 3, 2.0, 0.5);
    for j in 0..h {
        let lat = latitude(world, j, h);
        let t_sea = sea_level_temp(lat) + p.temp_offset_c;
        for i in 0..w {
            let k = j * w + i;
            let above = if land[k] { (height[k] - sea).max(0.0) } else { 0.0 };
            temp[k] = (t_sea - LAPSE_C_PER_FT * above) as f32;
        }

        // Share of the westerly sweep; belts blend across their boundaries (no seams).
        let alat = crate::core::fabs(lat);
        let westerly = match p.wind {
            Wind::FromWest => 1.0,
            Wind::FromEast => 0.0,
            Wind::Belts => smoothstep(25.0, 35.0, alat) * (1.0 - smoothstep(55.0, 65.0, alat)),
        };
        let belt = belt_factor(alat);
        let capacity = (0.35 + t_sea / 35.0).clamp(0.25, 1.1);

        for (from_west, share) in [(true, westerly), (false, 1.0 - westerly)] {
            if share <= 0.0 {
                continue;
            }
            let mut m = capacity;
            let mut prev_h = sea;
            for s in 0..w {
                let i = if from_west { s } else { w - 1 - s };
                let k = j * w + i;
                if !land[k] {
                    m += (capacity - m).max(0.0) * pickup;
                    prev_h = sea;
                    continue;
                }
                let rise = (smooth[k] - prev_h).max(0.0);
                prev_h = smooth[k];
                let frac = (rain_base + 0.5 * rise / 6000.0).min(0.9);
                let rain = m * frac;
                m -= 0.6 * rain;
                // Wavy belt edges: the belt factor sees a latitude perturbed by a few degrees.
            let wl = crate::core::fabs(lat + 5.0 * belt_noise.at(i as f64 / 90.0, j as f64 / 90.0));
            let wavy = belt_factor(wl) / belt;
            precip[k] += share * rain * mm_scale * belt * wavy * p.moisture;
            }
        }
    }

    // Rows are swept independently; blur to remove banding and spread orographic rain.
    for _ in 0..3 {
        precip = box_blur(w, h, &precip, land);
    }
    let mut noise = crate::core::noise::Fbm::new(noise_seed, 3, 2.0, 0.5);
    let precip = precip
        .iter()
        .enumerate()
        .map(|(k, &v)| {
            let (i, j) = ((k % w) as f64, (k / w) as f64);
            (v * (1.0 + 0.15 * noise.at(i / 60.0, j / 60.0))).max(0.0) as f32
        })
        .collect();
    Climate { temp, precip }
}

/// Precipitation multiplier by |latitude|: wet ITCZ, dry subtropical highs, wet westerlies.
fn belt_factor(alat: f64) -> f64 {
    let g = |c: f64, s: f64| libm::exp(-((alat - c) / s) * ((alat - c) / s));
    1.0 + 0.9 * g(0.0, 9.0) - 0.55 * g(27.0, 9.0) + 0.25 * g(55.0, 10.0)
}

/// 3x3 box blur over land cells only (ocean neither contributes nor receives).
fn box_blur(w: usize, h: usize, v: &[f64], land: &[bool]) -> Vec<f64> {
    let mut out = v.to_vec();
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            if !land[k] {
                continue;
            }
            let (mut s, mut c) = (0.0, 0.0);
            for dj in -1i64..=1 {
                for di in -1i64..=1 {
                    let (x, y) = (i as i64 + di, j as i64 + dj);
                    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                        continue;
                    }
                    let kk = y as usize * w + x as usize;
                    if land[kk] {
                        s += v[kk];
                        c += 1.0;
                    }
                }
            }
            out[k] = s / c;
        }
    }
    out
}

/// Distance (in cells) from every cell to the nearest `target` cell (two-pass chamfer).
pub fn distance_to(w: usize, h: usize, target: &[bool]) -> Vec<f64> {
    const D1: f64 = 1.0;
    const D2: f64 = std::f64::consts::SQRT_2;
    let big = (w + h) as f64 * 2.0;
    let mut d: Vec<f64> = target.iter().map(|&t| if t { 0.0 } else { big }).collect();
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            let mut v = d[k];
            if i > 0 {
                v = v.min(d[k - 1] + D1);
            }
            if j > 0 {
                v = v.min(d[k - w] + D1);
                if i > 0 {
                    v = v.min(d[k - w - 1] + D2);
                }
                if i + 1 < w {
                    v = v.min(d[k - w + 1] + D2);
                }
            }
            d[k] = v;
        }
    }
    for j in (0..h).rev() {
        for i in (0..w).rev() {
            let k = j * w + i;
            let mut v = d[k];
            if i + 1 < w {
                v = v.min(d[k + 1] + D1);
            }
            if j + 1 < h {
                v = v.min(d[k + w] + D1);
                if i + 1 < w {
                    v = v.min(d[k + w + 1] + D2);
                }
                if i > 0 {
                    v = v.min(d[k + w - 1] + D2);
                }
            }
            d[k] = v;
        }
    }
    d
}
