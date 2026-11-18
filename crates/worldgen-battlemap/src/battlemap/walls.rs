//! A town's walls, towers and gates in a chunk (`generate`): its towers' rings and floors
//! (`wallstyle::raster`) and its walls raised in stone on the ground, a palace's forecourt's low
//! garden wall and its gate's piers likewise, then the same structures as vectors, each drawn by
//! the one chunk that owns its middle, with the forecourt's gate and gravel walks.

use super::*;

/// What the walls' part reads: the layout, the chunk's halo corner (`hx0`, `hy0`, world ft), the
/// halo's grids it doesn't change (`ground_h`: the ground before anything was raised on it), the
/// eraser's gaps and the forecourts' piers, gates and walks found before it.
pub(super) struct Ctx<'a> {
    pub l: &'a crate::town::Layout,
    pub seed: u64,
    pub hx0: f64,
    pub hy0: f64,
    pub bld_h: &'a [u16],
    pub hw: &'a [f32],
    pub ground_h: &'a [f32],
    pub fence_gaps: &'a [crate::model::fences::FenceGap],
    pub fore_piers: &'a [([f64; 2], [f64; 2])],
    pub fore_gates: &'a [[[f64; 2]; 3]],
    pub walks: &'a [[[f64; 2]; 4]],
}

/// The chunk's state the walls' part writes: the halo's heights, lifts and urban bytes, the
/// vectors drawn and the gates' passages.
pub(super) struct Halo<'a> {
    pub hh: &'a mut [f32],
    pub lift: &'a mut [f32],
    pub urb_h: &'a mut [u8],
    pub shapes: &'a mut Vec<VectorShape>,
    pub gates: &'a mut Vec<([f64; 2], [f64; 2])>,
}

/// The walls' part of `generate` for one layout, in its order, with the forecourts' walls
/// (`fore_walls`) and `generate`'s closures over the chunk's frame (`center`, `range`,
/// `overlaps`, `owns`, `local`).
pub(super) fn build(
    ctx: &Ctx,
    halo: Halo,
    mut fore_walls: Vec<([f64; 2], [f64; 2])>,
    center: impl Fn(usize, usize) -> [f64; 2],
    range: impl Fn((f64, f64, f64, f64)) -> (usize, usize, f64, f64),
    overlaps: impl Fn((f64, f64, f64, f64)) -> bool,
    owns: impl Fn([f64; 2]) -> bool,
    local: impl Fn(&[f64; 2]) -> [f32; 2],
) {
    let &Ctx { l, seed, hx0, hy0, bld_h, hw, ground_h, fence_gaps, fore_piers, fore_gates, walks } = ctx;
    let Halo { hh, lift, urb_h, shapes, gates } = halo;
    // Walls (8 ft thick) and towers (22 ft across; 30 at gates): raised stone.
    // (`square`: a square tower, its sides along it.)
    let raise = |a: [f64; 2], b: [f64; 2], half: f64, val: u8, ft: f32, square: Option<[f64; 2]>, urb_h: &mut [u8], hh: &mut [f32], lift: &mut [f32]| {
        let reach = if square.is_some() { half * std::f64::consts::SQRT_2 } else { half };
        let bb = (a[0].min(b[0]) - reach, a[1].min(b[1]) - reach, a[0].max(b[0]) + reach, a[1].max(b[1]) + reach);
        if !overlaps(bb) {
            return;
        }
        let (i0, j0, i1, j1) = range(bb);
        if i1 < 0.0 || j1 < 0.0 {
            return;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                if bld_h[h] != 0 || hw[h] > hh[h] || urb_h[h] == URBAN_TOWER || urb_h[h] == URBAN_TOWER_FLOOR {
                    continue;
                }
                // Every square the drawn wall or tower touches stands on it.
                let on = match square {
                    Some(u) => {
                        let d = crate::town::geom::sub(center(i, j), a);
                        crate::town::geom::dot(d, u).abs() <= half + 1.5 && crate::town::geom::cross(u, d).abs() <= half + 1.5
                    }
                    None => crate::town::geom::seg_dist(center(i, j), a, b) <= half + 1.5,
                };
                if on {
                    urb_h[h] = val;
                    hh[h] = ground_h[h] + ft;
                    lift[h] = hh[h] - ground_h[h];
                }
            }
        }
    };
    // Towers (item 104): each a ring of wall in its shape round an open floor, a door through it on
    // the town side (`wallstyle::raster`, on the global lattice: every chunk lays the same).
    // (Every tower's ring first: one standing into another's floor walls it there.)
    let (gx0, gy0) = (crate::core::round(hx0 / SQUARE_FT) as i64, crate::core::round(hy0 / SQUARE_FT) as i64);
    // (`l`'s gatehouses and towers, once: pure in `l`.)
    let houses = crate::town::gatehouse::houses(&l);
    let towers = crate::interior::towers(&l);
    let rasters: Vec<crate::town::wallstyle::Raster> = towers
        .iter()
        .filter(|&&(t, r, _)| {
            let reach = r * std::f64::consts::SQRT_2 + 2.0;
            overlaps((t[0] - reach, t[1] - reach, t[0] + reach, t[1] + reach))
        })
        .map(|&(t, r, gate)| crate::town::wallstyle::tower_raster_in(&l, &houses, t, r, gate, SQUARE_FT).1)
        // (Only a gate block with a square in the halo: no other could set one.)
        .chain(crate::town::gatehouse::rasters_in(&houses, SQUARE_FT, [gx0, gy0, gx0 + HS as i64 - 1, gy0 + HS as i64 - 1]))
        .collect();
    for ring in [true, false] {
        for ras in &rasters {
            for (q, &v) in ras.cells.iter().enumerate().filter(|c| *c.1 != 0 && (*c.1 == crate::town::wallstyle::RING) == ring) {
                let (i, j) = (ras.i0 + (q % ras.nx) as i64 - gx0, ras.j0 + (q / ras.nx) as i64 - gy0);
                if i < 0 || j < 0 || i >= HS as i64 || j >= HS as i64 {
                    continue;
                }
                let h = j as usize * HS + i as usize;
                if bld_h[h] != 0 || hw[h] > hh[h] || urb_h[h] == URBAN_TOWER || urb_h[h] == URBAN_TOWER_FLOOR {
                    continue;
                }
                if v == crate::town::wallstyle::RING {
                    urb_h[h] = URBAN_TOWER;
                    hh[h] = ground_h[h] + TOWER_FT;
                    lift[h] = hh[h] - ground_h[h];
                } else {
                    urb_h[h] = URBAN_TOWER_FLOOR;
                }
            }
        }
    }
    // (Each as thick as it is built (item 104: a palisade thinner, rammed earth thicker), and never
    // so thin that a square's corner lets a diagonal through.)
    for w in &l.walls {
        for seg in w.windows(2) {
            let half = (0.5 * l.wall_style_at(crate::town::geom::lerp(seg[0], seg[1], 0.5)).thickness()).max(2.1);
            raise(seg[0], seg[1], half, URBAN_WALL, WALL_FT, None, urb_h, hh, lift);
        }
    }
    // (Roadmap 6 R6: the eraser's gaps through garden walls.)
    if !fence_gaps.is_empty() {
        fore_walls = fore_walls.into_iter().flat_map(|(a, b)| crate::town::fence::subtract(a, b, &[], &fence_gaps)).collect();
    }
    // A palace's forecourt: its low garden wall, the gate's piers (`interior::FORE_WALL_FT`,
    // `FORE_PIER_FT`); the gate's opening between them clear.
    for &(a, b) in &fore_walls {
        raise(a, b, 0.5 * crate::interior::FORE_WALL_FT, URBAN_WALL, GARDEN_WALL_FT, None, urb_h, hh, lift);
    }
    for &(p, u) in fore_piers {
        raise(p, p, 0.5 * crate::interior::FORE_PIER_FT, URBAN_WALL, PIER_FT, Some(u), urb_h, hh, lift);
    }
    // The same structures as vectors, each drawn by the one chunk that owns its middle. Walls
    // and towers carry a point of flags (`WALL_*`): a wall's ends standing bare (no tower on
    // them: where a ruin's stretch fell), a gate tower, a ruin's works.
    let ruined = !l.buildings.is_empty() && l.buildings.iter().all(|b| b.structure == crate::town::Structure::Ruin);
    let ruin_bit = if ruined { WALL_RUINED } else { 0 };
    let near = |p: [f64; 2], q: [f64; 2], r: f64| (p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1]) <= r * r;
    let bare = |p: [f64; 2], wi: usize| {
        !towers.iter().any(|t| near(p, t.0, t.1 + 1.0)) && !l.walls.iter().enumerate().any(|(k, w)| k != wi && (near(p, w[0], 1.0) || near(p, w[w.len() - 1], 1.0)))
    };
    // (Which side of each wall is outside: `town::wallside`, on a wall's flags point.)
    let outward = crate::town::wallside::outward(&l);
    for (wi, w) in l.walls.iter().enumerate() {
        let n = w.len();
        for (k, seg) in w.windows(2).enumerate() {
            if owns([0.5 * (seg[0][0] + seg[1][0]), 0.5 * (seg[0][1] + seg[1][1])]) {
                let ends = if k == 0 && bare(seg[0], wi) { WALL_BARE_A } else { 0 } | if k + 2 == n && bare(seg[1], wi) { WALL_BARE_B } else { 0 };
                let flags = [(ends | ruin_bit) as f32, outward[wi]];
                let ws = l.wall_style_at(crate::town::geom::lerp(seg[0], seg[1], 0.5));
                let style = ws.code(crate::town::wallstyle::TowerShape::Round) as f32;
                shapes.push(VectorShape { kind: ShapeKind::Wall, size: (ws.thickness() / SQUARE_FT) as f32, pts: vec![local(&seg[0]), local(&seg[1]), flags, [style, 0.0]] });
            }
        }
    }
    crate::town::wallside::bastions(&l, &owns, &local, shapes);
    for &(t, r, gate) in &towers {
        if owns(t) {
            // (A square tower: its flag, and the way its sides run as an angle; a D's the way it faces,
            // an octagon's the way its sides run. Then its style: `WallStyle::code` with its shape.)
            use crate::town::wallstyle::TowerShape;
            // (A gate tower: its gatehouse's form after the style, `gatehouse::House::code`.)
            let house = if gate { crate::town::gatehouse::of_tower(&houses, t) } else { None };
            let (shape, u, r) = match house {
                Some((h, second)) => {
                    let (s, u, r, _) = h.tower(second);
                    (s, u, r)
                }
                None if gate => (TowerShape::Round, [1.0, 0.0], r),
                None => {
                    let (s, u) = l.tower_form(t);
                    (s, u, r)
                }
            };
            let flags = match shape {
                TowerShape::Square => [(WALL_SQUARE | if gate { WALL_GATE } else { 0 } | ruin_bit) as f32, libm::atan2(u[1], u[0]) as f32],
                TowerShape::Round => [(if gate { WALL_GATE } else { 0 } | ruin_bit) as f32, 0.0],
                _ => [ruin_bit as f32, libm::atan2(u[1], u[0]) as f32],
            };
            let style = [l.wall_style_at(t).code(shape) as f32, house.map_or(0.0, |h| h.0.code())];
            shapes.push(VectorShape { kind: ShapeKind::Tower, size: (r / SQUARE_FT) as f32, pts: vec![local(&t), flags, style] });
        }
    }
    for &(a, b) in &fore_walls {
        if owns([0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]) {
            shapes.push(VectorShape { kind: ShapeKind::GardenWall, size: (crate::interior::FORE_WALL_FT / SQUARE_FT) as f32, pts: vec![local(&a), local(&b)] });
        }
    }
    for &(p, u) in fore_piers {
        if owns(p) {
            shapes.push(VectorShape { kind: ShapeKind::GatePier, size: (0.5 * crate::interior::FORE_PIER_FT / SQUARE_FT) as f32, pts: vec![local(&p), [libm::atan2(u[1], u[0]) as f32, 0.0]] });
        }
    }
    // (The capital's gate wrought iron, timber in the odd palace.)
    for g in fore_gates {
        if owns([0.5 * (g[0][0] + g[1][0]), 0.5 * (g[0][1] + g[1][1])]) {
            let timber = unit(hash3(seed, crate::core::round(g[0][0]) as i64, crate::core::round(g[0][1]) as i64, 0x6a7e)) < 0.25;
            shapes.push(VectorShape { kind: ShapeKind::ForecourtGate, size: timber as u8 as f32, pts: g.iter().map(&local).collect() });
        }
    }
    for q in walks {
        if owns(crate::town::geom::mean(q)) {
            shapes.push(VectorShape { kind: ShapeKind::GravelWalk, size: 0.0, pts: q.iter().map(&local).collect() });
        }
    }
    for (g, h) in gateways(&l, &towers) {
        gates.push((g, h));
        if owns([0.5 * (g[0] + h[0]), 0.5 * (g[1] + h[1])]) {
            let style = l.wall_style_at(g).code(crate::town::wallstyle::TowerShape::Round) as f32;
            // (Its form, the side the town is on and a bent entrance's way out: `gatehouse::House`.)
            let (form, side, exit) = crate::town::gatehouse::of_tower(&houses, g)
                .map_or((0.0, 0.0, 0.0), |(o, _)| (o.code(), if crate::town::geom::cross(crate::town::geom::sub(h, g), o.n) >= 0.0 { 1.0 } else { -1.0 }, o.exit as f32));
            shapes.push(VectorShape { kind: ShapeKind::Gate, size: ruined as u8 as f32, pts: vec![local(&g), local(&h), [style, form], [side, exit]] });
        }
    }
}
