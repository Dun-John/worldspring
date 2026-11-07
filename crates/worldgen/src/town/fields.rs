//! v63 countryside: what each field round a settlement is (`Layout::field_info`, one per
//! `Layout::fields`, in the same order): the way its furrows run, its use (arable, fallow, pasture,
//! orchard) and what bounds it (a hedge, a low dry-stone wall or nothing). Chosen once the fields are
//! final (after a palace has taken its ground), from the ground under each and keyed by its place, so
//! a field's look never depends on the order layouts are asked for.
//!
//! A village's arable round it is open field: long narrow strips (selions) in blocks, the furrows
//! along each strip, drawn as a pattern (plan zoom `sitefill.frag`, battlemap `ground.frag`), not as
//! polygons of their own.

use super::geom::*;
use super::{Layout, Site};
use crate::battlemap::Surface;
use crate::core::noise::{gradient2, smoothstep};
use crate::core::rng::{hash3, unit};
use crate::t0::biome::Biome;
use crate::t0::settle::Tier;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FieldUse {
    Arable = 0,
    Pasture = 1,
    Fallow = 2,
    Orchard = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FieldBound {
    None = 0,
    Hedge = 1,
    /// A low dry-stone wall.
    Wall = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct FieldInfo {
    /// The way the furrows run (radians in [0, π), world axes).
    pub angle: f32,
    pub kind: FieldUse,
    pub bound: FieldBound,
    /// Open-field strips (a village's arable): balks between strips along `angle`.
    pub strips: bool,
    /// A shade (0-7), so neighbouring fields differ.
    pub tone: u8,
}

/// How far a settlement's fields reach at `p` (local ft) as a share of the belt's plain radius
/// `scale` (about 1): a low-frequency noise keyed by the world position wanders the belt's edge in
/// and out, and the fields run on along the roads (`roads`: local ft) and the river.
pub(super) fn reach(site: &Site, seed: u64, p: P, roads: &[(Vec<P>, u8, f64)], scale: f64) -> f64 {
    let w = add(p, site.center);
    let k = 1.0 / (0.45 * scale).max(200.0);
    let n = 0.7 * gradient2(seed ^ 0x5eed_f1e1d, w[0] * k, w[1] * k) + 0.3 * gradient2(seed ^ 0x5eed_f1e1e, w[0] * 2.7 * k, w[1] * 2.7 * k);
    let road = roads.iter().flat_map(|r| r.0.windows(2)).map(|s| seg_dist(p, s[0], s[1])).fold(f64::MAX, f64::min);
    let by_road = 0.3 * (1.0 - smoothstep(60.0, 400.0, road));
    let by_river = if site.near_river(p, 150.0) { 0.2 } else if site.near_river(p, 400.0) { 0.1 } else { 0.0 };
    1.0 + 0.5 * n + by_road + by_river
}

/// The fields a settlement keeps of its candidate plots (local ft; class 0 always), outward by
/// `far` (a plot's distance over its reach, `reach`: 1 at the belt's edge) until they hold `want`
/// sq ft: the belt's edge wanders with the reach, and its outermost plots are thinned (a jitter
/// keyed by place, so some are skipped and some beyond kept). In the plots' order.
pub(super) fn belt_order(site: &Site, seed: u64, plots: &[(Vec<P>, u8)], want: f64, far: &dyn Fn(P) -> f64) -> Vec<Vec<P>> {
    let mut order: Vec<(f64, usize)> = plots
        .iter()
        .enumerate()
        .map(|(k, (f, class))| {
            let c = centroid(f);
            let w = add(c, site.center);
            let jitter = unit(hash3(seed, crate::core::round(w[0]) as i64, crate::core::round(w[1]) as i64, 0x7a11));
            (if *class == 0 { -1.0 } else { far(c) + 0.3 * jitter }, k)
        })
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut take = vec![false; plots.len()];
    let mut have = 0.0;
    for &(t, k) in &order {
        if t >= 0.0 && have >= want {
            break;
        }
        take[k] = true;
        have += area(&plots[k].0).abs();
    }
    plots.iter().zip(take).filter(|x| x.1).map(|x| x.0.0.clone()).collect()
}

/// Ground steeper than this (rise over run across the field) is pasture or walled.
const STEEP: f64 = 0.05;

/// Every field's info, from its place and the ground under it (`fields` in world ft, `l.center` the
/// settlement's middle, `site` planned round it).
pub(super) fn assign(site: &Site, seed: u64, l: &mut Layout) {
    let biome = crate::battlemap::land_biome(site.t0, l.center);
    let (wall0, hedge0): (f64, f64) = match biome {
        Biome::Alpine | Biome::Tundra | Biome::ColdDesert | Biome::Ice => (0.75, 0.0),
        Biome::Steppe | Biome::Taiga => (0.55, 0.2),
        Biome::HotDesert | Biome::SaltFlat | Biome::Savanna => (0.35, 0.1),
        Biome::TemperateForest | Biome::TemperateRainforest | Biome::Grassland => (0.12, 0.72),
        Biome::Swamp | Biome::Jungle => (0.05, 0.55),
        _ => (0.3, 0.4),
    };
    let grazing = match biome {
        Biome::Steppe | Biome::Savanna | Biome::Tundra | Biome::Alpine | Biome::Taiga | Biome::ColdDesert => 0.45,
        Biome::Swamp => 0.35,
        _ => 0.18,
    };
    let fruit = !matches!(biome, Biome::Alpine | Biome::Tundra | Biome::Ice | Biome::ColdDesert | Biome::HotDesert | Biome::SaltFlat | Biome::Taiga);
    let village = l.tier == Tier::Village;
    let r = l.radius.max(1.0);
    // How far out each field lies against the outermost in its direction: the belt's ragged rim is
    // more pasture, less fenced.
    const SECTORS: usize = 32;
    let sector = |p: P| ((libm::atan2(p[1], p[0]) / std::f64::consts::TAU + 0.5) * SECTORS as f64) as usize % SECTORS;
    let mut outer = [0.0f64; SECTORS];
    for f in &l.fields {
        let p = sub(centroid(f), l.center);
        outer[sector(p)] = outer[sector(p)].max(len(p));
    }
    let mut info = Vec::with_capacity(l.fields.len());
    let mut near: Vec<(f64, usize)> = Vec::new();
    for (i, f) in l.fields.iter().enumerate() {
        let c = centroid(f);
        let h = hash3(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 0xf1e1d);
        let roll = |k: i64| unit(hash3(h, k, 0, 0xf1e1d));
        let local = sub(c, l.center);
        let o = obb(f);
        // Slope: the rise along and across the field from its middle to its ends.
        let (along, across) = (mul(o.axis, 0.45 * o.long), mul([-o.axis[1], o.axis[0]], 0.45 * o.short));
        let rise = |d: P, w: f64| (site.height(add(local, d)) - site.height(sub(local, d))).abs() / (0.9 * w).max(1.0);
        let grade = rise(along, o.long).max(rise(across, o.short));
        let steep = grade > STEEP;
        let wet = site.near_river(local, 120.0);
        let far = len(local) / r;
        let rim = ((len(local) / outer[sector(local)].max(1.0) - 0.8) / 0.2).clamp(0.0, 1.0);
        // Use: pasture on steep and wet ground, more of it far out; arable near, some of it fallow.
        let pasture = grazing + if steep { 0.4 } else { 0.0 } + if wet { 0.45 } else { 0.0 } + if far > 2.0 { 0.15 } else { 0.0 } + 0.3 * rim;
        let kind = if roll(1) < pasture {
            FieldUse::Pasture
        } else if roll(2) < 0.13 {
            FieldUse::Fallow
        } else {
            FieldUse::Arable
        };
        if fruit && !steep && !wet && kind != FieldUse::Pasture {
            near.push((far, i));
        }
        let strips = village && !steep && matches!(kind, FieldUse::Arable | FieldUse::Fallow);
        // Furrows along the field's long side mostly, else across it, a little off square.
        let mut a = libm::atan2(o.axis[1], o.axis[0]);
        if strips {
            a += 0.04 * (2.0 * roll(3) - 1.0);
        } else {
            let square = o.long < 1.3 * o.short;
            if roll(4) < if square { 0.5 } else { 0.3 } {
                a += std::f64::consts::FRAC_PI_2;
            }
            a += 0.14 * (2.0 * roll(3) - 1.0);
        }
        let angle = a.rem_euclid(std::f64::consts::PI);
        // Bounds: walls on high, stony, dry or cold ground, hedges in temperate lowlands, some none
        // (open-field strips never); stock is fenced in more often than crops.
        let (wall, hedge) = if steep { ((wall0 + 0.45).min(0.9), hedge0.min(0.1)) } else { (wall0, hedge0) };
        let keep = if kind == FieldUse::Pasture { 1.15 } else { 1.0 } * (1.0 - 0.4 * rim);
        let b = roll(5);
        let bound = if strips {
            FieldBound::None
        } else if b < wall * keep {
            FieldBound::Wall
        } else if b < (wall + hedge) * keep {
            FieldBound::Hedge
        } else {
            FieldBound::None
        };
        info.push(FieldInfo { angle: angle as f32, kind, bound, strips, tone: (h >> 40) as u8 & 7 });
    }
    // A few orchards by the settlement's edge: the nearest fields that may hold one, about one in two.
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let cap = (l.fields.len() / 25).clamp(1, 6);
    let mut made = 0;
    for &(_, i) in near.iter().take(3 * cap) {
        if made >= cap {
            break;
        }
        let c = centroid(&l.fields[i]);
        if unit(hash3(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 0x0c4a2d)) < 0.5 {
            let fi = &mut info[i];
            fi.kind = FieldUse::Orchard;
            fi.strips = false;
            if fi.bound == FieldBound::None {
                fi.bound = if hedge0 >= wall0 { FieldBound::Hedge } else { FieldBound::Wall };
            }
            made += 1;
        }
    }
    l.field_info = info;
}

/// A field's byte on its battlemap squares (`Chunk::field`): the furrows' angle (6 bits over π) and
/// how its rows are drawn above (0 arable, 1 open-field strips, 2 fallow, 3 none: pasture, orchard).
pub fn square_byte(i: &FieldInfo) -> u8 {
    let a = (crate::core::round(i.angle as f64 / std::f64::consts::PI * 64.0) as u32 & 63) as u8;
    let mode = match i.kind {
        FieldUse::Pasture | FieldUse::Orchard => 3,
        FieldUse::Fallow => 2,
        FieldUse::Arable if i.strips => 1,
        FieldUse::Arable => 0,
    };
    a | mode << 6
}

/// The battlemap surface of a field's square by its byte (`square_byte`).
pub fn surface(byte: u8) -> Surface {
    if byte >> 6 == 3 { Surface::Pasture } else { Surface::Field }
}
