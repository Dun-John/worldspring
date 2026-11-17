//! Cover chosen per ground, town props, and the cover-gap checks.

use super::*;

/// The nearest square (within 8) at a park's gravel edge (`park` bit 8) that is off the buildings.
pub(super) fn gravel_edge(c: &Chunk, k: usize, park: &[u8]) -> Option<usize> {
    if park.is_empty() {
        return None;
    }
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    for r in 0..=8i64 {
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (x + dx, y + dy);
                if (dx.abs() != r && dy.abs() != r) || nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                    continue;
                }
                let nk = ny as usize * SQ + nx as usize;
                if park[nk] & 8 != 0 && c.building[nk] == 0 && c.water_level[nk] <= c.height[nk] {
                    return Some(nk);
                }
            }
        }
    }
    None
}

/// Nearest dry, non-road square within a few squares (by ring), for roadside placements (off
/// `keep_off`).
pub(super) fn nearest_verge(c: &Chunk, k: usize, keep_off: &[bool]) -> Option<usize> {
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    // The nearest square clear of the road, else the nearest one just off it.
    let mut verge = None;
    for r in 1..=4i64 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                    continue;
                }
                let nk = ny as usize * SQ + nx as usize;
                let road_bed = c.road_halo[(nk / SQ + 1) * HS + nk % SQ + 1] != 0;
                if !road_bed && !keep_off[nk] && c.building[nk] == 0 && c.water_level[nk] <= c.height[nk] {
                    if clear_of_roads(c, nk) {
                        return Some(nk);
                    }
                    verge = verge.or(Some(nk));
                }
            }
        }
    }
    verge
}

/// What the guarantee puts down in a park for cover (or, with `feature`, to see): a formal garden's
/// topiary and statues (beds); a common's shrubs and the biome's trees (wildflowers, tall grass).
/// No benches: a bench stands by a path (the park's own, or the max gap's at a round's edge).
pub(super) fn park_cover(formal: bool, b: Biome, feature: bool, rng: &mut Pcg32) -> Kind {
    let (trees, shrubs, _, _) = greenery(b);
    let shrub = shrubs.first().copied().unwrap_or(Kind::Bush);
    let opts: Vec<Kind> = match (formal, feature) {
        (true, false) => vec![Kind::Topiary, Kind::Topiary, Kind::GardenStatue],
        (true, true) => vec![Kind::Flowerbed],
        (false, false) => vec![shrub, shrub, trees.first().copied().unwrap_or(Kind::TreeDeciduous)],
        (false, true) => vec![Kind::Flowers, Kind::TallGrass],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

/// What the guarantee sets out in a castle's bailey, as cover or (`feature`) a prop: by a building
/// (within three squares), stores (barrels, crates, firewood); out in the yard a cart, a trough or a
/// haystack (in a garden yard, one of the biome's trees planted for the haystack), as a prop a
/// trough or a cart.
pub(super) fn yard_kind(c: &Chunk, k: usize, garden: bool, b: Biome, feature: bool, rng: &mut Pcg32) -> Kind {
    // (By a wall or a tower, small stores: nothing reaching it.)
    let by_building = within(k, 3, |q| c.building[q] != 0 || matches!(c.urban[q], URBAN_WALL | URBAN_TOWER));
    let tree = greenery(b).0.first().copied().unwrap_or(Kind::TreeDeciduous);
    let opts: &[Kind] = match (by_building, garden, feature) {
        (true, _, _) => &[Kind::Barrel, Kind::Crate, Kind::Firewood],
        (false, true, false) => &[Kind::Cart, Kind::WaterTrough, tree],
        (false, false, false) => &[Kind::Cart, Kind::WaterTrough, Kind::Haystack],
        (false, _, true) => &[Kind::WaterTrough, Kind::Cart],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

/// Whether any square within `r` (each way) of square `k` is one `of`.
fn within(k: usize, r: i64, of: impl Fn(usize) -> bool) -> bool {
    let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
    (-r..=r).any(|dy| {
        (-r..=r).any(|dx| {
            let (i, j) = (x + dx, y + dy);
            i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && of(j as usize * SQ + i as usize)
        })
    })
}

/// Cover in town (streets, plazas, crofts; a lawn takes the biome's own, a field `field_cover`).
pub(super) fn town_cover(urban: u8, rng: &mut Pcg32) -> Kind {
    let opts: &[Kind] = match urban {
        URBAN_PLAZA => &[Kind::Crate, Kind::Barrel, Kind::Cart],
        URBAN_GRAVEYARD => &[Kind::Headstone],
        URBAN_RUIN => &[Kind::RuinWall, Kind::RockPile],
        URBAN_CROFT => &[Kind::Firewood, Kind::Cart, Kind::Barrel, Kind::WaterTrough],
        _ => &[Kind::Crate, Kind::Barrel, Kind::Cart],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

/// A village's green's square (`Chunk::yard` bit 16).
pub(super) fn green(c: &Chunk, k: usize) -> bool {
    c.yard.get(k).is_some_and(|&y| y & 16 != 0)
}

/// What the guarantee sets out on a village's green: for cover the biome's shrubs and now and then
/// a bench (never crates, rocks or the forest's trees); to see, wildflowers.
pub(super) fn green_cover(b: Biome, feature: bool, rng: &mut Pcg32) -> Kind {
    let shrub = greenery(b).1.first().copied().unwrap_or(Kind::Bush);
    let opts: &[Kind] = if feature { &[Kind::Flowers] } else { &[shrub, shrub, Kind::Bench] };
    opts[rng.below(opts.len() as u32) as usize]
}

/// What the guarantee sets out on a field (v63 countryside; never a haystack, `farm` places those):
/// for cover thorn bushes and a clearance cairn, on pasture now and then a lone field tree or a
/// trough; to see, wildflowers and tall grass.
pub(super) fn field_cover(c: &Chunk, k: usize, b: Biome, feature: bool, rng: &mut Pcg32) -> Kind {
    let (trees, shrubs, _, _) = greenery(b);
    let shrub = shrubs.first().copied().unwrap_or(Kind::Bush);
    let tree = trees.first().copied().unwrap_or(Kind::TreeDeciduous);
    let opts: Vec<Kind> = match (c.field.get(k).is_some_and(|&b| b >> 6 == 3), feature) {
        (true, false) => vec![shrub, shrub, shrub, tree, shrub, shrub, shrub, Kind::WaterTrough],
        (false, false) => vec![shrub, shrub, Kind::RockPile],
        (_, true) => vec![Kind::Flowers, Kind::TallGrass],
    };
    opts[rng.below(opts.len() as u32) as usize]
}

/// Street life: barrels and crates against walls (market stalls stand in blocks: `market_blocks`;
/// haystacks in stack yards and by field gates: `farm`).
/// Keyed by global square so the result does not depend on which chunk asks.
/// `rowless`: graveyard squares whose headstones are laid otherwise (empty: none).
pub(super) fn place_town_props(c: &mut Chunk, seed: u64, gsx: i64, gsy: i64, rowless: &[bool]) {
    let pseed = hash2(seed, 0x70e5, 3);
    for k in 0..SQ * SQ {
        let urb = c.urban[k];
        if urb == 0 || c.building[k] != 0 || c.water_level[k] > c.height[k] || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 {
            continue;
        }
        let (i, j) = (k % SQ, k / SQ);
        let h = hash2(pseed, gsx + i as i64, gsy + j as i64);
        let r = unit(h);
        let by_wall = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
            let (x, y) = (i as i64 + dx, j as i64 + dy);
            x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && c.building[y as usize * SQ + x as usize] != 0
        });
        let kind = match urb {
            URBAN_STREET if by_wall && r < 0.035 => Some(if (h >> 20) & 1 == 0 { Kind::Barrel } else { Kind::Crate }),
            // Headstones in rows (every other square, most of them).
            URBAN_GRAVEYARD if (gsx + i as i64) % 2 == 0 && (gsy + j as i64) % 3 == 0 && r < 0.8 && !rowless.get(k).is_some_and(|&x| x) => Some(Kind::Headstone),
            _ => None,
        };
        if let Some(kind) = kind {
            let mut rng = Pcg32::new(h, 7);
            push_at(c, kind, k, &mut rng);
        }
    }
}

/// Put a `kind` down in square `k`, unless no way of it fits off the buildings (false).
pub(super) fn push_at(c: &mut Chunk, kind: Kind, k: usize, rng: &mut Pcg32) -> bool {
    let mut o = Object {
        kind,
        variant: rng.below(8) as u8,
        x: (k % SQ) as f32 + rng.range(0.25, 0.75) as f32,
        y: (k / SQ) as f32 + rng.range(0.25, 0.75) as f32,
        rot: rng.range(0.0, std::f64::consts::TAU) as f32,
        scale: rng.range(0.85, 1.2) as f32,
        sprite: 0,
        turned: false,
    };
    // Nothing reaches over a building; tall props draw above roofs, so not past the chunk edge
    // either (where a neighbour's building may stand): try it smaller and centred, a tall one
    // else as a crate, else nothing.
    let tall = info(kind as u16).height_ft >= 5.0;
    let fits = |c: &Chunk, o: &Object| if info(o.kind as u16).height_ft >= 5.0 { clear_of_buildings(c, o) } else { off_buildings(c, o) };
    if !fits(c, &o) {
        o.x = (k % SQ) as f32 + 0.5;
        o.y = (k / SQ) as f32 + 0.5;
        o.scale = 0.85;
        if !fits(c, &o) {
            // (On a park's lawn or gravel or on a field, a bush; else a crate, or in a lane one square
            // wide, barrels.)
            let park = matches!(c.urban[k], URBAN_PARK | URBAN_PATH | URBAN_FIELD);
            let tries: &[Kind] = if park { &[Kind::Bush] } else if tall { &[Kind::Crate, Kind::Barrel] } else { &[Kind::Barrel] };
            let Some(&small) = tries.iter().find(|&&kd| kd != kind && off_buildings(c, &Object { kind: kd, ..o })) else { return false };
            o.kind = small;
        }
    }
    c.objects.push(o);
    true
}

/// Whether an object's footprint disc touches no building square of the chunk.
pub(super) fn off_buildings(c: &Chunk, o: &Object) -> bool {
    !touches_square(o, |k| c.building[k] != 0)
}

/// Whether an object's footprint disc stays inside the chunk and off building squares.
pub(super) fn clear_of_buildings(c: &Chunk, o: &Object) -> bool {
    let r = info(o.kind as u16).radius * o.scale;
    if o.x - r < 0.0 || o.y - r < 0.0 || o.x + r > SQ as f32 || o.y + r > SQ as f32 {
        return false;
    }
    for y in (o.y - r) as usize..=((o.y + r) as usize).min(SQ - 1) {
        for x in (o.x - r) as usize..=((o.x + r) as usize).min(SQ - 1) {
            // Nearest point of the square to the disc centre.
            let dx = o.x - o.x.clamp(x as f32, x as f32 + 1.0);
            let dy = o.y - o.y.clamp(y as f32, y as f32 + 1.0);
            if dx * dx + dy * dy < r * r && c.building[y * SQ + x] != 0 {
                return false;
            }
        }
    }
    true
}

/// How far square `k` may be from cover (`cover_distance`): `MAX_COVER_GAP`, in a castle's yard
/// `YARD_COVER_GAP`.
pub fn cover_gap(c: &Chunk, k: usize) -> i32 {
    if c.yard.get(k).is_some_and(|&y| y & 3 != 0) {
        YARD_COVER_GAP
    } else if c.urban[k] == URBAN_FIELD {
        FIELD_COVER_GAP
    } else {
        MAX_COVER_GAP
    }
}

/// Whether the 32x32 window at (`wx`, `wy`) is a castle's yard's (half its squares not under water
/// at least) holding its cover: a wall, a tower or a building on any of its squares.
pub fn works_cover(c: &Chunk, wx: usize, wy: usize) -> bool {
    const W: usize = 32;
    if c.yard.is_empty() {
        return false;
    }
    let (mut yard, mut wet, mut works) = (0, 0, false);
    for y in wy..wy + W {
        for x in wx..wx + W {
            let k = y * SQ + x;
            yard += (c.yard[k] & 3 != 0) as usize;
            wet += (c.water_level[k] > c.height[k]) as usize;
            works |= c.building[k] != 0 || matches!(c.urban[k], URBAN_WALL | URBAN_TOWER);
        }
    }
    works && yard * 2 >= W * W - wet
}

/// Chebyshev-ish BFS distance (squares) from every square to the nearest cover object.
pub fn cover_distance(c: &Chunk) -> Vec<i32> {
    let mut dist = vec![i32::MAX; SQ * SQ];
    let mut queue = std::collections::VecDeque::new();
    for k in 0..SQ * SQ {
        if c.building[k] != 0 || matches!(c.urban[k], URBAN_WALL | URBAN_TOWER) || c.yard.get(k).is_some_and(|&y| y & 8 != 0) {
            dist[k] = 0;
            queue.push_back(k);
        }
    }
    for o in c.objects.iter().filter(|o| c.info(o).cover >= 1) {
        let (x, y) = (o.x as i64, o.y as i64);
        if x >= 0 && y >= 0 && (x as usize) < SQ && (y as usize) < SQ {
            let k = y as usize * SQ + x as usize;
            if dist[k] != 0 {
                dist[k] = 0;
                queue.push_back(k);
            }
        }
    }
    while let Some(k) = queue.pop_front() {
        let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                continue;
            }
            let nk = ny as usize * SQ + nx as usize;
            if dist[nk] > dist[k] + 1 {
                dist[nk] = dist[k] + 1;
                queue.push_back(nk);
            }
        }
    }
    dist
}

pub(super) fn compute_edges(c: &mut Chunk) {
    for j in 0..SQ {
        for i in 0..SQ {
            let k = j * SQ + i;
            let mut e = 0u8;
            for (bit_cliff, bit_slope, nk) in [(1u8, 4u8, (i + 1 < SQ).then(|| k + 1)), (2u8, 8u8, (j + 1 < SQ).then(|| k + SQ))] {
                // Building walls are implied by the building squares, and walls, towers and
                // decks are drawn as vectors: none of them are drawn as cliffs.
                let raised = |k: usize| c.building[k] != 0 || matches!(c.urban[k], URBAN_DECK | URBAN_WALL | URBAN_TOWER) || c.road_halo[(k / SQ + 1) * HS + k % SQ + 1] & 0x80 != 0;
                if let Some(nk) = nk.filter(|&nk| !raised(k) && !raised(nk)) {
                    if c.tier[nk] != c.tier[k] {
                        let dh = (c.height[nk] - c.height[k]).abs();
                        e |= if dh >= 4.0 { bit_cliff } else { bit_slope };
                    }
                }
            }
            if matches!(c.surface[k], Surface::Shallow | Surface::Mud) {
                e |= 16;
            }
            c.edges[k] = e;
        }
    }
    for o in &c.objects {
        let inf = c.info(o);
        if inf.difficult {
            let r = (inf.radius * o.scale) as i64;
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (o.x as i64 + dx, o.y as i64 + dy);
                    if x >= 0 && y >= 0 && (x as usize) < SQ && (y as usize) < SQ {
                        c.edges[y as usize * SQ + x as usize] |= 16;
                    }
                }
            }
        }
    }
}
