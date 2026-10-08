//! Hydrology on the T0 grid: lakes, water balance, rivers.
//!
//! - Depressions from priority-flood become lakes when large and deep enough, or drawn (the
//!   sketch's lakes, carved before); small ones are filled, so every other land cell drains
//!   downhill to the sea or a lake.
//! - Runoff (precipitation minus evapotranspiration) is accumulated downstream. Open water
//!   evaporates, so a lake in a dry basin can lose all its inflow: an endorheic salt lake or,
//!   when very dry, a salt flat. Desert streams lose water and fade out.
//! - Rivers are cells above a discharge threshold, split at confluences and joined into
//!   named chains along the largest-discharge path. By construction every river cell is
//!   strictly lower than the cell upstream of it.

use serde::Serialize;

use super::climate::Climate;
use super::flood::{neighbors, priority_flood, receivers};

/// Water surface value for dry ground.
pub const DRY: f32 = -30_000.0;
pub const NO_LAKE: u32 = u32::MAX;

/// Discharge (mm·cells) at which a stream is mapped as a river, before `river_density`.
pub const RIVER_Q: f64 = 90_000.0;
pub(crate) const MIN_LAKE_CELLS: usize = 12;
const MIN_LAKE_DEPTH_FT: f64 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LakeKind {
    Fresh,
    /// Endorheic: evaporation matches inflow, no outlet.
    Salt,
    /// Evaporation exceeds inflow: dry most of the year.
    SaltFlat,
}

#[derive(Clone, Debug)]
pub struct Lake {
    pub level_ft: f64,
    pub cells: Vec<u32>,
    pub kind: LakeKind,
    pub max_depth_ft: f64,
    /// The sketch stroke that drew it.
    pub stroke: Option<u32>,
}

/// What the sketch draws: per cell, the drawn lake it is in (`NO_LAKE` if none; may be empty);
/// per drawn lake, its stroke, whether it is salt and whether a river is drawn out of it; the
/// cells of drawn rivers' courses (may be empty).
#[derive(Clone, Copy)]
pub struct Drawn<'a> {
    pub of: &'a [u32],
    pub lakes: &'a [(u32, bool, bool)],
    pub courses: &'a [bool],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mouth {
    Ocean,
    Lake,
    /// Lost to evaporation / infiltration (desert wadis).
    Dry,
    /// Joins another river chain.
    Confluence,
}

#[derive(Clone, Debug)]
pub struct River {
    /// Cells from source to mouth (the mouth cell may be ocean or lake).
    pub cells: Vec<u32>,
    /// Discharge at each cell (mm·cells).
    pub q: Vec<f32>,
    pub mouth: Mouth,
    /// Chain this one flows into, for tributaries.
    pub into: Option<usize>,
}

pub struct Hydro {
    /// Water surface elevation per cell (sea level, lake level) or `DRY`.
    pub water: Vec<f32>,
    pub discharge: Vec<f32>,
    pub lake_of: Vec<u32>,
    pub lakes: Vec<Lake>,
    pub rivers: Vec<River>,
}

/// `feed`: extra discharge entering at cells (sketched rivers' sources); `drawn`: the sketch's
/// lakes (always lakes; a salt one has no outflow, one a river is drawn out of is fresh, and
/// none dries to a salt flat) and rivers (which lose no water in dry country).
#[allow(clippy::too_many_arguments)]
pub fn build(w: usize, h: usize, height: &mut [f64], land: &[bool], clim: &Climate, sea: f64, river_density: f64, feed: &[(usize, f64)], drawn: Drawn) -> Hydro {
    let n = w * h;
    let outlet: Vec<bool> = land.iter().map(|l| !l).collect();
    let fl = priority_flood(w, h, height, &outlet, 0.01);

    // Lakes: connected depressions that are big and deep enough. Everything else is filled.
    let mut lake_of = vec![NO_LAKE; n];
    let mut seen = vec![false; n];
    let mut lakes: Vec<Lake> = Vec::new();
    for start in 0..n {
        if seen[start] || !land[start] || fl.filled[start] - height[start] <= 1.0 {
            continue;
        }
        let mut comp = vec![start as u32];
        seen[start] = true;
        let mut k = 0;
        while k < comp.len() {
            let c = comp[k] as usize;
            k += 1;
            for (nb, _) in neighbors(w, h, c) {
                if !seen[nb] && land[nb] && fl.filled[nb] - height[nb] > 1.0 {
                    seen[nb] = true;
                    comp.push(nb as u32);
                }
            }
        }
        let depth = comp.iter().map(|&c| fl.filled[c as usize] - height[c as usize]).fold(0.0, f64::max);
        let drawn_as = comp.iter().find_map(|&c| drawn.of.get(c as usize).copied().filter(|&l| l != NO_LAKE));
        if drawn_as.is_some() || (comp.len() >= MIN_LAKE_CELLS && depth >= MIN_LAKE_DEPTH_FT) {
            let level = comp.iter().map(|&c| fl.filled[c as usize]).fold(f64::INFINITY, f64::min);
            let id = lakes.len() as u32;
            for &c in &comp {
                lake_of[c as usize] = id;
            }
            lakes.push(Lake { level_ft: level, cells: comp, kind: LakeKind::Fresh, max_depth_ft: depth, stroke: drawn_as.map(|l| drawn.lakes[l as usize].0) });
        }
    }
    // Fill every non-lake cell to the flooded surface: guarantees strictly downhill drainage.
    for i in 0..n {
        if land[i] && lake_of[i] == NO_LAKE {
            height[i] = fl.filled[i];
        }
    }

    let (rec, _) = receivers(w, h, &fl.filled);

    // Water balance. `raw` ignores losses (catchment supply); `q` includes them.
    let pet: Vec<f64> = clim.temp.iter().map(|&t| (350.0 + 55.0 * t as f64).max(0.0)).collect();
    let salt = |l: &Lake| l.stroke.is_some_and(|st| drawn.lakes.iter().any(|d| d.0 == st && d.1));
    let drains = |l: &Lake| l.stroke.is_some_and(|st| drawn.lakes.iter().any(|d| d.0 == st && d.2));
    let closed: Vec<bool> = lakes.iter().map(salt).collect();
    let mut q = vec![0.0f64; n];
    let mut raw = vec![0.0f64; n];
    for &(k, v) in feed {
        q[k] += v;
        raw[k] += v;
    }
    for &i in fl.order.iter().rev() {
        let i = i as usize;
        let p = clim.precip[i] as f64;
        let runoff = (p - 0.65 * pet[i]).max(0.0);
        q[i] += runoff;
        raw[i] += runoff;
        if lake_of[i] != NO_LAKE {
            // (A drawn salt lake loses all its water.)
            q[i] = if closed[lake_of[i] as usize] { 0.0 } else { (q[i] - pet[i]).max(0.0) };
        } else if p < 400.0 && !drawn.courses.get(i).is_some_and(|&c| c) {
            q[i] *= 0.985; // transmission loss in dry country
        }
        let r = rec[i] as usize;
        if r != i && land[r] {
            q[r] += q[i];
            raw[r] += raw[i];
        }
    }

    // Classify lakes by their water balance at the exit cell.
    for lake in &mut lakes {
        let exit = lake.cells.iter().map(|&c| c as usize).max_by(|&a, &b| raw[a].total_cmp(&raw[b]).then(a.cmp(&b))).unwrap();
        let evap: f64 = lake.cells.iter().map(|&c| pet[c as usize]).sum();
        let ratio = raw[exit] / evap.max(1.0);
        // Only dry climates make salt lakes; elsewhere small closed basins stay fresh
        // (groundwater seepage), like glacial kettle ponds.
        let precip = lake.cells.iter().map(|&c| clim.precip[c as usize] as f64).sum::<f64>() / lake.cells.len() as f64;
        lake.kind = if salt(lake) {
            LakeKind::Salt
        } else if precip > 550.0 || drains(lake) {
            LakeKind::Fresh
        } else if ratio < 0.35 {
            // (A drawn lake keeps its water.)
            if lake.stroke.is_some() { LakeKind::Salt } else { LakeKind::SaltFlat }
        } else if q[exit] <= 0.0 {
            LakeKind::Salt
        } else {
            LakeKind::Fresh
        };
    }

    // Water surface: sea and lakes (salt flats are dry). Tiles interpolate it over wet
    // corners only (`T0::sample_water`), so shorelines stay clean without dilation.
    let mut water = vec![DRY; n];
    for i in 0..n {
        if !land[i] {
            water[i] = sea as f32;
        }
    }
    for lake in &lakes {
        if lake.kind != LakeKind::SaltFlat {
            for &c in &lake.cells {
                water[c as usize] = lake.level_ft as f32;
            }
        }
    }

    let rivers = extract_rivers(w, h, land, &lake_of, &rec, &q, RIVER_Q / river_density);
    Hydro { water, discharge: q.iter().map(|&v| v as f32).collect(), lake_of, lakes, rivers }
}

fn extract_rivers(w: usize, h: usize, land: &[bool], lake_of: &[u32], rec: &[u32], q: &[f64], threshold: f64) -> Vec<River> {
    let n = w * h;
    let is_river = |i: usize| land[i] && lake_of[i] == NO_LAKE && q[i] >= threshold;
    let mut donors = vec![0u8; n];
    for i in 0..n {
        let r = rec[i] as usize;
        if is_river(i) && r != i {
            donors[r] = donors[r].saturating_add(1);
        }
    }
    let is_start = |i: usize| is_river(i) && donors[i] != 1;

    // Segments run from a source or confluence to the next confluence or a mouth.
    struct Seg {
        cells: Vec<u32>,
        mouth: Mouth,
    }
    let mut segs: Vec<Seg> = Vec::new();
    let mut seg_starting_at = vec![usize::MAX; n];
    for start in 0..n {
        if !is_start(start) {
            continue;
        }
        let mut cells = vec![start as u32];
        let mut cur = start;
        let mouth = loop {
            let nxt = rec[cur] as usize;
            if nxt == cur {
                break Mouth::Dry;
            }
            if !land[nxt] {
                cells.push(nxt as u32);
                break Mouth::Ocean;
            }
            if lake_of[nxt] != NO_LAKE {
                cells.push(nxt as u32);
                break Mouth::Lake;
            }
            if !is_river(nxt) {
                break Mouth::Dry;
            }
            cells.push(nxt as u32);
            if is_start(nxt) {
                break Mouth::Confluence;
            }
            cur = nxt;
        };
        seg_starting_at[start] = segs.len();
        segs.push(Seg { cells, mouth });
    }

    // Tree: a segment ending at a confluence flows into the segment starting there.
    let parent: Vec<Option<usize>> = segs
        .iter()
        .map(|s| (s.mouth == Mouth::Confluence).then(|| seg_starting_at[*s.cells.last().unwrap() as usize]))
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); segs.len()];
    for (k, p) in parent.iter().enumerate() {
        if let Some(p) = p {
            children[*p].push(k);
        }
    }
    let inflow = |k: usize| -> f64 {
        let c = &segs[k].cells;
        q[c[c.len().saturating_sub(2)] as usize]
    };

    // Chains: from each root walk upstream along the largest tributary; other tributaries
    // start their own chains flowing into this one.
    let mut chains: Vec<River> = Vec::new();
    let mut stack: Vec<(usize, Option<usize>)> =
        (0..segs.len()).filter(|&k| parent[k].is_none()).map(|k| (k, None)).collect();
    stack.sort_by(|a, b| inflow(b.0).total_cmp(&inflow(a.0)).then(a.0.cmp(&b.0)));
    stack.reverse();
    while let Some((root, into)) = stack.pop() {
        let mut path = vec![root];
        let mut cur = root;
        loop {
            let mut kids = children[cur].clone();
            if kids.is_empty() {
                break;
            }
            kids.sort_by(|a, b| inflow(*b).total_cmp(&inflow(*a)).then(a.cmp(b)));
            cur = kids[0];
            path.push(cur);
        }
        let chain_id = chains.len();
        // Cells from source (last in path) down to the root's mouth; confluence cells shared.
        let mut cells: Vec<u32> = Vec::new();
        for &k in path.iter().rev() {
            let c = &segs[k].cells;
            let skip = usize::from(!cells.is_empty());
            cells.extend_from_slice(&c[skip..]);
        }
        let mouth = if into.is_some() { Mouth::Confluence } else { segs[root].mouth };
        let qv = cells.iter().map(|&c| q[c as usize] as f32).collect();
        chains.push(River { cells, q: qv, mouth, into });
        for &k in &path {
            let main_child = path.iter().position(|&p| p == k).and_then(|i| path.get(i + 1)).copied();
            for &kid in &children[k] {
                if Some(kid) != main_child {
                    stack.push((kid, Some(chain_id)));
                }
            }
        }
    }
    // Chains are indexed by `into`, so short ones are kept here and filtered by consumers.
    chains
}
