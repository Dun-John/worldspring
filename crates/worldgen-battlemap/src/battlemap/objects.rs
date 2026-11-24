//! Object placement and the battlemap guarantee: cover, tiers, yards, raised and built features.

use super::*;

/// Side of `place_objects`' road grid: the chunk and two squares round it.
pub(super) const RN: usize = SQ + 4;

#[allow(clippy::too_many_arguments)]
pub(super) fn place_objects(c: &mut Chunk, biome: &[Biome], slope: &[f64], road_near: &[bool], yard: &Yard, keep: &Keep, seed: u64, gsx: i64, gsy: i64) {
    let grove_seed = hash2(seed, 0x6007, 2);
    // Each (biome, kind) pair has its own jittered global lattice; objects belong to the
    // chunk that contains them, so placement is identical whichever chunk is asked.
    let mut kinds: Vec<(Biome, Kind, f64, bool)> = Vec::new();
    let mut seen = [false; crate::t0::biome::ALL.len()];
    for &b in biome {
        if !seen[b as usize] {
            seen[b as usize] = true;
            for &(kd, dens, wet) in profile(b) {
                kinds.push((b, kd, dens, wet));
            }
        }
    }
    kinds.sort_by(|a, b| (a.0 as u8, a.1 as u16).cmp(&(b.0 as u8, b.1 as u16)));
    for (b, kind, dens, wet_ok) in kinds {
        // Twice the lattice density, thinned by a clustering field (keyed to world position):
        // things gather in patches and leave open ground between, at the same mean density.
        let spacing = 10.0 / crate::core::sqrt(dens);
        let cell = (spacing * 0.7).max(1.0);
        let lat_seed = hash3(seed, b as i64, kind as u16 as i64, 0x0b1);
        let clump_seed = hash3(grove_seed, b as i64, kind as u16 as i64, 0xc1);
        let (lx0, ly0) = (crate::core::floor(gsx as f64 / cell) as i64 - 1, crate::core::floor(gsy as f64 / cell) as i64 - 1);
        let (lx1, ly1) = (crate::core::floor((gsx + SQ as i64) as f64 / cell) as i64 + 1, crate::core::floor((gsy + SQ as i64) as f64 / cell) as i64 + 1);
        let is_tree = info(kind as u16).height_ft >= 15.0 && info(kind as u16).blocks_move;
        let tall = info(kind as u16).height_ft >= 5.0;
        let roads = if tall { keep.verge } else { road_near };
        for ly in ly0..=ly1 {
            for lx in lx0..=lx1 {
                let h = hash2(lat_seed, lx, ly);
                let mut rng = Pcg32::new(h, 3);
                let px = (lx as f64 + rng.next_f64()) * cell;
                let py = (ly as f64 + rng.next_f64()) * cell;
                let (ix, iy) = (crate::core::floor(px) as i64 - gsx, crate::core::floor(py) as i64 - gsy);
                if ix < 0 || iy < 0 || ix >= SQ as i64 || iy >= SQ as i64 {
                    continue;
                }
                let k = iy as usize * SQ + ix as usize;
                if biome[k] != b || c.surface[k].is_road() || c.building[k] != 0 {
                    continue;
                }
                // No trees or plants on a beach (a hot desert's own sand keeps its palms and cacti), nor a
                // tree whose trunk stands by one.
                let plant = TREES_AND_COVER.contains(&kind);
                if plant && beach_sand(c, k, b) {
                    continue;
                }
                if is_tree {
                    let rr = crate::core::ceil(info(kind as u16).radius as f64) as i64;
                    let by_beach = (-rr..=rr).any(|dy| {
                        (-rr..=rr).any(|dx| {
                            let (x, y) = (ix + dx, iy + dy);
                            x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && beach_sand(c, y as usize * SQ + x as usize, b)
                        })
                    });
                    if by_beach {
                        continue;
                    }
                }
                // Tundra: no bushes on its snow patches, no drifts on its grass.
                if b == Biome::Tundra && matches!((c.surface[k], kind), (Surface::Snow, Kind::Bush) | (Surface::TundraGrass, Kind::Snowdrift)) {
                    continue;
                }
                if c.urban[k] != 0 {
                    if !yard.ok.get(k).is_some_and(|&v| v) {
                        continue;
                    }
                    // In a bailey: off the ways (8 ft and the footprint's reach) and the gate mouths.
                    let reach_ft = (info(kind as u16).radius as f64 * 1.25) * SQUARE_FT;
                    let w = [yard.ox + (px - gsx as f64) * SQUARE_FT, yard.oy + (py - gsy as f64) * SQUARE_FT];
                    if yard.ways.iter().any(|&(a, b)| crate::town::geom::seg_dist(w, a, b) < 8.0 + reach_ft)
                        || yard.gates.iter().any(|&(g, hh)| crate::town::geom::seg_dist(w, g, hh) < 14.0 + reach_ft)
                    {
                        continue;
                    }
                }
                // Canopies and boulders keep clear of buildings (roofs stay visible), and
                // nothing overhangs a road or street: no square of one within the footprint.
                let r = info(kind as u16).radius as f64;
                // (A tree's drawn crown reaches 1.25 x its footprint at the largest scale, so trees and
                // boulders keep that, and one more square off a croft or field.)
                let (crown, farm_gap) = if tall { (1.6 * r, 1.0) } else { (1.1 * r, 0.0) };
                let reach = crate::core::ceil((r * 1.25).max(if tall { crown + farm_gap } else { 0.0 })) as i64;
                let (ox, oy) = (px - gsx as f64, py - gsy as f64);
                let blocked = (-reach..=reach).any(|dy| {
                    (-reach..=reach).any(|dx| {
                        let (x, y) = (ix + dx, iy + dy);
                        if x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && c.building[y as usize * SQ + x as usize] != 0 {
                            return true;
                        }
                        let ex = (x as f64 - ox).max(ox - (x + 1) as f64).max(0.0);
                        let ey = (y as f64 - oy).max(oy - (y + 1) as f64).max(0.0);
                        let within = ex * ex + ey * ey < crown * crown;
                        if (ex * ex + ey * ey < (crown + farm_gap) * (crown + farm_gap)) && x >= -1 && y >= -1 && x <= SQ as i64 && y <= SQ as i64 && keep.farm.get((y + 1) as usize * HS + (x + 1) as usize).is_some_and(|&f| f) {
                            return true;
                        }
                        if x < -2 || y < -2 || x >= SQ as i64 + 2 || y >= SQ as i64 + 2 || !roads[(y + 2) as usize * RN + (x + 2) as usize] {
                            return false;
                        }
                        within
                    })
                });
                if blocked {
                    continue;
                }
                // Groves and glades for trees (one shared field, so species mix in a grove);
                // tighter patches of rocks and bushes.
                let clump = if is_tree { 0.5 + 0.5 * fbm(grove_seed, px / 34.0, py / 34.0, 3, 2.0, 0.5) } else { 0.5 + 0.5 * fbm(clump_seed, px / 13.0, py / 13.0, 3, 2.0, 0.55) };
                let p = 0.85 * 0.98 * smoothstep(0.32, 0.68, clump);
                if rng.next_f64() > p {
                    continue;
                }
                let depth = c.water_level[k] - c.height[k];
                let ok = (if wet_ok { depth < 3.0 } else { depth <= 0.0 }) && !matches!(c.surface[k], Surface::Lava);
                if !ok || (is_tree && slope[k] > 0.8) {
                    continue;
                }
                c.objects.push(Object {
                    kind,
                    variant: (h >> 40) as u8 & 7,
                    x: (px - gsx as f64) as f32,
                    y: (py - gsy as f64) as f32,
                    rot: rng.range(0.0, std::f64::consts::TAU) as f32,
                    scale: rng.range(0.8, 1.25) as f32,
                    sprite: 0,
                    turned: false,
                });
            }
        }
    }
}

/// A beach square: sand that is not the biome's own (a hot desert's).
fn beach_sand(c: &Chunk, k: usize, b: Biome) -> bool {
    c.surface[k] == Surface::Sand && b != Biome::HotDesert
}

fn is_cover(o: &Object) -> bool {
    info(o.kind as u16).cover >= 1
}

/// Ground the guarantee counts: dry, bridge decks being exempt like the water they span (no
/// boulders on the bridge), and a crater's lava lake like water.
fn dry(c: &Chunk, k: usize) -> bool {
    c.water_level[k] <= c.height[k] && !matches!(c.surface[k], Surface::Planks | Surface::Lava)
}

/// Where props may go: dry ground that is not a building, road bed or bridge.
pub(super) fn placeable(c: &Chunk, k: usize) -> bool {
    dry(c, k) && c.building[k] == 0 && c.urban[k] != URBAN_ENTRANCE && c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] == 0
}

/// No road bed on this square or round it, nor a street round open ground (a rock or a bush
/// here doesn't overhang the way; a town's props still stand by its streets).
pub(super) fn clear_of_roads(c: &Chunk, k: usize) -> bool {
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    (-1..=1).all(|dy| {
        (-1..=1).all(|dx| {
            let (nx, ny) = (x + dx, y + dy);
            let inside = nx >= 0 && ny >= 0 && nx < SQ as i64 && ny < SQ as i64;
            c.road_halo[(ny + 1) as usize * HS + (nx + 1) as usize] == 0 && !(inside && c.urban[k] == 0 && c.urban[ny as usize * SQ + nx as usize] == URBAN_STREET)
        })
    })
}

/// The squares of `all` clear of roads (and off what is set out there already, a square's raised
/// features and their steps), else those off the road bed, else all of them; never one in
/// `keep_off` (walls, towers, gate passages and the squares next to them; raised features).
fn off_road_first(c: &Chunk, all: Vec<usize>, keep_off: &[bool]) -> Vec<usize> {
    // Squares under an object's footprint, within the bounds of `all`.
    let (x0, y0, x1, y1) = all.iter().fold((SQ, SQ, 0, 0), |a, &k| (a.0.min(k % SQ), a.1.min(k / SQ), a.2.max(k % SQ), a.3.max(k / SQ)));
    let w = (x1 + 1).saturating_sub(x0);
    let mut busy = vec![false; w * (y1 + 1).saturating_sub(y0)];
    for o in &c.objects {
        let r = c.info(o).radius * o.scale;
        if o.x + r < x0 as f32 || o.y + r < y0 as f32 || o.x - r > (x1 + 1) as f32 || o.y - r > (y1 + 1) as f32 {
            continue;
        }
        for y in (o.y - r).max(y0 as f32) as usize..=((o.y + r) as usize).min(y1) {
            for x in (o.x - r).max(x0 as f32) as usize..=((o.x + r) as usize).min(x1) {
                let (dx, dy) = (o.x - (x as f32 + 0.5), o.y - (y as f32 + 0.5));
                if dx * dx + dy * dy < (r + 0.5) * (r + 0.5) {
                    busy[(y - y0) * w + x - x0] = true;
                }
            }
        }
    }
    let open = |k: usize| !busy[(k / SQ - y0) * w + k % SQ - x0];
    let daises: Vec<([f64; 2], f64)> = c
        .structures
        .iter()
        .filter(|s| s.kind == ShapeKind::Dais)
        .map(|s| {
            let (d, (dx, dy)) = ([s.pts[0][0] as f64, s.pts[0][1] as f64], ((s.pts[1][0] - s.pts[0][0]) as f64, (s.pts[1][1] - s.pts[0][1]) as f64));
            (d, crate::core::sqrt(dx * dx + dy * dy) + 1.0)
        })
        .collect();
    let off_dais = |k: usize| {
        let (x, y) = ((k % SQ) as f64 + 0.5, (k / SQ) as f64 + 0.5);
        daises.iter().all(|&(d, r)| (x - d[0]) * (x - d[0]) + (y - d[1]) * (y - d[1]) > r * r)
    };
    let clear: Vec<usize> = all.iter().copied().filter(|&k| placeable(c, k) && clear_of_roads(c, k) && off_dais(k) && open(k) && !keep_off[k]).collect();
    if !clear.is_empty() {
        return clear;
    }
    let off_road: Vec<usize> = all.iter().copied().filter(|&k| placeable(c, k) && !keep_off[k]).collect();
    if off_road.is_empty() { all.into_iter().filter(|&k| !keep_off[k]).collect() } else { off_road }
}

/// The 32x32 windows (top-left squares) whose dry ground is all one tier, leaving out mostly-water
/// ones: the battlemap guarantee as `tests/vital/battlemap.rs` checks it.
fn flat_windows(c: &Chunk) -> Vec<(usize, usize)> {
    const W: usize = 32;
    let n = SQ - W + 1;
    // Per row, each run of W squares: lowest and highest dry tier, and how many are wet.
    let (mut lo, mut hi, mut wet) = (vec![i16::MAX; SQ * n], vec![i16::MIN; SQ * n], vec![0usize; SQ * n]);
    for y in 0..SQ {
        for x in 0..n {
            for k in y * SQ + x..y * SQ + x + W {
                if dry(c, k) {
                    lo[y * n + x] = lo[y * n + x].min(c.tier[k]);
                    hi[y * n + x] = hi[y * n + x].max(c.tier[k]);
                }
                wet[y * n + x] += (c.water_level[k] > c.height[k]) as usize;
            }
        }
    }
    let mut out = Vec::new();
    for wy in 0..n {
        for wx in 0..n {
            let (mut a, mut b, mut w) = (i16::MAX, i16::MIN, 0);
            for y in wy..wy + W {
                (a, b, w) = (a.min(lo[y * n + wx]), b.max(hi[y * n + wx]), w + wet[y * n + wx]);
            }
            if a == b && w * 2 < W * W {
                out.push((wx, wy));
            }
        }
    }
    out
}

/// On an open square, the squares of `cells` by its edge (a house or street within two squares),
/// where a stack of crates stands against a front rather than out on the paving; else all.
fn by_fronts(c: &Chunk, cells: Vec<usize>) -> Vec<usize> {
    if cells.iter().filter(|&&k| c.urban[k] == URBAN_PLAZA).count() * 2 <= cells.len() {
        return cells;
    }
    let edge: Vec<usize> = cells
        .iter()
        .copied()
        .filter(|&k| {
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            (-2..=2).any(|dy| {
                (-2..=2).any(|dx| {
                    let (i, j) = (x + dx, y + dy);
                    i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                        let q = j as usize * SQ + i as usize;
                        c.building[q] != 0 || c.urban[q] == URBAN_STREET
                    }
                })
            })
        })
        .collect();
    if edge.is_empty() { cells } else { edge }
}

/// On a field, the squares of `cells` by its edge or a hedgerow (within two squares of one that is
/// no field's, or of a hedgerow's or wall's, not on it), where a thorn bush or a cairn of cleared
/// stones stands rather than out on the ploughed land; none if there are none (else all, with
/// `fallback`). Not a field: all.
fn by_bounds(c: &Chunk, cells: Vec<usize>, fallback: bool) -> Vec<usize> {
    if cells.iter().filter(|&&k| c.urban[k] == URBAN_FIELD).count() * 2 <= cells.len() {
        return cells;
    }
    let run = |q: usize| c.yard.get(q).is_some_and(|&y| y & 8 != 0);
    let edge: Vec<usize> = cells
        .iter()
        .copied()
        .filter(|&k| {
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            !run(k)
                && (-2..=2).any(|dy| {
                    (-2..=2).any(|dx| {
                        let (i, j) = (x + dx, y + dy);
                        i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                            let q = j as usize * SQ + i as usize;
                            c.urban[q] != URBAN_FIELD || run(q)
                        }
                    })
                })
        })
        .collect();
    if edge.is_empty() && fallback { cells } else { edge }
}

/// The 32x32 windows `enforce` fills: those mostly dry with fewer than three cover objects and
/// buildings in them (every building with a square in a window, as `tests/vital/common.rs` counts); with
/// `feature`, those with no hazard or prop in them. What doesn't change while objects are added
/// (wet squares, the buildings in each window) is worked out once; the objects as they come.
struct Windows {
    feature: bool,
    /// Objects counted so far (objects are only added while `enforce` fills windows).
    counted: usize,
    /// Counted objects by square, and its summed-area table; the summed-area table of wet squares.
    at: Vec<u32>,
    cover: Vec<u32>,
    wet: Vec<u32>,
    /// Buildings in each window, once counted (up to three; `u8::MAX`: not yet).
    blds: Vec<u8>,
    /// The summed-area tables of a castle's yard's squares and of walls, towers and buildings
    /// (empty: no yard here, or `feature`): `works_cover`.
    yard: Vec<u32>,
    works: Vec<u32>,
}

impl Windows {
    const W: usize = 32;

    fn new(c: &Chunk, feature: bool) -> Self {
        let mut wet = vec![0u32; (SQ + 1) * (SQ + 1)];
        for y in 0..SQ {
            for x in 0..SQ {
                let k = y * SQ + x;
                let (i, up, left, diag) = ((y + 1) * (SQ + 1) + x + 1, y * (SQ + 1) + x + 1, (y + 1) * (SQ + 1) + x, y * (SQ + 1) + x);
                wet[i] = (c.water_level[k] > c.height[k]) as u32 + wet[up] + wet[left] - wet[diag];
            }
        }
        let n = SQ - Self::W + 1;
        let (mut yard, mut works) = (Vec::new(), Vec::new());
        if !feature && !c.yard.is_empty() {
            (yard, works) = (vec![0u32; (SQ + 1) * (SQ + 1)], vec![0u32; (SQ + 1) * (SQ + 1)]);
            for y in 0..SQ {
                for x in 0..SQ {
                    let k = y * SQ + x;
                    let (i, up, left, diag) = ((y + 1) * (SQ + 1) + x + 1, y * (SQ + 1) + x + 1, (y + 1) * (SQ + 1) + x, y * (SQ + 1) + x);
                    yard[i] = (c.yard[k] & 3 != 0) as u32 + yard[up] + yard[left] - yard[diag];
                    works[i] = (c.building[k] != 0 || matches!(c.urban[k], URBAN_WALL | URBAN_TOWER)) as u32 + works[up] + works[left] - works[diag];
                }
            }
        }
        Windows { feature, counted: 0, at: vec![0; SQ * SQ], cover: vec![0; (SQ + 1) * (SQ + 1)], wet, blds: vec![u8::MAX; n * n], yard, works }
    }

    fn sum(t: &[u32], x: usize, y: usize) -> u32 {
        let w = Self::W;
        t[(y + w) * (SQ + 1) + x + w] + t[y * (SQ + 1) + x] - t[y * (SQ + 1) + x + w] - t[(y + w) * (SQ + 1) + x]
    }

    /// The first thin window (by row, then column) that `skip` passes over.
    fn first(&mut self, c: &Chunk, skip: impl Fn((usize, usize)) -> bool) -> Option<(usize, usize)> {
        let w = Self::W;
        if c.objects.len() > self.counted {
            for o in c.objects[self.counted..].iter().filter(|o| if self.feature { c.info(o).feature } else { c.info(o).cover >= 1 }) {
                // (One centred off the chunk, past its top or left edge, is not in it.)
                let (x, y) = (o.x as usize, o.y as usize);
                if o.x >= 0.0 && o.y >= 0.0 && x < SQ && y < SQ {
                    self.at[y * SQ + x] += 1;
                }
            }
            self.counted = c.objects.len();
            for y in 0..SQ {
                for x in 0..SQ {
                    let (i, up, left, diag) = ((y + 1) * (SQ + 1) + x + 1, y * (SQ + 1) + x + 1, (y + 1) * (SQ + 1) + x, y * (SQ + 1) + x);
                    self.cover[i] = self.at[y * SQ + x] + self.cover[up] + self.cover[left] - self.cover[diag];
                }
            }
        }
        let need = if self.feature { 1 } else { 3 };
        let n = SQ - w + 1;
        for wy in 0..n {
            for wx in 0..n {
                let have = Self::sum(&self.cover, wx, wy) as usize;
                let wet = Self::sum(&self.wet, wx, wy) as usize;
                if wet * 2 >= w * w || have >= need || skip((wx, wy)) {
                    continue;
                }
                // (A castle's yard holding its curtains, towers or buildings: `works_cover`.)
                if !self.yard.is_empty() && Self::sum(&self.works, wx, wy) > 0 && Self::sum(&self.yard, wx, wy) as usize * 2 >= w * w - wet {
                    continue;
                }
                if !self.feature {
                    let q = wy * n + wx;
                    if self.blds[q] == u8::MAX {
                        let mut seen: Vec<u16> = Vec::new();
                        'scan: for y in wy..wy + w {
                            for x in wx..wx + w {
                                let b = c.building[y * SQ + x];
                                if b != 0 && !seen.contains(&b) {
                                    seen.push(b);
                                    if seen.len() >= need {
                                        break 'scan;
                                    }
                                }
                            }
                        }
                        self.blds[q] = seen.len() as u8;
                    }
                    if have + self.blds[q] as usize >= need {
                        continue;
                    }
                }
                return Some((wx, wy));
            }
        }
        None
    }
}

/// `shaped`: empty for ground as generated (one stream for the chunk); else every draw is keyed
/// by its own cell (global), and squares marked (on a plateau) get no knoll or built feature.
/// `park`: the parks' squares (1 a common's, 2 a formal garden's; empty: none).
#[allow(clippy::too_many_arguments)]
pub(super) fn enforce(c: &mut Chunk, biome: &[Biome], seed: u64, gsx: i64, gsy: i64, sea: f64, shaped: &[bool], keep_off: &[bool], lodge: &[bool], park: &[u8], bailey: &[u8], tier: crate::t0::settle::Tier) {
    let mut chunk_rng = Pcg32::new(hash3(seed, gsx, gsy, 0xe4f), 9);
    let keyed = !shaped.is_empty();
    let cell_rng = |phase: i64, x: i64, y: i64| Pcg32::new(hash3(seed, x, y, 0xe4f ^ (phase << 16)), 9);
    // What goes down for cover, and as a feature: a park's own, the town's, else the biome's.
    let in_park = |k: usize| park.get(k).copied().unwrap_or(0) & 3;
    let mut room: Option<Vec<u8>> = None;
    // (A castle's bailey: `yard_kind`.)
    let yard = |k: usize| bailey.get(k).copied().unwrap_or(0) & 3;
    let yard_major = |cells: &[usize]| !bailey.is_empty() && cells.iter().filter(|&&k| yard(k) != 0).count() * 2 > cells.len();
    let cover_kind = |c: &Chunk, k: usize, rng: &mut Pcg32| match in_park(k) {
        _ if yard(k) != 0 => yard_kind(c, k, yard(k) == 2, biome[k], false, rng),
        0 if c.urban[k] == URBAN_FIELD => field_cover(c, k, biome[k], false, rng),
        0 if green(c, k) => green_cover(biome[k], false, rng),
        0 if !matches!(c.urban[k], 0 | URBAN_PARK) => town_cover(c.urban[k], rng),
        // (Tundra's grass patches: no drifts, those keep to its snow.)
        0 if biome[k] == Biome::Tundra && c.surface[k] == Surface::TundraGrass => [Kind::Boulder, Kind::Bush, Kind::RockPile][rng.below(3) as usize],
        // (A beach: rocks, no plants.)
        0 if beach_sand(c, k, biome[k]) => [Kind::Boulder, Kind::RockPile][rng.below(2) as usize],
        0 => fallback_cover(biome[k], rng),
        p => park_cover(p & 3 == 2, biome[k], false, rng),
    };
    let feature_kind = |c: &Chunk, k: usize, rng: &mut Pcg32| match in_park(k) {
        _ if yard(k) != 0 => yard_kind(c, k, yard(k) == 2, biome[k], true, rng),
        0 if c.urban[k] == URBAN_FIELD => field_cover(c, k, biome[k], true, rng),
        0 if green(c, k) => green_cover(biome[k], true, rng),
        0 if !matches!(c.urban[k], 0 | URBAN_PARK) => town_cover(c.urban[k], rng),
        0 => fallback_feature(biome[k], rng),
        p => park_cover(p & 3 == 2, biome[k], true, rng),
    };

    // Elevation: every 16x16 cell spans at least two tiers.
    let mut deferred = false;
    for cy in 0..SQ / TIER_CELL {
        for cx in 0..SQ / TIER_CELL {
            let cells: Vec<usize> = (0..TIER_CELL * TIER_CELL).map(|q| (cy * TIER_CELL + q / TIER_CELL) * SQ + cx * TIER_CELL + q % TIER_CELL).collect();
            if cells.iter().all(|&k| !dry(c, k)) {
                continue;
            }
            // Tiers of the ground you can stand on (a shallows bed doesn't count).
            let t0 = c.tier[*cells.iter().find(|&&k| dry(c, k)).unwrap()];
            if cells.iter().any(|&k| dry(c, k) && c.tier[k] != t0) {
                continue;
            }
            // Mostly open plaza paving, or a park's: left to the windows below (a knoll or a feature
            // where a window is flat, not one to every cell).
            let dry_n = cells.iter().filter(|&&k| dry(c, k)).count();
            if cells.iter().filter(|&&k| dry(c, k) && (c.urban[k] == URBAN_PLAZA || in_park(k) != 0)).count() * 2 >= dry_n {
                deferred = true;
                continue;
            }
            if keyed && cells.iter().any(|&k| shaped[k]) {
                continue;
            }
            let mut own;
            let rng = if keyed {
                own = cell_rng(1, gsx / TIER_CELL as i64 + cx as i64, gsy / TIER_CELL as i64 + cy as i64);
                &mut own
            } else {
                &mut chunk_rng
            };
            raise(c, &cells, rng, gsx, gsy, sea, shaped, park, bailey, &mut room, tier);
        }
    }
    // Plaza paving answers to the guarantee itself rather than cell by cell: every 32x32 window
    // spans two tiers. A square's own features (laid out sparingly in a pattern on a city's
    // square: `plaza_features`) usually see to that; a window still flat gets one in its middle,
    // or anywhere in it if that fails.
    // (In a park, the window is any flat one, by lot, and its middle tried is half again as wide:
    // knolls taken from the first window on, row by row, stood in rows. A window where nothing could
    // be raised doesn't count against the 48.)
    let mut tried: Vec<(usize, usize)> = Vec::new();
    let (mut raised, mut tries) = (0, 0);
    while deferred && raised < 48 && tries < 96 {
        tries += 1;
        let open: Vec<(usize, usize)> = flat_windows(c).into_iter().filter(|w| tried.iter().filter(|t| *t == w).count() < 2).collect();
        let Some(&first) = open.first() else { break };
        let in_park = |w: &(usize, usize)| (0..32 * 32).filter(|&q| in_park((w.1 + q / 32) * SQ + w.0 + q % 32) != 0).count() * 2 >= 32 * 32;
        // (By each window's own lot, keyed by where it lies: a mound shaped elsewhere moves no choice.)
        let lot = |w: &(usize, usize)| hash3(seed, gsx + w.0 as i64, gsy + w.1 as i64, 0x9a7 + raised as i64);
        let (wx, wy) = if in_park(&first) { *open.iter().min_by_key(|w| (lot(w), **w)).expect("a window") } else { first };
        let span = match (tried.contains(&(wx, wy)), in_park(&(wx, wy))) {
            (true, _) => (0, 32),
            (false, true) => (4, 24),
            (false, false) => (8, 16),
        };
        tried.push((wx, wy));
        let cells: Vec<usize> = (0..span.1 * span.1).map(|q| (wy + span.0 + q / span.1) * SQ + wx + span.0 + q % span.1).collect();
        let mut own;
        let rng = if keyed {
            own = cell_rng(2 + span.0 as i64, gsx + wx as i64, gsy + wy as i64);
            &mut own
        } else {
            &mut chunk_rng
        };
        raised += raise(c, &cells, rng, gsx, gsy, sea, shaped, park, bailey, &mut room, tier) as usize;
    }


    // Cover: every 8x8 cell has at least one cover object (but open squares and fields: there the
    // windows and the gap below see to it, so crates aren't strewn one to a cell over the paving, nor
    // bushes over a field).
    for cy in 0..SQ / COVER_CELL {
        for cx in 0..SQ / COVER_CELL {
            let (x0, y0) = ((cx * COVER_CELL) as f32, (cy * COVER_CELL) as f32);
            let inside = |o: &Object| o.x >= x0 && o.x < x0 + COVER_CELL as f32 && o.y >= y0 && o.y < y0 + COVER_CELL as f32;
            let square = |q: usize| (cy * COVER_CELL + q / COVER_CELL) * SQ + cx * COVER_CELL + q % COVER_CELL;
            // Building walls are cover too.
            if c.objects.iter().any(|o| is_cover(o) && inside(o)) || (0..COVER_CELL * COVER_CELL).any(|q| c.building[square(q)] != 0) {
                continue;
            }
            let all: Vec<usize> = (0..COVER_CELL * COVER_CELL).map(square).filter(|&k| dry(c, k)).collect();
            if all.iter().filter(|&&k| matches!(c.urban[k], URBAN_PLAZA | URBAN_FIELD)).count() * 2 > all.len() {
                continue;
            }
            let cells = off_road_first(c, all, keep_off);
            if cells.is_empty() {
                continue;
            }
            let mut own;
            let rng = if keyed {
                own = cell_rng(4, gsx / COVER_CELL as i64 + cx as i64, gsy / COVER_CELL as i64 + cy as i64);
                &mut own
            } else {
                &mut chunk_rng
            };
            place_in(c, &cells, rng, &cover_kind);
        }
    }

    // Every 32x32 window that is mostly dry holds three cover objects or buildings: a window
    // whose cells' cover lies outside it (by water, or in a corner of each cell) gets more in its
    // middle.
    // (Only what is put down counts against the 64: a window with nowhere for anything doesn't.)
    let mut tried = vec![0u8; (SQ - 31) * (SQ - 31)];
    let mut windows = Windows::new(c, false);
    let (mut filled, mut tries) = (0, 0);
    while filled < 64 && tries < 4096 {
        tries += 1;
        let Some((wx, wy)) = windows.first(c, |w| tried[w.1 * (SQ - 31) + w.0] >= 2) else { break };
        let span = if tried[wy * (SQ - 31) + wx] > 0 { (0, 32) } else { (8, 16) };
        tried[wy * (SQ - 31) + wx] += 1;
        let all: Vec<usize> = (0..span.1 * span.1).map(|q| (wy + span.0 + q / span.1) * SQ + wx + span.0 + q % span.1).filter(|&k| dry(c, k) && c.building[k] == 0).collect();
        if all.is_empty() {
            continue;
        }
        let cells = by_bounds(c, by_fronts(c, off_road_first(c, all, keep_off)), span.1 == 32);
        if cells.is_empty() {
            continue;
        }
        let mut own;
        let rng = if keyed {
            own = cell_rng(7 + span.0 as i64, gsx + wx as i64, gsy + wy as i64);
            &mut own
        } else {
            &mut chunk_rng
        };
        let before = c.objects.len();
        if place_in(c, &cells, rng, &cover_kind) {
            filled += 1;
            yard_group(c, before, rng, keep_off, bailey);
        }
    }

    // Max gap: no dry square farther than MAX_COVER_GAP from cover (YARD_COVER_GAP in a castle's
    // yard: `cover_gap`; a square with no room for any near it, off walls, gates and raised
    // features, is given up).
    let mut given_up: Vec<usize> = Vec::new();
    for _ in 0..160 {
        let dist = cover_distance(c);
        let far = (0..SQ * SQ).filter(|&k| dry(c, k) && dist[k] > cover_gap(c, k) && !given_up.contains(&k)).max_by_key(|&k| (dist[k] - cover_gap(c, k), std::cmp::Reverse(k)));
        match far {
            Some(k) => {
                let mut own;
                let rng = if keyed {
                    own = cell_rng(5, gsx + (k % SQ) as i64, gsy + (k / SQ) as i64);
                    &mut own
                } else {
                    &mut chunk_rng
                };
                // On a road, put the cover on the nearest verge instead.
                let (k, edge, lodged) = if !placeable(c, k) || keep_off[k] {
                    match nearest_verge(c, k, keep_off) {
                        Some(v) => (v, false, false),
                        // (In a park's gravel round, its nearest edge: a statue in a formal garden, a
                        // bench on a common, off the path's middle.)
                        None => match gravel_edge(c, k, park) {
                            Some(v) => (v, true, false),
                            // (Mid-face on a broad cliff face, a boulder lodged there: `cliff`.)
                            None if lodge.get(k).is_some_and(|&l| l) && placeable(c, k) => (k, false, true),
                            None => {
                                given_up.push(k);
                                continue;
                            }
                        },
                    }
                } else {
                    (k, false, false)
                };
                let kind = if lodged {
                    Kind::Boulder
                } else if edge { if in_park(k) & 3 == 2 { Kind::GardenStatue } else { Kind::Bench } } else { cover_kind(c, k, rng) };
                let before = c.objects.len();
                if !push_at(c, kind, k, rng) {
                    given_up.push(k);
                } else {
                    yard_group(c, before, rng, keep_off, bailey);
                }
            }
            None => break,
        }
    }

    // Something to interact with: hazard/prop per 16x16 cell unless the chunk has atmosphere.
    if c.atmosphere == Atmosphere::None {
        for cy in 0..SQ / TIER_CELL {
            for cx in 0..SQ / TIER_CELL {
                let (x0, y0) = ((cx * TIER_CELL) as f32, (cy * TIER_CELL) as f32);
                let has = c.objects.iter().any(|o| {
                    info(o.kind as u16).feature && o.x >= x0 && o.x < x0 + TIER_CELL as f32 && o.y >= y0 && o.y < y0 + TIER_CELL as f32
                });
                if has {
                    continue;
                }
                let all: Vec<usize> =
                    (0..TIER_CELL * TIER_CELL).map(|q| (cy * TIER_CELL + q / TIER_CELL) * SQ + cx * TIER_CELL + q % TIER_CELL).filter(|&k| dry(c, k)).collect();
                // (Not one to every cell of a castle's yard: the windows below see to it there.)
                if yard_major(&all) {
                    continue;
                }
                let cells = off_road_first(c, all, keep_off);
                if cells.is_empty() {
                    continue;
                }
                let mut own;
                let rng = if keyed {
                    own = cell_rng(6, gsx / TIER_CELL as i64 + cx as i64, gsy / TIER_CELL as i64 + cy as i64);
                    &mut own
                } else {
                    &mut chunk_rng
                };
                place_in(c, &cells, rng, &feature_kind);
            }
        }
        // And in every mostly dry 32x32 window (whose cells' props may lie outside it).
        let mut tried = vec![0u8; (SQ - 31) * (SQ - 31)];
        let mut windows = Windows::new(c, true);
        let (mut filled, mut tries) = (0, 0);
        while filled < 64 && tries < 4096 {
            tries += 1;
            let Some((wx, wy)) = windows.first(c, |w| tried[w.1 * (SQ - 31) + w.0] >= 2) else { break };
            let span = if tried[wy * (SQ - 31) + wx] > 0 { (0, 32) } else { (8, 16) };
            tried[wy * (SQ - 31) + wx] += 1;
            let all: Vec<usize> = (0..span.1 * span.1).map(|q| (wy + span.0 + q / span.1) * SQ + wx + span.0 + q % span.1).filter(|&k| dry(c, k) && c.building[k] == 0).collect();
            // (In a castle's yard, by the buildings and the streets at first.)
            let cells = if span.0 > 0 && yard_major(&all) { by_fronts(c, off_road_first(c, all, keep_off)) } else { off_road_first(c, all, keep_off) };
            if cells.is_empty() {
                continue;
            }
            let mut own;
            let rng = if keyed {
                own = cell_rng(9 + span.0 as i64, gsx + wx as i64, gsy + wy as i64);
                &mut own
            } else {
                &mut chunk_rng
            };
            filled += place_in(c, &cells, rng, &feature_kind) as usize;
        }
    }
}

/// What the guarantee put down at `c.objects[at]` in a castle's yard, grouped: a store or two
/// beside it (barrels and crates by a cart or a stack of them, firewood by firewood, a barrel by a
/// trough), not strewn one by one.
fn yard_group(c: &mut Chunk, at: usize, rng: &mut Pcg32, keep_off: &[bool], bailey: &[u8]) {
    let Some(o) = c.objects.get(at) else { return };
    let (x, y) = (o.x as i64, o.y as i64);
    let yard = |k: usize| bailey.get(k).copied().unwrap_or(0) & 3 != 0;
    if x < 0 || y < 0 || x >= SQ as i64 || y >= SQ as i64 || !yard(y as usize * SQ + x as usize) {
        return;
    }
    let with: &[Kind] = match o.kind {
        Kind::Cart => &[Kind::Barrel, Kind::Crate],
        Kind::Crate | Kind::Barrel => &[Kind::Crate, Kind::Barrel],
        Kind::Firewood => &[Kind::Firewood],
        Kind::WaterTrough => &[Kind::Barrel],
        Kind::Haystack => &[Kind::Firewood],
        _ => &[],
    };
    let mut n = 0;
    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
        if n >= with.len() {
            break;
        }
        let (i, j) = (x + dx, y + dy);
        if i < 0 || j < 0 || i >= SQ as i64 || j >= SQ as i64 {
            continue;
        }
        let k = j as usize * SQ + i as usize;
        let busy = c.objects.iter().any(|q| q.x as i64 == i && q.y as i64 == j);
        if yard(k) && placeable(c, k) && !keep_off[k] && !busy && push_at(c, with[n], k, rng) {
            n += 1;
        }
    }
}

/// Something (`kind_of` the square) put down on one of `cells`, from a random one on, the first
/// where it fits off the buildings (at most 16 tried). Whether it did.
fn place_in(c: &mut Chunk, cells: &[usize], rng: &mut Pcg32, kind_of: &dyn Fn(&Chunk, usize, &mut Pcg32) -> Kind) -> bool {
    if cells.is_empty() {
        return false;
    }
    let start = rng.below(cells.len() as u32) as usize;
    for q in 0..cells.len().min(16) {
        let k = cells[(start + q) % cells.len()];
        let kind = kind_of(c, k, rng);
        if push_at(c, kind, k, rng) {
            return true;
        }
    }
    false
}

/// Make a flat stretch (`cells`) span two tiers: a natural knoll on open ground, a round built
/// feature on paving and in a formal garden (a raised planter or a basin there), at a dry spot far
/// enough inside the chunk that it is never clipped at the chunk border (chunks are generated
/// independently).
/// Squares marked in `shaped` (a plateau) are left alone; `park` as `enforce` has it; `room` the
/// lawns' room (`lawn_room`) while nothing has been raised since it was worked out.
/// Whether anything was raised.
#[allow(clippy::too_many_arguments)]
/// A horse park's squares in the parks' marks (`enforce`'s `park`; R5 towns, 114).
pub(super) const HORSE_PARK_MARK: u8 = 32;

fn raise(c: &mut Chunk, cells: &[usize], rng: &mut Pcg32, gsx: i64, gsy: i64, sea: f64, shaped: &[bool], park: &[u8], bailey: &[u8], room: &mut Option<Vec<u8>>, tier: crate::t0::settle::Tier) -> bool {
    let raised = raise_on(c, cells, rng, gsx, gsy, sea, shaped, park, bailey, room, tier);
    if raised {
        *room = None;
    }
    raised
}

#[allow(clippy::too_many_arguments)]
fn raise_on(c: &mut Chunk, cells: &[usize], rng: &mut Pcg32, gsx: i64, gsy: i64, sea: f64, shaped: &[bool], park: &[u8], bailey: &[u8], room_cache: &mut Option<Vec<u8>>, tier: crate::t0::settle::Tier) -> bool {
    // (Never on a motte: it is its own rise. `bailey` bit 4.)
    let interior = |k: usize| {
        let (x, y) = (k % SQ, k / SQ);
        x >= 7 && y >= 7 && x < SQ - 7 && y < SQ - 7 && !shaped.get(k).is_some_and(|&s| s) && !bailey.get(k).is_some_and(|&b| b & 4 != 0)
    };
    let yard = |k: usize| bailey.get(k).copied().unwrap_or(0) & 3;
    let garden = |k: usize| park.get(k).is_some_and(|&p| p & 3 == 2);
    // A lawn's (a green's, a close's, a park's) knoll is a gentle hillock, broad where there is room,
    // all of it on the lawn (no house or street lifted, off the water's edge and a park's pond's or
    // stream's banks, inside the chunk).
    let lawn = |c: &Chunk, k: usize| matches!(c.urban[k], URBAN_PARK | URBAN_PATH);
    let room = if cells.iter().any(|&k| lawn(c, k)) { room_cache.get_or_insert_with(|| lawn_room(c, park)).clone() } else { Vec::new() };
    // Open country, fields and lawns take a natural knoll; streets, plazas and formal gardens a
    // built feature.
    // (Off a park's lawn, a knoll or feature stays 8 squares away from it: its slopes would rise on
    // the lawn and its paths. `park` bit 16.)
    let by_park = |k: usize| park.get(k).is_some_and(|&p| p & 19 != 0);
    let good: Vec<usize> = cells
        .iter()
        .copied()
        .filter(|&k| placeable(c, k) && interior(k) && matches!(c.urban[k], 0 | URBAN_FIELD | URBAN_PARK) && !garden(k) && (c.urban[k] != URBAN_PARK || room[k] >= 7))
        .filter(|&k| c.urban[k] == URBAN_PARK || !by_park(k))
        .collect();
    let town: Vec<usize> = cells.iter().copied().filter(|&k| placeable(c, k) && interior(k) && (lawn(c, k) || !by_park(k))).collect();
    // Last resort: any dry square inside the chunk off the shaping (never one in the water: lifted,
    // it stood up as a spike; never near the border: the knoll would stop at the seam).
    let last: Vec<usize> = cells.iter().copied().filter(|&k| dry(c, k) && interior(k) && (lawn(c, k) || !by_park(k))).collect();
    // Paved ground (streets, plazas) gets a round built feature (a dais, a fountain, a
    // planter, a market cross) clear of stalls and props, rather than a knoll.
    if good.is_empty() && !town.is_empty() {
        // (R5 towns, 114: a horse park's trodden ground an auction block, a dais, not a knoll.)
        let hs: Vec<bool> = if park.iter().any(|&p| p & HORSE_PARK_MARK != 0) { park.iter().enumerate().map(|(k, &p)| p & HORSE_PARK_MARK != 0 && c.urban[k] == URBAN_YARD).collect() } else { Vec::new() };
        let horses = |k: usize| hs.get(k).is_some_and(|&h| h);
        let free = |c: &Chunk, k: usize| placeable(c, k) && interior(k) && (matches!(c.urban[k], URBAN_STREET | URBAN_PLAZA) || (garden(k) && c.urban[k] == URBAN_PARK) || horses(k));
        let kind = |k: usize, r: f64, rng: &mut Pcg32| {
            // (A castle's yard: a raised planter, in a garden yard now and then a fountain.)
            if horses(k) {
                PlazaFeature::Dais
            } else if yard(k) != 0 {
                if yard(k) == 2 && r >= 1.5 && rng.next_f64() < 0.3 { PlazaFeature::Fountain } else { PlazaFeature::Planter }
            } else if garden(k) {
                if r < 1.5 || rng.next_f64() < 0.7 { PlazaFeature::Planter } else { PlazaFeature::Fountain }
            } else if r < 1.5 {
                if rng.next_f64() < 0.5 { PlazaFeature::Cross } else { PlazaFeature::Dais }
            } else {
                [PlazaFeature::Dais, PlazaFeature::Fountain, PlazaFeature::Planter, PlazaFeature::Cross][rng.below(4) as usize]
            }
        };
        if built_feature(c, &town, rng, sea, tier, &free, &kind) {
            return true;
        }
    }
    let pool: &[usize] = if !good.is_empty() {
        &good
    } else if !town.is_empty() {
        &town
    } else if !last.is_empty() {
        &last
    } else {
        return false;
    };
    let mut spot = pool[rng.below(pool.len() as u32) as usize];
    // (On a lawn with no room for a whole knoll, the square with the most room round it.)
    if lawn(c, spot) && room[spot] < 7 {
        let most = pool.iter().filter(|&&k| lawn(c, k)).map(|&k| room[k]).max().unwrap_or(0);
        let best: Vec<usize> = pool.iter().copied().filter(|&k| lawn(c, k) && room[k] == most).collect();
        spot = best[rng.below(best.len() as u32) as usize];
        // (None but by the water's edge, a house or a road: no knoll here, the window round it tried
        // whole instead. In a park, a knoll cut off at its edge would rise in a step there: one needs
        // room for most of its slopes, else on its lawn a rocky outcrop with a boulder (a common's, where
        // one fits) or a raised planter with a tree.)
        let in_park = |k: usize| park.get(k).is_some_and(|&p| p & 3 != 0);
        if in_park(spot) && most < 6 {
            // (Its own park's lawn: a formal garden's beside a common's is another's.)
            let kind = park[spot] & 3;
            let lawns: Vec<usize> = pool.iter().copied().filter(|&k| park[k] & 3 == kind && c.urban[k] == URBAN_PARK).collect();
            let free = |c: &Chunk, k: usize| placeable(c, k) && interior(k) && c.urban[k] == URBAN_PARK && park[k] & 3 == kind && park[k] & 4 == 0;
            if lawns.is_empty() {
                return false;
            }
            return (!garden(spot) && outcrop(c, &lawns, rng, sea, &free)) || built_feature(c, &lawns, rng, sea, tier, &free, &|_, _, _| PlazaFeature::Planter);
        }
        if most < 2 {
            return false;
        }
    }
    let (sx, sy) = ((spot % SQ) as f64 + 0.5, (spot / SQ) as f64 + 0.5);
    if lawn(c, spot) {
        // (Its reach, `r × elong × swell`, inside the room round it: 8.6 squares broad, 6 small; with
        // no room even for that, it is kept to the lawn's own squares.)
        let broad = room.get(spot).is_some_and(|&r| r >= 10);
        let f = Relief {
            cx: sx + gsx as f64,
            cy: sy + gsy as f64,
            r: if broad { rng.range(4.5, 6.0) } else { rng.range(3.5, 4.2) },
            h: TIER_FT * if broad { rng.range(1.25, 1.45) } else { rng.range(1.1, 1.2) },
            steep: rng.range(0.0, 0.1),
            dir: {
                let a = rng.range(0.0, std::f64::consts::TAU);
                (libm::cos(a), libm::sin(a))
            },
            elong: rng.range(1.0, 1.2),
            wobble: [(rng.range(0.0, 0.1), rng.range(0.0, 6.28)), (rng.range(0.0, 0.06), rng.range(0.0, 6.28)), (0.0, 0.0)],
        };
        // (A park's dry lawn rises whatever the water's level below it, as `lawn_room` has it.)
        let water: Vec<f32> = (0..SQ * SQ)
            .map(|k| if park.get(k).is_some_and(|&p| p & 3 != 0) && c.water_level[k] <= c.height[k] { c.water_level[k].min(c.height[k] - 1.5) } else { c.water_level[k] })
            .collect();
        if room.get(spot).is_some_and(|&r| r >= 7) {
            apply_relief(&f, &mut c.height, &water, gsx, gsy, SQ);
        } else {
            let before = c.height.clone();
            apply_relief(&f, &mut c.height, &water, gsx, gsy, SQ);
            for k in 0..SQ * SQ {
                if !matches!(c.urban[k], URBAN_PARK | URBAN_PATH) || c.building[k] != 0 || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 {
                    c.height[k] = before[k];
                }
            }
        }
        c.height[spot] = c.height[spot].max(((c.tier[spot] as f64 + 1.2) * TIER_FT + sea) as f32);
        retier(c, sea);
        return true;
    }
    // (A rounded knoll, a tier and a little more: never a flat-topped mesa. Its reach, `r × elong ×
    // swell`, under 7 squares: inside the margin it stands off the chunk's edge.)
    let rise = TIER_FT * rng.range(1.2, 1.5);
    let f = Relief {
        cx: sx + gsx as f64,
        cy: sy + gsy as f64,
        r: rng.range(3.0, 3.6),
        h: rise,
        steep: rng.range(0.1, 0.4),
        dir: {
            let a = rng.range(0.0, std::f64::consts::TAU);
            (libm::cos(a), libm::sin(a))
        },
        elong: rng.range(1.0, 1.4),
        wobble: [(rng.range(0.0, 0.2), rng.range(0.0, 6.28)), (rng.range(0.0, 0.12), rng.range(0.0, 6.28)), (0.0, 0.0)],
    };
    apply_relief(&f, &mut c.height, &c.water_level, gsx, gsy, SQ);
    // Guarantee even if the relief was clipped: lift the spot itself a full tier.
    c.height[spot] = c.height[spot].max(((c.tier[spot] as f64 + 1.2) * TIER_FT + sea) as f32);
    retier(c, sea);
    true
}

/// A round built feature raised a tier (`kind` by the square and its radius in squares) on one of
/// `cands`, the largest round that fits (2.3, 1.5, 1 or half a square) on `free` squares of the first
/// one's tier clear of what stands there (a fountain or a dais within `tier`'s band); a planter with a tree in it. Whether one was.
fn built_feature(c: &mut Chunk, cands: &[usize], rng: &mut Pcg32, sea: f64, tier: crate::t0::settle::Tier, free: &dyn Fn(&Chunk, usize) -> bool, kind: &dyn Fn(usize, f64, &mut Pcg32) -> PlazaFeature) -> bool {
    let mut taken = vec![false; SQ * SQ];
    for o in &c.objects {
        let r = info(o.kind as u16).radius * o.scale + 0.5;
        for y in (o.y - r).max(0.0) as usize..=((o.y + r) as usize).min(SQ - 1) {
            for x in (o.x - r).max(0.0) as usize..=((o.x + r) as usize).min(SQ - 1) {
                taken[y * SQ + x] = true;
            }
        }
    }
    let base = c.tier[cands[0]];
    let fits = |k: usize| free(c, k) && !taken[k] && c.tier[k] == base;
    // Squares (offsets) within a round of radius r squares about a square's centre.
    let disc = |r: f64| -> Vec<(i64, i64)> {
        let n = r.floor() as i64;
        (-n..=n).flat_map(|dy| (-n..=n).map(move |dx| (dx, dy))).filter(|&(dx, dy)| ((dx * dx + dy * dy) as f64) <= r * r).collect()
    };
    let mut placed = None;
    'size: for r in [2.3, 1.5, 1.0, 0.5] {
        let offs = disc(r);
        let start = rng.below(cands.len() as u32) as usize;
        for q in 0..cands.len() {
            let k = cands[(start + q) % cands.len()];
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            let ok = offs.iter().all(|&(dx, dy)| {
                let (i, j) = (x + dx, y + dy);
                i >= 0 && j >= 0 && (i as usize) < SQ && (j as usize) < SQ && fits(j as usize * SQ + i as usize)
            });
            if ok {
                placed = Some((k, r, offs));
                break 'size;
            }
        }
    }
    let Some((k, r, offs)) = placed else { return false };
    let top = ((base as f64 + 1.2) * TIER_FT + sea) as f32;
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    let mut kind = kind(k, r, rng);
    // (A fountain or a dais keeps to its tier's band, by the plaza's: drawn across no more than its
    // band's top, so the round must cover the raised squares' centres within it, else a planter. A
    // planter or a cross's plinth the same, but shrunk to a smaller round rather than changed.)
    let mut r = r;
    let mut offs = offs;
    let mut rv = r as f32 + 0.45;
    if matches!(kind, PlazaFeature::Fountain | PlazaFeature::Dais) {
        let max = (plaza::across_kind(tier, kind).1 / (2.0 * SQUARE_FT)) as f32;
        if max >= r as f32 + 0.2 {
            rv = rv.min(max);
        } else {
            kind = PlazaFeature::Planter;
        }
    }
    if matches!(kind, PlazaFeature::Planter | PlazaFeature::Cross) {
        let max = (plaza::across_kind(tier, kind).1 / (2.0 * SQUARE_FT)) as f32;
        // (A planter's round reaches its raised squares' centres with 0.05 to spare, a cross's 0.2.)
        let lip = if kind == PlazaFeature::Planter { 0.05 } else { 0.2 };
        if max < r as f32 + lip {
            r = [1.5, 1.0, 0.5].into_iter().filter(|&q| q <= r).find(|&q| max >= q as f32 + lip).unwrap_or(0.5);
            let n = r.floor() as i64;
            offs = (-n..=n).flat_map(|dy| (-n..=n).map(move |dx| (dx, dy))).filter(|&(dx, dy)| ((dx * dx + dy * dy) as f64) <= r * r).collect();
            rv = (r as f32 + 0.45).min(max).max(r as f32 + 0.05);
        }
    }
    // (A planter's bed must clear the tree's crown by 3 ft: a round
    // too small for that is no planter, the tree stands bare.)
    if kind == PlazaFeature::Planter {
        rv = rv.min((plaza::across_kind(tier, kind).1 / (2.0 * SQUARE_FT)) as f32);
        if f64::from(rv) * 2.0 * SQUARE_FT < planter_crown_ft() + 3.0 {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            c.objects.push(Object { kind: Kind::TreeDeciduous, variant: rng.below(8) as u8, x: cx, y: cy, rot: 0.0, scale: PLANTER_TREE_SCALE, sprite: 0, turned: false });
            return true;
        }
    }
    for &(dx, dy) in &offs {
        c.height[(y + dy) as usize * SQ + (x + dx) as usize] = top;
    }
    let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
    let ang = rng.range(0.0, std::f64::consts::TAU);
    c.structures.push(VectorShape { kind: ShapeKind::Dais, size: kind as u8 as f32, pts: vec![[cx, cy], [cx + rv * libm::cos(ang) as f32, cy + rv * libm::sin(ang) as f32]] });
    if kind == PlazaFeature::Planter {
        c.objects.push(Object { kind: Kind::TreeDeciduous, variant: rng.below(8) as u8, x: cx, y: cy, rot: 0.0, scale: PLANTER_TREE_SCALE, sprite: 0, turned: false });
    }
    retier(c, sea);
    true
}

/// A rocky outcrop on a common's lawn, on one of `cands` where its round (1.5, 1 or half a square,
/// eased out a square and a half more, a little out of round) lies on `free` squares a square or
/// more off the gravel: the ground rising in a dome to a tier over its foot, bare rock within half a square of its
/// round's edge, a boulder on its top and two or three rock piles about it; nothing else that blocks
/// the way within two squares of its round. Away from another (12 squares) where it can be. With no
/// room for one, a lone boulder on a knob of rock a square across. Whether one was.
fn outcrop(c: &mut Chunk, cands: &[usize], rng: &mut Pcg32, sea: f64, free: &dyn Fn(&Chunk, usize) -> bool) -> bool {
    let off_gravel = |c: &Chunk, k: usize| {
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        (-1..=1).all(|dy| (-1..=1).all(|dx| {
            let (i, j) = (x + dx, y + dy);
            !(i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64) || c.urban[j as usize * SQ + i as usize] != URBAN_PATH
        }))
    };
    let lone = |c: &Chunk, k: usize| {
        let (x, y) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
        !c.objects.iter().any(|o| o.kind == Kind::Boulder && o.scale > 1.0 && (o.x - x) * (o.x - x) + (o.y - y) * (o.y - y) < 144.0)
    };
    let base = c.tier[cands[0]];
    // (Its round on one tier; out to all the dome reaches and a square more (the most its outline
    // swells), the park's lawn: no street, square, quay, gravel, water or bank, and nothing standing
    // where its boulder goes.)
    let fits = |c: &Chunk, k: usize, r: f64, reach: f64| {
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        let out = reach * 1.25 + 1.0;
        let n = crate::core::ceil(out) as i64;
        let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
        let bare = c.objects.iter().all(|o| {
            let gap = (r + 0.6) as f32 + info(o.kind as u16).radius * o.scale;
            (o.x - cx) * (o.x - cx) + (o.y - cy) * (o.y - cy) >= gap * gap
        });
        bare && (-n..=n).all(|dy| (-n..=n).all(|dx| {
            let (i, j) = (x + dx, y + dy);
            let d2 = (dx * dx + dy * dy) as f64;
            d2 > out * out
                || (i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                    let q = j as usize * SQ + i as usize;
                    free(c, q) && off_gravel(c, q) && (d2 > r * r || c.tier[q] == base)
                })
        }))
    };
    // (Its boulder blocks the way: nothing else that does stands within two squares of its round, so
    // no square is shut in between them.)
    let clear = |c: &Chunk, k: usize, r: f64| {
        let (x, y) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
        c.objects.iter().all(|o| {
            let reach = (r + 2.0) as f32 + info(o.kind as u16).radius * o.scale;
            !info(o.kind as u16).blocks_move || (o.x - x) * (o.x - x) + (o.y - y) * (o.y - y) >= reach * reach
        })
    };
    let start = rng.below(cands.len() as u32) as usize;
    let mut placed = None;
    // (Room for its round and a broad ease round it first, apart from another; else a narrower ease.
    // Their sizes rolled, so a row of them along a strip of lawn isn't one size over and over.)
    let sizes = [[1.5, 1.0, 0.5], [1.0, 1.5, 0.5], [1.0, 0.5, 1.5]][rng.below(3) as usize];
    'look: for (spaced, ease) in [(true, 1.5), (false, 1.5), (false, 0.5)] {
        for r in sizes {
            for q in 0..cands.len() {
                let k = cands[(start + q) % cands.len()];
                if (!spaced || lone(c, k)) && fits(c, k, r, r + ease) && clear(c, k, r) {
                    placed = Some((k, r, ease));
                    break 'look;
                }
            }
        }
    }
    let Some((k, r, ease)) = placed else {
        // (No room for one: a lone boulder on a knob of rock a square across, the knob hidden under it,
        // where the lawn runs three squares round it (and no gravel a square past that) and nothing
        // stands within a square and a half.)
        let knob = (0..cands.len()).map(|q| cands[(start + q) % cands.len()]).find(|&k| {
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            (-3..=3).all(|dy| (-3..=3).all(|dx| {
                let (i, j) = (x + dx, y + dy);
                i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                    let q = j as usize * SQ + i as usize;
                    free(c, q) && off_gravel(c, q)
                }
            })) && clear(c, k, 0.0)
                && c.objects.iter().all(|o| {
                    let gap = 1.5 + info(o.kind as u16).radius * o.scale;
                    (o.x - cx) * (o.x - cx) + (o.y - cy) * (o.y - cy) >= gap * gap
                })
        });
        let Some(k) = knob else { return false };
        c.height[k] = c.height[k].max(((c.tier[k] as f64 + 1.2) * TIER_FT + sea) as f32);
        c.surface[k] = Surface::Rock;
        let (cx, cy) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
        c.objects.push(Object { kind: Kind::Boulder, variant: rng.below(8) as u8, x: cx, y: cy, rot: rng.range(0.0, std::f64::consts::TAU) as f32, scale: rng.range(1.35, 1.5) as f32, sprite: 0, turned: false });
        retier(c, sea);
        return true;
    };
    let (x, y) = ((k % SQ) as f64 + 0.5, (k / SQ) as f64 + 0.5);
    let top = ((base as f64 + 1.2) * TIER_FT + sea) as f32;
    let (a, b) = (rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU));
    let reach = r + ease;
    let n = crate::core::ceil(reach * 1.25) as i64;
    for dy in -n..=n {
        for dx in -n..=n {
            let (i, j) = (k as i64 % SQ as i64 + dx, k as i64 / SQ as i64 + dy);
            if i < 0 || j < 0 || i >= SQ as i64 || j >= SQ as i64 {
                continue;
            }
            let q = j as usize * SQ + i as usize;
            let (px, py) = (i as f64 + 0.5 - x, j as f64 + 0.5 - y);
            let d = crate::core::sqrt(px * px + py * py);
            let th = libm::atan2(py, px);
            let wob = 1.0 + 0.15 * libm::sin(2.0 * th + a) + 0.1 * libm::sin(3.0 * th + b);
            let t = d / (reach * wob);
            if t >= 1.0 || !free(c, q) {
                continue;
            }
            let ease = (t * t * (3.0 - 2.0 * t)) as f32;
            c.height[q] = c.height[q].max(top - (top - c.height[q]) * ease);
            if d <= (r - 0.5) * wob {
                c.surface[q] = Surface::Rock;
            }
        }
    }
    let (cx, cy) = ((x + rng.range(-0.2, 0.2)) as f32, (y + rng.range(-0.2, 0.2)) as f32);
    c.objects.push(Object { kind: Kind::Boulder, variant: rng.below(8) as u8, x: cx, y: cy, rot: rng.range(0.0, std::f64::consts::TAU) as f32, scale: ((r + 0.6) / 0.9 * rng.range(0.9, 1.05)) as f32, sprite: 0, turned: false });
    for m in 0..2 + rng.below(2) {
        let t = a + std::f64::consts::TAU * (m as f64 + rng.range(0.0, 0.4)) / 3.0;
        let d = r + rng.range(0.1, 0.6);
        let kind = if rng.next_f64() < 0.6 { Kind::RockPile } else { Kind::RockSmall };
        let (ox, oy) = (x + d * libm::cos(t), y + d * libm::sin(t));
        if ox > 0.5 && oy > 0.5 && ox < SQ as f64 - 0.5 && oy < SQ as f64 - 0.5 {
            c.objects.push(Object { kind, variant: rng.below(4) as u8, x: ox as f32, y: oy as f32, rot: rng.range(0.0, std::f64::consts::TAU) as f32, scale: rng.range(0.6, 0.9) as f32, sprite: 0, turned: false });
        }
    }
    retier(c, sea);
    true
}
