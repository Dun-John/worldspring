//! The hand-built part of a chunk (`generate`): what stands along water drawn by hand, quays,
//! breakwaters and piers built by hand, crossings put down by hand and roads drawn by hand, the
//! decks over water (ferries' jetties and rafts, islets' decks), and the road crossings drawn
//! as vectors (bridges, fords' stepping stones, ferries).

use super::*;

/// What the hand-built part reads: the chunk's frame (`hx0`, `hy0`: its halo's corner, world ft;
/// `span`: the halo's side), the ground shaped by hand, the road crossings and the town decks
/// found before it, and the halo's grids it doesn't change.
pub(super) struct Ctx<'a> {
    pub world: &'a World,
    pub t0: &'a T0,
    pub seed: u64,
    pub sea: f64,
    pub hx0: f64,
    pub hy0: f64,
    pub span: f64,
    pub shaper: &'a Option<crate::ground::Shaper<'a>>,
    pub crossings: &'a [crate::lod::roads::RiverCrossing],
    pub crossing_shapes: &'a [CrossingShape],
    pub islet_decks: &'a [(Vec<[f64; 2]>, f32)],
    pub lined_decks: &'a [Vec<[f64; 2]>],
    pub look_h: &'a [crate::ground::Look],
    pub bld_h: &'a [u16],
    pub hw: &'a [f32],
}

/// The chunk's state the hand-built part writes: the halo's heights, lifts, road and urban
/// bytes, and the vectors, harbour things and stalls drawn.
pub(super) struct Halo<'a> {
    pub hh: &'a mut [f32],
    pub lift: &'a mut [f32],
    pub road_h: &'a mut [u8],
    pub urb_h: &'a mut [u8],
    pub shapes: &'a mut Vec<VectorShape>,
    pub dock: &'a mut Vec<([f64; 2], f32, Kind, u8, bool)>,
    pub stalls: &'a mut Vec<([f64; 2], f32, Kind)>,
}

/// What the rest of the chunk reads from the hand-built part: the halo squares paved as a quay
/// beside dug water (`quay_h`), the roads drawn by hand near the chunk and their squares' kinds
/// + 1 (`path_h`).
pub(super) struct Built {
    pub quay_h: Vec<bool>,
    pub path_near: Vec<crate::paths::Near>,
    pub path_h: Vec<u8>,
    /// Halo squares of a footpath's steps drawn by hand (empty if none), and each one's run:
    /// (key, way up along the stretch, its start, unit direction, the steps' width, the square's
    /// place along it), as `generate`'s `hand_runs` takes them.
    pub path_stairs: Vec<bool>,
    pub path_runs: Vec<PathRun>,
}

pub(super) type PathRun = ((u32, u32, u32), bool, [f64; 2], [f64; 2], f64, f64);

/// How far a hand road's banks reach past its band at most (ft).
const BED_EASE_MAX_FT: f64 = 20.0;

/// The hand-built part of `generate`, in its order, with `generate`'s closures over the chunk's
/// frame (`center`, `range`, `overlaps`, `owns`, `local`).
pub(super) fn build(
    ctx: &Ctx,
    halo: Halo,
    center: impl Fn(usize, usize) -> [f64; 2],
    range: impl Fn((f64, f64, f64, f64)) -> (usize, usize, f64, f64),
    overlaps: impl Fn((f64, f64, f64, f64)) -> bool,
    owns: impl Fn([f64; 2]) -> bool,
    local: impl Fn(&[f64; 2]) -> [f32; 2],
) -> Built {
    let &Ctx { world, t0, seed, sea, hx0, hy0, span, shaper, crossings, crossing_shapes, islet_decks, lined_decks, look_h, bld_h, hw } = ctx;
    let Halo { hh, lift, road_h, urb_h, shapes, dock, stalls } = halo;
    let bbox = bounds;
    // P145: what stands along water drawn by hand, by its kind (`hand_shore_things`).
    hand_shore_things(&world.file.edits, seed, sea, [hx0, hy0, hx0 + span, hy0 + span], dock);
    // R5 M20: quays and breakwaters built by hand: their made ground paved (a quay's with cargo
    // along it); a pier's deck over the water at one flat level, as a hand bridge's.
    // (R7 173: a shore quay's piers as piers of their own.)
    let near: Vec<std::borrow::Cow<crate::world::Wharf>> = crate::wharf::near(world, [hx0, hy0, hx0 + span, hy0 + span]).flat_map(|(_, w)| std::iter::once(std::borrow::Cow::Borrowed(w)).chain(crate::wharf::shore_piers(w, t0).into_iter().map(std::borrow::Cow::Owned))).collect();
    for w in near.iter().map(|w| w.as_ref()) {
        use crate::world::WharfKind;
        if w.kind == WharfKind::Pier {
            let top = crate::wharf::level(w, t0);
            let stone = w.material == crate::world::CrossingMaterial::Stone;
            for d in crate::wharf::decks(w) {
                let b = bbox(&d);
                let (i0, j0, i1, j1) = range(b);
                if !overlaps(b) || i1 < 0.0 || j1 < 0.0 {
                    continue;
                }
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let h = j * HS + i;
                        if hh[h] < top - 0.5 && crate::town::geom::contains(&d, center(i, j)) {
                            lift[h] += top - hh[h];
                            hh[h] = top;
                            road_h[h] = 0x80 | 3;
                            urb_h[h] = URBAN_DECK;
                        }
                    }
                }
                if owns(crate::town::geom::centroid(&d)) {
                    shapes.push(VectorShape { kind: ShapeKind::Deck, size: if stone { 3.0 } else { 0.0 }, pts: d.iter().map(&local).collect() });
                }
                if d.len() == 4 {
                    pier_bollards(&d, seed, dock);
                }
            }
            continue;
        }
        if w.kind == WharfKind::Quay && w.shore {
            // (A shore quay's pieces: cargo and works along each one's face; a timber one planked over.)
            let piers: Vec<Vec<[f64; 2]>> = crate::wharf::shore_piers(w, t0).iter().flat_map(crate::wharf::decks).collect();
            for (piece, m) in crate::wharf::shore_pieces(w).iter().filter(|p| overlaps(bbox(&p.0))) {
                quay_cargo(piece, seed, stalls);
                quay_works(piece, &piece[..*m], seed, &piers, true, dock);
                if w.material == crate::world::CrossingMaterial::Timber && owns(crate::town::geom::centroid(piece)) {
                    shapes.push(VectorShape { kind: ShapeKind::Deck, size: 0.0, pts: piece.iter().map(&local).collect() });
                }
            }
        } else if w.kind == WharfKind::Quay {
            for strip in crate::wharf::strips(w, t0).iter().filter(|p| p.len() == 4 && overlaps(bbox(p))) {
                quay_cargo(strip, seed, stalls);
                quay_works(strip, &strip[..2], seed, &[], true, dock);
            }
        } else {
            // (A breakwater's armour where the water meets it, its far end rounded.)
            armour(&w.pts, &crate::town::harbour::Mole { pts: w.pts.clone(), width: w.width, level: 0.0 }.edges(0.5 * w.width), &hh, &hw, hx0, hy0, &owns, &local, shapes);
        }
    }
    for h in (0..look_h.len()).filter(|&h| look_h[h] == crate::ground::Look::Made) {
        if urb_h[h] == 0 && hh[h] >= hw[h] {
            urb_h[h] = URBAN_PLAZA;
        }
    }
    // P146: beside dug water of the sea drawn by hand, a quay: paved as a town's plaza, two squares
    // wide (`QUAY_FT`; open dry ground only).
    let mut quay_h = vec![false; if shaper.as_ref().is_some_and(|s| s.has_waters()) { HS * HS } else { 0 }];
    if let Some(sh) = shaper.as_ref().filter(|_| !quay_h.is_empty()) {
        for h in 0..HS * HS {
            if urb_h[h] != 0 || bld_h[h] != 0 || road_h[h] != 0 || hh[h] < hw[h] {
                continue;
            }
            let p = center(h % HS, h / HS);
            if sh.water_edge(p[0], p[1], QUAY_FT, sea).is_some_and(|(look, _, _)| look.kind == crate::waters::WaterKind::Basin) {
                urb_h[h] = URBAN_PLAZA;
                quay_h[h] = true;
            }
        }
    }
    // Crossings put down by hand (`Edits::crossings`) near this chunk, over whatever is there:
    // a bridge's planks wherever the ground is below its deck, a ford's bed brought up to
    // wading depth, a ferry's jetties and raft (with the generated ferries' below).
    let mut hand: Vec<(CrossingShape, Deck)> = world
        .file
        .edits
        .crossings
        .values()
        .filter(|c| {
            let r = 0.5 * c.width + JETTY_FT + 40.0;
            overlaps((c.a[0].min(c.b[0]) - r, c.a[1].min(c.b[1]) - r, c.a[0].max(c.b[0]) + r, c.a[1].max(c.b[1]) + r)) && c.problem(world.geom.map_w_ft, world.geom.map_h_ft).is_none()
        })
        .map(|c| hand_crossing(c, world, t0))
        .collect();
    // (Roads drawn by hand: where each runs over water, a bridge or a ford as one put down by hand.)
    let path_near = if world.file.edits.paths.is_empty() { Vec::new() } else { crate::paths::near(&world.file.edits, [hx0 - 2.0 * SQUARE_FT, hy0 - 2.0 * SQUARE_FT, hx0 + span + 2.0 * SQUARE_FT, hy0 + span + 2.0 * SQUARE_FT], 0.0) };
    if !path_near.is_empty() {
        hand.extend(crate::paths::crossings_near(world, t0, [hx0, hy0, hx0 + span, hy0 + span], 40.0).iter().map(|c| hand_crossing(c, world, t0)));
    }
    for (c, top) in hand.iter().filter(|(c, _)| c.kind != ShapeKind::Ferry) {
        let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
        let len = crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
        let r = 0.5 * c.size + SQUARE_FT;
        let (i0, j0, i1, j1) = range((c.a[0].min(c.b[0]) - r, c.a[1].min(c.b[1]) - r, c.a[0].max(c.b[0]) + r, c.a[1].max(c.b[1]) + r));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let p = center(i, j);
                let (dx, dy) = (p[0] - c.a[0], p[1] - c.a[1]);
                let (along, across) = (dx * u[0] + dy * u[1], -dx * u[1] + dy * u[0]);
                if along < 0.0 || along > len || across.abs() > 0.5 * c.size {
                    continue;
                }
                let top = top.level(if len > 0.0 { along / len } else { 0.0 });
                if c.kind == ShapeKind::RoadBridge && hh[h] < top - 0.5 {
                    lift[h] += top - hh[h];
                    hh[h] = top;
                    road_h[h] = 0x80 | 3;
                } else if c.kind == ShapeKind::Ford && hw[h] > hh[h] && road_h[h] & 0x80 == 0 {
                    hh[h] = hh[h].max(hw[h] - crate::lod::terrain_refine::FORD_DEPTH_FT);
                }
            }
        }
    }
    // Roads drawn by hand: their band's squares on dry ground (a bridge's deck keeps its planks, a
    // ford's water stays) are road squares of their kind's look, at least a square wide so a narrow
    // path reads as one; a gravel road's squares are gravel (`path_h`, the road's kind + 1).
    // (Roadmap 7 item 168) Their beds graded on open ground: the band at the bed's surface
    // (`paths::beds`), its banks easing back to the ground at about one in two; not under a
    // network road, a town's ground, a building or water. A footpath's band climbing at a stair's
    // grade gets steps (`path_stairs`, and their runs for the treads).
    let mut path_stairs = Vec::new();
    let mut path_runs = Vec::new();
    if !path_near.is_empty() {
        // (Pruned as far out as `bed_at` reaches: the band at least `PATH_MIN_HALF_FT`.)
        let beds = crate::paths::beds(world, t0, [hx0, hy0, hx0 + span, hy0 + span], BED_EASE_MAX_FT + PATH_MIN_HALF_FT);
        let g_on = crate::town::stairs::StairParams::DEFAULT.g_on;
        for h in 0..HS * HS {
            if hw[h] > hh[h] || road_h[h] != 0 || urb_h[h] != 0 || bld_h[h] != 0 {
                continue;
            }
            let p = center(h % HS, h / HS);
            let Some(on) = crate::paths::bed_at(&beds, p, |b| b.half.max(PATH_MIN_HALF_FT) + BED_EASE_MAX_FT) else { continue };
            let band = on.half.max(PATH_MIN_HALF_FT);
            let dz = on.z - hh[h] as f64;
            let ease = (2.0 * dz.abs()).clamp(SQUARE_FT, BED_EASE_MAX_FT);
            let w = 1.0 - crate::core::noise::smoothstep(band, band + ease, on.d);
            hh[h] += (dz * w) as f32;
            if on.kind == crate::paths::PathKind::Footpath && on.d <= band && on.grade.abs() >= g_on {
                if path_stairs.is_empty() {
                    path_stairs = vec![false; HS * HS];
                }
                path_stairs[h] = true;
                let t = (p[0] - on.a[0]) * on.u[0] + (p[1] - on.a[1]) * on.u[1];
                path_runs.push(((u32::MAX, on.bed, on.seg), on.grade > 0.0, on.a, on.u, 2.0 * band, t));
            }
        }
    }
    let mut path_h = vec![0u8; if path_near.is_empty() { 0 } else { HS * HS }];
    for h in (0..path_h.len()).filter(|_| !path_near.is_empty()) {
        let Some((kind, _)) = crate::paths::at(&path_near, center(h % HS, h / HS), 0.0, PATH_MIN_HALF_FT) else { continue };
        if road_h[h] & 0x80 != 0 || hw[h] > hh[h] {
            continue;
        }
        path_h[h] = kind as u8 + 1;
        road_h[h] = match kind {
            crate::paths::PathKind::Gravel => road_h[h],
            crate::paths::PathKind::Paved | crate::paths::PathKind::Cobbled => crate::t0::roads::RoadClass::KingsRoad as u8 + 1,
            _ => crate::t0::roads::RoadClass::Track as u8 + 1,
        };
    }
    // A ferry's jetties and raft are decks too: planks over the water (the raft afloat).
    let sea_ferries = sea_ferry_shapes(world, t0, [hx0, hy0]);
    for c in crossing_shapes.iter().chain(hand.iter().map(|(c, _)| c)).chain(&sea_ferries).filter(|c| c.kind == ShapeKind::Ferry) {
        let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
        let mut decks: Vec<([f64; 2], f64, f64, f32)> = vec![
            ([c.a[0] - u[0] * 0.5 * JETTY_FT, c.a[1] - u[1] * 0.5 * JETTY_FT], JETTY_FT, JETTY_WIDTH_FT, DECK_CLEARANCE_FT),
            ([c.b[0] + u[0] * 0.5 * JETTY_FT, c.b[1] + u[1] * 0.5 * JETTY_FT], JETTY_FT, JETTY_WIDTH_FT, DECK_CLEARANCE_FT),
        ];
        if let Some((at, l, w)) = c.raft {
            decks.push((at, l, w, 1.0));
        }
        for (at, l, w, above) in decks {
            let r = 0.5 * l.max(w) + SQUARE_FT;
            let (i0, j0, i1, j1) = range((at[0] - r, at[1] - r, at[0] + r, at[1] + r));
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    let h = j * HS + i;
                    let p = center(i, j);
                    let (dx, dy) = (p[0] - at[0], p[1] - at[1]);
                    let (along, across) = (dx * u[0] + dy * u[1], -dx * u[1] + dy * u[0]);
                    if along.abs() <= 0.5 * l && across.abs() <= 0.5 * w && hw[h] > hh[h] {
                        let top = hw[h] + above;
                        lift[h] += top - hh[h];
                        hh[h] = top;
                        road_h[h] = 0x80 | 3;
                    }
                }
            }
        }
    }
    // Decks over water become plank road squares (same machinery as road bridges).
    for h in 0..HS * HS {
        if urb_h[h] == URBAN_DECK && hw[h] > hh[h] - DECK_CLEARANCE_FT {
            let top = hh[h].max(hw[h] + DECK_CLEARANCE_FT);
            lift[h] += top - hh[h];
            hh[h] = top;
            road_h[h] = 0x80 | 3;
        }
    }
    // R5 M20: and a deck over an islet is one flat deck over it and the water round it.
    for (d, top) in islet_decks {
        let (i0, j0, i1, j1) = range(bbox(d));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                if urb_h[h] == URBAN_DECK && hh[h] < *top && crate::town::geom::contains(d, center(i, j)) {
                    lift[h] += top - hh[h];
                    hh[h] = *top;
                    road_h[h] = 0x80 | 3;
                }
            }
        }
    }
    // Road crossings as vectors (`CrossingShape`); the chunk owning each draws it, except a
    // ford, whose stepping stones each chunk lays where they fall in it: every ~3.5 ft along
    // the road's centre line, a little to either side in turn, only on water.
    for (x, c) in crossings.iter().zip(crossing_shapes) {
        if c.kind == ShapeKind::Ford {
            let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
            let len = crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
            let mut stones = Vec::new();
            // Only across the channel at the crossing (a slanting road would lay a long line).
            let mid = (x.at[0] - c.a[0]) * u[0] + (x.at[1] - c.a[1]) * u[1];
            for m in 0..(len / FORD_STEP_FT) as usize {
                let t = (m as f64 + 0.5) * FORD_STEP_FT;
                if (t - mid).abs() > x.half_span() + 4.0 {
                    continue;
                }
                let jit = |salt: i64| unit(hash3(seed, m as i64, salt, (x.at[0] * 0.5) as i64 ^ (x.at[1] * 0.5) as i64)) - 0.5;
                let side = if m % 2 == 0 { 0.7 } else { -0.7 } + 0.6 * jit(1);
                let along = t + 0.8 * jit(2);
                let p = [c.a[0] + u[0] * along - u[1] * side, c.a[1] + u[1] * along + u[0] * side];
                if !owns(p) {
                    continue;
                }
                let (i, j) = (((p[0] - hx0) / SQUARE_FT) as usize, ((p[1] - hy0) / SQUARE_FT) as usize);
                let h = j.min(HS - 1) * HS + i.min(HS - 1);
                if hw[h] > hh[h] {
                    stones.push(local(&p));
                }
            }
            if !stones.is_empty() {
                shapes.push(VectorShape { kind: ShapeKind::Ford, size: (c.size / SQUARE_FT) as f32, pts: stones });
            }
            continue;
        }
        // (A lined bridge carries the road on: the road's own plank deck, which ran past the banks, is not drawn.)
        if !owns(x.at) || c.dropped || (c.kind == ShapeKind::RoadBridge && lined_decks.iter().any(|d| crate::town::geom::contains(d, x.at))) {
            continue;
        }
        let mut pts = vec![local(&c.a), local(&c.b)];
        if let Some(raft) = c.raft {
            pts.push(local(&raft.0));
        }
        if c.kind == ShapeKind::RoadBridge {
            // Old stone on some, more often on the greater roads (by where it crosses); else timber.
            let rate = [0.45, 0.35, 0.15][x.class as usize];
            let old = unit(hash3(seed, crate::core::round(x.at[0]) as i64, crate::core::round(x.at[1]) as i64, 0x0b5d)) < rate;
            pts.push([old as u8 as f32, 0.0]);
        }
        shapes.push(VectorShape { kind: c.kind, size: (c.size / SQUARE_FT) as f32, pts });
    }
    // The sea ferries' landing stages: each drawn by the chunk holding its mooring.
    for c in sea_ferries.iter().filter(|c| owns(c.a)) {
        shapes.push(VectorShape { kind: c.kind, size: (c.size / SQUARE_FT) as f32, pts: vec![local(&c.a), local(&c.b), local(&c.raft.expect("a raft").0)] });
    }
    // Those put down by hand: drawn by the chunk owning their middle; a ford's stones (along
    // its whole line, on water) by each chunk where they fall.
    for (c, _) in &hand {
        let mid = [0.5 * (c.a[0] + c.b[0]), 0.5 * (c.a[1] + c.b[1])];
        if c.kind == ShapeKind::Ford {
            let u = unit2([c.b[0] - c.a[0], c.b[1] - c.a[1]]);
            let len = crate::core::sqrt((c.b[0] - c.a[0]) * (c.b[0] - c.a[0]) + (c.b[1] - c.a[1]) * (c.b[1] - c.a[1]));
            let key = (mid[0] * 0.5) as i64 ^ (mid[1] * 0.5) as i64;
            let mut stones = Vec::new();
            for m in 0..(len / FORD_STEP_FT) as usize {
                let jit = |salt: i64| unit(hash3(seed, m as i64, salt, key)) - 0.5;
                let side = if m % 2 == 0 { 0.7 } else { -0.7 } + 0.6 * jit(1);
                let along = (m as f64 + 0.5) * FORD_STEP_FT + 0.8 * jit(2);
                let p = [c.a[0] + u[0] * along - u[1] * side, c.a[1] + u[1] * along + u[0] * side];
                if !owns(p) {
                    continue;
                }
                let (i, j) = (((p[0] - hx0) / SQUARE_FT) as usize, ((p[1] - hy0) / SQUARE_FT) as usize);
                let h = j.min(HS - 1) * HS + i.min(HS - 1);
                if hw[h] > hh[h] {
                    stones.push(local(&p));
                }
            }
            if !stones.is_empty() {
                shapes.push(VectorShape { kind: ShapeKind::Ford, size: (c.size / SQUARE_FT) as f32, pts: stones });
            }
        } else if owns(mid) {
            let mut pts = vec![local(&c.a), local(&c.b)];
            if let Some(raft) = c.raft {
                pts.push(local(&raft.0));
            }
            if c.kind == ShapeKind::RoadBridge {
                pts.push([c.stone as u8 as f32, 0.0]);
            }
            shapes.push(VectorShape { kind: c.kind, size: (c.size / SQUARE_FT) as f32, pts });
        }
    }
    Built { quay_h, path_near, path_h, path_stairs, path_runs }
}
