//! Flat binary payloads handed to JavaScript (zero-copy views on the receiving side).
//!
//! Terrain tile layout, little-endian, 48-byte header:
//!   0 magic u32 | 4 level u32 | 8 x u32 | 12 y u32
//!  16 base f32  | 20 min f32  | 24 max f32 | 28 grad_rms f32
//!  32 padded_len u32 (f32 count) | 36 tex_len u32 (u16 count) | 40 biome_len u32 (bytes) | 44 rivers_len u32 (bytes)
//!  48 padded heights f32[PADDED²]      (kept by the coordinator for child tiles; as generated)
//!  .. render texels u16[TILE_SAMPLES² * 4] as f16 RGBA: (h - base, dh/dx, dh/dy, water - base),
//!     h as drawn (at the three finest levels the ground shaped by hand: `shaped_heights`; base,
//!     min, max and grad_rms are of it too)
//!  .. biome texels u8[TILE_SAMPLES² * 4]: (primary, secondary, blend, coast distance)
//!  .. rivers: n_lines u32, n_verts u32, starts u32[n_lines + 1], verts f32[n_verts * 4]
//!     as (x, y in tile units 0..1, width ft, discharge). Each segment belongs to the one tile
//!     containing its midpoint, so neighbours never draw the same segment twice.
//!  .. roads: same layout, vertices as (x, y in tile units, bed width ft, class).
//!  .. site polygons (levels >= SITE_MIN_LEVEL): n u32, n_verts u32, starts u32[n + 1],
//!     attrs u32[n] (kind | ward << 8 | floors << 16 | has-function << 24; kind 0 building,
//!     1 plaza, 2 field, 3 block, 4 ruin, 5 graveyard, 6 deck: bridge or pier, 7 monument,
//!     8 a camp's clearing, 9 a tent (or a park's gazebo), 11 a park's water (its pond as a fan
//!     from its middle, its stream's pieces), 12 its gravel (a path's piece or a round), 13 a
//!     parterre, 14 a tree's (or topiary's) crown, 15 a flowerbed; a park's raised features as
//!     monuments; all of a park's in the tile holding its middle),
//!     verts f32[n_verts * 2] (tile units).
//!     A polygon belongs to
//!     the tile containing its centroid.
//!  .. site lines: the line layout, vertices (x, y, width ft, kind: 0 wall, 1 tower).

use crate::World;
use crate::core::tile::{HALO, PADDED, TILE_N, TILE_SAMPLES, TileKey};
use crate::lod::terrain_refine::TerrainOut;
use crate::t0::T0;

pub const TERRAIN_MAGIC: u32 = 0x5445_5252; // "TERR"
pub const HEADER_BYTES: usize = 48;

pub fn pack_terrain(world: &World, t0: &T0, key: &TileKey, tile: &TerrainOut) -> Vec<u8> {
    let padded = &tile.padded;
    assert_eq!(padded.len(), PADDED * PADDED);
    let s = world.geom.spacing_ft(key.level);
    // What is drawn (and read back for heights) is the ground shaped by hand; the samples sent on
    // for child tiles are as generated.
    let shaped = shaped_heights(world, t0, key, tile);
    let h: &[f32] = shaped.as_deref().unwrap_or(padded);
    let at = |i: usize, j: usize| h[(j + HALO) * PADDED + i + HALO];

    let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
    for j in 0..TILE_SAMPLES {
        for i in 0..TILE_SAMPLES {
            let v = at(i, j);
            min = min.min(v);
            max = max.max(v);
        }
    }
    let base = (min + max) * 0.5;

    let inv_2s = 1.0 / (2.0 * s);
    let (gx0, gy0) = (key.x as i64 * TILE_N as i64, key.y as i64 * TILE_N as i64);

    // Biome warp on a stride-4 grid anchored to the global lattice, interpolated per sample:
    // 16x fewer noise evaluations, still a pure function of lattice position (seamless).
    const WS: usize = 4;
    const WN: usize = TILE_N / WS + 1;
    let mut warp = vec![(0.0f64, 0.0f64); WN * WN];
    for gj in 0..WN {
        for gi in 0..WN {
            let (x, y) = ((gx0 + (gi * WS) as i64) as f64 * s, (gy0 + (gj * WS) as i64) as f64 * s);
            warp[gj * WN + gi] = t0.biome_warp(x, y);
        }
    }
    let mut tex = Vec::with_capacity(TILE_SAMPLES * TILE_SAMPLES * 4);
    let mut biome = Vec::with_capacity(TILE_SAMPLES * TILE_SAMPLES * 4);
    let mut grad_sq = 0.0f64;
    for j in 0..TILE_SAMPLES {
        let y = (gy0 + j as i64) as f64 * s;
        for i in 0..TILE_SAMPLES {
            let x = (gx0 + i as i64) as f64 * s;
            let k = (j + HALO) * PADDED + i + HALO;
            let gx = (h[k + 1] - h[k - 1]) as f64 * inv_2s;
            let gy = (h[k + PADDED] - h[k - PADDED]) as f64 * inv_2s;
            grad_sq += gx * gx + gy * gy;
            tex.push(f16_bits(h[k] - base));
            tex.push(f16_bits(gx as f32));
            tex.push(f16_bits(gy as f32));
            tex.push(f16_bits(t0.sample_water(x, y).max(tile.river_water[k]) - base));
            let (gi, gj) = ((i / WS).min(WN - 2), (j / WS).min(WN - 2));
            let (fx, fy) = ((i - gi * WS) as f64 / WS as f64, (j - gj * WS) as f64 / WS as f64);
            let lerp2 = |a: (f64, f64), b: (f64, f64), t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let top = lerp2(warp[gj * WN + gi], warp[gj * WN + gi + 1], fx);
            let bot = lerp2(warp[(gj + 1) * WN + gi], warp[(gj + 1) * WN + gi + 1], fx);
            biome.extend_from_slice(&t0.sample_biome(x, y, lerp2(top, bot, fy)));
        }
    }
    let grad_rms = crate::core::sqrt(grad_sq / (TILE_SAMPLES * TILE_SAMPLES) as f64) as f32;
    let (ox, oy) = world.geom.tile_origin_ft(key);
    let size = world.geom.tile_size_ft(key.level);
    // River lines stop at the shore: split pieces where they run out over a lake or the sea.
    let river_lines = tile.pieces.iter().flat_map(|p| {
        let mut runs: Vec<Vec<[f64; 4]>> = vec![Vec::new()];
        for c in &p.pts {
            if t0.sample_water(c.p[0], c.p[1]) > crate::t0::hydro::DRY {
                // Keep the first point in the water so the line reaches the shore.
                if let Some(run) = runs.last_mut().filter(|r| !r.is_empty()) {
                    run.push([c.p[0], c.p[1], c.w, c.q]);
                    runs.push(Vec::new());
                }
            } else {
                runs.last_mut().unwrap().push([c.p[0], c.p[1], c.w, c.q]);
            }
        }
        runs.into_iter().filter(|r| r.len() >= 2)
    });
    let rivers = pack_lines(ox, oy, size, river_lines);
    // Roads, and the streets towns draw as roads (approaches to gates, main streets, streets
    // over bridges) from site levels in.
    let town_roads: Vec<Vec<[f64; 4]>> = if key.level >= SITE_MIN_LEVEL {
        crate::town::layouts_near(world, t0, [ox, oy, ox + size, oy + size])
            .iter()
            .flat_map(|l| l.roads.iter().map(|(pts, class, w)| pts.iter().map(|p| [p[0], p[1], *w, *class as f64]).collect::<Vec<_>>()).collect::<Vec<_>>())
            .collect()
    } else {
        Vec::new()
    };
    // Network roads are sampled finely for carving; the renderer only needs about one vertex
    // per half sample of this level (far fewer capsules at continent zoom).
    let cell = 0.5 * size / TILE_N as f64;
    // Over a ferry's river the road is the ferry's line (class 5, drawn dashed), from the level
    // where a crossing is a few samples across.
    let ferries: Vec<_> = if key.level >= FERRY_MIN_LEVEL {
        crate::lod::roads::river_crossings(&tile.roads, &tile.pieces).into_iter().filter(|c| c.kind == crate::t0::roads::CrossingKind::Ferry).collect()
    } else {
        Vec::new()
    };
    let class_at = |r: &crate::lod::roads::RoadPiece, p: [f64; 2]| {
        if ferries.iter().any(|f| f.class == r.class && {
            let (a, x) = f.local(p);
            a.abs() <= f.half_span() && x.abs() <= 1.0
        }) {
            5.0
        } else {
            r.class as u8 as f64
        }
    };
    let roads = pack_lines(
        ox,
        oy,
        size,
        tile.roads.iter().map(|r| decimate(r.pts.iter().map(|c| [c.p[0], c.p[1], r.class.width_ft(), class_at(r, c.p)]).collect(), cell)).chain(town_roads),
    );

    let mut out = Vec::with_capacity(HEADER_BYTES + padded.len() * 4 + tex.len() * 2 + biome.len());
    for v in [TERRAIN_MAGIC, key.level as u32, key.x, key.y] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [base, min, max, grad_rms] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [padded.len() as u32, tex.len() as u32, biome.len() as u32, rivers.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in padded {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &tex {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&biome);
    out.extend_from_slice(&rivers);
    out.extend_from_slice(&roads);
    out.extend_from_slice(&pack_sites(world, t0, key, tile));
    out
}

/// A tile's heights as drawn, at the three finest levels: the ground shaped by hand
/// (`ground::Shaper`, a sheer edge eased over a sample or a battlemap square, whichever is wider)
/// over the samples it reaches, one round the texels for the gradients; `None` where nothing
/// shaped reaches the tile. Each sample is a pure function of its position (seamless), and at the
/// finest level a battlemap square's centre is shaped exactly as the battlemap shapes it.
pub fn shaped_heights(world: &World, t0: &T0, key: &TileKey, tile: &TerrainOut) -> Option<Vec<f32>> {
    if key.level + 2 < world.geom.max_level {
        return None;
    }
    let s = world.geom.spacing_ft(key.level);
    let (gx0, gy0) = (key.x as i64 * TILE_N as i64, key.y as i64 * TILE_N as i64);
    let (lo, hi) = (|g: i64| (g - 1) as f64 * s, |g: i64| (g + TILE_SAMPLES as i64) as f64 * s);
    let sh = crate::ground::Shaper::new(&world.file.edits, [lo(gx0), lo(gy0), hi(gx0), hi(gy0)], s.max(crate::battlemap::SQUARE_FT))?;
    let r = sh.reach();
    // The samples (-1 to TILE_SAMPLES, tile-local) inside the reach.
    let span = |a: f64, b: f64, g0: i64| ((crate::core::ceil(a / s) as i64 - g0).max(-1), (crate::core::floor(b / s) as i64 - g0).min(TILE_SAMPLES as i64));
    let ((i0, i1), (j0, j1)) = (span(r[0], r[2], gx0), span(r[1], r[3], gy0));
    let mut out = tile.padded.clone();
    for j in j0..=j1 {
        let y = (gy0 + j) as f64 * s;
        for i in i0..=i1 {
            let x = (gx0 + i) as f64 * s;
            let k = (j + HALO as i64) as usize * PADDED + (i + HALO as i64) as usize;
            out[k] = sh.at_point(x, y, out[k], t0.sample_water(x, y).max(tile.river_water[k]));
        }
    }
    Some(out)
}

/// A simple polygon as triangles (ear clipping); a convex one as itself.
fn ears(poly: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
    use crate::town::geom::{area, cross, sub};
    let n = poly.len();
    let ccw = area(poly) > 0.0;
    let turn = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        let t = cross(sub(b, a), sub(c, b));
        if ccw { t } else { -t }
    };
    if n < 3 || (0..n).all(|i| turn(poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]) >= 0.0) {
        return vec![poly.to_vec()];
    }
    let inside = |p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]| turn(a, b, p) > 0.0 && turn(b, c, p) > 0.0 && turn(c, a, p) > 0.0;
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::new();
    while idx.len() > 3 {
        let m = idx.len();
        let Some(k) = (0..m).find(|&k| {
            let (a, b, c) = (poly[idx[(k + m - 1) % m]], poly[idx[k]], poly[idx[(k + 1) % m]]);
            turn(a, b, c) > 0.0 && !idx.iter().any(|&j| ![idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]].contains(&j) && inside(poly[j], a, b, c))
        }) else {
            break;
        };
        out.push(vec![poly[idx[(k + m - 1) % m]], poly[idx[k]], poly[idx[(k + 1) % m]]]);
        idx.remove(k);
    }
    if idx.len() == 3 {
        out.push(idx.iter().map(|&j| poly[j]).collect());
    }
    out
}

/// Decks (world ft polygons) where network roads bridge rivers: along the road, over the
/// channel at the crossing's angle with a landing each side, as wide as the road plus 4 ft.
fn road_bridges(tile: &TerrainOut) -> Vec<Vec<[f64; 2]>> {
    crate::lod::roads::river_crossings(&tile.roads, &tile.pieces)
        .iter()
        .filter(|c| c.kind == crate::t0::roads::CrossingKind::Bridge)
        .map(|c| crate::town::rect(c.at, c.u, c.w / c.sin + 24.0, c.class.width_ft() + 4.0))
        .collect()
}

/// Ferry lines are drawn from this level (40-ft samples).
const FERRY_MIN_LEVEL: u8 = 10;

/// Settlements appear from this level (blocks), buildings from `BUILDING_MIN_LEVEL`.
pub const SITE_MIN_LEVEL: u8 = 9;
pub const BUILDING_MIN_LEVEL: u8 = 11;

fn pack_sites(world: &World, t0: &T0, key: &TileKey, tile: &TerrainOut) -> Vec<u8> {
    let (ox, oy) = world.geom.tile_origin_ft(key);
    let size = world.geom.tile_size_ft(key.level);
    let mut starts: Vec<u32> = vec![0];
    let mut attrs: Vec<u32> = Vec::new();
    let mut verts: Vec<f32> = Vec::new();
    let mut lines: Vec<Vec<[f64; 4]>> = Vec::new();
    if key.level >= SITE_MIN_LEVEL {
        let owns = |p: [f64; 2]| p[0] >= ox && p[0] < ox + size && p[1] >= oy && p[1] < oy + size;
        // (A park's polygons all belong to the tile holding the park's middle, so they are drawn in
        // order: `whole` while one is pushed.)
        let whole = std::cell::Cell::new(None::<bool>);
        let mut push = |poly: &[[f64; 2]], attr: u32| {
            if poly.len() < 3 || !whole.get().unwrap_or_else(|| owns(crate::town::geom::centroid(poly))) {
                return;
            }
            for p in poly {
                verts.extend_from_slice(&[((p[0] - ox) / size) as f32, ((p[1] - oy) / size) as f32]);
            }
            starts.push((verts.len() / 2) as u32);
            attrs.push(attr);
        };
        // Network roads over rivers: a deck where each road crosses a river (the battlemap
        // draws the full bridge).
        if key.level >= BUILDING_MIN_LEVEL {
            for d in road_bridges(tile) {
                push(&d, 6);
            }
            // Crossings put down by hand: a bridge's deck; a ferry's jetties, raft and rope.
            for c in world.file.edits.crossings.values().filter(|c| c.problem(world.geom.map_w_ft, world.geom.map_h_ft).is_none()) {
                for d in crate::battlemap::hand_decks(c) {
                    push(&d, 6);
                }
            }
        }
        for l in crate::town::layouts_near(world, t0, [ox, oy, ox + size, oy + size]) {
            for f in &l.fields {
                push(f, 2);
            }
            // A green (a village's or a town's) and a temple's close are the ground round them, as
            // on the battlemap; a town's squares are paved.
            // (A square where streets meet need not be convex: drawn as triangles.)
            // (A castle's bailey is its block's, not paved.)
            for p in l.plazas.iter().filter(|p| l.tier != crate::t0::settle::Tier::Village && !p.kind.is_lawn() && p.kind != crate::town::PlazaKind::Bailey) {
                for tri in ears(&p.pts) {
                    push(&tri, 1);
                }
            }
            // Parks as their battlemaps lay them out (the lawn is the ground, as a green's): gravel
            // paths and rounds, the pond and the stream, a formal garden's parterres and its basins,
            // planters and daises, the gazebo or bandstand, the trees' crowns and the beds (once
            // buildings are drawn).
            for pk in l.parks.iter().filter(|_| key.level >= BUILDING_MIN_LEVEL) {
                let pc = crate::town::geom::centroid(&pk.pts);
                if !owns(pc) {
                    continue;
                }
                whole.set(Some(true));
                let lay = crate::battlemap::park_layout(pk, crate::battlemap::land_biome(t0, pc), l.tier >= crate::t0::settle::Tier::City, world.stream("battlemap"));
                for (pts, w) in &lay.paths {
                    for q in pts.windows(2) {
                        let d = crate::town::geom::sub(q[1], q[0]);
                        let l = crate::town::geom::len(d);
                        if l > 0.5 {
                            push(&crate::town::rect(crate::town::geom::lerp(q[0], q[1], 0.5), crate::town::geom::mul(d, 1.0 / l), l + 0.5 * w, *w), 12);
                        }
                    }
                }
                for &(c, r) in &lay.rounds {
                    push(&crate::town::geom::circle(c, r, 14), 12);
                }
                // (The pond fanned from its middle: its outline is star-shaped about it, not convex.)
                if let Some(pond) = &lay.pond {
                    let mut fan = vec![pond.centre];
                    fan.extend(pond.pts.iter().copied());
                    fan.push(pond.pts[0]);
                    push(&fan, 11);
                }
                for s in lay.stream.windows(2) {
                    let d = crate::town::geom::sub(s[1].0, s[0].0);
                    let l = crate::town::geom::len(d);
                    if l > 0.5 {
                        push(&crate::town::rect(crate::town::geom::lerp(s[0].0, s[1].0, 0.5), crate::town::geom::mul(d, 1.0 / l), l + 2.0, crate::park::STREAM_FT), 11);
                    }
                }
                for q in &lay.parterres {
                    push(q, 13);
                }
                for &(c, r, _) in &lay.features {
                    push(&crate::town::geom::circle(c, r + 1.0, 12), 7);
                }
                for &(at, rot, kind, _, scale) in &lay.props {
                    let inf = crate::battlemap::info(kind as u16);
                    let r = inf.radius as f64 * crate::battlemap::SQUARE_FT * scale as f64;
                    if (inf.height_ft >= 15.0 && inf.blocks_move) || kind == crate::battlemap::Kind::Topiary {
                        push(&crate::town::geom::circle(at, 0.85 * r, 8), 14);
                    } else if kind == crate::battlemap::Kind::Flowerbed {
                        push(&crate::town::rect(at, [libm::cos(rot as f64), libm::sin(rot as f64)], 1.6 * r, 0.9 * r), 15);
                    } else if matches!(kind, crate::battlemap::Kind::Gazebo | crate::battlemap::Kind::Bandstand) {
                        push(&crate::town::geom::circle(at, 0.8 * r, 8), 9);
                    }
                }
                whole.set(None);
            }
            // A camp: its clearing, and its tents (as the battlemap's sprites lie).
            for (y, _) in &l.yards {
                push(y, 8);
            }
            for p in l.props.iter().filter(|p| p.kind == crate::battlemap::Kind::Tent) {
                let r = crate::battlemap::info(p.kind as u16).radius as f64 * crate::battlemap::SQUARE_FT * p.scale as f64;
                let poly = if p.variant == 3 {
                    crate::town::geom::circle(p.at, 0.95 * r, 10)
                } else {
                    let dir = [libm::cos(p.rot as f64), libm::sin(p.rot as f64)];
                    crate::town::rect(p.at, dir, 1.8 * r, 1.4 * r)
                };
                push(&poly, 9);
            }
            for d in l.bridges.iter().chain(&l.piers) {
                push(d, 6);
            }
            for m in &l.monuments {
                push(m, 7);
            }
            let detailed = key.level >= BUILDING_MIN_LEVEL || l.tier == crate::t0::settle::Tier::Village;
            if detailed {
                for b in &l.buildings {
                    let kind = match b.structure {
                        crate::town::Structure::Roofed => 0,
                        crate::town::Structure::Ruin => 4,
                        crate::town::Structure::Open => 5,
                    };
                    let attr = kind | (b.ward as u32) << 8 | (b.floors as u32) << 16 | (b.func.is_some() as u32) << 24;
                    // A keep of pieces: each piece (convex, outlined), its towers over its body;
                    // each with its kind + 1 (bits 25-28: 1 body, 2 round tower, 3 square tower, 4
                    // forebuilding, 5 wing) and how it is topped (bits 29-31: `town::keep_tops`, its
                    // donjon's end in bit 31).
                    if !b.parts.is_empty() {
                        let c = crate::town::geom::centroid(&b.poly);
                        whole.set(Some(owns(c)));
                        let tops = crate::town::keep_tops(b, world.stream("keep.tops"));
                        for (p, &top) in b.parts.iter().zip(&tops) {
                            use crate::town::PartKind as K;
                            let code = match p.kind {
                                K::Body => 1u32,
                                K::Tower { round: true } => 2,
                                K::Tower { round: false } => 3,
                                K::Forebuilding => 4,
                                K::Wing => 5,
                            };
                            push(&p.poly, attr | code << 25 | (top as u32) << 29);
                        }
                        whole.set(None);
                        continue;
                    }
                    push(&b.poly, attr);
                    // A keep's round corner towers (as the battlemap draws them), standing a
                    // little proud of its walls; each tile keeps the ones it owns.
                    if let Some(t) = crate::interior::battlements(b)
                        && t > 0.0
                        && b.poly.len() == 4
                    {
                        use crate::town::geom::{add, dist, mul, sub};
                        let p = &b.poly;
                        let (la, lb) = (dist(p[0], p[1]), dist(p[0], p[3]));
                        let rt = (t * 0.6).max((la.min(lb) * 0.11).min(13.0));
                        for k in 0..4 {
                            let (c, n, q) = (p[k], p[(k + 1) % 4], p[(k + 3) % 4]);
                            let into = add(mul(sub(n, c), 0.55 * rt / dist(c, n).max(1e-9)), mul(sub(q, c), 0.55 * rt / dist(c, q).max(1e-9)));
                            let at = add(c, into);
                            lines.push(vec![[at[0], at[1], 2.0 * rt, 1.0], [at[0] + 0.01, at[1], 2.0 * rt, 1.0]]);
                        }
                    }
                }
            } else {
                for bl in &l.blocks {
                    push(bl, 3);
                }
            }
            for w in &l.walls {
                lines.push(w.iter().map(|p| [p[0], p[1], 9.0, 0.0]).collect());
            }
            for t in &l.towers {
                // (A square tower: a stroke as wide as it, along its sides.)
                if let Some(u) = l.square_tower(*t) {
                    let (a, b) = (crate::town::geom::sub(*t, crate::town::geom::mul(u, 11.0)), crate::town::geom::add(*t, crate::town::geom::mul(u, 11.0)));
                    lines.push(vec![[a[0], a[1], 22.0, 3.0], [b[0], b[1], 22.0, 3.0]]);
                    continue;
                }
                lines.push(vec![[t[0], t[1], 22.0, 1.0], [t[0] + 0.01, t[1], 22.0, 1.0]]);
            }
            for t in &l.gate_towers {
                lines.push(vec![[t[0], t[1], 30.0, 1.0], [t[0] + 0.01, t[1], 30.0, 1.0]]);
            }
            for &(t, gate) in &l.extra_towers {
                if let Some(u) = l.square_tower(t).filter(|_| !gate) {
                    let (a, b) = (crate::town::geom::sub(t, crate::town::geom::mul(u, 11.0)), crate::town::geom::add(t, crate::town::geom::mul(u, 11.0)));
                    lines.push(vec![[a[0], a[1], 22.0, 3.0], [b[0], b[1], 22.0, 3.0]]);
                    continue;
                }
                let w = if gate { 30.0 } else { 22.0 };
                lines.push(vec![[t[0], t[1], w, 1.0], [t[0] + 0.01, t[1], w, 1.0]]);
            }
            // Flights of steps: hatched ink (style 2) along each axis, over the street's whole width.
            for ax in crate::town::stairs::axes(&l.stairs) {
                lines.push(vec![[ax.a[0], ax.a[1], ax.width, 2.0], [ax.b[0], ax.b[1], ax.width, 2.0]]);
            }
        }
    }
    let n = attrs.len() as u32;
    let mut out = Vec::with_capacity(8 + starts.len() * 4 + attrs.len() * 4 + verts.len() * 4);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&((verts.len() / 2) as u32).to_le_bytes());
    for v in &starts {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &attrs {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &verts {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&pack_lines(ox, oy, size, lines.into_iter()));
    out
}

/// Keep a polyline's vertices where it enters a new world-grid cell (and both ends). A pure
/// function of each vertex and its predecessor, so neighbouring tiles decimate the same line
/// the same way and their segments still meet.
fn decimate(line: Vec<[f64; 4]>, cell: f64) -> Vec<[f64; 4]> {
    if line.len() <= 2 {
        return line;
    }
    let key = |p: &[f64; 4]| (crate::core::floor(p[0] / cell) as i64, crate::core::floor(p[1] / cell) as i64);
    let last = line.len() - 1;
    // (Where the class changes, a ferry's line, the point stays.)
    line.iter().enumerate().filter(|&(i, p)| i == 0 || i == last || key(p) != key(&line[i - 1]) || p[3] != line[i - 1][3] || p[3] != line[i + 1][3]).map(|(_, p)| *p).collect()
}

/// Polylines of (x ft, y ft, a, b) as the segments this tile owns (by segment midpoint).
fn pack_lines(ox: f64, oy: f64, size: f64, lines: impl Iterator<Item = Vec<[f64; 4]>>) -> Vec<u8> {
    let owns = |ax: f64, ay: f64, bx: f64, by: f64| {
        let (mx, my) = (0.5 * (ax + bx), 0.5 * (ay + by));
        mx >= ox && mx < ox + size && my >= oy && my < oy + size
    };
    let mut starts: Vec<u32> = Vec::new();
    let mut verts: Vec<f32> = Vec::new();
    let push = |verts: &mut Vec<f32>, p: &[f64; 4]| {
        verts.extend_from_slice(&[((p[0] - ox) / size) as f32, ((p[1] - oy) / size) as f32, p[2] as f32, p[3] as f32]);
    };
    for line in lines {
        let mut open = false;
        for seg in line.windows(2) {
            let (a, b) = (&seg[0], &seg[1]);
            if owns(a[0], a[1], b[0], b[1]) {
                if !open {
                    starts.push((verts.len() / 4) as u32);
                    push(&mut verts, a);
                    open = true;
                }
                push(&mut verts, b);
            } else {
                open = false;
            }
        }
    }
    let n_lines = starts.len() as u32;
    starts.push((verts.len() / 4) as u32);
    let mut out = Vec::with_capacity(8 + starts.len() * 4 + verts.len() * 4);
    out.extend_from_slice(&n_lines.to_le_bytes());
    out.extend_from_slice(&((verts.len() / 4) as u32).to_le_bytes());
    for v in &starts {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &verts {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[inline]
pub fn f16_bits(v: f32) -> u16 {
    half::f16::from_f32(v).to_bits()
}
