//! A town plan's patch seeds (`plan_town`'s first phase): the plaza's rhombus, the spiral of
//! town and outskirts seeds, pairs mirrored across a river and a shore, and their Voronoi cells
//! (the town's relaxed twice; outskirts cells reaching past the outskirts dropped).

use super::*;

const GOLDEN: f64 = 2.399_963_229_728_653;

/// A patch's roll at a stage of the plan, from the per-patch streams' seed (`Seeds::pseed`).
#[inline]
pub(super) fn roll(pseed: u64, i: usize, stage: i64) -> f64 {
    unit(hash3(pseed, i as i64, stage, 0x9a7c))
}

/// What the rest of the plan reads of the seeding: the per-patch stream's seed (`roll`), the
/// patch spacing, the outskirts' radius, whether the plaza is the rhombus (face 0), the town's
/// patch count (its seeds first), every seed and its cell.
pub(super) struct Seeds {
    pub(super) pseed: u64,
    pub(super) spacing: f64,
    pub(super) r_out: f64,
    pub(super) rhombus: bool,
    pub(super) n_inner: usize,
    pub(super) total: usize,
    pub(super) sites: Vec<P>,
    pub(super) cells: Vec<Labeled>,
}

/// Seeds the patches of settlement `s` (urban radius `r`); draws the per-patch stream's seed
/// from `rng` (its first two draws).
pub(super) fn plant(site: &Site, s: &Settlement, r: f64, rng: &mut Pcg32) -> Seeds {
    let pop = s.population as f64;
    let n_spiral = match s.tier {
        Tier::Town => (pop / 220.0).clamp(9.0, 26.0),
        _ => (pop / 420.0).clamp(12.0, 110.0),
    } as usize;
    // Per-patch random streams: a roll depends only on (settlement, patch, stage), so a
    // change in one patch never reshuffles the rest.
    let pseed = rng.next_u32() as u64 | (rng.next_u32() as u64) << 32;
    let roll = |i: usize, stage: i64| self::roll(pseed, i, stage);
    let spacing = r * crate::core::sqrt(std::f64::consts::PI / n_spiral as f64);
    let r_edge = r * crate::core::sqrt((n_spiral as f64 + 0.5) / n_spiral as f64);
    // Outskirts: seed radius grows linearly with the index, so patches (and the fields cut
    // from them) grow larger away from town; cells reaching past `r_out` are dropped, which
    // leaves an organic edge instead of a circle.
    let n_outer = ((n_spiral as f64 * 1.6) as usize).max(12);
    let r_out = OUTSKIRTS * r;
    let step = (r_out - r_edge) / n_outer as f64;
    // The plaza: a seed at the centre ringed by four at half-diagonals s and L, so the
    // centre's cell is an s × L rectangle (watabou's market square).
    let polar = |d: f64, a: f64| [d * libm::cos(a), d * libm::sin(a)];
    let pa = roll(0, 1) * std::f64::consts::TAU;
    // (Each side √¾ of what it was: squares about three quarters of the old area.)
    let plaza_s = spacing * 0.866 * (0.55 + 0.25 * roll(0, 2));
    let plaza_l = plaza_s * (1.0 + 0.5 * roll(0, 3));
    let rhombus = !site.near_river([0.0, 0.0], plaza_l);
    let mut seeds: Vec<(P, bool)> = Vec::new();
    if rhombus {
        let h = std::f64::consts::FRAC_PI_2;
        seeds.push(([0.0, 0.0], true));
        for (d, a) in [(plaza_s, pa), (plaza_l, pa + h), (plaza_s, pa + 2.0 * h), (plaza_l, pa + 3.0 * h)] {
            seeds.push((polar(d, a), true));
        }
    }
    for i in 0..n_spiral + n_outer {
        if rhombus && i < 5 {
            continue;
        }
        let mut sr = Pcg32::new(hash2(pseed, i as i64, 0x5eed), 61);
        let (rad, local) = if i < n_spiral {
            (r * crate::core::sqrt((i as f64 + 0.5) / n_spiral as f64), spacing)
        } else {
            let rad = r_edge + ((i - n_spiral) as f64 + 0.5) * step;
            (rad, crate::core::sqrt(std::f64::consts::TAU * rad * step))
        };
        let a = i as f64 * GOLDEN + sr.range(-0.3, 0.3);
        let j = sr.range(0.0, 0.3 * local);
        let ja = sr.range(0.0, std::f64::consts::TAU);
        let p = [rad * libm::cos(a) + j * libm::cos(ja), rad * libm::sin(a) + j * libm::sin(ja)];
        // Near a river, seeds come in mirrored pairs across the channel instead (below).
        if site.near_river(p, 0.6 * spacing) {
            continue;
        }
        seeds.push((p, false));
    }
    // River: pairs of seeds mirrored across the centreline put a patch edge on the channel,
    // so blocks end at quays along it instead of losing buildings to the water one by one.
    for (c, t, hw) in river_samples(site, 0.9 * spacing, r_out * 1.1) {
        let n = [-t[1], t[0]];
        let off = hw + 0.45 * spacing;
        for q in [add(c, mul(n, off)), sub(c, mul(n, off))] {
            if !seeds.iter().any(|(p, fixed)| *fixed && dist(*p, q) < 0.4 * spacing) {
                seeds.push((q, true));
            }
        }
    }
    // Coast and lake shore: like the river, mirrored seed pairs across the shoreline put a
    // patch edge on it, so blocks line the water with a quay instead of straddling it.
    let shore = shore_points(site, 0.5 * spacing, r_out * 1.1);
    let mut shore_used: Vec<P> = Vec::new();
    for &(q, n) in &shore {
        if shore_used.iter().any(|u| dist(*u, q) < 0.85 * spacing) || site.near_river(q, 0.6 * spacing) {
            continue;
        }
        shore_used.push(q);
        seeds.push((sub(q, mul(n, 0.45 * spacing)), true));
        seeds.push((add(q, mul(n, 0.45 * spacing)), true));
    }
    seeds.retain(|(p, fixed)| *fixed || shore_used.iter().all(|u| dist(*u, *p) > 0.55 * spacing));
    // Inner (city) seeds first.
    let (inner_seeds, outer_seeds): (Vec<_>, Vec<_>) = seeds.into_iter().partition(|(p, _)| len(*p) < r_edge);
    let n_inner = inner_seeds.len();
    let seeds: Vec<(P, bool)> = inner_seeds.into_iter().chain(outer_seeds).collect();
    let total = seeds.len();
    let mut sites: Vec<P> = seeds.iter().map(|x| x.0).collect();
    let fixed_seed: Vec<bool> = seeds.iter().map(|x| x.1).collect();
    let boundary = circle([0.0, 0.0], r_out * 1.3, 48);
    let mut cells = voronoi(&sites, &boundary);
    for _ in 0..2 {
        for i in 0..n_inner {
            if cells[i].pts.len() >= 3 && !fixed_seed[i] {
                sites[i] = centroid(&cells[i].pts);
            }
        }
        cells = voronoi(&sites, &boundary);
    }
    for c in cells.iter_mut().skip(n_inner) {
        if c.pts.iter().any(|p| len(*p) > r_out) {
            *c = Labeled { pts: Vec::new(), labels: Vec::new() };
        }
    }
    Seeds { pseed, spacing, r_out, rhombus, n_inner, total, sites, cells }
}
