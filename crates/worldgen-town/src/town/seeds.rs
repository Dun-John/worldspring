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
    /// (161) Each seed's: on the planned lattice.
    pub(super) lattice: Vec<bool>,
    /// (189) Each seed's heading (radians) its lots run with on the planned part (`TownPlan::frames`).
    pub(super) frames: Vec<f64>,
}

/// Seeds the patches of settlement `s` (urban radius `r`); draws the per-patch stream's seed
/// from `rng` (its first two draws). (161) A town planned (`planning`, 0-1: `town::planning`)
/// has the part of it beyond its grown core (all of it, planned throughout) seeded on a lattice
/// turned to `axis` (radians: its main road's way in), so its patches, and the streets between
/// them, are regular blocks; the lattice's seeds are never relaxed. (189) On `pattern` Rings or
/// Radial the planned part is seeded on rings round the middle instead (`ring_seeds`), beyond a
/// round grown core.
#[allow(clippy::too_many_arguments)]
pub(super) fn plant(site: &Site, s: &Settlement, r: f64, planning: f64, axis: f64, pattern: super::pattern::Pattern, rng: &mut Pcg32) -> Seeds {
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
    // (161) The grown core: the side of a line across the town (`planning::core_line`), the
    // planned quarter beyond it, which way out rolled (along or across the lattice); and the
    // lattice: square blocks, or (half the towns) blocks drawn out along the axis, avenues
    // between them.
    let line = super::planning::core_line(planning);
    let out = axis + std::f64::consts::FRAC_PI_2 * (roll(0, 9) * 4.0).floor();
    let out = [libm::cos(out), libm::sin(out)];
    // (189: on rings, the grown core is round, a little wider the less planned the town.)
    let ringed = pattern.polar() && line.is_some();
    let core_r = match line {
        Some(t) if t >= 0.0 => r_edge * (0.5 - 0.25 * (planning - super::planning::MIXED) / (super::planning::PLANNED - super::planning::MIXED)),
        _ => 0.0,
    };
    let grown = |p: P| if ringed { len(p) < core_r } else { line.is_none_or(|t| dot(p, out) < t * r_edge) };
    let aspect = if roll(0, 7) < 0.5 { 1.0 + 0.15 * roll(0, 8) } else { 1.4 + 0.6 * roll(0, 8) };
    let (gx, gy) = (spacing * crate::core::sqrt(aspect), spacing / crate::core::sqrt(aspect));
    let (gu, gv) = ([libm::cos(axis), libm::sin(axis)], [-libm::sin(axis), libm::cos(axis)]);
    // (Planned throughout: the market square is the lattice's middle block.)
    let (pa, plaza_s, plaza_l) = if !ringed && line.is_some_and(|t| t < 0.0) { (axis, gx, gy) } else { (pa, plaza_s, plaza_l) };
    // (189) The rings' depth and the blocks' length along them (ft): Rings long blocks between
    // close rings, two lots deep (a ring street's houses back to back with the next's), Radial
    // long blocks out from the middle between many spokes.
    let (dr, w) = if pattern == super::pattern::Pattern::Rings {
        let dr = (0.6 * spacing).clamp(110.0, 170.0);
        (dr, 2.2 * dr)
    } else {
        let w = (0.7 * spacing).clamp(110.0, 200.0);
        ((1.6 * spacing).max(3.0 * w), w)
    };
    // (Where the first ring's blocks start: the grown core's edge, else just past the market
    // square; the rings fitted between there and the town's edge, the last along it.)
    let start = if core_r > 0.0 { core_r } else if rhombus { plaza_l + 0.45 * spacing } else { 0.35 * spacing };
    let dr = (r_edge - start).max(1.0) / crate::core::round((r_edge - start) / dr).max(1.0);
    let g = if ringed { dr.min(w) } else { gx.min(gy) };
    let block = if ringed { dr * w } else { gx * gy };
    // (Seed, fixed, on the lattice.)
    let mut seeds: Vec<(P, bool, bool)> = Vec::new();
    if rhombus {
        let h = std::f64::consts::FRAC_PI_2;
        seeds.push(([0.0, 0.0], true, false));
        for (d, a) in [(plaza_s, pa), (plaza_l, pa + h), (plaza_s, pa + 2.0 * h), (plaza_l, pa + 3.0 * h)] {
            seeds.push((polar(d, a), true, false));
        }
    }
    // (161) The shore's points, wanted by the lattice before the shore's seed pairs (below).
    let shore = shore_points(site, 0.5 * spacing, r_out * 1.1);
    // (Each lattice seed's mirror across the water, if it has one: their seeds' index pairs.)
    let mut partner: Vec<(usize, usize)> = Vec::new();
    if line.is_some() {
        let jit = (0.12 * (1.0 - planning) / (1.0 - super::planning::MIXED)).max(0.01) * spacing;
        let k = (r_edge / gx.min(gy)) as i64 + 1;
        // The lattice holds up to the water: a block by a river or the shore is clipped to the
        // bank by a mirror seed across it (the edge between them on the water's line), never
        // dropped for an odd patch. By a river, the town's middle's side keeps its lattice and
        // the far bank's band is its mirror; by the shore, a mirror stands in the water.
        let band = 0.75 * spacing;
        let mut cands: Vec<P> = Vec::new();
        if ringed {
            cands = ring_seeds(pseed, start, r_edge, dr, w, axis, jit, pattern == super::pattern::Pattern::Rings);
        } else {
            for ix in -k..=k {
                for iy in -k..=k {
                    let mut sr = Pcg32::new(hash2(pseed, ix, iy ^ 0x1a77), 67);
                    let (ja, j) = (sr.range(0.0, std::f64::consts::TAU), sr.range(0.0, jit));
                    cands.push(add(add(mul(gu, ix as f64 * gx), mul(gv, iy as f64 * gy)), [j * libm::cos(ja), j * libm::sin(ja)]));
                }
            }
        }
        let mut pts: Vec<(P, Option<P>)> = Vec::new();
        {
            for p in cands {
                if len(p) >= r_edge || grown(p) || seeds.iter().any(|(q, fixed, _)| *fixed && dist(*q, p) < 0.5 * g) {
                    continue;
                }
                if site.near_river(p, band) {
                    let Some(&(a, b, hw)) = site.river.iter().min_by(|x, y| seg_dist(p, x.0, x.1).total_cmp(&seg_dist(p, y.0, y.1))) else { continue };
                    let t = sub(b, a);
                    let tl = len(t);
                    if tl < 1e-9 {
                        continue;
                    }
                    let n = [-t[1] / tl, t[0] / tl];
                    let (sp, so) = (dot(sub(p, a), n), dot(mul(a, -1.0), n));
                    let m = sub(p, mul(n, 2.0 * sp));
                    if sp.abs() < hw + 0.2 * spacing {
                        // (A seed on the channel: its block split by the river, a half on each bank.)
                        let c = sub(p, mul(n, sp));
                        let d = hw + 0.15 * g;
                        pts.push((add(c, mul(n, d)), Some(sub(c, mul(n, d)))));
                    } else if (sp > 0.0) == (so > 0.0) {
                        pts.push((p, Some(m)));
                    } else {
                        // (The far bank's: kept unless a mirror stands there.)
                        pts.push((p, None));
                    }
                    continue;
                }
                if site.wet(p) {
                    continue;
                }
                let near_shore = shore.iter().filter(|(q, _)| dist(*q, p) < band).min_by(|x, y| dist(x.0, p).total_cmp(&dist(y.0, p)));
                let m = near_shore.map(|&(q, n)| sub(p, mul(n, 2.0 * dot(sub(p, q), n)))).filter(|&m| site.wet(m));
                pts.push((p, m));
            }
        }
        // (A lattice seed hard by another's mirror gives way to it.)
        let mirrors: Vec<P> = pts.iter().filter_map(|x| x.1).collect();
        for (p, m) in pts {
            if mirrors.iter().any(|q| dist(*q, p) < 0.55 * g) {
                continue;
            }
            seeds.push((p, false, true));
            if let Some(m) = m {
                partner.push((seeds.len() - 1, seeds.len()));
                seeds.push((m, false, true));
            }
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
        // (161: the planned quarter's seeds are the lattice's.)
        if i < n_spiral && !grown(p) {
            continue;
        }
        // Near a river, seeds come in mirrored pairs across the channel instead (below).
        if site.near_river(p, 0.6 * spacing) {
            continue;
        }
        seeds.push((p, false, false));
    }
    // River: pairs of seeds mirrored across the centreline put a patch edge on the channel,
    // so blocks end at quays along it instead of losing buildings to the water one by one.
    for (c, t, hw) in river_samples(site, 0.9 * spacing, r_out * 1.1) {
        let n = [-t[1], t[0]];
        let off = hw + 0.45 * spacing;
        // (Not where the lattice meets the river: its mirrors put the edge on the channel.)
        if seeds.iter().any(|(p, _, lat)| *lat && dist(*p, c) < hw + 0.8 * spacing) {
            continue;
        }
        for q in [add(c, mul(n, off)), sub(c, mul(n, off))] {
            if !seeds.iter().any(|(p, fixed, _)| *fixed && dist(*p, q) < 0.4 * spacing) {
                seeds.push((q, true, false));
            }
        }
    }
    // Coast and lake shore: like the river, mirrored seed pairs across the shoreline put a
    // patch edge on it, so blocks line the water with a quay instead of straddling it.
    let mut shore_used: Vec<P> = Vec::new();
    for &(q, n) in &shore {
        if shore_used.iter().any(|u| dist(*u, q) < 0.85 * spacing) || site.near_river(q, 0.6 * spacing) || seeds.iter().any(|(p, _, lat)| *lat && dist(*p, q) < 0.85 * spacing) {
            continue;
        }
        shore_used.push(q);
        seeds.push((sub(q, mul(n, 0.45 * spacing)), true, false));
        seeds.push((add(q, mul(n, 0.45 * spacing)), true, false));
    }
    // (Lattice seeds kept, and their partners' indices with them.)
    let mut keep: Vec<bool> = seeds.iter().map(|(p, fixed, lat)| *fixed || *lat || shore_used.iter().all(|u| dist(*u, *p) > 0.55 * spacing)).collect();
    // (A lattice block left a sliver or a small triangle beside the water or the square gives its
    // ground to its neighbours: its seed, and its mirror, dropped.)
    if line.is_some() {
        let pos: Vec<P> = seeds.iter().map(|x| x.0).collect();
        let first = voronoi(&pos, &circle([0.0, 0.0], r_out * 1.3, 48));
        for i in 0..seeds.len() {
            let c = &first[i].pts;
            if !seeds[i].2 || c.len() < 3 || len(seeds[i].0) >= r_edge || site.wet(seeds[i].0) {
                continue;
            }
            let a = area(c).abs();
            let corners = (0..c.len()).filter(|&k| {
                let (p, q, r) = (c[(k + c.len() - 1) % c.len()], c[k], c[(k + 1) % c.len()]);
                let (d1, d2) = (sub(q, p), sub(r, q));
                len(d1) > 0.15 * g && len(d2) > 0.15 * g && libm::atan2(cross(d1, d2), dot(d1, d2)).abs() > 15f64.to_radians()
            }).count();
            if a < 0.3 * block || (corners <= 3 && a < 0.6 * block) {
                keep[i] = false;
                for &(x, y) in &partner {
                    if x == i {
                        keep[y] = false;
                    } else if y == i {
                        keep[x] = false;
                    }
                }
            }
        }
    }
    let mut k = 0;
    seeds.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    // Inner (city) seeds first.
    let (inner_seeds, outer_seeds): (Vec<_>, Vec<_>) = seeds.into_iter().partition(|(p, _, _)| len(*p) < r_edge);
    let n_inner = inner_seeds.len();
    let seeds: Vec<(P, bool, bool)> = inner_seeds.into_iter().chain(outer_seeds).collect();
    let total = seeds.len();
    let mut sites: Vec<P> = seeds.iter().map(|x| x.0).collect();
    let fixed_seed: Vec<bool> = seeds.iter().map(|x| x.1 || x.2).collect();
    // (Planned throughout: the blocks by the water and the plaza's ring are cut as the lattice's.)
    let all = line.is_some_and(|t| t < 0.0);
    let lattice: Vec<bool> = seeds.iter().map(|x| x.2 || (all && x.1)).collect();
    // (189: on rings, a block's lots run along its ring (Rings) or out from the middle (Radial).)
    let turn = if pattern == super::pattern::Pattern::Rings { std::f64::consts::FRAC_PI_2 } else { 0.0 };
    let frames: Vec<f64> = seeds.iter().map(|x| if ringed { libm::atan2(x.0[1], x.0[0]) + turn } else { axis }).collect();
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
    // (189) On Rings, the corners near a line between rings drawn onto it (as far as planned:
    // half the way at the least, all of it planned throughout), so the ring streets run round
    // the middle, not zigzagging between rings of blocks of unlike counts (never by the river
    // or on the water, whose banks the cells keep). A corner moves the same in every cell it is
    // on (a function of where it is).
    if ringed && pattern == super::pattern::Pattern::Rings {
        let strict = 0.5 + 0.5 * ((planning - super::planning::MIXED) / (1.0 - super::planning::MIXED)).clamp(0.0, 1.0);
        for c in cells.iter_mut() {
            for p in c.pts.iter_mut() {
                let l = len(*p);
                let k = crate::core::round((l - start) / dr);
                let rk = start + k * dr;
                if k >= 0.0 && rk < r_edge - 1.0 && (l - rk).abs() < 0.3 * dr && l > 1e-9 && !site.near_river(*p, 0.35 * dr) && !site.wet(*p) {
                    *p = mul(*p, (l + strict * (rk - l)) / l);
                }
            }
        }
    }
    for c in cells.iter_mut().skip(n_inner) {
        if c.pts.iter().any(|p| len(*p) > r_out) {
            *c = Labeled { pts: Vec::new(), labels: Vec::new() };
        }
    }
    Seeds { pseed, spacing, r_out, rhombus, n_inner, total, sites, cells, lattice, frames }
}

/// (189) The planned part's seeds on rings round the middle, from `start` out to `r_edge`: rings
/// `dr` apart, on each the seeds about `w` apart (their count doubled as the rings widen, so the
/// spokes between them run on out), turned so a spoke runs along `axis`, each jittered up to
/// `jit` (from `pseed`, by ring and place). The cells between are blocks with ring streets on
/// their long sides (`rings`), or with spokes on theirs.
#[allow(clippy::too_many_arguments)]
fn ring_seeds(pseed: u64, start: f64, r_edge: f64, dr: f64, w: f64, axis: f64, jit: f64, rings: bool) -> Vec<P> {
    let tau = std::f64::consts::TAU;
    let mut out = Vec::new();
    let mut n = ((tau * (start + 0.5 * dr) / w) as usize).max(if rings { 6 } else { 8 });
    for k in 0.. {
        let rho = start + (k as f64 + 0.5) * dr;
        if rho >= r_edge {
            break;
        }
        while tau * rho / n as f64 > 1.45 * w {
            n *= 2;
        }
        for i in 0..n {
            let mut sr = Pcg32::new(hash2(pseed, k as i64, i as i64 ^ 0x2a11_0000), 67);
            let (ja, j) = (sr.range(0.0, tau), sr.range(0.0, jit));
            let a = axis + tau * (i as f64 + 0.5) / n as f64;
            out.push([rho * libm::cos(a) + j * libm::cos(ja), rho * libm::sin(a) + j * libm::sin(ja)]);
        }
    }
    out
}
