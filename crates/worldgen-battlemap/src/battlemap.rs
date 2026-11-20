//! Battlemap chunks: one finest-level tile = 128 × 128 five-foot squares.
//!
//! Everything here is the world's own detail at 5-ft scale (not a template): square heights
//! come from the L_max terrain plus deterministic micro-relief (knolls, outcrops, ditches,
//! dune ridges, hummocks), quantized into 5-ft tiers with cliff/slope edges; surfaces come
//! from biome and water; objects (vegetation, rocks, props, hazards) carry 5e tactical data.
//!
//! **Battlemap guarantee.** Rules are enforced on grid-aligned cells inside the chunk, so they
//! are local and deterministic yet hold for every window:
//! - every aligned 8 × 8 cell has ≥ 1 cover object (≥ half cover) → any 32 × 32 window holds ≥ 9;
//! - every aligned 16 × 16 cell spans ≥ 2 tiers → any 32 × 32 window contains one such cell;
//! - every aligned 16 × 16 cell has a hazard or prop cluster, or the chunk has atmosphere;
//! - no dry square is more than `MAX_COVER_GAP` squares from cover (`cover_gap`: more in a castle's
//!   yard and on a field, where walls and hedgerows are the cover).
//! Open water far from shore is exempt (cells that are entirely deep water).


use crate::World;
use crate::core::noise::{fbm, smoothstep};
use crate::core::rng::{Pcg32, hash2, hash3, unit};
use crate::core::tile::{HALO, PADDED, TileKey};
use crate::lod::terrain_refine::TerrainOut;
use crate::t0::T0;
use crate::t0::biome::Biome;
use crate::t0::roads::CrossingKind;

/// A park laid out (`town::Park`): what a park's chunks and tiles draw.
pub use crate::park::layout as park_layout;

thread_local! {
    // Parks laid out (`park_layout`) by (layout index, park), so a park reaching several chunks is
    // laid out once: kept with its layout (the same `Rc`, so the same park) and the other inputs.
    static PARKS: std::cell::RefCell<crate::core::hash::FastMap<(u32, usize), (std::rc::Rc<crate::town::Layout>, Biome, bool, u64, std::rc::Rc<crate::park::ParkLayout>)>> = Default::default();
}

/// Park `pki` of layout `l` laid out (`park_layout`), remembered.
fn park_laid(l: &std::rc::Rc<crate::town::Layout>, pki: usize, biome: Biome, city: bool, seed: u64) -> std::rc::Rc<crate::park::ParkLayout> {
    use std::rc::Rc;
    let key = (l.index, pki);
    if let Some(lay) = PARKS.with(|m| m.borrow().get(&key).filter(|e| Rc::ptr_eq(&e.0, l) && e.1 == biome && e.2 == city && e.3 == seed).map(|e| e.4.clone())) {
        return lay;
    }
    let lay = Rc::new(park_layout(&l.parks[pki], biome, city, seed));
    PARKS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 256 {
            m.clear();
        }
        m.insert(key, (l.clone(), biome, city, seed, lay.clone()));
    });
    lay
}

// v63 underground: a ruin's plan at site zoom; small closed relief drawn soft.
pub mod ruin_plan;
mod soft_relief;

// v63 castles: a castle's kitchen garden and its smithy's anvil.
mod kitchen;
mod moat;
// v63 countryside: where haystacks stand (stack yards, by field gates).
mod farm;
mod ground;
mod crossings;
mod hand;
mod walls;
mod objects;
mod shore;
mod plaza;
mod cover;
pub use ground::*;
pub use crossings::*;
use objects::*;
use shore::*;
use plaza::*;
pub use cover::*;
pub use crate::battle::*;
pub use crate::model::consts::{SQ, SQUARE_FT, DECK_CLEARANCE_FT};

const COVER_CELL: usize = 8;
const TIER_CELL: usize = 16;
/// A road drawn by hand marks the squares within at least this (ft) of its line, so a narrow one is a
/// line of squares unbroken on the diagonal.
const PATH_MIN_HALF_FT: f64 = 3.6;

/// Micro-relief feature grid (squares).
const RELIEF_CELL: i64 = 20;

// v63 countryside
/// Wall-walk and tower-top heights above the ground (ft).
const WALL_FT: f32 = 20.0;
const TOWER_FT: f32 = 30.0;
/// A palace forecourt's low garden wall and its gate's piers (ft).
const GARDEN_WALL_FT: f32 = 4.0;
const PIER_FT: f32 = 8.0;
/// (Roadmap 6 R6) A high wall's height (ft).
const HIGH_WALL_FT: f32 = 8.0;
/// Eave height per storey (ft): a building's squares stand this much per floor above its pad.
const STOREY_FT: f32 = 10.0;

/// A keep piece's code in the payload: 0 body, 1 round tower, 2 square tower, 3 forebuilding, 4
/// wing (its top, `town::keep_tops` with its donjon's end, in the high four bits).
fn part_code(kind: crate::town::PartKind) -> u8 {
    use crate::town::PartKind as K;
    match kind {
        K::Body => 0,
        K::Tower { round: true } => 1,
        K::Tower { round: false } => 2,
        K::Forebuilding => 3,
        K::Wing => 4,
        K::Apse => APSE_PART,
    }
}

/// Micro-relief feature from its grid cell (pure function of the cell): center (global
/// squares), radius (squares), height (ft, may be negative), steepness 0 smooth..1 cliff-edged,
/// elongation along `dir`, and an irregular outline (three harmonics of the angle).
struct Relief {
    cx: f64,
    cy: f64,
    r: f64,
    h: f64,
    steep: f64,
    dir: (f64, f64),
    elong: f64,
    wobble: [(f64, f64); 3],
}

/// A city street's line nearest a square where the ground was shaped by hand (`key`: layout,
/// street and segment in the chunk's `layouts_near`), for steps on slopes made by hand.
#[derive(Clone)]
struct HandStreet {
    key: (u32, u32, u32),
    a: [f64; 2],
    b: [f64; 2],
    width: f64,
    d: f64,
}

/// A road crossing as drawn: along the road from bank to bank (the run over the channel at
/// the crossing angle, extended until both corners are 4 ft clear of every river piece): a
/// bridge deck with an 8-ft landing each side, a ford's stepping stones, or a ferry's line
/// between its jetties' ends 14 ft out in the water (`JETTY_FT` back to the banks), its raft
/// (centre, length, width ft) on the line's sagging rope 30% of the way over. Pure functions
/// of the world, so every chunk finds the same.
struct CrossingShape {
    kind: ShapeKind,
    a: [f64; 2],
    b: [f64; 2],
    /// Width (ft): the deck's, the road's, the raft's.
    size: f64,
    raft: Option<([f64; 2], f64, f64)>,
    /// Merged into another, or given way to it (`merge_bridges`).
    dropped: bool,
    /// A bridge of old stone (else timber): put down so by hand.
    stone: bool,
}

pub fn generate(world: &World, t0: &T0, key: &TileKey, tile: &TerrainOut) -> Chunk {
    let g = &world.geom;
    let sea = world.params().sea_level_ft;
    let (ox, oy) = g.tile_origin_ft(key);
    let (gsx, gsy) = (key.x as i64 * SQ as i64, key.y as i64 * SQ as i64);
    let n = SQ * SQ;
    let seed = world.stream("battlemap");

    // Square centers are odd lattice samples of the finest level (2.5 ft spacing). Square
    // (i, j) with i, j in -1..=SQ (halo included) is sample (2i + 1, 2j + 1) of the tile.
    let sidx = |i: i64, j: i64| ((2 * j + 1 + HALO as i64) as usize) * PADDED + (2 * i + 1 + HALO as i64) as usize;
    let mut hh = vec![0f32; HS * HS];
    let mut hw = vec![0f32; HS * HS];
    // Road class + 1 per halo square (0 = none); bridges where a road meets water.
    let mut road_h = vec![0u8; HS * HS];
    let mut lift = vec![0f32; HS * HS];
    // Where roads cross rivers, and how (see `RiverCrossing`).
    // Settlements added by hand: their pads and links over the terrain (`t0::made::Ground`), their
    // links' river crossings as the network's.
    let made = crate::t0::made::Ground::new(world, t0, [ox - 2.0 * SQUARE_FT, oy - 2.0 * SQUARE_FT, ox + (SQ + 2) as f64 * SQUARE_FT, oy + (SQ + 2) as f64 * SQUARE_FT]);
    let made_roads = made.as_ref().map_or(Vec::new(), |m| m.pieces());
    // (Roadmap 6 R5: the network's stretches removed by hand left out, their bridges and fords
    // with them; the ground beneath shows, as carved.)
    let kept = crate::lod::roads::kept(world, t0, tile, key);
    let (tile_roads, tile_mask): (&[crate::lod::roads::RoadPiece], &[u8]) = (&kept.roads, &kept.mask);
    let mut crossings = crate::lod::roads::river_crossings(tile_roads, &tile.pieces);
    crossings.extend(crate::lod::roads::river_crossings(&made_roads, &tile.pieces));
    let mut crossing_shapes: Vec<CrossingShape> = crossings.iter().map(|x| crossing_shape(x, tile)).collect();
    merge_bridges(&crossings, &mut crossing_shapes);
    let x_ft = |i: i64| ox + (i as f64 + 0.5) * SQUARE_FT;
    let y_ft = |j: i64| oy + (j as f64 + 0.5) * SQUARE_FT;
    // Ground shaped by hand, applied as heights are read (so whatever stands on the ground
    // follows it); `plat_w` is the plateaus' weight per halo square (they are kept flat), `moved`
    // where the shaping changed the ground (the slope is measured on the shaped ground there).
    // (R5 M20: and big ports' breakwaters, `town::harbour`.)
    let shape_rect = [ox - SQUARE_FT, oy - SQUARE_FT, ox + (SQ + 1) as f64 * SQUARE_FT, oy + (SQ + 1) as f64 * SQUARE_FT];
    let moles = crate::town::harbour::raised_near(world, t0, shape_rect);
    let shaper = crate::ground::Shaper::with_extra(&world.file.edits, t0, &moles, shape_rect, SQUARE_FT);
    let (mut plat_w, mut moved) = if shaper.is_some() { (vec![0f32; HS * HS], vec![false; HS * HS]) } else { (Vec::new(), Vec::new()) };
    // R5 M20: what the shaping makes of each halo square (an islet's rock, a quay's made ground;
    // empty when nothing here is either).
    let mut look_h = vec![crate::ground::Look::Ground; if shaper.as_ref().is_some_and(|s| s.has_looks()) { HS * HS } else { 0 }];
    // (P6: a plateau's rock floor, `Plateau::floor`: scree, no trees or ground cover on it.)
    let mut floor_h = vec![false; if shaper.as_ref().is_some_and(|s| s.has_floors()) { HS * HS } else { 0 }];
    // (A river's channel stays as the river carved it: a link crosses it on a bridge or a ford.)
    let base_at = |x: f64, y: f64, c: usize| made.as_ref().filter(|_| tile.river_water[c] <= tile.padded[c]).map_or(tile.padded[c], |m| m.at(x, y, tile.padded[c]));
    for j in -1..=SQ as i64 {
        for i in -1..=SQ as i64 {
            let (x, y) = (ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT);
            let c = sidx(i, j);
            let h = (j + 1) as usize * HS + (i + 1) as usize;
            hh[h] = base_at(x, y, c);
            hw[h] = t0.sample_water(x, y).max(tile.river_water[c]);
            if let Some(s) = &shaper {
                let (look, floor);
                (hh[h], plat_w[h], look, floor) = s.at_square_look(gsx + i, gsy + j, hh[h], hw[h]);
                if !floor_h.is_empty() {
                    floor_h[h] = floor;
                }
                // (Water drawn by hand: its surface over the bed `at_square_look` dug.)
                hw[h] = s.water(x, y, hw[h]);
                moved[h] = hh[h] != tile.padded[c] || plat_w[h] > 0.0;
                if !look_h.is_empty() && plat_w[h] >= 0.5 {
                    look_h[h] = look;
                }
            }
            road_h[h] = tile_mask.get(c).copied().unwrap_or(0);
            if road_h[h] == 0
                && let Some(class) = made.as_ref().and_then(|m| m.on_track(x, y))
            {
                road_h[h] = class as u8 + 1;
            }
            // A ford or a ferry leaves the water as it is (wading, or the raft).
            if road_h[h] != 0 && hw[h] > hh[h] && crossings.iter().any(|x| x.kind != CrossingKind::Bridge && x.covers([x_ft(i), y_ft(j)])) {
                road_h[h] = 0;
            }
            if road_h[h] != 0 && hw[h] > hh[h] {
                // The deck carries the road at its own level (its embankments reach the
                // banks), at least clear of the water.
                let top = (hw[h] + DECK_CLEARANCE_FT).max(road_level(tile_roads, [x, y]).or_else(|| road_level(&made_roads, [x, y])).unwrap_or(f32::MIN));
                lift[h] = top - hh[h];
                hh[h] = top;
                road_h[h] |= 0x80;
            }
        }
    }
    sea_ferry_tracks(world, t0, [ox - SQUARE_FT, oy - SQUARE_FT], &hh, &hw, &mut road_h);
    // Settlements: buildings (roofs at eave height), streets and plazas, fields.
    let mut bld_h = vec![0u16; HS * HS];
    let mut urb_h = vec![0u8; HS * HS];
    // v63 countryside: the fields' furrows per halo square (`town::fields::square_byte`).
    let mut fld_h = vec![0u8; HS * HS];
    // A village's green per halo square (1; 0 elsewhere): its cover is the green's own (`enforce`).
    let mut green_h = vec![0u8; HS * HS];
    // (R5 item 123: squares of a steppe town or one with yurts, packed dirt, not cobbles.)
    let mut dirt_sq = vec![false; HS * HS];
    // Ground kept clear round a way underground (its own ground stays: graveyard grass, ruin
    // dirt, street cobbles).
    let mut ent_h = vec![false; HS * HS];
    let mut crags: Vec<crate::under::surface::Crag> = Vec::new();
    // Squares on a flight of steps (only where a flight comes near the chunk).
    let mut stair_h: Option<Vec<bool>> = None;
    // Where the ground was shaped by hand round a city's streets: the street line nearest each such
    // square (steps follow the slopes made by hand; empty when nothing is shaped here).
    let mut hand_st: Vec<Option<HandStreet>> = Vec::new();
    let mut refs: Vec<(u32, u32)> = Vec::new();
    let mut polys: Vec<(Vec<[f32; 2]>, u8, u8, u8, [u8; 3])> = Vec::new();
    let mut poly_parts: Vec<Vec<(u8, Vec<[f32; 2]>)>> = Vec::new();
    let mut shapes: Vec<VectorShape> = Vec::new();
    let local = |p: &[f64; 2]| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32];
    let chunk_ft = SQ as f64 * SQUARE_FT;
    let owns = |p: [f64; 2]| p[0] >= ox && p[0] < ox + chunk_ft && p[1] >= oy && p[1] < oy + chunk_ft;
    let mut wells: Vec<[f64; 2]> = Vec::new();
    // Market stalls in blocks on town plazas (world ft, rotation, kind).
    let mut stalls: Vec<([f64; 2], f32, Kind)> = Vec::new();
    // R5 M20: a harbour's things (`dock_works`): world ft, rotation, kind, look, afloat.
    let mut dock: Vec<([f64; 2], f32, Kind, u8, bool)> = Vec::new();
    // Props set out on squares by what they are for (world ft, rotation, kind, variant).
    let mut dressing: Vec<([f64; 2], f32, Kind, u8)> = Vec::new();
    // Temples' churchyards in closes (their headstones are the close's dressing).
    let mut churchyards: Vec<Vec<[f64; 2]>> = Vec::new();
    // (R5 towns: the squares of cemeteries' links from their roads, kept up a bank like their walks.)
    let mut cem_links: Vec<usize> = Vec::new();
    // The capital's palace's court's squares (halo-indexed): a lawn, nothing added on it but what it
    // sets out.
    let mut court_h = vec![false; HS * HS];
    // Round raised features on town squares: (centre, radius ft, kind, steps angle).
    let mut features: Vec<([f64; 2], f64, PlazaFeature, f64)> = Vec::new();
    let mut in_town = vec![false; HS * HS];
    // R7 159: whose ground a district's square is (an index into `town_land`, from 1: the land's
    // biome round the town's middle, the same in every chunk).
    let mut town_ix = vec![0u8; HS * HS];
    let mut town_land: Vec<Biome> = Vec::new();
    let mut ruins: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut statues: Vec<[f64; 2]> = Vec::new();
    // Props a layout sets out itself (a camp's tents and fire).
    let mut props: Vec<crate::town::Prop> = Vec::new();
    // Gates' passages (their towers' centres, world ft): kept clear of generated objects.
    let mut gates: Vec<([f64; 2], [f64; 2])> = Vec::new();
    // Parks (`park_layout`): which squares are a park's (1 a common, 2 a formal garden) and which
    // park (an index into `park_biomes`, from 1: the biome its lawn takes, the same in every chunk),
    // their ponds, streams (centre line and water's level), raised tiers (middle, top radius), swells
    // (middle, radius, rise), formal terraces (outline, level), sunk parterres, what they set out,
    // ground the guarantee keeps clear, and each park's outline, benches' look and paths (for the
    // benches the guarantee adds).
    let mut park_h = vec![0u8; HS * HS];
    let mut park_ix = vec![0u8; HS * HS];
    let mut park_biomes: Vec<Biome> = Vec::new();
    let mut swells: Vec<crate::park::Swell> = Vec::new();
    let mut terraces: Vec<(Vec<[f64; 2]>, f32)> = Vec::new();
    let mut park_keep: Vec<([f64; 2], [f64; 2], f64)> = Vec::new();
    let mut ponds: Vec<crate::park::Pond> = Vec::new();
    let mut streams: Vec<Vec<([f64; 2], f32)>> = Vec::new();
    let mut mounds: Vec<([f64; 2], f64)> = Vec::new();
    let mut sunk: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut park_props: Vec<([f64; 2], f32, Kind, u8, f32)> = Vec::new();
    let mut park_meta: Vec<ParkMeta> = Vec::new();
    // Camps' clearings, levelled (outline, height ft).
    let mut yards: Vec<(Vec<[f64; 2]>, f32)> = Vec::new();
    // (Star walls' bastions over the chunk: the guarantee adds nothing on their tops, which have their own props.)
    let mut bastion_tops: Vec<Vec<[f64; 2]>> = Vec::new();
    // A shell keep's motte: the ground raised to its top round the keep, easing down to the ground
    // round it (never lowered); a gravel path up it to the shell's gate (`Motte::way`).
    // Castles' baileys: grass (squares marked).
    let mut motte_path = vec![false; HS * HS];
    let mut bailey_h = vec![false; HS * HS];
    // (What the guarantee may set out where: 1 a bailey's, 2 a garden yard's; 4 on a motte (but its
    // toe), nothing.)
    let mut bailey_kind = vec![0u8; HS * HS];
    // A bailey's dressing (`dress_bailey`: world ft, rotation, kind, look, scale).
    let mut bailey_props: Vec<([f64; 2], f32, Kind, u8, f32)> = Vec::new();
    let mut bailey_ways: Vec<([f64; 2], [f64; 2])> = Vec::new();
    // v63 countryside: the layouts here, for `town::rural` below.
    let mut rural_ls: Vec<std::rc::Rc<crate::town::Layout>> = Vec::new();
    // (The layouts round the chunk and its halo, once: the mottes, moats and walls below read them.)
    let near = crate::town::layouts_near(world, t0, [ox - SQUARE_FT, oy - SQUARE_FT, ox + (SQ + 1) as f64 * SQUARE_FT, oy + (SQ + 1) as f64 * SQUARE_FT]);
    for l in &near {
        rural_ls.push(l.clone());
        for m in &l.mottes {
            let (a, b) = (crate::town::geom::add(m.at, crate::town::geom::mul(m.way, m.top_r - 8.0)), crate::town::geom::add(m.at, crate::town::geom::mul(m.way, m.foot_r + 6.0)));
            for j in 0..HS {
                for i in 0..HS {
                    let p = [ox + (i as f64 - 0.5) * SQUARE_FT, oy + (j as f64 - 0.5) * SQUARE_FT];
                    if crate::town::geom::seg_dist(p, a, b) < 4.0 {
                        motte_path[j * HS + i] = true;
                    }
                    let d = crate::town::geom::dist(p, m.at);
                    // (The mound, but its toe, where the guarantee's cover may stand round it: what
                    // stands on a square by it not reaching past its last 8 ft.)
                    if d < m.foot_r - 4.0 {
                        bailey_kind[j * HS + i] |= 4;
                    }
                    if d >= m.foot_r {
                        continue;
                    }
                    let t = ((d - m.top_r) / (m.foot_r - m.top_r)).clamp(0.0, 1.0);
                    let k = (1.0 - t * t * (3.0 - 2.0 * t)) as f32;
                    let h = j * HS + i;
                    if hw[h] <= hh[h] {
                        hh[h] = hh[h].max(hh[h] + (m.top - hh[h]) * k);
                    }
                }
            }
        }
    }
    // Castles' moats, dug before anything stands on the ground, and their drawbridges (v63 castles).
    let moats: Vec<crate::town::moat::Moat> = near.iter().flat_map(|l| l.moats.clone()).collect();
    let dug = moat::dig(&moats, ox - SQUARE_FT, oy - SQUARE_FT, &mut hh, &mut hw, &mut road_h, &mut lift);
    for m in &moats {
        for b in &m.bridges {
            let far = crate::town::geom::add(b.at, crate::town::geom::mul(b.dir, b.span));
            if owns(crate::town::geom::lerp(b.at, far, 0.5)) {
                shapes.push(VectorShape { kind: ShapeKind::Drawbridge, size: (b.width / SQUARE_FT) as f32, pts: vec![local(&b.at), local(&far), [m.wet as u8 as f32, 0.0]] });
            }
        }
        // A dry ditch's two banks inked along their tops (by the chunk holding each stretch's
        // middle).
        for &(a, z, down, bank) in &m.banks {
            if owns(crate::town::geom::lerp(a, z, 0.5)) {
                shapes.push(VectorShape { kind: ShapeKind::DitchBank, size: 0.0, pts: vec![local(&a), local(&z), [down[0] as f32, down[1] as f32], [bank as u8 as f32, 0.0]] });
            }
        }
    }
    // Ground before anything is raised on it (walls stand on the ground, not on roofs).
    let ground_h = hh.clone();
    let (hx0, hy0) = (ox - SQUARE_FT, oy - SQUARE_FT);
    let span = HS as f64 * SQUARE_FT;
    let center = |i: usize, j: usize| [hx0 + (i as f64 + 0.5) * SQUARE_FT, hy0 + (j as f64 + 0.5) * SQUARE_FT];
    let bbox = bounds;
    // Halo squares a polygon's bbox may cover.
    let range = |b: (f64, f64, f64, f64)| {
        let i0 = (((b.0 - hx0) / SQUARE_FT - 0.5).floor().max(0.0)) as usize;
        let j0 = (((b.1 - hy0) / SQUARE_FT - 0.5).floor().max(0.0)) as usize;
        let i1 = (((b.2 - hx0) / SQUARE_FT - 0.5).ceil()).min(HS as f64 - 1.0);
        let j1 = (((b.3 - hy0) / SQUARE_FT - 0.5).ceil()).min(HS as f64 - 1.0);
        (i0, j0, i1, j1)
    };
    let overlaps = |b: (f64, f64, f64, f64)| b.2 >= hx0 && b.0 <= hx0 + span && b.3 >= hy0 && b.1 <= hy0 + span;
    // (R5 towns, 114: whether every town round the chunk is a steppe one, `steppe_only`.)
    let (mut steppe_any, mut steppe_all) = (false, true);
    // (R5 M20: town bridges' decks over an islet and their flat level; lined bridges' spans.)
    let mut islet_decks: Vec<(Vec<[f64; 2]>, f32)> = Vec::new();
    let mut lined_decks: Vec<Vec<[f64; 2]>> = Vec::new();
    // (Roadmap 6 R6: parks' hedges and manors' yard walls changed by hand, as whole runs (world ft)
    // and gates' leaves, for `town::rural` below; the eraser's gaps.)
    let mut hand_fences = crate::town::rural::Battle::default();
    let fence_gaps: Vec<crate::model::fences::FenceGap> = world.file.edits.fence_gaps.values().cloned().collect();
    for (li, l) in near.into_iter().enumerate() {
        // (Roadmap 6 R5: its streets, lanes, decks and park paths removed by hand, drawn nowhere.)
        let cut = crate::town::cut::Cut::of(world, t0, &l);
        // (Roadmap 6 R6: its fences and hedges changed by hand.)
        let fcut = crate::town::fence::FenceCut::of(world, t0, &l);
        let cut_deck = |k: usize| cut.as_ref().is_some_and(|c| c.deck(k));
        if l.culture % 5 == 4 {
            steppe_any = true;
        } else {
            steppe_all = false;
        }
        let stamp = |poly: &[[f64; 2]], val: u8, urb_h: &mut Vec<u8>| {
            let b = bbox(poly);
            if !overlaps(b) {
                return;
            }
            let (i0, j0, i1, j1) = range(b);
            if i1 < 0.0 || j1 < 0.0 {
                return;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    let h = j * HS + i;
                    // Anything more specific than a street wins (plazas, graveyards, ruins).
                    if (urb_h[h] == 0 || val != URBAN_STREET) && crate::town::geom::contains(poly, center(i, j)) {
                        urb_h[h] = val;
                    }
                }
            }
        };
        // Only streets are paved (stamped from the town's road lines below); yards and
        // gardens between buildings keep their natural ground, but no mounds or ditches.
        let mut land_ix = 0u8;
        for d in &l.districts {
            let b = bbox(d);
            if !overlaps(b) {
                continue;
            }
            let (i0, j0, i1, j1) = range(b);
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    if crate::town::geom::contains(d, center(i, j)) {
                        in_town[j * HS + i] = true;
                        if land_ix == 0 {
                            town_land.push(land_biome(t0, l.center));
                            land_ix = town_land.len().min(255) as u8;
                        }
                        town_ix[j * HS + i] = land_ix;
                    }
                }
            }
        }
        for d in l.bridges.iter().enumerate().filter(|(k, _)| !cut_deck(*k)).map(|(_, d)| d).chain(&l.piers) {
            stamp(d, URBAN_DECK, &mut urb_h);
        }
        // R5 M20: a bridge over one of the world's islets runs flat over it, at the islet's top
        // or clear of its water, whichever is higher (`islet_decks`, after the decks are lifted).
        for d in l.bridges.iter().enumerate().filter(|(k, d)| !cut_deck(*k) && overlaps(bbox(d))).map(|(_, d)| d) {
            let b = bbox(d);
            for isl in crate::t0::islets::near_in(t0, &world.file.edits, [b.0, b.1, b.2, b.3]).filter(|i| crate::town::geom::poly_gap(d, &i.pts) < 1.0) {
                islet_decks.push((d.clone(), isl.level.max(isl.water + DECK_CLEARANCE_FT)));
            }
        }
        // (R5 M20: a lined bridge's span, flat at its level, `Layout::lined`.)
        if let Some((d, top)) = l.lined.as_ref().filter(|(d, _)| overlaps(bbox(d))) {
            islet_decks.push((d.clone(), *top));
            lined_decks.push(d.clone());
        }
        for y in &l.yards {
            stamp(&y.0, URBAN_YARD, &mut urb_h);
            yards.push(y.clone());
        }
        // A star wall's bastions: their terreplein packed earth (no scatter on it; the walls round it
        // are raised over its edge below).
        for q in l.bastions.iter().filter(|q| overlaps(bbox(q))) {
            stamp(q, URBAN_YARD, &mut urb_h);
            bastion_tops.push(q.clone());
        }
        crate::town::village::stamp(&l, &mut |poly, val| stamp(poly, val, &mut urb_h));
        // R5 towns (115): bare trodden earth round a steppe town's yurts, on ground nothing else
        // claims (a street, a square, a park stamped after it wins).
        // (A pen's ground and a cart's trodden too, a little past them so no square they reach keeps
        // its grass; a village's croft under them trodden as well.)
        let yurt_props: Vec<[[f64; 2]; 4]> = l.props.iter().filter(|p| matches!(p.kind, Kind::Cart | Kind::Haystack) && l.yurts.iter().any(|y| crate::town::geom::contains(&y.lot, p.at))).map(|p| crate::town::horses::foot(p.kind, p.at, p.rot, p.scale)).collect();
        let grow = |b: [[f64; 2]; 4]| {
            let m = crate::town::geom::mean(&b);
            b.map(|q| {
                let d = crate::town::geom::sub(q, m);
                crate::town::geom::add(m, crate::town::geom::mul(d, 1.0 + 3.6 / crate::town::geom::len(d).max(1e-9)))
            })
        };
        let pens: Vec<[[f64; 2]; 4]> = l.yurts.iter().filter_map(|y| y.pen.map(|p| p.pts)).chain(yurt_props).map(grow).collect();
        for poly in l.yurts.iter().map(|y| y.trodden.as_slice()).chain(pens.iter().map(|p| p.as_slice())) {
            let b = bbox(poly);
            if !overlaps(b) {
                continue;
            }
            let (i0, j0, i1, j1) = range(b);
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    let h = j * HS + i;
                    if (urb_h[h] == 0 || urb_h[h] == URBAN_CROFT) && crate::town::geom::contains(poly, center(i, j)) {
                        urb_h[h] = URBAN_YARD;
                    }
                }
            }
        }
        let steppe_from = features.len();
        for pl in &l.plazas {
            use crate::town::PlazaKind as K;
            // (P2 160: a quay plaza over a front that gave way to hand work is its strip again.)
            let p_cow = crate::town::harbour::plaza_pts(world, &l, pl);
            let p = &*p_cow;
            // A town's green and a temple's close are lawns; a village's green keeps the ground
            // round it as it is.
            let lawn = pl.kind.is_lawn() && l.tier >= crate::t0::settle::Tier::Town;
            stamp(p, if lawn { URBAN_PARK } else { URBAN_PLAZA }, &mut urb_h);
            if !lawn && pl.kind != K::Bailey && crate::town::yurts::dirt_squares(&l) {
                let b = bbox(p);
                if overlaps(b) {
                    let (i0, j0, i1, j1) = range(b);
                    if i1 >= 0.0 && j1 >= 0.0 {
                        for j in j0..=j1 as usize {
                            for i in i0..=i1 as usize {
                                if crate::town::geom::contains(p, center(i, j)) {
                                    dirt_sq[j * HS + i] = true;
                                }
                            }
                        }
                    }
                }
            }
            if pl.kind == K::Green && !lawn {
                stamp(p, 1, &mut green_h);
            }
            // A castle's bailey: an open yard of grass, dressed sparingly (`dress_bailey`).
            if pl.kind == K::Bailey {
                let (i0, j0, i1, j1) = range(bbox(p));
                if overlaps(bbox(p)) && i1 >= 0.0 && j1 >= 0.0 {
                    let garden = dress_bailey(pl, &l, t0, seed, &mut bailey_props, &mut bailey_ways);
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            if crate::town::geom::contains(p, center(i, j)) {
                                bailey_h[j * HS + i] = true;
                                bailey_kind[j * HS + i] |= if garden { 2 } else { 1 };
                            }
                        }
                    }
                }
                continue;
            }
            // A quay takes cargo, no well or market (R5 M20: bollards along its edge, a city's cranes).
            if pl.kind == K::Quay {
                if overlaps(bbox(p)) {
                    quay_cargo(p, seed, &mut stalls);
                    // (A city's quay front: its face along the shore; any other quay: its side 0-1.)
                    let front = matches!(p_cow, std::borrow::Cow::Borrowed(_)).then(|| l.quays.iter().find(|q| q.pts == pl.pts)).flatten();
                    let edge = front.map_or(&p[..2], |q| q.face());
                    quay_works(p, edge, seed, &l.piers, l.tier >= crate::t0::settle::Tier::City, &mut dock);
                }
                continue;
            }
            // The main square and a village's green have a well at the heart; the other squares
            // are dressed by what they are for (`dress_square`).
            let main = pl.kind.is_market() || pl.kind == K::Civic || (pl.kind == K::Green && !lawn);
            if !l.site {
                if main {
                    wells.push(crate::town::geom::centroid(p));
                }
                if l.tier >= crate::t0::settle::Tier::Town && overlaps(bbox(p)) {
                    let first = stalls.len();
                    // Lanes the features keep off: the town's streets and roads, and the
                    // world's roads across the square. Only those near it (features stand in
                    // the square and are far smaller than 200 ft: farther lanes never stop one).
                    let b = bbox(p);
                    let near = |q: &[[f64; 2]], w: f64| {
                        let m = w + 200.0;
                        q[0][0].min(q[1][0]) <= b.2 + m && q[0][0].max(q[1][0]) >= b.0 - m && q[0][1].min(q[1][1]) <= b.3 + m && q[0][1].max(q[1][1]) >= b.1 - m
                    };
                    let mut lanes: Vec<([f64; 2], [f64; 2], f64)> = Vec::new();
                    for (pts, _, w) in &l.roads {
                        lanes.extend(pts.windows(2).filter(|q| near(q, 0.5 * w)).map(|q| (q[0], q[1], 0.5 * w)));
                    }
                    for st in &l.streets {
                        lanes.extend(st.windows(2).filter(|q| near(q, 5.0)).map(|q| (q[0], q[1], 5.0)));
                    }
                    for (ri, k) in t0.roads.segments_near([b.0 - 50.0, b.1 - 50.0, b.2 + 50.0, b.3 + 50.0], 0.0) {
                        let rc = &t0.roads.roads[ri as usize];
                        for j in 0..16 {
                            let a = rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p;
                            let e = rc.eval(k as usize, (j + 1) as f64 / 16.0, 5.0, t0.cell_ft).p;
                            lanes.push((a, e, 12.0));
                        }
                    }
                    let ffirst = features.len();
                    if !main {
                        dress_square(pl, &l, t0, &lanes, seed, &mut features, &mut dressing);
                        continue;
                    }
                    // A city's squares are laid out first, in a pattern round the well, and the
                    // market fits round them; a town's features go where the market leaves room.
                    let patterned = l.tier >= crate::t0::settle::Tier::City;
                    if patterned {
                        plaza_features(p, &l.monuments, &[], &lanes, seed, true, pl.kind == K::Civic, &mut features);
                    }
                    let mut keep: Vec<([f64; 2], f64)> = features[ffirst..].iter().map(|f| (f.0, f.1)).collect();
                    let mut room = Room::new(p, &l, &lanes, &keep, seed ^ 0xd3a1);
                    room.taken.push((crate::town::geom::centroid(p), 6.0));
                    // A market cross by the well, which the stalls keep clear of.
                    let cross = main_cross(pl, &mut room, seed);
                    if let Some(x) = cross {
                        keep.push((x.0, cross_ft()));
                        dressing.push(x);
                    }
                    if let Some(m) = Market::of(pl.kind) {
                        market_blocks(p, &l.monuments, &keep, m, seed, &mut stalls);
                    }
                    if !patterned {
                        // (Clear of the market cross as of a monument.)
                        let mut stands = l.monuments.clone();
                        stands.extend(cross.map(|x| crate::town::geom::circle(x.0, cross_ft(), 8)));
                        plaza_features(p, &stands, &stalls[first..], &lanes, seed, false, false, &mut features);
                    }
                    // Then round its edge: notices, troughs, lamps; a civic square's stocks.
                    room.taken.extend(features[ffirst..].iter().map(|f| (f.0, f.1 + 2.0)));
                    room.taken.extend(stalls[first..].iter().map(|s| (s.0, info(s.2 as u16).radius as f64 * SQUARE_FT + 3.0)));
                    dress_main(pl, &l, &mut room, seed, &mut dressing);
                }
            }
        }
        // (R5 towns, 114: a steppe town's squares raise no planted trees, a dark crown on the dust: a
        // dais in a planter's place.)
        if l.culture % 5 == 4 {
            for f in features[steppe_from..].iter_mut().filter(|f| f.2 == PlazaFeature::Planter) {
                f.2 = PlazaFeature::Dais;
            }
        }
        // Parks: a lawn (a formal garden's mown), gravel paths and rounds on it; its pond, stream
        // and raised tier, round features raised a tier and what it sets out (laid out whole by
        // every chunk it reaches, so they agree).
        for (pki, pk) in l.parks.iter().enumerate() {
            let b = bbox(&pk.pts);
            if !overlaps((b.0 - 20.0, b.1 - 20.0, b.2 + 20.0, b.3 + 20.0)) {
                continue;
            }
            // R5 towns (114): a horse park's ground trodden bare (a camp's clearing's), its corrals'
            // fences the countryside's (`town::rural`), what it sets out (`town::horses`, cover enough
            // for the guarantee; its knolls as on open ground).
            if let Some(hp) = crate::town::horses::plan(pk) {
                stamp(&pk.pts, URBAN_YARD, &mut urb_h);
                // (Its squares marked, `HORSE_PARK_MARK`: the guarantee's rise there an auction block.)
                let (i0, j0, i1, j1) = range(b);
                if i1 >= 0.0 && j1 >= 0.0 {
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            if crate::town::geom::contains(&pk.pts, center(i, j)) {
                                park_h[j * HS + i] |= HORSE_PARK_MARK;
                            }
                        }
                    }
                }
                park_keep.extend(hp.keep());
                park_props.extend(hp.props.iter().copied());
                continue;
            }
            let pb = land_biome(t0, crate::town::geom::centroid(&pk.pts));
            let mut lay = park_laid(&l, pki, pb, l.tier >= crate::t0::settle::Tier::City, seed);
            // (Roadmap 6 R6: its hedges and topiary rows changed by hand: gone, or laid again as runs.)
            if let Some(ch) = fcut.as_ref().and_then(|c| c.park(pki)) {
                crate::town::fence::park_change(std::rc::Rc::make_mut(&mut lay), ch, &mut hand_fences);
            }
            // (Roadmap 6 R5: its paths removed by hand left out of the gravel, the lawn closing over;
            // what stands by them stays.)
            let gone = cut.as_ref().and_then(|c| c.paths(pki));
            let paths: Vec<&(Vec<[f64; 2]>, f64)> = lay.paths.iter().enumerate().filter(|(j, _)| !gone.is_some_and(|g| g.contains(&(*j as u32)))).map(|(_, p)| p).collect();
            // R5 towns: a cemetery's ground is a churchyard's (its rows its own, nothing added), its
            // walks gravel, what it sets out on it (`town::cemetery`).
            if pk.kind == crate::town::ParkKind::Cemetery {
                stamp(&pk.pts, URBAN_GRAVEYARD, &mut urb_h);
                churchyards.push(pk.pts.clone());
                // (A foot and a half wider than drawn: a slanting walk stays a ribbon once the ground
                // smooths its squares.)
                for (a, e, half) in paths.iter().flat_map(|(pts, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w + 1.5))) {
                    let (i0, j0, i1, j1) = range((a[0].min(e[0]) - half, a[1].min(e[1]) - half, a[0].max(e[0]) + half, a[1].max(e[1]) + half));
                    if i1 < 0.0 || j1 < 0.0 {
                        continue;
                    }
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            let h = j * HS + i;
                            // (Its link from the road over open ground outside, `cemetery::Plan::link`.)
                            let inside = crate::town::geom::contains(&pk.pts, center(i, j));
                            if ((urb_h[h] == URBAN_GRAVEYARD && inside) || (urb_h[h] == 0 && road_h[h] == 0 && !inside)) && crate::town::geom::seg_dist(center(i, j), a, e) < half {
                                urb_h[h] = URBAN_PATH;
                                if !inside {
                                    cem_links.push(h);
                                }
                            }
                        }
                    }
                }
                park_keep.extend(lay.keep.iter().copied());
                park_props.extend(lay.props.iter().copied());
                continue;
            }
            stamp(&pk.pts, URBAN_PARK, &mut urb_h);
            park_biomes.push(pb);
            let (i0, j0, i1, j1) = range(b);
            if i1 >= 0.0 && j1 >= 0.0 {
                let mark = if pk.kind == crate::town::ParkKind::Formal { 2 } else { 1 };
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        if crate::town::geom::contains(&pk.pts, center(i, j)) {
                            park_h[j * HS + i] = mark;
                            park_ix[j * HS + i] = park_biomes.len().min(255) as u8;
                        }
                    }
                }
            }
            // A square is gravel where a path's or a round's edge reaches past its middle (a path a
            // little wider than drawn, so a slanting one stays joined), but under a gazebo.
            let reserved = |p: [f64; 2]| lay.reserved.iter().any(|&(c, r)| crate::town::geom::dist(p, c) < r);
            let segs = paths.iter().flat_map(|(pts, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w + 0.6)));
            for (a, e, half) in segs.chain(lay.rounds.iter().map(|&(c, r)| (c, c, r))) {
                let (i0, j0, i1, j1) = range((a[0].min(e[0]) - half, a[1].min(e[1]) - half, a[0].max(e[0]) + half, a[1].max(e[1]) + half));
                if i1 < 0.0 || j1 < 0.0 {
                    continue;
                }
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let h = j * HS + i;
                        if urb_h[h] == URBAN_PARK && park_h[h] != 0 && crate::town::geom::seg_dist(center(i, j), a, e) < half && !reserved(center(i, j)) {
                            urb_h[h] = URBAN_PATH;
                        }
                    }
                }
            }
            ponds.extend(lay.pond.clone());
            if !lay.stream.is_empty() {
                streams.push(lay.stream.clone());
            }
            mounds.extend(lay.mound);
            swells.extend(lay.swells.iter().copied());
            terraces.extend(lay.level.map(|v| (pk.pts.clone(), v)));
            park_keep.extend(lay.keep.iter().copied());
            sunk.extend(lay.sunk.iter().cloned());
            park_meta.push(ParkMeta {
                poly: pk.pts.clone(),
                bench: lay.bench,
                segs: lay.paths.iter().flat_map(|(pts, _)| pts.windows(2).map(|q| (q[0], q[1]))).collect(),
                rounds: lay.rounds.clone(),
                axis: lay.axis,
            });
            for &(c, r, kind) in &lay.features {
                features.push((c, r, kind, unit(hash3(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 0x9a5)) * std::f64::consts::TAU));
            }
            park_props.extend(lay.props.iter().copied());
        }
        for (fi, f) in l.fields.iter().enumerate() {
            stamp(f, URBAN_FIELD, &mut urb_h);
            if let Some(info) = l.field_info.get(fi) {
                stamp(f, crate::town::fields::square_byte(info), &mut fld_h);
            }
        }
        // Ways underground: the opening's ground stays clear; the chunk holding it draws it.
        // `size` = kind + 16 × style (0 plain, 1 a ruin's broken stair, 2 a graveyard's
        // mausoleum headhouse, 4-6 a wild site's by theme: `under::surface`).
        // (An entrance made in a building's cellar is drawn by the building: the same for each of the
        // layout's entrances, so asked once.)
        let in_building = !l.entrances.is_empty() && crate::sitelinks::building_of(world, t0, l.index as usize).is_some();
        for e in l.entrances.iter().filter(|_| !in_building) {
            use crate::town::geom::{add, mul};
            let in_yard = l.buildings.iter().any(|b| b.structure == crate::town::Structure::Open && crate::town::geom::contains(&b.poly, e.at));
            let style = match e.kind {
                crate::under::UnderKind::Dungeon | crate::under::UnderKind::Crypt => 1u8,
                crate::under::UnderKind::Catacombs if in_yard => 2,
                _ => 0,
            };
            // v63 underground: a wild site's way down by its theme (`under::surface`), and the crag
            // a dwarven hall's gate is cut into.
            let style = crate::under::surface::entrance_style(world, t0, &l, e).unwrap_or(style);
            if style == crate::under::surface::STYLE_RUNE_GATE
                && let Some(c) = crate::under::surface::Crag::new(t0, e)
                && overlaps(c.bbox())
            {
                crags.push(c);
            }
            let clear = if style == crate::under::surface::STYLE_BARROW {
                crate::under::surface::barrow_ground(e)
            } else if style == 2 {
                // The headhouse (4 × 3 squares) and a square round it.
                let side = [-e.dir[1], e.dir[0]];
                let at = |a: f64, b: f64| add(e.at, add(mul(e.dir, a * SQUARE_FT), mul(side, b * SQUARE_FT)));
                vec![at(-2.5, -2.5), at(3.5, -2.5), at(3.5, 2.5), at(-2.5, 2.5)]
            } else {
                // A street grate needs no clearing; stairs a little; mouths in the wild more.
                let r = match e.kind {
                    crate::under::UnderKind::Sewer => 3.0,
                    crate::under::UnderKind::Catacombs => 9.0,
                    _ => 18.0,
                };
                crate::town::geom::circle(e.at, r, 16)
            };
            let b = bbox(&clear);
            if overlaps(b) {
                let (i0, j0, i1, j1) = range(b);
                if i1 >= 0.0 && j1 >= 0.0 {
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            if crate::town::geom::contains(&clear, center(i, j)) {
                                ent_h[j * HS + i] = true;
                            }
                        }
                    }
                }
            }
            if owns(e.at) {
                let inward = add(e.at, mul(e.dir, SQUARE_FT));
                shapes.push(VectorShape { kind: ShapeKind::Entrance, size: (e.kind as u8 + 16 * style) as f32, pts: vec![local(&e.at), local(&inward)] });
            }
        }
        // Drain grates in the streets over the sewers (style 3, along the street), as each
        // sewer section works them out (`under::drain_grates`).
        if l.tier >= crate::t0::settle::Tier::City && !l.site {
            let s = crate::under::SEWER_SECTION_FT;
            let (si0, sj0) = (crate::core::floor(ox / s) as i64, crate::core::floor(oy / s) as i64);
            let (si1, sj1) = (crate::core::floor((ox + chunk_ft - 1e-6) / s) as i64, crate::core::floor((oy + chunk_ft - 1e-6) / s) as i64);
            for sj in sj0..=sj1 {
                for si in si0..=si1 {
                    for (at, dir) in crate::under::drain_grates(t0, &l, [si as f64 * s, sj as f64 * s]) {
                        if owns(at) {
                            let k = (((at[1] - hy0) / SQUARE_FT) as usize) * HS + ((at[0] - hx0) / SQUARE_FT) as usize;
                            ent_h[k] = true;
                            let along = crate::town::geom::add(at, crate::town::geom::mul(dir, SQUARE_FT));
                            shapes.push(VectorShape { kind: ShapeKind::Entrance, size: (crate::under::UnderKind::Sewer as u8 + 48) as f32, pts: vec![local(&at), local(&along)] });
                        }
                    }
                }
            }
        }
        // Streets drawn as roads (approaches to gates, main streets, bridge streets): street
        // ground wherever they run, inside the walls or out.
        let hand = shaper.is_some() && l.tier >= crate::town::stairs::STAIR_TIER;
        for (ri, (pts, _, width)) in l.roads.iter().enumerate() {
            if cut.as_ref().is_some_and(|c| c.road(ri)) {
                continue;
            }
            let half = 0.5 * width;
            for (si, seg) in pts.windows(2).enumerate() {
                let b = (seg[0][0].min(seg[1][0]) - half, seg[0][1].min(seg[1][1]) - half, seg[0][0].max(seg[1][0]) + half, seg[0][1].max(seg[1][1]) + half);
                if !overlaps(b) {
                    continue;
                }
                let (i0, j0, i1, j1) = range(b);
                if i1 < 0.0 || j1 < 0.0 {
                    continue;
                }
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let h = j * HS + i;
                        let d = crate::town::geom::seg_dist(center(i, j), seg[0], seg[1]);
                        if urb_h[h] != URBAN_DECK && d < half {
                            urb_h[h] = URBAN_STREET;
                            // An own square the shaping reaches (or its neighbours'), off the plazas.
                            if hand
                                && (1..=SQ).contains(&i)
                                && (1..=SQ).contains(&j)
                                && [h, h - 1, h + 1, h - HS, h + HS].iter().any(|&q| moved[q])
                                && hand_st.get(h).is_none_or(|r| r.as_ref().is_none_or(|r| d < r.d))
                                && !l.plazas.iter().any(|p| crate::town::geom::contains(&p.pts, center(i, j)))
                            {
                                if hand_st.is_empty() {
                                    hand_st = vec![None; HS * HS];
                                }
                                hand_st[h] = Some(HandStreet { key: (li as u32, ri as u32, si as u32), a: seg[0], b: seg[1], width: *width, d });
                            }
                        }
                    }
                }
            }
        }
        // Flights of steps: the squares of the street under them, square at their ends (a round
        // end would reach past the landing), never on a plaza (the street stamp paves over plazas).
        for st in l.stairs.iter().filter(|_| l.tier >= crate::town::stairs::STAIR_TIER) {
            let half = 0.5 * st.width;
            let last = st.pts.len() - 2;
            for (k, seg) in st.pts.windows(2).enumerate() {
                let b = (seg[0][0].min(seg[1][0]) - half, seg[0][1].min(seg[1][1]) - half, seg[0][0].max(seg[1][0]) + half, seg[0][1].max(seg[1][1]) + half);
                if !overlaps(b) {
                    continue;
                }
                let (i0, j0, i1, j1) = range(b);
                if i1 < 0.0 || j1 < 0.0 {
                    continue;
                }
                let plazas: Vec<&Vec<[f64; 2]>> = l.plazas.iter().map(|p| &p.pts).filter(|p| {
                    let q = bbox(p);
                    q.0 <= b.2 && q.2 >= b.0 && q.1 <= b.3 && q.3 >= b.1
                }).collect();
                let (u, len) = (crate::town::geom::sub(seg[1], seg[0]), crate::town::geom::dist(seg[0], seg[1]).max(1e-9));
                for j in j0..=j1 as usize {
                    for i in i0..=i1 as usize {
                        let c = center(i, j);
                        let t = crate::town::geom::dot(crate::town::geom::sub(c, seg[0]), u) / len;
                        if (k == 0 && t < 0.0) || (k == last && t > len) || crate::town::geom::seg_dist(c, seg[0], seg[1]) >= half {
                            continue;
                        }
                        if !plazas.iter().any(|p| crate::town::geom::contains(p, c)) {
                            stair_h.get_or_insert_with(|| vec![false; HS * HS])[j * HS + i] = true;
                        }
                    }
                }
            }
        }
        if l.tier >= crate::town::stairs::STAIR_TIER && !l.stairs.is_empty() {
            for ax in crate::town::stairs::axes(&l.stairs) {
                let half = 0.5 * ax.width;
                let b = (ax.a[0].min(ax.b[0]) - half, ax.a[1].min(ax.b[1]) - half, ax.a[0].max(ax.b[0]) + half, ax.a[1].max(ax.b[1]) + half);
                if overlaps(b) {
                    let pts = vec![local(&ax.a), local(&ax.b), [(ax.s0 / SQUARE_FT) as f32, (ax.tread / SQUARE_FT) as f32]];
                    shapes.push(VectorShape { kind: ShapeKind::Stairs, size: (ax.width / SQUARE_FT) as f32, pts });
                }
            }
        }
        if l.tier == crate::t0::settle::Tier::Village {
            // Village lanes (10 ft wide).
            for (lk, lane) in l.streets.iter().enumerate() {
                if cut.as_ref().is_some_and(|c| c.street(lk)) {
                    continue;
                }
                for seg in lane.windows(2) {
                    let b = (seg[0][0].min(seg[1][0]) - 5.0, seg[0][1].min(seg[1][1]) - 5.0, seg[0][0].max(seg[1][0]) + 5.0, seg[0][1].max(seg[1][1]) + 5.0);
                    if !overlaps(b) {
                        continue;
                    }
                    let (i0, j0, i1, j1) = range(b);
                    if i1 < 0.0 || j1 < 0.0 {
                        continue;
                    }
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            if crate::town::geom::seg_dist(center(i, j), seg[0], seg[1]) < 5.0 && urb_h[j * HS + i] != URBAN_PLAZA {
                                urb_h[j * HS + i] = URBAN_STREET;
                            }
                        }
                    }
                }
            }
        }
        // The capital's palace's court and forecourt (`interior::court_plan`), where the chunk meets
        // them: the forecourt paved, nothing added on it, its walls and the gate's piers raised with
        // the town's walls below. (A coaching inn's carriage arch with its yard, from the one plan:
        // `interior::court_and_arch`.)
        let mut courts: Vec<(u32, crate::interior::Court, Option<Vec<[f64; 2]>>)> = Vec::new();
        let mut fore_walls: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let mut fore_piers: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let mut fore_gates: Vec<[[f64; 2]; 3]> = Vec::new();
        let mut walks: Vec<[[f64; 2]; 4]> = Vec::new();
        match &fcut {
            Some(c) => fore_walls.extend(crate::town::fence::garden_walls(crate::town::village::wall_runs(&l), c, &mut hand_fences.gates)),
            None => fore_walls.extend(crate::town::village::wall_runs(&l)),
        }
        for b in l.buildings.iter().filter(|b| crate::town::palace::merges(&l, b)) {
            let bb = bbox(&b.poly);
            if !overlaps((bb.0 - 40.0, bb.1 - 40.0, bb.2 + 40.0, bb.3 + 40.0)) {
                continue;
            }
            let Some((c, arch)) = crate::interior::court_and_arch(world, t0, &l, b) else { continue };
            if let Some(fc) = &c.forecourt {
                stamp(&fc.yard, URBAN_PLAZA, &mut urb_h);
                let (fi0, fj0, fi1, fj1) = range(bbox(&fc.yard));
                if fi1 >= 0.0 && fj1 >= 0.0 {
                    for j in fj0..=fj1 as usize {
                        for i in fi0..=fi1 as usize {
                            if crate::town::geom::contains(&fc.yard, center(i, j)) {
                                court_h[j * HS + i] = true;
                            }
                        }
                    }
                }
                fore_walls.extend(fc.walls.iter().copied());
                fore_piers.extend(fc.piers.iter().map(|&p| (p, fc.along)));
                fore_gates.push(fc.gate);
            }
            courts.push((b.id, c, arch));
        }
        for b in &l.buildings {
            let bb = bbox(&b.poly);
            if !overlaps(bb) {
                continue;
            }
            match b.structure {
                crate::town::Structure::Roofed => {}
                crate::town::Structure::Open => {
                    stamp(&b.poly, URBAN_GRAVEYARD, &mut urb_h);
                    // A close's churchyard has its rows laid along it (`dress_square`).
                    let c = crate::town::geom::centroid(&b.poly);
                    if l.plazas.iter().any(|p| p.kind == crate::town::PlazaKind::TempleClose && crate::town::geom::contains(&p.pts, c)) {
                        churchyards.push(b.poly.clone());
                    }
                    continue;
                }
                crate::town::Structure::Ruin => {
                    stamp(&b.poly, URBAN_RUIN, &mut urb_h);
                    ruins.push(b.poly.clone());
                    continue;
                }
            }
            let (i0, j0, i1, j1) = range(bb);
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            let mut id = 0u16;
            // (Its ground floor rides on ground shaped by hand: `ground::pad_shift`.)
            let mut pad = b.pad_ft;
            // (The capital's palace's court open to the sky: not the building's. A lawn with gravel
            // walks, its fountain and its benches, as its ground floor has them:
            // `interior::court_plan`.)
            let (court, mut arch) = match courts.iter().position(|c| c.0 == b.id) {
                Some(k) => {
                    let (_, c, a) = courts.swap_remove(k);
                    (Some(c), Some(a))
                }
                None => (None, None),
            };
            if let Some(c) = &court {
                // (A coaching inn's yard cobbled as the street.)
                stamp(&c.court, if c.paved { URBAN_PLAZA } else { URBAN_PARK }, &mut urb_h);
                let (ci0, cj0, ci1, cj1) = range(bbox(&c.court));
                if ci1 >= 0.0 && cj1 >= 0.0 {
                    for j in cj0..=cj1 as usize {
                        for i in ci0..=ci1 as usize {
                            if crate::town::geom::contains(&c.court, center(i, j)) {
                                court_h[j * HS + i] = true;
                            }
                        }
                    }
                }
                // (The walks drawn as the ground floor has them, straight-edged on their squares: on
                // the lawn, walked over; the fountain its size there, a square's a basin.)
                for (kind, q, back) in &c.pieces {
                    let m = crate::town::geom::mean(q);
                    match *kind {
                        "garden_path" => walks.push(*q),
                        "fountain" => park_props.push((m, 0.0, Kind::Fountain, 0, (0.5 * crate::town::geom::dist(q[0], q[1]) / SQUARE_FT).max(0.75) as f32)),
                        // (A monastery's garth: its cloister well in its middle, its size there.)
                        "cloister_well" => park_props.push((m, 0.0, Kind::Well, 0, (0.8 * crate::town::geom::dist(q[0], q[1]).min(crate::town::geom::dist(q[1], q[2])) / SQUARE_FT).max(0.8) as f32)),
                        "bench" => dressing.push((m, crate::park::facing(back.map_or([0.0, 1.0], |d| [-d[0], -d[1]])), Kind::Bench, 0)),
                        // (A coaching inn's yard: its trough along its box, its barrels, its cart.)
                        "water_trough" | "cart" | "barrels" => {
                            let (a, e) = (crate::town::geom::sub(q[1], q[0]), crate::town::geom::sub(q[3], q[0]));
                            let long = if crate::town::geom::dot(a, a) >= crate::town::geom::dot(e, e) { a } else { e };
                            let kind = match *kind {
                                "water_trough" => Kind::WaterTrough,
                                "cart" => Kind::Cart,
                                _ => Kind::Barrel,
                            };
                            dressing.push((m, crate::park::facing([-long[1], long[0]]), kind, 0));
                        }
                        _ => {}
                    }
                }
            }
            let (court, church) = court.map_or((None, None), |c| (Some(c.court), c.church));
            for j in j0..=j1 as usize {
                for i in i0..=i1 as usize {
                    if crate::town::geom::contains(&b.poly, center(i, j)) && !court.as_ref().is_some_and(|q| crate::town::geom::contains(q, center(i, j))) {
                        if id == 0 {
                            // (R5 M20: a building on a deck stands at the deck's level.)
                            if b.deck.is_none() {
                                pad += crate::ground::pad_shift(&world.file.edits, t0, &b.poly, b.pad_ft);
                            }
                            refs.push((l.index, b.id));
                            let battlements = crate::interior::battlements(b);
                            // (A castle's keep with no pieces: its top in the high two bits.)
                            let top = if battlements.is_some() { crate::town::plain_keep_top(b, world.stream("keep.tops")) } else { 0 };
                            polys.push((
                                b.poly.iter().map(|p| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32]).collect(),
                                b.floors,
                                roof_byte(b, battlements.is_some()),
                                crate::core::round(battlements.unwrap_or(0.0) / SQUARE_FT) as u8 | top << 6,
                                look_bytes(&l, b),
                            ));
                            // (Each piece's code, how it is topped above it: `town::keep_tops`.)
                            let tops = crate::town::keep_tops(b, world.stream("keep.tops"));
                            let mut parts: Vec<(u8, Vec<[f32; 2]>)> = b.parts.iter().zip(&tops).map(|(p, &top)| (part_code(p.kind) | top << 4, p.poly.iter().map(local).collect())).collect();
                            parts.extend(court.as_ref().map(|q| (COURT_PART, q.iter().map(local).collect())));
                            parts.extend(church.as_ref().map(|q| (CHURCH_PART, q.iter().map(local).collect())));
                            // (A manor: its main range and its cross wing, roofed apart.)
                            if let Some((main, wing)) = crate::interior::cross_wing_of(world, t0, &l, b) {
                                parts.push((MAIN_RANGE_PART, main.iter().map(local).collect()));
                                parts.push((CROSS_WING_PART, wing.iter().map(local).collect()));
                            }
                            // (A city's town hall on its arcade: its piers along the front.)
                            // (A coaching inn's carriage arch likewise: its piers either side of it.)
                            // (Its arch found with its court, else `arch_front`.)
                            if let Some(front) = crate::interior::arcade_front(world, t0, &l, b).or_else(|| arch.take().unwrap_or_else(|| crate::interior::arch_front(world, t0, &l, b))) {
                                parts.push((ARCADE_PART, front.iter().map(local).collect()));
                            }
                            // (An L, a U or a court: its ranges, `town::wings`.)
                            for q in crate::town::wings::ranges(b).into_iter().flatten() {
                                parts.push((RANGE_PART, q.iter().map(local).collect()));
                            }

                            poly_parts.push(parts);
                            id = refs.len().min(u16::MAX as usize) as u16;
                        }
                        let h = j * HS + i;
                        bld_h[h] = id;
                        lift[h] = pad + STOREY_FT * b.floors as f32 - hh[h];
                        hh[h] = pad + STOREY_FT * b.floors as f32;
                    }
                }
            }
        }
        // Walls (8 ft thick), towers and gates, a palace's forecourt (`walls::build`): raised stone,
        // then the same as vectors.
        walls::build(
            &walls::Ctx { l: &l, seed, hx0, hy0, bld_h: &bld_h, hw: &hw, ground_h: &ground_h, fence_gaps: &fence_gaps, fore_piers: &fore_piers, fore_gates: &fore_gates, walks: &walks },
            walls::Halo { hh: &mut hh, lift: &mut lift, urb_h: &mut urb_h, shapes: &mut shapes, gates: &mut gates },
            fore_walls,
            center,
            range,
            overlaps,
            owns,
            local,
        );
        // Deck style: 0 pier, else `bridge_style`.
        let style = |d: &Vec<[f64; 2]>| bridge_style(seed, &l, d);
        for (d, size) in l.bridges.iter().enumerate().filter(|(k, _)| !cut_deck(*k)).map(|(_, d)| (d, style(d))).chain(l.piers.iter().map(|d| (d, 0.0))) {
            if owns(crate::town::geom::centroid(d)) {
                shapes.push(VectorShape { kind: ShapeKind::Deck, size, pts: d.iter().map(local).collect() });
            }
        }
        // R5 M20: a port's breakwater armoured where water meets it, its lighthouse's lantern, the
        // boats moored by its piers, bollards along them.
        for m in &l.moles {
            armour(&m.pts, &m.edges(crate::town::harbour::MOLE_HEAD_FT), &hh, &hw, hx0, hy0, &owns, &local, &mut shapes);
        }
        // P2 160: a timber quay front planked over (a stone one is the quay's paving).
        for q in crate::town::harbour::quays_kept(world, &l).filter(|q| q.timber && owns(crate::town::geom::centroid(&q.pts))) {
            shapes.push(VectorShape { kind: ShapeKind::Deck, size: 0.0, pts: q.band().iter().map(local).collect() });
        }
        for b in l.buildings.iter().filter(|b| crate::town::harbour::is_lighthouse(b)) {
            let c = crate::town::geom::mean(&b.poly);
            if owns(c) {
                shapes.push(VectorShape { kind: ShapeKind::Lantern, size: (6.0 / SQUARE_FT) as f32, pts: vec![local(&c)] });
            }
        }
        for m in &l.moored {
            use crate::town::harbour::Afloat;
            let look = (hash2(seed ^ 0xb0a7, crate::core::round(m.at[0]) as i64, crate::core::round(m.at[1]) as i64) >> 20) as u8 & 3;
            match m.kind {
                Afloat::Cog => {
                    if owns(m.at) {
                        let bow = [m.at[0] + libm::cos(m.rot as f64) * SQUARE_FT, m.at[1] + libm::sin(m.rot as f64) * SQUARE_FT];
                        shapes.push(VectorShape { kind: ShapeKind::Ship, size: m.length / SQUARE_FT as f32, pts: vec![local(&m.at), local(&bow), [(look & 1) as f32, 0.0]] });
                    }
                }
                Afloat::Rowboat => dock.push((m.at, m.rot, Kind::Rowboat, look, true)),
                Afloat::FishingBoat => dock.push((m.at, m.rot, Kind::FishingBoat, look, true)),
                Afloat::Buoy => dock.push((m.at, 0.0, Kind::Buoy, look, true)),
            }
        }
        for d in l.piers.iter().filter(|d| overlaps(bbox(d))) {
            pier_bollards(d, seed, &mut dock);
        }
        props.extend(l.props.iter().copied());
        for q in l.bastions.iter().filter(|q| overlaps(bbox(q))) {
            props.extend(crate::town::wallside::bastion_props(q, l.wall_style_at(crate::town::geom::centroid(q)).thickness()));
        }
        for m in &l.monuments {
            statues.push(crate::town::geom::centroid(m));
        }
    }
    // The hand-built part (`hand::build`): what stands along water drawn by hand, quays, piers and
    // breakwaters built by hand, crossings put down by hand, roads drawn by hand, decks over
    // water, and the road crossings drawn as vectors.
    let hand::Built { quay_h, path_near, path_h } = hand::build(
        &hand::Ctx { world, t0, seed, sea, hx0, hy0, span, shaper: &shaper, crossings: &crossings, crossing_shapes: &crossing_shapes, islet_decks: &islet_decks, lined_decks: &lined_decks, look_h: &look_h, bld_h: &bld_h, hw: &hw },
        hand::Halo { hh: &mut hh, lift: &mut lift, road_h: &mut road_h, urb_h: &mut urb_h, shapes: &mut shapes, dock: &mut dock, stalls: &mut stalls },
        center,
        range,
        overlaps,
        owns,
        local,
    );
    let road_bed = hh.clone();
    let mut biome = vec![Biome::Grassland; n];
    let mut slope = vec![0f64; n];
    // Steps on slopes made by hand, by street segment and way up: (segment start, its direction,
    // street width, the squares' places along it).
    let mut hand_runs: std::collections::BTreeMap<((u32, u32, u32), bool), ([f64; 2], [f64; 2], f64, Vec<f64>)> = std::collections::BTreeMap::new();
    for j in 0..SQ {
        for i in 0..SQ {
            let k = j * SQ + i;
            let (x, y) = (ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT);
            biome[k] = Biome::from_u8(t0.sample_biome(x, y, t0.biome_warp(x, y))[0]);
            let c = sidx(i as i64, j as i64);
            let h = (j + 1) * HS + i + 1;
            // (Over a created settlement's pad or track, from the padded heights.)
            let plain = || match &made {
                Some(_) => {
                    let at = |dx: f64, dy: f64, c: usize| base_at(x + dx, y + dy, c);
                    ((at(2.5, 0.0, c + 1) - at(-2.5, 0.0, c - 1)) as f64 / 5.0, (at(0.0, 2.5, c + PADDED) - at(0.0, -2.5, c - PADDED)) as f64 / 5.0)
                }
                None => ((tile.padded[c + 1] - tile.padded[c - 1]) as f64 / 5.0, (tile.padded[c + PADDED] - tile.padded[c - PADDED]) as f64 / 5.0),
            };
            let (gx, gy) = match &shaper {
                Some(s) if [h, h - 1, h + 1, h - HS, h + HS].iter().any(|&q| moved[q]) => {
                    let at = |dx: f64, dy: f64, c: usize| s.at_point(x + dx, y + dy, base_at(x + dx, y + dy, c), hw[h]) as f64;
                    ((at(2.5, 0.0, c + 1) - at(-2.5, 0.0, c - 1)) / 5.0, (at(0.0, 2.5, c + PADDED) - at(0.0, -2.5, c - PADDED)) / 5.0)
                }
                _ => plain(),
            };
            slope[k] = crate::core::sqrt(gx * gx + gy * gy);
            // Steps on a city street's slope made by hand, or paving again where shaping levelled a
            // flight, judged by the climb along the street (square by square: a pure function of
            // the square, so chunks agree).
            if let Some(Some(st)) = hand_st.get(h) {
                let len = crate::town::geom::dist(st.a, st.b).max(1e-9);
                let u = [(st.b[0] - st.a[0]) / len, (st.b[1] - st.a[1]) / len];
                let (px, py) = plain();
                let (along, was) = ((gx * u[0] + gy * u[1]).abs(), (px * u[0] + py * u[1]).abs());
                let prm = crate::town::stairs::StairParams::DEFAULT;
                let flight = stair_h.as_ref().is_some_and(|s| s[h]);
                // (A flight by the slope's whole grade: its square's nearest line may be a street
                // crossing it. A plateau's top is flat, steps or not before.)
                if flight && slope[k] < prm.g_off && (plat_w[h] >= 0.5 || slope[k] < crate::core::sqrt(px * px + py * py) - HAND_MARGIN) {
                    stair_h.as_mut().expect("a flight")[h] = false;
                } else if !flight && along >= prm.g_on && along >= was + HAND_MARGIN && slope[k] < HAND_CLIFF && urb_h[h] == URBAN_STREET && road_h[h] == 0 && hw[h] <= hh[h] {
                    stair_h.get_or_insert_with(|| vec![false; HS * HS])[h] = true;
                    let t = crate::town::geom::dot([x - st.a[0], y - st.a[1]], u);
                    let up = gx * u[0] + gy * u[1] > 0.0;
                    hand_runs.entry((st.key, up)).or_insert((st.a, u, st.width, Vec::new())).3.push(t);
                }
            }
        }
    }

    // Their treads, square across the street from the bottom up, in step across chunks and across
    // a street's strips side by side (counted along the way up from the world's origin). Each run
    // (a segment's steps, split where they stop for more than a square) is `[along, across]`
    // ranges in world ft on its way up `w` and the normal `[-w.y, w.x]`; runs on one way up side
    // by side or end to end are drawn as one axis.
    let mut runs: Vec<([f64; 2], [f64; 2], [f64; 2])> = Vec::new();
    for (&(_, up), (a, u, width, ts)) in &hand_runs {
        let w = if up { *u } else { [-u[0], -u[1]] };
        let off = crate::town::geom::dot(*a, [-w[1], w[0]]);
        let mut ts = ts.clone();
        ts.sort_by(f64::total_cmp);
        let along = |t: f64| crate::town::geom::dot([a[0] + u[0] * t, a[1] + u[1] * t], w);
        let mut first = 0;
        for k in 1..=ts.len() {
            if k == ts.len() || ts[k] - ts[k - 1] > 1.5 * SQUARE_FT {
                let (s0, s1) = (along(ts[first]), along(ts[k - 1]));
                runs.push((w, [s0.min(s1), s0.max(s1)], [off - 0.5 * width, off + 0.5 * width]));
                first = k;
            }
        }
    }
    let mut group: Vec<usize> = (0..runs.len()).collect();
    fn root(g: &[usize], mut i: usize) -> usize {
        while g[i] != i {
            i = g[i];
        }
        i
    }
    let gap = |a: [f64; 2], b: [f64; 2]| (a[0] - b[1]).max(b[0] - a[1]);
    for i in 0..runs.len() {
        for j in i + 1..runs.len() {
            let (p, q) = (&runs[i], &runs[j]);
            if (p.0[0] - q.0[0]).abs() < 1e-3 && (p.0[1] - q.0[1]).abs() < 1e-3 && gap(p.1, q.1) <= 1.5 * SQUARE_FT && gap(p.2, q.2) <= 1.0 {
                let (a, b) = (root(&group, i), root(&group, j));
                group[a.max(b)] = a.min(b);
            }
        }
    }
    let tread = crate::town::stairs::TREAD;
    for r in (0..runs.len()).filter(|&r| root(&group, r) == r) {
        let w = runs[r].0;
        let nrm = [-w[1], w[0]];
        let (mut s, mut o) = ([f64::MAX, f64::MIN], [f64::MAX, f64::MIN]);
        for q in (0..runs.len()).filter(|&q| root(&group, q) == r) {
            s = [s[0].min(runs[q].1[0]), s[1].max(runs[q].1[1])];
            o = [o[0].min(runs[q].2[0]), o[1].max(runs[q].2[1])];
        }
        let (lo, hi, mid) = (s[0] - 0.75 * SQUARE_FT, s[1] + 0.75 * SQUARE_FT, 0.5 * (o[0] + o[1]));
        let at = |t: f64| [w[0] * t + nrm[0] * mid, w[1] * t + nrm[1] * mid];
        let s0 = lo - crate::core::floor(lo / tread) * tread;
        shapes.push(VectorShape { kind: ShapeKind::Stairs, size: ((o[1] - o[0]) / SQUARE_FT) as f32, pts: vec![local(&at(lo)), local(&at(hi)), [(s0 / SQUARE_FT) as f32, (tread / SQUARE_FT) as f32]] });
    }

    // Micro-relief from global feature cells (dry ground only).
    let rseed = hash2(seed, 0x5e11, 1);
    let (cx0, cy0) = (gsx.div_euclid(RELIEF_CELL) - 1, gsy.div_euclid(RELIEF_CELL) - 1);
    let (cx1, cy1) = ((gsx + SQ as i64).div_euclid(RELIEF_CELL) + 1, (gsy + SQ as i64).div_euclid(RELIEF_CELL) + 1);
    let (relief_base, mut reliefs) = (hh.clone(), Vec::new());
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            // Biome at the feature cell's true world position (not clamped into this chunk),
            // so a feature straddling two chunks is identical in both.
            let (wx, wy) = (((cx * RELIEF_CELL + RELIEF_CELL / 2) as f64 + 0.5) * SQUARE_FT, ((cy * RELIEF_CELL + RELIEF_CELL / 2) as f64 + 0.5) * SQUARE_FT);
            let b = Biome::from_u8(t0.sample_biome(wx, wy, t0.biome_warp(wx, wy))[0]);
            for f in relief_in_cell(rseed, cx, cy, b) {
                apply_relief(&f, &mut hh, &hw, gsx - 1, gsy - 1, HS);
                reliefs.push(f);
            }
        }
    }
    let soft = soft_relief::mask(&reliefs, &relief_base, gsx - 1, gsy - 1);
    // Micro-relief never lifts or digs a road bed, a building, a street, a field or the ground
    // at a way underground.
    let protected: Vec<bool> =
        (0..HS * HS).map(|h| road_h[h] != 0 || bld_h[h] != 0 || in_town[h] || ent_h[h] || matches!(urb_h[h], URBAN_STREET | URBAN_PLAZA | URBAN_WALL | URBAN_TOWER | URBAN_TOWER_FLOOR | URBAN_FIELD | URBAN_YARD)).collect();
    for h in 0..HS * HS {
        if protected[h] {
            hh[h] = road_bed[h];
        }
    }
    // Plateaus are flat: micro-relief fades out over them (empty when nothing is shaped here).
    for (h, &w) in plat_w.iter().enumerate() {
        if w > 0.0 {
            hh[h] = road_bed[h] + (hh[h] - road_bed[h]) * (1.0 - w);
        }
    }
    // v63 underground: a crag raised behind a dwarven hall's gate on flat ground (its rock drawn
    // below); buildings keep their pads.
    let mut crag_h = vec![0f32; if crags.is_empty() { 0 } else { HS * HS }];
    // Per square: 1 the face or its rim (rock), 2 by the face (inked; the rest of the crag soft).
    let mut crag_m = vec![0u8; crag_h.len()];
    let mut crag_w = vec![0f32; crag_h.len()];
    for c in &crags {
        let (i0, j0, i1, j1) = range(c.bbox());
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let p = center(i, j);
                if bld_h[h] == 0 {
                    crag_h[h] += c.lift(p) as f32;
                    crag_w[h] = crag_w[h].max(c.apron(p) as f32);
                }
                crag_m[h] |= u8::from(c.rock(p)) | 2 * u8::from(c.by_face(p));
            }
        }
    }
    // The micro-relief under it and just past its back fades out (no hollow cut into it).
    for (h, &w) in crag_w.iter().enumerate().filter(|&(_, &w)| w > 0.0) {
        hh[h] += crag_h[h] - w * (hh[h] - relief_base[h]);
    }
    // A camp's clearing is level: the ground eases to one height from just past the rim to
    // three quarters of the way in (so the tents stand on one tier). That height is the mean
    // of the ground in it when the chunk holds it all (camps are placed so), else the layout's
    // (moved as a building's pad is by ground shaped by hand).
    for (poly, level) in &yards {
        let c = crate::town::geom::centroid(poly);
        let reach = poly.iter().map(|q| crate::town::geom::dist(*q, c)).fold(0.0, f64::max);
        let b = (c[0] - 1.2 * reach, c[1] - 1.2 * reach, c[0] + 1.2 * reach, c[1] + 1.2 * reach);
        if !overlaps(b) {
            continue;
        }
        let (i0, j0, i1, j1) = range(b);
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        let whole = b.0 >= ox && b.1 >= oy && b.2 < ox + chunk_ft && b.3 < oy + chunk_ft;
        let level = if whole {
            let inside: Vec<f32> = (j0..=j1 as usize).flat_map(|j| (i0..=i1 as usize).map(move |i| (i, j))).filter(|&(i, j)| crate::town::geom::contains(poly, center(i, j))).map(|(i, j)| hh[j * HS + i]).collect();
            if inside.is_empty() { *level } else { inside.iter().sum::<f32>() / inside.len() as f32 }
        } else {
            *level + crate::ground::pad_shift(&world.file.edits, t0, poly, *level)
        };
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let t = ((crate::town::geom::dist(center(i, j), c) / reach - 0.75) / 0.4).clamp(0.0, 1.0);
                let w = 1.0 - t * t * (3.0 - 2.0 * t);
                if w > 0.0 && road_h[h] == 0 && bld_h[h] == 0 {
                    hh[h] += (level - hh[h]) * w as f32;
                }
            }
        }
    }
    // Parks' ground where their features stand, each square by its place alone (chunks agree): a
    // common's swells (easing out over their radius); a formal garden laid level (eased to the
    // streets over 24 ft in, cut or filled no more than a quarter of the way in, to 8 ft), its raised
    // features then standing on the terrace.
    let at_ease = |h: usize| road_h[h] == 0 && bld_h[h] == 0;
    let mut terraced = vec![false; if terraces.is_empty() { 0 } else { HS * HS }];
    for sw in &swells {
        let (c, r) = (sw.at, sw.reach());
        let (i0, j0, i1, j1) = range((c[0] - r, c[1] - r, c[0] + r, c[1] + r));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                if at_ease(h) && hw[h] <= hh[h] {
                    hh[h] += sw.lift(center(i, j));
                }
            }
        }
    }
    for (pts, level) in &terraces {
        let (i0, j0, i1, j1) = range(bbox(pts));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let din = crate::park::depth(pts, center(i, j));
                if din > 0.0 && at_ease(h) && hw[h] <= hh[h] {
                    // (Cut or filled no more than a quarter of the way in, to 8 ft: where the ground
                    // drawn strays from the plan, the terrace eases to it.)
                    let cap = (0.25 * din).min(8.0) as f32;
                    let target = level.clamp(hh[h] - cap, hh[h] + cap);
                    hh[h] += (target - hh[h]) * smoothstep(0.0, 24.0, din) as f32;
                    hw[h] = hw[h].min(hh[h] - 4.0);
                    terraced[h] = true;
                }
            }
        }
    }
    // Plaza features stand a tier above the paving (a fountain's rim, a planter, a cross's
    // steps), a formal garden's above its terrace: every square whose centre lies within the round.
    let floor = if terraced.is_empty() { Vec::new() } else { hh.clone() };
    for &(fc, r, _, _) in &features {
        let b = (fc[0] - r, fc[1] - r, fc[0] + r, fc[1] + r);
        if !overlaps(b) {
            continue;
        }
        let (i0, j0, i1, j1) = range(b);
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                if crate::town::geom::dist(center(i, j), fc) <= r && road_h[h] == 0 && bld_h[h] == 0 {
                    hh[h] = if terraced.get(h).is_some_and(|&t| t) { floor[h] } else { road_bed[h] } + (TIER_FT * 1.05) as f32;
                }
            }
        }
    }
    // Then by how far it is from the water's edge, a pond dug to its level (0.3 ft deep at its rim,
    // 0.2 ft deeper a foot in, to 3.5), its banks from 0.3 ft above the water (rising a tenth of a
    // foot a foot) eased out to the ground round it over its band, and no steeper than 0.35 from there
    // out to 30 ft past it, the water's level held a square past the rim (further out, well under the
    // ground: no wet shore drawn); a stream's bed below its water's level along it, its banks cut back
    // no steeper than three in ten for 40 ft and banked up where the ground beside it lies low; a
    // formal garden's parterres sunk inside their hedges; a raised tier 6 ft up, easing out over 20 ft
    // (no steeper than one in two).
    // (A park's own water and the two squares round it: `edges` bit 32, where the ground's drawing
    // keeps its dry fringe off the water's bed.)
    let mut park_wet = vec![false; HS * HS];
    for pond in &ponds {
        let (pts, level) = (&pond.pts, pond.level);
        let b = bbox(pts);
        let r = pond.band;
        const CONE: f64 = crate::park::POND_CONE;
        let (i0, j0, i1, j1) = range((b.0 - r - CONE, b.1 - r - CONE, b.2 + r + CONE, b.3 + r + CONE));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let p = center(i, j);
                let edge = (0..pts.len()).map(|e| crate::town::geom::seg_dist(p, pts[e], pts[(e + 1) % pts.len()])).fold(f64::MAX, f64::min);
                let inside = crate::town::geom::contains(pts, p);
                // (Past the band, other water's ground is left as it is.)
                if !at_ease(h) || (!inside && (edge >= r + CONE || (edge >= r && hw[h] > hh[h] - 1.0))) {
                    continue;
                }
                park_wet[h] |= inside || edge <= 2.0 * SQUARE_FT;
                // (Its own water there, whatever level a sea or river nearby would give it.)
                if inside {
                    hh[h] = level - (0.3 + 0.2 * edge).min(3.5) as f32;
                    hw[h] = level;
                } else {
                    hh[h] = crate::park::pond_bank(hh[h] as f64, edge, level as f64, r) as f32;
                    // (Within the band, 4 ft under the ground, never the land's no-water level: the
                    // water's surface between squares then meets the ground along the rim, not in
                    // steps at the squares.)
                    hw[h] = if edge <= SQUARE_FT {
                        level
                    } else if edge < r {
                        hh[h] - 4.0
                    } else {
                        hw[h].min(hh[h] - 4.0)
                    };
                }
            }
        }
    }
    for pts in &streams {
        let half = 0.5 * crate::park::STREAM_FT;
        const BANK: f64 = crate::park::STREAM_BANK;
        let b = pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, q| (a.0.min(q.0[0]), a.1.min(q.0[1]), a.2.max(q.0[0]), a.3.max(q.0[1])));
        let r = half + BANK;
        let (i0, j0, i1, j1) = range((b.0 - r, b.1 - r, b.2 + r, b.3 + r));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let p = center(i, j);
                if !at_ease(h) || hw[h] > hh[h] {
                    continue;
                }
                // The nearest point of its line, and the water's level there.
                let mut best: Option<(f64, f32)> = None;
                for s in pts.windows(2) {
                    let (q, t) = crate::park::project(s[0].0, s[1].0, p);
                    let d = crate::town::geom::dist(p, q);
                    if best.is_none_or(|x| d < x.0) {
                        best = Some((d, s[0].1 + (s[1].1 - s[0].1) * t as f32));
                    }
                }
                let Some((d, wl)) = best.filter(|x| x.0 < r) else { continue };
                park_wet[h] |= d < half + 2.0 * SQUARE_FT;
                // (The highest its water stands within a square and a half more than that: the water of
                // the square beside it upstream, on a falling stream.)
                let reach = d + 1.5 * SQUARE_FT;
                let mut up = wl;
                for s in pts.windows(2) {
                    let (q, t) = crate::park::project(s[0].0, s[1].0, p);
                    let dq = crate::town::geom::dist(p, q);
                    let ab = crate::town::geom::dist(s[0].0, s[1].0).max(1e-9);
                    if dq < reach {
                        let dt = crate::core::sqrt(reach * reach - dq * dq) / ab;
                        for tt in [(t - dt).max(0.0), (t + dt).min(1.0)] {
                            up = up.max(s[0].1 + (s[1].1 - s[0].1) * tt as f32);
                        }
                    }
                }
                if d < half {
                    hh[h] = hh[h].min(wl - (0.4 + 0.8 * (1.0 - d / half)) as f32);
                    hw[h] = wl;
                } else {
                    // (Cut back three in ten, and banked up where the ground beside it lies low: its
                    // water never stands above its banks.)
                    hh[h] = hh[h].min(crate::park::stream_bank(wl as f64, d - half) as f32);
                    // (0.3 ft over the water beside it out to 15 ft from its edge, then down a tenth of
                    // a foot a foot.)
                    if d < half + 25.0 {
                        hh[h] = hh[h].max(up + (0.3 - 0.1 * (d - half - 15.0).max(0.0)) as f32);
                    }
                    hw[h] = if d < half + SQUARE_FT { wl.min(hh[h] - 0.1) } else { hh[h] - 4.0 };
                }
            }
        }
    }
    // P146: water drawn by hand, as a park's: its squares and the two round it `edges` bit 32 (the
    // ground's drawing follows the water's edge, not the squares), and the dry squares by its
    // shore take its level (kept under their ground) instead of the land's no-water one, so the
    // surface between squares meets the ground along the shore, not in torn steps at the squares.
    // (`by_hand_water`: those squares, for the guarantee: no knoll in a cell the water reaches.)
    let mut by_hand_water = Vec::new();
    if let Some(sh) = shaper.as_ref().filter(|s| s.has_waters()) {
        by_hand_water = vec![false; HS * HS];
        for h in 0..HS * HS {
            let p = center(h % HS, h / HS);
            if hw[h] > hh[h] {
                let own = sh.outline_level(p[0], p[1]).is_some();
                park_wet[h] |= own;
                by_hand_water[h] = own;
                continue;
            }
            if let Some((_, d, l)) = sh.water_edge(p[0], p[1], 2.0 * SQUARE_FT, sea) {
                park_wet[h] = true;
                by_hand_water[h] = true;
                if d <= 1.5 * SQUARE_FT {
                    hw[h] = hw[h].max((l as f32).min(hh[h] - 0.3));
                }
            }
        }
    }
    for q in &sunk {
        let (i0, j0, i1, j1) = range(bbox(q));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        let (a, b) = crate::park::SUNK_BANK;
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let din = crate::park::depth(q, center(i, j));
                if din > 0.0 && at_ease(h) && hw[h] <= hh[h] {
                    let w = 0.5 * ((din - a) / (b - a)).clamp(0.0, 1.0) + 0.5 * smoothstep(a, b, din);
                    hh[h] -= (crate::park::SUNK_FT * w) as f32;
                    hw[h] = hw[h].min(hh[h] - 4.0);
                }
            }
        }
    }
    for &(c, r) in &mounds {
        let reach = r + 20.0;
        let (i0, j0, i1, j1) = range((c[0] - reach, c[1] - reach, c[0] + reach, c[1] + reach));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let d = crate::town::geom::dist(center(i, j), c);
                if at_ease(h) && d < reach && hw[h] <= hh[h] {
                    hh[h] += (6.0 * (1.0 - smoothstep(r, reach, d))) as f32;
                }
            }
        }
    }
    // v63 countryside: hedgerows, dry-stone walls and fences (not on a road or water), orchards,
    // copses, windmills' sails and water wheels (`town::rural`).
    let mut rural = crate::town::rural::Battle::default();
    {
        let blocked = |p: [f64; 2]| {
            let (i, j) = (((p[0] - ox) / SQUARE_FT).floor() as i64, ((p[1] - oy) / SQUARE_FT).floor() as i64);
            if i < -1 || j < -1 || i > SQ as i64 || j > SQ as i64 {
                return false;
            }
            hw[(j + 1) as usize * HS + (i + 1) as usize] > hh[(j + 1) as usize * HS + (i + 1) as usize] || (-1..=1).any(|dy| (-1..=1).any(|dx| tile_mask.get(sidx(i + dx, j + dy)).is_some_and(|&r| r != 0)))
        };
        let rect = [ox, oy, ox + chunk_ft, oy + chunk_ft];
        for l in &rural_ls {
            let fcut = crate::town::fence::FenceCut::of(world, t0, l);
            crate::town::rural::battle(l, t0, rect, &blocked, fcut.as_ref(), &mut rural);
        }
        // (Roadmap 6 R6: parks' hedges and yard walls changed by hand, fences drawn by hand (cut by
        // water alone), then the eraser's gaps through all of them.)
        if !hand_fences.runs.is_empty() || !hand_fences.gates.is_empty() {
            crate::town::fence::take(&hand_fences, rect, &mut rural);
        }
        if !world.file.edits.fences.is_empty() {
            let wet = |p: [f64; 2]| {
                let (i, j) = (((p[0] - ox) / SQUARE_FT).floor() as i64, ((p[1] - oy) / SQUARE_FT).floor() as i64);
                i >= -1 && j >= -1 && i <= SQ as i64 && j <= SQ as i64 && hw[(j + 1) as usize * HS + (i + 1) as usize] > hh[(j + 1) as usize * HS + (i + 1) as usize]
            };
            crate::town::fence::drawn(&world.file.edits, rect, &wet, &mut rural);
        }
        crate::town::fence::cut_gaps(&fence_gaps, &mut rural);
    }
    // (Roadmap 6 R6: a high wall, generated run changed or drawn by hand, raised as a garden wall
    // is, `HIGH_WALL_FT` tall, over the halo too; drawn by `rural::shapes`. A square stands on it
    // when its middle lies by it and over it, from its start up to short of its end, none on a
    // building's entrance: a gate's opening (`fence::gate_span`) always leaves one square free.)
    for &(a, b, _) in rural.runs.iter().filter(|r| r.2 == crate::town::rural::RunKind::Wall) {
        let half = 0.5 * crate::interior::FORE_WALL_FT;
        let l = crate::town::geom::dist(a, b);
        if l < 1e-6 {
            continue;
        }
        let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
        let r = half + 1.5;
        let (i0, j0, i1, j1) = range((a[0].min(b[0]) - r, a[1].min(b[1]) - r, a[0].max(b[0]) + r, a[1].max(b[1]) + r));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let h = j * HS + i;
                let d = crate::town::geom::sub(center(i, j), a);
                let t = crate::town::geom::dot(d, u);
                if bld_h[h] != 0 || ent_h[h] || hw[h] > hh[h] || matches!(urb_h[h], URBAN_WALL | URBAN_TOWER | URBAN_TOWER_FLOOR) || !(0.0..l).contains(&t) || crate::town::geom::cross(u, d).abs() > half + 1.5 {
                    continue;
                }
                urb_h[h] = URBAN_WALL;
                hh[h] = hh[h].max(ground_h[h] + HIGH_WALL_FT);
                lift[h] = hh[h] - ground_h[h];
            }
        }
    }
    let road: Vec<u8> = (0..n).map(|k| road_h[(k / SQ + 1) * HS + k % SQ + 1]).collect();
    let interior = |grid: &[f32]| -> Vec<f32> { (0..n).map(|k| grid[(k / SQ + 1) * HS + k % SQ + 1]).collect() };
    let height = interior(&hh);
    let water_level = interior(&hw);
    let dominant = dominant_biome(&biome, &water_level, &height);
    // A plateau's rock floor is scree, as the map draws it (`payload::shaped`): its boulders and
    // rubble, no trees (below) or ground cover. (After the chunk's own biome is known.)
    let on_floor = |k: usize| floor_h.get((k / SQ + 1) * HS + k % SQ + 1).is_some_and(|&f| f);
    if !floor_h.is_empty() {
        for k in (0..n).filter(|&k| on_floor(k)) {
            biome[k] = Biome::Alpine;
        }
    }
    // (A park's path doesn't go on under the water, nor over a step steeper than a cliff's grade
    // to a square beside it: it stops at the edge, but where the step is a raised feature's.)
    let on_feature = |h: usize| features.iter().any(|f| crate::town::geom::dist(center(h % HS, h / HS), f.0) <= f.1);
    let cliff = |h: usize| [1, HS].iter().any(|&o| [h + o, h - o].iter().any(|&nh| (hh[nh] - hh[h]).abs() > 2.75 && !on_feature(nh)));
    // (R5 towns: a cemetery's walks stay whole: its ground is levelled to the bed later, its tombs
    // stand by its walks.)
    let in_yard = |h: usize| churchyards.iter().any(|y| crate::town::geom::contains(y, center(h % HS, h / HS))) || cem_links.contains(&h);
    let urban: Vec<u8> = (0..n)
        .map(|k| {
            let h = (k / SQ + 1) * HS + k % SQ + 1;
            match urb_h[h] {
                _ if ent_h[h] => URBAN_ENTRANCE,
                URBAN_PATH if hw[h] > hh[h] || (cliff(h) && !in_yard(h)) => URBAN_PARK,
                u => u,
            }
        })
        .collect();

    crate::town::rural::shapes(&rural, ox, oy, &mut shapes);
    let mut chunk = Chunk {
        key: *key,
        tier: vec![0; n],
        surface: vec![Surface::Grass; n],
        edges: vec![0; n],
        objects: Vec::new(),
        atmosphere: atmosphere_for(dominant, hash3(seed, gsx, gsy, 0xa7)),
        height,
        water_level,
        halo_height: hh,
        halo_water: hw,
        road_halo: road_h,
        building: (0..n).map(|k| bld_h[(k / SQ + 1) * HS + k % SQ + 1]).collect(),
        buildings: refs,
        urban,
        building_polys: polys,
        building_parts: poly_parts,
        roof_lift: lift,
        structures: shapes,
        sprites: Vec::new(),
        yard: Vec::new(),
        stone: 0,
        field: Vec::new(),
    };
    retier(&mut chunk, sea);

    // Surfaces. Towns and cities pave their streets; villages have packed earth.
    let here = crate::town::layouts_near(world, t0, [ox, oy, ox + SQ as f64 * SQUARE_FT, oy + SQ as f64 * SQUARE_FT]);
    let tier_here = here.iter().map(|l| l.tier).max();
    let mid = [ox + SQ as f64 * SQUARE_FT / 2.0, oy + SQ as f64 * SQUARE_FT / 2.0];
    let away = |l: &crate::town::Layout| (l.center[0] - mid[0]).hypot(l.center[1] - mid[1]);
    chunk.stone = here.iter().filter(|l| Some(l.tier) == tier_here).min_by(|a, b| away(a).total_cmp(&away(b))).map_or(0, |l| (l.index + 1) & 0xff_ffff | (l.culture as u32 % 5 + 1) << 24);
    let paved = tier_here.is_some_and(|t| t >= crate::t0::settle::Tier::Town);
    let village = tier_here == Some(crate::t0::settle::Tier::Village);
    // (P145: the walls and castles a moat drawn by hand may run by, found when one does.)
    let mut moat_stone: Option<Vec<([f64; 2], [f64; 2])>> = None;
    // R7 159: a town's built ground (its wards and buildings) is the town's, not a beach, but for
    // the squares at the water's edge (any of the eight round it wet); where the sea's or a lake's
    // biome reaches in, the ground of the land round the town (packed earth under a building off
    // its wards). (A hot desert's sand stays: inland it is the town's too.)
    let town_ground = |k: usize| {
        let h = (k / SQ + 1) * HS + k % SQ + 1;
        (in_town[h] || bld_h[h] != 0)
            && [h - HS - 1, h - HS, h - HS + 1, h - 1, h + 1, h + HS - 1, h + HS, h + HS + 1].iter().all(|&q| chunk.halo_water[q] <= chunk.halo_height[q])
    };
    for k in 0..n {
        let depth = chunk.water_level[k] - chunk.height[k];
        let town = depth <= 0.0 && town_ground(k);
        chunk.surface[k] = if depth > 4.0 {
            Surface::Deep
        } else if depth > 0.0 {
            Surface::Shallow
        } else if !town && beach(depth as f64, chunk.water_level[k] as f64, sea, slope[k]) {
            Surface::Sand
        } else {
            match surface_for(biome[k], slope[k]) {
                // Tundra: patches of snow among its grass.
                Surface::TundraGrass if tundra_snowy(world, ox, oy, k) => Surface::Snow,
                Surface::Sand if town && matches!(biome[k], Biome::Ocean | Biome::Lake) => match town_ix[(k / SQ + 1) * HS + k % SQ + 1] {
                    0 => Surface::Dirt,
                    ix => match surface_for(town_land[ix as usize - 1], slope[k]) {
                        Surface::TundraGrass if tundra_snowy(world, ox, oy, k) => Surface::Snow,
                        s => s,
                    },
                },
                s => s,
            }
        };
        if depth <= 0.0 {
            // The ground's own kind, under any keep-clear mark.
            // (A path's square cut at a cliff is the park's lawn.)
            let mark = match urb_h[(k / SQ + 1) * HS + k % SQ + 1] {
                URBAN_PATH if chunk.urban[k] != URBAN_PATH => URBAN_PARK,
                u => u,
            };
            match mark {
                // A castle's bailey, a village's green, a croft and pasture keep the biome's own ground;
                // a path up a motte is gravel, a town's plaza paved.
                // (R5 M20: a quay's or breakwater's top is paved, in a village too.)
                URBAN_PLAZA if look_h.get((k / SQ + 1) * HS + k % SQ + 1) == Some(&crate::ground::Look::Made) => chunk.surface[k] = Surface::Cobble,
                URBAN_PLAZA if quay_h.get((k / SQ + 1) * HS + k % SQ + 1) == Some(&true) => chunk.surface[k] = Surface::Cobble,
                URBAN_PLAZA if motte_path[(k / SQ + 1) * HS + k % SQ + 1] => chunk.surface[k] = Surface::Gravel,
                URBAN_PLAZA if bailey_h[(k / SQ + 1) * HS + k % SQ + 1] => {}
                URBAN_PLAZA if dirt_sq[(k / SQ + 1) * HS + k % SQ + 1] => chunk.surface[k] = Surface::Dirt,
                URBAN_PLAZA if !village => chunk.surface[k] = Surface::Cobble,
                // (Not at the mouth of a way underground on the flight: it keeps the street.)
                URBAN_STREET if chunk.urban[k] == URBAN_STREET && stair_h.as_ref().is_some_and(|s| s[(k / SQ + 1) * HS + k % SQ + 1]) => chunk.surface[k] = Surface::Stairs,
                URBAN_STREET => chunk.surface[k] = if paved { Surface::Cobble } else { Surface::Road },
                URBAN_FIELD => {
                    // Crops get their own surface; pasture keeps the biome's ground.
                    if crate::town::fields::is_crop(fld_h[(k / SQ + 1) * HS + k % SQ + 1]) {
                        chunk.surface[k] = Surface::Field;
                    }
                }
                URBAN_DECK => chunk.surface[k] = if paved { Surface::Cobble } else { Surface::Road },
                URBAN_RUIN | URBAN_YARD => chunk.surface[k] = Surface::Dirt,
                URBAN_WALL | URBAN_TOWER | URBAN_TOWER_FLOOR => chunk.surface[k] = Surface::Rock,
                URBAN_PATH => chunk.surface[k] = Surface::Gravel,
                // (A formal garden's lawn is mown grass, whatever grows round the town; a common's is
                // the ground of the land round its middle, whatever the shore beside it.)
                // (A palace's court a mown lawn too.)
                URBAN_PARK if park_h[(k / SQ + 1) * HS + k % SQ + 1] == 2 || court_h[(k / SQ + 1) * HS + k % SQ + 1] => chunk.surface[k] = Surface::Grass,
                URBAN_PARK if park_ix[(k / SQ + 1) * HS + k % SQ + 1] != 0 => {
                    chunk.surface[k] = surface_for(park_biomes[park_ix[(k / SQ + 1) * HS + k % SQ + 1] as usize - 1], slope[k]);
                }
                // (R5 M20: an islet drawn by hand is bare rock, gravel at its rim.)
                0 => match look_h.get((k / SQ + 1) * HS + k % SQ + 1) {
                    Some(crate::ground::Look::Rock { rim: false }) => chunk.surface[k] = Surface::Rock,
                    Some(crate::ground::Look::Rock { rim: true }) => chunk.surface[k] = Surface::Gravel,
                    // (A plateau's rock floor is bare rock, never the alpine snow of its biome.)
                    _ if on_floor(k) => chunk.surface[k] = Surface::Rock,
                    _ => {}
                },
                _ => {}
            }
        }
        // P145: the bank of water drawn by hand, by its look (`waters::Form::look`): a pond's mud,
        // a lake's pebbles, an inlet's sand, a moat's earth (stone by a castle or wall), a
        // stream's gravel a square wide each side (no patches), dug water's stone edge (a quay's
        // paving is the plaza's, `quay_h`) (on open ground only: a street, a field or a yard
        // keeps its own).
        if depth <= 0.0
            && urb_h[(k / SQ + 1) * HS + k % SQ + 1] == 0
            && let Some(sh) = shaper.as_ref().filter(|s| s.has_waters())
        {
            let (x, y) = (ox + ((k % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((k / SQ) as f64 + 0.5) * SQUARE_FT);
            if let Some((look, d, _)) = sh.water_edge(x, y, HAND_BANK_FT, sea) {
                use crate::waters::WaterKind as W;
                let (reach, surface) = match look.kind {
                    W::Pond | W::Lake if look.dug => (4.0, Surface::Rock),
                    W::Pond => (6.0, Surface::Mud),
                    W::Lake => (5.0, Surface::Gravel),
                    W::Inlet => (8.0, Surface::Sand),
                    W::Basin => (QUAY_FT, Surface::Cobble),
                    W::Moat => (4.0, if moat_stone.get_or_insert_with(|| guarded_near(world, t0, [ox, oy, ox + SQ as f64 * SQUARE_FT, oy + SQ as f64 * SQUARE_FT])).iter().any(|&(a, b)| crate::town::geom::seg_dist([x, y], a, b) <= MOAT_STONE_FT) { Surface::Rock } else { Surface::Dirt }),
                    W::Stream => (6.0, Surface::Gravel),
                };
                if d <= reach {
                    chunk.surface[k] = surface;
                }
            }
        }
        if road[k] != 0 {
            chunk.surface[k] = if road[k] & 0x80 != 0 {
                Surface::Planks
            } else if road[k] == 1 {
                Surface::Cobble
            } else {
                Surface::Road
            };
        }
        // (A gravel road drawn by hand.)
        if path_h.get((k / SQ + 1) * HS + k % SQ + 1) == Some(&(crate::paths::PathKind::Gravel as u8 + 1)) && road[k] & 0x80 == 0 {
            chunk.surface[k] = Surface::Gravel;
        }
        if chunk.building[k] != 0 {
            chunk.surface[k] = Surface::Roof;
        }
    }
    // v63 countryside
    if chunk.urban.contains(&URBAN_FIELD) {
        chunk.field = (0..n).map(|k| if chunk.urban[k] == URBAN_FIELD && (chunk.surface[k] == Surface::Field || (fld_h[(k / SQ + 1) * HS + k % SQ + 1] >> 6 == 3)) { fld_h[(k / SQ + 1) * HS + k % SQ + 1] } else { 0 }).collect();
    }

    // Road squares (bridges and fords too) two squares round the chunk, and the chunk's
    // streets: scatter keeps its footprint off them, across chunk edges too.
    let road_near: Vec<bool> = (0..RN * RN)
        .map(|h| {
            let (i, j) = ((h % RN) as i64 - 2, (h / RN) as i64 - 2);
            let inside = i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64;
            // (Roads drawn by hand too: their band, a pure function of where the square is.)
            let drawn = || !path_near.is_empty() && crate::paths::at(&path_near, [ox + (i as f64 + 0.5) * SQUARE_FT, oy + (j as f64 + 0.5) * SQUARE_FT], 0.0, PATH_MIN_HALF_FT).is_some();
            tile_mask.get(sidx(i, j)).is_some_and(|&r| r != 0) || (inside && chunk.urban[j as usize * SQ + i as usize] == URBAN_STREET) || drawn()
                || (!inside && i >= -1 && j >= -1 && i <= SQ as i64 && j <= SQ as i64 && urb_h[(j + 1) as usize * HS + (i + 1) as usize] == URBAN_STREET)
        })
        .collect();
    // A castle's baileys take the land's own scatter (trees, bushes, rocks) on their open squares:
    // bit 4 (mottes) stays bare, and a square's width is kept round curtains, towers and roofs
    // (`place_objects` also keeps clear of the ways and the gate mouths by each thing's reach).
    let yard_ok: Vec<bool> = if bailey_kind.iter().all(|&v| v == 0) {
        Vec::new()
    } else {
        (0..n)
            .map(|k| {
                let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
                let h = |dx: i64, dy: i64| ((y + dy + 1) as usize) * HS + (x + dx + 1) as usize;
                chunk.urban[k] == URBAN_PLAZA
                    && bailey_kind[h(0, 0)] & 3 != 0
                    && bailey_kind[h(0, 0)] & 4 == 0
                    && (-1..=1).all(|dy| (-1..=1).all(|dx| !matches!(urb_h[h(dx, dy)], URBAN_WALL | URBAN_TOWER | URBAN_TOWER_FLOOR) && bailey_kind[h(dx, dy)] & 4 == 0))
            })
            .collect()
    };
    let yard = Yard { ok: &yard_ok, ways: &bailey_ways, gates: &gates, ox, oy };
    for k in (0..n).filter(|_| !crag_h.is_empty()) {
        if crag_m[(k / SQ + 1) * HS + k % SQ + 1] & 1 != 0 && chunk.water_level[k] <= chunk.height[k] && chunk.building[k] == 0 {
            chunk.surface[k] = Surface::Rock;
        }
    }
    // v63 countryside: a croft's or a field's squares (halo), which the land's scatter keeps off; and
    // a verge by the roads and streets (two squares) the land's trees and boulders keep off.
    let farm_h: Vec<bool> = urb_h.iter().map(|&u| matches!(u, URBAN_FIELD | URBAN_CROFT)).collect();
    let verge: Vec<bool> = (0..RN * RN)
        .map(|h| {
            let (i, j) = ((h % RN) as i64, (h / RN) as i64);
            (-2..=2i64).any(|dy| (-2..=2i64).any(|dx| (i + dx) >= 0 && (j + dy) >= 0 && (i + dx) < RN as i64 && (j + dy) < RN as i64 && road_near[(j + dy) as usize * RN + (i + dx) as usize]))
        })
        .collect();
    place_objects(&mut chunk, &biome, &slope, &road_near, &yard, &Keep { farm: &farm_h, verge: &verge }, seed, gsx, gsy);
    if !floor_h.is_empty() {
        let sq = |o: &Object| (o.y.max(0.0) as usize).min(SQ - 1) * SQ + (o.x.max(0.0) as usize).min(SQ - 1);
        chunk.objects.retain(|o| !(TREES_AND_COVER.contains(&o.kind) && on_floor(sq(o))));
    }
    // The land's scatter in a bailey gives way to what the bailey is dressed with (every prop of
    // the layout, not only this chunk's, so a seam cuts nothing differently).
    if !yard_ok.is_empty() {
        let kept: Vec<Object> = chunk
            .objects
            .iter()
            .filter(|o| {
                if !yard_ok[(o.y as usize).min(SQ - 1) * SQ + (o.x as usize).min(SQ - 1)] {
                    return true;
                }
                let r = (info(o.kind as u16).radius * o.scale) as f64 * SQUARE_FT;
                let p = [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
                !bailey_props.iter().any(|(q, _, kd, _, sc)| crate::town::geom::dist(p, *q) < r + (info(*kd as u16).radius * *sc) as f64 * SQUARE_FT + 3.0)
                    && !wells.iter().any(|q| crate::town::geom::dist(p, *q) < r + 12.0)
            })
            .copied()
            .collect();
        chunk.objects = kept;
    }
    let rowless: Vec<bool> = if churchyards.is_empty() {
        Vec::new()
    } else {
        (0..n).map(|k| churchyards.iter().any(|y| crate::town::geom::contains(y, [ox + ((k % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((k / SQ) as f64 + 0.5) * SQUARE_FT]))).collect()
    };
    place_town_props(&mut chunk, seed, gsx, gsy, &rowless);
    crate::town::rural::objects(&rural, ox, oy, &mut chunk);
    crate::town::fence::objects(&rural, ox, oy, &mut chunk.objects);
    // v63 countryside: haystacks in stack yards and by field gates (`farm`), each in the chunk holding
    // its middle, on its field's or croft's open ground, off what stands there already.
    {
        let mut hay: Vec<([f64; 2], f32, f32)> = Vec::new();
        for l in &rural_ls {
            farm::stacks(l, [ox, oy, ox + chunk_ft, oy + chunk_ft], &mut hay);
        }
        for (p, rot, scale) in hay {
            let (x, y) = (((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32);
            let k = (y as usize).min(SQ - 1) * SQ + (x as usize).min(SQ - 1);
            let o = Object { kind: Kind::Haystack, variant: (hash2(seed, (p[0] * 4.0) as i64, (p[1] * 4.0) as i64) >> 40) as u8 & 7, x, y, rot, scale, sprite: 0, turned: false };
            let r = info(Kind::Haystack as u16).radius * scale;
            let free = !chunk.objects.iter().any(|q| {
                let rq = chunk.info(q).radius * q.scale;
                (q.x - x) * (q.x - x) + (q.y - y) * (q.y - y) < (r + rq) * (r + rq)
            });
            if matches!(chunk.urban[k], URBAN_FIELD | URBAN_CROFT) && placeable(&chunk, k) && clear_of_roads(&chunk, k) && clear_of_buildings(&chunk, &o) && free {
                chunk.objects.push(o);
            }
        }
    }
    // Ruins: broken wall segments along the old outlines, rubble inside. Keyed by world
    // position so a ruin across a chunk border breaks the same way in both chunks.
    let wseed = hash2(seed, 0x2a11, 5);
    for poly in &ruins {
        let m = poly.len();
        for e in 0..m {
            let (a, b) = (poly[e], poly[(e + 1) % m]);
            let len = crate::town::geom::dist(a, b);
            let steps = (len / SQUARE_FT).floor().max(1.0) as usize;
            for s in 0..steps {
                let p = crate::town::geom::lerp(a, b, (s as f64 + 0.5) / steps as f64);
                let h = hash2(wseed, (p[0] * 2.0) as i64, (p[1] * 2.0) as i64);
                // Long runs survive, with breaches.
                let run = unit(hash2(wseed ^ 0x77, (p[0] / 20.0) as i64, (p[1] / 20.0) as i64));
                if run < 0.3 || unit(h) < 0.15 {
                    continue;
                }
                let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
                if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
                    continue;
                }
                let rot = libm::atan2(b[1] - a[1], b[0] - a[0]) as f32;
                let variant = (h >> 20) as u8 & 7;
                chunk.objects.push(Object { kind: Kind::RuinWall, variant, x: x as f32, y: y as f32, rot, scale: 1.0, sprite: 0, turned: false });
            }
        }
        let c = crate::town::geom::centroid(poly);
        let h = hash2(wseed, (c[0] * 2.0) as i64, (c[1] * 2.0) as i64);
        let (x, y) = ((c[0] - ox) / SQUARE_FT, (c[1] - oy) / SQUARE_FT);
        if unit(h) < 0.7 && x >= 0.0 && y >= 0.0 && x < SQ as f64 && y < SQ as f64 {
            chunk.objects.push(Object { kind: Kind::RockPile, variant: (h >> 20) as u8 & 7, x: x as f32, y: y as f32, rot: 0.0, scale: 1.3, sprite: 0, turned: false });
        }
    }
    for st in &statues {
        let (x, y) = ((st[0] - ox) / SQUARE_FT, (st[1] - oy) / SQUARE_FT);
        if x >= 0.0 && y >= 0.0 && x < SQ as f64 && y < SQ as f64 {
            chunk.objects.push(Object { kind: Kind::Statue, variant: 0, x: x as f32, y: y as f32, rot: 0.0, scale: 1.0, sprite: 0, turned: false });
        }
    }
    // A layout's own props: each in the chunk holding it, on dry ground off roads and roofs.
    for p in &props {
        let (x, y) = ((p.at[0] - ox) / SQUARE_FT, (p.at[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.building[k] != 0 || chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] {
            continue;
        }
        chunk.objects.push(Object { kind: p.kind, variant: p.variant, x: x as f32, y: y as f32, rot: p.rot, scale: p.scale, sprite: 0, turned: false });
    }
    // Plaza features: drawn by the chunk holding the centre; a planter's tree stands in it.
    for &(fc, r, kind, ang) in &features {
        let (x, y) = ((fc[0] - ox) / SQUARE_FT, (fc[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        // Drawn a little past the raised squares' centres, so the round covers them.
        let rv = (r + 2.0) / SQUARE_FT;
        let rim = [(x + rv * libm::cos(ang)) as f32, (y + rv * libm::sin(ang)) as f32];
        chunk.structures.push(VectorShape { kind: ShapeKind::Dais, size: kind as u8 as f32, pts: vec![[x as f32, y as f32], rim] });
        if kind == PlazaFeature::Planter {
            chunk.objects.push(Object { kind: Kind::TreeDeciduous, variant: (hash2(seed, x as i64, y as i64) >> 20) as u8 & 7, x: x as f32, y: y as f32, rot: 0.0, scale: 0.9, sprite: 0, turned: false });
        }
    }
    // The market: each stall in the chunk holding its centre, on open plaza ground.
    for (p, rot, kind) in stalls {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.urban[k] != URBAN_PLAZA || chunk.building[k] != 0 || chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] {
            continue;
        }
        let h = hash2(seed ^ 0x5a11, (p[0] * 2.0) as i64, (p[1] * 2.0) as i64);
        // A stall's look is its awning and its goods, rolled apart (sixteen).
        let looks = if kind == Kind::MarketStall { 15 } else { 7 };
        let o = Object { kind, variant: (h >> 20) as u8 & looks, x: x as f32, y: y as f32, rot, scale: 1.0, sprite: 0, turned: false };
        // The plaza's margin keeps stalls off buildings; across a chunk edge the neighbour's
        // squares are plaza too.
        let r = info(kind as u16).radius;
        let near_building = (-1i32..=1).any(|dy| {
            (-1i32..=1).any(|dx| {
                let (i, j) = ((o.x + dx as f32 * r) as i32, (o.y + dy as f32 * r) as i32);
                i >= 0 && j >= 0 && (i as usize) < SQ && (j as usize) < SQ && chunk.building[j as usize * SQ + i as usize] != 0
            })
        });
        if !near_building {
            chunk.objects.push(o);
        }
    }
    // R5 M20: a harbour's things, each in the chunk holding it: boats and buoys on open water (never
    // on a deck), bollards and cranes on dry ground clear of roofs and of what stands there already
    // (a crane on a quay's paving only).
    for (p, rot, kind, variant, afloat) in dock {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        let wet = chunk.water_level[k] > chunk.height[k];
        if chunk.building[k] != 0 || wet != afloat || (afloat && chunk.urban[k] == URBAN_DECK) || (kind == Kind::DockCrane && chunk.urban[k] != URBAN_PLAZA) {
            continue;
        }
        let r = info(kind as u16).radius;
        let clash = |o: &Object| {
            // (A crane's sills, not its jib's reach.)
            let own = if kind == Kind::DockCrane { 0.4 * r } else { r };
            let (dx, dy, rr) = (o.x - x as f32, o.y - y as f32, 0.8 * (own + chunk.info(o).radius * o.scale));
            dx * dx + dy * dy < rr * rr
        };
        if !afloat && chunk.objects.iter().any(clash) {
            continue;
        }
        if kind == Kind::DockCrane && (-1i32..=1).any(|dy| (-1i32..=1).any(|dx| {
            let (i, j) = (x as i32 + dx, y as i32 + dy);
            i >= 0 && j >= 0 && (i as usize) < SQ && (j as usize) < SQ && chunk.building[j as usize * SQ + i as usize] != 0
        })) {
            continue;
        }
        chunk.objects.push(Object { kind, variant, x: x as f32, y: y as f32, rot, scale: 1.0, sprite: 0, turned: false });
    }
    // What a square is dressed with: each in the chunk holding it, on open ground (paving, a lawn,
    // a street through the square), never on a roof, a road or the water (its layout keeps clear
    // of buildings); a market cross a little larger than its place, to read as a cross from afar.
    for (p, rot, kind, variant) in dressing {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.building[k] != 0 || chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] || !matches!(chunk.urban[k], URBAN_PLAZA | URBAN_STREET | URBAN_PARK | URBAN_GRAVEYARD) {
            continue;
        }
        let scale = if kind == Kind::MarketCross { MARKET_CROSS_SCALE } else { 1.0 };
        chunk.objects.push(Object { kind, variant, x: x as f32, y: y as f32, rot, scale, sprite: 0, turned: false });
    }
    // What a park sets out: each in the chunk holding it, on its lawn (what stands in a gravel round
    // or across a stream on the gravel or the water too, reeds in the water), never on a roof, a road
    // or reaching over a building.
    for (p, rot, kind, variant, scale) in park_props {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        let wet_ok = matches!(kind, Kind::Reeds | Kind::Footbridge);
        let gravel_ok = matches!(kind, Kind::Fountain | Kind::GardenStatue | Kind::Footbridge | Kind::Lychgate);
        // (R5 towns: a cemetery's on its graveyard ground.)
        let lawn = chunk.urban[k] == URBAN_PARK || chunk.urban[k] == URBAN_GRAVEYARD || chunk.urban[k] == URBAN_YARD;
        if chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || (!wet_ok && chunk.water_level[k] > chunk.height[k]) || !(lawn || (gravel_ok && chunk.urban[k] == URBAN_PATH)) {
            continue;
        }
        let o = Object { kind, variant, x: x as f32, y: y as f32, rot, scale, sprite: 0, turned: false };
        if off_buildings(&chunk, &o) {
            chunk.objects.push(o);
        }
    }
    // What a bailey is dressed with: each in the chunk holding it, on the bailey, off the roofs.
    for (p, rot, kind, variant, scale) in bailey_props {
        let (x, y) = ((p[0] - ox) / SQUARE_FT, (p[1] - oy) / SQUARE_FT);
        if x < 0.0 || y < 0.0 || x >= SQ as f64 || y >= SQ as f64 {
            continue;
        }
        let k = y as usize * SQ + x as usize;
        if chunk.road_halo[(k / SQ + 1) * HS + k % SQ + 1] != 0 || chunk.water_level[k] > chunk.height[k] || chunk.urban[k] != URBAN_PLAZA || bailey_kind[(k / SQ + 1) * HS + k % SQ + 1] & 3 == 0 {
            continue;
        }
        let o = Object { kind, variant, x: x as f32, y: y as f32, rot, scale, sprite: 0, turned: false };
        if off_buildings(&chunk, &o) {
            chunk.objects.push(o);
        }
    }
    // A well at the heart of every main square and village green.
    for w in wells {
        let (i, j) = (((w[0] - ox) / SQUARE_FT).floor(), ((w[1] - oy) / SQUARE_FT).floor());
        if i >= 0.0 && j >= 0.0 && (i as usize) < SQ && (j as usize) < SQ {
            let k = j as usize * SQ + i as usize;
            if chunk.building[k] == 0 && chunk.water_level[k] <= chunk.height[k] {
                chunk.objects.push(Object { kind: Kind::Well, variant: 0, x: ((w[0] - ox) / SQUARE_FT) as f32, y: ((w[1] - oy) / SQUARE_FT) as f32, rot: 0.0, scale: 1.0, sprite: 0, turned: false });
            }
        }
    }
    // Nothing generated stands on a wall or a tower, or in a gate's passage (a cart under the
    // portcullis); nor does what the guarantee adds below (`keep_off`: those squares, the ones next
    // to a wall or a tower, and a square's raised features with the walk round them).
    // (A footprint `r` ft round reaching into a gate's passage.)
    let in_gate = |p: [f64; 2], r: f64| {
        gates.iter().any(|&(g, h)| {
            let u = unit2([h[0] - g[0], h[1] - g[1]]);
            let len = crate::core::sqrt((h[0] - g[0]) * (h[0] - g[0]) + (h[1] - g[1]) * (h[1] - g[1]));
            let (dx, dy) = (p[0] - g[0], p[1] - g[1]);
            let (along, across) = (dx * u[0] + dy * u[1], -dx * u[1] + dy * u[0]);
            along > 12.0 - r && along < len - 12.0 + r && across.abs() < 14.0 + r
        })
    };
    // (Nor does anything in the way stand on a flight of steps.)
    let on_steps = |surface: &[Surface], o: &Object| info(o.kind as u16).blocks_move && surface[(o.y.max(0.0) as usize).min(SQ - 1) * SQ + (o.x.max(0.0) as usize).min(SQ - 1)] == Surface::Stairs;
    // (A gate's leaf stands in its wall's opening, its middle maybe on a raised square beside it.)
    let blocked = |c: &Chunk, o: &Object| (o.kind != Kind::Gate && on_wall(&c.urban, o)) || on_steps(&c.surface, o) || in_gate([ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT], (info(o.kind as u16).radius * o.scale) as f64 * SQUARE_FT);
    {
        let kept: Vec<bool> = chunk.objects.iter().map(|o| !blocked(&chunk, o)).collect();
        let mut it = kept.iter();
        chunk.objects.retain(|_| *it.next().unwrap());
    }
    // A close's churchyard and its hedge (a square round it): nothing added, no knoll over it.
    let churchyard: Vec<bool> = if churchyards.is_empty() {
        Vec::new()
    } else {
        (0..n)
            .map(|k| {
                let p = [ox + ((k % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((k / SQ) as f64 + 0.5) * SQUARE_FT];
                churchyards.iter().any(|y| crate::town::geom::contains(y, p) || (0..y.len()).any(|e| crate::town::geom::seg_dist(p, y[e], y[(e + 1) % y.len()]) < 1.5 * SQUARE_FT))
            })
            .collect()
    };
    let keep_off: Vec<bool> = (0..n)
        .map(|k| {
            let (x, y) = ((k % SQ) as i64, (k / SQ) as i64);
            let wall = |x: i64, y: i64| x >= 0 && y >= 0 && x < SQ as i64 && y < SQ as i64 && matches!(chunk.urban[y as usize * SQ + x as usize], URBAN_WALL | URBAN_TOWER | URBAN_TOWER_FLOOR);
            let p = [ox + (x as f64 + 0.5) * SQUARE_FT, oy + (y as f64 + 0.5) * SQUARE_FT];
            (-1..=1).any(|dy| (-1..=1).any(|dx| wall(x + dx, y + dy)))
                || in_gate(p, 2.5)
                || features.iter().any(|f| crate::town::geom::dist(p, f.0) < f.1 + 4.0)
                || churchyard.get(k).is_some_and(|&y| y)
                || court_h[(y as usize + 1) * HS + x as usize + 1]
                || chunk.urban[k] == URBAN_PATH
                || chunk.surface[k] == Surface::Stairs
                || bastion_tops.iter().any(|q| crate::town::geom::contains(q, p))
                || park_keep.iter().any(|&(a, b, r)| crate::town::geom::seg_dist(p, a, b) < r + 2.5)
                || bailey_kind[(y as usize + 1) * HS + x as usize + 1] & 4 != 0
                || (bailey_kind[(y as usize + 1) * HS + x as usize + 1] & 3 != 0 && bailey_ways.iter().any(|&(a, b)| crate::town::geom::seg_dist(p, a, b) < 8.0))
        })
        .collect();
    // (The baileys' squares, as `bailey_kind` has them; empty: none here.)
    let bailey: Vec<u8> = if bailey_kind.iter().all(|&v| v == 0) { Vec::new() } else { (0..n).map(|k| bailey_kind[(k / SQ + 1) * HS + k % SQ + 1]).collect() };
    // The parks' squares (empty: none here): the low bits 1 a common's, 2 a formal garden's (what
    // the guarantee puts down there is a park's); 4 by a pond or stream (no knoll: its banks stay
    // eased, by the band and 8 ft more); 8 gravel at a path's or round's edge, clear of features,
    // where cover may go when the lawn is out of reach (a big round's middle); 16 off the parks but
    // within 8 squares of one (no knoll or feature off a lawn there).
    let park: Vec<u8> = if park_h.iter().all(|&v| v == 0) {
        Vec::new()
    } else {
        let half = 0.5 * crate::park::STREAM_FT;
        let rmax: Vec<f64> = ponds.iter().map(|pd| pd.pts.iter().map(|&q| crate::town::geom::dist(q, pd.centre)).fold(0.0, f64::max)).collect();
        let mut marks: Vec<u8> = (0..n)
            .map(|k| {
                let kind = park_h[(k / SQ + 1) * HS + k % SQ + 1];
                if kind == 0 {
                    return 0;
                }
                let (x, y) = (k % SQ, k / SQ);
                let p = [ox + (x as f64 + 0.5) * SQUARE_FT, oy + (y as f64 + 0.5) * SQUARE_FT];
                let by_pond = ponds.iter().zip(&rmax).any(|(pd, &rmax)| {
                    let reach = pd.band + 8.0;
                    crate::town::geom::dist(p, pd.centre) < rmax + reach
                        && (crate::town::geom::contains(&pd.pts, p) || (0..pd.pts.len()).any(|e| crate::town::geom::seg_dist(p, pd.pts[e], pd.pts[(e + 1) % pd.pts.len()]) < reach))
                });
                let by_stream = streams.iter().any(|s| s.windows(2).any(|w| crate::town::geom::seg_dist(p, w[0].0, w[1].0) < half + crate::park::STREAM_BANK + 8.0));
                let lawn = |dx: i64, dy: i64| {
                    let (i, j) = (x as i64 + dx, y as i64 + dy);
                    i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && chunk.urban[j as usize * SQ + i as usize] == URBAN_PARK
                };
                let edge = chunk.urban[k] == URBAN_PATH
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| lawn(dx, dy))
                    && !features.iter().any(|f| crate::town::geom::dist(p, f.0) < f.1 + 4.0)
                    && !in_gate(p, 2.5);
                kind | if by_pond || by_stream { 4 } else { 0 } | if edge { 8 } else { 0 }
            })
            .collect();
        // (16: off the parks, within 8 squares of one, either way.)
        let near: Vec<bool> = (0..n)
            .map(|k| {
                let x = k % SQ;
                (x.saturating_sub(8)..=(x + 8).min(SQ - 1)).any(|i| marks[k - x + i] & 3 != 0)
            })
            .collect();
        for k in 0..n {
            let (x, y) = (k % SQ, k / SQ);
            if marks[k] & 3 == 0 && (y.saturating_sub(8)..=(y + 8).min(SQ - 1)).any(|j| near[j * SQ + x]) {
                marks[k] |= 16;
            }
        }
        marks
    };
    // Where the ground is shaped, the guarantee's draws are keyed by cell (a mound in one corner
    // moves nothing elsewhere) and plateaus get no knolls (`shaped`: empty when nothing is). Nor
    // does a cell that water drawn by hand reaches: its dry squares are not the ground's as
    // generated, so its draw would set the knoll elsewhere, out on the ground round the water.
    let shaped: Vec<bool> = if shaper.is_some() {
        (0..n)
            .map(|k| {
                let h = (k / SQ + 1) * HS + k % SQ + 1;
                plat_w[h] >= 0.5 || by_hand_water.get(h).is_some_and(|&w| w)
            })
            .collect()
    } else {
        Vec::new()
    };
    let before = if shaped.is_empty() { Vec::new() } else { chunk.height.clone() };
    // (Roadmap 6 R6: the eraser's gaps through hedges' and fences' pieces, before the guarantee
    // counts its cover.)
    crate::town::fence::gap_objects(&fence_gaps, ox, oy, &mut chunk.objects);
    let set_out = chunk.objects.len();
    chunk.yard = bailey.clone();
    // (The guarantee counts hedgerows and dry-stone walls as cover, and gives a village's green its
    // own: `Chunk::yard` bits 8 and 16.)
    if !rural.runs.is_empty() || green_h.iter().any(|&g| g != 0) {
        chunk.yard.resize(n, 0);
        crate::town::rural::cover(&rural, ox, oy, &mut chunk.yard);
        for k in 0..n {
            if green_h[(k / SQ + 1) * HS + k % SQ + 1] != 0 && chunk.urban[k] == URBAN_PLAZA {
                chunk.yard[k] |= 16;
            }
        }
    }
    enforce(&mut chunk, &biome, seed, gsx, gsy, sea, &shaped, &keep_off, &park, &bailey);
    // R5 towns (114): a steppe town's stone daises (its squares', a horse park's, the guarantee's
    // there) are banks of earth trodden by hooves (`PlazaFeature::Mound`).
    if steppe_any && steppe_all {
        for s in chunk.structures.iter_mut().filter(|s| s.kind == ShapeKind::Dais && s.size == PlazaFeature::Dais as u8 as f32) {
            s.size = PlazaFeature::Mound as u8 as f32;
        }
    }
    // A bench the guarantee put in a park is one of the park's, facing its nearest path, or the
    // middle of the gravel round it stands at; one out on the lawn, more than 12 ft from either, is a
    // shrub instead. Its flowerbeds lie square to a formal garden's axis.
    for o in chunk.objects[set_out..].iter_mut().filter(|o| matches!(o.kind, Kind::Bench | Kind::Flowerbed)) {
        let p = [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
        let Some(m) = park_meta.iter().find(|m| crate::town::geom::contains(&m.poly, p)) else { continue };
        if o.kind == Kind::Flowerbed {
            if let Some(a) = m.axis {
                // (The quarter turn of the axis nearest its own.)
                let q = std::f32::consts::FRAC_PI_2;
                o.rot = a + q * crate::core::round(((o.rot - a) / q) as f64) as f32;
            }
            continue;
        }
        o.variant = m.bench;
        let path = m.segs.iter().map(|&(a, b)| crate::park::project(a, b, p).0).map(|q| (crate::town::geom::dist(q, p), q));
        let round = m.rounds.iter().map(|&(c, r)| ((crate::town::geom::dist(c, p) - r).abs(), c));
        match path.chain(round).min_by(|a, b| a.0.total_cmp(&b.0)) {
            Some((d, q)) if d <= 12.0 => {
                if crate::town::geom::dist(q, p) > 1.0 {
                    o.rot = crate::park::facing(crate::town::geom::sub(q, p));
                }
            }
            _ => (o.kind, o.variant) = (Kind::Bush, o.variant % 4),
        }
    }
    // A path square the guarantee left at a step steeper than a cliff's grade (a knoll beside it)
    // is the park's lawn: the path stops at the edge (but at a raised feature's step).
    if !park.is_empty() {
        // (Raised features' rounds, centre and radius squared plus half a square, in squares.)
        let raised: Vec<([f32; 2], f32)> = chunk
            .structures
            .iter()
            .filter(|s| s.kind == ShapeKind::Dais)
            .map(|s| {
                let (dx, dy) = (s.pts[1][0] - s.pts[0][0], s.pts[1][1] - s.pts[0][1]);
                let r = crate::core::sqrt((dx * dx + dy * dy) as f64) as f32 + 0.5;
                (s.pts[0], r * r)
            })
            .collect();
        let feature = |k: usize| {
            let (x, y) = ((k % SQ) as f32 + 0.5, (k / SQ) as f32 + 0.5);
            raised.iter().any(|&(c, r2)| (x - c[0]) * (x - c[0]) + (y - c[1]) * (y - c[1]) <= r2)
        };
        let cut: Vec<usize> = (0..n)
            .filter(|&k| chunk.urban[k] == URBAN_PATH && !churchyard.get(k).is_some_and(|&y| y))
            .filter(|&k| {
                let (x, y) = (k % SQ, k / SQ);
                [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                    let (i, j) = (x as i64 + dx, y as i64 + dy);
                    i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 && {
                        let nk = j as usize * SQ + i as usize;
                        (chunk.height[nk] - chunk.height[k]).abs() > 2.75 && !feature(nk) && !feature(k)
                    }
                })
            })
            .collect();
        for k in cut {
            let h = (k / SQ + 1) * HS + k % SQ + 1;
            chunk.urban[k] = URBAN_PARK;
            chunk.surface[k] = match (park_h[h], park_ix[h]) {
                (2, _) => Surface::Grass,
                (_, ix) if ix != 0 => surface_for(park_biomes[ix as usize - 1], slope[k]),
                _ => chunk.surface[k],
            };
        }
    }
    // What it added in a park stands wholly on its lawn: one reaching over the edge (a statue over the
    // street) stands in the middle of its square, smaller, or is a bush there (the cover kept).
    let p = |o: &Object| [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
    let over = |o: &Object, poly: &[[f64; 2]]| crate::park::depth(poly, p(o)) < (info(o.kind as u16).radius * o.scale) as f64 * SQUARE_FT;
    for o in chunk.objects[set_out..].iter_mut().filter(|o| matches!(o.kind, Kind::GardenStatue | Kind::Topiary | Kind::Flowerbed | Kind::Bench)) {
        if let Some(m) = park_meta.iter().find(|m| crate::town::geom::contains(&m.poly, p(o)))
            && over(o, &m.poly)
        {
            (o.scale, o.x, o.y) = (0.85, o.x.floor() + 0.5, o.y.floor() + 0.5);
            if over(o, &m.poly) {
                (o.kind, o.variant) = (Kind::Bush, o.variant % 4);
            }
        }
    }
    // What it added that reaches a wall, a tower or a gate's passage (a cart by it) stands in the
    // middle of its square, smaller (a crate on paving), or goes.
    for k in (set_out..chunk.objects.len()).rev() {
        if !blocked(&chunk, &chunk.objects[k]) {
            continue;
        }
        let o = &mut chunk.objects[k];
        let q = (o.y.max(0.0) as usize).min(SQ - 1) * SQ + (o.x.max(0.0) as usize).min(SQ - 1);
        (o.scale, o.x, o.y) = (0.85, o.x.floor() + 0.5, o.y.floor() + 0.5);
        if chunk.surface[q].is_road() {
            o.kind = Kind::Crate;
        }
        if blocked(&chunk, &chunk.objects[k]) {
            chunk.objects.remove(k);
        }
    }
    // Enforcement knolls may have spilled onto a road bed or a building; those keep their
    // level (on a street or plaza a raised terrace or dais is fine), as does a plateau.
    let mut touched = false;
    for k in (0..shaped.len()).filter(|&k| shaped[k]) {
        if chunk.height[k] != before[k] {
            chunk.height[k] = before[k];
            touched = true;
        }
    }
    for k in 0..n {
        let h = (k / SQ + 1) * HS + k % SQ + 1;
        let bed = road_bed[h];
        if (chunk.road_halo[h] != 0 || chunk.building[k] != 0 || churchyard.get(k).is_some_and(|&y| y)) && chunk.height[k] != bed {
            chunk.height[k] = bed;
            touched = true;
        }
    }
    if touched {
        retier(&mut chunk, sea);
    }
    // Enforcement only edits the interior (≥ 7 squares from the edge); copy it back.
    for k in 0..n {
        chunk.halo_height[(k / SQ + 1) * HS + k % SQ + 1] = chunk.height[k];
    }
    // A dry ditch drawn shallower than it is dug (its tiers as dug: `moat::dig`).
    for (h, &d) in dug.soft.iter().enumerate() {
        if d != 0.0 {
            chunk.halo_height[h] += d;
            let (i, j) = (h % HS, h / HS);
            if (1..=SQ).contains(&i) && (1..=SQ).contains(&j) {
                chunk.height[(j - 1) * SQ + i - 1] += d;
            }
        }
    }
    // Nothing generated grows or stands on a dry ditch's floor (its sides and floor kept clear to read).
    chunk.objects.retain(|o| !dug.ditch[((o.y.max(0.0) as usize).min(SQ - 1) + 1) * HS + (o.x.max(0.0) as usize).min(SQ - 1) + 1]);
    apply_edits(&mut chunk, &world.file.edits, ox, oy);
    compute_edges(&mut chunk);
    crate::town::rural::mark(&rural, ox, oy, &mut chunk.edges);
    soft_relief::flag(&mut chunk.edges, &soft, &crag_h, &crag_m);
    for k in 0..n {
        let h = (k / SQ + 1) * HS + k % SQ + 1;
        // (A wet moat's water and its banks, as a park's pond's, v63 castles.)
        if park_wet[h] || dug.wet[h] {
            chunk.edges[k] |= 32;
        }
        // (A dry moat's ditch: difficult ground, its floor earth, v63 castles.)
        if dug.ditch[h] {
            chunk.edges[k] |= 16;
            if !chunk.surface[k].is_road() && chunk.building[k] == 0 && chunk.water_level[k] <= chunk.height[k] {
                chunk.surface[k] = Surface::Dirt;
            }
        }
    }
    chunk
}

/// Whether an object's footprint disc touches a square of the chunk for which `hit` holds.
fn touches_square(o: &Object, hit: impl Fn(usize) -> bool) -> bool {
    let r = info(o.kind as u16).radius * o.scale;
    let span = |v: f32| ((v - r).max(0.0) as usize, ((v + r).max(0.0) as usize).min(SQ - 1));
    let ((x0, x1), (y0, y1)) = (span(o.x), span(o.y));
    (y0..=y1).any(|y| {
        (x0..=x1).any(|x| {
            let (dx, dy) = (o.x - o.x.clamp(x as f32, x as f32 + 1.0), o.y - o.y.clamp(y as f32, y as f32 + 1.0));
            dx * dx + dy * dy < r * r && hit(y * SQ + x)
        })
    })
}

/// Whether an object's footprint touches a wall or tower square (it would be drawn over them).
fn on_wall(urban: &[u8], o: &Object) -> bool {
    touches_square(o, |k| matches!(urban[k], URBAN_WALL | URBAN_TOWER | URBAN_TOWER_FLOOR))
}

/// Objects put down and taken away by hand (`Edits::objects`, `Edits::cleared`): generated
/// objects matching a clear go, then those put down in this chunk (by their centre) are added.
/// A roof as packed: style (0 hip, 1 battlements, 2 cone) | (tint + 1) << 2 (0: picked).
fn roof_byte(b: &crate::town::Building, battlements: bool) -> u8 {
    let style = if battlements {
        1
    } else if b.roof == Some(crate::town::RoofStyle::Cone) {
        2
    } else {
        0
    };
    style | b.tint.map_or(0, |t| (t + 1) << 2)
}

/// A roof's look as packed (`town::roof::look`): material id | 0x80 a steppe home's yurt (R5 towns,
/// 115: its colour variant then its felt, `town::yurts::felt`), shape id | 0x10 smoke-darkened |
/// colour variant << 5, weathering 0-255.
fn look_bytes(l: &crate::town::Layout, b: &crate::town::Building) -> [u8; 3] {
    let k = crate::town::roof::look(l, b);
    let yurt = k.shape == crate::town::roof::Shape::Round && b.poly.len() == crate::town::roof::ROUND_SIDES && crate::town::yurts::is_steppe_home(l, b);
    [k.material as u8 | if yurt { 0x80 } else { 0 }, k.shape as u8 | if k.smoke { 0x10 } else { 0 } | k.shade << 5, k.wear]
}

fn apply_edits(c: &mut Chunk, e: &crate::world::Edits, ox: f64, oy: f64) {
    use crate::world::ObjKind;
    let chunk_ft = SQ as f64 * SQUARE_FT;
    if !e.cleared.is_empty() {
        let reach = |cl: &crate::world::Clear| cl.r.unwrap_or(0.25 * SQUARE_FT);
        let near: Vec<&crate::world::Clear> = e
            .cleared
            .values()
            .filter(|cl| {
                let r = reach(cl);
                cl.x + r >= ox && cl.x - r < ox + chunk_ft && cl.y + r >= oy && cl.y - r < oy + chunk_ft
            })
            .collect();
        if !near.is_empty() {
            c.objects.retain(|o| {
                let (x, y) = (ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT);
                let kind = o.kind as u16;
                !near.iter().any(|cl| {
                    let d2 = (x - cl.x) * (x - cl.x) + (y - cl.y) * (y - cl.y);
                    let r = reach(cl);
                    match cl.kind {
                        Some(k) => k == kind && d2 <= r * r,
                        None => cl.r.is_some() && d2 <= r * r && (cl.kinds.is_empty() || cl.kinds.contains(&kind)),
                    }
                })
            });
        }
    }
    for p in e.objects.values() {
        if p.x < ox || p.y < oy || p.x >= ox + chunk_ft || p.y >= oy + chunk_ft {
            continue;
        }
        let (x, y) = (((p.x - ox) / SQUARE_FT) as f32, ((p.y - oy) / SQUARE_FT) as f32);
        let scale = if p.scale.is_finite() { p.scale.clamp(0.1, 10.0) } else { 1.0 };
        let rot = if p.rot.is_finite() { p.rot } else { 0.0 };
        let mut o = Object { kind: Kind::Crate, variant: p.variant, x, y, rot, scale, sprite: 0, turned: p.turned };
        match &p.kind {
            ObjKind::Builtin(k) => match kind_of(*k) {
                Some(k) => o.kind = k,
                None => continue,
            },
            ObjKind::Sprite(_) => {
                let Some(id) = p.kind.sprite() else { continue };
                let at = match c.sprites.iter().position(|s| s.asset == id) {
                    Some(i) => i,
                    None => {
                        let m = e.sprites.get(id).cloned().unwrap_or_default();
                        let name = if m.name.is_empty() { "custom object".to_string() } else { m.name.clone() };
                        let info = KindInfo {
                            id: SPRITE_KIND + c.sprites.len() as u16,
                            name: "custom object",
                            radius: (m.size.clamp(0.1, 40.0) / 2.0),
                            blocks_move: m.blocks_move,
                            blocks_sight: m.blocks_sight,
                            cover: m.cover.min(3),
                            difficult: m.difficult,
                            height_ft: m.height_ft.max(0.0),
                            hazard: None,
                            feature: true,
                        };
                        c.sprites.push(ChunkSprite { asset: id.to_string(), name, info });
                        c.sprites.len() - 1
                    }
                };
                o.sprite = at as u16 + 1;
            }
        }
        c.objects.push(o);
    }
}

fn retier(c: &mut Chunk, sea: f64) {
    for k in 0..c.height.len() {
        c.tier[k] = crate::core::floor((c.height[k] as f64 - sea) / TIER_FT) as i16;
    }
}

/// The bailey squares scatter may use (empty `ok`: none here), with the ways and gates it keeps clear.
struct Yard<'a> {
    ok: &'a [bool],
    ways: &'a [([f64; 2], [f64; 2])],
    gates: &'a [([f64; 2], [f64; 2])],
    ox: f64,
    oy: f64,
}

/// v63 countryside: what the land's scatter keeps off besides (`place_objects`): a croft's or a
/// field's squares (`HS`² halo grid; their own trees are theirs), and for trees and boulders a verge
/// two squares wide by the roads and streets (`RN`² grid as `road_near`).
struct Keep<'a> {
    farm: &'a [bool],
    verge: &'a [bool],
}

/// A park as the guarantee's finishing touches need it: its outline, its benches' look, its paths
/// (segments) and gravel rounds, and a formal garden's axis.
struct ParkMeta {
    poly: Vec<[f64; 2]>,
    bench: u8,
    segs: Vec<([f64; 2], [f64; 2])>,
    rounds: Vec<([f64; 2], f64)>,
    axis: Option<f32>,
}
