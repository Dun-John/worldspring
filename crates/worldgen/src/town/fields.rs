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
    /// Hedgerows (or walls) across it (`rural::rows`): none on a pasture in a forest biome (open
    /// ground, its floor the forest's) or on a hot desert's watered fields.
    pub rows: bool,
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

/// How far (ft) a field or croft stops short of sand, and the step (ft) the ground is looked at.
const SAND_MARGIN: f64 = 8.0;
const SAND_STEP: f64 = 10.0;

/// What the battlemap draws at `w` (world ft; `battlemap::sand_ground`): 0 the land's own ground, 1
/// sand by the sea or a lake (a beach, a shore's biome) or their water, 2 a hot desert's sand.
fn sand_kind(site: &Site, sea: f64, w: P) -> u8 {
    let t0 = site.t0;
    let (level, mask) = t0.sample_lake(w[0], w[1]);
    if level > crate::t0::hydro::DRY {
        let p = sub(w, site.center);
        let water = if mask >= 0.5 { level as f64 } else { crate::t0::hydro::DRY as f64 };
        let h = site.height(p);
        if water > h {
            return 1;
        }
        if water - h > -3.0 && water <= sea + 0.5 {
            let d = |o: P| site.height(add(p, o)) - site.height(sub(p, o));
            let (gx, gy) = (d([5.0, 0.0]) / 5.0, d([0.0, 5.0]) / 5.0);
            if crate::battlemap::beach(water - h, water, sea, crate::core::sqrt(gx * gx + gy * gy)) {
                return 1;
            }
        }
    }
    match Biome::from_u8(t0.sample_biome(w[0], w[1], t0.biome_warp(w[0], w[1]))[0]) {
        Biome::Ocean | Biome::Lake => 1,
        Biome::HotDesert => 2,
        _ => 0,
    }
}

/// Whether a hot desert's ground at `c` (world ft) is watered: a river or standing water (an
/// oasis) within about 600 ft.
fn watered(site: &Site, c: P) -> bool {
    site.near_river(sub(c, site.center), 600.0)
        || [0.0, 300.0, 600.0].into_iter().any(|r: f64| {
            (0..if r > 0.0 { 8 } else { 1 }).any(|k| {
                let t = std::f64::consts::TAU * k as f64 / 8.0;
                site.t0.sample_water(c[0] + r * libm::cos(t), c[1] + r * libm::sin(t)) > crate::t0::hydro::DRY
            })
        })
}

/// The ground under a polygon (world ft) every `SAND_STEP` on the world lattice, out to the margin
/// round it: each point, whether it is inside, and its kind (`sand_kind`; a watered desert's is
/// the land's, 3). None where no sand can be near; empty where it is sand all over (at its corners,
/// its sides' middles and its middle).
fn sand_samples(site: &Site, sea: f64, poly: &[P]) -> Option<Vec<(P, bool, u8)>> {
    let t0 = site.t0;
    let biome = |q: &P| Biome::from_u8(t0.sample_biome(q[0], q[1], t0.biome_warp(q[0], q[1]))[0]);
    let c = centroid(poly);
    // (Looked at only where its corners, its sides' middles or its middle are sand, or the sea's
    // shore is near them: a beach within 6 ft of height.)
    let n = poly.len();
    let probes = poly.iter().copied().chain((0..n).map(|k| lerp(poly[k], poly[(k + 1) % n], 0.5))).chain(std::iter::once(c));
    let probes: Vec<P> = probes.collect();
    let mut desert = false;
    let mut near = false;
    for q in &probes {
        let b = biome(q);
        desert |= b == Biome::HotDesert;
        near |= matches!(b, Biome::Ocean | Biome::Lake | Biome::HotDesert) || {
            let (level, mask) = t0.sample_lake(q[0], q[1]);
            level > crate::t0::hydro::DRY && {
                let h = site.height(sub(*q, site.center));
                (mask >= 0.5 && level as f64 > h) || (level as f64 <= sea + 0.5 && h < level as f64 + 9.0)
            }
        };
    }
    if !near {
        return None;
    }
    let wet = desert && watered(site, c);
    // (Sand at every one of them: sand all over, as good as.)
    if probes.iter().all(|&q| match sand_kind(site, sea, q) {
        2 => !wet,
        k => k == 1,
    }) {
        return Some(Vec::new());
    }
    let reach = SAND_MARGIN + 0.5 * SAND_STEP;
    let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let f = |v: f64| crate::core::floor(v / SAND_STEP) as i64;
    // (Every other point first, the rest looked at only between two that differ, else as they are:
    // the lattice's even points, so a point's kind never depends on the polygon.)
    let even = |v: i64| v - v.rem_euclid(2);
    let (i0, j0) = (even(f(x0 - reach)), even(f(y0 - reach)));
    let (i1, j1) = (even(f(x1 + reach) + 2), even(f(y1 + reach) + 2));
    let w = (i1 - i0 + 1) as usize;
    let at = |i: i64, j: i64| (j - j0) as usize * w + (i - i0) as usize;
    let kind = |i: i64, j: i64| match sand_kind(site, sea, [i as f64 * SAND_STEP, j as f64 * SAND_STEP]) {
        2 if wet => 3,
        k => k,
    };
    let mut grid = vec![0u8; w * (j1 - j0 + 1) as usize];
    for j in (j0..=j1).step_by(2) {
        for i in (i0..=i1).step_by(2) {
            grid[at(i, j)] = kind(i, j);
        }
    }
    let mut out = Vec::new();
    for j in j0..=j1 {
        for i in i0..=i1 {
            let p = [i as f64 * SAND_STEP, j as f64 * SAND_STEP];
            let inside = contains(poly, p);
            if !inside && (0..n).all(|k| seg_dist(p, poly[k], poly[(k + 1) % n]) > reach) {
                continue;
            }
            let k = if i.rem_euclid(2) == 0 && j.rem_euclid(2) == 0 {
                grid[at(i, j)]
            } else {
                let (xs, ys) = (if i.rem_euclid(2) == 0 { [i, i] } else { [i - 1, i + 1] }, if j.rem_euclid(2) == 0 { [j, j] } else { [j - 1, j + 1] });
                let first = grid[at(xs[0], ys[0])];
                if xs.iter().all(|&a| ys.iter().all(|&b| grid[at(a, b)] == first)) { first } else { kind(i, j) }
            };
            out.push((p, inside, k));
        }
    }
    Some(out)
}

/// A field plot off sand (v63; `poly` local ft, as the plot comes and goes): where it reaches onto a
/// beach, a shore's sand or a hot desert's unwatered sand it is cut back along a straight line to
/// the land, a few feet short of the sand (`SAND_MARGIN`; of 32 ways round, the line keeping most
/// of its land), and dropped (`None`) where little of it is left. A pure function of its ground.
pub(super) fn off_sand(site: &Site, poly: &[P]) -> Option<Vec<P>> {
    let f: Vec<P> = poly.iter().map(|&p| add(p, site.center)).collect();
    let Some(pts) = sand_samples(site, site.sea, &f) else { return Some(poly.to_vec()) };
    if pts.is_empty() {
        return None;
    }
    let bad: Vec<P> = pts.iter().filter(|s| matches!(s.2, 1 | 2)).map(|s| s.0).collect();
    if bad.is_empty() {
        return Some(poly.to_vec());
    }
    let good: Vec<P> = pts.iter().filter(|s| s.1 && matches!(s.2, 0 | 3)).map(|s| s.0).collect();
    let mut best: Option<(usize, P, f64)> = None;
    for k in 0..32 {
        let t = std::f64::consts::TAU * k as f64 / 32.0;
        let n = [libm::cos(t), libm::sin(t)];
        let c = bad.iter().map(|&b| dot(b, n)).fold(f64::MIN, f64::max) + SAND_MARGIN + 0.5 * SAND_STEP;
        let kept = good.iter().filter(|&&g| dot(g, n) > c).count();
        if best.is_none_or(|b| kept > b.0) {
            best = Some((kept, n, c));
        }
    }
    let (kept, n, c) = best?;
    if kept * 3 < good.len() {
        return None;
    }
    let piece = clean(&clip_plain(&f, mul(n, -1.0), -c));
    if piece.len() < 3 || area(&piece).abs() < (0.3 * area(&f).abs()).max(1500.0) || obb(&piece).short < 20.0 {
        return None;
    }
    Some(piece.into_iter().map(|p| sub(p, site.center)).collect())
}

/// A village's crofts off sand (v63, world ft, once its buildings and fields are final).
pub(super) fn crofts_off_sand(site: &Site, l: &mut Layout) {
    let sea = site.sea;
    // Crofts: shortened from the back (their front on the lane and their cottage kept) until clear
    // of the sand by the margin, else dropped. (A churchyard keeps its low stone wall.)
    let homes: Vec<Vec<P>> = l.buildings.iter().map(|b| b.poly.clone()).collect();
    let crofts = std::mem::take(&mut l.crofts);
    for mut c in crofts {
        if c.kind == super::village::PlotKind::Churchyard || c.pts.len() < 3 {
            l.crofts.push(c);
            continue;
        }
        let Some(pts) = sand_samples(site, sea, &c.pts) else {
            l.crofts.push(c);
            continue;
        };
        let bad: Vec<P> = if pts.is_empty() { c.pts.clone() } else { pts.iter().filter(|s| matches!(s.2, 1 | 2)).map(|s| s.0).collect() };
        if bad.is_empty() {
            l.crofts.push(c);
            continue;
        }
        // (A shrine's or the manor's yard on sand is walled: no hedge or fence round it.)
        if c.kind != super::village::PlotKind::Croft || c.pts.len() != 4 {
            c.bound = super::village::Bound::Wall;
            l.crofts.push(c);
            continue;
        }
        let q = c.pts.clone();
        let held: Vec<&Vec<P>> = homes.iter().filter(|h| contains(&q, centroid(h))).collect();
        let clear = |quad: &[P]| {
            let reach = SAND_MARGIN + 0.5 * SAND_STEP;
            bad.iter().all(|&b| !contains(quad, b) && (0..4).all(|k| seg_dist(b, quad[k], quad[(k + 1) % 4]) > reach))
                && held.iter().all(|h| h.iter().all(|&v| contains(quad, v) && (0..4).all(|k| seg_dist(v, quad[k], quad[(k + 1) % 4]) > 2.0)))
        };
        let short = (4..=9).rev().map(|t| t as f64 / 10.0).map(|s| vec![q[0], q[1], lerp(q[1], q[2], s), lerp(q[0], q[3], s)]).find(|quad| clear(quad));
        if let Some(quad) = short {
            c.pts = quad;
            l.crofts.push(c);
        }
    }
}

/// Ground steeper than this (rise over run across the field) is pasture or walled.
const STEEP: f64 = 0.05;

/// Every field's info, from its place and the ground under it (`fields` in world ft, `l.center` the
/// settlement's middle, `site` planned round it).
pub(super) fn assign(site: &Site, seed: u64, l: &mut Layout) {
    let biome = crate::battlemap::land_biome(site.t0, l.center);
    let t0 = site.t0;
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
        // (A hot desert's watered fields, `off_sand`, are crops, open: no grazing on its sand.)
        let own_at = |q: &P| Biome::from_u8(t0.sample_biome(q[0], q[1], t0.biome_warp(q[0], q[1]))[0]);
        let dry = f.iter().chain(std::iter::once(&c)).any(|q| own_at(q) == Biome::HotDesert);
        let kind = if roll(1) < pasture && !dry {
            FieldUse::Pasture
        } else if roll(2) < 0.13 {
            FieldUse::Fallow
        } else {
            FieldUse::Arable
        };
        // (A pasture in a forest biome, its ground the forest's floor, is open ground: no bound, no
        // rows.)
        let own = match own_at(&c) {
            Biome::Ocean | Biome::Lake => biome,
            b => b,
        };
        let open = dry || (kind == FieldUse::Pasture && matches!(own, Biome::TemperateForest | Biome::TemperateRainforest | Biome::Taiga | Biome::Jungle));
        if fruit && !steep && !wet && !dry && kind != FieldUse::Pasture {
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
        // (open-field strips never, but for a village's closes); stock is fenced in more often than crops.
        let (wall, hedge) = if steep { ((wall0 + 0.45).min(0.9), hedge0.min(0.1)) } else { (wall0, hedge0) };
        let keep = if kind == FieldUse::Pasture { 1.15 } else { 1.0 } * (1.0 - 0.4 * rim);
        let b = roll(5);
        // (Round a village, most of its small fields near it are hedged all round, strips too: a
        // bocage of little closes.)
        let close = village && far < 1.7 && area(f).abs() < 20000.0 && roll(6) < 0.65;
        let bound = if open {
            FieldBound::None
        } else if close {
            if hedge0 >= wall0 { FieldBound::Hedge } else { FieldBound::Wall }
        } else if strips {
            FieldBound::None
        } else if b < wall * keep {
            FieldBound::Wall
        } else if b < (wall + hedge) * keep {
            FieldBound::Hedge
        } else {
            FieldBound::None
        };
        info.push(FieldInfo { angle: angle as f32, kind, bound, strips, tone: (h >> 40) as u8 & 7, rows: !open });
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

/// Whether a field's square (by its byte, `square_byte`) is crop ground (furrows) and not grazing.
pub fn is_crop(byte: u8) -> bool {
    byte >> 6 != 3
}
