//! (Roadmap 7, 189) A town's street pattern: what its planned part (`town::planning`) is laid
//! out on. Grid (the lattice turned to the chief road, as since 161), Rings (ring streets round
//! the core, radial lanes between them), Radial (main streets fanning out from the market square,
//! little cross-linking) or Linear (one long high street with back lanes). Set by hand in
//! `Edits.patterns` (`<layout>` → name), else picked (`auto`) from the town's land and culture.
//! Linear runs its high street along the town's long axis (`long_axis`); Rings round a bay are
//! harbour arcs (`harbour`).

use super::{Site, Tier};
use super::geom::{P, add, dist, len, mul, seg_dist};
use crate::World;
use crate::core::rng::{hash2, unit};
use crate::t0::T0;
use crate::t0::settle::Settlement;
use crate::world::Edits;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pattern {
    #[default]
    Grid,
    Rings,
    Radial,
    Linear,
}

/// The patterns by code (`Pattern::code`), with the names `Edits.patterns` keeps.
pub const NAMES: [&str; 4] = ["grid", "rings", "radial", "linear"];

impl Pattern {
    pub fn code(self) -> u8 {
        self as u8
    }
    pub fn from_code(c: u8) -> Pattern {
        match c {
            1 => Pattern::Rings,
            2 => Pattern::Radial,
            3 => Pattern::Linear,
            _ => Pattern::Grid,
        }
    }
    pub fn name(self) -> &'static str {
        NAMES[self as usize]
    }
    pub fn parse(s: &str) -> Option<Pattern> {
        NAMES.iter().position(|n| n.eq_ignore_ascii_case(s.trim())).map(|c| Pattern::from_code(c as u8))
    }
    /// Laid out on rings round the middle (Rings, Radial), not on a lattice.
    pub fn polar(self) -> bool {
        matches!(self, Pattern::Rings | Pattern::Radial)
    }
}

/// The pattern `edits` set for one of the world's own settlements, if any.
pub fn override_in(edits: &Edits, t0: &T0, index: usize) -> Option<Pattern> {
    if edits.patterns.is_empty() || index >= t0.settlements.len() {
        return None;
    }
    edits.patterns.get(&index.to_string()).and_then(|v| Pattern::parse(v))
}

/// A mark of the pattern set for layout `index` (0: none, auto), mixed into the caches' hashes.
pub fn mark(world: &World, t0: &T0, index: usize) -> u64 {
    override_in(&world.file.edits, t0, index).map_or(0, |p| p.code() as u64 + 1)
}

/// The pattern settlement `s` (urban radius `r`, planned `planning`, 0-1) picks from its land
/// and culture, deterministic from its seed. A grown town (no planned part) stays on the grid,
/// as before 189 (its plan untouched). Otherwise: rings round a hill top it stands on (mostly),
/// rings round a city's keep in the north and west and round a steppe camp (some), radial for
/// some southern and dry cities and capitals; the grid else.
pub(super) fn auto(site: &Site, s: &Settlement, r: f64, planning: f64) -> Pattern {
    if planning < super::planning::MIXED {
        return Pattern::Grid;
    }
    let u = unit(hash2(s.seed, 0x7a77, 189));
    // (On a hill top: the middle 20 ft or more above the ground round it, everywhere.)
    let h0 = site.height([0.0, 0.0]);
    let ring: Vec<f64> = (0..8)
        .map(|k| {
            let a = k as f64 * std::f64::consts::FRAC_PI_4;
            site.height([0.8 * r * libm::cos(a), 0.8 * r * libm::sin(a)])
        })
        .collect();
    let mean = ring.iter().sum::<f64>() / 8.0;
    let hill = h0 - mean >= 20.0 && ring.iter().all(|&h| h < h0 - 5.0);
    if hill && u < 0.75 {
        return Pattern::Rings;
    }
    // (A valley floor, a ridge line or a hillside: mostly a high street along it; a river bank:
    // some a high street along the river; a bay: some harbour arcs round it.)
    let w = unit(hash2(s.seed, 0x7a79, 189));
    // (Market towns, not the great cities: a city only along a narrow valley or ridge.)
    if let Some((_, land)) = long_axis(site, r).filter(|_| s.tier <= Tier::City) {
        if land && w < 0.6 {
            return Pattern::Linear;
        }
        if !land && s.tier <= Tier::Town && w < 0.35 {
            return Pattern::Linear;
        }
    }
    if harbour(site, r).is_some() && w < 0.4 {
        return Pattern::Rings;
    }
    let v = unit(hash2(s.seed, 0x7a78, 189));
    match s.culture % 5 {
        0 | 1 if s.tier >= Tier::City && v < 0.3 => Pattern::Rings,
        2 | 3 if (s.tier >= Tier::City || s.capital) && v < 0.35 => Pattern::Radial,
        4 if v < 0.3 => Pattern::Rings,
        _ => Pattern::Grid,
    }
}

/// (189) The long axis of the ground settlement `site` (urban radius `r`) stands on, if it has
/// one: along a valley floor, a ridge line or a hillside's contour (the ground 24 ft or more
/// higher or lower across it, both ways at 0.7 r added, and along it under half that: `true`),
/// else along the river it stands by (within 0.9 r: `false`). Radians, half turn.
pub(super) fn long_axis(site: &Site, r: f64) -> Option<(f64, bool)> {
    let h0 = site.height([0.0, 0.0]);
    let d = 0.7 * r;
    let rise = |a: f64| [d, -d].map(|t| site.height([t * libm::cos(a), t * libm::sin(a)]) - h0);
    let mut best: Option<(f64, f64)> = None;
    for k in 0..8 {
        let a = k as f64 * std::f64::consts::PI / 8.0;
        let (al, ac) = (rise(a), rise(a + std::f64::consts::FRAC_PI_2));
        let (along, across) = (al[0].abs() + al[1].abs(), ac[0].abs() + ac[1].abs());
        if across >= 24.0 && along <= 0.5 * across && best.is_none_or(|b| across - along > b.1) {
            best = Some((a, across - along));
        }
    }
    if let Some((a, _)) = best {
        return Some((a, true));
    }
    let (a, b, _) = site.river.iter().copied().filter(|&(a, b, _)| seg_dist([0.0, 0.0], a, b) < 0.9 * r && dist(a, b) > 1.0).min_by(|x, y| {
        seg_dist([0.0, 0.0], x.0, x.1).total_cmp(&seg_dist([0.0, 0.0], y.0, y.1))
    })?;
    Some((libm::atan2(b[1] - a[1], b[0] - a[0]), false))
}

/// (189) The harbour a bay town's rings arc round, if it stands on one: standing water on a
/// fifth to three fifths of the way round it at 0.8 r, all in one stretch. The middle of the
/// shore there (where the water begins, from the town's middle out towards it), a little out
/// on the water.
pub(super) fn harbour(site: &Site, r: f64) -> Option<P> {
    const N: usize = 16;
    let dirs: Vec<P> = (0..N).map(|k| {
        let a = k as f64 * std::f64::consts::TAU / N as f64;
        [libm::cos(a), libm::sin(a)]
    }).collect();
    let wet: Vec<bool> = dirs.iter().map(|&u| site.wet_standing(mul(u, 0.8 * r))).collect();
    let n = wet.iter().filter(|&&w| w).count();
    let runs = (0..N).filter(|&k| wet[k] && !wet[(k + N - 1) % N]).count();
    if !(N / 5..=3 * N / 5).contains(&n) || runs != 1 || site.wet_standing([0.0, 0.0]) {
        return None;
    }
    let mut u = [0.0, 0.0];
    for k in (0..N).filter(|&k| wet[k]) {
        u = add(u, dirs[k]);
    }
    if len(u) < 1e-6 {
        return None;
    }
    let u = mul(u, 1.0 / len(u));
    let mut t = 0.0;
    while t < 0.8 * r && !site.wet_standing(mul(u, t)) {
        t += 10.0;
    }
    Some(mul(u, t + 0.1 * r))
}

/// What `get_town_plan` and `get_feature` say of town layout `index`'s pattern: the one it is
/// laid out on (`resolved`: `Layout::pattern`) and the one set by hand, if any.
pub fn to_json(edits: &Edits, t0: &T0, index: usize, resolved: u8) -> serde_json::Value {
    let set = override_in(edits, t0, index);
    serde_json::json!({ "pattern": Pattern::from_code(resolved).name(), "set": set.map(|p| p.name()), "auto": set.is_none() })
}

/// What setting a town's pattern did (`change_own`).
#[derive(Debug, Clone, PartialEq)]
pub struct Changed {
    /// The pattern set before and now (`None`: auto).
    pub from: Option<Pattern>,
    pub to: Option<Pattern>,
    pub unchanged: bool,
    /// The edits inside it set aside, by field (those with any).
    pub aside: Vec<(&'static str, usize)>,
}

impl Changed {
    pub fn to_json(&self) -> serde_json::Value {
        let by: serde_json::Map<String, serde_json::Value> = self.aside.iter().map(|&(f, n)| (f.to_string(), n.into())).collect();
        let n: usize = self.aside.iter().map(|a| a.1).sum();
        let f = |p: Option<Pattern>| p.map_or("auto", |p| p.name());
        serde_json::json!({ "from": f(self.from), "to": f(self.to), "unchanged": self.unchanged, "set_aside": n, "set_aside_by": by })
    }
}

/// Set the pattern of one of the world's own towns, layout `index`, to `to` (`None`: auto) in
/// `edits`. It is laid out anew, so the edits keyed to its layout are set aside, as a planning
/// change does.
pub fn change_own(edits: &mut Edits, index: usize, to: Option<Pattern>) -> Changed {
    let key = index.to_string();
    let from = edits.patterns.get(&key).and_then(|v| Pattern::parse(v));
    match to {
        Some(p) => edits.patterns.insert(key, p.name().to_string()),
        None => edits.patterns.remove(&key),
    };
    if from == to {
        return Changed { from, to, unchanged: true, aside: Vec::new() };
    }
    let aside = crate::t0::made::set_aside_by_field(edits, index).into_iter().filter(|a| a.1 > 0).collect();
    Changed { from, to, unchanged: false, aside }
}
