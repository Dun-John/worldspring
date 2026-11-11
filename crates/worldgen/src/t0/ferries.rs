//! Ferries to near islands: each settled island that lies within `WATER_MAX_MI` of a bigger land
//! mass with roads (the mainland, or a bigger island, so chains work) gets one ferry, its
//! shortest crossing there. A crossing runs straight over the sea from a landing on one shore to
//! one on the other (where the water's contour crosses the line between two shore cells); the
//! cheapest by the water's miles (counted double) and the new road it needs on both sides, which
//! `roads::build` builds as tracks from each side's nearest road (or settlement) down to its
//! landing. The water itself is no road: no bed is carved for it and nothing routes along it;
//! the map draws it as a ferry's dashed line (`tile_lines`) and the battlemap a landing stage
//! with the ferry boat at each end.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::roads::{Inputs, Routed};
use super::settle::Settlement;
use crate::core::sqrt;

/// The most water a ferry crosses, shore to shore (miles), and the least ("about 4 miles": the
/// contour-to-contour crossings of islands four cells off run 4.2-4.7).
pub const WATER_MAX_MI: f64 = 4.75;
/// A crossing up to this long is taken before any longer one.
const WATER_SHORT_MI: f64 = 4.0;
const WATER_MIN_MI: f64 = 0.1;
/// A mile over the water counts this many of new road in choosing a crossing.
const WATER_WEIGHT: f64 = 2.0;
/// The longest a land leg may run (miles): where no road comes near a landing, its track runs on
/// to the nearest settlement (routed as roads are, by the ground's cost), up to this far.
const LEG_MAX_MI: f64 = 40.0;
/// A landing needs low ground behind it: within `LANDING_BACK_FT` and four times that inland
/// the ground stands at most this high over the sea (ft), not a cliff top over the water.
const LANDING_MAX_FT: f64 = 40.0;
const LANDING_BACK_FT: f64 = 150.0;
/// Where a land leg ends, back from its landing (ft): the road stops on dry ground at the shore.
pub const INLAND_FT: f64 = 12.0;
/// The water's contour is found along the line in steps of this (ft), then halved down.
const STEP_FT: f64 = 50.0;

/// A ferry over the sea: its landings where the water's contour meets each shore (world ft),
/// `a` on the island it serves, `b` on the bigger land. Its land legs end there; the finest
/// terrain's waterline (`shores`) may lie further out, over a shelf the contour leaves dry.
#[derive(Clone, Debug, Default)]
pub struct SeaFerry {
    pub a: [f64; 2],
    pub b: [f64; 2],
    /// `shores`, found once (from the finest terrain, so a pure function of the world).
    shore: std::sync::OnceLock<[[f64; 2]; 2]>,
}

impl PartialEq for SeaFerry {
    fn eq(&self, o: &Self) -> bool {
        self.a == o.a && self.b == o.b
    }
}

impl SeaFerry {
    pub fn new(a: [f64; 2], b: [f64; 2]) -> SeaFerry {
        SeaFerry { a, b, shore: Default::default() }
    }

    /// Its water's length (miles).
    pub fn mi(&self) -> f64 {
        dist(self.a, self.b) / 5280.0
    }

    /// Where the water begins out from each landing (`a`'s, `b`'s) on the finest terrain, as the
    /// battlemap reads it: the first battlemap square along the line, from the landing out to
    /// `SHORE_REACH_FT` (and never past half way), whose water stands above its ground; the
    /// landing itself where there is none. The same in every chunk and tile (found once per
    /// ferry, from the finest tiles along the line; the land legs' tracks run on to it and the
    /// stages stand there).
    pub fn shores(&self, world: &crate::world::World, t0: &super::T0) -> [[f64; 2]; 2] {
        *self.shore.get_or_init(|| [shore_out(world, t0, self.a, self.b), shore_out(world, t0, self.b, self.a)])
    }
}

/// How far out from a landing (ft) the finest terrain's waterline is looked for (a shelf may run
/// dry far past the water's contour).
pub const SHORE_REACH_FT: f64 = 1_500.0;

/// `SeaFerry::shores` from landing `p` towards `q`: walked in steps of half a battlemap square,
/// reading each square's centre sample (an odd sample of the finest level) from the tiles along
/// the way, each built from the coarsest level down.
fn shore_out(world: &crate::world::World, t0: &super::T0, p: [f64; 2], q: [f64; 2]) -> [f64; 2] {
    use crate::core::tile::{HALO, PADDED, TILE_N, TileKey};
    let g = &world.geom;
    let level = g.max_level;
    let s = g.spacing_ft(level);
    let square = 2.0 * s;
    let len = dist(p, q);
    if len <= 0.0 {
        return p;
    }
    let u = [(q[0] - p[0]) / len, (q[1] - p[1]) / len];
    let mut tiles: std::collections::BTreeMap<(u8, u32, u32), Vec<f32>> = Default::default();
    let reach = SHORE_REACH_FT.min(0.5 * len);
    for m in 0..=(reach / (0.5 * square)) as usize {
        let t = m as f64 * 0.5 * square;
        let c = [p[0] + u[0] * t, p[1] + u[1] * t];
        if c[0] < 0.0 || c[1] < 0.0 || c[0] >= g.map_w_ft || c[1] >= g.map_h_ft {
            break;
        }
        // The square's centre: lattice sample (2i + 1, 2j + 1) of the finest level.
        let (gi, gj) = (2 * crate::core::floor(c[0] / square) as i64 + 1, 2 * crate::core::floor(c[1] / square) as i64 + 1);
        let (tx, ty) = ((gi / TILE_N as i64) as u32, (gj / TILE_N as i64) as u32);
        for lv in 0..=level {
            let k = (lv, tx >> (level - lv), ty >> (level - lv));
            if !tiles.contains_key(&k) {
                let parent = lv.checked_sub(1).and_then(|pl| tiles.get(&(pl, tx >> (level - pl), ty >> (level - pl))));
                let made = crate::lod::terrain_refine::terrain_tile(world, t0, &TileKey::surface(lv, k.1, k.2), parent.map(|v| v.as_slice()));
                tiles.insert(k, made.padded.to_vec());
            }
        }
        let pad = &tiles[&(level, tx, ty)];
        let (li, lj) = ((gi - tx as i64 * TILE_N as i64) as usize + HALO, (gj - ty as i64 * TILE_N as i64) as usize + HALO);
        if t0.sample_water(gi as f64 * s, gj as f64 * s) > pad[lj * PADDED + li] {
            return c;
        }
    }
    p
}

/// A ferry and its land legs: per side, the cells from the road (or settlement) it leaves to the
/// shore and on to the next cell out, whose place the leg's end (ft, `INLAND_FT` back from the
/// landing) takes.
pub struct Planned {
    pub ferry: SeaFerry,
    pub legs: [(Vec<u32>, [f64; 2]); 2],
}

#[derive(PartialEq)]
struct Node(f64, u32);
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]))
}

/// The ferries `roads::build` builds, from the routes found (`paths`): per settled island at
/// most one, keyed by its cells alone (the islands in label order, cells in index order).
pub fn plan(inp: &Inputs, settlements: &[Settlement], paths: &[Routed]) -> Vec<Planned> {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    if !inp.world.file.params.generated_roads || settlements.is_empty() {
        return Vec::new();
    }
    let n = w * h;
    let masses = inp.masses();
    let of = &masses.of;
    let none = super::islands::NONE;
    // The roads and settlements each side's leg may leave from (not an isolated realm's).
    let mut net = vec![false; n];
    let realm = |c: u32| settlements.iter().any(|s| s.isolated && s.cell as u32 == c);
    for (_, cells, _) in paths.iter().filter(|p| p.1.first().is_some_and(|&c| !realm(c))) {
        for &c in cells {
            net[c as usize] = true;
        }
    }
    let mut settled = vec![false; masses.parts.len()];
    for s in settlements.iter().filter(|s| !s.isolated) {
        net[s.cell] = true;
        if of[s.cell] != none {
            settled[of[s.cell] as usize] = true;
        }
    }
    for s in settlements.iter().filter(|s| s.isolated) {
        net[s.cell] = false;
    }
    let mut roads = vec![false; masses.parts.len()];
    for (k, &on) in net.iter().enumerate() {
        if on && of[k] != none {
            roads[of[k] as usize] = true;
        }
    }
    // The cheapest new road from the nearest of them to each cell, and the way it goes.
    let cell_mi = cell / 5280.0;
    let cap = LEG_MAX_MI / cell_mi;
    let mut cost = vec![f64::INFINITY; n];
    // (Each cell's way's own length, cells: the cap is on that, not on the weighted cost.)
    let mut plain = vec![0.0f64; n];
    let mut parent = vec![u32::MAX; n];
    let mut heap = BinaryHeap::new();
    for (k, &on) in net.iter().enumerate() {
        if on {
            cost[k] = 0.0;
            heap.push(Node(0.0, k as u32));
        }
    }
    while let Some(Node(c, k)) = heap.pop() {
        let k = k as usize;
        if c > cost[k] {
            continue;
        }
        for (nb, d) in super::flood::neighbors(w, h, k) {
            if !super::roads::open_cell(inp, nb) {
                continue;
            }
            let nc = c + d * super::roads::step_cost(inp, k, nb, d);
            if plain[k] + d <= cap && nc < cost[nb] {
                cost[nb] = nc;
                plain[nb] = plain[k] + d;
                parent[nb] = k as u32;
                heap.push(Node(nc, nb as u32));
            }
        }
    }
    let at = |k: usize| [(k % w) as f64 * cell, (k / w) as f64 * cell];
    let shore = |k: usize| inp.land[k] && cost[k].is_finite() && super::flood::neighbors(w, h, k).any(|(nb, _)| !inp.land[nb]);
    let grid = crate::core::grid::Grid::from_vec(w, h, inp.hydro.water.clone());
    let seed = inp.world.stream("t0.biome.warp");
    let wet = |p: [f64; 2]| super::lake_at(&grid, cell, seed, p[0], p[1]).1 >= 0.5;
    // Where the water's contour crosses the line from shore cell `a` to `b`: the landings, if
    // the line is dry at both ends and at sea, unbroken, between.
    let crossing = |a: usize, b: usize| -> Option<([f64; 2], [f64; 2])> {
        let (pa, pb) = (at(a), at(b));
        let len = dist(pa, pb);
        let steps = crate::core::ceil(len / STEP_FT).max(2.0) as usize;
        let pt = |t: f64| [pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t];
        let ws: Vec<bool> = (0..=steps).map(|i| wet(pt(i as f64 / steps as f64))).collect();
        let (i0, i1) = (ws.iter().position(|&x| x)?, ws.iter().rposition(|&x| x)?);
        if i0 == 0 || i1 == steps || !ws[i0..=i1].iter().all(|&x| x) {
            return None;
        }
        // (At sea, not over a lake: the cell under the water's middle.)
        let mid = pt((i0 + i1) as f64 * 0.5 / steps as f64);
        let mk = (crate::core::round(mid[1] / cell) as usize).min(h - 1) * w + (crate::core::round(mid[0] / cell) as usize).min(w - 1);
        if inp.land[mk] {
            return None;
        }
        let edge = |dry: usize, sea: usize| {
            let (mut lo, mut hi) = (dry as f64 / steps as f64, sea as f64 / steps as f64);
            for _ in 0..10 {
                let m = 0.5 * (lo + hi);
                if wet(pt(m)) {
                    hi = m;
                } else {
                    lo = m;
                }
            }
            pt(lo)
        };
        Some((edge(i0 - 1, i0), edge(i1 + 1, i1)))
    };
    // (A landing on a cliff top: the ground just inland of it stands over `LANDING_MAX_FT`.)
    let sea = inp.world.params().sea_level_ft;
    let cliff = |p: [f64; 2], q: [f64; 2]| {
        let l = dist(p, q).max(1e-9);
        [LANDING_BACK_FT, 4.0 * LANDING_BACK_FT].iter().any(|&t| (inp.ground)(p[0] - (q[0] - p[0]) / l * t, p[1] - (q[1] - p[1]) / l * t) > sea + LANDING_MAX_FT)
    };
    let reach = crate::core::ceil(WATER_MAX_MI / cell_mi) as i64 + 2;
    // Each settled island's shore cells, by island.
    let mut by_island: Vec<Vec<usize>> = vec![Vec::new(); masses.parts.len()];
    for k in 0..n {
        let m = of[k];
        if m != none && m != masses.mainland && settled[m as usize] && shore(k) {
            by_island[m as usize].push(k);
        }
    }
    let mut out = Vec::new();
    for (isl, cells) in by_island.iter().enumerate() {
        let area = masses.parts[isl].area_mi2;
        // (Pairs by their least possible cost: the water at least the cells' distance less the
        // way from each centre to the shore.)
        let mut cand: Vec<(f64, usize, usize)> = Vec::new();
        for &a in cells {
            let (ax, ay) = ((a % w) as i64, (a / w) as i64);
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (bx, by) = (ax + dx, ay + dy);
                    if bx < 0 || by < 0 || bx >= w as i64 || by >= h as i64 {
                        continue;
                    }
                    let b = by as usize * w + bx as usize;
                    let m = of[b];
                    if m == none || m as usize == isl || !roads[m as usize] || masses.parts[m as usize].area_mi2 <= area || !shore(b) {
                        continue;
                    }
                    let gap = (sqrt((dx * dx + dy * dy) as f64) - 2.2).max(0.0) * cell_mi;
                    if gap > WATER_MAX_MI {
                        continue;
                    }
                    cand.push((WATER_WEIGHT * gap + (cost[a] + cost[b]) * cell_mi, a, b));
                }
            }
        }
        cand.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        // (A crossing within `WATER_SHORT_MI` if there is one, else the longer ones too.)
        let mut best: Option<(f64, usize, usize, [f64; 2], [f64; 2])> = None;
        for most in [WATER_SHORT_MI, WATER_MAX_MI] {
            for &(bound, a, b) in &cand {
                if best.as_ref().is_some_and(|v| bound >= v.0) {
                    break;
                }
                let Some((la, lb)) = crossing(a, b) else { continue };
                let water = dist(la, lb) / 5280.0;
                if !(WATER_MIN_MI..=most).contains(&water) || cliff(la, lb) || cliff(lb, la) {
                    continue;
                }
                let score = WATER_WEIGHT * water + (cost[a] + cost[b]) * cell_mi;
                if best.as_ref().is_none_or(|v| score < v.0) {
                    best = Some((score, a, b, la, lb));
                }
            }
            if best.is_some() {
                break;
            }
        }
        let Some((_, a, b, la, lb)) = best else { continue };
        let u = [(lb[0] - la[0]) / dist(la, lb), (lb[1] - la[1]) / dist(la, lb)];
        // Each leg: the way from the road to the shore cell, then the next cell out (the leg's
        // end takes its place, back from the landing).
        let leg = |k: usize, land: [f64; 2], out: [f64; 2]| {
            let mut v = vec![k as u32];
            let mut c = k;
            while parent[c] != u32::MAX {
                c = parent[c] as usize;
                v.push(c as u32);
            }
            v.reverse();
            let (x, y) = ((k % w) as i64 + crate::core::round(out[0]) as i64, (k / w) as i64 + crate::core::round(out[1]) as i64);
            let next = (y.clamp(0, h as i64 - 1) as usize) * w + x.clamp(0, w as i64 - 1) as usize;
            if next != k && !net[next] {
                v.push(next as u32);
            }
            (v, [land[0] - out[0] * INLAND_FT, land[1] - out[1] * INLAND_FT])
        };
        let legs = [leg(a, la, u), leg(b, lb, [-u[0], -u[1]])];
        // (Each leg its own end: two ending in one cell would run on into each other.)
        let last = |l: &(Vec<u32>, [f64; 2])| *l.0.last().expect("a leg");
        if legs.iter().any(|l| l.0.len() < 2) || last(&legs[0]) == last(&legs[1]) || out.iter().any(|p: &Planned| p.legs.iter().any(|l| legs.iter().any(|m| last(l) == last(m)))) {
            continue;
        }
        out.push(Planned { ferry: SeaFerry::new(la, lb), legs });
    }
    out
}

/// The ferries' lines over the sea near a rectangle (world ft), as the tile payload's road lines
/// (x, y, width, class 5: a ferry's dashed line): points every `step` ft along each from its
/// island's end, so every tile of a level cuts them alike. Only at `fine` levels (from the
/// payload's `FERRY_MIN_LEVEL`, L10, as river ferries): each line runs between the stages where
/// the water begins (`SeaFerry::shores`), and each land leg's track (class 2) runs on over the
/// shelf to its stage. Coarser levels show the overlay's line instead (`overlay_entries`).
pub fn tile_lines(world: &crate::world::World, t0: &super::T0, rect: [f64; 4], step: f64, fine: bool) -> Vec<Vec<[f64; 4]>> {
    if !fine {
        return Vec::new();
    }
    let width = super::roads::RoadClass::Track.width_ft();
    let pad = step + if fine { SHORE_REACH_FT } else { 0.0 };
    let mut out = Vec::new();
    for f in t0.ferries.iter().filter(|f| f.a[0].max(f.b[0]) + pad >= rect[0] && f.a[0].min(f.b[0]) - pad <= rect[2] && f.a[1].max(f.b[1]) + pad >= rect[1] && f.a[1].min(f.b[1]) - pad <= rect[3]) {
        let [a, b] = if fine { f.shores(world, t0) } else { [f.a, f.b] };
        let len = dist(a, b);
        if len <= 0.0 {
            continue;
        }
        let m = crate::core::ceil(len / step).max(1.0) as usize;
        let mut v: Vec<[f64; 4]> = (0..m).map(|i| i as f64 * step).map(|s| [a[0] + (b[0] - a[0]) * s / len, a[1] + (b[1] - a[1]) * s / len, width, 5.0]).collect();
        v.push([b[0], b[1], width, 5.0]);
        out.push(v);
        if fine {
            // (From the leg's end, `INLAND_FT` back from its landing, to its stage.)
            for (p, w) in [(f.a, a), (f.b, b)] {
                let u = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
                let u = if p == f.a { u } else { [-u[0], -u[1]] };
                let s = [p[0] - u[0] * INLAND_FT, p[1] - u[1] * INLAND_FT];
                let e = [w[0] - u[0] * 3.0, w[1] - u[1] * 3.0];
                if (e[0] - s[0]) * u[0] + (e[1] - s[1]) * u[1] > INLAND_FT {
                    out.push(vec![[s[0], s[1], width, 2.0], [e[0], e[1], width, 2.0]]);
                }
            }
        }
    }
    out
}

/// The ferries for the overlay's `sea_lanes` (`T0::overlay_json`), as lanes' entries joined by
/// commas: `c` 3 (a ferry), `f` 0, `e` the settlement each end serves (the nearest on its own land
/// within `LEG_MAX_MI` of its landing, -1 if none), `p` the water leg, landing to landing (whole ft;
/// the stages stand at most `SHORE_REACH_FT` further out, which the tiles show from L10).
pub fn overlay_entries(t0: &super::T0) -> String {
    use std::fmt::Write;
    if t0.ferries.is_empty() {
        return String::new();
    }
    use super::biome::Biome;
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let b = |k: usize| Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
    let land: Vec<bool> = (0..w * h).map(|k| b(k) != Biome::Ocean).collect();
    let lake: Vec<u32> = (0..w * h).map(|k| if b(k) == Biome::Lake { 0 } else { super::hydro::NO_LAKE }).collect();
    let isl = super::islands::label(w, h, cell, &land, &lake, &t0.biome.data);
    // (A landing's land: the land cell nearest it, half a cell back from the water.)
    let serves = |p: [f64; 2], q: [f64; 2]| -> i64 {
        let l = dist(p, q).max(1e-9);
        let back = [p[0] - (q[0] - p[0]) / l * 0.5 * cell, p[1] - (q[1] - p[1]) / l * 0.5 * cell];
        let (cx, cy) = (crate::core::round(back[0] / cell) as i64, crate::core::round(back[1] / cell) as i64);
        let mut best: Option<(f64, usize)> = None;
        for dy in -2..=2i64 {
            for dx in -2..=2i64 {
                let (x, y) = (cx + dx, cy + dy);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && isl.of[y as usize * w + x as usize] != super::islands::NONE {
                    let d = dist([x as f64 * cell, y as f64 * cell], back);
                    let k = y as usize * w + x as usize;
                    if best.is_none_or(|v| d < v.0) {
                        best = Some((d, k));
                    }
                }
            }
        }
        let Some((_, k)) = best else { return -1 };
        let mass = isl.of[k];
        t0.settlements
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.isolated && isl.of[s.cell] == mass && dist([s.x, s.y], p) <= LEG_MAX_MI * 5280.0)
            .min_by(|x, y| dist([x.1.x, x.1.y], p).total_cmp(&dist([y.1.x, y.1.y], p)).then(x.0.cmp(&y.0)))
            .map_or(-1, |(i, _)| i as i64)
    };
    let mut out = String::new();
    for (i, f) in t0.ferries.iter().enumerate() {
        let _ = write!(out, "{}{{\"c\":3,\"f\":0,\"e\":[{},{}],\"p\":[{},{},{},{}]}}", if i > 0 { "," } else { "" }, serves(f.a, f.b), serves(f.b, f.a), crate::core::round(f.a[0]) as i64, crate::core::round(f.a[1]) as i64, crate::core::round(f.b[0]) as i64, crate::core::round(f.b[1]) as i64);
    }
    out
}
